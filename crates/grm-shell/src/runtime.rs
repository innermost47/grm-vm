// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Apps that run: opened from their file, edited while they run (from their code, their network or their surface:
//! each edit is a new text, said again at once, written to its file at once), closed (their state kept for when
//! they open again). Their APIs live while they run. What runs is kept beside the world's memory, so that the
//! desktop opens again as it was left.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use grm_connect::api::{self, Event, Running};
use grm_engine::engine::Engine;
use grm_engine::Provenance;
use grm_sound::mixer::ReflexSource;

use crate::apps::{self, Parsed, Widget};
use crate::{Line, Shell};

/// An app as a window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppView {
    pub name: String,
    pub title: String,
    /// Its sign's name (`icon NAME`).
    pub icon: Option<String>,
    pub path: PathBuf,
    pub text: String,
    /// (line, why): what it could not say.
    pub errors: Vec<(usize, String)>,
    pub widgets: Vec<Widget>,
    /// Its own names given to a copy (an app opened while another running one holds the same network).
    pub prefix: IndexMap<String, String>,
    pub apis: Vec<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Its looks and style lines.
    pub looks: Vec<(usize, String)>,
}

/// A running app.
pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub text: String,
    pub parsed: Parsed,
    pub prefix: IndexMap<String, String>,
    pub errors: Vec<(usize, String)>,
    undo: Vec<String>,
    redo: Vec<String>,
    commands_done: Vec<String>,
}

/// What is kept beside the world's memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Desk {
    /// The apps running, in order: (name, file, the names of its copy).
    pub running: Vec<(String, PathBuf, IndexMap<String, String>)>,
    /// What closed apps were living through, to open them again where they were.
    pub closed: IndexMap<String, IndexMap<String, f64>>,
    /// The APIs said at the prompt (lines `api ...` and `read ...`).
    pub apis: Vec<String>,
    /// What played: (node, gain, pan, mute, solo).
    pub channels: Vec<(String, f64, f64, bool, bool)>,
    pub voice: Option<(bool, String, f64, f64)>,
}

pub fn source_of(app: &str) -> String {
    format!("app:{app}")
}

/// Where apps are: $GRM_APPS, or `apps` beside the world's memory.
pub fn apps_dir(memory: &Path) -> PathBuf {
    std::env::var_os("GRM_APPS").map(PathBuf::from).unwrap_or_else(|| {
        memory.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.join("apps")).unwrap_or_else(|| "apps".into())
    })
}

fn desk_path(memory: &Path) -> PathBuf {
    memory.with_extension("desk.json")
}

pub(crate) struct Runtime {
    pub apps: IndexMap<String, App>,
    pub closed: IndexMap<String, IndexMap<String, f64>>,
    apis: HashMap<String, Running>,
    events: (std::sync::mpsc::Sender<Event>, std::sync::mpsc::Receiver<Event>),
    pub latest: Arc<RwLock<IndexMap<String, f64>>>,
    pub desk_apis: Vec<String>,
    pub changed: bool,
    pub pending: Vec<Line>,
    last_tick: Option<std::time::Instant>,
    /// The desk's own clock (`play HZ`), ticks a second.
    pub desk_hz: Option<f64>,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            apps: IndexMap::new(),
            closed: IndexMap::new(),
            apis: HashMap::new(),
            events: api::events(),
            latest: Arc::new(RwLock::new(IndexMap::new())),
            desk_apis: vec![],
            changed: false,
            pending: vec![],
            last_tick: None,
            desk_hz: None,
        }
    }
}

impl Shell {
    pub fn apps_dir(&self) -> PathBuf {
        apps_dir(&self.memory)
    }

    /// The apps running, as a window shows them.
    pub fn app_views(&self) -> Vec<AppView> {
        self.rt
            .apps
            .values()
            .map(|a| AppView {
                name: a.name.clone(),
                title: a.parsed.title.clone().unwrap_or_else(|| a.name.clone()),
                icon: a.parsed.icon.clone(),
                path: a.path.clone(),
                text: a.text.clone(),
                errors: a.errors.clone(),
                widgets: a.parsed.widgets.clone(),
                prefix: a.prefix.clone(),
                apis: a.parsed.apis.iter().map(|x| x.name.clone()).collect(),
                can_undo: !a.undo.is_empty(),
                can_redo: !a.redo.is_empty(),
                looks: a.parsed.looks.clone(),
            })
            .collect()
    }

