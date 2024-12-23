//! Basic types used in the SuRaft implementation.

pub(crate) mod display_ext;
pub(crate) mod leased;

pub use serde_able::Serde;
pub use threaded::BoxAny;
pub use threaded::BoxAsyncOnceMut;
pub use threaded::BoxFuture;
pub use threaded::BoxOnce;
pub use threaded::OptionalSend;
pub use threaded::OptionalSync;

#[cfg(not(feature = "singlethreaded"))]
mod threaded {
    use std::any::Any;
    use std::future::Future;
    use std::pin::Pin;

    pub trait OptionalSend: Send {}
    impl<T: Send + ?Sized> OptionalSend for T {}

    pub trait OptionalSync: Sync {}
    impl<T: Sync + ?Sized> OptionalSync for T {}

    pub type BoxFuture<'a, T = ()> =
        Pin<Box<dyn Future<Output = T> + Send + 'a>>;
    pub type BoxAsyncOnceMut<'a, A, T = ()> =
        Box<dyn FnOnce(&mut A) -> BoxFuture<T> + Send + 'a>;
    pub type BoxOnce<'a, A, T = ()> = Box<dyn FnOnce(&A) -> T + Send + 'a>;
    pub type BoxAny = Box<dyn Any + Send>;
}

#[cfg(feature = "singlethreaded")]
mod threaded {
    use std::any::Any;
    use std::future::Future;
    use std::pin::Pin;

    pub trait OptionalSend {}
    impl<T: ?Sized> OptionalSend for T {}

    pub trait OptionalSync {}
    impl<T: ?Sized> OptionalSync for T {}

    pub type BoxFuture<'a, T = ()> = Pin<Box<dyn Future<Output = T> + 'a>>;
    pub type BoxAsyncOnceMut<'a, A, T = ()> =
        Box<dyn FnOnce(&mut A) -> BoxFuture<T> + 'a>;
    pub type BoxOnce<'a, A, T = ()> = Box<dyn FnOnce(&A) -> T + 'a>;
    pub type BoxAny = Box<dyn Any>;
}

mod serde_able {
    #[doc(hidden)]
    pub trait Serde: serde::Serialize + for<'a> serde::Deserialize<'a> {}
    impl<T> Serde for T where T: serde::Serialize + for<'a> serde::Deserialize<'a> {}
}
