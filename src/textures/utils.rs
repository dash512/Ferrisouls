//! Utils for swizzling. Includes Morton/Z-Order implementations as well as per-platform utilities.
//! 
//! The latter is essentially a direct port of Shadowth117's [DrSwizzler](https://github.com/Shadowth117/DrSwizzler).
//! I could not have done this on my own so once again massive thanks and credit to those who came before me.

use std::cmp;
use crate::textures::{dxgi::DxgiFormat, swizzling::FormatLayout};

pub trait MortonAlgorithms {
    ///Convert a Morton (Z-order) index to (x, y) block coordinates. 
    ///  
    ///Decodes a linear Morton index into its corresponding block coordinates inside the tile. 
    ///  
    ///For example: 0 -> (0, 0) || 1 -> (1, 0) || 2 -> (0, 1) || 3 -> (1, 1) || 4 -> (2, 0) || ...
    fn encode(x: usize, y: usize) -> usize {
        let mut result = 0;

        for bit in 0..16 {
            result |= ((x >> bit) & 1) << (bit * 2);
            result |= ((y >> bit) & 1) << (bit * 2 + 1);
        }

        result
    }

    ///Reverse decode. Converts (x, y) block coordinates to a Z-order index.
    fn decode(i: usize) -> (usize, usize) {
        let mut x = 0;
        let mut y = 0;

        for bit in 0..16 {
            x |= ((i >> (bit * 2)) & 1) << bit;
            y |= ((i >> (bit * 2 + 1)) & 1) << bit;
        }

        (x, y)
    }

    ///Converts swizzled index -> row-major index. Inputs should be powers of 2.
    fn index(t: usize, sx: usize, sy: usize) -> usize {
        debug_assert!(sx.is_power_of_two());
        debug_assert!(sy.is_power_of_two());

        let mut x = 0;
        let mut y = 0;

        let x_bits = sx.trailing_zeros() as usize;
        let y_bits = sy.trailing_zeros() as usize;

        let mut t_bit = 0;

        for bit in 0..x_bits.max(y_bits) {
            if bit < x_bits {
                x |= ((t >> t_bit) & 1) << bit;
                t_bit += 1;
            }

            if bit < y_bits {
                y |= ((t >> t_bit) & 1) << bit;
                t_bit += 1;
            }
        }

        y * sx + x
    }

    ///Converts row-major index -> swizzled index. Inputs should be powers of 2.
    fn reverse_index(i: usize, sx: usize, sy: usize) -> usize {
        debug_assert!(sx.is_power_of_two());
        debug_assert!(sy.is_power_of_two());

        let x = i % sx;
        let y = i / sx;

        let x_bits = sx.trailing_zeros() as usize;
        let y_bits = sy.trailing_zeros() as usize;

        let mut t = 0;
        let mut t_bit = 0;

        for bit in 0..x_bits.max(y_bits) {
            if bit < x_bits {
                t |= ((x >> bit) & 1) << t_bit;
                t_bit += 1;
            }

            if bit < y_bits {
                t |= ((y >> bit) & 1) << t_bit;
                t_bit += 1;
            }
        }

        t
    }


}


pub struct Morton;

impl MortonAlgorithms for Morton {}

impl Morton {
    pub fn iter(from: usize, to: usize) -> impl Iterator<Item = (usize, usize)> {
        (from..to).map(|x| Self::decode(x))
    }
}

///Optimized morton implementation for 8x8 tiles as used in Bloodborne textures.
pub struct Morton8;

impl MortonAlgorithms for Morton8 {
    /// Decode an 8×8 Morton index into `(x, y)`.
    ///
    /// `i` must be in the range `0..256`.
    #[inline]
    fn decode(i: usize) -> (usize, usize) {
        fn compact1by1(mut n: usize) -> usize {
            n &= 0x55;
            n = (n ^ (n >> 1)) & 0x33;
            n = (n ^ (n >> 2)) & 0x0F;
            n
        }

        (compact1by1(i), compact1by1(i >> 1))
    }

    /// Encode `(x, y)` coordinates into an 8×8 Morton index.
    ///
    /// `x` and `y` must be in the range `0..8`.
    #[inline]
    fn encode(x: usize, y: usize) -> usize {
        fn part1by1(mut n: usize) -> usize {
            n &= 0x07;
            n = (n | (n << 2)) & 0x33;
            n = (n | (n << 1)) & 0x55;
            n
        }

        part1by1(x) | (part1by1(y) << 1)
    }
}

impl Morton8 {
    pub fn iter() -> impl Iterator<Item = (usize, usize)> {
        (0usize..64usize).map(|x| Self::decode(x))
    }
}


//-----------------------------------------------------------------------------------------------------------
//Functions needed for special handling based on platform. All logic and notes from Shadowth117's DrSwizzler.
//--------------------------------------------------------------------------------------------------------------

pub struct PS5Util;

impl PS5Util {
    /// Extra high bits the 64KB families add on top of the 4KB patterns (indexed by log2(bytes per element)).
    pub const PS5_SOURCES: [[usize; 4]; 5] = [
        [4, 4, 4, 5],
        [3, 4, 4, 4],
        [3, 3, 4, 4],
        [3, 3, 3, 4],
        [2, 3, 3, 3],
    ];

