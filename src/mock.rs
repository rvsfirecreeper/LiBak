//! In-memory backend so the UI can run before the daemon and library exist.
//! The password "wrong" is rejected, to exercise the error path.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::backend::{
    Backend, BackupError, Event, ExclusionSummary, Password, Preset, Status,
};

const DUE_FIRST: Duration = Duration::from_secs(10);
const DUE_EVERY: Duration = Duration::from_secs(60);

struct State {
    status: Status,
    folders: Vec<PathBuf>,
    presets: Vec<(Preset, bool)>,
    subscribers: Vec<Sender<Event>>,
    picked: u32,
}

pub struct MockBackend {
    state: Arc<Mutex<State>>,
}

impl MockBackend {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let state = Arc::new(Mutex::new(State {
            status: Status::default(),
            folders: vec![home.join("Documents")],
            presets: Preset::ALL.iter().map(|&p| (p, p != Preset::Gaming)).collect(),
            subscribers: Vec::new(),
            picked: 0,
        }));

        let ticker = Arc::clone(&state);
        thread::spawn(move || {
            thread::sleep(DUE_FIRST);
            loop {
                {
                    let mut s = ticker.lock().unwrap();
                    if !s.status.paused {
                        s.subscribers.retain(|tx| tx.send(Event::BackupDue).is_ok());
                    }
                }
                thread::sleep(DUE_EVERY);
            }
        });

        Self { state }
    }

    fn notify(&self) {
        let mut s = self.state.lock().unwrap();
        s.subscribers.retain(|tx| tx.send(Event::StatusChanged).is_ok());
    }

    fn fake_run(&self, password: &Password, progress: &dyn Fn(f32)) -> Result<(), BackupError> {
        if password.expose() == "wrong" {
            return Err(BackupError::WrongPassword);
        }
        const STEPS: u32 = 20;
        for i in 1..=STEPS {
            thread::sleep(Duration::from_millis(100));
            progress(i as f32 / STEPS as f32);
        }
        Ok(())
    }
}

impl Backend for MockBackend {
    fn status(&self) -> Status {
        self.state.lock().unwrap().status.clone()
    }

    fn pause(&self) {
        self.state.lock().unwrap().status.paused = true;
        self.notify();
    }

    fn resume(&self) {
        self.state.lock().unwrap().status.paused = false;
        self.notify();
    }

    fn folders(&self) -> Vec<PathBuf> {
        self.state.lock().unwrap().folders.clone()
    }

    fn add_folder(&self, path: PathBuf) {
        let mut s = self.state.lock().unwrap();
        if !s.folders.contains(&path) {
            s.folders.push(path);
        }
    }

    fn remove_folder(&self, path: &Path) {
        self.state.lock().unwrap().folders.retain(|f| f != path);
    }

    fn pick_folder(&self) -> Option<PathBuf> {
        let mut s = self.state.lock().unwrap();
        s.picked += 1;
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Some(home.join(format!("Mock-folder-{}", s.picked)))
    }

    fn presets(&self) -> Vec<(Preset, bool)> {
        self.state.lock().unwrap().presets.clone()
    }

    fn set_preset(&self, preset: Preset, enabled: bool) {
        let mut s = self.state.lock().unwrap();
        if let Some(entry) = s.presets.iter_mut().find(|(p, _)| *p == preset) {
            entry.1 = enabled;
        }
    }

    fn exclusion_summary(&self) -> ExclusionSummary {
        let s = self.state.lock().unwrap();
        let on = s.presets.iter().filter(|(_, e)| *e).count() as u64;
        ExclusionSummary { items: on * 1_200, bytes: on * 3_400_000_000 }
    }

    fn subscribe(&self, events: Sender<Event>) {
        self.state.lock().unwrap().subscribers.push(events);
    }

    fn run_backup(&self, password: Password, progress: &dyn Fn(f32)) -> Result<(), BackupError> {
        self.fake_run(&password, progress)?;
        {
            let mut s = self.state.lock().unwrap();
            let now = SystemTime::now();
            s.status.last_backup = Some(now);
            s.status.next_due = Some(now + DUE_EVERY);
        }
        self.notify();
        Ok(())
    }

    fn run_export(
        &self,
        password: Password,
        _destination: PathBuf,
        progress: &dyn Fn(f32),
    ) -> Result<(), BackupError> {
        self.fake_run(&password, progress)
    }
}
