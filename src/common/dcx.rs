use std::error::Error;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::fs::FileExt;
use std::{fmt, fs};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::fs::{File};

use crate::oodle::core::{Oodle, Oodle26, Oodle28, Oodle29, OodleType};
use header_structs::*;

#[derive(Debug, Clone)]
pub struct DCXError {
    msg: String,
}

impl DCXError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}

impl Error for DCXError {}

impl fmt::Display for DCXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub mod header_structs {
    use std::{fs::File, os::windows::fs::FileExt};

    use crate::common::bytes::ByteOrder;
    use super::{DCXVersionInfo, DCXError, Path};

    ///Early, abbreviated compression version (Demon's Souls only).
    pub struct DCPHeaderStruct {
        dcp: [u8; 4], // asserted b"DCP"
        dflt: [u8; 4], // asserted b"DFLT"
        unks: [u64; 6], // asserted [0x20, 0x9000000, 0, 0, 0, 0x10100]
        dcs: [u8; 4], // asserted b"DCS"
        decompressed_size: usize,
        compressed_size: usize,
        byte_order: ByteOrder
    }

    impl DCPHeaderStruct {
        pub fn new(decompressed: usize, compressed: usize) -> Self {
            Self {
                dcp: *b"DCP\0",
                dflt: *b"DFLT",
                unks: [0x20, 0x9000000, 0, 0, 0, 0x10100],
                dcs: *b"DCS\0",
                decompressed_size: decompressed,
                compressed_size: compressed,
                byte_order: ByteOrder::BigEndian
            }
        }
    }

    /// Compression header (with variation in the `version` fields) in all FromSoft games after Demon's Souls.
    /// NOTE: Not asserting the five 'version' fields so that we can guess when a new format is available.
    pub struct DCXHeaderStruct {
        dcx: [u8; 4], // asserted b"DCX\0"
        version1: u32, // [0x10000, 0x11000]
        unk1: u32, // asserted 0x18
        unk2: u32, // asserted 0x24
        version2: u32, // [0x24, 0x44]
        version3: u32, //  [0x2C, 0x4C, `0x50 + chunk_count * 0x10` (DCX_EDGE)]
        dcs: [u8; 4], // asserted b"DCS\0"
        decompressed_size: u32,
        compressed_size: u32,
        dcp: [u8; 4], // asserted b"DCP\0"
        compression_type: [u8; 4], // asserted (b"ZSTD", b"EDGE", b"DFLT", b"KRAK")
        unk3: u32, // asserted 0x20
        compression_level: u8, // [6, 8, 9, variable (DCX_ZSTD)]
        _compression_level_pad: [u8; 3], // 3 * b"\0" padding
        version5: u32, // [0, 0x10000]
        version6: u32, // [0, 0xF000000]
        unk5: u32, // asserted 0
        version7: u32, // [0x10100, 0x101000]

        byte_order: ByteOrder // Not serialized.
    }

    impl DCXHeaderStruct {
        pub fn new(
            v1: u32,
            v2: u32,
            v3: u32,
            decompressed: u32,
            compressed: u32,
            compression_type: [u8; 4],
            compression_level: u8,
            v5: u32,
            v6: u32,
            v7: u32
            ) -> Self {

            Self {
                dcx: *b"DCX\0",
                version1: v1,
                unk1: 0x18,
                unk2: 0x24,
                version2: v2,
                version3: v3,
                dcs: *b"DCS\0",
                decompressed_size: decompressed,
                compressed_size: compressed,
                dcp: *b"DCP\0",
                compression_type: compression_type,
                unk3: 0x20,
                compression_level: compression_level,
                _compression_level_pad: *b"\0\0\0",
                version5: v5,
                version6: v6,
                unk5: 0u32,
                version7: v7,
                byte_order: ByteOrder::BigEndian
            }
        }

        pub fn from_file(path: &Path) -> Result<Self, DCXError> {
            let file = File::open(path)
                            .expect("File should exist.");
            let mut buffer = [0u8; 68];
            file.seek_read(&mut buffer, 0)
                .map_err(|e| DCXError {
                    msg: format!("Failed to read DCX header: {e}"),
                })?;;

            Self::from_bytes(&buffer)
        }

