use thiserror::Error;

#[derive(Debug, Error)]
pub enum DebuggerError {
    #[error("breakpoint not found")]
    BreakpointNotFound,
    #[error("breakpoint already exist")]
    BreakpointAlreadyExist,
    #[error("loc not found")]
    LocNotFound,
}
