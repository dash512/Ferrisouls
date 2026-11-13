//! Minimal module for handling KTX2, currently ASTC only. Port of SoulsFormatsNEXT's `KTX2.cs`.
//!
//! <https://registry.khronos.org/KTX/spec/2.0/ktxspec.v2.html>
//! vkFormat values are sourced from Khronos Vulkan-Headers (vulkan_core.h) and DFD constants
//! from Khronos khr_df.h.
//!
//! Cube maps and 3D textures are not supported. BasisLZ supercompression isn't either, because it is
//! a full ETC1S re-encode rather than a byte stream codec. Zstd and Zlib are.
//!
//! This module is rather untested.

use std::io::{Read as _, Write as _};

use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError, ReadContext, WriteContext};
use crate::textures::dxgi::DxgiFormat;
use crate::textures::headerizer::Image;

pub const KTX_MAGIC: [u8; 12] = [0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A];

pub const KHR_DF_MODEL_ASTC: u8 = 162;
pub const KHR_DF_PRIMARIES_BT709: u8 = 1;
pub const KHR_DF_TRANSFER_LINEAR: u8 = 1;
pub const KHR_DF_TRANSFER_SRGB: u8 = 2;
pub const KHR_DF_VERSION: u16 = 2;

/// Per level supercompression applied to the mip level byte ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum SupercompressionScheme {
    #[default]
    None = 0,
    Zstd = 1,
    BasisLz = 2,
    Zlib = 3,
}

impl SupercompressionScheme {
    fn from_u32(v: u32) -> Result<Self, BinaryReaderError> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            2 => Ok(Self::BasisLz),
            3 => Ok(Self::Zlib),
            v => Err(format!("Unknown KTX2 supercompression scheme {v:X}.").into())
        }
    }
}

#[derive(Clone, Debug)]
pub struct KTX2 {
    pub vk_format: u32,
    pub type_size: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub pixel_depth: u32,
    pub layer_count: u32,
    pub face_count: u32,
    pub level_count: u32,
    pub supercompression_scheme: SupercompressionScheme,

    pub color_model: u8,
    pub color_primaries: u8,
    pub transfer_function: u8,
    pub flags: u8,
    pub texel_block_dimension: [u8; 4],
    pub bytes_plane0: u8,

    /// `layers[layer][mip]`
    pub layers: Vec<Vec<Vec<u8>>>,

    pub zstd_compression_level: i32,
}

impl Default for KTX2 {
    fn default() -> Self {
        Self {
            vk_format: 0,
            type_size: 1,
            pixel_width: 0,
            pixel_height: 0,
            pixel_depth: 0,
            layer_count: 0,
            face_count: 1,
            level_count: 0,
            supercompression_scheme: SupercompressionScheme::None,
            color_model: 0,
            color_primaries: 0,
            transfer_function: 0,
            flags: 0,
            texel_block_dimension: [0; 4],
            bytes_plane0: 0,
            layers: Vec::new(),
            zstd_compression_level: 3,
        }
    }
}

/// Pads with zeroes until the position, relative to `start`, is a multiple of `align`.
fn pad_to(writer: &mut BinaryWriter, start: u64, align: u64) {
    let rem = (writer.position() as u64 - start) % align;
    if rem != 0 {
        writer.write_pattern(0u8, (align - rem) as usize);
    }
}

impl IO for KTX2 {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        reader.big_endian = false;
        reader.assert_bytes(&KTX_MAGIC)?;

        let mut ktx = KTX2 {
            vk_format: reader.read_u32()?,
            type_size: reader.read_u32()?,
            pixel_width: reader.read_u32()?,
            pixel_height: reader.read_u32()?,
            pixel_depth: reader.read_u32()?,
            layer_count: reader.read_u32()?,
            face_count: reader.read_u32()?,
            level_count: reader.read_u32()?,
            supercompression_scheme: SupercompressionScheme::from_u32(reader.read_u32()?)?,
            ..Default::default()
        };

        if ktx.face_count != 1 {
            return Err("KTX2 cube maps are not supported.".into());
        }
        if ktx.pixel_depth > 1 {
            return Err("KTX2 3D textures are not supported.".into());
        }

        let dfd_byte_offset = reader.read_i32()?;
        reader.skip(12); // dfdByteLength, kvdByteOffset, kvdByteLength
        reader.skip(16); // sgdByteOffset, sgdByteLength

        // Reading is bounds checked, so a bogus level count fails here rather than allocating.
        let entry_count = ktx.level_count.max(1);
        let mut level_index: Vec<(i64, i64)> = Vec::new();
        for _ in 0..entry_count {
            let offset = reader.read_i64()?;
            let length = reader.read_i64()?;
            reader.read_i64()?; // uncompressedByteLength
            level_index.push((offset, length));
        }

