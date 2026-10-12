use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};
use crate::textures::dds::{AlphaMode, D3D10ResourceDimension, DDS, DDSCAPS, DDSCAPS2, DDSD, DDSPF, DX10Header, ResourceMisc};
use crate::textures::swizzling::{Deswizzler, FormatLayout, Swizzler};
use crate::textures::tpf::{TextureType, TPFTexture, TPFPlatform};
use crate::textures::dxgi::*;
use crate::textures::utils::Util;

const MAX_MIP_COUNT: i32 = 30;
const PS5_MIN_BLOCK_DIMENSION: i32 = 8;

/// NOTE: Demon's Souls and Dark Souls 1 do NOT use PS3 swizzling and should be `TPFPlatform::PC`
/// 
/// This function assumes dxgi_format is valid and hardcodes default flags.
pub fn headerize(texture: &TPFTexture) -> Result<(Vec<u8>, String), FerrisoulsError> {
    let magic: &[u8] = texture.data.get(0..4).unwrap_or(&[]);
    match magic {
        b"DDS " => return Ok((texture.data.clone(), ".dds".into())),
        b"GNF " => return Ok((texture.data.clone(), ".gnf".into())),
        b"DFvN" => return Ok((texture.data.clone(), ".xtx".into())),
        _ => {} // continue if no valid magic, creating a header
    }

    let header = texture
        .header
        .as_ref()
        .ok_or_else(|| FerrisoulsError::custom("TPFTexture has no console header to headerize from"))?;

    if header.dxgi_format == DxgiFormat::UNKNOWN {
        return Err(FerrisoulsError::custom("Cannot headerize texture with unknown DXGI_FORMAT."));
    }

    let format = texture.format;
    let ty = texture.texture_type;
    let width = i32::from(header.width);
    let height = i32::from(header.height);

    if width <= 0 || height <= 0 {
        return Err(FerrisoulsError::Custom(format!(
            "Cannot headerize texture with dimensions {width}x{height}."
        )));
    }

    let mut mip_count = i32::from(texture.mipmap_count);
    if mip_count == 0 {
        mip_count = get_mip_count(width, height);
    }

    let depth: i32 = if header.texture_count > 0 {
        i32::from(header.texture_count)
    } else {
        match ty {
            TextureType::Cubemap => 6,
            // Volume should REALLY be defined hopefully because its count is arbitrary.
            // For a TextureArray it should REALLY REALLY be defined - SFNext
            TextureType::TPFTexture | TextureType::Volume | TextureType::TextureArray => 1,
        }
    };

    let images = rebuild_pixel_data(
        &texture.data,
        header.dxgi_format,
        width,
        height,
        depth,
        mip_count,
        ty,
        texture.platform,
        format,
    )?;

    // Failsafe for if whatever reason we don't read all of the mipmaps
    if let Some(read_mips) = images.first().map(|image| image.sub_images.len()).filter(|&n| n > 0) {
        mip_count = read_mips as i32;
    }

    let mut dds = DDS::new();
    dds.header.flags = DDSD::get_required_flags();
    dds.header.height = height;
    dds.header.width = width;

    let pitch_or_linear_size: i64 = if let Some(bpb) = DxgiFormat::compressed_bpb(format) {
        dds.header.flags |= DDSD::LINEARSIZE;
        let blocks_wide = i64::from(((width + 3) / 4).max(1));
        let blocks_high = i64::from(((height + 3) / 4).max(1));
        blocks_wide * blocks_high * i64::from(bpb)
    } else if let Some(bpp) = DxgiFormat::uncompressed_bpp(format) {
        dds.header.flags |= DDSD::PITCH;
        i64::from(width) * i64::from(bpp)
    } else {
        0
    };
    dds.header.pitch_or_linear_size = i32::try_from(pitch_or_linear_size)
        .map_err(|_| FerrisoulsError::custom("Texture size overflows the DDS header."))?;

    dds.header.depth = if ty == TextureType::Volume || ty == TextureType::TextureArray { depth } else { 0 };
    dds.header.mipmap_count = mip_count;

    dds.header.caps1 = DDSCAPS::TEXTURE;
    if ty == TextureType::Cubemap {
        dds.header.caps1 |= DDSCAPS::COMPLEX;
    }
    if mip_count > 1 {
        dds.header.caps1 |= DDSCAPS::COMPLEX | DDSCAPS::MIPMAP;
    }

    dds.header.caps2 = match ty {
        TextureType::Cubemap => DDSCAPS2::get_all_cubemap_faces(),
        TextureType::Volume => DDSCAPS2::Volume,
        _ => DDSCAPS2::empty(),
    };

    let is_dx10 = DxgiFormat::needs_dx10(header.dxgi_format.id());
    let fourcc = DxgiFormat::fourcc(format);
    {
        let pf = &mut dds.header.pixelformat;

        pf.flags = DDSPF::empty();
        if fourcc.is_some() || is_dx10 {
            pf.flags |= DDSPF::FOURCC;
        }

        if is_dx10 {
            pf.fourcc = *b"DX10";
        } else if let Some(cc) = fourcc {
            pf.fourcc = cc.as_bytes().try_into().unwrap();
        }

        match format {
            6 => {
                pf.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pf.rgb_bit_count = 16;
                pf.r_bitmask = 0b0111_1100_0000_0000;
                pf.g_bitmask = 0b0000_0011_1110_0000;
                pf.b_bitmask = 0b0000_0000_0001_1111;
                pf.a_bitmask = 0b1000_0000_0000_0000;
            }
            8 | 9 => {
                pf.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pf.rgb_bit_count = 32;
                pf.r_bitmask = 0x00FF_0000;
                pf.g_bitmask = 0x0000_FF00;
                pf.b_bitmask = 0x0000_00FF;
                pf.a_bitmask = 0xFF00_0000;
            }
            10 => {
                pf.flags |= DDSPF::RGB;
                pf.rgb_bit_count = 24;
                pf.r_bitmask = 0x00FF_0000;
                pf.g_bitmask = 0x0000_FF00;
                pf.b_bitmask = 0x0000_00FF;
            }
            16 => {
                pf.flags |= DDSPF::ALPHA;
                pf.rgb_bit_count = 8;
                pf.a_bitmask = 0x0000_00FF;
            }
            105 => {
                pf.flags |= DDSPF::ALPHAPIXELS | DDSPF::RGB;
                pf.rgb_bit_count = 32;
                pf.r_bitmask = 0x0000_00FF;
                pf.g_bitmask = 0x0000_FF00;
                pf.b_bitmask = 0x00FF_0000;
                pf.a_bitmask = 0xFF00_0000;
            }
            _ => {}
        }
    }

    if is_dx10 {
        dds.dx10_header = Some(DX10Header {
            dxgi_format: header.dxgi_format,
            resource_dimension: if ty == TextureType::Volume {
                D3D10ResourceDimension::TEXTURE3D
            } else {
                D3D10ResourceDimension::TEXTURE2D
            },
            misc_flag: if ty == TextureType::Cubemap { ResourceMisc::TEXTURECUBE } else { ResourceMisc::empty() },
            array_size: 1,
            alpha_mode: AlphaMode::UNKNOWN,
        });
    }

    dds.data = Image::write(&images);

    // Bandaid fix for the moment for PS5 textures that somehow don't have enough data
    if texture.platform == TPFPlatform::PS5 {
        dds.data.resize(dds.data.len() + 0x100, 0);
    }

    Ok((dds.to_bytes()?, ".dds".into()))
}


