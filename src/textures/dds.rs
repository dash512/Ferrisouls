use std::fs::File;
use std::path::Path;
use std::{io, error::Error, ops::BitOrAssign};
use std::io::{Error as IOError, ErrorKind, Read, Write};
use bitflags::bitflags;
pub use other_enums::*;
use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::dcx::DCXType;
use crate::errors::{BinaryReaderError, BinaryWriterError, DCXError, FerrisoulsError};
use crate::textures::swizzling::FormatLayout;
use crate::textures::dxgi::{DxgiFormat, bc::{self, Bc}};

#[derive(Debug)]
pub enum ImageExportFormat {
    DDS,
    PNG,
    TGA
}


mod other_enums {
    use super::*;

    bitflags! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct DDSD: u32 {
            const CAPS = 0x1; // always required
            const HEIGHT = 0x2; // always required
            const WIDTH = 0x4; // always required
            const PITCH = 0x8; // required when pitch is provided for an uncompressed texture
            const PIXELFORMAT = 0x1000; // always required
            const MIPMAPCOUNT = 0x20000; // required in a mipmapped texture
            const LINEARSIZE = 0x80000; // required when pitch is provided for a compressed texture
            const DEPTH = 0x800000; // required in a depth texture
        }
        
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct DDSCAPS: u32 {
            const COMPLEX = 0x8;
            const TEXTURE = 0x1000;
            const MIPMAP = 0x400000;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct  DDSCAPS2: u32 {
            const Cubemap = 0x200;
            const CubemapPositiveX = 0x400;
            const CubemapNegativeX = 0x800;
            const CubemapPositiveY = 0x1000;
            const CubemapNegativeY = 0x2000;
            const CubemapPositiveZ = 0x4000;
            const CubemapNegativeZ = 0x8000;
            const Volume = 0x200000;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct DDSPF: u32 {
            const ALPHAPIXELS = 0x1;
            const ALPHA = 0x2;
            const FOURCC = 0x4;
            const RGB = 0x40;
            const YUV = 0x200;
            const LUMINANCE = 0x20000;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ResourceMisc: u32 {
            const TEXTURECUBE = 0x4;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct D3D10ResourceDimension: u32 {
            const TEXTURE1D = 2;
            const TEXTURE2D = 3;
            const TEXTURE3D = 4;
        }

        /// Fills a formerly-reserved `miscFlags2` field in `DX10Header`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct AlphaMode: u32 {
            const UNKNOWN = 0;
            const STRAIGHT = 1;
            const PREMULTIPLIED = 2;
            const OPAQUE = 3;
            const CUSTOM = 4;
        }


    }


    impl DDSD {
        /// Starting value for all flags.
        pub fn get_required_flags() -> Self {
            Self::CAPS | Self::HEIGHT | Self::WIDTH | Self::PIXELFORMAT | Self::MIPMAPCOUNT
        }
    }

    impl Default for DDSD {
        fn default() -> Self {
            Self::get_required_flags()
        }
    }

    impl DDSCAPS2 {
        pub fn get_all_cubemap_faces() -> Self {
            Self::Cubemap | Self::CubemapPositiveX | Self::CubemapNegativeX | Self::CubemapPositiveY
            | Self::CubemapNegativeY | Self::CubemapPositiveZ | Self::CubemapNegativeZ
        }
    }

    impl Default for DDSCAPS {
        fn default() -> Self {
            Self::TEXTURE
        }
    }

    impl Default for DDSCAPS2 {
        fn default() -> Self {
            Self::empty()
        }
    }

    impl Default for DDSPF {
        fn default() -> Self {
            Self::empty()
        }
    }

    impl Default for D3D10ResourceDimension {
        fn default() -> Self {
            Self::TEXTURE2D
        }
    }

    impl Default for ResourceMisc {
        fn default() -> Self {
            Self::TEXTURECUBE
        }
    }

    impl Default for AlphaMode {
        fn default() -> Self {
            Self::PREMULTIPLIED
        }
    }

}


#[derive(Debug, Clone)]
pub struct DDSPixelFormat {
    size: i32, // asserted: 32
    pub flags: DDSPF, // u32
    pub fourcc: [u8; 4], // default=b"\0\0\0\0"
    pub rgb_bit_count: i32,
    pub r_bitmask: u32,
    pub g_bitmask: u32,
    pub b_bitmask: u32,
    pub a_bitmask: u32
}

impl Default for DDSPixelFormat {
    fn default() -> Self {
        Self {
            size: 32i32, 
            fourcc: *b"\0\0\0\0", 
            flags: DDSPF::default(),
            rgb_bit_count: 0, 
            r_bitmask: 0, 
            g_bitmask: 0, 
            b_bitmask: 0, 
            a_bitmask: 0
        }
    }
}

impl DDSPixelFormat {
    ///TODO: fix this.
    pub fn from_fourcc_tpf_format(fourcc: [u8; 4], tpf_format: u8) -> Result<Self, &'static str> {
        let mut pixelformat: DDSPixelFormat = Self {
            size: 32,
            fourcc: fourcc,
            ..Default::default()
        };

        if fourcc != [b'\0'; 4] {
            pixelformat.flags |= DDSPF::FOURCC;
            pixelformat.fourcc = fourcc;
        }

        match tpf_format {
            6 => {
                pixelformat.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pixelformat.rgb_bit_count = 16;
                pixelformat.r_bitmask = 0b01111100_00000000;
                pixelformat.g_bitmask = 0b00000011_11100000;
                pixelformat.b_bitmask = 0b00000000_00011111;
                pixelformat.a_bitmask = 0b10000000_00000000;
            },
            9 => {
                pixelformat.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pixelformat.rgb_bit_count = 32;
                pixelformat.r_bitmask = 0x00FF0000;
                pixelformat.g_bitmask = 0x0000FF00;
                pixelformat.b_bitmask = 0x000000FF;
                pixelformat.a_bitmask = 0xFF000000;
            },
            10 => {
                pixelformat.flags |= DDSPF::RGB;
                pixelformat.rgb_bit_count = 32;
                pixelformat.r_bitmask = 0x00FF0000;
                pixelformat.g_bitmask = 0x0000FF00;
                pixelformat.b_bitmask = 0x000000FF;
            },
            16 => {
                pixelformat.flags |= DDSPF::ALPHAPIXELS;
                pixelformat.rgb_bit_count = 8;
                pixelformat.a_bitmask = 0x000000FF;
            },
            105 => {
                pixelformat.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pixelformat.rgb_bit_count = 32;
                pixelformat.r_bitmask = 0x000000FF;
                pixelformat.g_bitmask = 0x0000FF00;
                pixelformat.b_bitmask = 0x00FF0000;
                pixelformat.a_bitmask = 0xFF000000;
            },
            _ => { return Err("Incorrect tpf_format for DDSPixelFormat"); }
        }
        Ok(pixelformat)
    }

}

impl IO for DDSPixelFormat {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> where Self: Sized {
        reader.matches(|r| r.read_i32_le(), &[32])?; //size
        let flags = DDSPF::from_bits_retain(reader.read_u32_le()?);

