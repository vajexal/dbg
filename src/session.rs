use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::process;
use std::rc::Rc;

use crate::breakpoint::Breakpoint;
use crate::context::Context;
use crate::error::DebuggerError;
use crate::loc_finder::{LocFinder, VarRef};
use crate::location::{TypedValueLoc, ValueLoc};
use crate::trap::Trap;
use crate::types::{Type, TypeStorage};
use crate::unwinder::Unwinder;
use crate::utils::WORD_SIZE;
use crate::var::{Operator, Value, Var};

use anyhow::{anyhow, bail, Result};
use bytes::{Buf, Bytes};
use nix::sys::{ptrace, wait};
use nix::unistd::Pid;

const READ_MEM_BUF_SIZE: usize = 512;
const FUNC_PROLOGUE_MAGIC_BYTES: [u8; 8] = [0xf3, 0x0f, 0x1e, 0xfa, 0x55, 0x48, 0x89, 0xe5];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionState {
    Started,
    Running,
    Exited,
}

pub struct DebugSession<R: gimli::Reader> {
    state: Cell<SessionState>,
    dwarf: gimli::Dwarf<R>,
    unwinder: Unwinder<R>,
    loc_finder: LocFinder<R>,
    type_storage: TypeStorage,
    child: process::Child,
    base_address: u64,
    breakpoints: HashMap<u64, Breakpoint>,
    traps: RefCell<HashMap<u64, Trap>>,
}

