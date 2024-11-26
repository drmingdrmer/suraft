use crate::async_runtime::OneshotSender;
use crate::suraft::responder::Responder;
use crate::suraft::WriteResult;
use crate::type_config::alias::OneshotReceiverOf;
use crate::type_config::alias::OneshotSenderOf;
use crate::type_config::TypeConfigExt;
use crate::TypeConfig;

/// A [`Responder`] implementation that sends the response via a oneshot
/// channel.
///
/// This could be used when the [`SuRaft::client_write`] caller want to wait for
/// the response.
///
/// [`SuRaft::client_write`]: `crate::suraft::SuRaft::client_write`
pub struct OneshotResponder<C>
where C: TypeConfig
{
    tx: OneshotSenderOf<C, WriteResult>,
}

impl<C> OneshotResponder<C>
where C: TypeConfig
{
    /// Create a new instance from a [`AsyncRuntime::Oneshot::Sender`].
    ///
    /// [`AsyncRuntime::Oneshot::Sender`]: `crate::async_runtime::Oneshot::Sender`
    pub fn new(tx: OneshotSenderOf<C, WriteResult>) -> Self {
        Self { tx }
    }
}

impl<C> Responder<C> for OneshotResponder<C>
where C: TypeConfig
{
    type Receiver = OneshotReceiverOf<C, WriteResult>;

    fn from_app_data(
        app_data: C::AppData,
    ) -> (C::AppData, Self, Self::Receiver)
    where Self: Sized {
        let (tx, rx) = C::oneshot();
        (app_data, Self { tx }, rx)
    }

    fn send(self, res: WriteResult) {
        let res = self.tx.send(res);

        if res.is_ok() {
            tracing::debug!("OneshotConsumer.tx.send: is_ok: {}", res.is_ok());
        } else {
            tracing::warn!("OneshotConsumer.tx.send: is_ok: {}", res.is_ok());
        }
    }
}
