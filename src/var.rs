#[derive(Debug)]
pub enum VarType {
    Base { 
        byte_size: u8, 
        encoding: gimli::DwAte, 
        name: String 
    },
    Const(Box<VarType>),
    Pointer(Box<VarType>),
}

impl VarType {
    pub fn unwind(&self) -> &Self {
        match self {
            VarType::Const(var_type) => var_type.unwind(),
            typ => typ,
        }
    }
}

#[derive(Debug)]
pub struct Var {
    pub typ: VarType,
    pub name: String,
    pub value: u64, // todo
}
