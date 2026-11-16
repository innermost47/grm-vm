// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Where the human has been, and what has changed.
//!
//! Places: what was shown (the desk or an app, in which view; a file; a sandbox; what was selected; the side tab;
//! where the view looked). Back and forward go through them, as in a browser (Alt+Left, Alt+Right, the mouse's
//! side buttons).
//!
//! Changes: every change can be taken back and given again, whatever it touched: the world (the prompt keeps it as
//! it was before), an app's text (each app keeps its own), where the nodes were put, the settings, a table's cells.
//! One list for all of them, in the order they were made (Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y), shown in the journal.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::{App, Tab};
use crate::desk::Mode;
use crate::settings::Settings;

/// What a place is.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    current: Option<String>,
    mode: Option<Mode>,
    desk_mode: Mode,
    file: Option<PathBuf>,
    sandbox: Option<String>,
    workshop: Option<String>,
    selected: Vec<String>,
    tab: Tab,
    dived: Option<String>,
    zoom: f32,
    offset: [f32; 2],
}

impl Place {
    /// Two places are the same place when the same thing is shown (looking around in it does not make a new one).
    fn same(&self, o: &Place) -> bool {
        self.current == o.current
            && self.mode == o.mode
            && self.desk_mode == o.desk_mode
            && self.file == o.file
            && self.sandbox == o.sandbox
            && self.workshop == o.workshop
            && self.selected == o.selected
            && self.tab == o.tab
            && self.dived == o.dived
    }
}

/// A table as it was: its header, its cells, its separator.
pub type Table = (Vec<String>, Vec<Vec<String>>, char);

/// A change, and what to give back to take it back.
#[derive(Debug, Clone)]
pub enum Change {
    /// The world changed by a line said (the prompt kept it as it was).
    World,
    /// An app's text (the app keeps its texts).
    App(String),
    /// An app opened (taken back: closed), or closed (taken back: opened again).
    Opened {
        name: String,
        path: PathBuf,
    },
    Closed {
        name: String,
        path: PathBuf,
    },
    /// Where the nodes were put.
    Layout {
        pos: HashMap<String, [f32; 2]>,
        pinned: HashSet<String>,
    },
    Settings(Box<Settings>),
    /// One cell of a table (row usize::MAX: its header).
    Cell {
        path: PathBuf,
        row: usize,
        col: usize,
        text: String,
    },
    /// A whole table (a row or a column added).
    Table {
        path: PathBuf,
        table: Table,
    },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub change: Change,
    /// What it was, in words.
    pub what: String,
    at: Instant,
}

#[derive(Default)]
pub struct Journey {
    back: Vec<Place>,
    forward: Vec<Place>,
    here: Option<Place>,
    pub undo: Vec<Entry>,
    pub redo: Vec<Entry>,
    /// Taking back or giving again, this frame (what it changes is not a new change).
    pub applying: bool,
    /// Apps opened or closed by taking back: when the prompt says so, it is not a new change.
    pub quiet: HashSet<String>,
}

const PLACES: usize = 100;
const CHANGES: usize = 300;
/// Changes of the same thing this close together are one change (a colour dragged, a cell typed in).
const TOGETHER: Duration = Duration::from_millis(1500);