impl<R: gimli::Reader> DebugSession<R> {
    pub fn new(
        child: process::Child,
        dwarf: gimli::Dwarf<R>,
        loc_finder: LocFinder<R>,
        type_storage: TypeStorage,
        unwinder: Unwinder<R>,
        base_address: u64,
    ) -> Self {
        Self {
            state: Cell::new(SessionState::Started),
            dwarf,
            unwinder,
            loc_finder,
            type_storage,
            child,
            base_address,
            breakpoints: HashMap::new(),
            traps: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_type_storage(&self) -> &TypeStorage {
        &self.type_storage
    }

    pub fn get_loc_finder(&self) -> &LocFinder<R> {
        &self.loc_finder
    }

    fn child_pid(&self) -> Pid {
        Pid::from_raw(self.child.id() as libc::pid_t)
    }

    pub fn get_state(&self) -> SessionState {
        self.state.get()
    }

    pub fn run(&self) -> Result<()> {
        ptrace::cont(self.child_pid(), None)?;

        self.state.set(SessionState::Running);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.child.kill()?;

        self.state.set(SessionState::Exited);

        Ok(())
    }

    pub fn wait(&self) -> Result<()> {
        if self.get_state() == SessionState::Exited {
            return Ok(());
        }

        log::trace!("wait for signal");

        if let wait::WaitStatus::Exited(_, _) = wait::waitpid(self.child_pid(), None)? {
            log::trace!("child exited");
            self.state.set(SessionState::Exited);
            return Ok(());
        }

        self.state.set(SessionState::Running);
        let ip = self.get_ip()?;
        log::trace!("stopped at {:#x}", ip);
        let prev_addr = ip - 1;

        if self.traps.borrow().contains_key(&prev_addr) {
            log::trace!("stopped at trap {:#x}", prev_addr);
            self.remove_trap(prev_addr)?;
            self.rewind()?;
            return Ok(());
        }

        if let Some(breakpoint) = self.breakpoints.get(&prev_addr) {
            log::trace!("stopped at breakpoint {}", breakpoint.loc);
            self.disable_bp(breakpoint)?;
            self.rewind()?;
            return Ok(());
        }

        Ok(())
    }

    pub fn cont(&self) -> Result<()> {
        log::trace!("continue");

        // find breakpoint
        let ip = self.get_ip()?;
        log::trace!("now at {:#x}", ip);

        if let Some(breakpoint) = self.breakpoints.get(&ip) {
            log::trace!("stopped at breakpoint {}", breakpoint.loc);

            self.single_step()?;
            if self.get_state() == SessionState::Exited {
                return Ok(());
            }
            self.enable_bp(breakpoint)?;
        }

        log::trace!("continue from {:#x}", self.get_ip()?);
        ptrace::cont(self.child_pid(), None)?;
        self.state.set(SessionState::Running);
        Ok(())
    }

    fn single_step(&self) -> Result<()> {
        ptrace::step(self.child_pid(), None)?;
        self.wait()
    }

    pub fn step(&self) -> Result<()> {
        let ip = self.get_ip()?;
        let start_line = self.loc_finder.find_line(ip).ok_or(anyhow!("can't find start line"))?;
        log::trace!("start line {}", start_line);
        let next_line_address = match self.loc_finder.find_next_line_address(&start_line) {
            Some(address) => address,
            None => return self.step_out(),
        };
        log::trace!("next line address {:#x}", next_line_address);
        let func_end = self.loc_finder.find_fund_end(ip).ok_or(anyhow!("can't find func end"))?;

        if next_line_address >= func_end {
            log::trace!("next line is outside of func. Step out");
            return self.step_out();
        }

        self.add_trap(next_line_address)?;
        self.cont()?;
        self.wait()
    }

    pub fn step_in(&self) -> Result<()> {
        let start_line = self.get_current_line()?.ok_or(anyhow!("can't find start line"))?;
        log::trace!("step in from {}", start_line);

        loop {
            self.single_step()?;
            match self.get_state() {
                SessionState::Started => panic!("child is not running"),
                SessionState::Running => {
                    if let Some(line) = self.get_current_line()? {
                        if line != start_line {
                            log::trace!("stepped in to {}", line);
                            return Ok(());
                        }
                    }
                }
                SessionState::Exited => return Ok(()),
            }
        }
    }

    pub fn step_out(&self) -> Result<()> {
        let ctx = self.get_context()?;
        if self.loc_finder.is_inside_main(ctx.ip) {
            log::trace!("step out of main");
            self.cont()?;
            self.wait()?;
            return Ok(());
        }

        let return_ip = self.get_func_return_addr(ctx)?;
        log::trace!("step out to {:#x}", return_ip);

        // there is posibility that we'll stop with bp <= start_bp (using some recursion), but we'll ignore this case for now
        self.add_trap(return_ip)?;
        self.cont()?;
        self.wait()
    }

    fn rewind(&self) -> Result<()> {
        log::trace!("rewind");

        let mut regs = ptrace::getregs(self.child_pid())?;
        log::trace!("current ip {:#x}", regs.rip);
        regs.rip -= 1;
        ptrace::setregs(self.child_pid(), regs)?;
        log::trace!("new ip {:#x}", regs.rip);

        Ok(())
    }

    fn get_func_return_addr(&self, ctx: Context) -> Result<u64> {
        let func_start = self.loc_finder.find_func_start(ctx.ip).ok_or(anyhow!("find func start"))?;

        self.check_func_prologue(func_start)?;

        let return_addr_location = if ctx.ip - func_start <= 4 {
            ctx.sp
        } else if ctx.ip - func_start <= 8 {
            ctx.sp + WORD_SIZE as u64
        } else {
            ctx.bp + WORD_SIZE as u64
        };

        let return_addr = self.read_address(return_addr_location, WORD_SIZE)?.get_u64_ne();
        Ok(return_addr)
    }

    fn check_func_prologue(&self, func_start: u64) -> Result<()> {
        let bytes = self.read_address(func_start, FUNC_PROLOGUE_MAGIC_BYTES.len())?;
        if bytes.as_ref() != FUNC_PROLOGUE_MAGIC_BYTES {
            bail!("func prologue not found");
        }

        Ok(())
    }

    /// get instruction pointer
    fn get_ip(&self) -> Result<u64> {
        let regs = ptrace::getregs(self.child_pid())?;
        Ok(regs.rip)
    }

    pub fn get_current_line(&self) -> Result<Option<Rc<str>>> {
        let ip = self.get_ip()?;
        let line = self.loc_finder.find_line(ip);
        Ok(line)
    }

    pub fn add_breakpoint<'a, S>(&mut self, loc: S) -> Result<()>
    where
        S: Into<Cow<'a, str>>,
    {
        let loc = loc.into().into_owned();
        let loc = self.prepare_breakpoint_loc(&loc)?;
        let addr = self.loc_finder.find_loc(&loc)?.ok_or(DebuggerError::LocNotFound)?;

        // can't use entry api here because of borrors
        if self.breakpoints.contains_key(&addr) {
            bail!(DebuggerError::BreakpointAlreadyExist)
        }

        log::trace!("set breakpoint at {:#x}", addr);
        let original_bytecode = ptrace::read(self.child_pid(), addr as ptrace::AddressType)?;
        let breakpoint = Breakpoint::new(addr, original_bytecode, loc);
        self.enable_bp(&breakpoint)?;

        self.breakpoints.insert(addr, breakpoint);

        Ok(())
    }

    pub fn list_breakpoints(&self) -> impl ExactSizeIterator<Item = &Breakpoint> {
        self.breakpoints.values()
    }

    pub fn get_breakpoint(&self, loc: &str) -> Option<&Breakpoint> {
        self.breakpoints.values().find(|&breakpoint| breakpoint.loc == loc)
    }

    pub fn remove_breakpoint(&mut self, loc: &str) -> Result<()> {
        match self.get_breakpoint(loc).map(|breakpoint| breakpoint.addr) {
            Some(addr) => {
                let breakpoint = self.breakpoints.remove(&addr);
                self.disable_bp(&breakpoint.unwrap())
            }
            None => Err(anyhow!(DebuggerError::BreakpointNotFound)),
        }
    }

    pub fn clear_breakpoints(&mut self) -> Result<()> {
        log::trace!("clear breakpoints");

        for breakpoint in self.breakpoints.values() {
            self.disable_bp(breakpoint)?;
        }

        self.breakpoints.clear();

        Ok(())
    }

    fn prepare_breakpoint_loc<'a>(&self, loc: &'a str) -> Result<Cow<'a, str>> {
        let loc = loc.trim();

        match loc.parse::<u64>() {
            Ok(_) => {
                let ip = match self.get_state() {
                    SessionState::Started => None,
                    SessionState::Running => Some(self.get_ip()?),
                    SessionState::Exited => panic!("can't get ip"),
                };

                let unit_name = self.loc_finder.find_unit(ip);

                match unit_name {
                    Some(unit_name) => Ok(Cow::from(format!("{}:{}", unit_name, loc))),
                    None => Err(anyhow!(DebuggerError::LocNotFound)),
                }
            }
            Err(_) => Ok(Cow::from(loc)),
        }
    }

