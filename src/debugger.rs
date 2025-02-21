use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Seek};
use std::mem;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process;
use std::rc::Rc;

use crate::loc_finder::{LocFinder, LocNotFound, VarRef};
use crate::unwinder::Unwinder;
use crate::utils::exit_status_sentinel::check;
use crate::var::{Type, TypeId, Var};

use anyhow::{anyhow, bail, Result};
use bytes::{Buf, BufMut, Bytes};
use thiserror::Error;

pub const WORD_SIZE: usize = 8;
const READ_MEM_BUF_SIZE: usize = 512;
const FUNC_PROLOGUE_MAGIC_BYTES: [u8; 8] = [0xf3, 0x0f, 0x1e, 0xfa, 0x55, 0x48, 0x89, 0xe5];

#[derive(Debug, Clone)]
pub struct Breakpoint {
    addr: u64,
    original_data: u64,
    pub loc: String,
    pub enabled: Cell<bool>,
}

impl Breakpoint {
    pub fn new<S: Into<String>>(addr: u64, original_data: u64, loc: S) -> Self {
        Self {
            addr,
            original_data,
            loc: loc.into(),
            enabled: Cell::new(false),
        }
    }
}

#[derive(Debug, Error)]
#[error("breakpoint not found")]
pub struct BreakpointNotFound;

#[derive(Debug)]
struct Trap {
    original_data: u64,
}

impl Trap {
    fn new(original_data: u64) -> Self {
        Self { original_data }
    }
}

pub struct Debugger<R: gimli::Reader> {
    state: Cell<DebuggerState>,
    dwarf: gimli::Dwarf<R>,
    unwinder: Unwinder<R>,
    loc_finder: LocFinder<R>,
    child: process::Child,
    breakpoints: Vec<Breakpoint>,
    traps: RefCell<HashMap<u64, Trap>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebuggerState {
    Started,
    Running,
    Exited,
}

impl<R: gimli::Reader> Debugger<R> {
    pub fn start<I, S>(prog: &Path, args: I, dwarf: gimli::Dwarf<R>, unwinder: Unwinder<R>) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let loc_finder = LocFinder::new(&dwarf)?;

        let mut command = process::Command::new(&prog);

        unsafe {
            command.pre_exec(|| {
                // todo nix crate
                check(libc::ptrace(libc::PTRACE_TRACEME, 0, std::ptr::null_mut::<i8>(), std::ptr::null_mut::<i8>()))?;

                Ok(())
            });
        }

        let child = command.args(args).spawn()?;

        let debugger = Self {
            state: Cell::new(DebuggerState::Started),
            dwarf,
            unwinder,
            loc_finder,
            child,
            breakpoints: Vec::new(),
            traps: RefCell::new(HashMap::new()),
        };

        let _ = debugger.wait()?;
        // wait will set state to running, so change it back
        debugger.state.set(DebuggerState::Started);

        Ok(debugger)
    }

    fn child_pid(&self) -> libc::pid_t {
        self.child.id() as libc::pid_t
    }

    pub fn get_state(&self) -> DebuggerState {
        self.state.get()
    }

    pub fn run(&self) -> Result<()> {
        check(unsafe { libc::ptrace(libc::PTRACE_CONT, self.child_pid(), std::ptr::null_mut::<i8>(), std::ptr::null_mut::<i8>()) })?;

        self.state.set(DebuggerState::Running);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.child.kill()?;

        self.state.set(DebuggerState::Exited);

        Ok(())
    }

