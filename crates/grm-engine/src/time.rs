// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Time: nodes that remember, learn how they move, and foresee.
//!
//! - **Memory** (`Timeline`): what a node was, precise near the present and coarser further back. The most recent
//!   moments are kept one by one; older ones are merged two by two into spans twice as long, again and again: a
//!   memory that grows with the logarithm of time, each span keeping its mean and its spread. Inspired by the
//!   scale-invariant memory of time of the brain (Shankar and Howard, 2012; here a simpler pyramid, not their
//!   Laplace transform). **Rewind** reads it back: the value at any past moment, with how sure the memory is.
//! - **Dynamics**: each tracked node learns, in real time, how it moves: its next value from its last ones (an
//!   oscillator: two lags make a swing, its period and its damping), from the last values of the nodes it is
//!   coupled to (oscillators that talk to each other), and a constant. A Bayesian regression (normal-inverse-gamma)
//!   that forgets slowly, so it follows a world that changes.
//! - **Foresight** (`forecast`): the tracked nodes run forward together, many times over (an ensemble), each run
//!   drawing a law the node could have (it does not know its law exactly) and a noise: the spread of the runs is
//!   the uncertainty, widening with the horizon.
//! - **Why**: a confidence index, what it rests on (the share of noise and of ignorance of its law, each lag and
//!   coupling with what it contributes, the nodes and sources involved), and how well its past forecasts came true
//!   (a first metacognition: does it know how much it knows?).
//! - **A new connection**: for each tracked node, shadow models try the couplings it does not have yet. When one
//!   predicts what happens clearly better (sequential evidence: the log-likelihood of what came, before seeing
//!   it), the node takes it: a link found by itself.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use indexmap::IndexMap;

use crate::bail;
use crate::error::Result;
use crate::hidden::{Remembered, Rhythms, StateModel};
use rayon::prelude::*;

/// Moments kept one by one before being merged (and spans kept at each level).
pub const PER_LEVEL: usize = 48;
/// How slowly the dynamics forget (per moment): about 1 / (1 - FORGET) moments of memory weigh in its law.
pub const FORGET: f64 = 0.998;
/// The log-likelihood a coupling must gain over the node's own law before the node takes it (e^6: 400 to 1).
pub const INSIGHT: f64 = 6.0;
/// What the noise is believed to be before anything is seen (almost nothing: the data decide quickly).
pub const NOISE_PRIOR: f64 = 1e-6;
/// Moments a shadow model watches before it may be taken.
pub const SHADOW_WARMUP: usize = 48;

// ---------------------------------------------------------------------- memory
/// A stretch of time remembered as one: its mean and its spread.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub start: f64,
    pub end: f64,
    pub mean: f64,
    /// Sum of squared deviations from the mean (spread = sqrt(m2 / count)).
    pub m2: f64,
    pub count: u64,
}

impl Span {
    fn merge(a: &Span, b: &Span) -> Span {
        let n = (a.count + b.count) as f64;
        let d = b.mean - a.mean;
        let mean = a.mean + d * b.count as f64 / n;
        let m2 = a.m2 + b.m2 + d * d * a.count as f64 * b.count as f64 / n;
        Span { start: a.start.min(b.start), end: a.end.max(b.end), mean, m2, count: a.count + b.count }
    }

    pub fn spread(&self) -> f64 {
        if self.count > 1 {
            (self.m2 / self.count as f64).sqrt()
        } else {
            0.0
        }
    }
}

/// A node's memory: level 0 holds the last moments one by one; each level above holds spans twice as long.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    levels: Vec<VecDeque<Span>>,
    per_level: usize,
    pub total: Span,
}

impl Timeline {
    pub fn new(per_level: usize) -> Timeline {
        Timeline {
            levels: vec![VecDeque::new()],
            per_level: per_level.max(2) & !1,
            total: Span { start: 0.0, end: 0.0, mean: 0.0, m2: 0.0, count: 0 },
        }
    }

    pub fn push(&mut self, t: f64, value: f64) {
        let s = Span { start: t, end: t, mean: value, m2: 0.0, count: 1 };
        self.total = if self.total.count == 0 { s } else { Span::merge(&self.total, &s) };
        self.levels[0].push_back(s);
        let mut k = 0;
        while self.levels[k].len() > self.per_level {
            // the two oldest of this level become one span of the level above (older than everything here)
            let a = self.levels[k].pop_front().unwrap();
            let b = self.levels[k].pop_front().unwrap();
            if self.levels.len() == k + 1 {
                self.levels.push(VecDeque::new());
            }
            self.levels[k + 1].push_back(Span::merge(&a, &b));
            k += 1;
        }
    }

    /// Everything remembered, from the oldest to the newest.
    pub fn spans(&self) -> Vec<Span> {
        self.levels.iter().rev().flat_map(|l| l.iter().copied()).collect()
    }

    /// How many spans it holds (it grows with the logarithm of the moments seen).
    pub fn size(&self) -> usize {
        self.levels.iter().map(|l| l.len()).sum()
    }

    /// The value at a past moment, as remembered: (mean of the span that holds it, its spread). Between two
    /// spans: the nearest. None before the first moment or after the last.
    pub fn at(&self, t: f64) -> Option<(f64, f64)> {
        let spans = self.spans();
        let first = spans.first()?;
        let last = spans.last()?;
        if t < first.start || t > last.end {
            return None;
        }
        let best = spans
            .iter()
            .min_by(|a, b| {
                let da = if t < a.start {
                    a.start - t
                } else if t > a.end {
                    t - a.end
                } else {
                    0.0
                };
                let db = if t < b.start {
                    b.start - t
                } else if t > b.end {
                    t - b.end
                } else {
                    0.0
                };
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        Some((best.mean, best.spread()))
    }

    pub fn last(&self) -> Option<f64> {
        self.levels[0].back().map(|s| s.mean)
    }
}

// ---------------------------------------------------------------------- a little randomness, reproducible
/// xorshift64*: the same runs for the same seed.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }

    pub fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545F4914F6CDD1D);
        ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    pub fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// Gamma(shape, 1) (Marsaglia and Tsang).
    pub fn gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            return self.gamma(shape + 1.0) * self.uniform().powf(1.0 / shape);
        }
        let d = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let x = self.normal();
            let v = (1.0 + c * x).powi(3);
            if v > 0.0 && self.uniform().ln() < 0.5 * x * x + d - d * v + d * v.ln() {
                return d * v;
            }
        }
    }
}

// ---------------------------------------------------------------------- small linear algebra
pub(crate) fn solve_spd(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let l = cholesky(a)?;
    let n = b.len();
    let mut y = vec![0.0; n];
    for i in 0..n {
        y[i] = (b[i] - (0..i).map(|k| l[i][k] * y[k]).sum::<f64>()) / l[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (y[i] - (i + 1..n).map(|k| l[k][i] * x[k]).sum::<f64>()) / l[i][i];
    }
    Some(x)
}

pub(crate) fn cholesky(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let d = a[i][i] - s;
                if !(d > 0.0) {
                    return None;
                }
                l[i][j] = d.sqrt();
            } else {
                l[i][j] = (a[i][j] - s) / l[j][j];
            }
        }
    }
    Some(l)
}

pub(crate) fn inverse_spd(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut out = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        let col = solve_spd(a, &e)?;
        for i in 0..n {
            out[i][j] = col[i];
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------- dynamics
/// What a node's next value is made of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Feature {
    /// Its own value `k` moments before.
    Lag(usize),
    /// Another node's value the moment before.
    Coupling(String),
    Constant,
}

/// A law of motion learned in real time: next = w . features + noise, w and the noise unknown (normal-inverse-
/// gamma), forgetting slowly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dynamics {
    pub features: Vec<Feature>,
    /// Information (V^-1) and information vector of the weights, in units of the noise.
    info: Vec<Vec<f64>>,
    vec: Vec<f64>,
    prior_info: Vec<Vec<f64>>,
    prior_vec: Vec<f64>,
    /// The noise: inverse-gamma(a, b).
    a: f64,
    b: f64,
    pub seen: u64,
}

