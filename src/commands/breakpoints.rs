use std::borrow::Cow;

use crate::session::DebugSession;
use anyhow::Result;

pub fn add<'a, R, S>(session: &mut DebugSession<R>, loc: S) -> Result<()>
where
    R: gimli::Reader,
    S: Into<Cow<'a, str>>,
{
    session.add_breakpoint(loc)?;
    println!("breakpoint set");

    Ok(())
}

pub fn remove<R: gimli::Reader>(session: &mut DebugSession<R>, loc: &str) -> Result<()> {
    session.remove_breakpoint(loc)?;
    println!("breakpoint removed");

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
    session.enable_breakpoint(loc)?;
    println!("breakpoint enabled");

    Ok(())
}

pub fn disable<R: gimli::Reader>(session: &DebugSession<R>, loc: &str) -> Result<()> {
    session.disable_breakpoint(loc)?;
    println!("breakpoint disabled");

    Ok(())
}

pub fn clear<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.clear_breakpoints()
}
