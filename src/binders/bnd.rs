use super::{BinderFlags, EntryFlags};
use crate::binary::{IO, BinaryReader, BinaryWriter, bytes::VariableUInt};
use crate::binders::{Binder, BinderEntry, BinderVersion};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};


#[derive(Debug, Clone)]
pub struct BNDHeader {
    _magic: [u8;4], // asserted b"BND\0"
    unk1: u16, // asserted 0xFFFF
    unk2: u16, // asserted 0

    version: i32, //internal version?
    file_size: u32,

    entry_count: i32,
    root_path_offset: i32, // is 0 if no root path exists

    format0: u16,
    format1: u16,

    _pad: i32, // asserted 0
}

impl IO for BNDHeader {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BND\0")?;

        reader.assert(0xFFFFu16)?;
        reader.assert(0u16)?;

        let version = reader.read_i32()?;
        let file_size = reader.read_u32()?;

        let entry_count = reader.read_i32()?;
        if entry_count < 0 {
            return Err(BinaryReaderError::Custom(
                    format!("Invalid BND entry count: {}", entry_count)
                )
            );
        }

        let root_path_offset = reader.read_i32()?;

        let format0 = reader.read_u16()?;
        let format1 = reader.read_u16()?;

        reader.assert(0i32)?;

        Ok(
            Self {
                _magic: *b"BND\0",
                unk1: 0xFFFFu16,
                unk2: 0u16,
                version,
                file_size,
                entry_count,
                root_path_offset,
                format0,
                format1,
                _pad: 0i32,
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(b"BND\0")?;

        writer.write_u16(0xFFFFu16)?;
        writer.write_u16(0u16)?;

        writer.write_i32(self.version)?;
        writer.write_u32(self.file_size)?;

        writer.write_i32(self.entry_count)?;
        writer.write_i32(self.root_path_offset)?;

        writer.write_u16(self.format0)?;
        writer.write_u16(self.format1)?;

        writer.write_i32(0i32)?;//_pad

        Ok(())
    }

}


#[derive(Debug, Clone)]
pub struct BNDEntryHeader {
    pub entry_id: i32,
    pub data_offset: u32,
    pub file_size: u32,
    pub name_offset: u32,
    pub name: String,

    slot: i64 // unique header key set when written. Used for reservations. Not serialized.
}

impl IO for BNDEntryHeader {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let entry_id = reader.read_i32()?;
        let data_offset = reader.read_u32()?;
        let file_size = reader.read_u32()?;
        let name_offset = reader.read_u32()?;

        let name = if name_offset != 0 {
            reader.step_in(name_offset as u64)?;
            let name = reader.read_shift_jis()?;
            reader.step_out()?;
            name
        } else {
            format!("File_{}", entry_id)
        };

        Ok(Self { entry_id, data_offset, file_size, name_offset, name, slot: -1 })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.slot = writer.position() as i64;

        writer.write_i32(self.entry_id)?;
        writer.reserve::<u32>(format!("FileOffset{}", self.slot))?;
        writer.write_u32(self.file_size)?;
        writer.reserve::<u32>(format!("FileName{}", self.slot))?;

        Ok(())
    }

}



#[derive(Debug, Clone)]
pub struct BNDEntry {
    pub header: BNDEntryHeader,
    pub data: Vec<u8>,
}

impl IO for BNDEntry {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let header = BNDEntryHeader::from_reader(reader)?;

        reader.step_in(header.data_offset as u64)?;
        let data = reader.read_bytes(header.file_size as usize)?;
        reader.step_out()?;

        Ok(Self { header, data })
    }

    // Writes only the file data and fills the offset reserved by header.to_writer().
    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::new(false, false); // TODO: is this right?
        
        let pos = writer.position() as u32;
        writer.fill::<u32>(format!("FileOffset{}", self.header.slot), pos)?;

        self.header.data_offset = pos;
        self.header.file_size = self.data.len() as u32;

        writer.write_bytes(&self.data)?;

        Ok(writer)
    }
}

impl BinderEntry for BNDEntry {
    type Identifier = i32;

    fn identity(&self) -> &Self::Identifier {
        &self.header.entry_id
    }
}


#[derive(Debug, Clone)]
pub struct BND {
    header: BNDHeader,
    root_file_path: String,
    entries: Vec<BNDEntry>,
}

impl BND {
    fn read_header(reader: &mut BinaryReader) -> Result<(BNDHeader, String), BinaryReaderError> {
        let header = BNDHeader::from_reader(reader)?;

        let root_file_path = if header.root_path_offset != 0 {
            if header.root_path_offset < 0 {
                return Err(BinaryReaderError::Custom(
                    format!("Invalid BND root path offset: {}", header.root_path_offset)
                ));
            }
            reader.step_in(header.root_path_offset as u64)?;
            let path = reader.read_ascii()?;
            reader.step_out()?;
            path
        } else {
            String::new()
        };

        Ok((header, root_file_path))
    }
}

impl IO for BND {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let (header, root_file_path) = Self::read_header(reader)?;

        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            entries.push(BNDEntry::from_reader(reader)?);
        }

        Ok(Self { header, entries, root_file_path })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(b"BND\0")?;
        writer.write_u16(0xFFFFu16)?;
        writer.write_u16(0u16)?;
        writer.write_i32(self.header.version)?;
        writer.reserve::<u32>("FileSize".to_string())?;
        writer.write_i32(self.entries.len() as i32)?;
        writer.reserve::<u32>("RootPath".to_string())?;
        writer.write_u16(self.header.format0)?;
        writer.write_u16(self.header.format1)?;
        writer.write_u32(0)?;

        //entry headers
        for entry in self.entries.iter_mut() {
            entry.header.to_writer(writer)?;
        }

        //root path
        if !self.root_file_path.is_empty() {
            let pos = writer.position() as u32;
            writer.fill::<u32>("RootPath".to_string(), pos)?;
            writer.write_shift_jis(&self.root_file_path, true)?;
        } else {
            writer.fill::<u32>("RootPath".to_string(), 0)?;
        }

        //entry names
        for entry in self.entries.iter() {
            let pos = writer.position() as u32;
            writer.fill::<u32>(format!("FileOffset{}", entry.header.slot), pos)?;
            writer.write_shift_jis(&entry.header.name, true)?;
        }

        writer.pad(0x10)?;

        //entry data
        let last = self.entries.len().saturating_sub(1);
        for (index, entry) in self.entries.iter_mut().enumerate() {
            entry.to_writer(writer)?;
            if index != last {
                writer.pad(0x10)?;
            }
        }

        let end = writer.position() as u32;
        writer.fill::<u32>("FileSize".to_string(), end)?;

        Ok(())
    }

}

impl Binder for BND {
    const VERSION: BinderVersion = BinderVersion::V1;

    type Entry = BNDEntry;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}
