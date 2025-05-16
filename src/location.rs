use anyhow::{anyhow, Result};

use crate::consts::WORD_SIZE;
use crate::error::DebuggerError;
use crate::types::TypeId;

#[derive(Debug, Clone)]
pub enum ValueLoc {
    Register { register: gimli::Register, offset: u16 },
    Address(u64),
    Value(u64),
}

impl ValueLoc {
    pub fn with_offset(self, delta: usize) -> Result<Self> {
        match self {
            ValueLoc::Register { register, offset } => {
                if offset as usize + delta >= WORD_SIZE {
                    Err(anyhow!(DebuggerError::InvalidLocation))
                } else {
                    Ok(Self::Register {
                        register,
                        offset: offset + delta as u16,
                    })
                }
            }
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
