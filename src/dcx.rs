use std::error::Error;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::FileExt;
use std::{fmt, fs};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::fs::{File};

use zstd::{Decoder as ZstdDecoder, Encoder as ZstdEnconder};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression as ZCompression;

use crate::dcx;
use crate::errors::{BinaryReaderError, DCXError, FerrisoulsError};
use crate::oodle::core::{Oodle, Oodle26, Oodle28, Oodle29, OodleType};
use crate::oodle::structs::CompressSettings;
use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::binary::bytes::ByteOrder;
use header_structs::*;


pub mod header_structs {
    use super::*;
     
     ///Info struct containing the variable fields in a DCX file's header
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
            
            debug.field("compression_type", &format_args!("{}", String::from_utf8_lossy(&self.compression_type)));
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


    ///Early, abbreviated compression version (Demon's Souls only).
    pub struct DCPHeader {
        dcp: [u8; 4], // asserted b"DCP"
        dflt: [u8; 4], // asserted b"DFLT"
        unks: [u32; 6], // asserted [0x20, 0x9000000, 0, 0, 0, 0x10100]
        dcs: [u8; 4], // asserted b"DCS"
        pub decompressed_size: u32,
        pub compressed_size: u32,
    }
    
    impl DCPHeader {
        pub fn new(decompressed: u32, compressed: u32) -> Self {
            Self {
                dcp: *b"DCP\0",
                dflt: *b"DFLT",
                unks: [0x20, 0x9000000, 0, 0, 0, 0x10100],
                dcs: *b"DCS\0",
                decompressed_size: decompressed,
                compressed_size: compressed,
            }
        }

        pub fn compressed_size(&self) -> u32 {
            self.compressed_size
        }

        pub fn decompressed_size(&self) -> u32 {
            self.decompressed_size
        }

    }
    
    impl IO for DCPHeader {
        fn from_reader(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
            reader.assert_bytes(b"DCP\0")?;
            reader.assert_bytes(b"DFLT")?;
            let unks: Vec<u32> = reader.read_vec(6)?;
            reader.assert_bytes(b"DCS\0")?;

            let decompressed_size = reader.read_u32()?;
            let compressed_size = reader.read_u32()?;

            Ok(
                Self {
                    dcp: *b"DCP\0",
                    dflt: *b"DFLT", 
                    unks: 
                    *unks.as_array().unwrap(), 
                    dcs: *b"DCS\0", 
                    decompressed_size,
                    compressed_size, 
                }
            )  
        }

        fn to_writer(&self) -> Result<BinaryWriter, FerrisoulsError> {
            let mut writer = BinaryWriter::default();

            writer.write_bytes(b"DCP\0")?;
            writer.write_bytes(b"DFLT")?;

            writer.write_u32(0x20u32)?;//unk1
            writer.write_u32(0x9000000u32)?;//unk2
            writer.write_u32(0u32)?;//unk3
            writer.write_u32(0u32)?;//unk4
            writer.write_u32(0u32)?;//unk5
            writer.write_u32(0x10100u32)?;//unk6

            writer.write_u32(self.decompressed_size)?;
            writer.write_u32(self.compressed_size)?;

            debug_assert_eq!(writer.length(), 72);

            Ok(writer)
        }
    }

    /// Compression header (with variation in the `version` fields) in all FromSoft games after Demon's Souls.
    /// NOTE: Not asserting the five 'version' fields so that we can guess when a new format is available.
    pub struct DCXHeader {
        dcx: [u8; 4], // asserted b"DCX\0"
        version1: u32, // [0x10000, 0x11000]
        unk1: u32, // asserted 0x18
        unk2: u32, // asserted 0x24
        version2: u32, // [0x24, 0x44]
        version3: u32, //  [0x2C, 0x4C, `0x50 + chunk_count * 0x10` (DCX_EDGE)]
        dcs: [u8; 4], // asserted b"DCS\0"
        pub decompressed_size: u32,
        pub compressed_size: u32,
        dcp: [u8; 4], // asserted b"DCP\0"
        compression_type: [u8; 4], // asserted (b"ZSTD", b"EDGE", b"DFLT", b"KRAK")
        unk3: u32, // asserted 0x20
        compression_level: u8, // [6, 8, 9, variable (DCX_ZSTD)]
        _compression_level_pad: [u8; 3], // 3 * b"\0" padding
        version5: u32, // [0, 0x10000]
        version6: u32, // [0, 0xF000000]
        unk4: u32, // asserted 0
        version7: u32, // [0x10100, 0x101000]
    }
    
