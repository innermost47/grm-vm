// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What a network of quantities says of itself, read from its relations alone (nothing is imposed):
//!
//! - where each quantity is on the scale of Planck (its value over the Planck unit of its dimension, in decades: the
//!   constants c, G, hbar, kB fall on 0);
//! - its family tree: generation 0 the fixed constants and a seed (one degree of freedom, H_end by default); a
//!   relation where all but one node is born gives birth to the last one, a generation after its youngest parent;
//!   and how it goes with the seed (its exponent: it follows the seed's cadence, goes against it, or stays);
//! - how the network vibrates when its relations are plucked (the two slowest modes of its graph): a shape the
//!   network decides;
//! - the equations it holds without any of them being written: combinations of its relations where every constant
//!   cancels (between two, three or four of its quantities), the pure number they give, and the relations that lead
//!   there (the smallest combination: no loop that does not serve).
//!
//! Only the relations of products are read (they are straight lines in log10); everything is computed in a fixed
//! order (no chance).

use indexmap::IndexMap;

use crate::network::Kind;
use crate::world::World;

/// The Planck units (CODATA 2018): mass, length, time, charge, temperature.
pub const PLANCK: [f64; 5] = [2.176_434e-8, 1.616_255e-35, 5.391_247e-44, 1.875_546_037_8e-18, 1.416_784e32];

/// A relation of products as read: its name, its text, its terms (quantity, power), its right side (log10).
type Rel = (String, String, Vec<(usize, f64)>, f64);

