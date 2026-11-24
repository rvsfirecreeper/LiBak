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

use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ffi::OsStr, path::PathBuf};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
pub(super) type Id = [u8; 32];
pub(super) fn id_to_name(id: &Id) -> String {
    hex::encode(id)
}
pub(super) fn name_to_id(name: &OsStr) -> Option<Id> {
    let id = hex::decode(name.as_encoded_bytes()).ok();
    if let Some(e) = id
        && e.len() == 32
    {
        return e.as_slice().try_into().ok();
    }
    None
}
#[derive(Zeroize, ZeroizeOnDrop)]
pub(super) struct ProcessedChunk {
    pub(super) nonce: [u8; 24],
    pub(super) id: Id,
    pub(super) data: Zeroizing<Vec<u8>>,
}
#[derive(Zeroize, ZeroizeOnDrop)]
pub(super) struct ProcessedFile {
    pub(super) chunks: Vec<ProcessedChunk>,
    pub(super) manifest: Vec<Id>,
}
pub(super) enum ScanResult {
    File(PathBuf),
    Folder(PathBuf),
    /// THE FIRST ONE IS THE SYMLINK'S PATH AND SECOND IS TARGET
    Symlink(PathBuf, PathBuf, SymlinkType),
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) enum Manifest {
    File {
        path: PathBuf,
        chunks: Vec<Id>,
    },
    Directory {
        path: PathBuf,
        content: HashSet<PathBuf>,
    },
    Symlink {
        path: PathBuf,
        target: PathBuf,
        symlink_type: SymlinkType,
    },
}
// thanks, Windows
#[derive(Clone, Deserialize, Serialize)]
pub(super) enum SymlinkType {
    Folder,
    File,
}
