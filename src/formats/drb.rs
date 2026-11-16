//! UI configs used until DS2. For more modern games, see `sblyt.rs`. For BB/DS3, look into GFX with JPEXS Flash Decompiler.

use std::{collections::{HashMap, VecDeque}, fs::File, io::Write, path::Path};

use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::dcx::{Compress, Decompress, CompressSettings, DCXType};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use self::{
    control::Control,
    dlg::{Dlg, Dlgo},
    shape::Shape,
    unknowns::{Anim, Scdl},
};

pub mod control;
pub mod dlg;
pub mod shape;
pub mod unknowns;

#[derive(Debug, Default)]
pub struct DRBTexture {
    pub name: String,
    pub name_offset: u32,

    ///Internal network path to the texture
    pub path: String,
    pub path_offset: u32,
}

impl DRBTexture {
    pub fn from_reader(reader: &mut BinaryReader, str_start: u64) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let name = reader.get_utf16(str_start + name_offset as u64)?;

        let path_offset = reader.read_u32()?;
        let path = reader.get_utf16(str_start + path_offset as u64)?;

        reader.assert::<u32>(0)?;
        reader.assert::<u32>(0)?;
        Ok(Self { name, name_offset, path, path_offset })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(self.path_offset)?;
        writer.write_u32(0)?;
        writer.write_u32(0)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DRBVersion {
    ///Armored Core: For Answer
    ACFA,
    ///Dark Souls, Demon's Souls, and Shadow Assault: Tenchu
    #[default]
    DS,
    ///Dark Souls: Remastered; adds scaling info to shapes
    DSR,
}


#[derive(Default)]
pub struct DRB {
    pub version: DRBVersion,
    /// True for PS3/X360, false otherwise.
    pub big_endian: bool,

    /// Unknown data used by Aniks.
    pub anip_bytes: Vec<u8>,
    /// Unknown data used by Aniks.
    pub intp_bytes: Vec<u8>,

    pub anims: Vec<Anim>,
    pub scdls: Vec<Scdl>,
    /// Groups of UI elements.
    pub dlgs: Vec<Dlg>,
    /// Available UI textures for reference; not actually necessary for textures to work.
    pub textures: Vec<DRBTexture>,

    /// Raw STR block contents, preserved so stored string offsets stay valid.
    str_data: Vec<u8>,
    str_count: u32,
}

impl DRB {
    ///Returns the element group with the given name.
    pub fn get(&self, name: &str) -> Option<&Dlg> {
        self.dlgs.iter().find(|d| d.name == name)
    }

    ///Checks whether the reader starts with a valid DRB magic.
    pub fn is(reader: &mut BinaryReader) -> Result<bool, BinaryReaderError> {
        if reader.length() < 4 {
            return Ok(false);
        }
        let magic = reader.with_position(0, |r| r.read_ascii_fixed(4))?;
        Ok(magic == "DRB\0" || magic == "\0BRD")
    }

    ///Reads a DRB. The version can't be detected from the file, so the caller supplies it.
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let big_endian = reader.with_position(0, |r| r.read_ascii_fixed(4))? == "\0BRD";
        reader.big_endian = big_endian;

        read_null_block(reader, "DRB\0")?;

        //STR: kept as raw bytes. Offsets used by get_utf16 are relative to the start of its data.
        let (str_start, str_count, str_size) = read_block_header(reader, "STR\0")?;
        let str_data = reader.read_bytes(str_size as usize)?;

        //TEXI
        let textures = read_list_block(reader, "TEXI", |r| DRBTexture::from_reader(r, str_start))?;

        let shpr_start = read_blob_block(reader, "SHPR")?;
        let ctpr_start = read_blob_block(reader, "CTPR")?;
        let anip_bytes = read_blob_bytes(reader, "ANIP")?;
        let intp_bytes = read_blob_bytes(reader, "INTP")?;
        let scdp_start = read_blob_block(reader, "SCDP")?;

        //Keyed blocks map block-relative offset -> item. Items are removed as their
        // parent claims them, so ownership moves into the parent.
        let mut shapes: HashMap<u32, Shape> =
            read_keyed_block(reader, "SHAP", |r| Shape::from_reader(r, version, shpr_start, str_start))?;
        let mut controls: HashMap<u32, Control> =
            read_keyed_block(reader, "CTRL", |r| Control::from_reader(r, ctpr_start, str_start))?;

