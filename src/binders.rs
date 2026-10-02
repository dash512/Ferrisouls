use std::cmp::Ordering;

use bitflags::bitflags;
pub use crate::binary::bytes::VariableUInt;
pub use crate::errors::{FerrisoulsError, BinaryReaderError, BinaryWriterError};

pub mod bnd;
pub mod bnd2;
pub mod bnd3;
pub mod bnd4;
pub mod bxf3;
pub mod bxf4;
pub mod hash_table;


pub enum BinderVersion {
    V1, // used in very old games such as Metal Wolf Chaos
    V2, // used in games such as MWC, Another Century's Episode 2, Armored Core: FF, AC: 9B, AC: LR. typically psp/ps2 games 
    V3, // used generally in games pre DS2 (2014)
    V4, // all games after DS2 (2014)
    VARIABLE //could be any of the above
}


bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BinderFlags: u32 {
        const IS_BIG_ENDIAN = 0b0000_0001;
        const HAS_IDS = 0b0000_0010;
        const HAS_NAMES_1 = 0b0000_0100;
        const HAS_NAMES_2 = 0b0000_1000;
        //const HAS_NAMES = 0b0000_1100; // checks both. Difference unknown.
        const HAS_LONG_OFFSETS = 0b0001_0000;
        const HAS_COMPRESSION = 0b0010_0000;
        const FLAG_6 = 0b0100_0000;
        const FLAG_7 = 0b1000_0000;

        const _ = !0;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EntryFlags: u32 {
        const NONE = 0;
        const COMPRESSED = 0b0000_0001;
        const FLAG_1 = 0b0000_0010;
        const FLAG_2 = 0b0000_0100;
        const FLAG_3 = 0b0000_1000;
        const FLAG_4 = 0b0001_0000;
        const FLAG_5 = 0b0010_0000;
        const FLAG_6 = 0b0100_0000;
        const FLAG_7 = 0b1000_0000;

        const _ = !0;
    }

}

impl BinderFlags {
    pub fn has_names(&self) -> bool {
        self.intersects(Self::HAS_NAMES_1 | Self::HAS_NAMES_2)
    }

    pub fn force_big_endian(self) -> bool {
        self.contains(Self::IS_BIG_ENDIAN)
    }

    pub fn has_ids(self) -> bool {
        self.contains(Self::HAS_IDS)
    }

    pub fn has_long_offsets(self) -> bool {
        self.contains(Self::HAS_LONG_OFFSETS)
    }

    pub fn has_compression(self) -> bool {
        self.contains(Self::HAS_COMPRESSION)
    }

    pub fn has_flag6(self) -> bool {
        self.contains(Self::FLAG_6)
    }

    pub fn has_flag7(self) -> bool {
        self.contains(Self::FLAG_7)
    }

    pub fn get_entry_header_size(self) -> usize {
        let mut size = 16;

        if self.contains(Self::HAS_IDS) {
            size += 4;
        }

        if self.has_names() {
            size += 4;
        }

        if self.contains(Self::HAS_COMPRESSION) {
            size += 8;
        }

        size += if self.contains(Self::HAS_LONG_OFFSETS) {
            8
        } else {
            4
        };

        if self == Self::HAS_NAMES_1 {
            size += 8;
        }

        size
    }

    pub fn from_byte(raw: u8, bit_big_endian: bool) -> Self {
        let reverse =
            bit_big_endian
            || (raw & 0x01) != 0 && (raw & 0x80) == 0;

        let value = if reverse {
            raw
        } else {
            raw.reverse_bits()
        };

        Self::from_bits_truncate(value as u32)
    }

    pub fn to_byte(self, bit_big_endian: bool) -> u8 {
        let format = self.bits();

        let reverse =
            bit_big_endian
            || (self.contains(Self::IS_BIG_ENDIAN) && self.contains(Self::FLAG_6));

        if reverse {
            format as u8
        } else {
            format.reverse_bits() as u8
        }
    }
}


impl EntryFlags {
    
    fn reverse_bits(byte: u8) -> u8 {
        byte.reverse_bits()
    }

    pub fn from_byte(raw: u8, bit_big_endian: bool) -> Self {
        let value = if bit_big_endian {
            raw
        } else {
            raw.reverse_bits()
        };

        Self::from_bits_truncate(value as u32)
    }

    pub fn to_byte(self, bit_big_endian: bool) -> u8 {
        let value = self.bits();

        if bit_big_endian {
            value as u8
        } else {
            value.reverse_bits() as u8
        }
    }

    pub fn is_compressed(&self) -> bool {
        self.contains(Self::COMPRESSED)
    }

}



/// This trait allows for basic handling of entries within any Binder that implements it.
/// 
/// Expects a BinderVersion, and an `Entry` type corresponding to the entries/files stored in this Binder.
/// 
//TODO: add more required fns to allow more runtime modification to the Binder itself as well as its entries
pub trait Binder {
    const VERSION: BinderVersion; // BinderVersion
    type Entry: BinderEntry; 

    fn entries(&mut self) -> &mut Vec<Self::Entry>;

    fn get(&mut self, index: usize) -> Option<&<Self as Binder>::Entry> {
        self.entries().get(index)
    }

    fn add(&mut self, entry: Self::Entry) {
        self.entries().push(entry);
    }
    
    fn remove(&mut self, index: usize) {
        self.entries().remove(index);
    }
    
    fn first(&mut self) -> Option<&<Self as Binder>::Entry> {
        self.entries().first()
    }

    fn last(&mut self) -> Option<&<Self as Binder>::Entry> {
        self.entries().last()
    }

    fn pop_last(&mut self) -> Option<Self::Entry> {
        self.entries().pop()
    }

    fn clear(&mut self) {
        self.entries().clear();
    }

    fn count(&mut self) -> usize {
        self.entries().len()
    }

    fn iter(&mut self) -> std::slice::IterMut<'_, <Self as Binder>::Entry> {
        self.entries().iter_mut()
    }

    fn find(&mut self, id: <<Self as Binder>::Entry as BinderEntry>::Identifier) -> Option<&Self::Entry> {
        self.entries()
            .iter()
            .find(
                |&e|
                *e.identity() == id
            )
    }

    fn insert(&mut self, index: usize, entry: Self::Entry) -> &mut <Self as Binder>::Entry {
        self.entries().insert_mut(index, entry)
    }

    fn is_empty(&mut self) -> bool {
        self.entries().is_empty()
    }

    fn append(&mut self, other: &mut Vec<Self::Entry>) {
        self.entries().append(other)
    }

    fn extend(&mut self, other: Vec<Self::Entry>) {
        self.entries().extend(other)
    }

}

/// Any objects inheriting this trait must belong to a Binder as its entries
/// 
/// Expects an Identifier (type of its id, e.g String) and a function `identity` that returns a reference to its id.
/// This is so that the parent Binder can identify it and perform actions based on which entry it is.
/// 
//TODO: implement more stuff here
pub trait BinderEntry {
    type Identifier: PartialEq; //.name / .id etc

    fn identity(&self) -> &Self::Identifier;
}