        let fourcc: [u8; 4] = reader.read_ascii_fixed(4)?
            .as_bytes()
            .try_into()
            .unwrap();

        let rgb_bit_count = reader.read_i32_le()?;

        let r_bitmask = reader.read_u32_le()?;
        let g_bitmask = reader.read_u32_le()?;
        let b_bitmask = reader.read_u32_le()?;
        let a_bitmask = reader.read_u32_le()?;

        Ok(
            Self {
                size: 32i32,
                flags,
                fourcc,
                rgb_bit_count,
                r_bitmask,
                g_bitmask,
                b_bitmask,
                a_bitmask
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32_le(32)?;
        writer.write_u32_le(self.flags.bits())?;

        writer.write_ascii_fixed(
            &String::from_utf8(self.fourcc.to_vec())
                .map_err(|e| BinaryWriterError::custom(e.to_string()))?,
            4)?;
        
        writer.write_i32_le(self.rgb_bit_count)?;
        writer.write_u32_le(self.r_bitmask)?;
        writer.write_u32_le(self.g_bitmask)?;
        writer.write_u32_le(self.b_bitmask)?;
        writer.write_u32_le(self.a_bitmask)
    }

}


#[derive(Clone)]
pub struct DX10Header {
    pub dxgi_format: DxgiFormat, // u32
    pub resource_dimension: D3D10ResourceDimension,
    pub misc_flag: ResourceMisc, // u32, default 0?
    pub array_size: u32, // typically one
    pub alpha_mode: AlphaMode
}

impl Default for DX10Header {
    fn default() -> Self {
        Self {
            dxgi_format: DxgiFormat::UNKNOWN,
            resource_dimension: D3D10ResourceDimension::default(),
            misc_flag: ResourceMisc::empty(),
            array_size: 1,
            alpha_mode: AlphaMode::UNKNOWN
        }
    }
}

impl DX10Header {
    pub fn get_default(dxgi_format: DxgiFormat) -> Self {
        Self {
            dxgi_format,
            resource_dimension: D3D10ResourceDimension::TEXTURE2D,
            misc_flag: ResourceMisc::TEXTURECUBE,
            array_size: 1,
            alpha_mode: AlphaMode::UNKNOWN
        }
    }
}

impl IO for DX10Header {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let dxgi_format = DxgiFormat::from_id(reader.read_u32_le()? as u8)
            .map_err(|e| BinaryReaderError::custom(e.to_string()))?;