        if dfd_byte_offset < 0 {
            return Err("KTX2 DFD offset is negative.".into());
        }
        reader.set_position(dfd_byte_offset as u64)?;
        reader.read_u32()?; // dfdTotalSize
        reader.read_u32()?; // vendorId | descriptorType
        let version_and_block_size = reader.read_u32()?;
        if version_and_block_size & 0xFFFF != u32::from(KHR_DF_VERSION) {
            return Err(format!("Unexpected DFD version {}.", version_and_block_size & 0xFFFF))?;
        }
        ktx.color_model = reader.read_u8()?;
        ktx.color_primaries = reader.read_u8()?;
        ktx.transfer_function = reader.read_u8()?;
        ktx.flags = reader.read_u8()?;
        for d in ktx.texel_block_dimension.iter_mut() {
            *d = reader.read_u8()?;
        }
        ktx.bytes_plane0 = reader.read_u8()?;

        let file_len = reader.length() as i64;
        let layers = ktx.layer_count.max(1) as usize;
        if layers as i64 > file_len {
            return Err(format!("KTX2 layer count {} is larger than the file.", ktx.layer_count))?;
        }
        ktx.layers = vec![Vec::new(); layers];

        for m in 0..ktx.level_count as usize {
            let (offset, length) = level_index[m];
            if offset < 0 || length < 0 || offset.saturating_add(length) > file_len {
                return Err(format!("KTX2 level {m} data is out of bounds."))?;
            }

            // Each mip level is an independent compressed stream covering all layers/faces of that level
            reader.set_position(offset as u64)?;
            let raw = reader.read_bytes(length as usize)?;
            let level_data = match ktx.supercompression_scheme {
                SupercompressionScheme::None => raw,
                SupercompressionScheme::Zstd => zstd::stream::decode_all(raw.as_slice())
                    .with_read_context(|| format!("KTX2 level {m} zstd decompression failed"))?,
                SupercompressionScheme::Zlib => {
                    let mut out = Vec::new();
                    flate2::read::ZlibDecoder::new(raw.as_slice())
                        .read_to_end(&mut out)
                        .with_read_context(|| format!("KTX2 level {m} zlib decompression failed"))?;
                    out
                }
                SupercompressionScheme::BasisLz => return Err("BasisLZ compression not implemented".into()),
            };

            let layer_size = level_data.len() / layers;
            for (l, layer) in ktx.layers.iter_mut().enumerate() {
                layer.push(level_data[l * layer_size..(l + 1) * layer_size].to_vec());
            }
        }

        Ok(ktx)
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.big_endian = false;
        let start = writer.position() as u64;

        // Trim to the shortest mip chain so all layers stay consistent
        let levels = self.layers.iter().map(Vec::len).min().unwrap_or(0);
        if levels == 0 {
            return Err("KTX2 has no mip levels to write.".into());
        }
        self.level_count = levels as u32;

        // layer_count is 0 for non-array textures, but layers always holds one entry per plane
        let layer_planes = self.layer_count.max(1) as usize;
        if self.layers.len() < layer_planes {
            return Err("KTX2 has fewer layers than its layer count.".into());
        }

        // Assemble level blobs, supercompressing each level if requested
        let mut level_blobs: Vec<Vec<u8>> = Vec::with_capacity(levels);
        let mut uncompressed_lengths: Vec<i64> = Vec::with_capacity(levels);
        for m in 0..levels {
            let mut blob: Vec<u8> = Vec::new();
            for layer in &self.layers[..layer_planes] {
                blob.extend_from_slice(&layer[m]);
            }
            uncompressed_lengths.push(blob.len() as i64);

            let blob = match self.supercompression_scheme {
                SupercompressionScheme::None => blob,
                SupercompressionScheme::Zstd => zstd::bulk::compress(&blob, self.zstd_compression_level)
                    .with_write_context(|| format!("KTX2 level {m} zstd compression failed"))?,
                SupercompressionScheme::Zlib => {
                    // Default compression level, i.e. the 0x78 0x9C zlib header
                    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    enc.write_all(&blob)
                        .and_then(|_| enc.finish())
                        .with_write_context(|| format!("KTX2 level {m} zlib compression failed"))?
                }
                SupercompressionScheme::BasisLz => return Err("BasisLZ compression not implemented".into()),
            };
            level_blobs.push(blob);
        }

        let dfd = self.build_dfd();
        let kvd = Self::build_kvd();

        // Header
        let header_size = 80 + levels as u64 * 24;
        let dfd_offset = header_size;
        let kvd_offset = dfd_offset + dfd.len() as u64;
        let level_alignment = u64::from(self.bytes_plane0).max(4);
        let level_start = (kvd_offset + kvd.len() as u64).next_multiple_of(level_alignment);
        if level_start > i32::MAX as u64 {
            return Err("KTX2 header offsets overflowed.".into());
        }

