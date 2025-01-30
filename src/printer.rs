use std::io::{self, Write};

use anyhow::{bail, Result};

use crate::debugger::Debugger;
use crate::var::{Var, VarType};

pub struct Printer<'a, R: gimli::Reader> {
    debugger: &'a Debugger<R>,
}

impl<'a, R: gimli::Reader> Printer<'a, R> {
    pub fn new(debugger: &'a Debugger<R>) -> Self {
        Self { debugger }
    }

    pub fn print(&self, var: &Var) -> Result<()> {
        let mut lock = io::stdout().lock();

        self.print_type(&mut lock, &var.typ)?;
        write!(lock, " {} = ", var.name)?;
        self.print_value(&mut lock, var, &var.typ)?;
        write!(lock, "\n")?;

        Ok(())
    }

    fn print_type(&self, f: &mut impl io::Write, var_type: &VarType) -> io::Result<()> {
        match var_type {
            VarType::Base { name, .. } => write!(f, "{}", name),
            VarType::Const(var_type) => {
                write!(f, "const ")?;
                self.print_type(f, var_type)
            }
            VarType::Pointer(var_type) => {
                self.print_type(f, var_type)?;
                write!(f, "*")
            }
        }
    }

    fn print_value(&self, f: &mut impl io::Write, var: &Var, var_type: &VarType) -> Result<()> {
        match var_type {
            VarType::Base { byte_size, encoding, .. } => {
                match *encoding {
                    gimli::DW_ATE_boolean => write!(f, "{}", var.value != 0)?,
                    gimli::DW_ATE_signed => match *byte_size {
                        2 => write!(f, "{}", var.value as i16)?,
                        4 => write!(f, "{}", var.value as i32)?,
                        8 => write!(f, "{}", var.value as i64)?,
                        _ => bail!("unsupported byte size"),
                    },
                    gimli::DW_ATE_unsigned => match *byte_size {
                        2 => write!(f, "{}", var.value as u16)?,
                        4 => write!(f, "{}", var.value as u32)?,
                        8 => write!(f, "{}", var.value as u64)?,
                        _ => bail!("unsupported byte size"),
                    },
                    _ => bail!("unsupported encoding"),
                };
            }
            VarType::Const(var_type) => self.print_value(f, var, var_type.as_ref())?,
            VarType::Pointer(var_type) => {
                // check for char*
                if let VarType::Base { encoding, .. } = var_type.unwind() {
                    if *encoding == gimli::DW_ATE_signed_char {
                        let s = self.debugger.read_c_string_at(var.value)?;
                        write!(f, "{:?}", s)?;
                        return Ok(());
                    }
                }

                write!(f, "{:#x}", var.value)?
            }
        };

        Ok(())
    }
}
