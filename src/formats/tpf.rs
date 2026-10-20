use std::io::Write;
use std::path::Path;

use crate::binary::{BinaryReader, BinaryWriter, IO, bytes::ByteOrder};
use crate::binders::bnd4::BND4Header;
use crate::binders::{Binder, BinderEntry, BinderVersion, MetaBinder};
use crate::dcx::{Compress, CompressSettings, DCXType};
use crate::dcx::{DCXType::DCP_EDGE, Decompress};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};
use crate::oodle::structs::OodleSettings;
use crate::textures::dds::{DDS, DDSCAPS2, ImageExportFormat, ResourceMisc};
use crate::textures::headerizer;
use crate::textures::dxgi::DxgiFormat;


#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TPFPlatform {
    PC = 0,
    Xbox360 = 1,
    PS3 = 2,
    PS4 = 4,
    XboxOne = 5,
    PS5 = 8, // Virtuos Games, Switch enum. Technically 4, but conflicts with From Software's PS4 enum.
    Switch = 67 // TPFTexture metadata is same as PC; textures are headered .xtx

}

impl TPFPlatform {
    pub fn from_u8(v: u8) -> Result<Self, BinaryReaderError> {
        Ok(match v {
            0 => TPFPlatform::PC,
            1 => TPFPlatform::Xbox360,
            2 => TPFPlatform::PS3,
            4 => TPFPlatform::PS4,
            5 => TPFPlatform::XboxOne,
            8 => TPFPlatform::PS5,
            67 => TPFPlatform::Switch,
            _ => return Err(BinaryReaderError::Custom(format!("Unknown TPF platform: {v}"))),
        })
    }
 
    fn is_big_endian(self) -> bool {
        matches!(self, TPFPlatform::Xbox360 | TPFPlatform::PS3)
    }
 
    /// PS4, Xbone, PS5: DX10-style console metadata.
    fn is_dx10_console(self) -> bool {
        matches!(self, TPFPlatform::PS4 | TPFPlatform::XboxOne | TPFPlatform::PS5)
    }
 
    /// Platforms whose textures carry a `TPFTextureHeader`.
    fn has_tex_header(self) -> bool {
        self != TPFPlatform::PC && self != TPFPlatform::Switch
    }
}


#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TextureType {
    TPFTexture = 0,  // one 2D texture
    Cubemap = 1, // six 2D textures
    Volume = 2,  // one 3D texture
    TextureArray = 3 // multiple standard textures 
}

impl TextureType {
    pub fn from_u8(int: u8) -> Result<Self, BinaryReaderError> {
        match int {
            0 => Ok(Self::TPFTexture),
            1 => Ok(Self::Cubemap),
            2 => Ok(Self::Volume),
            3 => Ok(Self::TextureArray),
            t => Err(BinaryReaderError::Custom(format!("Invalid TextureType: {}", t)))
        }
    }

    pub fn to_u8(&self) -> Result<u8, BinaryWriterError> {
        match self {
            Self::TPFTexture => Ok(0),
            Self::Cubemap => Ok(1),
            Self::Volume => Ok(2),
            Self::TextureArray => Ok(3),
            t => Err(BinaryWriterError::Custom(format!("Invalid TextureType: {:?}", t)))
        }
    }
}


#[derive(Debug, Clone, Default)]
pub struct FloatStruct {
    /// Unknown; probably some kind of ID.
    pub unk00: i32,
    /// Unknown; not confirmed to always be floats.
    pub values: Vec<f32>,
}
 
impl FloatStruct {
    fn read(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
        let unk00 = reader.read_i32()?;
        let length = reader.read_i32()?;
        if length < 0 || length % 4 != 0 {
            return Err(FerrisoulsError::Custom(format!("Unexpected FloatStruct length: {length}")));
        }
        let mut vals = Vec::<f32>::new();
        for _ in 0..(length/4) {
            vals.push(reader.read_f32()?)
        }
        Ok(FloatStruct { unk00, values: vals })
    }
 
    fn write(&self, writer: &mut BinaryWriter) {
        writer.write_i32(self.unk00);
        writer.write_i32(self.values.len() as i32 * 4);
        for i in &self.values {
            writer.write_f32(i.clone());
        }
    }
}


#[derive(Debug, Clone, Copy, Default)]
pub struct TPFTextureHeader {
    pub width: i32,
    pub height: i32,
    pub texture_count: i32,
    unk1: u16, // PS3 only
    unk2: u16, // 0x0 or 0xAAE4 in DeS, 0xD in BB/DS3 (console)
    pub remap: i32, // A value for remapping color channel order on PS3.
    pub dxgi_format: DxgiFormat, //default to Unknown 
}