        writer.write_bytes(&KTX_MAGIC)?;
        writer.write_i32(self.vk_format as i32);
        writer.write_i32(self.type_size as i32);
        writer.write_i32(self.pixel_width as i32);
        writer.write_i32(self.pixel_height as i32);
        writer.write_i32(self.pixel_depth as i32);
        writer.write_i32(self.layer_count as i32);
        writer.write_i32(self.face_count as i32);
        writer.write_i32(self.level_count as i32);
        writer.write_i32(self.supercompression_scheme as i32);
        writer.write_i32(dfd_offset as i32);
        writer.write_i32(dfd.len() as i32);
        writer.write_i32(kvd_offset as i32);
        writer.write_i32(kvd.len() as i32);
        writer.write_i64(0); // sgdByteOffset
        writer.write_i64(0); // sgdByteLength

        // Mip levels are stored smallest first (base level last)
        let mut level_offsets = vec![0u64; levels];
        let mut level_offset = level_start;
        for m in (0..levels).rev() {
            level_offsets[m] = level_offset;
            level_offset += level_blobs[m].len() as u64;
            level_offset = level_offset.next_multiple_of(level_alignment);
        }
        for m in 0..levels {
            writer.write_i64(level_offsets[m] as i64);
            writer.write_i64(level_blobs[m].len() as i64);
            writer.write_i64(uncompressed_lengths[m]); // uncompressedByteLength
        }

        writer.write_bytes(&dfd)?;
        writer.write_bytes(&kvd)?;
        pad_to(writer, start, level_alignment);

        for m in (0..levels).rev() {
            writer.write_bytes(&level_blobs[m])?;
            pad_to(writer, start, level_alignment);
        }

        Ok(())
    }
}

impl KTX2 {
    pub fn is(bytes: &[u8]) -> bool {
        bytes.starts_with(&KTX_MAGIC)
    }

    /// Write a KTX2 ASTC texture, uncompressed. `dxgi` is the ASTC DXGI format, which carries the
    /// block footprint and sRGB-ness.
    pub fn write_ktx2(images: &[Image], width: i32, height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat) -> Result<Vec<u8>, FerrisoulsError> {
        Self::write_ktx2_compressed(images, width, height, depth, mip_count, dxgi, SupercompressionScheme::None, 3)
    }

    /// Write a KTX2 ASTC texture, optionally supercompressing each mip level.
    #[allow(clippy::too_many_arguments)]
    pub fn write_ktx2_compressed(
        images: &[Image],
        width: i32,
        height: i32,
        depth: i32,
        mip_count: i32,
        dxgi: DxgiFormat,
        supercompression: SupercompressionScheme,
        zstd_compression_level: i32,
    ) -> Result<Vec<u8>, FerrisoulsError> {
        if supercompression == SupercompressionScheme::BasisLz {
            return Err("BasisLZ supercompression is not supported.".into());
        }
        if width < 0 || height < 0 || mip_count < 0 {
            return Err("KTX2 dimensions and mip count must not be negative.".into());
        }

        // We need to remap to KTX2's internal pixelformat
        let vk_format = dxgi
            .to_vulkan()
            .ok_or_else(|| format!("{} isn't a supported ASTC format.", dxgi.name()))?;
        let srgb = dxgi.is_astc_srgb();
        let (block_width, block_height) = dxgi.info().block_dimensions();

        let mut ktx = KTX2 {
            vk_format,
            type_size: 1,
            pixel_width: width as u32,
            pixel_height: height as u32,
            pixel_depth: 0,
            layer_count: if depth > 1 { depth as u32 } else { 0 },
            face_count: 1,
            level_count: mip_count as u32,
            supercompression_scheme: supercompression,
            color_model: KHR_DF_MODEL_ASTC,
            color_primaries: KHR_DF_PRIMARIES_BT709,
            transfer_function: if srgb { KHR_DF_TRANSFER_SRGB } else { KHR_DF_TRANSFER_LINEAR },
            flags: 0,
            texel_block_dimension: [block_width - 1, block_height - 1, 0, 0],
            bytes_plane0: 16,
            layers: images.iter().map(|i| i.sub_images.clone()).collect(),
            zstd_compression_level,
        };

        Ok(ktx.to_bytes()?)
    }

