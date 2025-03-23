use anyhow::Result;

use crate::printer::{InvalidPathError, Printer};
use crate::session::DebugSession;

pub fn print_var<R: gimli::Reader>(session: &DebugSession<R>, name: Option<String>) -> Result<()> {
    let printer = Printer::new(session);

    match name {
        Some(name) => {
            let path: Vec<&str> = name.split('.').collect();

            let var = match session.get_var(path[0])? {
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
            for var in session.get_vars()?.iter() {
                printer.print(var, &[])?;
            }
        }
    };

    Ok(())
}
