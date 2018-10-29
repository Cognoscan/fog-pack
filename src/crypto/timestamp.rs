use std::fmt;

/// Structure for holding a raw msgpack timestamp.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.seconds, self.nanoseconds)
    }
}
