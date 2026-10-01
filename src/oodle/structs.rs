use std::ffi::c_void;
use crate::oodle::enums::{Compressor, CompressionLevel, Profile, Jobify};
pub type OodleBool = i32;

#[derive(Debug, Copy, Clone)]
pub struct OodleSettings {
    pub compressor: Compressor,
    pub level: CompressionLevel
}
impl OodleSettings {
    pub const KRAK: OodleSettings = Self {
        compressor: Compressor::Kraken,
        level: CompressionLevel::Optimal5
    };

    pub fn new(compressor: Compressor, level: CompressionLevel) -> Self {
        Self { compressor, level }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
pub struct O26CompressOptions {
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



#[allow(non_snake_case)]
#[repr(C)]
pub struct O28CompressOptions {
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


#[allow(non_snake_case)]
#[repr(C)]
pub struct O29CompressOptions {
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


