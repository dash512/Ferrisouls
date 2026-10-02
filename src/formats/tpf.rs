use crate::{binary::bytes::ByteOrder, binders::{Binder, BinderEntry, BinderVersion}, textures::dxgi::DxgiFormat};

pub enum TPFPlatform {
    PC = 0,
    Xbox360 = 1,
    PS3 = 2,
    PS4 = 4,
    XboxOne = 5
}


impl TPFPlatform {
    fn get_byte_order(&self) -> ByteOrder {
        match &self {
            Self::PS3 | Self::Xbox360 => ByteOrder::BigEndian,
            _ => ByteOrder::LittleEndian
        }
    }
}


pub enum TextureType {
    Texture = 0,  // one 2D texture
    Cubemap = 1, // six 2D textures
    Volume = 2  // one 3D texture
}


pub struct TPFTextureSubStruct {
    data_offset: usize,
    data_size: isize,
    format: u8,
    texture_type: TextureType,
    mipmap_count: u8,
    texture_flags: u8
}

pub struct TPFTextureFloatSubStruct {
    unk0: i32,
    size: usize // 4 * data.len()
}

pub struct ConsoleInfo {
    width: u32,
    height: u32,
    texture_count: u32,
    unk1: u16, // PS3 only
    unk2: u16, // 0x0 or 0xAAE4 in DeS, 0xD in BB/DS3 (console)
    dxgi_format: DxgiFormat, //default to Unknown 
}

pub struct TPFTexture {
    stem: String,
    format: i32, //default 1
    texture_type: TextureType,
    mipmap_count: u8, //default 0
    texture_flags: u8, // 2/3 = DCX-compressed. Others unknown

    data: Vec<u8>,

    sub_struct: TPFTextureSubStruct,
    float_sub_struct: Option<TPFTextureFloatSubStruct>,

    console_info: Option<ConsoleInfo>,
    platform: TPFPlatform // platform stored in parent TPF, but is available for Textures for QoL - Grimrukh
}

impl BinderEntry for TPFTexture {
    type Identifier = String;

    fn identity(&self) -> &Self::Identifier {
        &self.stem
    }
}

impl TPFTexture {
    
}



pub struct TPFStruct {
    signature: [u8;4], // asserted b"TPF\0"
    _data_size: usize,
    file_count: usize,
    platform: TPFPlatform,
    tpf_flags: u8, // asserted [0, 1, 2, 3]
    encoding_type: u8, // asserted [0, 1, 2] where: 2 == UTF-16, 0/1 == shift_jis_2004
    _pad1: u8 // b'\0'
}

pub struct TPF {
    textures: Vec<TPFTexture>,
    platform: TPFPlatform, // default PC
    encoding_type: u8, // default 0
    tpf_flags: u8 // non-zero value on PS3 means textures have `unk2`; unknown otherwise
}

impl Binder for TPF {
    const VERSION: BinderVersion = BinderVersion::VARIABLE; // can be 3 or 4. TODO: is this correct?
    type Entry = TPFTexture;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.textures
    }
}

impl TPF {
    
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_byte_order() {
        assert_eq!(TPFPlatform::PC.get_byte_order(), ByteOrder::LittleEndian);
        assert_eq!(TPFPlatform::PS3.get_byte_order(), ByteOrder::BigEndian);
    }
}