// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The relational machine: every node and every relation a small processor that only talks to its neighbours, and
//! only works when one of its neighbours has something new to say (Gaussian belief propagation).
//!
//! A message is two numbers, a precision and a precision-weighted mean. A round: the nodes that heard something new
//! tell each of their relations what the others say; the relations that heard something new answer each of their
//! nodes. A processor that hears nothing new does nothing: a network whose measurements do not change falls silent,
//! and a change wakes only what it reaches.
//!
//! Both halves of a round are computed in parallel when there is enough to do, and give the same numbers whatever
//! the number of cores: each processor reads only what the round before wrote.
//!
//! What runs (lang.rs): product relations (straight in log10), sum relations (straight on signed values), the
//! converters between the two worlds (~g = g, redrawn by the converter itself), nonlinear sums (redrawn around
//! the current values), contacts (`keep`), measurements (anchors) and what the weights have learned (joints).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::bail;
use crate::error::{GrmError, Result};
use crate::network::{KeepOp, Kind, Link, Network};
use crate::units;

pub const LN10: f64 = std::f64::consts::LN_10;
pub const MIN_SIGMA: f64 = 1e-6; // the tightest relation a double-precision machine tells from exact
pub const PRIOR_LOG: f64 = 1e3; // weak anchor of an unmeasured positive node (dex)
pub const PRIOR_SIGNED: f64 = 1e6; // weak anchor of an unmeasured signed node (its unit)
pub const CONVERTER_SIGMA: f64 = 1e-9; // relative tightness of a converter
pub const DAMPING: f64 = 0.5; // a relation that sees its answers swing keeps this share of its last one
pub const SWINGS: u32 = 4; // after this many reversals a relation calms down
pub const QUIET: f64 = 1e-10; // a mean that moves less than this is not news
pub const QUIET_PRECISION: f64 = 1e-6; // a precision that changes less than this (relative) is not news
/// Below this much work in half a round, one core does it (sharing it out would cost more than it saves).
pub const PARALLEL_FROM: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FKind {
    Relation,
    Nonlinear,
    Convert,
    Keep,
    Anchor,
    Joint,
}

/// What the inputs were at a tick: (name, its measurement).
type Inputs = Vec<(String, Option<(f64, f64)>)>;
/// A term of a nonlinear sum: (coefficient, [(slot, power)]).
type Term = (f64, Vec<(usize, f64)>);
/// What a node tells its relations in a round: (relation, slot, precision, eta, how much news).
type Told = SmallVec<[(usize, usize, f64, f64, f64); 4]>;

/// A Gaussian in information form: (L, h).
pub type Gaussian = (Vec<Vec<f64>>, Vec<f64>);

/// What a group of weights remembers, kept by source: (L, h) of each source, summed when it speaks.
#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub names: Vec<String>,
    pub parts: IndexMap<String, Gaussian>,
}

impl Memory {
    fn total(&self) -> Gaussian {
        let n = self.names.len();
        let mut l = vec![vec![0.0; n]; n];
        let mut h = vec![0.0; n];
        for (pl, ph) in self.parts.values() {
            for a in 0..n {
                h[a] += ph[a];
                for b in 0..n {
                    l[a][b] += pl[a][b];
                }
            }
        }
        (l, h)
    }
}

/// A processor that is a relation, a measurement, a converter, a contact or a memory of weights: a straight
/// relation sum_s c_s u_s = b on the internal values u of its nodes, with weight w = 1/sigma^2.
#[derive(Debug, Clone)]
pub struct Factor {
    pub name: String,
    pub kind: FKind,
    pub vars: Vec<usize>,
    pub coef: Vec<f64>,
    pub b: f64,
    pub w: f64,
    pub out_lam: Vec<f64>,
    pub out_eta: Vec<f64>,
    pub in_lam: Vec<f64>,
    pub in_eta: Vec<f64>,
    /// (node index or -1 for a condition that never holds, lo, hi): the conditions it watches.
    pub watch: Vec<(isize, f64, f64)>,
    pub bound: f64,
    pub op: KeepOp,
    pub active: bool,
    pub text: String,
    pub damp: f64,
    pub swing: u32,
    pub trend: Vec<f64>,
    /// A nonlinear sum: [(coef, [(slot, power)])].
    pub terms: Vec<(f64, Vec<(usize, f64)>)>,
    /// A memory of weights: its parts by source, and their total (L, h).
    pub memory: Option<Memory>,
    pub l: Vec<Vec<f64>>,
    pub h: Vec<f64>,
    /// Where it comes from (the provenance of its relation), and how strongly it holds.
    pub source: String,
    pub strength: f64,
}

impl Factor {
    fn new(
        name: impl Into<String>,
        kind: FKind,
        vars: Vec<usize>,
        coef: Vec<f64>,
        b: f64,
        w: f64,
        text: impl Into<String>,
    ) -> Self {
        let k = vars.len();
        Factor {
            name: name.into(),
            kind,
            vars,
            coef,
            b,
            w,
            out_lam: vec![0.0; k],
            out_eta: vec![0.0; k],
            in_lam: vec![0.0; k],
            in_eta: vec![0.0; k],
            watch: vec![],
            bound: 0.0,
            op: KeepOp::AtLeast,
            active: false,
            text: text.into(),
            damp: 0.0,
            swing: 0,
            trend: vec![0.0; k],
            terms: vec![],
            memory: None,
            l: vec![],
            h: vec![],
            source: String::new(),
            strength: 1.0,
        }
    }
}

/// What a relation needs to read of the nodes while it answers (shared by every processor of a round).
struct View<'a> {
    x: &'a [f64],
    signed: &'a [bool],
    shift: &'a [f64],
}

impl View<'_> {
    fn value_si(&self, i: usize) -> f64 {
        let v = if self.signed[i] {
            self.x[i]
        } else if self.x[i] < 308.0 {
            10f64.powf(self.x[i])
        } else {
            f64::INFINITY
        };
        v * 10f64.powf(self.shift[i])
    }
}

/// How a round is shared out: in order of the biggest news first, at most `cap` relations a round (the rest wait
/// for the next one), or all of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub cap: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub net: Network,
    pub names: Vec<String>,
    pub index: HashMap<String, usize>,
    pub var_names: Vec<String>,
    pub var_signed: Vec<bool>,
    pub var_shift: Vec<f64>,
    pub start: Vec<f64>,
    pub fixed: HashMap<String, f64>,
    pub shift: HashMap<String, f64>,
    pub factors: Vec<Factor>,
    pub anchor: HashMap<String, usize>,
    pub joints: Vec<usize>,
    /// The factors that hold beliefs of several nodes together (net.priors).
    pub priors: Vec<usize>,
    pub node_factors: Vec<Vec<(usize, usize)>>,
    pub watchers: Vec<Vec<usize>>,
    pub lam: Vec<f64>,
    pub eta: Vec<f64>,
    pub x: Vec<f64>,
    pub last_move: Vec<f64>,
    pub dirty_nodes: Vec<usize>,
    pub dirty_factors: Vec<usize>,
    /// Whether each relation is waiting (what dirty_factors holds, to be asked at once).
    dirty_mark: Vec<bool>,
    /// How much news each waiting relation has (the biggest first when a round is capped).
    news: Vec<f64>,
    busy: Vec<usize>,
    pub rounds: u64,
    pub messages: u64,
    pub settled: bool,
    pub last_surprise: IndexMap<String, f64>,
    heard: bool,
    seen: Option<Inputs>,
    tick_state: Option<(bool, Inputs)>,
    /// How many threads share the work (None: as many as the host has).
    pub threads: Option<usize>,
    pool: Option<Arc<rayon::ThreadPool>>,
    pub budget: Budget,
    pub tolerance: Tolerance,
}

/// Small lists live on the stack (a node rarely has more than a few relations, a relation a few nodes).
type Small = SmallVec<[f64; 8]>;

/// For each k, the sum of all the values but the k-th (in the order Python adds them: before k, then after k).
fn others(values: &[f64]) -> Small {
    let n = values.len();
    let mut pre: Small = SmallVec::with_capacity(n + 1);
    pre.push(0.0);
    for k in 0..n {
        pre.push(pre[k] + values[k]);
    }
    let mut out: Small = smallvec::smallvec![0.0; n];
    let mut suf = 0.0;
    for k in (0..n).rev() {
        out[k] = pre[k] + suf;
        suf += values[k];
    }
    out
}

fn news(old_l: f64, old_e: f64, new_l: f64, new_e: f64, tol: Tolerance) -> bool {
    if old_l == new_l && old_e == new_e {
        return false;
    }
    if old_l <= 0.0 && new_l <= 0.0 {
        return false;
    }
    if old_l <= 0.0 || new_l <= 0.0 {
        return true;
    }
    if (new_l - old_l).abs() > tol.precision * old_l.max(new_l) {
        return true;
    }
    let (m0, m1) = (old_e / old_l, new_e / new_l);
    (m1 - m0).abs() > tol.mean + 1e-15 * m1.abs()
}

