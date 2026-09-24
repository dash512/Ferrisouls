
use std::{ffi::c_void, path::{Path, PathBuf}};
use libloading::{Library, Symbol};
use crate::common::dcx::DCXError;
use crate::oodle::{bindings, enums, structs};

pub trait Oodle {
    type Compressor;
    type CompressOptions;
    type CompressSettings;
    type FuzzSafe;
    type CheckCRC;
    type Verbosity;
    type DecodeThreadPhase;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> where Self: Sized;

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: Self::CompressSettings) -> Result<usize, DCXError>;

    ///Decompresses `input` data into `output`. Returns number of bytes written.
    unsafe fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, DCXError>;

    ///Returns size required for the buffer that takes the compressed output of `Self::compress`
    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError>;

    //Returns default compression options for given settings. (Compressor, CompressionLevel)
    unsafe fn get_default_compress_options(&self, settings: &Self::CompressSettings) -> *mut Self::CompressOptions;

    //Returns the buffer size needed for decompression, optionally accounting for possible data corruption.
    //This is not typically needed for Fromsoft games, as decompressed size is stored in DCX headers.
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, corruptable: bool) -> Result<usize, DCXError>;

    // Same as `get_decode_buffer_size` but for overwriting compressed data in-memory without excessive additional allocation.
    unsafe fn get_decode_buffer_size_in_place(&self, compressor: Self::Compressor, raw_size: usize, corruptable: bool) -> Result<usize, DCXError> {
        Err(DCXError::new("This function is not implemented for your Oodle version."))
    }

    ///Returns size required for the buffer that takes the compressed output of `Self::compress`
    unsafe fn get_compress_scratch_bound(&self, compressor: Self::Compressor, raw_size: usize) -> Result<usize, DCXError> {
        Err(DCXError::new("This function is not implemented for your Oodle version."))
    }
}


#[derive(Debug)]
pub struct Oodle26 {
    _lib: Library,
    lz_compress: bindings::o26::OodleLZ_Compress,
    lz_decompress: bindings::o26::OodleLZ_Decompress,
    lz_get_default_options: bindings::o26::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: bindings::o26::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: bindings::o26::OodleLZ_GetDecodeBufferSize
}

impl Oodle for Oodle26 {
    type Compressor = bindings::o26::Compressor;
    type CompressOptions = bindings::o26::CompressOptions;
    type CompressSettings = structs::o26::CompressSettings;
    type FuzzSafe = enums::o26::FuzzSafe;
    type CheckCRC = enums::o26::CheckCRC;
    type Verbosity = enums::o26::Verbosity;
    type DecodeThreadPhase = enums::o26::DecodeThreadPhase;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<bindings::o26::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<bindings::o26::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<bindings::o26::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<bindings::o26::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<bindings::o26::OodleLZ_GetDecodeBufferSize>(
                b"OodleLZ_GetDecodeBufferSize\0"
            )?
        };


        Ok(Self {
            _lib: lib,
            lz_compress: compress,
            lz_decompress: decompress,
            lz_get_default_options: get_default_options,
            lz_get_comp_size: get_required_size,
            lz_get_decode_size: get_decode_size
        })
    }

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: Self::CompressSettings) -> Result<usize, DCXError> {
        let p_options = unsafe { self.get_default_compress_options(&settings) };
        unsafe { (*p_options).seekChunkReset = true as i32; } // required for the game to not crash --TK
        unsafe { (*p_options).seekChunkLen = 0x40000; } // already default, but included for authenticity --TK

        let result = unsafe {
            (self.lz_compress)(
                settings.compressor,
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                settings.level,
                p_options,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
            )
        };

        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else if result as usize > output.len() {
            Err(DCXError::new("Oodle returned a size larger than the output buffer."))
        } else {
            Ok(result as usize)
        }

    }

    unsafe fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, DCXError> {
        let result = unsafe {
            (self.lz_decompress)(
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                output.len() as isize,
                Self::FuzzSafe::Yes,
                Self::CheckCRC::No,
                Self::Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                Self::DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_comp_size)(raw_size as isize) as usize ) }
    }

    unsafe fn get_default_compress_options(&self, settings: &Self::CompressSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)(settings.compressor, settings.level) }
    }

    unsafe fn get_decode_buffer_size(&self, raw_size: usize, corruptable: bool) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_decode_size)(raw_size as isize, corruptable as i32) as usize ) }
    }
}