        //ANIK/ANIO/SCDK/SCDO are read on demand by their parents, which step into them.
        let anik_start = skip_block(reader, "ANIK")?;
        let anio_start = skip_block(reader, "ANIO")?;
        let anims = read_list_block(reader, "ANIM", |r| 
            Anim::from_reader(r, anio_start, anik_start, version)
        )?;
        let scdk_start = skip_block(reader, "SCDK")?;
        let scdo_start = skip_block(reader, "SCDO")?;
        let scdls = read_list_block(reader, "SCDL", |r| 
            Scdl::from_reader(r, scdo_start, scdk_start, scdp_start)
        )?;

        let mut dlgos: HashMap<u32, Dlgo> = read_keyed_block(reader, "DLGO", |r| 
            Dlgo::from_reader(r, &mut shapes, &mut controls)
        )?;
        let dlgs = read_list_block(reader, "DLG\0", |r| 
            Dlg::from_reader(r, &mut shapes, &mut controls, &mut dlgos)
        )?;

        read_null_block(reader, "END\0")?;

        Ok(Self {
            version,
            big_endian,
            textures,
            anip_bytes,
            intp_bytes,
            anims,
            scdls,
            dlgs,
            str_data,
            str_count,
        })
    }

    pub fn from_bytes(data: &[u8], version: DRBVersion) -> Result<Self, BinaryReaderError> {
        Ok(Self::from_reader(
            &mut BinaryReader::new(data, true, false),
            version
        )?)
    }

    pub fn from_file(mut path: &Path, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let mut data = std::fs::read(path)?;
        Ok(Self::from_bytes(&data, version)?)
    }

    pub unsafe fn read(path: &Path, version: DRBVersion) -> Result<(Self, DCXType), BinaryReaderError> {
        let data = std::fs::read(path)?;
        let (bytes, dt) = unsafe { Decompress::raw(&data) }?;
        Ok((Self::from_bytes(&bytes, version)?, dt))
    }
        
    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = self.big_endian;
        let version = self.version;

        write_null_block(writer, "DRB\0")?;

        //STR (verbatim)
        let start = write_block_header(writer, "STR\0")?;
        writer.write_bytes(&self.str_data)?;
        finish_block_header(writer, "STR\0", start, self.str_count)?;

        //TEXI
        let start = write_block_header(writer, "TEXI")?;
        for tex in &mut self.textures {
            tex.to_writer(writer)?;
        }
        finish_block_header(writer, "TEXI", start, self.textures.len() as u32)?;

        //SHPR: shape bodies
        let start = write_blob_block(writer, "SHPR")?;
        let mut shpr_offsets: VecDeque<u32> = VecDeque::new();
        for dlg in &mut self.dlgs {
            shpr_offsets.push_back((writer.position() - start) as u32);
            dlg.shape.write_data(writer, version)?;
            for dlgo in &mut dlg.dlgos {
                shpr_offsets.push_back((writer.position() - start) as u32);
                dlgo.shape.write_data(writer, version)?;
            }
        }
        finish_blob_block(writer, "SHPR", start)?;

        //CTPR: control bodies
        let start = write_blob_block(writer, "CTPR")?;
        let mut ctpr_offsets: VecDeque<u32> = VecDeque::new();
        for dlg in &mut self.dlgs {
            ctpr_offsets.push_back((writer.position() - start) as u32);
            dlg.control.write_data(writer)?;
            for dlgo in &mut dlg.dlgos {
                ctpr_offsets.push_back((writer.position() - start) as u32);
                dlgo.control.write_data(writer)?;
            }
        }
        finish_blob_block(writer, "CTPR", start)?;

        write_blob_bytes(writer, "ANIP", &self.anip_bytes)?;
        write_blob_bytes(writer, "INTP", &self.intp_bytes)?;

        //SCDP
        let start = write_blob_block(writer, "SCDP")?;
        let mut scdp_offsets: VecDeque<u32> = VecDeque::new();
        for scdl in &self.scdls {
            for scdo in &scdl.scdos {
                for scdk in &scdo.scdks {
                    scdp_offsets.push_back((writer.position() - start) as u32);
                    scdk.scdp_to_writer(writer)?;
                }
            }
        }
        finish_blob_block(writer, "SCDP", start)?;

