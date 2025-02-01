use anyhow::Result;

use crate::debugger::Debugger;
use crate::printer::{InvalidPathError, Printer};

pub fn print_var<R: gimli::Reader>(debugger: &Debugger<R>, name: Option<String>) -> Result<()> {
    let printer = Printer::new(debugger);

    match name {
        Some(name) => {
            let path: Vec<&str> = name.split('.').collect();

            let var = match debugger.get_var(path[0])? {
                Some(var) => var,
                None => {
                    println!("{} not found", name);
                    return Ok(());
                }
            };

            if let Err(e) = printer.print(&var, &path[1..]) {
                match e.downcast_ref::<InvalidPathError>() {
                    Some(_) => println!("{}", e),
                    None => return Err(e),
                }
            }
        }
        None => {
            for var in debugger.get_vars()?.iter() {
                printer.print(var, &[])?;
            }
        }
    };

    Ok(())
}
