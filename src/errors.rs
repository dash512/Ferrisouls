use std::io;
use std::num::TryFromIntError;

use thiserror::Error;

/// Top-level error returned by the Ferrisouls library.
///
/// Most callers should only need to deal with this type and use `?`
/// throughout their code.
#[derive(Debug, Error)]
pub enum FerrisoulsError {
    #[error(transparent)]
    DCX(#[from] DCXError),

    #[error("DXGI format not found")]
    FormatNotFound,

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Swizzle(#[from] SwizzleError),

    #[error(transparent)]
    BinaryReader(#[from] BinaryReaderError),

    #[error(transparent)]
    BinaryWriter(#[from] BinaryWriterError),
}



/// Errors encountered while processing DCX data.
#[derive(Debug, Error)]
pub enum DCXError {
    #[error("compression failed: {source}")]
    Compression {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("decompression failed: {source}")]
    Decompression {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("invalid DCX data: {0}")]
    InvalidData(String),

    #[error("DCX error: {0}")]
    Custom(String),

    #[error("can't de/compress DCX type: {0}")]
    Unsupported(String),
}

impl DCXError {
    pub fn compression<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Compression {
            source: Box::new(source),
        }
    }

    pub fn decompression<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Decompression {
            source: Box::new(source),
        }
    }
}


/// Errors related to texture de/swizzling.
#[derive(Debug, Error)]
pub enum SwizzleError {
    #[error("{0}")]
    Swizzle(String),

    #[error("{0}")]
    Deswizzle(String),
}

impl SwizzleError {
    pub fn swizzle(msg: impl Into<String>) -> Self {
        Self::Swizzle(msg.into())
    }

    pub fn deswizzle(msg: impl Into<String>) -> Self {
        Self::Deswizzle(msg.into())
    }
}


/// Errors encountered while reading binary data.
#[derive(Debug, Error)]
pub enum BinaryReaderError {
    #[error("{0}")]
    Custom(String),

    #[error("I/O error at position {position}: {source}")]
    Io {
        position: u64,

        #[source]
        source: io::Error,
    },

    #[error("integer conversion failed: {0}")]
    Conversion(#[from] TryFromIntError),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error(
        "unexpected EOF at position {position}: \
         requested {requested} bytes, {remaining} bytes remaining"
    )]
    UnexpectedEof {
        position: u64,
        requested: usize,
        remaining: usize,
    },

    #[error(
        "out of bounds: offset {offset}, length {length}, \
         reader length {total}"
    )]
    OutOfBounds {
        offset: u64,
        length: usize,
        total: u64,
    },
}

impl BinaryReaderError {
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    pub fn io(position: u64, source: io::Error) -> Self {
        Self::Io { position, source }
    }

    pub fn unexpected_eof(
        position: u64,
        requested: usize,
        remaining: usize,
    ) -> Self {
        Self::UnexpectedEof {
            position,
            requested,
            remaining,
        }
    }

    pub fn out_of_bounds(
        offset: u64,
        length: usize,
        total: u64,
    ) -> Self {
        Self::OutOfBounds {
            offset,
            length,
            total,
        }
    }
}

impl From<io::Error> for BinaryReaderError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            position: 0,
            source,
        }
    }
}


/// Errors encountered while writing binary data.
#[derive(Debug, Error)]
pub enum BinaryWriterError {
    #[error("{0}")]
    Custom(String),

    #[error("integer conversion failed: {0}")]
    Conversion(#[from] TryFromIntError),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error(
        "out of bounds: offset {offset}, length {length}"
    )]
    OutOfBounds {
        offset: u64,
        length: usize,
    },
}

impl BinaryWriterError {
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    pub fn out_of_bounds(
        offset: u64,
        length: usize,
    ) -> Self {
        Self::OutOfBounds { offset, length }
    }
}
