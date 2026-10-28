//! This module contains the generic `IO` trait which is used for structs that are represented as bytes at any point.
//! It allows read and write operations with custom logic over how the data structure is handled.

pub mod misc;
pub mod read;
pub mod write;

pub use read::BinaryReader;
pub use write::BinaryWriter;
pub use misc::{ByteOrder, VariableUInt};

use zstd::zstd_safe::WriteBuf;
use crate::binders::bnd4::BND4;
use crate::dcx::{Compress, CompressSettings, DCXType, Decompress};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};
use std::{borrow::Cow, fs::File, io::{Read, Write}, path::Path};

/// Decompresses `data` if it starts with a DCX/DCP magic, otherwise borrows it unchanged.
pub unsafe fn decompress_if_needed(data: &[u8]) -> Result<(Cow<'_, [u8]>, DCXType), FerrisoulsError> {
    if data.starts_with(b"DCX\0") || data.starts_with(b"DCP\0") {
        let (decompressed, dt) = unsafe { Decompress::raw(data) }?;
        Ok((Cow::Owned(decompressed), dt))
    } else {
        Ok((Cow::Borrowed(data), DCXType::Null))
    }
}

pub trait IO {
    //Read

    ///Create new `Self` from a binary reader. Unimplemented by default. Will panic if called from any other
    ///read methods without being implemented.
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> where Self: Sized {
        unimplemented!()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        Ok(Self::from_reader(
            &mut BinaryReader::new(data, true, false)
        )?)
    }

    fn from_file(mut f: &File) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        let mut data = Vec::<u8>::new();
        f.read_to_end(&mut data)?;
        Ok(Self::from_bytes(&data)?)
    }

    fn from_path(path: &Path) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        Self::from_file(&File::open(path)?)
    }

    fn from_binder(binder: &mut BND4) -> Result<Self, BinaryReaderError> where Self: Sized {
        unimplemented!()
    }

    ///Similar to `from_path`, but handles the case where the source file is DCX compressed.
    /// 
    ///If the file Path ends in `.dcx`, decompresses the file, calling `from_bytes` on the data. 
    ///If not, calls `from_file` directly.
    /// 
    ///If you're trying to unpack a MetaBinder, use `unpack_binder()` instead.
    unsafe fn unpack(path: &Path) -> Result<(Self, DCXType), BinaryReaderError>
    where Self: Sized {
        let data = std::fs::read(path)?;
        let (bytes, dt) = unsafe { decompress_if_needed(&data) }
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;
        Ok((Self::from_bytes(&bytes)?, dt))
    }

    //Write

    ///Append self to existing binary writer. Unimplemented by default. Will panic if called from any other
    ///write methods without being implemented.
    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        unimplemented!()
    }

    ///Return new binary writer using self implementation.  
    ///Some structs may only implement `into_writer` and not `to_writer`.
    ///This is usually done when you want a "top-level" data structure 
    ///where a bunch of children get appended to it with their own `to_writer`
    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();
        self.to_writer(&mut writer)?;
        Ok(writer)
    }

    fn to_bytes(&mut self) -> Result<Vec<u8>, BinaryWriterError> {
        Ok(self.into_writer()?.into_inner())
    }

    ///Writes self as bytes to a specified path, compressing if needed. Raises error if the file already exists.
    unsafe fn to_file(&mut self, path: &Path, settings: CompressSettings) -> Result<(), BinaryWriterError> {
        let bytes = self.to_bytes()?;
        let compressed = unsafe { Compress::raw(&bytes, &settings) }
            .map_err(|e| BinaryWriterError::Custom(e.to_string()))?;

        let mut file = File::create_new(path)
            .map_err(|e| BinaryWriterError::custom(format!("{}: {}", path.display(), e)))?;
        file.write_all(&compressed).map_err(BinaryWriterError::io)?;

        Ok(())
    }

    ///Pack self into a BND4 to write. Unimplemented by default. Will panic if called before implementation.
    /// 
    ///If you're trying to pack a MetaBinder, use `pack_binder()` instead.
    unsafe fn pack(&mut self) -> Result<BND4, BinaryWriterError> {
        unimplemented!()
    }

}