/// What is not news: a mean that moves less than `mean`, a precision that changes less than `precision`
/// (relative). The machine's own are as fine as a double can tell; an eye, which only needs a picture right to
/// a part in ten thousand, settles far sooner with coarser ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    pub mean: f64,
    pub precision: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance { mean: QUIET, precision: QUIET_PRECISION }
    }
}

/// How much news a message brings (to put the biggest first).
fn news_size(old_l: f64, old_e: f64, new_l: f64, new_e: f64) -> f64 {
    if old_l <= 0.0 || new_l <= 0.0 {
        return f64::INFINITY;
    }
    let (m0, m1) = (old_e / old_l, new_e / new_l);
    (m1 - m0).abs() * new_l.sqrt() + ((new_l - old_l) / old_l.max(new_l)).abs()
}

impl Machine {
    pub fn new(net: Network) -> Result<Self> {
        let mut m = Machine {
            net,
            names: vec![],
            index: HashMap::new(),
            var_names: vec![],
            var_signed: vec![],
            var_shift: vec![],
            start: vec![],
            fixed: HashMap::new(),
            shift: HashMap::new(),
            factors: vec![],
            anchor: HashMap::new(),
            joints: vec![],
            priors: vec![],
            node_factors: vec![],
            watchers: vec![],
            lam: vec![],
            eta: vec![],
            x: vec![],
            last_move: vec![],
            dirty_nodes: vec![],
            dirty_factors: vec![],
            dirty_mark: vec![],
            news: vec![],
            busy: vec![],
            rounds: 0,
            messages: 0,
            settled: false,
            last_surprise: IndexMap::new(),
            heard: false,
            seen: None,
            tick_state: None,
            threads: None,
            pool: None,
            budget: Budget { cap: None },
            tolerance: Tolerance::default(),
        };
        m.compile()?;
        Ok(m)
    }

    // ------------------------------------------------------------------ building the processors
    fn compile(&mut self) -> Result<()> {
        let net = self.net.clone();
        self.names = net.nodes.iter().filter(|(_, s)| !s.fixed).map(|(n, _)| n.clone()).collect();
        for (n, s) in &net.nodes {
            if let Some(d) = &s.dim {
                self.shift.insert(n.clone(), units::unit_log10(d));
            }
        }
        for (n, s) in &net.nodes {
            if s.fixed {
                if !s.signed && s.guess <= 0.0 {
                    bail!("fixed node {n} must be positive (or signed)");
                }
                self.fixed.insert(n.clone(), s.guess / 10f64.powf(self.shift_of(n)));
            }
        }
        for n in self.names.clone() {
            let signed = net.nodes[&n].signed;
            self.add_var(&n, signed);
        }
        let mut twins: HashMap<String, usize> = HashMap::new();
        for r in &net.relations {
            let w_of = |sigma: f64, unit: f64| r.strength / (sigma.max(MIN_SIGMA) / unit).powi(2);
            let mut factor = match &r.kind {
                Kind::Product { terms, rhs } => {
                    let (mut vars, mut coef, mut b) = (vec![], vec![], *rhs);
                    for (n, p) in terms {
                        if let Some(&i) = self.index.get(n) {
                            vars.push(i);
                            coef.push(*p);
                        } else {
                            b -= p * self.fixed[n].log10();
                        }
                    }
                    if vars.is_empty() {
                        continue;
                    }
                    Factor::new(&r.name, FKind::Relation, vars, coef, b, w_of(r.sigma, 1.0), &r.text)
                }
                Kind::Sum { terms } if self.nonlinear_sum(terms) => {
                    // a term with two free nodes (k * x) or a power: redrawn around the current values
                    let (mut vars, mut nterms, mut b): (Vec<usize>, Vec<Term>, f64) = (vec![], vec![], 0.0);
                    for (c, t) in terms {
                        let (mut k, mut parts) = (*c, vec![]);
                        for (n, p) in t {
                            if let Some(&i) = self.index.get(n) {
                                if !vars.contains(&i) {
                                    vars.push(i);
                                }
                                parts.push((vars.iter().position(|v| *v == i).unwrap(), *p));
                            } else {
                                k *= self.fixed[n].powf(*p);
                            }
                        }
                        if parts.is_empty() {
                            b -= k;
                        } else {
                            nterms.push((k, parts));
                        }
                    }
                    let k = vars.len();
                    let mut f = Factor::new(
                        &r.name,
                        FKind::Nonlinear,
                        vars,
                        vec![0.0; k],
                        b,
                        w_of(r.sigma, self.unit_of(terms)),
                        &r.text,
                    );
                    f.terms = nterms;
                    f
                }
                Kind::Sum { terms } => {
                    let (mut vars, mut coef, mut b): (Vec<usize>, Vec<f64>, f64) = (vec![], vec![], 0.0);
                    for (c, t) in terms {
                        let free: Vec<&String> = t.keys().filter(|n| self.index.contains_key(*n)).collect();
                        let mut k = *c;
                        for (n, p) in t {
                            if !self.index.contains_key(n) {
                                k *= self.fixed[n].powf(*p);
                            }
                        }
                        if free.is_empty() {
                            b -= k;
                            continue;
                        }
                        let n = free[0].clone();
                        let i =
                            if self.var_signed[self.index[&n]] { self.index[&n] } else { self.twin(&n, &mut twins) };
                        if let Some(pos) = vars.iter().position(|v| *v == i) {
                            coef[pos] += k;
                        } else {
                            vars.push(i);
                            coef.push(k);
                        }
                    }
                    if vars.is_empty() {
                        continue;
                    }
                    Factor::new(&r.name, FKind::Relation, vars, coef, b, w_of(r.sigma, self.unit_of(terms)), &r.text)
                }
                Kind::Log { log, node } => {
                    // L = ln(y): y is held by its log10 u, so L = ln10 (u + shift): a straight relation
                    let (mut vars, mut coef, mut b) = (vec![], vec![], 0.0);
                    if let Some(&i) = self.index.get(log) {
                        vars.push(i);
                        coef.push(1.0);
                    } else {
                        b -= self.fixed[log];
                    }
                    if let Some(&i) = self.index.get(node) {
                        vars.push(i);
                        coef.push(-LN10);
                        b += LN10 * self.shift_of(node);
                    } else {
                        b += (self.fixed[node] * 10f64.powf(self.shift_of(node))).ln();
                    }
                    if vars.is_empty() {
                        continue;
                    }
                    Factor::new(&r.name, FKind::Relation, vars, coef, b, r.strength / (MIN_SIGMA * MIN_SIGMA), &r.text)
                }
                Kind::Keep { node, op, bound } => {
                    let Some(&i) = self.index.get(node) else { continue };
                    let mut f = Factor::new(&r.name, FKind::Keep, vec![i], vec![1.0], 0.0, 0.0, &r.text);
                    let k = self.shift_of(node);
                    f.bound = if self.var_signed[i] { bound / 10f64.powf(k) } else { bound.log10() - k };
                    f.op = *op;
                    f
                }
            };
            for c in &r.when {
                if let Some(&i) = self.index.get(&c.node) {
                    factor.watch.push((i as isize, c.lo, c.hi));
                } else if !(c.lo <= self.fixed[&c.node] && self.fixed[&c.node] <= c.hi) {
                    factor.watch.push((-1, 1.0, 0.0)); // a fixed condition that never holds
                }
            }
            factor.source = r.origin.source.clone();
            factor.strength = r.strength;
            self.factors.push(factor);
        }
        // the weights (`learn`) learn together: weights that share a relation keep one memory, by source
        for names in self.weight_groups() {
            let vars: Vec<usize> = names.iter().map(|n| self.index[n]).collect();
            let k = vars.len();
            let mut f = Factor::new(
                format!("weights {}", names.join(" ")),
                FKind::Joint,
                vars,
                vec![0.0; k],
                0.0,
                0.0,
                format!("what {} have learned", names.join(", ")),
            );
            let memory = self.link_memory(&names);
            let (l, h) = memory.total();
            f.memory = Some(memory);
            f.l = l;
            f.h = h;
            self.joints.push(self.factors.len());
            self.factors.push(f);
        }
        // beliefs of several nodes together (a state carried with its covariance, a vector measured together)
        for prior in net.priors.clone() {
            let Some(vars) = prior.names.iter().map(|n| self.index.get(n).copied()).collect::<Option<Vec<usize>>>()
            else {
                continue;
            };
            let k = vars.len();
            let mut f = Factor::new(
                format!("prior {}", prior.name),
                FKind::Joint,
                vars,
                vec![0.0; k],
                0.0,
                0.0,
                prior.name.clone(),
            );
            f.h = prior.info.iter().map(|row| row.iter().zip(&prior.mean).map(|(a, b)| a * b).sum()).collect();
            f.l = prior.info.clone();
            f.source = "prior".into();
            self.priors.push(self.factors.len());
            self.factors.push(f);
        }
        // one anchor per free node: its measurement, or a weak prior at its start
        for n in self.names.clone() {
            let i = self.index[&n];
            self.anchor.insert(n.clone(), self.factors.len());
            self.factors.push(Factor::new(format!("anchor {n}"), FKind::Anchor, vec![i], vec![1.0], 0.0, 0.0, ""));
            self.set_anchor(&n);
        }
        let nv = self.var_names.len();
        self.node_factors = vec![vec![]; nv];
        self.watchers = vec![vec![]; nv];
        for (fi, f) in self.factors.iter().enumerate() {
            for (s, &i) in f.vars.iter().enumerate() {
                self.node_factors[i].push((fi, s));
            }
            for &(i, _, _) in &f.watch {
                if i >= 0 {
                    self.watchers[i as usize].push(fi);
                }
            }
        }
        self.lam = vec![0.0; nv];
        self.eta = vec![0.0; nv];
        self.x = self.start.clone();
        self.last_move = vec![0.0; nv];
        self.dirty_nodes = (0..nv).collect();
        self.set_dirty((0..self.factors.len()).collect());
        self.news = vec![f64::INFINITY; self.factors.len()];
        Ok(())
    }

