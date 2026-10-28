//! General purpose split header/data container used before Dark Souls 3
//!
//! Header extensions: `.bhd`, `.*bhd` (magic `BHF3`)
//! Data extensions:   `.bdt`, `.*bdt` (magic `BDF3`)
//!
//! Headers and names live in the BHD, entry data lives in the BDT.

use crate::binary::{IO, SplitIO};
use crate::binary::{BinaryReader, BinaryWriter};
use crate::binders::{Binder, BinderEntry};
use crate::dcx::{Compress, CompressSettings, DCXType, Decompress};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use crate::oodle::OodleSettings;
use super::{BinderFlags, BinderVersion, EntryFlags};


///BDF3 header (0x10 bytes) at the start of the data file.
#[derive(Debug, Clone, Default)]
pub struct BDF3Header {
    _magic: [u8;4], // b"BDF3"
    pub signature: [u8;8], // ascii encoded
    _reserved: u32 // b'\0'*4
}
 
impl BDF3Header {
    /// Builds the BDF3 header that matches a BHF3 header.
    pub fn from_bhf(bhf: &BHF3Header) -> Self {
        Self {
            _magic: *b"BDF3",
            signature: bhf.signature,
            _reserved: 0,
        }
    }
}
 
impl IO for BDF3Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BDF3")?;
 
        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();
 
        reader.assert::<i32>(0i32)?;
 
        Ok(Self { 
            _magic: *b"BDF3", 
            signature, 
            _reserved: 0 
        })
    }
 
    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(b"BDF3")?;
        writer.write_bytes(&self.signature)?;
        writer.pad(4)?;
 
        debug_assert_eq!(writer.position(), 0x10);
 
        Ok(())
    }
}


///BHF3 header (0x18 bytes) at the start of the header file.
#[derive(Debug, Clone, Default)]
pub struct BHF3Header {
    _magic: [u8;4], // asserted b"BHF3"
    pub signature: [u8;8], // ascii encoded timestamp / version

    pub big_endian: bool,
    pub bit_big_endian: bool,

    pub flags: BinderFlags,
    pub entry_count: u32,
}

impl IO for BHF3Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BHF3")?;

        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();

        // `bit_big_endian` is needed to decode the flags byte, but sits after it (0xE).
        reader.step_in(0xE)?;
        let bit_big_endian = reader.read_boolean()?;
        reader.step_out()?;

        let flags = BinderFlags::from_byte(reader.read_u8()?, bit_big_endian);
        let big_endian = reader.read_boolean()?;

        let bit_big_endian_check = reader.read_boolean()?;
        if bit_big_endian_check != bit_big_endian {
            return Err(BinaryReaderError::custom("BHF3 bit big-endian flag read inconsistently"));
        }
        reader.assert(b'\0')?;

        reader.big_endian = big_endian || flags.force_big_endian();

        let entry_count = reader.read_u32()?;

        reader.assert::<i32>(0i32)?;
        reader.assert::<i32>(0i32)?;
        reader.assert::<i32>(0i32)?;

        Ok(
            Self {
                _magic: *b"BHF3",
                signature, 
                big_endian, 
                bit_big_endian, 
                flags, 
                entry_count 
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian || self.flags.force_big_endian();

        writer.write_bytes(b"BHF3")?;
        writer.write_bytes(&self.signature)?;

        writer.write_u8(self.flags.to_byte(self.bit_big_endian))?;
        writer.write_boolean(self.big_endian)?;
        writer.write_boolean(self.bit_big_endian)?;
        writer.pad(1)?;

        writer.write_u32(self.entry_count)?;
        writer.pad(12)?;

        debug_assert_eq!(writer.position(), 0x20);

        Ok(())
    }
}


#[derive(Debug, Clone, Copy, Default)]
pub struct BXF3EntryHeader {
    pub flags: EntryFlags,

    _pad: [u8;3], // b'\0'*3

    pub compressed_size: i32,
    pub data_offset: u32,

