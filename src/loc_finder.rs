use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{anyhow, bail, Result};

use crate::utils::ranges::Ranges;

#[derive(Debug)]
pub struct EntryRef<Offset: gimli::ReaderOffset> {
    pub unit_offset: gimli::DebugInfoOffset<Offset>,
    pub entry_offset: gimli::UnitOffset<Offset>,
}

impl<Offset: gimli::ReaderOffset> EntryRef<Offset> {
    pub fn new(unit_offset: gimli::DebugInfoOffset<Offset>, entry_offset: gimli::UnitOffset<Offset>) -> Self {
        Self { unit_offset, entry_offset }
    }
}

#[derive(Debug)]
pub struct LocFinder<R: gimli::Reader> {
    // todo string table
    locations: HashMap<Rc<str>, u64>, // location -> address
    lines: HashMap<u64, Rc<str>>,     // address -> line number
    funcs: HashMap<Rc<str>, EntryRef<R::Offset>>,
    func_ranges: Ranges<Rc<str>>,
    func_variables: HashMap<Rc<str>, HashMap<Rc<str>, EntryRef<R::Offset>>>,
    global_variables: HashMap<Rc<str>, EntryRef<R::Offset>>,
}

impl<R: gimli::Reader> LocFinder<R> {
    pub fn new(dwarf: &gimli::Dwarf<R>) -> Result<Self> {
        let mut loc_finder = Self {
            funcs: HashMap::new(),
            locations: HashMap::new(),
            lines: HashMap::new(),
            func_ranges: Ranges::new(),
            func_variables: HashMap::new(),
            global_variables: HashMap::new(),
        };

        let mut units = dwarf.units();

        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let unit_ref = unit.unit_ref(dwarf);

            // todo worker pool
            loc_finder.find_functions(&unit_ref)?;
            loc_finder.find_lines(&unit_ref)?;
        }

        Ok(loc_finder)
    }

    fn find_functions(&mut self, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
        // todo iterate all entries
        let mut tree = unit_ref.entries_tree(None)?;
        let root = tree.root()?;
        let mut children = root.children();

        while let Some(child) = children.next()? {
            let entry = child.entry();

            match entry.tag() {
                gimli::DW_TAG_subprogram => self.process_subprogram(unit_ref, &entry)?,
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => self.process_var(unit_ref, &entry, None)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn process_subprogram(&mut self, unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>) -> Result<()> {
        let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
        let name: Rc<str> = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);

        let unit_offset = unit_ref.header.offset().as_debug_info_offset().ok_or(anyhow!("can't get debug_info offest"))?;
        let entry_offset = entry.offset();
        let entry_ref = EntryRef::new(unit_offset, entry_offset);

        self.funcs.insert(name.clone(), entry_ref);

        let low_pc_attr = match entry.attr_value(gimli::DW_AT_low_pc)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let low_pc = unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        self.locations.insert(name.clone(), low_pc);
        let high_pc_attr = match entry.attr_value(gimli::DW_AT_high_pc)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let high_pc = match high_pc_attr {
            gimli::AttributeValue::Udata(size) => low_pc + size,
            high_pc => unit_ref.attr_address(high_pc)?.ok_or(anyhow!("get high pc value"))?,
        };

        // high_pc is the address of the first location past the last instruction associated with the entity,
        // so we do -1 because ranges are inclusive
        self.func_ranges.add(low_pc, high_pc - 1, name.clone());

        // process function parameters and variables
        let mut tree = unit_ref.entries_tree(Some(entry.offset()))?;
        let root = tree.root()?;
        let mut children = root.children();
        while let Some(child) = children.next()? {
            self.process_var(unit_ref, child.entry(), Some(name.clone()))?;
        }

        Ok(())
    }

    fn process_var(&mut self, unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>, func_name: Option<Rc<str>>) -> Result<()> {
        let name_attr = match entry.attr_value(gimli::DW_AT_name)? {
            Some(value) => value,
            None => return Ok(()),
        };

        let name: Rc<str> = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
        let unit_offset = unit_ref.header.offset().as_debug_info_offset().ok_or(anyhow!("can't get debug_info offest"))?;
        let entry_offset = entry.offset();
        let entry_ref = EntryRef::new(unit_offset, entry_offset);

        match func_name {
            Some(func_name) => self.func_variables.entry(func_name).or_default().insert(name.clone(), entry_ref),
            None => self.global_variables.insert(name.clone(), entry_ref),
        };

        Ok(())
    }

    fn find_lines(&mut self, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
        let program = match unit_ref.line_program.clone() {
            Some(program) => program,
            None => return Ok(()),
        };
        let mut rows = program.rows();

        while let Some((header, row)) = rows.next_row()? {
            let fileline = Self::get_fileline(unit_ref, row, &header)?;

            self.locations.insert(fileline.clone(), row.address());
            self.lines.insert(row.address(), fileline);
        }

        Ok(())
    }

    fn get_fileline(unit_ref: &gimli::UnitRef<R>, row: &gimli::LineRow, header: &gimli::LineProgramHeader<R>) -> Result<Rc<str>> {
        let path_name = match row.file(&header) {
            Some(file) => unit_ref.attr_string(file.path_name())?,
            None => bail!("get path name"),
        };

        let line = row.line().ok_or(anyhow!("get line number"))?;
        let fileline = format!("{}:{}", path_name.to_string()?, line);

        Ok(Rc::from(fileline))
    }

    pub fn find_loc(&self, loc: &str) -> Option<u64> {
        self.locations.get(loc).copied()
    }

    pub fn find_line(&self, address: u64) -> Option<Rc<str>> {
        self.lines.get(&address).cloned()
    }

    pub fn find_func(&self, func_name: &str) -> Option<&EntryRef<R::Offset>> {
        self.funcs.get(func_name)
    }

    pub fn find_func_by_address(&self, address: u64) -> Option<Rc<str>> {
        self.func_ranges.find_value(address).cloned()
    }

    pub fn get_vars(&self, func_name: Option<&str>) -> HashMap<Rc<str>, &EntryRef<R::Offset>> {
        let mut vars = HashMap::new();

        for (name, entry_ref) in self.global_variables.iter() {
            vars.insert(name.clone(), entry_ref);
        }

        if let Some(func_name) = func_name {
            self.func_variables.get(func_name).inspect(|func_vars| {
                for (name, entry_ref) in func_vars.iter() {
                    vars.insert(name.clone(), entry_ref);
                }
            });
        }

        vars
    }

    pub fn get_var(&self, name: &str, address: u64) -> Option<&EntryRef<R::Offset>> {
        if let Some(entry) = self.global_variables.get(name) {
            return Some(entry);
        }

        self.find_func_by_address(address)
            .and_then(|func| self.func_variables.get(&func))
            .and_then(|vars| vars.get(name))
    }
}
