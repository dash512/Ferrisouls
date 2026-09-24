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
    pub fn PS3(deswizzled: &mut Vec<u8>, dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        const MIN_DATA_SIZE: usize = 0usize;

        let (bits_per_pixel, pixel_block_size, dds_bytes_per_pixel_set) = dxgi_format.info().to_tuple();
        if dds_bytes_per_pixel_set as usize >= deswizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {dds_bytes_per_pixel_set} bytes).")
            ));
        }

        // Pad deswizzled data to minimum size if necessary.
        let deswizzled_len = deswizzled.len();
        pad_vec(deswizzled, MIN_DATA_SIZE, deswizzled_len);

        let swizzled_size = max((width * height * bits_per_pixel as u32) / 8, MIN_DATA_SIZE as u32);
        let mut swizzled: Vec<u8> = vec![0u8; swizzled_size as usize];

        let sy = height / pixel_block_size as u32;
        let sx = width / pixel_block_size as u32;

        for src_tile_i in 0..sx*sy {
            // Identical to PS3 deswizzling.
            let dest_tile_i = Morton8::index(src_tile_i as usize, sx as usize, sy as usize);
            let deswizzled_start = src_tile_i * dds_bytes_per_pixel_set as u32;
            let deswizzled_tile = &deswizzled[deswizzled_start as usize..deswizzled_start as usize + dds_bytes_per_pixel_set as usize];
            let swizzled_start = dest_tile_i * dds_bytes_per_pixel_set as usize;
            swizzled[swizzled_start..swizzled_start + dds_bytes_per_pixel_set as usize].copy_from_slice(deswizzled_tile);

        }
        Ok(swizzled)
    }

    ///Swizzle PS4 DDS bytes.
    #[allow(non_snake_case)]
    pub fn PS4(deswizzled: &mut Vec<u8>, dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        const MIN_DATA_SIZE: usize = 0x200usize;

        let (_bits_per_pixel, pixel_block_size, dds_bytes_per_pixel_set) = dxgi_format.info().to_tuple();
        if dds_bytes_per_pixel_set as usize >= deswizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {dds_bytes_per_pixel_set} bytes).")
            ));
        }

        let sx = width.div_ceil(pixel_block_size as u32).max(1);
        let sy = height.div_ceil(pixel_block_size as u32).max(1);

        let linear_size = sx as usize * sy as usize * dds_bytes_per_pixel_set as usize;

        if deswizzled.len() < linear_size {
            return Err(SwizzleError::new(
                "DDS texture is smaller than the expected linear texture size."
                    .to_string(),
            ));
        }
        
        let deswizzled = &deswizzled[..linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let out_size = std::cmp::max(
            macro_x_count as usize
                * macro_y_count as usize
                * 64
                * dds_bytes_per_pixel_set as usize,
            MIN_DATA_SIZE,
        );
        let mut swizzled = vec![0u8; out_size];

        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let src = (y as usize * sx as usize + x as usize) * dds_bytes_per_pixel_set as usize;

                        swizzled[stream_pos..stream_pos + dds_bytes_per_pixel_set as usize]
                            .copy_from_slice(
                                &deswizzled[src..src + dds_bytes_per_pixel_set as usize],
                            );
                    }
                    stream_pos += dds_bytes_per_pixel_set as usize
                }
            }
        }
        Ok(swizzled)

    }

}


impl Deswizzler {
    ///Deswizzle PS3 DDS bytes. Logic from Soulstruct.
    #[allow(non_snake_case)]
    pub fn PS3(swizzled: &mut Vec<u8>, dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let (bits_per_pixel, pixel_block_size, dds_bytes_per_pixel_set) = dxgi_format.info().to_tuple();
        if dds_bytes_per_pixel_set as usize >= swizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {dds_bytes_per_pixel_set} bytes).")
            ));
        }

        // Pad deswizzled data to minimum size if necessary.
        let deswizzled_size = max((width * height * bits_per_pixel as u32) / 8, dds_bytes_per_pixel_set as u32);
        let mut deswizzled: Vec<u8> = vec![0u8; deswizzled_size as usize];

        let sy = height / pixel_block_size as u32;
        let sx = width / pixel_block_size as u32;

        for src_tile_i in 0..sx*sy {
            // Identical to PS3 deswizzling.
            let dest_tile_i = Morton8::index(src_tile_i as usize, sx as usize, sy as usize);
            let swizzled_start = src_tile_i * dds_bytes_per_pixel_set as u32;
            let swizzled_tile = &swizzled[swizzled_start as usize..swizzled_start as usize + dds_bytes_per_pixel_set as usize];
            let deswizzled_start = dest_tile_i * dds_bytes_per_pixel_set as usize;
            deswizzled[deswizzled_start..deswizzled_start + dds_bytes_per_pixel_set as usize].copy_from_slice(swizzled_tile);

        }
        Ok(deswizzled)
    }

    ///Deswizzle PS4 DDS bytes.
    #[allow(non_snake_case)]
    pub fn PS4(swizzled: &mut Vec<u8>, dxgi_format: DxgiFormat, width: u32, height: u32) -> Result<Vec<u8>, SwizzleError> {
        let (_bits_per_pixel, pixel_block_size, dds_bytes_per_pixel_set) = dxgi_format.info().to_tuple();
        if dds_bytes_per_pixel_set as usize >= swizzled.len() {
            return Err(SwizzleError::new(
                format!("DDS texture is too small to contain a single pixel set (expected {dds_bytes_per_pixel_set} bytes).")
            ));
        }

        let sx = width.div_ceil(pixel_block_size as u32).max(1);
        let sy = height.div_ceil(pixel_block_size as u32).max(1);

        let linear_size = sx as usize * sy as usize * dds_bytes_per_pixel_set as usize;
        let mut deswizzled = vec![0u8; linear_size];

        let macro_x_count = sx.div_ceil(8);
        let macro_y_count = sy.div_ceil(8);

        let mut stream_pos = 0usize;

        for macro_y in 0..macro_y_count {
            for macro_x in 0..macro_x_count {
                for t in 0..64 {
                    let (tile_x, tile_y) = Morton8::decode(t);

                    if stream_pos + dds_bytes_per_pixel_set as usize > swizzled.len() {
                        return Ok(deswizzled)
                    }

                    let x = macro_x * 8 + tile_x;
                    let y = macro_y * 8 + tile_y;

                    if x < sx && y < sy {
                        let dst = (y as usize * sx as usize + x as usize) * dds_bytes_per_pixel_set as usize;

                        deswizzled[dst..dst + dds_bytes_per_pixel_set as usize]
                            .copy_from_slice(
                                &swizzled[stream_pos..stream_pos + dds_bytes_per_pixel_set as usize],
                            );

                    }
                    stream_pos += dds_bytes_per_pixel_set as usize
                }
            }
        }
        Ok(deswizzled)

    }

}


pub fn pad_vec(data: &mut Vec<u8>, min_size: usize, length: usize) {
    if min_size < length {
        return ();
    }

    let padding = b"\0".repeat(min_size - length);
    data.extend(padding);
}


#[cfg(test)]
mod tests {
    use std::error::Error;

use super::*;

    #[test]
    fn test_swizzle() {

    }
}
