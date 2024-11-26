use anyerror::AnyError;

/// Error variants related to configuration.
#[derive(Debug, thiserror::Error)]
#[derive(PartialEq, Eq)]
pub enum ConfigError {
    #[error("ParseError: {source} while parsing ({args:?})")]
    ParseError { source: AnyError, args: Vec<String> },

    /// The min election timeout is not smaller than the max election timeout.
    #[error("election timeout: min({min}) must be < max({max})")]
    ElectionTimeout { min: u64, max: u64 },

    #[error("election_timeout_min({election_timeout_min}) must be > heartbeat_interval({heartbeat_interval})")]
    ElectionTimeoutLTHeartBeat {
        election_timeout_min: u64,
        heartbeat_interval: u64,
    },
}