    pub fn enable_breakpoint(&self, loc: &str) -> Result<()> {
        match self.get_breakpoint(loc) {
            Some(breakpoint) => self.enable_bp(breakpoint),
            None => Err(anyhow!(DebuggerError::BreakpointNotFound)),
        }
    }

    fn enable_bp(&self, breakpoint: &Breakpoint) -> Result<()> {
        let bytecode_with_trap = (breakpoint.original_bytecode & !0xff) | 0xcc;

        log::trace!(
            "replace {:#x} with {:#x} at {:#x}",
            breakpoint.original_bytecode,
            bytecode_with_trap,
            breakpoint.addr
        );
        ptrace::write(self.child_pid(), breakpoint.addr as ptrace::AddressType, bytecode_with_trap)?;

        breakpoint.enabled.set(true);

        Ok(())
    }

    pub fn disable_breakpoint(&self, loc: &str) -> Result<()> {
        match self.get_breakpoint(loc) {
            Some(breakpoint) => self.disable_bp(breakpoint),
            None => Err(anyhow!(DebuggerError::BreakpointNotFound)),
        }
    }

    fn disable_bp(&self, breakpoint: &Breakpoint) -> Result<()> {
        ptrace::write(self.child_pid(), breakpoint.addr as ptrace::AddressType, breakpoint.original_bytecode)?;
        log::trace!("restored bytecode at {:#x} to {:#x}", breakpoint.addr, breakpoint.original_bytecode);

        breakpoint.enabled.set(false);

        Ok(())
    }

    fn add_trap(&self, addr: u64) -> Result<()> {
        match self.traps.borrow_mut().entry(addr) {
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(vacant_entry) => {
                log::trace!("set trap at {:#x}", addr);

                let original_bytecode = ptrace::read(self.child_pid(), addr as ptrace::AddressType)?;
                let bytecode_with_trap = (original_bytecode & !0xff) | 0xcc;

                log::trace!("replace {:#x} with {:#x} at {:#x}", original_bytecode, bytecode_with_trap, addr);
                ptrace::write(self.child_pid(), addr as ptrace::AddressType, bytecode_with_trap)?;

                vacant_entry.insert(Trap::new(original_bytecode));

                Ok(())
            }
        }
    }

