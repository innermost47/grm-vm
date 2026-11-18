// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What is in front, in the middle of the window: the space, the whole network, an app, a file, a sandbox. One
//! logic for every tab: which is lit is what is in front; a tab pressed brings it in front; what is closed gives
//! the place back to what was in front before it, as it was left (its view, what was chosen in it).

use crate::app::App;

/// What can be in front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Front {
    Space,
    /// The whole network (the desk).
    Desk,
    App(String),
    /// A file opened, by its place on the disk.
    File(std::path::PathBuf),
    Sandbox(String),
}

impl Front {
    /// Its word, where its view is kept.
    fn key(&self) -> String {
        match self {
            Front::Space => "space".into(),
            Front::Desk => "desk".into(),
            Front::App(a) => format!("app {a}"),
            Front::File(p) => format!("file {}", p.display()),
            Front::Sandbox(s) => format!("sandbox {s}"),
        }
    }
}

/// How a network was being looked at in a front, kept while another is in front.
pub struct Kept {
    view: crate::view::View,
    selected: Vec<String>,
    dived: Option<String>,
}

/// How many fronts are remembered to go back to.
const BEHIND: usize = 24;

impl App {
    /// What is in front now.
    pub fn front(&self) -> Front {
        if self.space_on {
            return Front::Space;
        }
        if let Some(f) = self.current_file.and_then(|k| self.files.get(k)) {
            return Front::File(f.path.clone());
        }
        if let Some(s) = &self.sandbox_shown {
            return Front::Sandbox(s.clone());
        }
        match &self.current {
            Some(a) => Front::App(a.clone()),
            None => Front::Desk,
        }
    }

    /// Whether it is still there to be brought in front.
    fn still_there(&self, f: &Front) -> bool {
        match f {
            Front::Space | Front::Desk => true,
            Front::App(a) => self.apps.iter().any(|x| &x.name == a),
            Front::File(p) => self.files.iter().any(|x| &x.path == p),
            Front::Sandbox(s) => self.picture.sandboxes.iter().any(|x| &x.name == s),
        }
    }

    /// Brought in front (what was in front is remembered, to come back to it as it was).
    pub fn go(&mut self, to: Front) {
        let from = self.front();
        if from == to {
            return;
        }
        self.behind.retain(|f| *f != to);
        self.behind.push(from.clone());
        if self.behind.len() > BEHIND {
            self.behind.remove(0);
        }
        self.show_front(from, to);
    }

    /// What was in front before the one now (closed, or left by its arrow): in front again, as it was left.
    pub fn leave_front(&mut self) {
        let from = self.front();
        while let Some(f) = self.behind.pop() {
            if f != from && self.still_there(&f) {
                self.show_front(from, f);
                return;
            }
        }
        self.show_front(from, Front::Desk);
    }

    fn show_front(&mut self, from: Front, to: Front) {
        // how the network was looked at in the one left is kept for when it comes back
        let kept = Kept { view: self.view.clone(), selected: self.selected.clone(), dived: self.dived.clone() };
        self.views_kept.insert(from.key(), kept);
        if matches!(from, Front::App(_)) {
            self.editing = false;
        }
        self.space_on = to == Front::Space;
        self.current_file = match &to {
            Front::File(p) => self.files.iter().position(|f| &f.path == p),
            _ => None,
        };
        self.sandbox_shown = match &to {
            Front::Sandbox(s) => Some(s.clone()),
            _ => None,
        };
        match &to {
            Front::App(a) => self.current = Some(a.clone()),
            Front::Desk => {
                self.current = None;
                self.launched = false;
            }
            _ => {}
        }
        if let Some(k) = self.views_kept.remove(&to.key()) {
            self.view = k.view;
            self.selected = k.selected.into_iter().filter(|s| self.picture.node(s).is_some()).collect();
            self.dived = k.dived;
        }
    }
}
