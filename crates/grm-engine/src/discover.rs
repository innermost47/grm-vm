// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Laws found at the same moment: what holds between nodes on every row the machine has seen (y = a x + b,
//! y = k x^p, y = k x1^p1 x2^p2), found by itself, written in its own language, and proposed to the network
//! through a sandbox (the immune system decides).
//!
//! The rows it looks at are kept all along what it lived, not only at the end: when there are too many, one in
//! two is let go, and from then on one in two is kept (its memory of rows thins evenly, as its memory of time).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::lang::fmt_g;

/// A law holds when it explains at least this share of what varies (r^2).
pub const HOLDS: f64 = 0.999;
/// Rows kept at most.
pub const ROWS: usize = 400;
/// Rows needed before a law is believed.
pub const MIN_ROWS: usize = 12;

/// The rows the machine keeps, evenly along what it lived.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rows {
    pub rows: Vec<IndexMap<String, f64>>,
    /// One row kept in `every`.
    pub every: usize,
    seen: usize,
}

impl Rows {
    pub fn new() -> Rows {
        Rows { rows: vec![], every: 1, seen: 0 }
    }

    pub fn push(&mut self, row: IndexMap<String, f64>) {
        let every = self.every.max(1);
        if self.seen.is_multiple_of(every) {
            self.rows.push(row);
            if self.rows.len() > ROWS {
                let mut k = 0;
                self.rows.retain(|_| {
                    k += 1;
                    k % 2 == 1
                });
                self.every = every * 2;
            }
        }
        self.seen += 1;
    }
}

/// What a law says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Form {
    /// y = a x + b
    Line { a: f64, b: f64 },
    /// y = k x^p
    Power { k: f64, p: f64 },
    /// y = k x1^p1 x2^p2
    /// y = k x1^p1 x2^p2 [x3^p3]
    Product { k: f64, p: Vec<f64> },
    /// y = a ln(x) + b
    Log { a: f64, b: f64 },
    /// y = k e^(a x)
    Exp { k: f64, a: f64 },
    /// y = a1 x1 + a2 x2 + b
    /// y = a1 x1 + a2 x2 [+ a3 x3] + b
    Sum { a: Vec<f64>, b: f64 },
    /// y = offset + amp e^(-x/tau) cos(2 pi x / period + phase): it swings (tau infinite: it does not fade)
    Wave { period: f64, tau: f64, amp: f64, offset: f64, phase: f64 },
}

/// A law found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Law {
    pub target: String,
    pub inputs: Vec<String>,
    pub form: Form,
    pub r2: f64,
    pub rows: usize,
    /// How far the rows are from it (in the target's unit): the sigma the relation is given.
    pub spread: f64,
    /// Where it holds, when it holds only there: (node, lo, hi).
    pub when: Option<(String, f64, f64)>,
    /// The typical size of its target and of its first input (what a spread in dex, or a helper node, starts from).
    pub typical: (f64, f64),
}

