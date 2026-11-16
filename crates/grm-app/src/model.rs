// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What the window shows, taken from the world without ever making the window wait: a question is sent to the
//! engine's thread, the answer is picked up at a later frame (`Pull`). Nothing here knows how it is drawn.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use grm_engine::engine::Engine;
use grm_engine::machine::FKind;
use grm_engine::world::World;

#[derive(Debug, Clone, Default)]
pub struct NodeView {
    pub name: String,
    pub value: f64,
    /// SI for a signed node, dex (a factor of 10^sigma) for a positive one.
    pub sigma: f64,
    pub signed: bool,
    pub source: String,
    pub observed: bool,
    pub fixed: bool,
    pub var: bool,
    pub learn: bool,
    pub input: bool,
    pub output: bool,
    pub tracked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RelationView {
    pub name: String,
    pub text: String,
    pub nodes: Vec<String>,
    pub source: String,
    pub strength: f64,
    /// How far it is from holding, in sigmas (0: it holds).
    pub tension: f64,
    /// Its conditions hold (a `when` not met: it sleeps).
    pub awake: bool,
    pub keep: bool,
    /// What it brings each tick, in bits, and the rent of its links.
    pub bits: f64,
    pub rent: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SandboxView {
    pub name: String,
    pub lines: Vec<String>,
}

/// The world as the window sees it.
#[derive(Debug, Clone, Default)]
pub struct Picture {
    pub nodes: Vec<NodeView>,
    pub relations: Vec<RelationView>,
    pub harmony: f64,
    pub ticks: u64,
    pub settled: bool,
    pub sandboxes: Vec<SandboxView>,
    pub sources: Vec<(String, usize)>,
    pub quarantine: Vec<String>,
    /// (tick, kind, text), the newest last.
    pub journal: Vec<(u64, String, String)>,
    pub integrating: Vec<String>,
    /// What the machine feels.
    pub mood: grm_engine::mind::Mood,
    /// Its workshops: (name, nodes, relations).
    pub workshops: Vec<(String, usize, usize)>,
}

impl Picture {
    /// A sandbox's network, as a picture (its own machine: what the global network would not see).
    /// A workshop, as a picture of its own (its nodes come from it).
    pub fn of_workshop(w: &World, name: &str) -> Option<Picture> {
        let ws = w.workshops.get(name)?;
        let mut p = Picture::of(ws);
        for n in &mut p.nodes {
            n.source = format!("workshop:{name}");
        }
        for r in &mut p.relations {
            r.source = format!("workshop:{name}");
        }
        Some(p)
    }

    pub fn of_sandbox(w: &World, name: &str) -> Option<Picture> {
        let sb = w.sandboxes.get(name)?;
        let machine = match &sb.machine {
            Some(m) => m.clone(),
            None => {
                let mut m = grm_engine::machine::Machine::new(sb.net.clone()).ok()?;
                m.run(2000);
                m
            }
        };
        let mut lone = World::new();
        lone.machine = machine;
        let mut p = Picture::of(&lone);
        for n in &mut p.nodes {
            n.source = format!("sandbox:{name}");
        }
        for r in &mut p.relations {
            r.source = format!("sandbox:{name}");
        }
        Some(p)
    }

    pub fn of(w: &World) -> Picture {
        let m = &w.machine;
        let net = &m.net;
        let nodes = net
            .nodes
            .iter()
            .filter(|(n, _)| !n.contains("__next"))
            .map(|(n, s)| {
                let (value, sigma) = m.reading(n);
                NodeView {
                    name: n.clone(),
                    value,
                    sigma,
                    signed: s.signed,
                    source: s.origin.source.clone(),
                    observed: net.measured.contains_key(n),
                    fixed: s.fixed,
                    var: s.var,
                    learn: s.learn,
                    input: s.input,
                    output: s.output,
                    tracked: w.time.nodes.contains_key(n),
                }
            })
            .collect();
        let mut tension: HashMap<&str, (f64, bool, bool)> = HashMap::new();
        for (k, f) in m.factors.iter().enumerate() {
            let entry = tension.entry(f.name.as_str()).or_insert((0.0, false, false));
            match f.kind {
                FKind::Relation | FKind::Nonlinear => {
                    let awake = f.w > 0.0 && (f.watch.is_empty() || f.active);
                    if awake {
                        entry.0 = entry.0.max(m.residual(k).abs().min(10.0));
                    }
                    entry.1 |= awake;
                }
                FKind::Keep => {
                    // a limit pressing: it holds something back
                    entry.1 |= f.watch.is_empty() || f.active;
                    entry.2 = true;
                }
                _ => {}
            }
        }
        let (_, _, _, accounts) = grm_engine::mind::accounts(m, grm_shell::host_capacity());
        let account: std::collections::HashMap<&str, (f64, f64)> =
            accounts.iter().map(|a| (a.name.as_str(), (a.bits, a.rent))).collect();
        let relations = net
            .relations
            .iter()
            .map(|r| {
                let (t, awake, keep) = tension.get(r.name.as_str()).copied().unwrap_or((0.0, false, false));
                let (bits, rent) = account.get(r.name.as_str()).copied().unwrap_or((0.0, 0.0));
                RelationView {
                    name: r.name.clone(),
                    text: r.text.clone(),
                    nodes: r.nodes(),
                    source: r.origin.source.clone(),
                    strength: r.strength,
                    tension: t,
                    awake,
                    keep,
                    bits,
                    rent,
                }
            })
            .collect();
        let start = w.journal.len().saturating_sub(300);
        Picture {
            nodes,
            relations,
            harmony: m.harmony(),
            ticks: net.ticks,
            settled: m.quiet(),
            sandboxes: w
                .sandboxes
                .values()
                .map(|s| SandboxView { name: s.name.clone(), lines: s.lines.clone() })
                .collect(),
            sources: w.sources().into_iter().collect(),
            mood: w.mood.clone(),
            workshops: w
                .workshops
                .iter()
                .map(|(n, ws)| (n.clone(), ws.machine.net.nodes.len(), ws.machine.net.relations.len()))
                .collect(),
            quarantine: w.quarantine.clone(),
            journal: w.journal[start..].iter().map(|e| (e.tick, e.kind.clone(), e.text.clone())).collect(),
            integrating: w.integrations.iter().map(|i| format!("{} · step {}", i.source, i.step)).collect(),
        }
    }

    pub fn node(&self, name: &str) -> Option<&NodeView> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

/// What a node is in time: its memory, what it will be, why.
#[derive(Debug, Clone, Default)]
pub struct Detail {
    pub node: String,
    /// (start, end, mean, spread) of each span it remembers, the oldest first.
    pub history: Vec<(f64, f64, f64, f64)>,
    /// (t, mean, sigma, low, high) of each step ahead.
    pub forecast: Vec<(f64, f64, f64, f64, f64)>,
    pub why: String,
    pub now: f64,
    pub unit: String,
}

impl Detail {
    pub fn of(w: &World, node: &str) -> Option<Detail> {
        let tracked = w.time.nodes.get(node)?;
        let mut history: Vec<(f64, f64, f64, f64)> =
            tracked.memory.spans().iter().map(|s| (s.start, s.end, s.mean, s.spread())).collect();
        history.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let steps = 24;
        let f = w.time.forecast(steps, 200, 1.0);
        let forecast = f
            .paths
            .get(node)
            .map(|p| {
                (0..steps.min(p.mean.len()))
                    .map(|h| (w.time.t + (h as f64 + 1.0) * w.time.dt, p.mean[h], p.sigma[h], p.low[h], p.high[h]))
                    .collect()
            })
            .unwrap_or_default();
        let why = w.time.why(node, 1, 200).map(|y| y.text(&w.node_sources())).unwrap_or_else(|e| e.message);
        Some(Detail { node: node.to_string(), history, forecast, why, now: w.time.t, unit: w.time.unit.clone() })
    }
}

/// A question sent to the engine, its answer picked up later: the window never waits.
pub struct Pull<T> {
    waiting: Option<Receiver<T>>,
    asked: Option<Instant>,
    pub every: Duration,
}

impl<T: Send + 'static> Pull<T> {
    pub fn new(every: Duration) -> Pull<T> {
        Pull { waiting: None, asked: None, every }
    }

    /// Asks again soon (whatever `every` says).
    pub fn soon(&mut self) {
        self.asked = None;
    }

    /// The answer, if one came; asks again when it is time.
    pub fn poll(&mut self, engine: &Engine, ask: impl FnOnce(&mut World) -> T + Send + 'static) -> Option<T> {
        let mut got = None;
        if let Some(rx) = &self.waiting {
            match rx.try_recv() {
                Ok(v) => {
                    got = Some(v);
                    self.waiting = None;
                }
                Err(TryRecvError::Disconnected) => self.waiting = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if self.waiting.is_none() && self.asked.is_none_or(|t| t.elapsed() >= self.every) {
            let (tx, rx) = channel();
            engine.send(move |w| {
                let _ = tx.send(ask(w));
                Ok(String::new())
            });
            self.waiting = Some(rx);
            self.asked = Some(Instant::now());
        }
        got
    }
}
