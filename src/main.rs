#![warn(clippy::pedantic)]
// The stupidest clippy lint, mixed in with some of the best.
#![allow(clippy::too_many_lines)]
use zeroize::{Zeroize, ZeroizeOnDrop};
slint::include_modules!();
mod backend;
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Key([u8; 32]);
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
    /// Why do you not have a home directory
    Homeless,
    /// Postcard serialization error
    Postcard(postcard::Error),
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
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
    AppWindow::new()?.run()
}