        let resource_dimension = D3D10ResourceDimension::from_bits_retain(reader.read_u32_le()?);
        let misc_flag = ResourceMisc::from_bits_retain(reader.read_u32_le()?);
        let array_size = reader.read_u32_le()?;
        let alpha_mode = AlphaMode::from_bits_retain(reader.read_u32_le()?);

        Ok(
            Self {
                dxgi_format,
                resource_dimension,
                misc_flag,
                array_size,
                alpha_mode
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u32_le(self.dxgi_format.id() as u32)?;
        writer.write_u32_le(self.resource_dimension.bits())?;
        writer.write_u32_le(self.misc_flag.bits())?;
        writer.write_u32_le(self.array_size)?;
        writer.write_u32_le(self.alpha_mode.bits())
    }
}

impl std::fmt::Debug for DX10Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DX10Header[Dimension: {:?} | Misc Flag: {:?} | Alpha Mode: {:?} | Array Size: {}]", 
            self.resource_dimension, self.misc_flag, self.alpha_mode, self.array_size
        )
    }
}


#[derive(Clone)]
pub struct DDSHeader {
    _magic: [u8; 4], // bytes, asserted = b"DDS " rstrip_null
    size: u32, // asserted = 0x7C (124)
    pub flags: DDSD, // u32
    pub height: i32,
    pub width: i32,
    pub pitch_or_linear_size: i32,
    pub depth: i32,
    pub mipmap_count: i32,
    reserved_1: [i32; 11], // unused; other programs sometimes write codes here like 'NVTT'  (TODO: is this u8 or i32?)
    pub pixelformat: DDSPixelFormat, // offset 0x4C,
    pub caps1: DDSCAPS, // u32
    pub caps2: DDSCAPS2, // u32
    caps3: i32, // unused,
    caps4: i32, // unused,
    reserved_2: i32 // unused
}

impl Default for DDSHeader {
    fn default() -> Self {
        Self {
            _magic: *b"DDS ",
            size: 0x7C,
            flags: DDSD::default(),
            height: 0,
            width: 0,
            pitch_or_linear_size: 0,
            depth: 0,
            mipmap_count: 1,
            reserved_1: [0i32;11],
            pixelformat: DDSPixelFormat::default(),
            caps1: DDSCAPS::default(),
            caps2: DDSCAPS2::default(),
            caps3: 0,
            caps4: 0,
            reserved_2: 0
        }
    }
}

impl DDSHeader {
    pub fn validate(&self) -> Result<(), String> {
        if &self._magic != b"DDS " {
            return Err(format!(
                "Invalid magic header: expected b\"DDS \", found {:?}",
                self._magic
            ));
        }

        if self.size != 124 {
            return Err(format!(
                "Invalid header size: expected 124, found {}",
                self.size
            ));
        }

        if self.pixelformat.size != 32 {
            return Err(format!(
                "Invalid pixel format size: expected 32, found {}",
                self.pixelformat.size
            ));
        }

        Ok(())
    }
}

impl IO for DDSHeader {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.assert_bytes(b"DDS ")?;
        let size = reader.matches(|r| r.read_i32_le(), &[0x7C])? as u32;

