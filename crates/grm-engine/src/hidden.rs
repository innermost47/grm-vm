// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What a node really is, behind what is measured of it: a hidden state.
//!
//! A thermometer does not give the temperature: it gives the temperature plus its own error. A law learned
//! straight on the measurements counts that error as if the world moved, and its forecasts far ahead are wider
//! than they need be. Here the two are told apart:
//!
//! ```text
//! x_t = a_1 x_{t-1} + ... + a_p x_{t-p} + b . u_{t-1} + c + w_t     the world moves (w: variance q)
//! y_t = x_t + v_t                                                    and is measured (v: variance r)
//! ```
//!
//! - **Following** (a Kalman filter): at each moment, what x is now, and how sure of it, from what was measured
//!   (a missing measurement is no measurement: the state goes on, less sure).
//! - **Consolidating** (expectation-maximisation, Shumway and Stoffer 1982): every so often, on the moments it
//!   remembers one by one, the law (a, b, c), the world's noise q and the senses' noise r are learned again,
//!   from the states as they were (a Kalman smoother: what the past was, knowing what came after). As a brain
//!   consolidates what it lived.

use crate::time::{cholesky, inverse_spd, solve_spd, Rng};
use serde::{Deserialize, Serialize};

type Mat = Vec<Vec<f64>>;

fn zeros(n: usize, m: usize) -> Mat {
    vec![vec![0.0; m]; n]
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let (n, k, m) = (a.len(), b.len(), b[0].len());
    let mut out = zeros(n, m);
    for i in 0..n {
        for l in 0..k {
            if a[i][l] != 0.0 {
                for j in 0..m {
                    out[i][j] += a[i][l] * b[l][j];
                }
            }
        }
    }
    out
}

fn t(a: &Mat) -> Mat {
    let mut out = zeros(a[0].len(), a.len());
    for (i, row) in a.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            out[j][i] = *v;
        }
    }
    out
}

fn add(a: &Mat, b: &Mat, k: f64) -> Mat {
    a.iter().zip(b).map(|(x, y)| x.iter().zip(y).map(|(p, q)| p + k * q).collect()).collect()
}

fn mv(a: &Mat, v: &[f64]) -> Vec<f64> {
    a.iter().map(|row| row.iter().zip(v).map(|(x, y)| x * y).sum()).collect()
}

fn symmetric(a: &mut Mat) {
    for i in 0..a.len() {
        for j in 0..i {
            let m = 0.5 * (a[i][j] + a[j][i]);
            a[i][j] = m;
            a[j][i] = m;
        }
    }
}

/// The largest a law's roots may be: a law of the world fades or swings, it does not blow up.
pub const MAX_RADIUS: f64 = 0.9995;

/// How fast a law makes its state grow, at most (the spectral radius of its transition): above 1, it blows up.
pub fn spectral_radius(theta: &[f64], p: usize) -> f64 {
    match p {
        0 => 0.0,
        1 => theta[0].abs(),
        2 => {
            let (a1, a2) = (theta[0], theta[1]);
            let disc = a1 * a1 + 4.0 * a2;
            if disc >= 0.0 {
                ((a1.abs() + disc.sqrt()) / 2.0).max((a1.abs() - disc.sqrt()).abs() / 2.0)
            } else {
                (-a2).sqrt()
            }
        }
        _ => {
            // how fast the transition grows a state, over many moments
            let f = StateModel::transition(theta, p);
            let mut v = vec![1.0; p];
            let mut log_growth = 0.0;
            let steps = 400;
            for _ in 0..steps {
                v = mv(&f, &v);
                let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm == 0.0 || !norm.is_finite() {
                    return if norm == 0.0 { 0.0 } else { f64::INFINITY };
                }
                log_growth += norm.ln();
                v.iter_mut().for_each(|x| *x /= norm);
            }
            (log_growth / steps as f64).exp()
        }
    }
}

/// Rhythms a node is measured with, on top of its state: cycles (a day, a week) that come back at fixed periods,
/// each an amplitude and a phase (a cosine and a sine).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Rhythms {
    /// Their periods, in moments.
    pub periods: Vec<f64>,
    /// [cos, sin] weights of each period.
    pub beta: Vec<f64>,
    /// How unsure of them it is.
    pub cov: Mat,
}

impl Rhythms {
    pub fn new(periods: &[f64]) -> Rhythms {
        let k = 2 * periods.len();
        Rhythms { periods: periods.to_vec(), beta: vec![0.0; k], cov: zeros(k, k) }
    }