    fn remove_trap(&self, addr: u64) -> Result<()> {
        if let Some(trap) = self.traps.borrow_mut().remove(&addr) {
            ptrace::write(self.child_pid(), addr as ptrace::AddressType, trap.original_bytecode)?;
            log::trace!("restored bytecode at {:#x} to {:#x}", addr, trap.original_bytecode);
        }

        Ok(())
    }

    pub fn get_vars(&self) -> Result<Vec<Var>> {
        let ip = self.get_ip()?;
        let current_func = self.loc_finder.find_func_by_address(ip).ok_or(anyhow!("get current func"))?;
        let mut vars = Vec::new();

        for (name, &var_ref) in self.loc_finder.get_vars(Some(current_func.as_ref())).iter() {
            let value = self.get_value_by_var_ref(current_func.as_ref(), var_ref)?;
            vars.push(Var::new(name.clone(), value));
        }

        Ok(vars)
    }

    pub fn get_var_loc(&self, path: &str) -> Result<TypedValueLoc> {
        let (operators, path) = Self::parse_path(path);
        let (&name, path) = path.split_first().ok_or(DebuggerError::InvalidPath)?;
        let ip = self.get_ip()?;
        let func = self.loc_finder.find_func_by_address(ip).ok_or(anyhow!("get current func"))?;
        let var_ref = match self.loc_finder.get_var(name, Some(func.as_ref())) {
            Some(var_ref) => var_ref,
            None => bail!(DebuggerError::VarNotFound(String::from(name))),
        };
        let mut loc = self.get_value_loc_by_var_ref(&func, var_ref)?;
        loc = self.unwind_loc(loc, path)?;
        loc = self.apply_operators(loc, &operators)?;

        Ok(loc)
    }

    pub fn get_var(&self, path: &str) -> Result<Var> {
        let loc = self.get_var_loc(path)?;
        let size = self.type_storage.get_type_size(loc.type_id)?;
        let buf = self.read_value_loc(loc.location, size)?;
        let value = Value::new(loc.type_id, buf);
        let name = Self::get_var_name(path)?;
        let var = Var::new(name, value);
        Ok(var)
    }

    fn get_value_loc_by_var_ref(&self, func: &str, var_ref: VarRef<R::Offset>) -> Result<TypedValueLoc> {
        let unit_header = self.dwarf.debug_info.header_from_offset(var_ref.entry_ref.unit_offset)?;
        let unit = self.dwarf.unit(unit_header)?;
        let entry = unit.entry(var_ref.entry_ref.entry_offset)?;
        let unit_ref = unit.unit_ref(&self.dwarf);

        let location = entry.attr_value(gimli::DW_AT_location)?.ok_or(anyhow!("get location attr"))?;
        let expr = location.exprloc_value().ok_or(anyhow!("get exprloc"))?;
        let evaluation = self.eval_expr(expr, &unit_ref, func)?;
        let mut pieces = evaluation.result();
        if !(pieces.len() == 1 && pieces[0].size_in_bits.is_none()) {
            bail!("can't read composite location");
        }
        let location = pieces.remove(0).location;

        Ok(TypedValueLoc::new(location.try_into()?, var_ref.type_id))
    }

    fn get_value_by_var_ref(&self, func: &str, var_ref: VarRef<R::Offset>) -> Result<Value> {
        let loc = self.get_value_loc_by_var_ref(func, var_ref)?;
        let size = self.type_storage.get_type_size(loc.type_id)?;
        let buf = self.read_value_loc(loc.location, size)?;

        Ok(Value::new(loc.type_id, buf))
    }

    fn unwind_loc(&self, loc: TypedValueLoc, path: &[&str]) -> Result<TypedValueLoc> {
        if path.is_empty() {
            return Ok(loc);
        }

        match self.type_storage.get(loc.type_id)? {
            Type::Const(subtype_id) | Type::Volatile(subtype_id) | Type::Atomic(subtype_id) | Type::Typedef(_, subtype_id) => {
                self.unwind_loc(loc.with_type(subtype_id), path)
            }
            Type::Pointer(subtype_id) => {
                let ptr = self.read_value_loc(loc.location, WORD_SIZE)?.get_u64_ne();
                if ptr == 0 {
                    bail!(DebuggerError::InvalidPath);
                }

                self.unwind_loc(TypedValueLoc::new(ValueLoc::Address(ptr), subtype_id), path)
            }
            Type::Struct { fields, .. } => match fields.iter().find(|&field| field.name.as_ref() == path[0]) {
                Some(field) => self.unwind_loc(TypedValueLoc::new(loc.location.with_offset(field.offset)?, field.type_id), &path[1..]),
                None => Err(anyhow!(DebuggerError::InvalidPath)),
            },
            Type::Union { fields, .. } => match fields.iter().find(|&field| field.name.as_ref() == path[0]) {
                Some(field) => self.unwind_loc(loc.with_type(field.type_id), &path[1..]),
                None => Err(anyhow!(DebuggerError::InvalidPath)),
            },
            _ => Err(anyhow!(DebuggerError::InvalidPath)),
        }
    }

