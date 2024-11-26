#[allow(clippy::module_inception)]
mod config;
mod errors;

#[cfg(test)]
mod config_test;

pub use config::Config;
pub(crate) use config::RuntimeConfig;