    /// The cosines and sines of its periods at moment t.
    pub fn harmonics(&self, t: f64) -> Vec<f64> {
        self.periods
            .iter()
            .flat_map(|p| {
                let a = std::f64::consts::TAU * t / p;
                [a.cos(), a.sin()]
            })
            .collect()
    }

    /// What its rhythms add at moment t (with weights `beta`, or its own).
    pub fn at(&self, t: f64, beta: Option<&[f64]>) -> f64 {
        let b = beta.unwrap_or(&self.beta);
        self.harmonics(t).iter().zip(b).map(|(h, w)| h * w).sum()
    }

    /// Weights it could have (drawn from what it believes of them).
    pub fn draw(&self, rng: &mut Rng) -> Vec<f64> {
        let k = self.beta.len();
        let l = cholesky(&self.cov).unwrap_or_else(|| zeros(k, k));
        let z: Vec<f64> = (0..k).map(|_| rng.normal()).collect();
        (0..k).map(|i| self.beta[i] + (0..=i).map(|j| l[i][j] * z[j]).sum::<f64>()).collect()
    }
}

/// The moments a node remembers one by one: what was measured (or nothing), the couplings' values the moment
/// before, and when.
#[derive(Debug, Clone, Copy)]
pub struct Remembered<'a> {
    pub ys: &'a [Option<f64>],
    pub us: &'a [Vec<f64>],
    pub ts: &'a [f64],
}

/// A node's hidden state and its law.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateModel {
    /// How many moments of its past the state holds (its lags).
    pub p: usize,
    /// The nodes whose value the moment before drives it.
    pub couplings: Vec<String>,
    /// [a_1 .. a_p, b_1 .. b_k, c].
    pub theta: Vec<f64>,
    /// How unsure of its law it is (the covariance of theta).
    pub cov_theta: Mat,
    /// The world's noise and the senses' noise (variances).
    pub q: f64,
    pub r: f64,
    /// The state now: [x_t, x_{t-1}, ..., x_{t-p+1}], and how sure of it.
    pub s: Vec<f64>,
    pub cov: Mat,
    /// The log-likelihood of what it remembers, under its law (after its last consolidation).
    pub loglik: f64,
    /// Its rhythms: what it is measured to be is its state plus them.
    pub rhythms: Rhythms,
}

struct Pass {
    mf: Vec<Vec<f64>>,
    pf: Vec<Mat>,
    mp: Vec<Vec<f64>>,
    pp: Vec<Mat>,
    loglik: f64,
}

impl StateModel {
    fn transition(theta: &[f64], p: usize) -> Mat {
        let mut f = zeros(p, p);
        f[0][..p].copy_from_slice(&theta[..p]);
        for i in 1..p {
            f[i][i - 1] = 1.0;
        }
        f
    }

    fn drive(theta: &[f64], p: usize, u: &[f64]) -> f64 {
        let k = theta.len() - p - 1;
        theta[p..p + k].iter().zip(u).map(|(b, x)| b * x).sum::<f64>() + theta[theta.len() - 1]
    }

    /// The state one moment later, before it is measured.
    fn predict_with(theta: &[f64], p: usize, q: f64, s: &[f64], cov: &Mat, u: &[f64]) -> (Vec<f64>, Mat) {
        let f = Self::transition(theta, p);
        let mut m = mv(&f, s);
        m[0] += Self::drive(theta, p, u);
        let mut pp = mul(&mul(&f, cov), &t(&f));
        pp[0][0] += q;
        (m, pp)
    }

    /// A measurement (or none) corrects the predicted state. Returns the log-likelihood of the measurement.
    fn correct(m: &mut [f64], pcov: &mut Mat, r: f64, y: Option<f64>) -> f64 {
        let Some(y) = y else { return 0.0 };
        let s = pcov[0][0] + r;
        let k: Vec<f64> = pcov.iter().map(|row| row[0] / s).collect();
        let innov = y - m[0];
        for i in 0..m.len() {
            m[i] += k[i] * innov;
        }
        let row0 = pcov[0].clone();
        for i in 0..m.len() {
            for j in 0..m.len() {
                pcov[i][j] -= k[i] * row0[j];
            }
        }
        symmetric(pcov);
        -0.5 * (innov * innov / s + s.ln() + std::f64::consts::TAU.ln())
    }