    impl DCXHeader {
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
                unk4: 0u32,
                version7: v7,
            }
        }

        pub fn from_version_info(vinfo: DCXVersionInfo, compressed_size: u32, decompressed_size: u32) -> Self {
            DCXHeader::new(
                vinfo.version1,
                vinfo.version2,
                vinfo.version3.unwrap(),
                decompressed_size,
                compressed_size,
                vinfo.compression_type,
                vinfo.compression_level.unwrap(),
                vinfo.version5,
                vinfo.version6,
                vinfo.version7,
            )
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
    
    impl IO for DCXHeader {
        fn from_reader(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
            reader.assert_bytes(b"DCX\0")?;

            let version1 = reader.read_u32()?;
            reader.assert::<u32>(0x18)?; // unk1
            reader.assert::<u32>(0x24)?; // unk2
            let version2 = reader.read_u32()?;
            let version3 = reader.read_u32()?;

            reader.assert_bytes(b"DCS\0")?;

            let decompressed_size = reader.read_u32()?;
            let compressed_size = reader.read_u32()?;

            reader.assert_bytes(b"DCP\0")?;

            let compression_type: [u8;4] = reader.read_bytes(4)?
                .as_slice()
                .try_into()
                .unwrap();

            if !matches!(
                &compression_type,
                b"ZSTD" | b"EDGE" | b"DFLT" | b"KRAK"
            ) {
                return Err(BinaryReaderError::Custom(
                    format!("Compression type `{:?}` is Invalid!", compression_type)
                ).into());
            }

            reader.assert::<u32>(0x20)?; //unk3

            let compression_level = reader.read_u8()?;
            reader.assert_bytes(b"\0\0\0")?; // compression level pad

            let version5 = reader.read_u32()?;
            let version6 = reader.read_u32()?;
            reader.assert::<u32>(0u32)?; // unk4
            let version7 = reader.read_u32()?;

            Ok(Self::new(
                version1,
                version2,
                version3,
                decompressed_size,
                compressed_size,
                compression_type,
                compression_level,
                version5,
                version6,
                version7
            ))
        }

        // Expects a 68 byte header from a file, which is then parsed into a DCXHeader instance
        fn from_bytes(buffer: &[u8]) -> Result<Self, FerrisoulsError> {
            if buffer.len() < 68 {
                return Err(BinaryReaderError::Custom("Invalid Header Size!".to_string()).into());
            }
            Self::from_reader(&mut BinaryReader::from(buffer, true, false))
        }

        fn from_path(path: &Path) -> Result<Self, FerrisoulsError> {
            let file = File::open(path)?;

            let mut buffer = [0u8; 68];
            file.seek_read(&mut buffer, 0)
                .map_err(|e| {
                    BinaryReaderError::Custom(
                        format!("Failed to read DCX header: {e}")
                    )
                }
            )?;


            Self::from_bytes(&buffer)
        }

        fn to_writer(&self) -> Result<BinaryWriter, FerrisoulsError> {
            let mut writer = BinaryWriter::default();

            writer.write_bytes(b"DCX\0")?;

            writer.write_u32(self.version1)?;
            writer.write_u32(0x18u32)?;
            writer.write_u32(0x24u32)?;
            writer.write_u32(self.version2)?;
            writer.write_u32(self.version3)?;

            writer.write_bytes(b"DCS\0")?;

            writer.write_u32(self.decompressed_size as u32)?;
            writer.write_u32(self.compressed_size as u32)?;

            writer.write_bytes(b"DCP\0")?;
            writer.write_bytes(&self.compression_type)?;

            writer.write_u32(0x20u32)?;

            writer.write_u8(self.compression_level)?;
            writer.write_bytes(&self._compression_level_pad)?;

            writer.write_u32(self.version5)?;
            writer.write_u32(self.version6)?;
            writer.write_u32(0u32)?;
            writer.write_u32(self.version7)?;

            debug_assert_eq!(writer.length(), 68);

            Ok(writer)
        }

    }
    
    impl fmt::Debug for DCXHeader {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut debug = f.debug_struct("DCXHeader");
            debug.field("Compressed Size", &self.compressed_size());
            debug.field("Uncompressed Size", &self.decompressed_size());
            debug.field("Version Info", &self.get_version_info());

            debug.finish()
        }
    }
    
    impl fmt::Display for DCXHeader {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{:#?}", self)
        }
    }


    pub struct DCXEdgeSubheader {
        dca: [u8; 4], // asserted b"DCA\0"
        pub dca_size: u32,
        egdt: [u8; 4], // asserted b"EgdT"
        unk1: u32, // asserted 0x10100
        unk2: u32, // asserted 0x24
        unk3: u32, // asserted 0x10
        unk4: u32, // asserted 0x10000
        pub last_block_decompressed_size: u32,
        pub egdt_size: u32,
        pub chunk_count: u32,
        unk5: u32, // asserted 0x100000
    }
    
    impl DCXEdgeSubheader {
        pub fn new(dca_size: u32, last_block_size: u32, egdt_size: u32, chunk_count: u32) -> Self {
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
            }
        }
    }
    
    impl IO for DCXEdgeSubheader {
        fn from_reader(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
            reader.assert_bytes(b"DCA\0")?;
            
            let dca_size = reader.read_u32()?;

            reader.assert_bytes(b"EgdT")?;

            reader.assert(0x10100u32)?; // unk1
            reader.assert(0x24u32)?; // unk2
            reader.assert(0x10u32)?; // unk3
            reader.assert(0x10000u32)?; // unk4

            let last_block_size = reader.read_u32()?;
            let egdt_size = reader.read_u32()?;
            let chunk_count = reader.read_u32()?;

            reader.assert(0x100000u32)?; // unk5

            Ok(Self::new(dca_size, last_block_size, egdt_size, chunk_count))
        }

        fn to_writer(&self) -> Result<BinaryWriter, FerrisoulsError> {
            let mut writer = BinaryWriter::default();

            writer.write_bytes(b"DCA\0")?;

            writer.write_u32(self.dca_size)?;
            writer.write_bytes(b"EgdT")?;

            writer.write_u32(0x10100u32)?;//unk1
            writer.write_u32(0x24u32)?;//unk2
            writer.write_u32(0x10u32)?;//unk3
            writer.write_u32(0x10000u32)?;//unk4

            writer.write_u32(self.last_block_decompressed_size);
            writer.write_u32(self.egdt_size);
            writer.write_u32(self.chunk_count);

            writer.write_u32(0x100000u32);//unk5

            debug_assert_eq!(writer.length(), 56);

            Ok(writer)
        }
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

    pub fn detect_from_file(path: &Path) -> Result<Self, FerrisoulsError> {
        let mut file = File::open(path)
                            .expect("File should exist.");
        
        let mut header = [0u8;68];
        file.read_exact(&mut header);

        Self::detect(
            &mut BinaryReader::from(&header, true, false)
        )
    }

    ///Takes first 68 bytes of a file (DCXHeader) and detects the DCXType.
    pub fn detect(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
        let magic = reader.read_bytes(4)?;

        if magic == b"DCP\0" {
            let dcx_fmt = reader.read_bytes(4)?;

            match dcx_fmt.as_slice() {
                b"DFLT" => return Ok(Self::DCP_DFLT),
                b"EDGE" => return Ok(Self::DCP_EDGE),
                _ => return Ok(Self::Unknown)
            }
        }

        if magic != b"DCX\0" {
            let b0 = reader.read_u8()?;
            let b1 = reader.read_u8()?;

            if b0 == 0x78 && matches!(b1, 0x01 | 0x5E | 0x9C | 0xDA) {
                return Ok(Self::Zlib);
            }
            return Ok(Self::Unknown);// very unlikely to be DCX at this point - Grimrukh
        }

        let header = DCXHeader::from_reader(reader)?;
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
    ///Special decompression handling for DCX_EDGE type.
    pub fn dcx_edge(mut reader: BinaryReader, header: DCXHeader) -> Result<Vec<u8>, FerrisoulsError> {
        let dca_start = reader.position() as u32;
        let subheader = DCXEdgeSubheader::from_reader(&mut reader)?;

        let hv3 = header.get_version_info().version3
            .ok_or(DCXError::InvalidData("Header has no `version3`.".to_string()))?;

        if hv3 != 0x50 + subheader.chunk_count * 0x10 {
            return Err(DCXError::InvalidData(
                "DCX_EDGE header 'version3' field does not match expected value (0x50 + chunk_count * 0x10).".to_string()
            ).into());
        }
        let last_block = subheader.last_block_decompressed_size;
        if last_block != 0x10000 && last_block != (header.decompressed_size() % 0x10000) {
            return Err(DCXError::InvalidData(
                "DCX_EDGE header 'version3' field does not match expected value (0x50 + chunk_count * 0x10).".to_string()
            ).into());
        }
        if subheader.egdt_size != 0x24 + subheader.chunk_count * 0x10 {
            return Err(DCXError::InvalidData(
                "DCX_EDGE subheader 'egdt_size' does not match expected value.".to_string()
            ).into());
        }

        let chunks_offset = dca_start + subheader.dca_size;
        let mut decompressed = Vec::<u8>::new();

        for i in 0..subheader.chunk_count {
            reader.assert::<u32>(0u32)?; // 'zero' field
            let offset = reader.read_u32()?;
            let chunk_size = reader.read_u32()?;
            let is_compressed_int = reader.read_u32()?;

            if !matches!(is_compressed_int, 0|1) {
                return Err(DCXError::InvalidData("DCX_EDGE chunk 'is_compressed' field is not 0 or 1.".to_string()).into());
            }

            reader.set_position(chunks_offset as u64 + offset as u64);
            let chunk = reader.read_bytes(chunk_size as usize)?;

            if is_compressed_int==0 {
                decompressed.extend(chunk);
                continue;
            }

            // Decompress using DEFLATE method. We use and flush a new Decompressor object for each chunk.
            // Decompressed chunks may occasionally be smaller than expected (0x10000 or final chunk size), so we pad as
            // necessary after each one. - Grimrukh
            let mut decompressor = flate2::Decompress::new(false); // raw deflate
            let mut decompressed_chunk = Vec::new();

            decompressor.decompress_vec(&chunk, &mut decompressed_chunk, flate2::FlushDecompress::Finish,)
                .map_err(|e| DCXError::decompression(e))?;

            let expected_decompressed_size = if i < subheader.chunk_count - 1 {
                0x10000
            } else {
                last_block
            } as usize;

            let decompressed_size = decompressed_chunk.len();
            if decompressed_size < expected_decompressed_size {
                decompressed_chunk.extend(b"\0".repeat(expected_decompressed_size - decompressed_size)); // pad
            }

            decompressed.extend(decompressed_chunk);
        }
        Ok(decompressed)
        
    }

    ///Takes compressed raw bytes and returns decompressed bytes and DCXType
    pub fn raw(raw_buffer: &[u8], oodle: &OodleType) -> Result<(Vec<u8>, DCXType), FerrisoulsError> {
        let mut reader = BinaryReader::from(raw_buffer, true, false);

        let dcx_type = DCXType::detect(&mut reader)?;

        let (compressed, decompressed_size) = match dcx_type {
            DCXType::Unknown => return Err(DCXError::Unsupported("Cannot decompress unknown DCX type.".to_string()).into()),

            DCXType::DCP_DFLT => {
                let dcpheader = DCPHeader::from_bytes(&raw_buffer)?;
                (reader.read_bytes(dcpheader.compressed_size() as usize)?, dcpheader.decompressed_size() as usize)
            } 

            _=> {
                let dcxheader = DCXHeader::from_bytes(&raw_buffer)?;

                if dcx_type == DCXType::DCX_EDGE {
                    return Ok((Self::dcx_edge(reader, dcxheader)?, dcx_type));
                }

                reader.assert_bytes(b"DCA\0")?;
                reader.assert::<u32>(8)?;

                (reader.read_bytes(dcxheader.compressed_size() as usize)?, dcxheader.decompressed_size() as usize)
            }
        };

        let decompressed = match dcx_type {
            DCXType::DCX_ZSTD => {
                let mut decoder = ZstdDecoder::new(&compressed[..])?;
                let mut decompressed = Vec::new();

                decoder.read_to_end(&mut decompressed)?;
                decompressed
            },

            DCXType::DCX_KRAK => {
                let dcxheader = DCXHeader::from_reader(&mut reader)?; //TODO: dont recall

                let compressed_size = dcxheader.compressed_size() as usize;
                let decompressed_size = dcxheader.decompressed_size() as usize;

                //0x0->0x44 : main header struct
                //0x44->0x48 : DCA\0 magic
                //0x48->0x4C : dca header size. Always 8 unless "edge" type dcx where its variable based on chunks
                let compressed = &raw_buffer[0x4C..0x4C + compressed_size];

                let mut decompressed_bytes: Vec<u8> = vec![0u8; decompressed_size];
                oodle.decompress(&compressed, &mut decompressed_bytes)?; // returns amount written

                decompressed_bytes
            },
            _=> {//Deflate types             
                let mut decoder = ZlibDecoder::new(&compressed[..]);
                let mut decompressed = Vec::new();

                decoder.read_to_end(&mut decompressed)?;
                decompressed
            }
        };

        if decompressed.len() != decompressed_size {
            return Err(DCXError::InvalidData("Decompressed data length does not match expect value from header.".to_string()).into())
        }

        Ok((decompressed, dcx_type))

    }

    pub fn file(path: &Path, oodle: &OodleType) -> Result<(Vec<u8>, DCXType), FerrisoulsError> {
        let mut file_data = Vec::new();
        File::open(path)?.read_to_end(&mut file_data)?;
        Self::raw(&file_data, oodle)
    }

}

impl Compress {
    ///Special compression handling for DCX_EDGE type.
    pub fn dcx_edge(raw_buffer: &[u8]) -> Result<Vec<u8>, FerrisoulsError> {
        let decompressed_size = raw_buffer.len();

        if decompressed_size == 0 {
            return Err(DCXError::InvalidData("Decompressed buffer is empty!".to_string()).into());
        }

        let mut writer = BinaryWriter::default();

        let mut chunk_count = decompressed_size / 0x10000;
        let last_block_decompressed_size = decompressed_size % 0x10000;
        if last_block_decompressed_size > 0 {
            chunk_count += 1 // add one more chunk for the remainder
        }

        let version_info = DCXType::DCX_EDGE.get_version_info().unwrap();

        let mut header = DCXHeader::new(
            version_info.version1, 
            version_info.version2, 
            (0x50 + chunk_count * 0x10) as u32, 
            decompressed_size as u32, 
            0u32, // reserve for patching later 
            version_info.compression_type, 
            version_info.compression_level.unwrap(), 
            version_info.version5, 
            version_info.version6, 
            version_info.version7
        );
        writer.append(header.to_bytes()?);

        let dca_start = writer.position() as u32;
        let egdt_start = dca_start + 8; // after b'DCA\0' magic and 'dca_size'

        let egdt_size = 0x10*chunk_count + 36; // subheader is 44 bytes, subtract 8 for start of DCA struct
        let dca_size = egdt_size + 8;

        let mut subheader = DCXEdgeSubheader::new(
            dca_size as u32,
            last_block_decompressed_size as u32,
            egdt_size as u32,
            chunk_count as u32,
        );
        writer.append(subheader.to_bytes()?);

        for i in 0..chunk_count {
            writer.write_u32(0);
            writer.reserve(format!("offset{i}"), 32)?;
            writer.reserve(format!("size{i}"), 32)?;
            writer.reserve(format!("is_compressed{i}"), 32)?;

        }

        subheader.dca_size = writer.position() as u32 - dca_start;
        subheader.egdt_size = writer.position() as u32 - egdt_start;

        let data_start = writer.position();
        let mut compressed_size = 0usize;
        for i in 0..chunk_count {
            let decompressed_chunk_size  = if i < chunk_count - 1 {
                0x10000
            } else {
                last_block_decompressed_size
            };

            let mut compressor = flate2::Compress::new(flate2::Compression::best(), false); // raw deflate
            let raw_offset = i * 0x10000;
            let decompressed_chunk = &raw_buffer[raw_offset..raw_offset + decompressed_chunk_size];

            let mut chunk = Vec::new();

            compressor.compress_vec( decompressed_chunk, &mut chunk, flate2::FlushCompress::Finish)
                .map_err(|e| DCXError::compression(e))?;

            let chunk_compressed_size = chunk.len(); 

            writer.fill(format!("offset{i}"), writer.position() - data_start);
            writer.fill(format!("size{i}"), chunk_compressed_size as u64);
            writer.fill(format!("is_compressed{i}"), (chunk_compressed_size < decompressed_chunk_size) as u32);
            compressed_size += chunk_compressed_size;
            writer.append(chunk);
            writer.pad_align(0x10);

        }

        writer.patch_u32(0x20, compressed_size as u32)?; // write to "reserved" `compressed` field

        Ok(writer.into_inner())

    }

    ///Special compression handling for DCX_ZSTD type.
    pub fn dcx_zstd(raw_buffer: &[u8], compression_level: i32) -> Result<Vec<u8>, FerrisoulsError> {
        let mut encoder = ZstdEnconder::new(Vec::new(), compression_level)?;

        encoder.set_parameter(zstd::zstd_safe::CParameter::WindowLog(16))?;
        encoder.include_checksum(false)?;

        let compressed = encoder.finish()?;
        Ok(compressed)
    }

    ///Compresses raw bytes and returns them
    pub fn raw(raw_buffer: &[u8], dcx_type: DCXType, oodle: &OodleType) -> Result<Vec<u8>, FerrisoulsError> {
        let mut compressed = match dcx_type {
            DCXType::Unknown => return Err(DCXError::Unsupported("Cannot compress unknown DCX type.".to_string()).into()),

            DCXType::Null => raw_buffer.to_owned(),

            DCXType::DCX_ZSTD => Self::dcx_zstd(raw_buffer, 15i32)?,

            DCXType::DCX_EDGE => return Ok(Self::dcx_edge(raw_buffer)?),

            DCXType::DCX_KRAK => {
                oodle.compress(raw_buffer, CompressSettings::KRAK)?
            },

            _=> { // deflate
                let mut encoder = ZlibEncoder::new(Vec::new(), ZCompression::new(7));
                encoder.write_all(raw_buffer);
                encoder.finish()?
            }
        };

        match dcx_type {
            DCXType::DCP_DFLT => {
                let header = DCPHeader::new(
                    raw_buffer.len() as u32, 
                    compressed.len() as u32
                ).to_bytes()?;

                compressed.extend(header);
                return Ok(compressed);

            }

            _=> { // "bad" types are already impossible from previous match. Covers DFLT, KRAK, ZSTD, EDGE
                let vinfo = dcx_type.get_version_info().unwrap();
                let mut output_struct = DCXHeader::from_version_info(
                    vinfo,
                    compressed.len() as u32,
                    raw_buffer.len() as u32
                ).to_bytes()?;

                output_struct.extend_from_slice(b"DCA\0\x00\x00\x00\x08");
                output_struct.extend_from_slice(&mut compressed);

                return Ok(output_struct);
            }
        }

    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_dcxtype_from_file() -> Result<(), FerrisoulsError> {
        let path = Path::new(r"...\tests\01_common.sblytbnd.dcx");
        let r = DCXType::detect_from_file(path)?;
        let h = DCXHeader::from_path(path)?;
        println!("{:?}\n{}", r, h);
        return Ok(());
        /*
        DCX_KRAK
        DCXHeader {
            Compressed Size: 21047,
            Uncompressed Size: 402623,
            Version Info: DCXVersionInfo {
                compression_type: KRAK,
                version1: 0x11000,
                version2: 0x44,
                version3: Some(0x4c),
                compression_level: Some(0x6),
                version5: 0x0,
                version6: 0x0,
                version7: 0x10100,
            },
        }*/
    }

    #[test]
    fn test_compress() {
        let oodle = unsafe {
            OodleType::get_oodle(Path::new(
                ".../tests/oo2core_6_win64.dll"
            ))
            .unwrap()
        };

        let file = Path::new(".../tests/01_common.sblytbnd.dcx");
        let mut f = File::open(file).unwrap();
        let mut empty = Vec::new();
        f.read_to_end(&mut empty);
        println!("Successfully read {:?} bytes", empty.len());

        let result = Decompress::file(file, &oodle).unwrap();
        println!("Successfully decompressed {:?} bytes", result.0.len());

        let new_result = Compress::raw(&result.0, DCXType::DCX_KRAK, &oodle).unwrap();
        println!("Successfully recompressed {:?} bytes", new_result.len());

        /*
        Successfully read 21123 bytes
        Successfully decompressed 402623 bytes
        Successfully recompressed 17851 bytes
        */
    }

}

