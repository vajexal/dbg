use anyhow::Result;

use crate::printer::Printer;
use crate::session::DebugSession;

pub fn print_var<R: gimli::Reader>(session: &DebugSession<R>, name: Option<String>) -> Result<()> {
    let printer = Printer::new(session);

    match name {
        Some(name) => {
            let path: Vec<&str> = name.split('.').collect();
            let var = session.get_var(path[0])?;
            printer.print(&var, &path[1..])?;
        }
        None => {
            for var in session.get_vars()?.iter() {
                printer.print(var, &[])?;
            }
        }
    };

    Ok(())
}