    /// Block element dimensions and byte size for the given element size,
    /// tile mode, and thick/thin (volume) selection.
    ///
    /// 1 Depth: 1 = 256b, 5 = 4kb, 9 = 64kb.
    /// Volume: 5 = 4kb3D, 9 = 64kb3D.
    ///
    /// Returns (block_width, block_height, block_depth, block_size).
    pub fn block_layout(bytes_per_pixel: u32, tile_mode: u32, volume: bool) -> Result<(usize, usize, usize, usize), String> {
        let mut bpe_index = 0usize;

        while (1u32 << bpe_index) < bytes_per_pixel {
            bpe_index += 1;
        }

        let layout: (i32, i32, i32, usize) = match (tile_mode, volume) {
            (1, false) => (
                [16, 16, 8, 8, 4][bpe_index],
                [16, 8, 8, 4, 4][bpe_index],
                1,
                0x100,
            ),
            (5, false) => (
                [64, 64, 32, 32, 16][bpe_index],
                [64, 32, 32, 16, 16][bpe_index],
                1,
                0x1000,
            ),
            (5, true) => (
                [16, 8, 8, 8, 4][bpe_index],
                [16, 16, 16, 8, 8][bpe_index],
                [16, 16, 8, 8, 8][bpe_index],
                0x1000,
            ),
            (9, false) => (
                [256, 256, 128, 128, 64][bpe_index],
                [256, 128, 128, 64, 64][bpe_index],
                1,
                0x10000,
            ),
            (9, true) => (
                [64, 32, 32, 32, 16][bpe_index],
                [32, 32, 32, 16, 16][bpe_index],
                [32, 32, 16, 16, 16][bpe_index],
                0x10000,
            ),
            _ => {
                return Err(format!(
                    "Unsupported Gen5 tile mode {} for {} surfaces!",
                    tile_mode,
                    if volume { "volume" } else { "thin" }
                ));
            }
        };

        Ok((
            layout.0 as usize,
            layout.1 as usize,
            layout.2 as usize,
            layout.3,
        ))
    }

    /// Gen5 4KB thin block bit scatter.
    /// Based on Kyty's Gen5Standard4KBOffsetInBlock.
    pub fn thin_4k_offset_in_block(x: usize, y: usize, bpe: usize) -> usize {
        let mut o = 0;

        match bpe {
            1 => {
                o ^= (y << 4) & 0x1f0;
                o ^= (y << 5) & 0x400;
                o ^= x & 0xf;
                o ^= (x << 5) & 0x200;
                o ^= (x << 6) & 0x800;
            }
            2 => {
                o ^= (y << 4) & 0x70;
                o ^= (y << 5) & 0x100;
                o ^= (y << 6) & 0x400;
                o ^= (x << 1) & 0xe;
                o ^= (x << 4) & 0x80;
                o ^= (x << 5) & 0x200;
                o ^= (x << 6) & 0x800;
            }
            4 => {
                o ^= (y << 4) & 0x70;
                o ^= (y << 5) & 0x100;
                o ^= (y << 6) & 0x400;
                o ^= (x << 2) & 0xc;
                o ^= (x << 5) & 0x80;
                o ^= (x << 6) & 0x200;
                o ^= (x << 7) & 0x800;
            }
            8 => {
                o ^= (y << 4) & 0x30;
                o ^= (y << 6) & 0x100;
                o ^= (y << 7) & 0x400;
                o ^= (x << 3) & 0x8;
                o ^= (x << 5) & 0xc0;
                o ^= (x << 6) & 0x200;
                o ^= (x << 7) & 0x800;
            }
            16 => {
                o ^= (y << 4) & 0x30;
                o ^= (y << 6) & 0x100;
                o ^= (y << 7) & 0x400;
                o ^= (x << 6) & 0xc0;
                o ^= (x << 7) & 0x200;
                o ^= (x << 8) & 0x800;
            }
            _ => {}
        }

        o
    }

