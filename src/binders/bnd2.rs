use crate::{binary::{BinaryReader, BinaryWriter, IO}, binders::{Binder, BinderEntry, BinderVersion}, errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError}};
use bitflags::{bitflags, parser::to_writer};


#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum BND2FilePathMode {
    Nameless = 0, // no names for files
    FileName = 1, // only names for file entries
    FullPath = 2, // all files have their full path
    BaseDirectory = 3 // all paths have a common root and each entry writes the rest of it
}

impl TryFrom<u8> for BND2FilePathMode {
    type Error = FerrisoulsError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Nameless),
            1 => Ok(Self::FileName),
            2 => Ok(Self::FullPath),
            3 => Ok(Self::BaseDirectory),
            _ => Err(BinaryReaderError::Custom(
                format!("Invalid BND2 file path mode: {}", value)
            ).into()),
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BND2HeaderFlags: u8 {
        const HEADER_ITEM      = 0x01;
        const ENDIAN           = 0x02;
        const FILE_VERSION     = 0x04;
        const FILE_SIZE        = 0x08;
        const FILE_NUM         = 0x10;
        const BASE_DIR_OFFSET  = 0x20;
        const ALIGNMENT_SIZE   = 0x40;
        const OPTION            = 0x80;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct BND2EntryFlags: u8 {
        const ID          = 0x01;
        const OFFSET      = 0x02;
        const SIZE        = 0x04;
        const NAME_OFFSET = 0x08;
        const FLAG5       = 0x10;
        const FLAG6       = 0x20;
        const FLAG7       = 0x40;
        const FLAG8       = 0x80;
    }
}


#[derive(Debug, Clone)]
pub struct BND2Header {
    pub header_info_flags: BND2HeaderFlags,
    pub entry_info_flags: BND2EntryFlags,

    pub unk06: u8,
    pub unk07: u8,

    pub file_version: i32,
    pub file_size: u32, // not serialized but useful to have

    pub file_count: u32,
    pub base_dir_offset: u32,
    pub alignment_size: u16,
    pub path_mode: BND2FilePathMode,
    pub unk1b: u8,
}

impl IO for BND2Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.big_endian = false;

        reader.assert_bytes(b"BND\0")?;

        let header_info_flags = BND2HeaderFlags::from_bits_retain(reader.read_u8()?);

        let entry_info_flags = BND2EntryFlags::from_bits_retain(reader.read_u8()?);

        let unk06 = reader.read_u8()?;
        let unk07 = reader.read_u8()?;

        let file_version = reader.read_i32()?;
        let file_size = reader.read_u32()?;
        let file_count = reader.read_u32()?;
        let base_dir_offset = reader.read_u32()?;
        let alignment_size = reader.read_u16()?;

        let path_mode = BND2FilePathMode::try_from(reader.read_u8()?)
            .map_err(|_| BinaryReaderError::custom("Failed to parse path mode"))?;

        let unk1b = reader.read_u8()?;
        if unk1b != 0 && unk1b != 1 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND2 unk1B: {}", unk1b)
            ));
        }

        reader.assert::<u32>(0)?;

        if !entry_info_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            reader.assert::<u32>(0)?;
        }

        Ok(Self {
            header_info_flags,
            entry_info_flags,
            unk06,
            unk07,
            file_version,
            file_size,
            file_count,
            base_dir_offset,
            alignment_size,
            path_mode,
            unk1b,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = false;

        writer.write_bytes(b"BND\0")?;
        writer.write_u8(self.header_info_flags.bits())?;
        writer.write_u8(self.entry_info_flags.bits())?;
        writer.write_u8(self.unk06)?;
        writer.write_u8(self.unk07)?;

        writer.write_i32(self.file_version)?;

        writer.reserve::<u32>("fileSize".to_string())?;

        writer.write_u32(self.file_count)?;
        writer.reserve::<u32>("baseDirOffset".to_string())?;

        writer.write_u16(self.alignment_size)?;
        writer.write_u8(self.path_mode as u8)?;
        writer.write_u8(self.unk1b)?;

        writer.write_u32(0)?;

        if !self.entry_info_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            writer.write_u32(0)?;
        }

        Ok(())
    }

}



#[derive(Debug, Clone)]
pub struct BND2EntryHeader {
    pub id: i32,
    pub name: String,
    pub offset: i32,
    pub size: i32,
}

