//! Module for handling swizzled console textures.
//! 
//! Logic for this module is HEAVILY inspired by and copied from [DrSwizzler](https://github.com/Shadowth117/DrSwizzler), 
//! to the point of essentially just being a port.
//! 
//! None of this could I have done on my own so massive credit to [Shadowth117](https://github.com/Shadowth117/).

use crate::textures::dxgi::DxgiFormat;
use crate::textures::utils::{Morton8, MortonAlgorithms, PS5Util, SwitchUtil, VitaUtil, Xbox360Util};
use crate::errors::SwizzleError;


/// Layout of a DXGI format in the order of information needed.
#[derive(Debug, Clone, Copy)]
pub struct FormatLayout {
    pub bits_per_texel: usize,
    pub bytes_per_block: usize,
    pub block_width: usize,
    pub block_height: usize,
}

impl FormatLayout {
    pub fn of(dxgi_format: DxgiFormat) -> Result<Self, String> {
        let info = dxgi_format.info();
        let (bits_per_texel, bytes_per_block, _) = info.to_tuple().map_err(|e| e.to_string())?;
        let (block_width, block_height) = info.block_dimensions();

        if bits_per_texel == 0 || bytes_per_block == 0 || block_width == 0 || block_height == 0 {
            return Err(format!("Format {:?} has no usable block layout.", dxgi_format));
        }

        Ok(Self {
            bits_per_texel: bits_per_texel as usize,
            bytes_per_block: bytes_per_block as usize,
            block_width: block_width as usize,
            block_height: block_height as usize,
        })
    }
}


pub struct Swizzler;

pub struct Deswizzler;

fn swizzle_err(message: &str) -> SwizzleError {
    SwizzleError::swizzle(message.to_string())
}

fn deswizzle_err(message: &str) -> SwizzleError {
    SwizzleError::deswizzle(message.to_string())
}

impl Swizzler {
    ///Swizzle PS3 DDS bytes. Logic from Soulstruct.
    #[allow(non_snake_case)]
    pub fn PS3(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32, min_data_size: usize) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= deswizzled.len() {
            return Ok(pad_vec(deswizzled, min_data_size));
        }

        let deswizzled = pad_vec(deswizzled, min_data_size);

