use anyhow::{bail, Result};
use bytes::{BufMut, BytesMut};

use crate::error::DebuggerError;
use crate::path::Path;
use crate::printer::Printer;
use crate::session::DebugSession;
use crate::types::Type;

pub fn print_var<R: gimli::Reader>(session: &DebugSession<R>, path: Option<&Path>) -> Result<()> {
    let printer = Printer::new(session);

    match path {
        Some(path) => {
            let var = session.get_var(path)?;
            printer.print(&var)?;
        }
        None => {
            for var in session.get_vars()?.iter() {
                printer.print(var)?;
            }
        }
    };

    Ok(())
}

pub fn set_var<R: gimli::Reader>(session: &DebugSession<R>, path: &Path, value: &str) -> Result<()> {
    let loc = session.get_var_loc(path)?;

    let mut buf = BytesMut::new();
    match session.get_type_storage().unwind_type(loc.type_id)? {
        Type::Base { encoding, size, .. } => match encoding {
            gimli::DW_ATE_boolean => {
                let value = value.parse::<bool>().map_err(|_| DebuggerError::InvalidValue)?;
                buf.put_i8(value as i8);
            }
            gimli::DW_ATE_signed => match size {
                1 => buf.put_i8(value.parse::<i8>().map_err(|_| DebuggerError::InvalidValue)?),
                2 => buf.put_i16_ne(value.parse::<i16>().map_err(|_| DebuggerError::InvalidValue)?),
                4 => buf.put_i32_ne(value.parse::<i32>().map_err(|_| DebuggerError::InvalidValue)?),
                8 => buf.put_i64_ne(value.parse::<i64>().map_err(|_| DebuggerError::InvalidValue)?),
                _ => bail!("unsupported byte size"),
            },
            gimli::DW_ATE_unsigned => match size {
                1 => buf.put_u8(value.parse::<u8>().map_err(|_| DebuggerError::InvalidValue)?),
                2 => buf.put_u16_ne(value.parse::<u16>().map_err(|_| DebuggerError::InvalidValue)?),
                4 => buf.put_u32_ne(value.parse::<u32>().map_err(|_| DebuggerError::InvalidValue)?),
                8 => buf.put_u64_ne(value.parse::<u64>().map_err(|_| DebuggerError::InvalidValue)?),
                _ => bail!("unsupported byte size"),
            },
            gimli::DW_ATE_float => match size {
                4 => buf.put_f32_ne(value.parse::<f32>().map_err(|_| DebuggerError::InvalidValue)?),
                8 => buf.put_f64_ne(value.parse::<f64>().map_err(|_| DebuggerError::InvalidValue)?),
                _ => bail!("unsupported byte size"),
            },
            _ => bail!("unsupported encoding"),
        },
        Type::Pointer(_) => {
            let ptr = if value == "null" {
                0
            } else {
                u64::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16).map_err(|_| DebuggerError::InvalidValue)?
            };
            buf.put_u64_ne(ptr);
        }
        Type::String(_) => {
            let new_str: String = serde_json::from_str(value).map_err(|_| DebuggerError::InvalidValue)?;
            let new_str_addr = session.alloc_c_string(&new_str)?;
            buf.put_u64_ne(new_str_addr);
        }
        Type::Enum { encoding, size, variants, .. } => {
            let enum_value = variants
                .iter()
                .find(|&variant| variant.name.as_ref() == value)
                .map(|variant| variant.value)
                .ok_or(DebuggerError::InvalidValue)?;

            match encoding {
                gimli::DW_ATE_signed => match size {
                    1 => buf.put_i8(enum_value as i8),
                    2 => buf.put_i16_ne(enum_value as i16),
                    4 => buf.put_i32_ne(enum_value as i32),
                    8 => buf.put_i64_ne(enum_value),
                    _ => bail!("invalid enum byte size"),
                },
                gimli::DW_ATE_unsigned => match size {
                    1 => buf.put_u8(enum_value as u8),
                    2 => buf.put_u16_ne(enum_value as u16),
                    4 => buf.put_u32_ne(enum_value as u32),
                    8 => buf.put_u64_ne(enum_value as u64),
                    _ => bail!("invalid enum byte size"),
                },
                _ => bail!("invalid enum encoding"),
            };
        }
        Type::Func(_) => {
            let address = session.get_loc_finder().find_loc(value)?.ok_or(DebuggerError::InvalidValue)?;
            buf.put_u64_ne(address);
        }
        _ => bail!(DebuggerError::InvalidPath),
    }

    session.write_location(loc.location, buf.into())?;

    Ok(())
}
