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

use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305, XNonce};
use rand::random;

use super::{
    chunking::process_file,
    scan::scan,
    types::{Manifest, ScanResult, id_to_name, name_to_id},
};
use crate::{
    BackupId, BackupMetadata,
    Error::{self, InvariantsFailed},
    Key,
    backend::{
        manifest_store::{dir_checkup, write_manifests},
        pass_to_key,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
pub(super) fn backup(
    root: &Path,
    key: &Key,
    exclude: &[&Path],
    store_dir: &Path,
) -> Result<Vec<Manifest>, Error> {
    let files = scan(root, root, exclude)?;
    let mut manifests = Vec::new();
    let mut store_hashes = HashSet::new();
    let store_dir_contents = store_dir.read_dir()?;
    for i in store_dir_contents {
        match name_to_id(&i?.file_name()) {
            None => {}
            Some(h) => _ = store_hashes.insert(h),
        }
    }
    let mut folder_indexes = HashMap::new();
    for file in files {
        match file {
            ScanResult::File(path) => {
                let file = File::open(root.join(&path))?;
                let processed_file = process_file(&file, key, &store_hashes)?;
                for i in &processed_file.chunks {
                    let mut chunk_file = File::create(
                        store_dir
                            .join(PathBuf::from_str(&id_to_name(&i.id)).map_err(
                                /* the string output is normalized, hashes should always be valid */
                                |_| Error::InvariantsFailed,
                            )?)
                            .with_added_extension("tmp"),
                    )?;
                    chunk_file.write_all(&i.nonce)?;
                    chunk_file.write_all(i.data.as_slice())?;
                    chunk_file.sync_all()?;
                    _ = store_hashes.insert(i.id);
                    drop(chunk_file);
                    fs::rename(
                        store_dir
                            .join(PathBuf::from_str(&id_to_name(&i.id)).map_err(
                                /* the string output is normalized, hashes should always be valid */
                                |_| Error::InvariantsFailed,
                            )?)
                            .with_added_extension("tmp"),
                        store_dir.join(PathBuf::from_str(&id_to_name(&i.id)).map_err(
                            /* the string output is normalized, hashes should always be valid */
                            |_| Error::InvariantsFailed,
                        )?),
                    )?;
                }
                let dir = path.parent();
                if let Some(dir) = dir {
                    match &mut manifests[*folder_indexes
                        .get(&dir.to_path_buf())
                        .expect("the dir should always come first")]
                    {
                        Manifest::Directory {
                            path: dir_path,
                            content,
                        } => {
                            content.insert(
                                path.strip_prefix(dir_path)
                                    .expect("literally parent btw")
                                    .to_owned(),
                            );
                        }
                        Manifest::File { path: _, chunks: _ }
                        | Manifest::Symlink {
                            path: _,
                            target: _,
                            symlink_type: _,
                        } => panic!("what how"),
                    }
                }
                manifests.push(Manifest::File {
                    path,
                    chunks: processed_file.manifest.clone(),
                });
            }
            ScanResult::Folder(path) => {
                manifests.push(Manifest::Directory {
                    path: path.clone(),
                    content: HashSet::new(),
                });
                let dir = path.parent();
                if let Some(dir) = dir {
                    match &mut manifests[*folder_indexes
                        .get(&dir.to_path_buf())
                        .expect("the dir should always come first")]
                    {
                        Manifest::Directory {
                            path: dir_path,
                            content,
                        } => {
                            content.insert(
                                path.strip_prefix(dir_path)
                                    .expect("literally parent btw")
                                    .to_owned(),
                            );
                        }
                        Manifest::File { path: _, chunks: _ }
                        | Manifest::Symlink {
                            path: _,
                            target: _,
                            symlink_type: _,
                        } => panic!("what how"),
                    }
                }
                folder_indexes.insert(path, manifests.len() - 1);
            }
            ScanResult::Symlink(path, target, symlink_type) => {
                manifests.push(Manifest::Symlink {
                    path: path.clone(),
                    target,
                    symlink_type,
                });

                let dir = path.parent();
                if let Some(dir) = dir {
                    match &mut manifests[*folder_indexes
                        .get(&dir.to_path_buf())
                        .expect("the dir should always come first")]
                    {
                        Manifest::Directory {
                            path: dir_path,
                            content,
                        } => {
                            content.insert(
                                path.strip_prefix(dir_path)
                                    .expect("literally parent btw")
                                    .to_owned(),
                            );
                        }
                        Manifest::File { path: _, chunks: _ }
                        | Manifest::Symlink {
                            path: _,
                            target: _,
                            symlink_type: _,
                        } => panic!("what how"),
                    }
                }
            }
        }
    }
    Ok(manifests)
}
pub(crate) fn full_backup(
    passwd: &str,
    root: &Path,
    name: &str,
    exclude: &[&Path],
) -> Result<BackupId, Error> {
    dir_checkup()?;
    let mut new_exclude = Vec::new();
    let libak_path = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join("libak");
    new_exclude.extend_from_slice(exclude);
    new_exclude.push(&libak_path);
    let (key, salt) = pass_to_key(passwd, None)?;
    let manifests = backup(root, &key, &new_exclude, &libak_path.join("chunks"))?;
    let backup_id = random();
    write_manifests(&manifests, backup_id, &key)?;
    fs::create_dir(libak_path.join("meta").join(id_to_name(&backup_id)))?;
    let mut meta_file = File::create(
        libak_path
            .join("meta")
            .join(id_to_name(&backup_id))
            .join("meta"),
    )?;
    let metadata = BackupMetadata {
        time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| /* 1970 was a while back */ InvariantsFailed)?,
        name: name.to_string(),
        salt,
    };
    let size = postcard::serialize_with_flavor::<BackupMetadata, postcard::ser_flavors::Size, usize>(
        &metadata,
        postcard::ser_flavors::Size::default(),
    )?;
    let mut data = Vec::with_capacity(size);
    data.extend(std::iter::repeat_n(0, size));
    postcard::to_slice(&metadata, data.as_mut_slice())?;
    meta_file.write_all(&data)?;
    let mut passwd_check_file = File::create(
        dirs::data_local_dir()
            .ok_or(Error::NoDataDir)?
            .join("libak")
            .join("meta")
            .join(id_to_name(&backup_id))
            .join("test"),
    )?;
    let nonce_internal: [u8; 24] = random();
    let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
    let mut test = Vec::from(super::PASS_TEST_CONTENTS);
    cipher.encrypt_in_place(&XNonce::from(nonce_internal), &[], &mut test)?;
    passwd_check_file.write_all(&nonce_internal)?;
    passwd_check_file.write_all(&test)?;
    Ok(backup_id)
}