    /// Gen5 64KB thin block bit scatter.
    /// Returns the byte offset within the 64KB block directly.
    pub fn thin_64k_offset_in_block(x: usize, y: usize, bpe: usize) -> usize {
        let mut byte_offset = 0;

        match bpe {
            1 => {
                byte_offset ^= x & 0xf;
                byte_offset ^= (x << 5) & 0x200;
                byte_offset ^= (x << 6) & 0x800;
                byte_offset ^= (x << 7) & 0x2000;
                byte_offset ^= (x << 8) & 0x8000;
                byte_offset ^= (y << 4) & 0x1f0;
                byte_offset ^= (y << 5) & 0x400;
                byte_offset ^= (y << 6) & 0x1000;
                byte_offset ^= (y << 7) & 0x4000;
            }
            2 => {
                byte_offset ^= (x << 1) & 0xe;
                byte_offset ^= (x << 4) & 0x80;
                byte_offset ^= (x << 5) & 0x200;
                byte_offset ^= (x << 6) & 0x800;
                byte_offset ^= (x << 7) & 0x2000;
                byte_offset ^= (x << 8) & 0x8000;
                byte_offset ^= (y << 4) & 0x70;
                byte_offset ^= (y << 5) & 0x100;
                byte_offset ^= (y << 6) & 0x400;
                byte_offset ^= (y << 7) & 0x1000;
                byte_offset ^= (y << 8) & 0x4000;
            }
            4 => {
                byte_offset ^= (x << 2) & 0xc;
                byte_offset ^= (x << 5) & 0x80;
                byte_offset ^= (x << 6) & 0x200;
                byte_offset ^= (x << 7) & 0x800;
                byte_offset ^= (x << 8) & 0x2000;
                byte_offset ^= (x << 9) & 0x8000;
                byte_offset ^= (y << 4) & 0x70;
                byte_offset ^= (y << 5) & 0x100;
                byte_offset ^= (y << 6) & 0x400;
                byte_offset ^= (y << 7) & 0x1000;
                byte_offset ^= (y << 8) & 0x4000;
            }
            8 => {
                byte_offset ^= (x << 3) & 0x8;
                byte_offset ^= (x << 5) & 0xc0;
                byte_offset ^= (x << 6) & 0x200;
                byte_offset ^= (x << 7) & 0x800;
                byte_offset ^= (x << 8) & 0x2000;
                byte_offset ^= (x << 9) & 0x8000;
                byte_offset ^= (y << 4) & 0x30;
                byte_offset ^= (y << 6) & 0x100;
                byte_offset ^= (y << 7) & 0x400;
                byte_offset ^= (y << 8) & 0x1000;
                byte_offset ^= (y << 9) & 0x4000;
            }
            16 => {
                byte_offset ^= (x << 6) & 0xc0;
                byte_offset ^= (x << 7) & 0x200;
                byte_offset ^= (x << 8) & 0x800;
                byte_offset ^= (x << 9) & 0x2000;
                byte_offset ^= (x << 10) & 0x8000;
                byte_offset ^= (y << 4) & 0x30;
                byte_offset ^= (y << 6) & 0x100;
                byte_offset ^= (y << 7) & 0x400;
                byte_offset ^= (y << 8) & 0x1000;
                byte_offset ^= (y << 9) & 0x4000;
            }
            _ => {}
        }

        byte_offset
    }

    /// 4KB volume block bit scatter.
    /// Also provides the base pattern for 64KB volume blocks.
    pub fn volume_4k_offset_in_block(x: usize, y: usize, z: usize, bpe: usize) -> usize {
        let mut o = 0;

        match bpe {
            1 => {
                o ^= x & 0x3;
                o ^= (x << 4) & 0x40;
                o ^= (x << 6) & 0x200;
                o ^= (y << 3) & 0x8;
                o ^= (y << 4) & 0x20;
                o ^= (y << 6) & 0x100;
                o ^= (y << 8) & 0x800;
                o ^= (z << 2) & 0x4;
                o ^= (z << 3) & 0x10;
                o ^= (z << 5) & 0x80;
                o ^= (z << 7) & 0x400;
            }
            2 => {
                o ^= (x << 1) & 0x2;
                o ^= (x << 5) & 0x40;
                o ^= (x << 7) & 0x200;
                o ^= (y << 3) & 0x8;
                o ^= (y << 4) & 0x20;
                o ^= (y << 6) & 0x100;
                o ^= (y << 8) & 0x800;
                o ^= (z << 2) & 0x4;
                o ^= (z << 3) & 0x10;
                o ^= (z << 5) & 0x80;
                o ^= (z << 7) & 0x400;
            }
            4 => {
                o ^= (x << 2) & 0x4;
                o ^= (x << 5) & 0x40;
                o ^= (x << 7) & 0x200;
                o ^= (y << 3) & 0x8;
                o ^= (y << 4) & 0x20;
                o ^= (y << 6) & 0x100;
                o ^= (y << 8) & 0x800;
                o ^= (z << 4) & 0x10;
                o ^= (z << 6) & 0x80;
                o ^= (z << 8) & 0x400;
            }
            8 => {
                o ^= (x << 3) & 0x8;
                o ^= (x << 5) & 0x40;
                o ^= (x << 7) & 0x200;
                o ^= (y << 5) & 0x20;
                o ^= (y << 7) & 0x100;
                o ^= (y << 9) & 0x800;
                o ^= (z << 4) & 0x10;
                o ^= (z << 6) & 0x80;
                o ^= (z << 8) & 0x400;
            }
            16 => {
                o ^= (x << 6) & 0x40;
                o ^= (x << 8) & 0x200;
                o ^= (y << 5) & 0x20;
                o ^= (y << 7) & 0x100;
                o ^= (y << 9) & 0x800;
                o ^= (z << 4) & 0x10;
                o ^= (z << 6) & 0x80;
                o ^= (z << 8) & 0x400;
            }
            _ => {}
        }

        o
    }