impl Dynamics {
    /// A law that starts believing that what was stays (its first lag weighs 1), knowing little.
    pub fn new(features: Vec<Feature>) -> Dynamics {
        let k = features.len();
        let mut prior_info = vec![vec![0.0; k]; k];
        let mut prior_mean = vec![0.0; k];
        for (i, f) in features.iter().enumerate() {
            prior_info[i][i] = 1e-2;
            if *f == Feature::Lag(1) {
                prior_mean[i] = 1.0;
            }
        }
        let prior_vec: Vec<f64> = (0..k).map(|i| prior_info[i][i] * prior_mean[i]).collect();
        Dynamics {
            features,
            info: prior_info.clone(),
            vec: prior_vec.clone(),
            prior_info,
            prior_vec,
            a: 1.0,
            b: NOISE_PRIOR,
            seen: 0,
        }
    }

    /// The same law without a feature (what it learned of the others is kept).
    pub fn without(&self, feature: &Feature) -> Dynamics {
        let Some(k) = self.features.iter().position(|f| f == feature) else { return self.clone() };
        let mut d = self.clone();
        d.features.remove(k);
        let drop = |m: &Vec<Vec<f64>>| -> Vec<Vec<f64>> {
            m.iter()
                .enumerate()
                .filter(|(i, _)| *i != k)
                .map(|(_, row)| row.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, v)| *v).collect())
                .collect()
        };
        d.info = drop(&self.info);
        d.prior_info = drop(&self.prior_info);
        d.vec.remove(k);
        d.prior_vec.remove(k);
        d
    }

    /// The same law with one more feature (what it learned is kept; the new weight starts at 0, knowing little).
    pub fn with(&self, feature: Feature) -> Dynamics {
        let k = self.features.len();
        let mut d = self.clone();
        d.features.insert(k - 1, feature); // before the constant
        let grow = |m: &Vec<Vec<f64>>, diag: f64| {
            let mut out = vec![vec![0.0; k + 1]; k + 1];
            let map = |i: usize| if i < k - 1 { i } else { i + 1 };
            for i in 0..k {
                for j in 0..k {
                    out[map(i)][map(j)] = m[i][j];
                }
            }
            out[k - 1][k - 1] = diag;
            out
        };
        let growv = |v: &Vec<f64>| {
            let mut out = v.clone();
            out.insert(k - 1, 0.0);
            out
        };
        d.info = grow(&self.info, 1e-2);
        d.prior_info = grow(&self.prior_info, 1e-2);
        d.vec = growv(&self.vec);
        d.prior_vec = growv(&self.prior_vec);
        d
    }

    pub fn mean(&self) -> Vec<f64> {
        solve_spd(&self.info, &self.vec).unwrap_or_else(|| vec![0.0; self.vec.len()])
    }

    /// The noise it believes in (its variance).
    pub fn noise(&self) -> f64 {
        if self.a > 1.0 {
            self.b / (self.a - 1.0)
        } else {
            self.b
        }
    }

    /// What it expects next, before seeing it: (mean, variance), its ignorance of its law included.
    pub fn predict(&self, x: &[f64]) -> (f64, f64) {
        let (m, s2, dof) = self.expect(x);
        (m, if dof > 2.0 { s2 * dof / (dof - 2.0) } else { s2 * 3.0 })
    }

    /// What it expects next as a Student-t: (mean, scale^2, degrees of freedom).
    pub fn expect(&self, x: &[f64]) -> (f64, f64, f64) {
        let m = self.mean();
        let mean: f64 = m.iter().zip(x).map(|(w, v)| w * v).sum();
        let vx = solve_spd(&self.info, x).unwrap_or_else(|| vec![0.0; x.len()]);
        let q: f64 = x.iter().zip(&vx).map(|(a, b)| a * b).sum();
        (mean, self.b / self.a * (1.0 + q), 2.0 * self.a)
    }

    /// Learns from one moment: features x gave y.
    pub fn learn(&mut self, x: &[f64], y: f64) {
        // forget a little: back toward the prior, so a world that changes is followed
        for i in 0..self.info.len() {
            for j in 0..self.info.len() {
                self.info[i][j] = FORGET * (self.info[i][j] - self.prior_info[i][j]) + self.prior_info[i][j];
            }
            self.vec[i] = FORGET * (self.vec[i] - self.prior_vec[i]) + self.prior_vec[i];
        }
        self.a = FORGET * (self.a - 1.0) + 1.0;
        self.b = FORGET * (self.b - NOISE_PRIOR) + NOISE_PRIOR;
        let m = self.mean();
        let vx = solve_spd(&self.info, x).unwrap_or_else(|| vec![0.0; x.len()]);
        let q: f64 = x.iter().zip(&vx).map(|(a, b)| a * b).sum();
        let r = y - m.iter().zip(x).map(|(w, v)| w * v).sum::<f64>();
        self.a += 0.5;
        self.b += 0.5 * r * r / (1.0 + q);
        for i in 0..x.len() {
            for j in 0..x.len() {
                self.info[i][j] += x[i] * x[j];
            }
            self.vec[i] += x[i] * y;
        }
        self.seen += 1;
    }

    /// A law it could have, drawn from what it believes: (weights, noise variance).
    fn draw(&self, rng: &mut Rng) -> (Vec<f64>, f64) {
        let s2 = self.b / rng.gamma(self.a).max(1e-300);
        let m = self.mean();
        let cov = inverse_spd(&self.info).unwrap_or_else(|| vec![vec![0.0; m.len()]; m.len()]);
        let l = cholesky(&cov).unwrap_or_else(|| vec![vec![0.0; m.len()]; m.len()]);
        let z: Vec<f64> = (0..m.len()).map(|_| rng.normal()).collect();
        let w = (0..m.len()).map(|i| m[i] + s2.sqrt() * (0..=i).map(|k| l[i][k] * z[k]).sum::<f64>()).collect();
        (w, s2)
    }

    /// Its swing, when its two first lags make one: (period in moments, what it keeps of itself each moment).
    pub fn swing(&self) -> Option<(f64, f64)> {
        let m = self.mean();
        let a1 = self.features.iter().position(|f| *f == Feature::Lag(1)).map(|i| m[i])?;
        let a2 = self.features.iter().position(|f| *f == Feature::Lag(2)).map(|i| m[i])?;
        let disc = a1 * a1 + 4.0 * a2;
        if disc >= 0.0 || a2 >= 0.0 {
            return None;
        }
        let r = (-a2).sqrt();
        let theta = (a1 / (2.0 * r)).clamp(-1.0, 1.0).acos();
        if theta <= 0.0 {
            return None;
        }
        Some((std::f64::consts::TAU / theta, r))
    }
}

// ---------------------------------------------------------------------- tracked nodes
/// How well its forecasts came true: of its one-step forecasts, how many fell within one sigma.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub forecasts: u64,
    pub within_one_sigma: u64,
    pub sum_z2: f64,
}

impl Calibration {
    pub fn coverage(&self) -> Option<f64> {
        (self.forecasts > 0).then(|| self.within_one_sigma as f64 / self.forecasts as f64)
    }

    /// In words: does it know how much it knows? (68 % within one sigma when it does)
    pub fn text(&self) -> String {
        match self.coverage() {
            None => "no forecast has come true or false yet".into(),
            Some(c) => {
                let verdict = if self.forecasts < 20 {
                    "too few to judge"
                } else if c < 0.55 {
                    "it is surer of itself than it should be"
                } else if c > 0.8 {
                    "it is less sure of itself than it could be"
                } else {
                    "it knows how much it knows"
                };
                format!(
                    "{:.0} % of its last {} forecasts fell within one sigma (68 % expected): {verdict}",
                    c * 100.0,
                    self.forecasts
                )
            }
        }
    }
}