impl Law {
    /// The law as statements of the language, its relation named `name` (a law that needs a logarithm says first
    /// the nodes that hold it). `positive(node)`: whether a node of the network is a positive one.
    pub fn statements(&self, name: &str, positive: &dyn Fn(&str) -> bool) -> Vec<String> {
        // coefficients written to twelve digits, and a law never surer than that (a law rounded and held exactly
        // would contradict the same law said the other way)
        let g = |v: f64| fmt_g(v, 12);
        let y = &self.target;
        let x = &self.inputs[0];
        let when =
            self.when.as_ref().map(|(z, lo, hi)| format!(" when {z} in {} .. {}", g(*lo), g(*hi))).unwrap_or_default();
        let dex = |spread: f64| {
            format!("{} dex", g((spread / (self.typical.0.abs().max(1e-300) * std::f64::consts::LN_10)).max(1e-9)))
        };
        let plain = g(self.spread.max(1e-9 * self.typical.0.abs().max(1.0)));
        let signed = |v: f64| if v < 0.0 { format!("- {}", g(-v)) } else { format!("+ {}", g(v)) };
        // a node of the language that is positive (in log): the node itself, or a positive twin of it
        let mut lines = vec![];
        let mut positive_of = |node: &str, typical: f64, tag: &str| -> String {
            if positive(node) {
                node.to_string()
            } else {
                let twin = format!("{name}_{tag}");
                lines.push(format!("node {twin} = {}", g(typical.abs().max(1e-12))));
                lines.push(format!("relate {twin}_is: {twin} = {node}"));
                twin
            }
        };
        let law = match &self.form {
            // a swing is no relation between nodes at one moment: what it is, said as nodes of its own (its
            // period, how long it takes to fade, how wide, around what)
            Form::Wave { period, tau, amp, offset, .. } => {
                let sd = |v: f64| {
                    g((self.spread / self.typical.0.abs().max(1e-12) * v.abs()).max(1e-9 * v.abs().max(1e-12)))
                };
                let mut measured = |node: String, v: f64, signed: bool| {
                    lines.push(format!("node {node} = {}{}", g(v), if signed { " signed" } else { "" }));
                    lines.push(format!("observe {node} = {} ± {}", g(v), sd(v)));
                };
                measured(format!("{y}_period"), *period, false);
                measured(format!("{y}_amplitude"), *amp, false);
                measured(format!("{y}_centre"), *offset, true);
                if tau.is_finite() {
                    measured(format!("{y}_fading"), *tau, false);
                }
                lines.pop().unwrap_or_default()
            }
            Form::Line { a, b } => format!("relate {name}: {y} = {} * {x} {} ± {plain}{when}", g(*a), signed(*b)),
            Form::Sum { a, b } => {
                let terms: Vec<String> = a
                    .iter()
                    .zip(&self.inputs)
                    .enumerate()
                    .map(
                        |(i, (a, n))| {
                            if i == 0 {
                                format!("{} * {n}", g(*a))
                            } else {
                                format!("{} * {n}", signed(*a))
                            }
                        },
                    )
                    .collect();
                format!("relate {name}: {y} = {} {} ± {plain}{when}", terms.join(" "), signed(*b))
            }
            Form::Power { k, p } => {
                let sigma = if positive(y) { dex(self.spread) } else { plain.clone() };
                format!("relate {name}: {y} = {} * {x}^{} ± {sigma}{when}", g(*k), g(*p))
            }
            Form::Product { k, p } => {
                let sigma = if positive(y) { dex(self.spread) } else { plain.clone() };
                let factors: Vec<String> = p.iter().zip(&self.inputs).map(|(p, n)| format!("{n}^{}", g(*p))).collect();
                format!("relate {name}: {y} = {} * {} ± {sigma}{when}", g(*k), factors.join(" * "))
            }
            Form::Log { a, b } => {
                let px = positive_of(x, self.typical.1, "x");
                lines.push(format!("node {name}_ln signed"));
                lines.push(format!("relate {name}_l: {name}_ln = ln({px})"));
                format!("relate {name}: {y} = {} * {name}_ln {} ± {plain}{when}", g(*a), signed(*b))
            }
            Form::Exp { k, a } => {
                let py = positive_of(y, self.typical.0, "y");
                lines.push(format!("node {name}_ln signed"));
                lines.push(format!("relate {name}_l: {name}_ln = ln({py})"));
                let sigma = g((self.spread / self.typical.0.abs().max(1e-300)).max(1e-9));
                format!("relate {name}: {name}_ln = {} * {x} {} ± {sigma}{when}", g(*a), signed(k.ln()))
            }
        };
        lines.push(law);
        lines
    }

    /// The law as one statement (when it needs no helper node), for a node that is not positive.
    pub fn statement(&self, name: &str) -> String {
        self.statements(name, &|_| false).pop().unwrap_or_default()
    }