    /// A forward pass of the filter over remembered moments: `law` is (theta, p, q, r), `start` the state before
    /// the first moment and how sure of it.
    fn pass(law: (&[f64], usize, f64, f64), ys: &[Option<f64>], us: &[Vec<f64>], start: (&[f64], &Mat)) -> Pass {
        let (theta, p, q, r) = law;
        let (s0, p0) = start;
        let n = ys.len();
        let mut out = Pass { mf: vec![], pf: vec![], mp: vec![], pp: vec![], loglik: 0.0 };
        let (mut s, mut cov) = (s0.to_vec(), p0.clone());
        for k in 0..n {
            let (mut m, mut pc) = Self::predict_with(theta, p, q, &s, &cov, &us[k]);
            out.mp.push(m.clone());
            out.pp.push(pc.clone());
            out.loglik += Self::correct(&mut m, &mut pc, r, ys[k]);
            out.mf.push(m.clone());
            out.pf.push(pc.clone());
            s = m;
            cov = pc;
        }
        out
    }

    /// Learns a law, the world's noise and the senses' noise from what it remembers one by one (`ys`, with the
    /// couplings' values the moment before each, `us`), starting from a first guess; then follows the state to
    /// now. None when there is too little to learn from.
    /// `rhythms`: the cycles it is measured with (their weights: a first guess).
    pub fn fit(
        p: usize,
        couplings: &[String],
        remembered: Remembered,
        start: (Vec<f64>, f64, f64),
        rhythms: Rhythms,
        iterations: usize,
    ) -> Option<StateModel> {
        let Remembered { ys, us, ts } = remembered;
        let n = ys.len();
        let observed: Vec<f64> = ys.iter().flatten().copied().collect();
        if observed.len() < 8 * (p + couplings.len() + 2 * rhythms.periods.len() + 1) || n != us.len() || n != ts.len()
        {
            return None;
        }
        let mut rh = rhythms;
        let hs: Vec<Vec<f64>> = ts.iter().map(|t| rh.harmonics(*t)).collect();
        // rhythms not known yet start from what the measurements say of them straight (else the state, free to
        // wander, would take the whole day for itself, and learning would crawl out of it)
        let kr = rh.beta.len();
        if kr > 0 && rh.beta.iter().all(|b| *b == 0.0) {
            let mut ah = zeros(kr, kr);
            let mut bh = vec![0.0; kr];
            let my = ys.iter().flatten().sum::<f64>() / ys.iter().flatten().count().max(1) as f64;
            for (y, h) in ys.iter().zip(&hs) {
                if let Some(y) = y {
                    for i in 0..kr {
                        bh[i] += h[i] * (y - my);
                        for j in 0..kr {
                            ah[i][j] += h[i] * h[j];
                        }
                    }
                }
            }
            for i in 0..kr {
                ah[i][i] += 1e-9 * (1.0 + ah[i][i]);
            }
            if let Some(beta) = solve_spd(&ah, &bh) {
                rh.beta = beta;
            }
        }
        let without = |rh: &Rhythms| -> Vec<Option<f64>> {
            ys.iter()
                .zip(&hs)
                .map(|(y, h)| y.map(|y| y - h.iter().zip(&rh.beta).map(|(a, b)| a * b).sum::<f64>()))
                .collect()
        };
        let mean = observed.iter().sum::<f64>() / observed.len() as f64;
        let var = observed.iter().map(|y| (y - mean).powi(2)).sum::<f64>() / observed.len() as f64;
        let floor = (var * 1e-8).max(1e-300);
        // the couplings' values are learned with centred values (a humidity of 80 % and a constant would hardly
        // be told apart, and the learning would crawl); the law is given back in the values as they are
        let k = couplings.len();
        let centre: Vec<f64> = (0..k).map(|j| us.iter().map(|u| u[j]).sum::<f64>() / n as f64).collect();
        let uc: Vec<Vec<f64>> = us.iter().map(|u| u.iter().zip(&centre).map(|(x, c)| x - c).collect()).collect();
        let to_centred = |th: &mut Vec<f64>| {
            let last = th.len() - 1;
            th[last] += (0..k).map(|j| th[p + j] * centre[j]).sum::<f64>();
        };
        let first = observed[0];
        let s0 = vec![first; p];
        let mut p0 = zeros(p, p);
        for i in 0..p {
            p0[i][i] = var.max(floor) * 10.0;
        }
        let (mut theta, mut q, mut r) = start;
        to_centred(&mut theta);
        let dim = theta.len();
        let mut loglik = f64::NEG_INFINITY;
        let mut cov_theta = zeros(dim, dim);
        for _ in 0..iterations {
            let f = Self::transition(&theta, p);
            let ya = without(&rh);
            let pass = Self::pass((&theta, p, q, r), &ya, &uc, (&s0, &p0));
            // the smoother: what each state was, knowing what came after
            let mut ms = pass.mf.clone();
            let mut ps = pass.pf.clone();
            let mut cross: Vec<Mat> = vec![zeros(p, p); n]; // Cov(s_k, s_{k-1})
            for k in (0..n.saturating_sub(1)).rev() {
                let ppi = inverse_spd(&pass.pp[k + 1]).unwrap_or_else(|| zeros(p, p));
                let j = mul(&mul(&pass.pf[k], &t(&f)), &ppi);
                let dm: Vec<f64> = ms[k + 1].iter().zip(&pass.mp[k + 1]).map(|(a, b)| a - b).collect();
                let corr = mv(&j, &dm);
                for i in 0..p {
                    ms[k][i] = pass.mf[k][i] + corr[i];
                }
                let dp = add(&ps[k + 1], &pass.pp[k + 1], -1.0);
                ps[k] = add(&pass.pf[k], &mul(&mul(&j, &dp), &t(&j)), 1.0);
                symmetric(&mut ps[k]);
                cross[k + 1] = mul(&ps[k + 1], &t(&j));
            }
            // the law: a regression of each state on the one before (with what that one might have been)
            let mut a = zeros(dim, dim);
            let mut b = vec![0.0; dim];
            let mut xx = 0.0;
            for k in 1..n {
                let mut z = ms[k - 1].clone();
                z.extend(uc[k].iter().copied());
                z.push(1.0);
                for i in 0..dim {
                    for jj in 0..dim {
                        let extra = if i < p && jj < p { ps[k - 1][i][jj] } else { 0.0 };
                        a[i][jj] += z[i] * z[jj] + extra;
                    }
                    let extra = if i < p { cross[k][0][i] } else { 0.0 };
                    b[i] += ms[k][0] * z[i] + extra;
                }
                xx += ps[k][0][0] + ms[k][0] * ms[k][0];
            }
            for i in 0..dim {
                a[i][i] += 1e-9 * (1.0 + a[i][i]); // a little prior, so it never divides by nothing
            }
            let Some(th) = solve_spd(&a, &b) else { break };
            theta = th;
            // a law that would blow up is not a law of the world: its swing is brought back inside (each lag k
            // shrunk by rho^k shrinks every root of the law by rho)
            let radius = spectral_radius(&theta, p);
            if radius > MAX_RADIUS {
                let rho = MAX_RADIUS / radius;
                for k in 0..p {
                    theta[k] *= rho.powi(k as i32 + 1);
                }
            }
            // what the law leaves unexplained: E[(x - theta . z)^2], whatever theta is
            let ath = mv(&a, &theta);
            let quad: f64 = theta.iter().zip(&ath).map(|(x, y)| x * y).sum();
            let lin: f64 = theta.iter().zip(&b).map(|(x, y)| x * y).sum();
            q = ((xx - 2.0 * lin + quad) / (n - 1) as f64).max(floor);
            // its rhythms: what is measured, less what its state was, regressed on their cosines and sines
            if kr > 0 {
                let mut ah = zeros(kr, kr);
                let mut bh = vec![0.0; kr];
                for k in 0..n {
                    if let Some(y) = ys[k] {
                        for i in 0..kr {
                            bh[i] += hs[k][i] * (y - ms[k][0]);
                            for j in 0..kr {
                                ah[i][j] += hs[k][i] * hs[k][j];
                            }
                        }
                    }
                }
                for i in 0..kr {
                    ah[i][i] += 1e-9 * (1.0 + ah[i][i]);
                }
                if let Some(beta) = solve_spd(&ah, &bh) {
                    rh.beta = beta;
                }
                rh.cov = inverse_spd(&ah).unwrap_or_else(|| zeros(kr, kr));
            }
            let mut sr = 0.0;
            let mut no = 0.0;
            for k in 0..n {
                if let Some(y) = ys[k] {
                    let rhythm: f64 = hs[k].iter().zip(&rh.beta).map(|(a, b)| a * b).sum();
                    sr += (y - rhythm - ms[k][0]).powi(2) + ps[k][0][0];
                    no += 1.0;
                }
            }
            r = (sr / no).max(floor);
            for row in rh.cov.iter_mut() {
                for v in row.iter_mut() {
                    *v *= r;
                }
            }
            cov_theta = inverse_spd(&a)
                .map(|inv| inv.iter().map(|row| row.iter().map(|v| v * q).collect()).collect())
                .unwrap_or(cov_theta);
            let gain = pass.loglik - loglik;
            loglik = pass.loglik;
            if gain.abs() < 1e-7 * (1.0 + loglik.abs()) {
                break;
            }
        }
        // back to the values as they are: c = c' - b . centre (and its uncertainty with it)
        let dim = theta.len();
        let mut tm = zeros(dim, dim);
        for i in 0..dim {
            tm[i][i] = 1.0;
        }
        for j in 0..k {
            tm[dim - 1][p + j] = -centre[j];
        }
        theta = mv(&tm, &theta);
        cov_theta = mul(&mul(&tm, &cov_theta), &t(&tm));
        let last = Self::pass((&theta, p, q, r), &without(&rh), us, (&s0, &p0));
        Some(StateModel {
            p,
            couplings: couplings.to_vec(),
            theta,
            cov_theta,
            q,
            r,
            s: last.mf.last()?.clone(),
            cov: last.pf.last()?.clone(),
            loglik: last.loglik,
            rhythms: rh,
        })
    }

