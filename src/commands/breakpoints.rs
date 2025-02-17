use std::borrow::Cow;

use crate::debugger::{BreakpointNotFound, Debugger};
use crate::loc_finder::LocNotFound;
use anyhow::Result;

pub fn add<'a, R, S>(debugger: &mut Debugger<R>, loc: S) -> Result<()>
where
    R: gimli::Reader,
    S: Into<Cow<'a, str>>,
{
    if let Err(e) = debugger.add_breakpoint(loc) {
        match e.downcast_ref::<LocNotFound>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn remove<R: gimli::Reader>(debugger: &mut Debugger<R>, index: usize) -> Result<()> {
    if let Err(e) = debugger.remove_breakpoint(index) {
        match e.downcast_ref::<BreakpointNotFound>() {
            Some(_) => println!("{}", e),
            None => return Err(e),
        }
    }

    Ok(())
}

pub fn list<R: gimli::Reader>(debugger: &Debugger<R>) -> Result<()> {
    let breakpoints = debugger.list_breakpoints();

    if breakpoints.is_empty() {
        println!("no breakpoints");
        return Ok(());
    }

    for breakpoint in breakpoints.iter() {
        println!("{}", breakpoint.loc);
    }

    Ok(())
}

pub fn enable<R: gimli::Reader>(debugger: &Debugger<R>, index: usize) -> Result<()> {
    match debugger.get_breakpoint(index) {
        Some(breakpoint) => debugger.enable_breakpoint(breakpoint)?,
        None => println!("breakpoint {} not found", index),
    }

    Ok(())
}

pub fn disable<R: gimli::Reader>(debugger: &Debugger<R>, index: usize) -> Result<()> {
    match debugger.get_breakpoint(index) {
        Some(breakpoint) => debugger.disable_breakpoint(breakpoint)?,
        None => println!("breakpoint {} not found", index),
    }

    Ok(())
}

pub fn clear<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.clear_breakpoints()
}
