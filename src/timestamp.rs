use std::cmp;
use std::convert::TryFrom;
use std::fmt;
use std::ops;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::time::SystemTime;

use byteorder::{LittleEndian, ReadBytesExt};

use fog_crypto::serde::{
    CryptoEnum, FOG_TYPE_ENUM, FOG_TYPE_ENUM_TIME_INDEX, FOG_TYPE_ENUM_TIME_NAME,
};

use serde::Deserialize;
use serde::Serialize;
use serde::{
    de::{Deserializer, EnumAccess, Error, Unexpected, VariantAccess},
    ser::{SerializeStructVariant, Serializer},
};
use serde_bytes::ByteBuf;

const NTP_EPOCH_OFFSET: i64 = -86400 * (70*365 + 17);
const MAX_NANOSEC: u32 = 999_999_999;
const NANOS_PER_SEC: i64 = 1_000_000_000;
const MICROS_PER_SEC: i64 = 1_000_000;
const MILLIS_PER_SEC: i64 = 1_000;
static UTC_LEAP: OnceLock<RwLock<LeapSeconds>> = OnceLock::new();

fn get_table() -> std::sync::RwLockReadGuard<'static, LeapSeconds> {
    let table = UTC_LEAP.get_or_init(|| RwLock::new(LeapSeconds::default()));
    match table.read() {
        Ok(o) => o,
        Err(e) => e.into_inner(),
    }
}

/// Correct a UTC timestamp to a proper TAI timestamp
fn utc_to_tai(t: Timestamp) -> Timestamp {
    let table = get_table();
    t - table.reverse_leap_seconds(t)
}

/// Convert a TAI timestamp into a UTC timestamp
fn tai_to_utc(t: Timestamp) -> Timestamp {
    let table = get_table();
    t + table.leap_seconds(t)
}

/// Set up the leap second table to go from TAI to UTC times.
pub fn set_utc_leap_seconds(table: LeapSeconds) {
    let store = UTC_LEAP.get_or_init(|| RwLock::new(LeapSeconds::default()));
    let mut store = match store.write() {
        Ok(o) => o,
        Err(e) => e.into_inner(),
    };
    *store = table;
}

/// A difference between [`Timestamp`] values. Can be used to adjust timestamps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TimeDifference {
    secs: i64,
    nanos: u32,
}

impl TimeDifference {
    /// Construct a new time difference. Fails if nanoseconds is one billion or
    /// more.
    pub fn new(secs: i64, nanos: u32) -> Option<Self> {
        if nanos > MAX_NANOSEC {
            return None;
        }
        Some(Self { secs, nanos })
    }

    /// Construct a `TimeDifference` from the specified number of seconds.
    pub fn from_secs(secs: i64) -> Self {
        Self { secs, nanos: 0 }
    }

    /// Construct a `TimeDifference` from the specified number of milliseconds.
    pub fn from_millis(millis: i64) -> Self {
        Self {
            secs: millis / MILLIS_PER_SEC,
            nanos: (millis % MILLIS_PER_SEC) as u32,
        }
    }

    /// Construct a `TimeDifference` from the specified number of microseconds.
    pub fn from_micros(micros: i64) -> Self {
        Self {
            secs: micros / MICROS_PER_SEC,
            nanos: (micros % MICROS_PER_SEC) as u32,
        }
    }

    /// Construct a `TimeDifference` from the specified number of nanoseconds.
    pub fn from_nanos(nanos: i64) -> Self {
        Self {
            secs: nanos / NANOS_PER_SEC,
            nanos: (nanos % NANOS_PER_SEC) as u32,
        }
    }

    /// Returns the fractional part of this `TimeDifference`, in nanoseconds.
    pub fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// Returns the total number of whole seconds contained by this `TimeDifference`.
    ///
    /// The returned value doesn't contain the fractional part of the difference.
    pub fn as_secs(&self) -> i64 {
        self.secs
    }
}

