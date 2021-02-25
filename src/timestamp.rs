use std::cmp;
use std::fmt;
use std::ops;
use std::time;
use std::convert::TryFrom;

use byteorder::{LittleEndian, ReadBytesExt};

use fog_crypto::serde::{
    FOG_TYPE_ENUM,
    FOG_TYPE_ENUM_TIME_NAME,
    FOG_TYPE_ENUM_TIME_INDEX,
    CryptoEnum
};

use serde::{
    de::{Deserialize, Deserializer, EnumAccess, Error, Unexpected, VariantAccess},
    ser::{Serialize, Serializer, SerializeStructVariant},
};
use serde_bytes::{ByteBuf, Bytes};

const MAX_NANOSEC: u32 = 1_999_999_999;

/// Structure for holding a raw fog-pack timestamp.
/// This stores time in some consistent internal format, which may or may not be UTC. UTC time
/// can always be extracted from it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    sec: i64,
    nano: u32,
    standard: u8,
}

impl Timestamp {
    /// Create a timestamp from a raw seconds + nanoseconds value
    pub fn from_utc(sec: i64, nano: u32) -> Option<Timestamp> {
        if nano > MAX_NANOSEC {
            None
        } else {
            Some(Timestamp {
                sec,
                nano,
                standard: 0,
            })
        }
    }

    pub fn from_sec(sec: i64) -> Timestamp {
        Timestamp {
            sec,
            nano: 0,
            standard: 0,
        }
    }

    /// Minimum possible time that can be represented
    pub fn min_value() -> Timestamp {
        Timestamp {
            sec: i64::MIN,
            nano: 0,
            standard: 0,
        }
    }

    /// Maximum possible time that can be represented
    pub fn max_value() -> Timestamp {
        Timestamp {
            sec: i64::MAX,
            nano: MAX_NANOSEC,
            standard: 0,
        }
    }

    pub fn min(self, other: Timestamp) -> Timestamp {
        if self < other {
            self
        } else {
            other
        }
    }

    pub fn max(self, other: Timestamp) -> Timestamp {
        if self > other {
            self
        } else {
            other
        }
    }

    /// Add 1 nanosecond to timestamp. Will go into leap second (nanoseconds > 1e6) before it goes
    /// to the next second.
    pub fn next(mut self) -> Timestamp {
        if self.nano < MAX_NANOSEC {
            self.nano += 1;
        } else {
            self.nano = 0;
            self.sec += 1;
        }
        self
    }

    /// Subtract 1 nanosecond from timestamp. Will go into leap second (nanoseconds > 1e6) when it
    /// must decrement a second.
    pub fn prev(mut self) -> Timestamp {
        if self.nano > 0 {
            self.nano -= 1;
        } else {
            self.nano = MAX_NANOSEC;
            self.sec -= 1;
        }
        self
    }

    /// Return the UNIX timestamp (number of seconds since January 1, 1970
    /// 0:00:00 UTC). As a reminder, this is UTC time and thus includes leap seconds.
    pub fn timestamp_utc(&self) -> i64 {
        self.sec
    }

    /// Returns the number of nanoseconds past the second count.
    pub fn timestamp_subsec_nanos(&self) -> u32 {
        self.nano
    }

    /// Convert into a byte vector. For extending an existing byte vector, see
    /// [`encode_vec`](Self::encode_vec).
    pub fn as_vec(&self) -> Vec<u8> {
        let mut v = Vec::new();
        self.encode_vec(&mut v);
        v
    }

    /// Encode onto a byte vector one of 3 formats:
    /// 1. If nanoseconds is nonzero, encode the standard byte, the seconds as little-endian i64,
    ///    and the nanoseconds as little-endian u32.
    /// 2. If nanoseconds is zero & seconds maps to a u32, encode the standard byte, and the
    ///    seconds as little-endian u32.
    /// 3. If nanoseconds is zero & seconds does not map to a u32, encode the standard byte, and
    ///    the seconds as little-endian i64.
    pub fn encode_vec(&self, vec: &mut Vec<u8>) {
        if self.nano != 0 {
            vec.reserve(1 + 8 + 4);
            vec.push(self.standard);
            vec.extend_from_slice(&self.sec.to_le_bytes());
            vec.extend_from_slice(&self.nano.to_le_bytes());
        } else if (self.sec <= u32::MAX as i64) && (self.sec >= 0) {
            vec.reserve(1 + 4);
            vec.push(self.standard);
            vec.extend_from_slice(&(self.sec as u32).to_le_bytes());
        } else {
            vec.reserve(1 + 8);
            vec.push(self.standard);
            vec.extend_from_slice(&self.sec.to_le_bytes());
        }
    }

