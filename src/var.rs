use std::rc::Rc;

use bytes::Bytes;

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

#[derive(Debug, Clone)]
pub struct Value {
    pub type_id: TypeId,
    pub buf: Bytes,
}

impl Value {
    pub fn new(type_id: TypeId, buf: Bytes) -> Self {
        Self { type_id, buf }
    }
}

#[derive(Debug, Clone)]
pub struct Var {
    pub name: Rc<str>,
    pub value: Value,
}

impl Var {
    pub fn new<S: Into<Rc<str>>>(name: S, value: Value) -> Self {
        Self { name: name.into(), value }
    }
}
