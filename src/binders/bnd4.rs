use crate::binary::IO;
use crate::binders::{Binder, BinderEntry};
use crate::dcx::{Compress, CompressSettings, DCXType, Decompress};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use crate::games::Game;
use crate::binary::{BinaryReader, BinaryWriter, bytes::VariableUInt};
use crate::oodle::structs::OodleSettings;
use super::{BinderFlags, BinderVersion, EntryFlags, FerrisoulsError, hash_table::BinderHashTable};


#[derive(Debug, Clone)]
pub struct BND4Header {
    _magic: [u8;4], // asserted b"BND4"

    unk04: bool, // asserted 0
    unk05: bool, // asserted 0
    _pad1: u16, // b'\0'*2

    _pad2: u8, // b'\0'
    pub big_endian: bool,
    pub bit_big_endian: bool,
    _pad3: u8, // b'\0'

    pub entry_count: u32,
    // NOTE: No `file_size` in V4.
    _header_size: u64, // asserted 0x40
    pub signature: [u8;8], // ascii encoded
    pub entry_header_size: u64,
    pub headers_end: u64,
    
    pub unicode: bool,
    pub flags: BinderFlags,
    pub extended: u8, // asserted [0, 1, 4, 128]
    _pad4: u8, // b'\0'
    reserved: u32, // b'\0'*4

    pub hash_table_offset: u64 // only non-zero if `extended = 4` - Grimrukh
}

