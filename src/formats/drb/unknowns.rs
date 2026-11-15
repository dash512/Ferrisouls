//! Unknown/unused structs within DRB files: anik, anio, anim, scdk, scdo, scdl.

use crate::{binary::{BinaryReader, BinaryWriter, IO}, errors::{BinaryReaderError, BinaryWriterError}, formats::drb::DRBVersion};

pub const ANIK_SIZE: u32 = 0x20;
pub const ANIO_SIZE: u32 = 0x10;
pub const SCDK_SIZE: u32 = 0x20;
pub const SCDO_SIZE: u32 = 0x10;

#[derive(Debug, Default)]
pub struct Anik {
    pub name: String,
    pub name_offset: u32,
    pub unk04: u32,
    pub unk08: u8, // 0-1
    pub unk09: u8, // 1-2
    pub unk0a: u16, // 0
    /// An offset into the INTP block.
    pub intp_offset: u32,
    /// An offset into the ANIP block.
    pub anip_offset: u32,
    pub unk14: u32,
    pub unk18: u32,
    pub unk1c: u32,
}

impl Anik {
    pub fn new() -> Self {
        Self { unk04: 1, unk09: 2, ..Default::default() }
    }
}

impl IO for Anik {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(name_offset as u64)?;
        Ok(Self {
            name,
            name_offset,
            unk04: reader.read_u32()?,
            unk08: reader.read_u8()?,
            unk09: reader.read_u8()?,
            unk0a: reader.read_u16()?,
            intp_offset: reader.read_u32()?,
            anip_offset: reader.read_u32()?,
            unk14: reader.read_u32()?,
            unk18: reader.read_u32()?,
            unk1c: reader.read_u32()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.unk04)?;
        writer.write_u8(self.unk08)?;
        writer.write_u8(self.unk09)?;
        writer.write_u16(self.unk0a)?;
        writer.write_u32(self.intp_offset)?;
        writer.write_u32(self.anip_offset)?; // was intp_offset
        writer.write_u32(self.unk14)?;
        writer.write_u32(self.unk18)?;
        writer.write_u32(self.unk1c)
    }
}

#[derive(Debug, Default)]
pub struct Anio {
    pub unk00: u32,
    pub unk0c: u32,
    pub aniks: Vec<Anik>,
}

impl Anio {
    /// `anik_start` is the absolute position of the ANIK block's data.
    pub fn from_reader(reader: &mut BinaryReader, anik_start: u64) -> Result<Self, BinaryReaderError> {
        let unk00 = reader.read_u32()?;
        let anik_count = reader.read_u32()?;
        let anik_offset = reader.read_u32()?;
        let unk0c = reader.read_u32()?;

        let mut aniks = Vec::with_capacity(anik_count as usize);
        for i in 0..anik_count {
            reader.step_in(anik_start + (anik_offset + ANIK_SIZE * i) as u64)?;
            aniks.push(Anik::from_reader(reader)?);
            reader.step_out()?;
        }
        Ok(Self { unk00, unk0c, aniks })
    }

    /// `anik_offset` is this Anio's first Anik, relative to the ANIK block data.
    pub fn to_writer(&self, writer: &mut BinaryWriter, anik_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.unk00)?;
        writer.write_u32(self.aniks.len() as u32)?;
        writer.write_u32(anik_offset)?;
        writer.write_u32(self.unk0c)
    }
}

#[derive(Debug, Default)]
pub struct Anim {
    pub name: String,
    pub name_offset: u32,
    pub unk0c: u32,
    pub unk10: u32, // 4
    pub unk14: u32, // 4
    pub unk18: u32, // 4
    pub unk1c: u32, // 1
    pub unk20: u32,
    pub unk24: u32,
    pub unk28: u32,
    pub unk2c: u32,
    pub anios: Vec<Anio>,
}

impl Anim {
    pub fn new() -> Self {
        Self { unk10: 4, unk14: 4, unk18: 4, unk1c: 1, ..Default::default() }
    }

