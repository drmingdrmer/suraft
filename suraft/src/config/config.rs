//! SuRaft runtime configuration.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyerror::AnyError;
use clap::Parser;
use rand::Rng;

use crate::config::errors::ConfigError;
use crate::AsyncRuntime;

/// The runtime configuration for a SuRaft node.
///
/// The default values used by this type should generally work well for SuRaft
/// clusters which will be running with nodes in multiple datacenter
/// availability zones with low latency between zones. These values should
/// typically be made configurable from the perspective of the application which
/// is being built on top of SuRaft.
///
/// When building the SuRaft configuration for your application, remember this
/// inequality from the SuRaft spec: `broadcastTime ≪ electionTimeout ≪ MTBF`.
///
/// > In this inequality `broadcastTime` is the average time it takes a server
/// > to send RPCs in
/// > parallel to every server in the cluster and receive their responses;
/// > `electionTimeout` is the
/// > election timeout described in Section 5.2; and `MTBF` is the average time
/// > between failures for
/// > a single server. The broadcast time should be an order of magnitude less
/// > than the election
/// > timeout so that leaders can reliably send the heartbeat messages required
/// > to keep followers
/// > from starting elections; given the randomized approach used for election
/// > timeouts, this
/// > inequality also makes split votes unlikely. The election timeout should be
/// > a few orders of
/// > magnitude less than `MTBF` so that the system makes steady progress. When
/// > the leader crashes,
/// > the system will be unavailable for roughly the election timeout; we would
/// > like this to
/// > represent only a small fraction of overall time.
///
/// What does all of this mean? Simply keep your election timeout settings high
/// enough that the performance of your network will not cause election
/// timeouts, but don't keep it so high that a real leader crash would cause
/// prolonged downtime. See the SuRaft spec §5.6 for more details.
#[derive(Clone, Debug, Parser)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// The minimum election timeout in milliseconds
    #[clap(long, default_value = "150")]
    pub election_timeout_min: u64,

    /// The maximum election timeout in milliseconds
    #[clap(long, default_value = "300")]
    pub election_timeout_max: u64,

    /// The heartbeat interval in milliseconds at which leaders will send
    /// heartbeats to followers
    #[clap(long, default_value = "50")]
    pub heartbeat_interval: u64,

    /// Enable or disable tick.
    ///
    /// If ticking is disabled, timeout based events are all disabled:
    /// a follower won't wake up to enter candidate state,
    /// and a leader won't send heartbeat.
    ///
    /// This flag is mainly used for test, or to build a consensus system that
    /// does not depend on wall clock. The value of this config is
    /// evaluated as follows:
    /// - being absent: true
    /// - `--enable-tick`: true
    /// - `--enable-tick=true`: true
    /// - `--enable-tick=false`: false
    // clap 4 requires `num_args = 0..=1`, or it complains about missing arg
    // error https://github.com/clap-rs/clap/discussions/4374
    #[clap(long,
           default_value_t = true,
           action = clap::ArgAction::Set,
           num_args = 0..=1,
           default_missing_value = "true"
    )]
    pub enable_tick: bool,

    /// Whether a leader sends heartbeat log to following nodes, i.e.,
    /// followers and learners.
    // clap 4 requires `num_args = 0..=1`, or it complains about missing arg
    // error https://github.com/clap-rs/clap/discussions/4374
    #[clap(long,
           default_value_t = true,
           action = clap::ArgAction::Set,
           num_args = 0..=1,
           default_missing_value = "true"
    )]
    pub enable_heartbeat: bool,

    /// Whether a follower will enter candidate state if it does not receive
    /// message from the leader for a while.
    // clap 4 requires `num_args = 0..=1`, or it complains about missing arg
    // error https://github.com/clap-rs/clap/discussions/4374
    #[clap(long,
           default_value_t = true,
           action = clap::ArgAction::Set,
           num_args = 0..=1,
           default_missing_value = "true"
    )]
    pub enable_elect: bool,
}

/// Updatable config for a raft runtime.
pub(crate) struct RuntimeConfig {
    pub(crate) enable_heartbeat: AtomicBool,
    pub(crate) enable_elect: AtomicBool,
}

impl RuntimeConfig {
    pub(crate) fn new(config: &Config) -> Self {
        Self {
            enable_heartbeat: AtomicBool::from(config.enable_heartbeat),
            enable_elect: AtomicBool::from(config.enable_elect),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        <Self as Parser>::parse_from(Vec::<&'static str>::new())
    }
}

impl Config {
    /// Generate a new random election timeout within the configured min & max.
    pub fn new_rand_election_timeout<RT: AsyncRuntime>(&self) -> Duration {
        let ms = RT::thread_rng()
            .gen_range(self.election_timeout_min..self.election_timeout_max);

        Duration::from_millis(ms)
    }

    /// Build a `Config` instance from a series of command line arguments.
    ///
    /// The first element in `args` must be the application name.
    pub fn build(args: &[&str]) -> Result<Config, ConfigError> {
        let config = <Self as Parser>::try_parse_from(args).map_err(|e| {
            ConfigError::ParseError {
                source: AnyError::from(&e),
                args: args.iter().map(|x| x.to_string()).collect(),
            }
        })?;
        config.validate()
    }

    /// Validate the state of this config.
    pub fn validate(self) -> Result<Config, ConfigError> {
        if self.election_timeout_min >= self.election_timeout_max {
            return Err(ConfigError::ElectionTimeout {
                min: self.election_timeout_min,
                max: self.election_timeout_max,
            });
        }

        if self.election_timeout_min <= self.heartbeat_interval {
            return Err(ConfigError::ElectionTimeoutLTHeartBeat {
                election_timeout_min: self.election_timeout_min,
                heartbeat_interval: self.heartbeat_interval,
            });
        }

        Ok(self)
    }
}
