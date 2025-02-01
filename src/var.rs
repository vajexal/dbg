use crate::debugger::WORD_SIZE;

#[derive(Debug)]
pub enum VarType {
    Base { name: String, encoding: gimli::DwAte, size: u16 },
    Const(Box<VarType>),
    Pointer(Box<VarType>),
    Struct { name: String, size: u16, fields: Vec<Field> },
    Typedef(String, Box<VarType>),
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub typ: VarType,
    pub offset: u16,
}

impl VarType {
    pub fn unwind_const(&self) -> &Self {
        match self {
            VarType::Const(var_type) => var_type.unwind_const(),
            typ => typ,
        }
    }

    pub fn get_size(&self) -> usize {
        match self {
            VarType::Base { size, .. } => *size as usize,
            VarType::Const(var_type) => var_type.get_size(),
            VarType::Pointer(_) => WORD_SIZE,
            VarType::Struct { size, .. } => *size as usize,
            VarType::Typedef(_, var_type) => var_type.get_size(),
        }
    }
}

#[derive(Debug)]
pub struct Var<R: gimli::Reader> {
    pub typ: VarType,
    pub name: String,
    pub location: gimli::Location<R>,
}
