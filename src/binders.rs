//! Module containing traits and shared stuff for Binder types.

use std::{cmp::Ordering, path::Path};

use bitflags::bitflags;
use regex::Regex;
use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::binders::{bnd4::{BND4, BND4EntryHeader, BND4Header}, identifiers::Identifier};
use crate::dcx::{self, DCXType};
use crate::binary::misc::VariableUInt;
use crate::binders::bnd4::BND4Entry;
use crate::errors::{FerrisoulsError, BinaryReaderError, BinaryWriterError};

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

impl BinderVersion {
    pub fn from_u8(int: u8) -> Self {
        match int {
            1 => Self::V1,
            2 => Self::V2,
            3 => Self::V3,
            4 => Self::V4,
            _=> Self::VARIABLE
        }
    }

    pub fn to_u8(&self) -> Result<u8, FerrisoulsError> {
        Ok(match self {
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3, 
            Self::V4 => 4,
            Self::VARIABLE => { return Err(FerrisoulsError::custom("Variable BinderVersion has no integer form."))}
        })
    }
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
        let b = self.bits() as u8;
        let keep = bit_big_endian
            || (self.contains(Self::IS_BIG_ENDIAN) && !self.contains(Self::FLAG_7));
        if keep { b } else { b.reverse_bits() }
    }
    
}

impl Default for BinderFlags {
    fn default() -> Self {
        Self::HAS_NAMES_1
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
        let b = self.bits() as u8;
        if bit_big_endian { b } else { b.reverse_bits() }
    }

    pub fn is_compressed(&self) -> bool {
        self.contains(Self::COMPRESSED)
    }

}

impl Default for EntryFlags {
    fn default() -> Self {
        Self::COMPRESSED
    }
}


///Setup primitive types as Identifiers
pub mod identifiers {
    use super::Regex;

    pub trait Identifier {
        fn as_string(&self) -> String;

        fn matches(&self, pattern: &Regex) -> bool;
    }

    impl Identifier for String {
        fn as_string(&self) -> String {
            self.clone()
        }
        fn matches(&self, pattern: &Regex) -> bool {
            pattern.is_match(self)
        }
    }

    impl<T> Identifier for Option<T> 
    where T: Identifier
    {
        fn as_string(&self) -> String { 
            match self {
                Some(s) => s.as_string(),
                None => panic!("as_string() called on a None option.")
            }
        }
        fn matches(&self, pattern: &Regex) -> bool {
            match self {
                Some(s) => s.matches(pattern),
                None => false
            }
        }
    }

    macro_rules! impl_primitive_identifiers {
        ($($ty:ty),* $(,)?) => {
            $(
                impl Identifier for $ty {
                    fn as_string(&self) -> String {
                        self.to_string()
                    }
                    fn matches(&self, pattern: &Regex) -> bool {
                        pattern.is_match(&self.as_string())
                    }
                }
            )*
        };
    }

    impl_primitive_identifiers!(
        bool,
        char,
        u8, u16, u32, u64, u128,
        i8, i16, i32, i64, i128,
        usize, isize,
        f32, f64,
    );
}



/// This trait allows for basic handling of entries within any Binder that implements it.
/// 
/// Expects a BinderVersion, and an `Entry` type corresponding to the entries/files stored in this Binder.
/// 
//TODO: add more required fns to allow more runtime modification to the Binder itself as well as its entries
pub trait Binder {
    const VERSION: BinderVersion; // BinderVersion
    type Entry: BinderEntry; 

    fn entries(&self) -> &Vec<Self::Entry>;

    fn entries_mut(&mut self) -> &mut Vec<Self::Entry>;

    fn get(&self, index: usize) -> Option<&<Self as Binder>::Entry> {
        self.entries().get(index)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut <Self as Binder>::Entry> {
        self.entries_mut().get_mut(index)
    }

    fn add(&mut self, entry: Self::Entry) {
        self.entries_mut().push(entry);
    }
    
    fn remove(&mut self, index: usize) {
        self.entries_mut().remove(index);
    }
    
    fn first(&self) -> Option<&<Self as Binder>::Entry> {
        self.entries().first()
    }

    fn last(&self) -> Option<&<Self as Binder>::Entry> {
        self.entries().last()
    }

    fn pop_last(&mut self) -> Option<Self::Entry> {
        self.entries_mut().pop()
    }

    fn clear(&mut self) {
        self.entries_mut().clear();
    }

    fn count(&self) -> usize {
        self.entries().len()
    }

    fn iter(&self) -> std::slice::Iter<'_, <Self as Binder>::Entry> {
        self.entries().iter()
    }