        // Expects a 68 byte header from a file, which is then parsed into a DCXHeaderStruct instance
        pub fn from_bytes(buffer: &[u8]) -> Result<Self, DCXError> {
            if buffer.len() < 68 {
                return Err(DCXError{msg: "Invalid Header Size!".to_string()});
            }

            let mut offset = 0;

            fn read_u32_be(buffer: &[u8], offset: &mut usize) -> u32 {
                let value = u32::from_be_bytes([
                    buffer[*offset],
                    buffer[*offset + 1],
                    buffer[*offset + 2],
                    buffer[*offset + 3],
                ]);

                *offset += 4;
                value
            }

            fn read_bytes<const N: usize>(buffer: &[u8], offset: &mut usize) -> [u8; N] {
                let value = buffer[*offset..*offset + N]
                    .try_into()
                    .unwrap();

                *offset += N;
                value
            }

            let dcx = read_bytes::<4>(buffer, &mut offset);

            if dcx != *b"DCX\0" {
                return Err(DCXError{msg: "Magic `DCX` is Incorrect.".to_string()});
            }

            let version1 = read_u32_be(buffer, &mut offset);
            let unk1 = read_u32_be(buffer, &mut offset);
            let unk2 = read_u32_be(buffer, &mut offset);
            let version2 = read_u32_be(buffer, &mut offset);
            let version3 = read_u32_be(buffer, &mut offset);

            let dcs = read_bytes::<4>(buffer, &mut offset);

            if dcs != *b"DCS\0" {
                return Err(DCXError{msg: "Magic `DCS` is Incorrect.".to_string()});
            }

            let decompressed_size = read_u32_be(buffer, &mut offset);
            let compressed_size = read_u32_be(buffer, &mut offset);

            let dcp = read_bytes::<4>(buffer, &mut offset);

            if dcp != *b"DCP\0" {
                return Err(DCXError{msg: "Magic `DCP` is Incorrect.".to_string()});
            }

            let compression_type = read_bytes::<4>(buffer, &mut offset);

            if !matches!(
                &compression_type,
                b"ZSTD" | b"EDGE" | b"DFLT" | b"KRAK"
            ) {
                return Err(DCXError{msg: "Compression type `{&compression_type}` is Invalid!".to_string()});
            }

            let unk3 = read_u32_be(buffer, &mut offset);

            let compression_level = buffer[offset];
            offset += 1;

            let compression_level_pad = read_bytes::<3>(buffer, &mut offset);

            let version5 = read_u32_be(buffer, &mut offset);
            let version6 = read_u32_be(buffer, &mut offset);
            let unk5 = read_u32_be(buffer, &mut offset);
            let version7 = read_u32_be(buffer, &mut offset);

            debug_assert_eq!(offset, 68);

            Ok(Self {
                dcx,
                version1,
                unk1,
                unk2,
                version2,
                version3,
                dcs,
                decompressed_size,
                compressed_size,
                dcp,
                compression_type,
                unk3,
                compression_level,
                _compression_level_pad: compression_level_pad,
                version5,
                version6,
                unk5,
                version7,
                byte_order: ByteOrder::BigEndian,
            })
        }

        pub fn to_bytes(&self) -> [u8; 68] {
            let mut buffer = [0u8; 68];
            let mut offset = 0;

            fn write_u32_be(buffer: &mut [u8], offset: &mut usize, value: u32) {
                buffer[*offset..*offset + 4].copy_from_slice(&value.to_be_bytes());
                *offset += 4;
            }

            fn write_bytes<const N: usize>(
                buffer: &mut [u8],
                offset: &mut usize,
                value: &[u8; N],
            ) {
                buffer[*offset..*offset + N].copy_from_slice(value);
                *offset += N;
            }

            write_bytes(&mut buffer, &mut offset, &self.dcx);

            write_u32_be(&mut buffer, &mut offset, self.version1);
            write_u32_be(&mut buffer, &mut offset, self.unk1);
            write_u32_be(&mut buffer, &mut offset, self.unk2);
            write_u32_be(&mut buffer, &mut offset, self.version2);
            write_u32_be(&mut buffer, &mut offset, self.version3);

            write_bytes(&mut buffer, &mut offset, &self.dcs);

            write_u32_be(&mut buffer, &mut offset, self.decompressed_size);
            write_u32_be(&mut buffer, &mut offset, self.compressed_size);

            write_bytes(&mut buffer, &mut offset, &self.dcp);

            write_bytes(&mut buffer, &mut offset, &self.compression_type);

            write_u32_be(&mut buffer, &mut offset, self.unk3);

            buffer[offset] = self.compression_level;
            offset += 1;

            write_bytes(
                &mut buffer,
                &mut offset,
                &self._compression_level_pad,
            );

            write_u32_be(&mut buffer, &mut offset, self.version5);
            write_u32_be(&mut buffer, &mut offset, self.version6);
            write_u32_be(&mut buffer, &mut offset, self.unk5);
            write_u32_be(&mut buffer, &mut offset, self.version7);

            debug_assert_eq!(offset, 68);

            buffer
        }

