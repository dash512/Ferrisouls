use byteorder::{BigEndian, LittleEndian};

#[derive(Debug, PartialEq)]
pub enum ByteOrder {
    BigEndian,
    LittleEndian
}

#[derive(Debug, Clone, Copy)]
pub enum VariableUInt {
    UInt(u32),
    ULong(u64),
}

impl VariableUInt {
    pub fn as_u64(&self) -> u64 {
        match self {
            Self::UInt(i) => *i as u64,
            Self::ULong(i) => *i,
        }
    }

    pub fn as_u32(&self) -> u32 {
        match self {
            Self::UInt(i) => *i,
            Self::ULong(i) => *i as u32,
        }
    }
}