        //SHAP: shape headers. Dlg and dlgo offsets go in separate queues because DLG and DLGO
        // blocks each consume their own.
        let start = write_block_header(writer, "SHAP")?;
        let mut dlg_shap_offsets: VecDeque<u32> = VecDeque::new();
        let mut dlgo_shap_offsets: VecDeque<u32> = VecDeque::new();
        for dlg in &mut self.dlgs {
            dlg_shap_offsets.push_back((writer.position() - start) as u32);
            dlg.shape.write_header(writer, shpr_offsets.pop_front().unwrap_or_default())?;
            for dlgo in &mut dlg.dlgos {
                dlgo_shap_offsets.push_back((writer.position() - start) as u32);
                dlgo.shape.write_header(writer, shpr_offsets.pop_front().unwrap_or_default())?;
            }
        }
        let count = (dlg_shap_offsets.len() + dlgo_shap_offsets.len()) as u32;
        finish_block_header(writer, "SHAP", start, count)?;

        //CTRL: control headers
        let start = write_block_header(writer, "CTRL")?;
        let mut dlg_ctrl_offsets: VecDeque<u32> = VecDeque::new();
        let mut dlgo_ctrl_offsets: VecDeque<u32> = VecDeque::new();
        for dlg in &mut self.dlgs {
            dlg_ctrl_offsets.push_back((writer.position() - start) as u32);
            dlg.control.write_header(writer, ctpr_offsets.pop_front().unwrap_or_default())?;
            for dlgo in &mut dlg.dlgos {
                dlgo_ctrl_offsets.push_back((writer.position() - start) as u32);
                dlgo.control.write_header(writer, ctpr_offsets.pop_front().unwrap_or_default())?;
            }
        }
        let count = (dlg_ctrl_offsets.len() + dlgo_ctrl_offsets.len()) as u32;
        finish_block_header(writer, "CTRL", start, count)?;

        //ANIK: one queued offset per anio, pointing at that anio's first anik.
        let start = write_block_header(writer, "ANIK")?;
        let mut count = 0u32;
        let mut anik_offsets: VecDeque<u32> = VecDeque::new();
        for anim in &mut self.anims {
            for anio in &mut anim.anios {
                anik_offsets.push_back((writer.position() - start) as u32);
                count += anio.aniks.len() as u32;
                for anik in &mut anio.aniks {
                    anik.to_writer(writer)?;
                }
            }
        }
        finish_block_header(writer, "ANIK", start, count)?;

        //ANIO: one queued offset per anim
        let start = write_block_header(writer, "ANIO")?;
        let mut count = 0u32;
        let mut anio_offsets: VecDeque<u32> = VecDeque::new();
        for anim in &self.anims {
            anio_offsets.push_back((writer.position() - start) as u32);
            count += anim.anios.len() as u32;
            for anio in &anim.anios {
                anio.to_writer(writer, anik_offsets.pop_front().unwrap_or_default())?;
            }
        }
        finish_block_header(writer, "ANIO", start, count)?;

        //ANIM
        let start = write_block_header(writer, "ANIM")?;
        for anim in &self.anims {
            anim.to_writer(writer, anio_offsets.pop_front().unwrap_or_default(), version)?;
        }
        finish_block_header(writer, "ANIM", start, self.anims.len() as u32)?;

        //SCDK: one queued offset per scdo.
        let start = write_block_header(writer, "SCDK")?;
        let mut count = 0u32;
        let mut scdk_offsets: VecDeque<u32> = VecDeque::new();
        for scdl in &self.scdls {
            for scdo in &scdl.scdos {
                scdk_offsets.push_back((writer.position() - start) as u32);
                count += scdo.scdks.len() as u32;
                for scdk in &scdo.scdks {
                    scdk.to_writer(writer, scdp_offsets.pop_front().unwrap_or_default())?;
                }
            }
        }
        finish_block_header(writer, "SCDK", start, count)?;

        //SCDO: one queued offset per scdl.
        let start = write_block_header(writer, "SCDO")?;
        let mut count = 0u32;
        let mut scdo_offsets: VecDeque<u32> = VecDeque::new();
        for scdl in &self.scdls {
            scdo_offsets.push_back((writer.position() - start) as u32);
            count += scdl.scdos.len() as u32;
            for scdo in &scdl.scdos {
                scdo.to_writer(writer, scdk_offsets.pop_front().unwrap_or_default())?;
            }
        }
        finish_block_header(writer, "SCDO", start, count)?;

