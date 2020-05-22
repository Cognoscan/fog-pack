use std::{fmt, io};
use std::error;
use crypto::CryptoError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    SchemaMismatch,
    BadHeader,
    FailDecompress,
    BadSize,
    BadSignature,
    BadEncode(usize, &'static str),
    FailValidate(usize, &'static str),
    CryptoError(CryptoError),
    ParseLimit(usize, &'static str),
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
        Error::CryptoError(err)
    }
}
