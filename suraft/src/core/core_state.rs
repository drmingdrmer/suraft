use crate::errors::Fatal;
use crate::errors::Infallible;
use crate::type_config::alias::JoinHandleOf;
use crate::type_config::alias::WatchReceiverOf;
use crate::TypeConfig;

/// The running state of Core
pub(crate) enum CoreState<C>
where C: TypeConfig
{
    /// The Core task is still running.
    Running(JoinHandleOf<C, Result<Infallible, Fatal>>),

    /// The Core task is waiting for a signal to finish joining.
    Joining(WatchReceiverOf<C, bool>),

    /// The Core task has finished. The return value of the task is stored.
    Done(Result<Infallible, Fatal>),
}

impl<C> CoreState<C>
where C: TypeConfig
{
    /// Returns `true` if the Core task is still running.
    pub(crate) fn is_running(&self) -> bool {
        matches!(self, CoreState::Running(_))
    }
}
