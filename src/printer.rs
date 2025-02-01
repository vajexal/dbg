use std::io::{self, Write};

use anyhow::{bail, Result};
use bytes::{Buf, Bytes};
use thiserror::Error;

use crate::debugger::Debugger;
use crate::var::{Field, Var, VarType};

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
        let mut lock = io::stdout().lock();

        self.print_type(&mut lock, &var.typ, path)?;
        let name = path.last().copied().unwrap_or(var.name.as_str());
        write!(lock, " {} = ", name)?;
        self.print_value(&mut lock, var, path)?;
        write!(lock, "\n")?;

        Ok(())
    }

    fn print_type(&self, f: &mut impl io::Write, var_type: &VarType, path: &[&str]) -> Result<()> {
        match var_type {
            VarType::Base { name, .. } => {
                if !path.is_empty() {
                    bail!(InvalidPathError);
                }

                write!(f, "{}", name)?;
            }
            VarType::Const(var_type) => {
                write!(f, "const ")?;
                self.print_type(f, var_type, path)?;
            }
            VarType::Pointer(var_type) => {
                if !path.is_empty() {
                    // todo follow fields behind pointer
                    bail!(InvalidPathError);
                }

                self.print_type(f, var_type, path)?;
                write!(f, "*")?;
            }
            VarType::Struct { name, fields, .. } => {
                if path.is_empty() {
                    write!(f, "{}", name)?;
                } else {
                    match fields.iter().find(|field| field.name == path[0]) {
                        Some(field) => self.print_type(f, &field.typ, &path[1..])?,
                        None => bail!(InvalidPathError),
                    }
                }
            }
            VarType::Typedef(name, var_type) => {
                if path.is_empty() {
                    write!(f, "{}", name)?;
                } else {
                    self.print_type(f, var_type, path)?;
                }
            }
        }

        Ok(())
    }

    fn print_value(&self, f: &mut impl io::Write, var: &Var<R>, path: &[&str]) -> Result<()> {
        let size = var.typ.get_size();
        let buf = self.debugger.read_location(&var.location, size)?;
        self.print_bytes(f, buf, &var.typ, path)
    }

    fn print_bytes(&self, f: &mut impl io::Write, mut buf: Bytes, var_type: &VarType, path: &[&str]) -> Result<()> {
        if Self::is_c_string_type(var_type) {
            return self.print_c_string(f, buf, path);
        }

        match var_type {
            VarType::Base { encoding, size, .. } => {
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
                    _ => bail!("unsupported encoding"),
                };
            }
            VarType::Const(var_type) => self.print_bytes(f, buf, var_type, path)?,
            VarType::Pointer(_) => {
                if !path.is_empty() {
                    // todo follow fields behind pointer
                    bail!(InvalidPathError);
                }

                let ptr = buf.get_u64_ne();
                if ptr == 0 {
                    write!(f, "null")?;
                } else {
                    write!(f, "{:#x}", ptr)?;
                }
            }
            VarType::Struct { fields, .. } => {
                if path.is_empty() {
                    self.print_struct_bytes(f, buf, fields)?;
                } else {
                    match fields.iter().find(|field| field.name == path[0]) {
                        Some(field) => self.print_bytes(f, buf.slice((field.offset as usize)..), &field.typ, &path[1..])?,
                        None => bail!(InvalidPathError),
                    }
                }
            }
            VarType::Typedef(_, var_type) => self.print_bytes(f, buf, var_type, path)?,
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

            self.print_bytes(f, buf.slice((field.offset as usize)..), &field.typ, &[])?;
        }

        write!(f, " }}")?;

        Ok(())
    }

    fn is_c_string_type(var_type: &VarType) -> bool {
        if let VarType::Pointer(sub_type) = var_type {
            if let VarType::Base { encoding, .. } = sub_type.unwind_const() {
                if *encoding == gimli::DW_ATE_signed_char {
                    return true;
                }
            }
        }

        false
    }

    fn print_c_string(&self, f: &mut impl io::Write, mut buf: Bytes, path: &[&str]) -> Result<()> {
        if !path.is_empty() {
            bail!(InvalidPathError);
        }

        let ptr = buf.get_u64_ne();
        let s = self.debugger.read_c_string_at(ptr)?;
        write!(f, "{:?}", s)?;

        Ok(())
    }
}