/// An equation the network holds without it being written.
#[derive(Debug, Clone)]
pub struct Found {
    /// Its quantities and their powers (the product of them is the number).
    pub terms: Vec<(usize, f64)>,
    /// The pure number it gives (1 for most).
    pub number: f64,
    /// The relations that lead to it, with how many times each is used.
    pub path: Vec<(f64, String, String)>,
    /// It is one of the network's own relations (not found: written).
    pub written: bool,
    /// The constants that cancel on the way (those its relations hold, gone from the equation).
    pub cancelled: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Universe {
    /// Its quantities (their names as said, prefix and all).
    pub names: Vec<String>,
    pub value: Vec<f64>,
    /// How sure the machine is (dex for a positive quantity).
    pub sigma: Vec<f64>,
    pub fixed: Vec<bool>,
    pub measured: Vec<bool>,
    /// Its value in Planck units, in decades.
    pub planck: Vec<f64>,
    /// Its generation from the seeds, the relation that gave it birth and its parents.
    pub born: Vec<Option<(usize, String, Vec<usize>)>>,
    /// How it goes with the seed: when the seed moves by a factor 10 and the network follows as little as it can
    /// (every relation holding, the constants still), it moves by 10^exponent.
    pub exponent: Vec<Option<f64>>,
    /// The two slowest modes of the network (each in -1 .. 1).
    pub modes: Vec<(f64, f64)>,
    /// Its relations of products, as pairs of quantities they tie.
    pub ties: Vec<(usize, usize)>,
    pub found: Vec<Found>,
    pub seed: Option<usize>,
}

/// Eigenvalues and eigenvectors (as columns) of a symmetric matrix, by Jacobi's rotations (sorted increasing).
pub fn eigen(a: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = a.to_vec();
    let mut v: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for _sweep in 0..60 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| m[i][j] * m[i][j])
            .sum();
        if off < 1e-22 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if m[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (m[q][q] - m[p][p]) / (2.0 * m[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (mkp, mkq) = (m[k][p], m[k][q]);
                    m[k][p] = c * mkp - s * mkq;
                    m[k][q] = s * mkp + c * mkq;
                }
                for k in 0..n {
                    let (mpk, mqk) = (m[p][k], m[q][k]);
                    m[p][k] = c * mpk - s * mqk;
                    m[q][k] = s * mpk + c * mqk;
                }
                for row in v.iter_mut() {
                    let (vkp, vkq) = (row[p], row[q]);
                    row[p] = c * vkp - s * vkq;
                    row[q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| m[i][i].total_cmp(&m[j][j]));
    let values = order.iter().map(|&i| m[i][i]).collect();
    let vectors = (0..n).map(|r| order.iter().map(|&i| v[r][i]).collect()).collect();
    (values, vectors)
}

/// A ratio made a small fraction (denominator up to 6), if it is one.
fn fraction(x: f64) -> Option<(i64, i64)> {
    for d in 1..=6i64 {
        let n = (x * d as f64).round();
        if (x * d as f64 - n).abs() < 1e-6 * d as f64 {
            return Some((n as i64, d));
        }
    }
    None
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a.abs()
    } else {
        gcd(b, a % b)
    }
}

impl World {
    /// What a source's network says of itself (see the module): `seed` is the name of its one degree of freedom
    /// (H_end, say), or none.
    pub fn universe(&self, source: &str, seed: Option<&str>) -> Universe {
        let net = &self.machine.net;
        let mut u = Universe::default();
        let mut index: IndexMap<String, usize> = IndexMap::new();
        for (n, spec) in &net.nodes {
            if spec.origin.source != source {
                continue;
            }
            index.insert(n.clone(), u.names.len());
            u.names.push(n.clone());
            let v = self.machine.value_si(n);
            u.value.push(v);
            u.sigma.push(self.machine.sigma(n));
            u.fixed.push(spec.fixed);
            u.measured.push(net.measured.contains_key(n));
            let shift: f64 = spec.dim.map(|d| (0..5).map(|i| d[i] * PLANCK[i].log10()).sum()).unwrap_or(0.0);
            u.planck.push(if v > 0.0 { v.log10() - shift } else { f64::NAN });
        }
        let n = u.names.len();
        // its relations of products: Σ power x log10(node) = rhs
        let rels: Vec<Rel> = net
            .relations
            .iter()
            .filter(|r| r.origin.source == source && r.when.is_empty())
            .filter_map(|r| match &r.kind {
                Kind::Product { terms, rhs } => {
                    let t: Vec<(usize, f64)> =
                        terms.iter().filter_map(|(k, p)| index.get(k).map(|&i| (i, *p))).collect();
                    (t.len() == terms.len() && !t.is_empty()).then(|| (r.name.clone(), r.text.clone(), t, *rhs))
                }
                _ => None,
            })
            .collect();
        for (_, _, t, _) in &rels {
            for a in 0..t.len() {
                for b in a + 1..t.len() {
                    u.ties.push((t[a].0, t[b].0));
                }
            }
        }
        // ---- its family tree from the constants and the seed
        u.seed = seed.and_then(|s| index.get(s).copied());
        u.born = vec![None; n];
        u.exponent = vec![None; n];
        for i in 0..n {
            if u.fixed[i] {
                u.born[i] = Some((0, "seed".into(), vec![]));
            }
        }
        if let Some(s) = u.seed {
            u.born[s] = Some((0, "seed".into(), vec![]));
        }
        let mut grew = true;
        while grew {
            grew = false;
            for (name, _, t, _) in &rels {
                for &(child, pc) in t {
                    if u.born[child].as_ref().is_some_and(|b| b.0 == 0) {
                        continue;
                    }
                    let parents: Vec<(usize, f64)> = t.iter().filter(|(k, _)| *k != child).copied().collect();
                    if parents.is_empty() || !parents.iter().all(|(p, _)| u.born[*p].is_some()) {
                        continue;
                    }
                    let g = 1 + parents.iter().map(|(p, _)| u.born[*p].as_ref().unwrap().0).max().unwrap_or(0);
                    if u.born[child].as_ref().is_none_or(|b| g < b.0) {
                        let _ = pc;
                        u.born[child] = Some((g, name.clone(), parents.iter().map(|(p, _)| *p).collect()));
                        grew = true;
                    }
                }
            }
        }
        // ---- how it vibrates: the two slowest modes of its graph (past those that only tell its pieces apart)
        let mut lap = vec![vec![0.0; n]; n];
        for (_, _, t, _) in &rels {
            let w = 1.0 / (t.len().max(2) - 1) as f64;
            for a in 0..t.len() {
                for b in 0..t.len() {
                    if a != b {
                        lap[t[a].0][t[b].0] -= w;
                        lap[t[a].0][t[a].0] += w;
                    }
                }
            }
        }
        u.modes = vec![(0.0, 0.0); n];
        if n >= 3 {
            let (vals, vecs) = eigen(&lap);
            let picked: Vec<usize> = (0..n).filter(|&k| vals[k] > 1e-9).take(2).collect();
            // each mode spread by rank (a mode often gathers most quantities near 0 and a few far: by rank, the
            // shape keeps its order and shows everyone), its sign fixed (the largest part positive)
            let col = |k: usize| -> Vec<f64> {
                let c: Vec<f64> = (0..n).map(|i| vecs[i][k]).collect();
                let s =
                    if c.iter().fold(0.0f64, |a, x| if x.abs() > a.abs() { *x } else { a }) < 0.0 { -1.0 } else { 1.0 };
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|&a, &b| (s * c[a]).total_cmp(&(s * c[b])).then(a.cmp(&b)));
                let mut r = vec![0.0; n];
                for (rank, &i) in order.iter().enumerate() {
                    r[i] = 2.0 * rank as f64 / (n - 1).max(1) as f64 - 1.0;
                }
                r
            };
            if let Some(&a) = picked.first() {
                let ca = col(a);
                let cb = picked.get(1).map(|&b| col(b)).unwrap_or_else(|| vec![0.0; n]);
                u.modes = (0..n).map(|i| (ca[i], cb[i])).collect();
            }
        }
        // ---- what it holds without it being written
        if rels.is_empty() {
            return u;
        }
        // the directions in which its quantities can move while every relation holds (the null space)
        let mut ata = vec![vec![0.0; n]; n];
        for (_, _, t, _) in &rels {
            for &(i, p) in t {
                for &(j, q) in t {
                    ata[i][j] += p * q;
                }
            }
        }
        let (vals, vecs) = eigen(&ata);
        let top = vals.iter().fold(1.0f64, |a, x| a.max(x.abs()));
        let free: Vec<usize> = (0..n).filter(|&k| vals[k].abs() < 1e-9 * top).collect();
        // each quantity's way of moving (its row of the null space)
        let way: Vec<Vec<f64>> = (0..n).map(|i| free.iter().map(|&k| vecs[i][k]).collect()).collect();
        let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        let in_rels: Vec<bool> = {
            let mut b = vec![false; n];
            for (_, _, t, _) in &rels {
                for &(i, _) in t {
                    b[i] = true;
                }
            }
            b
        };
        // the quantities that move (not a constant, not fixed by the constants alone), in a relation
        let movers: Vec<usize> =
            (0..n).filter(|&i| !u.fixed[i] && in_rels[i] && norm(&way[i]) > 1e-7 && u.value[i] > 0.0).collect();
        let mut found: Vec<(Vec<(usize, i64)>, f64)> = vec![];
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        let collinear = |a: usize, b: usize| -> Option<f64> {
            let (na, nb) = (norm(&way[a]), norm(&way[b]));
            let c = dot(&way[a], &way[b]) / (na * nb);
            (c.abs() > 1.0 - 1e-9).then(|| c.signum() * nb / na)
        };
        let small = |k: &[(usize, i64)]| k.iter().all(|(_, p)| p.abs() <= 6);
        // two that move together: a^p = b^q (times a number)
        for x in 0..movers.len() {
            for y in x + 1..movers.len() {
                let (a, b) = (movers[x], movers[y]);
                if let Some(r) = collinear(a, b) {
                    // way[b] = r way[a]: b^1 a^-r is constant
                    if let Some((num, den)) = fraction(r) {
                        let k = vec![(b, den), (a, -num)];
                        if small(&k) {
                            found.push((k, 0.0));
                        }
                    }
                }
            }
        }
        // three: c = a^α b^β (times a number), none two of them together already
        for x in 0..movers.len() {
            for y in x + 1..movers.len() {
                let (a, b) = (movers[x], movers[y]);
                if collinear(a, b).is_some() {
                    continue;
                }
                let (aa, ab, bb) = (dot(&way[a], &way[a]), dot(&way[a], &way[b]), dot(&way[b], &way[b]));
                let det = aa * bb - ab * ab;
                if det.abs() < 1e-12 * aa * bb {
                    continue;
                }
                for &c in &movers[y + 1..] {
                    if collinear(a, c).is_some() || collinear(b, c).is_some() {
                        continue;
                    }
                    let (ac, bc) = (dot(&way[a], &way[c]), dot(&way[b], &way[c]));
                    let alpha = (ac * bb - bc * ab) / det;
                    let beta = (bc * aa - ac * ab) / det;
                    let rest: Vec<f64> =
                        (0..free.len()).map(|k| way[c][k] - alpha * way[a][k] - beta * way[b][k]).collect();
                    if norm(&rest) > 1e-7 * norm(&way[c]) {
                        continue;
                    }
                    let (Some((na, da)), Some((nb, db))) = (fraction(alpha), fraction(beta)) else { continue };
                    let d = da * db / gcd(da, db);
                    let k = vec![(c, d), (a, -na * d / da), (b, -nb * d / db)];
                    if small(&k) && k.iter().all(|(_, p)| *p != 0) {
                        found.push((k, 0.0));
                    }
                }
            }
        }
        // the number each gives (at the machine's values), and how it is reached
        let nf: Vec<usize> = (0..n).filter(|&i| !u.fixed[i]).collect();
        let col_of: IndexMap<usize, usize> = nf.iter().enumerate().map(|(c, &i)| (i, c)).collect();
        // M: relations x the quantities that are not fixed; the smallest y with Mᵀ y = k is M (MᵀM)⁺ k
        let mut mtm = vec![vec![0.0; nf.len()]; nf.len()];
        for (_, _, t, _) in &rels {
            for &(i, p) in t {
                for &(j, q) in t {
                    if let (Some(&ci), Some(&cj)) = (col_of.get(&i), col_of.get(&j)) {
                        mtm[ci][cj] += p * q;
                    }
                }
            }
        }
        let (mv, mvec) = eigen(&mtm);
        let mtop = mv.iter().fold(1.0f64, |a, x| a.max(x.abs()));
        // how each quantity goes with the seed: the constants held, the least motion of the rest that moves the seed
        // (its way of moving projected on the seed's)
        let still: Vec<usize> = (0..nf.len()).filter(|&e| mv[e].abs() < 1e-9 * mtop).collect();
        let way2 = |i: usize| -> Vec<f64> { still.iter().map(|&e| mvec[col_of[&i]][e]).collect() };
        for i in 0..n {
            if u.fixed[i] {
                u.exponent[i] = Some(0.0);
            }
        }
        if let Some(s) = u.seed.filter(|s| col_of.contains_key(s)) {
            let ws = way2(s);
            let ss = dot(&ws, &ws);
            if ss > 1e-18 {
                for &i in &nf {
                    let e = dot(&way2(i), &ws) / ss;
                    u.exponent[i] = Some(if e.abs() < 1e-9 { 0.0 } else { e });
                }
            }
        }
        for (k, number) in found.iter_mut() {
            *number = k.iter().map(|(i, p)| *p as f64 * u.value[*i].log10()).sum();
        }
        for (k, logn) in found {
            // (MᵀM)⁺ k through its eigenvectors
            let mut target = vec![0.0; nf.len()];
            for (i, p) in &k {
                target[col_of[i]] = *p as f64;
            }
            let mut z = vec![0.0; nf.len()];
            for e in 0..nf.len() {
                if mv[e].abs() < 1e-9 * mtop {
                    continue;
                }
                let proj: f64 = (0..nf.len()).map(|r| mvec[r][e] * target[r]).sum::<f64>() / mv[e];
                for r in 0..nf.len() {
                    z[r] += proj * mvec[r][e];
                }
            }
            let mut path: Vec<(f64, String, String)> = rels
                .iter()
                .map(|(name, text, t, _)| {
                    let w: f64 = t.iter().filter_map(|(i, p)| col_of.get(i).map(|&c| p * z[c])).sum();
                    (w, name.clone(), text.clone())
                })
                .filter(|(w, _, _)| w.abs() > 1e-6)
                .collect();
            path.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()).then(a.1.cmp(&b.1)));
            let written = path.len() == 1;
            let mut cancelled: Vec<usize> = rels
                .iter()
                .filter(|(name, _, _, _)| path.iter().any(|p| &p.1 == name))
                .flat_map(|(_, _, t, _)| t.iter().map(|(i, _)| *i))
                .filter(|&i| u.fixed[i])
                .collect();
            cancelled.sort();
            cancelled.dedup();
            u.found.push(Found {
                terms: k.iter().map(|(i, p)| (*i, *p as f64)).collect(),
                number: 10f64.powf(logn),
                path,
                written,
                cancelled,
            });
        }
        // found (not written) first, a number of 1 first, then the most constants cancelled on the way (what the
        // fewest quantities say of the most), the simplest powers (the smallest sum of them), the fewest quantities,
        // the longest way
        u.found.sort_by(|a, b| {
            let one = |f: &Found| (f.number.log10().abs() > 1e-4) as u8;
            let powers = |f: &Found| f.terms.iter().map(|t| t.1.abs().round() as i64).sum::<i64>();
            let key = |f: &Found| {
                (
                    f.written as u8,
                    one(f),
                    std::cmp::Reverse(f.cancelled.len()),
                    powers(f),
                    f.terms.len(),
                    std::cmp::Reverse(f.path.len()),
                )
            };
            key(a).cmp(&key(b)).then(a.terms.iter().map(|t| t.0).cmp(b.terms.iter().map(|t| t.0)))
        });
        u
    }
}

impl Found {
    /// Its equation, written with the names given (the quantities with a positive power on the left).
    pub fn text(&self, name: &dyn Fn(usize) -> String) -> String {
        let side = |sign: f64| -> String {
            let parts: Vec<String> = self
                .terms
                .iter()
                .filter(|(_, p)| p * sign > 0.0)
                .map(|(i, p)| {
                    let p = p.abs();
                    if p == 1.0 {
                        name(*i)
                    } else {
                        format!("{}^{}", name(*i), crate::lang::g(p))
                    }
                })
                .collect();
            parts.join(" · ")
        };
        let number = if (self.number - 1.0).abs() < 1e-4 {
            String::new()
        } else {
            format!("{} · ", crate::lang::g(self.number))
        };
        let right = side(-1.0);
        format!("{} = {number}{}", side(1.0), if right.is_empty() { "1".into() } else { right })
    }
}
