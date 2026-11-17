// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The world of the machine: the global network and its machine, the sandboxes beside it, what is being integrated
//! little by little, what is in quarantine, and the logbook.
//!
//! - A **sandbox** is a network of its own, isolated: it reads the global network's beliefs at its border as fixed
//!   measurements, computes only itself, and the global network goes on living without knowing it is there.
//! - A **trial merge** puts the sandbox into a working copy of the global network (starting from where the global
//!   machine is: only what is new is woken) and reports what it would change: tensions, harmony, surprises,
//!   names in conflict. Nothing touches the global network.
//! - **Accepting** a merge integrates it little by little: its relations rise in strength step by step. If the
//!   harmony falls or a surprise rises while they rise, they go back to 0: **quarantine** (and what they taught the
//!   weights is taken back).
//! - A **recall** takes away everything that came from a source, everywhere.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::bail;
use crate::discover::{find_laws, Rows};
use crate::error::{GrmError, Result};
use crate::lang::{self, fmt_g, is_name};
use crate::machine::{FKind, Machine};
use crate::network::{Kind, Network, NodeSpec, Provenance};
use crate::time::Timekeeper;

/// How much the harmony may fall while a merge rises before it is put in quarantine.
pub const QUARANTINE_DROP: f64 = 0.15;
/// How far (in sigmas) a node of the global network may be moved by a merge before it counts as a surprise.
pub const SURPRISE_SIGMAS: f64 = 3.0;
/// The steps a merge's strength rises by, one per tick.
pub const RAMP: [f64; 5] = [0.1, 0.25, 0.5, 0.75, 1.0];

/// What the network believes of several nodes together: one Gaussian of several dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    pub names: Vec<String>,
    pub values: Vec<f64>,
    /// Exact sigmas (SI), from the whole network at once.
    pub sigmas: Vec<f64>,
    /// How they move together, from -1 to 1.
    pub correlation: Vec<Vec<f64>>,
}

impl Joint {
    pub fn text(&self) -> String {
        let mut out: Vec<String> = self
            .names
            .iter()
            .zip(&self.values)
            .zip(&self.sigmas)
            .map(|((n, v), s)| format!("{n} = {} ± {}", fmt_g(*v, 6), fmt_g(*s, 3)))
            .collect();
        let k = self.names.len();
        let mut together = vec![];
        for a in 0..k {
            for b in a + 1..k {
                let c = self.correlation[a][b];
                if c.abs() >= 0.1 {
                    together.push(format!("  {} and {}: {:+.2}", self.names[a], self.names[b], c));
                }
            }
        }
        if !together.is_empty() {
            out.push("they move together:".into());
            out.extend(together);
        } else if k > 1 {
            out.push("they move apart from each other (no correlation above 0.1)".into());
        }
        out.join("\n  ")
    }
}

/// What a sleeping world is written as.
pub const SLEEP_FORMAT: &str = "grm-vm-sleep-1";

#[derive(Serialize, Deserialize)]
struct Sleeping {
    format: String,
    net: Network,
    state: crate::machine::MachineState,
    sandboxes: Vec<(String, Vec<String>, Network)>,
    integrations: Vec<Integration>,
    quarantine: Vec<String>,
    journal: Vec<Entry>,
    rounds_per_tick: u64,
    time: Timekeeper,
    senses: IndexMap<String, f64>,
    rows: Rows,
    laws: Vec<(String, Vec<String>)>,
    embodied: IndexMap<String, usize>,
}

/// A line of the logbook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub tick: u64,
    pub kind: String,
    pub text: String,
}

/// A sandbox: a network being tried, isolated from the global one.
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub name: String,
    /// What was said in it, in order (what a merge says again to the global network).
    pub lines: Vec<String>,
    pub net: Network,
    pub machine: Option<Machine>,
}

impl Sandbox {
    pub fn source(&self) -> String {
        format!("sandbox:{}", self.name)
    }
}

/// What a merge would do, before it is done.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeReport {
    pub sandbox: String,
    pub new_nodes: Vec<String>,
    pub new_relations: Vec<String>,
    /// Relations of the global network the sandbox would replace (same name).
    pub replaced: Vec<String>,
    /// Nodes of the global network the sandbox says differently (signed, fixed, value).
    pub conflicts: Vec<String>,
    /// Relations of the global network that would hold less well: (name, sigmas before, sigmas after).
    pub tensions: Vec<(String, f64, f64)>,
    pub harmony_before: f64,
    pub harmony_after: f64,
    /// Nodes of the global network moved further than SURPRISE_SIGMAS of what it believed: (name, sigmas).
    pub surprises: Vec<(String, f64)>,
    /// What the trial cost: rounds and messages (only what is new was woken).
    pub rounds: u64,
    pub messages: u64,
    pub settled: bool,
}

impl MergeReport {
    /// Compatible with what the network holds (it may still be wrong: being compatible is not being true).
    pub fn compatible(&self) -> bool {
        self.surprises.is_empty() && self.harmony_after >= self.harmony_before - QUARANTINE_DROP / 3.0
    }

    pub fn text(&self) -> String {
        let mut out = vec![format!(
            "merge of {}: {} new node(s), {} new relation(s)",
            self.sandbox,
            self.new_nodes.len(),
            self.new_relations.len()
        )];
        if !self.replaced.is_empty() {
            out.push(format!("  ! it would replace: {}", self.replaced.join(", ")));
        }
        if !self.conflicts.is_empty() {
            out.push(format!("  ! said differently: {}", self.conflicts.join(", ")));
        }
        out.push(format!("  harmony {:.3} -> {:.3}", self.harmony_before, self.harmony_after));
        for (n, a, b) in self.tensions.iter().take(5) {
            out.push(format!("  tension {n}: {a:.2} -> {b:.2} sigma"));
        }
        for (n, s) in self.surprises.iter().take(5) {
            out.push(format!("  surprise {n}: moved {s:.1} sigma"));
        }
        out.push(format!(
            "  trial: {} rounds, {} messages{}",
            self.rounds,
            self.messages,
            if self.settled { "" } else { " (not settled)" }
        ));
        out.push(if self.compatible() {
            "  compatible with what it holds".to_string()
        } else {
            "  surprising: it would need to be confirmed (not rejected)".to_string()
        });
        out.join("\n")
    }
}

/// A merge rising in strength, step by step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    pub source: String,
    pub step: usize,
    pub harmony_before: f64,
}

/// What `remove` would take away: {kind, name, nodes, relations}.
#[derive(Debug, Clone, PartialEq)]
pub struct Removal {
    pub kind: &'static str,
    pub name: String,
    pub nodes: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct World {
    pub machine: Machine,
    pub sandboxes: IndexMap<String, Sandbox>,
    pub integrations: Vec<Integration>,
    pub quarantine: Vec<String>,
    pub journal: Vec<Entry>,
    pub rounds_per_tick: u64,
    /// The nodes that live in time: memory, law of motion, foresight.
    pub time: Timekeeper,
    /// How much a measurement of each node fed by data is trusted (its sigma, as it was first said).
    pub senses: IndexMap<String, f64>,
    /// The rows it has measured, kept evenly along what it lived (what laws are looked for in).
    pub rows: Rows,
    /// The laws it has found and proposed: (target, inputs).
    pub laws: Vec<(String, Vec<String>)>,
    /// The nodes whose hidden law the machine holds as its own relations, with the consolidation it holds.
    pub embodied: IndexMap<String, usize>,
    /// Its eyes: each a network of its own, near what it sees (not written when the world sleeps: an eye opens
    /// anew).
    pub eyes: IndexMap<String, crate::vision::Eye>,
    /// What it feels: pleasure and displeasure, flourishing, progress (of the moment: not kept when it sleeps).
    pub mood: crate::mind::Mood,
    /// Workshops: new networks of data, each a world of its own (its time, its laws), made apart from this one
    /// (nothing of it comes in until the human merges it).
    pub workshops: IndexMap<String, Box<World>>,
    /// What was written in it, line by line, when it is a workshop (what its window shows and lets change).
    pub written: Vec<String>,
}

impl Default for World {
    fn default() -> Self {
        World::new()
    }
}

/// The words of a line that are names of nodes.
fn names_in(line: &str) -> Vec<String> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '~'))
        .filter(|w| is_name(w))
        .map(|w| w.to_string())
        .collect()
}

impl World {
    pub fn new() -> Self {
        World {
            machine: Machine::new(Network::new()).expect("an empty network"),
            time: Timekeeper::new(),
            senses: IndexMap::new(),
            rows: Rows::new(),
            laws: vec![],
            embodied: IndexMap::new(),
            eyes: IndexMap::new(),
            sandboxes: IndexMap::new(),
            integrations: vec![],
            quarantine: vec![],
            journal: vec![],
            rounds_per_tick: 1000,
            mood: Default::default(),
            workshops: IndexMap::new(),
            written: vec![],
        }
    }

    pub fn net(&self) -> &Network {
        &self.machine.net
    }

