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
