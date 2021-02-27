mod bool;
mod float32;
mod float64;
mod integer;
mod str;

pub use self::bool::*;
pub use self::float32::*;
pub use self::float64::*;
pub use self::integer::*;
pub use self::str::*;
use std::collections::BTreeMap;

pub enum Validator {
    Null,
    Bool(BoolValidator),
    Int(IntValidator),
    F32(F32Validator),
    F64(F64Validator),
    Bin,
    Str(StrValidator),
    Time,
    Hash,
    Identity,
    StreamId,
    LockId,
    DataLockbox,
    IdentityLockbox,
    StreamLockbox,
    LockLockbox,
    Ref(String),
    Multi(Vec<Validator>),
    Enum(BTreeMap<String, Option<Validator>>),
    Any,
}