    pub fn shift_of(&self, n: &str) -> f64 {
        self.shift.get(n).copied().unwrap_or(0.0)
    }

    /// The signed twin of a positive node: ~q = q, tied by a converter.
    fn twin(&mut self, q: &str, twins: &mut HashMap<String, usize>) -> usize {
        if let Some(&t) = twins.get(q) {
            return t;
        }
        let t = format!("~{q}");
        self.add_var(&t, true);
        let f = Factor::new(
            format!("convert {q}"),
            FKind::Convert,
            vec![self.index[&t], self.index[q]],
            vec![1.0, 0.0],
            0.0,
            1.0,
            format!("{t} = {q}"),
        );
        self.factors.push(f);
        twins.insert(q.to_string(), self.index[&t]);
        self.index[&t]
    }

    fn add_var(&mut self, name: &str, signed: bool) {
        if self.index.contains_key(name) {
            return;
        }
        self.index.insert(name.to_string(), self.var_names.len());
        self.var_names.push(name.to_string());
        self.var_signed.push(signed);
        let start = if let Some(q) = name.strip_prefix('~') {
            self.net.nodes[q].guess
        } else if signed {
            self.net.nodes[name].guess / 10f64.powf(self.shift_of(name))
        } else {
            self.net.nodes[name].guess.log10() - self.shift_of(name)
        };
        self.var_shift.push(if name.starts_with('~') { 0.0 } else { self.shift_of(name) });
        self.start.push(start);
    }

    fn nonlinear_sum(&self, terms: &[(f64, IndexMap<String, f64>)]) -> bool {
        terms.iter().any(|(_, t)| {
            let free: Vec<(&String, &f64)> = t.iter().filter(|(n, _)| self.index.contains_key(*n)).collect();
            free.len() > 1 || (free.len() == 1 && (free[0].1 - 1.0).abs() > 1e-12)
        })
    }

    /// The machine's unit of a sum relation (that of its first node with a dimension).
    fn unit_of(&self, terms: &[(f64, IndexMap<String, f64>)]) -> f64 {
        for (_, t) in terms {
            for n in t.keys() {
                if let Some(s) = self.shift.get(n) {
                    return 10f64.powf(*s);
                }
            }
        }
        1.0
    }

    /// Weights that share a relation, together (connected groups).
    fn weight_groups(&self) -> Vec<Vec<String>> {
        let weights: Vec<String> = self.names.iter().filter(|n| self.net.nodes[*n].learn).cloned().collect();
        let mut parent: HashMap<String, String> = weights.iter().map(|w| (w.clone(), w.clone())).collect();
        fn root(parent: &mut HashMap<String, String>, a: &str) -> String {
            let mut a = a.to_string();
            while parent[&a] != a {
                let p = parent[&a].clone();
                let gp = parent[&p].clone();
                parent.insert(a.clone(), gp);
                a = p;
            }
            a
        }
        for r in &self.net.relations {
            let ws: Vec<String> = r.nodes().into_iter().filter(|n| parent.contains_key(n)).collect();
            for a in ws.iter().skip(1) {
                let ra = root(&mut parent, a);
                let r0 = root(&mut parent, &ws[0]);
                parent.insert(ra, r0);
            }
        }
        let mut groups: IndexMap<String, Vec<String>> = IndexMap::new();
        for w in &weights {
            let r = root(&mut parent, w);
            groups.entry(r).or_default().push(w.clone());
        }
        groups.into_values().collect()
    }

    /// The memory of a group of weights: what was learned before, by source (possibly by smaller groups that have
    /// met since), and each weight's own prior for the rest.
    fn link_memory(&self, names: &[String]) -> Memory {
        let n = names.len();
        let mut parts: IndexMap<String, Gaussian> = IndexMap::new();
        let mut covered: Vec<String> = vec![];
        for link in &self.net.links {
            let idx: Vec<usize> = link.names.iter().filter_map(|a| names.iter().position(|b| b == a)).collect();
            if idx.len() != link.names.len() {
                continue;
            }
            let part = parts.entry(link.source.clone()).or_insert_with(|| (vec![vec![0.0; n]; n], vec![0.0; n]));
            for (a, &ia) in idx.iter().enumerate() {
                part.1[ia] += link.h[a];
                for (b, &ib) in idx.iter().enumerate() {
                    part.0[ia][ib] += link.l[a][b];
                }
            }
            covered.extend(link.names.iter().cloned());
        }
        for (k, name) in names.iter().enumerate() {
            if covered.contains(name) {
                continue;
            }
            let (v, sg) = self.net.measured.get(name).copied().unwrap_or((self.net.nodes[name].guess, 1e3));
            let sh = self.shift_of(name);
            let (u, sg) = if self.var_signed[self.index[name]] {
                (v / 10f64.powf(sh), sg / 10f64.powf(sh))
            } else {
                (v.log10() - sh, sg)
            };
            let part = parts.entry("prior".into()).or_insert_with(|| (vec![vec![0.0; n]; n], vec![0.0; n]));
            part.0[k][k] += 1.0 / (sg * sg);
            part.1[k] += u / (sg * sg);
        }
        Memory { names: names.to_vec(), parts }
    }

    pub fn set_anchor(&mut self, name: &str) {
        let fi = self.anchor[name];
        let i = self.index[name];
        let sh = self.shift_of(name);
        // a weight's memory is its group's (joint), and a node carried together with others has their prior:
        // neither has an anchor of its own
        let held_elsewhere = self.net.nodes.get(name).is_some_and(|s| s.learn)
            || self.net.carries.iter().any(|c| c.targets.iter().any(|n| n == name));
        let (b, w) = if held_elsewhere {
            (0.0, 0.0)
        } else if let Some(&(v, s)) = self.net.measured.get(name) {
            if self.var_signed[i] {
                let k = 10f64.powf(sh);
                (v / k, 1.0 / (s.max(MIN_SIGMA) / k).powi(2))
            } else {
                (v.log10() - sh, 1.0 / s.max(MIN_SIGMA).powi(2))
            }
        } else {
            let p = if self.var_signed[i] { PRIOR_SIGNED / 10f64.powf(sh) } else { PRIOR_LOG };
            (self.start[i], 1.0 / (p * p))
        };
        self.factors[fi].b = b;
        self.factors[fi].w = w;
    }

    /// The relations waiting are these (and only these).
    fn set_dirty(&mut self, list: Vec<usize>) {
        self.dirty_mark = vec![false; self.factors.len()];
        for &f in &list {
            self.dirty_mark[f] = true;
        }
        self.dirty_factors = list;
    }

    /// These relations wait too.
    fn set_dirty_more(&mut self, list: Vec<usize>) {
        for f in list {
            if !self.dirty_mark[f] {
                self.dirty_mark[f] = true;
                self.dirty_factors.push(f);
            }
        }
    }

    /// Relations given a new weight (and woken): what a picture's edges do to the smoothness around them.
    pub fn reweigh(&mut self, updates: &[(usize, f64)]) {
        for &(fi, w) in updates {
            if self.factors[fi].w != w {
                self.factors[fi].w = w;
                self.wake_factor(fi);
            }
        }
    }

