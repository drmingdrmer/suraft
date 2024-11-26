use std::cmp::Ordering;
use std::fmt::Formatter;

use crate::storage::membership::NodeId;

pub mod vote_ext;

/// `Vote` represent the privilege of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Vote {
    pub term: u64,
    pub committed: bool,
    pub voted_for: NodeId,
}

impl PartialOrd for Vote {
    #[inline]
    fn partial_cmp(&self, other: &Vote) -> Option<Ordering> {
        match PartialOrd::partial_cmp(&self.term, &other.term) {
            Some(Ordering::Equal) => {}
            cmp => return cmp,
        };

        match PartialOrd::partial_cmp(&self.committed, &other.committed) {
            Some(Ordering::Equal) => {}
            cmp => return cmp,
        };

        if self.voted_for == other.voted_for {
            Some(Ordering::Equal)
        } else {
            None
        }
    }
}

impl std::fmt::Display for Vote {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<T{}-N{}:{}>",
            self.term,
            self.voted_for,
            if self.is_committed() { "Q" } else { "-" }
        )
    }
}

impl Vote {
    pub fn new(term: u64, node_id: NodeId) -> Self {
        Self {
            term,
            committed: false,
            voted_for: node_id,
        }
    }

    pub fn new_committed(term: u64, node_id: NodeId) -> Self {
        Self {
            term,
            committed: true,
            voted_for: node_id,
        }
    }

    pub(crate) fn commit(mut self) -> Self {
        self.committed = true;
        self
    }

    pub fn is_committed(&self) -> bool {
        self.committed
    }

    pub fn term(&self) -> u64 {
        self.term
    }

    pub fn voted_for(&self) -> &NodeId {
        &self.voted_for
    }
}

#[cfg(test)]
#[allow(clippy::nonminimal_bool)]
mod tests {

    use std::panic::UnwindSafe;

    use crate::storage::vote::Vote;
    use crate::testing::nid;

    #[test]
    fn test_vote_serde() -> anyhow::Result<()> {
        let v = Vote::new(1, nid(2));
        let s = serde_json::to_string(&v)?;
        assert_eq!(
            r#"{"leader_id":{"term":1,"voted_for":2},"committed":false}"#,
            s
        );

        let v2: Vote = serde_json::from_str(&s)?;
        assert_eq!(v, v2);

        Ok(())
    }

    #[test]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn test_vote_partial_order() -> anyhow::Result<()> {
        #[allow(clippy::redundant_closure)]
        let vote = |term, node_id: u64| Vote::new(term, nid(node_id));

        #[allow(clippy::redundant_closure)]
        let committed =
            |term, node_id: u64| Vote::new_committed(term, nid(node_id));

        // Compare term first
        assert!(vote(2, 2) > vote(1, 2));
        assert!(vote(2, 2) >= vote(1, 2));
        assert!(vote(1, 2) < vote(2, 2));
        assert!(vote(1, 2) <= vote(2, 2));

        // Committed greater than non-committed if leader_id is incomparable
        assert!(committed(2, 2) > vote(2, 2));
        assert!(committed(2, 2) >= vote(2, 2));
        assert!(committed(2, 1) > vote(2, 2));
        assert!(committed(2, 1) >= vote(2, 2));

        // Lower term committed is not greater
        assert!(!(committed(1, 1) > vote(2, 1)));
        assert!(!(committed(1, 1) >= vote(2, 1)));

        // Compare to itself
        assert!(committed(1, 1) >= committed(1, 1));
        assert!(committed(1, 1) <= committed(1, 1));
        assert!(committed(1, 1) == committed(1, 1));

        // Equal
        assert!(vote(2, 2) == vote(2, 2));
        assert!(vote(2, 2) >= vote(2, 2));
        assert!(vote(2, 2) <= vote(2, 2));

        // Incomparable
        assert!(!(vote(2, 2) > vote(2, 3)));
        assert!(!(vote(2, 2) >= vote(2, 3)));
        assert!(!(vote(2, 2) < vote(2, 3)));
        assert!(!(vote(2, 2) <= vote(2, 3)));
        assert!(!(vote(2, 2) == vote(2, 3)));

        // Incomparable committed
        {
            fn assert_panic<T, F: FnOnce() -> T + UnwindSafe>(f: F) {
                let res = std::panic::catch_unwind(f);
                assert!(res.is_err());
            }
            assert_panic(|| (committed(2, 2) > committed(2, 3)));
            assert_panic(|| (committed(2, 2) >= committed(2, 3)));
            assert_panic(|| (committed(2, 2) < committed(2, 3)));
            assert_panic(|| (committed(2, 2) <= committed(2, 3)));
        }

        Ok(())
    }
}
