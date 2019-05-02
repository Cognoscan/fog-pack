use std::fmt;
use std::ops;
use std::cmp;
use std::time;

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

    pub fn from_sec(sec: i64) -> Timestamp {
        Timestamp { sec, nano: 0 }
    }

    /// Minimum possible time that can be represented
    pub fn min_value() -> Timestamp {
        Timestamp {
            sec: i64::min_value(),
            nano: 0,
        }
    }

    /// Maximum possible time that can be represented
    pub fn max_value() -> Timestamp {
        Timestamp {
            sec: i64::max_value(),
            nano: 1_999_999_999,
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

    /// Create a Timestamp based on the current system time. Can fail if the system clock is 
    /// extremely wrong - the time is before Unix Epoch, or nanosecond portion is greater than 2 
    /// seconds.
    pub fn now() -> Option<Timestamp> {
        match time::SystemTime::now().duration_since(time::SystemTime::UNIX_EPOCH) {
            Ok(t) => {
                Timestamp::from_raw(t.as_secs() as i64, t.subsec_nanos())
            },
            Err(_) => None
        }
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
