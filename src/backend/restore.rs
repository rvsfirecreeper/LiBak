use super::types::{Manifest, id_to_name};
#[cfg(windows)]
use super::types::SymlinkType;
use crate::{Error, Error::CryptoError, Key};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use std::{
    fs::{self, File},
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
                    let nonce_internal: [u8; 24] = chunk_data[0..24]
                        .try_into()
                        .map_err(|_| Error::InvariantsFailed)?;
                    let nonce = XNonce::from(nonce_internal);
                    _ = chunk_data.drain(..24);
                    let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
                    cipher.decrypt_in_place(&nonce, chunk, &mut *chunk_data)?;
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    let mut decompressed_chunk =
                        zstd::bulk::decompress(chunk_data.as_slice(), 65536)?;
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