    fn log(&mut self, kind: &'static str, text: impl Into<String>) {
        let tick = self.machine.net.ticks;
        let text = text.into();
        // what it found by itself, or took in after trying it, is a burst of pleasure; what it had to put apart or
        // refuse, a burst of displeasure
        let burst = match kind {
            "found" | "insight" => 0.5,
            "integrated" | "merged" => 0.3,
            "revised" => -0.1,
            "quarantine" | "refused" => -0.4,
            _ => 0.0,
        };
        if burst != 0.0 {
            self.mood.reward(burst, tick, format!("{kind}: {text}"));
        }
        self.journal.push(Entry { tick, kind: kind.to_string(), text });
    }

    // ------------------------------------------------------------------ speaking to the global network
    /// Says a statement to the global network. A measurement is taken in place (only its node wakes); anything
    /// else changes the network, and the machine goes on from where it was (only what is new wakes).
    /// Returns None if the line is not a statement (a question for the interpreter).
    pub fn say(&mut self, line: &str, origin: &Provenance) -> Result<Option<String>> {
        self.say_all(&[line], origin).map(|mut v| v.pop().flatten())
    }

    /// Says several statements at once (a file): one change of the network, not one per line.
    pub fn say_all(&mut self, lines: &[&str], origin: &Provenance) -> Result<Vec<Option<String>>> {
        let simple = lines.iter().all(|l| {
            let w = lang::normal(l.trim()).split_whitespace().next().unwrap_or("").to_lowercase();
            w == "observe" || w == "forget"
        }) && self.machine.net.recording.is_none();
        if simple {
            let mut out = vec![];
            for l in lines {
                let known: Vec<String> =
                    names_in(l).into_iter().filter(|n| self.machine.index.contains_key(n)).collect();
                let said = lang::statement(&mut self.machine.net, l, None, origin)?;
                let name = names_in(&lang::normal(l.trim())).into_iter().nth(1).unwrap_or_default();
                if known.contains(&name) {
                    self.machine.remeasured(&name);
                } else {
                    let net = self.machine.net.clone();
                    self.machine = self.machine.grown(net)?;
                }
                out.push(said);
            }
            return Ok(out);
        }
        let mut net = self.machine.net.clone();
        let mut out = vec![];
        for l in lines {
            out.push(lang::statement(&mut net, l, None, origin)?);
        }
        self.machine = self.machine.grown(net)?;
        Ok(out)
    }

    /// One tick of the global network: it settles, the `set`s write, the merges rising take one more step.
    pub fn tick(&mut self, learn: bool) -> IndexMap<String, f64> {
        let t = self.machine.net.ticks as f64;
        // what the nodes that live in time were measured to be (not what the network made of them)
        let measured: IndexMap<String, f64> =
            self.time.nodes.keys().filter_map(|n| self.machine.net.measured.get(n).map(|m| (n.clone(), m.0))).collect();
        self.tick_at(t, learn, &measured)
    }

    /// A tick at a moment of the world's time (a row of data says when it was): the network settles, the `set`s
    /// write, the nodes that live in time remember and learn from what was `measured` of them (a node not in it
    /// was not measured: its hidden state goes on), the merges rising take one more step.
    pub fn tick_at(&mut self, t: f64, learn: bool, measured: &IndexMap<String, f64>) -> IndexMap<String, f64> {
        let before_surprise = self.machine.last_surprise.clone();
        // the rhythms of the laws the machine holds, at this moment
        for name in self.embodied.keys().cloned().collect::<Vec<_>>() {
            if let Some(h) = self.time.nodes.get(&name).and_then(|n| n.hidden.as_ref()) {
                for (p, v) in h.rhythms.periods.iter().zip(h.rhythms.harmonics(t).chunks(2)) {
                    let _ = self.machine.observe(
                        &format!("{name}__cos_{}", fmt_g(*p, 12).replace('.', "_")),
                        v[0],
                        Some(1e-9),
                    );
                    let _ = self.machine.observe(
                        &format!("{name}__sin_{}", fmt_g(*p, 12).replace('.', "_")),
                        v[1],
                        Some(1e-9),
                    );
                }
            }
        }
        let writes = self.machine.tick(self.rounds_per_tick, learn);
        let row: IndexMap<String, f64> =
            measured.iter().filter(|(_, v)| v.is_finite()).map(|(n, v)| (n.clone(), *v)).collect();
        if !row.is_empty() {
            self.rows.push(row);
        }
        if !self.time.nodes.is_empty() {
            let values: IndexMap<String, f64> = measured
                .iter()
                .filter(|(n, v)| self.time.nodes.contains_key(*n) && v.is_finite())
                .map(|(n, v)| (n.clone(), *v))
                .collect();
            for i in self.time.record(t, &values) {
                let kind = if i.revised { "revised" } else { "insight" };
                self.log(kind, i.text());
            }
            // a law consolidated since: the machine holds it as it is now
            for (name, held) in self.embodied.clone() {
                if self.time.nodes.get(&name).is_some_and(|n| n.consolidations() != held) {
                    let _ = self.embody(&name);
                }
            }
        }
        self.step_integrations();
        // what it feels after this tick: its surprise against what it expected, its room
        let surprise = if self.machine.last_surprise != before_surprise {
            crate::mind::rms(&self.machine.last_surprise)
        } else {
            None
        };
        let links = crate::mind::links(&self.machine.net);
        let room = crate::mind::capacity(&self.machine.net, self.mood.host);
        self.mood.after_tick(surprise, links, room);
        writes
    }

    // ------------------------------------------------------------------ what a source holds, said again
    /// Says again everything a source holds (an app being edited while it runs): what it said before is taken
    /// away and `lines` are said in its place, in one change of the network (nothing in between is ever seen).
    /// What it was living through is kept: a node written by its `set`s, a variable, an input or a weight keeps the
    /// value it had, if it is still there. A line that is not understood is skipped (the others are said): what
    /// comes back is, for each line, what it did, an error, or None when it is not a statement of the language
    /// (a command, for the prompt to run). `state`: values to start from (an app opened again), by node.
    pub fn rebuild(
        &mut self,
        source: &str,
        lines: &[String],
        base: Option<&std::path::Path>,
        state: &IndexMap<String, f64>,
    ) -> Result<Vec<std::result::Result<Option<String>, String>>> {
        let mut net = self.machine.net.clone();
        // what it was living through
        let mut kept: IndexMap<String, (f64, f64)> = IndexMap::new();
        let written: Vec<String> = net.sets.iter().map(|s| s.target.clone()).collect();
        for (n, spec) in &net.nodes {
            if spec.origin.source == source && (spec.var || spec.learn || spec.input || written.contains(n)) {
                if let Some(m) = net.measured.get(n) {
                    kept.insert(n.clone(), *m);
                }
            }
        }
        // what it said before, taken away
        let (nodes, relations) = net.from_source(source);
        for r in &relations {
            let sets: Vec<usize> = net
                .sets
                .iter()
                .enumerate()
                .filter(|(_, st)| {
                    st.source
                        .rsplit_once("__next")
                        .is_some_and(|(t, k)| t == st.target && *r == format!("set_{}_{}", st.target, k))
                })
                .map(|(k, _)| k)
                .collect();
            for k in sets.into_iter().rev() {
                let st = net.sets.remove(k);
                net.nodes.shift_remove(&st.source);
                net.measured.shift_remove(&st.source);
            }
            net.take_relation(r);
        }
        for n in &nodes {
            if net.degree(n) == 0 {
                net.nodes.shift_remove(n);
                net.measured.shift_remove(n);
                net.sets.retain(|s| s.target != *n && s.source != *n);
            } else {
                // still held by another source's relation: it stays, as nothing measured it
                net.measured.shift_remove(n);
            }
        }
        let origin = Provenance::of(source);
        let mut out = vec![];
        for l in lines {
            let mut trial = net.clone();
            match lang::statement(&mut trial, l, base, &origin) {
                Ok(said) => {
                    net = trial;
                    out.push(Ok(said));
                }
                Err(e) => out.push(Err(e.message)),
            }
        }
        // a block left open (network ... without end) is not kept open
        net.recording = None;
        // what it was living through, given back to what is still there
        for (n, m) in kept.iter() {
            if net.nodes.contains_key(n) && net.measured.contains_key(n) {
                net.measured.insert(n.clone(), *m);
            }
        }
        for (n, v) in state {
            if let Some(m) = net.measured.get_mut(n) {
                m.0 = *v;
            }
        }
        self.machine = self.machine.grown(net)?;
        // a node that is gone no longer lives in time
        let gone: Vec<String> =
            self.time.nodes.keys().filter(|n| !self.machine.net.nodes.contains_key(*n)).cloned().collect();
        for n in gone {
            self.time.nodes.shift_remove(&n);
        }
        self.quarantine.retain(|q| q != source || !lines.is_empty());
        Ok(out)
    }