    fn wake_factor(&mut self, fi: usize) {
        if !self.dirty_mark[fi] {
            self.dirty_mark[fi] = true;
            self.dirty_factors.push(fi);
        }
        self.news[fi] = f64::INFINITY;
    }

    // ------------------------------------------------------------------ talking to it while it runs
    /// A new measurement (an input: a key, a sensor, the past of the next moment). Wakes only its node.
    pub fn observe(&mut self, name: &str, value: f64, sigma: Option<f64>) -> Result<()> {
        let Some(&i) = self.index.get(name) else { bail!("{name} is not a free node") };
        let sigma = sigma.unwrap_or_else(|| {
            self.net.measured.get(name).map(|m| m.1).unwrap_or(if self.var_signed[i] {
                1e-3 * value.abs().max(1.0)
            } else {
                0.01
            })
        });
        self.net.measured.insert(name.to_string(), (value, sigma));
        self.set_anchor(name);
        self.wake_factor(self.anchor[name]);
        Ok(())
    }

    pub fn forget(&mut self, name: &str) {
        self.net.measured.shift_remove(name);
        self.remeasured(name);
    }

    /// Its measurement has changed in `net.measured` (by a statement): its anchor says it, and only it wakes.
    pub fn remeasured(&mut self, name: &str) {
        if self.anchor.contains_key(name) {
            self.set_anchor(name);
            self.wake_factor(self.anchor[name]);
        }
    }

    /// A node as the outside reads it: (value in SI, sigma in SI for a signed node or in dex for a positive one).
    pub fn reading(&self, name: &str) -> (f64, f64) {
        let s = self.sigma(name);
        let signed = self.index.get(name).map(|&i| self.var_signed[i]).unwrap_or(false);
        (self.value_si(name), if signed { s * 10f64.powf(self.shift_of(name)) } else { s })
    }

    /// How unsure it is of a node, as a share of its value (0.1: about ± 10 %).
    pub fn doubt(&self, name: &str) -> f64 {
        let signed = self.index.get(name).map(|&i| self.var_signed[i]).unwrap_or(false);
        if signed {
            let (m, sd) = self.reading(name);
            sd / m.abs().max(1e-12)
        } else {
            10f64.powf(self.sigma(name)) - 1.0
        }
    }

    pub fn value(&self, name: &str) -> f64 {
        if let Some(v) = self.fixed.get(name) {
            return *v;
        }
        let i = self.index[name];
        if self.var_signed[i] {
            self.x[i]
        } else if self.x[i] < 308.0 {
            10f64.powf(self.x[i])
        } else {
            f64::INFINITY
        }
    }

    /// The value inside the machine: log10 for a positive node (less its unit's shift), its own unit for a signed one.
    pub fn internal_value(&self, name: &str) -> f64 {
        match self.index.get(name) {
            Some(&i) => self.x[i],
            None => {
                let v = self.fixed.get(name).copied().unwrap_or(0.0);
                if self.net.nodes.get(name).is_some_and(|s| s.signed) {
                    v
                } else {
                    v.max(f64::MIN_POSITIVE).log10()
                }
            }
        }
    }

    /// The value as the outside reads it: in SI for a node with a dimension.
    pub fn value_si(&self, name: &str) -> f64 {
        self.value(name) * 10f64.powf(self.shift_of(name))
    }

    /// The uncertainty the messages give (not reliable around loops): dex, or the node's unit if signed.
    pub fn sigma(&self, name: &str) -> f64 {
        if self.fixed.contains_key(name) {
            return 0.0;
        }
        let lam = self.lam[self.index[name]];
        if lam > 0.0 {
            1.0 / lam.sqrt()
        } else {
            f64::INFINITY
        }
    }

    pub fn quiet(&self) -> bool {
        self.dirty_nodes.is_empty() && self.dirty_factors.is_empty()
    }

    pub fn processors(&self) -> usize {
        self.factors.len() + self.var_names.len()
    }

    fn parallel(&self, work: usize) -> bool {
        work >= PARALLEL_FROM && self.threads != Some(1)
    }

    // ------------------------------------------------------------------ one round
    /// One round: the nodes that heard something new speak to their relations, then the relations that heard
    /// something new answer their nodes. Returns how far the furthest node moved.
    pub fn round(&mut self) -> f64 {
        let tol = self.tolerance;
        let nf = self.factors.len();
        let mut woken = vec![false; nf];
        let mut woken_list: Vec<usize> = Vec::new();
        let dirty_factors = std::mem::take(&mut self.dirty_factors);
        for &f in &dirty_factors {
            self.dirty_mark[f] = false;
        }
        // a capped round: the biggest news first, the rest waits for the next round
        let (now, later) = match self.budget.cap {
            Some(cap) if dirty_factors.len() > cap => {
                let mut d = dirty_factors;
                d.sort_by(|a, b| {
                    self.news[*b].partial_cmp(&self.news[*a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b))
                });
                let later = d.split_off(cap);
                (d, later)
            }
            _ => (dirty_factors, vec![]),
        };
        for f in now {
            if !woken[f] {
                woken[f] = true;
                woken_list.push(f);
            }
        }
        // 1) the nodes that heard something new: to each relation, what all the others say
        let dirty_nodes = std::mem::take(&mut self.dirty_nodes);
        let factors = &self.factors;
        let node_factors = &self.node_factors;
        let speak = |&i: &usize| {
            let nfs = &node_factors[i];
            let lams: Small = nfs.iter().map(|&(f, s)| factors[f].out_lam[s]).collect();
            let etas: Small = nfs.iter().map(|&(f, s)| factors[f].out_eta[s]).collect();
            let (ol, oe) = (others(&lams), others(&etas));
            let mut told: Told = SmallVec::new();
            for (k, &(f, s)) in nfs.iter().enumerate() {
                let fac = &factors[f];
                if news(fac.in_lam[s], fac.in_eta[s], ol[k], oe[k], tol) {
                    let size = news_size(fac.in_lam[s], fac.in_eta[s], ol[k], oe[k]);
                    told.push((f, s, ol[k], oe[k], size));
                }
            }
            let lam: f64 = lams.iter().sum();
            let eta: f64 = etas.iter().sum();
            (i, told, lam, eta)
        };
        let spoken: Vec<(usize, Told, f64, f64)> = if self.parallel(dirty_nodes.len()) {
            self.pool(|| dirty_nodes.par_iter().map(speak).collect())
        } else {
            dirty_nodes.iter().map(speak).collect()
        };
        let mut moved: f64 = 0.0;
        for (i, told, lam, eta) in spoken {
            for (f, s, l, e, size) in told {
                let fac = &mut self.factors[f];
                fac.in_lam[s] = l;
                fac.in_eta[s] = e;
                self.news[f] = if woken[f] { self.news[f].max(size) } else { size };
                if !woken[f] {
                    woken[f] = true;
                    woken_list.push(f);
                }
                self.messages += 1;
            }
            self.lam[i] = lam;
            self.eta[i] = eta;
            if lam > 0.0 {
                let new = eta / lam;
                self.last_move[i] = (new - self.x[i]).abs();
                if self.last_move[i] > tol.mean + 1e-15 * new.abs() {
                    for &f in &self.watchers[i] {
                        if !woken[f] {
                            woken[f] = true;
                            woken_list.push(f);
                            self.news[f] = f64::INFINITY;
                        }
                    }
                }
                moved = moved.max(self.last_move[i]);
                self.x[i] = new;
            }
        }
        // 2) the relations that heard something new: to each node, what it would be if the relation held
        let view = View { x: &self.x, signed: &self.var_signed, shift: &self.var_shift };
        let answer_one = |fac: &mut Factor| -> SmallVec<[usize; 4]> {
            let old_l: Small = SmallVec::from_slice(&fac.out_lam);
            let old_e: Small = SmallVec::from_slice(&fac.out_eta);
            answer(fac, &view);
            (0..fac.vars.len())
                .filter(|&s| news(old_l[s], old_e[s], fac.out_lam[s], fac.out_eta[s], tol))
                .map(|s| fac.vars[s])
                .collect()
        };
        let answered: Vec<SmallVec<[usize; 4]>> = if self.parallel(woken_list.len()) {
            let mask = &woken;
            let factors = &mut self.factors;
            let mut work = move || {
                factors.par_iter_mut().enumerate().filter(|(f, _)| mask[*f]).map(|(_, fac)| answer_one(fac)).collect()
            };
            match &self.pool {
                Some(p) => p.install(work),
                None => work(),
            }
        } else {
            let mut sorted = woken_list.clone();
            sorted.sort_unstable();
            sorted.into_iter().map(|f| answer_one(&mut self.factors[f])).collect()
        };
        let mut node_dirty = vec![false; self.var_names.len()];
        let mut nodes = Vec::new();
        for list in answered {
            for i in list {
                self.messages += 1;
                if !node_dirty[i] {
                    node_dirty[i] = true;
                    nodes.push(i);
                }
            }
        }
        // a relation that had nothing to do this round has settled: its next work starts calm and undamped
        for &f in &self.busy {
            if !woken[f] {
                let fac = &mut self.factors[f];
                fac.swing = 0;
                fac.damp = 0.0;
                fac.trend = vec![0.0; fac.vars.len()];
            }
        }
        self.busy = woken_list;
        self.set_dirty_more(later);
        self.dirty_nodes = nodes;
        self.rounds += 1;
        moved
    }

