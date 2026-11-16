// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The prompt of the GRM machine: every command, the same in the terminal and in the window. A line goes in,
//! lines come out (each with its kind: said, dim, a warning, an error, a question waiting for y/n).
//!
//! It also keeps the sound: the mixer (the machine's own voice, and the channels the human plays), and what the
//! machine feels moves the voice (`feel`).

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
        Err(e) => (World::new(), Some(Line::warn(format!("⚠ {e}: it starts afresh (the file is left as it is)")))),
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
    /// The last picture each eye saw, cleaned, surprised, changed (for a window to show).
    pub sights: IndexMap<String, grm_engine::vision::Sight>,
    pub(crate) rt: runtime::Runtime,
    /// Whether the desk is kept beside the world's memory (not in a test run).
    pub keep_desk: bool,
    /// What it asks of the window.
    pub orders: Vec<Order>,
    /// Whether a window is there to answer (to choose a table's columns, to open a file).
    pub window: bool,
    before_cd: Option<PathBuf>,
    /// The world as it was before each change (`undo`), and what was taken back (`redo`).
    pub(crate) undo_world: Vec<World>,
    pub(crate) redo_world: Vec<World>,
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
            sights: IndexMap::new(),
            rt: Default::default(),
            keep_desk: true,
            orders: vec![],
            window: false,
            before_cd: None,
            undo_world: vec![],
            redo_world: vec![],
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
            "show", "status", "ask", "why", "inspect", "ins", "outputs", "outs", "master", "ms", "check", "chk",
            "journal", "sources", "insights", "forecast", "rewind", "help", "h", "history", "pwd", "ls", "head",
            "tail", "cat", "programs", "progs", "mixer", "apps", "undo", "redo", "stats", "joint", "focus", "fc",
            "clear", "tour", "lang", "cd", "sound", "voice", "volume", "mute", "unmute", "solo", "gain", "pan",
            "panic", "stop", "trial", "play", "pause", "render", "edit", "open", "new", "threads", "sleep", "mkdir",
            "touch", "cp", "mv", "rm",
        ];
        if !READING.contains(&word) {
            if let Some(w) = engine.query(|w| w.clone()) {
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
                    out.push(Line::warn(format!("⚠ in quarantine: {}", s.quarantine.join(", "))));
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
            "remove" if self.owner_of(rest.split_whitespace().last().unwrap_or("")).is_some() => {
                let thing = rest.split_whitespace().last().unwrap_or("").to_string();
                let (app, l) = self.owner_of(&thing).unwrap();
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
        let mut words = rest.split_whitespace();
        let file = PathBuf::from(words.next().unwrap_or(""));
        let name = words.next().unwrap_or("eye").to_string();
        let picture = match grm_connect::read_picture(&file) {
            Ok(p) => p,
            Err(e) => return vec![Line::error(format!("! {e}"))],
        };
        let eye = name.clone();
        let size = (picture.width, picture.height, picture.channels);
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
        let mut out = vec![Line::said(format!(
            "{name} saw {}x{}x{} in {} rounds ({:.2?})",
            size.0, size.1, size.2, sight.rounds, sight.elapsed
        ))];
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
                    Ok(format!(
                        "sandbox {sandbox}: {} ({} rows, by {}) — trial {sandbox}, then merge {sandbox}, then feed {sandbox}",
                        nodes.join(", "),
                        t.time.len(),
                        t.time_unit
                    ))
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
        let node = node.to_string();
        engine
            .query(move |w| -> grm_engine::Result<Reflex> {
                if !w.machine.net.nodes.contains_key(&node) {
                    grm_engine::bail!("no node {node}");
                }
                let mut r = Reflex::of(&w.machine, Clock::Audio { rate })?;
                // what follows from the sets, computed at each sample too (as far as a reflex can): every node a
                // relation gives from what is already computed, in the order it is reached
                let net = &w.machine.net;
                let mut known: Vec<String> = net.sets.iter().map(|s| s.target.clone()).collect();
                loop {
                    let mut grew = false;
                    for rel in &net.relations {
                        let nodes = rel.nodes();
                        if !nodes.iter().any(|n| known.contains(n)) {
                            continue;
                        }
                        for n in &nodes {
                            let free = !known.contains(n)
                                && !n.contains("__next")
                                && !net.measured.contains_key(n)
                                && !net.nodes.get(n).is_some_and(|s| s.fixed);
                            if free && r.follow(&w.machine, n).is_ok() {
                                known.push(n.clone());
                                grew = true;
                            }
                        }
                    }
                    if !grew {
                        break;
                    }
                }
                if !known.contains(&node) {
                    r.follow(&w.machine, &node).map_err(|e| {
                        GrmError::new(format!(
                            "{node}: {e} (play a node its sets write, or one a relation gives from them)"
                        ))
                    })?;
                }
                Ok(r)
            })
            .unwrap_or_else(|| Err(GrmError::new("the engine has stopped")))
    }

    fn sound(&mut self, engine: &Engine, word: &str, rest: &str) -> Vec<Line> {
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
                _ => vec![Line::warn("? sound on|off")],
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
                drop(mixer);
                match Self::reflex_of(engine, &first, rate) {
                    Ok(r) => match ReflexSource::new(r, &first) {
                        Some(source) => {
                            let mut m = self.mixer.lock().unwrap();
                            m.play(&first, Box::new(source), number.unwrap_or(0.5).clamp(0.0, 2.0));
                            vec![Line::said(format!("{first} plays, {} times a second (it rises from silence)", rate))]
                        }
                        None => vec![Line::error(format!("! {first} is not in its network"))],
                    },
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
        let values: IndexMap<String, f64> =
            params.iter().filter_map(|p| s.values.get(p).map(|v| (p.clone(), v.0))).collect();
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
