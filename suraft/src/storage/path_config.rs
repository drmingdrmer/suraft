use crate::storage::log::log_index::LogIndex;

pub(crate) struct PathConfig;

impl PathConfig {
    pub fn log_prefix() -> &'static str {
        "/log/"
    }

    pub fn log_entry(index: u64) -> String {
        format!("/log/{}.entry", LogIndex(index).ordered_encode())
    }

    pub fn membership_config() -> String {
        "/meta/membership_config.json".to_string()
    }

    pub fn last_log_path() -> String {
        "/meta/last_log_path.json".to_string()
    }

    pub fn last_purged_log_path() -> String {
        "/meta/last_purged_log_path.json".to_string()
    }
}