    fn pool<R: Send>(&self, work: impl FnOnce() -> R + Send) -> R {
        match &self.pool {
            Some(p) => p.install(work),
            None => work(),
        }
    }

    /// How many threads share the work of a round (None: as many as the host has). The numbers do not change.
    pub fn set_threads(&mut self, threads: Option<usize>) {
        self.threads = threads;
        self.pool = match threads {
            Some(t) if t > 1 => rayon::ThreadPoolBuilder::new().num_threads(t).build().ok().map(Arc::new),
            _ => None,
        };
    }

    /// Rounds until nothing has anything new to say (or max_rounds). Returns the rounds it took.
    pub fn run(&mut self, max_rounds: u64) -> u64 {
        let start = self.rounds;
        while self.rounds - start < max_rounds && !self.quiet() {
            self.round();
        }
        self.rounds - start
    }

    /// Rounds for at most `budget` of time (a frame): whatever it has reached is usable, even if it has not
    /// settled ("anytime"). Returns the rounds it took.
    pub fn run_for(&mut self, budget: Duration) -> u64 {
        let t0 = Instant::now();
        let start = self.rounds;
        while !self.quiet() && t0.elapsed() < budget {
            self.round();
        }
        self.rounds - start
    }

    // ------------------------------------------------------------------ the clock
    fn inputs_seen(&self) -> Inputs {
        self.net
            .nodes
            .iter()
            .filter(|(_, s)| s.input)
            .map(|(n, _)| (n.clone(), self.net.measured.get(n).copied()))
            .collect()
    }

    /// The start of a tick: when the inputs have changed, the weights keep what the inputs before taught them.
    pub fn begin_tick(&mut self, learn: bool) {
        let seen = self.inputs_seen();
        let new = learn || Some(&seen) != self.seen.as_ref();
        if new && self.heard {
            self.fold();
        }
        self.tick_state = Some((new, seen));
    }

    /// The end of a tick: its surprise, then every `set` whose conditions hold writes its variable (all at once,
    /// from the values before). Returns {variable: new value}.
    pub fn end_tick(&mut self) -> IndexMap<String, f64> {
        let (new, seen) = self.tick_state.take().unwrap_or_else(|| (false, self.seen.clone().unwrap_or_default()));
        self.settled = self.quiet();
        if new {
            self.last_surprise = self.surprise();
        }
        let mut writes: IndexMap<String, f64> = IndexMap::new();
        let mut carried: IndexMap<String, f64> = IndexMap::new();
        for st in &self.net.sets {
            if st.when.iter().all(|c| {
                let v = self.value_si(&c.node);
                c.lo <= v && v <= c.hi
            }) {
                writes.insert(st.target.clone(), self.value_si(&st.source));
                if st.carry {
                    carried.insert(st.target.clone(), self.reading(&st.source).1);
                }
            }
        }
        // carried together: the sources' joint belief becomes the targets' prior
        for group in self.net.carries.clone() {
            if !group.when.iter().all(|c| {
                let v = self.value_si(&c.node);
                c.lo <= v && v <= c.hi
            }) {
                continue;
            }
            let sources: Vec<&str> = group.sources.iter().map(|s| s.as_str()).collect();
            let Ok(cov) = self.covariance(&sources) else { continue };
            let k = sources.len();
            let info = crate::time::inverse_spd(&cov).unwrap_or_else(|| {
                (0..k)
                    .map(|i| (0..k).map(|j| if i == j { 1.0 / cov[i][i].max(1e-24) } else { 0.0 }).collect())
                    .collect()
            });
            let mean: Vec<f64> = sources.iter().map(|n| self.internal_value(n)).collect();
            if let Some(p) = self.net.priors.iter_mut().find(|p| p.name == group.prior) {
                p.mean = mean.clone();
                p.info = info.clone();
            }
            if let Some(&fi) = self.priors.iter().find(|&&fi| self.factors[fi].text == group.prior) {
                let f = &mut self.factors[fi];
                f.h = info.iter().map(|row| row.iter().zip(&mean).map(|(a, b)| a * b).sum()).collect();
                f.l = info;
                self.wake_factor(fi);
            }
        }
        for (name, v) in &writes {
            let old = self.net.measured.get(name).copied();
            if let Some(sigma) = carried.get(name) {
                // carried: its value and how sure of it the network was (never surer than a machine can tell)
                let sigma = if sigma.is_finite() { sigma.max(MIN_SIGMA * 1e-3) } else { PRIOR_SIGNED };
                let _ = self.observe(name, *v, Some(sigma));
            } else if old.map(|m| m.0) != Some(*v) {
                let _ = self.observe(name, *v, Some(old.map(|m| m.1).unwrap_or(1e-9)));
            }
        }
        self.heard = true;
        self.seen = Some(seen);
        self.net.ticks += 1;
        writes
    }

    /// One tick: the network settles (at most max_rounds), then the `set`s write.
    pub fn tick(&mut self, max_rounds: u64, learn: bool) -> IndexMap<String, f64> {
        self.begin_tick(learn);
        self.run(max_rounds);
        self.end_tick()
    }

    // ------------------------------------------------------------------ learning, by source
    /// What the current inputs teach the weights, relation by relation, kept by the source of each relation.
    /// Returns {joint factor: {source: (dL, dh)}}.
    pub fn evidence(&mut self) -> Vec<(usize, IndexMap<String, Gaussian>)> {
        let mut out = Vec::new();
        for &ji in &self.joints.clone() {
            let jvars = self.factors[ji].vars.clone();
            let n = jvars.len();
            let mut by_source: IndexMap<String, Gaussian> = IndexMap::new();
            for fi in 0..self.factors.len() {
                let kind = self.factors[fi].kind;
                if !matches!(kind, FKind::Relation | FKind::Nonlinear)
                    || !self.factors[fi].vars.iter().any(|i| jvars.contains(i))
                {
                    continue;
                }
                if !self.factors[fi].watch.is_empty() && !self.factors[fi].active {
                    continue;
                }
                if kind == FKind::Nonlinear {
                    let view = View { x: &self.x, signed: &self.var_signed, shift: &self.var_shift };
                    redraw(&mut self.factors[fi], &view);
                }
                let f = &self.factors[fi];
                let mut gsum: IndexMap<usize, f64> = IndexMap::new();
                let mut others_var = if f.w > 0.0 { 1.0 / f.w } else { f64::INFINITY };
                let mut rest = 0.0;
                for (s, (&i, &c)) in f.vars.iter().zip(f.coef.iter()).enumerate() {
                    if let Some(slot) = jvars.iter().position(|v| *v == i) {
                        *gsum.entry(slot).or_insert(0.0) += c;
                    } else if c != 0.0 {
                        let d = f.in_lam[s];
                        if d <= 0.0 {
                            others_var = f64::INFINITY;
                            break;
                        }
                        others_var += c * c / d;
                        rest += c * f.in_eta[s] / d;
                    }
                }
                if others_var.is_infinite() || gsum.is_empty() {
                    continue;
                }
                let b = self.linear_b(fi) - rest;
                let w = 1.0 / others_var;
                let part = by_source
                    .entry(self.factors[fi].source.clone())
                    .or_insert_with(|| (vec![vec![0.0; n]; n], vec![0.0; n]));
                for (&a, &ga) in &gsum {
                    part.1[a] += w * ga * b;
                    for (&c, &gc) in &gsum {
                        part.0[a][c] += w * ga * gc;
                    }
                }
            }
            out.push((ji, by_source));
        }
        out
    }

    fn linear_b(&self, fi: usize) -> f64 {
        let f = &self.factors[fi];
        if f.kind != FKind::Nonlinear {
            return f.b;
        }
        let u0: Vec<f64> = f
            .vars
            .iter()
            .enumerate()
            .map(|(s, &i)| {
                let lam = f.in_lam[s] + f.out_lam[s];
                if lam > 0.0 {
                    (f.in_eta[s] + f.out_eta[s]) / lam
                } else {
                    self.x[i]
                }
            })
            .collect();
        let vals: Vec<f64> = u0
            .iter()
            .zip(&f.vars)
            .map(|(&u, &i)| if self.var_signed[i] { u } else { 10f64.powf(u.clamp(-300.0, 300.0)) })
            .collect();
        let mut r0 = -f.b;
        for (k, parts) in &f.terms {
            let mut t = *k;
            for &(s, p) in parts {
                t *= vals[s].powf(p);
            }
            r0 += t;
        }
        f.coef.iter().zip(&u0).map(|(g, u)| g * u).sum::<f64>() - r0
    }