//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
//Read/Write:
//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-

pub struct Read;

pub struct Write;

impl Read {
    pub fn auto(platform: TPFPlatform, bytes: &[u8], width: i32, height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat, ty: TextureType, format: u8) -> Result<Vec<Image>, BinaryReaderError> {
        let mut reader = BinaryReader::new(bytes, false, false);
        match platform {
            TPFPlatform::Xbox360 => Self::XBOX360(&mut reader, width, height, depth, mip_count, dxgi),
            TPFPlatform::XboxOne => Err(BinaryReaderError::custom("Read images for XboxOne isn't implemented.")),
            TPFPlatform::PS3 => Self::PS3(&mut reader, width, height, depth, mip_count, dxgi, format),
            TPFPlatform::PS4 => Self::PS4(&mut reader, width, height, depth, mip_count, dxgi, ty),
            TPFPlatform::PS5 => Self::PS5(&mut reader, width, height, depth, mip_count, dxgi),
            // TODO: is this necessary? Left in to match soulsformats anyway
            TPFPlatform::PC | TPFPlatform::Switch => {
                Self::PS3(&mut reader, width, height, depth, mip_count, dxgi, format)
            }
        }
    }

    #[allow(non_snake_case)]
    pub fn XBOX360(reader: &mut BinaryReader, final_width: i32, final_height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat) -> Result<Vec<Image>, BinaryReaderError> {
        validate_read("X360", final_width, final_height, depth, mip_count)?;
        let layout = FormatLayout::of(dxgi).map_err(BinaryReaderError::custom)?;

        let mut images = Vec::with_capacity(depth as usize);

        for _ in 0..depth {
            let mut image = Image::new();
            for level in 0..mip_count {
                let (w, h) = padded_mip_size(final_width, final_height, level, &layout);
                let len = padded_mip_len(&layout, w, h);

                // Xbox 360 textures have minimum buffer caps. To read all the mips properly, you'd need
                // to extract them as tiles from these. - SFNext
                let raw = reader.read_bytes(to_usize(len)?)?;
                let mip = Deswizzler::Xbox360(&raw, dxgi, w as u32, h as u32)
                    .map_err(|e| BinaryReaderError::custom(e.to_string()))?;
                let mip = Util::extract_tile(
                    &mip,
                    dxgi,
                    w as usize,
                    0,
                    0,
                    w as usize,
                    h as usize,
                );
                image.sub_images.push(mip);
            }
            images.push(image);
        }
        Ok(images)
    }

