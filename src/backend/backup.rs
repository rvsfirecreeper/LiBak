use super::{
    chunking::process_file,
    scan::scan,
    types::{Manifest, ScanResult, id_to_name, name_to_id},
};
use crate::{Error, Key};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
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