        ///Extract non-constant field values.
        pub fn get_version_info(&self) -> DCXVersionInfo {
            DCXVersionInfo {
                compression_type: self.compression_type,
                version1: self.version1,
                version2: self.version2,
                version3: Some(self.version3),
                compression_level: Some(self.compression_level),
                version5: self.version5,
                version6: self.version6,
                version7: self.version7,
            }
        } 
    
        pub fn compressed_size(&self) -> u32 {
            self.compressed_size
        }

        pub fn decompressed_size(&self) -> u32 {
            self.decompressed_size
        }
        
        pub fn compression_type(&self) -> [u8;4] {
            self.compression_type
        }
    }

    pub struct DCXEdgeSubheader {
        dca: [u8; 4], // asserted b"DCA\0"
        dca_size: usize,
        egdt: [u8; 4], // asserted b"EgdT"
        unk1: u32, // asserted 0x10100
        unk2: u32, // asserted 0x24
        unk3: u32, // asserted 0x10
        unk4: u32, // asserted 0x10000
        last_block_decompressed_size: usize,
        egdt_size: usize,
        chunk_count: usize,
        unk5: u32, // asserted 0x100000
        byte_order: ByteOrder
    }

    impl DCXEdgeSubheader {
        pub fn new(dca_size: usize, last_block_size: usize, egdt_size: usize, chunk_count: usize) -> Self {
            Self {
                dca: *b"DCA\0",
                dca_size: dca_size,
                egdt: *b"EgdT",
                unk1: 0x10100,
                unk2: 0x24,
                unk3: 0x10,
                unk4: 0x10000,
                last_block_decompressed_size: last_block_size,
                egdt_size: egdt_size,
                chunk_count: chunk_count,
                unk5: 0x100000,
                byte_order: ByteOrder::BigEndian
            }
        }
    }

}

pub struct DCXVersionInfo {
    pub compression_type: [u8; 4],
    pub version1: u32,
    pub version2: u32,
    pub version3: Option<u32>, // not constant for `DCX_EDGE`
    pub compression_level: Option<u8>, // not constant for `DCX_ZSTD`
    pub version5: u32,
    pub version6: u32,
    pub version7: u32,
}

impl PartialEq for DCXVersionInfo {
    fn eq(&self, other: &Self) -> bool {
        if self.compression_type != other.compression_type {
            return false;
        }
        if self.version1 != other.version1 {
            return false;
        }
        if self.version2 != other.version2 {
            return false;
        }

        if let (Some(a), Some(b)) = (self.version3, other.version3) {
            if a != b {
                return false;
            }
        }

        if let (Some(a), Some(b)) = (self.compression_level, other.compression_level) {
            if a != b {
                return false;
            }
        }

        if self.version5 != other.version5 {
            return false;
        }
        if self.version6 != other.version6 {
            return false;
        }
        if self.version7 != other.version7 {
            return false;
        }

        true
    }
}

impl Hash for DCXVersionInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "DCXVersionInfo".hash(state);
    }
}

//Debug implementation to format integers into hex strings
impl fmt::Debug for DCXVersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("DCXVersionInfo");
        
        debug.field("compression_type", &self.compression_type);
        debug.field("version1", &format_args!("{:#x}", self.version1));
        debug.field("version2", &format_args!("{:#x}", self.version2));

        match self.version3 {
            Some(v) => debug.field("version3", &format_args!("Some({:#x})", v)),
            None => debug.field("version3", &None::<i32>),
        };

        match self.compression_level {
            Some(v) => debug.field("compression_level", &format_args!("Some({:#x})", v)),
            None => debug.field("compression_level", &None::<i32>),
        };

        debug.field("version5", &format_args!("{:#x}", self.version5));
        debug.field("version6", &format_args!("{:#x}", self.version6));
        debug.field("version7", &format_args!("{:#x}", self.version7));

        debug.finish()
    }
}


