//! Error types used throughout the crate.

use std::io;
use std::num::{ParseIntError, TryFromIntError};
use std::str::Utf8Error;
use std::string::{FromUtf8Error, FromUtf16Error};

use thiserror::Error;

/// `Result` that defaults to [`FerrisoulsError`].
pub type Result<T, E = FerrisoulsError> = std::result::Result<T, E>;

// ------------------------------------------------------------------------------------------------
// Context
// ------------------------------------------------------------------------------------------------

/// Wraps an error with a description of what was being attempted.
pub trait AddContext: Sized {
    fn add_context(self, context: impl Into<String>) -> Self;
}

/// Declares an extension trait for `Result` that converts any compatible error into `$target` and adds
/// context to it. Each target gets its own method names so the target type is never ambiguous at the `?`.
macro_rules! result_context {
    ($(#[$doc:meta])* $trait:ident => $target:ty, $eager:ident, $lazy:ident) => {
        $(#[$doc])*
        pub trait $trait<T> {
            /// Adds context to the error.
            fn $eager(self, context: impl Into<String>) -> std::result::Result<T, $target>;

            /// Adds context to the error, only building the message if there is one.
            fn $lazy<S: Into<String>>(self, context: impl FnOnce() -> S) -> std::result::Result<T, $target>;
        }

        impl<T, E: Into<$target>> $trait<T> for std::result::Result<T, E> {
            fn $eager(self, context: impl Into<String>) -> std::result::Result<T, $target> {
                self.map_err(|e| Into::<$target>::into(e).add_context(context))
            }

            fn $lazy<S: Into<String>>(self, context: impl FnOnce() -> S) -> std::result::Result<T, $target> {
                self.map_err(|e| Into::<$target>::into(e).add_context(context()))
            }
        }
    };
}

result_context!(Context => FerrisoulsError, context, with_context);
result_context!(ReadContext => BinaryReaderError, read_context, with_read_context);
result_context!(WriteContext => BinaryWriterError, write_context, with_write_context);

// ------------------------------------------------------------------------------------------------
// FormatError
// ------------------------------------------------------------------------------------------------

/// Errors about pixel formats themselves (as opposed to data in a format).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FormatError {
    #[error("DXGI format not found")]
    NotFound,

    #[error("{0} is a planar format, which needs a specific implementation")]
    Planar(String),

    #[error("{0} isn't supported for this operation")]
    Unsupported(String),
}

impl FormatError {
    pub fn planar(format: impl Into<String>) -> Self {
        Self::Planar(format.into())
    }

    pub fn unsupported(format: impl Into<String>) -> Self {
        Self::Unsupported(format.into())
    }
}

// ------------------------------------------------------------------------------------------------
// FerrisoulsError
// ------------------------------------------------------------------------------------------------

/// Top-level error returned by the Ferrisouls library.
///
/// Most callers should only need to deal with this type and use `?` throughout their code.
#[derive(Debug, Error)]
pub enum FerrisoulsError {
    #[error("Ferrisouls Error: {0}")]
    Custom(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Another error, with a description of what was being done when it happened.
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<FerrisoulsError>,
    },

    #[error("libloading error: {0}")]
    Libload(#[from] libloading::Error),

    #[error(transparent)]
    DCX(#[from] DCXError),

    /// Kept for compatibility. Prefer [`FormatError::NotFound`] in new code.
    #[error("DXGI format not found")]
    FormatNotFound,

    #[error(transparent)]
    Format(#[from] FormatError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Swizzle(#[from] SwizzleError),

    #[error(transparent)]
    BinaryReader(#[from] BinaryReaderError),

    #[error(transparent)]
    BinaryWriter(#[from] BinaryWriterError),
}

impl FerrisoulsError {
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    /// For features that exist in the format but aren't implemented here yet.
    pub fn not_implemented(what: impl Into<String>) -> Self {
        Self::NotImplemented(what.into())
    }
}

impl AddContext for FerrisoulsError {
    fn add_context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

impl From<&str> for FerrisoulsError {
    fn from(msg: &str) -> Self {
        Self::Custom(msg.to_owned())
    }
}

impl From<String> for FerrisoulsError {
    fn from(msg: String) -> Self {
        Self::Custom(msg)
    }
}

impl From<TryFromIntError> for FerrisoulsError {
    fn from(err: TryFromIntError) -> Self {
        Self::Custom(err.to_string())
    }
}


// ------------------------------------------------------------------------------------------------
// DCXError
// ------------------------------------------------------------------------------------------------

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
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(msg.into())
    }

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

impl From<&str> for DCXError {
    fn from(msg: &str) -> Self {
        Self::custom(msg.to_owned())
    }
}

impl From<String> for DCXError {
    fn from(msg: String) -> Self {
        Self::custom(msg)
    }
}

// ------------------------------------------------------------------------------------------------
// SwizzleError
// ------------------------------------------------------------------------------------------------

/// Errors related to texture de/swizzling.
#[derive(Debug, Error)]
pub enum SwizzleError {
    #[error("{0}")]
    Swizzle(String),

    #[error("{0}")]
    Deswizzle(String),

    #[error(transparent)]
    Format(#[from] FormatError),
}

impl SwizzleError {
    pub fn swizzle(msg: impl Into<String>) -> Self {
        Self::Swizzle(msg.into())
    }

    pub fn deswizzle(msg: impl Into<String>) -> Self {
        Self::Deswizzle(msg.into())
    }
}

impl From<String> for SwizzleError {
    fn from(msg: String) -> Self {
        Self::swizzle(msg)
    }
}

// ------------------------------------------------------------------------------------------------
// BinaryReaderError
// ------------------------------------------------------------------------------------------------

/// Errors encountered while reading binary data.
#[derive(Debug, Error)]
pub enum BinaryReaderError {
    #[error("{0}")]
    Custom(String),

    /// An I/O error at a known position in the reader.
    #[error("I/O error at position {position}: {source}")]
    Io {
        position: u64,

        #[source]
        source: io::Error,
    },

    /// An I/O error from something that has no reader position, such as a decompressor or a file.
    #[error("I/O error: {0}")]
    Stream(#[source] io::Error),

    #[error("integer conversion failed: {0}")]
    Conversion(#[from] TryFromIntError),

    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] Utf8Error),

    #[error("invalid UTF-16: {0}")]
    Utf16(String),

    #[error("failed to parse int: {0}")]
    ParseInt(#[from] ParseIntError),

    #[error("failed to parse xml: {0}")]
    Roxml(#[from] roxmltree::Error),

    #[error("png decoding error: {0}")]
    PngDecode(#[from] png::DecodingError),

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

    #[error(transparent)]
    Swizzle(#[from] SwizzleError),

    #[error(transparent)]
    Format(#[from] FormatError),

    #[error(transparent)]
    Dcx(#[from] DCXError),

    /// Another error, with a description of what was being read when it happened.
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<BinaryReaderError>,
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

    pub fn unexpected_eof(position: u64, requested: usize, remaining: usize) -> Self {
        Self::UnexpectedEof {
            position,
            requested,
            remaining,
        }
    }

    pub fn out_of_bounds(offset: u64, length: usize, total: u64) -> Self {
        Self::OutOfBounds {
            offset,
            length,
            total,
        }
    }
}

impl AddContext for BinaryReaderError {
    fn add_context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

impl From<io::Error> for BinaryReaderError {
    fn from(source: io::Error) -> Self {
        Self::Stream(source)
    }
}

impl From<FromUtf8Error> for BinaryReaderError {
    fn from(error: FromUtf8Error) -> Self {
        Self::Utf8(error.utf8_error())
    }
}

impl From<FromUtf16Error> for BinaryReaderError {
    fn from(error: FromUtf16Error) -> Self {
        Self::Utf16(error.to_string())
    }
}

impl From<&str> for BinaryReaderError {
    fn from(msg: &str) -> Self {
        Self::Custom(msg.to_owned())
    }
}

impl From<String> for BinaryReaderError {
    fn from(msg: String) -> Self {
        Self::Custom(msg)
    }
}

/// Unwraps errors that already have a reader equivalent and keeps the rest as text.
impl From<FerrisoulsError> for BinaryReaderError {
    fn from(error: FerrisoulsError) -> Self {
        match error {
            FerrisoulsError::BinaryReader(e) => e,
            FerrisoulsError::Swizzle(e) => Self::Swizzle(e),
            FerrisoulsError::DCX(e) => Self::Dcx(e),
            FerrisoulsError::Format(e) => Self::Format(e),
            FerrisoulsError::FormatNotFound => Self::Format(FormatError::NotFound),
            FerrisoulsError::Io(e) => Self::Stream(e),
            FerrisoulsError::Custom(msg) => Self::Custom(msg),
            FerrisoulsError::Context { context, source } => Self::from(*source).add_context(context),
            other => Self::Custom(other.to_string()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// BinaryWriterError
// ------------------------------------------------------------------------------------------------

/// Errors encountered while writing binary data.
#[derive(Debug, Error)]
pub enum BinaryWriterError {
    #[error("{0}")]
    Custom(String),

    #[error("integer conversion failed: {0}")]
    Conversion(#[from] TryFromIntError),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("png encoding error: {0}")]
    PngEncode(#[from] png::EncodingError),

    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] Utf8Error),

    #[error("out of bounds: offset {offset}, length {length}")]
    OutOfBounds { offset: u64, length: usize },

    #[error("I/O error: {0}")]
    IO(#[from] io::Error),

    /// Data that had to be read while writing (for example to validate it) was bad.
    #[error(transparent)]
    Reader(#[from] BinaryReaderError),

    #[error(transparent)]
    Swizzle(#[from] SwizzleError),

    #[error(transparent)]
    Format(#[from] FormatError),

    #[error(transparent)]
    Dcx(#[from] DCXError),

    /// Another error, with a description of what was being written when it happened.
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<BinaryWriterError>,
    },
}

impl BinaryWriterError {
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    pub fn io(error: io::Error) -> Self {
        Self::IO(error)
    }

    pub fn out_of_bounds(offset: u64, length: usize) -> Self {
        Self::OutOfBounds { offset, length }
    }
}

impl AddContext for BinaryWriterError {
    fn add_context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

impl From<&str> for BinaryWriterError {
    fn from(msg: &str) -> Self {
        Self::Custom(msg.to_owned())
    }
}

impl From<String> for BinaryWriterError {
    fn from(msg: String) -> Self {
        Self::Custom(msg)
    }
}

impl From<FromUtf8Error> for BinaryWriterError {
    fn from(error: FromUtf8Error) -> Self {
        Self::Utf8(error.utf8_error())
    }
}

/// Unwraps errors that already have a writer equivalent and keeps the rest as text.
impl From<FerrisoulsError> for BinaryWriterError {
    fn from(error: FerrisoulsError) -> Self {
        match error {
            FerrisoulsError::BinaryWriter(e) => e,
            FerrisoulsError::BinaryReader(e) => Self::Reader(e),
            FerrisoulsError::Swizzle(e) => Self::Swizzle(e),
            FerrisoulsError::DCX(e) => Self::Dcx(e),
            FerrisoulsError::Format(e) => Self::Format(e),
            FerrisoulsError::FormatNotFound => Self::Format(FormatError::NotFound),
            FerrisoulsError::Io(e) => Self::IO(e),
            FerrisoulsError::Custom(msg) => Self::Custom(msg),
            FerrisoulsError::Context { context, source } => Self::from(*source).add_context(context),
            other => Self::Custom(other.to_string()),
        }
    }
}
