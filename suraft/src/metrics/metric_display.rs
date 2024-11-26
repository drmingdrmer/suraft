use std::fmt;
use std::fmt::Formatter;

use crate::base::display_ext::DisplayOption;
use crate::base::display_ext::DisplayOptionExt;
use crate::metrics::Metric;

/// Display the value of a metric.
pub(crate) struct MetricDisplay<'a> {
    pub(crate) metric: &'a Metric,
}

impl<'a> fmt::Display for MetricDisplay<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.metric {
            Metric::Term(v) => write!(f, "{}", v),
            Metric::Vote(v) => write!(f, "{}", v.display()),
            Metric::LastLogIndex(v) => write!(f, "{}", DisplayOption(v)),
        }
    }
}