    #[allow(non_snake_case)]
    pub fn PS3(reader: &mut BinaryReader, final_width: i32, final_height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat, format: u8) -> Result<Vec<Image>, BinaryReaderError> {
        validate_read("PS3", final_width, final_height, depth, mip_count)?;
        let layout = FormatLayout::of(dxgi).map_err(BinaryReaderError::custom)?;
        let deswizzle = dxgi == DxgiFormat::R8G8B8A8_UNORM || matches!(format, 9 | 16 | 26);

        let mut images = Vec::with_capacity(depth as usize);

        for _ in 0..depth {
            let mut image = Image::new();
            reader.align(0x80);
            for level in 0..mip_count {
                let (w, h) = padded_mip_size(final_width, final_height, level, &layout);
                let len = padded_mip_len(&layout, w, h);

                let mut mip = reader.read_bytes(to_usize(len)?)?;
                if deswizzle {
                    mip = Deswizzler::PS3(&mip, dxgi, w as u32, h as u32)
                        .map_err(|e| BinaryReaderError::custom(e.to_string()))?;
                }
                image.sub_images.push(mip);
            }
            images.push(image);
        }
        Ok(images)
    }

    #[allow(non_snake_case)]
    pub fn PS4(reader: &mut BinaryReader, final_width: i32, final_height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat, ty: TextureType) -> Result<Vec<Image>, BinaryReaderError> {
        validate_read("PS4", final_width, final_height, depth, mip_count)?;
        let layout = FormatLayout::of(dxgi).map_err(BinaryReaderError::custom)?;
        let bytes_per_block = layout.bytes_per_block as u64;

        let min_block_width = 8 * layout.block_width as i32;
        let min_block_height = 8 * layout.block_height as i32;
        let min_dimension = layout.block_width.max(layout.block_height) as i32;

        let source_len = reader.length();
        let slice_buffer_length = source_len / depth as u64;

        let mut images: Vec<Image> = (0..depth).map(|_| Image::new()).collect();

        let mut mip_width = final_width;
        let mut mip_height = final_height;

        // Preserve the full buffer for a single mip, as PS4 swizzling can
        // address padded data beyond the visible image.
        let mut buffer_length: u64 = if mip_count == 1 {
            slice_buffer_length
        } else {
            linear_size(layout.bits_per_texel, final_width, final_height)
        };

        let slice_buffer_min: u64 = if depth > 1 { 0x400 } else { bytes_per_block * 0x40 };
        buffer_length = buffer_length.max(slice_buffer_min);

        let mut buffer_used: u64 = 0;

        // PS4 textures seem to lay out slices at the same level sequentially rather than having slices
        // go through each mip in their set before proceeding to the next slice
        for mip in 0..mip_count {
            if (mip_count > 1 || depth > 1) && buffer_length != slice_buffer_min && mip != 0 {
                buffer_length = (buffer_length / 4).max(slice_buffer_min);
            }

            let (minimum_size, deswizzle_width, deswizzle_height) = get_deswizzle_size(
                layout.bits_per_texel,
                mip_width,
                mip_height,
                min_block_width,
                min_block_height,
            )
            .map_err(BinaryReaderError::custom)?;

            buffer_length = buffer_length.max(minimum_size);

            let read_len = to_usize(buffer_length)?;
            let visible_size = linear_size(layout.bits_per_texel, mip_width, mip_height);
            let expected_size = to_usize(block_aligned_size(&layout, mip_width, mip_height))?;

            for slice in 0..depth as usize {
                let end = buffer_used
                    .checked_add(buffer_length)
                    .ok_or_else(|| BinaryReaderError::custom("PS4 buffer offset overflow"))?;

                if end > source_len {
                    return Err(BinaryReaderError::custom(format!(
                        "PS4 texture buffer too small: mip={}, slice={}, requested={}, remaining={}",
                        mip,
                        slice,
                        buffer_length,
                        source_len.saturating_sub(buffer_used),
                    )));
                }

                reader.set_position(buffer_used)?;
                let mut mip_data = reader.read_bytes(read_len)?;
                buffer_used = end;

                if visible_size <= bytes_per_block {
                    // Tiny mips still require a complete block.
                    mip_data.resize(layout.bytes_per_block, 0);
                } else {
                    mip_data = Deswizzler::PS4(&mip_data, dxgi, deswizzle_width as u32, deswizzle_height as u32)
                        .map_err(|e| {
                            BinaryReaderError::custom(format!(
                                "PS4 deswizzle failed at mip {} slice {}: {}",
                                mip, slice, e
                            ))
                        })?;

                    // Extract as a tile from the pixels if we haven't done that at the deswizzle step
                    if deswizzle_width != mip_width || deswizzle_height != mip_height {
                        mip_data = Util::extract_tile(
                            &mip_data,
                            dxgi,
                            deswizzle_width as usize,
                            0,
                            0,
                            mip_width as usize,
                            mip_height as usize,
                        );
                    }

                    if mip_data.len() < expected_size {
                        return Err(BinaryReaderError::custom(format!(
                            "PS4 deswizzler returned {} bytes, expected {} for {:?} {}x{} at mip {}",
                            mip_data.len(),
                            expected_size,
                            dxgi,
                            mip_width,
                            mip_height,
                            mip,
                        )));
                    }
                    mip_data.truncate(expected_size);
                }

                images[slice].sub_images.push(mip_data);
            }

            get_next_mip_dimensions(min_dimension, &mut mip_width, &mut mip_height);

            // Cubemaps seem to pad to the size of 8 textures
            if ty == TextureType::Cubemap {
                let padding = 8u64
                    .checked_sub(depth as u64)
                    .and_then(|extra_slices| extra_slices.checked_mul(buffer_length))
                    .ok_or_else(|| BinaryReaderError::custom("PS4 cubemap padding overflow"))?;

                buffer_used = buffer_used
                    .checked_add(padding)
                    .ok_or_else(|| BinaryReaderError::custom("PS4 cubemap offset overflow"))?;
            }
        }

        Ok(images)
    }

