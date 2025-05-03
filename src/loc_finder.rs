use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{anyhow, bail, Result};

use crate::types::{Field, Type, TypeId, TypeStorage};
use crate::utils::ranges::Ranges;

const MAIN_FUNC_NAME: &str = "main";

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

const VOID_TYPE_ID: TypeId = 0;

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
    pub fn make(dwarf: &gimli::Dwarf<R>, base_address: u64) -> Result<(Self, TypeStorage)> {
        let mut loc_finder = Self {
            base_address,
            funcs: HashMap::new(),
            locations: HashMap::new(),
            addr2line: HashMap::new(),
            lines: HashMap::new(),
            func_ranges: Ranges::new(),
            unit_ranges: Ranges::new(),
            main_unit: None,
            func_variables: HashMap::new(),
            global_variables: HashMap::new(),
        };
        let mut type_storage = TypeStorage::new();

        let mut units = dwarf.units();

        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let unit_ref = unit.unit_ref(dwarf);

            // todo worker pool
            loc_finder.process_unit(&unit_ref, &mut type_storage)?;
            loc_finder.find_lines(&unit_ref)?;
        }

        Ok((loc_finder, type_storage))
    }

    fn process_unit(&mut self, unit_ref: &gimli::UnitRef<R>, type_storage: &mut TypeStorage) -> Result<()> {
        // todo iterate all entries
        let mut tree = unit_ref.entries_tree(None)?;
        let root = tree.root()?;
        let root_entry = root.entry();
        if root_entry.tag() == gimli::DW_TAG_compile_unit {
            self.process_compile_unit(unit_ref, root_entry)?;
        }

        let mut visited_types = HashMap::new();
        let mut children = root.children();
        while let Some(child) = children.next()? {
            let entry = child.entry();

            match entry.tag() {
                gimli::DW_TAG_subprogram => self.process_subprogram(unit_ref, entry, type_storage, &mut visited_types)?,
                gimli::DW_TAG_variable => self.process_var(unit_ref, entry, None, type_storage, &mut visited_types)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn process_compile_unit(&mut self, unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>) -> Result<()> {
        let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
        let name: Rc<str> = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);

        let low_pc_attr = entry.attr_value(gimli::DW_AT_low_pc)?.ok_or(anyhow!("get low_pc attr"))?;
        let low_pc = self.base_address + unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        let high_pc_attr = entry.attr_value(gimli::DW_AT_high_pc)?.ok_or(anyhow!("get high_pc attr"))?;
        let high_pc = match high_pc_attr {
            gimli::AttributeValue::Udata(size) => low_pc + size,
            high_pc => self.base_address + unit_ref.attr_address(high_pc)?.ok_or(anyhow!("get high_pc value"))?,
        };

        // high_pc is the address of the first location past the last instruction associated with the entity,
        // so we do -1 because ranges are inclusive
        self.unit_ranges.add(low_pc, high_pc - 1, name);

        Ok(())
    }

    fn process_subprogram(
        &mut self,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        type_storage: &mut TypeStorage,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<()> {
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
        let low_pc = self.base_address + unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        self.locations.insert(name.clone(), low_pc);

        if name.as_ref() == MAIN_FUNC_NAME {
            // compile unit must be processed by now
            self.main_unit = Some(self.unit_ranges.find_value(low_pc).ok_or(anyhow!("can't get main unit"))?.clone());
        }

        let high_pc_attr = match entry.attr_value(gimli::DW_AT_high_pc)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let high_pc = match high_pc_attr {
            gimli::AttributeValue::Udata(size) => low_pc + size,
            high_pc => self.base_address + unit_ref.attr_address(high_pc)?.ok_or(anyhow!("get high pc value"))?,
        };

        // high_pc is the address of the first location past the last instruction associated with the entity,
        // so we do -1 because ranges are inclusive
        self.func_ranges.add(low_pc, high_pc - 1, name.clone());

        // process function parameters and variables
        let mut tree = unit_ref.entries_tree(Some(entry.offset()))?;
        let root = tree.root()?;
        let mut children = root.children();
        while let Some(child) = children.next()? {
            let child_entry = child.entry();
            match child_entry.tag() {
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => {
                    self.process_var(unit_ref, child_entry, Some(name.clone()), type_storage, visited_types)?
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn process_var(
        &mut self,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        func_name: Option<Rc<str>>,
        type_storage: &mut TypeStorage,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<()> {
        let name_attr = match entry.attr_value(gimli::DW_AT_name)? {
            Some(value) => value,
            None => return Ok(()),
        };

        let name: Rc<str> = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
        let unit_offset = unit_ref.header.offset().as_debug_info_offset().ok_or(anyhow!("can't get debug_info offest"))?;
        let entry_offset = entry.offset();
        let entry_ref = EntryRef::new(unit_offset, entry_offset);

        let type_id = self.process_entry_type(unit_ref, entry, type_storage, visited_types)?;
        let var_ref = VarRef::new(entry_ref, type_id);

        match func_name {
            Some(func_name) => self.func_variables.entry(func_name).or_default().insert(name.clone(), var_ref),
            None => self.global_variables.insert(name.clone(), var_ref),
        };

        Ok(())
    }

    fn process_entry_type(
        &mut self,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        type_storage: &mut TypeStorage,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<TypeId> {
        match entry.attr_value(gimli::DW_AT_type)? {
            Some(value) => match value {
                gimli::AttributeValue::UnitRef(offset) => {
                    let subtype_entry = unit_ref.entry(offset)?;
                    self.process_type(unit_ref, &subtype_entry, type_storage, visited_types)
                }
                _ => bail!("unknown type"),
            },
            None => Ok(VOID_TYPE_ID),
        }
    }

    fn process_type(
        &mut self,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        type_storage: &mut TypeStorage,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<TypeId> {
        let type_id = match visited_types.entry(entry.offset()) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let type_id = type_storage.add(Type::Void); // take slot
                entry.insert(type_id);
                type_id
            }
        };

        let typ = match entry.tag() {
            gimli::DW_TAG_base_type => {
                let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
                let name = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
                let encoding_attr = entry.attr_value(gimli::DW_AT_encoding)?.ok_or(anyhow!("get encoding value"))?;
                let encoding = match encoding_attr {
                    gimli::AttributeValue::Encoding(encoding) => encoding,
                    _ => bail!("unexpected encoding attr value"),
                };
                let byte_size = entry
                    .attr_value(gimli::DW_AT_byte_size)?
                    .ok_or(anyhow!("get byte size value"))?
                    .u16_value()
                    .ok_or(anyhow!("convert byte size to u8"))?;

                Type::Base {
                    name,
                    encoding,
                    size: byte_size,
                }
            }
            gimli::DW_TAG_const_type => {
                let subtype_id = self.process_entry_type(unit_ref, entry, type_storage, visited_types)?;

                Type::Const(subtype_id)
            }
            gimli::DW_TAG_pointer_type => {
                let subtype_id = self.process_entry_type(unit_ref, entry, type_storage, visited_types)?;
                let mut typ = Type::Pointer(subtype_id);

                // check for c-string
                let subtype = type_storage.unwind_type(subtype_id)?;
                if let Type::Base { encoding, .. } = subtype {
                    if encoding == gimli::DW_ATE_signed_char {
                        typ = Type::String(subtype_id)
                    }
                }

                typ
            }
            gimli::DW_TAG_structure_type => {
                // struct could be anonymous
                let name = match entry.attr_value(gimli::DW_AT_name)? {
                    Some(value) => Rc::from(unit_ref.attr_string(value)?.to_string()?),
                    None => Rc::from(""),
                };

                let byte_size = entry
                    .attr_value(gimli::DW_AT_byte_size)?
                    .ok_or(anyhow!("get byte size value"))?
                    .u16_value()
                    .ok_or(anyhow!("convert byte size to u8"))?;

                let mut fields = Vec::new();

                let mut tree = unit_ref.entries_tree(Some(entry.offset()))?;
                let root = tree.root()?;
                let mut children = root.children();
                while let Some(child) = children.next()? {
                    let child_entry = child.entry();
                    if child_entry.tag() != gimli::DW_TAG_member {
                        continue;
                    }

                    let member_name_attr = child_entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
                    let member_name = Rc::from(unit_ref.attr_string(member_name_attr)?.to_string()?);

                    // todo location
                    let member_location = child_entry
                        .attr_value(gimli::DW_AT_data_member_location)?
                        .ok_or(anyhow!("get data member location attr value"))?
                        .u16_value()
                        .ok_or(anyhow!("convert data member location to u8"))?;

                    let member_type_id = self.process_entry_type(unit_ref, child_entry, type_storage, visited_types)?;

                    fields.push(Field {
                        name: member_name,
                        type_id: member_type_id,
                        offset: member_location,
                    });
                }

                Type::Struct {
                    name,
                    size: byte_size,
                    fields: Rc::from(fields),
                }
            }
            gimli::DW_TAG_typedef => {
                let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
                let name = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
                let subtype_id = self.process_entry_type(unit_ref, entry, type_storage, visited_types)?;

                Type::Typedef(name, subtype_id)
            }
            _ => bail!("unexpected tag type"),
        };

        type_storage.replace(type_id, typ)?;

        Ok(type_id)
    }

    fn find_lines(&mut self, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
        let program = match unit_ref.line_program.clone() {
            Some(program) => program,
            None => return Ok(()),
        };
        let mut rows = program.rows();

        while let Some((header, row)) = rows.next_row()? {
            let file = match row.file(header) {
                Some(file) => file,
                None => bail!("get path"),
            };

            // build file path
            let mut path = PathBuf::new();
            if file.directory_index() != 0 {
                let dir = file.directory(header).ok_or(anyhow!("get directory"))?;
                path.push(unit_ref.attr_string(dir)?.to_string()?.as_ref());
            }
            path.push(unit_ref.attr_string(file.path_name())?.to_string()?.as_ref());
            let filepath = Rc::from(path.into_os_string().into_string().map_err(|_| anyhow!("convert path to string"))?);

            // build file line
            let line = row.line().ok_or(anyhow!("get line number"))?.get() as usize;
            let fileline: Rc<str> = Rc::from(format!("{}:{}", filepath, line));

            let address = self.base_address + row.address();
            self.locations.insert(fileline.clone(), address);
            self.addr2line.insert(address, fileline);

            if row.end_sequence() {
                continue;
            }

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

        Ok(())
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

    pub fn find_fund_end(&self, address: u64) -> Option<u64> {
        self.func_ranges.find_range(address).map(|(_, end)| end)
    }

    pub fn is_inside_main(&self, address: u64) -> bool {
        match self.find_func_by_address(address) {
            Some(func) => func.as_ref() == MAIN_FUNC_NAME,
            None => false,
        }
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
