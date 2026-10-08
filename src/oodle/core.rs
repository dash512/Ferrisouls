
use std::{ffi::c_void, path::{Path, PathBuf}, sync::OnceLock};
use libloading::{Library, Symbol};

use crate::errors::DCXError;
use crate::oodle::bindings::*;
use crate::oodle::structs::*;
use crate::oodle::enums::*;
use crate::steam::find_steam_game;

pub trait Oodle {
    type CompressOptions;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> where Self: Sized;

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: &OodleSettings) -> Result<usize, DCXError>;

    ///Decompresses `input` data into `output`. Returns number of bytes written.
    unsafe fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, DCXError>;

    ///Returns size required for the buffer that takes the compressed output of `compress`
    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError>;

    //Returns default compression options for given settings. (Compressor, CompressionLevel)
    unsafe fn get_default_compress_options(&self, settings: &OodleSettings) -> *mut Self::CompressOptions;

    //Returns the buffer size needed for decompression, optionally accounting for possible data corruption.
    //This is not typically needed for Fromsoft games, as decompressed size is stored in DCX headers.
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, compressor: Compressor, corruptable: bool) -> Result<usize, DCXError>;

    // Same as `get_decode_buffer_size` but for overwriting compressed data in-memory without excessive additional allocation.
    unsafe fn get_decode_buffer_size_in_place(&self, compressor: Compressor, raw_size: usize, corruptable: bool) -> Result<usize, DCXError> {
        Err(DCXError::Unsupported("This function is not implemented for your Oodle version.".to_string()))
    }

    ///Returns size required for the buffer that takes the compressed output of `compress`
    unsafe fn get_compress_scratch_bound(&self, compressor: Compressor, raw_size: usize) -> Result<usize, DCXError> {
        Err(DCXError::Unsupported("This function is not implemented for your Oodle version.".to_string()))
    }
}


#[derive(Debug)]
pub struct Oodle26 {
    _lib: Library,
    lz_compress: o26::OodleLZ_Compress,
    lz_decompress: o26::OodleLZ_Decompress,
    lz_get_default_options: o26::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: o26::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: o26::OodleLZ_GetDecodeBufferSize
}

impl Oodle for Oodle26 {
    type CompressOptions = O26CompressOptions;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<o26::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<o26::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<o26::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<o26::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<o26::OodleLZ_GetDecodeBufferSize>(
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

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: &OodleSettings) -> Result<usize, DCXError> {
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
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else if result as usize > output.len() {
            Err(DCXError::Custom("Oodle returned a size larger than the output buffer.".to_string()))
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
                FuzzSafe::Yes,
                CheckCRC::No,
                Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_comp_size)(raw_size as isize) as usize ) }
    }

    unsafe fn get_default_compress_options(&self, settings: &OodleSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)(settings.compressor, settings.level) }
    }

    ///Compressor argument is not needed in Oodle2.6, but the field is still required for transparency.
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, compressor: Compressor, corruptable: bool) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_decode_size)(raw_size as isize, corruptable as i32) as usize ) }
    }
    
}


#[derive(Debug)]
pub struct Oodle28 {
    _lib: Library,
    compressor: Option<Compressor>,
    lz_compress: o28::OodleLZ_Compress,
    lz_decompress: o28::OodleLZ_Decompress,
    lz_get_default_options: o28::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: o28::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: o28::OodleLZ_GetDecodeBufferSize,
    lz_get_scratchmem_bound: o28::OodleLZ_GetCompressScratchMemBound
}

