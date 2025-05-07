use crate::{error::DebuggerError, session::DebugSession};
use anyhow::{anyhow, Result};

pub fn run<R: gimli::Reader>(session: &DebugSession<R>) -> Result<()> {
    session.run()?;
    session.wait()
}

pub fn stop<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.stop()
}

pub fn cont<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.cont()?;
    session.wait()
}

pub fn step<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.step()
}

pub fn step_in<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.step_in()
}

pub fn step_out<R: gimli::Reader>(session: &mut DebugSession<R>) -> Result<()> {
    session.step_out()
}

pub fn location<R: gimli::Reader>(session: &DebugSession<R>) -> Result<()> {
    let loc = session.get_current_line()?.ok_or(anyhow!(DebuggerError::InvalidLocation))?;
    println!("{}", loc);
    Ok(())
}
