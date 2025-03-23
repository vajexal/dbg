#[derive(Debug)]
pub struct Trap {
    pub original_data: i64,
}

impl Trap {
    pub fn new(original_data: i64) -> Self {
        Self { original_data }
    }
}