        //SCDL
        let start = write_block_header(writer, "SCDL")?;
        for scdl in &self.scdls {
            scdl.to_writer(writer, scdo_offsets.pop_front().unwrap_or_default())?;
        }
        finish_block_header(writer, "SCDL", start, self.scdls.len() as u32)?;

        //DLGO: one queued offset per dlg, pointing at its first child.
        let start = write_block_header(writer, "DLGO")?;
        let mut count = 0u32;
        let mut dlgo_offsets: VecDeque<u32> = VecDeque::new();
        for dlg in &self.dlgs {
            dlgo_offsets.push_back((writer.position() - start) as u32);
            count += dlg.dlgos.len() as u32;
            for dlgo in &dlg.dlgos {
                dlgo.to_writer(writer, &mut dlgo_shap_offsets, &mut dlgo_ctrl_offsets)?;
            }
        }
        finish_block_header(writer, "DLGO", start, count)?;

        //DLG
        let start = write_block_header(writer, "DLG\0")?;
        for dlg in &self.dlgs {
            dlg.to_writer(writer, &mut dlg_shap_offsets, &mut dlg_ctrl_offsets, &mut dlgo_offsets)?;
        }
        finish_block_header(writer, "DLG\0", start, self.dlgs.len() as u32)?;

        write_null_block(writer, "END\0")?;
        Ok(())
    }

    pub fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();
        self.to_writer(&mut writer)?;
        Ok(writer)
    }

    pub fn to_bytes(&mut self) -> Result<Vec<u8>, BinaryWriterError> {
        Ok(self.into_writer()?.into_inner())
    }

    ///Writes self as bytes to a specified path, raises error if the file already exists.
    pub fn to_file(&mut self, path: &Path) -> Result<(), BinaryWriterError> {
        let bytes = self.to_bytes()?;
        let mut file = File::create_new(path)?;

        file.write_all(&bytes)?;

        Ok(())
    }

    ///Similar to `to_file` but compresses the output with given `CompressSettings`.
    pub unsafe fn write(&mut self, path: &Path, settings: CompressSettings) -> Result<(), BinaryWriterError> {
        let bytes = self.to_bytes()?;
        let compressed = unsafe { Compress::raw(&bytes, &settings) }?;

        let mut file = File::create_new(path)?;
        file.write_all(&compressed)?;

        Ok(())
    }
}

impl std::fmt::Display for DRB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DRB[Version: {:?} | BE: {:?} Anims: {} | Scdls: {} | Dlgs: {}, Textures: {}]", 
            self.version, self.big_endian, self.anims.len(), self.scdls.len(), self.dlgs.len(), self.textures.len()
        );
        Ok(())
    }
}

//-.-.-.--.-.-.-.-.-.-.-..-.-.-.-..-.-.-.-..-.-
// Utilities/helpers for blocks
//-.-.-.--.-.-.-.-.-.-.-..-.-.-.-..-.-.-.-..-.-

fn fourcc_to_i32(fourcc: &str) -> i32 {
    let b = fourcc.as_bytes();
    (b[0] as i32) | (b[1] as i32) << 8 | (b[2] as i32) << 16 | (b[3] as i32) << 24
}

fn read_null_block(reader: &mut BinaryReader, name: &str) -> Result<(), BinaryReaderError> {
    reader.assert::<i32>(fourcc_to_i32(name))?;
    reader.assert::<i32>(0)?;
    reader.assert::<i32>(0)?;
    reader.assert::<i32>(0)?;
    Ok(())
}

fn write_null_block(writer: &mut BinaryWriter, name: &str) -> Result<(), BinaryWriterError> {
    writer.write_i32(fourcc_to_i32(name))?;
    writer.write_i32(0)?;
    writer.write_i32(0)?;
    writer.write_i32(0)
}

///Returns the start of the blob's data and skips past it.
fn read_blob_block(reader: &mut BinaryReader, name: &str) -> Result<u64, BinaryReaderError> {
    reader.assert::<i32>(fourcc_to_i32(name))?;
    let size = reader.read_i32()?;
    reader.assert::<i32>(1)?;
    reader.assert::<i32>(0)?;
    let start = reader.position();
    reader.skip(size as u64)?;
    Ok(start)
}

