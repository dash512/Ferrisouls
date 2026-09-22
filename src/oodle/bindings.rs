use std::ffi::c_void;

pub mod o26 {
    use super::*;
    pub use crate::oodle::enums::o26::*;
    pub use crate::oodle::structs::o26::CompressOptions;
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_Compress = unsafe extern "system" fn(
            compressor: Compressor, // compressor
            raw_buf_array: *const c_void, // rawBuf
            raw_buf_size: isize, // rawLen
            comp_buf_array: *mut c_void, // compBuf
            level: CompressionLevel, // level
            p_options: *const CompressOptions, // pOptions
            dictionary_base: *const c_void, // dictionaryBase
            lrm: *const c_void, // lrm
            scratch_mem: *mut c_void, // scratchMem
            scratch_size: isize, // scratchSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_Decompress = unsafe extern "system" fn(
            comp_buf: *const c_void, // compBuf
            comp_buf_size: isize, // compBufSize
            raw_buf: *mut c_void, // rawBuf
            raw_size: isize, // rawLen (known uncompressed size)
            fuzz_safe: FuzzSafe, // fuzzSafe
            ccrc: CheckCRC, // checkCRC
            verbosity: Verbosity, // verbosity
            dec_buff_base: *mut c_void, // decBufBase
            dec_buf_size: isize, // decBufSize
            fp_callback: *mut c_void, // fpCallback
            callback_userdata: *mut c_void, // callbackUserData
            dec_mem: *mut c_void, // decoderMemory
            dec_mem_size: isize, // decoderMemorySize
            thread_phase: DecodeThreadPhase, // threadPhase
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_CompressOptions_GetDefault = unsafe extern "system" fn(
            compressor: Compressor,
            level: CompressionLevel
        ) -> *mut (CompressOptions);

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetCompressedBufferSizeNeeded = unsafe extern "system" fn(
            raw_size: isize // rawSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetDecodeBufferSize = unsafe extern "system" fn(
            raw_size: isize, // rawSize
            corruptable: i32 // corruption possible; bool
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetInPlaceDecodeBufferSize = unsafe extern "system" fn(
            comp_size: isize, // compressed size
            raw_size: isize // rawSize
        ) -> isize; 
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetChunkCompressor = unsafe extern "system" fn(
            comp_buf: *const c_void,
        ) -> Compressor; 
        
        
}

pub mod o28 {
    use super::*;
    pub use crate::oodle::enums::o28::*;
    pub use crate::oodle::structs::o28::CompressOptions;
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_Compress = unsafe extern "system" fn(
            compressor: Compressor, // compressor
            raw_buf_array: *const c_void, // rawBuf
            raw_buf_size: isize, // rawLen
            comp_buf_array: *mut c_void, // compBuf
            level: CompressionLevel, // level
            p_options: *const CompressOptions, // pOptions
            dictionary_base: *const c_void, // dictionaryBase
            lrm: *const c_void, // lrm
            scratch_mem: *mut c_void, // scratchMem
            scratch_size: isize, // scratchSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_Decompress = unsafe extern "system" fn(
            comp_buf: *const c_void, // compBuf
            comp_buf_size: isize, // compBufSize
            raw_buf: *mut c_void, // rawBuf
            raw_size: isize, // rawLen (known uncompressed size)
            fuzz_safe: FuzzSafe, // fuzzSafe
            ccrc: CheckCRC, // checkCRC
            verbosity: Verbosity, // verbosity
            dec_buff_base: *mut c_void, // decBufBase
            dec_buf_size: isize, // decBufSize
            fp_callback: *mut c_void, // fpCallback
            callback_userdata: *mut c_void, // callbackUserData
            dec_mem: *mut c_void, // decoderMemory
            dec_mem_size: isize, // decoderMemorySize
            thread_phase: DecodeThreadPhase, // threadPhase
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_CompressOptions_GetDefault = unsafe extern "system" fn(
            compressor: Compressor,
            level: CompressionLevel
        ) -> *mut (CompressOptions);
        // function is unchanged but fields in CompressOptions changed between 2.6 and 2.8

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetCompressedBufferSizeNeeded = unsafe extern "system" fn(
            compressor: Compressor, // compressor only needed in newer versions such as 2.8+
            raw_size: isize // rawSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetDecodeBufferSize = unsafe extern "system" fn(
            compressor: Compressor, // only needed in newer versions such as 2.8+
            raw_size: isize, // rawSize
            corruptable: i32 // corruption possible; bool
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetInPlaceDecodeBufferSize = unsafe extern "system" fn(
            compressor: Compressor, // only needed in newer versions such as 2.8+
            comp_size: isize, // compressed size
            raw_size: isize // rawSize
        ) -> isize; 
        
    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetChunkCompressor = unsafe extern "system" fn(
            comp_buf: *const c_void, 
            comp_buf_size: isize, // compressed size; added in 2.8 to avoid reading beyond buffer
        ) -> Compressor; 
        /*
        TODO: introduced in 2.8.1:
        OodleLZ_GetFirstChunkCompressor
        OodleLZ_GetAllChunksCompressor
         */
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetCompressScratchMemBound = unsafe extern "system" fn(
            compressor: Compressor, // compressor
            raw_size: isize // rawSize
        ) -> isize;
        // added in 2.8
        
}

pub mod o29 {
    use super::*;
    pub use crate::oodle::enums::o29::*;
    pub use crate::oodle::structs::o29::CompressOptions;
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_Compress = unsafe extern "system" fn(
            compressor: Compressor, // compressor
            raw_buf_array: *const c_void, // rawBuf
            raw_buf_size: isize, // rawLen
            comp_buf_array: *mut c_void, // compBuf
            level: CompressionLevel, // level
            p_options: *const CompressOptions, // pOptions
            dictionary_base: *const c_void, // dictionaryBase
            lrm: *const c_void, // lrm
            scratch_mem: *mut c_void, // scratchMem
            scratch_size: isize, // scratchSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_Decompress = unsafe extern "system" fn(
            comp_buf: *const c_void, // compBuf
            comp_buf_size: isize, // compBufSize
            raw_buf: *mut c_void, // rawBuf
            raw_size: isize, // rawLen (known uncompressed size)
            fuzz_safe: FuzzSafe, // fuzzSafe
            ccrc: CheckCRC, // checkCRC
            verbosity: Verbosity, // verbosity
            dec_buff_base: *mut c_void, // decBufBase
            dec_buf_size: isize, // decBufSize
            fp_callback: *mut c_void, // fpCallback
            callback_userdata: *mut c_void, // callbackUserData
            dec_mem: *mut c_void, // decoderMemory
            dec_mem_size: isize, // decoderMemorySize
            thread_phase: DecodeThreadPhase, // threadPhase
        ) -> isize;

    ///TODO: is this a zero argument function as of 2.9?
    #[allow(non_camel_case_types)]
    pub type OodleLZ_CompressOptions_GetDefault = unsafe extern "system" fn(
            //compressor: Compressor,
            //level: CompressionLevel
        ) -> *mut (CompressOptions); // fields in CompressOptions differ from 2.6

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetCompressedBufferSizeNeeded = unsafe extern "system" fn(
            compressor: Compressor, // compressor only needed in newer versions such as 2.8+
            raw_size: isize // rawSize
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetDecodeBufferSize = unsafe extern "system" fn(
            compressor: Compressor, // only needed in newer versions such as 2.8+
            raw_size: isize, // rawSize
            corruptable: i32 // corruption possible; bool
        ) -> isize;

    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetInPlaceDecodeBufferSize = unsafe extern "system" fn(
            compressor: Compressor, // only needed in newer versions such as 2.8+
            comp_size: isize, // compressed size
            raw_size: isize // rawSize
        ) -> isize;
    
    #[allow(non_camel_case_types)]
    pub type OodleLZ_GetCompressScratchMemBound = unsafe extern "system" fn(
            compressor: Compressor, // compressor
            raw_size: isize // rawSize
        ) -> isize;
        // added in 2.8

}
