
mod bool;
mod integer;
mod float32;

use std::collections::BTreeMap;
pub use self::bool::*;
pub use self::integer::*;
pub use self::float32::*;

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

