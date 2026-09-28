/// Logic adapted from SoulsFormatsNext and Constrata

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use encoding_rs::SHIFT_JIS;

use crate::errors::{BinaryReaderError, FerrisoulsError};

pub type Result<T> = std::result::Result<T, BinaryReaderError>;

pub struct BinaryReader {
    data: Cursor<Vec<u8>>,
    pub big_endian: bool,
    pub varint_long: bool,

    steps: Vec<u64>,
}

impl BinaryReader {
    //region Creation

    pub fn new(data: Vec<u8>, be: bool, long: bool) -> Self {
        Self {
            data: Cursor::new(data),
            big_endian: be,
            varint_long: long,
            steps: Vec::new(),
        }
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        Self::new(
            data.to_vec(),
            true,
            false
        )
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

    #[inline]
    pub fn is_eof(&self) -> bool {
        self.is_at_end()
    }

    pub fn set_position(&mut self, position: u64) -> Result<()> {
        if position > self.length() {
            return Err(BinaryReaderError::OutOfBounds {
                offset: position,
                length: 0,
                total: self.length(),
            });
        }

        self.data
            .seek(SeekFrom::Start(position))
            .map_err(|source| BinaryReaderError::Io {
                position: self.position(),
                source,
            })?;

        Ok(())
    }

    pub fn skip(&mut self, count: u64) -> Result<()> {
        let position = self.position();

        let new_position = position.checked_add(count).ok_or_else(|| {
            BinaryReaderError::InvalidData(
                "Reader position overflow".into(),
            )
        })?;

        self.set_position(new_position)
    }

    //region Generic types

    pub fn read<T: Readable>(&mut self) -> Result<T> {
        T::read_from(self)
    }

    pub fn assert<T>(&mut self, value: T) -> Result<T>
    where T: Readable + PartialEq + std::fmt::Debug {
        let lhs = T::read_from(self)?;

        if lhs != value {
            return Err(BinaryReaderError::Custom(
                format!("Asserted `{:?}` was incorrect!", value),
            ));
        }

        Ok(lhs)
    }


    pub fn read_vec<T: Readable>(&mut self, count: usize) -> Result<Vec<T>> {
        (0..count)
            .map(|_| self.read())
            .collect()
    }

    //region Input

    pub fn as_slice(&self) -> &[u8] {
        self.data.get_ref().as_slice()
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        let position = self.position();

        if self.remaining() < count as u64 {
            return Err(BinaryReaderError::UnexpectedEof {
                position,
                requested: count,
                remaining: self.remaining() as usize,
            });
        }

        let mut bytes = vec![0u8; count];

        self.data
            .read_exact(&mut bytes)
            .map_err(|source| BinaryReaderError::Io {
                position,
                source,
            })?;

        Ok(bytes)
    }

    pub fn read_bytes_into(&mut self, bytes: &mut [u8]) -> Result<()> {
        let position = self.position();

        if self.remaining() < bytes.len() as u64 {
            return Err(BinaryReaderError::UnexpectedEof {
                position,
                requested: bytes.len(),
                remaining: self.remaining() as usize,
            });
        }

        self.data
            .read_exact(bytes)
            .map_err(|source| BinaryReaderError::Io {
                position,
                source,
            })?;

        Ok(())
    }

    pub fn assert_bytes(&mut self, value: &[u8]) -> Result<Vec<u8>> {
        let lhs = Self::read_bytes(self, value.len())?;

        if lhs != value {
            return Err(
                BinaryReaderError::Custom(
                        format!("Asserted magic `{:?}` was incorrect!", value)
                    )
                );
        }
        Ok(lhs)
    }

    //region Stepping

    pub fn step_in(&mut self, offset: u64) -> Result<()> {
        let old_position = self.position();

        self.set_position(offset)?;
        self.steps.push(old_position);

        Ok(())
    }

    pub fn step_out(&mut self) -> Result<()> {
        let position = self.steps.pop().ok_or_else(|| {
            BinaryReaderError::InvalidData(
                "Reader is already stepped all the way out.".into(),
            )
        })?;

        self.set_position(position)
    }


    //region Alignment

    pub fn align(&mut self, alignment: u64) -> Result<()> {
        if alignment == 0 {
            return Ok(());
        }

        let remainder = self.position() % alignment;

        if remainder != 0 {
            self.skip(alignment - remainder)?;
        }

        Ok(())
    }

    pub fn align_relative(
        &mut self,
        start: u64,
        alignment: u64,
    ) -> Result<()> {
        if alignment == 0 {
            return Ok(());
        }

        let relative = self.position().saturating_sub(start);
        let remainder = relative % alignment;

        if remainder != 0 {
            self.skip(alignment - remainder)?;
        }

        Ok(())
    }

    //region Integers

    #[inline]
    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_bytes(1).map(|bytes| bytes[0])
    }

