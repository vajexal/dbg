use anyhow::{bail, Result};

use crate::{debugger::Debugger, var::Var};

pub fn print_var<R: gimli::Reader>(debugger: &Debugger<R>, name: Option<String>) -> Result<()> {
    match name {
        Some(name) => {
            let var = match debugger.get_var(&name)? {
                Some(var) => var,
                None => {
                    println!("{} not found", name);
                    return Ok(());
                }
            };

            print_var_internal(&var)?;
        }
        None => {
            for var in debugger.get_vars()?.iter() {
                print_var_internal(var)?;
            }
        }
    };

    Ok(())
}

pub(crate) fn print_var_internal(var: &Var) -> Result<()> {
    print!("{} {} = ", var.typ.description, var.name);
    match var.typ.encoding {
        gimli::DW_ATE_boolean => print!("{}", var.value != 0),
        gimli::DW_ATE_signed => match var.typ.byte_size {
            2 => println!("{}", var.value as i16),
            4 => println!("{}", var.value as i32),
            8 => println!("{}", var.value as i64),
            _ => bail!("unsupported byte size"),
        },
        gimli::DW_ATE_unsigned => match var.typ.byte_size {
            2 => println!("{}", var.value as u16),
            4 => println!("{}", var.value as u32),
            8 => println!("{}", var.value as u64),
            _ => bail!("unsupported byte size"),
        },
        _ => bail!("unsupported encoding"),
    };

    Ok(())
}
