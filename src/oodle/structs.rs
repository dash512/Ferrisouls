use std::ffi::c_void;

pub type OodleBool = i32;


pub mod o26 {
    use super::*;
    use crate::oodle::enums::o26::*;
        
    #[allow(non_snake_case)]
    #[repr(C)]
    pub struct CompressOptions {
        pub verbosity: usize,
        pub minMatchLen: i32,
        pub seekChunkReset: OodleBool,
        pub seekChunkLen: i32,
        pub profile: Profile,
        pub dictionarySize: i32,
        pub spaceSpeedTradeoffBytes: i32,
        pub maxHuffmansPerChunk: i32,
        pub sendQuantumCRCs: OodleBool,
        pub maxLocalDictionarySize: i32,
        pub makeLongRangeMatcher: OodleBool,
        pub matchTableSizeLog2: i32,
    }

    pub struct CompressSettings {
        pub compressor: Compressor,
        pub level: CompressionLevel
    }

    impl CompressSettings {
        pub const KRAK: CompressSettings = Self {
            compressor: Compressor::Kraken,
            level: CompressionLevel::Optimal5
        };

        pub fn new(compressor: Compressor, level: CompressionLevel) -> Self {
            Self { compressor, level }
        }
    }

}

pub mod o28 {
    use super::*;
    use crate::oodle::enums::o28::*;
        
    #[allow(non_snake_case)]
    #[repr(C)]
    pub struct CompressOptions {
        pub verbosity: usize,
        pub minMatchLen: i32,
        pub seekChunkReset: OodleBool,
        pub seekChunkLen: i32,
        pub profile: Profile,
        pub dictionarySize: i32,
        pub spaceSpeedTradeoffBytes: i32,
        pub maxHuffmansPerChunk: i32,
        pub sendQuantumCRCs: OodleBool,
        pub maxLocalDictionarySize: i32,
        pub makeLongRangeMatcher: OodleBool,
        pub matchTableSizeLog2: i32,
        //added as of 2.8
        pub jobify: Jobify,
        pub jobifyUserPtr: *mut c_void,
        pub farMatchMinLen: i32,
        pub farMatchOffsetLog2: i32,
    }

    pub struct CompressSettings {
        pub compressor: Compressor,
        pub level: CompressionLevel
    }

    impl CompressSettings {
        pub const KRAK: CompressSettings = Self {
            compressor: Compressor::Kraken,
            level: CompressionLevel::Optimal5
        };

        pub fn new(compressor: Compressor, level: CompressionLevel) -> Self {
            Self { compressor, level }
        }
    }

}

pub mod o29 {
    use super::*;
    use crate::oodle::enums::o29::*;
        
    #[allow(non_snake_case)]
    #[repr(C)]
    pub struct CompressOptions {
        pub unused_was_verbosity: usize, // deprecated in 2.9; should be 0
        pub minMatchLen: i32,
        pub seekChunkReset: OodleBool,
        pub seekChunkLen: i32,
        pub profile: Profile,
        pub dictionarySize: i32,
        pub spaceSpeedTradeoffBytes: i32,
        pub unused_was_maxHuffmansPerChunk: i32, // deprecated in 2.9; should be 0
        pub sendQuantumCRCs: OodleBool,
        pub maxLocalDictionarySize: i32,
        pub makeLongRangeMatcher: OodleBool,
        pub matchTableSizeLog2: i32,
        //unchanged since 2.8
        pub jobify: Jobify,
        pub jobifyUserPtr: *mut c_void,
        pub farMatchMinLen: i32,
        pub farMatchOffsetLog2: i32,
        //added in 2.9
        pub reserved: [u32; 4],
    }

    pub struct CompressSettings {
        pub compressor: Compressor,
        pub level: CompressionLevel
    }
    impl CompressSettings {
        pub const KRAK: CompressSettings = Self {
            compressor: Compressor::Kraken,
            level: CompressionLevel::Optimal5
        };

        pub fn new(compressor: Compressor, level: CompressionLevel) -> Self {
            Self { compressor, level }
        }
    }

}
