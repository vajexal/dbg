#[derive(Debug)]
pub enum VarType {
    Base { name: String, encoding: gimli::DwAte, size: u16 },
    Const(Box<VarType>),
    Pointer(Box<VarType>),
    Struct { name: String, size: u16, fields: Vec<Field> },
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub typ: VarType,
    pub offset: u16,
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
pub struct Var<R: gimli::Reader> {
    pub typ: VarType,
    pub name: String,
    pub location: gimli::Location<R>,
}
