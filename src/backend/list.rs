// SPDX-License-Identifier: GPL-3.0-or-later
//
// LiBak, a simple encrypted backup tool.
// Copyright (C) 2026 Raj Singh
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

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