    /// A moment t: the state goes on (driven by the couplings' values the moment before), then what was measured
    /// (less its rhythms) corrects it.
    pub fn step(&mut self, y: Option<f64>, u: &[f64], t: f64) {
        let (mut m, mut pc) = Self::predict_with(&self.theta, self.p, self.q, &self.s, &self.cov, u);
        let rhythm = self.rhythms.at(t, None);
        Self::correct(&mut m, &mut pc, self.r, y.map(|y| y - rhythm));
        self.s = m;
        self.cov = pc;
    }

    /// What it expects to measure at moment t, the next: (mean, variance), its state's uncertainty, the world's
    /// noise and the senses' noise included.
    pub fn next_measure(&self, u: &[f64], t: f64) -> (f64, f64) {
        let (m, pc) = Self::predict_with(&self.theta, self.p, self.q, &self.s, &self.cov, u);
        (m[0] + self.rhythms.at(t, None), pc[0][0] + self.r)
    }

    /// What it believes it is at moment t (its state and its rhythms), without its senses' error.
    pub fn value(&self, t: f64) -> f64 {
        self.s[0] + self.rhythms.at(t, None)
    }

    /// A law it could have (drawn from what it believes of it), or its mean.
    pub fn law(&self, rng: Option<&mut Rng>) -> Vec<f64> {
        match rng {
            None => self.theta.clone(),
            Some(rng) => {
                let l = cholesky(&self.cov_theta).unwrap_or_else(|| zeros(self.theta.len(), self.theta.len()));
                let z: Vec<f64> = (0..self.theta.len()).map(|_| rng.normal()).collect();
                (0..self.theta.len()).map(|i| self.theta[i] + (0..=i).map(|k| l[i][k] * z[k]).sum::<f64>()).collect()
            }
        }
    }

