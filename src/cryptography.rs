//! Parent module for scary hidden secret information you aren't allowed to know

use aes::{cipher::{array::Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit}, Aes128};
use crate::{binary::{BinaryReader, BinaryWriter, IO}, errors::{BinaryReaderError, BinaryWriterError}};

pub mod regulation;
pub mod sl2;


/// Indicates a hashed or encrypted section of a file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// The beginning of the range, inclusive.
    pub start_offset: i64,
    /// The end of the range, exclusive.
    pub end_offset: i64,
}

impl IO for Range {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self { start_offset: reader.read_i64()?, end_offset: reader.read_i64()? })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i64(self.start_offset)?;
        writer.write_i64(self.end_offset)
    }
}

impl Range {
    pub fn new(start_offset: i64, end_offset: i64) -> Self { Self { start_offset, end_offset } }

    pub fn read_ranges(reader: &mut BinaryReader) -> Result<Vec<Range>, BinaryReaderError> {
        let count = reader.read_i32()?;
        let mut ranges = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count {
            ranges.push(Range::from_reader(reader)?);
        }
        Ok(ranges)
    }

    pub fn write_ranges(writer: &mut BinaryWriter, ranges: &mut [Range]) -> Result<(), BinaryWriterError> {
        writer.write_i32(ranges.len() as i32)?;
        for mut range in ranges {
            range.to_writer(writer)?;
        }
        Ok(())
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SHAHash {
    /// 32-byte salted SHA hash.
    pub hash: [u8; 32],
    /// Hashed sections of the file.
    pub ranges: Vec<Range>,
}

impl Default for SHAHash {
    fn default() -> Self {
        Self { hash: [0; 32], ranges: Vec::new() }
    }
}

impl IO for SHAHash {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&reader.read_bytes(32)?);
        Ok(Self { hash, ranges: Range::read_ranges(reader)? })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(&self.hash)?;
        Range::write_ranges(writer, &mut self.ranges)
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AESKey {
    /// 16-byte encryption key.
    pub key: [u8; 16],
    /// Encrypted sections of the file.
    pub ranges: Vec<Range>,
}

impl Default for AESKey {
    fn default() -> Self {
        Self { key: [0; 16], ranges: Vec::new() }
    }
}

impl IO for AESKey {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let mut key = [0u8; 16];
        key.copy_from_slice(&reader.read_bytes(16)?);
        Ok(Self { key, ranges: Range::read_ranges(reader)? })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(&self.key)?;
        Range::write_ranges(writer, &mut self.ranges)
    }
}

impl AESKey {
    ///Decrypts file data in place (AES-128 ECB, no padding), skipping empty and unset ranges.
    pub fn decrypt(&self, bytes: &mut [u8]) {
        let cipher = Aes128::new_from_slice(&self.key).unwrap();

        for range in self.ranges.iter().filter(|r| {
            r.start_offset != -1
                && r.end_offset != -1
                && r.start_offset != r.end_offset
        }) {
            let (start, end) = (
                range.start_offset as usize,
                range.end_offset as usize,
            );

            if end > bytes.len() || start > end {
                continue;
            }

            for block in bytes[start..end].chunks_exact_mut(16) {
                let block = Array::slice_as_mut_array(block).unwrap();
                cipher.decrypt_block(block);
            }
        }
    }

    pub fn encrypt(&self, bytes: &mut [u8]) {
        let cipher = Aes128::new_from_slice(&self.key).unwrap();

        for range in self.ranges.iter().filter(|r| {
            r.start_offset != -1
                && r.end_offset != -1
                && r.start_offset != r.end_offset
        }) {
            let (start, end) = (
                range.start_offset as usize,
                range.end_offset as usize,
            );

            if end > bytes.len() || start > end {
                continue;
            }

            for block in bytes[start..end].chunks_exact_mut(16) {
                let block = Array::slice_as_mut_array(block).unwrap();
                cipher.encrypt_block(block);
            }
        }
    }

}

