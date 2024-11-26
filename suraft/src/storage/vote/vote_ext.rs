use crate::storage::vote::Vote;

pub trait VoteExt {
    fn next_term(&self) -> u64;

    fn is_committed(&self) -> bool;
}

impl VoteExt for Option<Vote> {
    fn next_term(&self) -> u64 {
        self.as_ref().map_or(0, |v| v.term + 1)
    }

    fn is_committed(&self) -> bool {
        self.as_ref().map_or(false, |v| v.committed)
    }
}

impl VoteExt for Option<&Vote> {
    fn next_term(&self) -> u64 {
        self.as_ref().map_or(0, |v| v.term + 1)
    }

    fn is_committed(&self) -> bool {
        self.as_ref().map_or(false, |v| v.committed)
    }
}
