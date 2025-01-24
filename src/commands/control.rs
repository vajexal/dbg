use crate::debugger::{Debugger, DebuggerState};
use anyhow::Result;

pub fn run<R: gimli::Reader>(debugger: &Debugger<R>) -> Result<DebuggerState> {
    debugger.run()?;
    debugger.wait()
}

pub fn stop<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<DebuggerState> {
    debugger.stop()
}

pub fn cont<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<DebuggerState> {
    if debugger.cont()? == DebuggerState::Exited {
        return Ok(DebuggerState::Exited);
    }

    debugger.wait()
}

pub fn step<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<DebuggerState> {
    debugger.step()
}

pub fn step_in<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<DebuggerState> {
    debugger.step_in()
}

pub fn step_out<R: gimli::Reader>(debugger: &mut Debugger<R>) -> Result<DebuggerState> {
    debugger.step_out()
}
