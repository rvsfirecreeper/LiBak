//! Optional user theme: `<config dir>/libak/theme.conf` (on Linux `~/.config/libak/theme.conf`).
//!
//! One `key = value` per line. Lines starting with `#` are comments (so a colour can
//! still be written as `#rrggbb`). Colours are `#rgb`, `#rrggbb` or `#rrggbbaa`.
//! Every key is optional; anything missing keeps the built-in default.
//!
//! ```text
//! # ~/.config/libak/theme.conf
//! accent      = #f0a830
//! background  = #14161a
//! panel       = #1b1e23
//! line        = #2a2e35
//! text        = #e7e9ee
//! muted       = #8b919c
//! success     = #7bd88f
//! error       = #ff7b72
//! # Text drawn on top of the accent colour (primary buttons, selections).
//! # If you leave it out, black or white is picked so it stays readable.
//! accent-text = #101114
//! ```

use crate::{AppWindow, Theme};
use slint::{Color, ComponentHandle};
use std::{fs, io, path::PathBuf};

fn theme_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("libak").join("theme.conf"))
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn parse_colour(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#')?;
    if !hex.is_ascii() {
        return None;
    }
    let byte = |s: &str| u8::from_str_radix(s, 16).ok();
    let nibble = |i: usize| byte(&hex[i..=i]).map(|n| n * 17);
    match hex.len() {
        3 => Some(Color::from_rgb_u8(nibble(0)?, nibble(1)?, nibble(2)?)),
        6 => Some(Color::from_rgb_u8(
            byte(&hex[0..2])?,
            byte(&hex[2..4])?,
            byte(&hex[4..6])?,
        )),
        8 => Some(Color::from_argb_u8(
            byte(&hex[6..8])?,
            byte(&hex[0..2])?,
            byte(&hex[2..4])?,
            byte(&hex[4..6])?,
        )),
        _ => None,
    }
}

/// Black or white, whichever is readable on `background`.
fn readable_on(background: Color) -> Color {
    let luma = 0.299 * f32::from(background.red())
        + 0.587 * f32::from(background.green())
        + 0.114 * f32::from(background.blue());
    if luma > 150.0 {
        Color::from_rgb_u8(0x10, 0x11, 0x14)
    } else {
        Color::from_rgb_u8(0xff, 0xff, 0xff)
    }
}

/// Applies the theme file (if there is one) to the window. Returns a problem
/// description for the UI to show; a missing file is not a problem.
pub fn apply(ui: &AppWindow) -> Option<String> {
    let path = theme_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => return Some(format!("Couldn't read theme.conf: {e}")),
    };

    let theme = ui.global::<Theme>();
    let mut problems = Vec::new();
    let mut accent_text_given = false;
    let mut accent = None;
    for (number, line) in contents.lines().enumerate().map(|(i, l)| (i + 1, l.trim())) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!("line {number}: expected \"key = #colour\""));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(colour) = parse_colour(value) else {
            problems.push(format!("line {number}: \"{value}\" is not a colour like #rrggbb"));
            continue;
        };
        match key {
            "accent" => {
                theme.set_accent(colour);
                accent = Some(colour);
            }
            "accent-text" => {
                theme.set_accent_text(colour);
                accent_text_given = true;
            }
            "background" => theme.set_page(colour),
            "panel" => theme.set_panel(colour),
            "line" => theme.set_line(colour),
            "text" => theme.set_text(colour),
            "muted" => theme.set_muted(colour),
            "success" => theme.set_ok(colour),
            "error" => theme.set_err(colour),
            other => problems.push(format!("line {number}: unknown key \"{other}\"")),
        }
    }
    if let (Some(accent), false) = (accent, accent_text_given) {
        theme.set_accent_text(readable_on(accent));
    }

    match problems.as_slice() {
        [] => None,
        [only] => Some(format!("theme.conf, {only}")),
        [first, rest @ ..] => Some(format!(
            "theme.conf, {first} (and {} more problems)",
            rest.len()
        )),
    }
}