#[derive(Debug)]
pub struct TPFTexture {
    pub name: String,
    pub format: u8, //default 1
    pub texture_type: TextureType,
    pub mipmap_count: u8, //default 0
    pub texture_flags: u8, // 2/3 = DCX-compressed. Others unknown

    pub data: Vec<u8>,

    pub header: Option<TPFTextureHeader>,
    pub platform: TPFPlatform, // platform stored in parent TPF, but is available for Textures for QoL - Grimrukh

    float_struct: Option<FloatStruct>
}

impl BinderEntry for TPFTexture {
    type Identifier = String;

    fn identity(&self) -> &Self::Identifier {
        &self.name
    }
}

impl TPFTexture {
    pub fn new(platform: TPFPlatform) -> Self {
        Self {
            name: "Unnamed".to_string(),
            platform,
            format: 1,
            texture_type: TextureType::TPFTexture,
            mipmap_count: 0,
            texture_flags: 0,
            data: Vec::new(),
            header: None,
            float_struct: None,
        }
    }

    ///Create a new TPFTexture with the specified information; cubemap and mipmaps are determined
    /// based on bytes. Assumes the input is a standard PC .dds file or a Dark Souls Remastered PS4 .gnf
    pub fn from_dds(name: &str, format: u8, flags1: u8, bytes: Vec<u8>, platform: TPFPlatform) -> Result<Self, FerrisoulsError> {
        let mut tex = TPFTexture::new(platform);
        tex.name = name.to_string();
        tex.format = format;
        tex.texture_flags = flags1;
 
        let mut dds = DDS::from_bytes(&bytes)?;
        tex.texture_type = if dds.header.caps2.contains(DDSCAPS2::Cubemap) {
            TextureType::Cubemap
        } else if dds.header.caps2.contains(DDSCAPS2::Volume) {
            TextureType::Volume
        } else if dds.header.depth > 1 {
            TextureType::TextureArray
        } else {
            TextureType::TPFTexture
        };
 
        let is_gnf = bytes.get(0..4) == Some(b"GNF ".as_slice());
        if platform == TPFPlatform::PC || platform == TPFPlatform::Switch || is_gnf {
            tex.data = bytes;
            return Ok(tex);
        }
 
        let mut header = TPFTextureHeader {
            dxgi_format: dds.dxgi_format(),
            width: dds.header.width as i32,
            height: dds.header.height as i32,
            ..Default::default()
        };
        header.texture_count = match tex.texture_type {
            TextureType::TPFTexture => 1,
            TextureType::Cubemap => 6,
            TextureType::Volume | TextureType::TextureArray => dds.header.depth,
        };
 
        let mut images = headerizer::get_dds_texture_buffers(&mut dds, &bytes)?;
 
        // Set this here in case we need to adjust it
        tex.mipmap_count = dds.header.mipmap_count as u8;
        match platform {
            TPFPlatform::Xbox360 => return Err(FerrisoulsError::custom("Not Implemented!")),
            TPFPlatform::XboxOne => return Err(FerrisoulsError::custom("Not Implemented!")),
            TPFPlatform::PS3 => {
                tex.data = headerizer::Write::PS3(&images, &dds, format)?;
            }
            TPFPlatform::PS4 => {
                tex.data = headerizer::Write::PS4(&mut images, &dds, tex.texture_type)?;
                header.unk2 = 0xD;
            }
            TPFPlatform::PS5 => return Err(FerrisoulsError::custom("Not Implemented!")),
            _ => {}
        }
        tex.header = Some(header);
        Ok(tex)
    }

    ///Returns `DDS` object built from headerized self.
    pub fn to_dds(&self) -> Result<DDS, FerrisoulsError> {
        let (bytes, _) = self.headerize()?;
        let mut dds = DDS::from_bytes(&bytes)?;

        Ok(dds)
    }

    ///Returns RGBA8 bytes and texture dimensions. Only the top mip of the first image is decoded.
    pub fn to_rgba(&self) -> Result<(Vec<u8>, (usize, usize)), FerrisoulsError> {
        let dds = self.to_dds()?;
        dds.to_rgba()
    }

    pub fn to_png(&self) -> Result<Vec<u8>, FerrisoulsError> {
        let dds = self.to_dds()?;
        dds.to_png()
    }