impl Oodle for Oodle28 {
    type CompressOptions = O28CompressOptions;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<o28::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<o28::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<o28::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<o28::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<o28::OodleLZ_GetDecodeBufferSize>(
                b"OodleLZ_GetDecodeBufferSize\0"
            )?
        };

        let get_scratchmem_bound = unsafe {
            *lib.get::<o28::OodleLZ_GetCompressScratchMemBound>(
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

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: &OodleSettings) -> Result<usize, DCXError> {
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
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else if result as usize > output.len() {
            Err(DCXError::Custom("Oodle returned a size larger than the output buffer.".to_string()))
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
                FuzzSafe::Yes,
                CheckCRC::No,
                Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::Custom("Failed to get needed buffer size for compression: compressor was not given. 
            Try setting it with `set_compressor`.".to_string())),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_comp_size)(comp, raw_size as isize) as usize )
            }
        }
    }

    unsafe fn get_default_compress_options(&self, settings: &OodleSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)(settings.compressor, settings.level) }
    }

    ///Requires compressor to be set with `Oodle28.set_compressor()`
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, compressor: Compressor, corruptable: bool) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_decode_size)(compressor, raw_size as isize, corruptable as i32) as usize ) }        
    }

    // Get size of buffer required for compressed data. Never returns error.
    unsafe fn get_compress_scratch_bound(&self, compressor: Compressor, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_scratchmem_bound)(compressor, raw_size as isize) as usize ) }
    }

}


#[derive(Debug)]
pub struct Oodle29 {
    _lib: Library,
    compressor: Option<Compressor>,
    lz_compress: o29::OodleLZ_Compress,
    lz_decompress: o29::OodleLZ_Decompress,
    lz_get_default_options: o29::OodleLZ_CompressOptions_GetDefault,
    lz_get_comp_size: o29::OodleLZ_GetCompressedBufferSizeNeeded,
    lz_get_decode_size: o29::OodleLZ_GetDecodeBufferSize,
    lz_get_scratchmem_bound: o29::OodleLZ_GetCompressScratchMemBound
}

impl Oodle for Oodle29 {
    type CompressOptions = O29CompressOptions;

    unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
        let lib: Library = unsafe { Library::new(path)? }; // "oo2core_win64.dll"

        let compress = unsafe {
            *lib.get::<o29::OodleLZ_Compress>(
                b"OodleLZ_Compress\0"
            )?
        };

        let decompress = unsafe {
            *lib.get::<o29::OodleLZ_Decompress>(
                b"OodleLZ_Decompress\0"
            )?
        };

        let get_default_options = unsafe {
            *lib.get::<o29::OodleLZ_CompressOptions_GetDefault>(
                b"OodleLZ_CompressOptions_GetDefault\0"
            )?
        };

        let get_required_size = unsafe {
            *lib.get::<o29::OodleLZ_GetCompressedBufferSizeNeeded>(
                b"OodleLZ_GetCompressedBufferSizeNeeded\0"
            )?
        };

        let get_decode_size = unsafe {
            *lib.get::<o29::OodleLZ_GetDecodeBufferSize>(
                b"OodleLZ_GetDecodeBufferSize\0"
            )?
        };

        let get_scratchmem_bound = unsafe {
            *lib.get::<o29::OodleLZ_GetCompressScratchMemBound>(
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

    unsafe fn compress(&self, input: &[u8], output: &mut [u8], settings: &OodleSettings) -> Result<usize, DCXError> {
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
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else if result as usize > output.len() {
            Err(DCXError::Custom("Oodle returned a size larger than the output buffer.".to_string()))
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
                FuzzSafe::Yes,
                CheckCRC::No,
                Verbosity::Null,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                DecodeThreadPhase::ThreadPhaseAll,
            )
        };
        if result == 0 {
            Err(DCXError::Custom("Oodle returned 0 on decompression.".to_string()))
        } else {
            Ok(result as usize)
        }
    }

    unsafe fn get_compressed_buffer_size(&self, raw_size: usize) -> Result<usize, DCXError> {
        match self.compressor {
            None => Err(DCXError::Custom("Failed to get needed buffer size for compression: compressor was not given. 
            Try setting it with `set_compressor`.".to_string())),
            Some(comp) => unsafe { 
                Ok( (self.lz_get_comp_size)(comp, raw_size as isize) as usize )
            }
        }
    }

    ///This function takes no arguments as of Oodle2.9. The settings field is still required for transparency but isn't used.
    unsafe fn get_default_compress_options(&self, settings: &OodleSettings) -> *mut Self::CompressOptions {
        unsafe { (self.lz_get_default_options)() }
    }

    ///Requires compressor to be set with `Oodle28.set_compressor()`
    unsafe fn get_decode_buffer_size(&self, raw_size: usize, compressor: Compressor, corruptable: bool) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_decode_size)(compressor, raw_size as isize, corruptable as i32) as usize ) }  
    }

    // Get size of buffer required for compressed data. Never returns error.
    unsafe fn get_compress_scratch_bound(&self, compressor: Compressor, raw_size: usize) -> Result<usize, DCXError> {
        unsafe { Ok( (self.lz_get_scratchmem_bound)(compressor, raw_size as isize) as usize ) }
    }

}


