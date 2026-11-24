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

#[cfg(windows)]
use super::types::SymlinkType;
use super::types::{Manifest, id_to_name};
use crate::{
    BackupMetadata,
    Error::{self, CryptoError},
    Key,
    backend::{manifest_store::read_manifests, pass_to_key},
};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use std::{
    ffi::OsStr,
    fs::{self, File, read_dir},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use zeroize::{Zeroize, Zeroizing};
pub(super) fn restore(
    manifests: &[Manifest],
    store_dir: &Path,
    target_dir: &Path,
    key: &Key,
) -> Result<(), Error> {
    let mut to_check = Vec::new();
    for manifest in manifests {
        match manifest {
            Manifest::File { path, chunks } => {
                let mut target_file = File::create(target_dir.join(path))?;
                for chunk in chunks {
                    let mut chunk_file = File::open(store_dir.join(
                        PathBuf::from_str(&id_to_name(chunk)).expect("Infallible Error Type."),
                    ))?;
                    let mut chunk_data = Zeroizing::new(Vec::new());
                    chunk_file.read_to_end(&mut chunk_data)?;
                    if chunk_data.len() < 24 {
                        // also tampering or corruption
                        return Err(Error::CryptoError);
                    }
                    let nonce_internal: [u8; 24] = chunk_data[0..24]
                        .try_into()
                        .map_err(|_| Error::InvariantsFailed)?;
                    let nonce = XNonce::from(nonce_internal);
                    _ = chunk_data.drain(..24);
                    let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
                    cipher.decrypt_in_place(&nonce, chunk, &mut *chunk_data)?;
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    let mut decompressed_chunk =
                        zstd::bulk::decompress(chunk_data.as_slice(), 1024 * 1024 * 8)?;
                    target_file.write_all(&decompressed_chunk)?;
                    decompressed_chunk.zeroize();
                }
            }
            Manifest::Directory { path, content: _ } => {
                if path != &PathBuf::from_str("").expect("Infallible") {
                    fs::create_dir(target_dir.join(path))?;
                }

                to_check.push(manifest.clone());
            }
            Manifest::Symlink {
                path,
                target,
                // For us Linux devs
                #[allow(unused)]
                symlink_type,
            } => {
                let path = target_dir.join(path);
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target, path)?;
                }
                #[cfg(windows)]
                {
                    match match symlink_type {
                        SymlinkType::File => std::os::windows::fs::symlink_file(target, path),
                        SymlinkType::Folder => std::os::windows::fs::symlink_dir(target, path),
                    } {
                        Ok(_) => {}
                        Err(e) => {
                            if e.raw_os_error() != Some(1314) {
                                return Err(e)?;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut absent_root = true;
    for dir in &mut to_check {
        match dir {
            Manifest::File { path: _, chunks: _ }
            | Manifest::Symlink {
                path: _,
                target: _,
                symlink_type: _,
            } => panic!("What"),
            Manifest::Directory { path, content } => {
                if path.is_empty() {
                    absent_root = false;
                }
                let num_entries = content.len();
                let mut num_files = 0;
                let dir_entries = fs::read_dir(target_dir.join(&path))?;
                for i in dir_entries {
                    num_files += 1;
                    content.remove(&PathBuf::from(
                        i?.path().file_name().expect("how would ReadDir return .."),
                    ));
                }
                if (!content.is_empty()) || absent_root || (num_entries != num_files) {
                    // TAMPERING
                    return Err(CryptoError);
                }
            }
        }
    }
    Ok(())
}
// holy pokemon reference
pub(crate) fn full_restore(
    passwd: &str,
    backup_id: &OsStr,
    target_dir: &Path,
) -> Result<(), Error> {
    if read_dir(target_dir)?.next().is_some() {
        return Err(Error::TargetNotEmpty);
    }
    let meta_dir = &dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join("libak/meta")
        .join(backup_id);
    let mut meta_file = File::open(meta_dir.join("meta"))?;
    let mut metadata_vec = Vec::new();
    meta_file.read_to_end(&mut metadata_vec)?;
    let metadata = postcard::from_bytes::<BackupMetadata>(&metadata_vec)?;
    let (key, _) = pass_to_key(passwd, Some(metadata.salt))?;
    let mut pass_test_file = File::open(meta_dir.join("test"))?;
    let mut pass_test_vec = Vec::new();
    let mut nonce_internal = [0u8; 24];
    pass_test_file.read_exact(&mut nonce_internal)?;
    pass_test_file.read_to_end(&mut pass_test_vec)?;
    let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
    cipher
        .decrypt_in_place(&XNonce::from(nonce_internal), &[], &mut pass_test_vec)
        .map_err(|_| Error::InvalidPassword)?;

    let manifests = read_manifests(backup_id, &key)?;
    restore(
        &manifests,
        &dirs::data_local_dir()
            .ok_or(Error::NoDataDir)?
            .join(PathBuf::from_str("libak/chunks").expect("Infallible")),
        target_dir,
        &key,
    )
}
