use std::fmt;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct FormatNotFoundError;
impl Error for FormatNotFoundError {}
impl fmt::Display for FormatNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DXGI Format not found!")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormatInfo {
    bits_per_texel: u8, // logical/storage bits per texel
    bytes_per_block: u8, // bytes occupied by one storage unit
    texels_per_block: u8, // texels represented by one storage unit's width
}

impl FormatInfo {
    ///Returns tuple of `(bits_per_texel, bytes_per_block, texels_per_block)`.
    pub fn to_tuple(&self) -> (u8, u8, u8) {
        (self.bits_per_texel, self.bytes_per_block, self.texels_per_block)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DxgiFormat {
    name: &'static str,
    value: u32, // dxgi ID as per microsoft
    description: &'static str,
    info: FormatInfo
}

// For more information, see:
// enum info: https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
// BC info: https://learn.microsoft.com/en-us/windows/win32/direct3d11/texture-block-compression-in-direct3d-11
// explanations: https://learn.microsoft.com/en-us/windows/win32/direct3d11/texture-block-compression-in-direct3d-11

impl DxgiFormat {
    // type def start

    pub const UNKNOWN: DxgiFormat = DxgiFormat {
        name: "UNKNOWN",
        value: 0,
        description: "The format is not known.",
        info: FormatInfo { bits_per_texel: 0, bytes_per_block: 0, texels_per_block: 1 }
    };

    pub const R32G32B32A32_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32G32B32A32_TYPELESS",
        value: 1,
        description: "A four-component, 128-bit typeless format that supports 32 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 128, bytes_per_block: 16, texels_per_block: 1 }
    };

    pub const R32G32B32A32_FLOAT: DxgiFormat = DxgiFormat {
        name: "R32G32B32A32_FLOAT",
        value: 2,
        description: "A four-component, 128-bit floating-point format that supports 32 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 128, bytes_per_block: 16, texels_per_block: 1 }
    };

    pub const R32G32B32A32_UINT: DxgiFormat = DxgiFormat {
        name: "R32G32B32A32_UINT",
        value: 3,
        description: "A four-component, 128-bit unsigned-integer format that supports 32 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 128, bytes_per_block: 16, texels_per_block: 1 }
    };

    pub const R32G32B32A32_SINT: DxgiFormat = DxgiFormat {
        name: "R32G32B32A32_SINT",
        value: 4,
        description: "A four-component, 128-bit signed-integer format that supports 32 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 128, bytes_per_block: 16, texels_per_block: 1 }
    };

    pub const R32G32B32_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32G32B32_TYPELESS",
        value: 5,
        description: "A three-component, 96-bit typeless format that supports 32 bits per color channel.",
        info: FormatInfo { bits_per_texel: 96, bytes_per_block: 12, texels_per_block: 1 }
    };

    pub const R32G32B32_FLOAT: DxgiFormat = DxgiFormat {
        name: "R32G32B32_FLOAT",
        value: 6,
        description: "A three-component, 96-bit floating-point format that supports 32 bits per color channel.",
        info: FormatInfo { bits_per_texel: 96, bytes_per_block: 12, texels_per_block: 1 }
    };

    pub const R32G32B32_UINT: DxgiFormat = DxgiFormat {
        name: "R32G32B32_UINT",
        value: 7,
        description: "A three-component, 96-bit unsigned-integer format that supports 32 bits per color channel.",
        info: FormatInfo { bits_per_texel: 96, bytes_per_block: 12, texels_per_block: 1 }
    };

    pub const R32G32B32_SINT: DxgiFormat = DxgiFormat {
        name: "R32G32B32_SINT",
        value: 8,
        description: "A three-component, 96-bit signed-integer format that supports 32 bits per color channel.",
        info: FormatInfo { bits_per_texel: 96, bytes_per_block: 12, texels_per_block: 1 }
    };

