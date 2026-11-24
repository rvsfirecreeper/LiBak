use argon2::Params;
use rand::random;

use crate::Key;
const V1_MEM_COST: u32 = 32768;
const V1_PAR_COST: u32 = 1;
const V1_ITER_COST: u32 = 2;
pub fn pass_to_key(pass: &str, salt: Option<[u8; 16]>) -> Result<(Key, [u8; 16]), crate::Error> {
    let params = Params::new(V1_MEM_COST, V1_ITER_COST, V1_PAR_COST, Some(32))?;
    let hasher = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let salt = salt.unwrap_or(random::<[u8; 16]>());
    let mut key = Key::create_from_slice(&[0u8; 32]);
    hasher.hash_password_into(pass.as_bytes(), &salt, &mut key.0)?;
    Ok((key, salt))
}
