// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Clocks: the machine lives at several speeds, as a brain does.
//!
//! - **Reflection** (the machine): belief propagation settles the network, learns, weighs what it is told. It
//!   takes rounds; its clock is the frame or the tick.
//! - **Reflex** (this file): what must happen at the speed of sound (48 000 times a second) cannot wait for rounds.
//!   A reflex runs a network's `set`s directly, each written from the values before (as a tick writes them), with
//!   the rest of the network (its parameters) as the machine last settled it. A guitar string, an oscillator, a
//!   filter of a DAW live here; the machine above tunes them and learns.

use indexmap::IndexMap;

use crate::bail;
use crate::error::Result;
use crate::machine::Machine;
use crate::network::{Cond, Kind, Relation};

/// How fast a region of the network lives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clock {
    /// Sound: so many samples a second (a reflex).
    Audio { rate: u32 },
    /// Pictures: so many frames a second (the machine, a frame's budget at a time).
    Frame { hz: f64 },
    /// When something happens (a click, a row of a file).
    Event,
}

#[derive(Debug, Clone)]
enum Expr {
    /// The value of another node.
    Copy(usize),
    /// -(sum of the other terms) / coefficient of the node: a sum solved for it (kept as -1 / coefficient); when
    /// the node is a factor of its term (`glow * gd - tot`), divided by the other factors too.
    Sum {
        terms: Vec<(f64, Vec<(usize, f64)>)>,
        over: f64,
        by: Vec<(usize, f64)>,
    },
    /// (10^rhs / product of the others)^(1/power): a product solved for it (10^rhs and 1/power kept computed).
    Product {
        rhs: f64,
        others: Vec<(usize, f64)>,
        power: f64,
    },
    /// ln(y) or e^L.
    Ln(usize),
    Exp(usize),
}

/// x^p, the usual powers without the general (and slow) one.
#[inline]
fn pow(x: f64, p: f64) -> f64 {
    if p == 1.0 {
        x
    } else if p == -1.0 {
        1.0 / x
    } else if p == 2.0 {
        x * x
    } else if p == -2.0 {
        1.0 / (x * x)
    } else if p == 0.5 {
        x.sqrt()
    } else if p == 3.0 {
        x * x * x
    } else {
        x.powf(p)
    }
}

/// A sound file played: its samples (48 000 a second, one channel), where it is in them.
#[derive(Debug, Clone)]
struct Sounding {
    target: usize,
    run: Option<usize>,
    data: std::sync::Arc<Vec<f32>>,
    at: usize,
}

/// A line that remembers: its node is what its source was so many samples ago.
#[derive(Debug, Clone)]
struct Delay {
    target: usize,
    source: usize,
    line: Vec<f64>,
    at: usize,
}

#[derive(Debug, Clone)]
struct Step {
    target: usize,
    expr: Expr,
    when: Vec<(usize, f64, f64)>,
    /// Where it was said (its order among all the sets, or all that follows: kept when they are sorted out).
    ix: usize,
}

impl Expr {
    /// The nodes it reads.
    fn reads(&self, out: &mut Vec<usize>) {
        match self {
            Expr::Copy(i) | Expr::Ln(i) | Expr::Exp(i) => out.push(*i),
            Expr::Sum { terms, by, .. } => {
                out.extend(terms.iter().flat_map(|(_, t)| t.iter().map(|(i, _)| *i)));
                out.extend(by.iter().map(|(i, _)| *i));
            }
            Expr::Product { others, .. } => out.extend(others.iter().map(|(i, _)| *i)),
        }
    }
}

/// A value so small that nothing physical is that small (far below Planck's time, 5e-44 s) is nothing: what a
/// filter fading out in silence comes to, after a while. Left as it is, it goes on down into the subnormal numbers
/// the processor no longer computes at its speed (and the square of one beyond 1e-154 is one): a hundred of them
/// made the sound slower and slower with nothing playing, until it broke.
const NOTHING: f64 = 1e-150;

/// The rail of a node nobody gave one: far beyond anything a sound or a count reaches (it only keeps the
/// infinite out).
pub const WIDE_RAIL: f64 = 1e12;

/// A value written to a node of this rail: what is not a number leaves the node as it was (the last sane value);
/// within half its rail it is as it is; beyond, it bends towards the rail (a soft knee, as a circuit saturates)
/// and never passes it.
#[inline]
fn guard(v: f64, old: f64, rail: f64) -> f64 {
    if !v.is_finite() {
        return if old.is_finite() { old } else { 0.0 };
    }
    let a = v.abs();
    if a < NOTHING {
        0.0
    } else if a <= 0.5 * rail {
        v
    } else {
        let h = 0.5 * rail;
        v.signum() * (h + h * ((a - h) / h).tanh())
    }
}

/// A network's `set`s (and the nodes that follow from them), run directly at the speed of a clock.
#[derive(Debug, Clone)]
pub struct Reflex {
    pub clock: Clock,
    pub names: IndexMap<String, usize>,
    pub values: Vec<f64>,
    sets: Vec<Step>,
    follow: Vec<Step>,
    delays: Vec<Delay>,
    /// The sound files it plays (a node each, its samples one by one while its switch is on).
    sounds: Vec<Sounding>,
    /// What it keeps when it starts again from nothing (its `memory` nodes: what it learned).
    kept: Vec<bool>,
    /// Where a sample's writes wait (kept between samples: nothing is allocated while it sounds).
    scratch: Vec<(usize, f64)>,
    /// For each set, the next one whose first condition is another (the sets said by one `repeat` often share it:
    /// when it does not hold, all of them are passed at once).
    skip: Vec<usize>,
    /// For each node, whether it writes it itself (a set's, a follower's or a delay's target), and its parameters
    /// (the others, by name): known once, not searched again each time the machine above tunes it.
    writes: Vec<bool>,
    params: std::cell::OnceCell<Vec<String>>,
    /// What only changes when a parameter does (a filter's coefficients from its knob, a step from a pitch): not
    /// computed at every sample, but when a parameter moved (and every so often all the same). Its sets and what
    /// follows, in their order.
    slow_sets: Vec<Step>,
    slow_follow: Vec<Step>,
    /// Sorted out (what is slow, what is not) for the steps as they are now.
    planned: bool,
    /// Its inputs written at every sample from outside (what the room hears): never slow.
    audio_in: Vec<bool>,
    /// Each node's rail (what it can never go beyond), and whether it is signed (a sound, not a pitch).
    rail: Vec<f64>,
    signed: Vec<bool>,
    /// A parameter moved since the slow part was last computed; samples since then.
    dirty: bool,
    since: u32,
    pub samples: u64,
}

/// The slow part is computed again at least this often (samples), whatever happened.
const SETTLE_EVERY: u32 = 64;