    /// A state it could be in now (drawn from how sure of it it is), or its mean.
    pub fn state(&self, rng: Option<&mut Rng>) -> Vec<f64> {
        match rng {
            None => self.s.clone(),
            Some(rng) => {
                let l = cholesky(&self.cov).unwrap_or_else(|| zeros(self.p, self.p));
                let z: Vec<f64> = (0..self.p).map(|_| rng.normal()).collect();
                (0..self.p).map(|i| self.s[i] + (0..=i).map(|k| l[i][k] * z[k]).sum::<f64>()).collect()
            }
        }
    }

    /// One moment of a run: the next state from a state, a law and the couplings' values (plus the world's noise).
    pub fn advance(theta: &[f64], p: usize, s: &[f64], u: &[f64], noise: f64) -> Vec<f64> {
        let x: f64 = theta[..p].iter().zip(s).map(|(a, b)| a * b).sum::<f64>() + Self::drive(theta, p, u) + noise;
        let mut next = vec![x];
        next.extend_from_slice(&s[..p - 1]);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_tells_the_world_from_the_senses() {
        // a swing (period 24) that moves a little by itself, measured by a poor thermometer
        let mut rng = Rng::new(9);
        let (a1, a2) = (2.0 * 0.999 * (std::f64::consts::TAU / 24.0).cos(), -0.999f64.powi(2));
        let (mut x1, mut x2) = (5.0, 4.8);
        let (q, r) = (0.02f64.powi(2), 0.5f64.powi(2));
        let mut ys = vec![];
        for _ in 0..600 {
            let x = a1 * x1 + a2 * x2 + q.sqrt() * rng.normal();
            x2 = x1;
            x1 = x;
            ys.push(Some(x + r.sqrt() * rng.normal()));
        }
        ys[300] = None; // a moment nothing was measured
        let us = vec![vec![]; ys.len()];
        let ts: Vec<f64> = (0..ys.len()).map(|k| k as f64).collect();
        let m = StateModel::fit(
            2,
            &[],
            Remembered { ys: &ys, us: &us, ts: &ts },
            (vec![1.0, 0.0, 0.0], 0.1, 0.1),
            Rhythms::default(),
            200,
        )
        .unwrap();
        assert!((m.r.sqrt() / 0.5 - 1.0).abs() < 0.15, "senses: {}", m.r.sqrt());
        assert!(m.q.sqrt() < 0.1, "world: {}", m.q.sqrt());
        assert!((m.theta[0] - a1).abs() < 0.05 && (m.theta[1] - a2).abs() < 0.05, "{:?}", m.theta);
        // what it believes now is far closer to the truth than the last measurement
        assert!((m.s[0] - x1).abs() < 0.3, "{} vs {x1}", m.s[0]);
    }

    #[test]
    fn a_law_never_blows_up() {
        assert!((spectral_radius(&[0.5, 0.0], 1) - 0.5).abs() < 1e-12);
        assert!((spectral_radius(&[1.8, -0.9, 0.0], 2) - 0.9f64.sqrt()).abs() < 1e-12);
        let r3 = spectral_radius(&[1.8, -0.9, 0.05, 0.0], 3);
        assert!(r3 > 0.8 && r3 < 1.2, "{r3}");
        // an explosive world, fitted: what is learned stays inside
        let mut rng = Rng::new(2);
        let mut x = 1.0;
        let ys: Vec<Option<f64>> = (0..200)
            .map(|_| {
                x = 1.02 * x + 0.1 * rng.normal();
                Some(x)
            })
            .collect();
        let us = vec![vec![]; ys.len()];
        let ts: Vec<f64> = (0..ys.len()).map(|k| k as f64).collect();
        let m = StateModel::fit(
            1,
            &[],
            Remembered { ys: &ys, us: &us, ts: &ts },
            (vec![1.0, 0.0], 0.1, 0.1),
            Rhythms::default(),
            50,
        )
        .unwrap();
        assert!(spectral_radius(&m.theta, 1) <= MAX_RADIUS + 1e-12, "{:?}", m.theta);
    }

    #[test]
    fn a_rhythm_is_told_from_the_weather() {
        // a day that comes back (amplitude 5) on top of a weather that wanders, read by a thermometer
        let mut rng = Rng::new(4);
        let mut weather = 0.0;
        let (mut ys, mut ts) = (vec![], vec![]);
        for k in 0..480 {
            weather = 0.95 * weather + 0.3 * rng.normal();
            let day = 5.0 * (std::f64::consts::TAU * (k as f64 - 9.0) / 24.0).sin();
            ys.push(Some(12.0 + weather + day + 0.2 * rng.normal()));
            ts.push(k as f64);
        }
        let us = vec![vec![]; ys.len()];
        let m = StateModel::fit(
            1,
            &[],
            Remembered { ys: &ys, us: &us, ts: &ts },
            (vec![1.0, 0.0], 0.1, 0.1),
            Rhythms::new(&[24.0]),
            100,
        )
        .unwrap();
        let amplitude = (m.rhythms.beta[0].powi(2) + m.rhythms.beta[1].powi(2)).sqrt();
        assert!((amplitude - 5.0).abs() < 0.5, "{amplitude}");
        assert!((m.theta[0] - 0.95).abs() < 0.1, "{:?}", m.theta);
        assert!((m.q.sqrt() - 0.3).abs() < 0.1 && (m.r.sqrt() - 0.2).abs() < 0.1, "q {} r {}", m.q.sqrt(), m.r.sqrt());
    }
}