    /// The texture buffers for internal mipmaps seemingly subdivide by 2 each time we go down a mip,
    /// UNTIL we reach 0x400. When the buffer should be 0x400, we instead skip to 0x200.
    /// All mipmap buffers after this will be 0x100 regardless of true size.
    /// While the buffers are larger than the actual texture size, the swizzling happens at the BUFFER
    /// level and thus reading the full buffer for deswizzling is paramount. - SFNext
    #[allow(non_snake_case)]
    pub fn PS5(reader: &mut BinaryReader, final_width: i32, final_height: i32, depth: i32, mip_count: i32, dxgi: DxgiFormat) -> Result<Vec<Image>, BinaryReaderError> {
        validate_read("PS5", final_width, final_height, depth, mip_count)?;
        let layout = FormatLayout::of(dxgi).map_err(BinaryReaderError::custom)?;
        let bytes_per_block = layout.bytes_per_block as u64;
        let min_dimension = layout.block_width.max(layout.block_height) as i32;

        let mut image_list: Vec<Image> = (0..depth).map(|_| Image::new()).collect();

        let slice_buffer_length = reader.length() / depth as u64;
        let calculated_buffer_length = linear_size(layout.bits_per_texel, final_width, final_height);

        for slice in 0..depth as usize {
            let mut mip_width = final_width;
            let mut mip_height = final_height;
            let mut buffer_length = slice_buffer_length;
            if mip_count > 1 {
                buffer_length /= 2;
            }

            // In some cases, we want the full buffer size because of overrun and the need for it in
            // deswizzling, but sometimes we want the calculated version since larger buffers don't have
            // padding, which means the smaller mips combined won't equal half the slice's buffer length
            buffer_length = buffer_length.max(calculated_buffer_length);

            let mut buffer_used: u64 = 0;
            for mip in 0..mip_count {
                if mip_count > 1 && buffer_length != 0x100 && mip != 0 {
                    buffer_length /= 2;

                    if buffer_length == 0x400 || buffer_length >= 0x10000 {
                        buffer_length /= 2;
                    }
                }

                // Make sure that we have enough bytes to actually deswizzle
                let (minimum_size, deswizzle_width, deswizzle_height) = get_deswizzle_size(
                    layout.bits_per_texel,
                    mip_width,
                    mip_height,
                    PS5_MIN_BLOCK_DIMENSION,
                    PS5_MIN_BLOCK_DIMENSION,
                )
                .map_err(BinaryReaderError::custom)?;

                buffer_length = buffer_length.max(minimum_size);
                buffer_used += buffer_length;

                let mip_offset = slice_buffer_length
                    .checked_mul((depth as usize - slice) as u64)
                    .and_then(|v| v.checked_sub(buffer_used))
                    .ok_or_else(|| BinaryReaderError::custom("PS5 mip offset underflowed"))?;

                reader.set_position(mip_offset)?;
                let mut mip_full = reader.read_bytes(to_usize(buffer_length)?)?;

                // If it's too small, we don't need to deswizzle
                if linear_size(layout.bits_per_texel, mip_width, mip_height) <= bytes_per_block {
                    let n = layout.bytes_per_block;
                    mip_full = mip_full
                        .get(..n)
                        .ok_or_else(|| BinaryReaderError::custom("PS5 mip buffer smaller than minimum pixel set"))?
                        .to_vec();
                } else {
                    mip_full = Deswizzler::PS5(&mip_full, dxgi, deswizzle_width as u32, deswizzle_height as u32, 1, 9)
                        .map_err(|e| BinaryReaderError::custom(e.to_string()))?;

                    // Extract as a tile from the pixels if we haven't done that at the deswizzle step
                    if deswizzle_width != mip_width || deswizzle_height != mip_height {
                        mip_full = Util::extract_tile(
                            &mip_full,
                            dxgi,
                            deswizzle_width as usize,
                            0,
                            0,
                            mip_width as usize,
                            mip_height as usize,
                        );
                    }
                }

                image_list[slice].sub_images.push(mip_full);
                get_next_mip_dimensions(min_dimension, &mut mip_width, &mut mip_height);
            }
        }

        Ok(image_list)
    }

}