    #[inline]
    pub fn bit(value: usize, source: usize, destination: usize) -> usize {
        ((value >> source) & 1) << destination
    }


    /// Shared PS5 pass. `tile == true` swizzles (linear -> tiled), `false` deswizzles (tiled -> linear).
    /// 
    /// `depth` is 1 for 2D textures; `tile_mode` is 9 (64KB thin) by default in DrSwizzler so it is here as well.
    pub fn transfer(input: &[u8], bpe: usize, block_size_px: usize, width: u32, height: u32, depth: u32, tile_mode: u32, tile: bool) -> Result<Vec<u8>, String> {
        if !bpe.is_power_of_two() || bpe > 16 {
            return Err(format!("Unsupported element size {bpe}!"));
        }
        if block_size_px == 0 {
            return Err("Invalid block size.".to_string());
        }
        let bpe_index = bpe.trailing_zeros() as usize;
        let depth = depth as usize;
        let is_volume = depth > 1;

        let (block_width, block_height, block_depth, block_size) = Self::block_layout(bpe as u32, tile_mode, is_volume)?;
        if block_width == 0 || block_height == 0 || block_depth == 0 {
            return Err("Invalid PS5 block layout.".to_string());
        }

        // Element dimensions (a 4x4 texel block is one element for block compressed formats)
        let elem_width = (width as usize).div_ceil(block_size_px);
        let elem_height = (height as usize).div_ceil(block_size_px);
        let elem_depth = if is_volume { depth } else { 1 };

        let padded_width = elem_width.div_ceil(block_width) * block_width;
        let padded_height = elem_height.div_ceil(block_height) * block_height;
        let blocks_per_row = padded_width / block_width;
        let block_slice_count = if is_volume { depth.div_ceil(block_depth) } else { 1 };
        let block_slice_size = block_depth
            .checked_mul(padded_width)
            .and_then(|n| n.checked_mul(padded_height))
            .and_then(|n| n.checked_mul(bpe))
            .ok_or_else(|| "Texture size overflowed.".to_string())?;
        let slice_linear_size = elem_width
            .checked_mul(elem_height)
            .and_then(|n| n.checked_mul(bpe))
            .ok_or_else(|| "Texture size overflowed.".to_string())?;

        let out_len = if tile {
            block_slice_size.checked_mul(block_slice_count)
        } else {
            slice_linear_size.checked_mul(elem_depth)
        }
        .ok_or_else(|| "Texture size overflowed.".to_string())?;
        let mut output = vec![0u8; out_len];

        for z in 0..elem_depth {
            let z_block_slice = if is_volume { z / block_depth } else { 0 };
            let z_in_block = if is_volume { z % block_depth } else { 0 };
            let slice_base = z * slice_linear_size;
            let block_slice_base = z_block_slice * block_slice_size;

            for y in 0..elem_height {
                let block_y = y / block_height;
                let y_in_block = y % block_height;
                let linear_row = slice_base + y * elem_width * bpe;

                for x in 0..elem_width {
                    let block_x = x / block_width;
                    let x_in_block = x % block_width;

                    let mut offset_in_block;
                    if is_volume {
                        // Thick 3D scatter. The 64KB family adds four high bits on top.
                        offset_in_block = Self::volume_4k_offset_in_block(x_in_block, y_in_block, z_in_block, bpe);
                        if block_size == 0x10000 {
                            offset_in_block ^= Self::bit(x_in_block, Self::PS5_SOURCES[bpe_index][0], 12);
                            offset_in_block ^= Self::bit(z_in_block, Self::PS5_SOURCES[bpe_index][1], 13);
                            offset_in_block ^= Self::bit(y_in_block, Self::PS5_SOURCES[bpe_index][2], 14);
                            offset_in_block ^= Self::bit(x_in_block, Self::PS5_SOURCES[bpe_index][3], 15);
                        }
                    } else if tile_mode == 9 {
                        offset_in_block = Self::thin_64k_offset_in_block(x_in_block, y_in_block, bpe);
                    } else {
                        offset_in_block = Self::thin_4k_offset_in_block(x_in_block, y_in_block, bpe);
                        if tile_mode == 1 {
                            offset_in_block &= 0xFF;
                        }
                    }

                    let block = block_x + blocks_per_row * block_y;
                    let tiled = block_slice_base + block * block_size + offset_in_block;
                    let linear = linear_row + x * bpe;
                    let (src, dst) = if tile { (linear, tiled) } else { (tiled, linear) };

                    if src + bpe > input.len() || dst + bpe > output.len() {
                        return Ok(output);
                    }
                    output[dst..dst + bpe].copy_from_slice(&input[src..src + bpe]);
                }
            }
        }
        Ok(output)
    }

}


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwitchVersion {
    Switch1,
    Switch2
}