impl Reflex {
    /// The reflex of a machine: its `set`s, its nodes as it last settled them (in SI), its `when`s.
    pub fn of(machine: &Machine, clock: Clock) -> Result<Reflex> {
        let net = &machine.net;
        let names: IndexMap<String, usize> = net.nodes.keys().enumerate().map(|(k, n)| (n.clone(), k)).collect();
        let values: Vec<f64> = net
            .nodes
            .keys()
            .map(|n| {
                // a set's target is what was last written to it; an input or a variable, what was said of it (exactly: not as
                // the machine has settled it so far, a hair away, which would move a tempo by a few samples)
                net.measured
                    .get(n)
                    .filter(|_| {
                        net.nodes.get(n).is_some_and(|s| s.input || s.var) || net.sets.iter().any(|s| s.target == *n)
                    })
                    .map(|m| m.0)
                    .unwrap_or_else(|| machine.value_si(n))
            })
            // a value that ran away (not a number, or beyond any sound) starts again from nothing
            .map(|v| if v.is_finite() && v.abs() < 1e6 { v } else { 0.0 })
            .collect();
        let kept: Vec<bool> = net.nodes.values().map(|n| n.memory).collect();
        let mut r = Reflex {
            clock,
            names,
            values,
            sets: vec![],
            follow: vec![],
            delays: vec![],
            sounds: vec![],
            kept,
            scratch: vec![],
            skip: vec![],
            writes: vec![],
            params: std::cell::OnceCell::new(),
            slow_sets: vec![],
            slow_follow: vec![],
            planned: false,
            audio_in: vec![],
            rail: net.nodes.values().map(|n| n.rail.unwrap_or(WIDE_RAIL)).collect(),
            signed: net.nodes.values().map(|n| n.signed).collect(),
            dirty: true,
            since: 0,
            samples: 0,
        };
        for st in &net.sets {
            // a sound file: its samples, one each sample while its switch is on (from where it was left)
            if let Some(rest) = st.text.strip_prefix("audio ") {
                let path = rest.split_once(" from ").map(|(_, p)| p).unwrap_or("");
                let path = path.rsplit_once(" when ").map(|(p, _)| p).unwrap_or(path).trim().trim_matches('"');
                let data = crate::wav::load(std::path::Path::new(path)).unwrap_or_default();
                let target = r.slot(&st.target)?;
                let run = if st.source.is_empty() { None } else { Some(r.slot(&st.source)?) };
                r.sounds.push(Sounding { target, run, data, at: 0 });
                continue;
            }
            // a delay: a line of so many samples (one at a slower clock)
            if let Some(by) = st.text.starts_with("delay ").then(|| st.text.rsplit_once(" by ")).flatten() {
                let seconds = crate::lang::seconds_of(by.1).unwrap_or(0.0);
                let n = match clock {
                    Clock::Audio { rate } => ((seconds * rate as f64).round() as usize).max(1),
                    _ => 1,
                };
                let (target, source) = (r.slot(&st.target)?, r.slot(&st.source)?);
                r.delays.push(Delay { target, source, line: vec![0.0; n], at: 0 });
                continue;
            }
            let expr = if let Some((_, k)) = st.source.rsplit_once("__next") {
                let rel = net.relation(&format!("set_{}_{}", st.target, k));
                let Some(rel) = rel else { bail!("the relation of set {} is missing", st.target) };
                r.solve(rel, &st.source)?
            } else {
                Expr::Copy(r.slot(&st.source)?)
            };
            let when = r.conds(&st.when)?;
            let ix = r.sets.len();
            r.sets.push(Step { target: r.slot(&st.target)?, expr, when, ix });
        }
        r.skip = vec![0; r.sets.len()];
        for i in (0..r.sets.len()).rev() {
            let first = r.sets[i].when.first();
            r.skip[i] = if first.is_some() && i + 1 < r.sets.len() && r.sets[i + 1].when.first() == first {
                r.skip[i + 1]
            } else {
                i + 1
            };
        }
        r.mark();
        Ok(r)
    }

    /// Which nodes it writes, and so which are its parameters.
    fn mark(&mut self) {
        let mut writes = vec![false; self.values.len()];
        let targets = self.sets.iter().chain(&self.slow_sets).chain(&self.follow).chain(&self.slow_follow);
        let targets = targets.map(|s| s.target).chain(self.sounds.iter().map(|f| f.target));
        for t in targets.chain(self.delays.iter().map(|d| d.target)) {
            writes[t] = true;
        }
        self.writes = writes;
        self.params = std::cell::OnceCell::new();
    }

    /// One more node it writes (a follower).
    fn writes_too(&mut self, target: usize) {
        self.writes[target] = true;
        self.params = std::cell::OnceCell::new();
    }

    /// A node written at every sample from outside (what the room hears): what reads it is never slow.
    pub fn audio_input(&mut self, slot: usize) {
        if self.audio_in.len() < self.values.len() {
            self.audio_in.resize(self.values.len(), false);
        }
        if !self.audio_in[slot] {
            self.audio_in[slot] = true;
            self.planned = false;
        }
    }

