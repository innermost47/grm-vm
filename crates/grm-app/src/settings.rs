// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What the human chose for the window, kept between sessions (a small JSON file in the user's config folder).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::text::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub theme: Theme,
    /// The size of everything (1: as it comes).
    pub text: f32,
    pub voice_on: bool,
    pub voice: String,
    pub voice_level: f64,
    pub volume: f64,
    /// The machine's colours (its apps inherit them).
    pub palette: crate::style::Palette,
    /// A typewriter font everywhere.
    pub typewriter: bool,
    /// The guided visit was seen (it comes by itself the first time only).
    pub toured: bool,
    /// Where the code stands beside an app.
    pub dock: crate::app::Dock,
    /// Apps' looks follow the hour (warmer in the evening).
    pub follow_hour: bool,
    /// Living colours: a slow breath through them (and the sound lifts them a little).
    pub breathe: bool,
    /// Who the machine speaks of (never to): empty, the host's user name (else "the human").
    pub human: String,
    /// What the MIDI controllers drive (learned).
    pub midi: Vec<crate::midi::Binding>,
}

impl Default for Settings {
    fn default() -> Self {
        let lang =
            std::env::var("LANG").map(|l| if l.starts_with("fr") { Lang::Fr } else { Lang::En }).unwrap_or(Lang::En);
        Settings {
            lang,
            theme: Theme::System,
            text: 1.0,
            voice_on: false,
            voice: "pad".into(),
            voice_level: 0.6,
            volume: 0.7,
            palette: crate::style::Palette::default(),
            typewriter: true,
            toured: false,
            dock: crate::app::Dock::Right,
            breathe: true,
            follow_hour: false,
            human: String::new(),
            midi: vec![],
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("grm-vm").join("window.json"))
}

impl Settings {
    pub fn load() -> Settings {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}
