use super::Error;
use crate::Key;
use aes_gcm::{AeadInOut, Aes256Gcm, KeyInit, Nonce};
use fastcdc::v2020::StreamCDC;
use std::{
    fs::{self, File},
    path::Path,
};
type Id = [u8; 32];
#[derive(Zeroize, ZeroizeOnDrop)]
struct ProcessedChunk {
    id: Id,
    data: Zeroizing<Vec<u8>>,
}
#[derive(Zeroize, ZeroizeOnDrop)]
struct ProcessedFile {
    chunks: Vec<ProcessedChunk>,
    manifest: Vec<Id>,
}
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
fn scan(path: &Path, exclude: &[&Path]) -> Result<Vec<File>, Error> {
    if !path.try_exists()? {
        return Err(Error::NonExistent);
    }
    let mut files = vec![];
    if path.is_dir() {
        for dir_entry in fs::read_dir(path)? {
            let dir_entry = dir_entry?;
            if !exclude.iter().any(|i| *i == dir_entry.path().as_path()) {
                files.push(File::open(dir_entry.path())?);
            }
        }
    } else if !exclude.contains(&path) {
        files.push(File::open(path)?);
    }

    Ok(files)
}
fn process_file(file: &File, key: &Key, store_hashes: &[[u8; 32]]) -> Result<ProcessedFile, Error> {
    let chunker = StreamCDC::new(file, 4096, 16384, 65534);
    let mut final_file = ProcessedFile {
        chunks: vec![],
        manifest: vec![],
    };
    let cipher = Aes256Gcm::new(&aes_gcm::Key::<Aes256Gcm>::from(key.0));
    for chunk in chunker {
        let nonce = Nonce::from(rand::random::<[u8; 12]>());
        let data_copy = Zeroizing::new(chunk?.data);
        let id = *blake3::keyed_hash(&key.0, &data_copy).as_bytes();
        let is_new = !store_hashes.contains(&id) && !final_file.manifest.contains(&id);
        final_file.manifest.push(id);
        if is_new {
            let mut compressed = Zeroizing::new(Vec::new());
            zstd::stream::compress_sized_into(
                data_copy.as_slice(),
                &mut *compressed,
                3,
                data_copy.len() as u64,
            )?;
            cipher.encrypt_in_place(&nonce, &id, &mut *compressed)?;
            final_file.chunks.push(ProcessedChunk {
                id,
                data: compressed,
            });
        }
    }
    Ok(final_file)
}