/// A coupling being tried in the shadow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub with: String,
    pub model: Dynamics,
    /// Log-likelihood of what came, before seeing it: the shadow minus the node's own law.
    pub gain: f64,
    pub watched: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracked {
    pub name: String,
    pub memory: Timeline,
    /// Its law learned straight on what was measured: quick to learn, what its shadows try links with.
    pub model: Dynamics,
    /// What it really is behind what is measured, once it has seen enough: its hidden state and its law.
    pub hidden: Option<StateModel>,
    /// The moments it keeps one by one to consolidate its hidden law: (when, what was measured, the values of
    /// every tracked node the moment before: a link found later can be learned on the past too).
    window: VecDeque<(f64, Option<f64>, IndexMap<String, f64>)>,
    since_fit: usize,
    /// How many times it has consolidated (every so often it searches its law afresh, not only from what it
    /// believed: a belief can be a dead end).
    consolidations: usize,
    /// Its last values, newest first.
    recent: VecDeque<f64>,
    lags: usize,
    pending: Option<(f64, f64)>,
    pub calibration: Calibration,
    pub shadows: Vec<Shadow>,
    /// The links it once found and no longer believes in (its shadows do not try them again).
    pub doubted: Vec<String>,
}

impl Tracked {
    /// What drives it, as its hidden law reads it: other nodes, and its cycles (as cosine and sine).
    fn couplings(&self) -> Vec<String> {
        self.model
            .features
            .iter()
            .filter_map(|f| if let Feature::Coupling(n) = f { Some(n.clone()) } else { None })
            .collect()
    }

    /// How many times it has consolidated its hidden law.
    pub fn consolidations(&self) -> usize {
        self.consolidations
    }

    /// Its links as they are found and doubted: other nodes, and cycles (one each).
    fn links(&self) -> Vec<String> {
        let mut out = vec![];
        for c in self.couplings() {
            if let Some(period) = c.strip_prefix(COS) {
                out.push(format!("{CYCLE}{period}"));
            } else if !c.starts_with(SIN) {
                out.push(c);
            }
        }
        out
    }

    /// The nodes it rests on (its cycles are not nodes).
    fn coupled_nodes(&self) -> Vec<String> {
        self.couplings().into_iter().filter(|c| !c.starts_with('@')).collect()
    }

    /// Its swing: from its hidden law when it has one, else from the law learned on its measurements.
    pub fn swing(&self) -> Option<(f64, f64)> {
        match &self.hidden {
            Some(h) if h.p >= 2 => swing_of(h.theta[0], h.theta[1]),
            _ => self.model.swing(),
        }
    }
}

/// The swing of a law whose two first lags weigh a1 and a2: (period in moments, what it keeps of itself).
pub fn swing_of(a1: f64, a2: f64) -> Option<(f64, f64)> {
    if a1 * a1 + 4.0 * a2 >= 0.0 || a2 >= 0.0 {
        return None;
    }
    let r = (-a2).sqrt();
    let theta = (a1 / (2.0 * r)).clamp(-1.0, 1.0).acos();
    (theta > 0.0).then(|| (std::f64::consts::TAU / theta, r))
}

/// A moment the network realised something (a link found by itself).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    pub node: String,
    pub with: String,
    pub gain: f64,
    pub t: f64,
    /// A link found, or a link it no longer believes in.
    pub revised: bool,
}

impl Insight {
    /// In words, as the logbook says it.
    pub fn text(&self) -> String {
        match (self.with.strip_prefix(CYCLE), self.revised) {
            (Some(p), false) => format!("{} comes back every {p} moments: a cycle found by itself (evidence e^{:.0})", self.node, self.gain),
            (Some(p), true) => format!("{} does not come back every {p} moments after all: once its senses are told apart, the cycle holds no weight", self.node),
            (None, false) => format!("{} follows {}: a link found by itself (evidence e^{:.0}: it predicts, it is not yet a cause)", self.node, self.with, self.gain),
            (None, true) => format!("{} does not follow {} after all: once its senses are told apart, the link holds no weight (an echo of its senses)", self.node, self.with),
        }
    }
}

/// What one tracked node is foreseen to be, moment after moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub mean: Vec<f64>,
    pub sigma: Vec<f64>,
    pub low: Vec<f64>,
    pub high: Vec<f64>,
}

/// A forecast of every tracked node, with its explanation.
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub start: f64,
    pub step: f64,
    pub paths: IndexMap<String, Path>,
}

/// What a run of the forecast draws (each one off: its mean, or nothing).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Draws {
    /// A law it could have.
    law: bool,
    /// The world's noise, moment after moment.
    world: bool,
    /// The senses' noise (what will be measured, not what will be).
    senses: bool,
    /// Where it is now (it is not sure of its state).
    state: bool,
}

const ALL: Draws = Draws { law: true, world: true, senses: true, state: true };

/// Why a forecast of a node is what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Why {
    pub node: String,
    pub horizon: usize,
    pub value: f64,
    pub sigma: f64,
    /// From 0 (no better than knowing how much it usually varies) to 1 (sure).
    pub confidence: f64,
    /// Where its uncertainty comes from, with each share: the world moving, its senses, what it does not know of
    /// its own law, where it is now.
    pub shares: Vec<(String, f64)>,
    /// The noises it has learned apart (sigmas), when it has a hidden state: (world, senses).
    pub noises: Option<(f64, f64)>,
    /// What each part of its law contributes to its next value: (part, contribution).
    pub contributions: Vec<(String, f64)>,
    /// Every tracked node the forecast rests on (itself, what it is coupled to, and so on).
    pub nodes: Vec<String>,
    pub swing: Option<(f64, f64)>,
    pub calibration: Calibration,
}

impl Why {
    pub fn text(&self, sources: &IndexMap<String, String>) -> String {
        let mut out = vec![format!(
            "{} in {} moment(s): {:.4} ± {:.2} — confidence {:.2}",
            self.node, self.horizon, self.value, self.sigma, self.confidence
        )];
        if self.shares.iter().all(|(_, s)| s.is_finite()) {
            let shares: Vec<String> =
                self.shares.iter().map(|(what, s)| format!("{:.0} % {what}", s * 100.0)).collect();
            out.push(format!("  its uncertainty: {}", shares.join(", ")));
        } else {
            out.push("  where its uncertainty comes from: not known yet (it has not remembered enough)".into());
        }
        if let Some((w, v)) = self.noises {
            out.push(format!("  it tells the world from its senses: the world moves by ± {w:.3} a moment, its senses err by ± {v:.3}"));
        }
        if let Some((p, r)) = self.swing {
            out.push(format!("  it swings: back every {p:.3} moments, keeping {r:.4} of itself each moment"));
        }
        out.push("  its next value is made of:".into());
        for (part, c) in &self.contributions {
            out.push(format!("    {part}: {c:+.4}"));
        }
        let nodes: Vec<String> = self
            .nodes
            .iter()
            .map(|n| match sources.get(n) {
                Some(s) => format!("{n} ({s})"),
                None => n.clone(),
            })
            .collect();
        out.push(format!("  it rests on: {}", nodes.join(", ")));
        out.push(format!("  {}", self.calibration.text()));
        out.join("\n")
    }
}

/// Where a consolidation starts from: (law, world's noise, senses' noise).
type Start = (Vec<f64>, f64, f64);

/// Moments a node must have seen before it learns its hidden state.
pub const MIN_HIDDEN: usize = 64;
/// Moments it keeps one by one to consolidate its hidden law.
pub const WINDOW: usize = 480;
/// Every how many moments it consolidates its hidden law.
pub const REFIT: usize = 24;