        let swizzled_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| swizzle_err("Linear texture size overflowed."))?;
        let mut swizzled = vec![0u8; swizzled_size];

        let sx = width as usize / layout.block_width;
        let sy = height as usize / layout.block_height;

        for tile_i in 0..sx * sy {
            let linear_tile_i = Morton8::index(tile_i, sx, sy);
            let src = linear_tile_i * bytes_per_block;
            let dst = tile_i * bytes_per_block;

            let tile = deswizzled
                .get(src..src + bytes_per_block)
                .ok_or_else(|| swizzle_err("Source offset out of range."))?;
            swizzled
                .get_mut(dst..dst + bytes_per_block)
                .ok_or_else(|| swizzle_err("Destination offset out of range."))?
                .copy_from_slice(tile);
        }
        Ok(swizzled)
    }

    ///Swizzle PS4 DDS bytes. `min_data_size` is `0x200usize` by default
    #[allow(non_snake_case)]
    pub fn PS4(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32, min_data_size: usize) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= deswizzled.len() {
            return Ok(deswizzled.to_vec());
        }

        let sx = (width as usize).div_ceil(layout.block_width).max(1);
        let sy = (height as usize).div_ceil(layout.block_height).max(1);

        let linear_size = sx
            .checked_mul(sy)
            .and_then(|n| n.checked_mul(bytes_per_block))
            .ok_or_else(|| swizzle_err("Linear texture size overflowed."))?;

        if deswizzled.len() < linear_size {
            return Err(swizzle_err("DDS texture is smaller than the expected linear texture size."));
        }

        let deswizzled = &deswizzled[..linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let out_size = macro_x_count
            .checked_mul(macro_y_count)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_mul(bytes_per_block))
            .ok_or_else(|| swizzle_err("Swizzled texture size overflowed."))?
            .max(min_data_size);

        let mut swizzled = vec![0u8; out_size];
        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let src = (y * sx + x) * bytes_per_block;

                        swizzled[stream_pos..stream_pos + bytes_per_block]
                            .copy_from_slice(&deswizzled[src..src + bytes_per_block]);
                    }
                    stream_pos += bytes_per_block;
                }
            }
        }
        Ok(swizzled)
    }

    ///Swizzle PS5 DDS bytes. `depth` is 1 for 2D textures, `tile_mode` is typically 9.
    #[allow(non_snake_case)]
    pub fn PS5(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32, depth: u32, tile_mode: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;

        PS5Util::transfer(deswizzled, layout.bytes_per_block, layout.block_width, width, height, depth, tile_mode, true)
            .map_err(SwizzleError::swizzle)
    }

    ///Swizzle Xbox360 DDS bytes.
    #[allow(non_snake_case)]
    pub fn Xbox360(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;

        if layout.bytes_per_block >= deswizzled.len() {
            return Ok(deswizzled.to_vec());
        }

        Xbox360Util::transfer(deswizzled, dxgi_format, width, height, layout.block_width, layout.bytes_per_block, false)
            .map_err(SwizzleError::swizzle)
    }

    ///Swizzle Switch DDS bytes.
    #[allow(non_snake_case)]
    pub fn Switch(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= deswizzled.len() {
            return Ok(deswizzled.to_vec());
        }

        let out_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| swizzle_err("Linear texture size overflowed."))?;
        let mut swizzled = vec![0u8; out_size];

        let sy = height as usize / layout.block_height;
        let sx = width as usize / layout.block_width;
        let (gob_height, per_unit) = SwitchUtil::params(sy, bytes_per_block);
        if gob_height == 0 || per_unit == 0 {
            return Err(swizzle_err("Texture is too short to contain a single Switch GOB (8 block rows)."));
        }

        let mut stream_pos = 0usize;
        for i1 in 0..sy / 8 / gob_height {
            for i2 in 0..sx / 4 / per_unit {
                for i3 in 0..gob_height {
                    for &gob_entry in SwitchUtil::SWITCH_GOB.iter() {
                        for i5 in 0..per_unit {
                            let row = (i1 * gob_height + i3) * 8 + gob_entry / 4;
                            let col = (i2 * 4 + gob_entry % 4) * per_unit + i5;
                            let src = bytes_per_block * (row * sx + col);

                            let src_bytes = deswizzled
                                .get(src..src + bytes_per_block)
                                .ok_or_else(|| swizzle_err("Source offset out of range."))?;
                            swizzled
                                .get_mut(stream_pos..stream_pos + bytes_per_block)
                                .ok_or_else(|| swizzle_err("Destination offset out of range."))?
                                .copy_from_slice(src_bytes);
                            stream_pos += bytes_per_block;
                        }
                    }
                }
            }
        }
        Ok(swizzled)
    }

    ///Swizzle Vita DDS bytes.
    #[allow(non_snake_case)]
    pub fn Vita(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= deswizzled.len() {
            return Ok(deswizzled.to_vec());
        }

        let out_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| swizzle_err("Linear texture size overflowed."))?;
        let mut swizzled = vec![0u8; out_size];

        let w = width as usize / layout.block_width;
        let h = height as usize / layout.block_height;
        if w == 0 || h == 0 {
            return Err(swizzle_err("Texture is smaller than one block."));
        }
        let max_u = w.ilog2();
        let max_v = h.ilog2();

        let mut j = 0usize;
        while j < w * h && j * bytes_per_block < deswizzled.len() {
            let (u, v) = VitaUtil::vita_decode(j, max_u, max_v);
            if u < w && v < h {
                let src = (v * w + u) * bytes_per_block;
                let dst = j * bytes_per_block;
                let src_bytes = deswizzled
                    .get(src..src + bytes_per_block)
                    .ok_or_else(|| swizzle_err("Source offset out of range."))?;
                swizzled
                    .get_mut(dst..dst + bytes_per_block)
                    .ok_or_else(|| swizzle_err("Destination offset out of range."))?
                    .copy_from_slice(src_bytes);
            }
            j += 1;
        }
        Ok(swizzled)
    }
}