    pub fn to_tga(&self) -> Result<Vec<u8>, FerrisoulsError> {
        let dds = self.to_dds()?;
        dds.to_tga()
    }

    ///Writes self texture to a specified path.
    pub fn save_as(&self, path: &Path, format: ImageExportFormat) -> Result<(), FerrisoulsError> {
        let mut dds = self.to_dds()?;
        dds.save_as(path, format)
    }

    ///Replaces self texture data with a given DDS.
    ///
    /// The DDS must contain texture data and must be of the same DXGI format as `self`. The texture is rebuilt for
    /// its platform, so PC and Switch keep a headered DDS and the console platforms are swizzled and get theirs updated.
    pub fn replace(&mut self, mut dds: DDS) -> Result<(), FerrisoulsError> {
        if dds.data.is_empty() {
            return Err(BinaryReaderError::invalid_data("Given DDS has no data.").into());
        }

        let dds_format = dds.dxgi_format();
        let format = DxgiFormat::from_internal(self.format);
        if dds_format != format {
            return Err(BinaryReaderError::InvalidData(format!(
                "DDS of format `{:?}` was expected to be: {:?}",
                dds_format, format
            )).into());
        }

        let rebuilt = Self::from_dds(&self.name, self.format, self.texture_flags, dds.to_bytes()?, self.platform)?;

        self.data = rebuilt.data;
        self.header = rebuilt.header;
        self.texture_type = rebuilt.texture_type;
        self.mipmap_count = rebuilt.mipmap_count;
        Ok(())
    }
    
    ///Crops self texture in place by creating a new DDS from the given dimensions and calling `Self::replace` with it.
    pub fn crop(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), FerrisoulsError> {
        let cropped = self.to_dds()?
            .crop(x, y, width, height)?;
        self.replace(cropped)
    }

    /// ## Safety:
    /// 
    ///Decompressor is unsafe due to calling oodle for KRAK. TPF entries are always DCP_EDGE and thus will never call unsafe code.
    pub fn read(reader: &mut BinaryReader, platform: TPFPlatform, tpf_flags: u8, encoding: u8) -> Result<Self, BinaryReaderError> {
        let file_offset = reader.read_u32()?;
        let file_size = reader.read_i32()?;

        let format = reader.read_u8()?; // 
        let texture_type = TextureType::from_u8(reader.read_u8()?)?;
        let mipmap_count = reader.read_u8()?;
        let texture_flags = reader.matches(|r| r.read_u8(), &[0, 1, 2, 3, 0x80])?;
 
        let mut header: Option<TPFTextureHeader> = None;
        if platform.has_tex_header() {
            let mut h: TPFTextureHeader = TPFTextureHeader {
                width: reader.read_i16()? as i32,
                height: reader.read_i16()? as i32,
                ..Default::default()
            };
 
            h.dxgi_format = DxgiFormat::from_internal(format);
 
            if platform == TPFPlatform::Xbox360 {
                reader.assert::<i32>(0)?;
            } else if platform == TPFPlatform::PS3 {
                h.unk1 = reader.read_i32()? as u16;
                if tpf_flags != 0 {
                    h.remap = reader.read_i32()?;
                }
            } else if platform.is_dx10_console() {
                h.texture_count = reader.read_i32()?;
                h.unk2 = reader.matches(|r| r.read_i32(), &[0, 0x9, 0xD])? as u16;
            }

            header = Some(h)
        }
 
        let name_offset = reader.read_u32()?;
        // Formerly 'Flags2', as seen in Yabber
        let has_float_struct = reader.matches(|r| r.read_i32(), &[0, 1])? == 1;
 
        if platform.is_dx10_console() {
            if let Some(header) = header.as_mut() {
                header.dxgi_format = DxgiFormat::from_id(reader.read_i32()? as u8)
                    .unwrap_or_default();
            }
        }
 
        let float_struct = if has_float_struct { 
            Some(FloatStruct::read(reader)
                .map_err(|e| BinaryReaderError::custom(e.to_string()))?
            ) 
        } else { None };
 
        let mut bytes = reader.get_bytes(file_offset as u64, file_size as usize)?;
        if texture_flags == 2 || texture_flags == 3 {
            let (decompressed, kind) = unsafe { 
                Decompress::raw(&bytes)
                    .map_err(|e| BinaryReaderError::custom(e.to_string()))?
            };
            if kind != DCP_EDGE {
                return Err(BinaryReaderError::InvalidData(format!(
                    "TPF compression is expected to be DCP_EDGE, but it was {kind:?}"
                )));
            }
            bytes = decompressed;
        }
 
        // Cubemap fix
        // Check if this is a DX10 FourCC, check if it's a cubemap.
        // FromSoft erroneously sets the image count for DX10 cubemaps to 6, which causes editors to
        // think there's an array of cubemaps instead of just 6 images and break.
        if platform == TPFPlatform::PC
            && bytes.len() > 0x8C
            && bytes[0x56] == 0x31
            && bytes[0x57] == 0x30
            && bytes[0x54] == 0x44
            && bytes[0x55] == 0x58
            && bytes[0x88] as u32 == ResourceMisc::TEXTURECUBE.bits()
            && bytes[0x8C] == 0x6
        {
            bytes[0x8C] = 0x1;
        }
 
        let name = match encoding {
            1 => reader.get_utf16(name_offset as u64)?,
            0 | 2 => reader.get_shift_jis(name_offset as u64)?,
            _ => String::new(), // C# leaves Name null; unreachable after the header assert
        };
 
        Ok(
            Self { 
                name, 
                platform, 
                format, 
                texture_type, 
                mipmap_count, 
                texture_flags, 
                data: bytes, 
                header, 
                float_struct 
            }
        )
    }
 