    fn parse_path(path: &str) -> (Vec<Operator>, Vec<&str>) {
        let operators: Vec<Operator> = path.chars().map_while(|c| Operator::try_from(c).ok()).collect();
        let path = path[operators.len()..].split('.').collect();

        (operators, path)
    }

    fn apply_operators(&self, loc: TypedValueLoc, operators: &[Operator]) -> Result<TypedValueLoc> {
        match operators.last() {
            Some(operator) => match operator {
                Operator::Ref => match loc.location {
                    ValueLoc::Address(address) => {
                        let ref_type_id = self.type_storage.get_type_ref(loc.type_id);
                        self.apply_operators(TypedValueLoc::new(ValueLoc::Value(address), ref_type_id), &operators[..operators.len() - 1])
                    }
                    _ => Err(anyhow!(DebuggerError::InvalidPath)),
                },
                Operator::Deref => match self.type_storage.get(loc.type_id)? {
                    Type::Pointer(subtype_id) => {
                        let ptr = self.read_value_loc(loc.location, WORD_SIZE)?.get_u64_ne();
                        if ptr == 0 {
                            bail!(DebuggerError::InvalidPath);
                        }

                        self.apply_operators(TypedValueLoc::new(ValueLoc::Address(ptr), subtype_id), &operators[..operators.len() - 1])
                    }
                    Type::Const(subtype_id) | Type::Volatile(subtype_id) | Type::Atomic(subtype_id) | Type::Typedef(_, subtype_id) => {
                        self.apply_operators(loc.with_type(subtype_id), operators)
                    }
                    _ => Err(anyhow!(DebuggerError::InvalidPath)),
                },
            },
            None => Ok(loc),
        }
    }

    fn get_var_name(path: &str) -> Result<Rc<str>> {
        let pos = path.find(|c| c != '*').ok_or(DebuggerError::InvalidPath)?;
        if pos == 0 {
            return Ok(Rc::from(path.split('.').next_back().unwrap()));
        }

        let (prefix, path) = path.split_at(pos);
        let name = format!("{}{}", prefix, path.split('.').next_back().unwrap());
        Ok(Rc::from(name))
    }

    fn eval_expr(&self, expr: gimli::Expression<R>, unit_ref: &gimli::UnitRef<R>, current_func: &str) -> Result<gimli::Evaluation<R>> {
        let mut eval = expr.evaluation(unit_ref.encoding());
        let mut result = eval.evaluate()?;

        loop {
            match result {
                gimli::EvaluationResult::Complete => break,
                gimli::EvaluationResult::RequiresFrameBase => {
                    let entry_ref = self.loc_finder.find_func(current_func).ok_or(anyhow!("no current func"))?;
                    let entry = unit_ref.entry(entry_ref.entry_offset)?;
                    let frame_base_attr = entry.attr_value(gimli::DW_AT_frame_base)?.ok_or(anyhow!("get frame base attr"))?;
                    let fram_base_expr = frame_base_attr.exprloc_value().ok_or(anyhow!("get exprloc"))?; // todo loclists
                    let frame_base_comleted_evaluation = self.eval_expr(fram_base_expr, unit_ref, current_func)?;
                    let frame_base = frame_base_comleted_evaluation
                        .value_result()
                        .ok_or(anyhow!("get value result"))?
                        .to_u64(!0u64)?;
                    log::trace!("frame base {:#x}", frame_base);
                    result = eval.resume_with_frame_base(frame_base)?;
                }
                gimli::EvaluationResult::RequiresCallFrameCfa => {
                    let ip = self.get_ip()?;
                    let cfa = self.unwinder.unwind_cfa(ip - self.base_address)?;

                    let cfa_value = match cfa {
                        gimli::CfaRule::RegisterAndOffset { register, offset } => {
                            let register_value = self.get_register_value(register)?;
                            let value = register_value as i64 + offset;
                            value as u64
                        }
                        gimli::CfaRule::Expression(unwind_expression) => {
                            let expression = self.unwinder.unwind_expression(&unwind_expression)?;
                            let evaluation = self.eval_expr(expression, unit_ref, current_func)?;
                            let value = evaluation.value_result().ok_or(anyhow!("get value result"))?;
                            value.to_u64(!0u64)?
                        }
                    };

                    log::trace!("cfa is {:#x}", cfa_value);
                    result = eval.resume_with_call_frame_cfa(cfa_value)?;
                }
                gimli::EvaluationResult::RequiresRelocatedAddress(address) => {
                    log::trace!("requires relocated address {:#x}", address);
                    result = eval.resume_with_relocated_address(self.base_address + address)?;
                }
                _ => bail!("can't provide {:?}", result),
            }
        }

        Ok(eval)
    }