#[derive(Debug)]
pub enum OodleType {
    O26(Oodle26),
    O28(Oodle28),
    O29(Oodle29)
}

impl OodleType {
    ///Decompresses `compressed` into `decompressed`. Returns amount of bytes written.
    pub unsafe fn decompress(&self, compressed: &[u8], decompressed: &mut [u8]) -> Result<usize, DCXError> {
        match self {
            OodleType::O26(inst) => unsafe {
                inst.decompress(compressed,decompressed) 
            },
            OodleType::O28(inst) => unsafe {
                inst.decompress(compressed,decompressed) 
            },
            OodleType::O29(inst) => unsafe {
                inst.decompress(compressed,decompressed)
            },
        }
    }

    ///Compresses and returns `input` as per `settings`.
    pub unsafe fn compress(&self, input: &[u8], settings: OodleSettings) -> Result<Vec<u8>, DCXError> {
        match self {
            OodleType::O26(inst) => unsafe {
                let required_size = inst.get_compressed_buffer_size(input.len())?;
                let mut output: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(input, &mut output, &settings)?;
                output.truncate(compressed_len);
                Ok(output)
            },
            OodleType::O28(inst) => unsafe {
                let required_size = inst.get_compress_scratch_bound(settings.compressor, input.len())?;
                let mut output: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(input, &mut output, &settings)?;
                output.truncate(compressed_len);
                Ok(output)
            },
            OodleType::O29(inst) => unsafe {
                let required_size = inst.get_compress_scratch_bound(settings.compressor, input.len())?;
                let mut output: Vec<u8> = vec![0u8; required_size];
                let compressed_len = inst.compress(input, &mut output, &settings)?;
                output.truncate(compressed_len);
                Ok(output)
            },
            _=> Err(DCXError::Custom(format!("Invalid Oodle compress settings: {:?} with {:?}", self, settings)))
        }
    }

    pub unsafe fn get(path: &Path) -> Result<OodleType, String> {
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
        let try_find = |root: Option<&Path>| -> Option<PathBuf> {
            let root = root?;

            for e in [
                "oo2core_6_win64.dll",
                "Game/oo2core_6_win64.dll",
                "Game/oo2core_8_win64.dll",
                "Game/oo2core_9_win64.dll",
            ] {
                let path = root.join(e);

                if path.is_file() {
                    return Some(path);
                }
            }
            None
        };

        let cwd = std::env::current_dir().ok();

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_owned));

        let any_game = find_steam_game(2622380)
            .or_else(|| find_steam_game(1245620))
            .or_else(|| find_steam_game(814380))
            .or_else(|| find_steam_game(1888160));

        try_find(cwd.as_deref())
            .or_else(|| try_find(exe_dir.as_deref()))
            .or_else(|| try_find(any_game.as_deref()))
            .ok_or_else(|| "Couldn't find Oodle".to_string())
    }

}



static OODLE: OnceLock<OodleType> = OnceLock::new();

pub unsafe fn init_oodle(path: &Path) -> Result<(), String> {
    let oodle = unsafe { OodleType::get(path)? };

    OODLE
        .set(oodle)
        .map_err(|_| "Oodle has already been initialized.".to_string())
}

pub fn get_oodle() -> Result<&'static OodleType, DCXError> {
    OODLE
        .get()
        .ok_or_else(|| {
            DCXError::custom("Oodle has not been loaded. Call init_oodle() first.")
        })
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_oo2core() {
        println!("{:?}", OodleType::find_oodle())
    }
}
