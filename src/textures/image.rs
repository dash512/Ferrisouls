//! Module for conversion between RGBA8 <-> DDS, PNG, TGA
//!
//! Supports:
//! -------
//! Decode & Encode:
//! * BC1, BC2, BC3, BC4, BC5, BC6H, BC7
//! 
//! *BC2 encode: explicit 4-bit alpha defined here + `intel_tex_2`'s BC1 encoder for colors.
//! 
//! Unsupported:
//! * signed BC4/BC5/BC6H encode, integer/SNORM/typeless-only layouts.
//!
//! Conventions:
//! -----------
//! * `*_SRGB` formats get the same bytes as `*_UNORM`. Pixels are assumed to already be sRGB so no conversion happens.
//! * Float formats treat color as linear light and store `srgb_to_linear(u8)`; alpha is linear. Decoding is the inverse (clamped to 0..1).
//! * Single/dual channel formats decode to gray / (R, G, 0). They encode from R / (R, G). A8 decodes to white with that alpha.
//! 
//! Note:
//! ----------
//! For anyone trying to use this module, be aware that it's not thoroughly tested and may 
//! have issues that won't reveal themselves right away.

use std::borrow::Cow;
use std::path::Path;

use crate::binary::IO;
use crate::errors::FerrisoulsError;
use crate::textures::dds::{AlphaMode, D3D10ResourceDimension, DDS, DDSCAPS, DDSCAPS2, DDSD, DDSPF, DX10Header, ResourceMisc};
use crate::textures::dxgi::DxgiFormat;
use crate::textures::swizzling::FormatLayout;



#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    RGBA,
    DDS(DxgiFormat),
    PNG,
    TGA,
}

/// Speed/quality trade-off for BC6H and BC7. 
/// 
/// Lower value = faster encode, worse quality. Defaults to slowest/highest quality.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Quality {
    Fastest,
    Faster,
    Fast,
    Normal,
    #[default]
    Slow,
}

impl Quality {
    fn bc7(self) -> intel_tex_2::bc7::EncodeSettings {
        match self {
            Self::Fastest => intel_tex_2::bc7::alpha_ultra_fast_settings(),
            Self::Faster => intel_tex_2::bc7::alpha_very_fast_settings(),
            Self::Fast => intel_tex_2::bc7::alpha_fast_settings(),
            Self::Normal => intel_tex_2::bc7::alpha_basic_settings(),
            Self::Slow => intel_tex_2::bc7::alpha_slow_settings(),
        }
    }

    fn bc6h(self) -> intel_tex_2::bc6h::EncodeSettings {
        match self {
            Self::Fastest => intel_tex_2::bc6h::very_fast_settings(),
            Self::Faster => intel_tex_2::bc6h::fast_settings(),
            Self::Fast => intel_tex_2::bc6h::basic_settings(),
            Self::Normal => intel_tex_2::bc6h::slow_settings(),
            Self::Slow => intel_tex_2::bc6h::very_slow_settings(),
        }
    }
}


#[derive(Debug, Clone, Copy, Default)]
pub struct EncodeOptions {
    pub quality: Quality,
    /// Write a full mip chain (2x2 box filter) instead of only the top level.
    pub mipmaps: bool,
}


///Uncompressed layout enum so decode and encode dont drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pixel {
    R8,
    A8,
    R8G8,
    R16,
    R16G16,
    Rgba16,
    R16F,
    R16G16F,
    Rgba16F,
    R32F,
    Rgba32F,
    B5G6R5,
    B5G5R5A1,
    B4G4R4A4,
    Rgba8,
    Bgra8,
    Bgrx8,
    Rgb10A2,
}

impl Pixel {
    pub fn of(format: DxgiFormat) -> Option<Self> {
        use DxgiFormat as D;
        Some(match format {
            D::R8_UNORM => Self::R8,
            D::A8_UNORM => Self::A8,
            D::R8G8_UNORM => Self::R8G8,
            D::R16_UNORM => Self::R16,
            D::R16G16_UNORM => Self::R16G16,
            D::R16G16B16A16_UNORM => Self::Rgba16,
            D::R16_FLOAT => Self::R16F,
            D::R16G16_FLOAT => Self::R16G16F,
            D::R16G16B16A16_FLOAT => Self::Rgba16F,
            D::R32_FLOAT => Self::R32F,
            D::R32G32B32A32_FLOAT => Self::Rgba32F,
            D::B5G6R5_UNORM => Self::B5G6R5,
            D::B5G5R5A1_UNORM => Self::B5G5R5A1,
            D::B4G4R4A4_UNORM => Self::B4G4R4A4,
            D::R8G8B8A8_UNORM | D::R8G8B8A8_UNORM_SRGB => Self::Rgba8,
            D::B8G8R8A8_UNORM | D::B8G8R8A8_UNORM_SRGB => Self::Bgra8,
            D::B8G8R8X8_UNORM | D::B8G8R8X8_UNORM_SRGB => Self::Bgrx8,
            D::R10G10B10A2_UNORM => Self::Rgb10A2,
            _ => return None,
        })
    }

    pub fn bytes(self) -> usize {
        match self {
            Self::R8 | Self::A8 => 1,
            Self::R8G8 | Self::R16 | Self::R16F | Self::B5G6R5 | Self::B5G5R5A1 | Self::B4G4R4A4 => 2,
            Self::R16G16 | Self::R16G16F | Self::R32F | Self::Rgba8 | Self::Bgra8 | Self::Bgrx8 | Self::Rgb10A2 => 4,
            Self::Rgba16 | Self::Rgba16F => 8,
            Self::Rgba32F => 16,
        }
    }