    /// In words.
    pub fn text(&self) -> String {
        let what = match &self.form {
            Form::Line { .. } => "follows a straight line of".to_string(),
            Form::Power { p, .. } => format!("is a power ({}) of", fmt_g(*p, 3)),
            Form::Product { .. } => "is a product of powers of".to_string(),
            Form::Log { .. } => "grows as the logarithm of".to_string(),
            Form::Exp { a, .. } => {
                if *a < 0.0 { "fades exponentially with" } else { "grows exponentially with" }.to_string()
            }
            Form::Sum { .. } => "is a sum of".to_string(),
            Form::Wave { period, tau, amp, .. } => {
                let fade = if tau.is_finite() {
                    format!(", fading over {}", fmt_g(*tau, 3))
                } else {
                    ", without fading".to_string()
                };
                format!("swings (a period of {}, {} wide{fade}) with", fmt_g(*period, 4), fmt_g(*amp, 3))
            }
        };
        let when = self
            .when
            .as_ref()
            .map(|(z, lo, hi)| format!(", while {z} is in {} .. {}", fmt_g(*lo, 4), fmt_g(*hi, 4)))
            .unwrap_or_default();
        format!(
            "{} {what} {}{when} (r² {:.5} over {} rows)",
            self.target,
            self.inputs.join(" and "),
            self.r2,
            self.rows
        )
    }
}

/// Least squares of y on the columns of x (with a constant): (coefficients, constant, r², residual spread).
fn regress(xs: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, f64, f64, f64)> {
    let n = y.len();
    let k = xs.len();
    if n < MIN_ROWS {
        return None;
    }
    let my = y.iter().sum::<f64>() / n as f64;
    let mx: Vec<f64> = xs.iter().map(|c| c.iter().sum::<f64>() / n as f64).collect();
    let mut a = vec![vec![0.0; k]; k];
    let mut b = vec![0.0; k];
    for r in 0..n {
        for i in 0..k {
            let di = xs[i][r] - mx[i];
            b[i] += di * (y[r] - my);
            for j in 0..k {
                a[i][j] += di * (xs[j][r] - mx[j]);
            }
        }
    }
    let w = crate::time::solve_spd(&a, &b)?;
    let c = my - w.iter().zip(&mx).map(|(w, m)| w * m).sum::<f64>();
    let syy: f64 = y.iter().map(|v| (v - my).powi(2)).sum();
    if !(syy > 0.0) {
        return None;
    }
    let sse: f64 = (0..n).map(|r| (y[r] - c - (0..k).map(|i| w[i] * xs[i][r]).sum::<f64>()).powi(2)).sum();
    Some((w, c, 1.0 - sse / syy, (sse / n as f64).sqrt()))
}

