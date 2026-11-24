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
