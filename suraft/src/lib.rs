#![doc = include_str!("lib_readme.md")]
#![cfg_attr(feature = "bt", feature(error_generic_member_access))]
#![cfg_attr(feature = "bench", feature(test))]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]
#![deny(unused_qualifications)]

macro_rules! func_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let n = &name[..name.len() - 3];
        let nn = n.replace("::{{closure}}", "");
        nn
    }};
}

pub extern crate openraft_macros;

mod config;
mod core;
mod quorum;

pub mod app;
pub mod base;
pub mod errors;
pub mod impls;
pub mod metrics;
pub mod network;
pub mod storage;
pub mod suraft;
pub mod testing;
pub mod type_config;

pub use anyerror;
pub use anyerror::AnyError;
pub use openraft_macros::add_async_trait;
pub use storage::vote::Vote;
pub use type_config::async_runtime;
#[cfg(feature = "tokio-rt")]
pub use type_config::async_runtime::tokio_impls::TokioRuntime;
pub use type_config::AsyncRuntime;

pub use crate::base::OptionalSend;
pub use crate::base::OptionalSync;
pub use crate::base::Serde;
pub use crate::config::Config;
pub use crate::metrics::Metrics;
pub use crate::network::Network;
pub use crate::storage::membership::Node;
pub use crate::storage::membership::NodeId;
pub use crate::suraft::SuRaft;
pub use crate::type_config::TypeConfig;