impl BND2EntryHeader {
    pub fn from_reader(reader: &mut BinaryReader, path_mode: BND2FilePathMode, entry_flags: BND2EntryFlags) -> Result<Self, BinaryReaderError> {
        let id = reader.read_i32()?;
        let offset = reader.read_i32()?;
        let size = reader.read_i32()?;

        let name = if entry_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            let name_offset = reader.read_i32()?;

            match path_mode {
                BND2FilePathMode::Nameless => id.to_string(),

                BND2FilePathMode::FileName
                | BND2FilePathMode::FullPath
                | BND2FilePathMode::BaseDirectory => {
                    if name_offset < 0 {
                        return Err(BinaryReaderError::Custom(
                            format!("Invalid BND2 name offset: {}", name_offset)
                        ));
                    }

                    reader.step_in(name_offset as u64)?;
                    let name = reader.read_shift_jis()?;
                    reader.step_out()?;

                    name
                }
            }
        } else {
            id.to_string()
        };

        Ok(Self {
            id,
            name,
            offset,
            size
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, path_mode: BND2FilePathMode, entry_flags: BND2EntryFlags, index: usize) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.id)?;

        writer.reserve::<u32>(format!("fileOffset_{}", index))?;
        writer.reserve::<u32>(format!("fileSize_{}", index))?;

        if entry_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            if path_mode == BND2FilePathMode::Nameless {
                writer.write_i32(0)?;
            } else {
                writer.reserve::<u32>(format!("nameOffset_{}", index))?;
            }
        }

        Ok(())
    }

}



#[derive(Debug, Clone)]
pub struct BND2Entry {
    pub header: BND2EntryHeader,
    pub data: Vec<u8>,
}

impl BND2Entry {
    pub fn from_reader(reader: &mut BinaryReader, header: BND2EntryHeader) -> Result<BND2Entry, BinaryReaderError> {
        if header.offset < 0 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND2 file offset: {}", header.offset)
            ));
        }

        if header.size < 0 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND2 file size: {}", header.size)
            ));
        }

        reader.step_in(header.offset as u64)?;

        let data = reader.read_bytes(header.size as usize)?;

        reader.step_out()?;

        Ok(Self {
            header,
            data
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter, alignment_size: u16, index: usize) -> Result<(), BinaryWriterError> {
        writer.pad(alignment_size as usize)?;

        self.header.offset = writer.position() as i32;
        self.header.size = self.data.len() as i32;

        writer.write_bytes(&self.data)?;

        writer.fill::<i32>(format!("fileOffset_{}", index), self.header.offset)?;
        writer.fill::<i32>(format!("fileSize_{}", index), self.header.size)?;

        Ok(())
    }

}

impl BinderEntry for BND2Entry {
    type Identifier = i32;

    fn identity(&self) -> &Self::Identifier {
        &self.header.id
    }
}

/// BND2 archive.
#[derive(Debug, Clone)]
pub struct BND2 {
    pub header_info_flags: BND2HeaderFlags,
    pub entry_flags: BND2EntryFlags,

    pub unk06: u8,
    pub unk07: u8,

    /// BND2 versions seen in SoulsFormats are 202 and 211.
    pub file_version: i32,

    /// Alignment used before each file's data.
    pub alignment_size: u16,

    pub path_mode: BND2FilePathMode,

    pub unk1b: u8,

    /// Used only with BaseDirectory path mode.
    pub base_directory: String,

    pub entries: Vec<BND2Entry>,
}

