#![warn(clippy::pedantic)]

use zeroize::{Zeroize, ZeroizeOnDrop};
slint::include_modules!();
mod backend;
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Key([u8; 32]);
pub enum Error {
    /// Random Error in IO
    Io(std::io::Error),
    /// The file or folder simply does not exist.
    NonExistent,
    /// Error in FastCDC StreamCDC
    ContentDefinedChunking(fastcdc::v2020::Error),
    /// Opaque Cryptographic or Otherwise Error
    CryptoError,
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<fastcdc::v2020::Error> for Error {
    fn from(value: fastcdc::v2020::Error) -> Self {
        Self::ContentDefinedChunking(value)
    }
}
impl From<aes_gcm::Error> for Error {
    fn from(_: aes_gcm::Error) -> Self {
        Self::CryptoError
    }
}
fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}