    /// The DFD as laid out for ASTC block compressed data.
    fn build_dfd(&self) -> Vec<u8> {
        const BLOCK_SIZE: u16 = 24 + 16; // basic block header + one sample
        let mut b = Vec::with_capacity(4 + BLOCK_SIZE as usize);
        b.extend_from_slice(&(4 + i32::from(BLOCK_SIZE)).to_le_bytes()); // dfdTotalSize
        b.extend_from_slice(&0i32.to_le_bytes()); // vendorId (Khronos) | descriptorType (basic)
        b.extend_from_slice(&KHR_DF_VERSION.to_le_bytes());
        b.extend_from_slice(&BLOCK_SIZE.to_le_bytes());
        b.push(self.color_model);
        b.push(self.color_primaries);
        b.push(self.transfer_function);
        b.push(self.flags);
        b.extend_from_slice(&self.texel_block_dimension);
        b.push(self.bytes_plane0);
        b.extend_from_slice(&[0u8; 7]); // bytesPlane1-7
        // Sample: channel R at bit 0, 128 bits long, unorm with full range bounds
        b.extend_from_slice(&0u16.to_le_bytes()); // bitOffset
        b.push(127); // bitLength - 1
        b.push(0); // channelType (channel R, no qualifiers)
        b.extend_from_slice(&[0u8; 4]); // samplePosition
        b.extend_from_slice(&0u32.to_le_bytes()); // sampleLower
        b.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // sampleUpper
        b
    }

    /// Key/value data, sorted by key as the spec requires.
    fn build_kvd() -> Vec<u8> {
        let mut entries: Vec<(&str, &[u8])> = vec![("KTXversion", b"2.0"), ("KTXwriter", b"SoulsFormatsNEXT")];
        entries.sort_by(|a, b| a.0.cmp(b.0));

        let mut b = Vec::new();
        for (key, value) in entries {
            let length = key.len() + 1 + value.len();
            b.extend_from_slice(&(length as i32).to_le_bytes());
            b.extend_from_slice(key.as_bytes());
            b.push(0);
            b.extend_from_slice(value);
            b.resize(b.len() + (4 - length % 4) % 4, 0); // valuePadding
        }
        b
    }

    /// The images in the Headerizer's slice major layout: one `Image` per layer with its mips.
    pub fn get_images(&self) -> Vec<Image> {
        self.layers.iter().map(|layer| Image { sub_images: layer.clone() }).collect()
    }

    /// The ASTC DXGI format of this texture, which also carries the footprint and sRGB-ness.
    pub fn dxgi_format(&self) -> Option<DxgiFormat> {
        if let Some(dxgi) = DxgiFormat::from_vulkan(self.vk_format) {
            return Some(dxgi);
        }

        // Non standard files can describe an ASTC texture through the DFD alone
        if self.vk_format == 0 && self.color_model == KHR_DF_MODEL_ASTC {
            return DxgiFormat::astc_from_footprint(
                self.texel_block_dimension[0].checked_add(1)?,
                self.texel_block_dimension[1].checked_add(1)?,
                self.transfer_function == KHR_DF_TRANSFER_SRGB,
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn images() -> Vec<Image> {
        // 8x8 ASTC 4x4: mip 0 is 2x2 blocks (64 bytes), mip 1 is 1 block (16 bytes)
        (0..2u8)
            .map(|layer| Image {
                sub_images: vec![
                    (0..64).map(|i| i as u8 ^ layer).collect(),
                    (0..16).map(|i| (i as u8).wrapping_mul(3) ^ layer).collect(),
                ],
            })
            .collect()
    }

    #[test]
    fn roundtrip_all() {
        for scheme in [SupercompressionScheme::None, SupercompressionScheme::Zstd, SupercompressionScheme::Zlib] {
            let imgs = images();
            let bytes = KTX2::write_ktx2_compressed(&imgs, 8, 8, 2, 2, DxgiFormat::ASTC_4X4_UNORM, scheme, 3).unwrap();
            assert!(KTX2::is(&bytes));

            let ktx = KTX2::from_bytes(&bytes).unwrap();
            assert!(ktx.dxgi_format() == Some(DxgiFormat::ASTC_4X4_UNORM), "{scheme:?}");
            assert_eq!((ktx.pixel_width, ktx.pixel_height, ktx.layer_count, ktx.level_count), (8, 8, 2, 2));

            let back = ktx.get_images();
            assert_eq!(back.len(), imgs.len());
            for (a, b) in back.iter().zip(&imgs) {
                assert_eq!(a.sub_images, b.sub_images, "{scheme:?}");
            }
        }
    }

    #[test]
    fn rejects_bad_data() {
        let img = vec![Image { sub_images: vec![vec![0u8; 16]] }];
        assert!(KTX2::write_ktx2(&img, 4, 4, 1, 1, DxgiFormat::BC7_UNORM).is_err());
        assert!(DxgiFormat::BC7_UNORM.to_vulkan().is_none());

        assert!(!KTX2::is(b"fih"));
        assert!(KTX2::from_bytes(&KTX_MAGIC).is_err());
        let mut bytes = KTX2::write_ktx2(&images(), 8, 8, 2, 2, DxgiFormat::ASTC_4X4_UNORM).unwrap();
        bytes.truncate(100);
        assert!(KTX2::from_bytes(&bytes).is_err());
    }
}

