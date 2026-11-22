use super::Error;
use crate::{
    Error::{CryptoError, InvariantsFailed},
    Key,
};
use aes_gcm::{AeadInOut, Aes256Gcm, KeyInit};
use fastcdc::v2020::StreamCDC;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
type Id = [u8; 32];
fn id_to_name(id: &Id) -> String {
    hex::encode(id)
}
fn name_to_id(name: &OsStr) -> Option<Id> {
    let id = hex::decode(name.as_encoded_bytes()).ok();
    if let Some(e) = id
        && e.len() == 32
    {
        return e.as_slice().try_into().ok();
    }
    None
}
#[derive(Zeroize, ZeroizeOnDrop)]
struct ProcessedChunk {
    nonce: [u8; 12],
    id: Id,
    data: Zeroizing<Vec<u8>>,
}
#[derive(Zeroize, ZeroizeOnDrop)]
struct ProcessedFile {
    chunks: Vec<ProcessedChunk>,
    manifest: Vec<Id>,
}
enum ScanResult {
    File(PathBuf),
    Folder(PathBuf),
    /// THE FIRST ONE IS THE SYMLINK'S PATH AND SECOND IS TARGET
    Symlink(PathBuf, PathBuf, SymlinkType),
}
#[derive(Clone)]
enum Manifest {
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
#[derive(Clone)]
enum SymlinkType {
    Folder,
    File,
}
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
fn scan(root: &Path, path: &Path, exclude: &[&Path]) -> Result<Vec<ScanResult>, Error> {
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
fn process_file(
    file: &File,
    key: &Key,
    store_hashes: &HashSet<[u8; 32]>,
) -> Result<ProcessedFile, Error> {
    let chunker = StreamCDC::new(file, 4096, 16384, 65536);
    let mut final_file = ProcessedFile {
        chunks: vec![],
        manifest: vec![],
    };
    let mut seen_chunks = HashSet::new();
    let cipher = Aes256Gcm::new(&aes_gcm::Key::<Aes256Gcm>::from(key.0));
    for chunk in chunker {
        let nonce_internal = rand::random::<[u8; 12]>();
        let nonce = aes_gcm::Nonce::from(nonce_internal);
        let data_copy = Zeroizing::new(chunk?.data);
        let id = *blake3::keyed_hash(&key.0, &data_copy).as_bytes();
        let is_new = !store_hashes.contains(&id) && !seen_chunks.contains(&id);
        final_file.manifest.push(id);
        if is_new {
            seen_chunks.insert(id);
            let mut compressed = Zeroizing::new(Vec::new());
            zstd::stream::compress_sized_into(
                data_copy.as_slice(),
                &mut *compressed,
                3,
                data_copy.len() as u64,
            )?;
            cipher.encrypt_in_place(&nonce, &id, &mut *compressed)?;
            final_file.chunks.push(ProcessedChunk {
                nonce: nonce_internal,
                id,
                data: compressed,
            });
        }
    }
    Ok(final_file)
}
fn backup(
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
fn restore(
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
                    let nonce_internal: [u8; 12] = chunk_data[0..12]
                        .try_into()
                        .map_err(|_| Error::InvariantsFailed)?;
                    let nonce = aes_gcm::Nonce::from(nonce_internal);
                    _ = chunk_data.drain(..12);
                    let cipher = Aes256Gcm::new(&aes_gcm::Key::<Aes256Gcm>::from(key.0));
                    cipher.decrypt_in_place(&nonce, chunk, &mut *chunk_data)?;
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    let decompressed_chunk = zstd::bulk::decompress(chunk_data.as_slice(), 65536)?;
                    target_file.write_all(&decompressed_chunk)?;
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
    for dir in &mut to_check {
        match dir {
            Manifest::File { path: _, chunks: _ }
            | Manifest::Symlink {
                path: _,
                target: _,
                symlink_type: _,
            } => panic!("What"),
            Manifest::Directory { path, content } => {
                let num_entries = content.len();
                let mut num_files = 0;
                let dir_entries = fs::read_dir(target_dir.join(&path))?;
                for i in dir_entries {
                    num_files += 1;
                    content.remove(&PathBuf::from(
                        i?.path().file_name().expect("how would ReadDir return .."),
                    ));
                }
                if (!content.is_empty()) || (num_entries != num_files) {
                    // TAMPERING
                    return Err(CryptoError);
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
