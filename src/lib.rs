pub mod element;
mod marker;

pub mod compress;
mod depth_tracking;
mod schema;
pub mod validator;
pub mod value;
pub mod value_ref;
mod document;
mod entry;
mod timestamp;
mod integer;

pub use schema::*;
pub use entry::*;
pub use document::*;
pub use timestamp::*;
pub use integer::*;

mod error;

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
/// The exclusive maximum allowed size of a raw document, including signatures, is 1 MiB. No
/// encoded document will ever be equal to or larger than this size.
pub const MAX_DOC_SIZE: usize = (1usize << 20) - 1; // 1 MiB
/// The exclusive maximum allowed size of a raw entry, including signatures, is 64 kiB. No encoded
/// entry will ever be equal to or larger than this size. This does not include the size of the
/// parent hash or the field for the entry.
pub const MAX_ENTRY_SIZE: usize = (1usize << 16) - 1; // 64 kiB
