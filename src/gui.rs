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

use crate::{
    AppWindow, BackupItem, Error,
    backend::{BackupSummary, full_backup, full_restore, list_backups},
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    path::{Component, Path, PathBuf},
    rc::Rc,
    thread,
};
use zeroize::Zeroizing;

/// Turns what the user typed into a path, expanding a leading `~` (the shell does that
/// normally, a text field doesn't).
fn expand_path(typed: &str) -> PathBuf {
    let typed = typed.trim();
    if typed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(typed));
    }
    if let Some(rest) = typed.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(typed)
}

/// Resolves the typed exclusions against the backup root. Relative entries count as
/// relative to the root. The scanner matches exact paths, so anything that isn't
/// inside the root (or contains `..`) could never match and is reported instead of
/// being silently ignored.
fn resolve_exclusions(root: &Path, typed: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut resolved = Vec::new();
    for entry in typed {
        let expanded = expand_path(entry);
        if expanded.components().any(|c| c == Component::ParentDir) {
            return Err(format!("\"{entry}\" contains \"..\", please type the full path."));
        }
        let full = if expanded.is_absolute() {
            expanded
        } else {
            root.join(expanded)
        };
        if full == root || !full.starts_with(root) {
            return Err(format!(
                "\"{entry}\" is not inside the folder being backed up."
            ));
        }
        resolved.push(full);
    }
    Ok(resolved)
}

/// Turns an error into something a non-technical person can act on.
fn describe(e: &Error) -> String {
    match e {
        Error::InvalidPassword => "Wrong password.".into(),
        Error::CryptoError => {
            "This backup is damaged or has been tampered with, and can't be trusted.".into()
        }
        Error::TargetNotEmpty => "The restore folder must be empty.".into(),
        Error::NonExistent => "That file or folder doesn't exist.".into(),
        Error::NoDataDir => "Couldn't find a place to store backups on this system.".into(),
        Error::Io(io) => format!("File error: {io}"),
        Error::Argon2(_) => "Couldn't derive a key from the password.".into(),
        Error::Postcard(_)
        | Error::ContentDefinedChunking(_)
        | Error::MysteriousStringConversionError
        | Error::InvalidHash
        | Error::InvariantsFailed => "Something went wrong internally.".into(),
    }
}

/// `YYYY-MM-DD HH:MM` in UTC from a Unix timestamp, no date crate needed
/// (Howard Hinnant's civil-from-days).
fn format_time(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

fn short_id(id: &OsStr) -> String {
    id.to_string_lossy().chars().take(8).collect()
}

fn to_items(backups: &[BackupSummary]) -> Vec<BackupItem> {
    backups
        .iter()
        .map(|b| BackupItem {
            name: b.metadata.name.as_str().into(),
            time: format_time(b.metadata.time.as_secs()).into(),
            id: short_id(&b.id).into(),
        })
        .collect()
}

fn show_message(ui: &AppWindow, text: impl Into<SharedString>, is_error: bool) {
    ui.set_message(text.into());
    ui.set_message_is_error(is_error);
}

thread_local! {
    /// Backup directory names parallel to the rows shown in the UI, so a click on row N
    /// maps to the right directory. Only ever touched on the UI thread.
    static IDS: RefCell<Vec<OsString>> = const { RefCell::new(Vec::new()) };
}

fn refresh(ui: &AppWindow) {
    match list_backups() {
        Ok(list) => {
            ui.set_backups(ModelRc::new(VecModel::from(to_items(&list))));
            IDS.with_borrow_mut(|ids| *ids = list.into_iter().map(|b| b.id).collect());
            ui.set_selected(-1);
        }
        Err(e) => show_message(ui, describe(&e), true),
    }
}

pub fn run() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    if let Some(problem) = crate::theme::apply(&ui) {
        show_message(&ui, problem, true);
    }
    refresh(&ui);
    ui.on_refresh_backups({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                refresh(&ui);
            }
        }
    });

    let exclusions: Rc<VecModel<SharedString>> = Rc::new(VecModel::default());
    ui.set_exclusions(ModelRc::from(exclusions.clone()));
    ui.on_add_exclusion({
        let exclusions = exclusions.clone();
        move |typed| {
            let typed = typed.trim();
            if !typed.is_empty() && !exclusions.iter().any(|e| e == typed) {
                exclusions.push(typed.into());
            }
        }
    });
    ui.on_remove_exclusion({
        let exclusions = exclusions.clone();
        move |i| {
            if let Ok(i) = usize::try_from(i)
                && i < exclusions.row_count()
            {
                exclusions.remove(i);
            }
        }
    });

    ui.on_start_backup({
        let weak = ui.as_weak();
        let exclusions = exclusions.clone();
        move |source, name, password| {
            let Some(ui) = weak.upgrade() else { return };
            let password = Zeroizing::new(password.to_string());
            let source = expand_path(source.as_str());
            if !source.is_dir() {
                show_message(&ui, "That folder doesn't exist.", true);
                return;
            }
            let typed: Vec<String> = exclusions.iter().map(|e| e.to_string()).collect();
            let excluded = match resolve_exclusions(&source, &typed) {
                Ok(e) => e,
                Err(msg) => {
                    show_message(&ui, msg, true);
                    return;
                }
            };
            let name = name.to_string();
            ui.set_busy(true);
            show_message(&ui, "", false);
            let weak = weak.clone();
            thread::spawn(move || {
                let excluded_refs: Vec<&Path> = excluded.iter().map(PathBuf::as_path).collect();
                let result = full_backup(&password, &source, &name, &excluded_refs);
                drop(password);
                _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_busy(false);
                    match result {
                        Ok(_) => {
                            refresh(&ui);
                            show_message(&ui, "Backup finished.", false);
                        }
                        Err(e) => show_message(&ui, describe(&e), true),
                    }
                });
            });
        }
    });

    ui.on_start_restore({
        let weak = ui.as_weak();
        move |index, target, password| {
            let Some(ui) = weak.upgrade() else { return };
            let password = Zeroizing::new(password.to_string());
            let Some(id) = usize::try_from(index)
                .ok()
                .and_then(|i| IDS.with_borrow(|ids| ids.get(i).cloned()))
            else {
                show_message(&ui, "Pick a backup first.", true);
                return;
            };
            let target = expand_path(target.as_str());
            ui.set_busy(true);
            show_message(&ui, "", false);
            let weak = weak.clone();
            thread::spawn(move || {
                let result = full_restore(&password, &id, &target);
                drop(password);
                _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_busy(false);
                    match result {
                        Ok(()) => show_message(&ui, "Restore finished.", false),
                        Err(e) => show_message(&ui, describe(&e), true),
                    }
                });
            });
        }
    });

    ui.run()
}
