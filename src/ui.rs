//! Binds the Slint window to a `Backend`. No backup logic lives here.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::backend::{Backend, BackupError, Event, Password, Preset};

slint::include_modules!();

#[derive(Clone)]
enum Pending {
    Backup,
    Export(PathBuf),
}

impl Pending {
    fn purpose(&self) -> &'static str {
        match self {
            Pending::Backup => "The password is needed to run the backup.",
            Pending::Export(_) => "The password is needed to export your backups.",
        }
    }
}

pub fn run(backend: Arc<dyn Backend>) -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let pending: Arc<Mutex<Option<Pending>>> = Arc::new(Mutex::new(None));

    refresh(&ui, &*backend);

    ui.on_request_backup({
        let ui = ui.as_weak();
        let pending = Arc::clone(&pending);
        move || {
            if let Some(ui) = ui.upgrade() {
                open_unlock(&ui, &pending, Pending::Backup);
            }
        }
    });

    ui.on_request_export({
        let ui = ui.as_weak();
        let pending = Arc::clone(&pending);
        let backend = Arc::clone(&backend);
        move || {
            let Some(ui) = ui.upgrade() else { return };
            if let Some(dest) = backend.pick_folder() {
                open_unlock(&ui, &pending, Pending::Export(dest));
            }
        }
    });

    ui.on_cancel_unlock({
        let ui = ui.as_weak();
        let pending = Arc::clone(&pending);
        move || {
            *pending.lock().unwrap() = None;
            if let Some(ui) = ui.upgrade() {
                ui.set_unlock_open(false);
                ui.set_unlock_error("".into());
            }
        }
    });

    ui.on_unlock({
        let weak = ui.as_weak();
        let pending = Arc::clone(&pending);
        let backend = Arc::clone(&backend);
        move |entered| {
            // The toolkit's own copy of the password cannot be wiped (PLAN.md section 4);
            // move it into a zeroizing buffer right away.
            let password = Password::new(entered.to_string());
            let Some(action) = pending.lock().unwrap().clone() else { return };
            let Some(ui) = weak.upgrade() else { return };
            ui.set_unlock_open(false);
            ui.set_unlock_error("".into());
            ui.set_busy(true);
            ui.set_progress(0.0);
            ui.set_message("".into());

            let weak = weak.clone();
            let pending = Arc::clone(&pending);
            let backend = Arc::clone(&backend);
            thread::spawn(move || {
                let report = {
                    let weak = weak.clone();
                    move |p: f32| {
                        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_progress(p));
                    }
                };
                let result = match &action {
                    Pending::Backup => backend.run_backup(password, &report),
                    Pending::Export(dest) => backend.run_export(password, dest.clone(), &report),
                };
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_busy(false);
                    match result {
                        Ok(()) => {
                            *pending.lock().unwrap() = None;
                            ui.set_message(match action {
                                Pending::Backup => "Backup finished.".into(),
                                Pending::Export(_) => "Export finished.".into(),
                            });
                        }
                        Err(BackupError::WrongPassword) => {
                            ui.set_unlock_error("Wrong password, try again.".into());
                            ui.set_unlock_open(true);
                        }
                        Err(BackupError::Failed(why)) => {
                            *pending.lock().unwrap() = None;
                            ui.set_message(format!("Failed: {why}").into());
                        }
                    }
                    refresh(&ui, &*backend);
                });
            });
        }
    });

    ui.on_toggle_pause({
        let ui = ui.as_weak();
        let backend = Arc::clone(&backend);
        move || {
            if backend.status().paused {
                backend.resume();
            } else {
                backend.pause();
            }
            if let Some(ui) = ui.upgrade() {
                refresh(&ui, &*backend);
            }
        }
    });

    ui.on_add_folder({
        let ui = ui.as_weak();
        let backend = Arc::clone(&backend);
        move || {
            if let Some(path) = backend.pick_folder() {
                backend.add_folder(path);
            }
            if let Some(ui) = ui.upgrade() {
                refresh(&ui, &*backend);
            }
        }
    });

    ui.on_remove_folder({
        let ui = ui.as_weak();
        let backend = Arc::clone(&backend);
        move |index| {
            if let Some(path) = backend.folders().get(index as usize) {
                backend.remove_folder(path);
            }
            if let Some(ui) = ui.upgrade() {
                refresh(&ui, &*backend);
            }
        }
    });

    ui.on_set_preset({
        let ui = ui.as_weak();
        let backend = Arc::clone(&backend);
        move |index, enabled| {
            if let Some(preset) = Preset::ALL.get(index as usize) {
                backend.set_preset(*preset, enabled);
            }
            if let Some(ui) = ui.upgrade() {
                refresh(&ui, &*backend);
            }
        }
    });

    // Daemon events arrive on a plain channel and are applied on the UI thread.
    let (tx, rx) = mpsc::channel();
    backend.subscribe(tx);
    {
        let weak = ui.as_weak();
        let backend = Arc::clone(&backend);
        thread::spawn(move || {
            for event in rx {
                let backend = Arc::clone(&backend);
                let result = weak.upgrade_in_event_loop(move |ui| {
                    if matches!(event, Event::BackupDue) && !ui.get_busy() {
                        ui.set_due_banner(true);
                    }
                    refresh(&ui, &*backend);
                });
                if result.is_err() {
                    break; // window closed
                }
            }
        });
    }

    ui.run()
}

