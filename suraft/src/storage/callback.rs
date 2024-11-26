//! Callbacks used by Storage API

use std::io;

use crate::async_runtime::MpscUnboundedSender;
use crate::async_runtime::MpscUnboundedWeakSender;
use crate::core::io::notification::Notification;
use crate::type_config::alias::MpscUnboundedWeakSenderOf;
use crate::type_config::alias::OneshotSenderOf;
use crate::type_config::async_runtime::oneshot::OneshotSender;
use crate::TypeConfig;

/// A callback for completion of io operation to [`LogStorage`].
///
/// [`LogStorage`]: `crate::storage::LogStorage`
pub struct IOFlushed<C>
where C: TypeConfig
{
    /// To notify to send when the IO complete.
    notification: Notification<C>,

    tx: MpscUnboundedWeakSenderOf<C, Notification<C>>,
}

impl<C> IOFlushed<C>
where C: TypeConfig
{
    pub(crate) fn new(
        notify: Notification<C>,
        tx: MpscUnboundedWeakSenderOf<C, Notification<C>>,
    ) -> Self {
        Self {
            notification: notify,
            tx,
        }
    }

    /// Report log io completion event.
    ///
    /// It will be called when the log is successfully appended to the storage
    /// or an error occurs.
    pub fn io_completed(mut self, result: Result<bool, io::Error>) {
        let Some(tx) = self.tx.upgrade() else {
            tracing::warn!(
                "failed to upgrade tx, Core may have closed the receiver"
            );
            return;
        };

        let send_res = match result {
            Err(e) => {
                tracing::error!(
                    "{}: IOFlushed error: {}, while flushing IO: {}",
                    func_name!(),
                    e,
                    self.notification
                );

                tx.send(Notification::StorageError { error: e })
            }
            Ok(done) => {
                tracing::debug!(
                    "{}: IOFlushed completed: {}",
                    func_name!(),
                    self.notification
                );
                self.notification.set_io_result(done);
                tx.send(self.notification)
            }
        };

        if let Err(e) = send_res {
            tracing::warn!("failed to send log io completion event: {}", e.0);
        }
    }
}

pub struct ClientCallback<C, T>
where
    C: TypeConfig,
    T: Send,
{
    client_tx: OneshotSenderOf<C, T>,
    tx: MpscUnboundedWeakSenderOf<C, Notification<C>>,
}

impl<C, T> ClientCallback<C, T>
where
    C: TypeConfig,
    T: Send,
{
    pub(crate) fn new(
        client_tx: OneshotSenderOf<C, T>,
        tx: MpscUnboundedWeakSenderOf<C, Notification<C>>,
    ) -> Self {
        Self { client_tx, tx }
    }

    /// Report log io completion event.
    ///
    /// It will be called when the log is successfully appended to the storage
    /// or an error occurs.
    pub fn io_completed(self, result: Result<T, io::Error>) {
        let Some(tx) = self.tx.upgrade() else {
            tracing::warn!(
                "failed to upgrade tx, Core may have closed the receiver"
            );
            return;
        };

        match result {
            Err(e) => {
                tracing::error!("{}: error: {}", func_name!(), e,);
                let send_res = tx.send(Notification::StorageError { error: e });
                if let Err(e) = send_res {
                    tracing::warn!("failed to send io error: {}", e.0);
                }
            }
            Ok(done) => {
                tracing::debug!("{}: Callback called", func_name!(),);
                let send_res = self.client_tx.send(done);
                if let Err(_e) = send_res {
                    tracing::warn!("failed to send");
                }
            }
        };
    }
}
