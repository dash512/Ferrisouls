use std::io::{Read, Seek, SeekFrom};

use crate::{binary::{BinaryReader, BinaryWriter, IO}, errors::{BinaryReaderError, BinaryWriterError}};
use crate::cryptography::{AESKey, SHAHash, Range};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BHD5Version {
    ///PC and Console version of DS1
    DS1,
    ///DS1 Remastered. Console versions; Virtuos Games variation
    DSR,
    ///OG/SOTFS for PC.
    ///Includes AES encryption and salting.
    DS2,
    ///DS3 and Sekiro, for PC.
    ///Includes size fields that identify how big the archive or its files are without padding.
    DS3,
    ///Elden Ring for PC.
    ///Improves the size of many fields by making them 64-bit, changes the unpadded size fields to be 32-bit, and resorts some fields.
    #[default]
    ER,
}


///Information about an individual file in the dvdbnd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    /// Hash of the full file path using From's algorithm.
    pub file_name_hash: u64,
    /// Full size of the file data in the BDT.
    pub padded_file_size: i32,
    /// File size after decryption; only included in DS3 and later. -1 if absent.
    pub unpadded_file_size: i64,
    /// Beginning of file data in the BDT.
    pub file_offset: i64,
    /// Hashing information for this file.
    pub sha_hash: Option<SHAHash>,
    /// Encryption information for this file.
    pub aes_key: Option<AESKey>,
}

impl Default for FileHeader {
    fn default() -> Self {
        Self {
            file_name_hash: 0,
            padded_file_size: 0,
            unpadded_file_size: -1,
            file_offset: 0,
            sha_hash: None,
            aes_key: None,
        }
    }
}

impl FileHeader {
    pub fn from_reader(reader: &mut BinaryReader, version: BHD5Version) -> Result<Self, BinaryReaderError> {
        let mut sha_hash_offset = 0u64;
        let mut aes_key_offset = 0u64;
        let mut unpadded_file_size = -1i64;

        let file_name_hash: u64;
        let padded_file_size: i32;
        let file_offset: i64;

        if version >= BHD5Version::ER {
            file_name_hash = reader.read_u64()?;
            padded_file_size = reader.read_i32()?;
            unpadded_file_size = reader.read_i32()? as i64;
            file_offset = reader.read_i64()?;
            sha_hash_offset = reader.read_i64()? as u64;
            aes_key_offset = reader.read_i64()? as u64;
        } else {
            file_name_hash = reader.read_u32()? as u64;
            padded_file_size = reader.read_i32()?;
            file_offset = reader.read_i64()?;

            if version >= BHD5Version::DS2 {
                sha_hash_offset = reader.read_i64()? as u64;
                aes_key_offset = reader.read_i64()? as u64;
            }
            if version >= BHD5Version::DS3 {
                unpadded_file_size = reader.read_i64()?;
            }
        }

        let mut sha_hash = None;
        if sha_hash_offset != 0 {
            reader.step_in(sha_hash_offset)?;
            sha_hash = Some(SHAHash::from_reader(reader)?);
            reader.step_out()?;
        }

        let mut aes_key = None;
        if aes_key_offset != 0 {
            reader.step_in(aes_key_offset)?;
            aes_key = Some(AESKey::from_reader(reader)?);
            reader.step_out()?;
        }

        Ok(Self { file_name_hash, padded_file_size, unpadded_file_size, file_offset, sha_hash, aes_key })
    }

    pub fn write(&self, writer: &mut BinaryWriter, version: BHD5Version, bucket: usize, file: usize) -> Result<(), BinaryWriterError> {
        let sha_name = format!("SHAHashOffset{bucket}:{file}");
        let aes_name = format!("AESKeyOffset{bucket}:{file}");

        if version >= BHD5Version::ER {
            writer.write_u64(self.file_name_hash)?;
            writer.write_i32(self.padded_file_size)?;
            writer.write_i32(self.unpadded_file_size as i32)?;
            writer.write_i64(self.file_offset)?;
            writer.reserve::<i64>(&sha_name)?;
            writer.reserve::<i64>(&aes_name)?;
        } else {
            writer.write_u32(self.file_name_hash as u32)?;
            writer.write_i32(self.padded_file_size)?;
            writer.write_i64(self.file_offset)?;

            if version >= BHD5Version::DS2 {
                writer.reserve::<i64>(&sha_name)?;
                writer.reserve::<i64>(&aes_name)?;
            }
            if version >= BHD5Version::DS3 {
                writer.write_i64(self.unpadded_file_size)?;
            }
        }
        Ok(())
    }

