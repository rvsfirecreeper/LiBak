use super::types::{Manifest, id_to_name};
use crate::{BackupId, Error, Key, backend::types::name_to_id};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use rand::random;
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};
use zeroize::Zeroizing;
pub(super) fn write_manifests(
    manifests: &[Manifest],
    backup_id: BackupId,
    key: &Key,
) -> Result<(), Error> {
    dir_checkup()?;
    let bak_id_hex = id_to_name(&backup_id);
    let manifest_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join(PathBuf::from_str("libak/manifests").expect("Infallible"))
        .join(bak_id_hex);
    fs::create_dir(&manifest_dir)?;
    for (i, manifest) in manifests.iter().enumerate() {
        let id = *blake3::keyed_hash(
            &key.0,
            match manifest {
                Manifest::Directory { path, content: _ }
                | Manifest::File { path, chunks: _ }
                | Manifest::Symlink {
                    path,
                    target: _,
                    symlink_type: _,
                } => path.as_os_str().as_encoded_bytes(),
            },
        )
        .as_bytes();
        let size = postcard::serialize_with_flavor::<Manifest, postcard::ser_flavors::Size, usize>(
            manifest,
            postcard::ser_flavors::Size::default(),
        )? + 16;
        let mut aad = Vec::new();
        aad.extend_from_slice(&id);
        aad.extend_from_slice(&backup_id);
        aad.extend_from_slice(&(i as u64).to_le_bytes());
        let mut serialized_manifest = Zeroizing::new(Vec::with_capacity(size));
        let nonce_internal: [u8; 24] = random();
        serialized_manifest.extend(std::iter::repeat_n(0, size - 16));
        postcard::to_slice(manifest, &mut serialized_manifest)?;
        let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
        cipher.encrypt_in_place(
            &XNonce::from(nonce_internal),
            &aad,
            &mut *serialized_manifest,
        )?;
        let mut manifile = File::create(manifest_dir.join(id_to_name(&id)))?;
        manifile.write_all(&(i as u64).to_le_bytes())?;
        manifile.write_all(&nonce_internal)?;
        manifile.write_all(&serialized_manifest)?;
    }
    Ok(())
}
pub(super) fn read_manifests(backup_id: &OsStr, key: &Key) -> Result<Vec<Manifest>, Error> {
    let manifest_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join(PathBuf::from_str("libak/manifests").expect("Infallible"))
        .join(backup_id);
    let mut manifests_indexed = Vec::new();
    for manifest in fs::read_dir(manifest_dir)? {
        let manifest = manifest?;
        let mut manifile = File::open(manifest.path())?;
        let mut index_bytes = [0u8; 8];
        manifile.read_exact(&mut index_bytes)?;
        let index = u64::from_le_bytes(index_bytes);
        let mut aad = Vec::new();
        aad.extend_from_slice(
            &name_to_id(manifest.path().file_name().ok_or(Error::InvariantsFailed)?)
                .ok_or(Error::InvalidHash)?,
        );
        aad.extend_from_slice(&name_to_id(backup_id).ok_or(Error::InvalidHash)?);
        aad.extend_from_slice(&index_bytes);
        let mut nonce_internal = [0u8; 24];
        manifile.read_exact(&mut nonce_internal)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let mut data_vec = Zeroizing::new(Vec::with_capacity(manifile.metadata()?.len() as usize));
        manifile.read_to_end(&mut data_vec)?;
        let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
        cipher.decrypt_in_place(&XNonce::from(nonce_internal), &aad, &mut *data_vec)?;
        manifests_indexed.push((index, postcard::from_bytes(&data_vec)?));
    }
    manifests_indexed.sort_by_key(|v| v.0);
    let mut manifests = Vec::with_capacity(manifests_indexed.len());
    for v in manifests_indexed {
        manifests.push(v.1);
    }
    Ok(manifests)
}
pub(super) fn dir_checkup() -> Result<(), Error> {
    let manifest_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join(PathBuf::from_str("libak/manifests").expect("Infallible"));
    if !fs::exists(&manifest_dir)? {
        fs::create_dir_all(manifest_dir)?;
    }
    let chunk_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join(PathBuf::from_str("libak/chunks").expect("Infallible"));
    if !fs::exists(&chunk_dir)? {
        fs::create_dir_all(chunk_dir)?;
    }
    let meta_dir = dirs::data_local_dir()
        .ok_or(Error::NoDataDir)?
        .join(PathBuf::from_str("libak/meta").expect("Infallible"));
    if !fs::exists(&meta_dir)? {
        fs::create_dir_all(meta_dir)?;
    }
    Ok(())
}