#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DCXType {
    Unknown = -1, // could not be detected
    Null = 0, // no compression
    Zlib = 1, // not really DCX but supported
    DCP_EDGE = 2, // DCP header, chunked deflate compression. Used in ACE:R TPFs.
    DCP_DFLT = 3, // DCP header, deflate compression. Used in DeS test maps.
    DCX_EDGE = 4, // DCX header, chunked deflate compression. Primarily used in DeS.
    DCX_DFLT_10000_24_9 = 5, // DCX header, deflate compression. Primarily used in DS1 and DS2.
    DCX_DFLT_10000_44_9 = 6, // DCX header, deflate compression. Primarily used in BB and DS3.
    DCX_DFLT_11000_44_8 = 7, // DCX header, deflate compression. Used for the backup regulation in DS3 save files.
    DCX_DFLT_11000_44_9 = 8, // DCX header, deflate compression. Used in Sekiro.
    DCX_DFLT_11000_44_9_15 = 9, // DCX header, deflate compression. Used in old ER regulation.
    DCX_KRAK = 10, // DCX header, Oodle compression. Used in Sekiro and Elden Ring.
    DCX_ZSTD = 11, // ZSTD compression. Used in new ER regulation.
}

impl DCXType {
    pub fn has_dcx_extension(&self) -> bool {
        (*self as i32) >= 2
    }

    pub fn detect_from_file(path: &Path) -> Result<Self, DCXError> {
        let mut file = File::open(path)
                            .expect("File should exist.");
        
        let mut header = [0u8;68];
        file.read_exact(&mut header);

        Self::detect(header)
    }

    ///Takes first 68 bytes of a file (DCXHeaderStruct) and detects the DCXType.
    pub fn detect(header: [u8; 68]) -> Result<Self, DCXError> {
        let magic = &header[0..4];

        if magic == b"DCP\0" {
            let dcx_fmt = &header[4..8];

            match dcx_fmt {
                b"DFLT" => return Ok(Self::DCP_DFLT),
                b"EDGE" => return Ok(Self::DCP_EDGE),
                _ => return Ok(Self::Unknown)
            }
        }

        if magic != b"DCX\0" {
            let b0 = header[0];
            let b1 = header[1];

            if b0 == 0x78 && matches!(b1, 0x01 | 0x5E | 0x9C | 0xDA) {
                return Ok(Self::Zlib);
            }
            return Ok(Self::Unknown);// very unlikely to be DCX at this point
        }

        let header = DCXHeaderStruct::from_bytes(&header)?;
        let header_vinfo = header.get_version_info();
        let dcx_type = DCXType::from_version_info(&header_vinfo);

        #[allow(irrefutable_let_patterns)]
        if let dcx = dcx_type.unwrap_or(Self::Unknown) {
            return Ok(dcx);
        }

        Ok(Self::Unknown) // just for transparency's sake
    }

