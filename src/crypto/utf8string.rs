use std::borrow::Cow;
use std::fmt::{self, Debug, Display};
use std::str::Utf8Error;

/// Represents an UTF-8 MessagePack string type.
///
/// According to the MessagePack spec, string objects may contain invalid byte sequence and the
/// behavior of a deserializer depends on the actual implementation when it received invalid byte
/// sequence.
/// Deserializers should provide functionality to get the original byte array so that applications
/// can decide how to handle the object.
///
/// Summarizing, it's prohibited to instantiate a string type with invalid UTF-8 sequences, however
/// it is possible to obtain an underlying bytes that were attempted to convert to a `String`. This
/// may happen when trying to unpack strings that were decoded using older MessagePack spec with
/// raw types instead of string/binary.
#[derive(Clone, Debug, PartialEq)]
pub struct Utf8String {
    s: Result<String, (Vec<u8>, Utf8Error)>,
}

impl Utf8String {
    /// Returns `true` if the string is valid UTF-8.
    pub fn is_str(&self) -> bool {
        self.s.is_ok()
    }

    /// Returns `true` if the string contains invalid UTF-8 sequence.
    pub fn is_err(&self) -> bool {
        self.s.is_err()
    }

    /// Returns the string reference if the string is valid UTF-8, or else `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self.s {
            Ok(ref s) => Some(s.as_str()),
            Err(..) => None,
        }
    }

    /// Returns the underlying `Utf8Error` if the string contains invalud UTF-8 sequence, or
    /// else `None`.
    pub fn as_err(&self) -> Option<&Utf8Error> {
        match self.s {
            Ok(..) => None,
            Err((_, ref err)) => Some(&err),
        }
    }

    /// Returns a byte slice of this `Utf8String`'s contents.
    pub fn as_bytes(&self) -> &[u8] {
        match self.s {
            Ok(ref s) => s.as_bytes(),
            Err(ref err) => &err.0[..],
        }
    }

    /// Consumes this object, yielding the string if the string is valid UTF-8, or else `None`.
    pub fn into_str(self) -> Option<String> {
        self.s.ok()
    }

    /// Converts a `Utf8String` into a byte vector.
    pub fn into_bytes(self) -> Vec<u8> {
        match self.s {
            Ok(s) => s.into_bytes(),
            Err(err) => err.0,
        }
    }

    pub fn as_ref(&self) -> Utf8StringRef {
        match self.s {
            Ok(ref s) => Utf8StringRef { s: Ok(s.as_str()) },
            Err((ref buf, err)) => Utf8StringRef { s: Err((&buf[..], err)) },
        }
    }
}

impl Display for Utf8String {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.s {
            Ok(ref s) => write!(fmt, "\"{}\"", s),
            Err(ref err) => Debug::fmt(&err.0, fmt),
        }
    }
}

impl<'a> From<String> for Utf8String {
    fn from(val: String) -> Self {
        Utf8String {
            s: Ok(val),
        }
    }
}

impl<'a> From<&'a str> for Utf8String {
    fn from(val: &str) -> Self {
        Utf8String {
            s: Ok(val.into()),
        }
    }
}

impl<'a> From<Cow<'a, str>> for Utf8String {
    fn from(val: Cow<'a, str>) -> Self {
        Utf8String {
            s: Ok(val.into_owned()),
        }
    }
}

/// A non-owning evil twin of `Utf8String`. Does exactly the same thing except ownership.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Utf8StringRef<'a> {
    s: Result<&'a str, (&'a [u8], Utf8Error)>,
}

impl<'a> Utf8StringRef<'a> {
    /// Returns `true` if the string is valid UTF-8.
    pub fn is_str(&self) -> bool {
        self.s.is_ok()
    }

    /// Returns `true` if the string contains invalid UTF-8 sequence.
    pub fn is_err(&self) -> bool {
        self.s.is_err()
    }

    /// Returns the string reference if the string is valid UTF-8, or else `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self.s {
            Ok(ref s) => Some(s),
            Err(..) => None,
        }
    }

    /// Returns the underlying `Utf8Error` if the string contains invalud UTF-8 sequence, or
    /// else `None`.
    pub fn as_err(&self) -> Option<&Utf8Error> {
        match self.s {
            Ok(..) => None,
            Err((_, ref err)) => Some(&err),
        }
    }

    /// Returns a byte slice of this string contents no matter whether it's valid or not UTF-8.
    pub fn as_bytes(&self) -> &[u8] {
        match self.s {
            Ok(ref s) => s.as_bytes(),
            Err(ref err) => err.0,
        }
    }

    /// Consumes this object, yielding the string if the string is valid UTF-8, or else `None`.
    pub fn into_str(self) -> Option<String> {
        self.s.ok().map(|s| s.into())
    }

    /// Converts a `Utf8StringRef` into a byte vector.
    pub fn into_bytes(self) -> Vec<u8> {
        match self.s {
            Ok(s) => s.as_bytes().into(),
            Err(err) => err.0.into(),
        }
    }
}

impl<'a> Display for Utf8StringRef<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.s {
            Ok(ref s) => write!(fmt, "\"{}\"", s),
            Err(ref err) => Debug::fmt(&err.0, fmt),
        }
    }
}

impl<'a> From<&'a str> for Utf8StringRef<'a> {
    fn from(val: &'a str) -> Self {
        Utf8StringRef {
            s: Ok(val),
        }
    }
}

impl<'a> Into<Utf8String> for Utf8StringRef<'a> {
    fn into(self) -> Utf8String {
        match self.s {
            Ok(s) => Utf8String { s: Ok(s.into()) },
            Err((buf, err)) => Utf8String { s: Err((buf.into(), err)) }
        }
    }
}
