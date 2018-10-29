use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind};
use std::error;

use rmp::decode::{MarkerReadError, ValueReadError};
use rmpv::Value;
use rmpv;


#[derive(Debug)]
pub enum Error {
    /// Error from reading marker byte.
    InvalidMarkerRead(io::Error),
    /// Error from reading data.
    InvalidDataRead(io::Error),
    /// Error from incorrect signature
    InvalidSignature(),
    /// Error from incorrect Identity
    InvalidIdentity(),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidMarkerRead(..) => "I/O error while reading marker byte",
            Error::InvalidDataRead(..)   => "I/O error while reading non-marker bytes",
            Error::InvalidSignature()    => "Encountered an invalid signature during decoding",
            Error::InvalidIdentity()     => "Encountered an identity type with invalid content",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::InvalidMarkerRead(ref err) => Some(err),
            Error::InvalidDataRead(ref err) => Some(err),
            Error::InvalidSignature() => None,
            Error::InvalidIdentity() => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Error::InvalidMarkerRead(ref err) => {
                write!(fmt, "I/O error while reading marker byte: {}", err)
            },
            Error::InvalidDataRead(ref err) => {
                write!(fmt, "I/O error while reading non-marker bytes: {}", err)
            },
            Error::InvalidCrypto(ref err) => {
                write!(fmt, "A cryptographic primitive did not pass validation: {}", err)
            },
            Error::InvalidExt(ref err) => {
                write!(fmt, "A supported Ext type did not pass validation: {}", err)
            },
        }
    }
}
