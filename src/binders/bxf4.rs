//! Split header/data container used since Dark Souls 3 (2016)
//!
//! Header extensions: `.bhd`, `.*bhd` (magic `BHF4`)
//! Data extensions:   `.bdt`, `.*bdt` (magic `BDF4`)
//! 
//! Headers and names live in the BHD, entry data lives in the BDT.

use crate::binary::{IO, SplitIO};
use crate::binary::{BinaryReader, BinaryWriter};
use crate::binders::{Binder, BinderEntry};
use crate::dcx::{Compress, CompressSettings, Decompress};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use crate::games::Game;
use crate::oodle::OodleSettings;

use super::{BinderFlags, BinderVersion, hash_table::BinderHashTable};
use super::bnd4::{BND4Entry, BND4EntryHeader, BND4Header};


/// BDF4 header (0x30 bytes) at the start of the data file.
/// Mirrors fields of the BHF4 header, but they don't always match it, and nothing depends on them.
#[derive(Debug, Clone, Default)]
pub struct BDF4Header {
    pub _magic: [u8;4], // b"BDF4"
 
    pub unk04: bool,
    pub unk05: bool,
    pub _pad1: [u8;3], // b'\0'*3
 
    pub big_endian: bool,
    pub bit_big_endian: bool, // stored inverted
    pub _pad2: u8, // b'\0'
    pub _reserved1: u32, // b'\0'*4
 
    pub _header_size: u64, // asserted [0x30, 0x40], 0x40 is probably just a mistake
    pub signature: [u8;8], // ascii encoded
    pub _reserved2: u64, // b'\0'*8
    pub _reserved3: u64, // b'\0'*8
}
 
impl BDF4Header {
    /// Builds the BDF4 header that matches a BHF4 header.
    pub fn from_bhf(bhf: &BHF4Header) -> Self {
        Self {
            _magic: *b"BDF4",
            unk04: bhf.unk04,
            unk05: bhf.unk05,
            _pad1: [0u8;3],
            big_endian: bhf.big_endian,
            bit_big_endian: bhf.bit_big_endian,
            _pad2: 0,
            _reserved1: 0,
            _header_size: 0x30,
            signature: bhf.signature,
            _reserved2: 0,
            _reserved3: 0,
        }
    }
}
 
impl IO for BDF4Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BDF4")?;
 
        let unk04 = reader.read_boolean()?;
        let unk05 = reader.read_boolean()?;
        reader.assert_bytes(b"\0\0\0")?;
 
        let big_endian = reader.read_boolean()?;
        reader.big_endian = big_endian;
 
        let bit_big_endian = !reader.read_boolean()?; // inverse
        reader.assert(b'\0')?;
        reader.assert::<i32>(0i32)?;
 
        let header_size = reader.matches(|r| r.read_u64(), &[0x30, 0x40])?;
 
        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();
 
        reader.assert::<u64>(0u64)?;
        reader.assert::<u64>(0u64)?;
 
        Ok(Self {
            _magic: *b"BDF4",
            unk04,
            unk05,
            _pad1: [0u8;3],
            big_endian,
            bit_big_endian,
            _pad2: 0,
            _reserved1: 0,
            _header_size: header_size,
            signature,
            _reserved2: 0,
            _reserved3: 0,
        })
    }
 
    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian;
 
        writer.write_bytes(b"BDF4")?;
 
        writer.write_boolean(self.unk04)?;
        writer.write_boolean(self.unk05)?;
        writer.pad(3)?;
 
        writer.write_boolean(self.big_endian)?;
        writer.write_boolean(!self.bit_big_endian)?; // inverse
        writer.pad(1)?;
 
        writer.pad(4)?;
        writer.write_u64(0x30)?;
        writer.write_bytes(&self.signature)?;
        writer.write_u64(0)?;
        writer.write_u64(0)?;
 
        debug_assert_eq!(writer.position(), 0x30);
 
        Ok(())
    }
}


