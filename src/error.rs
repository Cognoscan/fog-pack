use std::{fmt, io};
use std::error;
use crypto::CryptoError;

/// The default fog-pack Result type
pub type Result<T> = std::result::Result<T, Error>;

/// Possible fog-pack error conditions.
#[derive(Debug)]
pub enum Error {
    /// Occurs when a schema doesn't match the document's schema, a Schema was used when one isn't 
    /// specified by the document, or a NoSchema was used when a document specified a schema.
    SchemaMismatch,
    /// Occurs when the header (compression marker and optional schema) failed to parse correctly.
    BadHeader,
    /// Occurs when zstd compression fails, possibly due to a dictionary not being present in a 
    /// schema, a checksum failing, or and of the other zstd failure modes.
    FailDecompress,
    /// Document/Entry/Query was greater than maximum allowed size on decode
    BadSize,
    /// Signature attached to end of Document/Entry/Query didn't validate
    BadSignature,
    /// Basic fog-pack encoding failure, with reason string and remaining bytes in buffer when 
    /// error occurred.
    BadEncode(usize, &'static str),
    /// Schema validation failure, with reason string and remaining bytes in buffer when error 
    /// occurred. Also occurs when parsing a Schema or Query and it doesn't fit the accepted 
    /// format.
    FailValidate(usize, &'static str),
    /// Failure within the cryptographic submodule.
    CryptoError(CryptoError),
    /// Schema or validation hit some parsing limit, with reason string and remaining bytes in 
    /// buffer when error occurred.
    ParseLimit(usize, &'static str),
    /// Propagated I/O error. Generally occurs when end of buffer was reached before the decoder 
    /// expected it, meaning the Document/Entry/Query is likely incomplete.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SchemaMismatch       => write!(f, "Wrong schema for document"),
            Error::BadHeader            => write!(f, "Bad doc/entry header"),
            Error::FailDecompress       => write!(f, "Failed zstd decompression"),
            Error::BadSize              => write!(f, "Size greater than max allowed"),
            Error::BadSignature         => write!(f, "Signature didn't verify against doc/entry"),
            Error::BadEncode(len, s)    => write!(f, "Bad encoding with {} bytes left: {}", len, s),
            Error::FailValidate(len, s) => write!(f, "Failed validation with {} bytes left: {}", len, s),
            Error::CryptoError(ref err) => write!(f, "Crypto: {}", err),
            Error::ParseLimit(len, s)   => write!(f, "Parsing limit reached with {} bytes left: {}", len, s),
            Error::Io(ref err)          => write!(f, "Io: {}", err),
        }
    }
}

impl error::Error for Error { }

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<CryptoError> for Error {
    fn from(err: CryptoError) -> Error {
        match err {
            CryptoError::Io(err) => Error::Io(err),
            _ => Error::CryptoError(err),
        }
    }
}