    /// The weights keep what the inputs taught them, by source. Called before new inputs are heard, so each set of
    /// inputs is counted once.
    pub fn fold(&mut self) {
        for (ji, by_source) in self.evidence() {
            let f = &mut self.factors[ji];
            let mem = f.memory.as_mut().expect("a joint has a memory");
            let n = mem.names.len();
            for (source, (dl, dh)) in by_source {
                let part = mem.parts.entry(source).or_insert_with(|| (vec![vec![0.0; n]; n], vec![0.0; n]));
                for a in 0..n {
                    part.1[a] += dh[a];
                    for b in 0..n {
                        part.0[a][b] += dl[a][b];
                    }
                }
            }
            let (l, h) = mem.total();
            f.l = l;
            f.h = h;
            self.wake_factor(ji);
        }
        self.save_links();
    }

    /// Takes back everything a source taught the weights (a quarantine, a recall). Returns how many memories changed.
    pub fn unlearn(&mut self, source: &str) -> usize {
        let mut changed = 0;
        for ji in self.joints.clone() {
            let f = &mut self.factors[ji];
            let mem = f.memory.as_mut().unwrap();
            if mem.parts.shift_remove(source).is_some() {
                let (l, h) = mem.total();
                f.l = l;
                f.h = h;
                changed += 1;
                self.wake_factor(ji);
            }
        }
        self.save_links();
        changed
    }

    pub fn save_links(&mut self) {
        let mut links = Vec::new();
        for &ji in &self.joints {
            let mem = self.factors[ji].memory.as_ref().unwrap();
            for (source, (l, h)) in &mem.parts {
                links.push(Link { names: mem.names.clone(), source: source.clone(), l: l.clone(), h: h.clone() });
            }
        }
        self.net.links = links;
    }

    /// Sleep: every memory of weights loosens a little (its precision times factor): room to learn again.
    pub fn downscale(&mut self, factor: f64) {
        for ji in self.joints.clone() {
            let f = &mut self.factors[ji];
            let mem = f.memory.as_mut().unwrap();
            for (l, h) in mem.parts.values_mut() {
                for row in l.iter_mut() {
                    for v in row.iter_mut() {
                        *v *= factor;
                    }
                }
                for v in h.iter_mut() {
                    *v *= factor;
                }
            }
            let (l, h) = mem.total();
            f.l = l;
            f.h = h;
            self.wake_factor(ji);
        }
        self.save_links();
    }

    /// What every weight (`learn`) believes now: (value, sigma) inside the machine.
    pub fn beliefs(&self) -> IndexMap<String, (f64, f64)> {
        let mut out = IndexMap::new();
        for (name, spec) in &self.net.nodes {
            if let (true, Some(&i)) = (spec.learn, self.index.get(name)) {
                if self.lam[i] > 0.0 {
                    out.insert(name.clone(), (self.x[i], 1.0 / self.lam[i].sqrt()));
                }
            }
        }
        out
    }

    // ------------------------------------------------------------------ how well it holds
    /// How far relation f is from holding at the current values, in its own sigmas.
    pub fn residual(&self, fi: usize) -> f64 {
        let f = &self.factors[fi];
        let r = if f.kind == FKind::Nonlinear {
            let vals: Vec<f64> = f
                .vars
                .iter()
                .map(|&i| if self.var_signed[i] { self.x[i] } else { 10f64.powf(self.x[i].clamp(-300.0, 300.0)) })
                .collect();
            let mut r = -f.b;
            for (k, parts) in &f.terms {
                let mut t = *k;
                for &(s, p) in parts {
                    t *= vals[s].powf(p);
                }
                r += t;
            }
            r
        } else {
            f.vars.iter().zip(&f.coef).map(|(&i, c)| c * self.x[i]).sum::<f64>() - f.b
        };
        r * f.w.sqrt()
    }

    /// Prediction errors: how far each relation that hears the inputs is from holding, in its sigmas.
    pub fn surprise(&self) -> IndexMap<String, f64> {
        let inputs: Vec<usize> = self
            .net
            .nodes
            .iter()
            .filter(|(n, s)| s.input && self.index.contains_key(*n))
            .map(|(n, _)| self.index[n])
            .collect();
        let hidden: Vec<usize> = self
            .index
            .iter()
            .filter(|(n, _)| n.starts_with("drift_") && !n.ends_with("_before"))
            .map(|(_, &i)| i)
            .collect();
        let mut out = IndexMap::new();
        for (fi, f) in self.factors.iter().enumerate() {
            if matches!(f.kind, FKind::Relation | FKind::Nonlinear)
                && f.w > 0.0
                && (f.vars.iter().any(|i| inputs.contains(i))
                    || (f.vars.iter().any(|i| hidden.contains(i)) && f.name.ends_with("_law")))
            {
                if !f.watch.is_empty() && !f.active {
                    continue;
                }
                out.insert(f.name.clone(), self.residual(fi));
            }
        }
        out
    }

    /// How well it holds, from 0 to 1: its relations, and what it was told of the world (its measurements: a
    /// network that bends the world to keep its relations is not in harmony). Each counts by how close it is to
    /// holding (exp(-r^2/8), r in its sigmas). 1: all of them hold.
    pub fn harmony(&self) -> f64 {
        let mut total = 0.0;
        let mut n = 0.0;
        for (fi, f) in self.factors.iter().enumerate() {
            let measured = f.kind == FKind::Anchor
                && f.w > 0.0
                && f.name.strip_prefix("anchor ").is_some_and(|name| {
                    self.net.measured.contains_key(name) && !self.net.nodes.get(name).is_some_and(|s| s.learn || s.var)
                });
            if measured
                || (matches!(f.kind, FKind::Relation | FKind::Nonlinear)
                    && f.w > 0.0
                    && (f.watch.is_empty() || f.active))
            {
                let r = self.residual(fi).min(10.0);
                total += (-r * r / 8.0).exp();
                n += 1.0;
            }
        }
        if n > 0.0 {
            total / n
        } else {
            1.0
        }
    }

    // ------------------------------------------------------------------ changing the network without starting over
    /// A machine for a network that has changed, that starts from where this one is: every node and relation that
    /// is still there keeps its values and its messages, and only what is new (or changed) is woken. Adding a
    /// node does not make the whole network solve itself again.
    pub fn grown(&self, net: Network) -> Result<Machine> {
        let mut m = Machine::new(net)?;
        m.threads = self.threads;
        m.pool = self.pool.clone();
        m.budget = self.budget;
        m.tolerance = self.tolerance;
        m.rounds = self.rounds;
        m.messages = self.messages;
        m.heard = self.heard;
        m.seen = self.seen.clone();
        for (name, &i) in &m.index {
            if let Some(&j) = self.index.get(name) {
                if self.var_signed[j] == m.var_signed[i] {
                    m.x[i] = self.x[j];
                    m.lam[i] = self.lam[j];
                    m.eta[i] = self.eta[j];
                }
            }
        }
        let old: HashMap<&str, usize> = self.factors.iter().enumerate().map(|(k, f)| (f.name.as_str(), k)).collect();
        let mut woken = vec![true; m.factors.len()];
        for (fi, f) in m.factors.iter_mut().enumerate() {
            let Some(&k) = old.get(f.name.as_str()) else { continue };
            let o = &self.factors[k];
            let same_nodes = o.vars.len() == f.vars.len()
                && o.vars.iter().zip(&f.vars).all(|(a, b)| self.var_names[*a] == m.var_names[*b]);
            let same_law = o.kind == f.kind
                && o.strength == f.strength
                && o.watch.len() == f.watch.len()
                && (f.kind == FKind::Nonlinear
                    || f.kind == FKind::Convert
                    || (o.coef == f.coef && o.b == f.b && o.w == f.w))
                && o.terms == f.terms
                && o.bound == f.bound
                && o.l == f.l
                && o.h == f.h;
            if same_nodes && same_law {
                f.out_lam = o.out_lam.clone();
                f.out_eta = o.out_eta.clone();
                f.in_lam = o.in_lam.clone();
                f.in_eta = o.in_eta.clone();
                f.coef = o.coef.clone();
                f.b = o.b;
                f.w = o.w;
                f.active = o.active;
                f.damp = o.damp;
                f.swing = o.swing;
                f.trend = o.trend.clone();
                woken[fi] = false;
            }
        }
        // what is new speaks; the nodes it touches listen and answer
        m.set_dirty((0..m.factors.len()).filter(|f| woken[*f]).collect());
        let mut nodes = vec![false; m.var_names.len()];
        for &f in &m.dirty_factors {
            for &i in &m.factors[f].vars {
                nodes[i] = true;
            }
        }
        // a node that lost a relation hears it is gone
        for (name, &j) in &self.index {
            if let Some(&i) = m.index.get(name) {
                if self.node_factors[j].len() != m.node_factors[i].len() {
                    nodes[i] = true;
                }
            }
        }
        m.dirty_nodes = (0..m.var_names.len()).filter(|i| nodes[*i]).collect();
        Ok(m)
    }
}

