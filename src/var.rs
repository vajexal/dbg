use std::rc::Rc;

pub type TypeId = usize;

#[derive(Debug, Clone)]
pub enum Type {
    Void,
    Base { name: Rc<str>, encoding: gimli::DwAte, size: u16 },
    Const(TypeId),
    Pointer(TypeId),
    Struct { name: Rc<str>, size: u16, fields: Vec<Field> },
    Typedef(Rc<str>, TypeId),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Rc<str>,
    pub type_id: TypeId,
    pub offset: u16,
}

#[derive(Debug)]
pub struct Var<R: gimli::Reader> {
    pub type_id: TypeId,
    pub name: String,
    pub location: gimli::Location<R>,
}