        let flags = DDSD::from_bits_retain(reader.read_u32_le()?);

        let height = reader.read_i32_le()?;
        let width = reader.read_i32_le()?;
        let pitch_or_linear_size = reader.read_i32_le()?;
        let depth = reader.read_i32_le()?;
        let mipmap_count = reader.read_i32_le()?;

        let reserved_1: [i32; 11] = reader.multi(|r| r.read_i32_le(), 11)?
            .try_into().unwrap();

        let pixelformat = DDSPixelFormat::from_reader(reader)?;
        let caps1 = DDSCAPS::from_bits_retain(reader.read_u32_le()?);
        let caps2 = DDSCAPS2::from_bits_retain(reader.read_u32_le()?);

        let caps3 = reader.read_i32_le()?;
        let caps4 = reader.read_i32_le()?;
        let reserved_2 = reader.read_i32_le()?;

        Ok(
            Self {
                _magic: *b"DDS ",
                size,
                flags,
                height,
                width,
                pitch_or_linear_size,
                depth,
                mipmap_count,
                reserved_1,
                pixelformat,
                caps1,
                caps2,
                caps3,
                caps4,
                reserved_2
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_bytes(b"DDS ")?;
        writer.write_u32_le(0x7C)?; // size

        writer.write_u32_le(self.flags.bits())?;

        writer.write_i32_le(self.height)?;
        writer.write_i32_le(self.width)?;
        writer.write_i32_le(self.pitch_or_linear_size)?;
        writer.write_i32_le(self.depth)?;
        writer.write_i32_le(self.mipmap_count)?;

        for value in &self.reserved_1 {
            writer.write_i32_le(*value)?;
        }

        self.pixelformat.to_writer(writer)?;

        writer.write_u32_le(self.caps1.bits())?;
        writer.write_u32_le(self.caps2.bits())?;

        writer.write_i32_le(self.caps3)?;
        writer.write_i32_le(self.caps4)?;
        writer.write_i32_le(self.reserved_2)
    }
}

impl std::fmt::Debug for DDSHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DDSHeader[Dimensions: {}x{} | Flags: {:?} | Format: {:?} | CAPS: {:?} | CAPS2: {:?}]", 
            self.width, self.height, self.flags, self.pixelformat, self.caps1, self.caps2
        )
    }
}


pub struct DDS {
    pub header: DDSHeader,
    pub dx10_header: Option<DX10Header>,
    pub data: Vec<u8>,
    pub dcx_type: DCXType
}

impl DDS {
    pub fn new() -> Self {
        Self {
            header: DDSHeader::default(),
            dx10_header: None,
            data: Vec::<u8>::new(),
            dcx_type: DCXType::Unknown
        }
    }

    pub fn data_offset(&self) -> usize {
        if self.header.pixelformat.fourcc == *b"DX10" { 0x94 } else { 0x80 }
    }
 