    pub fn write_hash_and_key(&mut self, writer: &mut BinaryWriter, version: BHD5Version, bucket: usize, file: usize) -> Result<(), BinaryWriterError> {
        if version < BHD5Version::DS2 {
            return Ok(());
        }
        let sha_name = format!("SHAHashOffset{bucket}:{file}");
        let aes_name = format!("AESKeyOffset{bucket}:{file}");

        match &mut self.sha_hash {
            None => writer.fill::<i64>(&sha_name, 0)?,
            Some(sha) => {
                writer.fill::<i64>(&sha_name, writer.position() as i64)?;
                sha.to_writer(writer)?;
            }
        }
        match &mut self.aes_key {
            None => writer.fill::<i64>(&aes_name, 0)?,
            Some(aes) => {
                writer.fill::<i64>(&aes_name, writer.position() as i64)?;
                aes.to_writer(writer)?;
            }
        }
        Ok(())
    }

    /// Reads and decrypts (if necessary) this file's data from the BDT.
    pub fn read_file<R: Read + Seek>(&self, bdt: &mut R) -> Result<Vec<u8>, BinaryReaderError> {
        let mut bytes = vec![0u8; self.padded_file_size as usize];
        bdt.seek(SeekFrom::Start(self.file_offset as u64))?;
        bdt.read_exact(&mut bytes)?;
        if let Some(aes) = &self.aes_key {
            aes.decrypt(&mut bytes);
        }
        Ok(bytes)
    }
}


/// A collection of files grouped by their hash.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub files: Vec<FileHeader>,
}

impl Bucket {
    pub fn from_reader(reader: &mut BinaryReader, version: BHD5Version, bit_64: bool) -> Result<Self, BinaryReaderError> {
        let file_header_count = reader.read_i32()?;
        if bit_64 {
            reader.assert::<i32>(1)?; // unknown flag
        }
        let file_headers_offset = if bit_64 { reader.read_i64()? as u64 } else { reader.read_i32()? as u64 };

        let mut files = Vec::with_capacity(file_header_count.max(0) as usize);
        reader.step_in(file_headers_offset)?;
        for _ in 0..file_header_count {
            files.push(FileHeader::from_reader(reader, version)?);
        }
        reader.step_out()?;

        Ok(Self { files })
    }

    pub fn write(&self, writer: &mut BinaryWriter, bit_64: bool, index: usize) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.files.len() as i32)?;
        if bit_64 {
            writer.write_i32(1)?;
            writer.reserve::<i64>(&format!("FileHeadersOffset{index}"))?;
        } else {
            writer.reserve::<i32>(&format!("FileHeadersOffset{index}"))?;
        }
        Ok(())
    }

    pub fn write_file_headers(&self, writer: &mut BinaryWriter, version: BHD5Version, bit_64: bool, index: usize) -> Result<(), BinaryWriterError> {
        if bit_64 {
            writer.fill::<i64>(&format!("FileHeadersOffset{index}"), writer.position() as i64)?;
        } else {
            writer.fill::<i32>(&format!("FileHeadersOffset{index}"), writer.position() as i32)?;
        }
        
        for (i, file) in self.files.iter().enumerate() {
            file.write(writer, version, index, i)?;
        }
        Ok(())
    }
}


#[derive(Debug, Default)]
pub struct BHD5 {
    pub version: BHD5Version,
    ///Used for file data SHA hashes.
    pub salt: String,

    pub big_endian: bool,
    ///Unknown; possibly whether crypto is allowed? Offsets are present regardless.
    pub unk05: bool,
    /// Whether the header uses 64-bit counts and offsets (Elden Ring layout). Detected on read.
    pub bit_64: bool,

    pub buckets: Vec<Bucket>,
}

