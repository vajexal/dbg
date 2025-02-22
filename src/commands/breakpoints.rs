use std::borrow::Cow;

use crate::debugger::Debugger;
use crate::error::DebuggerError;
use anyhow::Result;

pub fn add<'a, R, S>(debugger: &mut Debugger<R>, loc: S) -> Result<()>
where
    R: gimli::Reader,
    S: Into<Cow<'a, str>>,
{
    if let Err(e) = debugger.add_breakpoint(loc) {
        match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn remove<R: gimli::Reader>(debugger: &mut Debugger<R>, loc: &str) -> Result<()> {
    if let Err(e) = debugger.remove_breakpoint(loc) {
        match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn list<R: gimli::Reader>(debugger: &Debugger<R>) -> Result<()> {
    let breakpoints_iter = debugger.list_breakpoints();

    if breakpoints_iter.len() == 0 {
        println!("no breakpoints");
        return Ok(());
    }

    for breakpoint in breakpoints_iter {
        println!("{}", breakpoint.loc);
    }

    Ok(())
}

pub fn enable<R: gimli::Reader>(debugger: &Debugger<R>, loc: &str) -> Result<()> {
    if let Err(e) = debugger.enable_breakpoint(loc) {
        match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn disable<R: gimli::Reader>(debugger: &Debugger<R>, loc: &str) -> Result<()> {
    if let Err(e) = debugger.disable_breakpoint(loc) {
        match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn clear<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.clear_breakpoints()
}