    pub fn dxgi_format(&self) -> DxgiFormat {
        if let Some(h) = &self.dx10_header {
            return h.dxgi_format;
        }
        // https://learn.microsoft.com/en-us/windows/uwp/gaming/complete-code-for-ddstextureloader
        match &self.header.pixelformat.fourcc {
            b"DXT1" | b"DXT2" => DxgiFormat::BC1_UNORM,
            b"DXT3" => DxgiFormat::BC2_UNORM,
            b"DXT4" | b"DXT5" => DxgiFormat::BC3_UNORM,
            b"ATI1" => DxgiFormat::BC4_UNORM,
            b"BC4S" => DxgiFormat::BC4_SNORM,
            b"ATI2" | b"BC5U" => DxgiFormat::BC5_UNORM,
            b"BC5S" => DxgiFormat::BC5_SNORM,
            b"RGBG" => DxgiFormat::R8G8_B8G8_UNORM,
            b"GRGB" => DxgiFormat::G8R8_G8B8_UNORM,
            [36, 0, 0, 0] => DxgiFormat::R16G16B16A16_UNORM,
            [110, 0, 0, 0] => DxgiFormat::R16G16B16A16_SNORM,
            [111, 0, 0, 0] => DxgiFormat::R16_FLOAT,
            [112, 0, 0, 0] => DxgiFormat::R16G16_FLOAT,
            [113, 0, 0, 0] => DxgiFormat::R16G16B16A16_FLOAT,
            [114, 0, 0, 0] => DxgiFormat::R32_FLOAT,
            [115, 0, 0, 0] => DxgiFormat::R32G32_FLOAT,
            [116, 0, 0, 0] => DxgiFormat::R32G32B32A32_FLOAT,
            _ => DxgiFormat::R8G8B8A8_UNORM,
        }
    }

    ///Returns self with headers parsed. Data is left empty.
    pub fn read_headers(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let header = DDSHeader::from_reader(reader)?;

        let dx10_header: Option<DX10Header> = if header.pixelformat.fourcc == *b"DX10" {
            Some(DX10Header::from_reader(reader)?)
        } else {
            None
        };

        Ok(
            Self {
                header,
                dx10_header: dx10_header,
                data: Vec::<u8>::new(),
                dcx_type: DCXType::Null
            }
        )
        
    }

    pub fn write_headers(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.header.to_writer(writer)?;
        match self.dx10_header.as_mut() {
            Some(mut h) => h.to_writer(writer)?,
            None => {}
        }
        Ok(())
    }

    pub fn to_file(&mut self, path: &Path) -> Result<(), BinaryWriterError> where Self: Sized {
        let mut file = File::create_new(path)
            .map_err(BinaryWriterError::IO)?;

        file.write_all(&self.to_bytes()?)
            .map_err(BinaryWriterError::IO)?;

        Ok(())
    }

    ///Crop a block compressed (BC1-BC7) single-mip 2D texture without decompressing it. Returns a complete DDS file.
    ///
    ///`x`, `y`, `width` and `height` must be multiples of 4, because BC blocks are 4x4 texels and are stored independently.
    pub fn crop(&self, x: u16, y: u16, width: u16, height: u16) -> Result<DDS, FerrisoulsError> {
        let layout = FormatLayout::of(self.dxgi_format()).map_err(FerrisoulsError::Custom)?;
        if layout.block_width != 4 || layout.block_height != 4 {
            return Err(FerrisoulsError::custom("crop currently supports BC1 through BC7 formats only"));
        }
        let block_size = layout.bytes_per_block;
 
        if [x, y, width, height].iter().any(|v| v % 4 != 0) {
            return Err(FerrisoulsError::custom("All input values for block compressed cropping should be divisible by 4!"));
        }
        if width == 0 || height == 0 {
            return Err(FerrisoulsError::custom("Crop width and height must be positive!"));
        }
 
        let src_width = self.header.width as usize;
        let src_height = self.header.height as usize;

        let (x, y, width, height) = (x as usize, y as usize, width as usize, height as usize);
        if x + width > src_width || y + height > src_height {
            return Err(FerrisoulsError::custom("Crop is too large!"));
        }
 
        //cropping the top mip alone would invalidate the remaining mip chain
        if self.header.mipmap_count > 1 {
            return Err(FerrisoulsError::custom("Cropping mipmapped textures is not supported!"));
        }
 
        if self.header.caps2.contains(DDSCAPS2::Cubemap)
            || self.header.caps2.contains(DDSCAPS2::Volume)
            || self.header.depth > 1 {
            return Err(FerrisoulsError::custom("Cropping cubemaps, volume textures and texture arrays is not supported!"));
        }
 
        if self.dx10_header.as_ref().map(|h| h.array_size).unwrap_or(1) > 1 {
            return Err(FerrisoulsError::custom("Cropping texture arrays is not supported!"));
        }
 
        let src_blocks_wide = src_width.div_ceil(4);
        let src_blocks_high = src_height.div_ceil(4);
        let crop_blocks_wide = width / 4;
        let crop_blocks_high = height / 4;
 
        let src_row_bytes = src_blocks_wide * block_size;
        let crop_row_bytes = crop_blocks_wide * block_size;
        let src_x_bytes = (x / 4) * block_size;
        let src_y_blocks = y / 4;
 
        let expected_src_len = src_row_bytes * src_blocks_high;
        if self.data.len() < expected_src_len {
            return Err(FerrisoulsError::custom("DDS texture data is truncated"));
        }
 
        let mut cropped_data = Vec::with_capacity(crop_row_bytes * crop_blocks_high);
        for row in 0..crop_blocks_high {
            let start = (src_y_blocks + row) * src_row_bytes + src_x_bytes;
            cropped_data.extend_from_slice(&self.data[start..start + crop_row_bytes]);
        }
 
        let mut cropped = DDS {
            header: self.header.clone(),
            dx10_header: self.dx10_header.clone(),
            data: cropped_data,
            dcx_type: self.dcx_type,
        };
 
        cropped.header.width = width as i32;
        cropped.header.height = height as i32;
        cropped.header.pitch_or_linear_size = i32::try_from(cropped.data.len())
            .map_err(|_| FerrisoulsError::custom("Cropped texture is too large for a DDS header."))?;
        cropped.header.mipmap_count = 1;
        cropped.header.depth = 0;
 
        cropped.header.flags.remove(DDSD::PITCH | DDSD::MIPMAPCOUNT | DDSD::DEPTH);
        cropped.header.flags.insert(DDSD::LINEARSIZE);
        cropped.header.caps1.remove(DDSCAPS::COMPLEX | DDSCAPS::MIPMAP);
 
        Ok(cropped)
    }