// ---------------------------------------------------------------------- joint beliefs
impl Machine {
    /// The information matrix of the whole network as it is drawn now (each relation straight where it stands,
    /// each measurement, each memory of weights, each contact that pushes), row by row: (column, value).
    fn information(&self) -> Vec<Vec<(usize, f64)>> {
        let n = self.var_names.len();
        let mut rows: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
        for f in &self.factors {
            if !f.watch.is_empty() && !f.active && matches!(f.kind, FKind::Relation | FKind::Nonlinear) {
                continue;
            }
            match f.kind {
                FKind::Joint => {
                    for (a, &i) in f.vars.iter().enumerate() {
                        for (b, &j) in f.vars.iter().enumerate() {
                            *rows[i].entry(j).or_insert(0.0) += f.l[a][b];
                        }
                    }
                }
                FKind::Keep => {
                    if f.active {
                        *rows[f.vars[0]].entry(f.vars[0]).or_insert(0.0) += f.strength / (MIN_SIGMA * MIN_SIGMA);
                    }
                }
                _ => {
                    if f.w <= 0.0 {
                        continue;
                    }
                    for (a, &i) in f.vars.iter().enumerate() {
                        for (b, &j) in f.vars.iter().enumerate() {
                            *rows[i].entry(j).or_insert(0.0) += f.w * f.coef[a] * f.coef[b];
                        }
                    }
                }
            }
        }
        rows.into_iter().map(|r| r.into_iter().collect()).collect()
    }

    /// The joint belief of some nodes, from the whole network at once: their covariance (internal units: log10
    /// for a positive node, its own unit for a signed one), exact for the network drawn straight where it stands
    /// (belief propagation's own sigmas are not, around loops). Each column is solved by conjugate gradients.
    pub fn covariance(&self, names: &[&str]) -> Result<Vec<Vec<f64>>> {
        let idx: Vec<usize> = names
            .iter()
            .map(|n| self.index.get(*n).copied().ok_or_else(|| GrmError::new(format!("{n} is not a free node"))))
            .collect::<Result<_>>()?;
        let j = self.information();
        let n = j.len();
        let diag: Vec<f64> =
            (0..n).map(|i| j[i].iter().find(|(c, _)| *c == i).map(|(_, v)| *v).unwrap_or(0.0)).collect();
        let apply = |x: &[f64]| -> Vec<f64> { j.iter().map(|row| row.iter().map(|(c, v)| v * x[*c]).sum()).collect() };
        let mut cov = vec![vec![0.0; idx.len()]; idx.len()];
        for (col, &i) in idx.iter().enumerate() {
            // solve J z = e_i (preconditioned by the diagonal)
            let mut z = vec![0.0; n];
            let mut r = vec![0.0; n];
            r[i] = 1.0;
            let pre = |r: &[f64]| -> Vec<f64> {
                r.iter().zip(&diag).map(|(a, d)| if *d > 0.0 { a / d } else { *a }).collect()
            };
            let mut zr = pre(&r);
            let mut p = zr.clone();
            let mut rz: f64 = r.iter().zip(&zr).map(|(a, b)| a * b).sum();
            let first = rz;
            for _ in 0..(20 * n).max(100) {
                let ap = apply(&p);
                let pap: f64 = p.iter().zip(&ap).map(|(a, b)| a * b).sum();
                if !(pap > 0.0) {
                    break;
                }
                let alpha = rz / pap;
                for k in 0..n {
                    z[k] += alpha * p[k];
                    r[k] -= alpha * ap[k];
                }
                zr = pre(&r);
                let rz_new: f64 = r.iter().zip(&zr).map(|(a, b)| a * b).sum();
                if rz_new <= 1e-28 * first {
                    break;
                }
                let beta = rz_new / rz;
                rz = rz_new;
                for k in 0..n {
                    p[k] = zr[k] + beta * p[k];
                }
            }
            for (row, &k) in idx.iter().enumerate() {
                cov[row][col] = z[k];
            }
        }
        // the same number on both sides of the diagonal
        for a in 0..idx.len() {
            for b in 0..a {
                let m = 0.5 * (cov[a][b] + cov[b][a]);
                cov[a][b] = m;
                cov[b][a] = m;
            }
        }
        Ok(cov)
    }
}

// ---------------------------------------------------------------------- sleeping and waking
/// The state of every processor, as it is now: what `wake` puts back so that the machine goes on from where it
/// stopped, without solving anything again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineState {
    pub x: Vec<f64>,
    pub lam: Vec<f64>,
    pub eta: Vec<f64>,
    pub factors: Vec<FactorState>,
    pub dirty_nodes: Vec<usize>,
    pub dirty_factors: Vec<usize>,
    pub rounds: u64,
    pub messages: u64,
    pub settled: bool,
    pub heard: bool,
    pub seen: Option<Inputs>,
    pub last_surprise: IndexMap<String, f64>,
    pub threads: Option<usize>,
    pub cap: Option<usize>,
}

/// What one processor is in the middle of saying.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorState {
    pub name: String,
    pub out_lam: Vec<f64>,
    pub out_eta: Vec<f64>,
    pub in_lam: Vec<f64>,
    pub in_eta: Vec<f64>,
    pub coef: Vec<f64>,
    pub b: f64,
    pub w: f64,
    pub active: bool,
    pub damp: f64,
    pub swing: u32,
    pub trend: Vec<f64>,
}

impl Machine {
    /// Everything it is in the middle of (its network apart).
    pub fn state(&self) -> MachineState {
        MachineState {
            x: self.x.clone(),
            lam: self.lam.clone(),
            eta: self.eta.clone(),
            factors: self
                .factors
                .iter()
                .map(|f| FactorState {
                    name: f.name.clone(),
                    out_lam: f.out_lam.clone(),
                    out_eta: f.out_eta.clone(),
                    in_lam: f.in_lam.clone(),
                    in_eta: f.in_eta.clone(),
                    coef: f.coef.clone(),
                    b: f.b,
                    w: f.w,
                    active: f.active,
                    damp: f.damp,
                    swing: f.swing,
                    trend: f.trend.clone(),
                })
                .collect(),
            dirty_nodes: self.dirty_nodes.clone(),
            dirty_factors: self.dirty_factors.clone(),
            rounds: self.rounds,
            messages: self.messages,
            settled: self.settled,
            heard: self.heard,
            seen: self.seen.clone(),
            last_surprise: self.last_surprise.clone(),
            threads: self.threads,
            cap: self.budget.cap,
        }
    }

    /// A machine as it was put to sleep: the same network, the same messages, nothing solved again.
    pub fn wake(net: Network, st: MachineState) -> Result<Machine> {
        let mut m = Machine::new(net)?;
        if m.factors.len() != st.factors.len()
            || m.x.len() != st.x.len()
            || m.factors.iter().zip(&st.factors).any(|(f, s)| f.name != s.name || f.vars.len() != s.out_lam.len())
        {
            bail!("the sleeping state does not match its network");
        }
        for (f, s) in m.factors.iter_mut().zip(st.factors) {
            f.out_lam = s.out_lam;
            f.out_eta = s.out_eta;
            f.in_lam = s.in_lam;
            f.in_eta = s.in_eta;
            f.coef = s.coef;
            f.b = s.b;
            f.w = s.w;
            f.active = s.active;
            f.damp = s.damp;
            f.swing = s.swing;
            f.trend = s.trend;
        }
        m.x = st.x;
        m.lam = st.lam;
        m.eta = st.eta;
        m.dirty_nodes = st.dirty_nodes;
        m.set_dirty(st.dirty_factors);
        m.news = vec![f64::INFINITY; m.factors.len()];
        m.rounds = st.rounds;
        m.messages = st.messages;
        m.settled = st.settled;
        m.heard = st.heard;
        m.seen = st.seen;
        m.last_surprise = st.last_surprise;
        m.set_threads(st.threads);
        m.budget.cap = st.cap;
        Ok(m)
    }
}

// ---------------------------------------------------------------------- how a relation answers
fn holds(fac: &Factor, view: &View) -> bool {
    fac.watch.iter().all(|&(i, lo, hi)| {
        i >= 0 && {
            let v = view.value_si(i as usize);
            lo <= v && v <= hi
        }
    })
}