impl SwitchVersion {
    ///Order of 16-byte units inside a 512-byte GOB (64 bytes wide x 8 rows).
    ///Each entry is `row * 4 + column`, indexed by position in the GOB.
    pub fn gob(&self) -> [usize;32] {
        match self {
            Self::Switch1 => [
                0, 4, 1, 5, 8, 12, 9, 13, 16, 20, 17, 21, 24, 28, 25, 29,
                2, 6, 3, 7, 10, 14, 11, 15, 18, 22, 19, 23, 26, 30, 27, 31,
            ],

            Self::Switch2 => [
                0, 4, 8, 12, 1, 5, 9, 13, 16, 20, 24, 28, 17, 21, 25, 29,
                2, 6, 10, 14, 3, 7, 11, 15, 18, 22, 26, 30, 19, 23, 27, 31
            ]
        }
    }
}

pub struct SwitchUtil;

impl SwitchUtil {
    ///Maximum number of 8-row GOB groups handled per outer pass.
    pub const MAX_GOB_BATCH: usize = 16;

    pub fn validate_order(order: &[usize; 32]) -> bool {
        let mut seen = [false; 32];
        order.iter().all(|&e| e < 32 && !std::mem::replace(&mut seen[e], true))
    }

    ///Number of block columns consumed per swizzle unit. Four of these make one
    /// 64-byte-wide tile row (16B -> 1, 8B -> 2, 4B -> 4, 2B -> 8, 1B -> 16).
    pub fn columns_per_unit(bytes_per_block: usize) -> usize {
        (16 / bytes_per_block.max(1)).max(1)
    }

    ///Returns (max GOB groups per batch, block columns per swizzle unit).
    pub fn params(sy: usize, bytes_per_block: usize) -> (usize, usize) {
        let gob_height = (sy / 8).min(Self::MAX_GOB_BATCH);
        (gob_height, Self::columns_per_unit(bytes_per_block))
    }

    ///Crops the top-left `tile_width` x `tile_height` pixel region out of a block
    /// buffer whose full row is `buffer_width` pixels wide (port of ExtractTileASTC).
    pub fn extract_tile_astc(tex: &[u8], block_bytes: usize, footprint_w: usize, footprint_h: usize, buffer_width: usize, tile_width: usize, tile_height: usize) -> Option<Vec<u8>> {
        let padded_blocks_x = buffer_width / footprint_w;
        let blocks_x = tile_width.div_ceil(footprint_w);
        let blocks_y = tile_height.div_ceil(footprint_h);
        let row_bytes = blocks_x.checked_mul(block_bytes)?;
        let mut out = vec![0u8; row_bytes.checked_mul(blocks_y)?];

        for y in 0..blocks_y {
            let src = y * padded_blocks_x * block_bytes;
            out[y * row_bytes..(y + 1) * row_bytes].copy_from_slice(tex.get(src..src + row_bytes)?);
        }
        Some(out)
    }

    ///Crops a `tile_width` x `tile_height` pixel region (from the top left) out of a block buffer whose
    /// rows are `buffer_width` pixels wide. Works for non square block footprints (ExtractTileASTC).
    pub fn extract_tile_blocks(layout: &FormatLayout, data: &[u8], buffer_width: i32, tile_width: i32, tile_height: i32) -> Result<Vec<u8>, &'static str> {
        let bytes_per_block = layout.bytes_per_block;
        let padded_blocks_x = buffer_width.max(0) as usize / layout.block_width;
        let blocks_x = (tile_width.max(0) as usize).div_ceil(layout.block_width);
        let blocks_y = (tile_height.max(0) as usize).div_ceil(layout.block_height);