    ///Returns RGBA8 bytes and texture dimensions. Only the top mip of the first image is decoded.
    pub fn to_rgba(&self) -> Result<(Vec<u8>, (usize, usize)), FerrisoulsError> {
        let format: DxgiFormat = self.dxgi_format();

        let width = usize::try_from(self.header.width)
            .ok()
            .filter(|&w| w > 0)
            .ok_or_else(|| FerrisoulsError::custom("Invalid texture dimensions"))?;
        let height = usize::try_from(self.header.height)
            .ok()
            .filter(|&h| h > 0)
            .ok_or_else(|| FerrisoulsError::custom("Invalid texture dimensions"))?;

        let output_len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| FerrisoulsError::custom("Output length overflow!"))?;

        let data = &self.data;

        if let Some(bytes_per_pixel) = bc::uncompressed_bpp(format) {
            let required = width
                .checked_mul(height)
                .and_then(|n| n.checked_mul(bytes_per_pixel))
                .ok_or_else(|| FerrisoulsError::custom("Required length overflow!"))?;

            if data.len() < required {
                return Err(FerrisoulsError::Custom(format!(
                    "Not enough data! Need {required} bytes for {width}x{height} {format:?}, have {}.",
                    data.len()
                )));
            }

            let mut output = Vec::with_capacity(output_len);
            for pixel in data[..required].chunks_exact(bytes_per_pixel) {
                output.extend_from_slice(&bc::expand_pixel(format, pixel));
            }
            return Ok((output, (width, height)));
        }

