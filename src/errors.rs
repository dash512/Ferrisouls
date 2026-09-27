use std::error::Error;
use std::fmt;
use std::io;
use std::num::TryFromIntError;


#[derive(Debug)]
pub enum FerrisoulsError {
    DCX(DCXError),
    FormatNotFound(FormatNotFoundError),
    Swizzle(SwizzleError),
    BinaryReader(BinaryReaderError),
    BinaryWriter(BinaryWriterError),
}

impl Error for FerrisoulsError {}
impl fmt::Display for FerrisoulsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DCX(e) => write!(f, "{}", e),
            Self::FormatNotFound(e) => write!(f, "{}", e),
            Self::Swizzle(e) => write!(f, "{}", e),
            Self::BinaryReader(e) => write!(f, "{}", e),
            Self::BinaryWriter(e) => write!(f, "{}", e),
        }
    }
}


impl From<DCXError> for FerrisoulsError {
    fn from(value: DCXError) -> Self {
        Self::DCX(value)
    }
}
impl From<FormatNotFoundError> for FerrisoulsError {
    fn from(value: FormatNotFoundError) -> Self {
        Self::FormatNotFound(value)
    }
}
impl From<SwizzleError> for FerrisoulsError {
    fn from(value: SwizzleError) -> Self {
        Self::Swizzle(value)
    }
}
impl From<BinaryReaderError> for FerrisoulsError {
    fn from(value: BinaryReaderError) -> Self {
        Self::BinaryReader(value)
    }
}
impl From<BinaryWriterError> for FerrisoulsError {
    fn from(value: BinaryWriterError) -> Self {
        Self::BinaryWriter(value)
    }
}
impl From<io::Error> for FerrisoulsError {
    fn from(source: io::Error) -> Self {
        Self::BinaryReader(BinaryReaderError::Io { position: 0, source: source }) 
    }
}
impl From<TryFromIntError> for FerrisoulsError {
    fn from(error: TryFromIntError) -> Self {
        Self::BinaryReader(BinaryReaderError::Conversion(error))
    }
}



#[derive(Debug, Clone)]
pub struct DCXError {
    pub msg: String,
}
impl DCXError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}
impl Error for DCXError {}
impl fmt::Display for DCXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}


#[derive(Debug, Clone)]
pub struct FormatNotFoundError;
impl Error for FormatNotFoundError {}
impl fmt::Display for FormatNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DXGI Format not found!")
    }
}



#[derive(Debug, Clone)]
pub struct SwizzleError {
    msg: String,
}
impl SwizzleError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}
impl Error for SwizzleError {}
impl fmt::Display for SwizzleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}



#[derive(Debug)]
pub enum BinaryReaderError {
    Custom(String),
    Io {
        position: u64,
        source: io::Error,
    },
    Conversion(TryFromIntError),
    InvalidData(String),
    UnexpectedEof {
        position: u64,
        requested: usize,
        remaining: usize,
    },
    OutOfBounds {
        offset: u64,
        length: usize,
        total: u64,
    },
}
impl Error for BinaryReaderError {}
impl From<io::Error> for BinaryReaderError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            position: 0,
            source,
        }
    }
}
impl From<TryFromIntError> for BinaryReaderError {
    fn from(error: TryFromIntError) -> Self {
        Self::Conversion(error)
    }
}
impl fmt::Display for BinaryReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(e) => write!(f, "{}", e),
            Self::InvalidData(e) => write!(f, "Invalid Data: {}", e),
            Self::OutOfBounds{offset, length, total} => {
                write!(f, "Out of Bounds! Tried to access {}; {} for a reader with length: {}", offset, length, total)
            },
            Self::Io{position, source} => write!(f, "Error at {} in reader:\n{}", position, source),
            Self::UnexpectedEof{position, requested, remaining} => {
                write!(f, "Unexpected EoF! Requested {} bytes at {} for a reader with {} bytes remaining.",
                requested, position, remaining)
            },
            Self::Custom(e) => write!(f, "{}", e)
        }
    }
}



#[derive(Debug)]
pub enum BinaryWriterError {
    Custom(String),
    Conversion(TryFromIntError),
    InvalidData(String),
    OutOfBounds {
        offset: u64,
        length: usize,
    },
}
impl Error for BinaryWriterError {}
impl From<TryFromIntError> for BinaryWriterError {
    fn from(error: TryFromIntError) -> Self {
        Self::Conversion(error)
    }
}
impl fmt::Display for BinaryWriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(e) => write!(f, "{}", e),
            Self::InvalidData(e) => write!(f, "Invalid Data: {}", e),
            Self::OutOfBounds{offset, length} => {
                write!(f, "Out of Bounds! Tried to access {}; {}.", offset, length)
            },
            Self::Custom(e) => write!(f, "{}", e)
        }
    }
}