fn open_unlock(ui: &AppWindow, pending: &Mutex<Option<Pending>>, action: Pending) {
    ui.set_unlock_purpose(action.purpose().into());
    ui.set_unlock_error("".into());
    *pending.lock().unwrap() = Some(action);
    ui.set_unlock_open(true);
}

fn refresh(ui: &AppWindow, backend: &dyn Backend) {
    let status = backend.status();
    let now = SystemTime::now();
    ui.set_last_backup(format_last_backup(status.last_backup, now).into());
    ui.set_next_due(format_next_due(status.next_due, now).into());
    ui.set_paused(status.paused);

    let folders: Vec<SharedString> = backend
        .folders()
        .iter()
        .map(|p| p.display().to_string().into())
        .collect();
    ui.set_folders(ModelRc::from(Rc::new(VecModel::from(folders))));

    let presets: Vec<PresetItem> = backend
        .presets()
        .into_iter()
        .map(|(preset, enabled)| PresetItem { label: preset.label().into(), enabled })
        .collect();
    ui.set_presets(ModelRc::from(Rc::new(VecModel::from(presets))));

    let summary = backend.exclusion_summary();
    ui.set_exclusion_summary(
        format!(
            "{} items ({}) are skipped and will not be backed up.",
            summary.items,
            format_bytes(summary.bytes)
        )
        .into(),
    );
}

fn format_last_backup(last: Option<SystemTime>, now: SystemTime) -> String {
    match last {
        None => "never".into(),
        Some(t) => match now.duration_since(t) {
            Ok(age) => format!("{} ago", format_span(age)),
            Err(_) => "just now".into(),
        },
    }
}

fn format_next_due(next: Option<SystemTime>, now: SystemTime) -> String {
    match next {
        None => "when the schedule next fires".into(),
        Some(t) => match t.duration_since(now) {
            Ok(left) => format!("in {}", format_span(left)),
            Err(_) => "due now".into(),
        },
    }
}

fn format_span(d: Duration) -> String {
    let secs = d.as_secs();
    let plural = |n: u64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    match secs {
        0..=59 => plural(secs, "second"),
        60..=3599 => plural(secs / 60, "minute"),
        3600..=86399 => plural(secs / 3600, "hour"),
        _ => plural(secs / 86400, "day"),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans() {
        assert_eq!(format_span(Duration::from_secs(1)), "1 second");
        assert_eq!(format_span(Duration::from_secs(120)), "2 minutes");
        assert_eq!(format_span(Duration::from_secs(3600)), "1 hour");
        assert_eq!(format_span(Duration::from_secs(3 * 86400)), "3 days");
    }

    #[test]
    fn bytes() {
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(3_400_000_000), "3.4 GB");
    }

    #[test]
    fn last_backup_and_next_due() {
        let now = SystemTime::now();
        assert_eq!(format_last_backup(None, now), "never");
        assert_eq!(format_last_backup(Some(now - Duration::from_secs(7200)), now), "2 hours ago");
        assert_eq!(format_next_due(Some(now - Duration::from_secs(5)), now), "due now");
    }
}
