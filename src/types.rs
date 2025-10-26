use std::cell::RefCell;
use std::rc::Rc;

use thiserror::Error;

use crate::loc_finder::EntryRef;

#[derive(Debug, Error)]
pub enum TypeError {
    #[error("invalid type id {0}")]
    InvalidTypeId(TypeId),
}

pub type TypeId = usize;
pub const VOID_TYPE_ID: TypeId = 0;

type Result<T> = std::result::Result<T, TypeError>;

#[derive(Debug, Clone)]
pub enum Type<R: gimli::Reader> {
    Void,
    Base {
        name: Rc<str>,
        encoding: gimli::DwAte,
        size: u16,
    },
    Const(TypeId),
    Volatile(TypeId),
    Atomic(TypeId),
    Pointer(TypeId),
    String(TypeId),
    Array {
        subtype_id: TypeId,
        count: ArrayCount<R>,
    },
    Struct {
        name: Option<Rc<str>>,
        size: u16,
        fields: Rc<Vec<Field>>,
    },
    Enum {
        name: Option<Rc<str>>,
        encoding: gimli::DwAte,
        size: u16,
        variants: Rc<Vec<EnumVariant>>,
    },
    Union {
        name: Option<Rc<str>>,
        size: u16,
        fields: Rc<Vec<UnionField>>,
    },
    Typedef(Rc<str>, TypeId),
    FuncDef {
        name: Option<Rc<str>>,
        return_type_id: TypeId,
        args: Rc<Vec<TypeId>>,
    },
    Func(TypeId), // pointer to a function
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Rc<str>,
    pub type_id: TypeId,
    pub offset: u16,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Rc<str>,
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct UnionField {
    pub name: Rc<str>,
    pub type_id: TypeId,
}

#[derive(Debug, Clone)]
pub enum ArrayCount<R: gimli::Reader> {
    Static(usize),
    Dynamic(EntryRef<R::Offset>),
}

#[derive(Debug)]
pub struct TypeStorage<R: gimli::Reader> {
    types: RefCell<Vec<Type<R>>>,
}

impl<R: gimli::Reader> TypeStorage<R> {
    pub fn new() -> Self {
        Self {
            types: RefCell::new(vec![Type::Void]),
        }
    }

    pub fn add(&mut self, typ: Type<R>) -> TypeId {
        let mut types = self.types.borrow_mut();
        types.push(typ);
        types.len() - 1
    }

    pub fn replace(&mut self, type_id: TypeId, typ: Type<R>) -> Result<()> {
        let mut types = self.types.borrow_mut();
        if type_id < types.len() {
            types[type_id] = typ;
            Ok(())
        } else {
            Err(TypeError::InvalidTypeId(type_id))
        }
    }

    pub fn get(&self, type_id: TypeId) -> Result<Type<R>> {
        self.types.borrow().get(type_id).cloned().ok_or(TypeError::InvalidTypeId(type_id))
    }

    pub fn unwind_type(&self, type_id: TypeId) -> Result<Type<R>> {
        match self.get(type_id)? {
            Type::Const(subtype_id) | Type::Volatile(subtype_id) | Type::Atomic(subtype_id) | Type::Typedef(_, subtype_id) => self.unwind_type(subtype_id),
            typ => Ok(typ),
        }
    }

    pub fn get_type_ref(&self, type_id: TypeId) -> TypeId {
        let mut types = self.types.borrow_mut();

        types
            .iter()
            .position(|typ| match *typ {
                Type::Pointer(subtype_id) => subtype_id == type_id,
                _ => false,
            })
            .unwrap_or_else(|| {
                types.push(Type::Pointer(type_id));
                types.len() - 1
            })
    }
}
