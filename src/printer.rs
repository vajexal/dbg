use std::io::{self, Write};

use anyhow::{bail, Result};
use bytes::{Buf, Bytes};
use thiserror::Error;

use crate::debugger::Debugger;
use crate::var::{Field, Type, TypeId, Var};

#[derive(Error, Debug)]
#[error("invalid path")]
pub struct InvalidPathError;

pub struct Printer<'a, R: gimli::Reader> {
    debugger: &'a Debugger<R>,
}

impl<'a, R: gimli::Reader> Printer<'a, R> {
    pub fn new(debugger: &'a Debugger<R>) -> Self {
        Self { debugger }
    }

    pub fn print(&self, var: &Var<R>, path: &[&str]) -> Result<()> {
        // we don't use stdout lock because we want print nothing in case of error
        let mut buf = Vec::new();

        self.print_type(&mut buf, var.type_id, path)?;
        let name = path.last().copied().unwrap_or(var.name.as_str());
        write!(buf, " {} = ", name)?;
        self.print_value(&mut buf, var, path)?;

        println!("{}", std::str::from_utf8(&buf)?);

        Ok(())
    }

    fn print_type(&self, f: &mut impl io::Write, type_id: TypeId, path: &[&str]) -> Result<()> {
        match self.debugger.get_type(type_id) {
            Type::Void => {
                if !path.is_empty() {
                    bail!(InvalidPathError);
                }
                write!(f, "void")?;
            }
            Type::Base { name, encoding, .. } => {
                if !path.is_empty() {
                    bail!(InvalidPathError);
                }
                match *encoding {
                    gimli::DW_ATE_boolean => write!(f, "bool")?,
                    _ => write!(f, "{}", name)?,
                };
            }
            Type::Const(subtype_id) => {
                write!(f, "const ")?;
                self.print_type(f, *subtype_id, path)?;
            }
            Type::Pointer(subtype_id) => {
                self.print_type(f, *subtype_id, path)?;
                if path.is_empty() {
                    write!(f, "*")?;
                }
            }
            Type::Struct { name, fields, .. } => {
                if path.is_empty() {
                    write!(f, "{}", name)?;
                } else {
                    match fields.iter().find(|field| field.name.as_ref() == path[0]) {
                        Some(field) => self.print_type(f, field.type_id, &path[1..])?,
                        None => bail!(InvalidPathError),
                    }
                }
            }
            Type::Typedef(name, subtype_id) => {
                if path.is_empty() {
                    write!(f, "{}", name)?;
                } else {
                    self.print_type(f, *subtype_id, path)?;
                }
            }
        }

        Ok(())
    }

    fn print_value(&self, f: &mut impl io::Write, var: &Var<R>, path: &[&str]) -> Result<()> {
        let size = self.debugger.get_type_size(var.type_id);
        let buf = self.debugger.read_location(&var.location, size)?;
        self.print_bytes(f, buf, var.type_id, path)
    }

    fn print_bytes(&self, f: &mut impl io::Write, mut buf: Bytes, type_id: TypeId, path: &[&str]) -> Result<()> {
        let typ = self.debugger.get_type(type_id);

        match typ {
            Type::Void => bail!(InvalidPathError),
            Type::Base { encoding, size, .. } => {
                if !path.is_empty() {
                    bail!(InvalidPathError);
                }

                match *encoding {
                    gimli::DW_ATE_boolean => write!(f, "{}", buf.get_u8() != 0)?,
                    gimli::DW_ATE_signed => match size {
                        1 => write!(f, "{}", buf.get_i8())?,
                        2 => write!(f, "{}", buf.get_i16_ne())?,
                        4 => write!(f, "{}", buf.get_i32_ne())?,
                        8 => write!(f, "{}", buf.get_i64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    gimli::DW_ATE_unsigned => match size {
                        1 => write!(f, "{}", buf.get_u8())?,
                        2 => write!(f, "{}", buf.get_u16_ne())?,
                        4 => write!(f, "{}", buf.get_u32_ne())?,
                        8 => write!(f, "{}", buf.get_u64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    gimli::DW_ATE_float => match size {
                        4 => write!(f, "{}", buf.get_f32_ne())?,
                        8 => write!(f, "{}", buf.get_f64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    _ => bail!("unsupported encoding"),
                };
            }
            Type::Const(subtype_id) => self.print_bytes(f, buf, *subtype_id, path)?,
            Type::Pointer(subtype_id) => {
                let ptr = buf.get_u64_ne();
                // is it null?
                if ptr == 0 {
                    if !path.is_empty() {
                        bail!(InvalidPathError);
                    }
                    return Ok(write!(f, "null")?);
                }

                // is it c-string?
                let subtype = self.unwind_type(*subtype_id);
                if let Type::Base { encoding, .. } = subtype {
                    if *encoding == gimli::DW_ATE_signed_char {
                        if !path.is_empty() {
                            bail!(InvalidPathError);
                        }
                        return self.print_c_string(f, ptr, path);
                    }
                }

                // is it void* ?
                if let Type::Void = subtype {
                    if !path.is_empty() {
                        bail!(InvalidPathError);
                    }
                    write!(f, "{:#x}", ptr)?;
                    return Ok(());
                }

                if path.is_empty() {
                    write!(f, "&")?;
                }
                let size = self.debugger.get_type_size(*subtype_id);
                let buf = self.debugger.read_address(ptr, size)?;
                self.print_bytes(f, buf, *subtype_id, path)?;
            }
            Type::Struct { fields, .. } => {
                if path.is_empty() {
                    self.print_struct_bytes(f, buf, fields)?;
                } else {
                    match fields.iter().find(|field| field.name.as_ref() == path[0]) {
                        Some(field) => self.print_bytes(f, buf.slice((field.offset as usize)..), field.type_id, &path[1..])?,
                        None => bail!(InvalidPathError),
                    }
                }
            }
            Type::Typedef(_, subtype_id) => self.print_bytes(f, buf, *subtype_id, path)?,
        };

        Ok(())
    }

    fn print_struct_bytes(&self, f: &mut impl io::Write, buf: Bytes, fields: &Vec<Field>) -> Result<()> {
        write!(f, "{{ ")?;

        for (i, field) in fields.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{} = ", field.name)?;

            self.print_bytes(f, buf.slice((field.offset as usize)..), field.type_id, &[])?;
        }

        write!(f, " }}")?;

        Ok(())
    }

    fn unwind_type(&self, type_id: TypeId) -> &Type {
        let mut typ = self.debugger.get_type(type_id);
        loop {
            typ = match typ {
                Type::Const(subtype_id) => self.debugger.get_type(*subtype_id),
                Type::Typedef(_, subtype_id) => self.debugger.get_type(*subtype_id),
                _ => break,
            }
        }

        typ
    }

    fn print_c_string(&self, f: &mut impl io::Write, ptr: u64, path: &[&str]) -> Result<()> {
        if !path.is_empty() {
            bail!(InvalidPathError);
        }

        let s = self.debugger.read_c_string_at(ptr)?;
        write!(f, "{:?}", s)?;

        Ok(())
    }
}