    /// Something was written into it from outside: its slow part is computed again before the next sample.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// What is slow, sorted out from what is not: a node is slow when every step that writes it reads only slow
    /// nodes (the parameters, then what is made of them alone) and not itself; the rest goes at every sample.
    fn plan(&mut self) {
        // all of them back in their order, then sorted out again
        let mut sets: Vec<Step> = std::mem::take(&mut self.sets);
        sets.append(&mut self.slow_sets);
        sets.sort_by_key(|s| s.ix);
        let mut follow: Vec<Step> = std::mem::take(&mut self.follow);
        follow.append(&mut self.slow_follow);
        follow.sort_by_key(|s| s.ix);
        let n = self.values.len();
        let fast_in = |i: usize| self.audio_in.get(i).copied().unwrap_or(false);
        let mut slow: Vec<bool> = (0..n).map(|i| !self.writes.get(i).copied().unwrap_or(true) && !fast_in(i)).collect();
        // what each target is written from
        let mut by_target: std::collections::HashMap<usize, Vec<Vec<usize>>> = std::collections::HashMap::new();
        for st in sets.iter().chain(&follow) {
            let mut r = vec![];
            st.expr.reads(&mut r);
            r.extend(st.when.iter().map(|c| c.0));
            by_target.entry(st.target).or_default().push(r);
        }
        for d in &self.delays {
            by_target.remove(&d.target);
        }
        for f in &self.sounds {
            by_target.remove(&f.target);
        }
        loop {
            let mut grew = false;
            for (&t, ways) in &by_target {
                if !slow[t] && ways.iter().all(|r| r.iter().all(|&i| i != t && slow[i])) {
                    slow[t] = true;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        let (slow_sets, sets): (Vec<Step>, Vec<Step>) = sets.into_iter().partition(|s| slow[s.target]);
        let (slow_follow, follow): (Vec<Step>, Vec<Step>) = follow.into_iter().partition(|s| slow[s.target]);
        self.sets = sets;
        self.follow = follow;
        self.slow_sets = slow_sets;
        self.slow_follow = slow_follow;
        self.skip = vec![0; self.sets.len()];
        for i in (0..self.sets.len()).rev() {
            let first = self.sets[i].when.first();
            self.skip[i] = if first.is_some() && i + 1 < self.sets.len() && self.sets[i + 1].when.first() == first {
                self.skip[i + 1]
            } else {
                i + 1
            };
        }
        self.planned = true;
        self.dirty = true;
    }

    /// The slow part computed again (its sets at once, then what follows), until it holds (a few passes: what is
    /// made of what is made of a parameter).
    fn settle(&mut self) {
        for _ in 0..8 {
            let mut changed = false;
            let mut writes = std::mem::take(&mut self.scratch);
            writes.clear();
            for st in &self.slow_sets {
                if self.holds(&st.when) {
                    writes.push((st.target, self.eval(&st.expr)));
                }
            }
            for &(i, v) in &writes {
                let v = guard(v, self.values[i], self.rail[i]);
                if self.values[i] != v {
                    self.values[i] = v;
                    changed = true;
                }
            }
            self.scratch = writes;
            for k in 0..self.slow_follow.len() {
                if self.holds(&self.slow_follow[k].when) {
                    let t = self.slow_follow[k].target;
                    let v = guard(self.eval(&self.slow_follow[k].expr), self.values[t], self.rail[t]);
                    if self.values[t] != v {
                        self.values[t] = v;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        self.dirty = false;
        self.since = 0;
    }

    /// The node that went the farthest of what it writes (a signed one: a sound, not a pitch), and its value:
    /// what ran away, most likely, when the sound did.
    pub fn wildest(&self) -> Option<(String, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, v) in self.values.iter().enumerate() {
            if !self.writes.get(i).copied().unwrap_or(false) || !self.signed.get(i).copied().unwrap_or(false) {
                continue;
            }
            let a = if v.is_finite() { v.abs() } else { f64::INFINITY };
            if best.is_none_or(|(_, b)| a > b) {
                best = Some((i, a));
            }
        }
        best.and_then(|(i, _)| self.names.get_index(i).map(|(n, _)| (n.clone(), self.values[i])))
    }

    /// How much of it is slow: (sets, what follows) computed only when a parameter moves.
    pub fn slow_sizes(&self) -> (usize, usize) {
        (self.slow_sets.len(), self.slow_follow.len())
    }

    /// Whether this node is one of its parameters (it reads it, the machine above tunes it).
    pub fn is_param(&self, i: usize) -> bool {
        !self.writes.get(i).copied().unwrap_or(true)
    }

    fn slot(&self, n: &str) -> Result<usize> {
        match self.names.get(n) {
            Some(&i) => Ok(i),
            None => bail!("no node {n}"),
        }
    }

    fn conds(&self, when: &[Cond]) -> Result<Vec<(usize, f64, f64)>> {
        when.iter().map(|c| Ok((self.slot(&c.node)?, c.lo, c.hi))).collect()
    }

    /// A relation solved for one of its nodes.
    fn solve(&self, rel: &Relation, node: &str) -> Result<Expr> {
        match &rel.kind {
            Kind::Sum { terms } => {
                let mut over = None;
                let mut by = vec![];
                let mut rest = vec![];
                for (c, t) in terms {
                    if t.get(node).is_some_and(|p| *p == 1.0) && over.is_none() {
                        over = Some(*c);
                        by = t
                            .iter()
                            .filter(|(n, _)| *n != node)
                            .map(|(n, p)| Ok((self.slot(n)?, *p)))
                            .collect::<Result<Vec<_>>>()?;
                    } else if t.contains_key(node) {
                        bail!("{}: {node} is not alone in its term (a reflex solves straight relations)", rel.name);
                    } else {
                        let parts = t.iter().map(|(n, p)| Ok((self.slot(n)?, *p))).collect::<Result<Vec<_>>>()?;
                        rest.push((*c, parts));
                    }
                }
                let Some(over) = over else { bail!("{}: {node} is not in it", rel.name) };
                Ok(Expr::Sum { terms: rest, over: -1.0 / over, by })
            }
            Kind::Product { terms, rhs } => {
                let Some(&power) = terms.get(node) else { bail!("{}: {node} is not in it", rel.name) };
                let others = terms
                    .iter()
                    .filter(|(n, _)| *n != node)
                    .map(|(n, p)| Ok((self.slot(n)?, *p)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Product { rhs: 10f64.powf(*rhs), others, power: 1.0 / power })
            }
            Kind::Log { log, node: y } if log == node => Ok(Expr::Ln(self.slot(y)?)),
            Kind::Log { log, node: y } if y == node => Ok(Expr::Exp(self.slot(log)?)),
            _ => bail!("{}: a reflex does not run this kind of relation", rel.name),
        }
    }

    /// A node that follows the `set`s at every sample, computed by this relation.
    pub fn follow_by(&mut self, rel: &Relation, node: &str) -> Result<()> {
        let expr = self.solve(rel, node)?;
        let target = self.slot(node)?;
        let when = self.conds(&rel.when)?;
        let ix = self.follow.len() + self.slow_follow.len();
        self.follow.push(Step { target, expr, when, ix });
        self.planned = false;
        self.writes_too(target);
        Ok(())
    }

    /// A node that follows the `set`s at every sample, computed by one of its relations (an output, a mix).
    pub fn follow(&mut self, machine: &Machine, node: &str) -> Result<()> {
        // (not a set's own relation: it says what a node's next value is, it does not give the nodes it reads; first
        // one where the node stands alone, else one where it is a factor of its term)
        let own = |r: &&Relation| !r.nodes().iter().any(|n| n.contains("__next"));
        let alone = |r: &&Relation| {
            matches!(&r.kind, Kind::Sum { terms } if terms.iter().any(|(_, t)| t.len() == 1 && t.get(node) == Some(&1.0)))
                || matches!(&r.kind, Kind::Product { terms, .. } if terms.contains_key(node))
        };
        let factor = |r: &&Relation| {
            matches!(&r.kind, Kind::Sum { terms }
                if terms.iter().filter(|(_, t)| t.contains_key(node)).count() == 1
                    && terms.iter().any(|(_, t)| t.get(node) == Some(&1.0)))
        };
        let rels = &machine.net.relations;
        let rel = rels.iter().filter(own).find(alone).or_else(|| rels.iter().filter(own).find(factor));
        let Some(rel) = rel else { bail!("no relation gives {node}") };
        let expr = self.solve(rel, node)?;
        let target = self.slot(node)?;
        let when = self.conds(&rel.when)?;
        let ix = self.follow.len() + self.slow_follow.len();
        self.follow.push(Step { target, expr, when, ix });
        self.planned = false;
        self.writes_too(target);
        Ok(())
    }

    /// Takes the values of the parameters the machine has settled since (the reflex keeps its own `set`s).
    pub fn tune(&mut self, machine: &Machine) {
        for (n, &i) in &self.names {
            if !self.writes[i] && (machine.index.contains_key(n) || machine.fixed.contains_key(n)) {
                self.values[i] = machine.value_si(n);
            }
        }
        self.dirty = true;
    }

    /// Its parameters: the nodes it reads but does not write (what the machine above tunes).
    pub fn params(&self) -> &[String] {
        self.params
            .get_or_init(|| self.names.iter().filter(|(_, &i)| !self.writes[i]).map(|(n, _)| n.clone()).collect())
    }

    /// Takes these values (in SI) for its parameters; what it writes itself is left alone.
    pub fn tune_values(&mut self, values: &IndexMap<String, f64>) {
        for (n, v) in values {
            if let Some(&i) = self.names.get(n) {
                // what is only the machine settling a hair further is not a change (the same sound twice)
                let old = self.values[i];
                if !self.writes[i] && v.is_finite() && (v - old).abs() > 1e-7 * old.abs().max(1e-9) {
                    self.values[i] = *v;
                    self.dirty = true;
                }
            }
        }
    }

    /// How big it is: its sets, the nodes that follow, its delays, its nodes.
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.sets.len() + self.slow_sets.len(),
            self.follow.len() + self.slow_follow.len(),
            self.delays.len(),
            self.values.len(),
        )
    }

    /// Where a node is kept (to read it at each sample without looking its name up).
    pub fn slot_of(&self, node: &str) -> Option<usize> {
        self.names.get(node).copied()
    }

    /// One sample, and the value at `slot` after it.
    pub fn step_read(&mut self, slot: usize) -> f64 {
        self.step();
        self.values[slot]
    }

    fn eval(&self, e: &Expr) -> f64 {
        let v = &self.values;
        match e {
            Expr::Copy(i) => v[*i],
            Expr::Sum { terms, over, by } => {
                let mut s = 0.0;
                for (c, parts) in terms {
                    let mut t = *c;
                    for &(i, p) in parts {
                        t *= pow(v[i], p);
                    }
                    s += t;
                }
                let mut d = 1.0;
                for &(i, p) in by {
                    d *= pow(v[i], p);
                }
                s * over / d
            }
            Expr::Product { rhs, others, power } => {
                let mut prod = 1.0;
                for &(i, p) in others {
                    prod *= pow(v[i], p);
                }
                pow(rhs / prod, *power)
            }
            Expr::Ln(i) => v[*i].ln(),
            Expr::Exp(i) => v[*i].exp(),
        }
    }

    #[inline]
    fn holds(&self, when: &[(usize, f64, f64)]) -> bool {
        when.iter().all(|&(i, lo, hi)| lo <= self.values[i] && self.values[i] <= hi)
    }

    /// One sample: every `set` whose conditions hold writes its node, all at once from the values before; then the
    /// nodes that follow.
    pub fn step(&mut self) {
        self.step_with(&[]);
    }

    /// One sample, `mid` written after its sets and before what follows (what another part computed at this
    /// sample: its sets read it as it was the sample before, what follows reads it as it is now).
    fn step_with(&mut self, mid: &[(usize, f64)]) {
        if !self.planned {
            self.plan();
        }
        // what only changes with a parameter: when one moved (or every so often)
        self.since += 1;
        if self.dirty || self.since >= SETTLE_EVERY {
            self.settle();
        }
        // what each line remembers comes out first (the sets and what follows may read it)
        for d in &self.delays {
            self.values[d.target] = d.line[d.at];
        }
        for f in &mut self.sounds {
            let on = f.run.is_none_or(|r| self.values[r] > 0.5);
            self.values[f.target] = if on && !f.data.is_empty() {
                let v = f.data[f.at] as f64;
                f.at = (f.at + 1) % f.data.len();
                v
            } else {
                0.0
            };
        }
        let mut writes = std::mem::take(&mut self.scratch);
        writes.clear();
        let mut i = 0;
        let n = self.sets.len();
        while i < n {
            let s = &self.sets[i];
            if let Some(&(c, lo, hi)) = s.when.first() {
                let v = self.values[c];
                if !(lo <= v && v <= hi) {
                    // its first condition does not hold: nor for the sets after it that share it
                    i = self.skip[i];
                    continue;
                }
            }
            if self.holds(&s.when[1.min(s.when.len())..]) {
                writes.push((s.target, self.eval(&s.expr)));
            }
            i += 1;
        }
        for &(i, v) in &writes {
            self.values[i] = guard(v, self.values[i], self.rail[i]);
        }
        self.scratch = writes;
        for &(i, v) in mid {
            self.values[i] = v;
        }
        for k in 0..self.follow.len() {
            if self.holds(&self.follow[k].when) {
                let v = self.eval(&self.follow[k].expr);
                let t = self.follow[k].target;
                self.values[t] = guard(v, self.values[t], self.rail[t]);
            }
        }
        // and each line takes what its source is now
        for d in &mut self.delays {
            d.line[d.at] = self.values[d.source];
            d.at = (d.at + 1) % d.line.len();
        }
        self.samples += 1;
    }

    /// What follows from the sets computed once from where they start (before the first sample reads it: not left
    /// where the machine had settled it so far, which differs a hair from one moment to the next).
    pub fn prime(&mut self) {
        for k in 0..self.follow.len() {
            if self.holds(&self.follow[k].when) {
                let v = self.eval(&self.follow[k].expr);
                self.values[self.follow[k].target] = v;
            }
        }
        for k in 0..self.slow_follow.len() {
            if self.holds(&self.slow_follow[k].when) {
                let v = self.eval(&self.slow_follow[k].expr);
                self.values[self.slow_follow[k].target] = v;
            }
        }
        self.dirty = true;
    }

    /// Its memories (the `memory` nodes) as they are now: what it learned while it ran.
    pub fn memories(&self) -> IndexMap<String, f64> {
        self.names.iter().filter(|(_, &i)| self.kept[i]).map(|(n, &i)| (n.clone(), self.values[i])).collect()
    }

    /// Everything it writes itself starts again from nothing (what its sets write, what its delays hold, what
    /// follows from them): when it ran away, or when asked. Its parameters stay as they are.
    pub fn reset(&mut self) {
        for s in self.sets.iter().chain(&self.slow_sets) {
            if !self.kept[s.target] {
                self.values[s.target] = 0.0;
            }
        }
        for d in &mut self.delays {
            d.line.iter_mut().for_each(|x| *x = 0.0);
            self.values[d.target] = 0.0;
        }
        for f in self.follow.iter().chain(&self.slow_follow) {
            self.values[f.target] = 0.0;
        }
        self.dirty = true;
    }

    /// `n` samples of a node (a block of sound).
    pub fn block(&mut self, node: &str, n: usize) -> Result<Vec<f32>> {
        let i = self.slot(node)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            self.step();
            out.push(self.values[i] as f32);
        }
        Ok(out)
    }

    pub fn value(&self, node: &str) -> Option<f64> {
        self.names.get(node).map(|&i| self.values[i])
    }
}

/// Who computes a node when a reflex is shared among several threads.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Owner {
    /// Nobody (a parameter), or every part alike (a clock: each part has its own, the same).
    All,
    Part(usize),
    /// What mixes the parts: after them, sample after sample.
    Tail,
}

/// A reflex shared among the processor's cores, without changing what it computes at any sample. Its parts are
/// groups of nodes (an instrument, her) that read nothing of one another, or only of a part that reads nothing:
/// her first (the sources), the instruments that follow her hand next (the followers), what mixes them last. A
/// block of the sources is computed while the followers compute the block before (each sample reading her at that
/// sample, as kept): so they go side by side, and the sound comes one block later (a few milliseconds).
#[derive(Debug, Clone)]
pub struct Parts {
    pub parts: Vec<Reflex>,
    pub tail: Reflex,
    owner: Vec<Owner>,
    /// Which parts are sources (they read no other part).
    source: Vec<bool>,
    /// The parts each thread runs (one after the other).
    threads: Vec<Vec<usize>>,
    /// For each follower, what it reads of the sources (node, source part); the tail's (node, part).
    imports: Vec<Vec<(usize, usize)>>,
    tail_imports: Vec<(usize, usize)>,
    /// For each part, the nodes kept at every sample, where each is among them; what was kept (a source's last
    /// block, and the one before).
    record: Vec<Vec<usize>>,
    record_at: Vec<std::collections::HashMap<usize, usize>>,
    kept: Vec<Vec<f64>>,
    kept_before: Vec<Vec<f64>>,
    tail_record: Vec<usize>,
    tail_at: std::collections::HashMap<usize, usize>,
    tail_kept: Vec<f64>,
    /// The block before: what came from outside at each of its samples.
    inputs_before: Vec<Vec<(usize, f64)>>,
}

impl Reflex {
    /// Where a node's name says it belongs: its first word (`t1` of `t1_v2_cut`, `elle` of `elle_pitch`).
    fn group_of(name: &str) -> &str {
        name.split('_').next().unwrap_or(name)
    }

    /// What each of its steps costs while it runs `samples` samples: for each target node, how many conditions
    /// were looked at and how many steps computed.
    fn census(&mut self, samples: usize) -> Vec<(usize, usize)> {
        if !self.planned {
            self.plan();
        }
        let mut cost: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for _ in 0..samples {
            let mut i = 0;
            let n = self.sets.len();
            while i < n {
                let st = &self.sets[i];
                *cost.entry(st.target).or_default() += 1;
                if let Some(&(c, lo, hi)) = st.when.first() {
                    let v = self.values[c];
                    if !(lo <= v && v <= hi) {
                        i = self.skip[i];
                        continue;
                    }
                }
                if self.holds(&st.when[1.min(st.when.len())..]) {
                    *cost.entry(st.target).or_default() += 3;
                }
                i += 1;
            }
            for f in &self.follow {
                *cost.entry(f.target).or_default() += 3 + f.when.len();
            }
            self.step();
        }
        let mut v: Vec<(usize, usize)> = cost.into_iter().collect();
        v.sort();
        v
    }

    /// This reflex shared among `n` cores, when it has parts that can go side by side and are heavy enough for it
    /// to pay; `taps`: the nodes the channels listen to (kept at every sample).
    pub fn parts(&self, n: usize, taps: &[usize]) -> Option<Parts> {
        use std::collections::{BTreeSet, HashMap, HashSet};
        if n < 2 {
            return None;
        }
        let mut base = self.clone();
        if !base.planned {
            base.plan();
        }
        let names: Vec<&str> = base.names.keys().map(|s| s.as_str()).collect();
        let group: Vec<&str> = names.iter().map(|n| Self::group_of(n)).collect();
        let written: Vec<bool> = (0..base.values.len()).map(|i| base.writes.get(i).copied().unwrap_or(false)).collect();
        // each group: what it costs (counted while it runs a little), what it reads of the other groups
        let mut cost: IndexMap<&str, usize> = IndexMap::new();
        for (k, c) in base.clone().census(512) {
            *cost.entry(group[k]).or_default() += c;
        }
        let mut reads: IndexMap<&str, BTreeSet<&str>> = IndexMap::new();
        let steps = base.sets.iter().chain(&base.slow_sets).chain(&base.follow).chain(&base.slow_follow);
        for st in steps {
            let g = group[st.target];
            cost.entry(g).or_default();
            let mut r = vec![];
            st.expr.reads(&mut r);
            r.extend(st.when.iter().map(|c| c.0));
            let e = reads.entry(g).or_default();
            e.extend(r.into_iter().filter(|&i| written[i] && group[i] != g).map(|i| group[i]));
        }
        for d in &base.delays {
            *cost.entry(group[d.target]).or_default() += 512;
            let e = reads.entry(group[d.target]).or_default();
            if group[d.source] != group[d.target] && written[d.source] {
                e.insert(group[d.source]);
            }
        }
        for f in &base.sounds {
            *cost.entry(group[f.target]).or_default() += 512;
            reads.entry(group[f.target]).or_default();
        }
        // too little to be worth sharing out (a few hundred steps a sample)
        if cost.values().sum::<usize>() < 300 * 512 {
            return None;
        }
        // a clock (it reads only itself and parameters, it plays no file, it is light): each part has its own
        let has_sound: HashSet<&str> = base.sounds.iter().map(|f| group[f.target]).collect();
        let replicated: HashSet<&str> = reads
            .iter()
            .filter(|(g, r)| r.is_empty() && !has_sound.contains(**g) && cost[**g] < 64 * 512)
            .map(|(g, _)| *g)
            .collect();
        let reads_of = |g: &str| -> Vec<&str> {
            reads.get(g).map(|r| r.iter().copied().filter(|h| !replicated.contains(h)).collect()).unwrap_or_default()
        };
        // what reads several groups mixes them: the tail; and what reads the tail is the tail too
        let mut tail: HashSet<&str> =
            cost.keys().filter(|g| !replicated.contains(**g) && reads_of(g).len() >= 2).copied().collect();
        loop {
            let more: Vec<&str> = cost
                .keys()
                .filter(|g| !tail.contains(**g) && !replicated.contains(**g))
                .filter(|g| reads_of(g).iter().any(|h| tail.contains(h)))
                .copied()
                .collect();
            if more.is_empty() {
                break;
            }
            tail.extend(more);
        }
        // the groups left, and those that read one another (even through others) made one
        let left: Vec<&str> =
            cost.keys().filter(|g| !tail.contains(**g) && !replicated.contains(**g)).copied().collect();
        if left.len() < 2 {
            return None;
        }
        let at: HashMap<&str, usize> = left.iter().enumerate().map(|(k, g)| (*g, k)).collect();
        let m = left.len();
        let mut reach = vec![vec![false; m]; m];
        for (a, g) in left.iter().enumerate() {
            for h in reads_of(g) {
                if let Some(&b) = at.get(h) {
                    reach[a][b] = true;
                }
            }
        }
        for k in 0..m {
            for a in 0..m {
                if reach[a][k] {
                    for b in 0..m {
                        if reach[k][b] {
                            reach[a][b] = true;
                        }
                    }
                }
            }
        }
        let unit: Vec<usize> =
            (0..m).map(|a| (0..m).find(|&b| b == a || (reach[a][b] && reach[b][a])).unwrap_or(a)).collect();
        let mut units: Vec<usize> = unit.clone();
        units.sort();
        units.dedup();
        if units.len() < 2 {
            return None;
        }
        let part_of_group: HashMap<&str, usize> =
            left.iter().enumerate().map(|(a, g)| (*g, units.iter().position(|u| *u == unit[a]).unwrap_or(0))).collect();
        let np = units.len();
        // a source reads no other part; a follower reads only sources; anything else: not shared out
        let mut reads_part: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); np];
        for g in &left {
            let p = part_of_group[g];
            for h in reads_of(g) {
                if let Some(&q) = part_of_group.get(h) {
                    if q != p {
                        reads_part[p].insert(q);
                    }
                }
            }
        }
        let source: Vec<bool> = reads_part.iter().map(|r| r.is_empty()).collect();
        if reads_part.iter().any(|r| r.iter().any(|q| !source[*q])) {
            return None;
        }
        let owner: Vec<Owner> = (0..base.values.len())
            .map(|i| {
                let g = group[i];
                if !written[i] || replicated.contains(g) {
                    Owner::All
                } else if let Some(&p) = part_of_group.get(g) {
                    Owner::Part(p)
                } else {
                    Owner::Tail
                }
            })
            .collect();
        // each part: its own steps (and the clocks); the rest it only reads
        let keep = |o: Owner, mine: Owner| o == Owner::All || o == mine;
        let make = |mine: Owner| -> Reflex {
            let mut r = base.clone();
            r.sets.retain(|s| keep(owner[s.target], mine));
            r.slow_sets.retain(|s| keep(owner[s.target], mine));
            r.follow.retain(|s| keep(owner[s.target], mine));
            r.slow_follow.retain(|s| keep(owner[s.target], mine));
            r.delays.retain(|d| keep(owner[d.target], mine));
            r.sounds.retain(|f| keep(owner[f.target], mine));
            r.mark();
            r.planned = false;
            r
        };
        let mut parts: Vec<Reflex> = (0..np).map(|p| make(Owner::Part(p))).collect();
        let mut tail_r = make(Owner::Tail);
        let read_nodes = |r: &Reflex| -> BTreeSet<usize> {
            let mut all = vec![];
            for st in r.sets.iter().chain(&r.slow_sets).chain(&r.follow).chain(&r.slow_follow) {
                st.expr.reads(&mut all);
                all.extend(st.when.iter().map(|c| c.0));
            }
            all.extend(r.delays.iter().map(|d| d.source));
            all.into_iter().collect()
        };
        let mut imports = vec![vec![]; np];
        let mut record: Vec<Vec<usize>> = vec![vec![]; np];
        for p in 0..np {
            for i in read_nodes(&parts[p]) {
                if let Owner::Part(q) = owner[i] {
                    if q != p {
                        imports[p].push((i, q));
                        if !record[q].contains(&i) {
                            record[q].push(i);
                        }
                    }
                }
            }
        }
        for (p, imp) in imports.iter().enumerate() {
            for &(i, _) in imp {
                parts[p].audio_input(i);
            }
        }
        let mut tail_imports = vec![];
        for i in read_nodes(&tail_r) {
            if let Owner::Part(q) = owner[i] {
                tail_imports.push((i, q));
                tail_r.audio_input(i);
                if !record[q].contains(&i) {
                    record[q].push(i);
                }
            }
        }
        let mut tail_record = vec![];
        for &t in taps {
            match owner[t] {
                Owner::Part(q) if !record[q].contains(&t) => record[q].push(t),
                Owner::Tail if !tail_record.contains(&t) => tail_record.push(t),
                _ => {}
            }
        }
        // what comes from outside at every sample stays so in every part
        for (i, a) in base.audio_in.iter().enumerate() {
            if *a {
                for r in parts.iter_mut().chain(std::iter::once(&mut tail_r)) {
                    r.audio_input(i);
                }
            }
        }
        // the parts shared among the threads: the heaviest first, each to the lightest thread so far
        let part_cost: Vec<usize> =
            (0..np).map(|p| left.iter().filter(|g| part_of_group[*g] == p).map(|g| cost[*g]).sum()).collect();
        let mut order: Vec<usize> = (0..np).collect();
        order.sort_by(|a, b| part_cost[*b].cmp(&part_cost[*a]).then(a.cmp(b)));
        let nt = n.min(np);
        let mut threads: Vec<Vec<usize>> = vec![vec![]; nt];
        let mut load = vec![0usize; nt];
        for p in order {
            let t = (0..nt).min_by_key(|&t| (load[t], t)).unwrap_or(0);
            load[t] += part_cost[p];
            threads[t].push(p);
        }
        let record_at = record.iter().map(|r| r.iter().enumerate().map(|(k, &i)| (i, k)).collect()).collect();
        let tail_at = tail_record.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        Some(Parts {
            kept: vec![vec![]; np],
            kept_before: vec![vec![]; np],
            parts,
            tail: tail_r,
            owner,
            source,
            threads,
            imports,
            tail_imports,
            record,
            record_at,
            tail_record,
            tail_at,
            tail_kept: vec![],
            inputs_before: vec![],
        })
    }
}

impl Parts {
    /// Who computes this node: its part's reflex (a parameter or a clock: the first part's).
    fn holder(&self, i: usize) -> &Reflex {
        match self.owner.get(i).copied().unwrap_or(Owner::All) {
            Owner::Part(p) => &self.parts[p],
            Owner::Tail => &self.tail,
            Owner::All => &self.parts[0],
        }
    }