#[derive(Debug)]
pub struct Oodle28 {
    _lib: Library,
    compressor: Option<enums::o28::Compressor>,
    lz_compress: bindings::o28::OodleLZ_Compress,
    lz_decompress: bindings::o28::OodleLZ_Decompress,
    lz_get_default_options: bindings::o28::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: bindings::o28::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: bindings::o28::OodleLZ_GetDecodeBufferSize,
    lz_get_scratchmem_bound: bindings::o28::OodleLZ_GetCompressScratchMemBound
}

impl Oodle28 {
    pub fn set_compressor(mut self, compressor: enums::o28::Compressor) {
        self.compressor = Some(compressor)
    }
}

impl Oodle for Oodle28 {
    type Compressor = bindings::o28::Compressor;
    type CompressOptions = bindings::o28::CompressOptions;
    type CompressSettings = structs::o28::CompressSettings;
    type FuzzSafe = enums::o28::FuzzSafe;
    type CheckCRC = enums::o28::CheckCRC;
    type Verbosity = enums::o28::Verbosity;
    type DecodeThreadPhase = enums::o28::DecodeThreadPhase;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<bindings::o28::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<bindings::o28::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<bindings::o28::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<bindings::o28::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<bindings::o28::OodleLZ_GetDecodeBufferSize>(
                b"OodleLZ_GetDecodeBufferSize\0"
            )?
        };

        let get_scratchmem_bound = unsafe {
            *lib.get::<bindings::o28::OodleLZ_GetCompressScratchMemBound>(
                b"OodleLZ_GetCompressScratchMemBound\0"
            )?
        };


        Ok(Self {
            _lib: lib,
            compressor: None,
            lz_compress: compress,
            lz_decompress: decompress,
            lz_get_default_options: get_default_options,
            lz_get_comp_size: get_required_size,
            lz_get_decode_size: get_decode_size,
            lz_get_scratchmem_bound: get_scratchmem_bound
        })
    }

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: Self::CompressSettings) -> Result<usize, DCXError> {
        let p_options = unsafe { self.get_default_compress_options(&settings) };
        unsafe { (*p_options).seekChunkReset = true as i32; } // required for the game to not crash --TK
        unsafe { (*p_options).seekChunkLen = 0x40000; } // already default, but included for authenticity --TK

        let result = unsafe {
            (self.lz_compress)(
                settings.compressor,
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                settings.level,
                p_options,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
            )
        };

        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else if result as usize > output.len() {
            Err(DCXError::new("Oodle returned a size larger than the output buffer."))
        } else {
            Ok(result as usize)
        }

    }

    unsafe fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, DCXError> {
        let result = unsafe {
            (self.lz_decompress)(
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                output.len() as isize,
                Self::FuzzSafe::Yes,
                Self::CheckCRC::No,
                Self::Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                Self::DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::new("Failed to get needed buffer size for compression: compressor was not given. 
            Try setting it with `set_compressor`.")),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_comp_size)(comp, raw_size as isize) as usize )
            }
        }
    }

    unsafe fn get_default_compress_options(&self, settings: &Self::CompressSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)(settings.compressor, settings.level) }
    }

    ///Requires compressor to be set with `Oodle28.set_compressor()`
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, corruptable: bool) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::new("Failed to get decode buffer size: compressor was not given. 
            Try setting it with `set_compressor`.")),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_decode_size)(comp, raw_size as isize, corruptable as i32) as usize )
            }
        }
        
    }

    // Get size of buffer required for compressed data. Never returns error.
    unsafe fn get_compress_scratch_bound(&self, compressor: Self::Compressor, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_scratchmem_bound)(compressor, raw_size as isize) as usize ) }
    }

}


#[derive(Debug)]
pub struct Oodle29 {
    _lib: Library,
    compressor: Option<enums::o29::Compressor>,
    lz_compress: bindings::o29::OodleLZ_Compress,
    lz_decompress: bindings::o29::OodleLZ_Decompress,
    lz_get_default_options: bindings::o29::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: bindings::o29::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: bindings::o29::OodleLZ_GetDecodeBufferSize,
    lz_get_scratchmem_bound: bindings::o29::OodleLZ_GetCompressScratchMemBound
}

impl Oodle29 {
    pub fn set_compressor(mut self, compressor: enums::o29::Compressor) {
        self.compressor = Some(compressor)
    }
}

