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

mod backup;
mod chunking;
mod list;
mod manifest_store;
mod passwd;
mod restore;
mod scan;
mod types;
#[cfg(test)]
use backup::backup;
#[cfg(test)]
use manifest_store::read_manifests;
#[cfg(test)]
use manifest_store::write_manifests;
pub(crate) use backup::full_backup;
pub(crate) use list::{BackupSummary, list_backups};
pub use passwd::pass_to_key;
pub(crate) use restore::full_restore;
#[cfg(test)]
use restore::restore;
#[cfg(test)]
mod tests;
static PASS_TEST_CONTENTS: &[u8; 58] =
    b"this is an important file, designed for testing passwords.";
