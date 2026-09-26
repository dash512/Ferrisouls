use std::cmp::{max, min};
use std::error::Error;
use std::fmt;

use crate::textures::dxgi::DxgiFormat;
use crate::textures::utils::{Morton8, MortonAlgorithms};

#[derive(Debug, Clone)]
pub struct SwizzleError {
    msg: String,
}

impl SwizzleError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}

impl Error for SwizzleError {}

impl fmt::Display for SwizzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}


pub struct Swizzler;

pub struct Deswizzler;

impl Swizzler {
    ///Swizzle PS3 DDS bytes. Logic from Soulstruct.
    #[allow(non_snake_case)]
    pub fn PS3(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        const MIN_DATA_SIZE: usize = 0usize;

        let (bits_per_texel, bytes_per_block, texels_per_block) = dxgi_format.info().to_tuple().unwrap();
        let texels_per_block = texels_per_block as usize;
        let block_size = bytes_per_block as u32;

        if texels_per_block >= deswizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {texels_per_block} bytes).")
            ));
        }

        // Pad deswizzled data to minimum size if necessary.
        let deswizzled_len = deswizzled.len();
        let deswizzled: Vec<u8> = pad_vec(deswizzled, MIN_DATA_SIZE, deswizzled_len);

        let swizzled_size = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(bits_per_texel as u32))
            .and_then(|n| n.checked_div(8))
            .ok_or_else(|| SwizzleError::new("Linear texture size overflowed."))?;
        let mut swizzled: Vec<u8> = vec![0u8; swizzled_size as usize];

        let sy = (height / block_size) as usize;
        let sx = (width / block_size) as usize;

        for src_tile_i in 0..sx*sy {
            // Identical to PS3 deswizzling.
            let dest_tile_i = Morton8::index(src_tile_i, sx, sy);
            let deswizzled_start = src_tile_i * texels_per_block;
            let deswizzled_tile = &deswizzled[deswizzled_start..deswizzled_start + texels_per_block];
            let swizzled_start = dest_tile_i * texels_per_block;
            swizzled[swizzled_start..swizzled_start + texels_per_block].copy_from_slice(deswizzled_tile);

        }
        Ok(swizzled)
    }

    ///Swizzle PS4 DDS bytes.
    #[allow(non_snake_case)]
    pub fn PS4(deswizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        const MIN_DATA_SIZE: usize = 0x200usize;

        let (_bits_per_texel, bytes_per_block, texels_per_block) = dxgi_format.info().to_tuple().unwrap();
        let texels_per_block = texels_per_block as usize;
        let block_size = bytes_per_block as u32;

        if texels_per_block >= deswizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {texels_per_block} bytes).")
            ));
        }

        let sx = width.div_ceil(block_size).max(1) as usize;
        let sy = height.div_ceil(block_size).max(1) as usize;

        let linear_size = sx
            .checked_mul(sy)
            .and_then(|n| n.checked_mul(texels_per_block as usize))
            .ok_or_else(|| SwizzleError::new("Linear texture size overflowed."))?;

        if deswizzled.len() < linear_size {
            return Err(SwizzleError::new(
                "DDS texture is smaller than the expected linear texture size."
                    .to_string(),
            ));
        }
        
        let deswizzled = &deswizzled[..linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let out_size = macro_x_count
            .checked_mul(macro_y_count)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_mul(texels_per_block))
            .unwrap_or(usize::MAX)
            .max(MIN_DATA_SIZE);

        let mut swizzled = vec![0u8; out_size];
        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let src = (y * sx + x) * texels_per_block;

                        swizzled[stream_pos..stream_pos + texels_per_block]
                            .copy_from_slice(
                                &deswizzled[src..src + texels_per_block],
                            );
                    }
                    stream_pos += texels_per_block
                }
            }
        }
        Ok(swizzled)

    }

}


impl Deswizzler {
    ///Deswizzle PS3 DDS bytes. Logic from Soulstruct.
    #[allow(non_snake_case)]
    pub fn PS3(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let (bits_per_texel, bytes_per_block, texels_per_block) = dxgi_format.info().to_tuple().unwrap();
        let texels_per_block = texels_per_block as usize;
        let block_size = bytes_per_block as u32;
        
        if texels_per_block >= swizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {texels_per_block} bytes).")
            ));
        }

        // Pad deswizzled data to minimum size if necessary.
        let deswizzled_size = max((width * height * bits_per_texel as u32) / 8, texels_per_block as u32);
        let mut deswizzled: Vec<u8> = vec![0u8; deswizzled_size as usize];

        let sy = (height / block_size) as usize;
        let sx = (width / block_size) as usize;

        for src_tile_i in 0..sx*sy {
            // Identical to PS3 deswizzling.
            let dest_tile_i = Morton8::index(src_tile_i, sx, sy);
            let swizzled_start = src_tile_i * texels_per_block;
            let swizzled_tile = &swizzled[swizzled_start..swizzled_start + texels_per_block];
            let deswizzled_start = dest_tile_i * texels_per_block;
            deswizzled[deswizzled_start..deswizzled_start + texels_per_block].copy_from_slice(swizzled_tile);

        }
        Ok(deswizzled)
    }

    ///Deswizzle PS4 DDS bytes.
    #[allow(non_snake_case)]
    pub fn PS4(swizzled: &[u8], dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let (_bits_per_texel, bytes_per_block, texels_per_block) = dxgi_format.info().to_tuple().unwrap();
        let texels_per_block = texels_per_block as usize;
        let block_size = bytes_per_block as u32;
        
        if texels_per_block >= swizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {texels_per_block} bytes).")
            ));
        }

        let sx = width.div_ceil(block_size).max(1) as usize;
        let sy = height.div_ceil(block_size).max(1) as usize;

        let linear_size = sx * sy * texels_per_block;
        let mut deswizzled = vec![0u8; linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    if stream_pos + texels_per_block > swizzled.len() {
                        return Ok(deswizzled)
                    }

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let dst = (y * sx + x) * texels_per_block;

                        deswizzled[dst..dst + texels_per_block]
                            .copy_from_slice(
                                &swizzled[stream_pos..stream_pos + texels_per_block],
                            );

                    }
                    stream_pos += texels_per_block
                }
            }
        }
        Ok(deswizzled)

    }

}


pub fn pad_vec(data: &[u8], min_size: usize, length: usize) -> Vec<u8> {
    if min_size < length {
        return data.to_vec();
    }
    let mut output = Vec::<u8>::new();
    let padding = b"\0".repeat(min_size - length);
    output.extend(padding);
    output
}


#[cfg(test)]
mod tests {
    use std::error::Error;

use super::*;

    #[test]
    fn test_swizzle() {

    }
}