impl Write {
    #[allow(non_snake_case)]
    pub fn PS3(images: &[Image], dds: &DDS, format: u8) -> Result<Vec<u8>, BinaryWriterError> {
        let max_mip_count = images.iter().map(|i| i.sub_images.len()).max().unwrap_or(0);

        let dxgi = dds.dxgi_format();
        let layout = FormatLayout::of(dxgi).map_err(BinaryWriterError::custom)?;
        let swizzle = dxgi == DxgiFormat::R8G8B8A8_UNORM || matches!(format, 9 | 16 | 26);

        let min_block_width = 8 * layout.block_width as i32;
        let min_block_height = 8 * layout.block_height as i32;
        let min_dimension = layout.block_width.max(layout.block_height) as i32;

        let mut writer = BinaryWriter::new(false, false);
        let mut width = dds.header.width;
        let mut height = dds.header.height;
        for m in 0..max_mip_count {
            let (buffer_length_min, _, _) = get_deswizzle_size(layout.bits_per_texel, width, height, min_block_width, min_block_height)
                .map_err(BinaryWriterError::custom)?;
            for img in images {
                let Some(source) = img.sub_images.get(m) else {
                    continue;
                };

                writer.pad_align(0x80);

                let swizzled;
                let buffer: &[u8] = if swizzle {
                    swizzled = Swizzler::PS3(source, dxgi, width as u32, height as u32, buffer_length_min as usize)
                        .map_err(|e| BinaryWriterError::custom(e.to_string()))?;
                    &swizzled
                } else {
                    source
                };

                writer.write_bytes(buffer)?;

                // Pad out mipmap buffers as needed
                if (buffer.len() as u64) < buffer_length_min {
                    writer.write_pattern(0u8, (buffer_length_min - buffer.len() as u64) as usize);
                }
            }

            get_next_mip_dimensions(min_dimension, &mut width, &mut height);
        }

        Ok(writer.into_inner())
    }

