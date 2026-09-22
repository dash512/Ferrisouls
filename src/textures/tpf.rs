use crate::common::bytes::ByteOrder;

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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_byte_order() {
        assert_eq!(TPFPlatform::PC.get_byte_order(), ByteOrder::LittleEndian);
        assert_eq!(TPFPlatform::PS3.get_byte_order(), ByteOrder::BigEndian);
    }
}