    /// `p` must hold exactly `self.bytes()` bytes.
    pub fn decode(self, p: &[u8]) -> [u8; 4] {
        match self {
            Self::R8 => [p[0], p[0], p[0], 255],
            Self::A8 => [255, 255, 255, p[0]],
            Self::R8G8 => [p[0], p[1], 0, 255],

            Self::R16 => {
                let v = unorm16(p, 0);
                [v, v, v, 255]
            }
            Self::R16G16 => [unorm16(p, 0), unorm16(p, 1), 0, 255],
            Self::Rgba16 => [unorm16(p, 0), unorm16(p, 1), unorm16(p, 2), unorm16(p, 3)],

            Self::R16F => {
                let v = hdr_to_u8(half(p, 0));
                [v, v, v, 255]
            }
            Self::R16G16F => [hdr_to_u8(half(p, 0)), hdr_to_u8(half(p, 1)), 0, 255],
            Self::Rgba16F => [
                hdr_to_u8(half(p, 0)),
                hdr_to_u8(half(p, 1)),
                hdr_to_u8(half(p, 2)),
                linear_to_u8(half(p, 3)),
            ],

            Self::R32F => {
                let v = hdr_to_u8(f32le(p, 0));
                [v, v, v, 255]
            }
            Self::Rgba32F => [
                hdr_to_u8(f32le(p, 0)),
                hdr_to_u8(f32le(p, 1)),
                hdr_to_u8(f32le(p, 2)),
                linear_to_u8(f32le(p, 3)),
            ],

            Self::B5G6R5 => {
                let v = u16le(p);
                [expand((v >> 11) & 0x1F, 31), expand((v >> 5) & 0x3F, 63), expand(v & 0x1F, 31), 255]
            }
            Self::B5G5R5A1 => {
                let v = u16le(p);
                [
                    expand((v >> 10) & 0x1F, 31),
                    expand((v >> 5) & 0x1F, 31),
                    expand(v & 0x1F, 31),
                    if v & 0x8000 != 0 { 255 } else { 0 },
                ]
            }
            Self::B4G4R4A4 => {
                let v = u16le(p);
                [
                    expand((v >> 8) & 0xF, 15),
                    expand((v >> 4) & 0xF, 15),
                    expand(v & 0xF, 15),
                    expand((v >> 12) & 0xF, 15),
                ]
            }

            Self::Rgba8 => [p[0], p[1], p[2], p[3]],
            Self::Bgra8 => [p[2], p[1], p[0], p[3]],
            Self::Bgrx8 => [p[2], p[1], p[0], 255],

            Self::Rgb10A2 => {
                let v = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                [
                    expand(v & 0x3FF, 1023),
                    expand((v >> 10) & 0x3FF, 1023),
                    expand((v >> 20) & 0x3FF, 1023),
                    expand(v >> 30, 3),
                ]
            }
        }
    }

    pub fn encode(self, [r, g, b, a]: [u8; 4], out: &mut Vec<u8>) {
        match self {
            Self::R8 => out.push(r),
            Self::A8 => out.push(a),
            Self::R8G8 => out.extend_from_slice(&[r, g]),

            // v * 257 puts v in both bytes, which `unorm16` maps back exactly
            Self::R16 => out.extend_from_slice(&(u16::from(r) * 257).to_le_bytes()),
            Self::R16G16 => {
                for c in [r, g] {
                    out.extend_from_slice(&(u16::from(c) * 257).to_le_bytes());
                }
            }
            Self::Rgba16 => {
                for c in [r, g, b, a] {
                    out.extend_from_slice(&(u16::from(c) * 257).to_le_bytes());
                }
            }

            Self::R16F => out.extend_from_slice(&f32_to_half(srgb_to_linear(r)).to_le_bytes()),
            Self::R16G16F => {
                for c in [r, g] {
                    out.extend_from_slice(&f32_to_half(srgb_to_linear(c)).to_le_bytes());
                }
            }
            Self::Rgba16F => {
                for v in [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), f32::from(a) / 255.0] {
                    out.extend_from_slice(&f32_to_half(v).to_le_bytes());
                }
            }

            Self::R32F => out.extend_from_slice(&srgb_to_linear(r).to_le_bytes()),
            Self::Rgba32F => {
                for v in [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), f32::from(a) / 255.0] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }

            Self::B5G6R5 => {
                let v = (quantize(r, 31) << 11) | (quantize(g, 63) << 5) | quantize(b, 31);
                out.extend_from_slice(&(v as u16).to_le_bytes());
            }
            Self::B5G5R5A1 => {
                let v = (u32::from(a >= 128) << 15) | (quantize(r, 31) << 10) | (quantize(g, 31) << 5) | quantize(b, 31);
                out.extend_from_slice(&(v as u16).to_le_bytes());
            }
            Self::B4G4R4A4 => {
                let v = (quantize(a, 15) << 12) | (quantize(r, 15) << 8) | (quantize(g, 15) << 4) | quantize(b, 15);
                out.extend_from_slice(&(v as u16).to_le_bytes());
            }

            Self::Rgba8 => out.extend_from_slice(&[r, g, b, a]),
            Self::Bgra8 => out.extend_from_slice(&[b, g, r, a]),
            Self::Bgrx8 => out.extend_from_slice(&[b, g, r, 255]),

            Self::Rgb10A2 => {
                let v = quantize(r, 1023) | (quantize(g, 1023) << 10) | (quantize(b, 1023) << 20) | (quantize(a, 3) << 30);
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
}


///Block compressed layouts (4x4 blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bc {
    Bc1,
    Bc2,
    Bc3,
    Bc4 { signed: bool },
    Bc5 { signed: bool },
    Bc6h { signed: bool },
    Bc7,
}

