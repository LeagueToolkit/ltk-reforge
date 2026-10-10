//! The app's store (#ADR-0096): the runs it keeps, their snapshots and the bytes they name.

mod tier;

pub use self::tier::{KEYSPACE, RunTier};
