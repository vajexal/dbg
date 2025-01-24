#[derive(Debug)]
pub struct VarType {
    pub byte_size: u8,
    pub encoding: gimli::DwAte,
    pub description: String,
}

#[derive(Debug)]
pub struct Var {
    pub typ: VarType,
    pub name: String,
    pub value: u64, // todo
}
