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
    let chunker = StreamCDC::new(file, 1024 * 512, 1024 * 1024 * 2, 1024 * 1024 * 8);
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
            let mut compressed = Zeroizing::new(Vec::with_capacity(data_copy.len() + 32768 + 16));
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