    /// A node as it is now.
    pub fn value_at(&self, i: usize) -> f64 {
        self.holder(i).values[i]
    }

    /// Every reflex of it (the parts, then the tail).
    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Reflex> {
        self.parts.iter_mut().chain(std::iter::once(&mut self.tail))
    }

    /// How many cores it is shared among.
    pub fn count(&self) -> usize {
        self.threads.len()
    }

    /// A block: `inputs[k]`, what comes from outside at its k-th sample ((node, value) pairs). The sources compute
    /// it while the followers compute the block before, then the tail does; after it, `tapped(node, k)` says what
    /// each node listened to was at each sample of the block before (the sound comes one block later).
    pub fn block(&mut self, frames: usize, inputs: &[Vec<(usize, f64)>]) {
        // a source's last block is the followers' to read now
        for p in 0..self.parts.len() {
            if self.source[p] {
                std::mem::swap(&mut self.kept[p], &mut self.kept_before[p]);
            }
        }
        let before = std::mem::replace(&mut self.inputs_before, inputs.to_vec());
        let late = before.len();
        let record = &self.record;
        let record_at = &self.record_at;
        let imports = &self.imports;
        let source = &self.source;
        let kept_before = &self.kept_before;
        let run = |p: usize, r: &mut Reflex, kept: &mut Vec<f64>| {
            // a source: this block; a follower: the block before, reading the sources at each of its samples
            let (count, ins) = if source[p] { (frames, inputs) } else { (late, before.as_slice()) };
            kept.clear();
            kept.reserve(count * record[p].len());
            let mut mid: Vec<(usize, f64)> = Vec::with_capacity(imports[p].len());
            for k in 0..count {
                if let Some(ins) = ins.get(k) {
                    for &(i, v) in ins {
                        r.values[i] = v;
                    }
                }
                mid.clear();
                for &(i, q) in &imports[p] {
                    let m = record[q].len();
                    if let Some(v) = kept_before[q].get(k * m + record_at[q][&i]) {
                        mid.push((i, *v));
                    }
                }
                r.step_with(&mid);
                kept.extend(record[p].iter().map(|&i| r.values[i]));
            }
        };
        // each thread its parts (the first thread is this one)
        let mut slots: Vec<Option<(&mut Reflex, &mut Vec<f64>)>> =
            self.parts.iter_mut().zip(self.kept.iter_mut()).map(Some).collect();
        let mut work: Vec<Vec<(usize, &mut Reflex, &mut Vec<f64>)>> = vec![];
        for t in &self.threads {
            work.push(t.iter().filter_map(|&p| slots[p].take().map(|(r, k)| (p, r, k))).collect());
        }
        std::thread::scope(|scope| {
            let mut rest = work.into_iter();
            let first = rest.next();
            for jobs in rest {
                let run = &run;
                scope.spawn(move || {
                    for (p, r, k) in jobs {
                        run(p, r, k);
                    }
                });
            }
            for (p, r, k) in first.into_iter().flatten() {
                run(p, r, k);
            }
        });
        // the tail, the block before, hearing each part at its sample
        self.tail_kept.clear();
        for k in 0..late {
            if let Some(ins) = before.get(k) {
                for &(i, v) in ins {
                    self.tail.values[i] = v;
                }
            }
            let mut mid: Vec<(usize, f64)> = vec![];
            for &(i, q) in &self.tail_imports {
                let m = self.record[q].len();
                let kept = if self.source[q] { &self.kept_before[q] } else { &self.kept[q] };
                if let Some(v) = kept.get(k * m + self.record_at[q][&i]) {
                    mid.push((i, *v));
                }
            }
            self.tail.step_with(&mid);
            let t = &self.tail;
            self.tail_kept.extend(self.tail_record.iter().map(|&i| t.values[i]));
        }
    }

