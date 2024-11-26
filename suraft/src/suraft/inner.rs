use std::fmt;
use std::future::Future;
use std::sync::Arc;

use tracing::debug;
use tracing::trace;
use tracing::Level;

use crate::async_runtime::watch::WatchReceiver;
use crate::async_runtime::watch::WatchSender;
use crate::async_runtime::MpscUnboundedSender;
use crate::config::RuntimeConfig;
use crate::core::core_state::CoreState;
use crate::core::io::api_message::APIMessage;
use crate::core::TickHandle;
use crate::errors::Fatal;
use crate::storage::membership::NodeId;
use crate::type_config::alias::AsyncRuntimeOf;
use crate::type_config::alias::MpscUnboundedSenderOf;
use crate::type_config::alias::OneshotReceiverOf;
use crate::type_config::alias::WatchReceiverOf;
use crate::type_config::AsyncRuntime;
use crate::type_config::TypeConfigExt;
use crate::Config;
use crate::Metrics;
use crate::OptionalSend;
use crate::TypeConfig;

/// SuRaftInner is the internal handle and provides internally used APIs to
/// communicate with `Core`.
pub(in crate::suraft) struct SuRaftInner<C>
where C: TypeConfig
{
    #[allow(dead_code)]
    pub(in crate::suraft) id: NodeId,
    pub(in crate::suraft) config: Arc<Config>,
    pub(in crate::suraft) runtime_config: Arc<RuntimeConfig>,
    pub(in crate::suraft) tick_handle: TickHandle<C>,
    pub(in crate::suraft) tx_api: MpscUnboundedSenderOf<C, APIMessage<C>>,
    pub(in crate::suraft) rx_metrics: WatchReceiverOf<C, Metrics<C>>,
    pub(in crate::suraft) core_state: std::sync::Mutex<CoreState<C>>,
}

impl<C> SuRaftInner<C>
where C: TypeConfig
{
    /// Send a [`APIMessage`] to Core
    pub(crate) async fn send_msg(
        &self,
        mes: APIMessage<C>,
    ) -> Result<(), Fatal> {
        let send_res = self.tx_api.send(mes);

        if let Err(e) = send_res {
            let fatal = self
                .get_core_stopped_error(
                    "sending APIMessage to Core",
                    Some(e.0.to_string()),
                )
                .await;
            return Err(fatal);
        }
        Ok(())
    }

    /// Receive a message from Core, return error if Core has stopped.
    pub(crate) async fn recv_msg<T, E>(
        &self,
        rx: impl Future<Output = Result<T, E>>,
    ) -> Result<T, Fatal>
    where
        T: OptionalSend,
        E: OptionalSend,
    {
        let recv_res = rx.await;
        debug!(
            "{} receives result is error: {:?}",
            func_name!(),
            recv_res.is_err()
        );

        match recv_res {
            Ok(x) => Ok(x),
            Err(_) => {
                let fatal = self
                    .get_core_stopped_error(
                        "receiving rx from Core",
                        None::<&'static str>,
                    )
                    .await;
                tracing::error!(
                    error = debug(&fatal),
                    "error when {}",
                    func_name!()
                );
                Err(fatal)
            }
        }
    }

    /// Invoke Core by sending a [`APIMessage`] and blocks waiting for response.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(crate) async fn call_core<T>(
        &self,
        mes: APIMessage<C>,
        rx: OneshotReceiverOf<C, T>,
    ) -> Result<T, Fatal>
    where
        T: OptionalSend,
    {
        let sum = if tracing::enabled!(Level::DEBUG) {
            Some(mes.to_string())
        } else {
            None
        };

        self.send_msg(mes).await?;

        let recv_res = rx.await;
        trace!(
            "id={} call_core receives result is error: {:?}",
            self.id,
            recv_res.is_err()
        );

        match recv_res {
            Ok(x) => Ok(x),
            Err(_) => {
                let fatal = self
                    .get_core_stopped_error("receiving rx from Core", sum)
                    .await;
                tracing::error!(error = debug(&fatal), "core_call fatal error");
                Err(fatal)
            }
        }
    }

    #[allow(dead_code)]
    pub(in crate::suraft) fn is_core_running(&self) -> bool {
        let state = self.core_state.lock().unwrap();
        state.is_running()
    }

    /// Get the error that caused Core to stop.
    pub(in crate::suraft) async fn get_core_stopped_error(
        &self,
        when: impl fmt::Display,
        message_summary: Option<impl fmt::Display + Default>,
    ) -> Fatal {
        // Wait for the core task to finish.
        self.join_core_task().await;

        // Retrieve the result.
        let core_res = {
            let state = self.core_state.lock().unwrap();
            if let CoreState::Done(core_task_res) = &*state {
                core_task_res.clone()
            } else {
                unreachable!("Core should have already quit")
            }
        };

        tracing::error!(
            core_result = debug(&core_res),
            "failure {}; message: {}",
            when,
            message_summary.unwrap_or_default()
        );

        // Safe unwrap: core_res is always an error
        core_res.unwrap_err()
    }

    /// Wait for `Core` task to finish and record the returned value from
    /// the task.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(in crate::suraft) async fn join_core_task(&self) {
        // Get the Running state of Core,
        // or an error if Core has been in Joining state.
        let running_res = {
            let mut state = self.core_state.lock().unwrap();

            match &*state {
                CoreState::Running(_) => {
                    let (tx, rx) = C::watch_channel::<bool>(false);

                    let prev =
                        std::mem::replace(&mut *state, CoreState::Joining(rx));

                    let CoreState::Running(join_handle) = prev else {
                        unreachable!()
                    };

                    Ok((join_handle, tx))
                }
                CoreState::Joining(watch_rx) => Err(watch_rx.clone()),
                CoreState::Done(_) => {
                    // Core has already finished exiting, nothing to do
                    return;
                }
            }
        };

        match running_res {
            Ok((join_handle, tx)) => {
                let join_res = join_handle.await;

                tracing::info!(res = debug(&join_res), "Core exited");

                let core_task_res = match join_res {
                    Err(err) => {
                        if AsyncRuntimeOf::<C>::is_panic(&err) {
                            Err(Fatal::Panicked)
                        } else {
                            Err(Fatal::Stopped)
                        }
                    }
                    Ok(returned_res) => returned_res,
                };

                {
                    let mut state = self.core_state.lock().unwrap();
                    *state = CoreState::Done(core_task_res);
                }
                tx.send(true).ok();
            }
            Err(mut rx) => {
                // Other thread is waiting for the core to finish.
                loop {
                    let res = rx.changed().await;
                    if res.is_err() {
                        break;
                    }
                    if *rx.borrow_watched() {
                        break;
                    }
                }
            }
        }
    }
}
