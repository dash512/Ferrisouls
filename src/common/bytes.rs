use byteorder::{BigEndian, LittleEndian};

#[derive(Debug, PartialEq)]
pub enum ByteOrder {
    BigEndian,
    LittleEndian
}