    pub fn write_header(&mut self, writer: &mut BinaryWriter, index: usize, platform: TPFPlatform, tpf_flags: u8) -> Result<(), BinaryWriterError> {
        if platform == TPFPlatform::PC {
            let dds = DDS::from_bytes(&self.data)
                .map_err(|e| BinaryWriterError::custom(e.to_string()))?;

            self.texture_type = if dds.header.caps2.contains(DDSCAPS2::Cubemap) {
                TextureType::Cubemap
            } else if dds.header.caps2.contains(DDSCAPS2::Volume) {
                TextureType::Volume
            } else {
                TextureType::TPFTexture
            };
            self.mipmap_count = dds.header.mipmap_count as u8;
        }
 
        writer.reserve::<u32>(&format!("FileData{index}"));
        writer.reserve::<i32>(&format!("FileSize{index}"));
 
        writer.write_u8(self.format);
        writer.write_u8(self.texture_type.to_u8()?);
        writer.write_u8(self.mipmap_count);
        writer.write_u8(self.texture_flags as u8);
 
        if platform.has_tex_header() {
            let header = self
                .header
                .as_ref()
                .ok_or_else(|| BinaryWriterError::Custom(format!("TPFTexture {} is missing its header", self.name)))?;
            writer.write_i16(header.width as i16);
            writer.write_i16(header.height as i16);
 
            if platform == TPFPlatform::Xbox360 {
                writer.write_i32(0);
            } else if platform == TPFPlatform::PS3 {
                writer.write_i32(header.unk1 as i32);
                if tpf_flags != 0 {
                    writer.write_i32(header.remap);
                }
            } else if platform.is_dx10_console() {
                writer.write_i32(header.texture_count);
                writer.write_i32(header.unk2 as i32);
            }
        }
 
        writer.reserve::<u32>(&format!("FileName{index}"));
        // Formerly 'Flags2', as seen in Yabber
        writer.write_i32(if self.float_struct.is_none() { 0 } else { 1 });
 
        // Note: the reader also reads this for PS5, but the original writer does not write it.
        if platform == TPFPlatform::PS4 || platform == TPFPlatform::XboxOne {
            if let Some(header) = &self.header {
                writer.write_i32(self.format as i32);
            }
        }
 
        if let Some(fs) = &self.float_struct {
            fs.write(writer);
        }
        Ok(())
    }
 
    pub fn write_name(&self, writer: &mut BinaryWriter, index: usize, encoding: u8) {
        writer.fill(&format!("FileName{index}"), writer.position() as u32);
        if encoding == 1 {
            writer.write_utf16(&self.name, true);
        } else if encoding == 0 || encoding == 2 {
            writer.write_shift_jis(&self.name, true);
        }
    }
 
