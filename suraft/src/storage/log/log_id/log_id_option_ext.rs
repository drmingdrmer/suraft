use crate::storage::log::log_id::LogId;

/// This helper trait extracts information from an `Option<LogId>`.
pub trait LogIdOptionExt {
    /// Returns the log index if it is not a `None`.
    fn index(&self) -> Option<u64>;

    /// Returns the next log index.
    ///
    /// If self is `None`, it returns 0.
    fn next_index(&self) -> u64;

    fn next_term(&self) -> u64;
}

impl LogIdOptionExt for LogId {
    fn index(&self) -> Option<u64> {
        Some(self.index)
    }

    fn next_index(&self) -> u64 {
        self.index + 1
    }

    fn next_term(&self) -> u64 {
        self.term + 1
    }
}

impl<T> LogIdOptionExt for &T
where T: LogIdOptionExt
{
    fn index(&self) -> Option<u64> {
        LogIdOptionExt::index(*self)
    }

    fn next_index(&self) -> u64 {
        LogIdOptionExt::next_index(*self)
    }

    fn next_term(&self) -> u64 {
        LogIdOptionExt::next_term(*self)
    }
}

impl<T> LogIdOptionExt for Option<T>
where T: LogIdOptionExt
{
    fn index(&self) -> Option<u64> {
        self.as_ref().and_then(|x| x.index())
    }

    fn next_index(&self) -> u64 {
        match self {
            None => 0,
            Some(log_id) => log_id.next_index(),
        }
    }

    fn next_term(&self) -> u64 {
        match self {
            None => 0,
            Some(log_id) => log_id.next_term(),
        }
    }
}
