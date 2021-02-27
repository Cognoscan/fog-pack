use fog_crypto::{hash::Hash, CryptoError};
use std::fmt;

use serde::{de, ser};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
    /// Occurs when a subtype is using a version format that is no longer accepted. This is mainly
    /// for recognizing when the Cryptographic types and signatures use old, no longer accepted
    /// algorithms.
    OldVersion(String),
    /// Occurs when a schema doesn't match the document's schema, a Schema was used when one isn't
    /// specified by the document, or a NoSchema was used when a document specified a schema.
    SchemaMismatch {
        actual: Option<Hash>,
        expected: Option<Hash>,
    },
    /// Occurs when serde serialization or deserialization fails
    SerdeFail(String),
    /// Occurs when the header (compression marker and optional schema) failed to parse correctly.
    BadHeader,
    /// Occurs when zstd compression fails, possibly due to a dictionary not being present in a
    /// schema, a checksum failing, or and of the other zstd failure modes.
    FailDecompress(String),
    /// Document/Entry/Query was greater than maximum allowed size on decode
    LengthTooLong { max: usize, actual: usize },
    /// Document/Entry/Query ended too early.
    LengthTooShort {
        step: &'static str,
        actual: usize,
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
            Error::BadHeader => f.write_str("Data has bad header format"),
            Error::FailDecompress(ref err) => write!(f, "Failed decompression step: {}", err),
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
            _ => None,
        }
    }
}

impl std::convert::From<CryptoError> for Error {
    fn from(e: CryptoError) -> Self {
        Self::CryptoError(e)
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