    ///Returns the final size of the TPFTexture that was written.
    /// 
    /// ## Safety:
    /// 
    ///Compressor is unsafe due to calling oodle for KRAK. TPF entries are always DCP_EDGE and thus will never call unsafe code.
    pub fn write_data(&self, writer: &mut BinaryWriter, settings: CompressSettings, index: usize) -> Result<i32, BinaryWriterError> {
        writer.fill(&format!("FileData{index}"), writer.position() as u32);
 
        let compressed;
        let bytes: &[u8] = if self.texture_flags == 2 || self.texture_flags == 3 {
            compressed = unsafe { 
                Compress::raw(&self.data, &settings)
                    .map_err(|e| BinaryWriterError::custom(e.to_string()))? 
            };
            &compressed
        } else {
            &self.data
        };
 
        writer.fill(&format!("FileSize{index}"), bytes.len() as i32);
        writer.write_bytes(bytes);
 
        Ok(bytes.len() as i32)
    }
 
    ///Attempt to create a full DDS file from headerless console textures. Returns the file bytes
    /// and the appropriate extension (".dds", ".gnf" or ".xtx").
    pub fn headerize(&self) -> Result<(Vec<u8>, String), FerrisoulsError> {
        //returns data unchanged if it starts with a valid header.
        headerizer::headerize(self)
    }

}
 
impl std::fmt::Display for TPFTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size = match self.header {
            Some(h) => format!("{}x{}", h.width, h.height),
            None => "Headerless".to_string()
        };
        write!(f, "{:?}[{} | {} | Format: {} | Size: {}]\n",
            self.texture_type, self.name, size, self.format, self.data.len()
        )
    }
}


#[derive(Debug)]
pub struct TPF {
    _magic: [u8;4], // asserted b"TPF\0"

    _data_size: usize,
    file_count: usize,

    platform: TPFPlatform,
    tpf_flags: u8, // asserted [0, 1, 2, 3]
    encoding: u8, // asserted [0, 1, 2] where: 2 == UTF-16, 0/1 == shift_jis_2004

    _pad1: u8, // b'\0'

    textures: Vec<TPFTexture>,
}

impl Default for TPF {
    /// Creates an empty TPF configured for DS3.
    fn default() -> Self {
        Self { 
            _magic: *b"TPF\0",
            _data_size: 0,
            file_count: 0,
            textures: Vec::new(), 
            platform: TPFPlatform::PC,
            encoding: 1, 
            tpf_flags: 3,
            _pad1: b'\0'
        }
    }
}

impl TPF {
    pub fn new(platform: TPFPlatform, flags: u8, encoding: u8, textures: Vec<TPFTexture>) -> Self {
        Self { 
            _magic: *b"TPF\0",
            _data_size: 0,
            file_count: textures.len(),
            textures,
            platform,
            encoding,
            tpf_flags: flags,
            _pad1: b'\0'
        }
    }

    pub fn is(reader: &mut BinaryReader) -> bool {
        if reader.length() < 4 {
            return false;
        }
        reader.read_ascii_fixed(4).map(|m| m == "TPF\0").unwrap_or(false)
    }
}

impl IO for TPF {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.big_endian = false;
        reader.assert_bytes(b"TPF\0")?;
        let mut platform = TPFPlatform::from_u8(reader.get::<u8>(0xC)?)?;
        reader.big_endian = platform.is_big_endian();
 
        reader.read_i32()?; // Data length
        let file_count = reader.read_i32()?;
        reader.skip(1); // Platform
        let flags = reader.matches(|r| r.read_u8(), &[0, 1, 2, 3])?;
        let encoding = reader.matches(|r| r.read_u8(), &[0, 1, 2])?;
        reader.assert::<u8>(0)?;
 
        // We have to check if this is a Switch tpf because Virtuos used the PS4 enum
        if platform == TPFPlatform::PS4 && reader.length() >= 0x28 {
            // On PS4, this will always be the Unk2 texture metadata area, which we've only observed
            // with values 0, 0x9, and 0xD, so we can take the highest, 0xD to compare against.
            // On Switch, this will ALWAYS be either the pointer to the next texture or, in the case
            // of a single texture, the texture name string. String values will never be this low.
            let saved = reader.position();
            reader.set_position(0x24);
            let v = reader.read_i32()?;
            reader.set_position(saved);
            if v > 0xD {
                platform = TPFPlatform::Switch;
            }
        }
 
        let mut textures = Vec::with_capacity(file_count.max(0) as usize);
        for _ in 0..file_count {
            textures.push(TPFTexture::read(reader, platform, flags, encoding)?);
        }
        Ok(TPF::new(platform, flags, encoding, textures))
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        let platform = self.platform;
        writer.big_endian = platform.is_big_endian();
        writer.write_ascii("TPF\0", false);
        writer.reserve::<i32>("DataSize");
        writer.write_i32(self.textures.len() as i32);
        writer.write_u8(if platform == TPFPlatform::Switch { 4 } else { platform as u8 });
        writer.write_u8(self.tpf_flags);
        writer.write_u8(self.encoding);
        writer.write_u8(0);
 
