use anyhow::{bail, Result};
use bytes::{BufMut, BytesMut};

use crate::error::DebuggerError;
use crate::printer::Printer;
use crate::session::DebugSession;
use crate::types::Type;

pub fn print_var<R: gimli::Reader>(session: &DebugSession<R>, name: Option<&str>) -> Result<()> {
    let printer = Printer::new(session);

    match name {
        Some(name) => {
            let var = session.get_var(name)?;
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

pub fn set_var<R: gimli::Reader>(session: &DebugSession<R>, name: &str, value: &str) -> Result<()> {
    let loc = session.get_var_loc(name)?;

    let mut buf = BytesMut::new();
    match session.get_type_storage().get(loc.type_id)? {
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
        _ => bail!(DebuggerError::InvalidPath),
    }

    session.write_location(loc.location, buf.into())?;

    Ok(())
}
