use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub addr: u64,
    pub original_data: i64,
    pub loc: String,
    pub enabled: Cell<bool>,
}

impl Breakpoint {
    pub fn new<S: Into<String>>(addr: u64, original_data: i64, loc: S) -> Self {
        Self {
            addr,
            original_data,
            loc: loc.into(),
            enabled: Cell::new(false),
        }
    }
}
