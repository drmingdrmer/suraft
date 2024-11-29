#![cfg_attr(feature = "bt", feature(error_generic_member_access))]

#[macro_use]
#[path = "../fixtures/mod.rs"]
mod fixtures;

mod t50_append_entries_backoff;
mod t50_append_entries_backoff_rejoin;
#[cfg(feature = "loosen-follower-log-revert")]
mod t60_feature_loosen_follower_log_revert;
mod t61_allow_follower_log_revert;