        let row_len = blocks_x * bytes_per_block;
        let mut out = vec![0u8; row_len * blocks_y];
        for y in 0..blocks_y {
            let src = y * padded_blocks_x * bytes_per_block;
            let row = data.get(src..src + row_len).ok_or("Tile extraction ran past the end of the buffer")?;
            out[y * row_len..(y + 1) * row_len].copy_from_slice(row);
        }
        Ok(out)
    }

    ///Copies a tightly packed level into a buffer with the row stride and height of the tile aligned grid.
    pub fn pad_to_swizzle_grid(layout: &FormatLayout, source: &[u8], width: i32, height: i32, de_w: i32, de_h: i32) -> Result<Vec<u8>, &'static str> {
        let bytes_per_block = layout.bytes_per_block;
        let src_blocks_x = (width.max(0) as usize).div_ceil(layout.block_width);
        let src_blocks_y = (height.max(0) as usize).div_ceil(layout.block_height);
        let dst_blocks_x = de_w.max(0) as usize / layout.block_width;
        let dst_blocks_y = de_h.max(0) as usize / layout.block_height;

        let src_row = src_blocks_x * bytes_per_block;
        let dst_row = dst_blocks_x * bytes_per_block;
        let total = dst_row.checked_mul(dst_blocks_y).ok_or("Switch 2 swizzle buffer size overflowed")?;

        let mut out = vec![0u8; total];
        for y in 0..src_blocks_y.min(dst_blocks_y) {
            let s = y * src_row;
            if s >= source.len() {
                break;
            }
            let n = src_row.min(dst_row).min(source.len() - s);
            out[y * dst_row..y * dst_row + n].copy_from_slice(&source[s..s + n]);
        }
        Ok(out)
    }

    ///Size of a level as written. Single mip, single slice textures pad block rows to a multiple of 128
    /// instead of the next power of two when that is smaller.
    pub fn get_write_size(layout: &FormatLayout, width: i32, height: i32, single_level_single_slice: bool) -> Result<(u64, i32, i32), &'static str> {
        let (mut size, de_w, mut de_h) = Self::get_deswizzle_size_wh(layout, width, height)?;

        if single_level_single_slice {
            let block_rows = (height as usize).div_ceil(layout.block_height);
            let mut padded_rows = de_h as usize / layout.block_height;
            let mult128 = ((block_rows + 127) / 128 * 128).max(8);
            if mult128 < padded_rows {
                padded_rows = mult128;
                de_h = (padded_rows * layout.block_height) as i32;
                size = (de_w as u64 / layout.block_width as u64) * layout.bytes_per_block as u64 * padded_rows as u64;
            }
        }

        Ok((size, de_w, de_h))
    }

    ///Get swizzle size with pixel block width and height instead of assuming square texels.
    /// Returns (buffer length, deswizzle width, deswizzle height).
    pub fn get_deswizzle_size_wh(layout: &FormatLayout, width: i32, height: i32) -> Result<(u64, i32, i32), &'static str> {
        let bytes_per_block = layout.bytes_per_block;
        if bytes_per_block == 0 || layout.block_width == 0 || layout.block_height == 0 || width <= 0 || height <= 0 {
            return Err("Invalid dimensions or format for Switch 2 deswizzling");
        }

        // Block columns, padded so that each block row is a multiple of 64 bytes wide
        let blocks_per_row_min = (64 / bytes_per_block).max(1);
        let blocks_x = (width as usize)
            .div_ceil(layout.block_width)
            .next_multiple_of(blocks_per_row_min)
            .max(blocks_per_row_min);

        // Block rows, padded to the next power of two (minimum of one full tile)
        let blocks_y = (height as usize).div_ceil(layout.block_height).max(8).next_power_of_two();

        let de_w = blocks_x.checked_mul(layout.block_width).and_then(|v| i32::try_from(v).ok());
        let de_h = blocks_y.checked_mul(layout.block_height).and_then(|v| i32::try_from(v).ok());
        let size = (blocks_x as u64)
            .checked_mul(bytes_per_block as u64)
            .and_then(|v| v.checked_mul(blocks_y as u64));

        match (size, de_w, de_h) {
            (Some(size), Some(w), Some(h)) => Ok((size, w, h)),
            _ => Err("Switch 2 deswizzle size overflowed"),
        }
    }
}


pub struct VitaUtil;

impl VitaUtil {
    ///Special Morton decode for the Vita.
    /// 
    ///Bits are taken v-first, then u, alternating.
    pub fn vita_decode(j: usize, max_u: u32, max_v: u32) -> (usize, usize) {
        let (mut u, mut v) = (0usize, 0usize);
        let mut orig = j;
        let mut k = 0;
        while k < max_u || k < max_v {
            if k < max_v {
                v |= (orig & 1) << k;
                orig >>= 1;
            }
            if k < max_u {
                u |= (orig & 1) << k;
                orig >>= 1;
            }
            k += 1;
        }
        (u, v)
    }
}


pub struct Xbox360Util;

impl Xbox360Util {
    ///Swap every pair of bytes (Xbox 360 is big endian). A trailing odd byte is left untouched.
    pub fn byte_swap_16(raw: &[u8]) -> Vec<u8> {
        let mut out = raw.to_vec();
        for pair in out.chunks_exact_mut(2) {
            pair.swap(0, 1);
        }
        out
    }

    ///Returns (align_x, align_y) in pixels. (0, 0) means the format is unsupported.
    pub fn get_align(format: DxgiFormat) -> (usize, usize) {
        match format {
            DxgiFormat::A8_UNORM | DxgiFormat::R8_TYPELESS | DxgiFormat::R8_UNORM
            | DxgiFormat::R8_SNORM | DxgiFormat::R8_UINT | DxgiFormat::R8_SINT => (64, 64),

            DxgiFormat::R8G8_TYPELESS | DxgiFormat::R8G8_UNORM | DxgiFormat::R8G8_SNORM | DxgiFormat::R8G8_UINT => (64, 32),
            DxgiFormat::R8G8B8A8_TYPELESS | DxgiFormat::R8G8B8A8_UNORM
            | DxgiFormat::R8G8B8A8_UNORM_SRGB | DxgiFormat::R8G8B8A8_SNORM | DxgiFormat::R8G8B8A8_UINT | DxgiFormat::R8G8B8A8_SINT
            | DxgiFormat::B8G8R8A8_TYPELESS | DxgiFormat::B8G8R8A8_UNORM | DxgiFormat::B8G8R8A8_UNORM_SRGB => (32, 32),

            DxgiFormat::BC1_TYPELESS | DxgiFormat::BC1_UNORM | DxgiFormat::BC1_UNORM_SRGB 
            | DxgiFormat::BC2_TYPELESS | DxgiFormat::BC2_UNORM | DxgiFormat::BC2_UNORM_SRGB
            | DxgiFormat::BC3_TYPELESS | DxgiFormat::BC3_UNORM | DxgiFormat::BC3_UNORM_SRGB
            //TODO: is this correct for BC4/5?
            | DxgiFormat::BC4_TYPELESS | DxgiFormat::BC4_UNORM 
            | DxgiFormat::BC5_TYPELESS | DxgiFormat::BC5_UNORM => (128, 128),
            // P8 and everything else: no known alignment.
            _ => (0, 0),
        }
    }

