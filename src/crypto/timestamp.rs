use std::fmt;
use std::ops;
use std::cmp;

/// Structure for holding a raw msgpack timestamp.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub sec: i64,
    pub nano: u32,
}

impl Timestamp {
    /// Create a UTC timestamp from a raw seconds + nanoseconds value
    pub fn from_raw(sec: i64, nano: u32) -> Option<Timestamp> {
        if nano > 1_999_999_999 {
            None
        }
        else {
            Some(Timestamp { sec, nano })
        }
    }

    /// Return the UNIX timestamp (number of seconds since January 1, 1970 
    /// 0:00:00 UTC)
    pub fn timestamp(&self) -> i64 {
        self.sec
    }

    pub fn timestamp_subsec_nanos(&self) -> u32 {
        self.nano
    }

}

impl ops::Add<i64> for Timestamp {
    type Output = Timestamp;
    fn add(self, rhs: i64) -> Self {
        Timestamp {
            sec: self.sec + rhs,
            nano: self.nano
        }
    }
}

impl ops::Sub<i64> for Timestamp {
    type Output = Timestamp;
    fn sub(self, rhs: i64) -> Self {
        Timestamp {
            sec: self.sec - rhs,
            nano: self.nano
        }
    }
}

impl cmp::Ord for Timestamp {
    fn cmp(&self, other: &Timestamp) -> cmp::Ordering {
        if self.sec == other.sec {
            self.nano.cmp(&other.nano)
        }
        else {
            self.sec.cmp(&other.sec)
        }
    }
}

impl cmp::PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Timestamp) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} sec + {} ns", self.sec, self.nano)
    }
}