    pub fn get_version_info(self) -> Option<DCXVersionInfo> {
        match self {
            DCXType::DCP_DFLT | DCXType::DCP_EDGE | DCXType::Zlib | DCXType::Null | DCXType::Unknown => None,

            DCXType::DCX_EDGE => {
                Some(DCXVersionInfo {
                    compression_type: *b"EDGE",
                    version1: 0x10000,
                    version2: 0x24,
                    version3: None,
                    compression_level: Some(9),
                    version5: 0x10000,
                    version6: 0,
                    version7: 0x100100,
                })
            },

            DCXType::DCX_DFLT_10000_24_9 => {
                Some(DCXVersionInfo {
                    compression_type: *b"DFLT",
                    version1: 0x10000,
                    version2: 0x24,
                    version3: Some(0x2C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100,
                })
            },

            DCXType::DCX_DFLT_10000_44_9 => {
                Some(DCXVersionInfo {
                    compression_type: *b"DFLT",
                    version1: 0x10000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                })
            },

            DCXType::DCX_DFLT_11000_44_8 => {
                Some(DCXVersionInfo {
                    compression_type: *b"DFLT",
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(8),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                })
            },

            DCXType::DCX_DFLT_11000_44_9 => {
                Some(DCXVersionInfo {
                    compression_type: *b"DFLT",
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                })
            },

            DCXType::DCX_DFLT_11000_44_9_15 => {
                Some(DCXVersionInfo {
                    compression_type: *b"DFLT",
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0xF000000,
                    version7: 0x010100
                })
            },

            DCXType::DCX_KRAK => {
                Some(DCXVersionInfo {
                    compression_type: *b"KRAK",
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(6),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                })
            },

            DCXType::DCX_ZSTD => {
                Some(DCXVersionInfo {
                    compression_type: *b"ZSTD",
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: None,
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                })
            }
        
        }
    }

    pub fn from_version_info(vinfo: &DCXVersionInfo) -> Option<Self> {
         match vinfo {
            DCXVersionInfo {
                    compression_type: [b'E', b'D', b'G', b'E'],
                    version1: 0x10000,
                    version2: 0x24,
                    version3: None,
                    compression_level: Some(9),
                    version5: 0x10000,
                    version6: 0,
                    version7: 0x100100,
            } => Some(DCXType::DCX_EDGE),

            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x10000,
                    version2: 0x24,
                    version3: Some(0x2C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100,
            } => Some(DCXType::DCX_DFLT_10000_24_9),

            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x10000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_DFLT_10000_44_9),

            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(8),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_DFLT_11000_44_8),

            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_DFLT_11000_44_9),

            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0xF000000,
                    version7: 0x010100
                } => Some(DCXType::DCX_DFLT_11000_44_9_15),

            // evil variant discovered by cr1msonyokai (self proclaimed victim)
            DCXVersionInfo {
                    compression_type: [b'D', b'F', b'L', b'T'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0x15000000,
                    version7: 0x010100
                } => Some(DCXType::DCX_DFLT_11000_44_9_15),

            DCXVersionInfo {
                    compression_type: [b'K', b'R', b'A', b'K'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(6),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_KRAK),
            
            // added to handle how AC6 uses compression 9 instead of 6 for KRAK
            DCXVersionInfo {
                    compression_type: [b'K', b'R', b'A', b'K'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: Some(9),
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_KRAK),

            DCXVersionInfo {
                    compression_type: [b'Z', b'S', b'T', b'D'],
                    version1: 0x11000,
                    version2: 0x44,
                    version3: Some(0x4C),
                    compression_level: None,
                    version5: 0,
                    version6: 0,
                    version7: 0x010100
                } => Some(DCXType::DCX_ZSTD),
            _ => None      
        }
    }

}

pub struct Decompress;
pub struct Compress;

impl Decompress {
    pub fn raw(raw_buffer: &Vec<u8>, oodle: &OodleType) -> Result<Vec<u8>, DCXError> {
        let header: &[u8; 68] = raw_buffer[..0x44].try_into().unwrap();
        let dcx_type = DCXType::detect(*header)?;

        if dcx_type != DCXType::DCX_KRAK {
            todo!();
            return Err(DCXError::new("Cannot decompress this DCX type yet."))
        }
        
        let header = DCXHeaderStruct::from_bytes(header)?;

        debug_assert_eq!(&raw_buffer[0x44..0x48], b"DCA\0");
        debug_assert_eq!(&raw_buffer[0x48..0x4C], b"\x00\x00\x00\x08");

        let compressed_size = header.compressed_size() as usize;
        let decompressed_size = header.decompressed_size() as usize;

        //0x0->0x44 : main header struct
        //0x44->0x48 : DCA\0 magic
        //0x48->0x4C : dca header size. Always 8 unless "edge" type dcx where its variable based on chunks
        let compressed = &raw_buffer[0x4C..0x4C + compressed_size];

        let decompressed = match oodle {
            OodleType::O26(inst) => unsafe {
                let mut decompressed_bytes: Vec<u8> = vec![0u8; decompressed_size];
                inst.decompress(
                    compressed,
                    &mut decompressed_bytes[..],
                )?;
                Ok(decompressed_bytes)
            },
            OodleType::O28(inst) => unsafe {
                let mut decompressed_bytes: Vec<u8> = vec![0u8; decompressed_size];
                inst.decompress(
                    compressed,
                    &mut decompressed_bytes[..],
                )?;
                Ok(decompressed_bytes)
            },
            OodleType::O29(inst) => unsafe {
                let mut decompressed_bytes: Vec<u8> = vec![0u8; decompressed_size];
                inst.decompress(
                    compressed,
                    &mut decompressed_bytes[..],
                )?;
                Ok(decompressed_bytes)
            },
        };
        decompressed

    }