fn answer(fac: &mut Factor, view: &View) {
    if matches!(fac.kind, FKind::Relation | FKind::Nonlinear) && !fac.watch.is_empty() {
        fac.active = holds(fac, view);
        if !fac.active {
            fac.out_lam = vec![0.0; fac.vars.len()];
            fac.out_eta = vec![0.0; fac.vars.len()];
            return;
        }
    }
    match fac.kind {
        FKind::Nonlinear => redraw(fac, view),
        FKind::Joint => joint(fac),
        FKind::Convert => convert(fac, view),
        FKind::Keep => contact(fac, view),
        FKind::Relation => {
            let ol: Small = SmallVec::from_slice(&fac.out_lam);
            let oe: Small = SmallVec::from_slice(&fac.out_eta);
            linear(fac);
            for s in 0..fac.vars.len() {
                let (l0, e0, l1, e1) = (ol[s], oe[s], fac.out_lam[s], fac.out_eta[s]);
                if l0 > 0.0 && l1 > 0.0 {
                    let step = e1 / l1 - e0 / l0;
                    if step.abs() > QUIET + 1e-15 * (e1 / l1).abs() {
                        if step * fac.trend[s] < 0.0 {
                            fac.swing += 1;
                        }
                        fac.trend[s] = step;
                    }
                }
            }
            if fac.swing >= SWINGS {
                fac.damp = DAMPING;
            }
            if fac.damp > 0.0 {
                let d = fac.damp;
                for s in 0..fac.vars.len() {
                    fac.out_lam[s] = d * ol[s] + (1.0 - d) * fac.out_lam[s];
                    fac.out_eta[s] = d * oe[s] + (1.0 - d) * fac.out_eta[s];
                }
            }
        }
        FKind::Anchor => linear(fac),
    }
}

fn linear(fac: &mut Factor) {
    let k = fac.coef.len();
    let mut var: Small = SmallVec::with_capacity(k);
    let mut mean: Small = SmallVec::with_capacity(k);
    for s in 0..k {
        let (c, d) = (fac.coef[s], fac.in_lam[s]);
        if d > 0.0 && c != 0.0 {
            var.push(c * c / d);
            mean.push(c * fac.in_eta[s] / d);
        } else {
            var.push(if c != 0.0 { f64::INFINITY } else { 0.0 });
            mean.push(0.0);
        }
    }
    let (ov, om) = (others(&var), others(&mean));
    for s in 0..k {
        let c = fac.coef[s];
        let total = (if fac.w > 0.0 { 1.0 / fac.w } else { f64::INFINITY }) + ov[s];
        if c == 0.0 || total.is_infinite() {
            fac.out_lam[s] = 0.0;
            fac.out_eta[s] = 0.0;
        } else {
            let lam = c * c / total;
            fac.out_lam[s] = lam;
            fac.out_eta[s] = lam * (fac.b - om[s]) / c;
        }
    }
}

/// ~q = q, that is x = 10^y: drawn as a straight relation around the current y, three times.
fn convert(fac: &mut Factor, view: &View) {
    for _ in 0..3 {
        let lam = fac.in_lam[1] + fac.out_lam[1];
        let y0 = if lam > 0.0 { (fac.in_eta[1] + fac.out_eta[1]) / lam } else { view.x[fac.vars[1]] };
        let y0 = y0.clamp(-300.0, 300.0);
        let v = 10f64.powf(y0);
        fac.coef = vec![1.0, -v * LN10];
        fac.b = v * (1.0 - y0 * LN10);
        fac.w = fac.strength / (CONVERTER_SIGMA * v.max(1.0)).powi(2);
        linear(fac);
    }
}

/// A sum whose terms multiply free nodes or raise them to a power: drawn as a straight relation around where its
/// nodes are, three times, by the relation itself.
fn redraw(fac: &mut Factor, view: &View) {
    for _ in 0..3 {
        let u0: Vec<f64> = fac
            .vars
            .iter()
            .enumerate()
            .map(|(s, &i)| {
                let lam = fac.in_lam[s] + fac.out_lam[s];
                if lam > 0.0 {
                    (fac.in_eta[s] + fac.out_eta[s]) / lam
                } else {
                    view.x[i]
                }
            })
            .collect();
        let vals: Vec<f64> = u0
            .iter()
            .zip(&fac.vars)
            .map(|(&u, &i)| if view.signed[i] { u } else { 10f64.powf(u.clamp(-300.0, 300.0)) })
            .collect();
        let mut r0 = -fac.b;
        let mut grad = vec![0.0; fac.vars.len()];
        for (k, parts) in &fac.terms {
            let mut term = *k;
            for &(s, p) in parts {
                term *= vals[s].powf(p);
            }
            r0 += term;
            for &(s, p) in parts {
                let i = fac.vars[s];
                if view.signed[i] {
                    let mut others = k * p * (if vals[s] != 0.0 || p >= 1.0 { vals[s].powf(p - 1.0) } else { 0.0 });
                    for &(s2, p2) in parts {
                        if s2 != s {
                            others *= vals[s2].powf(p2);
                        }
                    }
                    grad[s] += others;
                } else {
                    grad[s] += term * p * LN10;
                }
            }
        }
        let b = grad.iter().zip(&u0).map(|(g, u)| g * u).sum::<f64>() - r0;
        fac.coef = grad;
        let saved = fac.b;
        fac.b = b;
        linear(fac);
        fac.b = saved;
    }
}

/// What a group of weights remembers, told to each weight given what the others hear.
fn joint(fac: &mut Factor) {
    let n = fac.vars.len();
    if n == 1 {
        fac.out_lam[0] = fac.l[0][0];
        fac.out_eta[0] = fac.h[0];
        return;
    }
    for s in 0..n {
        let o: Vec<usize> = (0..n).filter(|k| *k != s).collect();
        let m = o.len();
        let mut a = vec![vec![0.0; m + 2]; m];
        for (r, &i) in o.iter().enumerate() {
            for (c, &j) in o.iter().enumerate() {
                a[r][c] = fac.l[i][j] + if r == c { fac.in_lam[i] } else { 0.0 };
            }
            a[r][m] = fac.l[s][i];
            a[r][m + 1] = fac.h[i] + fac.in_eta[i];
        }
        match solve(&mut a, m, 2) {
            Some(sol) => {
                let jso: Vec<f64> = o.iter().map(|&i| fac.l[s][i]).collect();
                fac.out_lam[s] = fac.l[s][s] - jso.iter().zip(&sol).map(|(a, x)| a * x[0]).sum::<f64>();
                fac.out_eta[s] = fac.h[s] - jso.iter().zip(&sol).map(|(a, x)| a * x[1]).sum::<f64>();
            }
            None => {
                fac.out_lam[s] = 0.0;
                fac.out_eta[s] = 0.0;
            }
        }
    }
}

/// Solves A X = B (A m x m, B m x k, side by side in `a`) by elimination with partial pivoting.
fn solve(a: &mut [Vec<f64>], m: usize, k: usize) -> Option<Vec<Vec<f64>>> {
    for col in 0..m {
        let piv = (col..m)
            .max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap_or(std::cmp::Ordering::Equal))?;
        if a[piv][col] == 0.0 || !a[piv][col].is_finite() {
            return None;
        }
        a.swap(col, piv);
        for r in col + 1..m {
            let f = a[r][col] / a[col][col];
            if f != 0.0 {
                for c in col..m + k {
                    a[r][c] -= f * a[col][c];
                }
            }
        }
    }
    let mut x = vec![vec![0.0; k]; m];
    for r in (0..m).rev() {
        for c in 0..k {
            let mut v = a[r][m + c];
            for j in r + 1..m {
                v -= a[r][j] * x[j][c];
            }
            x[r][c] = v / a[r][r];
        }
    }
    Some(x)
}

/// keep u >= bound: pushes (holds u at the bound) only when the conditions it watches hold and what the other
/// relations say of u is beyond the bound.
fn contact(fac: &mut Factor, view: &View) {
    let ok = holds(fac, view);
    let d = fac.in_lam[0];
    let others = if d > 0.0 { fac.in_eta[0] / d } else { view.x[fac.vars[0]] };
    let beyond = match fac.op {
        KeepOp::AtLeast => others < fac.bound,
        KeepOp::AtMost => others > fac.bound,
    };
    fac.active = ok && beyond && fac.strength > 0.0;
    if fac.active {
        let lam = fac.strength / (MIN_SIGMA * MIN_SIGMA);
        fac.out_lam[0] = lam;
        fac.out_eta[0] = lam * fac.bound;
    } else {
        fac.out_lam[0] = 0.0;
        fac.out_eta[0] = 0.0;
    }
}
