use std::io;
use std::io::Write;

use anyhow::{bail, Result};
use bytes::Buf;

use crate::error::DebuggerError;
use crate::session::DebugSession;
use crate::types::{ArrayCount, Type, TypeId};
use crate::var::{Value, Var};

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
        match self.session.get_type_storage().get(type_id)? {
            Type::Void => write!(f, "void")?,
            Type::Base { name, encoding, .. } => {
                match encoding {
                    gimli::DW_ATE_boolean => write!(f, "bool")?,
                    _ => write!(f, "{}", name)?,
                };
            }
            Type::Const(subtype_id) => {
                write!(f, "const ")?;
                self.print_type(f, subtype_id)?;
            }
            Type::Volatile(subtype_id) => {
                write!(f, "volatile ")?;
                self.print_type(f, subtype_id)?;
            }
            Type::Atomic(subtype_id) => {
                write!(f, "_Atomic ")?;
                self.print_type(f, subtype_id)?;
            }
            Type::Pointer(subtype_id) | Type::String(subtype_id) => {
                self.print_type(f, subtype_id)?;
                write!(f, "*")?;
            }
            Type::Array { subtype_id, count } => {
                self.print_type(f, subtype_id)?;
                match count {
                    ArrayCount::Static(count) => write!(f, "[{}]", count)?,
                    ArrayCount::Dynamic(_) | ArrayCount::Flexible => write!(f, "[]")?,
                };
            }
            Type::Struct { name, fields, .. } => match name {
                Some(name) => write!(f, "{}", name)?,
                None => {
                    write!(f, "struct {{ ")?;

                    for (i, field) in fields.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }
                        self.print_type(f, field.type_id)?; // anonymous struct can't make recursion
                        write!(f, " {}", field.name)?;
                    }

                    write!(f, " }}")?;
                }
            },
            Type::Enum { name, variants, .. } => match name {
                Some(name) => write!(f, "enum {}", name)?,
                None => {
                    write!(f, "enum {{ ")?;

                    for (i, variant) in variants.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", variant.name)?;
                    }

                    write!(f, " }}")?;
                }
            },
            Type::Union { name, fields, .. } => match name {
                Some(name) => write!(f, "union {}", name)?,
                None => {
                    write!(f, "union {{ ")?;

                    for (i, field) in fields.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }
                        self.print_type(f, field.type_id)?; // anonymous union can't make recursion
                        write!(f, " {}", field.name)?;
                    }

                    write!(f, " }}")?;
                }
            },
            Type::Typedef(name, _) => write!(f, "{}", name)?,
            Type::FuncDef { name, return_type_id, args } => {
                self.print_type(f, return_type_id)?;
                write!(f, " ")?;
                if let Some(name) = name {
                    write!(f, "{}", name)?;
                }
                write!(f, "(")?;
                for (i, arg_type_id) in args.iter().copied().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    self.print_type(f, arg_type_id)?;
                }
                write!(f, ")")?;
            }
            Type::Func(subtype_id) => self.print_type(f, subtype_id)?,
        };

        Ok(())
    }

    fn print_value(&self, f: &mut impl io::Write, mut value: Value) -> Result<()> {
        let typ = self.session.get_type_storage().get(value.type_id)?;

        match typ {
            Type::Void | Type::Union { .. } | Type::FuncDef { .. } => bail!(DebuggerError::InvalidPath),
            Type::Base { encoding, size, .. } => {
                match encoding {
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
            Type::Const(subtype_id) | Type::Volatile(subtype_id) | Type::Atomic(subtype_id) | Type::Typedef(_, subtype_id) => {
                self.print_value(f, Value::new(subtype_id, value.buf))?
            }
            Type::Pointer(_) => {
                let ptr = value.buf.get_u64_ne();
                if ptr == 0 {
                    return Ok(write!(f, "null")?);
                }

                write!(f, "{:#x}", ptr)?;
            }
            Type::String(_) => {
                let ptr = value.buf.get_u64_ne();
                let s = self.session.read_c_string(ptr)?;
                write!(f, "{:?}", s)?;
            }
            Type::Array { subtype_id, count } => {
                let count = match count {
                    ArrayCount::Flexible => return Ok(write!(f, "[...]")?),
                    _ => self.session.get_array_count(count)?,
                };
                let subtype_size = self.session.get_type_size(subtype_id)?;

                write!(f, "[")?;
                for i in 0..count {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    let offset = i * subtype_size;
                    self.print_value(f, Value::new(subtype_id, value.buf.slice(offset..offset + subtype_size)))?;
                }
                write!(f, "]")?;
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
            Type::Enum { encoding, size, variants, .. } => {
                let enum_value = match encoding {
                    gimli::DW_ATE_signed => match size {
                        1 => value.buf.get_i8() as i64,
                        2 => value.buf.get_i16_ne() as i64,
                        4 => value.buf.get_i32_ne() as i64,
                        8 => value.buf.get_i64_ne(),
                        _ => bail!("invalid enum subtype byte size"),
                    },
                    gimli::DW_ATE_unsigned => match size {
                        1 => value.buf.get_u8() as i64,
                        2 => value.buf.get_u16_ne() as i64,
                        4 => value.buf.get_u32_ne() as i64,
                        8 => value.buf.get_u64_ne() as i64,
                        _ => bail!("invalid enum subtype byte size"),
                    },
                    _ => bail!("invalid enum subtype encoding"),
                };

                match variants.iter().find(|&variant| variant.value == enum_value) {
                    Some(variant) => write!(f, "{}", variant.name)?,
                    None => write!(f, "{}", enum_value)?,
                };
            }
            Type::Func(_) => {
                let ptr = value.buf.get_u64_ne();
                if ptr == 0 {
                    return Ok(write!(f, "null")?);
                }

                match self.session.get_loc_finder().find_func_by_address(ptr) {
                    Some(func_name) => write!(f, "{}", func_name)?,
                    None => write!(f, "{:#x}", ptr)?,
                }
            }
        };

        Ok(())
    }
}