    /// A node listened to, at the k-th sample of the block before (None: not kept, or not yet).
    pub fn tapped(&self, i: usize, k: usize) -> Option<f64> {
        match self.owner.get(i).copied()? {
            Owner::Part(q) => {
                let m = self.record[q].len();
                let kept = if self.source[q] { &self.kept_before[q] } else { &self.kept[q] };
                self.record_at[q].get(&i).and_then(|&j| kept.get(k * m + j).copied())
            }
            Owner::Tail => {
                let m = self.tail_record.len();
                self.tail_at.get(&i).and_then(|&j| self.tail_kept.get(k * m + j).copied())
            }
            Owner::All => Some(self.parts[0].values[i]),
        }
    }

    /// Every node as it is now, each from who computes it.
    pub fn values(&self) -> Vec<f64> {
        (0..self.owner.len()).map(|i| self.value_at(i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::Provenance;
    use crate::world::World;

    const SINE: [&str; 14] = [
        "node omega = 1",
        "node dt = 0.01",
        "node step = 0.01",
        "relate step_def: step = omega * dt",
        "observe omega = 1",
        "observe dt = 0.01",
        "var cos_val = 1",
        "var sine_wave = 0",
        "set cos_val = cos_val - step * sine_wave",
        "set sine_wave = sine_wave + step * cos_val",
        "node modulated_signal signed",
        "relate modulation: modulated_signal = sine_wave * 5.0 + 2.0",
        "output final_output",
        "relate out_link: final_output = modulated_signal",
    ];

    #[test]
    fn a_reflex_writes_what_the_ticks_write() {
        let mut slow = World::new();
        slow.say_all(&SINE, &Provenance::default()).unwrap();
        let mut fast = World::new();
        fast.say_all(&SINE, &Provenance::default()).unwrap();
        fast.machine.run(1000);
        let mut reflex = Reflex::of(&fast.machine, Clock::Audio { rate: 48000 }).unwrap();
        reflex.follow(&fast.machine, "modulated_signal").unwrap();
        reflex.follow(&fast.machine, "final_output").unwrap();
        for _ in 0..40 {
            slow.tick(false);
            reflex.step();
            for n in ["cos_val", "sine_wave"] {
                let (a, b) = (slow.machine.net.measured[n].0, reflex.value(n).unwrap());
                assert!((a - b).abs() < 1e-6, "{n}: ticks {a}, reflex {b}");
            }
        }
        let s = reflex.value("sine_wave").unwrap();
        assert!((reflex.value("final_output").unwrap() - (5.0 * s + 2.0)).abs() < 1e-12);
    }

    #[test]
    fn a_second_of_sound_is_computed_faster_than_it_lasts() {
        let mut w = World::new();
        w.say_all(&SINE, &Provenance::default()).unwrap();
        w.machine.run(1000);
        let mut reflex = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        reflex.follow(&w.machine, "modulated_signal").unwrap();
        let t0 = std::time::Instant::now();
        let block = reflex.block("modulated_signal", 48000).unwrap();
        assert!(t0.elapsed() < std::time::Duration::from_secs(1), "{:?}", t0.elapsed());
        assert_eq!(block.len(), 48000);
        // the two sets write at once, from the values before (explicit Euler): the circle opens a little at each
        // step, by (1 + step^2), as the Python machine does too
        let (c, s) = (reflex.value("cos_val").unwrap(), reflex.value("sine_wave").unwrap());
        let want = 1.0001f64.powi(48000);
        assert!(((c * c + s * s) / want - 1.0).abs() < 1e-6, "{} vs {want}", c * c + s * s);
    }

    #[test]
    fn an_echo_is_a_delay_and_a_relation_that_brings_it_back() {
        // a sound that starts and stays: heard again 10 ms later half as loud, then a quarter... (1, 1.5, 1.75)
        let mut w = World::new();
        w.say_all(
            &[
                "node dry = 1 fixed",
                "node wet signed",
                "delay back = wet by 10ms",
                "relate echo: wet = dry + 0.5 * back",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(200);
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        r.follow(&w.machine, "wet").unwrap();
        let slot = r.slot_of("wet").unwrap();
        let out: Vec<f64> = (0..1500).map(|_| r.step_read(slot)).collect();
        for (k, want) in [(0, 1.0), (479, 1.0), (480, 1.5), (959, 1.5), (960, 1.75), (1440, 1.875)] {
            assert!((out[k] - want).abs() < 1e-9, "sample {k}: {} instead of {want}", out[k]);
        }
    }

    #[test]
    fn shared_among_cores_it_computes_the_same_one_block_later() {
        // her (a source: resonators), an instrument that follows her hand (it reads her), what mixes them; and a
        // knob that only changes when turned (its coefficient is slow)
        let mut lines: Vec<String> =
            vec!["input sk_rate = 0.25".into(), "node sk_c".into(), "relate kc: sk_c = 0.5 * sk_rate".into()];
        for k in 1..=120 {
            lines.push(format!("var her_lo{k} = {}", k as f64 / 100.0));
            lines.push(format!("var her_bp{k} = 0"));
            lines.push(format!("set her_lo{k} = her_lo{k} + 0.05 * her_bp{k}"));
            lines.push(format!("set her_bp{k} = 0.999 * her_bp{k} - 0.05 * her_lo{k}"));
            lines.push(format!("var ins_y{k} = 0"));
            lines.push(format!("set ins_y{k} = sk_c * ins_y{k} + her_lo{k}"));
        }
        lines.push("node out signed".into());
        lines.push("relate mix: out = ins_y1 + her_lo2".into());
        let mut w = World::new();
        let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();
        w.say_all(&refs, &Provenance::default()).unwrap();
        w.machine.run(5);
        let mut one = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        one.follow(&w.machine, "sk_c").unwrap();
        one.follow(&w.machine, "out").unwrap();
        one.prime();
        let out = one.slot_of("out").unwrap();
        let mut parts = one.parts(2, &[out]).expect("shared out");
        assert_eq!(parts.count(), 2);
        // the knob is computed only when turned
        let mut alone = vec![];
        let mut shared = vec![];
        for b in 0..20 {
            if b == 10 {
                let turn: IndexMap<String, f64> = [("sk_rate".to_string(), 0.75)].into_iter().collect();
                one.tune_values(&turn);
                for r in parts.all_mut() {
                    r.tune_values(&turn);
                }
            }
            for _ in 0..128 {
                one.step();
                alone.push(one.values[out]);
            }
            parts.block(128, &vec![vec![]; 128]);
            shared.extend((0..128).map(|k| parts.tapped(out, k).unwrap_or(f64::NAN)));
        }
        assert!(one.slow_sizes().1 >= 1, "the knob's coefficient is slow: {:?}", one.slow_sizes());
        // the same, one block later: exactly, sample for sample (but where the knob was turned: the instrument
        // that follows takes it a block sooner, and soon is the same again)
        let differ = (0..19 * 128).filter(|&k| (alone[k] - shared[k + 128]).abs() > 1e-9).count();
        assert!(differ < 2 * 128, "{differ} samples differ");
        assert!((12 * 128..19 * 128).all(|k| (alone[k] - shared[k + 128]).abs() < 1e-9));
        assert!(alone[..128 * 9].iter().zip(&shared[128..128 * 10]).all(|(a, b)| a == b));
    }

    #[test]
    fn a_node_never_goes_beyond_its_rail() {
        // a resonator pushed past what holds it (it grows at every turn): on its rail it bends, and sings there,
        // bounded, never to the infinite; a node with no rail of its own is only kept from the infinite
        let mut w = World::new();
        w.say_all(
            &[
                "var lo = 0.01",
                "var bp = 0",
                "set lo = lo + 0.1 * bp",
                "set bp = 1.01 * bp - 0.1 * lo",
                "rail lo 4",
                "rail bp 4",
                "var grow = 1",
                "set grow = grow * 2",
            ],
            &Provenance::default(),
        )
        .unwrap();
        assert!(w.say("rail nowhere 3", &Provenance::default()).is_err());
        w.machine.run(10);
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        let (lo, grow) = (r.slot_of("lo").unwrap(), r.slot_of("grow").unwrap());
        let mut most: f64 = 0.0;
        for _ in 0..200_000 {
            r.step();
            assert!(r.values[lo].abs() <= 4.0 && r.values[grow].is_finite());
            most = most.max(r.values[lo].abs());
        }
        // it still sings (it was not silenced), near its rail
        assert!(most > 1.5, "{most}");
        assert!(r.values[grow] <= WIDE_RAIL);
    }

    #[test]
    fn what_fades_out_in_silence_comes_to_nothing_and_its_parameters_are_known() {
        // a resonator rung once, then silence: it fades, and comes to exactly nothing (never to the subnormal
        // numbers that made the sound slower and slower with nothing playing)
        let mut w = World::new();
        w.say_all(
            &[
                "var lo = 1",
                "var bp = 0",
                "node g = 0.5 fixed",
                "set lo = lo + 0.1 * bp",
                "set bp = 0.9 * bp - 0.1 * lo",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(10);
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        let (lo, bp) = (r.slot_of("lo").unwrap(), r.slot_of("bp").unwrap());
        for _ in 0..200_000 {
            r.step();
            assert!(!r.values[lo].is_subnormal() && !r.values[bp].is_subnormal());
        }
        assert_eq!((r.values[lo], r.values[bp]), (0.0, 0.0));
        // what it reads without writing it, known once
        assert!(r.params().contains(&"g".to_string()) && !r.params().contains(&"lo".to_string()));
        assert!(r.is_param(r.slot_of("g").unwrap()) && !r.is_param(lo));
    }

    #[test]
    fn written_as_symplectic_euler_the_circle_stays_closed() {
        // the new cosine is a node of its own; the sine is written from it: the amplitude holds for ever
        let mut w = World::new();
        w.say_all(
            &[
                "node step = 0.01 fixed",
                "var cos_val = 1",
                "var sine_wave = 0",
                "node cos_new signed",
                "relate turn: cos_new = cos_val - step * sine_wave",
                "set cos_val = cos_new",
                "set sine_wave = sine_wave + step * cos_new",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(1000);
        let mut reflex = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        reflex.follow(&w.machine, "cos_new").unwrap();
        let mut worst: f64 = 0.0;
        for _ in 0..48000 {
            reflex.step();
            let (c, s) = (reflex.value("cos_val").unwrap(), reflex.value("sine_wave").unwrap());
            worst = worst.max((c * c + s * s - 1.0).abs());
        }
        assert!(worst < 0.01, "{worst}");
    }
}
