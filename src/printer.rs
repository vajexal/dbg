use std::io::{self, Write};

use anyhow::{bail, Result};
use bytes::{Buf, Bytes};

use crate::debugger::{Debugger, WORD_SIZE};
use crate::var::{Var, VarType};

pub struct Printer<'a, R: gimli::Reader> {
    debugger: &'a Debugger<R>,
}

impl<'a, R: gimli::Reader> Printer<'a, R> {
    pub fn new(debugger: &'a Debugger<R>) -> Self {
        Self { debugger }
    }

    pub fn print(&self, var: &Var<R>) -> Result<()> {
        let mut lock = io::stdout().lock();

        self.print_type(&mut lock, &var.typ)?;
        write!(lock, " {} = ", var.name)?;
        self.print_value(&mut lock, var)?;
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
            VarType::Struct { name, .. } => write!(f, "{}", name),
        }
    }

    fn print_value(&self, f: &mut impl io::Write, var: &Var<R>) -> Result<()> {
        let size = match var.typ.unwind() {
            VarType::Base { size, .. } => *size as usize,
            VarType::Const(_) => panic!("can't get const type size"),
            VarType::Pointer(_) => WORD_SIZE,
            VarType::Struct { size, .. } => *size as usize,
        };

        let buf = self.debugger.read_location(&var.location, size)?;
        self.print_bytes(f, buf,&var.typ)
    }

    fn print_bytes(&self, f: &mut impl io::Write, mut buf: Bytes, var_type: &VarType) -> Result<()> {
        match var_type {
            VarType::Base { encoding, size, .. } => {
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
            VarType::Const(var_type) => self.print_bytes(f, buf, var_type.as_ref())?,
            VarType::Pointer(var_type) => {
                let ptr = buf.get_u64_ne();

                // check for char*
                if let VarType::Base { encoding, .. } = var_type.unwind() {
                    if *encoding == gimli::DW_ATE_signed_char {
                        let s = self.debugger.read_c_string_at(ptr)?;
                        write!(f, "{:?}", s)?;
                        return Ok(());
                    }
                }

                write!(f, "{:#x}", ptr)?
            }
            VarType::Struct { fields, .. } => {
                write!(f, "{{ ")?;

                for (i, field) in fields.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} = ", field.name)?;

                    self.print_bytes(f, buf.slice((field.offset as usize)..), &field.typ)?;
                }

                write!(f, " }}")?;
            },
        };

        Ok(())
    }
}