    /// Whether the apps changed since this was last asked (and forgets it).
    pub fn apps_changed(&mut self) -> bool {
        std::mem::take(&mut self.rt.changed)
    }

    /// Lines that came by themselves (an API said something).
    pub fn take_pending(&mut self) -> Vec<Line> {
        std::mem::take(&mut self.rt.pending)
    }

    /// The apps that can be opened: the files of the apps' folder.
    pub fn app_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(self.apps_dir())
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "grm"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    /// `app open FILE|NAME`, `app new NAME`, `app close NAME`, `app undo NAME`, `app redo NAME`, `apps`.
    pub(crate) fn app_command(&mut self, engine: &Engine, rest: &str) -> Vec<Line> {
        let mut words = rest.split_whitespace();
        let what = words.next().unwrap_or("");
        let arg = words.collect::<Vec<_>>().join(" ");
        match what {
            "" | "list" => {
                let mut out: Vec<Line> = self
                    .rt
                    .apps
                    .values()
                    .map(|a| {
                        Line::said(format!(
                            "{} (running): {}{}",
                            a.name,
                            a.path.display(),
                            if a.errors.is_empty() {
                                String::new()
                            } else {
                                format!(", {} line(s) not said", a.errors.len())
                            }
                        ))
                    })
                    .collect();
                for f in self.app_files() {
                    let n = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if !self.rt.apps.contains_key(&n) {
                        out.push(Line::dim(format!("{n}: {}", f.display())));
                    }
                }
                if out.is_empty() {
                    out.push(Line::dim(format!("no app yet: app new NAME (in {})", self.apps_dir().display())));
                }
                out
            }
            "open" => {
                let path = self.find_app(&arg);
                self.open_app(engine, &path)
            }
            "new" => self.new_app(engine, &arg),
            "close" => self.close_app(engine, &arg),
            "undo" => self.undo_app(engine, &arg, true),
            "redo" => self.undo_app(engine, &arg, false),
            _ => vec![Line::warn("? app open FILE|NAME, app new NAME, app close NAME, app undo|redo NAME, apps")],
        }
    }

    fn find_app(&self, arg: &str) -> PathBuf {
        let p = PathBuf::from(arg);
        if p.exists() {
            return p;
        }
        let in_dir = self.apps_dir().join(if arg.ends_with(".grm") { arg.to_string() } else { format!("{arg}.grm") });
        if in_dir.exists() {
            in_dir
        } else {
            p
        }
    }