        let tpf_flags = self.tpf_flags;
        for (i, tex) in self.textures.iter_mut().enumerate() {
            tex.write_header(writer, i, platform, tpf_flags)?;
        }
 
        for (i, tex) in self.textures.iter().enumerate() {
            tex.write_name(writer, i, self.encoding);
        }
 
        let mut texture_padding_size = 0x4;
        if platform == TPFPlatform::PS3 {
            writer.pad(0x100);
            texture_padding_size = 0x80;
        } else if platform == TPFPlatform::PS4 {
            writer.pad(0x10);
        }
 
        let data_start = writer.position();
        let mut texture_data_size: i64 = 0;
        for (i, tex) in self.textures.iter().enumerate() {
            // Padding for texture data varies wildly across games, so don't worry about this too much
            if !tex.data.is_empty() {
                writer.pad(texture_padding_size);
            }
 
            texture_data_size += tex.write_data(
                writer, 
                CompressSettings::DCPEDGE,
                i
            )? as i64;
        }
        if platform == TPFPlatform::PS3 {
            writer.pad(texture_padding_size);
            writer.fill("DataSize", texture_data_size as i32);
        } else {
            writer.fill("DataSize", (writer.position() - data_start) as i32);
        }
        Ok(())
    }
}

impl Binder for TPF {
    const VERSION: BinderVersion = BinderVersion::VARIABLE; // can be 3 or 4. TODO: is this correct?
    type Entry = TPFTexture;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.textures
    }
}

impl<'a> IntoIterator for &'a TPF {
    type Item = &'a TPFTexture;
    type IntoIter = std::slice::Iter<'a, TPFTexture>;
 
    fn into_iter(self) -> Self::IntoIter {
        self.textures.iter()
    }
}

impl std::fmt::Display for TPF {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TPF[Platform: {:?} | Flags: {:?} | Encoding: {:?}] {{\n", 
            self.platform, self.tpf_flags, self.encoding
        );
        for t in self.into_iter() {
            write!(f, "    ");
            t.fmt(f);
        }
        write!(f, "}}")
    }
}



#[cfg(test)]
mod tests {
    use std::path::Path;

use super::*;

    #[test]
    fn platform_byte_order() {
        assert_eq!(TPFPlatform::PC.is_big_endian(), false);
        assert_eq!(TPFPlatform::PS3.is_big_endian(), true);
    }

    #[test]
    fn write_texture() {
        unsafe {crate::oodle::core::oodle_auto()};

        let (tpf, dt) = unsafe {
            TPF::unpack(Path::new("tests/common.tpf.dcx")).unwrap()
        };

        tpf.textures[0].save_as(Path::new("C:/Users/lstr/Downloads/test.dds"), ImageExportFormat::DDS).unwrap();
        tpf.textures[0].save_as(Path::new("C:/Users/lstr/Downloads/test.png"), ImageExportFormat::PNG).unwrap();
    }

    #[test]
    fn inspct_tpf() {
        unsafe {crate::oodle::core::oodle_auto()};

        let (tpf, dt) = unsafe {
            TPF::unpack(Path::new("tests/common.tpf.dcx")).unwrap()
        };

        println!("{}", tpf);
        /*
        TPF[Platform: PC | Flags: 3 | Encoding: 1] {
            TPFTexture[MENU_BG_Base1 | Headerless | Format: 102 | Size: 1048724]
            TPFTexture[MENU_BG_Base2 | Headerless | Format: 102 | Size: 524436]
            TPFTexture[MENU_BG_KeyGuide1 | Headerless | Format: 102 | Size: 524436]
           ...
        }
         */

        let mut dds = tpf.textures[0].to_dds().unwrap();

        dds.to_file(Path::new("C:/Users/lstr/Downloads/test.dds")).unwrap(); // export dds for inspection.

        println!("{:?}", dds)
        /*
        DDS[Dimensions: 1024x1024 | DCXType: Null | Size: 1048576] {
            DX10: true
            Flags: DDSD(CAPS | HEIGHT | WIDTH | PIXELFORMAT | LINEARSIZE)
            FourCC: [68, 88, 49, 48]
            CAPS: DDSCAPS(TEXTURE)
            CAPS2: DDSCAPS2(0x0)
        }
         */
    }

}

