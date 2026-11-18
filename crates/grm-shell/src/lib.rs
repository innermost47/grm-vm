// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The prompt of the GRM machine: every command, the same in the terminal and in the window. A line goes in,
//! lines come out (each with its kind: said, dim, a warning, an error, a question waiting for y/n).
//!
//! It also keeps the sound: the mixer (the machine's own voice, and the channels the human plays), and what the
//! machine feels moves the voice (`feel`).

/// The longest side of a picture given to the eye (a bigger one is made smaller first).
pub const SIGHT: usize = 160;

pub mod apps;
pub mod fs;
pub mod runtime;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use grm_connect::{Field, FieldKind, Table};
use grm_engine::engine::Engine;
use grm_engine::lang::g;
use grm_engine::reflex::{Clock, Reflex};
use grm_engine::world::World;
use grm_engine::{GrmError, Provenance};
use grm_sound::ambient::{Feeling, Kind as VoiceKind, Layers, Voice};
use grm_sound::mixer::{Mixer, ReflexSource};

/// What a line that comes out is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Said,
    Dim,
    Warning,
    Error,
    /// A question: the next line typed is its answer (y/n).
    Ask,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
}

impl Line {
    fn said(t: impl Into<String>) -> Line {
        Line { kind: Kind::Said, text: t.into() }
    }
    fn dim(t: impl Into<String>) -> Line {
        Line { kind: Kind::Dim, text: t.into() }
    }
    fn warn(t: impl Into<String>) -> Line {
        Line { kind: Kind::Warning, text: t.into() }
    }
    fn error(t: impl Into<String>) -> Line {
        Line { kind: Kind::Error, text: t.into() }
    }
    fn ask(t: impl Into<String>) -> Line {
        Line { kind: Kind::Ask, text: t.into() }
    }
}

pub const HELP: &str = "\
the language (GRML): node, relate, keep, observe, forget, var, set, carry, learn, input, output, fuse,
  network ... end, call, use FILE.grm
the clock:      tick [N]            N ticks: it settles, the sets write
                show [NAME...]      what the nodes are now (value ± sigma)
                status              harmony, rounds, messages, frame
                joint NAME...       what the network believes of them together (a vector: its components)
sandboxes:      sandbox NAME        opens an isolated network beside the global one
                in NAME: STATEMENT  says it in the sandbox (the global network is not touched)
                run NAME            the sandbox computes itself
                trial NAME          what a merge would change (nothing is changed)
                merge NAME          merges it, little by little (it rises tick by tick)
                refuse NAME         drops it
the immune system:
                sources             where everything came from
                quarantine SOURCE   it stops holding, what it taught is taken back
                release SOURCE      it rises again, little by little
                recall SOURCE       everything that came from it, taken away (asks first)
                remove NAME         a network (a source), a node or a relation, taken away (asks first)
                journal             the logbook
time:           track NODE [LAGS] [with A B..]  it remembers, learns how it moves, can be foreseen
                forecast NODE [STEPS]           what it will be, with its spread
                why NODE [STEPS]                the confidence of a forecast, and what it rests on
                rewind NODE MOMENT              what it was (MOMENT < 0: so many moments ago)
                embody NODE                     the machine holds its hidden law as its own relations
                insights                        the links it found by itself (and those it stopped believing in)
                discover                        the laws that hold at the same moment (also after a file)
the world:      connect meteo LAT LON [DAYS]    the weather of a place, hour by hour (Open-Meteo)
                connect owid SLUG ENTITY        a chart of Our World in Data, for a country (or World)
                connect file FILE.json|csv      an answer saved before
                take NAME N N...                the fields chosen become sandbox NAME (then trial, merge)
                feed NAME                       once merged: its rows come in, each at its moment
the eye:        see FILE.png [NAME]             an eye sees it: cleaned, surprised, what changed (pictures written
                                                beside it), its glance heard by the network as inputs
apps:           apps                            those running, those that can be opened
                app new NAME                    a new app (a file in the apps' folder)
                app open FILE|NAME              it runs: its network in the world, its surface, its APIs
                app close NAME                  it stops (its file stays; what it lived through is kept)
                app undo|redo NAME              its last edit taken back, or given again
                (an app's file: the language, `widget NAME: KIND NODE..`, `api ...`, `read ...`, `clock S`,
                commands; widgets: slider toggle button meter value plot text picture pad keys play scope)
APIs:           api NAME: get URL [every S]     asked again every S seconds
                api NAME: post URL every S with NODE..        values sent
                api NAME: websocket URL [send NODE.. every S] a stream
                read NAME PATH -> NODE [± SIGMA]              what is read into a node (PATH: a.b[0].c, [-1] the last)
sound:          sound on|off                    the machine's own voice (calm, capped)
                voice pad|engine|wind           how it sounds
                volume V                        all of it, 0 .. 1
                play NODE [GAIN]                a node played at the speed of sound (its network's sets run 48 000
                                                times a second, tuned by the network)
                stop|mute|unmute|solo NODE      a channel of the mixer
                gain NODE G, pan NODE P         its level (0 .. 2), its place (-1 left .. 1 right)
                mixer                           what plays
                panic                           everything down, in a breath
                render NODE SECONDS FILE.wav    a node played into a file
files:          pwd, cd [DIR|..|~|-], ls [-l] [-a] [PATTERN], head|tail [-n N] FILE, cat FILE, mkdir [-p] DIR,
                cp [-r] FROM.. TO, mv FROM.. TO, rm [-r] PATH.. (asks first), touch FILE
                new NAME.ext (a file, opened to write), new NAME (a program), edit FILE, programs, run NAME
data:           load FILE.csv [COLUMN...]       a row a tick, into its columns' nodes (asked for when not said)
asking it:      ask NODE, put NAME = VALUE, why NODE, inspect NAME, outputs, master [SOURCE], check, stats
                focus [NAME|all], play [HZ] / pause (the clock), undo / redo (the whole world, 30 kept)
the prompt:     history [N], !N, !!, clear, A ; B (several at once), help WORD, tour, lang fr|en
looks:          look NAME: accent = #rrggbb, background = .., text = .., round = N, size = N [when NODE in ..]
                style STATEMENT (into the looks' own network: hour, warmth...), widget ... look NAME
memory:         sleep [FILE]        writes the whole world (it also falls asleep by itself on quit, in grm.world)
                wake FILE           the world as it fell asleep there
                forget everything   a new world (asks first)
the machine:    threads N|all       how many threads share a round (the numbers do not change)
                help, quit";

/// Where the world sleeps between two sessions: $GRM_MEMORY, or grm.world in the folder it is started from.
/// The words of a command, a path among them: "between quotes" a path may have spaces; without quotes, words that
/// together name a file or folder that exists are one (`see C:\Pictures\Capture d'écran 1.png eye`).
pub fn path_words(rest: &str) -> Vec<String> {
    let mut tokens: Vec<(String, bool)> = vec![];
    let mut current = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    for c in rest.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                was_quoted = true;
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() || was_quoted {
                    tokens.push((std::mem::take(&mut current), was_quoted));
                }
                was_quoted = false;
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() || was_quoted {
        tokens.push((current, was_quoted));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        let mut took = 1;
        if !tokens[i].1 {
            for j in (i + 2..=tokens.len()).rev() {
                if tokens[i..j].iter().any(|t| t.1) {
                    continue;
                }
                let joined = tokens[i..j].iter().map(|t| t.0.as_str()).collect::<Vec<_>>().join(" ");
                if crate::fs::expand(&joined).exists() {
                    took = j - i;
                    break;
                }
            }
        }
        out.push(tokens[i..i + took].iter().map(|t| t.0.as_str()).collect::<Vec<_>>().join(" "));
        i += took;
    }
    out
}

/// The links the host's memory allows now (read again at most every ten seconds).
pub fn host_capacity() -> usize {
    static LAST: std::sync::Mutex<Option<(std::time::Instant, usize)>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, n)) = *last {
        if at.elapsed() < Duration::from_secs(10) {
            return n;
        }
    }
    let free = sysinfo::System::new()
        .map(|mut sys| {
            sys.refresh_memory();
            sys.available_memory()
        })
        .unwrap_or(0);
    let n = grm_engine::mind::host_capacity((free > 0).then_some(free));
    *last = Some((std::time::Instant::now(), n));
    n
}

