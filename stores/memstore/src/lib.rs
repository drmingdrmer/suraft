use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Error;
use std::sync::Arc;
use std::sync::Mutex;

use suraft::storage::LogStorage;
use tracing::debug;

/// An in-memory log storage implementing the [`LogStorage`] trait.
#[derive(Debug, Clone, Default)]
pub struct MemLogStore {
    store: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl<C> LogStorage<C> for MemLogStore
where C: suraft::TypeConfig
{
    async fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let store = self.store.lock().unwrap();
        let got = store.get(path).cloned();

        debug!("MemLogStore::read: path={}, got={:?}", path, got);
        Ok(got)
    }

    async fn write(
        &mut self,
        path: &str,
        buf: &[u8],
        exclusive: bool,
    ) -> Result<bool, Error> {
        debug!("MemLogStore::write: path={}, exclusive={}", path, exclusive);

        let mut store = self.store.lock().unwrap();
        if store.contains_key(path) {
            if exclusive {
                return Ok(false);
            }
        }

        store.insert(path.to_string(), buf.to_vec());
        Ok(true)
    }

    async fn list(
        &mut self,
        prefix: &str,
        start_after: &str,
    ) -> Result<Vec<String>, Error> {
        let right = prefix_right_bound(prefix);

        let store = self.store.lock().unwrap();
        let keys = if let Some(right) = right {
            store.range(prefix.to_string()..right.to_string())
        } else {
            store.range(prefix.to_string()..)
        }
        .filter_map(|(k, _)| {
            if k.as_str() > start_after {
                Some(k.clone())
            } else {
                None
            }
        });

        let keys = keys.collect();
        debug!(
            "MemLogStore::list prefix={}; start_after={}; got:{:?}",
            prefix, start_after, keys
        );
        Ok(keys)
    }
}

pub fn prefix_right_bound(p: &str) -> Option<String> {
    let mut chars = p.chars().collect::<Vec<_>>();

    // Start from the end of the character list and look for the first character
    // that is not \u{10FFFF}
    for i in (0..chars.len()).rev() {
        if chars[i] as u32 != 0x10FFFF {
            // Try to increment the character
            if let Some(next_char) = char::from_u32(chars[i] as u32 + 1) {
                chars[i] = next_char;
                // Remove all characters after the incremented one
                chars.truncate(i + 1);
                return Some(chars.iter().collect());
            } else {
                // If incrementing results in an invalid character, return None
                return None;
            }
        }
    }

    // If all characters are \u{10FFFF} or the string is empty, return None
    None
}