    #[allow(non_snake_case)]
    pub fn PS4(images: &mut Vec<Image>, dds: &DDS, ty: TextureType) -> Result<Vec<u8>, BinaryWriterError> {
        add_empty_image_padding(images, ty);
        let max_mip_count = images.iter().map(|i| i.sub_images.len()).max().unwrap_or(0);

        let dxgi = dds.dxgi_format();
        let layout = FormatLayout::of(dxgi).map_err(BinaryWriterError::custom)?;

        let min_block_width = 8 * layout.block_width as i32;
        let min_block_height = 8 * layout.block_height as i32;
        let min_dimension = layout.block_width.max(layout.block_height) as i32;

        let mut writer = BinaryWriter::new(false, false);
        let mut width = dds.header.width;
        let mut height = dds.header.height;
        for m in 0..max_mip_count {
            let (buffer_length_min, _, _) = get_deswizzle_size(layout.bits_per_texel, width, height, min_block_width, min_block_height)
                .map_err(BinaryWriterError::custom)?;
            for img in images.iter() {
                let Some(source) = img.sub_images.get(m) else {
                    continue;
                };

                let buffer = Swizzler::PS4(source, dxgi, width as u32, height as u32, buffer_length_min as usize)
                    .map_err(|e| BinaryWriterError::custom(e.to_string()))?;
                writer.write_bytes(&buffer)?;

                // Pad out mipmap buffers as needed
                if (buffer.len() as u64) < buffer_length_min {
                    writer.write_pattern(0u8, (buffer_length_min - buffer.len() as u64) as usize);
                }
            }

            get_next_mip_dimensions(min_dimension, &mut width, &mut height);
        }

        Ok(writer.into_inner())
    }

}

