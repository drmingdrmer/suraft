use crate::storage::log::log_id::LogId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IOState {
    pub(crate) submitted: Option<LogId>,
    pub(crate) flushed: Option<LogId>,
}

impl IOState {
    pub fn submit(&mut self, log_id: LogId) {
        debug_assert!(Some(&log_id) >= self.submitted.as_ref());
        self.submitted = Some(log_id);
    }

    pub fn flush(&mut self, log_id: LogId) {
        debug_assert!(Some(&log_id) >= self.flushed.as_ref());
        self.flushed = Some(log_id);
    }

    pub fn submitted(&self) -> Option<&LogId> {
        self.submitted.as_ref()
    }

    pub fn flushed(&self) -> Option<&LogId> {
        self.flushed.as_ref()
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.submitted == self.flushed
    }
}