    pub fn size(&self) -> usize {
        if self.nano != 0 {
            1 + 8 + 4
        } else if (self.sec <= u32::MAX as i64) && (self.sec >= 0) {
            1 + 4
        } else {
            1 + 8
        }
    }

    /// Create a Timestamp based on the current system time. Can fail if the system clock is
    /// extremely wrong - the time is before Unix Epoch, or nanosecond portion is greater than 2
    /// seconds.
    pub fn now() -> Option<Timestamp> {
        match time::SystemTime::now().duration_since(time::SystemTime::UNIX_EPOCH) {
            Ok(t) => Timestamp::from_utc(t.as_secs() as i64, t.subsec_nanos()),
            Err(_) => None,
        }
    }
}

impl ops::Add<i64> for Timestamp {
    type Output = Timestamp;
    fn add(self, rhs: i64) -> Self {
        Timestamp {
            sec: self.sec + rhs,
            nano: self.nano,
            standard: self.standard,
        }
    }
}

impl ops::Sub<i64> for Timestamp {
    type Output = Timestamp;
    fn sub(self, rhs: i64) -> Self {
        Timestamp {
            sec: self.sec - rhs,
            nano: self.nano,
            standard: self.standard,
        }
    }
}

impl cmp::Ord for Timestamp {
    fn cmp(&self, other: &Timestamp) -> cmp::Ordering {
        if self.sec == other.sec {
            self.nano.cmp(&other.nano)
        } else {
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
        write!(f, "UTC: {} sec + {} ns", self.sec, self.nano)
    }
}

impl TryFrom<&[u8]> for Timestamp {
    type Error = String;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut raw = value;
        let standard = raw
            .read_u8()
            .map_err(|_| String::from("missing time standard byte"))?;
        let (sec, nano) = match value.len() {
            13 => {
                let sec = raw.read_i64::<LittleEndian>().unwrap();
                let nano = raw.read_u32::<LittleEndian>().unwrap();
                (sec, nano)
            }
            9 => {
                let sec = raw.read_i64::<LittleEndian>().unwrap();
                (sec, 0)
            }
            5 => {
                let sec = raw.read_u32::<LittleEndian>().unwrap() as i64;
                (sec, 0)
            }
            _ => {
                return Err(format!(
                    "not a recognized Timestamp length ({} bytes)",
                    value.len()
                ))
            }
        };
        Ok(Timestamp {
            sec,
            nano,
            standard,
        })
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // Human-readable format instead of compacted byte sequence
            let mut sv = serializer.serialize_struct_variant(
                FOG_TYPE_ENUM,
                FOG_TYPE_ENUM_TIME_INDEX as u32,
                FOG_TYPE_ENUM_TIME_NAME,
                2
            )?;
            // Always serialize all fields, in case the field names are omitted and this is turned 
            // into just an array
            sv.serialize_field("std", &self.standard)?;
            sv.serialize_field("secs", &self.sec)?;
            sv.serialize_field("nanos", &self.sec)?;
            sv.end()
        } else {
            // Use compacted byte sequence if not human-readable
            let value = ByteBuf::from(self.as_vec());
            serializer.serialize_newtype_variant(
                FOG_TYPE_ENUM,
                FOG_TYPE_ENUM_TIME_INDEX as u32,
                FOG_TYPE_ENUM_TIME_NAME,
                &value,
            )
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimeVisitor {
            is_human_readable: bool,
        }

        impl<'de> serde::de::Visitor<'de> for TimeVisitor {
            type Value = Timestamp;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                write!(
                    fmt,
                    "{} enum with variant {} (id {})",
                    FOG_TYPE_ENUM, FOG_TYPE_ENUM_TIME_NAME, FOG_TYPE_ENUM_TIME_INDEX
                )
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                let variant = match data.variant()? {
                    (CryptoEnum::Time, variant) => variant,
                    (e, _) => {
                        return Err(A::Error::invalid_type(
                            Unexpected::Other(e.as_str()),
                            &"Time",
                        ))
                    }
                };
                if self.is_human_readable {
                    use serde::de::MapAccess;
                    struct TimeStructVisitor;
                    impl<'de> serde::de::Visitor<'de> for TimeStructVisitor {
                        type Value = Timestamp;
                        fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                            write!(fmt, "timestamp struct")
                        }

                        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                            where A: MapAccess<'de>
                        {
                            let mut secs: Option<i64> = None;
                            let mut nanos: u32 = 0;
                            while let Some(key) = map.next_key::<String>()? {
                                match key.as_ref() {
                                    "std" => {
                                        let v: u8 = map.next_value()?;
                                        if v != 0 {
                                            return Err(A::Error::invalid_value(Unexpected::Unsigned(v as u64), &"0"));
                                        }
                                    },
                                    "secs" => {
                                        secs = Some(map.next_value()?);
                                    },
                                    "nanos" => {
                                        nanos = map.next_value()?;
                                    },
                                    _ => return Err(A::Error::unknown_field(
                                            key.as_ref(),
                                            &["std", "secs", "nanos"]
                                    )),
                                }
                            }
                            let secs = secs.ok_or(A::Error::missing_field("secs"))?;
                            Timestamp::from_utc(secs, nanos).ok_or(A::Error::custom("Invalid timestamp"))
                        }
                    }
                    variant.struct_variant(&["std", "secs", "nanos"], TimeStructVisitor)
                } else {
                    let bytes: &Bytes = variant.newtype_variant()?;
                    Timestamp::try_from(bytes.as_ref()).map_err(|e| A::Error::custom(e))
                }
            }
        }

        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_enum(
            FOG_TYPE_ENUM,
            &[FOG_TYPE_ENUM_TIME_NAME],
            TimeVisitor { is_human_readable },
        )
    }
}


