mod bool;
mod float32;
mod integer;

pub use self::bool::*;
pub use self::float32::*;
pub use self::integer::*;
use std::collections::BTreeMap;

pub enum Validator {
    Null,
    Bool(BoolValidator),
    Int(IntValidator),
    F32(F32Validator),
    F64,
    Bin,
    Str,
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