/// A fully decoded leap second table, suitable for converting between time standards.
///
/// This table should contain strictly increasing timestamps and the associated
/// time differences to apply from that timestamp onward. It can be used to get
/// the difference to apply, and can take an invalid timestamp and correct it.
///
/// All timestamps should use TAI time starting from the 1970 UNIX epoch, and
/// the timestamps in this table should be no different. That means this does
/// *not* match the NTP leap second table; use
/// [`from_ntp_file`][LeapSeconds::from_ntp_file] to parse one appropriately.
///
/// This struct doesn't implement `Serialize` or `Deserialize` for a reason;
/// it's very easy to construct invalid tables. Consider making a format that
/// delta-encodes timestamps and differences instead, which is more amenable to
/// schema-based validation.
#[derive(Clone, Debug)]
pub struct LeapSeconds(pub Vec<(Timestamp, TimeDifference)>);

impl Default for LeapSeconds {
    fn default() -> Self {
        let file = include_str!("leap-seconds.list");
        Self::from_ntp_file(file).unwrap()
    }
}

impl LeapSeconds {
    /// Construct a new leap second table. Assumes the `Timestamp` values are
    /// strictly increasing, that the `TimeDifference` values don't change by
    /// more than one second, and that the timestamps are spaced more than a
    /// few seconds apart.
    pub fn new(table: Vec<(Timestamp, TimeDifference)>) -> Self {
        Self(table)
    }

    /// Look up the amount of time to subtract from a timestamp that has leap
    /// seconds in it. Used for converting from UTC to TAI.
    pub fn reverse_leap_seconds(&self, t: Timestamp) -> TimeDifference {
        for leap_second in self.0.iter().rev() {
            if (t - leap_second.1) >= leap_second.0 {
                return leap_second.1
            }
        }
        TimeDifference::default()
    }

    /// Look up the amount of time to add to a timestamp to compensate for leap
    /// seconds, according to this table. Used for converting from TAI to UTC.
    pub fn leap_seconds(&self, t: Timestamp) -> TimeDifference {
        for leap_second in self.0.iter().rev() {
            if t >= leap_second.0 {
                return leap_second.1;
            }
        }
        TimeDifference::default()
    }

    /// Parse a NTP leap seconds file that has been read in as a string.
    ///
    /// The latest leap seconds file can be fetched from
    /// <https://hpiers.obspm.fr/iers/bul/bulc/ntp/leap-seconds.list>. This is
    /// the file published by IERS, the official source of leap second
    /// publications.
    ///
    /// Historical leap seconds files are available at
    /// <https://hpiers.obspm.fr/iers/bul/bulc/ntp/>.
    ///
    /// The default leap seconds list is loaded from a compiled-in version of
    /// this list, which will be updated whenever a new list is published
    /// (bumping the patch version of this crate).
    pub fn from_ntp_file(file: &str) -> Option<Self> {
        let mut table = Vec::new();
        for line in file.lines() {
            if let Some(first_char) = line.chars().next() {
                if first_char == '#' {
                    continue;
                } else {
                    let mut data = line.split_whitespace();

                    // Get the UTC time since 1900-01-01 00:00:00
                    let Some(secs_utc) = data.next() else { return None };
                    let Ok(secs_utc) = str::parse::<i64>(secs_utc) else { return None };

                    // Get the delta to apply to the timestamp from this point onward
                    let Some(delta) = data.next() else { return None };
                    let Ok(delta) = str::parse::<i64>(delta) else { return None };

                    // Create a proper TAI timestamp and put in the correct time delta to apply
                    let time = Timestamp::from_tai_secs(secs_utc + delta + NTP_EPOCH_OFFSET);
                    let delta = TimeDifference::from_secs(-delta);
                    table.push((time, delta));
                }
            }
        }
        Some(LeapSeconds(table))
    }
}

