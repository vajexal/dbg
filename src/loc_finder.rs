use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{anyhow, bail, Result};
use thiserror::Error;

use crate::utils::ranges::Ranges;
use crate::var::{Field, Type, TypeId};

const MAIN_FUNC_NAME: &str = "main";

#[derive(Debug, Error)]
#[error("loc not found")]
pub struct LocNotFound;

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

#[derive(Debug)]
pub struct LocFinder<R: gimli::Reader> {
    // todo string table
    locations: HashMap<Rc<str>, u64>, // location -> address
    lines: HashMap<u64, Rc<str>>,     // address -> line number
    funcs: HashMap<Rc<str>, EntryRef<R::Offset>>,
    func_ranges: Ranges<Rc<str>>,
    unit_ranges: Ranges<Rc<str>>,
    main_unit: Option<Rc<str>>, // unit where main func is located
    func_variables: HashMap<Rc<str>, HashMap<Rc<str>, VarRef<R::Offset>>>,
    global_variables: HashMap<Rc<str>, VarRef<R::Offset>>,
    types: Vec<Type>,
}

impl<R: gimli::Reader> LocFinder<R> {
    pub fn new(dwarf: &gimli::Dwarf<R>) -> Result<Self> {
        let mut loc_finder = Self {
            funcs: HashMap::new(),
            locations: HashMap::new(),
            lines: HashMap::new(),
            func_ranges: Ranges::new(),
            unit_ranges: Ranges::new(),
            main_unit: None,
            func_variables: HashMap::new(),
            global_variables: HashMap::new(),
            types: vec![Type::Void],
        };

        let mut units = dwarf.units();

        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let unit_ref = unit.unit_ref(dwarf);

            // todo worker pool
            loc_finder.process_unit(&unit_ref)?;
            loc_finder.find_lines(&unit_ref)?;
        }

        Ok(loc_finder)
    }

    fn process_unit(&mut self, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
        // todo iterate all entries
        let mut tree = unit_ref.entries_tree(None)?;
        let root = tree.root()?;
        let root_entry = root.entry();
        if root_entry.tag() == gimli::DW_TAG_compile_unit {
            self.process_compile_unit(unit_ref, &root_entry)?;
        }

        let mut visited_types = HashMap::new();
        let mut children = root.children();
        while let Some(child) = children.next()? {
            let entry = child.entry();

            match entry.tag() {
                gimli::DW_TAG_subprogram => self.process_subprogram(unit_ref, entry, &mut visited_types)?,
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => self.process_var(unit_ref, entry, None, &mut visited_types)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn process_compile_unit(&mut self, unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>) -> Result<()> {
        let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
        let name: Rc<str> = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);

        let low_pc_attr = entry.attr_value(gimli::DW_AT_low_pc)?.ok_or(anyhow!("get low_pc attr"))?;
        let low_pc = unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        let high_pc_attr = entry.attr_value(gimli::DW_AT_high_pc)?.ok_or(anyhow!("get high_pc attr"))?;
        let high_pc = match high_pc_attr {
            gimli::AttributeValue::Udata(size) => low_pc + size,
            high_pc => unit_ref.attr_address(high_pc)?.ok_or(anyhow!("get high_pc value"))?,
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
        let low_pc = unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

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
            self.process_var(unit_ref, child.entry(), Some(name.clone()), visited_types)?;
        }

        Ok(())
    }

    fn process_var(
        &mut self,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        func_name: Option<Rc<str>>,
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

        let type_id = self.process_entry_type(unit_ref, entry, visited_types)?;
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
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<TypeId> {
        match entry.attr_value(gimli::DW_AT_type)? {
            Some(value) => match value {
                gimli::AttributeValue::UnitRef(offset) => {
                    let subtype_entry = unit_ref.entry(offset)?;
                    self.process_type(unit_ref, &subtype_entry, visited_types)
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
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<TypeId> {
        let type_id = self.types.len();

        match visited_types.entry(entry.offset()) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => {
                entry.insert(type_id);
                self.types.push(Type::Void); // taking slot
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
                let subtype_id = self.process_entry_type(unit_ref, entry, visited_types)?;

                Type::Const(subtype_id)
            }
            gimli::DW_TAG_pointer_type => {
                let subtype_id = self.process_entry_type(unit_ref, entry, visited_types)?;

                Type::Pointer(subtype_id)
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

                    let member_type_id = self.process_entry_type(unit_ref, child_entry, visited_types)?;

                    fields.push(Field {
                        name: member_name,
                        type_id: member_type_id,
                        offset: member_location,
                    });
                }

                Type::Struct { name, size: byte_size, fields }
            }
            gimli::DW_TAG_typedef => {
                let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
                let name = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
                let subtype_id = self.process_entry_type(unit_ref, entry, visited_types)?;

                Type::Typedef(name, subtype_id)
            }
            _ => bail!("unexpected tag type"),
        };

        self.types[type_id] = typ;

        Ok(type_id)
    }

    pub fn get_type(&self, type_id: TypeId) -> &Type {
        self.types.get(type_id).unwrap()
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

    pub fn find_loc<F>(&self, loc: &str, get_ip_fn: F) -> Result<Option<u64>>
    where
        F: Fn() -> Result<Option<u64>>,
    {
        let loc = loc.trim();

        // if loc is a number, then we'll assume it's a line and therefore search by current unit plus loc
        if loc.parse::<u64>().is_ok() {
            // try to find current unit
            let unit_name = match get_ip_fn()? {
                Some(ip) => self.find_unit(ip),
                None => self.main_unit.clone(),
            };

            return match unit_name {
                Some(unit_name) => {
                    let loc_with_unit = format!("{}:{}", unit_name, loc);
                    Ok(self.locations.get(loc_with_unit.as_str()).copied())
                }
                None => Ok(None),
            };
        }

        Ok(self.locations.get(loc).copied())
    }

    pub fn find_line(&self, address: u64) -> Option<Rc<str>> {
        self.lines.get(&address).cloned()
    }

    pub fn find_func(&self, func_name: &str) -> Option<EntryRef<R::Offset>> {
        self.funcs.get(func_name).copied()
    }

    pub fn find_func_by_address(&self, address: u64) -> Option<Rc<str>> {
        self.func_ranges.find_value(address).cloned()
    }

    pub fn find_unit(&self, address: u64) -> Option<Rc<str>> {
        self.unit_ranges.find_value(address).cloned()
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
        match func_name {
            Some(func_name) => self.func_variables.get(func_name).and_then(|vars| vars.get(name).copied()),
            None => self.global_variables.get(name).copied(),
        }
    }
}
