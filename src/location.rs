use anyhow::{anyhow, Result};

use crate::{error::DebuggerError, types::TypeId};

#[derive(Debug, Clone)]
pub enum ValueLoc {
    Register { register: gimli::Register, offset: u16 },
    Address(u64),
    Value(u64),
}

impl ValueLoc {
    pub fn with_offset(self, delta: u16) -> Result<Self> {
        match self {
            ValueLoc::Register { register, offset } => Ok(Self::Register {
                register,
                offset: offset + delta,
            }),
            ValueLoc::Address(address) => Ok(Self::Address(address + delta as u64)),
            ValueLoc::Value(_) => Err(anyhow!(DebuggerError::InvalidLocation)),
        }
    }
}

impl<R: gimli::Reader> TryFrom<gimli::Location<R>> for ValueLoc {
    type Error = DebuggerError;

    fn try_from(value: gimli::Location<R>) -> Result<Self, Self::Error> {
        match value {
            gimli::Location::Register { register } => Ok(ValueLoc::Register { register, offset: 0 }),
            gimli::Location::Address { address } => Ok(ValueLoc::Address(address)),
            gimli::Location::Value { value } => Ok(ValueLoc::Value(value.to_u64(!0u64).map_err(|_| DebuggerError::InvalidLocation)?)),
            _ => Err(DebuggerError::InvalidLocation),
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