/// Structure for holding a raw fog-pack timestamp.
/// This stores a TAI timestamp relative to the Unix epoch of 1970-01-01T00:00:00Z.
/// This is what a correctly configured Linux TIME_TAI clock would return. It
/// also matches the IEEE 1588 Precision Time Protocol standard epoch and timescale.
///
/// This is *not* UTC time, Unix Time, or POSIX Time.
///
/// Functions for converting from and to UTC are available as
/// [`from_utc`][Self::from_utc] and [`utc`][Self::utc]. The conversion is
/// handled using a built-in table of leap seconds. This table can be updated by
/// calling [`set_utc_leap_seconds`] with a new table. See [`LeapSeconds`] for
/// how to create a new table.
///
/// While these functions do their best to provide correct round-trip conversion
/// for TAI->UTC->TAI and UTC->TAI->UTC, the handling around the exact leap
/// second point can create a delta, due to UTC Unix time reusing a seconds
/// value during the leap second. Using TAI directly if possible is thus
/// preferred, as is sticking to Timestamps as much as possible and only
/// converting back to UTC when you need to display the timestamp for people.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Timestamp {
    secs: i64,
    nanos: u32,
}

impl Timestamp {
    /// Create a TAI timestamp from a raw seconds + nanoseconds value. This
    /// should be the number of seconds since the Unix epoch of
    /// 1970-01-01T00:00:00Z, without any leap seconds thrown about.
    pub fn from_tai(secs: i64, nanos: u32) -> Option<Timestamp> {
        if nanos > MAX_NANOSEC {
            return None;
        }
        Some(Timestamp { secs, nanos })
    }

    /// Create a timestamp from a raw UTC seconds + nanosecond value. This
    /// should be the number of seconds since the Unix epoch of
    /// 1970-01-01T00:00:00Z, with the usual UTC leap seconds thrown in. This is
    /// commonly referred to as Unix time, and is the default timebase for many
    /// computer systems.
    ///
    /// The UTC-to-TAI conversion is handled using a built-in table of leap
    /// seconds. This table can be updated by calling [`set_utc_leap_seconds`]
    /// with the table.
    pub fn from_utc(secs: i64, nanos: u32) -> Option<Timestamp> {
        if nanos > MAX_NANOSEC {
            return None;
        }
        Some(utc_to_tai(Timestamp { secs, nanos }))
    }

    /// Create a timestamp from a raw UTC seconds value. See
    /// [`from_utc`][Self::from_utc] for details.
    pub fn from_utc_secs(secs: i64) -> Timestamp {
        utc_to_tai(Timestamp { secs, nanos: 0 })
    }

    /// Create a timestamp from a raw TAI seconds value. See
    /// [`from_tai`][Self::from_tai] for details.
    pub fn from_tai_secs(secs: i64) -> Timestamp {
        Timestamp { secs, nanos: 0 }
    }

    /// Zero time - TAI Unix epoch time
    pub const fn zero() -> Timestamp {
        Timestamp { secs: 0, nanos: 0 }
    }

    /// Minimum possible time that can be represented
    pub const fn min_value() -> Timestamp {
        Timestamp {
            secs: i64::MIN,
            nanos: 0,
        }
    }

    /// Maximum possible time that can be represented
    pub const fn max_value() -> Timestamp {
        Timestamp {
            secs: i64::MAX,
            nanos: MAX_NANOSEC,
        }
    }

    /// Find the earlier of two timestamps and return it.
    pub fn min(self, other: Timestamp) -> Timestamp {
        if self < other {
            self
        } else {
            other
        }
    }

    /// Find the later of two timestamps and return it.
    pub fn max(self, other: Timestamp) -> Timestamp {
        if self > other {
            self
        } else {
            other
        }
    }

    /// Add 1 nanosecond to timestamp.
    pub fn next(mut self) -> Timestamp {
        if self.nanos < MAX_NANOSEC {
            self.nanos += 1;
        } else {
            self.nanos = 0;
            self.secs += 1;
        }
        self
    }

    /// Subtract 1 nanosecond from timestamp.
    pub fn prev(mut self) -> Timestamp {
        if self.nanos > 0 {
            self.nanos -= 1;
        } else {
            self.nanos = MAX_NANOSEC;
            self.secs -= 1;
        }
        self
    }

    /// Return the UNIX timestamp (number of seconds since January 1, 1970
    /// 0:00:00 UTC). As a reminder, this is UTC time and thus has leap seconds
    /// removed/added.
    ///
    /// The TAI-to-UTC conversion is handled using a built-in table of leap
    /// seconds. This table can be updated by calling [`set_utc_leap_seconds`]
    /// with the table.
    pub fn utc(&self) -> (i64, u32) {
        let t = tai_to_utc(*self);
        (t.secs, t.nanos)
    }

