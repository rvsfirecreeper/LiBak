#![warn(clippy::pedantic)]
slint::include_modules!();
mod backend;
pub enum Error {
    /// Random Error in IO
    IoError(std::io::Error),
    /// The file or folder simply does not exist.
    NonExistent,
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}