impl Bc {
    pub fn of(format: DxgiFormat) -> Option<Self> {
        use DxgiFormat as D;
        Some(match format {
            D::BC1_TYPELESS | D::BC1_UNORM | D::BC1_UNORM_SRGB => Self::Bc1,
            D::BC2_TYPELESS | D::BC2_UNORM | D::BC2_UNORM_SRGB => Self::Bc2,
            D::BC3_TYPELESS | D::BC3_UNORM | D::BC3_UNORM_SRGB => Self::Bc3,
            D::BC4_TYPELESS | D::BC4_UNORM => Self::Bc4 { signed: false },
            D::BC4_SNORM => Self::Bc4 { signed: true },
            D::BC5_TYPELESS | D::BC5_UNORM => Self::Bc5 { signed: false },
            D::BC5_SNORM => Self::Bc5 { signed: true },
            D::BC6H_TYPELESS | D::BC6H_UF16 => Self::Bc6h { signed: false },
            D::BC6H_SF16 => Self::Bc6h { signed: true },
            D::BC7_TYPELESS | D::BC7_UNORM | D::BC7_UNORM_SRGB => Self::Bc7,
            _ => return None,
        })
    }

    /// Size of one compressed 4x4 block in bytes.
    pub fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 | Self::Bc4 { .. } => 8,
            _ => 16,
        }
    }

    /// Decodes one block into a 4x4 RGBA8 block (row pitch of 16 bytes).
    pub fn decode_block(self, block: &[u8], out: &mut [u8; 64]) {
        match self {
            Self::Bc1 => bcdec_rs::bc1(block, out, 4 * 4),
            Self::Bc2 => bcdec_rs::bc2(block, out, 4 * 4),
            Self::Bc3 => bcdec_rs::bc3(block, out, 4 * 4),
            Self::Bc7 => bcdec_rs::bc7(block, out, 4 * 4),

            // single channel, expanded to opaque gray
            Self::Bc4 { signed } => {
                let mut r = [0u8; 16];
                bcdec_rs::bc4(block, &mut r, 4, signed);
                for (i, &value) in r.iter().enumerate() {
                    let v = if signed { snorm_to_u8(value) } else { value };
                    out[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, 255]);
                }
            }

            // two channels, third rebuilt as the Z of a unit normal
            Self::Bc5 { signed } => {
                let mut rg = [0u8; 32];
                bcdec_rs::bc5(block, &mut rg, 4 * 2, signed);
                for i in 0..16 {
                    let (r, g) = if signed {
                        (snorm_to_u8(rg[i * 2]), snorm_to_u8(rg[i * 2 + 1]))
                    } else {
                        (rg[i * 2], rg[i * 2 + 1])
                    };
                    out[i * 4..i * 4 + 4].copy_from_slice(&[r, g, normal_z(r, g), 255]);
                }
            }

            // HDR, clamped to 0..1 and sRGB encoded
            Self::Bc6h { signed } => {
                let mut rgb = [0f32; 48];
                bcdec_rs::bc6h_float(block, &mut rgb, 4 * 3, signed);
                for i in 0..16 {
                    out[i * 4..i * 4 + 4].copy_from_slice(&[
                        hdr_to_u8(rgb[i * 3]),
                        hdr_to_u8(rgb[i * 3 + 1]),
                        hdr_to_u8(rgb[i * 3 + 2]),
                        255,
                    ]);
                }
            }
        }
    }

    /// Encodes the whole image. Dimensions that aren't multiples of 4 are padded by edge replication.
    pub fn encode(self, img: &RgbaImage, quality: Quality) -> Result<Vec<u8>, FerrisoulsError> {
        let padded = img.padded_to_blocks();
        let p: &RgbaImage = &padded;
        let w = u32::try_from(p.width).map_err(|_| FerrisoulsError::custom("Width exceeds encoder limit"))?;
        let h = u32::try_from(p.height).map_err(|_| FerrisoulsError::custom("Height exceeds encoder limit"))?;

        let rgba = intel_tex_2::RgbaSurface { data: &p.data, width: w, height: h, stride: w * 4 };

        Ok(match self {
            Self::Bc1 => intel_tex_2::bc1::compress_blocks(&rgba), // alpha is ignored (opaque)
            Self::Bc3 => intel_tex_2::bc3::compress_blocks(&rgba),
            Self::Bc7 => intel_tex_2::bc7::compress_blocks(&quality.bc7(), &rgba),

            Self::Bc2 => {
                let colors = intel_tex_2::bc1::compress_blocks(&rgba);
                let (bw, bh) = (p.width / 4, p.height / 4);
                let mut out = Vec::with_capacity(bw * bh * 16);
                for by in 0..bh {
                    for bx in 0..bw {
                        // explicit alpha: 4 bits per pixel, pixel 0 in the low nibble of byte 0
                        let mut alpha = [0u8; 8];
                        for i in 0..16 {
                            let o = ((by * 4 + i / 4) * p.width + bx * 4 + i % 4) * 4 + 3;
                            let n = ((u16::from(p.data[o]) * 15 + 127) / 255) as u8;
                            alpha[i / 2] |= n << (4 * (i % 2));
                        }
                        out.extend_from_slice(&alpha);
                        let c = (by * bw + bx) * 8;
                        out.extend_from_slice(&colors[c..c + 8]);
                    }
                }
                out
            }

            Self::Bc4 { signed: false } => {
                let r: Vec<u8> = p.data.chunks_exact(4).map(|px| px[0]).collect();
                let surface = intel_tex_2::RSurface { data: &r, width: w, height: h, stride: w };
                intel_tex_2::bc4::compress_blocks(&surface)
            }
            Self::Bc5 { signed: false } => {
                let rg: Vec<u8> = p.data.chunks_exact(4).flat_map(|px| [px[0], px[1]]).collect();
                let surface = intel_tex_2::RgSurface { data: &rg, width: w, height: h, stride: w * 2 };
                intel_tex_2::bc5::compress_blocks(&surface)
            }

            Self::Bc6h { signed: false } => {
                let mut buf = Vec::with_capacity(p.width * p.height * 8);
                for px in p.data.chunks_exact(4) {
                    for (i, &c) in px.iter().enumerate() {
                        let v = if i < 3 { srgb_to_linear(c) } else { f32::from(c) / 255.0 };
                        buf.extend_from_slice(&f32_to_half(v).to_le_bytes());
                    }
                }
                let surface = intel_tex_2::RgbaSurface { data: &buf, width: w, height: h, stride: w * 8 };
                intel_tex_2::bc6h::compress_blocks(&quality.bc6h(), &surface)
            }

            Self::Bc4 { .. } | Self::Bc5 { .. } | Self::Bc6h { .. } => {
                return Err(FerrisoulsError::custom("Signed BC4/BC5/BC6H encoding is not supported"));
            }
        })
    }

    ///Bytes per pixel for the needed uncompressed formats
    pub fn uncompressed_bpp(format: DxgiFormat) -> Option<usize> {
        match format {
            DxgiFormat::R8_UNORM | DxgiFormat::A8_UNORM => Some(1),
            DxgiFormat::B5G6R5_UNORM | DxgiFormat::B5G5R5A1_UNORM => Some(2),
            DxgiFormat::R8G8B8A8_UNORM | DxgiFormat::R8G8B8A8_UNORM_SRGB
            | DxgiFormat::B8G8R8A8_UNORM | DxgiFormat::B8G8R8A8_UNORM_SRGB
            | DxgiFormat::B8G8R8X8_UNORM | DxgiFormat::B8G8R8X8_UNORM_SRGB => Some(4),
            DxgiFormat::R16G16B16A16_UNORM | DxgiFormat::R16G16B16A16_FLOAT => Some(8),
            _ => None,
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Pixel(Pixel),
    Block(Bc),
}

impl Codec {
    pub fn of(format: DxgiFormat) -> Option<Self> {
        Bc::of(format).map(Self::Block).or_else(|| Pixel::of(format).map(Self::Pixel))
    }

    /// Byte length of one `width` x `height` surface (a single mip, one slice).
    pub fn surface_len(self, width: usize, height: usize) -> Option<usize> {
        match self {
            Self::Pixel(p) => width.checked_mul(height)?.checked_mul(p.bytes()),
            Self::Block(bc) => width
                .div_ceil(4)
                .checked_mul(height.div_ceil(4))?
                .checked_mul(bc.block_bytes()),
        }
    }
}


//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-


///Generic image of RGBA8 bytes, used as the intermediary between formats.
#[derive(Debug, Clone)]
pub struct RgbaImage {
    pub data: Vec<u8>, // RGBA8, top-left origin
    pub width: usize,
    pub height: usize,
}

impl RgbaImage {
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Result<Self, FerrisoulsError> {
        let img = Self { data, width, height };
        img.validate()?;
        Ok(img)
    }

    pub fn validate(&self) -> Result<(), FerrisoulsError> {
        let expected = self
            .width
            .checked_mul(self.height)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| FerrisoulsError::custom("Image size overflow"))?;
        if self.width == 0 || self.height == 0 || self.data.len() != expected {
            return Err(FerrisoulsError::custom("Image data doesn't match its dimensions"));
        }
        Ok(())
    }

    ///Returns a cropped copy of the image.
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Option<Self> {
        if w == 0 || h == 0 || x.checked_add(w)? > self.width || y.checked_add(h)? > self.height {
            return None;
        }
        let mut data = Vec::with_capacity(w * h * 4);
        for row in y..y + h {
            let start = (row * self.width + x) * 4;
            data.extend_from_slice(&self.data[start..start + w * 4]);
        }
        Some(Self { data, width: w, height: h })
    }

    pub fn save(&self, path: &Path, format: ImageFormat) -> Result<(), FerrisoulsError> {
        let bytes = match format {
            ImageFormat::DDS(fmt) => self.to_dds(fmt)?,
            ImageFormat::PNG => self.to_png()?,
            ImageFormat::TGA => self.to_tga()?,
            ImageFormat::RGBA => {
                std::fs::write(path, &self.data)?;
                return Ok(());
            }
        };
        std::fs::write(path, bytes)?;
        Ok(())
    }

    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
    //Decoding:
    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

    ///Decodes the first `width` x `height` surface of raw pixel data (no DDS header). Extra trailing
    /// bytes (mips, other slices) are ignored.
    pub fn from_surface(data: &[u8], format: DxgiFormat, width: usize, height: usize) -> Result<Self, FerrisoulsError> {
        if width == 0 || height == 0 {
            return Err(FerrisoulsError::custom("Invalid texture dimensions"));
        }
        let codec = Codec::of(format).ok_or_else(|| FerrisoulsError::custom(format!("Unsupported DXGI format: {format:?}")))?;
        let required = codec
            .surface_len(width, height)
            .ok_or_else(|| FerrisoulsError::custom("Required length overflow"))?;
        let src = data.get(..required).ok_or_else(|| {
            FerrisoulsError::custom(format!(
                "Not enough data! Need {required} bytes for {width}x{height} {format:?}, have {}.",
                data.len()
            ))
        })?;
        let out_len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| FerrisoulsError::custom("Output length overflow"))?;

        let out = match codec {
            Codec::Pixel(p) => {
                let mut out = Vec::with_capacity(out_len);
                for px in src.chunks_exact(p.bytes()) {
                    out.extend_from_slice(&p.decode(px));
                }
                out
            }
            Codec::Block(bc) => {
                let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
                let bb = bc.block_bytes();
                let mut out = vec![0u8; out_len];
                let mut block = [0u8; 64];

                for by in 0..bh {
                    for bx in 0..bw {
                        let o = (by * bw + bx) * bb;
                        bc.decode_block(&src[o..o + bb], &mut block);

                        // only copy pixels inside the real image
                        let rows = (height - by * 4).min(4);
                        let cols = (width - bx * 4).min(4);
                        for py in 0..rows {
                            let d = ((by * 4 + py) * width + bx * 4) * 4;
                            out[d..d + cols * 4].copy_from_slice(&block[py * 16..py * 16 + cols * 4]);
                        }
                    }
                }
                out
            }
        };

        Ok(Self { data: out, width, height })
    }

    ///Decodes the top mip of the first slice of a parsed DDS.
    ///
    /// Uses `dds.data` as the pixel payload. If your reader leaves that empty, slice the file bytes
    /// at `dds.data_offset()` and call [`from_surface`](Self::from_surface) instead.
    pub fn from_dds(dds: &DDS) -> Result<Self, FerrisoulsError> {
        let width = usize::try_from(dds.header.width)
            .map_err(|_| FerrisoulsError::custom("Invalid DDS width"))?;
        let height = usize::try_from(dds.header.height)
            .map_err(|_| FerrisoulsError::custom("Invalid DDS height"))?;
        Self::from_surface(&dds.data, dds.dxgi_format(), width, height)
    }

    ///Decodes a TGA into RGBA8.
    ///
    /// Supports uncompressed and RLE true-color (24/32 bpp) and grayscale (8 bpp), any of the four
    /// origins. A 32 bpp file whose descriptor declares 0 alpha bits is treated as opaque, per the spec.
    pub fn from_tga(bytes: &[u8]) -> Result<Self, FerrisoulsError> {
        if bytes.len() < 18 {
            return Err(FerrisoulsError::custom("Truncated TGA header"));
        }

        let id_len = bytes[0] as usize;
        let cmap_type = bytes[1];
        let img_type = bytes[2];
        let cmap_len = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let cmap_depth = bytes[7] as usize;
        let width = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
        let height = u16::from_le_bytes([bytes[14], bytes[15]]) as usize;
        let bpp = bytes[16];
        let desc = bytes[17];

        let (rle, gray) = match img_type {
            2 => (false, false),
            10 => (true, false),
            3 => (false, true),
            11 => (true, true),
            t => return Err(FerrisoulsError::custom(format!("Unsupported TGA image type: {t}"))),
        };
        if width == 0 || height == 0 {
            return Err(FerrisoulsError::custom("TGA has zero dimensions"));
        }

        let bytes_pp = match (gray, bpp) {
            (false, 24) => 3,
            (false, 32) => 4,
            (true, 8) => 1,
            _ => return Err(FerrisoulsError::custom(format!("Unsupported TGA bit depth: {bpp}"))),
        };

        // Skip the ID field and any (unused) color map.
        let mut pos = 18 + id_len;
        if cmap_type != 0 {
            pos += cmap_len * cmap_depth.div_ceil(8);
        }

        let total = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(bytes_pp))
            .ok_or_else(|| FerrisoulsError::custom("TGA size overflow"))?;

        let rle_buf;
        let raw: &[u8] = if rle {
            rle_buf = decode_rle(bytes, pos, bytes_pp, total)?;
            &rle_buf
        } else {
            let end = pos.checked_add(total).ok_or_else(|| FerrisoulsError::custom("TGA size overflow"))?;
            bytes.get(pos..end).ok_or_else(|| FerrisoulsError::custom("Truncated TGA pixel data"))?
        };

        let has_alpha = bytes_pp == 4 && desc & 0x0F != 0;
        let top_down = desc & 0x20 != 0;
        let right_to_left = desc & 0x10 != 0;

        let mut data = vec![0u8; width * height * 4];
        for y in 0..height {
            let sy = if top_down { y } else { height - 1 - y };
            let row = &raw[sy * width * bytes_pp..(sy + 1) * width * bytes_pp];

            for x in 0..width {
                let sx = if right_to_left { width - 1 - x } else { x };
                let p = &row[sx * bytes_pp..(sx + 1) * bytes_pp];

                let rgba = match bytes_pp {
                    1 => [p[0], p[0], p[0], 255],
                    3 => [p[2], p[1], p[0], 255],
                    _ => [p[2], p[1], p[0], if has_alpha { p[3] } else { 255 }],
                };

                let d = (y * width + x) * 4;
                data[d..d + 4].copy_from_slice(&rgba);
            }
        }

        Ok(Self { data, width, height })
    }

    ///Decodes a PNG of any color type or bit depth into RGBA8.
    pub fn from_png(bytes: &[u8]) -> Result<Self, FerrisoulsError> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        // palette/tRNS expanded, 16-bit stripped to 8-bit, low bit depths expanded
        decoder.set_transformations(png::Transformations::normalize_to_color8());

        let mut reader = decoder.read_info().map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        let (width, height) = {
            let info = reader.info();
            (info.width as usize, info.height as usize)
        };

        let buf_len = reader
            .output_line_size(width as u32)
            .and_then(|line| line.checked_mul(height))
            .ok_or_else(|| FerrisoulsError::custom("PNG size overflow"))?;
        let mut buf = vec![0u8; buf_len];

        let frame = reader.next_frame(&mut buf).map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        if frame.bit_depth != png::BitDepth::Eight {
            return Err(FerrisoulsError::custom("Unexpected PNG bit depth after normalization"));
        }

        let channels = match frame.color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => return Err(FerrisoulsError::custom("Indexed PNG was not expanded")),
        };

        let mut data = Vec::with_capacity(width * height * 4);
        for row in buf.chunks_exact(frame.line_size).take(height) {
            for px in row[..width * channels].chunks_exact(channels) {
                data.extend_from_slice(&match channels {
                    1 => [px[0], px[0], px[0], 255],
                    2 => [px[0], px[0], px[0], px[1]],
                    3 => [px[0], px[1], px[2], 255],
                    _ => [px[0], px[1], px[2], px[3]],
                });
            }
        }

        Ok(Self { data, width, height })
    }

    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
    //Encoding:
    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

    ///Encodes the top level only as raw pixel data in `format` (no header). This is what you want
    /// when injecting into a TPF/swizzled container.
    pub fn to_surface(&self, format: DxgiFormat, quality: Quality) -> Result<Vec<u8>, FerrisoulsError> {
        self.validate()?;
        let codec = Codec::of(format).ok_or_else(|| FerrisoulsError::custom(format!("Unsupported DXGI format for encoding: {format:?}")))?;

        let out = match codec {
            Codec::Pixel(p) => {
                let mut out = Vec::with_capacity(self.width * self.height * p.bytes());
                for px in self.data.chunks_exact(4) {
                    p.encode([px[0], px[1], px[2], px[3]], &mut out);
                }
                out
            }
            Codec::Block(bc) => bc.encode(self, quality)?,
        };

        let expected = codec.surface_len(self.width, self.height).ok_or_else(|| FerrisoulsError::custom("Surface size overflow"))?;
        if out.len() != expected {
            return Err(FerrisoulsError::custom(format!("{format:?} encoder produced {} bytes, expected {expected}", out.len())));
        }
        Ok(out)
    }

    ///Single-mip DDS in `format` with default quality.
    pub fn to_dds(&self, format: DxgiFormat) -> Result<Vec<u8>, FerrisoulsError> {
        self.to_dds_with(format, EncodeOptions::default())
    }

    ///DDS in `format`. Always writes a DX10 header so the format is unambiguous.
    pub fn to_dds_with(&self, format: DxgiFormat, opts: EncodeOptions) -> Result<Vec<u8>, FerrisoulsError> {
        self.validate()?;
        let codec = Codec::of(format).ok_or_else(|| FerrisoulsError::custom(format!("Unsupported DXGI format for DDS output: {format:?}")))?;

        let width = i32::try_from(self.width).map_err(|_| FerrisoulsError::custom("Width exceeds DDS limit"))?;
        let height = i32::try_from(self.height).map_err(|_| FerrisoulsError::custom("Height exceeds DDS limit"))?;

        let levels = if opts.mipmaps {
            (usize::BITS - self.width.max(self.height).leading_zeros()) as usize
        } else {
            1
        };

        let mut data = Vec::new();
        let mut next: Option<RgbaImage> = None;
        for level in 0..levels {
            let img = next.as_ref().unwrap_or(self);
            data.extend_from_slice(&img.to_surface(format, opts.quality)?);
            if level + 1 < levels {
                next = Some(img.half_size());
            }
        }

        // pitch for uncompressed formats, total top-level size for block formats
        let (size_field, is_block) = match codec {
            Codec::Pixel(p) => (self.width * p.bytes(), false),
            Codec::Block(_) => (
                codec.surface_len(self.width, self.height).ok_or_else(|| FerrisoulsError::custom("Surface size overflow"))?,
                true,
            ),
        };

        let mut dds = DDS::new();
        // NOTE: if your `get_required_flags()` doesn't include the mipmap-count flag, readers that
        // check it will ignore `mipmap_count`; add it here for `levels > 1`.
        dds.header.flags = DDSD::get_required_flags() | if is_block { DDSD::LINEARSIZE } else { DDSD::PITCH };
        dds.header.height = height;
        dds.header.width = width;
        dds.header.pitch_or_linear_size =
            i32::try_from(size_field).map_err(|_| FerrisoulsError::custom("Texture size overflows the DDS header"))?;
        dds.header.depth = 0;
        dds.header.mipmap_count = levels as i32;

        dds.header.caps1 = DDSCAPS::TEXTURE;
        if levels > 1 {
            dds.header.caps1 |= DDSCAPS::COMPLEX | DDSCAPS::MIPMAP;
        }
        dds.header.caps2 = DDSCAPS2::empty();

        dds.header.pixelformat.flags = DDSPF::FOURCC;
        dds.header.pixelformat.fourcc = *b"DX10";

        dds.dx10_header = Some(DX10Header {
            dxgi_format: format,
            resource_dimension: D3D10ResourceDimension::TEXTURE2D,
            misc_flag: ResourceMisc::empty(),
            array_size: 1,
            alpha_mode: AlphaMode::UNKNOWN,
        });

        dds.data = data;
        Ok(dds.to_bytes()?)
    }

    ///Encodes as TGA. Always top-left origin, 32 bpp with 8 alpha bits.
    pub fn to_tga(&self) -> Result<Vec<u8>, FerrisoulsError> {
        self.validate()?;
        let w = u16::try_from(self.width).map_err(|_| FerrisoulsError::custom("Width exceeds TGA limit"))?;
        let h = u16::try_from(self.height).map_err(|_| FerrisoulsError::custom("Height exceeds TGA limit"))?;

        let mut header = [0u8; 18];
        header[2] = 2; // uncompressed true-color
        header[12..14].copy_from_slice(&w.to_le_bytes());
        header[14..16].copy_from_slice(&h.to_le_bytes());
        header[16] = 32;
        header[17] = 0x28; // 8-bit alpha + top-left origin

        let mut tga = Vec::with_capacity(18 + self.data.len());
        tga.extend_from_slice(&header);
        tga.extend(self.data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]));
        Ok(tga)
    }

    ///Encodes as 8-bit RGBA PNG.
    pub fn to_png(&self) -> Result<Vec<u8>, FerrisoulsError> {
        self.validate()?;
        let w = u32::try_from(self.width).map_err(|_| FerrisoulsError::custom("Width exceeds PNG limit"))?;
        let h = u32::try_from(self.height).map_err(|_| FerrisoulsError::custom("Height exceeds PNG limit"))?;

        let mut buffer = Vec::new();
        let mut encoder = png::Encoder::new(&mut buffer, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        writer.write_image_data(&self.data).map_err(|e| FerrisoulsError::custom(e.to_string()))?;
        writer.finish().map_err(|e| FerrisoulsError::custom(e.to_string()))?;

        Ok(buffer)
    }

    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
    //Internal stuff:
    //-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

    ///Next mip level: dimensions halved (min 1), each texel the average of its source area.
    /// Straight (non-premultiplied, non-gamma-correct) averaging.
    fn half_size(&self) -> RgbaImage {
        let (nw, nh) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut data = Vec::with_capacity(nw * nh * 4);

        for y in 0..nh {
            let y0 = y * self.height / nh;
            let y1 = (((y + 1) * self.height).div_ceil(nh)).max(y0 + 1).min(self.height);
            for x in 0..nw {
                let x0 = x * self.width / nw;
                let x1 = (((x + 1) * self.width).div_ceil(nw)).max(x0 + 1).min(self.width);

                let mut sum = [0u32; 4];
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        let o = (sy * self.width + sx) * 4;
                        for c in 0..4 {
                            sum[c] += u32::from(self.data[o + c]);
                        }
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as u32;
                for s in sum {
                    data.push(((s + n / 2) / n) as u8);
                }
            }
        }

        RgbaImage { data, width: nw, height: nh }
    }

    ///Pads width/height up to multiples of 4 by replicating the right/bottom edge.
    fn padded_to_blocks(&self) -> Cow<'_, RgbaImage> {
        let (w, h) = (self.width.next_multiple_of(4), self.height.next_multiple_of(4));
        if (w, h) == (self.width, self.height) {
            return Cow::Borrowed(self);
        }

        let mut data = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            let sy = y.min(self.height - 1);
            for x in 0..w {
                let sx = x.min(self.width - 1);
                let o = (sy * self.width + sx) * 4;
                data.extend_from_slice(&self.data[o..o + 4]);
            }
        }
        Cow::Owned(RgbaImage { data, width: w, height: h })
    }
}