impl Oodle for Oodle29 {
    type Compressor = bindings::o29::Compressor;
    type CompressOptions = bindings::o29::CompressOptions;
    type CompressSettings = structs::o29::CompressSettings;
    type FuzzSafe = enums::o29::FuzzSafe;
    type CheckCRC = enums::o29::CheckCRC;
    type Verbosity = enums::o29::Verbosity;
    type DecodeThreadPhase = enums::o29::DecodeThreadPhase;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<bindings::o29::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<bindings::o29::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<bindings::o29::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<bindings::o29::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<bindings::o29::OodleLZ_GetDecodeBufferSize>(
                b"OodleLZ_GetDecodeBufferSize\0"
            )?
        };

        let get_scratchmem_bound = unsafe {
            *lib.get::<bindings::o29::OodleLZ_GetCompressScratchMemBound>(
                b"OodleLZ_GetCompressScratchMemBound\0"
            )?
        };


        Ok(Self {
            _lib: lib,
            compressor: None,
            lz_compress: compress,
            lz_decompress: decompress,
            lz_get_default_options: get_default_options,
            lz_get_comp_size: get_required_size,
            lz_get_decode_size: get_decode_size,
            lz_get_scratchmem_bound: get_scratchmem_bound
        })
    }

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: Self::CompressSettings) -> Result<usize, DCXError> {
        let p_options = unsafe { self.get_default_compress_options(&settings) };
        unsafe { (*p_options).seekChunkReset = true as i32; } // required for the game to not crash --TK
        unsafe { (*p_options).seekChunkLen = 0x40000; } // already default, but included for authenticity --TK

        let result = unsafe {
            (self.lz_compress)(
                settings.compressor,
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                settings.level,
                p_options,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
            )
        };

        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else if result as usize > output.len() {
            Err(DCXError::new("Oodle returned a size larger than the output buffer."))
        } else {
            Ok(result as usize)
        }

    }

    unsafe fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, DCXError> {
        let result = unsafe {
            (self.lz_decompress)(
                input.as_ptr() as *const c_void,
                input.len() as isize,
                output.as_mut_ptr() as *mut c_void,
                output.len() as isize,
                Self::FuzzSafe::Yes,
                Self::CheckCRC::No,
                Self::Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                Self::DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::new("Oodle returned 0 on decompression."))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::new("Failed to get needed buffer size for compression: compressor was not given. 
            Try setting it with `set_compressor`.")),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_comp_size)(comp, raw_size as isize) as usize )
            }
        }
    }

    ///This function takes no arguments as of Oodle2.9. The settings field is still required for transparency but isn't used.
    unsafe fn get_default_compress_options(&self, settings: &Self::CompressSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)() }
    }

    ///Requires compressor to be set with `Oodle28.set_compressor()`
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, corruptable: bool) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::new("Failed to get decode buffer size: compressor was not given. 
            Try setting it with `set_compressor`.")),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_decode_size)(comp, raw_size as isize, corruptable as i32) as usize )
            }
        }
        
    }

    // Get size of buffer required for compressed data. Never returns error.
    unsafe fn get_compress_scratch_bound(&self, compressor: Self::Compressor, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_scratchmem_bound)(compressor, raw_size as isize) as usize ) }
    }

}


#[derive(Debug)]
pub enum OodleType {
    O26(Oodle26),
    O28(Oodle28),
    O29(Oodle29)
}

pub fn get_oodle(path: &Path) -> Result<OodleType, String> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("oo2core_6_win64.dll") => unsafe {
            Oodle26::load(path)
                .map(OodleType::O26)
                .map_err(|e| format!("Failed to load Oodle: {e}"))
        },

        Some("oo2core_8_win64.dll") => unsafe {
            Oodle28::load(path)
                .map(OodleType::O28)
                .map_err(|e| format!("Failed to load Oodle: {e}"))
        },

        Some("oo2core_9_win64.dll") => unsafe {
            Oodle29::load(path)
                .map(OodleType::O29)
                .map_err(|e| format!("Failed to load Oodle: {e}"))
        },

        _ => Err("Given file is not a valid Oodle library.".to_string()),
    }
}

pub fn find_oodle() -> Result<PathBuf, String> {
    let paths: [&Path; 1] = [
        Path::new("../oodle.dll") // TODO
    ];

    for p in paths {
        if p.exists() {
            return Ok(p.to_owned());
        }
    }
    Err("Couldn't find Oodle".to_string())
}


#[cfg(test)]
mod tests {
    use super::*;
}