/// The laws that hold on the rows. `known`: (target, inputs) already held, not proposed again.
pub fn find_laws(rows: &[IndexMap<String, f64>], known: &[(String, Vec<String>)]) -> Vec<Law> {
    // the columns filled on most rows, in the order they came
    let mut count: IndexMap<String, usize> = IndexMap::new();
    for r in rows {
        for (n, v) in r {
            if v.is_finite() {
                *count.entry(n.clone()).or_insert(0) += 1;
            }
        }
    }
    let cols: Vec<String> = count.iter().filter(|(_, c)| **c * 5 >= rows.len() * 4).map(|(n, _)| n.clone()).collect();
    let full = |names: &[&String]| -> Vec<&IndexMap<String, f64>> {
        rows.iter().filter(|r| names.iter().all(|n| r.get(*n).is_some_and(|v| v.is_finite()))).collect()
    };
    let column = |names: &[&String]| -> Vec<Vec<f64>> {
        let f = full(names);
        names.iter().map(|n| f.iter().map(|r| r[*n]).collect()).collect()
    };
    // the same nodes, whichever is the target
    let is_known = |t: &str, ins: &[String]| {
        let mut b: Vec<&str> = ins.iter().map(|s| s.as_str()).chain(std::iter::once(t)).collect();
        b.sort();
        known.iter().any(|(kt, ki)| {
            let mut a: Vec<&str> = ki.iter().map(|s| s.as_str()).chain(std::iter::once(kt.as_str())).collect();
            a.sort();
            a == b
        })
    };
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let mut laws = vec![];
    // a node a law already gives is not looked at again
    let mut explained: Vec<String> =
        known.iter().flat_map(|(t, ins)| std::iter::once(t.clone()).chain(ins.iter().cloned())).collect();
    // two nodes: the later one as a law of the earlier one (the simplest form that holds)
    for (j, y) in cols.iter().enumerate() {
        for x in cols.iter().take(j) {
            if is_known(y, std::slice::from_ref(x)) {
                continue;
            }
            let c = column(&[x, y]);
            if let Some(law) = pair_law(&c[0], &c[1], x, y).or_else(|| wave_law(&c[0], &c[1], x, y)) {
                laws.push(law);
                // both are held by it: neither is looked at again as a law of others
                explained.push(y.clone());
                explained.push(x.clone());
            }
        }
    }
    // three nodes, for what no single node explains: a product of powers, a sum, or a line that holds in two
    // regimes of a third node
    if cols.len() <= 16 {
        for y in &cols {
            if explained.contains(y) {
                continue;
            }
            // two inputs, then three: the fewest that hold
            let spread_of = |v: &[f64]| {
                let m = mean(v);
                (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt()
            };
            let others: Vec<&String> = cols.iter().filter(|c| *c != y).collect();
            let mut sets: Vec<Vec<&String>> = vec![];
            for i in 0..others.len() {
                for j in i + 1..others.len() {
                    sets.push(vec![others[i], others[j]]);
                }
            }
            if others.len() <= 12 {
                for i in 0..others.len() {
                    for j in i + 1..others.len() {
                        for k in j + 1..others.len() {
                            sets.push(vec![others[i], others[j], others[k]]);
                        }
                    }
                }
            }
            for inputs in sets {
                let names: Vec<String> = inputs.iter().map(|s| s.to_string()).collect();
                if is_known(y, &names) {
                    continue;
                }
                let mut wanted: Vec<&String> = inputs.clone();
                wanted.push(y);
                let c = column(&wanted);
                let m = inputs.len();
                let n = c[m].len();
                let typical = (mean(&c[m]), mean(&c[0]));
                let sy = spread_of(&c[m]);
                // each input must count (else it is a law of fewer nodes, said with a passenger)
                let counts =
                    |w: &[f64], cols: &[Vec<f64>]| (0..m).all(|i| (w[i] * spread_of(&cols[i])).abs() > 1e-3 * sy);
                if let Some((w, b, r2, sd)) = regress(&c[..m], &c[m]) {
                    if r2 >= HOLDS && counts(&w, &c) {
                        laws.push(Law {
                            target: y.clone(),
                            inputs: names,
                            form: Form::Sum { a: w, b },
                            r2,
                            rows: n,
                            spread: sd,
                            when: None,
                            typical,
                        });
                        explained.push(y.clone());
                        break;
                    }
                }
                if !c.iter().flatten().all(|v| *v > 0.0) {
                    continue;
                }
                let logs: Vec<Vec<f64>> = c.iter().map(|col| col.iter().map(|v| v.ln()).collect()).collect();
                if let Some((w, lk, r2, _)) = regress(&logs[..m], &logs[m]) {
                    let ly = spread_of(&logs[m]);
                    let counts_log = (0..m).all(|i| (w[i] * spread_of(&logs[i])).abs() > 1e-3 * ly);
                    if r2 >= HOLDS && counts_log {
                        let k = lk.exp();
                        let sd = ((0..n)
                            .map(|r| (c[m][r] - k * (0..m).map(|i| c[i][r].powf(w[i])).product::<f64>()).powi(2))
                            .sum::<f64>()
                            / n as f64)
                            .sqrt();
                        laws.push(Law {
                            target: y.clone(),
                            inputs: names,
                            form: Form::Product { k, p: w },
                            r2,
                            rows: n,
                            spread: sd,
                            when: None,
                            typical,
                        });
                        explained.push(y.clone());
                        break;
                    }
                }
            }
        }
        // two regimes: y a line of x below some value of z, another line above (the value searched among all)
        for y in &cols {
            if explained.contains(y) {
                continue;
            }
            'regimes: for x in cols.iter().filter(|x| *x != y) {
                if is_known(y, std::slice::from_ref(x)) {
                    continue;
                }
                for z in cols.iter().filter(|z| *z != y && *z != x) {
                    let mut f: Vec<(f64, f64, f64)> = full(&[x, y, z]).iter().map(|r| (r[z], r[x], r[y])).collect();
                    if f.len() < 2 * MIN_ROWS {
                        continue;
                    }
                    f.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    if let Some((k, left, right)) = split(&f) {
                        let n = f.len();
                        for (part, (a, b, r2, sd), lo, hi) in
                            [(&f[..k], left, f[0].0, f[k - 1].0), (&f[k..], right, f[k].0, f[n - 1].0)]
                        {
                            let xs: Vec<f64> = part.iter().map(|p| p.1).collect();
                            let ys: Vec<f64> = part.iter().map(|p| p.2).collect();
                            laws.push(Law {
                                target: y.clone(),
                                inputs: vec![x.clone()],
                                form: Form::Line { a, b },
                                r2,
                                rows: part.len(),
                                spread: sd,
                                when: Some((z.clone(), lo, hi)),
                                typical: (mean(&ys), mean(&xs)),
                            });
                        }
                        explained.push(y.clone());
                        break 'regimes;
                    }
                }
            }
        }
    }
    laws
}