/// BHF4 header (0x40 bytes). Same layout as `BND4Header`, except that the
/// "headers end" field is always zero.
#[derive(Debug, Clone, Default)]
pub struct BHF4Header {
    _magic: [u8;4], // b"BHF4"

    pub unk04: bool,
    pub unk05: bool,
    _pad1: [u8;2], // b'\0'*2

    _pad2: u8, // b'\0'
    pub big_endian: bool,
    pub bit_big_endian: bool, // stored inverted
    _pad3: u8, // b'\0'

    pub entry_count: u32,
    _header_size: u64, // asserted 0x40
    pub signature: [u8;8], // ascii encoded
    pub entry_header_size: u64,
    _headers_end: u64, // asserted 0 (BND4 stores a real value here, BXF4 doesn't)

    pub unicode: bool,
    pub flags: BinderFlags,
    pub extended: u8, // asserted [0, 4]
    _pad4: u8, // b'\0'
    _reserved: u32, // b'\0'*4

    pub hash_table_offset: u64,  // only non-zero if `extended = 4`
}

impl IO for BHF4Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BHF4")?;

        let unk04 = reader.read_boolean()?;
        let unk05 = reader.read_boolean()?;

        reader.assert_bytes(b"\0\0")?;

        reader.assert(b'\0')?;
        let big_endian = reader.read_boolean()?;
        let bit_big_endian = !reader.read_boolean()?; // inverse
        reader.assert(b'\0')?;

        reader.big_endian = big_endian;

        let entry_count = reader.read_u32()?;

        reader.assert::<u64>(0x40u64)?; // header size

        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();

        let entry_header_size = reader.read_u64()?;
        reader.assert::<u64>(0u64)?; // headers end, unused in BXF4

        let unicode = reader.read_boolean()?;

        let flags = BinderFlags::from_byte(reader.read_u8()?, bit_big_endian);

        let extended = reader.matches(|r| r.read_u8(), &[0, 4])?;

        reader.assert(b'\0')?;
        reader.assert_bytes(b"\0\0\0\0")?;

        let hash_table_offset = reader.read_u64()?;

        Ok(Self {
            _magic: *b"BHF4",
            _pad1: *b"\0\0",
            _pad2: 0u8,
            _pad3: 0u8,
            _pad4: 0u8,
            _header_size: 0x40u64,
            _headers_end: 0u64,
            _reserved: 0,
            unk04,
            unk05,
            big_endian,
            bit_big_endian,
            entry_count,
            signature,
            entry_header_size,
            unicode,
            flags,
            extended,
            hash_table_offset,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian;

        writer.write_bytes(b"BHF4")?;

        writer.write_boolean(self.unk04)?;
        writer.write_boolean(self.unk05)?;
        writer.pad(2)?;

        writer.pad(1)?;
        writer.write_boolean(self.big_endian)?;
        writer.write_boolean(!self.bit_big_endian)?; // inverse
        writer.pad(1)?;

        writer.write_u32(self.entry_count)?;
        writer.write_u64(0x40)?;
        writer.write_bytes(&self.signature)?;
        writer.write_u64(self.flags.get_entry_header_size() as u64)?;
        writer.write_u64(0)?; // headers end, unused in BXF4

        writer.write_boolean(self.unicode)?;

        writer.write_u8(self.flags.to_byte(self.bit_big_endian))?;
        writer.write_u8(self.extended)?;
        writer.pad(1)?;

        writer.pad(4)?;

        writer.reserve::<u64>("HashTableOffset".to_string())?;

        debug_assert_eq!(writer.position(), 0x40);

        Ok(())
    }
}


///Entry headers are identical to BND4's, so `BXF4`'s use `BND4Entry` as  well.
///The difference is that entry *data* lives in the BDT, while headers and names live in the BHD.
#[derive(Debug, Clone)]
pub struct BXF4 {
    pub header: BHF4Header,
    pub entries: Vec<BND4Entry>,
}

