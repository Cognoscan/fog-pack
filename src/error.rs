//! Library error types.
//!
use crate::compress::CompressionError;
use fog_crypto::{hash::Hash, CryptoError};
use std::fmt;

use serde::{de, ser};

/// A fog-pack Result, normally returning a fog-pack [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[allow(unused)]
#[derive(Clone, Debug)]
enum ValidateError {
    /// Validation failed inside an array
    InArray {
        index: usize,
        err: Box<ValidateError>,
    },
    /// Validation failed inside a map
    InMap {
        key: String,
        err: Box<ValidateError>,
    },
    /// Validation failed inside an enum
    InEnum {
        name: String,
        err: Box<ValidateError>,
    },
    /// Validation of an array failed at a specific index
    FailArray { index: usize, err: String },
    /// Validation of a map failed at a specific key
    FailMap { key: String, err: String },
    /// All of the available "multi" validators failed
    FailMulti { errs: Vec<ValidateError> },
    /// The core validation error
    FailValue {
        failure: String,
        value: crate::value::Value,
    },
    /// Some other fog-pack error occurred in here
    SubError(Box<Error>),
}

/// A fog-pack error. Encompasses any issues that can happen during validation,
/// encoding, or decoding.
#[derive(Clone, Debug)]
pub enum Error {
    /// Occurs when a subtype is using a version format that is no longer accepted. This is mainly
    /// for recognizing when the Cryptographic types and signatures use old, no longer accepted
    /// algorithms.
    OldVersion(String),
    /// Occurs when a schema doesn't match the document's schema, a Schema was used when one isn't
    /// specified by the document, or a NoSchema was used when a document specified a schema.
    SchemaMismatch {
        /// The actual schema of the document
        actual: Option<Hash>,
        /// The expected schema of the document
        expected: Option<Hash>,
    },
    /// Occurs when serde serialization or deserialization fails
    SerdeFail(String),
    /// Occurs when the header (compression marker and optional schema) failed to parse correctly.
    BadHeader(String),
    /// Occurs when zstd compression fails, possibly due to a dictionary not being present in a
    /// schema, a checksum failing, or any of the other zstd failure modes.
    Compression(CompressionError),
    /// Document/Entry/Query was greater than maximum allowed size on decode
    LengthTooLong {
        /// The maximum allowed size
        max: usize,
        /// The object's actual size
        actual: usize,
    },
    /// Document/Entry/Query ended too early.
    LengthTooShort {
        /// What step of the decoding we were on when it failed.
        step: &'static str,
        /// The actual length of the object
        actual: usize,
        /// The remaining length of the object that we were expecting
        expected: usize,
    },
    /// Signature attached to end of Document/Entry/Query didn't validate
    BadSignature,
    /// Basic fog-pack encoding failure
    BadEncode(String),
    /// Schema validation failure.
    FailValidate(String),
    /// Failure within the cryptographic submodule.
    CryptoError(CryptoError),
    /// Schema or validation hit some parsing limit.
    ParseLimit(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::OldVersion(ref err) => write!(f, "Old version: {}", err),
            Error::SchemaMismatch {
                ref actual,
                ref expected,
            } => match (actual, expected) {
                (Some(actual), Some(expected)) => write!(
                    f,
                    "Expected schema {}, but document used schema {}",
                    expected, actual
                ),
                (Some(actual), None) => {
                    write!(f, "Expected no schema, but document used schema {}", actual)
                }
                (None, Some(expected)) => write!(
                    f,
                    "Expected schema {}, but document used no schema",
                    expected
                ),
                (None, None) => write!(
                    f,
                    "Expected and got no schema (this should not have been an error)"
                ),
            },
            Error::SerdeFail(ref msg) => f.write_str(msg),
            Error::BadHeader(ref err) => write!(f, "Data has bad header format: {}", err),
            Error::Compression(_) => write!(f, "Compression codec error"),
            Error::LengthTooLong { max, actual } => write!(
                f,
                "Data too long: was {} bytes, maximum allowed is {}",
                actual, max
            ),
            Error::LengthTooShort {
                step,
                actual,
                expected,
            } => write!(
                f,
                "Expected data length {}, but got {} on step [{}]",
                expected, actual, step
            ),
            Error::BadSignature => write!(f, "A signature failed to verify"),
            Error::BadEncode(ref err) => write!(f, "Basic data encoding failure: {}", err),
            Error::FailValidate(ref err) => write!(f, "Failed validation: {}", err),
            Error::CryptoError(_) => write!(f, "Cryptographic Error"),
            Error::ParseLimit(ref err) => write!(f, "Hit parsing limit: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::CryptoError(ref err) => Some(err),
            Error::Compression(ref err) => Some(err),
            _ => None,
        }
    }
}

impl std::convert::From<CryptoError> for Error {
    fn from(e: CryptoError) -> Self {
        Self::CryptoError(e)
    }
}

impl std::convert::From<CompressionError> for Error {
    fn from(e: CompressionError) -> Self {
        Self::Compression(e)
    }
}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::SerdeFail(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::SerdeFail(msg.to_string())
    }
}
