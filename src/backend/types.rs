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
