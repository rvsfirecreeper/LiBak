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

use super::types::{ScanResult, SymlinkType};
use crate::Error::{self, InvariantsFailed};
use std::{fs, path::Path};
pub(super) fn scan(root: &Path, path: &Path, exclude: &[&Path]) -> Result<Vec<ScanResult>, Error> {
    if exclude.contains(&path) {
        return Ok(vec![]);
    }
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(vec![ScanResult::Symlink(
            path.strip_prefix(root)
                .map_err(|_| /* Something within the scan should Never, never ever not be in the root or the universe is super broken*/ Error::InvariantsFailed)?
                .to_owned(),
            fs::read_link(path)?,
            match fs::metadata(path) {
                Ok(m) => {
                    if m.is_dir() {
                        SymlinkType::Folder
                    }
                    else {SymlinkType::File}
                },
                Err(_) => SymlinkType::File,
            }
        )]);
    }
    let mut files = vec![];
    {
        let meta = path.metadata()?;
        if !(meta.is_dir() || meta.is_file() || meta.is_symlink()) {
            return Ok(files);
        }
    }
    if path.is_dir() {
        files.push(ScanResult::Folder(
            path.strip_prefix(root)
                .map_err(|_| /* See line 96 */ InvariantsFailed)?
                .to_path_buf(),
        ));
        for dir_entry in fs::read_dir(path)? {
            files.append(&mut scan(root, dir_entry?.path().as_path(), exclude)?);
        }
    } else {
        files.push(ScanResult::File(
            path.strip_prefix(root)
                .map_err(|_| /* Something within the scan should Never, never ever not be in the root or the universe is super broken*/ Error::InvariantsFailed)?
                .to_owned(),
        ));
    }

    Ok(files)
}
