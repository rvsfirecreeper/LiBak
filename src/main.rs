#![warn(clippy::pedantic)]
// The stupidest clippy lint, mixed in with some of the best.
#![allow(clippy::too_many_lines)]
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};
slint::include_modules!();
mod backend;
mod gui;
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Key([u8; 32]);
#[derive(Serialize, Deserialize)]
pub struct BackupMetadata {
    name: String,
    salt: [u8; 16],
    time: Duration,
}
impl Key {
    fn create_from_slice(slice: &[u8; 32]) -> Self {
        Self(*slice)
    }
}
pub type BackupId = [u8; 32];
#[cfg_attr(test, derive(Debug))]
pub enum Error {
    /// Random Error in IO
    Io(std::io::Error),
    /// The file or folder simply does not exist.
    NonExistent,
    /// Error in `FastCDC`'s `StreamCDC`
    ContentDefinedChunking(fastcdc::v2020::Error),
    /// Opaque Cryptographic or Otherwise Error
    CryptoError,
    /// Mysterious String Conversion Error
    MysteriousStringConversionError,
    /// why is there something in the store that has an invalid hash
    InvalidHash,
    /// What how did you get this why are you reading this anyway this is impossible(should be)
    InvariantsFailed,
    /// Why do you not have a local data directory
    NoDataDir,
    /// Postcard serialization error
    Postcard(postcard::Error),
    InvalidPassword,
    /// avoids hard failing with scary tampering warnings on some data
    TargetNotEmpty,
    /// Argon2 Error
    // PHOSPHOROUS theen Sulfur Chlorine and Argon POTASSIUM calcium so y'all grow strong SCANDIUM Titanium Vanadium and Chromium and Manganeeeeeeeeese
    // This is the periodic table noble gases stable Halogens and Alkali react aggressively each period will see new outer shells while electrons are added moving to the right
    Argon2(argon2::Error),
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<argon2::Error> for Error {
    fn from(value: argon2::Error) -> Self {
        Self::Argon2(value)
    }
}
impl From<postcard::Error> for Error {
    fn from(value: postcard::Error) -> Self {
        Self::Postcard(value)
    }
}
impl From<fastcdc::v2020::Error> for Error {
    fn from(value: fastcdc::v2020::Error) -> Self {
        Self::ContentDefinedChunking(value)
    }
}
impl From<chacha20poly1305::Error> for Error {
    fn from(_: chacha20poly1305::Error) -> Self {
        Self::CryptoError
    }
}
fn main() -> Result<(), slint::PlatformError> {
    gui::run()
}
