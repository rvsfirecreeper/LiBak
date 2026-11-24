use crate::{BackupMetadata, Error};
use std::{ffi::OsString, fs};

/// One entry in the backup list. `id` is the hex directory name, which is exactly
/// what `full_restore` wants as its `backup_id`.
pub(crate) struct BackupSummary {
    pub(crate) id: OsString,
    pub(crate) metadata: BackupMetadata,
}

/// Reads the (unauthenticated) metadata of every backup, newest first.
/// Backups whose metadata can't be read are skipped instead of failing the whole list.
pub(crate) fn list_backups() -> Result<Vec<BackupSummary>, Error> {
    let meta_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join("libak/meta");
    if !fs::exists(&meta_dir)? {
        return Ok(Vec::new());
    }
    let mut backups = Vec::new();
    for entry in fs::read_dir(meta_dir)? {
        let entry = entry?;
        let Ok(bytes) = fs::read(entry.path().join("meta")) else {
            continue;
        };
        let Ok(metadata) = postcard::from_bytes::<BackupMetadata>(&bytes) else {
            continue;
        };
        backups.push(BackupSummary {
            id: entry.file_name(),
            metadata,
        });
    }
    backups.sort_by_key(|b| std::cmp::Reverse(b.metadata.time));
    Ok(backups)
}