/// A path said back to the prompt: between quotes when it has spaces.
pub fn quoted(path: &str) -> String {
    if path.contains(char::is_whitespace) {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

impl Shell {
    /// The app that writes this node or relation, and its line: only when what the network holds under that name
    /// came from that app (a node of the same name from a table, or a widget of the same name, is not the app's).
    fn app_owning(&self, engine: &Engine, thing: &str) -> Option<(String, usize)> {
        let (app, l) = self.owner_of(thing)?;
        let line = self.rt.apps.get(&app)?.text.lines().nth(l - 1).unwrap_or("").trim().to_string();
        if line.split_whitespace().next() == Some("widget") {
            return None;
        }
        let name = thing.to_string();
        let source = engine.query(move |w| {
            let net = &w.machine.net;
            net.nodes
                .get(&name)
                .map(|s| s.origin.source.clone())
                .or_else(|| net.relations.iter().find(|r| r.name == name).map(|r| r.origin.source.clone()))
        })??;
        (source == runtime::source_of(&app)).then_some((app, l))
    }
}

pub fn memory() -> PathBuf {
    std::env::var_os("GRM_MEMORY").map(PathBuf::from).unwrap_or_else(|| "grm.world".into())
}

/// The world as it fell asleep (or a new one), and what to say about it.
pub fn wake_or_born(path: &std::path::Path) -> (World, Option<Line>) {
    if !path.exists() {
        return (World::new(), None);
    }
    match World::wake(path) {
        Ok(w) => {
            let said = format!(
                "woke from {}: {} node(s), {} relation(s), {} in time",
                path.display(),
                w.net().nodes.len(),
                w.net().relations.len(),
                w.time.nodes.len()
            );
            (w, Some(Line::dim(said)))
        }
        Err(e) => (World::new(), Some(Line::warn(format!("{e}: it starts afresh (the file is left as it is)")))),
    }
}

/// What the world last answered: its fields, and how to make a table of the chosen ones.
pub struct Answer {
    pub fields: Vec<Field>,
    json: Option<serde_json::Value>,
    csv: Option<(String, String, Option<serde_json::Value>)>, // (csv, entity, metadata)
}

impl Answer {
    fn table(&self, chosen: &[String]) -> grm_engine::Result<Table> {
        let refs: Vec<&str> = chosen.iter().map(|s| s.as_str()).collect();
        if let Some(j) = &self.json {
            return grm_connect::table(j, &refs);
        }
        let (csv, entity, meta) = self.csv.as_ref().unwrap();
        let cols: Vec<&str> = refs.iter().map(|p| p.trim_start_matches("[].")).collect();
        grm_connect::owid_table(csv, entity, &cols, meta.as_ref())
    }
}

/// What the prompt asks of a window (a terminal leaves them).
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// A file to show, to write in.
    OpenFile(PathBuf),
    /// The folder changed.
    Folder,
    /// A network brought to the front (None: all of them).
    Focus(Option<String>),
    /// A table to load: which columns?
    Columns {
        path: PathBuf,
        names: Vec<String>,
        written: Vec<String>,
        numeric: Vec<bool>,
    },
    /// A table whose columns would go into nodes that already hold other data: which names?
    Collide {
        path: PathBuf,
        /// The file's name, as a name (a prefix to offer).
        stem: String,
        columns: Vec<String>,
        into: indexmap::IndexMap<String, String>,
        /// The nodes already holding other data.
        taken: Vec<String>,
    },
    /// A workshop to show.
    Workshop(String),
    Tour,
    Lang(String),
    Clear,
}

/// The prompt.
pub struct Shell {
    pub memory: PathBuf,
    asking: Option<(String, String)>,
    answer: Option<Answer>,
    tables: HashMap<String, Table>,
    pub history: Vec<String>,
    /// The mixer the sound device plays (or nothing plays it: `render` still works).
    pub mixer: Arc<Mutex<Mixer>>,
    feel_at: Option<(Instant, u64, usize)>,
    /// What the sound was last told of each parameter, and how many times it was told only what changed (every so
    /// often it is told everything again: a channel or a world that came since hears it all).
    tuned: std::collections::HashMap<String, f64>,
    tunings: u32,
    /// The last picture each eye saw, cleaned, surprised, changed (for a window to show).
    pub sights: IndexMap<String, grm_engine::vision::Sight>,
    pub(crate) rt: runtime::Runtime,
    /// Whether the desk is kept beside the world's memory (not in a test run).
    pub keep_desk: bool,
    /// The app that is always there, opened before the desk and never closed (her: `mind/home.grm`).
    pub always: Option<PathBuf>,
    /// What it asks of the window.
    pub orders: Vec<Order>,
    /// Whether a window is there to answer (to choose a table's columns, to open a file).
    pub window: bool,
    before_cd: Option<PathBuf>,
    /// Who the machine speaks of (never to): set by `human NAME`, else the host's user name.
    pub human: Option<String>,
    /// Whether it answers in French (`lang fr`).
    pub french: bool,
    /// The world as it was before each change (`undo`), and what was taken back (`redo`).
    pub(crate) undo_world: Vec<World>,
    pub(crate) redo_world: Vec<World>,
    /// How many times the world was kept before a change (the window counts what can be taken back).
    pub snapshots: u64,
}

pub(crate) fn said(answer: grm_engine::Result<String>) -> Vec<Line> {
    match answer {
        Ok(s) if s.is_empty() => vec![],
        Ok(s) => s.lines().map(|l| Line::said(l.trim_start())).collect(),
        Err(e) => e.message.lines().map(|l| Line::error(format!("! {}", l.trim_start()))).collect(),
    }
}

impl Shell {
    pub fn new(memory: PathBuf, mixer: Arc<Mutex<Mixer>>) -> Shell {
        Shell {
            memory,
            asking: None,
            answer: None,
            tables: HashMap::new(),
            history: vec![],
            mixer,
            feel_at: None,
            tuned: Default::default(),
            tunings: 0,
            sights: IndexMap::new(),
            rt: Default::default(),
            keep_desk: true,
            always: None,
            orders: vec![],
            window: false,
            before_cd: None,
            human: None,
            french: false,
            undo_world: vec![],
            redo_world: vec![],
            snapshots: 0,
        }
    }

    /// Whether the next line answers a question (y/n).
    pub fn asking(&self) -> bool {
        self.asking.is_some()
    }

    /// A line typed: what comes out, and whether it was the last (quit).
    /// What it asks of the window, since last asked.
    pub fn take_orders(&mut self) -> Vec<Order> {
        std::mem::take(&mut self.orders)
    }

    pub fn run(&mut self, engine: &Engine, line: &str) -> (Vec<Line>, bool) {
        let line = line.trim().to_string();
        // several commands on a line, separated by `;`
        if self.asking.is_none() && line.contains(';') && !line.contains('"') {
            let mut out = vec![];
            for part in line.split(';').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                let (lines, quit) = self.run(engine, part);
                out.extend(lines);
                if quit {
                    return (out, true);
                }
            }
            return (out, false);
        }
        // the history: !! the last line again, !N line N again
        if let Some(rest) = line.strip_prefix('!').filter(|_| self.asking.is_none()) {
            let again = if rest == "!" {
                self.history.last().cloned()
            } else {
                rest.parse::<usize>().ok().and_then(|n| self.history.get(n.wrapping_sub(1)).cloned())
            };
            return match again {
                Some(l) => {
                    let (mut out, quit) = self.run(engine, &l);
                    out.insert(0, Line::dim(l));
                    (out, quit)
                }
                None => (vec![Line::warn("? nothing there in the history")], false),
            };
        }
        if let Some((what, name)) = self.asking.take() {
            if matches!(line.to_lowercase().as_str(), "y" | "yes" | "o" | "oui") {
                if what == "app-line" {
                    let (app, l) = name.split_once(' ').unwrap_or(("", "0"));
                    let l: usize = l.parse().unwrap_or(0);
                    let Some(text) = self.rt.apps.get(app).map(|a| a.text.clone()) else { return (vec![], false) };
                    let new = apps::with_line(&text, l, None);
                    let mut out = vec![Line::said(format!("line {l} taken from {app}"))];
                    out.extend(self.edit_app(engine, app, new));
                    return (out, false);
                }
                if what == "rm" {
                    return (self.remove_files(&name), false);
                }
                if what == "quarantine" {
                    return (said(engine.ask(move |w| w.quarantine(&name, "said by the human"))), false);
                }
                let answer = match what.as_str() {
                    "forget" => engine.ask(|w| {
                        *w = World::new();
                        Ok("a new world".to_string())
                    }),
                    "recall" => engine.ask(move |w| w.recall(&name)),
                    _ => engine.ask(move |w| {
                        let words: Vec<&str> = name.split_whitespace().collect();
                        let p = w.removal(&words)?;
                        w.remove(&p)
                    }),
                };
                return (said(answer), false);
            }
            return (vec![Line::said("nothing taken away")], false);
        }
        if line.is_empty() {
            return (vec![], false);
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        let (word, rest) = line.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((line.as_str(), ""));
        let rest = rest.to_string();
        // the world as it is, kept before anything that may change it (`undo` gives it back)
        const READING: &[&str] = &[
            "show",
            "status",
            "ask",
            "why",
            "inspect",
            "ins",
            "outputs",
            "outs",
            "master",
            "ms",
            "check",
            "chk",
            "journal",
            "sources",
            "insights",
            "forecast",
            "rewind",
            "help",
            "h",
            "history",
            "pwd",
            "ls",
            "head",
            "tail",
            "cat",
            "programs",
            "progs",
            "mixer",
            "apps",
            "undo",
            "redo",
            "stats",
            "joint",
            "focus",
            "fc",
            "clear",
            "tour",
            "lang",
            "cd",
            "sound",
            "voice",
            "volume",
            "mute",
            "unmute",
            "solo",
            "gain",
            "pan",
            "panic",
            "stop",
            "trial",
            "play",
            "pause",
            "render",
            "edit",
            "open",
            "new",
            "threads",
            "sleep",
            "mkdir",
            "rent",
            "touch",
            "cp",
            "mv",
            "rm",
            "witness",
            "conscience",
            "human",
        ];
        // an app keeps its own texts, and opening or closing one is taken back by closing or opening it: the world
        // is not kept for them
        if !READING.contains(&word) && word != "app" {
            if let Some(w) = engine.query(|w| w.clone()) {
                self.snapshots += 1;
                self.undo_world.push(w);
                if self.undo_world.len() > 30 {
                    self.undo_world.remove(0);
                }
                self.redo_world.clear();
            }
        }
        if let Some(out) = self.more(engine, word, &rest) {
            return (out, false);
        }
        let out = match word {
            "quit" | "q" | "exit" => return (vec![], true),
            "help" | "h" => HELP.lines().map(Line::said).collect(),
            "sleep" => {
                let to = if rest.is_empty() { self.memory.clone() } else { PathBuf::from(&rest) };
                said(engine.ask(move |w| w.sleep(&to).map(|_| format!("asleep in {} (it goes on)", to.display()))))
            }
            "wake" => {
                let from = PathBuf::from(&rest);
                said(engine.ask(move |w| {
                    *w = World::wake(&from)?;
                    Ok(format!("woke from {}", from.display()))
                }))
            }
            "forget" if rest == "everything" => {
                self.asking = Some(("forget".into(), String::new()));
                vec![Line::ask("! forget everything: a new world, nothing kept? (y/n)")]
            }
            "tick" | "t" => {
                let n: u64 = rest.parse().unwrap_or(1);
                said(engine.ask(move |w| {
                    let mut last = Default::default();
                    for _ in 0..n {
                        last = w.tick(false);
                    }
                    Ok(last.iter().map(|(k, v)| format!("{k} <- {}", g(*v))).collect::<Vec<_>>().join(", "))
                }))
            }
            "show" => {
                let s = engine.settled(Duration::from_secs(2));
                let names: Vec<&str> = rest.split_whitespace().collect();
                s.values
                    .iter()
                    .filter(|(n, _)| names.is_empty() || names.contains(&n.as_str()))
                    .map(|(n, (v, sg))| Line::said(format!("{n} = {} ± {}", g(*v), g(*sg))))
                    .collect()
            }
            "status" => {
                let s = engine.snapshot();
                let mut out = vec![Line::said(format!(
                    "harmony {:.3}, {} rounds, {} messages, {} ticks, frame {} ({:?}){}",
                    s.harmony,
                    s.rounds,
                    s.messages,
                    s.ticks,
                    s.frame,
                    s.compute,
                    if s.settled { ", settled" } else { ", computing" }
                ))];
                if !s.quarantine.is_empty() {
                    out.push(Line::warn(format!("in quarantine: {}", s.quarantine.join(", "))));
                }
                out
            }
            "sandbox" => said(engine.ask(move |w| w.open_sandbox(&rest).map(|_| format!("sandbox {rest} opened")))),
            "in" => match rest.split_once(':') {
                None => vec![Line::warn("? in NAME: STATEMENT")],
                Some((name, stmt)) => {
                    let (name, stmt) = (name.trim().to_string(), stmt.trim().to_string());
                    said(engine.ask(move |w| Ok(w.say_in(&name, &stmt)?.unwrap_or_else(|| "? not a statement".into()))))
                }
            },
            "run" => said(engine.ask(move |w| {
                let r = w.run_sandbox(&rest, 10000)?;
                let m = w.sandboxes[&rest].machine.as_ref().unwrap();
                let vals: Vec<String> = m.net.nodes.keys().map(|n| format!("{n} = {}", g(m.value_si(n)))).collect();
                Ok(format!("{r} rounds\n{}", vals.join("\n")))
            })),
            "trial" => said(engine.ask(move |w| Ok(w.trial_merge(&rest, 10000)?.text()))),
            "merge" => said(engine.ask(move |w| w.accept_merge(&rest))),
            "refuse" => said(engine.ask(move |w| w.refuse_merge(&rest))),
            "sources" => said(engine.ask(|w| {
                Ok(w.sources()
                    .iter()
                    .map(|(s, n)| {
                        format!("{s}: {n} relation(s){}", if w.quarantine.contains(s) { " (quarantine)" } else { "" })
                    })
                    .collect::<Vec<_>>()
                    .join("\n"))
            })),
            "quarantine" if rest.starts_with("app:") => {
                self.asking = Some(("quarantine".into(), rest.clone()));
                vec![Line::ask(format!("! {rest} is a running app: quarantine it (it stops holding)? (y/n)"))]
            }
            "recall" if rest.starts_with("app:") => {
                vec![Line::warn(format!("? {rest} is a running app: app close {} (its file stays)", &rest[4..]))]
            }
            "quarantine" => said(engine.ask(move |w| w.quarantine(&rest, "said by the human"))),
            "release" => said(engine.ask(move |w| w.release(&rest))),
            "recall" => {
                let name = rest.clone();
                match engine.ask(move |w| {
                    let (n, r) = w.net().from_source(&name);
                    Ok(format!("{} node(s), {} relation(s)", n.len(), r.len()))
                }) {
                    Ok(what) => {
                        self.asking = Some(("recall".into(), rest.clone()));
                        vec![Line::ask(format!("! recall {rest}: {what} at most (what others hold stays)? (y/n)"))]
                    }
                    Err(e) => vec![Line::error(format!("! {e}"))],
                }
            }
            "remove" if self.app_owning(engine, rest.split_whitespace().last().unwrap_or("")).is_some() => {
                let thing = rest.split_whitespace().last().unwrap_or("").to_string();
                let (app, l) = self.app_owning(engine, &thing).unwrap();
                let text = self.rt.apps[&app].text.lines().nth(l - 1).unwrap_or("").trim().to_string();
                self.asking = Some(("app-line".into(), format!("{app} {l}")));
                vec![Line::ask(format!(
                    "! {thing} is written by the app {app}, line {l}: «{text}». Take this line from it? (y/n)"
                ))]
            }
            "remove" => {
                let name = rest.clone();
                match engine.ask(move |w| {
                    let words: Vec<&str> = name.split_whitespace().collect();
                    let p = w.removal(&words)?;
                    Ok(format!("{} {}: {} node(s), {} relation(s)", p.kind, p.name, p.nodes.len(), p.relations.len()))
                }) {
                    Ok(what) => {
                        let base = if rest.split_whitespace().last() == Some("base") {
                            "  (its base: what it was born with)"
                        } else {
                            ""
                        };
                        self.asking = Some(("remove".into(), rest.clone()));
                        vec![Line::ask(format!("! remove {what}? (y/n){base}"))]
                    }
                    Err(e) => vec![Line::warn(format!("? {e}"))],
                }
            }
            "journal" => said(engine.ask(|w| {
                Ok(w.journal
                    .iter()
                    .map(|e| format!("[{}] {}: {}", e.tick, e.kind, e.text))
                    .collect::<Vec<_>>()
                    .join("\n"))
            })),
            "track" => {
                let words: Vec<String> = rest.split_whitespace().map(|w| w.to_string()).collect();
                match words.first().cloned() {
                    None => vec![Line::warn("? track NODE [LAGS] [with A B..]")],
                    Some(node) => {
                        let lags = words.get(1).and_then(|l| l.parse().ok()).unwrap_or(2);
                        let with: Vec<String> = words.iter().skip_while(|w| *w != "with").skip(1).cloned().collect();
                        said(engine.ask(move |w| {
                            let c: Vec<&str> = with.iter().map(|s| s.as_str()).collect();
                            w.track(&node, lags, &c)
                        }))
                    }
                }
            }
            "forecast" => {
                let mut words = rest.split_whitespace();
                let node = words.next().unwrap_or("").to_string();
                let steps: usize = words.next().and_then(|s| s.parse().ok()).unwrap_or(24);
                said(engine.ask(move |w| {
                    if !w.time.nodes.contains_key(&node) {
                        grm_engine::bail!("{node} does not live in time (track it first)");
                    }
                    let f = w.time.forecast(steps, 300, 1.0);
                    let p = &f.paths[&node];
                    Ok((0..steps)
                        .map(|h| {
                            format!(
                                "{}  {} ± {}  [{} .. {}]",
                                w.time.when(w.time.t + (h as f64 + 1.0) * w.time.dt),
                                g(p.mean[h]),
                                g(p.sigma[h]),
                                g(p.low[h]),
                                g(p.high[h])
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"))
                }))
            }
            "why" => {
                let mut words = rest.split_whitespace();
                let node = words.next().unwrap_or("").to_string();
                let steps: usize = words.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                said(engine.ask(move |w| Ok(w.time.why(&node, steps, 300)?.text(&w.node_sources()))))
            }
            "rewind" => {
                let mut words = rest.split_whitespace();
                let node = words.next().unwrap_or("").to_string();
                let at: f64 = words.next().and_then(|s| s.parse().ok()).unwrap_or(-1.0);
                said(engine.ask(move |w| {
                    let t = if at < 0.0 { w.time.t + at } else { at };
                    let (v, spread) = w.time.rewind(&node, t)?;
                    Ok(format!("{node} at {}: {} (as remembered: ± {})", w.time.when(t), g(v), g(spread)))
                }))
            }
            "discover" => said(engine.ask(|w| {
                let found = w.discover();
                Ok(if found.is_empty() {
                    "no new law holds on the rows it has seen".to_string()
                } else {
                    found.join("\n")
                })
            })),
            "joint" => said(engine.ask(move |w| {
                let names: Vec<&str> = rest.split_whitespace().collect();
                Ok(w.joint(&names)?.text())
            })),
            "see" => self.see(engine, &rest),
            "embody" => said(engine.ask(move |w| w.embody(&rest))),
            "insights" => said(engine.ask(|w| {
                Ok(w.time
                    .insights
                    .iter()
                    .map(|i| format!("at {}: {}", w.time.when(i.t), i.text()))
                    .collect::<Vec<_>>()
                    .join("\n"))
            })),
            "connect" => match connect(&rest) {
                Ok(a) => {
                    let mut out: Vec<Line> = a
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(k, f)| {
                            let text = format!("{:>2}. {}", k + 1, f.text());
                            if f.kind == FieldKind::Number {
                                Line::said(text)
                            } else {
                                Line::dim(text)
                            }
                        })
                        .collect();
                    out.push(Line::said("take NAME N N...: the fields chosen become a sandbox"));
                    self.answer = Some(a);
                    out
                }
                Err(e) => vec![Line::error(format!("! {e}"))],
            },
            "take" => self.take(engine, &rest),
            "feed" => match self.tables.get(&rest).cloned() {
                None => vec![Line::warn(format!("? no rows waiting for {rest}"))],
                Some(table) => said(engine.ask(move |w| {
                    for n in table.numbers.keys() {
                        if !w.time.nodes.contains_key(n) {
                            w.track(n, 3, &[])?;
                        }
                    }
                    w.time.unit = table.time_unit.clone();
                    let fed = w.feed_rows(&table.rows())?;
                    let found: Vec<String> = w.time.insights.iter().map(|i| i.text()).collect();
                    Ok(format!(
                        "{fed} rows, each at its moment{}",
                        if found.is_empty() { String::new() } else { format!("\n{}", found.join("\n")) }
                    ))
                })),
            },
            "threads" => {
                let t: Option<usize> = rest.parse().ok();
                said(engine.ask(move |w| {
                    w.machine.set_threads(t);
                    Ok(format!("threads: {}", t.map(|t| t.to_string()).unwrap_or("all".into())))
                }))
            }
            "app" => self.app_command(engine, &rest),
            "apps" => self.app_command(engine, ""),
            "api" | "read" => self.desk_api(engine, &line),
            "sound" | "voice" | "volume" | "play" | "stop" | "mute" | "unmute" | "solo" | "gain" | "pan" | "mixer"
            | "panic" | "render" => self.sound(engine, word, &rest),
            _ => {
                let origin = Provenance::of("typed");
                let typed = line.clone();
                match engine.ask(move |w| match w.say(&typed, &origin)? {
                    Some(s) => Ok(s),
                    None => Ok(format!("? not understood: {} (help)", typed.split_whitespace().next().unwrap_or(""))),
                }) {
                    Ok(s) if s.is_empty() => vec![],
                    Ok(s) if s.starts_with("? ") => vec![Line::warn(s)],
                    other => said(other),
                }
            }
        };
        (out, false)
    }

    fn see(&mut self, engine: &Engine, rest: &str) -> Vec<Line> {
        let words = path_words(rest);
        let file = crate::fs::expand(words.first().map(String::as_str).unwrap_or(""));
        let name = words.get(1).cloned().unwrap_or("eye".into());
        let picture = match grm_connect::read_picture(&file) {
            Ok(p) => p,
            Err(e) => return vec![Line::error(format!("! {e}"))],
        };
        // a big picture is made smaller first: the eye sees it in seconds, not in minutes
        let (w0, h0) = (picture.width, picture.height);
        let picture = picture.shrunk(SIGHT);
        let smaller = (picture.width, picture.height) != (w0, h0);
        let eye = name.clone();
        let size = (picture.width, picture.height, picture.channels);
        let colour = picture.mean_oklab();
        let Some(sight) = engine.query(move |w| w.see(&eye, &picture, Duration::from_secs(20))) else {
            return vec![Line::error("! the engine has stopped")];
        };
        let sight = match sight {
            Ok(s) => s,
            Err(e) => return vec![Line::error(format!("! {e}"))],
        };
        let stem = file.with_extension("");
        let write = |tag: &str, p: &grm_engine::vision::Picture| {
            let path = PathBuf::from(format!("{}-{tag}.png", stem.display()));
            grm_connect::write_picture(&path, p).map(|_| path.display().to_string())
        };
        let mut out = vec![];
        if smaller {
            out.push(Line::dim(format!("made smaller to be seen: {w0}x{h0} -> {}x{}", size.0, size.1)));
        }
        out.push(Line::said(format!(
            "{name} saw {}x{}x{} in {} rounds ({:.2?})",
            size.0, size.1, size.2, sight.rounds, sight.elapsed
        )));
        match write("clean", &sight.clean) {
            Ok(p) => out.push(Line::dim(format!("cleaned: {p}"))),
            Err(e) => out.push(Line::error(format!("! {e}"))),
        }
        if let Ok(p) = write("surprise", &grm_connect::sigma_picture(&sight.surprise)) {
            out.push(Line::dim(format!("surprise: {p}")));
        }
        if let Some(change) = &sight.change {
            if let Ok(p) = write("change", &grm_connect::sigma_picture(change)) {
                out.push(Line::dim(format!("what changed: {p}")));
            }
        }
        let top: Vec<String> = sight.surprises(3).iter().map(|(x, y, s)| format!("({x}, {y}): {s:.1} sigma")).collect();
        out.push(Line::said(format!("most surprised at {}", top.join(", "))));
        out.push(Line::said(format!(
            "its glance ({}x{}) is heard by the network as {name}_glance_*",
            sight.glance.0, sight.glance.1
        )));
        self.sights.insert(name, sight);
        // her eye: she is given the picture too (its colour, and that one more came), when she is there
        let (l, a, b) = colour;
        let given = engine
            .query(move |w| -> grm_engine::Result<String> {
                if !w.machine.net.nodes.contains_key("elle_seen_n") {
                    return Ok(String::new());
                }
                let n = w.machine.value_si("elle_seen_n").round() + 1.0;
                let origin = grm_engine::Provenance::of("eye:elle");
                for line in [
                    format!("observe elle_seen_l = {l:.4}"),
                    format!("observe elle_seen_a = {a:.4}"),
                    format!("observe elle_seen_b = {b:.4}"),
                    format!("observe elle_seen_n = {n}"),
                ] {
                    w.say(&line, &origin)?;
                }
                Ok(format!("she sees it too ({l:.2}, {a:+.2}, {b:+.2})"))
            })
            .and_then(|r| r.ok())
            .unwrap_or_default();
        if !given.is_empty() {
            out.push(Line::dim(given));
        }
        out
    }

    fn take(&mut self, engine: &Engine, rest: &str) -> Vec<Line> {
        let mut words = rest.split_whitespace();
        let name = words.next().unwrap_or("").to_string();
        let Some(a) = &self.answer else { return vec![Line::warn("? connect first")] };
        let chosen: Vec<String> = words
            .filter_map(|n| n.parse::<usize>().ok())
            .filter_map(|k| a.fields.get(k.wrapping_sub(1)))
            .map(|f| f.path.clone())
            .collect();
        match a.table(&chosen) {
            Ok(table) => {
                let mut out: Vec<Line> = table
                    .texts
                    .iter()
                    .map(|(n, texts)| {
                        Line::dim(format!("{n}: {} text(s), shown but not yet turned into nodes", texts.len()))
                    })
                    .collect();
                let t = table.clone();
                let sandbox = name.clone();
                out.extend(said(engine.ask(move |w| {
                    let nodes = grm_connect::into_sandbox(w, &sandbox, &t)?;
                    let mut text = format!(
                        "sandbox {sandbox}: {} ({} rows, by {}) — trial {sandbox}, then merge {sandbox}, then feed {sandbox}",
                        nodes.join(", "),
                        t.time.len(),
                        t.time_unit
                    );
                    // as with a table from a file, she looks by herself for what holds in it (between its columns,
                    // and each against its time: a line, a power, a swing...), and says it into the same sandbox
                    let rows: Vec<IndexMap<String, f64>> = (0..t.time.len())
                        .map(|k| {
                            std::iter::once(("time".to_string(), t.time[k]))
                                .chain(t.numbers.iter().filter_map(|(n, v)| v.get(k).copied().flatten().map(|v| (n.clone(), v))))
                                .collect()
                        })
                        .collect();
                    for (k, law) in grm_engine::discover::find_laws(&rows, &[]).iter().enumerate() {
                        let name = format!("{sandbox}_found_{}", k + 1);
                        let lines = law.statements(&name, &|n| w.machine.net.nodes.get(n).is_some_and(|s| !s.signed));
                        let said: Result<(), GrmError> = lines.iter().try_for_each(|l| w.say_in(&sandbox, l).map(|_| ()));
                        if said.is_ok() {
                            text.push_str(&format!("\nfound by itself: {}", law.text()));
                        }
                    }
                    Ok(text)
                })));
                self.tables.insert(name, table);
                out
            }
            Err(e) => vec![Line::error(format!("! {e}"))],
        }
    }

    // ------------------------------------------------------------------ sound
    /// A reflex of a node of the world: its network's sets, played at the speed of sound.
    pub(crate) fn reflex_of(engine: &Engine, node: &str, rate: u32) -> grm_engine::Result<Reflex> {
        Self::reflex_in(engine, None, node, rate)
    }

    /// A node's reflex, from the global network or from a workshop.
    pub(crate) fn reflex_in(
        engine: &Engine,
        workshop: Option<&str>,
        node: &str,
        rate: u32,
    ) -> grm_engine::Result<Reflex> {
        Self::reflex_following(engine, workshop, &[node.to_string()], rate)
    }

    /// The reflex of a world's whole network, following these nodes too (none: only what its sets write and what
    /// follows from them).
    pub(crate) fn reflex_following(
        engine: &Engine,
        workshop: Option<&str>,
        nodes: &[String],
        rate: u32,
    ) -> grm_engine::Result<Reflex> {
        let nodes = nodes.to_vec();
        let workshop = workshop.map(String::from);
        engine
            .query(move |main| -> grm_engine::Result<Reflex> {
                let w: &World = match &workshop {
                    Some(n) => match main.workshops.get(n) {
                        Some(ws) => ws,
                        None => grm_engine::bail!("no workshop {n}"),
                    },
                    None => main,
                };
                for node in &nodes {
                    if !w.machine.net.nodes.contains_key(node) {
                        grm_engine::bail!("no node {node}");
                    }
                }
                let mut r = Reflex::of(&w.machine, Clock::Audio { rate })?;
                // what follows from the sets, computed at each sample too (as far as a reflex can): every node a
                // relation gives from what is already computed, in the order it is reached
                let net = &w.machine.net;
                // (a set: thousands of nodes are looked up at each pass, a list made it grow with their square)
                let mut known: std::collections::HashSet<String> = net.sets.iter().map(|s| s.target.clone()).collect();
                // (what the reflex does not compute: a set's next value, what is measured or fixed)
                let settled = |n: &String| {
                    n.contains("__next") || net.measured.contains_key(n) || net.nodes.get(n).is_some_and(|s| s.fixed)
                };
                // (what it reads as it is: an input, a variable no set writes)
                let given = |n: &String| net.nodes.get(n).is_some_and(|s| s.input || s.var);
                // first, each node by a relation all of whose other nodes are already computed (in the order they
                // are reached: what it reads is up to date); then, what is left, by any relation that gives it
                for strict in [true, false] {
                    loop {
                        let mut grew = false;
                        for rel in &net.relations {
                            let nodes = rel.nodes();
                            if !nodes.iter().any(|n| known.contains(n)) {
                                continue;
                            }
                            if strict && nodes.iter().any(|n| n.contains("__next")) {
                                continue;
                            }
                            for n in &nodes {
                                let free = !known.contains(n) && !settled(n);
                                if !free {
                                    continue;
                                }
                                let done = if strict {
                                    nodes.iter().all(|m| m == n || known.contains(m) || settled(m) || given(m))
                                        && r.follow_by(rel, n).is_ok()
                                } else {
                                    r.follow(&w.machine, n).is_ok()
                                };
                                if done {
                                    known.insert(n.clone());
                                    grew = true;
                                }
                            }
                        }
                        if !grew {
                            break;
                        }
                    }
                }
                for node in &nodes {
                    if !known.contains(node) {
                        r.follow(&w.machine, node).map_err(|e| {
                            GrmError::new(format!(
                                "{node}: {e} (play a node its sets write, or one a relation gives from them)"
                            ))
                        })?;
                        known.insert(node.clone());
                    }
                }
                r.prime();
                Ok(r)
            })
            .unwrap_or_else(|| Err(GrmError::new("the engine has stopped")))
    }

    fn sound(&mut self, engine: &Engine, word: &str, rest: &str) -> Vec<Line> {
        // unplugged and plugged in again: the world's sound made anew from the network as it is now (what she
        // learned kept)
        if word == "sound" && rest.trim() == "again" {
            let memories = self.mixer.lock().map(|m| m.world_memories()).unwrap_or_default();
            self.refresh_channels(engine);
            if let Ok(mut m) = self.mixer.lock() {
                m.reset();
                m.world_restore(&memories);
            }
            return vec![Line::said("the sound unplugged and plugged in again")];
        }
        let mut words = rest.split_whitespace();
        let first = words.next().unwrap_or("").to_string();
        let number = words.next().and_then(|v| v.parse::<f64>().ok());
        let Ok(mut mixer) = self.mixer.lock() else { return vec![Line::error("! the mixer is not there")] };
        match word {
            "sound" => match first.as_str() {
                "on" => {
                    mixer.ambient_on = true;
                    mixer.ambient.fade_in();
                    vec![Line::said("its voice rises, slowly")]
                }
                "off" => {
                    mixer.ambient_on = false;
                    vec![Line::said("its voice goes quiet")]
                }
                _ => vec![Line::warn("? sound on|off|again")],
            },
            "voice" => {
                let kind = match first.as_str() {
                    "pad" => VoiceKind::Pad,
                    "engine" => VoiceKind::Engine,
                    "wind" => VoiceKind::Wind,
                    _ => return vec![Line::warn("? voice pad|engine|wind")],
                };
                let rate = mixer.rate;
                let volume = mixer.ambient.volume;
                mixer.ambient = grm_sound::ambient::Ambient::new(kind, volume, rate);
                // the new voice is heard at once if the voice was on (not after it is turned on again)
                if mixer.ambient_on {
                    mixer.ambient.fade_in();
                }
                vec![Line::said(format!("its voice: {first}"))]
            }
            "volume" => match first.parse::<f64>() {
                Ok(v) => {
                    mixer.master = v.clamp(0.0, 1.0);
                    vec![Line::said(format!("volume {:.2}", mixer.master))]
                }
                Err(_) => vec![Line::warn("? volume 0 .. 1")],
            },
            "play" => {
                let rate = mixer.rate;
                let mut taps = mixer.taps();
                drop(mixer);
                // the world's network plays it: one network for all that plays (one time, one of her)
                if !taps.contains(&first) {
                    taps.push(first.clone());
                }
                match self.refresh_world(engine, &taps) {
                    Ok(()) => {
                        let mut m = self.mixer.lock().unwrap();
                        if m.tap(&first, number.unwrap_or(0.5).clamp(0.0, 2.0)) {
                            vec![Line::said(format!("{first} plays, {} times a second (it rises from silence)", rate))]
                        } else {
                            vec![Line::error(format!("! {first} is not in its network"))]
                        }
                    }
                    Err(e) => vec![Line::error(format!("! {e}"))],
                }
            }
            "stop" => {
                if mixer.stop(&first) {
                    vec![Line::said(format!("{first} fades out"))]
                } else {
                    vec![Line::warn(format!("? {first} does not play"))]
                }
            }
            "mute" | "unmute" | "solo" | "gain" | "pan" => match mixer.channels.get_mut(&first) {
                None => vec![Line::warn(format!("? {first} does not play"))],
                Some(c) => {
                    match word {
                        "mute" => c.mute = true,
                        "unmute" => c.mute = false,
                        "solo" => c.solo = !c.solo,
                        "gain" => c.gain = number.unwrap_or(c.gain).clamp(0.0, 2.0),
                        _ => c.pan = number.unwrap_or(c.pan).clamp(-1.0, 1.0),
                    }
                    vec![Line::said(format!(
                        "{first}: gain {:.2}, pan {:+.2}{}{}",
                        c.gain,
                        c.pan,
                        if c.mute { ", muted" } else { "" },
                        if c.solo { ", solo" } else { "" }
                    ))]
                }
            },
            "mixer" => {
                let mut out = vec![Line::said(format!(
                    "volume {:.2}; its voice {} ({:.2}); peak {:.2}",
                    mixer.master,
                    if mixer.ambient_on { "on" } else { "off" },
                    mixer.ambient_gain,
                    mixer.peak
                ))];
                for c in mixer.views() {
                    out.push(Line::said(format!(
                        "{}: gain {:.2}, pan {:+.2}{}{} — peak {:.2}",
                        c.name,
                        c.gain,
                        c.pan,
                        if c.mute { ", muted" } else { "" },
                        if c.solo { ", solo" } else { "" },
                        c.peak
                    )));
                }
                out
            }
            "panic" => {
                mixer.panic();
                vec![Line::said("everything down, in a breath")]
            }
            "render" => {
                drop(mixer);
                let mut w = rest.split_whitespace();
                let (Some(node), Some(seconds), Some(file)) =
                    (w.next(), w.next().and_then(|s| s.parse::<f64>().ok()), w.next())
                else {
                    return vec![Line::warn("? render NODE SECONDS FILE.wav")];
                };
                let rate = 48000;
                match Self::reflex_of(engine, node, rate) {
                    Ok(r) => {
                        let Some(source) = ReflexSource::new(r, node) else {
                            return vec![Line::error(format!("! {node}"))];
                        };
                        let mut m = Mixer::new(rate);
                        m.master = 1.0;
                        m.play(node, Box::new(source), 1.0);
                        let mut frames = vec![0.0f32; 2 * (seconds.clamp(0.0, 600.0) * rate as f64) as usize];
                        m.render(&mut frames);
                        match grm_sound::write_wav(std::path::Path::new(file), &frames, rate) {
                            Ok(()) => vec![Line::said(format!("{node}: {seconds} s written to {file}"))],
                            Err(e) => vec![Line::error(format!("! {file}: {e}"))],
                        }
                    }
                    Err(e) => vec![Line::error(format!("! {e}"))],
                }
            }
            _ => vec![],
        }
    }

    /// What the machine feels moves its voice, and the channels hear the network's parameters. Called often
    /// (a few times a second): returns what is worth a bell or a swell, once.
    pub fn feel(&mut self, engine: &Engine) {
        self.hear_apis(engine);
        self.clock(engine);
        let s = engine.snapshot();
        self.remember_values(&s.values);
        let now = Instant::now();
        let journal_len = engine.query(|w| w.journal.len()).unwrap_or(0);
        let (activity, news) = match self.feel_at {
            Some((at, rounds, seen)) => {
                let dt = now.duration_since(at).as_secs_f64().max(1e-3);
                let rate = (s.rounds.saturating_sub(rounds)) as f64 / dt;
                (1.0 - (-rate / 40.0).exp(), journal_len.saturating_sub(seen))
            }
            None => (0.0, 0),
        };
        let kinds: Vec<String> = if news > 0 {
            engine
                .query(move |w| w.journal.iter().rev().take(news).map(|e| e.kind.clone()).collect())
                .unwrap_or_default()
        } else {
            vec![]
        };
        self.feel_at = Some((now, s.rounds, journal_len));
        // the voices: the relations that weigh most, each with how far it is from holding
        let voices: Vec<Voice> = engine
            .query(|w| {
                let m = &w.machine;
                let mut v: Vec<(f64, f64)> = m
                    .factors
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        matches!(f.kind, grm_engine::machine::FKind::Relation | grm_engine::machine::FKind::Nonlinear)
                            && f.w > 0.0
                    })
                    .map(|(k, f)| (f.w.ln(), m.residual(k).abs()))
                    .collect();
                v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                v.truncate(6);
                let top = v.first().map(|x| x.0).unwrap_or(1.0);
                let low = v.last().map(|x| x.0).unwrap_or(0.0);
                v.iter()
                    .map(|(w, r)| Voice {
                        weight: if top > low { 0.4 + 0.6 * (w - low) / (top - low) } else { 1.0 },
                        strain: *r,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let params = self.mixer.lock().map(|m| m.params()).unwrap_or_default();
        // only what moved since it was last told (the sound's lock is held for what changed, not for thousands of
        // nodes that did not)
        self.tunings = (self.tunings + 1) % 20;
        if self.tunings == 0 {
            self.tuned.clear();
        }
        let mut values: IndexMap<String, f64> = IndexMap::new();
        for p in &params {
            if let Some(v) = s.values.get(p).map(|v| v.0) {
                if self.tuned.get(p) != Some(&v) {
                    self.tuned.insert(p.clone(), v);
                    values.insert(p.clone(), v);
                }
            }
        }
        if let Ok(mut m) = self.mixer.lock() {
            m.ambient
                .feel(Feeling { activity, harmony: s.harmony, pleasure: 2.0 * s.harmony - 1.0, key: 1.0 }, &voices);
            for k in &kinds {
                match k.as_str() {
                    "found" | "insight" => m.ambient.discovery(1.5),
                    "revised" | "quarantine" => m.ambient.surprise(),
                    _ => {}
                }
            }
            m.tune(&values);
        }
    }

    /// The mixer's layers of the machine's voice.
    pub fn set_layers(&self, layers: Layers) {
        if let Ok(mut m) = self.mixer.lock() {
            m.ambient.set_layers(layers);
        }
    }
}

/// Asks the world (or a saved answer) and lists what it can give.
fn connect(rest: &str) -> grm_engine::Result<Answer> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let num = |k: usize| words.get(k).and_then(|w| w.parse::<f64>().ok());
    match words.first().copied() {
        Some("meteo") => {
            let (Some(lat), Some(lon)) = (num(1), num(2)) else { grm_engine::bail!("connect meteo LAT LON [DAYS]") };
            let days = num(3).unwrap_or(30.0) as u32;
            let url = grm_connect::open_meteo_url(lat, lon, &grm_connect::WEATHER, days, 1);
            let json: serde_json::Value =
                serde_json::from_str(&grm_connect::fetch(&url)?).map_err(|e| GrmError::new(e.to_string()))?;
            Ok(Answer { fields: grm_connect::discover(&json), json: Some(json), csv: None })
        }
        Some("owid") => {
            let (Some(slug), Some(_)) = (words.get(1), words.get(2)) else {
                grm_engine::bail!("connect owid SLUG ENTITY")
            };
            let entity = words[2..].join(" ");
            let (csv_url, meta_url) = grm_connect::owid_urls(slug);
            let csv = grm_connect::fetch(&csv_url)?;
            let meta = grm_connect::fetch(&meta_url).ok().and_then(|m| serde_json::from_str(&m).ok());
            csv_answer(csv, entity, meta)
        }
        Some("file") => {
            let path = words.get(1).copied().unwrap_or("");
            let text = std::fs::read_to_string(path).map_err(|e| GrmError::new(format!("{path}: {e}")))?;
            if path.ends_with(".csv") {
                csv_answer(text, words.get(2..).map(|w| w.join(" ")).unwrap_or_else(|| "World".into()), None)
            } else {
                let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| GrmError::new(e.to_string()))?;
                Ok(Answer { fields: grm_connect::discover(&json), json: Some(json), csv: None })
            }
        }
        _ => grm_engine::bail!("connect meteo LAT LON [DAYS] | connect owid SLUG ENTITY | connect file FILE"),
    }
}

fn csv_answer(csv: String, entity: String, meta: Option<serde_json::Value>) -> grm_engine::Result<Answer> {
    let records = grm_connect::csv_records(&csv);
    let fields = grm_connect::discover(&records)
        .into_iter()
        .filter(|f| !matches!(f.path.as_str(), "[].Entity" | "[].Code" | "[].Year"))
        .collect();
    Ok(Answer { fields, json: None, csv: Some((csv, entity, meta)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_spaces_is_one_word_quoted_or_not() {
        let dir = std::env::temp_dir().join(format!("grm-words-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Capture d'écran 2026-10-07 110132.png");
        std::fs::write(&file, b"x").unwrap();
        let f = file.display().to_string();
        assert_eq!(path_words(&format!("{f} eye")), vec![f.clone(), "eye".into()]);
        assert_eq!(path_words(&format!("\"{f}\" eye")), vec![f.clone(), "eye".into()]);
        assert_eq!(path_words("\"a b\" c"), vec!["a b".to_string(), "c".into()]);
        assert_eq!(path_words("a  b"), vec!["a".to_string(), "b".into()]);
        assert_eq!(quoted("a b"), "\"a b\"");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Someone else playing beside her: a tone going a tone up and down, a note each half second.
    const OTHER: &str = "var oc = 0\nset oc = oc + 1\nset oc = 0 when oc in 24000 .. 1000000000\nvar op = 220 positive\n\
        set op = 246.94 when oc in 23999.5 .. 24000.5 and op in 0 .. 230\nset op = 220 when oc in 23999.5 .. 24000.5 and op in 230 .. 999\n\
        var og = 0\nset og = 1 when oc in 0 .. 18000\nset og = 0 when oc in 18000 .. 1000000000\nnode ostep\nrelate os: ostep * 12000 = op\n\
        call lfo(name=ov, step=ostep, shape=0)\n";

    /// Her network (mind/elle.grm) played with a tone of her own: what she chooses, eighth after eighth.
    fn her_notes(seconds: usize, tag: &str, lessons: u8) -> Vec<(f64, f64, f64, f64)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-elle-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let app = format!(
            "app Her\ninput bpm = 120 positive\ninput playing = 1\nnode tr_step\nrelate s: tr_step = 0.00000138888889 * bpm\n\
             use {r}/apps/networks/lfo.grm\nuse {r}/apps/networks/transport.grm\ncall transport(name=tr, step=tr_step, run=playing)\n\
             input hp = 220 positive\ninput hg = 0\ninput cs = 1\ninput ch = 1\ninput lz = {lessons}\nnode ear signed\nvar elle_pitch = 440 positive\nvar elle_gate = 0\n\
             node vstep\nrelate vs: vstep * 12000 = elle_pitch\ncall lfo(name=v, step=vstep, shape=0)\n\
             {other}relate hearing: ear = 0.3 * elle_gate * v_out{other_sum}\n\
             use {r}/mind/elle.grm\ncall elle(name=elle, ear=ear, clock_i=tr_i, clock_f=tr_f, ctx_song=cs, ctx_human=ch, lessons=lz)\n",
            r = root.display(),
            lessons = lessons,
            other = if tag == "hear" { OTHER } else { "" },
            other_sum = if tag == "hear" { " - 0.3 * elle_gate * v_out + 0.4 * og * ov_out" } else { "" }
        );
        // her first lesson, from its file, when she has lessons
        let mut app = app;
        if lessons > 0 {
            let lesson = match tag {
                "scale" => "04_major.grm",
                "chords" => "07_four_chords.grm",
                _ => "01_one_note.grm",
            };
            let text = std::fs::read_to_string(root.join("mind/lessons").join(lesson)).unwrap();
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                app.push_str(
                    &line.replace("{name}", "elle").replace("{here}", "elle_lesson in 0.5 .. 1.5 and lz in 0.5 .. 999"),
                );
                app.push('\n');
            }
        }
        let file = dir.join("her.grm");
        std::fs::write(&file, app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        let (out, _) = sh.run(&engine, &format!("app open {}", file.display()));
        assert!(out.iter().all(|l| l.kind != Kind::Error && l.kind != Kind::Warning), "{out:?}");
        // the app said whole before she plays (its last node known)
        for _ in 0..200 {
            let (out, _) = sh.run(&engine, "show elle_born");
            if out.iter().any(|l| l.text.contains("elle_born =")) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // nothing drawn by chance: two of her, made from the same network, play the same, sample after sample
        if tag == "twins" {
            let mut one = Shell::reflex_of(&engine, "ear", 48000).unwrap();
            let mut two = Shell::reflex_of(&engine, "ear", 48000).unwrap();
            let differ: Vec<String> = one
                .names
                .iter()
                .filter(|(n, &i)| !n.contains("__next") && one.values[i] != two.values[i])
                .map(|(n, &i)| format!("{n}: {} {}", one.values[i], two.values[i]))
                .take(12)
                .collect();
            assert!(differ.is_empty(), "they start apart: {differ:?}");
            let (mut bass, mut harmony) = (0, 0);
            for k in 0..(48000 * 6) {
                one.step();
                two.step();
                if one.value("elle_bgate").unwrap_or(0.0) > 0.5 {
                    bass += 1;
                    assert!(one.value("elle_bpitch") < one.value("elle_pitch"), "her bass under her note");
                }
                if one.value("elle_hgate").unwrap_or(0.0) > 0.5 {
                    harmony += 1;
                }
                if k % 240 == 0 {
                    assert_eq!(one.value("elle_choice"), two.value("elle_choice"), "at sample {k}");
                    assert_eq!(one.value("elle_dhit1"), two.value("elle_dhit1"), "at sample {k}");
                }
            }
            // lessons off: a bass under her, and now and then a third below her notes
            assert!(bass > 0 && harmony > 0, "bass {bass} harmony {harmony}");
        }
        let (out, _) = sh.run(&engine, "play ear");
        assert!(out[0].text.contains("plays"), "{out:?}");
        let mut notes = vec![];
        let mut buf = vec![0.0f32; 2 * 240];
        for _ in 0..(seconds * 200) {
            mixer.lock().unwrap().render(&mut buf);
            let m = mixer.lock().unwrap();
            let v = |n: &str| m.value("ear", n).unwrap_or(f64::NAN);
            let kick = if tag == "hear" {
                v("elle_hint")
            } else if v("elle_dhit1") > 0.5 {
                v("tr_i")
            } else {
                -1.0
            };
            notes.push((v("elle_semi"), v("elle_choice"), v("elle_gate"), kick));

            // never a note a semitone above her bass (a minor ninth), and always in her scale
            if v("elle_gate") > 0.5 && v("elle_ev") < 0.5 {
                let deg = v("elle_deg").round() as i64;
                assert!(v(&format!("elle_sc{deg}")) > 0.5, "degree {deg} out of her scale (tonic {})", v("elle_tonic"));
                if v("elle_bgate") > 0.5 {
                    let pc = |hz: f64| ((12.0 * (hz / 55.0).log2()).round() as i64).rem_euclid(12);
                    let (m, b) = (pc(v("elle_pitch")), pc(v("elle_bpitch")));
                    assert!((m - b).rem_euclid(12) != 1, "a minor ninth over her bass: {m} over {b}");
                }
            }
        }
        if tag == "hear" {
            // notes heard on a steady beat: her pulse falls in with them
            let sync = mixer.lock().unwrap().value("ear", "elle_sync").unwrap_or(0.0);
            assert!(sync > 0.7, "her pulse and what she hears: {sync}");
        }
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
        notes
    }

    #[test]
    fn the_bars_turn_as_a_clock_s_hands_and_the_sound_can_be_plugged_in_again() {
        // sixteen bars make a turn, and move the dial above by one: nothing counts on without end
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-dials-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let app = format!(
            "app T\ninput bpm = 2400 positive\ninput playing = 1\nnode tr_step\nrelate s: tr_step = 0.00000138888889 * bpm\nuse {r}/apps/networks/lfo.grm\nuse {r}/apps/networks/transport.grm\ncall transport(name=tr, step=tr_step, run=playing)\n",
            r = root.display()
        );
        let file = dir.join("t.grm");
        std::fs::write(&file, app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        sh.run(&engine, &format!("app open {}", file.display()));
        let mut r = Shell::reflex_of(&engine, "tr_click", 48000).unwrap();
        // a bar at 2400 bpm: 4800 samples; 17 bars and a half
        let (mut most, mut turned) = (0.0f64, false);
        for _ in 0..(4800 * 35 / 2) {
            r.step();
            most = most.max(r.value("tr_bar").unwrap_or(0.0));
            turned |= r.value("tr_bar2").unwrap_or(0.0) > 0.5;
        }
        assert!(most <= 15.0 && turned, "the bar went to {most}");
        assert_eq!(r.value("tr_bar"), Some(1.0));
        // unplugged and plugged in again: it plays on
        sh.run(&engine, "play tr_click");
        let (out, _) = sh.run(&engine, "sound again");
        assert!(out[0].text.contains("plugged in again"), "{out:?}");
        assert!(mixer.lock().unwrap().has_world());
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_drum_machine_has_two_bars_of_steps() {
        // a kick on the first step of the second bar only: heard there, and not in the first bar; one bar long, the
        // first bar's steps again (and so no kick at all)
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-drums-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut app = String::from("app D\ninput bpm = 600 positive\ninput playing = 1\ninput z = 0\nnode tr_step\n");
        app.push_str("relate s: tr_step = 0.00000138888889 * bpm\n");
        for n in ["lfo", "adsr", "noise", "transport", "drums"] {
            app.push_str(&format!("use {}/apps/networks/{n}.grm\n", root.display()));
        }
        app.push_str("call transport(name=tr, step=tr_step, run=playing)\n");
        app.push_str(
            "call drums(name=d, clock_i=tr_i, clock_f=tr_f, clock_h=tr_h, her_k=z, her_s=z, her_h=z, her_t=z)\n",
        );
        let file = dir.join("d.grm");
        std::fs::write(&file, app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer);
        let (out, _) = sh.run(&engine, &format!("app open {}", file.display()));
        assert!(out.iter().all(|l| l.kind != Kind::Error), "{out:?}");
        let mut r = Shell::reflex_of(&engine, "d_out", 48000).unwrap();
        for x in ["k", "s", "h", "t"] {
            for k in 1..=32 {
                let i = r.slot_of(&format!("d_{x}_s{k}")).unwrap();
                r.values[i] = if x == "k" && k == 17 { 1.0 } else { 0.0 };
            }
        }
        let kicks = |r: &mut grm_engine::reflex::Reflex| {
            let mut at = vec![];
            let mut was = false;
            for _ in 0..(3 * 16 * 1200) {
                r.step();
                let on = r.value("d_k_g").unwrap_or(0.0) > 0.5;
                if on && !was {
                    at.push((r.value("tr_h").unwrap_or(-1.0), r.value("tr_i").unwrap_or(-1.0)));
                }
                was = on;
            }
            at
        };
        let at = kicks(&mut r);
        assert!(!at.is_empty() && at.iter().all(|(h, i)| *h == 1.0 && *i == 0.0), "{at:?}");
        let long = r.slot_of("d_long").unwrap();
        r.values[long] = 0.0;
        assert!(kicks(&mut r).is_empty());
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sound_is_never_held_long_by_what_tunes_it() {
        // her world (thousands of nodes) with a channel beside it: what the machine tells the sound, a few times a
        // second, must hold the sound's lock for a moment, not for the time of many blocks (the clicks it made)
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-tune-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        sh.always = Some(root.join("mind/home.grm"));
        sh.wake(&engine);
        let app = format!(
            "app Tone\nuse {r}/apps/networks/lfo.grm\ninput tstep = 0.0183333 positive\ncall lfo(name=tl, step=tstep, shape=0)\nnode tone signed\nrelate t: tone = 0.3 * tl_out\n",
            r = root.display()
        );
        let file = dir.join("tone.grm");
        std::fs::write(&file, app).unwrap();
        sh.run(&engine, &format!("app open {}", file.display()));
        sh.run(&engine, "play tone");
        let params = mixer.lock().unwrap().params();
        assert!(params.len() > 1000, "{}", params.len());
        let t0 = std::time::Instant::now();
        let p = mixer.lock().unwrap().params();
        assert!(t0.elapsed() < Duration::from_millis(20), "params held it {:?}", t0.elapsed());
        let values: IndexMap<String, f64> = p.iter().map(|n| (n.clone(), 0.5)).collect();
        let t0 = std::time::Instant::now();
        mixer.lock().unwrap().tune(&values);
        assert!(t0.elapsed() < Duration::from_millis(20), "tune held it {:?}", t0.elapsed());
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            sh.feel(&engine);
            assert!(t0.elapsed() < Duration::from_millis(200), "feel took {:?}", t0.elapsed());
        }
        // what did not move is not told again
        let before = sh.tuned.len();
        sh.feel(&engine);
        assert!(sh.tuned.len() >= before && before > 0, "{before} of {}", params.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn she_is_always_there_hears_the_room_sees_and_dreams() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-home-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        sh.always = Some(root.join("mind/home.grm"));
        let out = sh.wake(&engine);
        assert!(out.iter().all(|l| l.kind != Kind::Error && l.kind != Kind::Warning), "{out:?}");
        assert!(mixer.lock().unwrap().has_world(), "she runs with nothing played");
        // she does not close
        let (out, _) = sh.run(&engine, "app close home");
        assert!(out[0].text.contains("always"), "{out:?}");
        // a tone in the room, from another app
        let app = format!(
            "app Tone\nuse {r}/apps/networks/lfo.grm\ninput tstep = 0.0183333 positive\ncall lfo(name=tl, step=tstep, shape=0)\nnode tone signed\nrelate t: tone = 0.3 * tl_out\n",
            r = root.display()
        );
        let file = dir.join("tone.grm");
        std::fs::write(&file, app).unwrap();
        let (out, _) = sh.run(&engine, &format!("app open {}", file.display()));
        assert!(out.iter().all(|l| l.kind != Kind::Error), "{out:?}");
        let (out, _) = sh.run(&engine, "play tone");
        assert!(out[0].text.contains("plays"), "{out:?}");
        let mut buf = vec![0.0f32; 2 * 480];
        let glow0 = mixer.lock().unwrap().world_value("rv_glow").unwrap_or(0.0);
        for _ in 0..300 {
            mixer.lock().unwrap().render(&mut buf);
        }
        let m = mixer.lock().unwrap();
        assert!(m.world_value("elle_tot").unwrap_or(0.0) > 1e-6, "she hears nothing");
        assert!(m.world_value("rv_glow").unwrap_or(0.0) > glow0 + 0.1, "her reverie is not lit by it");
        let l1 = m.world_value("rv_l1").unwrap();
        assert!(l1.is_finite() && (0.0..=1.0).contains(&l1));
        drop(m);
        // a picture for her eye: a red one
        let pic = dir.join("red.png");
        let mut p = grm_engine::vision::Picture::new(8, 8, 3);
        for y in 0..8 {
            for x in 0..8 {
                p.set(x, y, 0, 1.0);
                p.set(x, y, 1, 0.0);
                p.set(x, y, 2, 0.0);
            }
        }
        grm_connect::write_picture(&pic, &p).unwrap();
        let (out, _) = sh.run(&engine, &format!("see {}", pic.display()));
        assert!(out.iter().any(|l| l.text.contains("she sees it too")), "{out:?}");
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_arpeggiator_goes_round_the_keys_and_its_latch_holds_them() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-arp-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let app = format!(
            "app Arp\ninput bpm = 120 positive\ninput playing = 1\nnode tr_step\nrelate s: tr_step = 0.00000138888889 * bpm\n\
             use {r}/apps/networks/lfo.grm\nuse {r}/apps/networks/transport.grm\nuse {r}/apps/networks/arpeggiator.grm\n\
             call transport(name=tr, step=tr_step, run=playing)\n\
             input on = 1\ninput latch = 1\ninput mode = 0\ninput oct = 1 positive\ninput rate = 2 positive\n\
             input p1 = 220 positive\ninput g1 = 1\ninput p2 = 330 positive\ninput g2 = 1\ninput p3 = 440 positive\ninput g3 = 0\ninput p4 = 550 positive\ninput g4 = 0\n\
             call arpeggiator(name=a, clock_i=tr_i, clock_f=tr_f, on=on, latch=latch, mode=mode, oct=oct, rate=rate, p1=p1, g1=g1, p2=p2, g2=g2, p3=p3, g3=g3, p4=p4, g4=g4)\n\
             node heard signed\nrelate h: heard = 0.1 * a_gate * tr_click\n",
            r = root.display()
        );
        let file = dir.join("arp.grm");
        std::fs::write(&file, app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        let (out, _) = sh.run(&engine, &format!("app open {}", file.display()));
        assert!(out.iter().all(|l| l.kind != Kind::Error && l.kind != Kind::Warning), "{out:?}");
        sh.run(&engine, "play heard");
        let mut buf = vec![0.0f32; 2 * 600];
        let mut listen = |mixer: &Arc<Mutex<Mixer>>, blocks: usize| {
            let mut seen = vec![];
            for _ in 0..blocks {
                mixer.lock().unwrap().render(&mut buf);
                let m = mixer.lock().unwrap();
                if m.value("heard", "a_gate").unwrap_or(0.0) > 0.5 {
                    seen.push(m.value("heard", "a_pitch").unwrap_or(0.0).round() as i64);
                }
            }
            seen.dedup();
            seen
        };
        // two keys held: one after the other
        let held = listen(&mixer, 160);
        assert!(held.contains(&220) && held.contains(&330) && !held.contains(&440), "{held:?}");
        // let go, latched: they go on
        for n in ["g1", "g2"] {
            mixer.lock().unwrap().set_now(n, 0.0);
            sh.run(&engine, &format!("observe {n} = 0 ± 0.000001"));
        }
        let latched = listen(&mixer, 160);
        assert!(latched.contains(&220) && latched.contains(&330), "{latched:?}");
        // not latched: silence
        mixer.lock().unwrap().set_now("latch", 0.0);
        sh.run(&engine, "observe latch = 0 ± 0.000001");
        listen(&mixer, 20);
        assert!(listen(&mixer, 160).is_empty());
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sound_file_plays_as_a_node() {
        let dir = std::env::temp_dir().join(format!("grm-sound-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let wav = dir.join("tone.wav");
        let tone: Vec<f32> =
            (0..48000).map(|k| (k as f32 * 220.0 * std::f32::consts::TAU / 48000.0).sin() * 0.5).collect();
        // written as two channels (left and right the same): read as one
        let both: Vec<f32> = tone.iter().flat_map(|v| [*v, *v]).collect();
        grm_sound::write_wav(&wav, &both, 48000).unwrap();
        let app = format!(
            "app Pl\ninput go = 1\naudio pl from {} when go\nnode heard signed\nrelate h: heard = pl\n",
            wav.display()
        );
        let file = dir.join("pl.grm");
        std::fs::write(&file, app).unwrap();
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(dir.join("w.world"), mixer.clone());
        let (out, _) = sh.run(&engine, &format!("app open {}", file.display()));
        assert!(out.iter().all(|l| l.kind != Kind::Error && l.kind != Kind::Warning), "{out:?}");
        let mut r = Shell::reflex_of(&engine, "heard", 48000).unwrap();
        let mut got = vec![];
        for _ in 0..100 {
            r.step();
            got.push(r.value("pl").unwrap_or(f64::NAN) as f32);
        }
        // the file, sample after sample (as written: 16 bits)
        assert!(got.iter().zip(&tone).all(|(a, b)| (a - b).abs() < 1e-3), "{:?}", &got[..5]);
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn she_hears_a_melody_by_her_ear_alone() {
        // someone plays a tone up, a tone down, again: what her ear takes of it is steps of two semitones
        let a = her_notes(8, "hear", 1);
        let mut steps: Vec<i64> = a.iter().map(|n| n.3.round() as i64).collect();
        steps.dedup();
        steps.retain(|s| *s != 0);
        assert!(steps.iter().filter(|s| s.abs() == 2).count() * 2 > steps.len(), "{steps:?}");
        assert!(steps.contains(&2) && steps.contains(&-2), "{steps:?}");
    }

    #[test]
    fn in_a_lesson_of_steps_she_keeps_to_its_major_scale() {
        let a = her_notes(12, "scale", 1);
        let degrees: std::collections::BTreeSet<i64> =
            a.iter().map(|n| (n.0.round() as i64 - 24).rem_euclid(12)).collect();
        assert!(degrees.iter().all(|d| [0, 2, 4, 5, 7, 9, 11].contains(d)), "{degrees:?}");
        assert!(degrees.len() >= 3, "she moves in it: {degrees:?}");
    }

    #[test]
    fn in_a_lesson_of_chords_her_bass_plays_their_roots_and_she_breathes() {
        let a = her_notes(16, "chords", 1);
        let degrees: std::collections::BTreeSet<i64> =
            a.iter().filter(|n| n.2 > 0.5).map(|n| (n.0.round() as i64 - 24).rem_euclid(12)).collect();
        assert!(degrees.iter().all(|d| [0, 2, 4, 5, 7, 9, 11].contains(d)), "{degrees:?}");
        assert!(a.iter().any(|n| n.2 < 0.5), "she rests now and then");
    }

    #[test]
    fn her_first_lesson_is_one_note_a_beat() {
        let a = her_notes(6, "l", 1);
        let semis: Vec<i64> = a.iter().map(|n| n.0.round() as i64).collect();
        assert!(semis.iter().all(|s| *s == 24), "one note only: {semis:?}");
        assert!(a.iter().any(|n| n.2 > 0.5), "she plays it");
        // her hand on the kick starts where a groove puts it: on the beats of the bar (0 and 8)
        let mut kicks: Vec<i64> = a.iter().filter(|n| n.3 >= 0.0).map(|n| n.3.round() as i64).collect();
        kicks.sort();
        kicks.dedup();
        assert!(!kicks.is_empty() && kicks.iter().all(|k| *k == 0 || *k == 8), "{kicks:?}");
    }

    #[test]
    fn her_network_plays_by_itself_and_the_same_way_twice() {
        let a = her_notes(12, "a", 0);
        let semis: Vec<i64> = a.iter().map(|n| n.0.round() as i64).collect();
        assert!(semis.iter().all(|s| (12..=36).contains(s)), "out of her range: {semis:?}");
        let mut seen = semis.clone();
        seen.dedup();
        assert!(seen.len() > 8, "she hardly moves: {seen:?}");
        assert!(a.iter().any(|n| n.2 > 0.5) && a.iter().any(|n| n.2 < 0.5), "no notes");
        // she chooses how long each note lasts: some an eighth (a quarter of a second here), some longer
        let mut runs = vec![];
        let mut run = 0;
        for n in &a {
            if n.2 > 0.5 {
                run += 1;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
        // a sample every 5 ms: an eighth held is about 37 of them, a quarter about 87
        assert!(runs.iter().any(|r| *r < 50) && runs.iter().any(|r| *r > 60), "{runs:?}");
        // nothing drawn by chance (the twins inside it agree)
        her_notes(1, "twins", 0);
    }

    #[test]
    fn the_prompt_says_plays_and_asks_before_taking_away() {
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(std::env::temp_dir().join("grm-shell-test.world"), mixer.clone());
        let step = std::f64::consts::TAU * 220.0 / 48000.0;
        for l in [
            format!("node step = {step} fixed"),
            "var c = 1".into(),
            "var s = 0".into(),
            "node c_new signed".into(),
            "relate turn: c_new = c - step * s".into(),
            "set c = c_new".into(),
            "set s = s + step * c_new".into(),
        ] {
            let (out, quit) = sh.run(&engine, &l);
            assert!(!quit && out.iter().all(|l| l.kind != Kind::Error), "{l}: {out:?}");
        }
        let (out, _) = sh.run(&engine, "play s 0.5");
        assert!(out[0].text.contains("plays"), "{out:?}");
        let mut buf = vec![0.0f32; 2 * 48000];
        mixer.lock().unwrap().render(&mut buf);
        assert!(mixer.lock().unwrap().peak > 0.1);
        // what follows from the sets (c_new) is computed at each sample too: a steady 220 Hz, not a drift
        mixer.lock().unwrap().render(&mut buf);
        let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
        let crossings = left.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((crossings as i64 - 220).abs() <= 2, "{crossings}");
        let (out, _) = sh.run(&engine, "remove turn");
        assert_eq!(out[0].kind, Kind::Ask);
        let (out, _) = sh.run(&engine, "n");
        assert_eq!(out[0].text, "nothing taken away");
        let (out, _) = sh.run(&engine, "frobnicate");
        assert_eq!(out[0].kind, Kind::Warning);
        sh.feel(&engine);
        let (_, quit) = sh.run(&engine, "quit");
        assert!(quit);
        engine.stop();
    }

    #[test]
    fn the_machine_tunes_what_plays() {
        let engine = Engine::start(World::new(), Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let mut sh = Shell::new(std::env::temp_dir().join("grm-shell-test2.world"), mixer.clone());
        for l in [
            "node step = 0.0576",
            "observe step = 0.0576 ± 0.000001",
            "var c = 1",
            "var s = 0",
            "node c_new signed",
            "relate turn: c_new = c - step * s",
            "set c = c_new",
            "set s = s + step * c_new",
            "play s 0.5",
        ] {
            let (out, _) = sh.run(&engine, l);
            assert!(out.iter().all(|l| l.kind != Kind::Error), "{l}: {out:?}");
        }
        let crossings = |m: &Arc<Mutex<Mixer>>| {
            let mut buf = vec![0.0f32; 2 * 48000];
            m.lock().unwrap().render(&mut buf);
            let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
            left.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as i64
        };
        assert!((crossings(&mixer) - 440).abs() <= 3);
        sh.run(&engine, "observe step = 0.0864 ± 0.000001");
        engine.settled(Duration::from_secs(2));
        sh.feel(&engine);
        assert!((crossings(&mixer) - 660).abs() <= 3);
        engine.stop();
    }
}