    pub fn from_reader(reader: &mut BinaryReader, anio_start: u64, anik_start: u64, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(name_offset as u64)?;
        let anio_count = reader.read_u32()?;
        let anio_offset = reader.read_u32()?;

        let unk0c = reader.read_u32()?;
        let unk10 = reader.read_u32()?;
        let unk14 = reader.read_u32()?;
        let unk18 = reader.read_u32()?;
        let unk1c = reader.read_u32()?;

        let (unk20, unk24, unk28, unk2c) = if version != DRBVersion::ACFA {
            (reader.read_u32()?, reader.read_u32()?, reader.read_u32()?, reader.read_u32()?)
        } else {
            (0, 0, 0, 0)
        };

        let mut anios = Vec::with_capacity(anio_count as usize);
        for i in 0..anio_count {
            reader.step_in(anio_start + (anio_offset + ANIO_SIZE * i) as u64)?;
            anios.push(Anio::from_reader(reader, anik_start)?);
            reader.step_out()?;
        }

        Ok(Self { name, name_offset, unk0c, unk10, unk14, unk18, unk1c, unk20, unk24, unk28, unk2c, anios })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, anio_offset: u32, version: DRBVersion) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.anios.len() as u32)?;
        writer.write_u32(anio_offset)?;
        writer.write_u32(self.unk0c)?;
        writer.write_u32(self.unk10)?;
        writer.write_u32(self.unk14)?;
        writer.write_u32(self.unk18)?;
        writer.write_u32(self.unk1c)?;
        if version != DRBVersion::ACFA {
            writer.write_u32(self.unk20)?;
            writer.write_u32(self.unk24)?;
            writer.write_u32(self.unk28)?;
            writer.write_u32(self.unk2c)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Scdk {
    pub name: String,
    pub name_offset: u32,
    pub unk04: u32,
    pub unk08: u32, // 1
    pub unk0c: u32, // 0-1
    pub unk14: u32,
    pub unk18: u32,
    pub unk1c: u32,
    /// An index into the anim list.
    pub anim_index: u32,
    pub scdp04: u32, // 0-1
}

impl Scdk {
    pub fn new() -> Self {
        Self { unk08: 1, ..Default::default() }
    }

    pub fn from_reader(reader: &mut BinaryReader, scdp_start: u64) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(name_offset as u64)?;
        let unk04 = reader.read_u32()?;
        let unk08 = reader.read_u32()?;
        let unk0c = reader.read_u32()?;
        let scdp_offset = reader.read_u32()?;
        let unk14 = reader.read_u32()?;
        let unk18 = reader.read_u32()?;
        let unk1c = reader.read_u32()?;

        reader.step_in(scdp_start + scdp_offset as u64)?;
        let anim_index = reader.read_u32()?;
        let scdp04 = reader.read_u32()?;
        reader.step_out()?;

        Ok(Self { name, name_offset, unk04, unk08, unk0c, unk14, unk18, unk1c, anim_index, scdp04 })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, scdp_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.unk04)?;
        writer.write_u32(self.unk08)?;
        writer.write_u32(self.unk0c)?;
        writer.write_u32(scdp_offset)?;
        writer.write_u32(self.unk14)?;
        writer.write_u32(self.unk18)?;
        writer.write_u32(self.unk1c)
    }

    pub fn scdp_to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.anim_index)?;
        writer.write_u32(self.scdp04)
    }
}

#[derive(Debug, Default)]
pub struct Scdo {
    pub name: String,
    pub name_offset: u32,
    pub unk0c: u32,
    pub scdks: Vec<Scdk>,
}

impl Scdo {
    pub fn from_reader(reader: &mut BinaryReader, scdk_start: u64, scdp_start: u64) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(name_offset as u64)?;
        let scdk_count = reader.read_u32()?;
        let scdk_offset = reader.read_u32()?;
        let unk0c = reader.read_u32()?;

        let mut scdks = Vec::with_capacity(scdk_count as usize);
        for i in 0..scdk_count {
            reader.step_in(scdk_start + (scdk_offset + SCDK_SIZE * i) as u64)?;
            scdks.push(Scdk::from_reader(reader, scdp_start)?);
            reader.step_out()?;
        }
        Ok(Self { name, name_offset, unk0c, scdks })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, scdk_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.scdks.len() as u32)?;
        writer.write_u32(scdk_offset)?;
        writer.write_u32(self.unk0c)
    }
}

#[derive(Debug, Default)]
pub struct Scdl {
    pub name: String,
    pub name_offset: u32,
    pub unk0c: u32,
    pub scdos: Vec<Scdo>,
}

impl Scdl {
    pub fn from_reader(reader: &mut BinaryReader, scdo_start: u64, scdk_start: u64, scdp_start: u64) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(name_offset as u64)?;
        let scdo_count = reader.read_u32()?;
        let scdo_offset = reader.read_u32()?;
        let unk0c = reader.read_u32()?;

        let mut scdos = Vec::with_capacity(scdo_count as usize);
        for i in 0..scdo_count {
            reader.step_in(scdo_start + (scdo_offset + SCDO_SIZE * i) as u64)?;
            scdos.push(Scdo::from_reader(reader, scdk_start, scdp_start)?);
            reader.step_out()?;
        }
        Ok(Self { name, name_offset, unk0c, scdos })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, scdo_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.scdos.len() as u32)?;
        writer.write_u32(scdo_offset)?;
        writer.write_u32(self.unk0c)
    }
}

