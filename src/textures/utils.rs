use std::cmp;

pub trait MortonAlgorithms {
    ///Convert a Morton (Z-order) index to (x, y) block coordinates. 
    ///  
    ///Decodes a linear Morton index into its corresponding block coordinates inside the tile. 
    ///  
    ///For example: 0 -> (0, 0) || 1 -> (1, 0) || 2 -> (0, 1) || 3 -> (1, 1) || 4 -> (2, 0) || ...
    fn encode(x: u32, y: u32) -> u32 {
        let mut result = 0;

        for bit in 0..16 {
            result |= ((x >> bit) & 1) << (bit * 2);
            result |= ((y >> bit) & 1) << (bit * 2 + 1);
        }

        result
    }

    ///Reverse decode. Converts (x, y) block coordinates to a Z-order index.
    fn decode(i: u32) -> (u32, u32) {
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

///Optimized morton implementation for 8x8 tiles as used in Bloodborne textures.
pub struct Morton8;
impl MortonAlgorithms for Morton8 {
    /// Decode an 8×8 Morton index into `(x, y)`.
    ///
    /// `i` must be in the range `0..64`.
    #[inline]
    fn decode(i: u32) -> (u32, u32) {
        fn compact1by1(mut n: u32) -> u32 {
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
    fn encode(x: u32, y: u32) -> u32 {
        fn part1by1(mut n: u32) -> u32 {
            n &= 0x07;
            n = (n | (n << 2)) & 0x33;
            n = (n | (n << 1)) & 0x55;
            n
        }

        part1by1(x) | (part1by1(y) << 1)
    }

    #[inline]
    fn index(t: usize, sx: usize, sy: usize) -> usize {
        let (x, y) = Self::decode(t as u32);
        y as usize * sx + x as usize
    }

    #[inline]
    fn reverse_index(i: usize, sx: usize, sy: usize) -> usize {
        let x = i % sx;
        let y = i / sx;

        Self::encode(x as u32, y as u32) as usize
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