impl BXF4 {
    /// Creates an empty `BXF4` using per-game defaults for `unk04`, `unk05`, `unicode` and `extended`.
    pub fn new(game: Game, flags: BinderFlags, signature: [u8;8]) -> Self {
        let (unk04, unk05, unicode, extended) = BND4Header::defaults(game);

        Self {
            header: BHF4Header {
                _magic: *b"BHF4",
                _pad1: *b"\0\0",
                _pad2: 0u8,
                _pad3: 0u8,
                _pad4: 0u8,
                _header_size: 0x40u64,
                _headers_end: 0u64,
                _reserved: 0,
                unk04,
                unk05,
                big_endian: false,
                bit_big_endian: false,
                entry_count: 0,
                signature,
                entry_header_size: flags.get_entry_header_size() as u64,
                unicode,
                flags,
                extended: extended as u8,
                hash_table_offset: 0,
            },
            entries: Vec::new(),
        }
    }

    /// Names come from the BHD, data comes from the BDT.
    pub fn read_entry(bhd: &mut BinaryReader, bdt: &mut BinaryReader, header: BND4EntryHeader, unicode: bool) -> Result<BND4Entry, BinaryReaderError> {
        // Names are referenced by absolute offsets from the start of the BHD.
        let name = match header.name_offset {
            Some(offset) => {
                bhd.step_in(offset as u64)?;
                let name = if unicode {
                    bhd.read_utf16()?
                } else {
                    bhd.read_shift_jis()?
                };
                bhd.step_out()?;
                Some(name)
            }
            None => None,
        };

        bdt.step_in(header.data_offset.as_u64())?;
        let stored_data = bdt.read_bytes(header.compressed_size as usize)?;
        bdt.step_out()?;

        let data = if header.flags.is_compressed() {
            let (data, _) = unsafe { Decompress::raw(&stored_data) }
                .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;

            let expected_size = header.uncompressed_size
                .ok_or_else(|| BinaryReaderError::custom("Compressed BXF4 entry has no uncompressed size"))?;

            if data.len() != expected_size as usize {
                return Err(BinaryReaderError::Custom(
                    format!("Expected entry size of {}, got {}", expected_size, data.len())
                ));
            }

            data
        } else {
            stored_data
        };

        Ok(BND4Entry { name, header, data })
    }

    pub fn write_entry(entry: &BND4Entry, bhd: &mut BinaryWriter, bdt: &mut BinaryWriter, flags: BinderFlags, index: usize) -> Result<(), BinaryWriterError> {
        if !entry.data.is_empty() {
            bdt.pad_align(0x10)?;
        }

        let data_offset = bdt.position();
        let uncompressed_size = entry.data.len() as i64;

        let stored_data = if entry.header.flags.is_compressed() {
            let compset = CompressSettings::Oodle(OodleSettings::KRAK);
            unsafe { Compress::raw(&entry.data, &compset) }
                .map_err(|e| BinaryWriterError::Custom(e.to_string()))?
        } else {
            entry.data.clone()
        };

        let compressed_size = stored_data.len() as i64;

        bhd.fill::<u64>(format!("FileCompressedSize{}", index), compressed_size as u64)?;

        if flags.has_compression() {
            bhd.fill::<u64>(format!("FileUncompressedSize{}", index), uncompressed_size as u64)?;
        }

        if flags.has_long_offsets() {
            bhd.fill::<u64>(format!("FileDataOffset{}", index), data_offset as u64)?;
        } else {
            bhd.fill::<u32>(format!("FileDataOffset{}", index), data_offset as u32)?;
        }

        bdt.write_bytes(&stored_data)?;

        Ok(())
    }
}