impl BND2 {
    fn read_header(reader: &mut BinaryReader) -> Result<(Self, Vec<BND2EntryHeader>), BinaryReaderError> {
        reader.big_endian = false;

        let magic = reader.read_bytes(4)?;

        if magic.as_slice() != b"BND\0" {
            return Err(BinaryReaderError::Custom(
                "Invalid BND2 magic.".to_string()
            ));
        }

        let header_info_flags = BND2HeaderFlags::from_bits_retain(reader.read_u8()?);

        let entry_flags = BND2EntryFlags::from_bits_retain(reader.read_u8()?);

        let unk06 = reader.read_u8()?;
        let unk07 = reader.read_u8()?;

        let file_version = reader.read_i32()?;
        let _file_size = reader.read_i32()?;
        let file_count = reader.read_i32()?;

        if file_count < 0 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND2 file count: {}", file_count)
            ));
        }

        let base_dir_offset = reader.read_i32()?;
        let alignment_size = reader.read_u16()?;

        let path_mode = BND2FilePathMode::try_from(reader.read_u8()?)
            .map_err(|_| BinaryReaderError::Custom("Failed to get path mode".to_string()))?;

        let unk1b = reader.read_u8()?;
        let unk1c = reader.read_u32()?;

        if unk1c != 0 {
            return Err(BinaryReaderError::Custom(
                format!("Invalid BND2 Unk1C: 0x{:08X}", unk1c)
            ));
        }

        if !entry_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            reader.assert::<u32>(0)?; //strange unkown field when !NAME_OFFSET
        }

        let base_directory =
            if path_mode == BND2FilePathMode::BaseDirectory && entry_flags.contains(BND2EntryFlags::NAME_OFFSET)
            {
                if base_dir_offset < 0 {
                    return Err(BinaryReaderError::Custom(
                        format!("Invalid BND2 base directory offset: {}", base_dir_offset)
                    ));
                }

                reader.step_in(base_dir_offset as u64)?;
                let result = reader.read_shift_jis();
                reader.step_out()?;

                result?

            } else {
                String::new()
            };

        let mut file_headers = Vec::with_capacity(file_count as usize);

        for _ in 0..file_count {
            file_headers.push(BND2EntryHeader::from_reader(reader, path_mode, entry_flags)?);
        }

        let bnd = Self {
            header_info_flags,
            entry_flags,
            unk06,
            unk07,
            file_version,
            alignment_size,
            path_mode,
            unk1b,
            base_directory,
            entries: Vec::new(),
        };

        Ok((bnd, file_headers))
    }

    fn from_reader(&self, reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let (mut bnd, file_headers) = Self::read_header(reader)?;

        let mut entries: Vec<BND2Entry> = Vec::with_capacity(file_headers.len());

        for header in file_headers {
            entries.push(BND2Entry::from_reader(reader, header)?);
        }

        bnd.entries = entries;

        Ok(bnd)
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = false;

        writer.write_bytes(b"BND\0")?;

        writer.write_u8(self.header_info_flags.bits())?;
        writer.write_u8(self.entry_flags.bits())?;

        writer.write_u8(self.unk06)?;
        writer.write_u8(self.unk07)?;

        writer.write_i32(self.file_version)?;

        writer.reserve::<u32>("fileSize".to_string())?;

        writer.write_i32(self.entries.len() as i32)?;

        writer.reserve::<u32>("baseDirOffset".to_string())?;

        writer.write_u16(self.alignment_size)?;
        writer.write_u8(self.path_mode as u8)?;

        writer.write_u8(self.unk1b)?;
        writer.write_u32(0)?;

        if !self.entry_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            writer.write_u32(0)?;
        }

        //entry headers
        for (index, entry) in self.entries.iter().enumerate() {
            entry.header.to_writer(writer, self.path_mode, self.entry_flags, index)?;
        }

        if self.entry_flags.contains(BND2EntryFlags::NAME_OFFSET) {
            self.write_file_names(writer)?;
        } else {
            writer.fill::<i32>("baseDirOffset".to_string(), 0)?;
        }

        //write file data
        for (index, entry) in self.entries.iter_mut().enumerate() {
            entry.to_writer(writer, self.alignment_size, index)?;
        }

        writer.fill::<i32>("fileSize".to_string(), writer.position() as i32)?;

        Ok(())
    }

    fn write_file_names(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        if self.path_mode == BND2FilePathMode::BaseDirectory {
            writer.fill::<i32>("baseDirOffset".to_string(), writer.position() as i32)?;
            writer.write_shift_jis(&self.base_directory, true)?;
        } else {
            writer.fill::<i32>("baseDirOffset".to_string(), 0)?;
        }

        if self.path_mode == BND2FilePathMode::Nameless {
            return Ok(());
        }

        for (index, entry) in self.entries.iter().enumerate() {
            writer.fill::<i32>(format!("nameOffset_{}", index), writer.position() as i32)?;

            let mut name = entry.header.name.clone();

            if self.path_mode == BND2FilePathMode::FullPath {
                let rooted = is_path_rooted(&name);

                if !rooted {
                    name = format!("K:\\{}", name);
                }
            }

            writer.write_shift_jis(&name, true)?;
        }

        Ok(())
    }

}

impl Binder for BND2 {
    const VERSION: BinderVersion = BinderVersion::V2;

    type Entry = BND2Entry;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}


fn is_path_rooted(path: &str) -> bool {
    if path.starts_with('\\') || path.starts_with('/') {
        return true;
    }

    let bytes = path.as_bytes();

    bytes.len() >= 3
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}