/// The nodes that live in time: their memories, their laws, their forecasts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Timekeeper {
    pub nodes: IndexMap<String, Tracked>,
    /// The last value of every tracked node (what couplings read).
    last: IndexMap<String, f64>,
    pub t: f64,
    pub insights: Vec<Insight>,
    /// Whether shadow models look for new connections by themselves.
    pub curious: bool,
    /// Whether nodes learn a hidden state (to tell the world from their senses).
    pub hidden_states: bool,
    /// What its moments are counted in: "tick", "hour" (since 1970), "year"...
    pub unit: String,
    /// How long a moment lasts (the time between the last two).
    pub dt: f64,
}

impl Timekeeper {
    pub fn new() -> Timekeeper {
        Timekeeper {
            curious: true,
            hidden_states: true,
            unit: "tick".into(),
            dt: 1.0,
            t: f64::NEG_INFINITY,
            ..Default::default()
        }
    }

    /// A moment as a human reads it.
    pub fn when(&self, t: f64) -> String {
        moment_text(t, &self.unit)
    }

    /// A node starts living in time: a memory, and a law with `lags` lags of itself (2: it can swing) and the
    /// couplings given.
    pub fn track(&mut self, name: &str, lags: usize, couplings: &[&str]) {
        let mut features: Vec<Feature> = (1..=lags.max(1)).map(Feature::Lag).collect();
        features.extend(couplings.iter().map(|c| Feature::Coupling(c.to_string())));
        features.push(Feature::Constant);
        self.nodes.insert(
            name.to_string(),
            Tracked {
                name: name.to_string(),
                memory: Timeline::new(PER_LEVEL),
                model: Dynamics::new(features),
                hidden: None,
                window: VecDeque::new(),
                since_fit: 0,
                consolidations: 0,
                recent: VecDeque::new(),
                lags: lags.max(1),
                pending: None,
                calibration: Calibration::default(),
                shadows: vec![],
                doubted: vec![],
            },
        );
    }

    /// What drives a hidden state at moment t: the couplings' values the moment before, and its cycles.
    fn inputs(couplings: &[String], last: &IndexMap<String, f64>, t: f64) -> Vec<f64> {
        couplings.iter().map(|c| input_value(c, last, t).unwrap_or(0.0)).collect()
    }