#[cfg(test)]
mod test {
    use super::*;

    fn edge_cases() -> Vec<(usize, Timestamp)> {
        let mut test_cases = Vec::new();
        test_cases.push((5, Timestamp::from_utc(0, 0).unwrap()));
        test_cases.push((5, Timestamp::from_utc(1, 0).unwrap()));
        test_cases.push((13, Timestamp::from_utc(1, 1).unwrap()));
        test_cases.push((5, Timestamp::from_utc(u32::MAX as i64 - 1, 0).unwrap()));
        test_cases.push((5, Timestamp::from_utc(u32::MAX as i64 - 0, 0).unwrap()));
        test_cases.push((9, Timestamp::from_utc(u32::MAX as i64 + 1, 0).unwrap()));
        test_cases.push((9, Timestamp::from_utc(i64::MIN, 0).unwrap()));
        test_cases.push((13, Timestamp::from_utc(i64::MIN, 1).unwrap()));
        test_cases
    }

    #[test]
    fn roundtrip() {
        for (index, case) in edge_cases().iter().enumerate() {
            println!(
                "Test #{}: '{}' with expected length = {}",
                index, case.1, case.0
            );
            let mut enc = Vec::new();
            case.1.encode_vec(&mut enc);
            assert_eq!(enc.len(), case.0);
            let decoded = Timestamp::try_from(enc.as_ref()).unwrap();
            assert_eq!(decoded, case.1);
        }
    }

    #[test]
    fn too_long() {
        for case in edge_cases() {
            println!("Test with Timestamp = {}", case.1);
            let mut enc = Vec::new();
            case.1.encode_vec(&mut enc);
            enc.push(0u8);
            assert!(Timestamp::try_from(enc.as_ref()).is_err());
        }
    }

    #[test]
    fn too_short() {
        for case in edge_cases() {
            println!("Test with Timestamp = {}", case.1);
            let mut enc = Vec::new();
            case.1.encode_vec(&mut enc);
            enc.pop();
            assert!(Timestamp::try_from(enc.as_ref()).is_err());
        }
    }

}