impl SplitIO for BXF4 {
    fn from_readers(bhd: &mut BinaryReader, bdt: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        BDF4Header::from_reader(bdt)?; // validate only, BHF is all that matters here
 
        let header = BHF4Header::from_reader(bhd)?;
        bhd.big_endian = header.big_endian;
 
        let expected_entry_header_size = header.flags.get_entry_header_size();
 
        if header.entry_header_size != expected_entry_header_size as u64 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BXF4 entry header size: expected 0x{:X}, got 0x{:X}",
                expected_entry_header_size,
                header.entry_header_size
            )));
        }
 
        if header.extended == 4 {
            bhd.step_in(header.hash_table_offset)?;
            BinderHashTable::from_reader(bhd)?; // not stored, just read to assert that it's correct
            bhd.step_out()?;
        } else if header.hash_table_offset != 0 {
            return Err(BinaryReaderError::Custom(
                format!("BXF4 has no hash table, but hash table offset is 0x{:X}", header.hash_table_offset)
            ));
        }
 
        let mut entries = Vec::with_capacity(header.entry_count as usize);
 
        for _ in 0..header.entry_count {
            let entry_header = BND4EntryHeader::from_reader(bhd, header.flags, header.bit_big_endian)?;
            entries.push(Self::read_entry(bhd, bdt, entry_header, header.unicode)?);
        }
 
        Ok(Self { header, entries })
    }
 
    fn to_writers(&mut self, bhd: &mut BinaryWriter, bdt: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        bhd.big_endian = self.header.big_endian;
        bdt.big_endian = self.header.big_endian;
 
        self.header.entry_count = self.entries.len() as u32; // modifying entries at all may make this inconsistent
 
        BDF4Header::from_bhf(&self.header).to_writer(bdt)?;
        self.header.to_writer(bhd)?;
 
        // Entry headers
        for (index, entry) in self.entries.iter().enumerate() {
            entry.header.to_writer(bhd, self.header.flags, self.header.bit_big_endian, index)?;
        }
 
        // Entry names
        if self.header.flags.has_names() {
            for (index, entry) in self.entries.iter().enumerate() {
                bhd.fill::<u32>(format!("FileNameOffset{}", index), bhd.position() as u32)?;
 
                if self.header.unicode {
                    bhd.write_utf16(entry.name.as_deref().unwrap_or(""), true)?;
                } else {
                    bhd.write_shift_jis(entry.name.as_deref().unwrap_or(""), true)?;
                }
            }
        }
 
        // Hash table
        if self.header.extended == 4 {
            bhd.pad_align(8)?;
            bhd.fill::<u64>("HashTableOffset".to_string(), bhd.position())?;
 
            let names: Vec<String> = self.entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    entry.name.clone().ok_or_else(|| {
                        BinaryWriterError::Custom(format!("BXF4 entry {} requires a name for the hash table.", index))
                    })
                })
                .collect::<Result<_, _>>()?;
 
            let mut hash_table = BinderHashTable::from_names(&names)?;
            hash_table.to_writer(bhd)?;
        } else {
            bhd.fill::<u64>("HashTableOffset".to_string(), 0)?;
        }
 
        // File data goes in the BDT, sizes/offsets are filled back into the BHD
        for (index, entry) in self.entries.iter().enumerate() {
            Self::write_entry(entry, bhd, bdt, self.header.flags, index)?;
        }
 
        Ok(())
    }
}

impl Binder for BXF4 {
    const VERSION: BinderVersion = BinderVersion::V1;
    type Entry = BND4Entry;

    fn entries(&self) -> &Vec<Self::Entry> { &self.entries }
    fn entries_mut(&mut self) -> &mut Vec<Self::Entry> { &mut self.entries }
}


#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_bxf4() {
        unsafe { crate::oodle::auto_oodle(); }

        unsafe {
        let bxf = BXF4::read(
            Path::new("tests/00_solo_l.tpfbhd"),
            Path::new("tests/00_solo_l.tpfbdt"),
        ).unwrap();
        println!("{:#?} -> {}", bxf.0.header, bxf.0.count())
        }
    }
}