    pub fn wait(&self) -> Result<()> {
        if self.get_state() == DebuggerState::Exited {
            return Ok(());
        }

        log::trace!("wait for signal");

        let mut status: libc::c_int = 0;

        check(unsafe { libc::waitpid(self.child_pid(), &mut status as *mut libc::c_int, 0) })?;

        if libc::WIFEXITED(status) {
            log::trace!("child exited");
            self.state.set(DebuggerState::Exited);
            return Ok(());
        }

        self.state.set(DebuggerState::Running);
        let ip = self.get_ip()?;
        log::trace!("stopped at {:#x}", ip);
        let prev_addr = ip - 1;

        if self.traps.borrow().contains_key(&prev_addr) {
            log::trace!("stopped at trap {:#x}", prev_addr);
            self.remove_trap(prev_addr)?;
            self.rewind()?;
            return Ok(());
        }

        if let Some(breakpoint) = self.breakpoints.iter().find(|&breakpoint| breakpoint.addr == prev_addr) {
            log::trace!("stopped at breakpoint {}", breakpoint.loc);
            self.disable_breakpoint(breakpoint)?;
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

        if let Some(breakpoint) = self.breakpoints.iter().find(|&breakpoint| breakpoint.addr == ip) {
            log::trace!("stopped at breakpoint {}", breakpoint.loc);

            self.single_step()?;
            if self.get_state() == DebuggerState::Exited {
                return Ok(());
            }
            self.enable_breakpoint(breakpoint)?;
        }

        log::trace!("continue from {:#x}", self.get_ip()?);
        check(unsafe { libc::ptrace(libc::PTRACE_CONT, self.child_pid(), std::ptr::null_mut::<i8>(), std::ptr::null_mut::<i8>()) })?;
        self.state.set(DebuggerState::Running);
        Ok(())
    }

    fn single_step(&self) -> Result<()> {
        check(unsafe {
            libc::ptrace(
                libc::PTRACE_SINGLESTEP,
                self.child_pid(),
                std::ptr::null_mut::<i8>(),
                std::ptr::null_mut::<i8>(),
            )
        })?;

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
                DebuggerState::Started => panic!("child is not running"),
                DebuggerState::Running => {
                    if let Some(line) = self.get_current_line()? {
                        if line != start_line {
                            log::trace!("stepped in to {}", line);
                            return Ok(());
                        }
                    }
                }
                DebuggerState::Exited => return Ok(()),
            }
        }
    }

    pub fn step_out(&self) -> Result<()> {
        let ip = self.get_ip()?;
        if self.loc_finder.is_inside_main(ip) {
            log::trace!("step out of main");
            self.cont()?;
            self.wait()?;
            return Ok(());
        }

        let return_ip = self.get_func_return_addr()?;
        log::trace!("step out to {:#x}", return_ip);

        // there is posibility that we'll stop with bp <= start_bp (using some recursion), but we'll ignore this case for now
        self.add_trap(return_ip)?;
        self.cont()?;
        self.wait()
    }

    fn rewind(&self) -> Result<()> {
        log::trace!("rewind");

        let mut regs: libc::user_regs_struct = unsafe { mem::zeroed() };

        check(unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                self.child_pid(),
                std::ptr::null_mut::<i8>(),
                &mut regs as *mut libc::user_regs_struct as *mut i8,
            )
        })?;

        log::trace!("current ip {:#x}", regs.rip);

        regs.rip -= 1;

        check(unsafe {
            libc::ptrace(
                libc::PTRACE_SETREGS,
                self.child_pid(),
                std::ptr::null_mut::<i8>(),
                &mut regs as *mut libc::user_regs_struct as *mut i8,
            )
        })?;

        log::trace!("new ip {:#x}", regs.rip);

        Ok(())
    }

    fn get_func_return_addr(&self) -> Result<u64> {
        // todo get all registers in one syscall
        let ip = self.get_ip()?;
        let func_start = self.loc_finder.find_func_start(ip).ok_or(anyhow!("find func start"))?;

        self.check_func_prologue(func_start)?;

        let return_addr_location = if ip - func_start <= 4 {
            self.get_sp()?
        } else if ip - func_start <= 8 {
            self.get_sp()? + WORD_SIZE as u64
        } else {
            self.get_bp()? + WORD_SIZE as u64
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
        let mut regs: libc::user_regs_struct = unsafe { mem::zeroed() };

        check(unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                self.child_pid(),
                std::ptr::null_mut::<i8>(),
                &mut regs as *mut libc::user_regs_struct as *mut i8,
            )
        })?;

        Ok(regs.rip)
    }

    /// get stack base pointer
    fn get_bp(&self) -> Result<u64> {
        self.get_register_value(gimli::X86_64::RBP)
    }

    /// get stack pointer
    fn get_sp(&self) -> Result<u64> {
        self.get_register_value(gimli::X86_64::RSP)
    }

    fn get_current_line(&self) -> Result<Option<Rc<str>>> {
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
        let addr = self.loc_finder.find_loc(&loc)?.ok_or(LocNotFound)?;

        log::trace!("set breakpoint at {:#x}", addr);

        let data = check(unsafe { libc::ptrace(libc::PTRACE_PEEKTEXT, self.child_pid(), addr, std::ptr::null_mut::<i8>()) })?;

        let breakpoint = Breakpoint::new(addr, data as u64, loc);

        self.enable_breakpoint(&breakpoint)?;

        self.breakpoints.push(breakpoint);

        Ok(())
    }

    pub fn list_breakpoints(&self) -> &Vec<Breakpoint> {
        &self.breakpoints
    }

    pub fn get_breakpoint(&self, index: usize) -> Option<&Breakpoint> {
        self.breakpoints.get(index)
    }

    pub fn remove_breakpoint(&mut self, index: usize) -> Result<()> {
        if index >= self.breakpoints.len() {
            bail!(BreakpointNotFound);
        }

        let breakpoint = self.breakpoints.remove(index);

        self.disable_breakpoint(&breakpoint)?;

        Ok(())
    }

    pub fn clear_breakpoints(&mut self) -> Result<()> {
        log::trace!("clear breakpoints");

        for breakpoint in self.breakpoints.iter() {
            self.disable_breakpoint(breakpoint)?;
        }

        self.breakpoints.clear();

        Ok(())
    }

    fn prepare_breakpoint_loc<'a>(&self, loc: &'a str) -> Result<Cow<'a, str>> {
        let loc = loc.trim();

        match loc.parse::<u64>() {
            Ok(_) => {
                let ip = match self.get_state() {
                    DebuggerState::Started => None,
                    DebuggerState::Running => Some(self.get_ip()?),
                    DebuggerState::Exited => panic!("can't get ip"),
                };

                let unit_name = self.loc_finder.find_unit(ip);

                match unit_name {
                    Some(unit_name) => Ok(Cow::from(format!("{}:{}", unit_name, loc))),
                    None => Err(anyhow!(LocNotFound)),
                }
            }
            Err(_) => Ok(Cow::from(loc)),
        }
    }

    pub fn enable_breakpoint(&self, breakpoint: &Breakpoint) -> Result<()> {
        let data_with_trap = (breakpoint.original_data & !0xff) | 0xcc;

        log::trace!("replace {:#x} with {:#x}", breakpoint.addr, data_with_trap);

        check(unsafe { libc::ptrace(libc::PTRACE_POKETEXT, self.child_pid(), breakpoint.addr, data_with_trap) })?;

        breakpoint.enabled.set(true);

        let readback_data = check(unsafe { libc::ptrace(libc::PTRACE_PEEKTEXT, self.child_pid(), breakpoint.addr, std::ptr::null_mut::<i8>()) })?;
        log::trace!("data after trap {:#x}: {:#x}", breakpoint.addr, readback_data);

        Ok(())
    }

    pub fn disable_breakpoint(&self, breakpoint: &Breakpoint) -> Result<()> {
        check(unsafe { libc::ptrace(libc::PTRACE_POKETEXT, self.child_pid(), breakpoint.addr, breakpoint.original_data) })?;

        log::trace!("restored data at {:#x} to {:#x}", breakpoint.addr, breakpoint.original_data);

        breakpoint.enabled.set(false);

        Ok(())
    }

    fn add_trap(&self, addr: u64) -> Result<()> {
        match self.traps.borrow_mut().entry(addr) {
            Entry::Occupied(occupied_entry) => Ok(()),
            Entry::Vacant(vacant_entry) => {
                log::trace!("set trap at {:#x}", addr);

                let original_data = check(unsafe { libc::ptrace(libc::PTRACE_PEEKTEXT, self.child_pid(), addr, std::ptr::null_mut::<i8>()) })?;
                let data_with_trap = (original_data & !0xff) | 0xcc;

                log::trace!("replace {:#x} with {:#x}", addr, data_with_trap);
                check(unsafe { libc::ptrace(libc::PTRACE_POKETEXT, self.child_pid(), addr, data_with_trap) })?;

                let readback_data = check(unsafe { libc::ptrace(libc::PTRACE_PEEKTEXT, self.child_pid(), addr, std::ptr::null_mut::<i8>()) })?;
                log::trace!("data after trap {:#x}: {:#x}", addr, readback_data);

                vacant_entry.insert(Trap::new(original_data as u64));

                Ok(())
            }
        }
    }

    fn remove_trap(&self, addr: u64) -> Result<()> {
        if let Some(trap) = self.traps.borrow_mut().remove(&addr) {
            check(unsafe { libc::ptrace(libc::PTRACE_POKETEXT, self.child_pid(), addr, trap.original_data) })?;
            log::trace!("restored data at {:#x} to {:#x}", addr, trap.original_data);
        }

        Ok(())
    }

    pub fn get_vars(&self) -> Result<Vec<Var<R>>> {
        let ip = self.get_ip()?;
        let current_func = self.loc_finder.find_func_by_address(ip).ok_or(anyhow!("get current func"))?;
        let mut vars = Vec::new();

        for (name, &var_ref) in self.loc_finder.get_vars(Some(current_func.as_ref())).iter() {
            let var = self.get_var_by_entry_ref(name, current_func.as_ref(), var_ref)?;
            vars.push(var);
        }

        Ok(vars)
    }

    pub fn get_var(&self, name: &str) -> Result<Option<Var<R>>> {
        let ip = self.get_ip()?;
        let current_func = self.loc_finder.find_func_by_address(ip).ok_or(anyhow!("get current func"))?;
        let var_ref = match self.loc_finder.get_var(name, Some(current_func.as_ref())) {
            Some(var_ref) => var_ref,
            None => return Ok(None),
        };

        let current_func = self.loc_finder.find_func_by_address(ip).ok_or(anyhow!("get current func"))?;
        let var = self.get_var_by_entry_ref(name, current_func.as_ref(), var_ref)?;

        Ok(Some(var))
    }

    fn get_var_by_entry_ref(&self, name: &str, func: &str, var_ref: VarRef<R::Offset>) -> Result<Var<R>> {
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

        Ok(Var {
            type_id: var_ref.type_id,
            name: name.to_string(),
            location: pieces.remove(0).location,
        })
    }

    pub fn get_type(&self, type_id: TypeId) -> &Type {
        self.loc_finder.get_type(type_id)
    }

    // todo decouple
    pub fn get_type_size(&self, type_id: TypeId) -> usize {
        match self.get_type(type_id) {
            Type::Void => 0, // todo maybe should panic
            Type::Base { size, .. } => *size as usize,
            Type::Const(subtype_id) => self.get_type_size(*subtype_id),
            Type::Pointer(_) => WORD_SIZE,
            Type::Struct { size, .. } => *size as usize,
            Type::Typedef(_, subtype_id) => self.get_type_size(*subtype_id),
        }
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
                    let cfa = self.unwinder.unwind_cfa(ip)?;

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
                    // todo seems like relative address (PIE)
                    result = eval.resume_with_relocated_address(address)?;
                }
                _ => bail!("can't provide {:?}", result),
            }
        }

        Ok(eval)
    }

    fn get_register_value(&self, register: gimli::Register) -> Result<u64> {
        let register_name = gimli::X86_64::register_name(register).ok_or(anyhow!("get {} register", register.0))?;

        let mut regs: libc::user_regs_struct = unsafe { mem::zeroed() };

        check(unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                self.child_pid(),
                std::ptr::null_mut::<i8>(),
                &mut regs as *mut libc::user_regs_struct as *mut i8,
            )
        })?;

        let value = match register_name {
            "rax" => regs.rax,
            "rdx" => regs.rdx,
            "rcx" => regs.rcx,
            "rbx" => regs.rbx,
            "rsi" => regs.rsi,
            "rdi" => regs.rdi,
            "rbp" => regs.rbp,
            "rsp" => regs.rsp,
            "r8" => regs.r8,
            "r9" => regs.r9,
            "r10" => regs.r10,
            "r11" => regs.r11,
            "r12" => regs.r12,
            "r13" => regs.r13,
            "r14" => regs.r14,
            "r15" => regs.r15,
            _ => bail!("get {} register", register_name),
        };

        Ok(value)
    }

    pub fn read_c_string_at(&self, addr: u64) -> Result<String> {
        log::trace!("read c string at {:#x}", addr);

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

    pub fn read_location(&self, location: &gimli::Location<R>, size: usize) -> Result<Bytes> {
        log::trace!("read {} bytes from {:?}", size, location);

        let mut buf = vec![0; size];

        match location {
            gimli::Location::Register { register } => {
                if size > WORD_SIZE {
                    bail!("too many bytes to read")
                }
                let value = self.get_register_value(*register)?;
                buf.put_u64_ne(value);
            }
            gimli::Location::Address { address } => {
                self.read_memory(*address, &mut buf)?;
            }
            gimli::Location::Value { value } => {
                if size > WORD_SIZE {
                    bail!("too many bytes to read")
                }
                let value = value.to_u64(!0u64)?;
                buf.put_u64_ne(value);
            }
            gimli::Location::Bytes { value } => buf.extend_from_slice(&value.to_slice()?),
            _ => bail!("can't read location {:?}", location),
        }

        Ok(buf.into())
    }

    pub fn read_address(&self, addr: u64, size: usize) -> Result<Bytes> {
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
}