    pub const R16G16B16A16_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_TYPELESS",
        value: 9,
        description: "A four-component, 64-bit typeless format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R16G16B16A16_FLOAT: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_FLOAT",
        value: 10,
        description: "A four-component, 64-bit floating-point format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R16G16B16A16_UNORM: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_UNORM",
        value: 11,
        description: "A four-component, 64-bit unsigned-normalized-integer format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R16G16B16A16_UINT: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_UINT",
        value: 12,
        description: "A four-component, 64-bit unsigned-integer format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R16G16B16A16_SNORM: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_SNORM",
        value: 13,
        description: "A four-component, 64-bit signed-normalized-integer format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R16G16B16A16_SINT: DxgiFormat = DxgiFormat {
        name: "R16G16B16A16_SINT",
        value: 14,
        description: "A four-component, 64-bit signed-integer format that supports 16 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32G32_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32G32_TYPELESS",
        value: 15,
        description: "A two-component, 64-bit typeless format that supports 32 bits for the red channel and 32 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32G32_FLOAT: DxgiFormat = DxgiFormat {
        name: "R32G32_FLOAT",
        value: 16,
        description: "A two-component, 64-bit floating-point format that supports 32 bits for the red channel and 32 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32G32_UINT: DxgiFormat = DxgiFormat {
        name: "R32G32_UINT",
        value: 17,
        description: "A two-component, 64-bit unsigned-integer format that supports 32 bits for the red channel and 32 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32G32_SINT: DxgiFormat = DxgiFormat {
        name: "R32G32_SINT",
        value: 18,
        description: "A two-component, 64-bit signed-integer format that supports 32 bits for the red channel and 32 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32G8X24_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32G8X24_TYPELESS",
        value: 19,
        description: "A two-component, 64-bit typeless format that supports 32 bits for the red channel, 8 bits for the green channel, and 24 bits are unused.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const D32_FLOAT_S8X24_UINT: DxgiFormat = DxgiFormat {
        name: "D32_FLOAT_S8X24_UINT",
        value: 20,
        description: "A 32-bit floating-point component, and two unsigned-integer components with an additional 32 bits. This format supports 32-bit depth, 8-bit stencil, and 24 bits are unused.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R32_FLOAT_X8X24_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32_FLOAT_X8X24_TYPELESS",
        value: 21,
        description: "A 32-bit floating-point component, and two typeless components with an additional 32 bits. This format supports a 32-bit red channel, 8 bits are unused, and 24 bits are unused.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const X32_TYPELESS_G8X24_UINT: DxgiFormat = DxgiFormat {
        name: "X32_TYPELESS_G8X24_UINT",
        value: 22,
        description: "A 32-bit typeless component, and two unsigned-integer components with an additional 32 bits. This format has 32 bits unused, 8 bits for the green channel, and 24 bits are unused.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const R10G10B10A2_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R10G10B10A2_TYPELESS",
        value: 23,
        description: "A four-component, 32-bit typeless format that supports 10 bits for each color and 2 bits for alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R10G10B10A2_UNORM: DxgiFormat = DxgiFormat {
        name: "R10G10B10A2_UNORM",
        value: 24,
        description: "A four-component, 32-bit unsigned-normalized-integer format that supports 10 bits for each color and 2 bits for alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R10G10B10A2_UINT: DxgiFormat = DxgiFormat {
        name: "R10G10B10A2_UINT",
        value: 25,
        description: "A four-component, 32-bit unsigned-integer format that supports 10 bits for each color and 2 bits for alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R11G11B10_FLOAT: DxgiFormat = DxgiFormat {
        name: "R11G11B10_FLOAT",
        value: 26,
        description: "Three partial-precision floating-point numbers encoded into a single 32-bit value.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_TYPELESS",
        value: 27,
        description: "A four-component, 32-bit typeless format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_UNORM: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_UNORM",
        value: 28,
        description: "A four-component, 32-bit unsigned-normalized-integer format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_UNORM_SRGB",
        value: 29,
        description: "A four-component, 32-bit unsigned-normalized integer sRGB format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_UINT: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_UINT",
        value: 30,
        description: "A four-component, 32-bit unsigned-integer format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_SNORM: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_SNORM",
        value: 31,
        description: "A four-component, 32-bit signed-normalized-integer format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8B8A8_SINT: DxgiFormat = DxgiFormat {
        name: "R8G8B8A8_SINT",
        value: 32,
        description: "A four-component, 32-bit signed-integer format that supports 8 bits per channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R16G16_TYPELESS",
        value: 33,
        description: "A two-component, 32-bit typeless format that supports 16 bits for the red channel and 16 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_FLOAT: DxgiFormat = DxgiFormat {
        name: "R16G16_FLOAT",
        value: 34,
        description: "A two-component, 32-bit floating-point format that supports 16 bits for the red channel and 16 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_UNORM: DxgiFormat = DxgiFormat {
        name: "R16G16_UNORM",
        value: 35,
        description: "A two-component, 32-bit unsigned-normalized-integer format that supports 16 bits each for the green and red channels.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_UINT: DxgiFormat = DxgiFormat {
        name: "R16G16_UINT",
        value: 36,
        description: "A two-component, 32-bit unsigned-integer format that supports 16 bits for the red channel and 16 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_SNORM: DxgiFormat = DxgiFormat {
        name: "R16G16_SNORM",
        value: 37,
        description: "A two-component, 32-bit signed-normalized-integer format that supports 16 bits for the red channel and 16 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R16G16_SINT: DxgiFormat = DxgiFormat {
        name: "R16G16_SINT",
        value: 38,
        description: "A two-component, 32-bit signed-integer format that supports 16 bits for the red channel and 16 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R32_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R32_TYPELESS",
        value: 39,
        description: "A single-component, 32-bit typeless format that supports 32 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const D32_FLOAT: DxgiFormat = DxgiFormat {
        name: "D32_FLOAT",
        value: 40,
        description: "A single-component, 32-bit floating-point format that supports 32 bits for depth.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R32_FLOAT: DxgiFormat = DxgiFormat {
        name: "R32_FLOAT",
        value: 41,
        description: "A single-component, 32-bit floating-point format that supports 32 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R32_UINT: DxgiFormat = DxgiFormat {
        name: "R32_UINT",
        value: 42,
        description: "A single-component, 32-bit unsigned-integer format that supports 32 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R32_SINT: DxgiFormat = DxgiFormat {
        name: "R32_SINT",
        value: 43,
        description: "A single-component, 32-bit signed-integer format that supports 32 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R24G8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R24G8_TYPELESS",
        value: 44,
        description: "A two-component, 32-bit typeless format that supports 24 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const D24_UNORM_S8_UINT: DxgiFormat = DxgiFormat {
        name: "D24_UNORM_S8_UINT",
        value: 45,
        description: "A 32-bit z-buffer format that supports 24 bits for depth and 8 bits for stencil.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R24_UNORM_X8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R24_UNORM_X8_TYPELESS",
        value: 46,
        description: "A 32-bit format containing a 24-bit single-component unsigned-normalized integer and an additional typeless 8 bits.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const X24_TYPELESS_G8_UINT: DxgiFormat = DxgiFormat {
        name: "X24_TYPELESS_G8_UINT",
        value: 47,
        description: "A 32-bit format containing a 24-bit typeless component and an additional 8-bit unsigned-integer component.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R8G8_TYPELESS",
        value: 48,
        description: "A two-component, 16-bit typeless format that supports 8 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R8G8_UNORM: DxgiFormat = DxgiFormat {
        name: "R8G8_UNORM",
        value: 49,
        description: "A two-component, 16-bit unsigned-normalized-integer format that supports 8 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R8G8_UINT: DxgiFormat = DxgiFormat {
        name: "R8G8_UINT",
        value: 50,
        description: "A two-component, 16-bit unsigned-integer format that supports 8 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R8G8_SNORM: DxgiFormat = DxgiFormat {
        name: "R8G8_SNORM",
        value: 51,
        description: "A two-component, 16-bit signed-normalized-integer format that supports 8 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R8G8_SINT: DxgiFormat = DxgiFormat {
        name: "R8G8_SINT",
        value: 52,
        description: "A two-component, 16-bit signed-integer format that supports 8 bits for the red channel and 8 bits for the green channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R16_TYPELESS",
        value: 53,
        description: "A single-component, 16-bit typeless format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_FLOAT: DxgiFormat = DxgiFormat {
        name: "R16_FLOAT",
        value: 54,
        description: "A single-component, 16-bit floating-point format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const D16_UNORM: DxgiFormat = DxgiFormat {
        name: "D16_UNORM",
        value: 55,
        description: "A single-component, 16-bit unsigned-normalized-integer format that supports 16 bits for depth.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_UNORM: DxgiFormat = DxgiFormat {
        name: "R16_UNORM",
        value: 56,
        description: "A single-component, 16-bit unsigned-normalized-integer format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_UINT: DxgiFormat = DxgiFormat {
        name: "R16_UINT",
        value: 57,
        description: "A single-component, 16-bit unsigned-integer format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_SNORM: DxgiFormat = DxgiFormat {
        name: "R16_SNORM",
        value: 58,
        description: "A single-component, 16-bit signed-normalized-integer format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R16_SINT: DxgiFormat = DxgiFormat {
        name: "R16_SINT",
        value: 59,
        description: "A single-component, 16-bit signed-integer format that supports 16 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const R8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "R8_TYPELESS",
        value: 60,
        description: "A single-component, 8-bit typeless format that supports 8 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const R8_UNORM: DxgiFormat = DxgiFormat {
        name: "R8_UNORM",
        value: 61,
        description: "A single-component, 8-bit unsigned-normalized-integer format that supports 8 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const R8_UINT: DxgiFormat = DxgiFormat {
        name: "R8_UINT",
        value: 62,
        description: "A single-component, 8-bit unsigned-integer format that supports 8 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const R8_SNORM: DxgiFormat = DxgiFormat {
        name: "R8_SNORM",
        value: 63,
        description: "A single-component, 8-bit signed-normalized-integer format that supports 8 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const R8_SINT: DxgiFormat = DxgiFormat {
        name: "R8_SINT",
        value: 64,
        description: "A single-component, 8-bit signed-integer format that supports 8 bits for the red channel.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const A8_UNORM: DxgiFormat = DxgiFormat {
        name: "A8_UNORM",
        value: 65,
        description: "A single-component, 8-bit unsigned-normalized-integer format for alpha only.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const R1_UNORM: DxgiFormat = DxgiFormat {
        name: "R1_UNORM",
        value: 66,
        description: "A single-component, 1-bit unsigned-normalized integer format that supports 1 bit for the red channel.",
        info: FormatInfo { bits_per_texel: 1, bytes_per_block: 1, texels_per_block: 8 }
    };

    pub const R9G9B9E5_SHAREDEXP: DxgiFormat = DxgiFormat {
        name: "R9G9B9E5_SHAREDEXP",
        value: 67,
        description: "Three partial-precision floating-point numbers encoded into a single 32-bit value, all sharing the same 5-bit exponent.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R8G8_B8G8_UNORM: DxgiFormat = DxgiFormat {
        name: "R8G8_B8G8_UNORM",
        value: 68,
        description: "A four-component, 32-bit unsigned-normalized-integer packed RGB format. Each 32-bit block describes a pair of pixels.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 4, texels_per_block: 2 }
    };

    pub const G8R8_G8B8_UNORM: DxgiFormat = DxgiFormat {
        name: "G8R8_G8B8_UNORM",
        value: 69,
        description: "A four-component, 32-bit unsigned-normalized-integer packed RGB format. Each 32-bit block describes a pair of pixels.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 4, texels_per_block: 2 }
    };

    pub const BC1_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC1_TYPELESS",
        value: 70,
        description: "Four-component typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC1_UNORM: DxgiFormat = DxgiFormat {
        name: "BC1_UNORM",
        value: 71,
        description: "Four-component block-compression format.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC1_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "BC1_UNORM_SRGB",
        value: 72,
        description: "Four-component block-compression format for sRGB data.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC2_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC2_TYPELESS",
        value: 73,
        description: "Four-component typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC2_UNORM: DxgiFormat = DxgiFormat {
        name: "BC2_UNORM",
        value: 74,
        description: "Four-component block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC2_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "BC2_UNORM_SRGB",
        value: 75,
        description: "Four-component block-compression format for sRGB data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC3_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC3_TYPELESS",
        value: 76,
        description: "Four-component typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC3_UNORM: DxgiFormat = DxgiFormat {
        name: "BC3_UNORM",
        value: 77,
        description: "Four-component block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC3_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "BC3_UNORM_SRGB",
        value: 78,
        description: "Four-component block-compression format for sRGB data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC4_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC4_TYPELESS",
        value: 79,
        description: "One-component typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC4_UNORM: DxgiFormat = DxgiFormat {
        name: "BC4_UNORM",
        value: 80,
        description: "One-component block-compression format.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC4_SNORM: DxgiFormat = DxgiFormat {
        name: "BC4_SNORM",
        value: 81,
        description: "One-component block-compression format.",
        info: FormatInfo { bits_per_texel: 4, bytes_per_block: 8, texels_per_block: 4 }
    };

    pub const BC5_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC5_TYPELESS",
        value: 82,
        description: "Two-component typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC5_UNORM: DxgiFormat = DxgiFormat {
        name: "BC5_UNORM",
        value: 83,
        description: "Two-component block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC5_SNORM: DxgiFormat = DxgiFormat {
        name: "BC5_SNORM",
        value: 84,
        description: "Two-component block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const B5G6R5_UNORM: DxgiFormat = DxgiFormat {
        name: "B5G6R5_UNORM",
        value: 85,
        description: "A three-component, 16-bit unsigned-normalized-integer format that supports 5 bits for blue, 6 bits for green, and 5 bits for red.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const B5G5R5A1_UNORM: DxgiFormat = DxgiFormat {
        name: "B5G5R5A1_UNORM",
        value: 86,
        description: "A four-component, 16-bit unsigned-normalized-integer format that supports 5 bits for each color channel and 1-bit alpha.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const B8G8R8A8_UNORM: DxgiFormat = DxgiFormat {
        name: "B8G8R8A8_UNORM",
        value: 87,
        description: "A four-component, 32-bit unsigned-normalized-integer format that supports 8 bits for each color channel and 8-bit alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const B8G8R8X8_UNORM: DxgiFormat = DxgiFormat {
        name: "B8G8R8X8_UNORM",
        value: 88,
        description: "A four-component, 32-bit unsigned-normalized-integer format that supports 8 bits for each color channel and 8 bits unused.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const R10G10B10_XR_BIAS_A2_UNORM: DxgiFormat = DxgiFormat {
        name: "R10G10B10_XR_BIAS_A2_UNORM",
        value: 89,
        description: "A four-component, 32-bit 2.8-biased fixed-point format that supports 10 bits for each color channel and 2-bit alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const B8G8R8A8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "B8G8R8A8_TYPELESS",
        value: 90,
        description: "A four-component, 32-bit typeless format that supports 8 bits for each channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const B8G8R8A8_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "B8G8R8A8_UNORM_SRGB",
        value: 91,
        description: "A four-component, 32-bit unsigned-normalized standard RGB format that supports 8 bits for each channel including alpha.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const B8G8R8X8_TYPELESS: DxgiFormat = DxgiFormat {
        name: "B8G8R8X8_TYPELESS",
        value: 92,
        description: "A four-component, 32-bit typeless format that supports 8 bits for each color channel, and 8 bits are unused.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const B8G8R8X8_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "B8G8R8X8_UNORM_SRGB",
        value: 93,
        description: "A four-component, 32-bit unsigned-normalized standard RGB format that supports 8 bits for each color channel, with 8 bits unused.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const BC6H_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC6H_TYPELESS",
        value: 94,
        description: "A typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC6H_UF16: DxgiFormat = DxgiFormat {
        name: "BC6H_UF16",
        value: 95,
        description: "A block-compression format using unsigned floating-point data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC6H_SF16: DxgiFormat = DxgiFormat {
        name: "BC6H_SF16",
        value: 96,
        description: "A block-compression format using signed floating-point data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC7_TYPELESS: DxgiFormat = DxgiFormat {
        name: "BC7_TYPELESS",
        value: 97,
        description: "A typeless block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC7_UNORM: DxgiFormat = DxgiFormat {
        name: "BC7_UNORM",
        value: 98,
        description: "A block-compression format.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const BC7_UNORM_SRGB: DxgiFormat = DxgiFormat {
        name: "BC7_UNORM_SRGB",
        value: 99,
        description: "A block-compression format for sRGB data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 }
    };

    pub const AYUV: DxgiFormat = DxgiFormat {
        name: "AYUV",
        value: 100,
        description: "Most common YUV 4:4:4 video resource format.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const Y410: DxgiFormat = DxgiFormat {
        name: "Y410",
        value: 101,
        description: "10-bit per channel packed YUV 4:4:4 video resource format.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 4, texels_per_block: 1 }
    };

    pub const Y416: DxgiFormat = DxgiFormat {
        name: "Y416",
        value: 102,
        description: "16-bit per channel packed YUV 4:4:4 video resource format.",
        info: FormatInfo { bits_per_texel: 64, bytes_per_block: 8, texels_per_block: 1 }
    };

    pub const NV12: DxgiFormat = DxgiFormat {
        name: "NV12",
        value: 103,
        description: "Most common YUV 4:2:0 video resource format.",
        info: FormatInfo { bits_per_texel: 12, bytes_per_block: 6, texels_per_block: 2 }
    };

    pub const P010: DxgiFormat = DxgiFormat {
        name: "P010",
        value: 104,
        description: "10-bit per channel planar YUV 4:2:0 video resource format. Each component uses 16 bits of storage.",
        info: FormatInfo { bits_per_texel: 24, bytes_per_block: 12, texels_per_block: 2 }
    };

    pub const P016: DxgiFormat = DxgiFormat {
        name: "P016",
        value: 105,
        description: "16-bit per channel planar YUV 4:2:0 video resource format.",
        info: FormatInfo { bits_per_texel: 24, bytes_per_block: 12, texels_per_block: 2 }
    };

    pub const FORMAT_420_OPAQUE: DxgiFormat = DxgiFormat {
        name: "420_OPAQUE",
        value: 106,
        description: "8-bit per channel planar YUV 4:2:0 video resource format with an opaque implementation-defined layout.",
        info: FormatInfo { bits_per_texel: 12, bytes_per_block: 6, texels_per_block: 2 }
    };

    pub const YUY2: DxgiFormat = DxgiFormat {
        name: "YUY2",
        value: 107,
        description: "Most common YUV 4:2:2 video resource format.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 4, texels_per_block: 2 }
    };

    pub const Y210: DxgiFormat = DxgiFormat {
        name: "Y210",
        value: 108,
        description: "10-bit per channel packed YUV 4:2:2 video resource format. Each component uses 16 bits of storage.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 8, texels_per_block: 2 }
    };

    pub const Y216: DxgiFormat = DxgiFormat {
        name: "Y216",
        value: 109,
        description: "16-bit per channel packed YUV 4:2:2 video resource format.",
        info: FormatInfo { bits_per_texel: 32, bytes_per_block: 8, texels_per_block: 2 }
    };

    pub const NV11: DxgiFormat = DxgiFormat {
        name: "NV11",
        value: 110,
        description: "Most common planar YUV 4:1:1 video resource format.",
        info: FormatInfo { bits_per_texel: 12, bytes_per_block: 6, texels_per_block: 4 }
    };

    pub const AI44: DxgiFormat = DxgiFormat {
        name: "AI44",
        value: 111,
        description: "4-bit palletized YUV format commonly used for DVD subpicture.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const IA44: DxgiFormat = DxgiFormat {
        name: "IA44",
        value: 112,
        description: "4-bit palletized YUV format commonly used for DVD subpicture.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const P8: DxgiFormat = DxgiFormat {
        name: "P8",
        value: 113,
        description: "8-bit palletized format used for palletized RGB and YUV data.",
        info: FormatInfo { bits_per_texel: 8, bytes_per_block: 1, texels_per_block: 1 }
    };

    pub const A8P8: DxgiFormat = DxgiFormat {
        name: "A8P8",
        value: 114,
        description: "8-bit palletized format with 8 bits of alpha.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const B4G4R4A4_UNORM: DxgiFormat = DxgiFormat {
        name: "B4G4R4A4_UNORM",
        value: 115,
        description: "A four-component, 16-bit unsigned-normalized integer format that supports 4 bits for each channel including alpha.",
        info: FormatInfo { bits_per_texel: 16, bytes_per_block: 2, texels_per_block: 1 }
    };

    pub const DXGI_FORMATS: &[DxgiFormat] = &[
        Self::UNKNOWN,
        Self::R32G32B32A32_TYPELESS,
        Self::R32G32B32A32_FLOAT,
        Self::R32G32B32A32_UINT,
        Self::R32G32B32A32_SINT,
        Self::R32G32B32_TYPELESS,
        Self::R32G32B32_FLOAT,
        Self::R32G32B32_UINT,
        Self::R32G32B32_SINT,
        Self::R16G16B16A16_TYPELESS,
        Self::R16G16B16A16_FLOAT,
        Self::R16G16B16A16_UNORM,
        Self::R16G16B16A16_UINT,
        Self::R16G16B16A16_SNORM,
        Self::R16G16B16A16_SINT,
        Self::R32G32_TYPELESS,
        Self::R32G32_FLOAT,
        Self::R32G32_UINT,
        Self::R32G32_SINT,
        Self::R32G8X24_TYPELESS,
        Self::D32_FLOAT_S8X24_UINT,
        Self::R32_FLOAT_X8X24_TYPELESS,
        Self::X32_TYPELESS_G8X24_UINT,
        Self::R10G10B10A2_TYPELESS,
        Self::R10G10B10A2_UNORM,
        Self::R10G10B10A2_UINT,
        Self::R11G11B10_FLOAT,
        Self::R8G8B8A8_TYPELESS,
        Self::R8G8B8A8_UNORM,
        Self::R8G8B8A8_UNORM_SRGB,
        Self::R8G8B8A8_UINT,
        Self::R8G8B8A8_SNORM,
        Self::R8G8B8A8_SINT,
        Self::R16G16_TYPELESS,
        Self::R16G16_FLOAT,
        Self::R16G16_UNORM,
        Self::R16G16_UINT,
        Self::R16G16_SNORM,
        Self::R16G16_SINT,
        Self::R32_TYPELESS,
        Self::D32_FLOAT,
        Self::R32_FLOAT,
        Self::R32_UINT,
        Self::R32_SINT,
        Self::R24G8_TYPELESS,
        Self::D24_UNORM_S8_UINT,
        Self::R24_UNORM_X8_TYPELESS,
        Self::X24_TYPELESS_G8_UINT,
        Self::R8G8_TYPELESS,
        Self::R8G8_UNORM,
        Self::R8G8_UINT,
        Self::R8G8_SNORM,
        Self::R8G8_SINT,
        Self::R16_TYPELESS,
        Self::R16_FLOAT,
        Self::D16_UNORM,
        Self::R16_UNORM,
        Self::R16_UINT,
        Self::R16_SNORM,
        Self::R16_SINT,
        Self::R8_TYPELESS,
        Self::R8_UNORM,
        Self::R8_UINT,
        Self::R8_SNORM,
        Self::R8_SINT,
        Self::A8_UNORM,
        Self::R1_UNORM,
        Self::R9G9B9E5_SHAREDEXP,
        Self::R8G8_B8G8_UNORM,
        Self::G8R8_G8B8_UNORM,
        Self::BC1_TYPELESS,
        Self::BC1_UNORM,
        Self::BC1_UNORM_SRGB,
        Self::BC2_TYPELESS,
        Self::BC2_UNORM,
        Self::BC2_UNORM_SRGB,
        Self::BC3_TYPELESS,
        Self::BC3_UNORM,
        Self::BC3_UNORM_SRGB,
        Self::BC4_TYPELESS,
        Self::BC4_UNORM,
        Self::BC4_SNORM,
        Self::BC5_TYPELESS,
        Self::BC5_UNORM,
        Self::BC5_SNORM,
        Self::B5G6R5_UNORM,
        Self::B5G5R5A1_UNORM,
        Self::B8G8R8A8_UNORM,
        Self::B8G8R8X8_UNORM,
        Self::R10G10B10_XR_BIAS_A2_UNORM,
        Self::B8G8R8A8_TYPELESS,
        Self::B8G8R8A8_UNORM_SRGB,
        Self::B8G8R8X8_TYPELESS,
        Self::B8G8R8X8_UNORM_SRGB,
        Self::BC6H_TYPELESS,
        Self::BC6H_UF16,
        Self::BC6H_SF16,
        Self::BC7_TYPELESS,
        Self::BC7_UNORM,
        Self::BC7_UNORM_SRGB,
        Self::AYUV,
        Self::Y410,
        Self::Y416,
        Self::NV12,
        Self::P010,
        Self::P016,
        Self::FORMAT_420_OPAQUE,
        Self::YUY2,
        Self::Y210,
        Self::Y216,
        Self::NV11,
        Self::AI44,
        Self::IA44,
        Self::P8,
        Self::A8P8,
        Self::B4G4R4A4_UNORM,
    ];

    // type def end

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn id(&self) -> u32 {
        self.value
    }

    pub fn info(&self) -> FormatInfo {
        self.info
    }
    
    pub fn describe(&self) -> &str {
        self.description
    }

    pub fn from_id(id: u32) -> Result<DxgiFormat, FormatNotFoundError> {
        for format in Self::DXGI_FORMATS.iter() {
            if id == format.id() {
                return Ok(format.clone());
            }
        }
        Err(FormatNotFoundError)
    }

    pub fn from_name(name: &str) -> Result<DxgiFormat, FormatNotFoundError> {
        for format in Self::DXGI_FORMATS.iter() {
            if name == format.name {
                return Ok(format.clone());
            }
        }
        Err(FormatNotFoundError)
    }

    /// Get dxgi format from internal enum value found in TPFTexture object
    pub fn from_tpftexture_id(id: u8) -> Result<DxgiFormat, FormatNotFoundError> {
        match id {
            0 | 1 | 25 | 29 | 108 | 109 => Ok(Self::BC1_UNORM),
            3 => Ok(Self::BC2_UNORM),
            5 | 23 | 33 | 110 => Ok(Self::BC3_UNORM),
            6 => Ok(Self::B5G5R5A1_UNORM),
            8 | 10 | 105 => Ok(Self::R8G8B8A8_UNORM),
            9 => Ok(Self::B8G8R8A8_UNORM),
            16 => Ok(Self::A8_UNORM),
            22 => Ok(Self::R16G16B16A16_UNORM),
            24 | 103 => Ok(Self::BC4_UNORM),
            100 | 113 | 115 => Ok(Self::BC6H_UF16),
            102 | 106 | 107 => Ok(Self::BC7_UNORM),
            104 => Ok(Self::BC5_UNORM),
            112 => Ok(Self::BC7_UNORM_SRGB),
            _=> Err(FormatNotFoundError)
        }
    }
}


#[cfg(test)]
mod tests {
    use std::error::Error;

use super::*;

    #[test]
    fn test_id_get_works() -> Result<(), FormatNotFoundError> {
        let format = DxgiFormat::from_id(71)?;
        assert_eq!(format, DxgiFormat::BC1_UNORM);
        return Ok(());
    }

    #[test]
    fn test_from_tpftexture_and_properties() -> Result<(), FormatNotFoundError> {
        let format = DxgiFormat::from_tpftexture_id(106)?;
        assert_eq!(format.id(), 98);
        assert_eq!(format.name(), "BC7_UNORM");
        assert_eq!(format.info(), FormatInfo { bits_per_texel: 8, bytes_per_block: 16, texels_per_block: 4 });
        return Ok(());
    }
}