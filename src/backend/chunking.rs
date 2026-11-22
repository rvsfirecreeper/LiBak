use super::types::{ProcessedChunk, ProcessedFile};
use crate::{Error, Key};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use fastcdc::v2020::StreamCDC;
use std::{collections::HashSet, fs::File};
use zeroize::Zeroizing;
pub(super) fn process_file(
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
    let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
    for chunk in chunker {
        let nonce_internal = rand::random::<[u8; 24]>();
        let nonce = XNonce::from(nonce_internal);
        let data_copy = Zeroizing::new(chunk?.data);
        let id = *blake3::keyed_hash(&key.0, &data_copy).as_bytes();
        let is_new = !store_hashes.contains(&id) && !seen_chunks.contains(&id);
        final_file.manifest.push(id);
        if is_new {
            seen_chunks.insert(id);
            let mut compressed = Zeroizing::new(Vec::with_capacity(data_copy.len() + 304));
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
