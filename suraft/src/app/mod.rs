use std::fmt;

use crate::OptionalSend;
use crate::OptionalSync;
use crate::Serde;

/// A trait defining application specific data.
///
/// The intention of this trait is that applications which are using this crate
/// will be able to use their own concrete data types throughout their
/// application without having to serialize and deserialize their data as it
/// goes through SuRaft. Instead, applications can present their data models
/// as-is to SuRaft, SuRaft will present it to the application's `LogStorage`
/// and `RaftStateMachine` impl when ready, and the application may then deal
/// with the data directly in the storage engine without having to do a
/// preliminary deserialization.
///
/// ## Note
///
/// The trait is automatically implemented for all types which satisfy its
/// super traits.
pub trait AppData:
    fmt::Debug + OptionalSend + OptionalSync + 'static + Serde
{
}

impl<T> AppData for T where T: fmt::Debug + OptionalSend + OptionalSync + 'static + Serde
{}
