/// Logic adapted from SoulsFormatsNext and Constrata

use std::{num::TryFromIntError, path::Path};
use std::collections::HashMap;

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use encoding_rs::SHIFT_JIS;
use md5::{Digest, Md5};

use crate::errors::{FerrisoulsError, BinaryWriterError};

pub type Result<T> = std::result::Result<T, BinaryWriterError>;

#[derive(Debug, Clone)]
pub struct Reservation {
    offset: usize,
    length: usize,
}

#[derive(Debug, Clone)]
pub struct BinaryWriter {
    data: Vec<u8>,
    position: usize,

    pub big_endian: bool,
    pub varint_long: bool,

    steps: Vec<usize>,

    reservations: HashMap<String, Reservation>
}

impl BinaryWriter {
    //region Creation

    pub fn new(big_endian: bool, varint_long: bool) -> Self {
        Self {
            data: Vec::new(),
            position: 0,
            big_endian: big_endian,
            varint_long: varint_long,
            steps: Vec::new(),
            reservations: HashMap::new()
        }
    }

    pub fn with_capacity(capacity: usize, big_endian: bool, varint_long: bool) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            position: 0,
            big_endian: big_endian,
            varint_long: varint_long,
            steps: Vec::new(),
            reservations: HashMap::new()
        }
    }

    pub fn from_bytes(data: Vec<u8>, big_endian: bool, varint_long: bool) -> Self {
        let position = data.len();

        Self {
            data,
            position,
            big_endian: big_endian,
            varint_long: varint_long,
            steps: Vec::new(),
            reservations: HashMap::new()
        }
    }

    // Editing
    
    pub fn set(&mut self, data: Vec<u8>) {
        self.data = data
    }

    pub fn append(&mut self, data: Vec<u8>) {
        self.data.extend(data);
    }

    //region Position

    #[inline]
    pub fn position(&self) -> u64 {
        self.position as u64
    }

    #[inline]
    pub fn length(&self) -> u64 {
        self.data.len() as u64
    }

    #[inline]
    pub fn remaining(&self) -> u64 {
        self.data.len().saturating_sub(self.position) as u64
    }

    #[inline]
    pub fn is_at_end(&self) -> bool {
        self.position >= self.data.len()
    }

    pub fn set_position(&mut self, position: u64) -> Result<()> {
        let position = usize::try_from(position)?;

        self.position = position;

        // Expanding the writer allows seeking beyond the current end.
        if self.position > self.data.len() {
            self.data.resize(self.position, 0);
        }

        Ok(())
    }

    pub fn skip(&mut self, count: u64) -> Result<()> {
        let count = usize::try_from(count)?;

        let position = self
            .position
            .checked_add(count)
            .ok_or_else(|| {
                BinaryWriterError::InvalidData(
                    "Writer position overflow".into(),
                )
            })?;

        self.set_position(position as u64)
    }

    //region Output

    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or_else(|| {
                BinaryWriterError::InvalidData(
                    "Writer position overflow".into(),
                )
            })?;

        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        self.data[self.position..end].copy_from_slice(bytes);
        self.position = end;

        Ok(())
    }

    pub fn write_zeros(&mut self, count: usize) -> Result<()> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| {
                BinaryWriterError::InvalidData(
                    "Writer position overflow".into(),
                )
            })?;

        if end > self.data.len() {
            self.data.resize(end, 0);
        } else {
            self.data[self.position..end].fill(0);
        }

        self.position = end;

        Ok(())
    }

    //region Stepping

    pub fn step_in(&mut self, offset: u64) -> Result<()> {
        self.steps.push(self.position);
        self.set_position(offset)
    }

    pub fn step_out(&mut self) -> Result<()> {
        let position = self.steps.pop().ok_or_else(|| {
            BinaryWriterError::InvalidData(
                "Writer is already stepped all the way out.".into(),
            )
        })?;

        self.set_position(position as u64)
    }

    //region Alignment
    pub fn pad(&mut self, count: usize) -> Result<()> {
        self.write_bytes(&b"\0".repeat(count))
    }

    pub fn pad_align(&mut self, align: u64) -> Result<()> {
        if align == 0 {
            return Ok(());
        }

        let position = self.position();
        let remainder = position % align;

        if remainder != 0 {
            self.write_zeros((align - remainder) as usize)?;
        }

        Ok(())
    }

    pub fn pad_relative(&mut self, start: u64, align: u64) -> Result<()> {
        if align == 0 {
            return Ok(());
        }

        let relative = self.position().saturating_sub(start);
        let remainder = relative % align;

        if remainder != 0 {
            self.write_zeros((align - remainder) as usize)?;
        }

        Ok(())
    }

    //region Integers

    #[inline]
    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_bytes(&[value])
    }

    #[inline]
    pub fn write_i8(&mut self, value: i8) -> Result<()> {
        self.write_bytes(&[value as u8])
    }

    #[inline]
    pub fn write_u16(&mut self, value: u16) -> Result<()> {
        let mut bytes = [0u8; 2];

        if self.big_endian {
            BigEndian::write_u16(&mut bytes, value);
        } else {
            LittleEndian::write_u16(&mut bytes, value);
        }

        self.write_bytes(&bytes)
    }

    #[inline]
    pub fn write_i16(&mut self, value: i16) -> Result<()> {
        self.write_u16(value as u16)
    }

    #[inline]
    pub fn write_u24(&mut self, value: u32) -> Result<()> {
        if value > 0xFF_FFFF {
            return Err(BinaryWriterError::InvalidData(format!(
                "u24 value out of range: 0x{value:X}"
            )));
        }

        let mut bytes = [0u8; 3];

        if self.big_endian {
            BigEndian::write_u24(&mut bytes, value);
        } else {
            LittleEndian::write_u24(&mut bytes, value);
        }

        self.write_bytes(&bytes)
    }

    #[inline]
    pub fn write_i24(&mut self, value: i32) -> Result<()> {
        if !(-8_388_608..=8_388_607).contains(&value) {
            return Err(BinaryWriterError::InvalidData(format!(
                "i24 value out of range: {value}"
            )));
        }

        self.write_u24((value as u32) & 0xFF_FFFF)
    }

    #[inline]
    pub fn write_u32(&mut self, value: u32) -> Result<()> {
        let mut bytes = [0u8; 4];

        if self.big_endian {
            BigEndian::write_u32(&mut bytes, value);
        } else {
            LittleEndian::write_u32(&mut bytes, value);
        }

        self.write_bytes(&bytes)
    }

    #[inline]
    pub fn write_i32(&mut self, value: i32) -> Result<()> {
        self.write_u32(value as u32)
    }

    #[inline]
    pub fn write_u64(&mut self, value: u64) -> Result<()> {
        let mut bytes = [0u8; 8];

        if self.big_endian {
            BigEndian::write_u64(&mut bytes, value);
        } else {
            LittleEndian::write_u64(&mut bytes, value);
        }

        self.write_bytes(&bytes)
    }

    #[inline]
    pub fn write_i64(&mut self, value: i64) -> Result<()> {
        self.write_u64(value as u64)
    }

    #[inline]
    pub fn write_u128(&mut self, value: u128) -> Result<()> {
        let bytes = if self.big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };

        self.write_bytes(&bytes)
    }

    #[inline]
    pub fn write_i128(&mut self, value: i128) -> Result<()> {
        self.write_u128(value as u128)
    }

    //region Floats

    #[inline]
    pub fn write_f32(&mut self, value: f32) -> Result<()> {
        self.write_u32(value.to_bits())
    }

    #[inline]
    pub fn write_f64(&mut self, value: f64) -> Result<()> {
        self.write_u64(value.to_bits())
    }

    //region Bool

    pub fn write_boolean(&mut self, value: bool) -> Result<()> {
        self.write_u8(value as u8)
    }

    //region Fixed-wdith

    pub fn write_varint(&mut self, value: i64) -> Result<()> {
        if self.varint_long {
            self.write_i64(value)
        } else {
            let value = i32::try_from(value)?;
            self.write_i32(value)
        }
    }

    pub fn reserve_varint(&mut self, name: String) -> Result<()> {
        if self.varint_long {
            self.reserve::<i64>(name)?;
        } else {
            self.reserve::<i32>(name)?;
        };

        Ok(())
    }

    pub fn fill_varint(&mut self, name: String, value: i64) -> Result<()> {
        let res = self.reservations.get(&name)
            .ok_or(BinaryWriterError::Custom("Reservation doesn't exist!".to_string()))?;

        let old_position = self.position();

        self.set_position(res.offset as u64)?;

        if self.varint_long {
            self.write::<i64>(value)?;
        } else {
            self.write::<i32>(value as i32)?;
        }
        self.set_position(old_position)?;

        self.reservations.remove(&name);

        Ok(())
    }
   
    //region leb128/7bit ints

    pub fn write_leb128_u64(&mut self, mut value: u64) -> Result<()> {
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            self.write_u8(byte)?;

            if value == 0 {
                break;
            }
        }

        Ok(())
    }

    pub fn write_leb128_i64(&mut self, mut value: i64) -> Result<()> {
        loop {
            let byte = (value as u8) & 0x7F;
            let sign_bit = byte & 0x40 != 0;

            value >>= 7;

            let done =
                (value == 0 && !sign_bit) ||
                (value == -1 && sign_bit);

            self.write_u8(if done {
                byte
            } else {
                byte | 0x80
            })?;

            if done {
                break;
            }
        }

        Ok(())
    }

    pub fn write_7bit_u64(&mut self, mut value: u64) -> Result<()> {
        while value >= 0x80 {
            self.write_u8((value as u8) | 0x80)?;
            value >>= 7;
        }

        self.write_u8(value as u8)
    }

    //region ASCII/UTF8

    pub fn write_ascii(&mut self, value: &str) -> Result<()> {
        if !value.is_ascii() {
            return Err(BinaryWriterError::InvalidData(
                "String contains non-ASCII characters".into(),
            ));
        }

        self.write_bytes(value.as_bytes())?;
        self.write_u8(0)
    }

    pub fn write_ascii_fixed(
        &mut self,
        value: &str,
        length: usize,
    ) -> Result<()> {
        let bytes = value.as_bytes();

        if bytes.len() > length {
            return Err(BinaryWriterError::InvalidData(format!(
                "ASCII string is {} bytes, maximum is {length}",
                bytes.len()
            )));
        }

        self.write_bytes(bytes)?;
        self.write_zeros(length - bytes.len())
    }

    pub fn write_utf8(&mut self, value: &str) -> Result<()> {
        self.write_bytes(value.as_bytes())?;
        self.write_u8(0)
    }

    pub fn write_utf8_fixed(
        &mut self,
        value: &str,
        length: usize,
    ) -> Result<()> {
        let bytes = value.as_bytes();

        if bytes.len() > length {
            return Err(BinaryWriterError::InvalidData(format!(
                "UTF-8 string is {} bytes, maximum is {length}",
                bytes.len()
            )));
        }

        self.write_bytes(bytes)?;
        self.write_zeros(length - bytes.len())
    }

    //region shift-jis

    pub fn write_shift_jis(&mut self, value: &str, terminate: bool) -> Result<()> {
        let (encoded, _, had_errors) = SHIFT_JIS.encode(value);

        if had_errors {
            return Err(BinaryWriterError::InvalidData(
                "String cannot be represented in Shift-JIS".into(),
            ));
        }

        self.write_bytes(&encoded)?;

        if terminate {
            self.write_u8(0)?;
        }
        Ok(())
    }

    pub fn write_shift_jis_fixed(
        &mut self,
        value: &str,
        length: usize,
    ) -> Result<()> {
        let (encoded, _, had_errors) = SHIFT_JIS.encode(value);

        if had_errors {
            return Err(BinaryWriterError::InvalidData(
                "String cannot be represented in Shift-JIS".into(),
            ));
        }

        if encoded.len() > length {
            return Err(BinaryWriterError::InvalidData(format!(
                "Shift-JIS string is {} bytes, maximum is {length}",
                encoded.len()
            )));
        }

        self.write_bytes(&encoded)?;
        self.write_zeros(length - encoded.len())
    }

    //region len-prefixed strings

    pub fn write_string_u8(&mut self, value: &str) -> Result<()> {
        let len = u8::try_from(value.len())?;

        self.write_u8(len)?;
        self.write_bytes(value.as_bytes())
    }

    pub fn write_string_u16(&mut self, value: &str) -> Result<()> {
        let len = u16::try_from(value.len())?;

        self.write_u16(len)?;
        self.write_bytes(value.as_bytes())
    }

    pub fn write_string_u32(&mut self, value: &str) -> Result<()> {
        let len = u32::try_from(value.len())?;

        self.write_u32(len)?;
        self.write_bytes(value.as_bytes())
    }

    //region utf16

    pub fn write_utf16(&mut self, value: &str, terminate: bool) -> Result<()> {
        let units = value.encode_utf16();

        for unit in units {
            self.write_u16(unit)?;
        }

        if terminate {
            self.write_u16(0)?;
        }
        Ok(())
    }

    pub fn write_utf16_fixed(
        &mut self,
        value: &str,
        units: usize,
    ) -> Result<()> {
        let encoded: Vec<u16> = value.encode_utf16().collect();
        let encoded_len = encoded.len();

        if encoded_len > units {
            return Err(BinaryWriterError::InvalidData(format!(
                "UTF-16 string contains {} units, maximum is {units}",
                encoded_len
            )));
        }

        for unit in encoded {
            self.write_u16(unit)?;
        }

        for _ in 0..(units - value.encode_utf16().count()) {
            self.write_u16(0)?;
        }

        Ok(())
    }

    //region write at offset

    pub fn write_at(
        &mut self,
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        let old_position = self.position();

        self.set_position(offset)?;
        self.write_bytes(bytes)?;
        self.set_position(old_position)?;

        Ok(())
    }

    pub fn write_u32_at(
        &mut self,
        offset: u64,
        value: u32,
    ) -> Result<()> {
        let old_position = self.position();

        self.set_position(offset)?;
        self.write_u32(value)?;
        self.set_position(old_position)?;

        Ok(())
    }

    pub fn write_u64_at(
        &mut self,
        offset: u64,
        value: u64,
    ) -> Result<()> {
        let old_position = self.position();

        self.set_position(offset)?;
        self.write_u64(value)?;
        self.set_position(old_position)?;

        Ok(())
    }

    //region Generic types

    pub fn write<T: Writable>(&mut self, value: T) -> Result<()> {
        T::write_to(&value, self)
    }

    //region Reserves and patches

    pub fn reserve<T: Writable>(&mut self, name: String) -> Result<u64> {
        let offset = self.position();
        let size = std::mem::size_of::<T>();

        self.write_zeros(size)?;

        self.reservations.insert(name, Reservation { offset: offset as usize, length: size});

        Ok(offset)
    }

    pub fn fill<T: Writable>(&mut self, name: String, value: T) -> Result<()> {
        let res = self.reservations.get(&name)
            .ok_or(BinaryWriterError::Custom("Reservation doesn't exist!".to_string()))?;

        let old_position = self.position();

        self.set_position(res.offset as u64)?;
        self.write::<T>(value)?;
        self.set_position(old_position)?;

        self.reservations.remove(&name);

        Ok(())
    }


    pub fn fill_iter<T: Writable + ExactSizeIterator>(&mut self, name: String, value: T) -> Result<()> {
        let res = self.reservations.get(&name)
            .ok_or(BinaryWriterError::Custom("Reservation doesn't exist!".to_string()))?;

        if value.len() > res.length {
            return Err(BinaryWriterError::Custom("Value doesn't match reservation length!".to_string()));
        }

        let old_position = self.position();

        self.set_position(res.offset as u64);
        self.write::<T>(value);
        self.set_position(old_position)?;

        self.reservations.remove(&name);

        Ok(())
    }

    pub fn patch_u8(
        &mut self,
        offset: u64,
        value: u8,
    ) -> Result<()> {
        self.write_at(offset, &[value])
    }

    pub fn patch_u32(
        &mut self,
        offset: u64,
        value: u32,
    ) -> Result<()> {
        self.write_u32_at(offset, value)
    }

    pub fn patch_u64(
        &mut self,
        offset: u64,
        value: u64,
    ) -> Result<()> {
        self.write_u64_at(offset, value)
    }

    //region Hashing
    pub fn prepend_md5_hash(&mut self) -> Result<()> {
        let mut hasher = Md5::new();
        hasher.update(self.data.clone());
        let hash = hasher.finalize();
        self.data.splice(0..0, hash);
        Ok(())
    }

}

impl Default for BinaryWriter {
    fn default() -> Self {
        Self::new(true, false)
    }
}




pub trait Writable: Sized {
    fn write_to(&self, writer: &mut BinaryWriter) -> Result<()>;
}

macro_rules! impl_writable {
    ($($ty:ty => $method:ident),* $(,)?) => {
        $(
            impl Writable for $ty {
                #[inline]
                fn write_to(&self, writer: &mut BinaryWriter) -> Result<()> {
                    writer.$method(*self)
                }
            }
        )*
    };
}

impl_writable! {
    u8   => write_u8,
    i8   => write_i8,
    u16  => write_u16,
    i16  => write_i16,
    u32  => write_u32,
    i32  => write_i32,
    u64  => write_u64,
    i64  => write_i64,
    u128 => write_u128,
    i128 => write_i128,
    f32  => write_f32,
    f64  => write_f64,
    bool => write_boolean,
}