    pub fn align_up(ptr: usize, alignment: usize) -> usize {
        (ptr + alignment - 1) & !(alignment - 1)
    }

    ///XGAddress2DTiledOffset() from the XDK.
    /// 
    ///x/y: block coordinate, width: image width in blocks, log_bpb: log2(bytes per block).
    pub fn tiled_offset(x: usize, y: usize, width: usize, log_bpb: u32) -> usize {
        let aligned_width = Self::align_up(width, 32);
        // top bits of coordinates
        let macro_ = ((x >> 5) + (y >> 5) * (aligned_width >> 5)) << (log_bpb + 7);
        // lower bits of coordinates (result is 6-bit value)
        let micro = ((x & 7) + ((y & 0xE) << 2)) << log_bpb;
        // mix micro/macro + add few remaining x/y bits
        let offset = macro_ + ((micro & !0xF) << 1) + (micro & 0xF) + ((y & 1) << 4);
        // mix bits again
        (((offset & !0x1FF) << 3) // upper bits (offset bits [*-9])
            + ((y & 16) << 7)// next 1 bit
            + ((offset & 0x1C0) << 2)// next 3 bits (offset bits [8-6])
            + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6)// next 2 bits
            + (offset & 0x3F)) // lower 6 bits (offset bits [5-0])
            >> log_bpb
    }

    /// Shared tile/untile pass. `untile == false` tiles (swizzles), `true` untiles (deswizzles).
    pub fn transfer(input: &[u8], format: DxgiFormat, width: u32, height: u32, block_size: usize, bytes_per_block: usize, untile: bool) -> Result<Vec<u8>, String> {
        if block_size == 0 || bytes_per_block == 0 {
            return Err("Invalid block size for Xbox 360 texture.".to_string());
        }
        let (align_x, align_y) = Self::get_align(format);
        if align_x == 0 || align_y == 0 {
            return Err(format!("Xbox 360 alignment is not known for format {format:?}."));
        }

        let width1 = Self::align_up(width as usize, align_x);
        let height1 = Self::align_up(height as usize, align_y);
        let size = (width1 / block_size)
            .checked_mul(height1 / block_size)
            .and_then(|n| n.checked_mul(bytes_per_block))
            .ok_or_else(|| "Texture size overflowed.".to_string())?;

        // Copy into a buffer of the aligned size (padded with zeroes / truncated).
        let mut padded = vec![0u8; size];
        let n = input.len().min(size);
        padded[..n].copy_from_slice(&input[..n]);
        let mut output = vec![0u8; size];

        let block_width = width1 / block_size;
        let original_block_width = width as usize / block_size;
        let block_height = height1 / block_size;
        let log_bpb = bytes_per_block.ilog2();

        for y in 0..block_height {
            for x in 0..original_block_width {
                let addr = Self::tiled_offset(x, y, block_width, log_bpb);
                let sy = addr / block_width;
                let sx = addr % block_width;

                let linear = (y * original_block_width + x) * bytes_per_block;
                let tiled = (sy * block_width + sx) * bytes_per_block;
                let (src, dst) = if untile { (tiled, linear) } else { (linear, tiled) };

                let src_bytes = padded
                    .get(src..src + bytes_per_block)
                    .ok_or_else(|| "Xbox 360 source offset out of range.".to_string())?;
                output
                    .get_mut(dst..dst + bytes_per_block)
                    .ok_or_else(|| "Xbox 360 destination offset out of range.".to_string())?
                    .copy_from_slice(src_bytes);
            }
        }
        Ok(output)
    }

}


pub struct Util;

impl Util {
    /// Extracts a tile from a pixel buffer. Expects tile dimensions divisible by two.
    pub fn extract_tile(
        tex_buffer: &[u8],
        pixel_format: DxgiFormat,
        tex_buffer_total_width: usize,
        tile_leftmost_pixel: usize,
        tile_topmost_pixel: usize,
        tile_width: usize,
        tile_height: usize
    ) -> Vec<u8> {
        let (format_bpp, pixel_set_size, _) = pixel_format.info().to_tuple().unwrap();
        let (block_width, _) = pixel_format.info().block_dimensions();  

        Self::extract_tile_raw(
            tex_buffer,
            tex_buffer_total_width,
            tile_leftmost_pixel,
            tile_topmost_pixel,
            tile_width,
            tile_height,
            pixel_set_size as usize,
            block_width as usize,
            format_bpp as i32,
        )
    }