    pub fn file(path: &Path, oodle: &OodleType) -> Result<Vec<u8>, DCXError> {
        let mut file_data = Vec::new();
        File::open(path)
            .expect("File should exist.")
            .read_to_end(&mut file_data)
            .unwrap();
        Self::raw(&file_data, oodle)
    }

}

impl Compress {
    pub fn raw(raw_buffer: &Vec<u8>, dcx_type: DCXType, oodle: &OodleType) -> Result<Vec<u8>, DCXError> {
        if dcx_type != DCXType::DCX_KRAK {
            todo!();
            return Err(DCXError::new("Cannot compress this DCX type yet."))
        }

        let mut compressed = match oodle {
            OodleType::O26(inst) => unsafe {
                let required_size = inst.get_compressed_buffer_size(raw_buffer.len())?;
                let mut compressed_bytes: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(
                    raw_buffer,
                    &mut compressed_bytes[..],
                    <Oodle26 as Oodle>::CompressSettings::KRAK
                )?;
                compressed_bytes.truncate(compressed_len);
                compressed_bytes
            },
            OodleType::O28(inst) => unsafe {
                let required_size = inst.get_compress_scratch_bound(
                    <Oodle28 as Oodle>::Compressor::Kraken,
                    raw_buffer.len())?;
                let mut compressed_bytes: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(
                    raw_buffer,
                    &mut compressed_bytes[..],
                    <Oodle28 as Oodle>::CompressSettings::KRAK
                )?;
                compressed_bytes.truncate(compressed_len);
                compressed_bytes
            },
            OodleType::O29(inst) => unsafe {
                let required_size = inst.get_compress_scratch_bound(
                    <Oodle29 as Oodle>::Compressor::Kraken, 
                    raw_buffer.len())?;
                let mut compressed_bytes: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(
                    raw_buffer,
                    &mut compressed_bytes[..],
                    <Oodle29 as Oodle>::CompressSettings::KRAK
                )?;
                compressed_bytes.truncate(compressed_len);
                compressed_bytes
            },
        };

        // TODO: Needs to:
        let vinfo = dcx_type.get_version_info().unwrap();
        let mut header = DCXHeaderStruct::new(
            vinfo.version1,
            vinfo.version2,
            vinfo.version3.unwrap(),
            raw_buffer.len() as u32,
            compressed.len() as u32,
            vinfo.compression_type,
            vinfo.compression_level.unwrap(),
            vinfo.version5,
            vinfo.version6,
            vinfo.version7,
        ).to_bytes().to_vec();

        // for "edge" types, dca header contains additional chunks so the size cannot be assumed to be 8
        header.extend_from_slice(b"DCA\0\x00\x00\x00\x08");
        header.extend_from_slice(&mut compressed);

        Ok(header)
    }

}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::oodle::core::get_oodle;

    #[test]
    #[ignore = "no"]
    fn get_dcxtype_from_file() -> Result<(), DCXError> {
        let path = Path::new("../tests/01_common.sblytbnd.dcx");
        let r = DCXType::detect_from_file(path)?;
        println!("{:?}", r);
        return Ok(());
    }

    #[test]
    fn test_compress() {
        let oodle = unsafe {
            get_oodle(Path::new(
                "../tests/oo2core_6_win64.dll"
            ))
            .unwrap()
        };

        let file = Path::new("../tests/01_common.sblytbnd.dcx");
        let mut f = File::open(file).unwrap();
        let mut empty = Vec::new();
        f.read_to_end(&mut empty);
        println!("Successfully read {:?} bytes", empty.len());

        let result = Decompress::file(file, &oodle).unwrap();
        println!("Successfully decompressed {:?} bytes", result.len());

        let new_result = Compress::raw(&result, DCXType::DCX_KRAK, &oodle).unwrap();
        println!("Successfully recompressed {:?} bytes", new_result.len());

        /*
        Successfully read 21123 bytes
        Successfully decompressed 402623 bytes
        Successfully recompressed 17851 bytes
        */
    }

}