impl Deswizzler {
    ///Deswizzle PS3 DDS bytes. Logic from Soulstruct.
    #[allow(non_snake_case)]
    pub fn PS3(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::swizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= swizzled.len() {
            return Ok(swizzled.to_vec());
        }

        let deswizzled_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| deswizzle_err("Linear texture size overflowed."))?;
        let mut deswizzled = vec![0u8; deswizzled_size];

        let sx = width as usize / layout.block_width;
        let sy = height as usize / layout.block_height;

        for tile_i in 0..sx * sy {
            let linear_tile_i = Morton8::index(tile_i, sx, sy);
            let src = tile_i * bytes_per_block;
            let dst = linear_tile_i * bytes_per_block;

            let tile = swizzled
                .get(src..src + bytes_per_block)
                .ok_or_else(|| deswizzle_err("Source offset out of range."))?;
            deswizzled
                .get_mut(dst..dst + bytes_per_block)
                .ok_or_else(|| deswizzle_err("Destination offset out of range."))?
                .copy_from_slice(tile);
        }
        Ok(deswizzled)
    }

    ///Deswizzle PS4 DDS bytes.
    #[allow(non_snake_case)]
    pub fn PS4(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::deswizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= swizzled.len() {
            return Ok(swizzled.to_vec());
        }

        let sx = (width as usize).div_ceil(layout.block_width).max(1);
        let sy = (height as usize).div_ceil(layout.block_height).max(1);

        let linear_size = sx
            .checked_mul(sy)
            .and_then(|n| n.checked_mul(bytes_per_block))
            .ok_or_else(|| deswizzle_err("Linear texture size overflowed."))?;
        let mut deswizzled = vec![0u8; linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    if stream_pos + bytes_per_block > swizzled.len() {
                        return Ok(deswizzled);
                    }

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let dst = (y * sx + x) * bytes_per_block;
                        deswizzled[dst..dst + bytes_per_block]
                            .copy_from_slice(&swizzled[stream_pos..stream_pos + bytes_per_block]);
                    }
                    stream_pos += bytes_per_block;
                }
            }
        }
        Ok(deswizzled)
    }

    ///Deswizzle PS5 DDS bytes. `depth` is 1 for 2D textures, `tile_mode` is typically 9.
    #[allow(non_snake_case)]
    pub fn PS5(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32, depth: u32, tile_mode: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::deswizzle)?;

        PS5Util::transfer(swizzled, layout.bytes_per_block, layout.block_width, width, height, depth, tile_mode, false)
            .map_err(SwizzleError::deswizzle)
    }

    ///Deswizzle Xbox360 DDS bytes.
    #[allow(non_snake_case)]
    pub fn Xbox360(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::deswizzle)?;

        if layout.bytes_per_block >= swizzled.len() {
            return Ok(swizzled.to_vec());
        }

        Xbox360Util::transfer(swizzled, dxgi_format, width, height, layout.block_width, layout.bytes_per_block, true)
            .map_err(SwizzleError::deswizzle)
    }

    ///Deswizzle Switch DDS bytes.
    #[allow(non_snake_case)]
    pub fn Switch(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::deswizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= swizzled.len() {
            return Ok(swizzled.to_vec());
        }

        let out_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| deswizzle_err("Linear texture size overflowed."))?;
        let mut deswizzled = vec![0u8; out_size];

        let sy = height as usize / layout.block_height;
        let sx = width as usize / layout.block_width;
        let (gob_height, per_unit) = SwitchUtil::params(sy, bytes_per_block);
        if gob_height == 0 || per_unit == 0 {
            return Err(deswizzle_err("Texture is too short to contain a single Switch GOB (8 block rows)."));
        }

        let mut stream_pos = 0usize;
        for i1 in 0..sy / 8 / gob_height {
            for i2 in 0..sx / 4 / per_unit {
                for i3 in 0..gob_height {
                    for &gob_entry in SwitchUtil::SWITCH_GOB.iter() {
                        for i5 in 0..per_unit {
                            let src_bytes = swizzled
                                .get(stream_pos..stream_pos + bytes_per_block)
                                .ok_or_else(|| deswizzle_err("Source offset out of range."))?;
                            stream_pos += bytes_per_block;

                            let row = (i1 * gob_height + i3) * 8 + gob_entry / 4;
                            let col = (i2 * 4 + gob_entry % 4) * per_unit + i5;
                            let dst = bytes_per_block * (row * sx + col);

                            deswizzled
                                .get_mut(dst..dst + bytes_per_block)
                                .ok_or_else(|| deswizzle_err("Destination offset out of range."))?
                                .copy_from_slice(src_bytes);
                        }
                    }
                }
            }
        }
        Ok(deswizzled)
    }

    ///Deswizzle Vita DDS bytes.
    #[allow(non_snake_case)]
    pub fn Vita(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let layout = FormatLayout::of(dxgi_format).map_err(SwizzleError::deswizzle)?;
        let bytes_per_block = layout.bytes_per_block;

        if bytes_per_block >= swizzled.len() {
            return Ok(swizzled.to_vec());
        }

        let out_size = calc_buffer_size(layout.bits_per_texel, width, height, 0, bytes_per_block)
            .ok_or_else(|| deswizzle_err("Linear texture size overflowed."))?;
        let mut deswizzled = vec![0u8; out_size];

        let w = width as usize / layout.block_width;
        let h = height as usize / layout.block_height;
        if w == 0 || h == 0 {
            return Err(deswizzle_err("Texture is smaller than one block."));
        }
        let max_u = w.ilog2();
        let max_v = h.ilog2();

        let mut j = 0usize;
        while j < w * h && j * bytes_per_block < swizzled.len() {
            let (u, v) = VitaUtil::vita_decode(j, max_u, max_v);
            if u < w && v < h {
                let src = j * bytes_per_block;
                let dst = (v * w + u) * bytes_per_block;
                let src_bytes = swizzled
                    .get(src..src + bytes_per_block)
                    .ok_or_else(|| deswizzle_err("Source offset out of range."))?;
                deswizzled
                    .get_mut(dst..dst + bytes_per_block)
                    .ok_or_else(|| deswizzle_err("Destination offset out of range."))?
                    .copy_from_slice(src_bytes);
            }
            j += 1;
        }
        Ok(deswizzled)
    }
}


