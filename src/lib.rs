pub mod element;
mod marker;

pub mod compress;
mod depth_tracking;
mod document;
mod entry;
mod integer;
mod query;
mod schema;
mod timestamp;
pub mod validator;
mod value;
mod value_ref;

pub use document::*;
pub use entry::*;
pub use integer::*;
pub use query::*;
pub use schema::*;
pub use timestamp::*;
pub use value::Value;
pub use value_ref::ValueRef;

pub mod error;

pub use fog_crypto::{
    hash::Hash,
    identity::Identity,
    lock::LockId,
    lockbox::{
        DataLockbox, DataLockboxRef, IdentityLockbox, IdentityLockboxRef, LockLockbox,
        LockLockboxRef, StreamLockbox, StreamLockboxRef,
    },
    stream::StreamId,
};

mod de;
mod ser;

/// The maximum nesting depth allowed for any fog-pack value. No encoded document will ever nest
/// Map/Array markers deeper than this.
pub const MAX_DEPTH: usize = 100;
/// The maximum allowed size of a raw document, including signatures, is 1 MiB. No encoded document
/// will ever be larger than this size.
pub const MAX_DOC_SIZE: usize = (1usize << 20) - 1; // 1 MiB
/// The maximum allowed size of a raw entry, including signatures, is 64 kiB. No encoded entry will
/// ever be larger than this size.
pub const MAX_ENTRY_SIZE: usize = (1usize << 16) - 1; // 64 kiB
/// The maximum allowed size of a raw query, is 64 kiB. No encoded query will ever be larger than
/// this size.
pub const MAX_QUERY_SIZE: usize = (1usize << 16) - 1; // 64 kiB