//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-
//Helpers:
//-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-.-


fn pad_to(value: i32, pad: i32) -> i32 {
    let value = value.max(0);
    (value.saturating_add(pad - 1) / pad * pad).max(pad)
}


/// Number of mip levels down to 1x1: `floor(log2(max(width, height))) + 1`.
pub fn get_mip_count(width: i32, height: i32) -> i32 {
    (i32::BITS - width.max(height).max(1).leading_zeros()) as i32
}


fn validate_read(platform: &str, width: i32, height: i32, depth: i32, mip_count: i32) -> Result<(), BinaryReaderError> {
    if width <= 0 || height <= 0 {
        return Err(BinaryReaderError::custom(format!(
            "{platform} texture dimensions must be positive, got {width}x{height}"
        )));
    }
    if depth <= 0 {
        return Err(BinaryReaderError::custom(format!("{platform} texture depth must be positive, got {depth}")));
    }
    if !(1..=MAX_MIP_COUNT).contains(&mip_count) {
        return Err(BinaryReaderError::custom(format!(
            "{platform} texture mip count must be between 1 and {MAX_MIP_COUNT}, got {mip_count}"
        )));
    }
    Ok(())
}


fn to_usize(value: u64) -> Result<usize, BinaryReaderError> {
    usize::try_from(value).map_err(|_| BinaryReaderError::custom("Buffer length exceeds addressable memory"))
}


/// `bits_per_texel * width * height / 8`.
fn linear_size(bits_per_texel: usize, width: i32, height: i32) -> u64 {
    (bits_per_texel as u64)
        .saturating_mul(width.max(0) as u64)
        .saturating_mul(height.max(0) as u64)
        / 8
}


/// Size of a mip in whole blocks, `ceil(width / block_width) * ceil(height / block_height) * bytes_per_block`.
fn block_aligned_size(layout: &FormatLayout, width: i32, height: i32) -> u64 {
    let blocks_wide = (width.max(0) as u64).div_ceil(layout.block_width as u64);
    let blocks_high = (height.max(0) as u64).div_ceil(layout.block_height as u64);
    blocks_wide * blocks_high * layout.bytes_per_block as u64
}


/// Dimensions of mip `level`, padded up to a whole number of blocks.
fn padded_mip_size(final_width: i32, final_height: i32, level: i32, layout: &FormatLayout) -> (i32, i32) {
    (
        pad_to(final_width >> level, layout.block_width as i32),
        pad_to(final_height >> level, layout.block_height as i32),
    )
}


/// Length of a padded mip in bytes, never smaller than a single block.
fn padded_mip_len(layout: &FormatLayout, width: i32, height: i32) -> u64 {
    linear_size(layout.bits_per_texel, width, height).max(layout.bytes_per_block as u64)
}


pub fn rebuild_pixel_data(bytes: &[u8], dxgi: DxgiFormat, width: i32, height: i32, depth: i32, mip_count: i32, ty: TextureType, platform: TPFPlatform, format: u8) -> Result<Vec<Image>, BinaryReaderError> {
    Read::auto(platform, bytes, width, height, depth, mip_count, dxgi, ty, format)
}


fn get_next_mip_dimensions(min_dimension: i32, mip_width: &mut i32, mip_height: &mut i32) {
    *mip_width = if *mip_width > min_dimension { *mip_width / 2 } else { min_dimension };
    *mip_height = if *mip_height > min_dimension { *mip_height / 2 } else { min_dimension };
}


/// PS4 textures require some extra, blank images to ensure they function properly.
fn add_empty_image_padding(images: &mut Vec<Image>, ty: TextureType) {
    if ty == TextureType::Cubemap && !images.is_empty() {
        while images.len() < 8 {
            let mut padding = Image::new();
            for img in &images[0].sub_images {
                padding.sub_images.push(vec![0u8; img.len()]);
            }
            images.push(padding);
        }
    }
}


