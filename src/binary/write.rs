/// Logic adapted from SoulsFormatsNext and Constrata

use std::{borrow::Cow, fmt::Debug};
use std::collections::HashMap;

use encoding_rs::SHIFT_JIS;
use md5::{Digest, Md5};
use zerocopy::IntoBytes;

use crate::errors::BinaryWriterError;

pub type Result<T> = std::result::Result<T, BinaryWriterError>;

#[inline]
fn invalid(msg: impl Into<String>) -> BinaryWriterError {
    BinaryWriterError::InvalidData(msg.into())
}


pub trait Writable: Sized {
    /// Number of bytes this type occupies when written.
    const SIZE: usize = std::mem::size_of::<Self>();

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

//region BinaryWriter

#[derive(Debug, Clone, Copy)]
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
    reservations: HashMap<String, Reservation>,
}

/// Generates the endian-aware writers for the plain integer types.
macro_rules! int_writers {
    ($($ty:ty => $name:ident, $le:ident, $be:ident);* $(;)?) => {
        $(
            #[inline]
            pub fn $name(&mut self, value: $ty) -> Result<()> {
                if self.big_endian {
                    self.$be(value)
                } else {
                    self.$le(value)
                }
            }

            #[inline]
            pub fn $le(&mut self, value: $ty) -> Result<()> {
                self.write_bytes(&value.to_le_bytes())
            }

            #[inline]
            pub fn $be(&mut self, value: $ty) -> Result<()> {
                self.write_bytes(&value.to_be_bytes())
            }
        )*
    };
}

impl BinaryWriter {
    //region Creation

    pub fn new(big_endian: bool, varint_long: bool) -> Self {
        Self::from_parts(Vec::new(), 0, big_endian, varint_long)
    }

    pub fn with_capacity(capacity: usize, big_endian: bool, varint_long: bool) -> Self {
        Self::from_parts(Vec::with_capacity(capacity), 0, big_endian, varint_long)
    }

    /// Wraps existing bytes; the position starts at the end (append mode).
    pub fn from_bytes(data: Vec<u8>, big_endian: bool, varint_long: bool) -> Self {
        let position = data.len();
        Self::from_parts(data, position, big_endian, varint_long)
    }

    fn from_parts(data: Vec<u8>, position: usize, big_endian: bool, varint_long: bool) -> Self {
        Self {
            data,
            position,
            big_endian,
            varint_long,
            steps: Vec::new(),
            reservations: HashMap::new(),
        }
    }

    //region Editing

    pub fn set(&mut self, data: Vec<u8>) {
        self.data = data;
    }

    pub fn append(&mut self, data: impl AsRef<[u8]>) {
        self.data.extend_from_slice(data.as_ref());
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

    /// Seeks to `position`. Seeking past the end zero-extends the buffer.
    pub fn set_position(&mut self, position: u64) -> Result<()> {
        self.position = usize::try_from(position)?;

        if self.position > self.data.len() {
            self.data.resize(self.position, 0);
        }

        Ok(())
    }

    /// Position after advancing `count` bytes, with overflow checking.
    fn end_after(&self, count: usize) -> Result<usize> {
        self.position
            .checked_add(count)
            .ok_or_else(|| invalid("Writer position overflow!"))
    }

    pub fn skip(&mut self, count: u64) -> Result<()> {
        let end = self.end_after(usize::try_from(count)?)?;
        self.set_position(end as u64)
    }

    /// Runs `f` with the writer positioned at `offset`, then restores the
    /// previous position (even if `f` fails).
    pub fn with_position<R>(
        &mut self,
        offset: u64,
        f: impl FnOnce(&mut Self) -> Result<R>,
    ) -> Result<R> {
        let old_position = self.position;

        self.set_position(offset)?;
        let result = f(self);

        // The buffer only ever grows, so the old position is always valid.
        self.position = old_position;
        result
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
        let end = self.end_after(bytes.len())?;

        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        self.data[self.position..end].copy_from_slice(bytes);
        self.position = end;

        Ok(())
    }

    pub fn write_pattern(&mut self, pattern: u8, length: usize) -> Result<()> {
        self.write_bytes(&pattern.as_bytes().repeat(length))
    }

    pub fn write_zeros(&mut self, count: usize) -> Result<()> {
        let end = self.end_after(count)?;

        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        self.data[self.position..end].fill(0);
        self.position = end;

        Ok(())
    }

    //region Stepping

    pub fn step_in(&mut self, offset: u64) -> Result<()> {
        self.steps.push(self.position);
        self.set_position(offset)
    }

    pub fn step_out(&mut self) -> Result<()> {
        let position = self
            .steps
            .pop()
            .ok_or_else(|| invalid("Writer is already stepped all the way out."))?;

        self.set_position(position as u64)
    }

    //region Alignment

    pub fn pad(&mut self, count: usize) -> Result<()> {
        self.write_bytes(&b"\0".repeat(count))
    }

    pub fn pad_align(&mut self, align: u64) -> Result<()> {
        self.pad_relative(0, align)
    }

    pub fn pad_relative(&mut self, start: u64, align: u64) -> Result<()> {
        if align == 0 {
            return Ok(());
        }

        let remainder = self.position().saturating_sub(start) % align;

        if remainder != 0 {
            self.write_zeros((align - remainder) as usize)?;
        }

        Ok(())
    }

    //region Integers & floats

    int_writers! {
        u16  => write_u16,  write_u16_le,  write_u16_be;
        i16  => write_i16,  write_i16_le,  write_i16_be;
        u32  => write_u32,  write_u32_le,  write_u32_be;
        i32  => write_i32,  write_i32_le,  write_i32_be;
        u64  => write_u64,  write_u64_le,  write_u64_be;
        i64  => write_i64,  write_i64_le,  write_i64_be;
        u128 => write_u128, write_u128_le, write_u128_be;
        i128 => write_i128, write_i128_le, write_i128_be;
    }

    #[inline]
    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_bytes(&[value])
    }

