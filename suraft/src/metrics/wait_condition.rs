use std::fmt;

use crate::metrics::metric_display::MetricDisplay;
use crate::metrics::Metric;

/// A condition that the application wait for.
#[derive(Debug)]
pub(crate) enum Condition {
    GE(Metric),
    EQ(Metric),
}

impl Condition {
    /// Build a new condition which the application will await to meet or exceed.
    pub(crate) fn ge(v: Metric) -> Self {
        Self::GE(v)
    }

    /// Build a new condition which the application will await to meet.
    pub(crate) fn eq(v: Metric) -> Self {
        Self::EQ(v)
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Condition::GE(v) => v.name(),
            Condition::EQ(v) => v.name(),
        }
    }

    pub(crate) fn op(&self) -> &'static str {
        match self {
            Condition::GE(_) => ">=",
            Condition::EQ(_) => "==",
        }
    }

    pub(crate) fn value(&self) -> MetricDisplay<'_> {
        match self {
            Condition::GE(v) => v.value(),
            Condition::EQ(v) => v.value(),
        }
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.name(), self.op(), self.value())
    }
}
