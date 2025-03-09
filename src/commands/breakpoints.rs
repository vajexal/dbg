use std::borrow::Cow;

use crate::debugger::Debugger;
use crate::error::DebuggerError;
use anyhow::Result;

pub fn add<'a, R, S>(debugger: &mut Debugger<R>, loc: S) -> Result<()>
where
    R: gimli::Reader,
    S: Into<Cow<'a, str>>,
{
    match debugger.add_breakpoint(loc) {
        Ok(_) => println!("breakpoint set"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn remove<R: gimli::Reader>(debugger: &mut Debugger<R>, loc: &str) -> Result<()> {
    match debugger.remove_breakpoint(loc) {
        Ok(_) => println!("breakpoint removed"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

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
    match debugger.enable_breakpoint(loc) {
        Ok(_) => println!("breakpoint enabled"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn disable<R: gimli::Reader>(debugger: &Debugger<R>, loc: &str) -> Result<()> {
    match debugger.disable_breakpoint(loc) {
        Ok(_) => println!("breakpoint disabled"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn clear<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.clear_breakpoints()
}