    #[inline]
    pub fn write_i8(&mut self, value: i8) -> Result<()> {
        self.write_bytes(&value.to_ne_bytes())
    }

    #[inline]
    pub fn write_boolean(&mut self, value: bool) -> Result<()> {
        self.write_u8(value as u8)
    }

    // 24-bit

    fn write_u24_endian(&mut self, value: u32, big_endian: bool) -> Result<()> {
        if value > 0xFF_FFFF {
            return Err(invalid(format!("u24 value out of range: 0x{value:X}")));
        }

        let le = value.to_le_bytes();
        let mut bytes = [le[0], le[1], le[2]];

        if big_endian {
            bytes.reverse();
        }

        self.write_bytes(&bytes)
    }

    fn write_i24_endian(&mut self, value: i32, big_endian: bool) -> Result<()> {
        if !(-8_388_608..=8_388_607).contains(&value) {
            return Err(invalid(format!("i24 value out of range: {value}")));
        }

        self.write_u24_endian((value as u32) & 0xFF_FFFF, big_endian)
    }

    #[inline]
    pub fn write_u24(&mut self, value: u32) -> Result<()> {
        self.write_u24_endian(value, self.big_endian)
    }

    #[inline]
    pub fn write_u24_le(&mut self, value: u32) -> Result<()> {
        self.write_u24_endian(value, false)
    }

    #[inline]
    pub fn write_u24_be(&mut self, value: u32) -> Result<()> {
        self.write_u24_endian(value, true)
    }

    #[inline]
    pub fn write_i24(&mut self, value: i32) -> Result<()> {
        self.write_i24_endian(value, self.big_endian)
    }

    #[inline]
    pub fn write_i24_le(&mut self, value: i32) -> Result<()> {
        self.write_i24_endian(value, false)
    }

    #[inline]
    pub fn write_i24_be(&mut self, value: i32) -> Result<()> {
        self.write_i24_endian(value, true)
    }

    // Floats

    #[inline]
    pub fn write_f32(&mut self, value: f32) -> Result<()> {
        self.write_u32(value.to_bits())
    }

    #[inline]
    pub fn write_f32_le(&mut self, value: f32) -> Result<()> {
        self.write_u32_le(value.to_bits())
    }

    #[inline]
    pub fn write_f32_be(&mut self, value: f32) -> Result<()> {
        self.write_u32_be(value.to_bits())
    }

    #[inline]
    pub fn write_f64(&mut self, value: f64) -> Result<()> {
        self.write_u64(value.to_bits())
    }

    #[inline]
    pub fn write_f64_le(&mut self, value: f64) -> Result<()> {
        self.write_u64_le(value.to_bits())
    }