    pub entry_id: Option<i32>,
    pub name_offset: Option<u32>,
    pub uncompressed_size: Option<i32>,
}

impl BXF3EntryHeader {
    pub fn from_reader(reader: &mut BinaryReader, format: BinderFlags, bit_big_endian: bool) -> Result<Self, BinaryReaderError> {
        let flags = EntryFlags::from_byte(reader.read_u8()?, bit_big_endian);

        reader.assert_bytes(b"\0\0\0")?; // _pad

        let compressed_size = reader.read_i32()?;
        let data_offset = reader.read_u32()?;

        let entry_id = if format.has_ids() {
            Some(reader.read_i32()?)
        } else {
            None
        };

        let name_offset = if format.has_names() {
            Some(reader.read_u32()?)
        } else {
            None
        };

        let uncompressed_size = if format.has_compression() {
            Some(reader.read_i32()?)
        } else {
            None
        };

        Ok(Self {
            flags,
            _pad: [0u8;3],
            compressed_size,
            data_offset,
            entry_id,
            name_offset,
            uncompressed_size,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, format: BinderFlags, bit_big_endian: bool, index: usize) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.flags.to_byte(bit_big_endian))?;
        writer.pad(3)?;

        writer.reserve::<u32>(format!("FileCompressedSize{}", index))?;
        writer.reserve::<u32>(format!("FileDataOffset{}", index))?;

        if format.has_ids() {
            writer.write_i32(self.entry_id.ok_or_else(||
                BinaryWriterError::Custom(format!("BXF3 entry {} requires an ID.", index))
            )?)?;
        }

        if format.has_names() {
            writer.reserve::<u32>(format!("FileNameOffset{}", index))?;
        }

        if format.has_compression() {
            writer.reserve::<u32>(format!("FileUncompressedSize{}", index))?;
        }

        Ok(())
    }
}


#[derive(Debug, Clone)]
pub struct BXF3Entry {
    pub name: Option<String>, // not serialized. For convenience.
    pub header: BXF3EntryHeader,
    pub data: Vec<u8>,
}

impl BinderEntry for BXF3Entry {
    type Identifier = Option<i32>;

    fn identity(&self) -> &Self::Identifier {
        &self.header.entry_id
    }
}


#[derive(Debug, Clone)]
pub struct BXF3 {
    pub header: BHF3Header,
    pub entries: Vec<BXF3Entry>,
}

impl BXF3 {
    /// Creates an empty `BXF3`.
    pub fn new(flags: BinderFlags, signature: [u8;8]) -> Self {
        Self {
            header: BHF3Header {
                _magic: *b"BHF3",
                signature,
                big_endian: false,
                bit_big_endian: false,
                flags,
                entry_count: 0,
            },
            entries: Vec::new(),
        }
    }

    pub fn read_entry(bhd: &mut BinaryReader, bdt: &mut BinaryReader, header: BXF3EntryHeader) -> Result<BXF3Entry, BinaryReaderError> {
        //names are referenced by absolute offsets from the start of the BHD
        let name = match header.name_offset {
            Some(offset) => {
                bhd.step_in(offset as u64)?;
                let name = bhd.read_shift_jis()?;
                bhd.step_out()?;
                Some(name)
            }
            None => None,
        };

        bdt.step_in(header.data_offset as u64)?;
        let stored_data = bdt.read_bytes(header.compressed_size as usize)?;
        bdt.step_out()?;

        let data = if header.flags.is_compressed() {
            let (data, _) = unsafe { Decompress::raw(&stored_data) }
                .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;

            let expected_size = header.uncompressed_size
                .ok_or_else(|| BinaryReaderError::custom("Compressed BXF3 entry has no uncompressed size"))?;

            if data.len() != expected_size as usize {
                return Err(BinaryReaderError::Custom(format!("Expected entry size of {}, got {}", expected_size, data.len())));
            }

            data
        } else {
            stored_data
        };

        Ok(BXF3Entry { 
            name, 
            header, 
            data 
        })
    }