    pub fn new_app(&mut self, engine: &Engine, name: &str) -> Vec<Line> {
        if !grm_engine::lang::is_name(name) {
            return vec![Line::warn("? app new NAME (a name of the language)")];
        }
        let dir = self.apps_dir();
        let path = dir.join(format!("{name}.grm"));
        if !path.exists() {
            let text = format!(
                "app {name}\n# its network: the language, as everywhere\nnode {name}_level = 0.5\n# its surface: what the human sees and touches\nwidget level: slider {name}_level 0 .. 1\nwidget shown: meter {name}_level 0 .. 1\n"
            );
            if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, text)) {
                return vec![Line::error(format!("! {}: {e}", path.display()))];
            }
        }
        self.open_app(engine, &path)
    }

    pub fn open_app(&mut self, engine: &Engine, path: &Path) -> Vec<Line> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => return vec![Line::error(format!("! {}: {e}", path.display()))],
        };
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if !grm_engine::lang::is_name(&name) {
            return vec![Line::error(format!("! {name}: an app's file is named as a name of the language"))];
        }
        // where it is, wherever the machine is started from next time
        let path = &path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if self.rt.apps.contains_key(&name) {
            return vec![Line::dim(format!("{name} runs already"))];
        }
        let app = App {
            name: name.clone(),
            path: path.to_path_buf(),
            text: String::new(),
            parsed: Parsed::default(),
            prefix: IndexMap::new(),
            errors: vec![],
            undo: vec![],
            redo: vec![],
            commands_done: vec![],
        };
        self.rt.apps.insert(name.clone(), app);
        let state = self.rt.closed.shift_remove(&name).unwrap_or_default();
        let mut out = self.apply(engine, &name, text, state, false);
        out.insert(0, Line::said(format!("{name} opens ({})", path.display())));
        out
    }

    pub fn close_app(&mut self, engine: &Engine, name: &str) -> Vec<Line> {
        let Some(app) = self.rt.apps.get(name) else { return vec![Line::warn(format!("? {name} does not run"))] };
        let source = source_of(name);
        let played: Vec<String> = app.parsed.widgets.iter().flat_map(|w| w.nodes.clone()).collect();
        let s = source.clone();
        let state = engine.query(move |w| w.state_of(&s)).unwrap_or_default();
        let s = source.clone();
        let _ = engine.query(move |w| w.rebuild(&s, &[], None, &IndexMap::new()));
        self.rt.closed.insert(name.to_string(), state);
        self.rt.apis.retain(|k, _| !k.starts_with(&format!("{name}/")));
        // what it played goes quiet
        if let Ok(mut m) = self.mixer.lock() {
            for n in m.playing() {
                if played.contains(&n) {
                    m.stop(&n);
                }
            }
        }
        self.rt.apps.shift_remove(name);
        self.refresh_channels(engine);
        self.rt.changed = true;
        self.save_desk();
        vec![Line::said(format!("{name} closes (its file stays, what it was living through is kept)"))]
    }

    fn undo_app(&mut self, engine: &Engine, name: &str, back: bool) -> Vec<Line> {
        let Some(app) = self.rt.apps.get_mut(name) else { return vec![Line::warn(format!("? {name} does not run"))] };
        let (from, to) = if back { (&mut app.undo, &mut app.redo) } else { (&mut app.redo, &mut app.undo) };
        let Some(text) = from.pop() else { return vec![Line::dim("nothing to take back")] };
        to.push(app.text.clone());
        let state = IndexMap::new();
        self.apply(engine, name, text, state, true)
    }

    /// A new text for a running app (edited from its code, its network or its surface): said again at once,
    /// written to its file at once.
    pub fn edit_app(&mut self, engine: &Engine, name: &str, text: String) -> Vec<Line> {
        let Some(app) = self.rt.apps.get_mut(name) else { return vec![Line::warn(format!("? {name} does not run"))] };
        if app.text == text {
            return vec![];
        }
        app.undo.push(app.text.clone());
        if app.undo.len() > 200 {
            app.undo.remove(0);
        }
        app.redo.clear();
        self.apply(engine, name, text, IndexMap::new(), true)
    }

    /// Says an app's text again into the world, in one change.
    fn apply(
        &mut self,
        engine: &Engine,
        name: &str,
        text: String,
        state: IndexMap<String, f64>,
        save: bool,
    ) -> Vec<Line> {
        let source = source_of(name);
        let Some(app) = self.rt.apps.get(name) else { return vec![] };
        let base = app.path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let parsed = apps::parse(&text, &base);
        // a network another running app holds: this one runs its own copy (its names given a prefix)
        let others: Vec<String> = self.rt.apps.keys().filter(|n| *n != name).map(|n| source_of(n)).collect();
        let held: Vec<String> = engine
            .query(move |w| {
                let net = &w.machine.net;
                net.nodes
                    .iter()
                    .filter(|(_, s)| others.contains(&s.origin.source))
                    .map(|(n, _)| n.clone())
                    .chain(net.relations.iter().filter(|r| others.contains(&r.origin.source)).map(|r| r.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut prefix = app.prefix.clone();
        for d in apps::defined(&parsed.flat) {
            if held.contains(&d) && !prefix.contains_key(&d) {
                prefix.insert(d.clone(), format!("{name}__{d}"));
            }
        }
        let parsed = parsed.renamed(&prefix);
        let flat: Vec<String> = parsed.flat.iter().map(|f| f.text.clone()).collect();
        let s = source.clone();
        let b = base.clone();
        let results = engine.query(move |w| w.rebuild(&s, &flat, Some(&b), &state));
        let mut out = vec![];
        let mut errors = parsed.errors.clone();
        match results {
            Some(Ok(results)) => {
                for (f, r) in parsed.flat.iter().zip(results) {
                    let why = match r {
                        Err(e) => Some(e),
                        Ok(None) => {
                            Some(format!("not a statement: {}", f.text.split_whitespace().next().unwrap_or("")))
                        }
                        Ok(Some(_)) => None,
                    };
                    if let Some(why) = why {
                        let why = match &f.from {
                            Some(from) => format!("{why} (in {from})"),
                            None => why,
                        };
                        if !errors.iter().any(|(l, _)| *l == f.line) {
                            errors.push((f.line, why));
                        }
                    }
                }
            }
            Some(Err(e)) => errors.push((0, e.message)),
            None => errors.push((0, "the engine has stopped".into())),
        }
        errors.sort_by_key(|e| e.0);
        let new_prefix: Vec<String> =
            prefix.iter().filter(|(k, _)| !app.prefix.contains_key(*k)).map(|(k, v)| format!("{k} -> {v}")).collect();
        if !new_prefix.is_empty() {
            out.push(Line::warn(format!(
                "{name} runs its own copy of what another running app holds: {}",
                new_prefix.join(", ")
            )));
        }
        for (l, e) in &errors {
            out.push(Line::warn(format!("{name}, line {l}: {e}")));
        }
        // its commands: those not run yet
        let done = app.commands_done.clone();
        let commands = parsed.commands.clone();
        // its APIs: started, or started again when they changed
        let apis = parsed.apis.clone();
        {
            let app = self.rt.apps.get_mut(name).unwrap();
            app.text = text.clone();
            app.parsed = parsed;
            app.prefix = prefix;
            app.errors = errors;
        }
        for (_, c) in &commands {
            if !done.contains(c) {
                let (lines, _) = self.run(engine, c);
                out.extend(lines);
            }
        }
        self.rt.apps.get_mut(name).unwrap().commands_done = commands.into_iter().map(|c| c.1).collect();
        self.start_apis(engine, name, apis);
        // what plays goes on, as it is now written
        self.refresh_channels(engine);
        if save {
            let path = self.rt.apps[name].path.clone();
            let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
            if on_disk != text {
                if let Err(e) = std::fs::write(&path, &text) {
                    out.push(Line::error(format!("! {}: {e}", path.display())));
                }
            }
        }
        self.rt.changed = true;
        self.save_desk();
        out
    }

    fn start_apis(&mut self, engine: &Engine, owner: &str, apis: Vec<grm_connect::api::Api>) {
        let names: Vec<String> = apis.iter().map(|a| format!("{owner}/{}", a.name)).collect();
        self.rt.apis.retain(|k, _| !k.starts_with(&format!("{owner}/")) || names.contains(k));
        for a in apis {
            let key = format!("{owner}/{}", a.name);
            if self.rt.apis.get(&key).is_some_and(|r| r.api == a) {
                continue;
            }
            // the nodes it reads into exist (signed, if nothing said otherwise)
            let nodes: Vec<String> = a.reads.iter().map(|r| r.node.clone()).collect();
            let origin = Provenance::of(if owner == "desk" { "api".to_string() } else { source_of(owner) });
            let _ = engine.query(move |w| {
                for n in nodes {
                    if !w.machine.net.nodes.contains_key(&n) {
                        let _ = w.say(&format!("node {n} signed"), &origin);
                    }
                }
            });
            let latest = self.rt.latest.clone();
            let values = Arc::new(move |names: &[String]| -> Vec<(String, f64)> {
                let l = latest.read().map(|l| l.clone()).unwrap_or_default();
                names.iter().filter_map(|n| l.get(n).map(|v| (n.clone(), *v))).collect()
            });
            self.rt.pending.push(Line::dim(format!("api {} starts", a.name)));
            let running = api::start(a, self.rt.events.0.clone(), values);
            self.rt.apis.insert(key, running);
        }
    }

    /// `api ...` and `read ...` said at the prompt: APIs of the desk (no app).
    pub(crate) fn desk_api(&mut self, engine: &Engine, line: &str) -> Vec<Line> {
        let mut lines = self.rt.desk_apis.clone();
        let word = line.split_whitespace().next().unwrap_or("");
        if word == "api" {
            if let Ok(a) = grm_connect::api::parse_api(&line[4..]) {
                lines.retain(|l| {
                    !(l.starts_with("api ") && grm_connect::api::parse_api(&l[4..]).is_ok_and(|b| b.name == a.name))
                });
            }
        }
        lines.push(line.to_string());
        let text = lines.join("\n");
        let parsed = apps::parse(&text, Path::new("."));
        if let Some((_, e)) = parsed.errors.last() {
            return vec![Line::error(format!("! {e}"))];
        }
        self.rt.desk_apis = lines;
        let n = parsed.apis.len();
        self.start_apis(engine, "desk", parsed.apis);
        self.save_desk();
        vec![Line::said(format!("{n} api(s) of the desk"))]
    }

    /// What the APIs read goes into the machine; what they said, to the human.
    pub(crate) fn hear_apis(&mut self, engine: &Engine) {
        let events: Vec<Event> = self.rt.events.1.try_iter().collect();
        for e in events {
            match e {
                Event::Values(api, values) => {
                    let lines: Vec<String> = values
                        .iter()
                        .map(|(n, v, s)| match s {
                            Some(s) => format!("observe {n} = {v} ± {s}"),
                            None => format!("observe {n} = {v}"),
                        })
                        .collect();
                    let origin = Provenance::of(format!("api:{api}"));
                    engine.send(move |w| {
                        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                        w.say_all(&refs, &origin).map(|_| String::new())
                    });
                }
                Event::Said(_, text) => self.rt.pending.push(Line::dim(text)),
            }
        }
    }

    /// Every channel that plays a node is made again from the network as it is now (it goes on from where it was).
    pub(crate) fn refresh_channels(&mut self, engine: &Engine) {
        let (playing, rate) = match self.mixer.lock() {
            Ok(m) => (m.playing(), m.rate),
            Err(_) => return,
        };
        for node in playing {
            match Self::reflex_of(engine, &node, rate) {
                Ok(r) => {
                    if let Some(source) = ReflexSource::new(r, &node) {
                        if let Ok(mut m) = self.mixer.lock() {
                            m.swap(&node, Box::new(source));
                        }
                    }
                }
                Err(_) => {
                    if let Ok(mut m) = self.mixer.lock() {
                        m.stop(&node);
                    }
                    self.rt.pending.push(Line::dim(format!("{node} no longer plays (it is no longer in the network)")));
                }
            }
        }
    }

    /// Keeps what runs beside the world's memory.
    pub fn save_desk(&self) {
        if !self.keep_desk {
            return;
        }
        let desk = self.desk();
        if let Ok(s) = serde_json::to_string_pretty(&desk) {
            let _ = std::fs::write(desk_path(&self.memory), s);
        }
    }

    pub fn desk(&self) -> Desk {
        let (channels, voice) = match self.mixer.lock() {
            Ok(m) => (
                m.channels
                    .iter()
                    .filter(|(n, _)| m.playing().contains(n))
                    .map(|(n, c)| (n.clone(), c.gain, c.pan, c.mute, c.solo))
                    .collect(),
                Some((
                    m.ambient_on,
                    match m.ambient.kind {
                        grm_sound::ambient::Kind::Engine => "engine".to_string(),
                        grm_sound::ambient::Kind::Wind => "wind".to_string(),
                        grm_sound::ambient::Kind::Pad => "pad".to_string(),
                    },
                    m.ambient_gain,
                    m.master,
                )),
            ),
            Err(_) => (vec![], None),
        };
        Desk {
            running: self.rt.apps.values().map(|a| (a.name.clone(), a.path.clone(), a.prefix.clone())).collect(),
            closed: self.rt.closed.clone(),
            apis: self.rt.desk_apis.clone(),
            channels,
            voice,
        }
    }

    /// The desk as it was left: its apps running again (said again from their files, the world keeping what they
    /// were living through), its APIs, what played.
    pub fn restore_desk(&mut self, engine: &Engine) -> Vec<Line> {
        if !self.keep_desk {
            return vec![];
        }
        let Ok(text) = std::fs::read_to_string(desk_path(&self.memory)) else { return vec![] };
        let Ok(desk) = serde_json::from_str::<Desk>(&text) else {
            return vec![Line::warn("the desk could not be read: it starts empty")];
        };
        self.rt.closed = desk.closed.clone();
        let mut out = vec![];
        for (name, path, prefix) in &desk.running {
            let Ok(text) = std::fs::read_to_string(path) else {
                out.push(Line::warn(format!("{name}: {} is not there any more", path.display())));
                continue;
            };
            self.rt.apps.insert(
                name.clone(),
                App {
                    name: name.clone(),
                    path: path.clone(),
                    text: String::new(),
                    parsed: Parsed::default(),
                    prefix: prefix.clone(),
                    errors: vec![],
                    undo: vec![],
                    redo: vec![],
                    // what it ran when it opened ran then: not again (what plays is given back below)
                    commands_done: vec![],
                },
            );
            let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let parsed = apps::parse(&text, &base);
            self.rt.apps.get_mut(name).unwrap().commands_done = parsed.commands.iter().map(|c| c.1.clone()).collect();
            out.extend(self.apply(engine, name, text, IndexMap::new(), false));
        }
        if !desk.apis.is_empty() {
            for l in desk.apis.clone() {
                self.desk_api(engine, &l);
            }
        }
        if let Some((on, kind, gain, master)) = desk.voice {
            if let Ok(mut m) = self.mixer.lock() {
                m.ambient_gain = gain;
                m.master = master;
            }
            let _ = self.run(engine, &format!("voice {kind}"));
            if on {
                let _ = self.run(engine, "sound on");
            }
        }
        for (node, gain, pan, mute, solo) in desk.channels {
            let (lines, _) = self.run(engine, &format!("play {node} {gain}"));
            if lines.iter().any(|l| l.kind == crate::Kind::Error) {
                continue;
            }
            if let Ok(mut m) = self.mixer.lock() {
                if let Some(c) = m.channels.get_mut(&node) {
                    c.pan = pan;
                    c.mute = mute;
                    c.solo = solo;
                }
            }
        }
        self.rt.changed = true;
        if !desk.running.is_empty() {
            out.insert(
                0,
                Line::dim(format!(
                    "the desk as it was left: {}",
                    desk.running.iter().map(|r| r.0.as_str()).collect::<Vec<_>>().join(", ")
                )),
            );
        }
        out
    }

    /// The world ticks as often as the running apps ask (`clock S`: the shortest wins).
    pub(crate) fn clock(&mut self, engine: &Engine) {
        let desk = self.rt.desk_hz.map(|hz| 1.0 / hz);
        let Some(every) = self.rt.apps.values().filter_map(|a| a.parsed.clock).chain(desk).reduce(f64::min) else {
            self.rt.last_tick = None;
            return;
        };
        let now = std::time::Instant::now();
        let due = self.rt.last_tick.is_none_or(|t| now.duration_since(t).as_secs_f64() >= every);
        if due {
            self.rt.last_tick = Some(now);
            engine.send(|w| {
                w.tick(false);
                Ok(String::new())
            });
        }
    }

    /// Keeps the values the APIs send.
    pub(crate) fn remember_values(&self, values: &IndexMap<String, (f64, f64)>) {
        if let Ok(mut l) = self.rt.latest.write() {
            *l = values.iter().map(|(k, v)| (k.clone(), v.0)).collect();
        }
    }

    /// The app (and its line) that writes this name itself, if one does.
    pub fn owner_of(&self, name: &str) -> Option<(String, usize)> {
        for a in self.rt.apps.values() {
            let original = a.prefix.iter().find(|(_, v)| *v == name).map(|(k, _)| k.as_str()).unwrap_or(name);
            if let Some(l) = apps::line_of(&a.text, original) {
                return Some((a.name.clone(), l));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grm_engine::world::World;
    use grm_sound::mixer::Mixer;
    use std::sync::Mutex;
    use std::time::Duration;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("grm-runtime-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("apps")).unwrap();
        d
    }

    #[test]
    fn apps_run_are_edited_while_they_run_and_share_nothing_by_accident() {
        let d = dir("share");
        std::fs::write(
            d.join("apps/count.grm"),
            "network counter(name)\n  var {name}_n = 0\n  set {name}_n = {name}_n + 1\nend\n",
        )
        .unwrap();
        std::fs::write(d.join("apps/a.grm"), "use count.grm\ncall counter(name=c)\nwidget n: value c_n\n").unwrap();
        std::fs::write(d.join("apps/b.grm"), "use count.grm\ncall counter(name=c)\n").unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(d.join("grm.world"), mixer.clone());
        let out = sh.app_command(&engine, "open a");
        assert!(out.iter().all(|l| l.kind != crate::Kind::Error && l.kind != crate::Kind::Warning), "{out:?}");
        let tick = |n: usize| {
            engine.query(move |w| {
                for _ in 0..n {
                    w.tick(false);
                }
            })
        };
        tick(3);
        // alone: no copy
        assert!(sh.rt.apps["a"].prefix.is_empty());
        // b opens while a runs: b runs its own copy
        let out = sh.app_command(&engine, "open b");
        assert!(out.iter().any(|l| l.text.contains("its own copy")), "{out:?}");
        assert_eq!(sh.rt.apps["b"].prefix["c_n"], "b__c_n");
        tick(2);
        let v = |n: &'static str| engine.query(move |w| w.machine.net.measured.get(n).map(|m| m.0)).flatten();
        assert_eq!(v("c_n"), Some(5.0));
        assert_eq!(v("b__c_n"), Some(2.0));
        // edited while it runs: it goes on from where it was, and its file is written at once
        let text = sh.rt.apps["a"].text.clone() + "node extra = 2\n";
        let out = sh.edit_app(&engine, "a", text.clone());
        assert!(out.is_empty(), "{out:?}");
        assert_eq!(v("c_n"), Some(5.0));
        assert_eq!(std::fs::read_to_string(d.join("apps/a.grm")).unwrap(), text);
        // a line it cannot say: said where it is, the rest goes on
        let out = sh.edit_app(&engine, "a", text.clone() + "relate broken = =\n");
        assert!(out.iter().any(|l| l.text.starts_with("a, line 5:")), "{out:?}");
        // taken back
        sh.app_command(&engine, "undo a");
        assert_eq!(sh.rt.apps["a"].text, text);
        // removing what an app writes asks, and takes its line from it
        let (out, _) = sh.run(&engine, "remove extra");
        assert_eq!(out[0].kind, crate::Kind::Ask, "{out:?}");
        sh.run(&engine, "y");
        assert!(!sh.rt.apps["a"].text.contains("extra"));
        // closed, opened again where it was
        sh.app_command(&engine, "close a");
        assert_eq!(v("c_n"), None);
        assert_eq!(v("b__c_n"), Some(2.0), "the other app is not touched");
        sh.app_command(&engine, "open a");
        assert_eq!(v("c_n"), Some(5.0));
        // the desk, kept and opened again in a new session
        sh.save_desk();
        let world = engine.stop();
        let engine = Engine::start(world, Duration::from_millis(4));
        let mut sh2 = Shell::new(d.join("grm.world"), mixer);
        let out = sh2.restore_desk(&engine);
        assert!(out[0].text.contains("b, a") || out[0].text.contains("a, b"), "{out:?}");
        assert_eq!(sh2.rt.apps["b"].prefix["c_n"], "b__c_n");
        let v = |n: &'static str| engine.query(move |w| w.machine.net.measured.get(n).map(|m| m.0)).flatten();
        assert_eq!(v("c_n"), Some(5.0));
        assert_eq!(v("b__c_n"), Some(2.0));
        engine.stop();
    }

    #[test]
    fn what_plays_goes_on_while_its_app_is_edited_and_apis_feed_the_machine() {
        let d = dir("live");
        let app = "input step = 0.0576\nvar c = 1\nvar s = 0\nnode c_new signed\nrelate turn: c_new = c - step * s\nset c = c_new\nset s = s + step * c_new\nplay s 0.5\n";
        std::fs::write(d.join("apps/osc.grm"), app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(d.join("grm.world"), mixer.clone());
        sh.app_command(&engine, "open osc");
        let crossings = || {
            let mut buf = vec![0.0f32; 2 * 48000];
            mixer.lock().unwrap().render(&mut buf);
            let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
            (left.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as i64, left)
        };
        let (n, before) = crossings();
        assert!((n - 440).abs() <= 3, "{n}");
        // edited while it sounds: no silence, no jump
        let out = sh.edit_app(&engine, "osc", format!("{app}node more = 2\n"));
        assert!(out.is_empty(), "{out:?}");
        let (n, after) = crossings();
        assert!((n - 440).abs() <= 3, "{n}");
        let jump = (after[0] - before[before.len() - 1]).abs();
        assert!(jump < 0.05, "it goes on from where it was: {jump}");
        // an API of the desk: what it reads goes into the machine
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in listener.incoming().take(3) {
                let mut s = stream.unwrap();
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = r#"{"now": {"t": 21.5}}"#;
                let _ =
                    write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        sh.run(&engine, &format!("api w: get http://127.0.0.1:{port}/ every 0.2"));
        let (out, _) = sh.run(&engine, "read w now.t -> outside ± 0.5");
        assert!(out[0].text.contains("1 api"), "{out:?}");
        let mut got = None;
        for _ in 0..50 {
            sh.feel(&engine);
            got = engine.query(|w| w.machine.net.measured.get("outside").map(|m| m.0)).flatten();
            if got.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(got, Some(21.5));
        engine.stop();
    }
}