impl IO for BND4Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BND4")?;

        let unk04 = reader.read_boolean()?;
        let unk05 = reader.read_boolean()?;

        reader.assert_bytes(b"\0\0")?;

        reader.assert(b'\0')?;
        let big_endian = reader.read_boolean()?;
        let bit_big_endian = !reader.read_boolean()?; // inverse
        reader.assert(b'\0')?;

        reader.big_endian = big_endian;

        let entry_count = reader.read_u32()?;

        let header_size = reader.assert::<u64>(0x40u64)?;

        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();

        let entry_header_size = reader.read_u64()?;
        let headers_end = reader.read_u64()?;

        let unicode = reader.read_boolean()?;

        let flags = BinderFlags::from_byte(reader.read_u8()?, bit_big_endian);

        let extended = reader.read_u8()?;
        if !matches!(extended, 0 | 1 | 4 | 0x80) {
            return Err(BinaryReaderError::Custom(format!("Invalid BND4 extended value: 0x{:02X}", extended)));
        }

        reader.assert(b'\0')?;
        reader.assert_bytes(b"\0\0\0\0")?;

        let hash_table_offset = reader.read_u64()?;

        Ok(Self {
            _magic: *b"BND4",
            unk04,
            unk05,
            _pad1: 0u16,
            _pad2: 0u8,
            big_endian,
            bit_big_endian,
            _pad3: 0u8,
            entry_count,
            _header_size: header_size,
            signature,
            entry_header_size,
            headers_end,
            unicode,
            flags,
            extended,
            _pad4: 0u8,
            reserved: 0u32,
            hash_table_offset,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian;

        writer.write_bytes(b"BND4")?;

        writer.write_boolean(self.unk04)?;
        writer.write_boolean(self.unk05)?;
        writer.pad(2)?;

        writer.pad(1)?;
        writer.write_boolean(self.big_endian)?;

        writer.write_boolean(!self.bit_big_endian)?; //inverse
        writer.pad(1)?;

        writer.write_u32(self.entry_count)?;
        writer.write_u64(0x40)?;
        writer.write_bytes(&self.signature)?;
        writer.write_u64(self.entry_header_size)?;

        writer.reserve::<u64>("HeadersEnd".to_string())?;

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

impl BND4Header {
    ///returns default values per game for `unk04`, `unk05`, `unicode` and `extended`
    pub fn defaults(game: Game) -> (bool, bool, bool, i32) {
        let table_type = match game {
            Game::BB => 0,
            //value hasn't changed since DS3
            Game::DS3 => 4,
            Game::SDT => 4,
            Game::ER => 4,
            Game::AC6 => 4,
            Game::NR => 4,
            _=> 0 // TODO: is this correct for sotfs?
        };
        (false, false, true, table_type)
    }
}



#[derive(Debug, Clone)]
pub struct BND4EntryHeader {
    pub flags: EntryFlags,

    _pad: [u8;3], // b'\0'*3
    _marker: i32, // asserted -1
    
    pub compressed_size: i64,
    pub uncompressed_size: Option<i64>,

    pub data_offset: VariableUInt,

    pub entry_id: Option<i32>, // -1 when not given
    pub name_offset:  Option<u32>,
    
    // Only when format == NAMES_1.
    pub names1_id: Option<i32>,
    pub _names1_pad: Option<i32>, // always 0
}

impl BND4EntryHeader {
    pub fn from_reader(reader: &mut BinaryReader, format: BinderFlags, bit_big_endian: bool) -> Result<Self, BinaryReaderError> {
        let flags = EntryFlags::from_byte(reader.read_u8()?, bit_big_endian);

        reader.assert_bytes(b"\0\0\0")?; //_pad1

        let marker = reader.assert::<i32>(-1i32)?;

        let compressed_size = reader.read_i64()?;

        let uncompressed_size = if format.has_compression() {
            Some(reader.read_i64()?)
        } else {
            None
        };

        let data_offset = if format.has_long_offsets() {
            VariableUInt::ULong(reader.read_u64()?)
        } else {
            VariableUInt::UInt(reader.read_u32()?)
        };

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

        let (names1_id, names1_pad) = if format == BinderFlags::HAS_NAMES_1 {
            (
                Some(reader.read_i32()?),
                Some(reader.read_i32()?),
            )
        } else {
            (None, None)
        };

        Ok(Self {
            flags,
            _pad: [0u8;3],
            _marker: marker,
            compressed_size,
            uncompressed_size,
            data_offset,
            entry_id,
            name_offset,
            names1_id,
            _names1_pad: names1_pad,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, format: BinderFlags, bit_big_endian: bool, index: usize) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.flags.to_byte(bit_big_endian))?;
        writer.pad(3)?;

        writer.write_i32(-1)?;

        writer.reserve::<u64>(format!("FileCompressedSize{}", index))?;

        if format.has_compression() {
            writer.reserve::<u64>(format!("FileUncompressedSize{}", index))?;
        }

        if format.has_long_offsets() {
            writer.reserve::<u64>(format!("FileDataOffset{}", index))?;
        } else {
            writer.reserve::<u32>(format!("FileDataOffset{}", index))?;
        }

        if format.has_ids() {
            writer.write_i32(self.entry_id.ok_or_else(|| 
                BinaryWriterError::Custom(format!("BND4 entry {} requires an ID.", index))
            )?)?;
        }

        if format.has_names() {
            writer.reserve::<u32>(format!("FileNameOffset{}", index))?;
        }

        if format == BinderFlags::HAS_NAMES_1 {
            writer.write_i32(self.names1_id.ok_or_else(|| 
                BinaryWriterError::Custom(format!("BND4 entry {} requires a Names1 ID.", index))
            )?)?;

            writer.write_i32(self._names1_pad.unwrap_or(0))?;
        }

        Ok(())
    }

}


#[derive(Debug, Clone)]
pub struct BND4Entry {
    pub name: Option<String>, // not serialized. For convenience.
    pub header: BND4EntryHeader,
    pub data: Vec<u8>,
}

impl BND4Entry {
    pub fn from_reader(reader: &mut BinaryReader, flags: BinderFlags, bit_big_endian: bool, unicode: bool) -> Result<Self, BinaryReaderError> {
        let header = BND4EntryHeader::from_reader(reader, flags, bit_big_endian)?;

        // Names are referenced by absolute offsets from the start of the BND4.
        let name = match header.name_offset {
            Some(offset) => {
                reader.step_in(offset as u64)?;
                let name = if unicode {
                    reader.read_utf16()?
                } else {
                    reader.read_shift_jis()?
                };
                reader.step_out()?;
                Some(name)
            }
            None => None,
        };

        reader.step_in(header.data_offset.as_u64())?;
        let stored_data = reader.read_bytes(header.compressed_size as usize)?;
        reader.step_out()?;

        eprintln!("entry {:?}: flags={:?} compressed={} first4={:02X?}",
    name, header.flags, header.flags.is_compressed(), &stored_data[..stored_data.len().min(4)]);

        let data = if header.flags.is_compressed() {
            let (data, _) = unsafe { Decompress::raw(&stored_data) }
                .map_err(|e| BinaryReaderError::custom(e.to_string()))?;

            let expected_size = header.uncompressed_size
                .ok_or_else(|| BinaryReaderError::custom(
                    "Compressed BND4 entry has no uncompressed size"
                ))?;

            if data.len() != expected_size as usize {
                return Err(BinaryReaderError::Custom(
                    format!("Expected entry size of {}, got {}", expected_size, data.len())
                ));
            }

            data
        } else {
            stored_data
        };

        Ok(Self { name, header, data })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, flags: BinderFlags, index: usize) -> Result<(), BinaryWriterError> {
        if !self.data.is_empty() {
            writer.pad_align(0x10)?;
        }

        let data_offset = writer.position();
        let uncompressed_size = self.data.len() as i64;

        let stored_data = if self.header.flags.is_compressed() {
            let compset = CompressSettings::Oodle(DCXType::DCX_KRAK, OodleSettings::KRAK);
            unsafe { Compress::raw(&self.data, &compset) }
                .map_err(|e| BinaryWriterError::custom(e.to_string()))?

        } else {
            self.data.clone()
        };

        let compressed_size = stored_data.len() as i64;

        writer.fill::<u64>(format!("FileCompressedSize{}", index), compressed_size as u64)?;

        if flags.has_compression() {
            writer.fill::<u64>(format!("FileUncompressedSize{}", index), uncompressed_size as u64)?;
        }

        if flags.has_long_offsets() {
            writer.fill::<u64>(format!("FileDataOffset{}", index), data_offset as u64)?;
        } else {
            writer.fill::<u32>(format!("FileDataOffset{}", index), data_offset as u32)?;
        }

        writer.write_bytes(&stored_data)?;

        Ok(())
    }

}

impl BinderEntry for BND4Entry {
    type Identifier = Option<i32>;

    fn identity(&self) -> &Self::Identifier {
        &self.header.entry_id
    }
}



#[derive(Debug, Clone)]
pub struct BND4 {
    pub header: BND4Header,
    pub entries: Vec<BND4Entry>,
}

impl IO for BND4 {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let header = BND4Header::from_reader(reader)?;

        reader.big_endian = header.big_endian; // set by flag

        let expected_entry_header_size = header.flags.get_entry_header_size();

        if header.entry_header_size != expected_entry_header_size as u64 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND4 entry header size: expected 0x{:X}, got 0x{:X}",
                expected_entry_header_size,
                header.entry_header_size
            )));
        }

        let mut entries = Vec::with_capacity(header.entry_count as usize);

        for _ in 0..header.entry_count {
            entries.push(BND4Entry::from_reader(reader, header.flags, header.bit_big_endian, header.unicode)?);
        }

        if header.extended == 4 {
            reader.step_in(header.hash_table_offset)?;
            BinderHashTable::from_reader(reader)?; // not stored, just read to assert that it's correct
            reader.step_out()?;
        } else {
            reader.assert::<i64>(0)?;
        }

        Ok(Self { header, entries })
    }  

    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();

        writer.big_endian = self.header.big_endian;

        self.header.entry_count = self.entries.len() as u32; // modifying entries at all may make this incosnistent

        self.header.to_writer(&mut writer)?;

        // Entry headers
        for (index, entry) in self.entries.iter().enumerate() {
            entry.header.to_writer(&mut writer, self.header.flags, self.header.bit_big_endian, index)?;
        }

        // Entry names
        if self.header.flags.has_names() {
            for (index, entry) in self.entries.iter().enumerate() {
                writer.fill::<u32>(format!("FileNameOffset{}", index), writer.position() as u32)?;

                if self.header.unicode {
                    writer.write_utf16(entry.name.as_deref().unwrap_or(""), true)?;
                } else {
                    writer.write_shift_jis(entry.name.as_deref().unwrap_or(""), true)?;
                }
            }
        }

        // Hash table
        if self.header.extended == 4 {
            writer.pad_align(8)?; //align to 8 bytes
            writer.fill::<u64>("HashTableOffset".to_string(), writer.position())?;

            let names: Vec<String> = self.entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    entry.name.clone().ok_or_else(|| {
                        BinaryWriterError::Custom(
                            format!("BND4 entry {} requires a name for the hash table.", index)
                        )
                    })
                })
                .collect::<Result<_, _>>()?;

            let mut hash_table = BinderHashTable::from_names(&names)?;
            hash_table.to_writer(&mut writer)?;

        } else {
            writer.fill::<u64>("HashTableOffset".to_string(), 0)?;
        }

        //header region ends AFTER hash table
        writer.fill::<u64>("HeadersEnd".to_string(), writer.position())?;

        // File data
        for (index, entry) in self.entries.iter().enumerate() {
            entry.to_writer(&mut writer, self.header.flags, index)?;
        }

        Ok(writer)
    }

}

impl Binder for BND4 {
    const VERSION: BinderVersion = BinderVersion::V1;

    type Entry = BND4Entry;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}