    pub fn write_entry(entry: &BXF3Entry, bhd: &mut BinaryWriter, bdt: &mut BinaryWriter, flags: BinderFlags, index: usize) -> Result<(), BinaryWriterError> {
        if !entry.data.is_empty() {
            bdt.pad_align(0x10)?;
        }

        let data_offset = bdt.position();
        let uncompressed_size = entry.data.len() as u32;

        let stored_data = if entry.header.flags.is_compressed() {
            let compset = CompressSettings::DFLT(DCXType::DCX_DFLT_10000_24_9, 7u32);
            unsafe { Compress::raw(&entry.data, &compset) }
                .map_err(|e| BinaryWriterError::Custom(e.to_string()))?
        } else {
            entry.data.clone()
        };

        bhd.fill::<u32>(format!("FileCompressedSize{}", index), stored_data.len() as u32)?;
        bhd.fill::<u32>(format!("FileDataOffset{}", index), data_offset as u32)?;

        if flags.has_compression() {
            bhd.fill::<u32>(format!("FileUncompressedSize{}", index), uncompressed_size)?;
        }

        bdt.write_bytes(&stored_data)?;

        Ok(())
    }
}

impl SplitIO for BXF3 {
    fn from_readers(bhd: &mut BinaryReader, bdt: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        BDF3Header::from_reader(bdt)?;
        let header = BHF3Header::from_reader(bhd)?;

        let mut entries = Vec::with_capacity(header.entry_count as usize);

        for _ in 0..header.entry_count {
            let entry_header = BXF3EntryHeader::from_reader(bhd, header.flags, header.bit_big_endian)?;
            entries.push(Self::read_entry(bhd, bdt, entry_header)?);
        }

        Ok(Self { header, entries })
    }

    fn into_writers(&mut self) -> Result<(BinaryWriter, BinaryWriter), BinaryWriterError> {
        let mut bhd = BinaryWriter::default();
        let mut bdt = BinaryWriter::default();

        let big_endian = self.header.big_endian || self.header.flags.force_big_endian();
        bhd.big_endian = big_endian;
        bdt.big_endian = big_endian;

        self.header.entry_count = self.entries.len() as u32; // modifying entries at all may make this inconsistent

        BDF3Header::from_bhf(&self.header).to_writer(&mut bdt)?;
        self.header.to_writer(&mut bhd)?;

        //write entry headers first
        for (index, entry) in self.entries.iter().enumerate() {
            entry.header.to_writer(&mut bhd, self.header.flags, self.header.bit_big_endian, index)?;
        }

        //entry names (always Shift-JIS)
        if self.header.flags.has_names() {
            for (index, entry) in self.entries.iter().enumerate() {
                bhd.fill::<u32>(format!("FileNameOffset{}", index), bhd.position() as u32)?;
                bhd.write_shift_jis(entry.name.as_deref().unwrap_or(""), true)?;
            }
        }

        //file data goes in the BDT, sizes/offsets are filled back into the BHD
        for (index, entry) in self.entries.iter().enumerate() {
            Self::write_entry(entry, &mut bhd, &mut bdt, self.header.flags, index)?;
        }

        Ok((bhd, bdt))
    }
}

impl Binder for BXF3 {
    const VERSION: BinderVersion = BinderVersion::V3;
    type Entry = BXF3Entry;

    fn entries(&self) -> &Vec<Self::Entry> { &self.entries }
    fn entries_mut(&mut self) -> &mut Vec<Self::Entry> { &mut self.entries }
}


#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_bxf3() {
        unsafe { crate::oodle::auto_oodle(); }

        unsafe {
        let bxf = BXF3::unpack(
            Path::new("tests/GI_EnvM_m11.tpfbhd"),
            Path::new("tests/GI_EnvM_m11.tpfbdt"),
        ).unwrap();
        println!("{:#?} -> {}", bxf.0.header, bxf.0.count())
        }
    }
}
