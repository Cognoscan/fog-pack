use std::{fmt, io};
use std::error::Error;

#[derive(Debug)]
pub enum CryptoError {
    UnsupportedVersion,
    DecryptFailed,
    BadLength,
    BadKey,
    BadFormat,
    NotInStorage,
    Io(io::Error),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CryptoError::UnsupportedVersion   => write!(f, "Chosen crypto version not supported."),
            CryptoError::DecryptFailed        => write!(f, "Could not decrypt with key"),
            CryptoError::BadKey               => write!(f, "Crypto key is weak or invalid"),
            CryptoError::BadLength            => write!(f, "Provided data length is invalid"),
            CryptoError::BadFormat            => write!(f, "Format of data does not match specification"),
            CryptoError::NotInStorage         => write!(f, "Provided Key/Identity/StreamKey is not in storage"),
            CryptoError::Io(ref err)          => err.fmt(f),
        }
    }
}

impl Error for CryptoError {
    fn description(&self) -> &str {
        match *self {
            CryptoError::UnsupportedVersion   => "unsupported version",
            CryptoError::DecryptFailed        => "decryption failed",
            CryptoError::BadKey               => "weak or invalid key",
            CryptoError::BadLength            => "invalid data length",
            CryptoError::BadFormat            => "incorrect format for data",
            CryptoError::NotInStorage         => "Key/Identity/StreamKey not in storage",
            CryptoError::Io(ref err)          => err.description(),
        }
    }
}

impl From<io::Error> for CryptoError {
    fn from(err: io::Error) -> CryptoError {
        CryptoError::Io(err)
    }
}
