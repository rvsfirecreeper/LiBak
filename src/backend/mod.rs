mod backup;
mod chunking;
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
pub use passwd::pass_to_key;
#[cfg(test)]
use restore::restore;
#[cfg(test)]
mod tests;
static PASS_TEST_CONTENTS: &[u8; 58] =
    b"this is an important file, designed for testing passwords.";
