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
    /// -(sum of the other terms) / coefficient of the node: a sum solved for it.
    Sum {
        terms: Vec<(f64, Vec<(usize, f64)>)>,
        over: f64,
    },
    /// (10^rhs / product of the others)^(1/power): a product solved for it.
    Product {
        rhs: f64,
        others: Vec<(usize, f64)>,
        power: f64,
    },
    /// ln(y) or e^L.
    Ln(usize),
    Exp(usize),
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
    pub samples: u64,
}

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
            r.sets.push(Step { target: r.slot(&st.target)?, expr, when });
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
        Ok(r)
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
                let mut rest = vec![];
                for (c, t) in terms {
                    if t.len() == 1 && t.get(node).is_some_and(|p| *p == 1.0) && over.is_none() {
                        over = Some(*c);
                    } else if t.contains_key(node) {
                        bail!("{}: {node} is not alone in its term (a reflex solves straight relations)", rel.name);
                    } else {
                        let parts = t.iter().map(|(n, p)| Ok((self.slot(n)?, *p))).collect::<Result<Vec<_>>>()?;
                        rest.push((*c, parts));
                    }
                }
                let Some(over) = over else { bail!("{}: {node} is not in it", rel.name) };
                Ok(Expr::Sum { terms: rest, over })
            }
            Kind::Product { terms, rhs } => {
                let Some(&power) = terms.get(node) else { bail!("{}: {node} is not in it", rel.name) };
                let others = terms
                    .iter()
                    .filter(|(n, _)| *n != node)
                    .map(|(n, p)| Ok((self.slot(n)?, *p)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Product { rhs: *rhs, others, power })
            }
            Kind::Log { log, node: y } if log == node => Ok(Expr::Ln(self.slot(y)?)),
            Kind::Log { log, node: y } if y == node => Ok(Expr::Exp(self.slot(log)?)),
            _ => bail!("{}: a reflex does not run this kind of relation", rel.name),
        }
    }

    /// A node that follows the `set`s at every sample, computed by one of its relations (an output, a mix).
    pub fn follow(&mut self, machine: &Machine, node: &str) -> Result<()> {
        let rel = machine.net.relations.iter().find(|r| matches!(&r.kind, Kind::Sum { terms } if terms.iter().any(|(_, t)| t.len() == 1 && t.get(node) == Some(&1.0)))
            || matches!(&r.kind, Kind::Product { terms, .. } if terms.contains_key(node)));
        let Some(rel) = rel else { bail!("no relation gives {node}") };
        let expr = self.solve(rel, node)?;
        let target = self.slot(node)?;
        let when = self.conds(&rel.when)?;
        self.follow.push(Step { target, expr, when });
        Ok(())
    }

    /// Takes the values of the parameters the machine has settled since (the reflex keeps its own `set`s).
    pub fn tune(&mut self, machine: &Machine) {
        let written: Vec<usize> = self.written();
        for (n, &i) in &self.names {
            if !written.contains(&i) && (machine.index.contains_key(n) || machine.fixed.contains_key(n)) {
                self.values[i] = machine.value_si(n);
            }
        }
    }

    fn written(&self) -> Vec<usize> {
        self.sets
            .iter()
            .map(|s| s.target)
            .chain(self.follow.iter().map(|s| s.target))
            .chain(self.delays.iter().map(|d| d.target))
            .collect()
    }

    /// Its parameters: the nodes it reads but does not write (what the machine above tunes).
    pub fn params(&self) -> Vec<String> {
        let written = self.written();
        self.names.iter().filter(|(_, i)| !written.contains(i)).map(|(n, _)| n.clone()).collect()
    }

    /// Takes these values (in SI) for its parameters; what it writes itself is left alone.
    pub fn tune_values(&mut self, values: &IndexMap<String, f64>) {
        let written = self.written();
        for (n, v) in values {
            if let Some(&i) = self.names.get(n) {
                // what is only the machine settling a hair further is not a change (the same sound twice)
                let old = self.values[i];
                if !written.contains(&i) && v.is_finite() && (v - old).abs() > 1e-7 * old.abs().max(1e-9) {
                    self.values[i] = *v;
                }
            }
        }
    }

    /// How big it is: its sets, the nodes that follow, its delays, its nodes.
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (self.sets.len(), self.follow.len(), self.delays.len(), self.values.len())
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
            Expr::Sum { terms, over } => {
                let mut s = 0.0;
                for (c, parts) in terms {
                    let mut t = *c;
                    for &(i, p) in parts {
                        t *= if p == 1.0 { v[i] } else { v[i].powf(p) };
                    }
                    s += t;
                }
                -s / over
            }
            Expr::Product { rhs, others, power } => {
                let mut prod = 1.0;
                for &(i, p) in others {
                    prod *= v[i].powf(p);
                }
                (10f64.powf(*rhs) / prod).powf(1.0 / power)
            }
            Expr::Ln(i) => v[*i].ln(),
            Expr::Exp(i) => v[*i].exp(),
        }
    }

    fn holds(&self, when: &[(usize, f64, f64)]) -> bool {
        when.iter().all(|&(i, lo, hi)| lo <= self.values[i] && self.values[i] <= hi)
    }

    /// One sample: every `set` whose conditions hold writes its node, all at once from the values before; then the
    /// nodes that follow.
    pub fn step(&mut self) {
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
            self.values[i] = v;
        }
        self.scratch = writes;
        for k in 0..self.follow.len() {
            if self.holds(&self.follow[k].when) {
                let v = self.eval(&self.follow[k].expr);
                self.values[self.follow[k].target] = v;
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
    }

    /// Its memories (the `memory` nodes) as they are now: what it learned while it ran.
    pub fn memories(&self) -> IndexMap<String, f64> {
        self.names.iter().filter(|(_, &i)| self.kept[i]).map(|(n, &i)| (n.clone(), self.values[i])).collect()
    }

    /// Everything it writes itself starts again from nothing (what its sets write, what its delays hold, what
    /// follows from them): when it ran away, or when asked. Its parameters stay as they are.
    pub fn reset(&mut self) {
        for s in &self.sets {
            if !self.kept[s.target] {
                self.values[s.target] = 0.0;
            }
        }
        for d in &mut self.delays {
            d.line.iter_mut().for_each(|x| *x = 0.0);
            self.values[d.target] = 0.0;
        }
        for f in &self.follow {
            self.values[f.target] = 0.0;
        }
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