        let (kind, block_bytes) = Bc::from_format(format).ok_or_else(|| {
            FerrisoulsError::Custom(format!("Unsupported DXGI format for RGBA conversion: {format:?}"))
        })?;

        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);

        let required = blocks_x
            .checked_mul(blocks_y)
            .and_then(|n| n.checked_mul(block_bytes))
            .ok_or_else(|| FerrisoulsError::custom("Required length overflow!"))?;

        if data.len() < required {
            return Err(FerrisoulsError::Custom(format!(
                "Not enough data! Need {required} bytes for {width}x{height} {format:?}, have {}.",
                data.len()
            )));
        }

        let mut output = vec![0u8; output_len];
        let mut rgba_block = [0u8; 4 * 4 * 4];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let src_offset = (by * blocks_x + bx) * block_bytes;
                kind.decode(&data[src_offset..src_offset + block_bytes], &mut rgba_block);

                //only copy pixels inside the real image dimensions
                let rows = (height - by * 4).min(4);
                let cols = (width - bx * 4).min(4);

                for py in 0..rows {
                    let src = py * 4 * 4;
                    let dst = ((by * 4 + py) * width + bx * 4) * 4;
                    output[dst..dst + cols * 4].copy_from_slice(&rgba_block[src..src + cols * 4]);
                }
            }
        }

        Ok((output, (width, height)))
    }

    ///Encodes self in PNG format, returned as a vector of bytes.
    pub fn to_png(&self) -> Result<Vec<u8>, FerrisoulsError> {
        let (rgba, (width, height)) = self.to_rgba()?;

        let mut buffer = Vec::new();
        let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut buffer), width as u32, height as u32);

        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header()
            .map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        writer.write_image_data(&rgba)
            .map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        writer.finish()
            .map_err(|e| FerrisoulsError::custom(e.to_string()))?;

        Ok(buffer)
    }

    ///Encodes self in TGA format, returned as a vector of bytes.
    pub fn to_tga(&self) -> Result<Vec<u8>, FerrisoulsError> {
        let (rgba, (width, height)) = self.to_rgba()?;

        if rgba.len() != width * height * 4 {
            return Err(FerrisoulsError::custom("RGBA data size doesn't match dimensions"));
        }

        let mut tga = Vec::with_capacity(18 + rgba.len());

        //header
        tga.push(0);// ID length
        tga.push(0);// Color map type
        tga.push(2);// Image type: uncompressed true-color

        tga.extend_from_slice(&[0, 0]); // Color map origin
        tga.extend_from_slice(&[0, 0]); // Color map length
        tga.push(0); // Color map depth

        tga.extend_from_slice(&[0, 0]); // X origin
        tga.extend_from_slice(&[0, 0]); // Y origin

        tga.extend_from_slice(&(width as u16).to_le_bytes());
        tga.extend_from_slice(&(height as u16).to_le_bytes());

        tga.push(32); // Bits per pixel
        tga.push(0x28); // 8-bit alpha + top-left origin

        //RGBA -> BGRA
        for pixel in rgba.chunks_exact(4) {
            tga.extend_from_slice(&[
                pixel[2],
                pixel[1],
                pixel[0],
                pixel[3],
            ]);
        }

        debug_assert_eq!(tga.len(), 18 + width * height * 4);

        Ok(tga)
    }

    ///Writes self to a given path, encoded in a given format.
    pub fn save_as(&mut self, path: &Path, format: ImageExportFormat) -> Result<(), FerrisoulsError> {
        match format {
            ImageExportFormat::DDS => {
                self.to_file(path)?;
            },
            ImageExportFormat::PNG => {
                std::fs::write(path, self.to_tga()?)?;
            },
            ImageExportFormat::TGA => {
                std::fs::write(path, self.to_tga()?)?;
            },
            f=> { return Err(FerrisoulsError::Custom(format!("Invalid save format: {f:?}"))); }
        }
        
        Ok(())
    }   

}

impl IO for DDS {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let mut dds = Self::read_headers(reader)?;
        dds.data = reader.read_to_end();
        Ok(dds)
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.write_headers(writer)?;
        writer.write_bytes(&self.data)?;
        Ok(())
    }
}

impl std::fmt::Display for DDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DDS[Dimensions: {}x{} | DCXType: {:?} | Size: {}]", 
            self.header.width, self.header.height, self.dcx_type, self.data.len()
        )
    }
}

impl std::fmt::Debug for DDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", self); // display first
        write!(f, "{{\n     DX10: {}\n     Flags: {:?}\n     FourCC: {:?}\n     CAPS: {:?}\n     CAPS2: {:?}\n}}",
            self.dx10_header.is_some(), self.header.flags, self.header.pixelformat.fourcc, self.header.caps1, self.header.caps2
        )
    }
}



#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_dds() {
        let dds = DDS::from_path(Path::new("tests/SB_Icon_04.dds")).unwrap();
        println!("{:?}", dds)
    }
}