    /// Return the TAI number of seconds since January 1, 1970 00:00:00 UTC.
    pub fn tai_secs(&self) -> i64 {
        self.secs
    }

    /// Returns the number of nanoseconds past the second count.
    pub fn tai_subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// Calculates the time that has elapsed between the other timestamp and
    /// this one. Effectively `self - other`.
    pub fn time_since(&self, other: &Timestamp) -> TimeDifference {
        let rhs = TimeDifference {
            secs: other.secs,
            nanos: other.nanos,
        };
        let new = *self - rhs;
        TimeDifference {
            secs: new.secs,
            nanos: new.nanos,
        }
    }

    /// Convert into a byte vector. For extending an existing byte vector, see
    /// [`encode_vec`](Self::encode_vec).
    pub fn as_vec(&self) -> Vec<u8> {
        let mut v = Vec::new();
        self.encode_vec(&mut v);
        v
    }

    /// Encode onto a byte vector one of 3 formats:
    /// 1. If nanoseconds is nonzero, encode the seconds as little-endian i64,
    ///    and the nanoseconds as little-endian u32.
    /// 2. If nanoseconds is zero & seconds maps to a u32, encode just the
    ///    seconds as little-endian u32.
    /// 3. If nanoseconds is zero & seconds does not map to a u32, encode the
    ///    seconds as little-endian i64.
    pub fn encode_vec(&self, vec: &mut Vec<u8>) {
        if self.nanos != 0 {
            vec.reserve(8 + 4);
            vec.extend_from_slice(&self.secs.to_le_bytes());
            vec.extend_from_slice(&self.nanos.to_le_bytes());
        } else if (self.secs <= u32::MAX as i64) && (self.secs >= 0) {
            vec.reserve(4);
            vec.extend_from_slice(&(self.secs as u32).to_le_bytes());
        } else {
            vec.reserve(8);
            vec.extend_from_slice(&self.secs.to_le_bytes());
        }
    }

    /// Return the number of bytes needed to encode the timestamp as a byte
    /// vector with [`encode_vec`][Self::encode_vec].
    pub fn size(&self) -> usize {
        if self.nanos != 0 {
            8 + 4
        } else if (self.secs <= u32::MAX as i64) && (self.secs >= 0) {
            4
        } else {
            8
        }
    }

    /// Create a Timestamp based on the current system time.
    pub fn now() -> Timestamp {
        Timestamp::from(SystemTime::now())
    }
}

impl From<SystemTime> for Timestamp {
    fn from(value: SystemTime) -> Self {
        let t = value.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        Timestamp::from_utc(t.as_secs() as i64, t.subsec_nanos()).unwrap()
    }
}

impl ops::Add<i64> for Timestamp {
    type Output = Timestamp;
    fn add(mut self, rhs: i64) -> Self {
        self.secs += rhs;
        self
    }
}

impl ops::AddAssign<i64> for Timestamp {
    fn add_assign(&mut self, rhs: i64) {
        self.secs += rhs;
    }
}

impl ops::Sub<i64> for Timestamp {
    type Output = Timestamp;
    fn sub(mut self, rhs: i64) -> Self {
        self.secs -= rhs;
        self
    }
}

impl ops::SubAssign<i64> for Timestamp {
    fn sub_assign(&mut self, rhs: i64) {
        self.secs -= rhs;
    }
}

impl ops::Add<TimeDifference> for Timestamp {
    type Output = Timestamp;
    fn add(mut self, rhs: TimeDifference) -> Timestamp {
        self += rhs;
        self
    }
}

impl ops::AddAssign<TimeDifference> for Timestamp {
    fn add_assign(&mut self, rhs: TimeDifference) {
        self.nanos += rhs.nanos;
        if self.nanos > MAX_NANOSEC {
            self.nanos -= MAX_NANOSEC + 1;
            self.secs += 1;
        }
        self.secs += rhs.secs;
    }
}

impl ops::Sub<TimeDifference> for Timestamp {
    type Output = Timestamp;
    fn sub(mut self, rhs: TimeDifference) -> Timestamp {
        self -= rhs;
        self
    }
}

