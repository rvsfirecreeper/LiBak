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
