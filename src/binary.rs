pub mod bytes;
pub mod read;
pub mod write;

pub use read::BinaryReader;
pub use write::BinaryWriter;
use crate::errors::FerrisoulsError;
use std::{fs::File, path::Path, io::Read};

pub trait IO {
    //Read
    fn from_reader(reader: &mut BinaryReader) -> std::result::Result<Self, FerrisoulsError> where Self: Sized {
        unimplemented!()
    }

    fn from_bytes(data: &[u8]) -> std::result::Result<Self, FerrisoulsError>
    where Self: Sized {
        Ok(Self::from_reader(
            &mut BinaryReader::from(data, true, false)
        )?)
    }

    fn from_file(mut f: &File) -> std::result::Result<Self, FerrisoulsError>
    where Self: Sized {
        let mut data = Vec::<u8>::new();
        f.read_to_end(&mut data)?;
        Ok(Self::from_reader(
            &mut BinaryReader::from(&data, true, false)
        )?)
    }

    fn from_path(path: &Path) -> std::result::Result<Self, FerrisoulsError>
    where Self: Sized {
        let file = File::open(path)?;
        Ok(Self::from_file(&file)?)
    }

    //Write

    ///Append self to existing binary writer.
    fn to_writer(&self, writer: &mut BinaryWriter) -> std::result::Result<(), FerrisoulsError> {
        unimplemented!()
    }

    ///Return new binary writer using self implementation.  
    ///Some structs may only implement `into_writer` and not `to_writer`.
    ///This is usually done when you want a "top-level" data structure 
    ///where a bunch of children get appended to it with their own `to_writer`
    fn into_writer(&self) -> std::result::Result<BinaryWriter, FerrisoulsError> {
        let mut writer = BinaryWriter::default();
        self.to_writer(&mut writer);
        Ok(writer)
    }

    fn to_bytes(&self) -> std::result::Result<Vec<u8>, FerrisoulsError> {
        Ok(self.into_writer()?.into_inner())
    }

    fn to_file(&self, path: &Path) -> std::result::Result<(), FerrisoulsError> {
        todo!()
    }
}