    #[inline]
    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    #[inline]
    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;

        Ok(if self.big_endian {
            BigEndian::read_u16(&bytes)
        } else {
            LittleEndian::read_u16(&bytes)
        })
    }

    #[inline]
    pub fn read_i16(&mut self) -> Result<i16> {
        let bytes = self.read_bytes(2)?;

        Ok(if self.big_endian {
            BigEndian::read_i16(&bytes)
        } else {
            LittleEndian::read_i16(&bytes)
        })
    }

    #[inline]
    pub fn read_u24(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(3)?;

        Ok(if self.big_endian {
            BigEndian::read_u24(&bytes)
        } else {
            LittleEndian::read_u24(&bytes)
        })
    }

    #[inline]
    pub fn read_i24(&mut self) -> Result<i32> {
        let value = self.read_u24()?;

        Ok(if value & 0x80_0000 != 0 {
            (value | 0xFF00_0000) as i32
        } else {
            value as i32
        })
    }

    #[inline]
    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;

        Ok(if self.big_endian {
            BigEndian::read_u32(&bytes)
        } else {
            LittleEndian::read_u32(&bytes)
        })
    }

    #[inline]
    pub fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_bytes(4)?;

        Ok(if self.big_endian {
            BigEndian::read_i32(&bytes)
        } else {
            LittleEndian::read_i32(&bytes)
        })
    }

    #[inline]
    pub fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_bytes(8)?;

        Ok(if self.big_endian {
            BigEndian::read_u64(&bytes)
        } else {
            LittleEndian::read_u64(&bytes)
        })
    }

    #[inline]
    pub fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.read_bytes(8)?;

        Ok(if self.big_endian {
            BigEndian::read_i64(&bytes)
        } else {
            LittleEndian::read_i64(&bytes)
        })
    }

    #[inline]
    pub fn read_u128(&mut self) -> Result<u128> {
        let bytes = self.read_bytes(16)?;
        let bytes: [u8; 16] = bytes.try_into().map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid u128 byte count".into(),
            )
        })?;

        Ok(if self.big_endian {
            u128::from_be_bytes(bytes)
        } else {
            u128::from_le_bytes(bytes)
        })
    }

    #[inline]
    pub fn read_i128(&mut self) -> Result<i128> {
        Ok(self.read_u128()? as i128)
    }

    //region Floats

    #[inline]
    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    #[inline]
    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    //region Bool

    pub fn read_boolean(&mut self) -> Result<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(BinaryReaderError::InvalidData(format!(
                "Invalid boolean: 0x{value:02X}"
            ))),
        }
    }

    //region Fixed-width / Varint

    pub fn read_varint(&mut self) -> Result<i64> {
        if self.varint_long {
            self.read_i64()
        } else {
            Ok(self.read_i32()? as i64)
        }
    }

    //region LEB128 / 7-bit ints

    pub fn read_leb128_u64(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;

        loop {
            let byte = self.read_u8()?;

            if shift >= 64 && (byte & 0x7F) != 0 {
                return Err(BinaryReaderError::InvalidData(
                    "Unsigned LEB128 value overflows u64".into(),
                ));
            }

            value |= ((byte & 0x7F) as u64)
                .checked_shl(shift)
                .ok_or_else(|| {
                    BinaryReaderError::InvalidData(
                        "Unsigned LEB128 shift overflow".into(),
                    )
                })?;

            if byte & 0x80 == 0 {
                return Ok(value);
            }

            shift += 7;

            if shift >= 64 {
                return Err(BinaryReaderError::InvalidData(
                    "Unsigned LEB128 value is too long".into(),
                ));
            }
        }
    }

    pub fn read_leb128_i64(&mut self) -> Result<i64> {
        let mut value = 0i64;
        let mut shift = 0u32;

        loop {
            let byte = self.read_u8()?;
            let payload = (byte & 0x7F) as i64;

            if shift >= 64 {
                return Err(BinaryReaderError::InvalidData(
                    "Signed LEB128 value is too long".into(),
                ));
            }

            value |= payload.checked_shl(shift).ok_or_else(|| {
                BinaryReaderError::InvalidData(
                    "Signed LEB128 shift overflow".into(),
                )
            })?;

            let continuation = byte & 0x80 != 0;

            if !continuation {
                // Sign extend when the final payload byte has bit 6 set.
                if shift < 63 && byte & 0x40 != 0 {
                    value |= (!0i64) << (shift + 7);
                }

                return Ok(value);
            }

            shift += 7;

            if shift >= 64 {
                return Err(BinaryReaderError::InvalidData(
                    "Signed LEB128 value is too long".into(),
                ));
            }
        }
    }

    pub fn read_7bit_u64(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;

        loop {
            let byte = self.read_u8()?;

            if shift >= 64 {
                return Err(BinaryReaderError::InvalidData(
                    "7-bit encoded value is too long".into(),
                ));
            }

            value |= ((byte & 0x7F) as u64)
                .checked_shl(shift)
                .ok_or_else(|| {
                    BinaryReaderError::InvalidData(
                        "7-bit value shift overflow".into(),
                    )
                })?;

            if byte & 0x80 == 0 {
                return Ok(value);
            }

            shift += 7;
        }
    }

    //region ASCII / UTF-8

    pub fn read_ascii(&mut self) -> Result<String> {
        let bytes = self.read_cstring_bytes()?;

        if !bytes.is_ascii() {
            return Err(BinaryReaderError::InvalidData(
                "String contains non-ASCII bytes".into(),
            ));
        }

        Ok(String::from_utf8(bytes).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid ASCII string".into(),
            )
        })?)
    }

    pub fn read_ascii_fixed(
        &mut self,
        length: usize,
    ) -> Result<String> {
        let bytes = self.read_bytes(length)?;

        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());

        let bytes = &bytes[..end];

        if !bytes.is_ascii() {
            return Err(BinaryReaderError::InvalidData(
                "String contains non-ASCII bytes".into(),
            ));
        }

        Ok(String::from_utf8(bytes.to_vec()).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid ASCII string".into(),
            )
        })?)
    }

    pub fn read_utf8(&mut self) -> Result<String> {
        let bytes = self.read_cstring_bytes()?;

        String::from_utf8(bytes).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid UTF-8 string".into(),
            )
        })
    }

    pub fn read_utf8_fixed(
        &mut self,
        length: usize,
    ) -> Result<String> {
        let bytes = self.read_bytes(length)?;

        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());

        String::from_utf8(bytes[..end].to_vec()).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid UTF-8 string".into(),
            )
        })
    }

    //region Shift-JIS

    pub fn read_shift_jis(&mut self) -> Result<String> {
        let bytes = self.read_cstring_bytes()?;

        let (text, _, had_errors) = SHIFT_JIS.decode(&bytes);

        if had_errors {
            return Err(BinaryReaderError::InvalidData(
                "String contains invalid Shift-JIS data".into(),
            ));
        }

        Ok(text.into_owned())
    }

    pub fn read_shift_jis_fixed(
        &mut self,
        length: usize,
    ) -> Result<String> {
        let bytes = self.read_bytes(length)?;

        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());

        let (text, _, had_errors) = SHIFT_JIS.decode(&bytes[..end]);

        if had_errors {
            return Err(BinaryReaderError::InvalidData(
                "String contains invalid Shift-JIS data".into(),
            ));
        }

        Ok(text.into_owned())
    }

    //region Length-prefixed strings

    pub fn read_string_u8(&mut self) -> Result<String> {
        let length = self.read_u8()? as usize;
        self.read_utf8_bytes(length)
    }

    pub fn read_string_u16(&mut self) -> Result<String> {
        let length = self.read_u16()? as usize;
        self.read_utf8_bytes(length)
    }

    pub fn read_string_u32(&mut self) -> Result<String> {
        let length = usize::try_from(self.read_u32()?)?;
        self.read_utf8_bytes(length)
    }

    fn read_utf8_bytes(&mut self, length: usize) -> Result<String> {
        let bytes = self.read_bytes(length)?;

        String::from_utf8(bytes).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid UTF-8 string".into(),
            )
        })
    }

    //region UTF-16

    pub fn read_utf16(&mut self) -> Result<String> {
        let mut units = Vec::new();

        loop {
            let unit = self.read_u16()?;

            if unit == 0 {
                break;
            }

            units.push(unit);
        }

        String::from_utf16(&units).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid UTF-16 string".into(),
            )
        })
    }

    pub fn read_utf16_fixed(
        &mut self,
        units: usize,
    ) -> Result<String> {
        let mut encoded = Vec::with_capacity(units);

        for _ in 0..units {
            encoded.push(self.read_u16()?);
        }

        let end = encoded
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(encoded.len());

        String::from_utf16(&encoded[..end]).map_err(|_| {
            BinaryReaderError::InvalidData(
                "Invalid UTF-16 string".into(),
            )
        })
    }

    //region C strings

    fn read_cstring_bytes(&mut self) -> Result<Vec<u8>> {
        let start = self.position();

        while !self.is_at_end() {
            if self.read_u8()? == 0 {
                let end = self.position() - 1;

                return Ok(self.data.get_ref()
                    [start as usize..end as usize]
                    .to_vec());
            }
        }

        Err(BinaryReaderError::UnexpectedEof {
            position: start,
            requested: 1,
            remaining: 0,
        })
    }

    //region Peek

    pub fn peek_u8(&mut self) -> Result<u8> {
        let position = self.position();
        let value = self.read_u8()?;
        self.set_position(position)?;
        Ok(value)
    }

    pub fn peek_u16(&mut self) -> Result<u16> {
        let position = self.position();
        let value = self.read_u16()?;
        self.set_position(position)?;
        Ok(value)
    }

    pub fn peek_u32(&mut self) -> Result<u32> {
        let position = self.position();
        let value = self.read_u32()?;
        self.set_position(position)?;
        Ok(value)
    }

    pub fn peek_u64(&mut self) -> Result<u64> {
        let position = self.position();
        let value = self.read_u64()?;
        self.set_position(position)?;
        Ok(value)
    }

    //region Read at offset

    pub fn read_at(
        &mut self,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>> {
        let old_position = self.position();

        self.set_position(offset)?;

        let result = self.read_bytes(length);

        self.set_position(old_position)?;

        result
    }

    pub fn read_u8_at(&mut self, offset: u64) -> Result<u8> {
        let old_position = self.position();

        self.set_position(offset)?;
        let result = self.read_u8();
        self.set_position(old_position)?;

        result
    }

    pub fn read_u32_at(&mut self, offset: u64) -> Result<u32> {
        let old_position = self.position();

        self.set_position(offset)?;
        let result = self.read_u32();
        self.set_position(old_position)?;

        result
    }

    pub fn read_u64_at(&mut self, offset: u64) -> Result<u64> {
        let old_position = self.position();

        self.set_position(offset)?;
        let result = self.read_u64();
        self.set_position(old_position)?;

        result
    }
}

impl Default for BinaryReader {
    fn default() -> Self {
        Self::new(
            Vec::new(),
            true,
            false
        )
    }
}



pub trait Readable: Sized {
    fn read_from(reader: &mut BinaryReader) -> Result<Self>;
}

macro_rules! impl_readable {
    ($($ty:ty => $method:ident),* $(,)?) => {
        $(
            impl Readable for $ty {
                #[inline]
                fn read_from(reader: &mut BinaryReader) -> Result<Self> {
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

