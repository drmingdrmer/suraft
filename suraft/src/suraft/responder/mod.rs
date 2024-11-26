//! API to consumer a response when a client write request is completed.

pub(crate) mod impls;
pub use impls::OneshotResponder;

use crate::suraft::WriteResult;
use crate::OptionalSend;
use crate::TypeConfig;

/// A trait that lets `Core` send the response or an error of a client write
/// request back to the client or to somewhere else.
///
/// It is created for each request [`AppData`], and is sent to `Core`.
/// Once the request is completed,
/// the `Core` send the result [`WriteResult`] via it.
/// The implementation of the trait then forward the response to application.
/// There could optionally be a receiver to wait for the response.
///
/// Usually an implementation of [`Responder`] is a oneshot channel Sender,
/// and [`Responder::Receiver`] is a oneshot channel Receiver.
///
/// [`AppData`]: `crate::app::AppData`
pub trait Responder<C>: OptionalSend + 'static
where C: TypeConfig
{
    /// An optional receiver to receive the result sent by `Core`.
    ///
    /// If the application does not need to wait for the response, it can be
    /// `()`.
    type Receiver;

    /// Build a new instance from the application request.
    fn from_app_data(
        app_data: C::AppData,
    ) -> (C::AppData, Self, Self::Receiver)
    where Self: Sized;

    /// Send result when the request has been completed.
    ///
    /// This method is called by the `Core` once the request has been
    /// applied to state machine.
    fn send(self, result: WriteResult);
}
