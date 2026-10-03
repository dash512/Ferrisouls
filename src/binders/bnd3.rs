use super::{BinderFlags, EntryFlags};
use crate::binary::{IO, BinaryReader, BinaryWriter, bytes::VariableUInt};
use crate::binders::{Binder, BinderEntry, BinderVersion};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};


#[derive(Debug, Clone)]
pub struct BND3Header {
    _magic: [u8;4], // asserted b"BND3"
    pub signature: [u8;8], // ascii encoded string

    pub flags: BinderFlags,//u8
    pub bit_big_endian: bool,
    pub big_endian: bool,

    _pad: u8, // b'\0'

    pub entry_count: i32,
    pub entry_headers_end: i32,
    
    pub unk2: u32, // b'\0'*4
    pub reserved: [u8;4] // b'\0'*4
}

impl IO for BND3Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"BND3")?;

        let signature = reader.read_bytes(8)?
            .try_into()
            .unwrap();

        let bit_big_endian = reader.read_boolean()?;

        let raw_flags = reader.read_u8()?;

        let flags = BinderFlags::from_byte(raw_flags, bit_big_endian);

        let big_endian = reader.read_boolean()?;

        reader.assert(bit_big_endian)?;

        reader.assert(b'\0')?;

        reader.big_endian = big_endian || flags.contains(BinderFlags::IS_BIG_ENDIAN);

        let entry_count = reader.read_i32()?;
        let entry_headers_end = reader.read_i32()?;

        let unk2 = reader.read_u32()?; // sometimes 0x80000000 in DeS, else always 0
        reader.assert_bytes(b"\0\0\0\0")?;

        //entry data:...

        Ok(
            Self {
                _magic: *b"BND3",
                signature,
                flags,
                bit_big_endian,
                big_endian,
                _pad: 0u8,
                entry_count,
                entry_headers_end,
                unk2,
                reserved: [0u8;4],
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian || self.flags.contains(BinderFlags::IS_BIG_ENDIAN);

        writer.write_bytes(b"BND3")?;

        writer.write_bytes(&self.signature)?;

        writer.write_u8(self.flags.to_byte(self.bit_big_endian))?;

        writer.write_boolean(self.big_endian)?;
        writer.write_boolean(self.bit_big_endian)?;

        writer.pad(1)?;

        writer.write_i32(self.entry_count)?;
        writer.write_i32(self.entry_headers_end)?;

        writer.write_u32(self.unk2)?;
        writer.pad(4)?;

        Ok(())
    }

}


#[derive(Debug, Clone)]
pub struct BND3EntryHeader {
    pub flags: EntryFlags,

    _pad1: [u8; 3],

    pub compressed_size: u32,

    pub data_offset: VariableUInt, // u64 when LONG_OFFSETS else u32

    pub entry_id: Option<i32>, // optional; -1 when not given.
    pub name_offset: Option<u32>, // offset to shift-jis encoded entry name
    pub uncompressed_size: Option<u32>,
}

