mod backend;
mod mock;
mod ui;

use std::sync::Arc;

fn main() -> Result<(), slint::PlatformError> {
    // Swap this for a daemon client implementing `Backend` once the daemon exists.
    let backend = Arc::new(mock::MockBackend::new());
    ui::run(backend)
}
