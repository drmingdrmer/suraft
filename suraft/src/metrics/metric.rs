use std::cmp::Ordering;

use crate::metrics::metric_display::MetricDisplay;
use crate::storage::vote::Vote;
use crate::Metrics;
use crate::TypeConfig;

/// A metric entry of a SuRaft node.
///
/// This is used to specify which metric to observe.
#[derive(Debug)]
pub enum Metric {
    Term(u64),
    Vote(Option<Vote>),
    LastLogIndex(Option<u64>),
}

impl Metric {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Metric::Term(_) => "term",
            Metric::Vote(_) => "vote",
            Metric::LastLogIndex(_) => "last_log_index",
        }
    }

    pub(crate) fn value(&self) -> MetricDisplay<'_> {
        MetricDisplay { metric: self }
    }
}

/// Metric can be compared with RaftMetrics by comparing the corresponding field
/// of RaftMetrics.
impl<C> PartialEq<Metric> for Metrics<C>
where C: TypeConfig
{
    fn eq(&self, other: &Metric) -> bool {
        match other {
            Metric::Term(v) => self.vote.as_ref().map(|x| x.term()) == Some(*v),
            Metric::Vote(v) => &self.vote == v,
            Metric::LastLogIndex(v) => {
                self.committed.as_ref().map(|x| x.index) == *v
            }
        }
    }
}

/// Metric can be compared with RaftMetrics by comparing the corresponding field
/// of RaftMetrics.
impl<C> PartialOrd<Metric> for Metrics<C>
where C: TypeConfig
{
    fn partial_cmp(&self, other: &Metric) -> Option<Ordering> {
        match other {
            Metric::Term(v) => {
                Some(self.vote.as_ref().map(|x| x.term()).cmp(&Some(*v)))
            }
            Metric::Vote(v) => self.vote.partial_cmp(v),
            Metric::LastLogIndex(v) => {
                Some(self.committed.as_ref().map(|x| x.index).cmp(v))
            }
        }
    }
}
