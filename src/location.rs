use crate::var::TypeId;

#[derive(Debug, Clone)]
pub enum ValueLoc {
    Empty,
    Register { register: gimli::Register, offset: u16 },
    Address(u64),
}

impl ValueLoc {
    pub fn with_offset(self, delta: u16) -> Self {
        match self {
            ValueLoc::Empty => panic!("can't add offset to empty address"),
            ValueLoc::Register { register, offset } => Self::Register {
                register,
                offset: offset + delta,
            },
            ValueLoc::Address(address) => Self::Address(address + delta as u64),
        }
    }
}

impl<R: gimli::Reader> From<gimli::Location<R>> for ValueLoc {
    fn from(value: gimli::Location<R>) -> Self {
        match value {
            gimli::Location::Register { register } => ValueLoc::Register { register, offset: 0 },
            gimli::Location::Address { address } => ValueLoc::Address(address),
            _ => ValueLoc::Empty,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedValueLoc {
    pub location: ValueLoc,
    pub type_id: TypeId,
}

impl TypedValueLoc {
    pub fn new(location: ValueLoc, type_id: TypeId) -> Self {
        Self { location, type_id }
    }

    pub fn with_type(self, type_id: TypeId) -> Self {
        Self {
            location: self.location,
            type_id,
        }
    }
}