/// A straight line found: (a, b, r^2, spread).
type Fit = (f64, f64, f64, f64);
/// Where rows split, and the line on each side.
type Split = (usize, Fit, Fit);

/// Where rows sorted by z split into two straight lines of y in x, each holding (r^2 >= HOLDS): the split, and
/// each line (a, b, r^2, spread). Sums are carried along, so every split is tried at the cost of one pass.
fn split(rows: &[(f64, f64, f64)]) -> Option<Split> {
    #[derive(Clone, Copy, Default)]
    struct Sums {
        n: f64,
        x: f64,
        y: f64,
        xx: f64,
        xy: f64,
        yy: f64,
    }
    let line = |s: Sums| -> Option<(f64, f64, f64, f64)> {
        let sxx = s.xx - s.x * s.x / s.n;
        let sxy = s.xy - s.x * s.y / s.n;
        let syy = s.yy - s.y * s.y / s.n;
        if !(sxx > 0.0 && syy > 0.0) {
            return None;
        }
        let a = sxy / sxx;
        let b = (s.y - a * s.x) / s.n;
        let sse = (syy - a * sxy).max(0.0);
        Some((a, b, 1.0 - sse / syy, (sse / s.n).sqrt()))
    };
    let n = rows.len();
    let mut prefix = vec![Sums::default(); n + 1];
    for (k, (_, x, y)) in rows.iter().enumerate() {
        let p = prefix[k];
        prefix[k + 1] =
            Sums { n: p.n + 1.0, x: p.x + x, y: p.y + y, xx: p.xx + x * x, xy: p.xy + x * y, yy: p.yy + y * y };
    }
    let total = prefix[n];
    let mut best: Option<Split> = None;
    for k in MIN_ROWS..=n - MIN_ROWS {
        if rows[k].0 == rows[k - 1].0 {
            continue; // a split falls between two values of z
        }
        let l = prefix[k];
        let r = Sums {
            n: total.n - l.n,
            x: total.x - l.x,
            y: total.y - l.y,
            xx: total.xx - l.xx,
            xy: total.xy - l.xy,
            yy: total.yy - l.yy,
        };
        if let (Some(a), Some(b)) = (line(l), line(r)) {
            if a.2 >= HOLDS && b.2 >= HOLDS && best.as_ref().is_none_or(|(_, x, y)| a.2.min(b.2) > x.2.min(y.2)) {
                best = Some((k, a, b));
            }
        }
    }
    best
}