    /// Consolidates a node's hidden law on the moments it keeps one by one. It starts from what it believed
    /// (its hidden law, a new coupling at 0) and from the law learned on its measurements, and keeps the law that
    /// explains best what it remembers.
    fn consolidate(node: &mut Tracked) {
        let all = node.couplings();
        // what drives its state (other nodes), and the rhythms it is measured with (its cycles)
        let couplings: Vec<String> = all.iter().filter(|c| !c.starts_with('@')).cloned().collect();
        let periods: Vec<f64> = all.iter().filter_map(|c| c.strip_prefix(COS)).filter_map(|p| p.parse().ok()).collect();
        let p = node.lags.max(1);
        let ys: Vec<Option<f64>> = node.window.iter().map(|(_, y, _)| *y).collect();
        let us: Vec<Vec<f64>> = node.window.iter().map(|(_, _, prev)| Self::inputs(&couplings, prev, 0.0)).collect();
        let ts: Vec<f64> = node.window.iter().map(|(t, _, _)| *t).collect();
        let half = 0.5 * node.model.noise().max(1e-12);
        // the law learned straight on the measurements, as the hidden law is: [lags.., couplings.., constant]
        let mean = node.model.mean();
        let mut straight: Vec<f64> = vec![];
        for (k, f) in node.model.features.iter().enumerate() {
            match f {
                Feature::Coupling(c) if c.starts_with('@') => {}
                _ => straight.push(mean[k]),
            }
        }
        let mut starts: Vec<(Start, Rhythms, usize)> = vec![];
        let fresh = |h: Option<&StateModel>| {
            let mut r = Rhythms::new(&periods);
            if let Some(h) = h {
                for (i, p) in periods.iter().enumerate() {
                    if let Some(j) = h.rhythms.periods.iter().position(|q| q == p) {
                        r.beta[2 * i] = h.rhythms.beta[2 * j];
                        r.beta[2 * i + 1] = h.rhythms.beta[2 * j + 1];
                    }
                }
            }
            r
        };
        let same =
            node.hidden.as_ref().is_some_and(|h| h.p == p && h.couplings == couplings && h.rhythms.periods == periods);
        if let Some(h) = node.hidden.as_ref().filter(|h| h.p == p) {
            let mut th = h.theta[..p].to_vec();
            th.extend(
                couplings
                    .iter()
                    .map(|c| h.couplings.iter().position(|x| x == c).map(|j| h.theta[p + j]).unwrap_or(0.0)),
            );
            th.push(h.theta[h.theta.len() - 1]);
            // what it knew only needs adjusting; a new link deserves a longer search
            starts.push(((th, h.q, h.r), fresh(Some(h)), if same { 8 } else { 40 }));
        }
        // afresh: when something changed, and every tenth time
        if !same || node.consolidations.is_multiple_of(10) {
            starts.push(((straight.clone(), half, half), fresh(None), 40));
            let mut alone = straight.clone();
            for j in 0..couplings.len() {
                alone[p + j] = 0.0;
            }
            starts.push(((alone, half, half), fresh(None), 40));
        }
        let remembered = Remembered { ys: &ys, us: &us, ts: &ts };
        let best = starts
            .into_iter()
            .filter_map(|(start, rhythms, iterations)| {
                StateModel::fit(p, &couplings, remembered, start, rhythms, iterations)
            })
            .max_by(|a, b| a.loglik.partial_cmp(&b.loglik).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(h) = best {
            node.hidden = Some(h);
        }
        node.since_fit = 0;
        node.consolidations += 1;
    }

    /// A node consolidates its hidden law, then thinks again about its links: does the hidden law explain what
    /// it remembers clearly better with a link than without? (by more than half the log of the moments per weight
    /// it adds, as the Bayesian information criterion asks: a link must pay for itself). A new link at once; all
    /// its links every tenth time; and any link its hidden law gives no weight to.
    fn think(node: &mut Tracked, name: &str, accepted: Option<&String>, t: f64) -> Vec<Insight> {
        let mut found = vec![];
        let name = name.to_string();
        Self::consolidate(node);
        // critical thinking: does the hidden law explain what it remembers clearly better with a link than
        // without? (by more than half the log of the moments per weight it adds, as the Bayesian information
        // criterion asks: a link must pay for itself). A new link at once; all its links every tenth time.
        let checks: Vec<String> = match accepted {
            Some(link) => vec![link.clone()],
            None if node.consolidations % 10 == 1 => node.links(),
            None => vec![],
        };
        for link in checks {
            let Some(with) = node.hidden.as_ref().map(|h| h.loglik) else { break };
            let mut alt = node.clone();
            alt.model = shrink(&alt.model, &link);
            Self::consolidate(&mut alt);
            let weights = if link.starts_with(CYCLE) { 2.0 } else { 1.0 };
            let price = 0.5 * weights * (node.window.len() as f64).ln();
            if let Some(without) = alt.hidden.as_ref().map(|h| h.loglik) {
                if with - without < price {
                    let shadows = std::mem::take(&mut node.shadows);
                    *node = alt;
                    node.shadows = shadows;
                    node.doubted.push(link.clone());
                    node.shadows.retain(|s| !near(&s.with, &link));
                    found.push(Insight {
                        node: name.clone(),
                        with: link.clone(),
                        gain: with - without,
                        t,
                        revised: true,
                    });
                }
            }
        }
        // critical thinking, later: a link its hidden law gives no weight to (less than two sigmas of what it
        // knows of it) was an echo of its senses, not a link of the world: it stops believing in it
        let doubted = Self::unfounded(node);
        for c in &doubted {
            node.model = shrink(&node.model, c);
            node.doubted.push(c.clone());
            found.push(Insight { node: name.clone(), with: c.clone(), gain: 0.0, t, revised: true });
        }
        if !doubted.is_empty() {
            Self::consolidate(node);
            node.shadows.retain(|s| !doubted.contains(&s.with));
            for other in node.shadows.iter_mut() {
                other.model = extend(&node.model, &other.with);
                other.gain = 0.0;
                other.watched = 0;
            }
        }
        found
    }

    /// The links its hidden law gives no weight to: a coupling within two sigmas of nothing; a cycle whose two
    /// weights together are within what chance gives (chi-square of 2 under 6: 95 %).
    fn unfounded(node: &Tracked) -> Vec<String> {
        let Some(h) = &node.hidden else { return vec![] };
        let mut out = vec![];
        for (j, c) in h.couplings.iter().enumerate() {
            let k = h.p + j;
            if h.theta[k].abs() < 2.0 * h.cov_theta[k][k].max(0.0).sqrt() {
                out.push(c.clone());
            }
        }
        for (i, period) in h.rhythms.periods.iter().enumerate() {
            let (a, b) = (2 * i, 2 * i + 1);
            let cov =
                vec![vec![h.rhythms.cov[a][a], h.rhythms.cov[a][b]], vec![h.rhythms.cov[b][a], h.rhythms.cov[b][b]]];
            let w = [h.rhythms.beta[a], h.rhythms.beta[b]];
            let chi2 = solve_spd(&cov, &w).map(|x| x[0] * w[0] + x[1] * w[1]).unwrap_or(f64::INFINITY);
            if chi2 < 6.0 {
                out.push(format!("{CYCLE}{period}"));
            }
        }
        out
    }

    /// A moment: the values of the tracked nodes at time t. Each node checks its last forecast, learns, remembers,
    /// follows its hidden state; its shadows learn too, and a shadow that has clearly done better becomes its law
    /// (an insight). A node with no value at this moment is not measured: its hidden state goes on, less sure.
    pub fn record(&mut self, t: f64, values: &IndexMap<String, f64>) -> Vec<Insight> {
        let mut found = vec![];
        let mut thinking: Vec<(String, Option<String>)> = vec![];
        let names: Vec<String> = self.nodes.keys().cloned().collect();
        let last = self.last.clone();
        for name in &names {
            let y = values.get(name).copied().filter(|y| y.is_finite());
            let mut node = self.nodes[name].clone();
            if let Some(y) = y {
                if let Some((m, v)) = node.pending.take() {
                    let z = (y - m) / v.sqrt().max(1e-300);
                    node.calibration.forecasts += 1;
                    node.calibration.sum_z2 += z * z;
                    if z.abs() <= 1.0 {
                        node.calibration.within_one_sigma += 1;
                    }
                }
                if let Some(x) = features_of(&node.model, &node.recent, &last, t) {
                    let (m, s2, dof) = node.model.expect(&x);
                    let own = log_student(y, m, s2, dof);
                    for sh in node.shadows.iter_mut() {
                        if let Some(xs) = features_of(&sh.model, &node.recent, &last, t) {
                            let (ms, ss2, sdof) = sh.model.expect(&xs);
                            sh.gain += log_student(y, ms, ss2, sdof) - own;
                            sh.watched += 1;
                            sh.model.learn(&xs, y);
                        }
                    }
                    node.model.learn(&x, y);
                }
            }
            // its hidden state goes on, and what was measured (if anything) corrects it
            let mut refit = false;
            let mut accepted: Option<String> = None;
            if let Some(h) = node.hidden.as_mut() {
                let hu = Self::inputs(&h.couplings, &last, t);
                h.step(y, &hu, t);
            }
            node.window.push_back((t, y, last.clone()));
            while node.window.len() > WINDOW {
                node.window.pop_front();
            }
            node.since_fit += 1;
            // an insight: a coupling that predicts clearly better becomes part of its law
            if let Some(k) = node.shadows.iter().position(|s| s.watched >= SHADOW_WARMUP && s.gain > INSIGHT) {
                let sh = node.shadows.remove(k);
                // the link joins its law as it is now (a shadow made earlier may hold links since doubted)
                node.model = extend(&node.model, &sh.with);
                for other in node.shadows.iter_mut() {
                    other.model = extend(&node.model, &other.with);
                    other.gain = 0.0;
                    other.watched = 0;
                }
                // its hidden law is learned again at once, with the link, on the moments it keeps
                refit = true;
                accepted = Some(sh.with.clone());
                found.push(Insight { node: name.clone(), with: sh.with, gain: sh.gain, t, revised: false });
            }
            node.shadows.retain(|s| s.gain > -INSIGHT * 4.0);
            if self.hidden_states
                && node.window.len() >= MIN_HIDDEN
                && (node.hidden.is_none() || node.since_fit >= REFIT || refit)
            {
                thinking.push((name.clone(), accepted));
            }
            if let Some(y) = y {
                node.memory.push(t, y);
                node.recent.push_front(y);
                node.recent.truncate(node.lags.max(2));
            }
            self.nodes.insert(name.clone(), node);
        }
        // consolidating and thinking again are the long part: each node does it on its own, all at once (the
        // numbers are the same whatever the cores: each node only reads itself)
        let mut minds: Vec<(String, Option<String>, Tracked)> = thinking
            .into_iter()
            .map(|(n, a)| {
                let node = self.nodes[&n].clone();
                (n, a, node)
            })
            .collect();
        let thought: Vec<Vec<Insight>> =
            minds.par_iter_mut().map(|(name, accepted, node)| Self::think(node, name, accepted.as_ref(), t)).collect();
        for ((name, _, node), insights) in minds.into_iter().zip(thought) {
            self.nodes.insert(name, node);
            found.extend(insights);
        }
        for name in &names {
            if let Some(&y) = values.get(name).filter(|y| y.is_finite()) {
                self.last.insert(name.clone(), y);
            }
        }
        // the next forecast of each node (checked at the next moment), and new shadows to try: the other nodes,
        // and the cycles its past seems to hold
        if t > self.t && self.t.is_finite() {
            self.dt = t - self.t;
        }
        let next = t + self.dt;
        for name in &names {
            let node = &self.nodes[name];
            let pending = match &node.hidden {
                Some(h) => Some(h.next_measure(&Self::inputs(&h.couplings, &self.last, next), next)),
                None => features_of(&node.model, &node.recent, &self.last, next).map(|x| node.model.predict(&x)),
            };
            let mut shadows = vec![];
            if self.curious {
                let have: Vec<String> = node.links();
                let mut candidates: Vec<String> = names.iter().filter(|o| *o != name).cloned().collect();
                if node.since_fit == 0 || node.window.len() == MIN_HIDDEN {
                    candidates.extend(cycles(node, &self.unit, self.dt).into_iter().map(|p| format!("{CYCLE}{p}")));
                }
                for other in candidates {
                    if !have.contains(&other)
                        && !doubts(&node.doubted, &other)
                        && !have.iter().any(|h| near(h, &other))
                        && !node.shadows.iter().any(|s| s.with == other)
                    {
                        shadows.push(Shadow { model: extend(&node.model, &other), with: other, gain: 0.0, watched: 0 });
                    }
                }
            }
            let node = self.nodes.get_mut(name).unwrap();
            node.pending = pending;
            node.shadows.extend(shadows);
        }
        self.t = t;
        self.insights.extend(found.iter().cloned());
        found
    }

    /// What a node expects to be measured next: (mean, variance).
    pub fn expected(&self, name: &str) -> Option<(f64, f64)> {
        self.nodes.get(name).and_then(|n| n.pending)
    }

    /// The value of a node at a past moment, as remembered: (value, spread).
    pub fn rewind(&self, name: &str, t: f64) -> Result<(f64, f64)> {
        let Some(node) = self.nodes.get(name) else { bail!("{name} does not live in time (track it first)") };
        match node.memory.at(t) {
            Some(v) => Ok(v),
            None => bail!("{name} remembers nothing at {t}"),
        }
    }

    /// Runs every tracked node forward `steps` moments, `runs` times. Each run draws what `draws` says (else its
    /// mean, or no noise). A node with a hidden state runs its state, and what would be measured of it is its
    /// state plus its senses' noise; the couplings read the states of the others.
    fn ensemble(&self, steps: usize, runs: usize, draws: Draws, seed: u64) -> IndexMap<String, Vec<Vec<f64>>> {
        let mut rng = Rng::new(seed);
        let names: Vec<String> = self.nodes.keys().cloned().collect();
        let mut out: IndexMap<String, Vec<Vec<f64>>> =
            names.iter().map(|n| (n.clone(), vec![vec![0.0; runs]; steps])).collect();
        for r in 0..runs {
            // for each node: (its law, its world noise, its state or its recent values)
            let mut laws: Vec<(Vec<f64>, f64)> = vec![];
            let mut rhythms: Vec<Vec<f64>> = vec![];
            let mut states: Vec<Vec<f64>> = vec![];
            let mut recent: Vec<VecDeque<f64>> = vec![];
            for n in &names {
                let node = &self.nodes[n];
                match &node.hidden {
                    Some(h) => {
                        let law = h.law(if draws.law { Some(&mut rng) } else { None });
                        laws.push((law, h.q));
                        rhythms.push(if draws.law { h.rhythms.draw(&mut rng) } else { h.rhythms.beta.clone() });
                        states.push(h.state(if draws.state { Some(&mut rng) } else { None }));
                    }
                    None => {
                        let m = &node.model;
                        laws.push(if draws.law { m.draw(&mut rng) } else { (m.mean(), m.noise()) });
                        states.push(vec![]);
                        rhythms.push(vec![]);
                    }
                }
                recent.push(node.recent.clone());
            }
            let mut last: IndexMap<String, f64> = self.last.clone();
            for (i, n) in names.iter().enumerate() {
                if let (Some(s), Some(h)) = (states[i].first(), &self.nodes[n].hidden) {
                    last.insert(n.clone(), *s + h.rhythms.at(self.t, Some(&rhythms[i])));
                }
            }
            for h in 0..steps {
                let at = self.t + (h + 1) as f64 * self.dt;
                let mut next = vec![0.0; names.len()];
                let mut measured = vec![0.0; names.len()];
                for (i, n) in names.iter().enumerate() {
                    let node = &self.nodes[n];
                    let (law, q) = &laws[i];
                    if let Some(hm) = &node.hidden {
                        let u = Self::inputs(&hm.couplings, &last, at);
                        let w = if draws.world { q.sqrt() * rng.normal() } else { 0.0 };
                        states[i] = StateModel::advance(law, hm.p, &states[i], &u, w);
                        next[i] = states[i][0] + hm.rhythms.at(at, Some(&rhythms[i]));
                        measured[i] = next[i] + if draws.senses { hm.r.sqrt() * rng.normal() } else { 0.0 };
                    } else {
                        let Some(x) = features_of(&node.model, &recent[i], &last, at) else {
                            next[i] = recent[i].front().copied().unwrap_or(f64::NAN);
                            measured[i] = next[i];
                            continue;
                        };
                        let mut v: f64 = law.iter().zip(&x).map(|(a, b)| a * b).sum();
                        if draws.world {
                            v += q.sqrt() * rng.normal();
                        }
                        next[i] = v;
                        measured[i] = v;
                    }
                }
                for (i, n) in names.iter().enumerate() {
                    recent[i].push_front(next[i]);
                    recent[i].truncate(self.nodes[n].lags.max(2));
                    last.insert(n.clone(), next[i]);
                    out[n][h][r] = measured[i];
                }
            }
        }
        out
    }

    /// What the tracked nodes will be measured to be, moment after moment, with the spread of `runs` runs.
    pub fn forecast(&self, steps: usize, runs: usize, step: f64) -> Forecast {
        let seed = self.t.to_bits() ^ steps as u64;
        let runs_of = self.ensemble(steps, runs.max(2), ALL, seed);
        let paths = runs_of
            .into_iter()
            .map(|(n, per_step)| {
                let mut p = Path { mean: vec![], sigma: vec![], low: vec![], high: vec![] };
                for mut vals in per_step {
                    vals.retain(|v| v.is_finite());
                    let k = vals.len().max(1) as f64;
                    let mean = vals.iter().sum::<f64>() / k;
                    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (k - 1.0).max(1.0);
                    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let q = |f: f64| {
                        vals.get(((vals.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(f64::NAN)
                    };
                    p.mean.push(mean);
                    p.sigma.push(var.sqrt());
                    p.low.push(q(0.16));
                    p.high.push(q(0.84));
                }
                (n, p)
            })
            .collect();
        Forecast { start: self.t, step, paths }
    }

    /// Why the forecast of a node `horizon` moments ahead is what it is.
    pub fn why(&self, name: &str, horizon: usize, runs: usize) -> Result<Why> {
        let Some(node) = self.nodes.get(name) else { bail!("{name} does not live in time (track it first)") };
        let horizon = horizon.max(1);
        let f = self.forecast(horizon, runs, 1.0);
        let path = &f.paths[name];
        let (value, sigma) = (path.mean[horizon - 1], path.sigma[horizon - 1]);
        // where the uncertainty comes from: runs that draw only one thing each
        let seed = self.t.to_bits() ^ 0x5eed;
        let spread = |draws: Draws| {
            let runs_of = self.ensemble(horizon, runs.max(2), draws, seed);
            let vals = &runs_of[name][horizon - 1];
            let k = vals.len() as f64;
            let m = vals.iter().sum::<f64>() / k;
            vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (k - 1.0).max(1.0)
        };
        let none = Draws { law: false, world: false, senses: false, state: false };
        let mut parts = vec![
            ("the world moving".to_string(), spread(Draws { world: true, ..none })),
            ("what it does not know yet of its own law".to_string(), spread(Draws { law: true, ..none })),
        ];
        if node.hidden.is_some() {
            parts.push(("its senses".to_string(), spread(Draws { senses: true, ..none })));
            parts.push(("not knowing exactly where it is now".to_string(), spread(Draws { state: true, ..none })));
        }
        let total = parts.iter().map(|(_, v)| v).sum::<f64>().max(1e-300);
        let shares = parts.into_iter().map(|(w, v)| (w, v / total)).collect();
        let typical = node.memory.total.spread();
        let confidence = if typical > 0.0 { (1.0 - sigma / typical).clamp(0.0, 1.0) } else { 0.0 };
        let mut contributions = vec![];
        match &node.hidden {
            Some(h) => {
                let u = Self::inputs(&h.couplings, &self.last, self.t + self.dt);
                for k in 0..h.p {
                    let when =
                        if k == 0 { "the moment before".to_string() } else { format!("{} moments before", k + 1) };
                    contributions.push((format!("{name} {when} (as it believes it was)"), h.theta[k] * h.s[k]));
                }
                for (j, c) in h.couplings.iter().enumerate() {
                    contributions.push((link_label(c), h.theta[h.p + j] * u[j]));
                }
                contributions.push(("a constant".to_string(), h.theta[h.theta.len() - 1]));
                let next = self.t + self.dt;
                for (i, p) in h.rhythms.periods.iter().enumerate() {
                    let one =
                        Rhythms { periods: vec![*p], beta: h.rhythms.beta[2 * i..2 * i + 2].to_vec(), cov: vec![] };
                    contributions.push((format!("its cycle of {p} moments"), one.at(next, None)));
                }
            }
            None => {
                if let Some(x) = features_of(&node.model, &node.recent, &self.last, self.t + self.dt) {
                    let w = node.model.mean();
                    for (k, feat) in node.model.features.iter().enumerate() {
                        let part = match feat {
                            Feature::Lag(1) => format!("{name} the moment before"),
                            Feature::Lag(k) => format!("{name} {k} moments before"),
                            Feature::Coupling(n) => link_label(n),
                            Feature::Constant => "a constant".to_string(),
                        };
                        contributions.push((part, w[k] * x[k]));
                    }
                }
            }
        }
        // a cycle is one part (its cosine and its sine together)
        let mut merged: Vec<(String, f64)> = vec![];
        for (part, c) in contributions {
            match merged.iter_mut().find(|(p, _)| *p == part) {
                Some(m) => m.1 += c,
                None => merged.push((part, c)),
            }
        }
        let contributions = merged;
        // every node the forecast rests on: itself, its couplings, theirs...
        let mut nodes = vec![name.to_string()];
        let mut k = 0;
        while k < nodes.len() {
            if let Some(t) = self.nodes.get(&nodes[k]) {
                for c in t.coupled_nodes() {
                    if !nodes.contains(&c) {
                        nodes.push(c);
                    }
                }
            }
            k += 1;
        }
        Ok(Why {
            node: name.to_string(),
            horizon,
            value,
            sigma,
            confidence,
            shares,
            noises: node.hidden.as_ref().map(|h| (h.q.sqrt(), h.r.sqrt())),
            contributions,
            nodes,
            swing: node.swing(),
            calibration: node.calibration.clone(),
        })
    }
}

/// A moment as a human reads it: hours since 1970 as a date and an hour, a year as a year, else the number.
pub fn moment_text(t: f64, unit: &str) -> String {
    match unit {
        "hour" if t.is_finite() => {
            let days = (t / 24.0).floor();
            let (y, m, d) = civil(days as i64);
            let h = t - days * 24.0;
            format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", h.floor() as i64, ((h - h.floor()) * 60.0).round() as i64)
        }
        "year" => format!("{t:.0}"),
        _ => format!("{t}"),
    }
}

/// The civil date of a number of days since 1970-01-01 (Howard Hinnant's algorithm).
pub fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

fn features_of(model: &Dynamics, recent: &VecDeque<f64>, last: &IndexMap<String, f64>, t: f64) -> Option<Vec<f64>> {
    model
        .features
        .iter()
        .map(|f| match f {
            Feature::Lag(k) => recent.get(k - 1).copied(),
            Feature::Coupling(n) => input_value(n, last, t),
            Feature::Constant => Some(1.0),
        })
        .collect()
}

/// A cycle, as its links are named: "@cycle:24" (a link that is not a node: a name of the language never starts
/// with @); as its law reads it: its cosine and its sine at each moment.
pub const CYCLE: &str = "@cycle:";
const COS: &str = "@cos:";
const SIN: &str = "@sin:";

/// The value of what drives a node at moment t: another node's last value, or a cycle's cosine or sine.
fn input_value(name: &str, last: &IndexMap<String, f64>, t: f64) -> Option<f64> {
    if let Some(p) = name.strip_prefix(COS) {
        return p.parse::<f64>().ok().map(|p| (std::f64::consts::TAU * t / p).cos());
    }
    if let Some(p) = name.strip_prefix(SIN) {
        return p.parse::<f64>().ok().map(|p| (std::f64::consts::TAU * t / p).sin());
    }
    last.get(name).copied()
}

/// How a link is said in a why.
fn link_label(link: &str) -> String {
    match link.strip_prefix(COS).or_else(|| link.strip_prefix(SIN)).or_else(|| link.strip_prefix(CYCLE)) {
        Some(p) => format!("its cycle of {p} moments"),
        None => format!("{link} the moment before"),
    }
}

/// Two cycles a few percent apart are the same rhythm.
fn near(a: &str, b: &str) -> bool {
    match (
        a.strip_prefix(CYCLE).and_then(|p| p.parse::<f64>().ok()),
        b.strip_prefix(CYCLE).and_then(|p| p.parse::<f64>().ok()),
    ) {
        (Some(x), Some(y)) => (x - y).abs() <= 0.15 * x.max(y),
        _ => a == b,
    }
}

/// Whether a link (or a rhythm close to it) has been doubted already.
fn doubts(doubted: &[String], link: &str) -> bool {
    doubted.iter().any(|d| near(d, link))
}

/// A law with one more link: a coupling, or a cycle (its cosine and its sine).
fn extend(model: &Dynamics, link: &str) -> Dynamics {
    match link.strip_prefix(CYCLE) {
        Some(p) => model.with(Feature::Coupling(format!("{COS}{p}"))).with(Feature::Coupling(format!("{SIN}{p}"))),
        None => model.with(Feature::Coupling(link.to_string())),
    }
}

/// A law without a link.
fn shrink(model: &Dynamics, link: &str) -> Dynamics {
    match link.strip_prefix(CYCLE) {
        Some(p) => {
            model.without(&Feature::Coupling(format!("{COS}{p}"))).without(&Feature::Coupling(format!("{SIN}{p}")))
        }
        None => model.without(&Feature::Coupling(link.to_string())),
    }
}

/// The cycles a node's past seems to hold (in moments): the strongest peaks of its periodogram, and the
/// rhythms of its unit (a day and a week for hours, a day for minutes), when it remembers at least two of them.
fn cycles(node: &Tracked, unit: &str, dt: f64) -> Vec<f64> {
    let ys: Vec<f64> = node.window.iter().filter_map(|(_, y, _)| *y).collect();
    let n = ys.len();
    let mut out: Vec<f64> = vec![];
    let hint: &[f64] = match unit {
        "hour" => &[24.0, 168.0],
        "minute" => &[1440.0],
        "day" => &[7.0, 365.25],
        "month" => &[12.0],
        _ => &[],
    };
    for h in hint {
        let p = (h / dt.max(1e-12)).round();
        if p >= 3.0 && n as f64 >= 2.0 * p {
            out.push(p);
        }
    }
    if n >= 24 {
        // the periodogram of what it remembers, without its trend
        let mean = ys.iter().sum::<f64>() / n as f64;
        let slope = {
            let tm = (n as f64 - 1.0) / 2.0;
            let num: f64 = ys.iter().enumerate().map(|(k, y)| (k as f64 - tm) * (y - mean)).sum();
            let den: f64 = (0..n).map(|k| (k as f64 - tm).powi(2)).sum();
            num / den
        };
        let detrended: Vec<f64> =
            ys.iter().enumerate().map(|(k, y)| y - mean - slope * (k as f64 - (n as f64 - 1.0) / 2.0)).collect();
        let power = |p: f64| {
            let (mut c, mut s) = (0.0, 0.0);
            for (k, y) in detrended.iter().enumerate() {
                let a = std::f64::consts::TAU * k as f64 / p;
                c += y * a.cos();
                s += y * a.sin();
            }
            c * c + s * s
        };
        let periods: Vec<f64> = (3..=(n / 3)).map(|p| p as f64).collect();
        let powers: Vec<f64> = periods.iter().map(|p| power(*p)).collect();
        let mut sorted = powers.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        // local peaks well above the rest
        let mut peaks: Vec<(f64, f64)> = (1..powers.len().saturating_sub(1))
            .filter(|&i| powers[i] > powers[i - 1] && powers[i] >= powers[i + 1] && powers[i] > 20.0 * median)
            .map(|i| (periods[i], powers[i]))
            .collect();
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (p, _) in peaks.into_iter().take(1) {
            if !out.iter().any(|q| (q - p).abs() <= 1.0) {
                out.push(p);
            }
        }
    }
    out
}

/// log of a Student-t density (what a normal-inverse-gamma law expects of the next value: heavy-tailed while it
/// has seen little, a normal once it has seen much).
fn log_student(y: f64, m: f64, scale2: f64, dof: f64) -> f64 {
    let s2 = scale2.max(1e-300);
    let z2 = (y - m).powi(2) / s2;
    ln_gamma((dof + 1.0) / 2.0)
        - ln_gamma(dof / 2.0)
        - 0.5 * (dof * std::f64::consts::PI * s2).ln()
        - (dof + 1.0) / 2.0 * (1.0 + z2 / dof).ln()
}

/// ln Γ(x) (Lanczos).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * std::f64::consts::TAU.ln() + (x + 0.5) * t.ln() - t + a.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_precise_near_the_present_and_coarser_further_back() {
        let mut tl = Timeline::new(8);
        for t in 0..1000 {
            tl.push(t as f64, t as f64);
        }
        assert!(tl.size() < 8 * 10); // about 8 per level, log2(1000 / 8) levels
        assert_eq!(tl.at(999.0), Some((999.0, 0.0))); // the last moments, one by one
        let (old, spread) = tl.at(10.0).unwrap(); // far back: the mean of a long span
        assert!((old - 10.0).abs() <= 2.0 * spread + 64.0 && spread > 1.0, "{old} {spread}");
        assert!(tl.at(-1.0).is_none());
        assert_eq!(tl.total.count, 1000);
    }

    #[test]
    fn a_node_learns_its_swing_and_foresees_it() {
        let mut tk = Timekeeper::new();
        tk.track("x", 2, &[]);
        let mut rng = Rng::new(7);
        let period = 24.0;
        for t in 0..600 {
            let v = 10.0 + 5.0 * (std::f64::consts::TAU * t as f64 / period).sin() + 0.05 * rng.normal();
            tk.record(t as f64, &IndexMap::from([("x".to_string(), v)]));
        }
        let (p, keep) = tk.nodes["x"].model.swing().unwrap();
        assert!((p - period).abs() < 0.5 && keep > 0.95, "period {p}, keeps {keep}");
        let f = tk.forecast(24, 300, 1.0);
        let truth = 10.0 + 5.0 * (std::f64::consts::TAU * 606.0 / period).sin();
        let path = &f.paths["x"];
        assert!((path.mean[6] - truth).abs() < 3.0 * path.sigma[6] + 0.2, "{} vs {truth}", path.mean[6]);
        assert!(path.sigma[23] > path.sigma[0]); // the further, the less sure
        let why = tk.why("x", 6, 300).unwrap();
        println!("{}", why.text(&IndexMap::new()));
        assert!(why.confidence > 0.7, "{}", why.text(&IndexMap::new()));
        let c = why.calibration.coverage().unwrap();
        assert!(c > 0.5 && c < 0.85, "{}", why.calibration.text());
    }

    #[test]
    fn a_node_finds_by_itself_what_it_is_coupled_to() {
        // ten different worlds: the true link is found in each, a false one in none
        for seed in 1..=10 {
            let mut tk = Timekeeper::new();
            tk.track("cause", 1, &[]);
            tk.track("effect", 1, &[]);
            tk.track("unrelated", 1, &[]);
            let mut rng = Rng::new(seed);
            let (mut c, mut e) = (0.0, 0.0);
            let mut found = vec![];
            for t in 0..400 {
                let c_next = 0.9 * c + rng.normal();
                e = 0.5 * e + 0.8 * c + 0.1 * rng.normal(); // the effect follows the cause the moment before
                c = c_next;
                let values = IndexMap::from([
                    ("cause".to_string(), c),
                    ("effect".to_string(), e),
                    ("unrelated".to_string(), rng.normal()),
                ]);
                found.extend(tk.record(t as f64, &values));
            }
            assert!(found.iter().any(|i| i.node == "effect" && i.with == "cause"), "seed {seed}: {found:?}");
            assert!(
                !found.iter().any(|i| i.revised && i.with == "cause"),
                "a true link is kept, seed {seed}: {found:?}"
            );
            assert!(!found.iter().any(|i| !(i.node == "effect" && i.with == "cause")), "seed {seed}: {found:?}");
            let why = tk.why("effect", 3, 200).unwrap();
            assert_eq!(why.nodes, vec!["effect", "cause"]);
        }
    }

    #[test]
    fn a_hidden_state_tells_the_world_from_the_senses_and_foresees_closer() {
        // a swing that moves a little by itself (± 0.02 a moment), read by a poor thermometer (± 0.5)
        let world = |seed: u64| {
            let mut rng = Rng::new(seed);
            let (a1, a2) = (2.0 * 0.999 * (std::f64::consts::TAU / 24.0).cos(), -0.999f64.powi(2));
            let (mut x1, mut x2) = (5.0, 4.8);
            let mut truth = vec![];
            let mut measured = vec![];
            for _ in 0..720 {
                let x = a1 * x1 + a2 * x2 + 0.02 * rng.normal();
                x2 = x1;
                x1 = x;
                truth.push(x);
                measured.push(x + 0.5 * rng.normal());
            }
            (truth, measured)
        };
        let mut inside = [0, 0];
        let mut width = [0.0, 0.0];
        for seed in 1..=4 {
            let (truth, measured) = world(seed);
            for (k, hidden) in [true, false].into_iter().enumerate() {
                let mut tk = Timekeeper::new();
                tk.hidden_states = hidden;
                tk.track("x", 2, &[]);
                for (t, y) in measured.iter().take(696).enumerate() {
                    tk.record(t as f64, &IndexMap::from([("x".to_string(), *y)]));
                }
                let f = tk.forecast(24, 300, 1.0);
                let p = &f.paths["x"];
                // what will be measured in 24 moments: the truth plus the thermometer's error
                if (measured[719] - p.mean[23]).abs() <= p.sigma[23] {
                    inside[k] += 1;
                }
                width[k] += p.sigma[23];
                if hidden {
                    let h = tk.nodes["x"].hidden.as_ref().unwrap();
                    assert!((h.r.sqrt() / 0.5 - 1.0).abs() < 0.2, "senses {}", h.r.sqrt());
                    assert!((h.value(695.0) - truth[695]).abs() < 0.4, "{} vs {}", h.value(695.0), truth[695]);
                    let why = tk.why("x", 24, 300).unwrap();
                    assert!(why.noises.is_some() && why.shares.len() == 4, "{}", why.text(&IndexMap::new()));
                }
            }
        }
        // the hidden state foresees with a narrower spread, and its spread still holds the truth
        assert!(width[0] < 0.6 * width[1], "hidden {} vs straight {}", width[0] / 4.0, width[1] / 4.0);
        assert!(inside[0] >= 2, "{inside:?}");
    }

    #[test]
    fn a_day_is_found_by_itself_and_foreseen() {
        // hour by hour: a weather that wanders, a day that comes back, a thermometer
        let mut rng = Rng::new(21);
        let mut weather = 0.0;
        let mut rows = vec![];
        for k in 0..720 {
            weather = 0.97 * weather + 0.4 * rng.normal();
            let t = 12.0 + weather + 5.0 * (std::f64::consts::TAU * (k as f64 - 9.0) / 24.0).sin() + 0.2 * rng.normal();
            rows.push((k as f64, t));
        }
        let mut sigma = [0.0; 2];
        for (k, curious) in [true, false].into_iter().enumerate() {
            let mut tk = Timekeeper::new();
            tk.unit = "hour".into();
            tk.curious = curious;
            tk.track("temperature", 3, &[]);
            for (t, v) in rows.iter().take(696) {
                tk.record(*t, &IndexMap::from([("temperature".to_string(), *v)]));
            }
            if curious {
                assert!(tk.insights.iter().any(|i| i.with == "@cycle:24" && !i.revised), "{:?}", tk.insights);
                assert!(!tk.insights.iter().any(|i| i.with == "@cycle:24" && i.revised), "{:?}", tk.insights);
                let why = tk.why("temperature", 24, 300).unwrap();
                assert!(
                    why.contributions.iter().any(|(p, _)| p == "its cycle of 24 moments"),
                    "{}",
                    why.text(&IndexMap::new())
                );
            }
            let f = tk.forecast(24, 300, 1.0);
            let p = &f.paths["temperature"];
            sigma[k] = p.sigma[23];
            let truth = rows[719].1;
            assert!((truth - p.mean[23]).abs() < 3.0 * p.sigma[23], "{truth} vs {} ± {}", p.mean[23], p.sigma[23]);
        }
        // knowing its day, it foresees the next one closer
        assert!(sigma[0] < 0.8 * sigma[1], "with its day {} / without {}", sigma[0], sigma[1]);
    }
}
