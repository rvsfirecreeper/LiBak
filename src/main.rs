//! The UI's entire view of the backend: the `Backend` trait is the daemon's interface.
//!
//! The UI never touches the core library. The daemon owns all of that and the UI only
//! sends requests and receives events. How requests are carried (socket protocol, how the
//! password reaches the daemon) is up to the implementation of this trait.
//!
//! Getters are called on the UI thread, so implementations must answer from a cache
//! (a daemon client should update it from socket events, not query on each call).
//!
//! Module tree: this root owns the contract and wires `mock` into `ui`. Both are leaf
//! children that reach only their parent (`super::`), never each other.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

use zeroize::Zeroizing;

mod mock;
mod ui;

/// A password that is wiped on drop. Deliberately not `Clone` or `Debug`.
pub struct Password(Zeroizing<String>);

impl Password {
    pub fn new(password: String) -> Self {
        Self(Zeroizing::new(password))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub last_backup: Option<SystemTime>,
    pub next_due: Option<SystemTime>,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Developer,
    Gaming,
    SystemJunk,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Developer, Preset::Gaming, Preset::SystemJunk];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Developer => "Developer files (target/, node_modules/)",
            Preset::Gaming => "Game libraries and shader caches",
            Preset::SystemJunk => "Caches, trash and temporary files",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExclusionSummary {
    pub items: u64,
    pub bytes: u64,
}

// `Failed` is produced by real backends, not the mock.
#[allow(dead_code)]
#[derive(Debug)]
pub enum BackupError {
    WrongPassword,
    Failed(String),
}

/// Messages pushed from the daemon (or the mock) to the UI.
#[derive(Clone, Debug)]
pub enum Event {
    BackupDue,
    StatusChanged,
}

pub trait Backend: Send + Sync {
    fn status(&self) -> Status;
    fn pause(&self);
    fn resume(&self);
    fn folders(&self) -> Vec<PathBuf>;
    fn add_folder(&self, path: PathBuf);
    fn remove_folder(&self, path: &Path);
    /// Ask the user for a folder. May block; called on the UI thread.
    fn pick_folder(&self) -> Option<PathBuf>;
    fn presets(&self) -> Vec<(Preset, bool)>;
    fn set_preset(&self, preset: Preset, enabled: bool);
    fn exclusion_summary(&self) -> ExclusionSummary;
    /// Register a receiver for daemon events.
    fn subscribe(&self, events: Sender<Event>);

    // These take the password and block until the daemon reports the result. Called on a
    // worker thread; `progress` is 0.0..=1.0.
    fn run_backup(&self, password: Password, progress: &dyn Fn(f32)) -> Result<(), BackupError>;
    fn run_export(
        &self,
        password: Password,
        destination: PathBuf,
        progress: &dyn Fn(f32),
    ) -> Result<(), BackupError>;
}

fn main() -> Result<(), slint::PlatformError> {
    // Swap this for a daemon client implementing `Backend` once the daemon exists.
    let backend = Arc::new(mock::MockBackend::new());
    ui::run(backend)
}
