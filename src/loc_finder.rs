use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Result;

use crate::consts::{FUNC_EPILOGUE_SIZE, FUNC_PROLOGUE_SIZE, MAIN_FUNC_NAME};
use crate::types::TypeId;
use crate::utils::ranges::Ranges;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct EntryRef<Offset: gimli::ReaderOffset> {
    pub unit_offset: gimli::DebugInfoOffset<Offset>,
    pub entry_offset: gimli::UnitOffset<Offset>,
}

impl<Offset: gimli::ReaderOffset> EntryRef<Offset> {
    pub fn new(unit_offset: gimli::DebugInfoOffset<Offset>, entry_offset: gimli::UnitOffset<Offset>) -> Self {
        Self { unit_offset, entry_offset }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VarRef<Offset: gimli::ReaderOffset> {
    pub entry_ref: EntryRef<Offset>,
    pub type_id: TypeId,
}

impl<Offset: gimli::ReaderOffset> VarRef<Offset> {
    pub fn new(entry_ref: EntryRef<Offset>, type_id: TypeId) -> Self {
        Self { entry_ref, type_id }
    }
}

#[allow(clippy::type_complexity)]
#[derive(Debug)]
pub struct LocFinder<R: gimli::Reader> {
    // todo string table
    base_address: u64,
    locations: HashMap<Rc<str>, u64>,  // location -> address
    addr2line: HashMap<u64, Rc<str>>,  // address -> line
    lines: HashMap<Rc<str>, Vec<u64>>, // filepath -> { line: address }
    funcs: HashMap<Rc<str>, EntryRef<R::Offset>>,
    func_ranges: Ranges<Rc<str>>,
    unit_ranges: Ranges<Rc<str>>,
    main_unit: Option<Rc<str>>, // unit where main func is located
    func_variables: HashMap<Rc<str>, HashMap<Rc<str>, VarRef<R::Offset>>>,
    global_variables: HashMap<Rc<str>, VarRef<R::Offset>>,
}

impl<R: gimli::Reader> LocFinder<R> {
    pub fn new(base_address: u64) -> Self {
        Self {
            base_address,
            locations: HashMap::new(),
            addr2line: HashMap::new(),
            lines: HashMap::new(),
            funcs: HashMap::new(),
            func_ranges: Ranges::new(),
            unit_ranges: Ranges::new(),
            main_unit: None,
            func_variables: HashMap::new(),
            global_variables: HashMap::new(),
        }
    }

    pub fn add_compile_unit(&mut self, name: Rc<str>, low_pc: u64, high_pc: u64) {
        self.unit_ranges.add(self.base_address + low_pc, self.base_address + high_pc, name);
    }

    pub fn add_func_entry_ref(&mut self, name: Rc<str>, entry_ref: EntryRef<R::Offset>) {
        self.funcs.insert(name, entry_ref);
    }

    pub fn add_location(&mut self, name: Rc<str>, address: u64) {
        self.locations.insert(name, self.base_address + address);
    }

    // todo start_address, end_address
    pub fn add_func_range(&mut self, name: Rc<str>, low_pc: u64, high_pc: u64) {
        self.func_ranges.add(self.base_address + low_pc, self.base_address + high_pc, name.clone());

        if name.as_ref() == MAIN_FUNC_NAME {
            // compile unit must be processed by now
            self.main_unit = self.unit_ranges.find_value(self.base_address + low_pc).cloned();
        }
    }

    pub fn add_var(&mut self, name: Rc<str>, var_ref: VarRef<R::Offset>, func_name: Option<Rc<str>>) {
        match func_name {
            Some(func_name) => self.func_variables.entry(func_name).or_default().insert(name, var_ref),
            None => self.global_variables.insert(name, var_ref),
        };
    }

    pub fn add_line(&mut self, filepath: Rc<str>, line: usize, address: u64) {
        let fileline: Rc<str> = Rc::from(format!("{}:{}", filepath, line));

        let address = self.base_address + address;
        self.locations.entry(fileline.clone()).or_insert(address);

        if self.is_func_prologue(address) || self.is_func_epilogue(address) {
            return;
        }

        self.addr2line.insert(address, fileline);

        let lines = self.lines.entry(filepath).or_default();
        // skip empty lines
        while lines.len() < line {
            lines.push(0);
        }
        // save only first line appearance
        if lines.len() == line {
            lines.push(address);
        }
    }

    pub fn find_loc(&self, loc: &str) -> Result<Option<u64>> {
        Ok(self.locations.get(loc).copied())
    }

    pub fn find_line(&self, address: u64) -> Option<Rc<str>> {
        self.addr2line.get(&address).cloned()
    }

    pub fn find_next_line_address(&self, fileline: &str) -> Option<u64> {
        let (filepath, line) = Self::parse_fileline(fileline)?;
        self.lines.get(filepath)?.iter().skip(line as usize + 1).find(|&&address| address != 0).copied()
    }

    pub fn find_func(&self, func_name: &str) -> Option<EntryRef<R::Offset>> {
        self.funcs.get(func_name).copied()
    }

    pub fn find_func_by_address(&self, address: u64) -> Option<Rc<str>> {
        self.func_ranges.find_value(address).cloned()
    }

    pub fn find_unit(&self, address: Option<u64>) -> Option<Rc<str>> {
        match address {
            Some(address) => self.unit_ranges.find_value(address).cloned(),
            None => self.main_unit.clone(),
        }
    }

    pub fn find_func_start(&self, address: u64) -> Option<u64> {
        self.func_ranges.find_range(address).map(|(start, _)| start)
    }

    pub fn find_func_end(&self, address: u64) -> Option<u64> {
        self.func_ranges.find_range(address).map(|(_, end)| end)
    }

    pub fn is_inside_main(&self, address: u64) -> bool {
        match self.find_func_by_address(address) {
            Some(func) => func.as_ref() == MAIN_FUNC_NAME,
            None => false,
        }
    }

    fn is_func_prologue(&self, address: u64) -> bool {
        self.find_func_start(address)
            .map(|start| address - start < FUNC_PROLOGUE_SIZE as u64)
            .unwrap_or(false)
    }

    fn is_func_epilogue(&self, address: u64) -> bool {
        self.find_func_end(address)
            .map(|end| end - address < FUNC_EPILOGUE_SIZE as u64)
            .unwrap_or(false)
    }

    fn parse_fileline(fileline: &str) -> Option<(&str, u64)> {
        fileline
            .rsplit_once(':')
            .and_then(|(filepath, line)| line.parse::<u64>().map(|line| (filepath, line)).ok())
    }

    pub fn get_vars(&self, func_name: Option<&str>) -> HashMap<Rc<str>, VarRef<R::Offset>> {
        let mut vars = HashMap::new();

        for (name, &var_ref) in self.global_variables.iter() {
            vars.insert(name.clone(), var_ref);
        }

        if let Some(func_name) = func_name {
            self.func_variables.get(func_name).inspect(|&func_vars| {
                for (name, &var_ref) in func_vars.iter() {
                    vars.insert(name.clone(), var_ref);
                }
            });
        }

        vars
    }

    pub fn get_var(&self, name: &str, func_name: Option<&str>) -> Option<VarRef<R::Offset>> {
        func_name
            .and_then(|func_name| self.func_variables.get(func_name))
            .and_then(|vars| vars.get(name).copied())
            .or_else(|| self.global_variables.get(name).copied())
    }
}