/// Similar to `IO`, but for split formats stored across a header file and a data file (BXF3 / BXF4).
pub trait SplitIO {
    //Read

    ///Create new `Self` from a header reader and a data reader. Unimplemented by default.
    fn from_readers(bhd: &mut BinaryReader, bdt: &mut BinaryReader) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        unimplemented!()
    }

    fn from_bytes(bhd: &[u8], bdt: &[u8]) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        Ok(Self::from_readers(
            &mut BinaryReader::new(bhd, true, false),
            &mut BinaryReader::new(bdt, true, false),
        )?)
    }

    fn from_files(mut bhd: &File, mut bdt: &File) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        let mut bhd_data = Vec::<u8>::new();
        let mut bdt_data = Vec::<u8>::new();
        bhd.read_to_end(&mut bhd_data)?;
        bdt.read_to_end(&mut bdt_data)?;
        Ok(Self::from_bytes(&bhd_data, &bdt_data)?)
    }

    fn from_paths(bhd_path: &Path, bdt_path: &Path) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        Self::from_files(&File::open(bhd_path)?, &File::open(bdt_path)?)
    }

    ///Similar to `from_paths`, but handles either file being DCX compressed.
    ///Returns the `DCXType` of the header and the data file respectively.
    unsafe fn unpack(bhd_path: &Path, bdt_path: &Path) -> Result<(Self, DCXType, DCXType), BinaryReaderError>
    where Self: Sized {
        let bhd_data = std::fs::read(bhd_path)?;
        let bdt_data = std::fs::read(bdt_path)?;

        let (bhd_bytes, bhd_dt) = unsafe { decompress_if_needed(&bhd_data) }
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;
        let (bdt_bytes, bdt_dt) = unsafe { decompress_if_needed(&bdt_data) }
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;

        Ok((Self::from_bytes(&bhd_bytes, &bdt_bytes)?, bhd_dt, bdt_dt))
    }

    //Write

    ///Return new `(bhd, bdt)` binary writers using self implementation. Unimplemented by default.
    fn into_writers(&mut self) -> Result<(BinaryWriter, BinaryWriter), BinaryWriterError> {
        unimplemented!()
    }

    fn to_bytes(&mut self) -> Result<(Vec<u8>, Vec<u8>), BinaryWriterError> {
        let (bhd, bdt) = self.into_writers()?;
        Ok((bhd.into_inner(), bdt.into_inner()))
    }

    ///Writes self as bytes to the header and data paths, compressing each if needed.
    ///Raises error if either file already exists.
    unsafe fn to_files(&mut self, bhd_path: &Path, bdt_path: &Path, bhd_settings: CompressSettings, bdt_settings: CompressSettings) -> Result<(), BinaryWriterError> {
        let (bhd_bytes, bdt_bytes) = self.to_bytes()?;

        let bhd_compressed = unsafe { Compress::raw(&bhd_bytes, &bhd_settings) }
            .map_err(|e| BinaryWriterError::Custom(e.to_string()))?;
        let bdt_compressed = unsafe { Compress::raw(&bdt_bytes, &bdt_settings) }
            .map_err(|e| BinaryWriterError::Custom(e.to_string()))?;

        let mut bhd_file = File::create_new(bhd_path)
            .map_err(|e| BinaryWriterError::custom(format!("{}: {}", bhd_path.display(), e)))?;
        let mut bdt_file = File::create_new(bdt_path)
            .map_err(|e| BinaryWriterError::custom(format!("{}: {}", bdt_path.display(), e)))?;

        bhd_file.write_all(&bhd_compressed).map_err(BinaryWriterError::io)?;
        bdt_file.write_all(&bdt_compressed).map_err(BinaryWriterError::io)?;

        Ok(())
    }
}