impl ops::SubAssign<TimeDifference> for Timestamp {
    fn sub_assign(&mut self, rhs: TimeDifference) {
        if self.nanos < rhs.nanos {
            self.nanos += MAX_NANOSEC + 1;
            self.secs -= 1;
        }
        self.nanos -= rhs.nanos;
        self.secs -= rhs.secs;
    }
}

impl cmp::Ord for Timestamp {
    fn cmp(&self, other: &Timestamp) -> cmp::Ordering {
        match self.secs.cmp(&other.secs) {
            cmp::Ordering::Equal => self.nanos.cmp(&other.nanos),
            other => other,
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
        write!(f, "TAI: {} secs + {} ns", self.secs, self.nanos)
    }
}

impl TryFrom<&[u8]> for Timestamp {
    type Error = String;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut raw = value;
        let (secs, nanos) = match value.len() {
            12 => {
                let secs = raw.read_i64::<LittleEndian>().unwrap();
                let nanos = raw.read_u32::<LittleEndian>().unwrap();
                (secs, nanos)
            }
            8 => {
                let secs = raw.read_i64::<LittleEndian>().unwrap();
                (secs, 0)
            }
            4 => {
                let secs = raw.read_u32::<LittleEndian>().unwrap() as i64;
                (secs, 0)
            }
            _ => {
                return Err(format!(
                    "not a recognized Timestamp length ({} bytes)",
                    value.len()
                ))
            }
        };
        Ok(Timestamp { secs, nanos })
    }
}

impl serde::ser::Serialize for Timestamp {
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
                2,
            )?;
            // Always serialize all fields, in case the field names are omitted and this is turned
            // into just an array
            sv.serialize_field("secs", &self.secs)?;
            sv.serialize_field("nanos", &self.nanos)?;
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

impl<'de> serde::de::Deserialize<'de> for Timestamp {
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
                        fn expecting(
                            &self,
                            fmt: &mut fmt::Formatter<'_>,
                        ) -> Result<(), fmt::Error> {
                            write!(fmt, "timestamp struct")
                        }

                        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                        where
                            A: MapAccess<'de>,
                        {
                            let mut secs: Option<i64> = None;
                            let mut nanos: u32 = 0;
                            while let Some(key) = map.next_key::<String>()? {
                                match key.as_ref() {
                                    "std" => {
                                        let v: u8 = map.next_value()?;
                                        if v != 0 {
                                            return Err(A::Error::invalid_value(
                                                Unexpected::Unsigned(v as u64),
                                                &"0",
                                            ));
                                        }
                                    }
                                    "secs" => {
                                        secs = Some(map.next_value()?);
                                    }
                                    "nanos" => {
                                        nanos = map.next_value()?;
                                    }
                                    _ => {
                                        return Err(A::Error::unknown_field(
                                            key.as_ref(),
                                            &["std", "secs", "nanos"],
                                        ))
                                    }
                                }
                            }
                            let secs = secs.ok_or_else(|| A::Error::missing_field("secs"))?;
                            Timestamp::from_tai(secs, nanos)
                                .ok_or_else(|| A::Error::custom("Invalid timestamp"))
                        }
                    }
                    variant.struct_variant(&["std", "secs", "nanos"], TimeStructVisitor)
                } else {
                    let bytes: ByteBuf = variant.newtype_variant()?;
                    Timestamp::try_from(bytes.as_ref()).map_err(A::Error::custom)
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
        vec![
            (5, Timestamp::from_tai(0, 0).unwrap()),
            (5, Timestamp::from_tai(1, 0).unwrap()),
            (13, Timestamp::from_tai(1, 1).unwrap()),
            (5, Timestamp::from_tai(u32::MAX as i64 - 1, 0).unwrap()),
            (5, Timestamp::from_tai(u32::MAX as i64, 0).unwrap()),
            (9, Timestamp::from_tai(u32::MAX as i64 + 1, 0).unwrap()),
            (9, Timestamp::from_tai(i64::MIN, 0).unwrap()),
            (13, Timestamp::from_tai(i64::MIN, 1).unwrap()),
        ]
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