/// The simplest law of y as a function of x that holds: a line, else the best of a power, a logarithm and an
/// exponential.
fn pair_law(xv: &[f64], yv: &[f64], x: &str, y: &str) -> Option<Law> {
    let n = yv.len();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let typical = (mean(yv), mean(xv));
    let law = |form: Form, r2: f64, spread: f64| Law {
        target: y.to_string(),
        inputs: vec![x.to_string()],
        form,
        r2,
        rows: n,
        spread,
        when: None,
        typical,
    };
    if let Some((w, b, r2, sd)) = regress(&[xv.to_vec()], yv) {
        if r2 >= HOLDS {
            return Some(law(Form::Line { a: w[0], b }, r2, sd));
        }
    }
    let spread =
        |f: &dyn Fn(f64) -> f64| (xv.iter().zip(yv).map(|(x, y)| (y - f(*x)).powi(2)).sum::<f64>() / n as f64).sqrt();
    let mut best: Option<Law> = None;
    let mut consider = |candidate: Law| {
        if candidate.r2 >= HOLDS && best.as_ref().is_none_or(|b| candidate.r2 > b.r2) {
            best = Some(candidate);
        }
    };
    let x_pos = xv.iter().all(|v| *v > 0.0);
    let y_pos = yv.iter().all(|v| *v > 0.0);
    if x_pos && y_pos {
        let lx: Vec<f64> = xv.iter().map(|v| v.ln()).collect();
        let ly: Vec<f64> = yv.iter().map(|v| v.ln()).collect();
        if let Some((w, lk, r2, _)) = regress(&[lx], &ly) {
            let (k, p) = (lk.exp(), w[0]);
            consider(law(Form::Power { k, p }, r2, spread(&|x| k * x.powf(p))));
        }
    }
    if x_pos {
        let lx: Vec<f64> = xv.iter().map(|v| v.ln()).collect();
        if let Some((w, b, r2, sd)) = regress(&[lx], yv) {
            consider(law(Form::Log { a: w[0], b }, r2, sd));
        }
    }
    if y_pos {
        let ly: Vec<f64> = yv.iter().map(|v| v.ln()).collect();
        if let Some((w, lk, r2, _)) = regress(&[xv.to_vec()], &ly) {
            let (k, a) = (lk.exp(), w[0]);
            consider(law(Form::Exp { k, a }, r2, spread(&|x| k * (a * x).exp())));
        }
    }
    best
}

/// A swing that holds well enough (a measured swing is noisier than a law of the same moment).
pub const SWINGS: f64 = 0.98;