/// Returns (minimum buffer length, deswizzle width, deswizzle height).
fn get_deswizzle_size(bits_per_texel: usize, width: i32, height: i32, min_block_width: i32, min_block_height: i32) -> Result<(u64, i32, i32), &'static str> {
    if bits_per_texel == 0 || width <= 0 || height <= 0 || min_block_width <= 0 || min_block_height <= 0 {
        return Err("Invalid dimensions or format for deswizzling");
    }

    fn round_up(value: i32, alignment: i32) -> Option<i32> {
        value
            .checked_add(alignment - 1)?
            .checked_div(alignment)?
            .checked_mul(alignment)
    }

    let de_w = round_up(width, min_block_width)
        .ok_or("Deswizzle dimension overflow")?
        .max(min_block_width);
    let de_h = round_up(height, min_block_height)
        .ok_or("Deswizzle dimension overflow")?
        .max(min_block_height);

    let size = (bits_per_texel as u64)
        .checked_mul(de_w as u64)
        .and_then(|v| v.checked_mul(de_h as u64))
        .ok_or("Deswizzle buffer size overflow")?
        / 8;

    Ok((size, de_w, de_h))
}

/// Grab a dds's texture buffer data. May lower `dds.header.mipmap_count` if the data runs out.
pub fn get_dds_texture_buffers(dds: &mut DDS, dds_bytes: &[u8]) -> Result<Vec<Image>, BinaryReaderError> {
    let layout = FormatLayout::of(dds.dxgi_format()).map_err(BinaryReaderError::custom)?;
    let min_image_size = layout.bytes_per_block as i64;
    let full_image_size = linear_size(layout.bits_per_texel, dds.header.width, dds.header.height) as i64;

    let mut reader = BinaryReader::new(dds_bytes, false, false);
    reader.set_position(dds.data_offset() as u64)?;
    let mut image_list = Vec::new();

    if dds.header.caps2.contains(DDSCAPS2::Volume) {
        let mut depth = dds.header.depth;
        let mut m = 0;
        while m < dds.header.mipmap_count {
            let mut img = Image::new();
            let mut image_size = full_image_size;
            for _ in 0..depth {
                if reader.position() + image_size as u64 > reader.length() {
                    dds.header.mipmap_count = m; // fix if needed
                    break;
                }

                img.sub_images.push(reader.read_bytes(image_size as usize)?);
                image_size = (image_size / 4).max(min_image_size);
            }

            // After we hit 1 depth, we should continue with 1 map for each subsequent mip
            if depth != 1 {
                depth /= 2;
            }
            image_list.push(img);
            m += 1;
        }
    } else {
        // We can read CubeMaps and standard textures together
        let mut depth = 1;
        // If someone tries to put in a screwy cubemap with less than 6 textures
        if dds.header.caps2.contains(DDSCAPS2::Cubemap) {
            depth = 6;
        } else if dds.header.depth > 1 {
            depth = dds.header.depth;
        }

        for _ in 0..depth {
            let mut img = Image::new();
            let mut image_size = full_image_size;
            let mut m = 0;
            while m < dds.header.mipmap_count {
                if reader.position() + image_size as u64 > reader.length() {
                    // Fix mipmapcount if we have to
                    dds.header.mipmap_count = m;
                    break;
                }

                img.sub_images.push(reader.read_bytes(image_size as usize)?);
                image_size = (image_size / 4).max(min_image_size);
                m += 1;
            }
            image_list.push(img);
        }
    }

    Ok(image_list)
}

#[derive(Debug, Clone, Default)]
pub struct Image {
    /// Used for a particular image's mipmap, or for all images of a particular miplevel for a
    /// volume texture.
    pub sub_images: Vec<Vec<u8>>,
}

impl Image {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(images: &[Image]) -> Vec<u8> {
        images.iter().flat_map(|i| i.sub_images.iter()).flatten().copied().collect()
    }
}