    #[inline]
    pub fn write_f64_be(&mut self, value: f64) -> Result<()> {
        self.write_u64_be(value.to_bits())
    }

    //region Generic writing

    #[inline]
    pub fn write<T: Writable>(&mut self, value: T) -> Result<()> {
        value.write_to(self)
    }

    pub fn write_slice<T: Writable>(&mut self, values: &[T]) -> Result<()> {
        values.iter().try_for_each(|v| v.write_to(self))
    }

    //region Varints (width depends on `varint_long`)

    pub fn write_varint(&mut self, value: i64) -> Result<()> {
        if self.varint_long {
            self.write(value)
        } else {
            self.write(i32::try_from(value)?)
        }
    }

    pub fn reserve_varint(&mut self, name: impl Into<String>) -> Result<()> {
        if self.varint_long {
            self.reserve::<i64>(name)?;
        } else {
            self.reserve::<i32>(name)?;
        }

        Ok(())
    }

    pub fn fill_varint(&mut self, name: impl AsRef<str>, value: i64) -> Result<()> {
        if self.varint_long {
            self.fill(name, value)
        } else {
            self.fill(name, i32::try_from(value)?)
        }
    }

    //region LEB128 / 7-bit ints

    pub fn write_leb128_u64(&mut self, mut value: u64) -> Result<()> {
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            self.write_u8(byte)?;

            if value == 0 {
                return Ok(());
            }
        }
    }

    pub fn write_leb128_i64(&mut self, mut value: i64) -> Result<()> {
        loop {
            let byte = (value as u8) & 0x7F;
            let sign_bit = byte & 0x40 != 0;

            value >>= 7;

            let done = (value == 0 && !sign_bit) || (value == -1 && sign_bit);

            self.write_u8(if done { byte } else { byte | 0x80 })?;

            if done {
                return Ok(());
            }
        }
    }

    /// .NET-style 7-bit encoded int; byte-for-byte identical to unsigned LEB128.
    #[inline]
    pub fn write_7bit_u64(&mut self, value: u64) -> Result<()> {
        self.write_leb128_u64(value)
    }

    //region Strings: shared helpers

    /// Writes `bytes` followed by zero padding up to `length` bytes.
    fn write_padded(&mut self, bytes: &[u8], length: usize, kind: &str) -> Result<()> {
        if bytes.len() > length {
            return Err(invalid(format!(
                "{kind} string is {} bytes, maximum is {length}",
                bytes.len()
            )));
        }

        self.write_bytes(bytes)?;
        self.write_zeros(length - bytes.len())
    }

    fn write_and_pad(&mut self, bytes: &[u8], length: usize, pad_with: u8) -> Result<()> {
        self.write_bytes(bytes)?;
        let pad = pad_with.as_bytes().repeat(length - bytes.len());
        self.write_bytes(&pad)
    }

    fn write_null_terminated(&mut self, bytes: &[u8]) -> Result<()> {
        self.write_bytes(bytes)?;
        self.write_u8(0)
    }

    fn encode_shift_jis<'a>(value: &'a str) -> Result<Cow<'a, [u8]>> {
        let (encoded, _, had_errors) = SHIFT_JIS.encode(value);

        if had_errors {
            return Err(invalid("String cannot be represented in Shift-JIS"));
        }

        Ok(encoded)
    }

    fn ensure_ascii(value: &str) -> Result<()> {
        if value.is_ascii() {
            Ok(())
        } else {
            Err(invalid("String contains non-ASCII characters"))
        }
    }

    //region ASCII / UTF-8

    pub fn write_ascii(&mut self, value: &str, terminate: bool) -> Result<()> {
        Self::ensure_ascii(value)?;
        self.write_bytes(value.as_bytes())?;
        if terminate {
            self.write_u8(0u8)?;
        }
        Ok(())
    }

    pub fn write_ascii_fixed(&mut self, value: &str, length: usize) -> Result<()> {
        Self::ensure_ascii(value)?;
        self.write_padded(value.as_bytes(), length, "ASCII")
    }

    pub fn write_utf8(&mut self, value: &str, terminate: bool) -> Result<()> {
        self.write_bytes(value.as_bytes())?;
        if terminate {
            self.write_u8(0u8)?;
        }
        Ok(())
    }

    pub fn write_utf8_fixed(&mut self, value: &str, length: usize) -> Result<()> {
        self.write_padded(value.as_bytes(), length, "UTF-8")
    }

    //region Shift-JIS

    pub fn write_shift_jis(&mut self, value: &str, terminate: bool) -> Result<()> {
        let encoded = Self::encode_shift_jis(value)?;

        self.write_bytes(&encoded)?;

        if terminate {
            self.write_u8(0)?;
        }

        Ok(())
    }

    pub fn write_shift_jis_fixed(&mut self, value: &str, length: usize, pad_with: u8) -> Result<()> {
        let encoded = Self::encode_shift_jis(value)?;
        self.write_and_pad(&encoded, length, pad_with)
    }

    //region Length-prefixed strings

    /// Writes the byte length as `L`, then the raw UTF-8 bytes.
    pub fn write_string_prefixed<L>(&mut self, value: &str) -> Result<()>
    where
        L: Writable + TryFrom<usize>,
        BinaryWriterError: From<<L as TryFrom<usize>>::Error>,
    {
        self.write(L::try_from(value.len())?)?;
        self.write_bytes(value.as_bytes())
    }

    pub fn write_string_u8(&mut self, value: &str) -> Result<()> {
        self.write_string_prefixed::<u8>(value)
    }

    pub fn write_string_u16(&mut self, value: &str) -> Result<()> {
        self.write_string_prefixed::<u16>(value)
    }

    pub fn write_string_u32(&mut self, value: &str) -> Result<()> {
        self.write_string_prefixed::<u32>(value)
    }

    //region UTF-16

    pub fn write_utf16(&mut self, value: &str, terminate: bool) -> Result<()> {
        for unit in value.encode_utf16() {
            self.write_u16(unit)?;
        }

        if terminate {
            self.write_u16(0)?;
        }

        Ok(())
    }

    /// Writes `value` padded with zero code units up to `length` UTF-16 units.
    pub fn write_utf16_fixed(&mut self, value: &str, length: usize, pad_with: u8) -> Result<()> {
        let encoded: Vec<u16> = value.encode_utf16().collect();

        if encoded.len() > length {
            return Err(invalid(format!(
                "UTF-16 string contains {} units, maximum is {length}",
                encoded.len()
            )));
        }

        self.write_slice(&encoded)?;
        self.write_pattern(pad_with, (length - encoded.len()) * 2)
    }

    //region Write at offset (position is restored afterwards)

    pub fn write_at(&mut self, offset: u64, bytes: &[u8]) -> Result<()> {
        self.with_position(offset, |w| w.write_bytes(bytes))
    }

    pub fn write_value_at<T: Writable>(&mut self, offset: u64, value: T) -> Result<()> {
        self.with_position(offset, |w| w.write(value))
    }

    //region Reservations

    /// Writes `T::SIZE` zero bytes and remembers where, so the value can be
    /// supplied later with [`fill`](Self::fill). Returns the reserved offset.
    pub fn reserve<T: Writable>(&mut self, name: impl Into<String>) -> Result<u64> {
        let offset = self.position();

        self.write_zeros(T::SIZE)?;
        self.reservations.insert(
            name.into(),
            Reservation { offset: offset as usize, length: T::SIZE },
        );

        Ok(offset)
    }

    /// Fills a reservation and consumes it. `T` must match the size it was reserved with.
    pub fn fill<T: Writable>(&mut self, name: impl AsRef<str>, value: T) -> Result<()> where T: Debug {
        let name = name.as_ref();

        let res = *self
            .reservations
            .get(name)
            .ok_or_else(|| BinaryWriterError::Custom(
                format!("Reservation doesn't exist! Name: {}, Value: {:?}",
                name, value)
            ))?;

        if T::SIZE != res.length {
            return Err(invalid(format!(
                "Reservation '{name}' is {} bytes, but value is {} bytes",
                res.length,
                T::SIZE
            )));
        }

        self.write_value_at(res.offset as u64, value)?;
        self.reservations.remove(name);

        Ok(())
    }

    //region Hashing

    pub fn prepend_md5_hash(&mut self) -> Result<()> {
        let hash = Md5::digest(&self.data);
        self.data.splice(0..0, hash);

        Ok(())
    }
}

impl Default for BinaryWriter {
    fn default() -> Self {
        Self::new(true, false)
    }
}