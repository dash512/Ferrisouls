use std::{io, error::Error, ops::BitOrAssign};
use std::io::{Read, Error as IOError, ErrorKind};
use other_enums::*;
use crate::textures::dxgi::{self, DxgiFormat};
use crate::common::dcx::{DCXError, DCXType};

mod other_enums {
    pub enum DDSD {
        CAPS = 0x1, // always required
        HEIGHT = 0x2, // always required
        WIDTH = 0x4, // always required
        PITCH = 0x8, // required when pitch is provided for an uncompressed texture
        PIXELFORMAT = 0x1000, // always required
        MIPMAPCOUNT = 0x20000, // required in a mipmapped texture
        LINEARSIZE = 0x80000, // required when pitch is provided for a compressed texture
        DEPTH = 0x800000 // required in a depth texture
    }

    impl DDSD {
        pub fn int(self) -> u32 {
            self as u32
        }

        /// Starting value for all flags.
        pub fn get_required_flags() -> u32 {
            Self::CAPS.int() | Self::HEIGHT.int() | Self::WIDTH.int() | Self::PIXELFORMAT.int()
        }
    }

    pub enum DDSCAPS {
        COMPLEX = 0x8,
        TEXTURE = 0x1000,
        MIPMAP = 0x400000
    }

    pub enum  DDSCAPS2 {
        Cubemap = 0x200,
        CubemapPositiveX = 0x400,
        CubemapNegativeX = 0x800,
        CubemapPositiveY = 0x1000,
        CubemapNegativeY = 0x2000,
        CubemapPositiveZ = 0x4000,
        CubemapNegativeZ = 0x8000,
        Volume = 0x200000
    }

    impl DDSCAPS2 {
        pub fn int(self) -> u32 {
            self as u32
        }

        pub fn get_all_cubemap_faces() -> u32 {
            Self::Cubemap.int() | Self::CubemapPositiveX.int() | Self::CubemapNegativeX.int() | Self::CubemapPositiveY.int()
            | Self::CubemapNegativeY.int() | Self::CubemapPositiveZ.int() | Self::CubemapNegativeZ.int()
        }
    }

    pub enum DDPF {
        ALPHAPIXELS = 0x1,
        ALPHA = 0x2,
        FOURCC = 0x4,
        RGB = 0x40,
        YUV = 0x200,
        LUMINANCE = 0x20000
    }

    impl DDPF {
        pub fn int(self) -> u32 {
            self as u32
        }
    }

    pub enum D3D10ResourceDimension {
        TEXTURE1D = 2,
        TEXTURE2D = 3,
        TEXTURE3D = 4
    }

    pub enum ResourceMisc {
        TEXTURECUBE = 0x4
    }

    /// Fills a formerly-reserved `miscFlags2` field in `DX10Header`.
    pub enum AlphaMode {
        UNKNOWN = 0,
        STRAIGHT = 1,
        PREMULTIPLIED = 2,
        OPAQUE = 3,
        CUSTOM = 4
    }


}


pub struct DDSPixelFormat {
    size: u32, // asserted: 32
    flags: u32, // `DDPF` bit flags
    pub fourcc: [u8; 4], // default=b"\0\0\0\0"
    rgb_bit_count: u32,
    r_bitmask: u32,
    g_bitmask: u32,
    b_bitmask: u32,
    a_bitmask: u32
}

impl DDSPixelFormat {
    pub fn from_fourcc_tpf_format(fourcc: [u8; 4], tpf_format: u8) -> Result<Self, &'static str> {
        let mut pixelformat: DDSPixelFormat = Self {
            size: 32,
            flags: 0,
            fourcc: fourcc,
            rgb_bit_count: 0u32,
            r_bitmask: 0u32,
            g_bitmask: 0u32,
            b_bitmask: 0u32,
            a_bitmask: 0u32
        };

        if fourcc != [b'\0'; 4] {
            pixelformat.flags |= (DDPF::FOURCC.int());
            pixelformat.fourcc = fourcc;
        }