    fn get_context(&self) -> Result<Context> {
        let regs = ptrace::getregs(self.child_pid())?;
        Ok(Context::new(regs))
    }

    fn get_register_value(&self, register: gimli::Register) -> Result<u64> {
        let mut regs = ptrace::getregs(self.child_pid())?;
        let value_ref = Self::get_register_ref(&mut regs, register)?;

        Ok(*value_ref)
    }

    fn set_register_value(&self, register: gimli::Register, value: u64) -> Result<()> {
        let mut regs = ptrace::getregs(self.child_pid())?;
        let value_ref = Self::get_register_ref(&mut regs, register)?;
        *value_ref = value;
        ptrace::setregs(self.child_pid(), regs)?;

        Ok(())
    }

    fn get_register_ref(regs: &mut libc::user_regs_struct, register: gimli::Register) -> Result<&mut u64> {
        let register_name = gimli::X86_64::register_name(register).ok_or(anyhow!("get {} register", register.0))?;

        let value = match register_name {
            "rax" => &mut regs.rax,
            "rdx" => &mut regs.rdx,
            "rcx" => &mut regs.rcx,
            "rbx" => &mut regs.rbx,
            "rsi" => &mut regs.rsi,
            "rdi" => &mut regs.rdi,
            "rbp" => &mut regs.rbp,
            "rsp" => &mut regs.rsp,
            "r8" => &mut regs.r8,
            "r9" => &mut regs.r9,
            "r10" => &mut regs.r10,
            "r11" => &mut regs.r11,
            "r12" => &mut regs.r12,
            "r13" => &mut regs.r13,
            "r14" => &mut regs.r14,
            "r15" => &mut regs.r15,
            _ => bail!("get {} register", register_name),
        };

        Ok(value)
    }

    pub fn read_c_string(&self, addr: u64) -> Result<String> {
        log::trace!("read c string at {:#x}", addr);

        if addr == 0 {
            return Ok(String::from("null")); // special case
        }

        let mut buf = Vec::new();
        let mut read_buf = [0; READ_MEM_BUF_SIZE];

        // todo maybe process_vm_readv
        let mut procmem = fs::File::open(format!("/proc/{}/mem", self.child_pid()))?;
        procmem.seek(io::SeekFrom::Start(addr))?;

        loop {
            let n = procmem.read(&mut read_buf)?;

            match read_buf[..n].iter().position(|&b| b == 0) {
                Some(pos) => {
                    buf.extend_from_slice(&read_buf[..pos]);
                    let s = String::from_utf8(buf)?;
                    return Ok(s);
                }
                None => buf.extend_from_slice(&read_buf),
            }
        }
    }

    fn read_value_loc(&self, loc: ValueLoc, size: usize) -> Result<Bytes> {
        log::trace!("read {} bytes from {:?}", size, loc);

        let mut buf = vec![0; size];

        match loc {
            ValueLoc::Register { register, offset } => {
                if offset as usize + size > WORD_SIZE {
                    bail!("too many bytes to read")
                }
                let value = self.get_register_value(register)?;
                buf.copy_from_slice(&value.to_ne_bytes()[offset as usize..offset as usize + size]);
            }
            ValueLoc::Address(address) => self.read_memory(address, &mut buf)?,
            ValueLoc::Value(value) => {
                if size > WORD_SIZE {
                    bail!("too many bytes to read")
                }
                buf.copy_from_slice(&value.to_ne_bytes()[..size]);
            }
        };

        Ok(buf.into())
    }

