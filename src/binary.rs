pub mod bytes;
pub mod read;
pub mod write;

pub use read::BinaryReader;
pub use write::BinaryWriter;
use zstd::zstd_safe::WriteBuf;
use crate::{dcx::{Compress, CompressSettings, DCXType, Decompress}, errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError}};
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
        Ok(Self::from_reader(
            &mut BinaryReader::new(&data, true, false)
        )?)
    }

    fn from_path(path: &Path) -> Result<Self, BinaryReaderError>
    where Self: Sized {
        Self::from_file(&File::open(path)?)
    }

    ///Similar to `from_path`, but handles the case where the source file is DCX compressed.
    /// 
    ///If the file Path ends in `.dcx`, decompresses the file, calling `from_bytes` on the data. 
    ///If not, calls `from_file` directly.
    unsafe fn unpack(path: &Path) -> Result<(Self, DCXType), BinaryReaderError>
    where Self: Sized {
        let data = std::fs::read(path)?;
        let (bytes, dt) = unsafe { decompress_if_needed(&data) }
            .map_err(|e| BinaryReaderError::custom(e.to_string()))?;
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
            .map_err(|e| BinaryWriterError::custom(e.to_string()))?;

        let mut file = File::create_new(path)
            .map_err(|e| BinaryWriterError::custom(format!("{}: {}", path.display(), e)))?;
        file.write_all(&compressed).map_err(BinaryWriterError::io)?;
        
        Ok(())
    }

}