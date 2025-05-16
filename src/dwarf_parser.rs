use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{anyhow, bail, Context, Result};

use crate::loc_finder::{EntryRef, LocFinder, VarRef};
use crate::types::{EnumVariant, Field, Type, TypeId, TypeStorage, UnionField, VOID_TYPE_ID};

pub struct DwarfParser;

impl DwarfParser {
    pub fn parse<R: gimli::Reader>(dwarf: &gimli::Dwarf<R>, base_address: u64) -> Result<(LocFinder<R>, TypeStorage)> {
        let mut loc_finder = LocFinder::new(base_address);
        let mut type_storage = TypeStorage::new();

        let mut units = dwarf.units();

        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let unit_ref = unit.unit_ref(dwarf);

            // todo worker pool
            Self::process_unit(&mut loc_finder, &mut type_storage, &unit_ref)?;
            Self::find_lines(&mut loc_finder, &unit_ref)?;
        }

        Ok((loc_finder, type_storage))
    }

    fn process_unit<R: gimli::Reader>(loc_finder: &mut LocFinder<R>, type_storage: &mut TypeStorage, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
        // todo iterate all entries
        let mut tree = unit_ref.entries_tree(None)?;
        let root = tree.root()?;
        let root_entry = root.entry();
        if root_entry.tag() == gimli::DW_TAG_compile_unit {
            Self::process_compile_unit(loc_finder, unit_ref, root_entry)?;
        }

        let mut visited_types = HashMap::new();
        let mut children = root.children();
        while let Some(child) = children.next()? {
            let entry = child.entry();

            match entry.tag() {
                gimli::DW_TAG_subprogram => Self::process_subprogram(loc_finder, type_storage, unit_ref, entry, &mut visited_types)?,
                gimli::DW_TAG_variable => Self::process_var(loc_finder, type_storage, unit_ref, entry, None, &mut visited_types)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn process_compile_unit<R: gimli::Reader>(
        loc_finder: &mut LocFinder<R>,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
    ) -> Result<()> {
        let name = Self::get_name(unit_ref, entry)?;

        let low_pc_attr = entry.attr_value(gimli::DW_AT_low_pc)?.ok_or(anyhow!("get low_pc attr"))?;
        let low_pc = unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        let high_pc_attr = entry.attr_value(gimli::DW_AT_high_pc)?.ok_or(anyhow!("get high_pc attr"))?;
        let high_pc = match high_pc_attr {
            gimli::AttributeValue::Udata(size) => low_pc + size,
            high_pc => unit_ref.attr_address(high_pc)?.ok_or(anyhow!("get high_pc value"))?,
        };

        // high_pc is the address of the first location past the last instruction associated with the entity,
        // so we do -1 because ranges are inclusive
        loc_finder.add_compile_unit(name, low_pc, high_pc - 1);

        Ok(())
    }

    fn process_subprogram<R: gimli::Reader>(
        loc_finder: &mut LocFinder<R>,
        type_storage: &mut TypeStorage,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<()> {
        let name = Self::get_name(unit_ref, entry)?;

        let unit_offset = unit_ref.header.offset().as_debug_info_offset().ok_or(anyhow!("can't get debug_info offest"))?;
        let entry_offset = entry.offset();
        let entry_ref = EntryRef::new(unit_offset, entry_offset);

        loc_finder.add_func_entry_ref(name.clone(), entry_ref);

        let low_pc_attr = match entry.attr_value(gimli::DW_AT_low_pc)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let low_pc = unit_ref.attr_address(low_pc_attr)?.ok_or(anyhow!("get low_pc value"))?;

        loc_finder.add_location(name.clone(), low_pc);

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
        loc_finder.add_func_range(name.clone(), low_pc, high_pc - 1);

        // process function parameters and variables
        let mut tree = unit_ref.entries_tree(Some(entry.offset()))?;
        let root = tree.root()?;
        let mut children = root.children();
        while let Some(child) = children.next()? {
            let child_entry = child.entry();
            match child_entry.tag() {
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => {
                    Self::process_var(loc_finder, type_storage, unit_ref, child_entry, Some(name.clone()), visited_types)?
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn process_var<R: gimli::Reader>(
        loc_finder: &mut LocFinder<R>,
        type_storage: &mut TypeStorage,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        func_name: Option<Rc<str>>,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<()> {
        let name = match Self::get_optional_name(unit_ref, entry)? {
            Some(name) => name,
            None => return Ok(()),
        };

        let unit_offset = unit_ref.header.offset().as_debug_info_offset().ok_or(anyhow!("can't get debug_info offest"))?;
        let entry_offset = entry.offset();
        let entry_ref = EntryRef::new(unit_offset, entry_offset);

        let type_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;
        let var_ref = VarRef::new(entry_ref, type_id);

        loc_finder.add_var(name, var_ref, func_name);

        Ok(())
    }

    fn process_entry_type<R: gimli::Reader>(
        type_storage: &mut TypeStorage,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
        visited_types: &mut HashMap<gimli::UnitOffset<R::Offset>, TypeId>,
    ) -> Result<TypeId> {
        match entry.attr_value(gimli::DW_AT_type)? {
            Some(value) => match value {
                gimli::AttributeValue::UnitRef(offset) => {
                    let subtype_entry = unit_ref.entry(offset)?;
                    Self::process_type(type_storage, unit_ref, &subtype_entry, visited_types)
                        .with_context(|| format!("failed to process type at {:?}", subtype_entry.offset().to_unit_section_offset(unit_ref.unit)))
                }
                _ => bail!("unknown type"),
            },
            None => Ok(VOID_TYPE_ID),
        }
    }

    fn process_type<R: gimli::Reader>(
        type_storage: &mut TypeStorage,
        unit_ref: &gimli::UnitRef<R>,
        entry: &gimli::DebuggingInformationEntry<R>,
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
                let name = Self::get_name(unit_ref, entry)?;
                let encoding_attr = entry.attr_value(gimli::DW_AT_encoding)?.ok_or(anyhow!("get encoding value"))?;
                let encoding = match encoding_attr {
                    gimli::AttributeValue::Encoding(encoding) => encoding,
                    _ => bail!("unexpected encoding attr value"),
                };
                let size = Self::get_byte_size(entry)?;

                Type::Base { name, encoding, size }
            }
            gimli::DW_TAG_const_type => {
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                Type::Const(subtype_id)
            }
            gimli::DW_TAG_volatile_type => {
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                Type::Volatile(subtype_id)
            }
            gimli::DW_TAG_atomic_type => {
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                Type::Atomic(subtype_id)
            }
            gimli::DW_TAG_pointer_type => {
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                match type_storage.unwind_type(subtype_id)? {
                    Type::Base { encoding, .. } => {
                        // check for c-string
                        if encoding == gimli::DW_ATE_signed_char {
                            Type::String(subtype_id)
                        } else {
                            Type::Pointer(subtype_id)
                        }
                    }
                    Type::FuncDef { .. } => Type::Func(subtype_id),
                    _ => Type::Pointer(subtype_id),
                }
            }
            gimli::DW_TAG_array_type => {
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;
                let dimensions = Self::map_subtree(unit_ref, entry, gimli::DW_TAG_subrange_type, |child_entry| {
                    let count = match child_entry.attr_value(gimli::DW_AT_count)? {
                        Some(value) => value.udata_value().ok_or(anyhow!("get count attr value"))?,
                        None => {
                            let lower_bound = match child_entry.attr_value(gimli::DW_AT_lower_bound)? {
                                Some(value) => value.udata_value().ok_or(anyhow!("get lower bound attr value"))?,
                                None => 0,
                            };

                            let upper_bound = child_entry
                                .attr_value(gimli::DW_AT_upper_bound)?
                                .ok_or(anyhow!("no attributes to find dimension upper bound"))?
                                .udata_value()
                                .ok_or(anyhow!("get upper bound attr value"))?;

                            upper_bound - lower_bound + 1
                        }
                    };

                    Ok(count as usize)
                })?;

                // create type for every nested dimension
                let subtype_id = dimensions
                    .iter()
                    .skip(1)
                    .copied()
                    .rev()
                    .fold(subtype_id, |subtype_id, count| type_storage.add(Type::Array { subtype_id, count }));

                Type::Array {
                    subtype_id,
                    count: dimensions[0],
                }
            }
            gimli::DW_TAG_structure_type => {
                let name = Self::get_optional_name(unit_ref, entry)?;
                let size = Self::get_byte_size(entry)?;

                let fields = Self::map_subtree(unit_ref, entry, gimli::DW_TAG_member, |child_entry| {
                    let member_name = Self::get_name(unit_ref, child_entry)?;

                    // todo location
                    let member_location = child_entry
                        .attr_value(gimli::DW_AT_data_member_location)?
                        .ok_or(anyhow!("get data member location attr value"))?
                        .u16_value()
                        .ok_or(anyhow!("convert data member location to u16"))?;

                    let member_type_id = Self::process_entry_type(type_storage, unit_ref, child_entry, visited_types)?;

                    Ok(Field {
                        name: member_name,
                        type_id: member_type_id,
                        offset: member_location,
                    })
                })?;

                Type::Struct {
                    name,
                    size,
                    fields: Rc::from(fields),
                }
            }
            gimli::DW_TAG_enumeration_type => {
                let name = Self::get_optional_name(unit_ref, entry)?;
                let (encoding, size) = match entry.attr_value(gimli::DW_AT_type)? {
                    Some(_) => {
                        let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;
                        match type_storage.get(subtype_id)? {
                            Type::Base { encoding, size, .. } => (encoding, size),
                            _ => bail!("invalid enum subtype"),
                        }
                    }
                    None => {
                        let encoding_attr = entry.attr_value(gimli::DW_AT_encoding)?.ok_or(anyhow!("get encoding value"))?;
                        let encoding = match encoding_attr {
                            gimli::AttributeValue::Encoding(encoding) => encoding,
                            _ => bail!("unexpected encoding attr value"),
                        };
                        let size = Self::get_byte_size(entry)?;

                        (encoding, size)
                    }
                };

                let variants = Self::map_subtree(unit_ref, entry, gimli::DW_TAG_enumerator, |child_entry| {
                    let variant_name = Self::get_name(unit_ref, child_entry)?;
                    let variant_value = child_entry
                        .attr_value(gimli::DW_AT_const_value)?
                        .ok_or(anyhow!("get const value attr"))?
                        .sdata_value()
                        .ok_or(anyhow!("get variant value"))?;

                    Ok(EnumVariant {
                        name: variant_name,
                        value: variant_value,
                    })
                })?;

                Type::Enum {
                    name,
                    encoding,
                    size,
                    variants: Rc::new(variants),
                }
            }
            gimli::DW_TAG_union_type => {
                let name = Self::get_optional_name(unit_ref, entry)?;
                let size = Self::get_byte_size(entry)?;

                let fields = Self::map_subtree(unit_ref, entry, gimli::DW_TAG_member, |child_entry| {
                    let name = Self::get_name(unit_ref, child_entry)?;
                    let type_id = Self::process_entry_type(type_storage, unit_ref, child_entry, visited_types)?;

                    Ok(UnionField { name, type_id })
                })?;

                Type::Union {
                    name,
                    size,
                    fields: Rc::new(fields),
                }
            }
            gimli::DW_TAG_typedef => {
                let name = Self::get_name(unit_ref, entry)?;
                let subtype_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                Type::Typedef(name, subtype_id)
            }
            gimli::DW_TAG_subroutine_type => {
                let name = Self::get_optional_name(unit_ref, entry)?;
                let return_type_id = Self::process_entry_type(type_storage, unit_ref, entry, visited_types)?;

                let args = Self::map_subtree(unit_ref, entry, gimli::DW_TAG_formal_parameter, |child_entry| {
                    Self::process_entry_type(type_storage, unit_ref, child_entry, visited_types)
                })?;

                Type::FuncDef {
                    name,
                    return_type_id,
                    args: Rc::new(args),
                }
            }
            tag_type => bail!("unexpected tag type {}", tag_type),
        };

        type_storage.replace(type_id, typ)?;

        Ok(type_id)
    }

    fn get_name<R: gimli::Reader>(unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>) -> Result<Rc<str>> {
        let name_attr = entry.attr_value(gimli::DW_AT_name)?.ok_or(anyhow!("get name attr value"))?;
        let name = Rc::from(unit_ref.attr_string(name_attr)?.to_string()?);
        Ok(name)
    }

    fn get_optional_name<R: gimli::Reader>(unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>) -> Result<Option<Rc<str>>> {
        match entry.attr_value(gimli::DW_AT_name)? {
            Some(value) => Ok(Some(Rc::from(unit_ref.attr_string(value)?.to_string()?))),
            None => Ok(None),
        }
    }

    fn get_byte_size<R: gimli::Reader>(entry: &gimli::DebuggingInformationEntry<R>) -> Result<u16> {
        entry
            .attr_value(gimli::DW_AT_byte_size)?
            .ok_or(anyhow!("get byte size value"))?
            .u16_value()
            .ok_or(anyhow!("convert byte size to u16"))
    }

    fn map_subtree<R, F, T>(unit_ref: &gimli::UnitRef<R>, entry: &gimli::DebuggingInformationEntry<R>, entry_tag: gimli::DwTag, mut f: F) -> Result<Vec<T>>
    where
        R: gimli::Reader,
        F: FnMut(&gimli::DebuggingInformationEntry<R>) -> Result<T>,
    {
        let mut tree = unit_ref.entries_tree(Some(entry.offset()))?;
        let root = tree.root()?;
        let mut children = root.children();
        let mut result = Vec::new();
        while let Some(child) = children.next()? {
            let child_entry = child.entry();
            if child_entry.tag() == entry_tag {
                let value = f(child.entry())?;
                result.push(value);
            }
        }

        Ok(result)
    }

    fn find_lines<R: gimli::Reader>(loc_finder: &mut LocFinder<R>, unit_ref: &gimli::UnitRef<R>) -> Result<()> {
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

            let line = row.line().ok_or(anyhow!("get line number"))?.get() as usize;

            loc_finder.add_line(filepath, line, row.address(), row.end_sequence());
        }

        Ok(())
    }
}
