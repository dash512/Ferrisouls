pub mod bytes;
pub mod read;
pub mod write;
pub mod hash;

pub use read::BinaryReader;
pub use write::BinaryWriter;
use crate::errors::FerrisoulsError;
use std::{fs::File, path::Path, io::Read};

pub trait IO {
    //Read
    fn from_reader(reader: &mut BinaryReader) -> std::result::Result<Self, FerrisoulsError> where Self: Sized;

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
    fn to_writer(&self) -> std::result::Result<BinaryWriter, FerrisoulsError>;

    fn to_bytes(&self) -> std::result::Result<Vec<u8>, FerrisoulsError> {
        Ok(self.to_writer()?.into_inner())
    }

    fn to_file(&self, path: &Path) -> std::result::Result<(), FerrisoulsError> {
        todo!()
    }
}