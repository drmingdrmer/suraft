use std::fmt;
use std::fmt::Debug;

use crate::base::display_ext::DisplaySliceExt;
use crate::storage::log::log_id::LogId;
use crate::TypeConfig;

pub type EntryPayload<C> = Vec<<C as TypeConfig>::AppData>;

/// A SuRaft log entry.
#[derive(Default)]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(bound = "")]
pub struct Entry<C>
where C: TypeConfig
{
    pub log_id: LogId,

    /// This entry's payload.
    pub payload: EntryPayload<C>,
}

impl<C> Entry<C>
where C: TypeConfig
{
    pub fn new_blank(log_id: LogId) -> Self {
        Self {
            log_id,
            payload: Vec::new(),
        }
    }

    pub fn get_log_id(&self) -> &LogId {
        &self.log_id
    }

    pub fn set_log_id(&mut self, log_id: &LogId) {
        self.log_id = log_id.clone();
    }
}

impl<C> Clone for Entry<C>
where
    C: TypeConfig,
    C::AppData: Clone,
{
    fn clone(&self) -> Self {
        Self {
            log_id: self.log_id.clone(),
            payload: self.payload.clone(),
        }
    }
}

impl<C> Debug for Entry<C>
where C: TypeConfig
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("log_id", &self.log_id)
            .field("payload", &self.payload)
            .finish()
    }
}

impl<C> PartialEq for Entry<C>
where
    C::AppData: PartialEq,
    C: TypeConfig,
{
    fn eq(&self, other: &Self) -> bool {
        self.log_id == other.log_id && self.payload == other.payload
    }
}

impl<C> AsRef<Entry<C>> for Entry<C>
where C: TypeConfig
{
    fn as_ref(&self) -> &Entry<C> {
        self
    }
}

impl<C> fmt::Display for Entry<C>
where
    C: TypeConfig,
    C::AppData: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.log_id, self.payload.display())
    }
}
