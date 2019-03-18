use super::crypto::CryptoError;
use std::{fmt, io};
use std::error::Error;

#[derive(Debug)]
pub enum DbError {
    Io(io::Error),
    Crypto(CryptoError),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DbError::Crypto(ref err) => err.fmt(f),
            DbError::Io(ref err)     => err.fmt(f),
        }
    }
}

impl Error for DbError {
    fn description(&self) -> &str {
        match *self {
            DbError::Crypto(ref err) => err.description(),
            DbError::Io(ref err)     => err.description(),
        }
    }
}

impl From<io::Error> for DbError {
    fn from(err: io::Error) -> DbError {
        DbError::Io(err)
    }
}

impl From<CryptoError> for DbError {
    fn from(err: CryptoError) -> DbError {
        DbError::Crypto(err)
    }
}