impl BND3EntryHeader {
    pub fn from_reader(reader: &mut BinaryReader, bndflags: BinderFlags, bit_big_endian: bool) -> Result<Self, BinaryReaderError> {
        let flags = EntryFlags::from_byte(reader.read_u8()?, bit_big_endian);
        
        reader.assert_bytes(b"\0\0\0")?;

        let compressed_size = reader.read_u32()?;

        let data_offset = if bndflags.has_long_offsets() {
            VariableUInt::ULong(reader.read_u64()?)
        } else {
            VariableUInt::UInt(reader.read_u32()?)
        };

        let entry_id = if bndflags.has_ids() {
            Some(reader.read_i32()?)
        } else {
            None // -1i32 in SFNext
        };

        let name_offset = if bndflags.has_names() {
            Some(reader.read_u32()?)
        } else {
            None
        };

        let uncompressed_size = if bndflags.has_compression() {
            Some(reader.read_u32()?)
        } else {
            None // -1i32 in SFNext
        };

        Ok(
            Self {
                flags,
                _pad1: [0u8;3],
                compressed_size,
                data_offset,
                entry_id,
                name_offset,
                uncompressed_size
            }
        )
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, parent: &BND3Header) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.flags.to_byte(parent.bit_big_endian))?;

        writer.pad(3)?;

        writer.write_u32(self.compressed_size)?;

        match self.data_offset {
            VariableUInt::ULong(value)
                if parent.flags.has_long_offsets() =>
            {
                writer.write_u64(value)?;
            }

            VariableUInt::UInt(value)
                if !parent.flags.has_long_offsets() =>
            {
                writer.write_u32(value)?;
            }

            _ => return Err(BinaryWriterError::invalid_data("Invalid data offset width for BND3 format."))
        }

        if parent.flags.has_ids() {
            let entry_id = self.entry_id.ok_or_else(|| {
                BinaryWriterError::invalid_data("BND3 entry requires an entry ID.")
            })?;

            writer.write_i32(entry_id)?;
        }

        if parent.flags.has_names() {
            let offset = self.name_offset.ok_or_else(|| {
                BinaryWriterError::invalid_data("BND3 entry requires a name offset.")
            })?;

            writer.write_u32(offset)?;
        }

        if parent.flags.has_compression() {
            let offset = self.uncompressed_size.ok_or_else(|| {
                BinaryWriterError::invalid_data("BND3 entry requires uncompressed size.")
            })?;

            writer.write_u32(offset)?;
        }

        Ok(())
    }

}



#[derive(Debug, Clone)]
pub struct BND3Entry {
    pub header: BND3EntryHeader,
    pub data: Vec<u8>
}

impl BinderEntry for BND3Entry {
    type Identifier = Option<i32>;

    fn identity(&self) -> &Self::Identifier {
        &self.header.entry_id
    }
}



#[derive(Debug, Clone)]
pub struct BND3 {
    pub header: BND3Header,
    pub entries: Vec<BND3Entry>
}

impl IO for BND3 {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let header = BND3Header::from_reader(reader)?;

        if header.entry_count < 0 {
            return Err(BinaryReaderError::invalid_data("Invalid BND3 entry count."));
        }

        let mut entries = Vec::with_capacity(header.entry_count as usize);

        for _ in 0..header.entry_count {
            let entry_header = BND3EntryHeader::from_reader(reader, header.flags, header.bit_big_endian)?;

            let data_offset = match entry_header.data_offset {
                VariableUInt::ULong(value) => value,
                VariableUInt::UInt(value) => value as u64,
                _=> {return Err(BinaryReaderError::invalid_data("Data offset should be u32 or u64."));}
            };

            reader.step_in(data_offset)?;
            let data = reader.read_bytes(entry_header.compressed_size as usize)?;
            reader.step_out()?;

            entries.push(BND3Entry {header: entry_header, data});
        }

        Ok(
            Self {
                header,
                entries,
            }
        )
    }

    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();
        writer.big_endian = self.header.big_endian;

        self.header.entry_count = self.entries.len() as i32;
        self.header.entry_headers_end = 0;

        self.header.to_writer(&mut writer)?;

        let header_start = writer.position();

        for entry in self.entries.iter_mut() {
            entry.header.compressed_size = entry.data.len() as u32;
            entry.header.to_writer(&mut writer, &self.header)?;
        }

        for entry in self.entries.iter_mut() {
            if self.header.flags.has_names() {
                // Name offsets are expected to already be populated by the caller.
            }
        }

        if self.header.entry_headers_end != 0 {
            self.header.entry_headers_end = writer.position() as i32;
        }

        let _ = header_start;

        Ok(writer)
    }

}

impl Binder for BND3 {
    const VERSION: BinderVersion = BinderVersion::V1;

    type Entry = BND3Entry;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

