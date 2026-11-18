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

/// Which tips are said (when the pointer rests on something), by where they are. All, unless the human hides some.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tips {
    /// The window's own (its bars, its panels, its menus).
    pub window: bool,
    /// The apps that come with the machine.
    pub native: bool,
    /// The human's own apps.
    pub mine: bool,
    /// Apps from elsewhere.
    pub others: bool,
}

impl Default for Tips {
    fn default() -> Self {
        Tips { window: true, native: true, mine: true, others: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Which tips are said.
    pub tips: Tips,
    /// Where it is kept, when not in the user's configuration folder (a home of its own).
    #[serde(skip)]
    pub at: Option<PathBuf>,
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
    /// Her name, chosen by the human (empty: Her).
    pub her: String,
    /// The names of those who played with her (she keeps apart what she learned with each, up to four; nothing
    /// of them but their place here).
    pub humans: Vec<String>,
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
            toured: false,
            dock: crate::app::Dock::Right,
            breathe: true,
            follow_hour: false,
            human: String::new(),
            her: String::new(),
            humans: vec![],
            tips: Tips::default(),
            midi: vec![],
            at: None,
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
    /// Who plays with her now, as she knows them apart (1 .. 4; a name not given is "someone", the first).
    pub fn human_number(&mut self) -> u32 {
        let name = if self.human.trim().is_empty() { "someone".to_string() } else { self.human.trim().to_string() };
        if let Some(k) = self.humans.iter().position(|h| *h == name) {
            return k as u32 + 1;
        }
        if self.humans.len() < 4 {
            self.humans.push(name);
            return self.humans.len() as u32;
        }
        4
    }

    /// Her name: the one chosen, else Her.
    pub fn her_name(&self) -> String {
        Some(self.her.trim()).filter(|n| !n.is_empty()).unwrap_or("Her").to_string()
    }

    /// The settings kept at `at` (a home of its own), else in the user's configuration folder.
    pub fn load_from(at: Option<PathBuf>) -> Settings {
        let file = at.clone().or_else(path);
        let mut s: Settings = file
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        s.at = at;
        s
    }

    pub fn save(&self) {
        if let Some(p) = self.at.clone().or_else(path) {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}
