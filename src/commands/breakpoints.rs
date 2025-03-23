use std::borrow::Cow;

use crate::error::DebuggerError;
use crate::session::DebugSession;
use anyhow::Result;

pub fn add<'a, R, S>(session: &mut DebugSession<R>, loc: S) -> Result<()>
where
    R: gimli::Reader,
    S: Into<Cow<'a, str>>,
{
    match session.add_breakpoint(loc) {
        Ok(_) => println!("breakpoint set"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn remove<R: gimli::Reader>(session: &mut DebugSession<R>, loc: &str) -> Result<()> {
    match session.remove_breakpoint(loc) {
        Ok(_) => println!("breakpoint removed"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn list<R: gimli::Reader>(session: &DebugSession<R>) -> Result<()> {
    let breakpoints_iter = session.list_breakpoints();

    if breakpoints_iter.len() == 0 {
        println!("no breakpoints");
        return Ok(());
    }

    for breakpoint in breakpoints_iter {
        println!("{}", breakpoint.loc);
    }

    Ok(())
}

pub fn enable<R: gimli::Reader>(session: &DebugSession<R>, loc: &str) -> Result<()> {
    match session.enable_breakpoint(loc) {
        Ok(_) => println!("breakpoint enabled"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn disable<R: gimli::Reader>(session: &DebugSession<R>, loc: &str) -> Result<()> {
    match session.disable_breakpoint(loc) {
        Ok(_) => println!("breakpoint disabled"),
        Err(e) => match e.downcast_ref::<DebuggerError>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        },
    };

    Ok(())
}

pub fn clear<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.clear_breakpoints()
}
