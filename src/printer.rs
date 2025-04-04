use std::io::{self, Write};

use anyhow::{bail, Result};
use bytes::Buf;

use crate::error::DebuggerError;
use crate::session::DebugSession;
use crate::var::{Type, TypeId, Value, Var};

pub struct Printer<'a, R: gimli::Reader> {
    session: &'a DebugSession<R>,
}

impl<'a, R: gimli::Reader> Printer<'a, R> {
    pub fn new(session: &'a DebugSession<R>) -> Self {
        Self { session }
    }

    pub fn print(&self, var: &Var) -> Result<()> {
        // we don't use stdout lock because we want print nothing in case of error
        let mut buf = Vec::new();

        self.print_type(&mut buf, var.value.type_id)?;
        write!(buf, " {} = ", var.name)?;
        self.print_value(&mut buf, var.value.clone())?;

        println!("{}", std::str::from_utf8(&buf)?);

        Ok(())
    }

    fn print_type(&self, f: &mut impl io::Write, type_id: TypeId) -> Result<()> {
        match self.session.get_type(type_id) {
            Type::Void => {
                write!(f, "void")?;
            }
            Type::Base { name, encoding, .. } => {
                match *encoding {
                    gimli::DW_ATE_boolean => write!(f, "bool")?,
                    _ => write!(f, "{}", name)?,
                };
            }
            Type::Const(subtype_id) => {
                write!(f, "const ")?;
                self.print_type(f, *subtype_id)?;
            }
            Type::Pointer(subtype_id) => {
                self.print_type(f, *subtype_id)?;
                write!(f, "*")?;
            }
            Type::Struct { name, .. } => {
                write!(f, "{}", name)?;
            }
            Type::Typedef(name, _) => {
                write!(f, "{}", name)?;
            }
        }

        Ok(())
    }

    fn print_value(&self, f: &mut impl io::Write, mut value: Value) -> Result<()> {
        let typ = self.session.get_type(value.type_id);

        match typ {
            Type::Void => bail!(DebuggerError::InvalidPath),
            Type::Base { encoding, size, .. } => {
                match *encoding {
                    gimli::DW_ATE_boolean => write!(f, "{}", value.buf.get_u8() != 0)?,
                    gimli::DW_ATE_signed => match size {
                        1 => write!(f, "{}", value.buf.get_i8())?,
                        2 => write!(f, "{}", value.buf.get_i16_ne())?,
                        4 => write!(f, "{}", value.buf.get_i32_ne())?,
                        8 => write!(f, "{}", value.buf.get_i64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    gimli::DW_ATE_unsigned => match size {
                        1 => write!(f, "{}", value.buf.get_u8())?,
                        2 => write!(f, "{}", value.buf.get_u16_ne())?,
                        4 => write!(f, "{}", value.buf.get_u32_ne())?,
                        8 => write!(f, "{}", value.buf.get_u64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    gimli::DW_ATE_float => match size {
                        4 => write!(f, "{}", value.buf.get_f32_ne())?,
                        8 => write!(f, "{}", value.buf.get_f64_ne())?,
                        _ => bail!("unsupported byte size"),
                    },
                    _ => bail!("unsupported encoding"),
                };
            }
            Type::Const(subtype_id) => self.print_value(f, Value::new(*subtype_id, value.buf))?,
            Type::Pointer(subtype_id) => {
                let ptr = value.buf.get_u64_ne();
                // is it null?
                if ptr == 0 {
                    return Ok(write!(f, "null")?);
                }

                // is it c-string?
                let subtype = self.unwind_type(*subtype_id);
                if let Type::Base { encoding, .. } = subtype {
                    if *encoding == gimli::DW_ATE_signed_char {
                        let s = self.session.read_c_string_at(ptr)?;
                        write!(f, "{:?}", s)?;
                        return Ok(());
                    }
                }

                // is it void* ?
                if let Type::Void = subtype {
                    write!(f, "{:#x}", ptr)?;
                    return Ok(());
                }

                write!(f, "&")?;
                let size = self.session.get_type_size(*subtype_id)?;
                let buf = self.session.read_address(ptr, size)?;
                self.print_value(f, Value::new(*subtype_id, buf))?;
            }
            Type::Struct { fields, .. } => {
                write!(f, "{{ ")?;

                for (i, field) in fields.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} = ", field.name)?;
                    self.print_value(f, Value::new(field.type_id, value.buf.slice((field.offset as usize)..)))?;
                }

                write!(f, " }}")?;
            }
            Type::Typedef(_, subtype_id) => self.print_value(f, Value::new(*subtype_id, value.buf))?,
        };

        Ok(())
    }

    fn unwind_type(&self, type_id: TypeId) -> &Type {
        let mut typ = self.session.get_type(type_id);
        loop {
            typ = match typ {
                Type::Const(subtype_id) => self.session.get_type(*subtype_id),
                Type::Typedef(_, subtype_id) => self.session.get_type(*subtype_id),
                _ => break,
            }
        }

        typ
    }
}