    /// The values a source's nodes are living through (what `rebuild` keeps), to open it again where it was.
    pub fn state_of(&self, source: &str) -> IndexMap<String, f64> {
        let net = &self.machine.net;
        let written: Vec<String> = net.sets.iter().map(|s| s.target.clone()).collect();
        net.nodes
            .iter()
            .filter(|(n, s)| s.origin.source == source && (s.var || s.learn || s.input || written.contains(*n)))
            .filter_map(|(n, _)| net.measured.get(n).map(|m| (n.clone(), m.0)))
            .collect()
    }

    // ------------------------------------------------------------------ sleeping and waking
    /// Writes the whole world to a file: its network, the state of every processor, its sandboxes, what is rising
    /// and what is in quarantine, its logbook, and its nodes that live in time (their memories, their laws, their
    /// hidden states). `wake` gives it back exactly as it was.
    pub fn sleep(&self, path: &std::path::Path) -> Result<()> {
        let mut machine_net = self.machine.net.clone();
        machine_net.recording = None;
        let sleeping = Sleeping {
            format: SLEEP_FORMAT.to_string(),
            net: machine_net,
            state: self.machine.state(),
            sandboxes: self.sandboxes.values().map(|s| (s.name.clone(), s.lines.clone(), s.net.clone())).collect(),
            integrations: self.integrations.clone(),
            quarantine: self.quarantine.clone(),
            journal: self.journal.clone(),
            rounds_per_tick: self.rounds_per_tick,
            time: self.time.clone(),
            senses: self.senses.clone(),
            rows: self.rows.clone(),
            laws: self.laws.clone(),
            embodied: self.embodied.clone(),
        };
        let bytes = bincode::serialize(&sleeping).map_err(|e| GrmError::new(format!("cannot write the world: {e}")))?;
        // written beside, then put in place: a world is never half written
        let tmp = path.with_extension("sleeping");
        std::fs::write(&tmp, bytes).map_err(|e| GrmError::new(format!("{}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, path).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
        // its workshops: each a world of its own, asleep beside it (with what was written in it)
        let dir = path.with_extension("workshops");
        if !self.workshops.is_empty() || dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| GrmError::new(format!("{}: {e}", dir.display())))?;
            for (name, ws) in &self.workshops {
                ws.sleep(&dir.join(format!("{name}.world")))?;
                let lines = serde_json::to_string(&ws.written).unwrap_or_default();
                let _ = std::fs::write(dir.join(format!("{name}.lines.json")), lines);
            }
            // a workshop closed since: its files go
            if let Ok(d) = std::fs::read_dir(&dir) {
                for e in d.flatten() {
                    let p = e.path();
                    let file = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let name = file.split('.').next().unwrap_or("").to_string();
                    if !self.workshops.contains_key(&name) {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
        Ok(())
    }

    /// A world as it was put to sleep.
    pub fn wake(path: &std::path::Path) -> Result<World> {
        let bytes = std::fs::read(path).map_err(|e| GrmError::new(format!("{}: {e}", path.display())))?;
        let s: Sleeping = bincode::deserialize(&bytes).map_err(|_| {
            GrmError::new(format!("{} is not a sleeping world (or of another version)", path.display()))
        })?;
        if s.format != SLEEP_FORMAT {
            bail!("{} is a world of another format ({})", path.display(), s.format);
        }
        let mut sandboxes = IndexMap::new();
        for (name, lines, net) in s.sandboxes {
            sandboxes.insert(name.clone(), Sandbox { name, lines, net, machine: None });
        }
        let mut w = World {
            machine: Machine::wake(s.net, s.state)?,
            sandboxes,
            integrations: s.integrations,
            quarantine: s.quarantine,
            journal: s.journal,
            rounds_per_tick: s.rounds_per_tick,
            time: s.time,
            senses: s.senses,
            rows: s.rows,
            laws: s.laws,
            embodied: s.embodied,
            eyes: IndexMap::new(),
            mood: Default::default(),
            workshops: IndexMap::new(),
            written: vec![],
        };
        // its workshops, woken too
        let dir = path.with_extension("workshops");
        if let Ok(d) = std::fs::read_dir(&dir) {
            let mut files: Vec<std::path::PathBuf> =
                d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "world")).collect();
            files.sort();
            for f in files {
                let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Ok(mut ws) = World::wake(&f) {
                    ws.written = std::fs::read_to_string(dir.join(format!("{name}.lines.json")))
                        .ok()
                        .and_then(|t| serde_json::from_str(&t).ok())
                        .unwrap_or_default();
                    w.workshops.insert(name, Box::new(ws));
                }
            }
        }
        w.log("woke", format!("woke from {}", path.display()));
        Ok(w)
    }

    // ------------------------------------------------------------------ time
    /// A node starts living in time: it remembers, learns how it moves, and can be foreseen.
    pub fn track(&mut self, name: &str, lags: usize, couplings: &[&str]) -> Result<String> {
        // a vector lives in time component by component
        if let Some(&d) = self.machine.net.vectors.get(name) {
            for k in 0..d {
                self.track(&format!("{name}_{k}"), lags, couplings)?;
            }
            return Ok(format!(
                "{name} lives in time: its {d} components remember, learn how they move, and can be foreseen"
            ));
        }
        for n in std::iter::once(&name).chain(couplings.iter()) {
            if !self.machine.net.nodes.contains_key(*n) {
                bail!("no node {n}");
            }
        }
        self.time.track(name, lags, couplings);
        let text = format!(
            "{name} lives in time: it remembers, learns how it moves ({lags} lag(s){}), and can be foreseen",
            if couplings.is_empty() { String::new() } else { format!(", with {}", couplings.join(", ")) }
        );
        self.log("time", text.clone());
        Ok(text)
    }

    /// The machine holds a node's hidden law as its own relations: its state (`x__state`), its states before
    /// (carried from tick to tick with their uncertainty), its law (`x__law`: the state from the states before, the
    /// couplings the moment before, a constant, the world's noise), and how it is measured (`x__seen`: the node is
    /// its state plus its rhythms, give or take its senses' noise). The machine then follows the node itself, as a
    /// Kalman filter does, with every other relation of the network heard at once; a moment not measured is
    /// foreseen by the machine. The law is held again each time it is consolidated. A positive node (held in log by
    /// the machine) is tied to its state by the converter between the two worlds.
    pub fn embody(&mut self, name: &str) -> Result<String> {
        let Some(node) = self.time.nodes.get(name) else { bail!("{name} does not live in time (track it first)") };
        let Some(h) = node.hidden.clone() else { bail!("{name} has no hidden law yet (it needs 64 moments)") };
        let held = node.consolidations();
        let g = |v: f64| fmt_g(v, 12);
        let term = |a: f64, n: &str| format!("{} {} * {n}", if a < 0.0 { "-" } else { "+" }, g(a.abs()));
        let state = format!("{name}__state");
        let mut lines: Vec<String> = vec![];
        let new = !self.embodied.contains_key(name);
        if new {
            lines.push(format!("node {state} = {} signed", g(h.s[0])));
            // its states before, as sure of them as the hidden law is, then carried together from tick to tick
            // (the lags of one state move together: what links them is carried too)
            for k in 1..=h.p {
                let lag = format!("{state}_{k}");
                let i = (k - 1).min(h.p - 1);
                lines.push(format!("var {lag} = {}", g(h.s[i])));
                lines.push(format!("observe {lag} = {} ± {}", g(h.s[i]), g(h.cov[i][i].max(1e-24).sqrt())));
            }
            let targets: Vec<String> = (1..=h.p).map(|k| format!("{state}_{k}")).collect();
            let sources: Vec<String> =
                (0..h.p).map(|k| if k == 0 { state.clone() } else { format!("{state}_{k}") }).collect();
            lines.push(format!("carry {} = {}", targets.join(", "), sources.join(", ")));
            for c in &h.couplings {
                lines.push(format!("var {c}__before = {}", g(self.machine.value_si(c))));
                lines.push(format!("carry {c}__before = {c}"));
            }
        }
        let mut law = format!("relate {name}__law: {state} =");
        for k in 0..h.p {
            law.push_str(&format!(" {}", term(h.theta[k], &format!("{state}_{}", k + 1))));
        }
        for (j, c) in h.couplings.iter().enumerate() {
            law.push_str(&format!(" {}", term(h.theta[h.p + j], &format!("{c}__before"))));
        }
        law.push_str(&format!(" {} ± {}", term(h.theta[h.theta.len() - 1], "1"), g(h.q.sqrt())));
        lines.push(law.replace("= +", "=").replace(" * 1 ±", " ±"));
        let mut seen = format!("relate {name}__seen: {name} = {state}");
        for (i, p) in h.rhythms.periods.iter().enumerate() {
            let tag = g(*p).replace('.', "_");
            for (wave, f) in [("cos", 0usize), ("sin", 1)] {
                let node = format!("{name}__{wave}_{tag}");
                if !self.machine.net.nodes.contains_key(&node) {
                    lines.push(format!("input {node}"));
                }
                seen.push_str(&format!(" {}", term(h.rhythms.beta[2 * i + f], &node)));
            }
        }
        seen.push_str(&format!(" ± {}", g(h.r.sqrt())));
        lines.push(seen);
        let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
        self.say_all(&refs, &Provenance::of(format!("time:{name}")))?;
        self.embodied.insert(name.to_string(), held);
        let text = if new {
            format!(
                "the machine holds the law of {name}: it follows it itself ({}{} lag(s), world ± {}, senses ± {})",
                if h.rhythms.periods.is_empty() { "" } else { "its cycles, " },
                h.p,
                fmt_g(h.q.sqrt(), 3),
                fmt_g(h.r.sqrt(), 3)
            )
        } else {
            format!("the law of {name} the machine holds is the one consolidated now")
        };
        if new {
            self.log("time", text.clone());
        }
        Ok(text)
    }

    /// An eye sees a picture (it opens at the picture's size the first time, or anew if the size changes): it
    /// cleans it, is surprised where it does not fit, sees what changed, and its glance (the top of its pyramid)
    /// comes into the world's network as inputs `NAME_glance_x_y` (one per channel: `NAME_glance_x_y_c`).
    pub fn see(
        &mut self,
        name: &str,
        picture: &crate::vision::Picture,
        budget: std::time::Duration,
    ) -> Result<crate::vision::Sight> {
        if !is_name(name) {
            bail!("an eye is named as a node is: {name}");
        }
        let fits = self
            .eyes
            .get(name)
            .is_some_and(|e| (e.width, e.height, e.channels) == (picture.width, picture.height, picture.channels));
        if !fits {
            let eye = crate::vision::Eye::new(
                name,
                picture.width,
                picture.height,
                picture.channels,
                crate::vision::SENSE,
                crate::vision::SMOOTH,
            )?;
            let (nodes, relations) = eye.size();
            self.eyes.insert(name.to_string(), eye);
            self.log(
                "eye",
                format!(
                    "eye {name} opened: {}x{}x{}, {nodes} nodes, {relations} relations",
                    picture.width, picture.height, picture.channels
                ),
            );
        }
        let sight = self.eyes.get_mut(name).unwrap().see(picture, budget)?;
        let (gw, gh, values) = &sight.glance;
        let channels = picture.channels;
        let glance = |x: usize, y: usize, c: usize| {
            if channels == 1 {
                format!("{name}_glance_{x}_{y}")
            } else {
                format!("{name}_glance_{x}_{y}_{c}")
            }
        };
        let missing: Vec<String> = (0..channels)
            .flat_map(|c| (0..*gh).flat_map(move |y| (0..*gw).map(move |x| (x, y, c))))
            .map(|(x, y, c)| glance(x, y, c))
            .filter(|n| !self.machine.net.nodes.contains_key(n))
            .map(|n| format!("input {n}"))
            .collect();
        if !missing.is_empty() {
            let refs: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            self.say_all(&refs, &Provenance::of(format!("eye:{name}")))?;
        }
        for c in 0..channels {
            for y in 0..*gh {
                for x in 0..*gw {
                    let v = values[c * gw * gh + y * gw + x];
                    self.machine.observe(&glance(x, y, c), v, Some(crate::vision::SENSE / 4.0))?;
                }
            }
        }
        Ok(sight)
    }

    /// What the network believes of several nodes together (a vector's components, or any nodes): each value
    /// with its exact sigma, and how they move together (their correlations).
    pub fn joint(&self, names: &[&str]) -> Result<Joint> {
        let mut all: Vec<String> = vec![];
        for n in names {
            match self.machine.net.vectors.get(*n) {
                Some(&d) => all.extend((0..d).map(|k| format!("{n}_{k}"))),
                None => all.push(n.to_string()),
            }
        }
        let refs: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
        let cov = self.machine.covariance(&refs)?;
        let m = &self.machine;
        let k = all.len();
        let mut values = vec![];
        let mut sigmas = vec![];
        for (i, n) in all.iter().enumerate() {
            let v = m.value_si(n);
            let sd = cov[i][i].max(0.0).sqrt();
            let signed = m.var_signed[m.index[n]];
            // a positive node is held in log10: its sigma in SI, near its value
            let shift = m.shift.get(n).copied().unwrap_or(0.0);
            values.push(v);
            sigmas.push(if signed { sd * 10f64.powf(shift) } else { v * std::f64::consts::LN_10 * sd });
        }
        let mut correlation = vec![vec![0.0; k]; k];
        for a in 0..k {
            for b in 0..k {
                let d = (cov[a][a] * cov[b][b]).sqrt();
                correlation[a][b] = if d > 0.0 {
                    cov[a][b] / d
                } else if a == b {
                    1.0
                } else {
                    0.0
                };
            }
        }
        Ok(Joint { names: all, values, sigmas, correlation })
    }

    /// Rows of data, each at its moment: each row is measured, then a tick at that moment.
    ///
    /// A value missing from a row is no measurement. A node that lives in time is then given what it expected
    /// (its forecast, with its spread) as a soft measurement, so the relations still hear of it; it is never
    /// remembered as measured. Any other node keeps what it believes.
    pub fn feed_rows(&mut self, rows: &[(f64, IndexMap<String, f64>)]) -> Result<usize> {
        let mut fed = 0;
        for (t, row) in rows {
            let mut measured = IndexMap::new();
            for (n, v) in row {
                // how much a measurement of it is trusted: as it was first said, for every row
                let sense = match self.senses.get(n) {
                    Some(s) => *s,
                    None => {
                        let s = self.machine.net.measured.get(n).map(|m| m.1).unwrap_or(1e-3 * v.abs().max(1.0));
                        self.senses.insert(n.clone(), s);
                        s
                    }
                };
                if v.is_finite() {
                    self.machine.observe(n, *v, Some(sense))?;
                    measured.insert(n.clone(), *v);
                } else if self.embodied.contains_key(n) {
                    self.machine.forget(n); // the machine's own law foresees it
                } else if let Some((m, var)) = self.time.expected(n) {
                    let i = self.machine.index.get(n).copied();
                    let signed = i.map(|i| self.machine.var_signed[i]).unwrap_or(true);
                    let sd = var.max(0.0).sqrt();
                    if signed {
                        self.machine.observe(n, m, Some(sd.max(1e-12)))?;
                    } else if m > 0.0 {
                        self.machine.observe(n, m, Some((sd / (m * std::f64::consts::LN_10)).max(1e-9)))?;
                    } else {
                        self.machine.forget(n);
                    }
                } else {
                    self.machine.forget(n); // no measurement: the node keeps what it believes
                }
            }
            self.tick_at(*t, true, &measured);
            fed += 1;
        }
        // after a file, it looks for what holds between its columns
        if fed >= crate::discover::MIN_ROWS {
            self.discover();
        }
        Ok(fed)
    }

    /// Looks for laws that hold on the rows it has seen at the same moment, and proposes each to the network
    /// through a sandbox: a compatible one is merged little by little; a surprising one waits, in its sandbox,
    /// for a human to look at it. Returns what it found, in words.
    pub fn discover(&mut self) -> Vec<String> {
        let laws = find_laws(&self.rows.rows, &self.laws);
        let mut said = vec![];
        for law in laws {
            let k = (1..).find(|k| {
                let n = format!("found_{k}");
                !self.sandboxes.contains_key(&n) && self.machine.net.relation(&n).is_none()
            });
            let name = format!("found_{}", k.unwrap_or(1));
            let nodes = &self.machine.net.nodes;
            let statements = law.statements(&name, &|n: &str| nodes.get(n).is_some_and(|s| !s.signed));
            self.laws.push((law.target.clone(), law.inputs.clone()));
            let tried = self.open_sandbox(&name).and_then(|_| {
                for l in &statements {
                    self.say_in(&name, l)?;
                }
                self.trial_merge(&name, 2000)
            });
            match tried {
                Ok(report) if report.compatible() => {
                    let _ = self.accept_merge(&name);
                    let text = format!("{} — merged, little by little ({name})", law.text());
                    self.log("found", text.clone());
                    said.push(text);
                }
                Ok(_) => {
                    let text =
                        format!("{} — surprising: it waits in sandbox {name} for a human to look at it", law.text());
                    self.log("found", text.clone());
                    said.push(text);
                }
                Err(e) => {
                    let _ = self.sandboxes.shift_remove(&name);
                    said.push(format!("{}: {e}", law.text()));
                }
            }
        }
        said
    }

    /// Where each node came from (what `why` names).
    pub fn node_sources(&self) -> IndexMap<String, String> {
        self.machine.net.nodes.iter().map(|(n, s)| (n.clone(), s.origin.source.clone())).collect()
    }

    // ------------------------------------------------------------------ sandboxes
    /// What a workshop made, said as lines (what a merge says again here, through a sandbox): its nodes, its
    /// relations, and what its inputs read last.
    pub fn workshop_lines(&self, name: &str) -> Result<Vec<String>> {
        let Some(ws) = self.workshops.get(name) else { bail!("no workshop {name}") };
        let net = &ws.machine.net;
        let mut out = vec![];
        for (n, spec) in &net.nodes {
            if n.contains("__") {
                continue;
            }
            out.push(if spec.input {
                format!("input {n}{}", if spec.signed { "" } else { " positive" })
            } else {
                format!("node {n}{}", if spec.signed { " signed" } else { "" })
            });
        }
        for r in &net.relations {
            if !r.when.is_empty() {
                let when: Vec<String> =
                    r.when.iter().map(|c| format!("{} in {} .. {}", c.node, lang::g(c.lo), lang::g(c.hi))).collect();
                out.push(format!("relate {}: {} when {}", r.name, r.text, when.join(" and ")));
            } else {
                out.push(format!("relate {}: {}", r.name, r.text));
            }
        }
        // what it measured (a positive node's measurement is kept in decades), and what its inputs read last
        for (n, spec) in &net.nodes {
            if n.contains("__") {
                continue;
            }
            let v = match net.measured.get(n) {
                Some((m, _)) => Some(if spec.signed { *m } else { 10f64.powf(*m) }),
                None if spec.input => Some(ws.machine.value(n)),
                None => None,
            };
            if let Some(v) = v.filter(|v| v.is_finite()) {
                out.push(format!("observe {n} = {}", lang::g(v)));
            }
        }
        Ok(out)
    }

    pub fn open_sandbox(&mut self, name: &str) -> Result<()> {
        if !is_name(name) {
            bail!("a sandbox is named as a node is: {name}");
        }
        if self.sandboxes.contains_key(name) {
            bail!("there is already a sandbox {name}");
        }
        self.sandboxes.insert(
            name.to_string(),
            Sandbox { name: name.to_string(), lines: vec![], net: Network::new(), machine: None },
        );
        self.log("sandbox", format!("sandbox {name} opened"));
        Ok(())
    }

    /// Says a statement in a sandbox. A node of the global network it names comes in as the global network says
    /// it (signed or not, its value): the sandbox works on the same world.
    pub fn say_in(&mut self, sandbox: &str, line: &str) -> Result<Option<String>> {
        let global = &self.machine.net;
        let Some(sb) = self.sandboxes.get_mut(sandbox) else { bail!("no sandbox {sandbox}") };
        let origin = Provenance::of(sb.source());
        for n in names_in(line) {
            if !sb.net.nodes.contains_key(&n) {
                if let Some(spec) = global.nodes.get(&n) {
                    sb.net.nodes.insert(n.clone(), spec.clone());
                }
            }
        }
        let said = lang::statement(&mut sb.net, line, None, &origin)?;
        if said.is_some() {
            sb.lines.push(line.to_string());
            sb.machine = None;
        }
        Ok(said)
    }

    /// The network a sandbox runs: its own, with the global network's beliefs at its border as measurements.
    fn sandbox_view(&self, sandbox: &str) -> Result<Network> {
        let Some(sb) = self.sandboxes.get(sandbox) else { bail!("no sandbox {sandbox}") };
        let mut view = sb.net.clone();
        for (n, spec) in &sb.net.nodes {
            if spec.origin.source != sb.source()
                && self.machine.net.nodes.contains_key(n)
                && !sb.net.measured.contains_key(n)
                && self.machine.index.contains_key(n)
            {
                let (v, s) = self.machine.reading(n);
                if s.is_finite() && (spec.signed || v > 0.0) {
                    view.measured.insert(n.clone(), (v, s.max(1e-12)));
                }
            }
        }
        Ok(view)
    }

    /// The sandbox computes itself (the global network is not touched). Returns its rounds.
    pub fn run_sandbox(&mut self, sandbox: &str, max_rounds: u64) -> Result<u64> {
        let view = self.sandbox_view(sandbox)?;
        let sb = self.sandboxes.get_mut(sandbox).unwrap();
        let mut m = match sb.machine.take() {
            Some(old) => old.grown(view)?,
            None => Machine::new(view)?,
        };
        let r = m.run(max_rounds);
        sb.machine = Some(m);
        Ok(r)
    }

    /// The global network with the sandbox in it, as a working copy (nothing is touched).
    fn with_sandbox(&self, sandbox: &str, strength: f64) -> Result<Network> {
        let Some(sb) = self.sandboxes.get(sandbox) else { bail!("no sandbox {sandbox}") };
        let mut net = self.machine.net.clone();
        let origin = Provenance::of(sb.source());
        for l in &sb.lines {
            lang::statement(&mut net, l, None, &origin)?;
        }
        for r in net.relations.iter_mut().filter(|r| r.origin.source == sb.source()) {
            r.strength = strength;
        }
        Ok(net)
    }

    /// A trial merge: what the sandbox would change in the global network, before anything is changed.
    pub fn trial_merge(&self, sandbox: &str, max_rounds: u64) -> Result<MergeReport> {
        let Some(sb) = self.sandboxes.get(sandbox) else { bail!("no sandbox {sandbox}") };
        let global = &self.machine;
        let net = self.with_sandbox(sandbox, 1.0)?;
        let mut trial = global.grown(net)?;
        let (r0, m0) = (trial.rounds, trial.messages);
        trial.run(max_rounds);
        let gnet = &global.net;
        let new_nodes = sb.net.nodes.keys().filter(|n| !gnet.nodes.contains_key(*n)).cloned().collect();
        let new_relations: Vec<String> =
            sb.net.relations.iter().map(|r| r.name.clone()).filter(|n| gnet.relation(n).is_none()).collect();
        let replaced = sb.net.relations.iter().map(|r| r.name.clone()).filter(|n| gnet.relation(n).is_some()).collect();
        let conflicts = sb
            .net
            .nodes
            .iter()
            .filter(|(n, s)| {
                gnet.nodes
                    .get(*n)
                    .is_some_and(|g: &NodeSpec| g.signed != s.signed || g.fixed != s.fixed || g.guess != s.guess)
            })
            .map(|(n, _)| n.clone())
            .collect();
        let mut tensions = vec![];
        let names: IndexMap<&str, usize> =
            global.factors.iter().enumerate().map(|(k, f)| (f.name.as_str(), k)).collect();
        for (k, f) in trial.factors.iter().enumerate() {
            if !matches!(f.kind, FKind::Relation | FKind::Nonlinear) || f.source == sb.source() {
                continue;
            }
            if let Some(&j) = names.get(f.name.as_str()) {
                let (a, b) = (global.residual(j).abs(), trial.residual(k).abs());
                if b > a + 1.0 {
                    tensions.push((f.name.clone(), a, b));
                }
            }
        }
        tensions.sort_by(|x, y| (y.2 - y.1).partial_cmp(&(x.2 - x.1)).unwrap_or(std::cmp::Ordering::Equal));
        let mut surprises = vec![];
        for (n, &i) in &global.index {
            if n.starts_with('~') || !trial.index.contains_key(n) {
                continue;
            }
            let s = global.sigma(n);
            if s.is_finite() && s > 0.0 {
                let moved = (trial.x[trial.index[n]] - global.x[i]).abs() / s;
                if moved > SURPRISE_SIGMAS {
                    surprises.push((n.clone(), moved));
                }
            }
        }
        surprises.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(MergeReport {
            sandbox: sandbox.to_string(),
            new_nodes,
            new_relations,
            replaced,
            conflicts,
            tensions,
            harmony_before: global.harmony(),
            harmony_after: trial.harmony(),
            surprises,
            rounds: trial.rounds - r0,
            messages: trial.messages - m0,
            settled: trial.quiet(),
        })
    }

    /// Accepts a merge: the sandbox goes into the global network, its relations at the first step of their rise.
    pub fn accept_merge(&mut self, sandbox: &str) -> Result<String> {
        let harmony_before = self.machine.harmony();
        let net = self.with_sandbox(sandbox, RAMP[0])?;
        self.machine = self.machine.grown(net)?;
        let sb = self.sandboxes.shift_remove(sandbox).unwrap();
        self.integrations.push(Integration { source: sb.source(), step: 0, harmony_before });
        let text = format!("{} merged: it rises little by little ({:.0} %)", sb.source(), RAMP[0] * 100.0);
        self.log("merged", text.clone());
        Ok(text)
    }

    pub fn refuse_merge(&mut self, sandbox: &str) -> Result<String> {
        if self.sandboxes.shift_remove(sandbox).is_none() {
            bail!("no sandbox {sandbox}");
        }
        let text = format!("sandbox {sandbox} not merged: the global network was not touched");
        self.log("refused", text.clone());
        Ok(text)
    }

    fn set_strength(&mut self, source: &str, strength: f64) -> Result<()> {
        let mut net = self.machine.net.clone();
        for r in net.relations.iter_mut().filter(|r| r.origin.source == source) {
            r.strength = strength;
        }
        self.machine = self.machine.grown(net)?;
        Ok(())
    }

    /// Each merge rising takes one more step, unless the network has lost its harmony or been surprised since it
    /// came in: then it goes to quarantine.
    pub fn step_integrations(&mut self) {
        for it in self.integrations.clone() {
            let harmony = self.machine.harmony();
            let surprised = self.machine.last_surprise.values().any(|r| r.abs() > SURPRISE_SIGMAS * 2.0);
            if harmony < it.harmony_before - QUARANTINE_DROP || surprised {
                let _ = self.quarantine(
                    &it.source,
                    &format!(
                        "harmony {:.3} -> {:.3}{}",
                        it.harmony_before,
                        harmony,
                        if surprised { ", surprised" } else { "" }
                    ),
                );
                continue;
            }
            if it.step + 1 < RAMP.len() {
                let s = RAMP[it.step + 1];
                if let Some(i) = self.integrations.iter_mut().find(|i| i.source == it.source) {
                    i.step += 1;
                }
                if self.set_strength(&it.source, s).is_ok() && s >= 1.0 {
                    self.log("integrated", format!("{} holds fully", it.source));
                    self.integrations.retain(|i| i.source != it.source);
                }
            }
        }
    }

    /// Everything that came from a source stops holding (strength 0), and what it taught the weights is taken back.
    /// It stays in the network, visible, until it is confirmed or recalled.
    pub fn quarantine(&mut self, source: &str, why: &str) -> Result<String> {
        self.set_strength(source, 0.0)?;
        self.machine.unlearn(source);
        if !self.quarantine.iter().any(|s| s == source) {
            self.quarantine.push(source.to_string());
        }
        self.integrations.retain(|i| i.source != source);
        let text = format!("{source} in quarantine: {why}");
        self.log("quarantine", text.clone());
        Ok(text)
    }

    /// A source out of quarantine: it rises again, little by little.
    pub fn release(&mut self, source: &str) -> Result<String> {
        if !self.quarantine.iter().any(|s| s == source) {
            bail!("{source} is not in quarantine");
        }
        self.quarantine.retain(|s| s != source);
        let harmony_before = self.machine.harmony();
        self.set_strength(source, RAMP[0])?;
        self.integrations.push(Integration { source: source.to_string(), step: 0, harmony_before });
        let text = format!("{source} out of quarantine: it rises again");
        self.log("released", text.clone());
        Ok(text)
    }

    /// Takes away everything that came from a source, everywhere: its relations, its nodes that nothing else holds,
    /// and what it taught the weights.
    pub fn recall(&mut self, source: &str) -> Result<String> {
        let (nodes, relations) = self.machine.net.from_source(source);
        let plan = Removal { kind: "source", name: source.to_string(), nodes, relations };
        let plan = self.keep_held(plan);
        self.machine.unlearn(source);
        self.quarantine.retain(|s| s != source);
        self.integrations.retain(|i| i.source != source);
        self.remove(&plan)
    }

    // ------------------------------------------------------------------ taking away
    /// The nodes of a plan that something else still holds stay.
    fn keep_held(&self, mut plan: Removal) -> Removal {
        let net = &self.machine.net;
        let held: Vec<String> =
            net.relations.iter().filter(|r| !plan.relations.contains(&r.name)).flat_map(|r| r.nodes()).collect();
        let written: Vec<String> = net
            .sets
            .iter()
            .filter(|s| !plan.nodes.contains(&s.target))
            .flat_map(|s| [s.target.clone(), s.source.clone()])
            .collect();
        plan.nodes.retain(|n| !held.contains(n) && !written.contains(n));
        plan
    }

    /// What `remove` would take away, without taking it:
    ///   remove SOURCE      a network in the network (where things came from): its relations, its nodes nothing
    ///                      else holds
    ///   remove NODE        the node and every relation that holds it
    ///   remove RELATION    the relation (a `set`'s: with what it writes)
    pub fn removal(&self, words: &[&str]) -> Result<Removal> {
        let net = &self.machine.net;
        let kind =
            words.first().map(|w| w.to_lowercase()).filter(|w| matches!(w.as_str(), "network" | "node" | "relation"));
        let name =
            if kind.is_some() { words.get(1).copied().unwrap_or("") } else { words.first().copied().unwrap_or("") };
        let sources: Vec<String> = net
            .relations
            .iter()
            .map(|r| r.origin.source.clone())
            .chain(net.nodes.values().map(|s| s.origin.source.clone()))
            .collect();
        if matches!(kind.as_deref(), None | Some("network")) && sources.iter().any(|s| s == name) {
            let (nodes, relations) = net.from_source(name);
            return Ok(self.keep_held(Removal { kind: "network", name: name.to_string(), nodes, relations }));
        }
        let name = net.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
        if matches!(kind.as_deref(), None | Some("node")) && net.nodes.contains_key(name) {
            let relations =
                net.relations.iter().filter(|r| r.nodes().iter().any(|n| n == name)).map(|r| r.name.clone()).collect();
            return Ok(Removal { kind: "node", name: name.to_string(), nodes: vec![name.to_string()], relations });
        }
        if matches!(kind.as_deref(), None | Some("relation")) && net.relation(name).is_some() {
            return Ok(Removal {
                kind: "relation",
                name: name.to_string(),
                nodes: vec![],
                relations: vec![name.to_string()],
            });
        }
        bail!("nothing called {name}")
    }

    /// Takes away what removal() said; the machine goes on from where it was. It goes in the logbook.
    pub fn remove(&mut self, plan: &Removal) -> Result<String> {
        let mut net = self.machine.net.clone();
        let mut nodes = plan.nodes.clone();
        for r in &plan.relations {
            // a `set`'s relation: the set and what it writes go with it
            let sets: Vec<usize> = net
                .sets
                .iter()
                .enumerate()
                .filter(|(_, st)| {
                    st.source
                        .rsplit_once("__next")
                        .is_some_and(|(t, k)| t == st.target && *r == format!("set_{}_{}", st.target, k))
                })
                .map(|(k, _)| k)
                .collect();
            for k in sets.into_iter().rev() {
                let st = net.sets.remove(k);
                nodes.push(st.source);
            }
            net.take_relation(r);
        }
        for n in &nodes {
            let held: Vec<String> =
                net.relations.iter().filter(|r| r.nodes().iter().any(|x| x == n)).map(|r| r.name.clone()).collect();
            for r in held {
                net.take_relation(&r);
            }
            net.sets.retain(|s| s.target != *n && s.source != *n);
            net.nodes.shift_remove(n);
            net.measured.shift_remove(n);
            net.links.retain(|l| !l.names.contains(n));
        }
        self.machine = self.machine.grown(net)?;
        let text = format!(
            "removed {} {}: {} node(s), {} relation(s)",
            plan.kind,
            plan.name,
            nodes.len(),
            plan.relations.len()
        );
        self.log("removed", text.clone());
        Ok(text)
    }

    /// The sources the network holds things from, with how many relations each.
    pub fn sources(&self) -> IndexMap<String, usize> {
        let mut out: IndexMap<String, usize> = IndexMap::new();
        for r in &self.machine.net.relations {
            *out.entry(r.origin.source.clone()).or_insert(0) += 1;
        }
        out
    }

    /// How strongly each relation of a source holds now.
    pub fn strength_of(&self, source: &str) -> Option<f64> {
        self.machine.net.relations.iter().find(|r| r.origin.source == source).map(|r| r.strength)
    }

    /// The relations of a kind a sandbox may not bring without a human saying yes (reserved for the sas).
    pub fn is_product(rel: &Kind) -> bool {
        matches!(rel, Kind::Product { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(lines: &[&str]) -> World {
        let mut w = World::new();
        w.say_all(lines, &Provenance::of("base")).unwrap();
        w.machine.run(1000);
        w
    }

    #[test]
    fn a_measurement_is_taken_in_place() {
        let mut w = world(&["node x signed", "node y signed", "relate r: y = 2 * x", "observe x = 1"]);
        let before = w.machine.messages;
        w.say("observe x = 3", &Provenance::default()).unwrap();
        w.machine.run(1000);
        assert!((w.machine.value("y") - 6.0).abs() < 1e-6);
        assert!(w.machine.messages - before < 20);
    }

    #[test]
    fn growing_the_network_wakes_only_what_is_new() {
        let mut lines: Vec<String> = (0..300).map(|k| format!("relate c{k}: a{} = a{k} + 1", k + 1)).collect();
        lines.insert(0, "observe a0 = 0".into());
        lines.insert(0, "node a0 signed".into());
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let mut w = world(&refs);
        let fresh = w.machine.messages;
        let before = w.machine.messages;
        w.say("relate extra: b = a300 * 2", &Provenance::default()).unwrap();
        w.machine.run(1000);
        assert!((w.machine.value("b") - 600.0).abs() < 1e-3, "{}", w.machine.value("b"));
        // a new relation at the end of a chain of 300: a few messages, not the whole chain again
        assert!(w.machine.messages - before < fresh / 20, "{} vs {}", w.machine.messages - before, fresh);
    }

    #[test]
    fn a_sandbox_is_isolated_and_its_merge_reported() {
        let mut w = world(&["node x signed", "node y signed", "relate r: y = 2 * x", "observe x = 1 ± 0.001"]);
        w.open_sandbox("idea").unwrap();
        w.say_in("idea", "relate s: z = y + 1").unwrap();
        w.run_sandbox("idea", 1000).unwrap();
        let sb = w.sandboxes["idea"].machine.as_ref().unwrap();
        assert!((sb.value("z") - 3.0).abs() < 1e-3);
        assert!(!w.net().nodes.contains_key("z")); // the global network does not know it
        let report = w.trial_merge("idea", 1000).unwrap();
        assert_eq!(report.new_relations, vec!["s"]);
        assert!(report.compatible(), "{}", report.text());
        assert!(!w.net().nodes.contains_key("z")); // a trial touches nothing
        w.accept_merge("idea").unwrap();
        assert_eq!(w.strength_of("sandbox:idea"), Some(RAMP[0]));
        for _ in 0..RAMP.len() {
            w.tick(false);
        }
        assert_eq!(w.strength_of("sandbox:idea"), Some(1.0));
        assert!((w.machine.value("z") - 3.0).abs() < 1e-3);
    }

    #[test]
    fn a_contradiction_is_surprising_and_goes_to_quarantine() {
        let mut w = world(&[
            "node x signed",
            "node y signed",
            "relate r: y = 2 * x",
            "observe x = 1 ± 0.001",
            "observe y = 2 ± 0.001",
        ]);
        w.open_sandbox("liar").unwrap();
        w.say_in("liar", "relate lie: y = 10 * x").unwrap();
        let report = w.trial_merge("liar", 1000).unwrap();
        assert!(!report.compatible(), "{}", report.text());
        assert!(!report.tensions.is_empty());
        w.accept_merge("liar").unwrap();
        for _ in 0..RAMP.len() {
            w.tick(false);
        }
        assert!(w.quarantine.contains(&"sandbox:liar".to_string()), "{:?}", w.journal);
        assert_eq!(w.strength_of("sandbox:liar"), Some(0.0));
        w.machine.run(1000);
        assert!((w.machine.value("y") - 2.0).abs() < 1e-2); // the network is itself again
        w.recall("sandbox:liar").unwrap();
        assert!(w.net().relation("lie").is_none());
    }

    #[test]
    fn what_a_source_taught_can_be_taken_back() {
        let mut w = world(&["input x", "input y", "learn k = 0", "relate fit: y = k * x ± 0.01"]);
        for (x, y) in [(1.0, 2.0), (2.0, 4.0), (3.0, 6.0)] {
            w.say(&format!("observe x = {x}"), &Provenance::default()).unwrap();
            w.say(&format!("observe y = {y}"), &Provenance::default()).unwrap();
            w.tick(true);
        }
        w.tick(true);
        assert!((w.machine.value("k") - 2.0).abs() < 1e-2);
        // what it learned is kept by the source of the relation that taught it
        assert!(w.machine.net.links.iter().any(|l| l.source == "base"));
        w.machine.unlearn("base");
        w.machine.forget("x");
        w.machine.forget("y");
        w.machine.run(1000);
        assert!(w.machine.value("k").abs() < 0.1, "{}", w.machine.value("k"));
    }

    #[test]
    fn remove_a_network_keeps_what_others_hold() {
        let mut w = World::new();
        w.say_all(&["node x signed", "relate a1: y = 2 * x"], &Provenance::of("one")).unwrap();
        w.say_all(&["relate b1: z = 3 * x"], &Provenance::of("two")).unwrap();
        let plan = w.removal(&["one"]).unwrap();
        assert_eq!(plan.relations, vec!["a1"]);
        assert_eq!(plan.nodes, vec!["y"]); // x is held by two
        w.remove(&plan).unwrap();
        assert!(w.net().relation("a1").is_none() && w.net().nodes.contains_key("x"));
        let plan = w.removal(&["node", "z"]).unwrap();
        w.remove(&plan).unwrap();
        assert!(w.net().relation("b1").is_none());
    }

    #[test]
    fn a_world_put_to_sleep_wakes_exactly_as_it_was() {
        let mut w = World::new();
        w.say_all(
            &["input x", "input y", "learn k = 0", "relate fit: y = k * x ± 0.01", "var n = 0", "set n = n + 1"],
            &Provenance::of("base"),
        )
        .unwrap();
        w.track("x", 2, &[]).unwrap();
        w.track("y", 2, &[]).unwrap();
        let mut rng = crate::time::Rng::new(5);
        let row = |k: usize, rng: &mut crate::time::Rng| {
            let x = (k as f64 / 8.0).sin() * 3.0 + 0.1 * rng.normal();
            (k as f64, IndexMap::from([("x".to_string(), x), ("y".to_string(), 2.0 * x + 0.05 * rng.normal())]))
        };
        let rows: Vec<_> = (0..200).map(|k| row(k, &mut rng)).collect();
        w.feed_rows(&rows[..120]).unwrap();
        w.open_sandbox("idea").unwrap();
        w.say_in("idea", "relate s: z = y + 1").unwrap();
        let path = std::env::temp_dir().join(format!("grm-sleep-test-{}.grm", std::process::id()));
        w.sleep(&path).unwrap();
        let mut woken = World::wake(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(woken.journal.last().unwrap().kind, "woke");
        assert!(woken.sandboxes.contains_key("idea"));
        // both go on with the same rows: the same numbers, to the last bit
        w.feed_rows(&rows[120..]).unwrap();
        woken.feed_rows(&rows[120..]).unwrap();
        for n in ["x", "y", "k", "n"] {
            assert_eq!(w.machine.value(n).to_bits(), woken.machine.value(n).to_bits(), "{n}");
        }
        assert_eq!(w.time.forecast(12, 100, 1.0), woken.time.forecast(12, 100, 1.0));
        assert_eq!(w.time.insights, woken.time.insights);
        assert!(World::wake(std::path::Path::new("/nonexistent/grm")).is_err());
    }

    #[test]
    fn a_missing_value_is_foreseen_heard_by_the_relations_and_never_remembered_as_measured() {
        let mut w = World::new();
        w.say_all(&["input x", "node y signed", "relate double: y = 2 * x ± 0.01"], &Provenance::of("base")).unwrap();
        w.track("x", 2, &[]).unwrap();
        let mut rows = vec![];
        for k in 0..300 {
            let x = 5.0 * (std::f64::consts::TAU * k as f64 / 30.0).sin();
            // x is not measured one moment in five; y never
            let v = if k % 5 == 4 { f64::NAN } else { x };
            rows.push((k as f64, IndexMap::from([("x".to_string(), v)])));
        }
        w.feed_rows(&rows).unwrap();
        // remembered: only what was measured
        assert_eq!(w.time.nodes["x"].memory.total.count, 240);
        // the last moment was a missing one: x was foreseen, and y heard of it through its relation
        let truth = 5.0 * (std::f64::consts::TAU * 299.0 / 30.0).sin();
        assert!((w.machine.value("x") - truth).abs() < 0.2, "{} vs {truth}", w.machine.value("x"));
        assert!((w.machine.value("y") - 2.0 * truth).abs() < 0.4, "{}", w.machine.value("y"));
    }

    #[test]
    fn laws_found_in_a_file_are_proposed_and_merged() {
        let mut w = World::new();
        w.say_all(&["input celsius", "input fahrenheit", "input noise"], &Provenance::of("file:t.csv")).unwrap();
        let mut rng = crate::time::Rng::new(2);
        let rows: Vec<_> = (0..200)
            .map(|k| {
                let c = 30.0 * rng.uniform() - 5.0;
                (
                    k as f64,
                    IndexMap::from([
                        ("celsius".into(), c),
                        ("fahrenheit".into(), 1.8 * c + 32.0),
                        ("noise".into(), rng.normal()),
                    ]),
                )
            })
            .collect();
        w.feed_rows(&rows).unwrap();
        assert!(
            w.journal
                .iter()
                .any(|e| e.kind == "found" && e.text.contains("fahrenheit follows a straight line of celsius")),
            "{:?}",
            w.journal
        );
        // it holds in the network: a new celsius gives its fahrenheit
        for _ in 0..crate::world::RAMP.len() {
            w.tick(false);
        }
        w.machine.forget("fahrenheit");
        w.say("observe celsius = 100", &Provenance::default()).unwrap();
        w.machine.run(10000);
        assert!(
            (w.machine.value("fahrenheit") - 212.0).abs() < 0.5,
            "{} {:?}",
            w.machine.value("fahrenheit"),
            w.journal
        );
        // it does not propose it twice
        let again = w.discover();
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn a_vector_holds_in_the_machine_and_lives_in_time() {
        let mut w = World::new();
        w.say_all(
            &["node v[3] signed", "relate e: energy = dot(v, v)", "observe v = [1, 2, 3] ± 0.0001"],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(10000);
        assert!((w.machine.value("energy") - 14.0).abs() < 1e-3, "{}", w.machine.value("energy"));
        w.track("v", 1, &[]).unwrap();
        assert_eq!(w.time.nodes.len(), 3);
    }

    #[test]
    fn a_law_with_a_logarithm_holds_in_the_network() {
        let mut w = World::new();
        w.say_all(&["input velocity", "input modulus"], &Provenance::of("file:cf4.csv")).unwrap();
        let mut rng = crate::time::Rng::new(3);
        let rows: Vec<_> = (0..150)
            .map(|k| {
                let v = 1000.0 + 20000.0 * rng.uniform();
                (
                    k as f64,
                    IndexMap::from([
                        ("velocity".into(), v),
                        ("modulus".into(), 5.0 / std::f64::consts::LN_10 * v.ln() + 15.78),
                    ]),
                )
            })
            .collect();
        w.feed_rows(&rows).unwrap();
        assert!(
            w.journal
                .iter()
                .any(|e| e.text.contains("modulus grows as the logarithm of velocity") && e.text.contains("merged")),
            "{:?}",
            w.journal
        );
        for _ in 0..crate::world::RAMP.len() {
            w.tick(false);
        }
        w.machine.forget("modulus");
        w.say("observe velocity = 10000", &Provenance::default()).unwrap();
        w.machine.run(20000);
        let want = 5.0 / std::f64::consts::LN_10 * 10000f64.ln() + 15.78;
        assert!((w.machine.value("modulus") - want).abs() < 0.05, "{} vs {want}", w.machine.value("modulus"));
    }

    #[test]
    fn the_machine_follows_a_node_with_its_own_law() {
        // a world that moves (± 0.2 a moment), with a day, read by a poor thermometer (± 0.5)
        let mut rng = crate::time::Rng::new(17);
        let (mut x, mut truth, mut rows) = (0.0, vec![], vec![]);
        for k in 0..600 {
            x = 0.95 * x + 0.2 * rng.normal();
            let real = x + 3.0 * (std::f64::consts::TAU * k as f64 / 24.0).sin();
            truth.push(real);
            let measured = if k > 450 && k % 4 == 0 { f64::NAN } else { real + 0.5 * rng.normal() };
            rows.push((k as f64, IndexMap::from([("t".to_string(), measured)])));
        }
        let mut w = World::new();
        w.say_all(&["input t"], &Provenance::of("file:t.csv")).unwrap();
        w.time.unit = "hour".into();
        w.track("t", 1, &[]).unwrap();
        w.feed_rows(&rows[..400]).unwrap();
        let said = w.embody("t").unwrap();
        assert!(said.contains("holds the law of t"), "{said}");
        assert!(w.net().relation("t__law").is_some() && w.net().relation("t__seen").is_some());
        // from now on, what the machine believes the temperature is: its state and its day, its thermometer apart
        let (mut machine_err, mut raw_err, mut n) = (0.0, 0.0, 0.0);
        for k in 400..600 {
            w.feed_rows(&rows[k..k + 1]).unwrap();
            let believed =
                w.machine.value("t__state") + w.time.nodes["t"].hidden.as_ref().unwrap().rhythms.at(k as f64, None);
            if k >= 420 {
                machine_err += (believed - truth[k]).powi(2);
                let m = rows[k].1["t"];
                raw_err += if m.is_finite() { (m - truth[k]).powi(2) } else { 0.25 };
                n += 1.0;
            }
        }
        let (machine_err, raw_err) = ((machine_err / n).sqrt(), (raw_err / n).sqrt());
        assert!(machine_err < 0.75 * raw_err, "the machine {machine_err} / the thermometer {raw_err}");
    }

    #[test]
    fn nodes_believed_together_move_together() {
        // two unknowns whose sum is known well: each is unsure, their sum is not: they move against each other
        let mut w = World::new();
        w.say_all(
            &[
                "node x signed",
                "node z signed",
                "relate total: y = x + z",
                "observe x = 3 ± 1",
                "observe z = 7 ± 1",
                "observe y = 10 ± 0.1",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(10000);
        let j = w.joint(&["x", "z"]).unwrap();
        assert!((j.correlation[0][1] + 100.0 / 101.0).abs() < 1e-4, "{}", j.text());
        assert!((j.sigmas[0] - (101.0f64 / 201.0).sqrt()).abs() < 1e-4, "{}", j.text());
        // a vector: its components, together
        let mut v = World::new();
        v.say_all(
            &["node p[2] signed", "relate same: p_0 = p_1 ± 0.01", "observe p_0 = 1 ± 1"],
            &Provenance::default(),
        )
        .unwrap();
        v.machine.run(10000);
        let j = v.joint(&["p"]).unwrap();
        assert_eq!(j.names, vec!["p_0", "p_1"]);
        assert!(j.correlation[0][1] > 0.99, "{}", j.text());
    }

    #[test]
    fn a_state_carried_together_keeps_how_its_parts_move_together() {
        let mut w = World::new();
        w.say_all(
            &[
                "node a signed",
                "node b signed",
                "relate sum: s = a + b",
                "observe s = 10 ± 0.01",
                "observe a = 4 ± 1",
                "carry a1, b1 = a, b",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.tick(false);
        w.machine.run(10000);
        // a1 and b1 now hold what a and b were believed to be together: their sum is sure, each is not
        let j = w.joint(&["a1", "b1"]).unwrap();
        assert!(j.correlation[0][1] < -0.99, "{}", j.text());
        assert!((w.machine.value("a1") + w.machine.value("b1") - 10.0).abs() < 1e-3);
        // a vector measured together
        let mut v = World::new();
        v.say_all(
            &["node p[2] signed", "observe p = [1, 2] cov [[1, 0.9], [0.9, 1]]", "observe p_0 = 3 ± 0.001"],
            &Provenance::default(),
        )
        .unwrap();
        v.machine.run(10000);
        // measuring one moves the other with it
        assert!((v.machine.value("p_1") - (2.0 + 0.9 * 2.0)).abs() < 0.01, "{}", v.machine.value("p_1"));
        v.say("forget p", &Provenance::default()).unwrap();
        assert!(v.net().priors.is_empty());
    }

    #[test]
    fn the_machine_follows_a_positive_node_too() {
        let mut rng = crate::time::Rng::new(23);
        let (mut x, mut rows) = (0.0, vec![]);
        for k in 0..400 {
            x = 0.9 * x + 0.5 * rng.normal();
            rows.push((k as f64, IndexMap::from([("kelvin".to_string(), 280.0 + x + 0.3 * rng.normal())])));
        }
        let mut w = World::new();
        w.say_all(&["input kelvin = 280 positive"], &Provenance::of("file:k.csv")).unwrap();
        w.track("kelvin", 1, &[]).unwrap();
        w.feed_rows(&rows[..300]).unwrap();
        w.embody("kelvin").unwrap();
        w.feed_rows(&rows[300..]).unwrap();
        let state = w.machine.value("kelvin__state");
        assert!((state - 280.0).abs() < 5.0 && (state - rows[399].1["kelvin"]).abs() < 1.5, "{state}");
    }

    #[test]
    fn a_source_said_again_keeps_what_it_was_living_through() {
        let mut w = World::new();
        w.say("node x = 2", &Provenance::of("typed")).unwrap();
        let app = |extra: &str| -> Vec<String> {
            let mut l: Vec<String> =
                ["var t = 0", "set t = t + 1", "node y", "relate r: y = 3 * x"].iter().map(|s| s.to_string()).collect();
            if !extra.is_empty() {
                l.push(extra.to_string());
            }
            l
        };
        let out = w.rebuild("app:a", &app(""), None, &IndexMap::new()).unwrap();
        assert!(out.iter().all(|o| o.is_ok()));
        for _ in 0..5 {
            w.tick(false);
        }
        assert_eq!(w.machine.net.measured["t"].0, 5.0);
        // edited while it runs: a relation changed, a line that is not understood, a new node
        let mut lines = app("node z");
        lines[3] = "relate r: y = 4 * x".into();
        lines.push("relate broken: = =".into());
        let out = w.rebuild("app:a", &lines, None, &IndexMap::new()).unwrap();
        assert!(out[5].is_err() && out[4].is_ok());
        assert_eq!(w.machine.net.measured["t"].0, 5.0, "it goes on from where it was");
        w.machine.run(1000);
        assert!((w.machine.value_si("y") / w.machine.value_si("x") - 4.0).abs() < 1e-6);
        assert_eq!(w.machine.net.sets.len(), 1, "one set, not two");
        assert!(w.machine.net.nodes.contains_key("z"));
        // what others said stays; what it said goes when it says nothing
        let state = w.state_of("app:a");
        assert_eq!(state["t"], 5.0);
        w.rebuild("app:a", &[], None, &IndexMap::new()).unwrap();
        assert!(w.machine.net.nodes.contains_key("x"));
        assert!(!w.machine.net.nodes.contains_key("y") && !w.machine.net.nodes.contains_key("t"));
        assert!(w.machine.net.relations.is_empty() && w.machine.net.sets.is_empty());
        // opened again where it was
        w.rebuild("app:a", &app(""), None, &state).unwrap();
        assert_eq!(w.machine.net.measured["t"].0, 5.0);
    }
}
