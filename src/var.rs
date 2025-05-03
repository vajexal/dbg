use std::rc::Rc;

use anyhow::anyhow;
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

#[derive(Debug)]
pub enum Operator {
    Ref,
    Deref,
}

impl TryFrom<char> for Operator {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '&' => Ok(Operator::Ref),
            '*' => Ok(Operator::Deref),
            _ => Err(anyhow!("invalid operator")),
        }
    }
}