//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
//Other image funcs:
//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

///Crop a block compressed (BC1-BC7) single-mip 2D texture without decompressing it. Returns a complete DDS file.
///
///`x`, `y`, `width` and `height` must be multiples of 4, because BC blocks are 4x4 texels and are stored independently.
pub fn crop_dds(dds: &DDS, x: u16, y: u16, width: u16, height: u16) -> Result<DDS, FerrisoulsError> {
    let layout = FormatLayout::of(dds.dxgi_format()).map_err(FerrisoulsError::Custom)?;
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

    let src_width = dds.header.width as usize;
    let src_height = dds.header.height as usize;

    let (x, y, width, height) = (x as usize, y as usize, width as usize, height as usize);
    if x + width > src_width || y + height > src_height {
        return Err(FerrisoulsError::custom("Crop is too large!"));
    }

    //cropping the top mip alone would invalidate the remaining mip chain
    if dds.header.mipmap_count > 1 {
        return Err(FerrisoulsError::custom("Cropping mipmapped textures is not supported!"));
    }

    if dds.header.caps2.contains(DDSCAPS2::Cubemap)
        || dds.header.caps2.contains(DDSCAPS2::Volume)
        || dds.header.depth > 1 {
        return Err(FerrisoulsError::custom("Cropping cubemaps, volume textures and texture arrays is not supported!"));
    }

    if dds.dx10_header.as_ref().map(|h| h.array_size).unwrap_or(1) > 1 {
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
    if dds.data.len() < expected_src_len {
        return Err(FerrisoulsError::custom("DDS texture data is truncated"));
    }

    let mut cropped_data = Vec::with_capacity(crop_row_bytes * crop_blocks_high);
    for row in 0..crop_blocks_high {
        let start = (src_y_blocks + row) * src_row_bytes + src_x_bytes;
        cropped_data.extend_from_slice(&dds.data[start..start + crop_row_bytes]);
    }

    let mut cropped = DDS {
        header: dds.header.clone(),
        dx10_header: dds.dx10_header.clone(),
        data: cropped_data,
        dcx_type: dds.dcx_type,
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

///A decently efficient function to crop a TGA image from its raw bytes. 
pub fn crop_tga(src: &[u8], x: usize, y: usize, w: usize, h: usize) -> Option<Vec<u8>> {
    if src.len() < 18 {
        return None;
    }

    let id_len = src[0] as usize;
    let color_map_type = src[1];
    let image_type = src[2];

    //only handle uncompressed true-color format
    if color_map_type != 0 || image_type != 2 {
        return None;
    }

    let src_w = u16::from_le_bytes([src[12], src[13]]) as usize;
    let src_h = u16::from_le_bytes([src[14], src[15]]) as usize;
    let bpp = src[16];

    if bpp != 24 && bpp != 32 {
        return None;
    }

    let bytes_per_pixel = (bpp / 8) as usize;
    let data_offset = 18 + id_len;

    if x + w > src_w || y + h > src_h {
        return None;
    }

    let src_stride = src_w * bytes_per_pixel;
    let dst_stride = w * bytes_per_pixel;

    let mut out = vec![0u8; 18 + h * dst_stride];

    //header
    out[2] = 2; // type
    out[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    out[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    out[16] = bpp;

    //preserve the origin
    out[17] = src[17] & 0x30;

    for row in 0..h {
        let src_start = data_offset + (y + row) * src_stride + x * bytes_per_pixel;
        let dst_start = 18 + row * dst_stride;

        out[dst_start..dst_start + dst_stride]
            .copy_from_slice(&src[src_start..src_start + dst_stride]);
    }

    Some(out)
}

//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
//Math:
//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

fn decode_rle(src: &[u8], mut pos: usize, bytes_pp: usize, total: usize) -> Result<Vec<u8>, FerrisoulsError> {
    // One 2-byte packet expands to at most 128 pixels, so the input bounds the output.
    // Capping the allocation stops a lying header from requesting gigabytes up front.
    let mut raw = Vec::with_capacity(total.min(src.len().saturating_mul(128)));

    while raw.len() < total {
        let header = *src.get(pos).ok_or_else(|| FerrisoulsError::custom("Truncated TGA RLE data"))?;
        pos += 1;
        let count = (header & 0x7F) as usize + 1;

        if header & 0x80 != 0 {
            let px = src.get(pos..pos + bytes_pp).ok_or_else(|| FerrisoulsError::custom("Truncated TGA RLE data"))?;
            pos += bytes_pp;
            for _ in 0..count {
                raw.extend_from_slice(px);
            }
        } else {
            let n = count * bytes_pp;
            raw.extend_from_slice(src.get(pos..pos + n).ok_or_else(|| FerrisoulsError::custom("Truncated TGA RLE data"))?);
            pos += n;
        }
    }

    raw.truncate(total);
    Ok(raw)
}

fn u16le(p: &[u8]) -> u32 {
    u32::from(u16::from_le_bytes([p[0], p[1]]))
}

///Reads little-endian u16 channel `i` and maps 0..=65535 to 0..=255 with rounding.
fn unorm16(p: &[u8], i: usize) -> u8 {
    let v = u32::from(u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]));
    ((v * 255 + 32767) / 65535) as u8
}

fn half(p: &[u8], i: usize) -> f32 {
    half_to_f32(u16::from_le_bytes([p[i * 2], p[i * 2 + 1]]))
}

fn f32le(p: &[u8], i: usize) -> f32 {
    f32::from_le_bytes([p[i * 4], p[i * 4 + 1], p[i * 4 + 2], p[i * 4 + 3]])
}

///Maps a `bits`-bit value (0..=max) to 0..=255 with rounding.
fn expand(v: u32, max: u32) -> u8 {
    ((v * 255 + max / 2) / max) as u8
}

///Maps 0..=255 to 0..=max with rounding.
fn quantize(v: u8, max: u32) -> u32 {
    (u32::from(v) * max + 127) / 255
}

///Maps a signed byte (-127..=127, as written by bcdec_rs) to 1..=255.
fn snorm_to_u8(value: u8) -> u8 {
    value ^ 0x80
}

fn normal_z(r: u8, g: u8) -> u8 {
    let x = f32::from(r) / 255.0 * 2.0 - 1.0;
    let y = f32::from(g) / 255.0 * 2.0 - 1.0;
    let z = (1.0 - x * x - y * y).max(0.0).sqrt();
    ((z * 0.5 + 0.5) * 255.0 + 0.5) as u8
}

///Clamps a linear HDR value to 0..1 and encodes it as sRGB.
fn hdr_to_u8(value: f32) -> u8 {
    let v = value.clamp(0.0, 1.0);
    let encoded = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (encoded * 255.0 + 0.5) as u8
}

fn linear_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn srgb_to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn half_to_f32(half: u16) -> f32 {
    let sign = u32::from(half >> 15) & 1;
    let exponent = u32::from(half >> 10) & 0x1F;
    let fraction = u32::from(half) & 0x3FF;

    let bits = if exponent == 0 {
        if fraction == 0 {
            sign << 31
        } else {
            // subnormal half; renormalize
            let mut exponent = 127 - 15 + 1;
            let mut fraction = fraction;
            while fraction & 0x400 == 0 {
                fraction <<= 1;
                exponent -= 1;
            }
            (sign << 31) | (exponent << 23) | ((fraction & 0x3FF) << 13)
        }
    } else if exponent == 0x1F {
        (sign << 31) | 0x7F80_0000 | (fraction << 13)
    } else {
        (sign << 31) | ((exponent + 127 - 15) << 23) | (fraction << 13)
    };

    f32::from_bits(bits)
}

///f32 -> f16 with round-to-nearest. Meant for finite values; overflow becomes infinity.
fn f32_to_half(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    let frac = bits & 0x007F_FFFF;

    if exp >= 31 {
        return sign | 0x7C00;
    }
    if exp <= 0 {
        // subnormal half
        if exp < -10 {
            return sign;
        }
        let mant = frac | 0x0080_0000;
        let shift = (14 - exp) as u32;
        let h = (mant >> shift) as u16;
        let round = ((mant >> (shift - 1)) & 1) as u16;
        return sign | (h + round);
    }

    let h = sign | ((exp as u16) << 10) | (frac >> 13) as u16;
    // a rounding carry into the exponent is correct
    h + ((frac >> 12) & 1) as u16
}

