use crate::debugger::{Debugger, DebuggerState};
use anyhow::Result;

pub fn run<R: gimli::Reader>(debugger: &Debugger<R>) -> Result<()> {
    debugger.run()?;
    debugger.wait()
}

pub fn stop<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.stop()
}

pub fn cont<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.cont()?;
    if debugger.get_state() == DebuggerState::Exited {
        return Ok(());
    }
    debugger.wait()
}

pub fn step<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.step()
}

pub fn step_in<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.step_in()
}

pub fn step_out<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<()> {
    debugger.step_out()
}
