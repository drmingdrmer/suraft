use std::io;

use openraft_macros::add_async_trait;

use crate::storage::log::entry::Entry;
use crate::storage::membership::Membership;
use crate::storage::path_config::PathConfig;
use crate::storage::LogStorage;
use crate::TypeConfig;

#[add_async_trait]
pub trait LogStorageExt<C>: LogStorage<C>
where C: TypeConfig
{
    async fn read_last_log_entry(
        &mut self,
    ) -> Result<Option<Entry<C>>, io::Error> {
        let last_log_path = PathConfig::last_log_path();
        let data = self.read(&last_log_path).await?;

        let last: String = if let Some(data) = data {
            serde_json::from_slice(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            "".to_string()
        };

        let entry_paths = self.list(PathConfig::log_prefix(), &last).await?;

        let last = entry_paths.last().cloned().unwrap_or(last);
        if last.is_empty() {
            return Ok(None);
        }

        let Some(data) = self.read(&last).await? else {
            return Ok(None);
        };

        let entry: Entry<C> = serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(entry))
    }

    async fn read_log_entry(
        &mut self,
        index: u64,
    ) -> Result<Option<Entry<C>>, io::Error> {
        let path = PathConfig::log_entry(index);
        let data = self.read(&path).await?;

        if let Some(data) = data {
            let entry: Entry<C> = serde_json::from_slice(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn write_log_entry(
        &mut self,
        entry: &Entry<C>,
    ) -> Result<bool, io::Error> {
        let log_id = &entry.log_id;
        let path = PathConfig::log_entry(log_id.index);

        let buf = serde_json::to_vec(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.write(&path, &buf, true).await
    }

    async fn read_membership(
        &mut self,
    ) -> Result<Option<Membership>, io::Error> {
        let path = PathConfig::membership_config();
        let data = self.read(&path).await?;

        if let Some(data) = data {
            let m = serde_json::from_slice(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(m))
        } else {
            Ok(None)
        }
    }

    async fn write_membership(
        &mut self,
        membership: &Membership,
    ) -> Result<(), io::Error> {
        let path = PathConfig::membership_config();

        let buf = serde_json::to_vec(membership)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.write(&path, &buf, false).await?;
        Ok(())
    }
}

impl<C, T> LogStorageExt<C> for T
where
    C: TypeConfig,
    T: LogStorage<C>,
{
}