    fn iter_mut(&mut self) -> std::slice::IterMut<'_, <Self as Binder>::Entry> {
        self.entries_mut().iter_mut()
    }

    fn find(&self, id: <<Self as Binder>::Entry as BinderEntry>::Identifier) -> Option<&Self::Entry> {
        self.entries()
            .iter()
            .find(
                |&e|
                *e.identity() == id
            )
    }

    fn find_mut(&mut self, id: <<Self as Binder>::Entry as BinderEntry>::Identifier) -> Option<&mut Self::Entry> {
        self.entries_mut()
            .iter_mut()
            .find(
                |e|
                *e.identity() == id
            )
    }

    ///Find all entries in self whose `identity` matches a given Regex `pattern`, applying `f` to each.
    /// 
    ///If `Self::Entry::Identifier` is an `Option`, None values will never match.
    fn execute_regex<F, I>(&mut self, pattern: &Regex, identity: I, mut f: F)
    where
        I: Fn(&Self::Entry) -> &str,
        F: FnMut(&mut Self::Entry),
    {
        for entry in self.entries_mut().iter_mut() {
            if pattern.is_match(identity(entry)) {
                f(entry);
            }
        }
    }

    /*TODO: a way to modify existing entries by finding it by a given entry's id and replacing it in-place
    fn modify(&mut self, entry: <<Self as Binder>::Entry) -> Option<&Self::Entry> {
        match self.find(*entry.identity()) {
            Some(e) => 
        }
    }
    */

    fn insert(&mut self, index: usize, entry: Self::Entry) -> &mut <Self as Binder>::Entry {
        self.entries_mut().insert_mut(index, entry)
    }

    fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    fn append(&mut self, other: &mut Vec<Self::Entry>) {
        self.entries_mut().append(other)
    }

    fn extend(&mut self, other: Vec<Self::Entry>) {
        self.entries_mut().extend(other)
    }

}

/// Any objects inheriting this trait must belong to a Binder as its entries
/// 
/// Expects an Identifier (type of its id, e.g String) and a function `identity` that returns a reference to its id.
/// This is so that the parent Binder can identify it and perform actions based on which entry it is.
/// 
//TODO: implement more stuff here
pub trait BinderEntry {
    type Identifier: PartialEq + identifiers::Identifier; //.name / .id etc

    fn identity(&self) -> &Self::Identifier;

    ///Returns `true` if own identity matches given Regex pattern.
    /// 
    ///If `Self::Identifier` is an `Option`, None values will always return false.
    fn matches(&self, pattern: &Regex) -> bool {
        self.identity().matches(pattern)
    }
}



#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::{binary::decompress_if_needed, binders::bnd4::BND4, oodle::init_oodle};
    use super::*;

    #[test]
    fn test_all_flags() {
        for &bbe in &[false, true] {
            let bad: Vec<u8> = (0u8..=255)
                .filter(|&b| BinderFlags::from_byte(b, bbe).to_byte(bbe) != b)
                .collect();
            println!("bit_big_endian={bbe}: {} bad bytes: {:02X?}", bad.len(), bad);
        }
    }

    #[test]
    fn test_real_flags() {
        unsafe { init_oodle(Path::new("tests/oo2core_6_win64.dll")) };
        let data = std::fs::read("tests/01_common.sblytbnd.dcx").unwrap();
        let (bytes, _) = unsafe { decompress_if_needed(&data) }.unwrap();
        println!("original: unicode={:02X} flags={:02X} extended={:02X}", bytes[0x30], bytes[0x31], bytes[0x32]);

        let (mut bnd, _) = unsafe { BND4::read(Path::new("tests/01_common.sblytbnd.dcx")) }.unwrap();
        let out = bnd.to_bytes().unwrap();
        println!("rewritten: unicode={:02X} flags={:02X} extended={:02X}", out[0x30], out[0x31], out[0x32]);
    }
}