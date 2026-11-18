// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The window as it was left: which apps are shown and how (code, network, app, launched), the side panel's tab,
//! what was selected, where every point of the network was put and how the view was moved, the wishes for the
//! panels, the log, the history of the prompt, the window's size. Kept beside the world's memory
//! (`grm.window.json`), with the apps that run (`grm.desk.json`, kept by the prompt).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Code,
    Network,
    Plot,
    App,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Window {
    pub current: Option<String>,
    pub modes: HashMap<String, Mode>,
    pub launched: bool,
    pub editing: bool,
    pub xray: bool,
    pub side_tab: String,
    pub selected: Vec<String>,
    pub positions: HashMap<String, [f32; 2]>,
    pub pinned: Vec<String>,
    pub zoom: f32,
    pub offset: [f32; 2],
    pub fitted: bool,
    pub side_wish: Option<f32>,
    pub bottom_wish: Option<f32>,
    #[serde(default = "yes")]
    pub side_open: bool,
    #[serde(default = "yes")]
    pub space_on: bool,
    /// The zone in place of the space, and those in windows of their own (as the space writes them).
    pub zone_center: Option<String>,
    pub detached: Vec<String>,
    #[serde(default = "yes")]
    pub prompt_open: bool,
    /// The window was full screen.
    pub fullscreen: bool,
    /// The window was maximised (as large as the screen, with its title bar).
    #[serde(default)]
    pub maximized: bool,
    pub history: Vec<String>,
    /// (kind: s said, d dim, w warning, e error, a ask; text)
    pub log: Vec<(char, String)>,
    pub size: Option<[f32; 2]>,
}

fn yes() -> bool {
    true
}

pub fn path(memory: &Path) -> PathBuf {
    memory.with_extension("window.json")
}

impl Window {
    pub fn load(memory: &Path) -> Option<Window> {
        std::fs::read_to_string(path(memory)).ok().and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save(&self, memory: &Path) {
        if let Ok(s) = serde_json::to_string(self) {
            let _ = std::fs::write(path(memory), s);
        }
    }
}
