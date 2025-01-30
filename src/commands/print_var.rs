use anyhow::Result;

use crate::debugger::Debugger;
use crate::printer::Printer;

pub fn print_var<R: gimli::Reader>(debugger: &Debugger<R>, name: Option<String>) -> Result<()> {
    let printer = Printer::new(debugger);

    match name {
        Some(name) => {
            let var = match debugger.get_var(&name)? {
                Some(var) => var,
                None => {
                    println!("{} not found", name);
                    return Ok(());
                }
            };

            printer.print(&var)?;
        }
        None => {
            for var in debugger.get_vars()?.iter() {
                printer.print(var)?;
            }
        }
    };

    Ok(())
}
