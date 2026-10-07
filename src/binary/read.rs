/// Logic adapted from SoulsFormatsNext and Constrata

use std::fmt::Debug;
use std::io::Cursor;

use encoding_rs::SHIFT_JIS;
use zerocopy::IntoBytes;

use crate::errors::BinaryReaderError;

pub type Result<T> = std::result::Result<T, BinaryReaderError>;

#[inline]
fn invalid(msg: impl Into<String>) -> BinaryReaderError {
    BinaryReaderError::InvalidData(msg.into())
}

fn assertion_failed(found: &dyn Debug, expected: &dyn Debug, loc: u64) -> BinaryReaderError {
    BinaryReaderError::Custom(format!(
        "Asserted value was incorrect! Got: `{found:?}` | Expected: `{expected:?}` | At: {loc:?}"
    ))
}

/// Truncates at the first NUL byte (for fixed-width string fields).
fn trim_nul(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

//region Readable

pub trait Readable: Sized {
    fn read_from(reader: &mut BinaryReader<'_>) -> Result<Self>;
}

macro_rules! impl_readable {
    ($($ty:ty => $method:ident),* $(,)?) => {
        $(
            impl Readable for $ty {
                #[inline]
                fn read_from(reader: &mut BinaryReader<'_>) -> Result<Self> {
                    reader.$method()
                }
            }
        )*
    };
}

impl_readable! {
    u8   => read_u8,
    i8   => read_i8,
    u16  => read_u16,
    i16  => read_i16,
    u32  => read_u32,
    i32  => read_i32,
    u64  => read_u64,
    i64  => read_i64,
    u128 => read_u128,
    i128 => read_i128,
    f32  => read_f32,
    f64  => read_f64,
    bool => read_boolean,
}

//region BinaryReader

#[derive(Debug, Clone)]
pub struct BinaryReader<'a> {
    pub data: Cursor<&'a [u8]>,
    pub big_endian: bool,
    pub varint_long: bool,

    steps: Vec<u64>,
}

/// Generates the endian-aware readers for the plain integer types.
macro_rules! int_readers {
    ($($ty:ty => $name:ident, $le:ident, $be:ident);* $(;)?) => {
        $(
            #[inline]
            pub fn $name(&mut self) -> Result<$ty> {
                if self.big_endian {
                    self.$be()
                } else {
                    self.$le()
                }
            }

            #[inline]
            pub fn $le(&mut self) -> Result<$ty> {
                const N: usize = std::mem::size_of::<$ty>();
                Ok(<$ty>::from_le_bytes(self.read_array::<N>()?))
            }

            #[inline]
            pub fn $be(&mut self) -> Result<$ty> {
                const N: usize = std::mem::size_of::<$ty>();
                Ok(<$ty>::from_be_bytes(self.read_array::<N>()?))
            }
        )*
    };
}

impl<'a> BinaryReader<'a> {
    //region Creation

    pub fn new(data: &'a [u8], big_endian: bool, varint_long: bool) -> Self {
        Self {
            data: Cursor::new(data),
            big_endian,
            varint_long,
            steps: Vec::new(),
        }
    }

    #[deprecated(note = "use `BinaryReader::new`")]
    pub fn from(data: &'a [u8], big_endian: bool, varint_long: bool) -> Self {
        Self::new(data, big_endian, varint_long)
    }

    //region Position

    #[inline]
    pub fn position(&self) -> u64 {
        self.data.position()
    }

    #[inline]
    pub fn length(&self) -> u64 {
        self.data.get_ref().len() as u64
    }

    #[inline]
    pub fn remaining(&self) -> u64 {
        self.length().saturating_sub(self.position())
    }

    #[inline]
    pub fn is_at_end(&self) -> bool {
        self.position() >= self.length()
    }

    pub fn set_position(&mut self, position: u64) -> Result<()> {
        if position > self.length() {
            return Err(BinaryReaderError::OutOfBounds {
                offset: position,
                length: 0,
                total: self.length(),
            });
        }

        self.data.set_position(position);
        Ok(())
    }

    pub fn skip(&mut self, count: u64) -> Result<()> {
        let position = self
            .position()
            .checked_add(count)
            .ok_or_else(|| invalid("Reader position overflow"))?;

        self.set_position(position)
    }

    /// Runs `f` with the reader positioned at `offset`, then restores the
    /// previous position (even if `f` fails).
    pub fn with_position<R>(
        &mut self,
        offset: u64,
        f: impl FnOnce(&mut Self) -> Result<R>
    ) -> Result<R> {
        let old_position = self.position();

        self.set_position(offset)?;
        let result = f(self);

        // The data is immutable, so the old position is always still valid.
        self.data.set_position(old_position);
        result
    }

    //region Stepping

    pub fn step_in(&mut self, offset: u64) -> Result<()> {
        let old_position = self.position();

        self.set_position(offset)?;
        self.steps.push(old_position);

        Ok(())
    }

    pub fn step_out(&mut self) -> Result<()> {
        let position = self
            .steps
            .pop()
            .ok_or_else(|| invalid("Reader is already stepped all the way out."))?;

        self.set_position(position)
    }

    //region Alignment

    pub fn align(&mut self, alignment: u64) -> Result<()> {
        self.align_relative(0, alignment)
    }

    pub fn align_relative(&mut self, start: u64, alignment: u64) -> Result<()> {
        if alignment == 0 {
            return Ok(());
        }

        let remainder = self.position().saturating_sub(start) % alignment;

        if remainder != 0 {
            self.skip(alignment - remainder)?;
        }

        Ok(())
    }

    //region Raw input

    /// Borrows the next `count` bytes straight from the underlying data (no copy).
    pub fn read_slice(&mut self, count: usize) -> Result<&'a [u8]> {
        let position = self.position();
        let data: &'a [u8] = *self.data.get_ref();
        let start = position as usize;

        let slice = start
            .checked_add(count)
            .and_then(|end| data.get(start..end))
            .ok_or(BinaryReaderError::UnexpectedEof {
                position,
                requested: count,
                remaining: self.remaining() as usize,
            })?;

        self.data.set_position(position + count as u64);
        Ok(slice)
    }

    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut array = [0u8; N];
        array.copy_from_slice(self.read_slice(N)?);
        Ok(array)
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        self.read_slice(count).map(<[u8]>::to_vec)
    }

    pub fn read_bytes_into(&mut self, bytes: &mut [u8]) -> Result<()> {
        bytes.copy_from_slice(self.read_slice(bytes.len())?);
        Ok(())
    }

    /// Reads bytes up to (and consuming) the next NUL, returning them without it.
    fn read_cstring_slice(&mut self) -> Result<&'a [u8]> {
        let start = self.position();
        let data: &'a [u8] = *self.data.get_ref();
        let rest = data.get(start as usize..).unwrap_or(&[]);

        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(BinaryReaderError::UnexpectedEof {
                position: start,
                requested: 1,
                remaining: 0,
            })?;

        self.data.set_position(start + len as u64 + 1);
        Ok(&rest[..len])
    }

    /// Reads a fixed-width field and truncates it at the first NUL.
    fn read_fixed_slice(&mut self, length: usize) -> Result<&'a [u8]> {
        self.read_slice(length).map(trim_nul)
    }

    //region Generic reading

    #[inline]
    pub fn read<T: Readable>(&mut self) -> Result<T> {
        T::read_from(self)
    }

    pub fn read_vec<T: Readable>(&mut self, count: usize) -> Result<Vec<T>> {
        (0..count).map(|_| self.read()).collect()
    }

    /// Reads a `T` without advancing the position.
    pub fn peek<T: Readable>(&mut self) -> Result<T> {
        let position = self.position();
        self.with_position(position, |r| r.read())
    }

    /// Reads a `T` at  `offset` without advancing the position.
    pub fn get<T: Readable>(&mut self, offset: u64) -> Result<T> {
        self.with_position(offset, |r| r.read())
    }

    /// Reads a `T` at `offset`, then returns to the current position.
    pub fn read_value_at<T: Readable>(&mut self, offset: u64) -> Result<T> {
        self.with_position(offset, |r| r.read())
    }

    pub fn peek_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        let position = self.position();
        self.with_position(position, |r| r.read_bytes(count))
    }

    pub fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        self.with_position(offset, |r| r.read_bytes(length))
    }

    //region Assertions

    /// Reads a `T` and errors unless it equals `expected`.
    pub fn assert<T>(&mut self, expected: T) -> Result<T>
    where
        T: Readable + PartialEq + Debug,
    {
        let found = self.read::<T>()?;

        if found != expected {
            return Err(assertion_failed(&found, &expected, self.position()));
        }

        Ok(found)
    }

    pub fn assert_varint(&mut self, expected: i64) -> Result<i64> {
        let found = self.read_varint()?;

        if found != expected {
            return Err(assertion_failed(&found, &expected, self.position()));
        }

        Ok(found)
    }

    pub fn assert_bytes(&mut self, expected: &[u8]) -> Result<Vec<u8>> {
        let found = self.read_slice(expected.len())?;

        if found != expected {
            return Err(assertion_failed(&found, &expected, self.position()));
        }

        Ok(found.to_vec())
    }

    pub fn assert_pattern(&mut self, pattern: u8, length: usize) -> Result<Vec<u8>> {
        self.assert_bytes(&pattern.as_bytes().repeat(length))
    }

    ///Asserts that value returned by given closure `f` is in array `values`. Returns which value matched, else error.
    pub fn matches<R: Readable>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<R>,
        values: &[R]
    ) -> Result<R> 
    where R: PartialEq<R> + Debug + Clone {
        let read = f(self)?;
        for v in values {
            if *v == read {
                return Ok(v.clone())
            }
        }
        Err(BinaryReaderError::Custom(format!("Match failed! {:?} was not in asserted array: {:?}", read, values)))
    }

    //region Integers & floats

    int_readers! {
        u16  => read_u16,  read_u16_le,  read_u16_be;
        i16  => read_i16,  read_i16_le,  read_i16_be;
        u32  => read_u32,  read_u32_le,  read_u32_be;
        i32  => read_i32,  read_i32_le,  read_i32_be;
        u64  => read_u64,  read_u64_le,  read_u64_be;
        i64  => read_i64,  read_i64_le,  read_i64_be;
        u128 => read_u128, read_u128_le, read_u128_be;
        i128 => read_i128, read_i128_le, read_i128_be;
    }

    #[inline]
    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_slice(1)?[0])
    }

    #[inline]
    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_boolean(&mut self) -> Result<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(invalid(format!("Invalid boolean: 0x{value:02X}"))),
        }
    }

    // 24-bit

    fn read_u24_endian(&mut self, big_endian: bool) -> Result<u32> {
        let mut bytes = self.read_array::<3>()?;

        if big_endian {
            bytes.reverse();
        }

        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
    }

    fn read_i24_endian(&mut self, big_endian: bool) -> Result<i32> {
        // Shift into the top 24 bits, then arithmetic-shift back to sign extend.
        Ok(((self.read_u24_endian(big_endian)? << 8) as i32) >> 8)
    }

    #[inline]
    pub fn read_u24(&mut self) -> Result<u32> {
        self.read_u24_endian(self.big_endian)
    }

    #[inline]
    pub fn read_u24_le(&mut self) -> Result<u32> {
        self.read_u24_endian(false)
    }

    #[inline]
    pub fn read_u24_be(&mut self) -> Result<u32> {
        self.read_u24_endian(true)
    }

    #[inline]
    pub fn read_i24(&mut self) -> Result<i32> {
        self.read_i24_endian(self.big_endian)
    }

    #[inline]
    pub fn read_i24_le(&mut self) -> Result<i32> {
        self.read_i24_endian(false)
    }

    #[inline]
    pub fn read_i24_be(&mut self) -> Result<i32> {
        self.read_i24_endian(true)
    }

    // Floats

    #[inline]
    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    #[inline]
    pub fn read_f32_le(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32_le()?))
    }

    #[inline]
    pub fn read_f32_be(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32_be()?))
    }

    #[inline]
    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    #[inline]
    pub fn read_f64_le(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64_le()?))
    }

    #[inline]
    pub fn read_f64_be(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64_be()?))
    }

    //region Varints (width depends on `varint_long`)

    pub fn read_varint(&mut self) -> Result<i64> {
        if self.varint_long {
            self.read()
        } else {
            Ok(self.read::<i32>()? as i64)
        }
    }

    //region LEB128 / 7-bit ints

    pub fn read_leb128_u64(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;

        loop {
            let byte = self.read_u8()?;
            let payload = (byte & 0x7F) as u64;

            // The 10th byte (shift 63) only has room for a single bit.
            if shift == 63 && payload > 1 {
                return Err(invalid("Unsigned LEB128 value overflows u64"));
            }

            value |= payload << shift;

            if byte & 0x80 == 0 {
                return Ok(value);
            }

            shift += 7;

            if shift >= 64 {
                return Err(invalid("Unsigned LEB128 value is too long"));
            }
        }
    }

    pub fn read_leb128_i64(&mut self) -> Result<i64> {
        let mut value = 0i64;
        let mut shift = 0u32;

        loop {
            let byte = self.read_u8()?;

            value |= ((byte & 0x7F) as i64) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                // Sign extend when the final payload byte has bit 6 set.
                if shift < 64 && byte & 0x40 != 0 {
                    value |= -1i64 << shift;
                }

                return Ok(value);
            }

            if shift >= 64 {
                return Err(invalid("Signed LEB128 value is too long"));
            }
        }
    }

    /// .NET-style 7-bit encoded int; byte-for-byte identical to unsigned LEB128.
    #[inline]
    pub fn read_7bit_u64(&mut self) -> Result<u64> {
        self.read_leb128_u64()
    }

    //region Strings: decoding helpers

    fn decode_utf8(bytes: &[u8]) -> Result<String> {
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| invalid("Invalid UTF-8 string"))
    }

    fn decode_ascii(bytes: &[u8]) -> Result<String> {
        if !bytes.is_ascii() {
            return Err(invalid("String contains non-ASCII bytes"));
        }

        Self::decode_utf8(bytes)
    }

    fn decode_shift_jis(bytes: &[u8]) -> Result<String> {
        let (text, _, had_errors) = SHIFT_JIS.decode(bytes);

        if had_errors {
            return Err(invalid("String contains invalid Shift-JIS data"));
        }

        Ok(text.into_owned())
    }

    fn decode_utf16(units: &[u16]) -> Result<String> {
        String::from_utf16(units).map_err(|_| invalid("Invalid UTF-16 string"))
    }

    //region ASCII / UTF-8

    pub fn read_ascii(&mut self) -> Result<String> {
        Self::decode_ascii(self.read_cstring_slice()?)
    }

    pub fn read_ascii_fixed(&mut self, length: usize) -> Result<String> {
        Self::decode_ascii(self.read_fixed_slice(length)?)
    }

    pub fn read_utf8(&mut self) -> Result<String> {
        Self::decode_utf8(self.read_cstring_slice()?)
    }

    pub fn read_utf8_fixed(&mut self, length: usize) -> Result<String> {
        Self::decode_utf8(self.read_fixed_slice(length)?)
    }

    pub fn get_ascii(&mut self, offset: u64) -> Result<String> {
        self.step_in(offset)?;
        let out = self.read_ascii()?;
        self.step_out()?;
        Ok(out)
    }

    //region Shift-JIS

    pub fn read_shift_jis(&mut self) -> Result<String> {
        Self::decode_shift_jis(self.read_cstring_slice()?)
    }

    pub fn read_shift_jis_fixed(&mut self, length: usize) -> Result<String> {
        Self::decode_shift_jis(self.read_fixed_slice(length)?)
    }

    pub fn get_shift_jis(&mut self, offset: u64) -> Result<String> {
        self.step_in(offset)?;
        let out = self.read_shift_jis()?;
        self.step_out()?;
        Ok(out)
    }

    //region Length-prefixed strings

    /// Reads a length of type `L`, then that many bytes as UTF-8.
    pub fn read_string_prefixed<L>(&mut self) -> Result<String>
    where
        L: Readable + TryInto<usize>,
    {
        let length: usize = self
            .read::<L>()?
            .try_into()
            .map_err(|_| invalid("String length prefix doesn't fit in usize"))?;

        Self::decode_utf8(self.read_slice(length)?)
    }

    pub fn read_string_u8(&mut self) -> Result<String> {
        self.read_string_prefixed::<u8>()
    }

    pub fn read_string_u16(&mut self) -> Result<String> {
        self.read_string_prefixed::<u16>()
    }

    pub fn read_string_u32(&mut self) -> Result<String> {
        self.read_string_prefixed::<u32>()
    }

    //region UTF-16

    pub fn read_utf16(&mut self) -> Result<String> {
        let mut units = Vec::new();

        loop {
            match self.read_u16()? {
                0 => break,
                unit => units.push(unit),
            }
        }

        Self::decode_utf16(&units)
    }

    pub fn read_utf16_fixed(&mut self, units: usize) -> Result<String> {
        let encoded = self.read_vec::<u16>(units)?;
        let end = encoded.iter().position(|&u| u == 0).unwrap_or(encoded.len());

        Self::decode_utf16(&encoded[..end])
    }

    pub fn get_utf16(&mut self, offset: u64) -> Result<String> {
        self.step_in(offset)?;
        let out = self.read_utf16()?;
        self.step_out()?;
        Ok(out)
    }
}