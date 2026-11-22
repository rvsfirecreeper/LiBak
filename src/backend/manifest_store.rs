use super::types::{Manifest, id_to_name};
use crate::{BackupId, Error, Key};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use rand::random;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    str::FromStr,
};
use zeroize::Zeroizing;
pub(super) fn write_manifests(manifests: &[Manifest], backup_id: BackupId, key: &Key) -> Result<(), Error> {
    dir_checkup()?;
    let bak_id_hex = id_to_name(&backup_id);
    let manifest_dir = dirs::data_local_dir()
        .ok_or(Error::Homeless)?
        .join(PathBuf::from_str("libak/manifests").expect("Infallible"))
        .join(bak_id_hex);
    fs::create_dir(&manifest_dir)?;
    for manifest in manifests {
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
        let mut manifile = File::create(manifest_dir.join(id_to_name(&id)))?;
        let size = postcard::serialize_with_flavor::<Manifest, postcard::ser_flavors::Size, usize>(
            manifest,
            postcard::ser_flavors::Size::default(),
        )? + 16;
        let mut serialized_manifest = Zeroizing::new(Vec::with_capacity(size));
        let nonce_internal: [u8; 24] = random();
        for _ in 16..size {
            serialized_manifest.push(0u8);
        }
        postcard::to_slice(manifest, &mut serialized_manifest)?;
        let cipher = XChaCha20Poly1305::new(&chacha20poly1305::Key::from(key.0));
        cipher.encrypt_in_place(
            &XNonce::from(nonce_internal),
            &id,
            &mut *serialized_manifest,
        )?;
        manifile.write_all(&nonce_internal)?;
        manifile.write_all(&serialized_manifest)?;
    }
    Ok(())
}
pub(super) fn dir_checkup() -> Result<(), Error> {
    let manifest_dir = dirs::data_local_dir()
        .ok_or(Error::Homeless)?
        .join(PathBuf::from_str("libak/manifests").expect("Infallible"));
    if !fs::exists(&manifest_dir)? {
        fs::create_dir_all(manifest_dir)?;
    }
    let chunk_dir = dirs::data_local_dir()
        .ok_or(Error::Homeless)?
        .join(PathBuf::from_str("libak/chunks").expect("Infallible"));
    if !fs::exists(&chunk_dir)? {
        fs::create_dir_all(chunk_dir)?;
    }
    Ok(())
}
