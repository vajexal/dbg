use thiserror::Error;

#[derive(Debug, Error)]
pub enum DebuggerError {
    #[error("breakpoint not found")]
    BreakpointNotFound,
    #[error("breakpoint already exist")]
    BreakpointAlreadyExist,
    #[error("loc not found")]
    LocNotFound,
    #[error("{0} not found")]
    VarNotFound(String),
    #[error("invalid path")]
    InvalidPath,
    #[error("invalid value")]
    InvalidValue,
}