impl Journey {
    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// A change made: it can be taken back. Changes of the same thing close together are one.
    pub fn record(&mut self, change: Change, what: impl Into<String>) {
        if self.applying {
            return;
        }
        if let Some(last) = self.undo.last() {
            let together = last.at.elapsed() < TOGETHER;
            let same = match (&last.change, &change) {
                (Change::Settings(_), Change::Settings(_)) => true,
                (Change::Cell { path: a, row: r, col: c, .. }, Change::Cell { path: b, row: s, col: d, .. }) => {
                    a == b && r == s && c == d
                }
                _ => false,
            };
            if together && same {
                self.undo.last_mut().unwrap().at = Instant::now();
                return;
            }
        }
        self.undo.push(Entry { change, what: what.into(), at: Instant::now() });
        if self.undo.len() > CHANGES {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

impl App {
    fn place_now(&self) -> Place {
        Place {
            current: self.current.clone(),
            mode: self.current.as_ref().map(|c| self.mode_of(c)),
            desk_mode: self.desk_mode,
            file: self.current_file.and_then(|k| self.files.get(k)).map(|f| f.path.clone()),
            sandbox: self.sandbox_shown.clone(),
            workshop: self.workshop_shown.clone(),
            selected: self.selected.clone(),
            tab: self.tab,
            dived: self.dived.clone(),
            zoom: self.view.zoom,
            offset: [self.view.offset.x, self.view.offset.y],
        }
    }

    fn go_to(&mut self, p: &Place) {
        self.current = p.current.clone().filter(|c| self.apps.iter().any(|a| &a.name == c));
        if let (Some(c), Some(m)) = (&self.current, p.mode) {
            self.modes.insert(c.clone(), m);
        }
        self.desk_mode = p.desk_mode;
        self.current_file = p.file.as_ref().and_then(|path| self.files.iter().position(|f| &f.path == path));
        self.sandbox_shown = p.sandbox.clone();
        self.workshop_shown = p.workshop.clone().filter(|w| self.picture.workshops.iter().any(|x| &x.0 == w));
        self.selected = p.selected.iter().filter(|s| self.picture.node(s).is_some()).cloned().collect();
        self.tab = p.tab;
        self.dived = p.dived.clone().filter(|d| self.picture.nodes.iter().any(|n| &n.source == d));
        self.view.zoom = p.zoom;
        self.view.offset = egui::vec2(p.offset[0], p.offset[1]);
        self.view.fitted = true;
        self.editing = false;
        self.journey.here = Some(self.place_now());
    }

    /// Each frame: a new place is remembered (where the human was before goes back).
    pub fn follow(&mut self) {
        self.journey.applying = false;
        let now = self.place_now();
        match self.journey.here.take() {
            Some(before) if !before.same(&now) => {
                self.journey.back.push(before);
                if self.journey.back.len() > PLACES {
                    self.journey.back.remove(0);
                }
                self.journey.forward.clear();
            }
            _ => {}
        }
        self.journey.here = Some(now);
    }

    pub fn go_back(&mut self) {
        if let Some(p) = self.journey.back.pop() {
            let now = self.place_now();
            self.journey.forward.push(now);
            self.go_to(&p);
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(p) = self.journey.forward.pop() {
            let now = self.place_now();
            self.journey.back.push(now);
            self.go_to(&p);
        }
    }

    /// Takes the last change back (or gives the last one taken back again).
    pub fn take_back(&mut self, again: bool) {
        let entry = if again { self.journey.redo.pop() } else { self.journey.undo.pop() };
        let Some(entry) = entry else { return };
        self.journey.applying = true;
        // the human taking something back is a signal too: a little displeasure, said in the journal
        let (amount, why) = if again {
            (0.03, format!("the human gave back: {}", entry.what))
        } else {
            (-0.05, format!("the human took back: {}", entry.what))
        };
        let _ = self.engine.query(move |w| {
            let tick = w.machine.net.ticks;
            w.mood.reward(amount, tick, why)
        });
        let word = if again { "redo" } else { "undo" };
        let opposite = match entry.change {
            Change::World => {
                self.say(word);
                Change::World
            }
            Change::App(name) => {
                self.say(format!("app {word} {name}"));
                Change::App(name)
            }
            Change::Opened { name, path } => {
                self.journey.quiet.insert(name.clone());
                self.say(if again {
                    format!("app open {}", grm_shell::quoted(&path.display().to_string()))
                } else {
                    format!("app close {name}")
                });
                Change::Opened { name, path }
            }
            Change::Closed { name, path } => {
                self.journey.quiet.insert(name.clone());
                // put in the trash meanwhile: it comes back from there
                let open = if path.exists() {
                    format!("app open {}", grm_shell::quoted(&path.display().to_string()))
                } else {
                    format!("app restore {name}")
                };
                self.say(if again { format!("app close {name}") } else { open });
                Change::Closed { name, path }
            }
            Change::Layout { pos, pinned } => {
                let was = Change::Layout {
                    pos: std::mem::replace(&mut self.graph.pos, pos),
                    pinned: std::mem::replace(&mut self.graph.pinned, pinned),
                };
                self.graph.motion = 1.0;
                was
            }
            Change::Settings(s) => Change::Settings(Box::new(std::mem::replace(&mut self.settings, *s))),
            Change::Cell { path, row, col, text } => {
                let mut was = text.clone();
                if let Some((head, cells, _)) =
                    self.files.iter_mut().find(|f| f.path == path).and_then(|f| f.table.as_mut())
                {
                    let cell = if row == usize::MAX {
                        head.get_mut(col)
                    } else {
                        cells.get_mut(row).and_then(|r| r.get_mut(col))
                    };
                    if let Some(c) = cell {
                        was = std::mem::replace(c, text);
                    }
                }
                if let Some(f) = self.files.iter_mut().find(|f| f.path == path) {
                    f.table_saved = false;
                }
                Change::Cell { path, row, col, text: was }
            }
            Change::Table { path, table } => {
                let mut was = table.clone();
                if let Some(f) = self.files.iter_mut().find(|f| f.path == path) {
                    if let Some(t) = f.table.as_mut() {
                        was = std::mem::replace(t, table);
                    }
                    f.table_saved = false;
                }
                Change::Table { path, table: was }
            }
        };
        let back = Entry { change: opposite, what: entry.what, at: Instant::now() };
        if again {
            self.journey.undo.push(back);
        } else {
            self.journey.redo.push(back);
        }
    }

    /// Where the nodes are now, as a change about to be made to them.
    pub fn layout_change(&self) -> Change {
        Change::Layout { pos: self.graph.pos.clone(), pinned: self.graph.pinned.clone() }
    }
}
