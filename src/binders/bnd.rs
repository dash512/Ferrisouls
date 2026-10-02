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
}

impl BNDEntryHeader {
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
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

        Ok(
            Self {
                entry_id,
                data_offset,
                file_size,
                name_offset,
                name
            }
        )
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter, index: usize) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.entry_id)?;

        writer.reserve::<u32>(format!("FileOffset{}", index))?;

        writer.write_u32(self.file_size)?;

        writer.reserve::<u32>(format!("FileName{}", index))?;

        Ok(())
    }

}



#[derive(Debug, Clone)]
pub struct BNDEntry {
    pub header: BNDEntryHeader,
    pub data: Vec<u8>
}

impl BNDEntry {
    pub fn from_reader(reader: &mut BinaryReader, header: BNDEntryHeader) -> Result<Self, BinaryReaderError> {
        reader.step_in(header.data_offset as u64)?;
        let data = reader.read_bytes(header.file_size as usize)?;
        reader.step_out()?;

        Ok(
            Self {
                header,
                data
            }
        )
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter, index: usize) -> Result<(), BinaryWriterError> {
        writer.fill::<u32>(format!("FileOffset{}", index), writer.position() as u32)?;

        self.header.data_offset = writer.position() as u32;
        self.header.file_size = self.data.len() as u32;

        writer.write_bytes(&self.data)?;

        Ok(())
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
    fn read_header(reader: &mut BinaryReader) -> Result<(BNDHeader, String, Vec<BNDEntryHeader>), BinaryReaderError> {
        let header = BNDHeader::from_reader(reader)?;

        let root_file_path =
            if header.root_path_offset != 0 {
                if header.root_path_offset < 0 {
                    return Err(BinaryReaderError::Custom(
                            format!("Invalid BND root path offset: {}", header.root_path_offset)
                        )
                    );
                }

                reader.step_in(header.root_path_offset as u64)?;
                let root_file_path = reader.read_ascii()?;
                reader.step_out()?;

                root_file_path
            } else {
                String::new()
            };

        let mut entry_headers =
            Vec::with_capacity(header.entry_count as usize);

        for _ in 0..header.entry_count {
            entry_headers.push(BNDEntryHeader::from_reader(reader)?);
        }

        Ok(
            (
                header,
                root_file_path,
                entry_headers
            )
        )
    }

    fn from_reader(&self, reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let (header, root_file_path, entry_headers) =
            Self::read_header(reader)?;

        let mut entries =
            Vec::with_capacity(entry_headers.len());

        for entry_header in entry_headers {
            entries.push(BNDEntry::from_reader(reader, entry_header)?
            );
        }

        Ok(
            Self {
                header,
                entries,
                root_file_path
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(b"BND\0")?;

        writer.write_u16(0xFFFFu16);
        writer.write_u16(0u16);

        writer.write_i32(self.header.version)?;

        writer.reserve::<u32>("FileSize".to_string())?;

        writer.write_i32(self.entries.len() as i32)?;

        writer.reserve::<u32>("RootFilePath".to_string())?;

        writer.write_u16(self.header.format0)?;
        writer.write_u16(self.header.format1)?;

        writer.write_u32(0)?;

        //file headers
        for (index, entry) in self.entries.iter_mut().enumerate() {
            entry.header.to_writer(writer, index)?;
        }


        if self.root_file_path != "" {
            writer.fill::<i32>("RootFilePath".to_string(), writer.position() as i32)?;

            writer.write_shift_jis(&self.root_file_path, true)?;
        }
        else {
            writer.fill::<i32>("RootFilePath".to_string(), 0)?;
        }


        for (index, entry) in self.entries.iter().enumerate() {
            writer.fill::<i32>(format!("FileName{}", index), writer.position() as i32)?;

            writer.write_shift_jis(&entry.header.name, true)?;
        }

        writer.pad(0x10)?;

        //Files
        let entry_count = self.entries.len();
        for (index, entry) in self.entries.iter_mut().enumerate() {
            entry.to_writer(writer, index)?;

            if index != entry_count - 1 {
                writer.pad(0x10)?;
            }
        }

        writer.fill::<i32>("FileSize".to_string(), writer.position() as i32)?;

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