    pub fn extract_tile_raw(
        tex_buffer: &[u8],
        mut tex_buffer_total_width: usize,
        mut tile_leftmost_pixel: usize,
        mut tile_topmost_pixel: usize,
        mut tile_width: usize,
        mut tile_height: usize,
        pixel_set_size: usize,
        pixel_block_size: usize,
        format_bpp: i32,
    ) -> Vec<u8> {
        let output_len = ((format_bpp as usize)
            .saturating_mul(tile_width)
            .saturating_mul(tile_height))
            / 8;

        let mut tile_buffer = vec![0u8; output_len];

        if pixel_block_size == 4 {
            tile_height /= 4;
            tile_topmost_pixel /= 4;
            tile_leftmost_pixel /= 4;
            tex_buffer_total_width /= 4;
            tile_width /= 4;
        }

        for i in tile_topmost_pixel..tile_height {
            let row_start = tile_leftmost_pixel
                .saturating_mul(pixel_set_size)
                .saturating_add(
                    i.saturating_mul(tex_buffer_total_width)
                        .saturating_mul(pixel_set_size),
                );

            let row_len = pixel_set_size.saturating_mul(tile_width);
            let dest_start = i.saturating_mul(row_len);

            // Avoid out-of-bounds access if the supplied dimensions or buffer size are invalid.
            if row_start >= tex_buffer.len() || dest_start >= tile_buffer.len() {
                break;
            }

            let copy_len = cmp::min(
                row_len,
                cmp::min(
                    tex_buffer.len() - row_start,
                    tile_buffer.len() - dest_start,
                ),
            );

            tile_buffer[dest_start..dest_start + copy_len]
                .copy_from_slice(&tex_buffer[row_start..row_start + copy_len]);
        }

        tile_buffer
    }

    ///Calculates dimensions from a pixel buffer size and aspect ratio.
    pub fn dimensions_from_buffer(buffer_size: usize, pixel_size: usize, aspect_width: i32, aspect_height: i32) -> (i32, i32) {
        Self::get_dimensions(
            (buffer_size / pixel_size.max(1)) as i32,
            aspect_width,
            aspect_height,
        )
    }

    ///Calculates dimensions from an area and aspect ratio.
    pub fn get_dimensions(area: i32, aspect_width: i32, aspect_height: i32) -> (i32, i32) {
        if aspect_width == 0 || area < 0 || aspect_height < 0 {
            return (0, 0);
        }

        let mult_factor_width = area * aspect_width / aspect_width;
        let mult_factor_height = area * aspect_height / aspect_width;

        let width = (mult_factor_width as f64).sqrt() as i32;
        let height = (mult_factor_height as f64).sqrt() as i32;

        (width, height)
    }

    ///(blocks_x, blocks_y, padded_x, padded_y), all in blocks.
    pub fn padded_grid(width: u32, height: u32, bw: usize, bh: usize, bpb: usize) -> (usize, usize, usize, usize) {
        let blocks_x = (width as usize).div_ceil(bw);
        let blocks_y = (height as usize).div_ceil(bh);
        let unit_cols = 4 * SwitchUtil::columns_per_unit(bpb);
        (blocks_x, blocks_y, blocks_x.next_multiple_of(unit_cols), blocks_y.next_multiple_of(8))
    }

    ///Pad bytes to a minimum length and return as a vector.
    pub fn expand_array(data: &[u8], min_size: usize) -> Vec<u8> {
        let mut vector = data.to_vec();

        if vector.len() < min_size {
            vector.resize(min_size, 0);
        }
        vector
    }

    ///Calculate linear buffer size in bytes.
    /// 
    ///`bits_per_texel * width * height / 8`, floored by `min` and `at_least`.
    pub fn calc_buffer_size(bits_per_texel: usize, width: u32, height: u32, min: usize, at_least: usize) -> Option<usize> {
        let size = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(bits_per_texel)?
            / 8;
        Some(size.max(min).max(at_least))
    }

    /// Returns (minimum buffer length, deswizzle width, deswizzle height).
    pub fn get_deswizzle_size(bits_per_texel: usize, width: i32, height: i32, min_block_width: i32, min_block_height: i32) -> Result<(u64, i32, i32), &'static str> {
        if bits_per_texel == 0 || width <= 0 || height <= 0 || min_block_width <= 0 || min_block_height <= 0 {
            return Err("Invalid dimensions or format for deswizzling");
        }

        fn round_up(value: i32, alignment: i32) -> Option<i32> {
            value.checked_add(alignment - 1)?.checked_div(alignment)?.checked_mul(alignment)
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

}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_test_all() {
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(Morton::decode(Morton::encode(x, y)), (x, y));
                assert_eq!(Morton8::decode(Morton8::encode(x, y)), (x, y));
            }
        }

        for sy in [1, 2, 4, 8, 16, 32] {
            for sx in [1, 2, 4, 8, 16, 32] {
                let size = sx * sy;

                for t in 0..size {
                    let i = Morton::index(t, sx, sy);

                    assert_eq!(
                        t,
                        Morton::reverse_index(i, sx, sy),
                        "failed for t={t}, sx={sx}, sy={sy}"
                    );
                }
            }
        }
    }

}