impl BHD5 {
    pub fn new(version: BHD5Version) -> Self {
        Self { version, bit_64: version >= BHD5Version::ER, ..Default::default() }
    }

    pub fn supports_encryption(&self) -> bool { self.version >= BHD5Version::DS2 }
    pub fn supports_unpadded_size(&self) -> bool { self.version >= BHD5Version::DS3 }
    pub fn supports_long_fields(&self) -> bool { self.version >= BHD5Version::DS3 }

    ///Whether the reader starts with the BHD5 magic.
    pub fn is_header(reader: &mut BinaryReader) -> Result<bool, BinaryReaderError> {
        if reader.length() < 4 {
            return Ok(false);
        }
        Ok(reader.with_position(0, |r| r.read_ascii_fixed(4))? == "BHD5")
    }

    pub fn from_reader(reader: &mut BinaryReader, version: BHD5Version) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BHD5")?;

        // 0 = big endian, -1 = little endian
        let big_endian = reader.matches(|r| r.read_i8(), &[-1, 0])? == 0;
        reader.big_endian = big_endian;

        let unk05 = reader.read_boolean()?;

        reader.assert::<u8>(0)?;
        reader.assert::<u8>(0)?;
        reader.assert::<i32>(1)?;

        let _file_size = reader.read_u32()?;

        let mut bit_64 = false;
        if reader.length() > 0x28 {
            reader.step_in(0x14)?;
            let s1 = reader.read_i32()?;
            reader.read_i32()?;
            let s2 = reader.read_i32()?;
            reader.step_out()?;

            bit_64 = s1 == 0 && s2 == 0;
        }

        let bucket_count: u64 = if bit_64 { reader.read_u64()? } else { reader.read_u32()? as u64 };
        let buckets_offset: u64 = if bit_64 { reader.read_u64()? } else { reader.read_u32()? as u64 };

        let salt = if version >= BHD5Version::DS2 {
            let salt_length = reader.read_u32()? as usize;
            reader.read_ascii_fixed(salt_length)? // no padding
        } else {
            String::new()
        };

        reader.set_position(buckets_offset)?;
        let mut buckets = Vec::with_capacity(bucket_count as usize);
        for _ in 0..bucket_count {
            buckets.push(Bucket::from_reader(reader, version, bit_64)?);
        }

        Ok(Self { version, salt, big_endian, unk05, bit_64, buckets })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian;
        let version = self.version;
        let bit_64 = self.bit_64;

        writer.write_bytes(b"BHD5")?;
        writer.write_u8(if self.big_endian { 0 } else { 0xFF })?;
        writer.write_u8(self.unk05 as u8)?;
        writer.write_u8(0)?;
        writer.write_u8(0)?;
        writer.write_i32(1)?;
        writer.reserve::<i32>("FileSize")?;

        if bit_64 {
            writer.write_u64(self.buckets.len() as u64)?;
            writer.reserve::<i64>("BucketsOffset")?;
        } else {
            writer.write_u32(self.buckets.len() as u32)?;
            writer.reserve::<i32>("BucketsOffset")?;
        }

        if version >= BHD5Version::DS2 {
            writer.write_u32(self.salt.len() as u32)?;
            writer.write_bytes(self.salt.as_bytes())?;
        }

        if bit_64 {
            writer.fill::<i64>("BucketsOffset", writer.position() as i64)?;
        } else {
            writer.fill::<i32>("BucketsOffset", writer.position() as i32)?;
        }

        for (i, bucket) in self.buckets.iter().enumerate() {
            bucket.write(writer, bit_64, i)?;
        }

        for (i, bucket) in self.buckets.iter().enumerate() {
            bucket.write_file_headers(writer, version, bit_64, i)?;
        }

        for (i, bucket) in self.buckets.iter_mut().enumerate() {
            for (j, file) in bucket.files.iter_mut().enumerate() {
                file.write_hash_and_key(writer, version, i, j)?;
            }
        }

        writer.fill::<i32>("FileSize", writer.position() as i32)
    }

    pub fn to_bytes(&mut self) -> Result<Vec<u8>, BinaryWriterError> {
        let mut writer = BinaryWriter::default();
        self.to_writer(&mut writer)?;
        Ok(writer.into_inner())
    }
}