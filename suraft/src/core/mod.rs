//! The `Core` is a `Runtime` supporting the raft algorithm implementation
//! `Engine`.
//!
//! It passes events from an application or timer or network to `Engine` to
//! drive it to run. Also, it receives and execute `Command` emitted by `Engine`
//! to apply raft state to underlying storage or forward messages to other raft
//! nodes.

pub(crate) mod core;
pub(crate) mod core_state;
pub(crate) mod io;
pub(crate) mod roles;
mod tick;

use roles::candidate::Candidate;
use roles::leader::Leader;
pub(crate) use tick::Tick;
pub(crate) use tick::TickHandle;

pub(crate) type LeaderState<C> = Option<Box<Leader<C>>>;
pub(crate) type CandidateState<C> = Option<Candidate<C>>;