/// y swinging with x (x going one way: a time): its period from where it crosses its middle, then the best period
/// and fading near it (on a fixed grid, then a finer one: nothing drawn by chance), each with its least squares.
fn wave_law(xv: &[f64], yv: &[f64], x: &str, y: &str) -> Option<Law> {
    let n = yv.len();
    if n < 3 * MIN_ROWS || !xv.windows(2).all(|w| w[1] > w[0]) {
        return None;
    }
    let mean = yv.iter().sum::<f64>() / n as f64;
    // where it crosses its middle
    let mut crossings = vec![];
    for k in 1..n {
        let (a, b) = (yv[k - 1] - mean, yv[k] - mean);
        if a == 0.0 || a.signum() != b.signum() && b != 0.0 {
            let f = if a == b { 0.0 } else { a / (a - b) };
            crossings.push(xv[k - 1] + f * (xv[k] - xv[k - 1]));
        }
    }
    if crossings.len() < 4 {
        return None;
    }
    let mut gaps: Vec<f64> = crossings.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_by(|a, b| a.total_cmp(b));
    let guess = 2.0 * gaps[gaps.len() / 2];
    let span = xv[n - 1] - xv[0];
    if !(guess > 0.0) || span < 1.5 * guess {
        return None;
    }
    let fit = |period: f64, fade: f64| -> Option<(Vec<f64>, f64, f64, f64)> {
        let w = std::f64::consts::TAU / period;
        let e = |t: f64| (-(t - xv[0]) * fade).exp();
        let c: Vec<f64> = xv.iter().map(|t| e(*t) * (w * (t - xv[0])).cos()).collect();
        let s: Vec<f64> = xv.iter().map(|t| e(*t) * (w * (t - xv[0])).sin()).collect();
        regress(&[c, s], yv)
    };
    // (period, fade, r2)
    let mut best = (guess, 0.0, f64::MIN);
    let (mut p_lo, mut p_hi) = (0.85 * guess, 1.15 * guess);
    let (mut f_lo, mut f_hi) = (0.0, 5.0 / span);
    for _ in 0..4 {
        for i in 0..=24 {
            let period = p_lo + (p_hi - p_lo) * i as f64 / 24.0;
            for j in 0..=24 {
                let fade = f_lo + (f_hi - f_lo) * j as f64 / 24.0;
                if let Some((_, _, r2, _)) = fit(period, fade) {
                    if r2 > best.2 {
                        best = (period, fade, r2);
                    }
                }
            }
        }
        let (dp, df) = ((p_hi - p_lo) / 12.0, (f_hi - f_lo) / 12.0);
        (p_lo, p_hi) = (best.0 - dp, best.0 + dp);
        (f_lo, f_hi) = ((best.1 - df).max(0.0), best.1 + df);
    }
    let (period, fade, _) = best;
    let (w, offset, r2, spread) = fit(period, fade)?;
    if r2 < SWINGS {
        return None;
    }
    // its phase at the first x, and its width there
    let amp = (w[0] * w[0] + w[1] * w[1]).sqrt();
    let phase = (-w[1]).atan2(w[0]);
    // a fading slower than ten times what was seen is no fading
    let tau = if fade * span < 0.1 { f64::INFINITY } else { 1.0 / fade };
    Some(Law {
        target: y.to_string(),
        inputs: vec![x.to_string()],
        form: Form::Wave { period, tau, amp, offset, phase },
        r2,
        rows: n,
        spread,
        when: None,
        typical: (amp, span),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Rng;

    #[test]
    fn a_swing_that_fades_is_found_from_its_rows_alone() {
        // a pendulum let go: its angle, sampled every 50 ms, a little noise
        let mut rng = Rng::new(3);
        let mut rows = Rows::new();
        for k in 0..300 {
            let t = k as f64 * 0.05;
            let angle = 0.3 * (-t / 6.0).exp() * (std::f64::consts::TAU * t / 2.006).cos() + 0.002 * rng.normal();
            rows.push(IndexMap::from([("t".to_string(), t), ("angle".to_string(), angle)]));
        }
        let laws = find_laws(&rows.rows, &[]);
        let Some(law) = laws.iter().find(|l| matches!(l.form, Form::Wave { .. })) else { panic!("{laws:?}") };
        let Form::Wave { period, tau, amp, .. } = law.form else { unreachable!() };
        assert!((period - 2.006).abs() < 0.01, "{period}");
        assert!((tau - 6.0).abs() < 0.4, "{tau}");
        assert!((amp - 0.3).abs() < 0.01, "{amp}");
        let lines = law.statements("found_1", &|_| false);
        assert!(lines.iter().any(|l| l.starts_with("node angle_period = 2.00")), "{lines:?}");
        assert!(law.text().starts_with("angle swings"), "{}", law.text());
        let mut w = crate::world::World::new();
        for l in &lines {
            w.say(l, &crate::Provenance::default()).unwrap_or_else(|e| panic!("{l}: {e}"));
        }
    }

    #[test]
    fn laws_are_found_and_written_in_the_language() {
        let mut rng = Rng::new(8);
        let mut rows = Rows::new();
        for _ in 0..1000 {
            let l = 0.5 + 2.0 * rng.uniform();
            let m = 1.0 + rng.uniform();
            let v = 3.0 * rng.uniform() + 0.5;
            let noise = rng.normal();
            rows.push(IndexMap::from([
                ("length".to_string(), l),
                ("period".to_string(), 2.0 * std::f64::consts::PI * (l / 9.81).sqrt()), // a pendulum
                ("celsius".to_string(), 10.0 * l + noise),
                ("fahrenheit".to_string(), 1.8 * (10.0 * l + noise) + 32.0), // a line
                ("mass".to_string(), m),
                ("speed".to_string(), v),
                ("energy".to_string(), 0.5 * m * v * v), // a product
                ("unrelated".to_string(), rng.normal()),
            ]));
        }
        assert!(rows.rows.len() <= ROWS && rows.every > 1);
        let laws = find_laws(&rows.rows, &[]);
        let find = |t: &str| laws.iter().find(|l| l.target == t).unwrap_or_else(|| panic!("{t}: {laws:?}"));
        assert!(matches!(find("period").form, Form::Power { p, .. } if (p - 0.5).abs() < 1e-6));
        assert!(
            matches!(find("fahrenheit").form, Form::Line { a, b } if (a - 1.8).abs() < 1e-9 && (b - 32.0).abs() < 1e-6)
        );
        assert!(
            matches!(&find("energy").form, Form::Product { p, .. } if (p[0] - 1.0).abs() < 1e-6 && (p[1] - 2.0).abs() < 1e-6)
        );
        assert!(!laws.iter().any(|l| l.target == "unrelated" || l.inputs.contains(&"unrelated".to_string())));
        assert!(find("fahrenheit")
            .statement("found_1")
            .starts_with("relate found_1: fahrenheit = 1.8 * celsius + 32 ± "));
        let known = vec![("fahrenheit".to_string(), vec!["celsius".to_string()])];
        assert!(!find_laws(&rows.rows, &known).iter().any(|l| l.target == "fahrenheit"));
    }

    #[test]
    fn logarithms_exponentials_sums_and_regimes_are_found() {
        let mut rng = Rng::new(13);
        let mut rows = vec![];
        for _ in 0..300 {
            let v = 1000.0 + 20000.0 * rng.uniform();
            let t = 10.0 * rng.uniform();
            let (a, b) = (rng.uniform(), rng.uniform());
            let s = rng.uniform();
            let x = 4.0 * rng.uniform();
            rows.push(IndexMap::from([
                ("velocity".to_string(), v),
                ("modulus".to_string(), 5.0 / std::f64::consts::LN_10 * v.ln() + 15.78), // the Hubble law, in magnitudes
                ("time".to_string(), t),
                ("atoms".to_string(), 100.0 * (-0.3 * t).exp()), // a decay
                ("a".to_string(), a),
                ("b".to_string(), b),
                ("total".to_string(), 2.0 * a + 3.0 * b + 1.0), // a sum
                ("switch".to_string(), s),
                ("x".to_string(), x),
                ("answer".to_string(), if s < 0.5 { 2.0 * x } else { 7.0 - x }), // two regimes
            ]));
        }
        let laws = find_laws(&rows, &[]);
        let find = |t: &str| laws.iter().find(|l| l.target == t).unwrap_or_else(|| panic!("{t}: {laws:#?}"));
        assert!(matches!(find("modulus").form, Form::Log { a, .. } if (a - 2.1715).abs() < 1e-3));
        assert!(matches!(find("atoms").form, Form::Exp { a, .. } if (a + 0.3).abs() < 1e-6));
        assert!(
            matches!(&find("total").form, Form::Sum { a, b } if (a[0] - 2.0).abs() < 1e-6 && (a[1] - 3.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6)
        );
        let regimes: Vec<&Law> = laws.iter().filter(|l| l.target == "answer").collect();
        assert_eq!(regimes.len(), 2, "{regimes:#?}");
        assert!(regimes.iter().all(|l| l.when.as_ref().is_some_and(|(z, _, _)| z == "switch")));
        // in its language, with the nodes a logarithm needs
        let lines = find("modulus").statements("found_1", &|_| false);
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert!(lines[3].contains("= ln(found_1_x)"), "{lines:?}");
    }

    #[test]
    fn laws_of_three_nodes_are_found() {
        let mut rng = Rng::new(31);
        let mut rows = vec![];
        for _ in 0..300 {
            let (a, b, c) = (1.0 + rng.uniform(), 1.0 + rng.uniform(), 1.0 + rng.uniform());
            let (u, v, w) = (rng.normal(), rng.normal(), rng.normal());
            rows.push(IndexMap::from([
                ("a".to_string(), a),
                ("b".to_string(), b),
                ("c".to_string(), c),
                ("pressure".to_string(), 8.314 * a * b * b / c), // a product of three
                ("u".to_string(), u),
                ("v".to_string(), v),
                ("w".to_string(), w),
                ("budget".to_string(), u + 2.0 * v - 3.0 * w + 4.0), // a sum of three
            ]));
        }
        let laws = find_laws(&rows, &[]);
        let find = |t: &str| laws.iter().find(|l| l.target == t).unwrap_or_else(|| panic!("{t}: {laws:#?}"));
        assert!(
            matches!(&find("pressure").form, Form::Product { p, .. } if p.len() == 3 && (p[1] - 2.0).abs() < 1e-6 && (p[2] + 1.0).abs() < 1e-6)
        );
        assert!(
            matches!(&find("budget").form, Form::Sum { a, b } if a.len() == 3 && (a[2] + 3.0).abs() < 1e-6 && (b - 4.0).abs() < 1e-6)
        );
        let said = find("budget").statement("found_9");
        assert!(said.starts_with("relate found_9: budget = 1 * u + 2 * v - 3 * w + 4 ± "), "{said}");
    }
}