    fn read_address(&self, addr: u64, size: usize) -> Result<Bytes> {
        log::trace!("read {} bytes from address {:#x}", size, addr);

        let mut buf = vec![0; size];
        self.read_memory(addr, &mut buf)?;

        Ok(buf.into())
    }

    fn read_memory(&self, addr: u64, buf: &mut Vec<u8>) -> Result<()> {
        // todo maybe process_vm_readv
        let mut procmem = fs::File::open(format!("/proc/{}/mem", self.child_pid()))?;
        procmem.seek(io::SeekFrom::Start(addr))?;
        procmem.read_exact(buf.as_mut_slice())?;

        Ok(())
    }

    pub fn write_location(&self, location: ValueLoc, mut value: Bytes) -> Result<()> {
        log::trace!("write {:?} to {:?}", value, location);

        match location {
            ValueLoc::Register { register, offset } => {
                if offset as usize + value.len() > WORD_SIZE {
                    bail!(DebuggerError::InvalidValue);
                }

                let new_value = if value.len() == WORD_SIZE {
                    value.get_u64_ne()
                } else {
                    let mut buf = self.get_register_value(register)?.to_ne_bytes();
                    buf[offset as usize..offset as usize + value.len()].copy_from_slice(&value);
                    u64::from_ne_bytes(buf)
                };

                self.set_register_value(register, new_value)
            }
            ValueLoc::Address(address) => self.write_memory(address, &value),
            _ => bail!(DebuggerError::InvalidLocation),
        }
    }

    fn write_memory(&self, addr: u64, buf: &[u8]) -> Result<()> {
        let mut procmem = fs::OpenOptions::new().write(true).open(format!("/proc/{}/mem", self.child_pid()))?;
        procmem.seek(io::SeekFrom::Start(addr))?;
        procmem.write_all(buf)?;

        Ok(())
    }

    pub fn alloc_c_string(&self, s: &str) -> Result<u64> {
        log::trace!("allocate c string {:?}", s);

        let new_str_addr = self.child_alloc(s.len() + 1)?;
        log::trace!("allocated memory at {:#x}", new_str_addr);

        let mut buf = String::with_capacity(s.len() + 1);
        buf.push_str(s);
        buf.push('\0');

        self.write_memory(new_str_addr, buf.as_bytes())?;

        Ok(new_str_addr)
    }

    fn child_alloc(&self, size: usize) -> Result<u64> {
        log::trace!("allocate {} bytes", size);

        let mut regs = ptrace::getregs(self.child_pid())?; // backup registers
        #[allow(clippy::clone_on_copy)]
        let original_regs = regs.clone();

        regs.rax = 0x9; // mmap syscall
        regs.rdi = 0; // address
        regs.rsi = size as u64;
        regs.rdx = (libc::PROT_READ | libc::PROT_WRITE) as u64;
        regs.r10 = (libc::MAP_PRIVATE | libc::MAP_ANONYMOUS) as u64;
        regs.r8 = (-1_i64) as u64; // allocate on memory
        regs.r9 = 0; // offset

        let original_bytecode = ptrace::read(self.child_pid(), regs.rip as ptrace::AddressType)?;
        let bytecode_with_syscall = (original_bytecode & !0xffff) | 0x050f; // set syscall instruction
        ptrace::write(self.child_pid(), regs.rip as ptrace::AddressType, bytecode_with_syscall)?;
        log::trace!("replace {:#x} with {:#x} at {:#x}", original_bytecode, bytecode_with_syscall, regs.rip);
        ptrace::setregs(self.child_pid(), regs)?;

        ptrace::step(self.child_pid(), None)?;
        if let wait::WaitStatus::Exited(_, _) = wait::waitpid(self.child_pid(), None)? {
            self.state.set(SessionState::Exited);
            bail!("child exited");
        }

        let regs = ptrace::getregs(self.child_pid())?;
        if (regs.rax as i64) < 0 {
            log::trace!("error allocating memory: {}", -(regs.rax as i64)); // log errno
            bail!("can't allocate memory");
        }

        ptrace::write(self.child_pid(), original_regs.rip as ptrace::AddressType, original_bytecode)?; // restore bytecode
        ptrace::setregs(self.child_pid(), original_regs)?; // restore registers

        Ok(regs.rax)
    }
}