fn write_blob_block(writer: &mut BinaryWriter, name: &str) -> Result<u64, BinaryWriterError> {
    writer.write_i32(fourcc_to_i32(name))?;
    writer.reserve::<i32>(&format!("BlobBlockSize{name}"))?;
    writer.write_i32(1)?;
    writer.write_i32(0)?;
    Ok(writer.position())
}

fn finish_blob_block(writer: &mut BinaryWriter, name: &str, start: u64) -> Result<(), BinaryWriterError> {
    writer.pad(0x10)?;
    writer.fill::<i32>(&format!("BlobBlockSize{name}"), (writer.position() - start) as i32)
}

fn read_blob_bytes(reader: &mut BinaryReader, name: &str) -> Result<Vec<u8>, BinaryReaderError> {
    reader.assert::<i32>(fourcc_to_i32(name))?;
    let size = reader.read_i32()?;
    reader.assert::<i32>(1)?;
    reader.assert::<i32>(0)?;
    reader.read_bytes(size as usize)
}

fn write_blob_bytes(writer: &mut BinaryWriter, name: &str, bytes: &[u8]) -> Result<(), BinaryWriterError> {
    writer.write_i32(fourcc_to_i32(name))?;
    writer.reserve::<i32>("BlobSize")?;
    writer.write_i32(1)?;
    writer.write_i32(0)?;
    let start = writer.position();
    writer.write_bytes(bytes)?;
    writer.pad(0x10)?;
    writer.fill::<i32>("BlobSize", (writer.position() - start) as i32)
}

///Returns (data start, count, size).
fn read_block_header(reader: &mut BinaryReader, name: &str) -> Result<(u64, u32, u32), BinaryReaderError> {
    reader.assert::<i32>(fourcc_to_i32(name))?;
    let size = reader.read_u32()?;
    let count = reader.read_u32()?;
    reader.assert::<i32>(0)?;
    Ok((reader.position(), count, size))
}

fn write_block_header(writer: &mut BinaryWriter, name: &str) -> Result<u64, BinaryWriterError> {
    writer.write_i32(fourcc_to_i32(name))?;
    writer.reserve::<i32>(&format!("BlockSize{name}"))?;
    writer.reserve::<i32>(&format!("BlockCount{name}"))?;
    writer.write_i32(0)?;
    Ok(writer.position())
}

fn finish_block_header(writer: &mut BinaryWriter, name: &str, start: u64, count: u32) -> Result<(), BinaryWriterError> {
    writer.pad(0x10)?;
    writer.fill::<i32>(&format!("BlockSize{name}"), (writer.position() - start) as i32)?;
    writer.fill::<i32>(&format!("BlockCount{name}"), count as i32)
}

///Reads a block whose items are referenced by block-relative offset.
fn read_keyed_block<T>(
    reader: &mut BinaryReader,
    name: &str,
    mut read_item: impl FnMut(&mut BinaryReader) -> Result<T, BinaryReaderError>,
) -> Result<HashMap<u32, T>, BinaryReaderError> {
    let (start, count, size) = read_block_header(reader, name)?;
    let mut map = HashMap::with_capacity(count as usize);
    for _ in 0..count {
        let offset = (reader.position() - start) as u32;
        map.insert(offset, read_item(reader)?);
    }
    reader.set_position(start + size as u64);
    Ok(map)
}

///Reads a block whose items are only consumed in order.
fn read_list_block<T>(
    reader: &mut BinaryReader,
    name: &str,
    mut read_item: impl FnMut(&mut BinaryReader) -> Result<T, BinaryReaderError>,
) -> Result<Vec<T>, BinaryReaderError> {
    let (start, count, size) = read_block_header(reader, name)?;
    let mut list = Vec::with_capacity(count as usize);
    for _ in 0..count {
        list.push(read_item(reader)?);
    }
    reader.set_position(start + size as u64);
    Ok(list)
}

///Records a block's data start and skips past it. Its parents read items by stepping in.
fn skip_block(reader: &mut BinaryReader, name: &str) -> Result<u64, BinaryReaderError> {
    let (start, _count, size) = read_block_header(reader, name)?;
    reader.set_position(start + size as u64);
    Ok(start)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "IO"]
    fn test_drb() {
        let (drb, dt) = unsafe { 
            DRB::read(
                Path::new("tests/ds1/menu.drb.dcx"),
                DRBVersion::DSR
            )
        }.unwrap();

        println!("{}", drb);
    }
}