///Pad bytes to a minimum length and return as a vector.
pub fn pad_vec(data: &[u8], min_size: usize) -> Vec<u8> {
    let mut vector = data.to_vec();

    if vector.len() < min_size {
        vector.resize(min_size, 0);
    }
    vector
}

///Calculate linear buffer size in bytes.
/// 
///`bits_per_texel * width * height / 8`, floored by `min` and `at_least`.
fn calc_buffer_size(bits_per_texel: usize, width: u32, height: u32, min: usize, at_least: usize) -> Option<usize> {
    let size = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(bits_per_texel)?
        / 8;
    Some(size.max(min).max(at_least))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps4_roundtrip_bc7() {
        let (w, h) = (64u32, 64u32);
        let len = (w / 4 * h / 4 * 16) as usize;
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();

        let swizzled = Swizzler::PS4(&data, DxgiFormat::BC7_UNORM, w, h, 0).unwrap();
        let back = Deswizzler::PS4(&swizzled, DxgiFormat::BC7_UNORM, w, h).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn ps3_roundtrip_rgba8() {
        let (w, h) = (32u32, 32u32);
        let len = (w * h * 4) as usize;
        let data: Vec<u8> = (0..len).map(|i| (i % 253) as u8).collect();

        let swizzled = Swizzler::PS3(&data, DxgiFormat::R8G8B8A8_UNORM, w, h, 0).unwrap();
        let back = Deswizzler::PS3(&swizzled, DxgiFormat::R8G8B8A8_UNORM, w, h).unwrap();
        assert_eq!(back, data);
    }
}