#[derive(Debug)]
pub struct Trap {
    pub original_bytecode: i64,
}

impl Trap {
    pub fn new(original_bytecode: i64) -> Self {
        Self { original_bytecode }
    }
}
