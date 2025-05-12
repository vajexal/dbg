use std::rc::Rc;

use bytes::Bytes;

use crate::types::TypeId;

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