        match tpf_format {
            6 => {
                pixelformat.flags |= DDPF::ALPHAPIXELS.int() | DDPF::RGB.int();
                pixelformat.rgb_bit_count = 16;
                pixelformat.r_bitmask = 0b01111100_00000000;
                pixelformat.g_bitmask = 0b00000011_11100000;
                pixelformat.b_bitmask = 0b00000000_00011111;
                pixelformat.a_bitmask = 0b10000000_00000000;
            },
            9 => {
                pixelformat.flags |= DDPF::ALPHAPIXELS.int() | DDPF::RGB.int();
                pixelformat.rgb_bit_count = 32;
                pixelformat.r_bitmask = 0x00FF0000;
                pixelformat.g_bitmask = 0x0000FF00;
                pixelformat.b_bitmask = 0x000000FF;
                pixelformat.a_bitmask = 0xFF000000;
            },
            10 => {
                pixelformat.flags |= DDPF::RGB.int();
                pixelformat.rgb_bit_count = 32;
                pixelformat.r_bitmask = 0x00FF0000;
                pixelformat.g_bitmask = 0x0000FF00;
                pixelformat.b_bitmask = 0x000000FF;
            },
            16 => {
                pixelformat.flags |= DDPF::ALPHAPIXELS.int();
                pixelformat.rgb_bit_count = 8;
                pixelformat.a_bitmask = 0x000000FF;
            },
            105 => {
                pixelformat.flags |= DDPF::ALPHAPIXELS.int() | DDPF::RGB.int();
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


pub struct DDSHeader {
    magic: [u8; 4], // bytes, asserted = b"DDS " rstrip_null
    size: u32, // asserted = 0x7C (124)
    flags: u32,
    pub height: u32,
    pub width: u32,
    pitch_or_linear_size: u32,
    depth: u32,
    pub mipmap_count: u32,
    reserved_1: [u8; 11], // unused; other programs sometimes write codes here like 'NVTT'
    pub pixelformat: DDSPixelFormat, // offset 0x4C,
    caps1: u32, // `DDSCAPS` bit flags,
    caps2: u32, // `Self` bit flags,
    caps3: u32, // unused,
    caps4: u32, // unused,
    reserved_2: u32 // unused
}

impl DDSHeader {
    pub fn validate(&self) -> Result<(), String> {
        if &self.magic != b"DDS " {
            return Err(format!(
                "Invalid magic header: expected b\"DDS \", found {:?}",
                self.magic
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

    /// Reads a DDS Header from any reader implementing `std::io::Read`.
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buffer = [0u8; 96]; // mem::size_of::<DDSHeader>()
        reader.read_exact(&mut buffer)?;

        let header: DDSHeader = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };

        header.validate().map_err(|e| IOError::new(ErrorKind::InvalidData, e))?;

        Ok(header)
    }
}


pub struct DX10Header {
    dxgi_format: u32, // dxgi::DxgiFormat.value
    resource_dimension: D3D10ResourceDimension,
    misc_flag: u32, // RESOURCE_MISC; used for cubemaps
    array_size: u32, // typically one
    alpha_mode: AlphaMode
}

impl DX10Header {
    pub fn get_default(dxgi_format: DxgiFormat) -> Self {
        Self {
            dxgi_format: dxgi_format.id(),
            resource_dimension: D3D10ResourceDimension::TEXTURE2D,
            misc_flag: 0,
            array_size: 1,
            alpha_mode: AlphaMode::UNKNOWN
        }
    }

    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buffer = [0u8; 16]; // mem::size_of::<DX10Header>()
        reader.read_exact(&mut buffer)?;

        let header: DX10Header = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };

        Ok(header)
    }
}

struct DDS {
    header: Option<DDSHeader>,
    dx10_headerr: Option<DX10Header>,
    data: Vec<u8>,
    dcx_type: DCXType
}

impl DDS {
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buffer = [0u8; 96]; // mem::size_of::<DDSHeader>()
        reader.read_exact(&mut buffer)?;

        let header: DDSHeader = DDSHeader::read_from(reader)?;

        let dx10_header: Option<DX10Header> = if header.pixelformat.fourcc == *b"DX10" {
            Some(DX10Header::read_from(reader)?)
        } else {
            None
        };

        Ok(
            Self {
                header: Some(header),
                dx10_headerr: dx10_header,
                data: Vec::<u8>::new(),
                dcx_type: DCXType::Null
            }
    //write_to


}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fih() {
    }
}