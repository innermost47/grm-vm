// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The eye: pictures into the network.
//!
//! An eye is a network of its own, as a retina is: every pixel a node, tied to its neighbours ("I look like the one
//! beside me, more or less"), and a pyramid above them (each node of a level the mean of four below), up to a
//! glance of a few nodes that the world's network hears. It computes by itself, near what it sees, and sends on
//! only what is worth it.
//!
//! What it gives of a picture:
//! - **what it sees, cleaned**: its noise taken away, its missing pixels filled, its edges kept (a neighbour that
//!   differs much is believed less: robust weights, as the Robot Web keeps its faulty measurements from spreading);
//! - **its surprise**: at each pixel, how far what came is from what its neighbours said it would be, in sigmas
//!   (what belief propagation already knows: the message every other part of the eye sends the pixel);
//! - **what changed** since the last picture, in sigmas;
//! - **a glance**: the top of its pyramid, a few nodes the world's network hears as inputs.
//!
//! Recognising what is in a picture is another step (features learned), not this one.

use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::bail;
use crate::error::Result;
use crate::machine::Machine;
use crate::network::{Kind, Network, NodeSpec, Provenance, Relation};

/// How sure of each pixel the eye is, by default (on 0 .. 1).
pub const SENSE: f64 = 0.1;
/// How much neighbours may differ, by default.
pub const SMOOTH: f64 = 0.05;
/// A neighbour that differs by this many of its sigmas is believed half as much, and less and less beyond
/// (Cauchy weights: an edge is let be an edge).
pub const ROBUST: f64 = 2.0;
/// The glance: the top of the pyramid has at most this many nodes a side.
pub const GLANCE: usize = 8;
/// How much a picture may change from one to the next, by itself (on 0 .. 1).
pub const MOTION: f64 = 0.02;

/// A picture: width x height x channels, row by row, each value in 0 .. 1.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    /// NaN: a pixel not seen.
    pub data: Vec<f32>,
}

impl Picture {
    /// The same picture made smaller, so that its longest side is at most `side` (each new pixel the mean of the
    /// pixels it covers; pixels not seen are left out of the mean). A smaller picture is given back as it is.
    pub fn shrunk(&self, side: usize) -> Picture {
        let longest = self.width.max(self.height);
        if longest <= side || side == 0 {
            return self.clone();
        }
        let k = longest as f64 / side as f64;
        let w = ((self.width as f64 / k).round() as usize).max(1);
        let h = ((self.height as f64 / k).round() as usize).max(1);
        let c = self.channels;
        let mut data = vec![f32::NAN; w * h * c];
        for y in 0..h {
            let (y0, y1) = (
                (y as f64 * k) as usize,
                (((y + 1) as f64 * k) as usize).min(self.height).max((y as f64 * k) as usize + 1),
            );
            for x in 0..w {
                let (x0, x1) = (
                    (x as f64 * k) as usize,
                    (((x + 1) as f64 * k) as usize).min(self.width).max((x as f64 * k) as usize + 1),
                );
                for ch in 0..c {
                    let (mut sum, mut n) = (0.0f64, 0usize);
                    for yy in y0..y1.min(self.height) {
                        for xx in x0..x1.min(self.width) {
                            let v = self.data[(yy * self.width + xx) * c + ch];
                            if v.is_finite() {
                                sum += v as f64;
                                n += 1;
                            }
                        }
                    }
                    if n > 0 {
                        data[(y * w + x) * c + ch] = (sum / n as f64) as f32;
                    }
                }
            }
        }
        Picture { width: w, height: h, channels: c, data }
    }

    pub fn new(width: usize, height: usize, channels: usize) -> Picture {
        Picture { width, height, channels, data: vec![0.0; width * height * channels] }
    }

    /// Its colour as a whole, in OKLab (lightness 0 .. 1, green to red, blue to yellow): the mean of its pixels
    /// (those not seen left out). A grey picture is grey.
    pub fn mean_oklab(&self) -> (f64, f64, f64) {
        let lin = |c: f64| if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        let (mut sl, mut sa, mut sb, mut n) = (0.0, 0.0, 0.0, 0.0);
        for y in 0..self.height {
            for x in 0..self.width {
                let (r, g, b) = if self.channels >= 3 {
                    (self.at(x, y, 0) as f64, self.at(x, y, 1) as f64, self.at(x, y, 2) as f64)
                } else {
                    let v = self.at(x, y, 0) as f64;
                    (v, v, v)
                };
                if !(r.is_finite() && g.is_finite() && b.is_finite()) {
                    continue;
                }
                let (r, g, b) = (lin(r), lin(g), lin(b));
                let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
                let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
                let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
                sl += 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
                sa += 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
                sb += 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
                n += 1.0;
            }
        }
        if n == 0.0 {
            (0.5, 0.0, 0.0)
        } else {
            (sl / n, sa / n, sb / n)
        }
    }

    pub fn at(&self, x: usize, y: usize, c: usize) -> f32 {
        self.data[(y * self.width + x) * self.channels + c]
    }

    pub fn set(&mut self, x: usize, y: usize, c: usize, v: f32) {
        self.data[(y * self.width + x) * self.channels + c] = v;
    }
}

/// What the eye made of a picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Sight {
    pub clean: Picture,
    /// How sure it is of each pixel (same layout as the picture).
    pub sigma: Picture,
    /// How far each pixel was from what its neighbours said, in sigmas.
    pub surprise: Picture,
    /// How far each pixel moved since the last picture, in sigmas (None for the first).
    pub change: Option<Picture>,
    /// The top of its pyramid: (width, height, values row by row, channel by channel).
    pub glance: (usize, usize, Vec<f64>),
    pub rounds: u64,
    pub elapsed: Duration,
}

impl Sight {
    /// Where it was most surprised: (x, y, sigmas), the strongest first.
    pub fn surprises(&self, how_many: usize) -> Vec<(usize, usize, f32)> {
        let s = &self.surprise;
        let mut all: Vec<(usize, usize, f32)> = (0..s.height)
            .flat_map(|y| (0..s.width).map(move |x| (x, y)))
            .map(|(x, y)| (x, y, (0..s.channels).map(|c| s.at(x, y, c)).fold(0.0f32, f32::max)))
            .collect();
        all.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(how_many);
        all
    }
}

/// An eye: its network, its machine, and where each pixel is in it.
#[derive(Debug, Clone)]
pub struct Eye {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub sense: f64,
    pub smooth: f64,
    /// The sizes of its levels, from the pixels up.
    pub levels: Vec<(usize, usize)>,
    machine: Machine,
    /// The relations between neighbours: (factor, its weight when the neighbours agree).
    neighbours: Vec<(usize, f64)>,
    /// For each pixel and channel: its variable and its anchor (the measurement).
    pixels: Vec<(usize, usize)>,
    /// The top level's variables, channel by channel.
    top: Vec<usize>,
    previous: Option<(Vec<f64>, Vec<f64>)>,
}

fn node_name(eye: &str, c: usize, level: usize, x: usize, y: usize) -> String {
    format!("{eye}_c{c}_l{level}_{x}_{y}")
}

fn sum(name: String, terms: Vec<(f64, &str)>, sigma: f64, origin: &Provenance) -> Relation {
    Relation {
        name,
        kind: Kind::Sum {
            terms: terms.into_iter().map(|(c, n)| (c, IndexMap::from([(n.to_string(), 1.0)]))).collect(),
        },
        sigma,
        when: vec![],
        text: String::new(),
        origin: origin.clone(),
        strength: 1.0,
    }
}

impl Eye {
    /// An eye for pictures of this size: `sense`, how sure it is of each pixel; `smooth`, how much neighbours may
    /// differ.
    pub fn new(name: &str, width: usize, height: usize, channels: usize, sense: f64, smooth: f64) -> Result<Eye> {
        if width == 0 || height == 0 || channels == 0 || width * height * channels > 4_000_000 {
            bail!("an eye sees from 1 to 4 million values");
        }
        let origin = Provenance::of(format!("eye:{name}"));
        let mut net = Network::new();
        let mut levels = vec![(width, height)];
        while levels.last().is_some_and(|(w, h)| *w > GLANCE || *h > GLANCE) {
            let (w, h) = *levels.last().unwrap();
            levels.push((w.div_ceil(2), h.div_ceil(2)));
        }
        for c in 0..channels {
            for (l, (w, h)) in levels.iter().enumerate() {
                for y in 0..*h {
                    for x in 0..*w {
                        let mut spec = NodeSpec::new(0.5, true);
                        spec.origin = origin.clone();
                        net.nodes.insert(node_name(name, c, l, x, y), spec);
                    }
                }
            }
            // neighbours at the pixels: alike, more or less
            for y in 0..height {
                for x in 0..width {
                    let here = node_name(name, c, 0, x, y);
                    if x + 1 < width {
                        let there = node_name(name, c, 0, x + 1, y);
                        net.relations.push(sum(
                            format!("{here}_h"),
                            vec![(1.0, &here), (-1.0, &there)],
                            smooth,
                            &origin,
                        ));
                    }
                    if y + 1 < height {
                        let there = node_name(name, c, 0, x, y + 1);
                        net.relations.push(sum(
                            format!("{here}_v"),
                            vec![(1.0, &here), (-1.0, &there)],
                            smooth,
                            &origin,
                        ));
                    }
                }
            }
            // the pyramid: each node the mean of the (up to) four below
            for l in 1..levels.len() {
                let (w, h) = levels[l];
                let (bw, bh) = levels[l - 1];
                for y in 0..h {
                    for x in 0..w {
                        let up = node_name(name, c, l, x, y);
                        let below: Vec<String> =
                            [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)]
                                .into_iter()
                                .filter(|(a, b)| *a < bw && *b < bh)
                                .map(|(a, b)| node_name(name, c, l - 1, a, b))
                                .collect();
                        let k = below.len() as f64;
                        let mut terms = vec![(1.0, up.as_str())];
                        terms.extend(below.iter().map(|b| (-1.0 / k, b.as_str())));
                        net.relations.push(sum(format!("{up}_mean"), terms, 1e-3, &origin));
                    }
                }
            }
        }
        let mut machine = Machine::new(net)?;
        // a picture right to a part in a hundred thousand is right
        machine.tolerance = crate::machine::Tolerance { mean: 1e-5, precision: 1e-3 };
        let factor: std::collections::HashMap<&str, usize> =
            machine.factors.iter().enumerate().map(|(k, f)| (f.name.as_str(), k)).collect();
        let neighbours: Vec<(usize, f64)> = machine
            .factors
            .iter()
            .enumerate()
            .filter(|(_, f)| f.name.ends_with("_h") || f.name.ends_with("_v"))
            .map(|(k, f)| (k, f.w))
            .collect();
        let mut pixels = vec![];
        for y in 0..height {
            for x in 0..width {
                for c in 0..channels {
                    let n = node_name(name, c, 0, x, y);
                    pixels.push((machine.index[&n], factor[format!("anchor {n}").as_str()]));
                }
            }
        }
        let (tw, th) = *levels.last().unwrap();
        let top_level = levels.len() - 1;
        let mut top = vec![];
        for c in 0..channels {
            for y in 0..th {
                for x in 0..tw {
                    top.push(machine.index[&node_name(name, c, top_level, x, y)]);
                }
            }
        }
        Ok(Eye {
            name: name.to_string(),
            width,
            height,
            channels,
            sense,
            smooth,
            levels,
            machine,
            neighbours,
            pixels,
            top,
            previous: None,
        })
    }

    /// How many nodes and relations it is made of.
    pub fn size(&self) -> (usize, usize) {
        (self.machine.var_names.len(), self.machine.net.relations.len())
    }

    pub fn set_threads(&mut self, threads: Option<usize>) {
        self.machine.set_threads(threads);
    }

    /// Sees a picture: measures its pixels (a NaN is not seen), settles (keeping its edges), and says what it saw.
    pub fn see(&mut self, picture: &Picture, budget: Duration) -> Result<Sight> {
        if (picture.width, picture.height, picture.channels) != (self.width, self.height, self.channels) {
            bail!(
                "{}: an eye of {}x{}x{} cannot see {}x{}x{}",
                self.name,
                self.width,
                self.height,
                self.channels,
                picture.width,
                picture.height,
                picture.channels
            );
        }
        let t0 = Instant::now();
        let r0 = self.machine.rounds;
        let names: Vec<String> = self.pixels.iter().map(|(v, _)| self.machine.var_names[*v].clone()).collect();
        for (k, name) in names.iter().enumerate() {
            let v = picture.data[k];
            if v.is_finite() {
                self.machine.observe(name, v as f64, Some(self.sense))?;
            } else {
                self.machine.forget(name);
            }
        }
        // the neighbours that differ much are believed less (robust weights, drawn again three times)
        let w0: Vec<(usize, f64)> = self.neighbours.clone();
        self.machine.reweigh(&w0);
        let each = budget / 4;
        for pass in 0..4 {
            let deadline = Instant::now() + each;
            while !self.machine.quiet() && Instant::now() < deadline {
                self.machine.round();
            }
            if pass < 3 {
                let updates: Vec<(usize, f64)> = w0
                    .iter()
                    .map(|&(fi, w)| {
                        let f = &self.machine.factors[fi];
                        let r = (self.machine.x[f.vars[0]] - self.machine.x[f.vars[1]]).abs() * w.sqrt();
                        (fi, w / (1.0 + (r / ROBUST).powi(2)))
                    })
                    .collect();
                self.machine.reweigh(&updates);
            }
        }
        // what it saw, how sure, and how surprised
        let mut clean = Picture::new(self.width, self.height, self.channels);
        let mut sigma = clean.clone();
        let mut surprise = clean.clone();
        let mut means = vec![];
        let mut sigmas = vec![];
        for (k, &(v, a)) in self.pixels.iter().enumerate() {
            let x = self.machine.x[v];
            let lam = self.machine.lam[v];
            let sd = if lam > 0.0 { 1.0 / lam.sqrt() } else { f64::INFINITY };
            clean.data[k] = x as f32;
            sigma.data[k] = sd as f32;
            means.push(x);
            sigmas.push(sd);
            // what every other part of the eye says this pixel is: the message its measurement hears
            let f = &self.machine.factors[a];
            let (cl, ce) = (f.in_lam[0], f.in_eta[0]);
            let seen = picture.data[k];
            surprise.data[k] = if seen.is_finite() && cl > 0.0 {
                ((seen as f64 - ce / cl).abs() / (1.0 / cl + self.sense * self.sense).sqrt()) as f32
            } else {
                0.0
            };
        }
        let change = self.previous.as_ref().map(|(m, s)| {
            let mut p = Picture::new(self.width, self.height, self.channels);
            for k in 0..p.data.len() {
                let seen = picture.data[k];
                p.data[k] = if seen.is_finite() {
                    ((seen as f64 - m[k]).abs()
                        / (s[k].min(1.0).powi(2) + self.sense * self.sense + MOTION * MOTION).sqrt())
                        as f32
                } else {
                    0.0
                };
            }
            p
        });
        self.previous = Some((means, sigmas));
        let (tw, th) = *self.levels.last().unwrap();
        let glance = (tw, th, self.top.iter().map(|&v| self.machine.x[v]).collect());
        Ok(Sight { clean, sigma, surprise, change, glance, rounds: self.machine.rounds - r0, elapsed: t0.elapsed() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Rng;

    /// A picture of a bright square on a soft gradient.
    fn scene(w: usize, h: usize, square: (usize, usize)) -> Picture {
        let mut p = Picture::new(w, h, 1);
        for y in 0..h {
            for x in 0..w {
                let inside = x >= square.0 && x < square.0 + w / 4 && y >= square.1 && y < square.1 + h / 4;
                p.set(x, y, 0, if inside { 0.9 } else { 0.2 + 0.3 * x as f32 / w as f32 });
            }
        }
        p
    }

    fn rmse(a: &Picture, b: &Picture, only: Option<&[usize]>) -> f64 {
        let idx: Vec<usize> = only.map(|o| o.to_vec()).unwrap_or_else(|| (0..a.data.len()).collect());
        (idx.iter().map(|&k| ((a.data[k] - b.data[k]) as f64).powi(2)).sum::<f64>() / idx.len() as f64).sqrt()
    }

    #[test]
    fn it_cleans_a_picture_keeps_its_edges_and_fills_what_is_missing() {
        let truth = scene(48, 48, (12, 12));
        let mut rng = Rng::new(4);
        let mut seen = truth.clone();
        let mut missing = vec![];
        for k in 0..seen.data.len() {
            if rng.uniform() < 0.15 {
                seen.data[k] = f32::NAN;
                missing.push(k);
            } else {
                seen.data[k] += (0.1 * rng.normal()) as f32;
            }
        }
        let mut eye = Eye::new("eye", 48, 48, 1, SENSE, SMOOTH).unwrap();
        let sight = eye.see(&seen, Duration::from_secs(20)).unwrap();
        let noisy: Vec<usize> = (0..seen.data.len()).filter(|k| !missing.contains(k)).collect();
        let before = rmse(&seen, &truth, Some(&noisy));
        let after = rmse(&sight.clean, &truth, Some(&noisy));
        assert!(after < 0.5 * before, "cleaned {after} / seen {before}");
        assert!(
            rmse(&sight.clean, &truth, Some(&missing)) < 0.08,
            "missing {}",
            rmse(&sight.clean, &truth, Some(&missing))
        );
        // the square's edge is kept: a pixel inside and one outside, side by side, stay apart
        assert!(
            sight.clean.at(12, 20, 0) - sight.clean.at(11, 20, 0) > 0.4,
            "{} {}",
            sight.clean.at(12, 20, 0),
            sight.clean.at(11, 20, 0)
        );
        assert_eq!(sight.glance.0 * sight.glance.1, sight.glance.2.len());
    }

    #[test]
    fn it_is_surprised_where_something_does_not_fit_and_sees_what_moved() {
        let mut first = scene(32, 32, (4, 4));
        first.set(24, 20, 0, 1.0); // a spot that does not fit
        let mut eye = Eye::new("eye", 32, 32, 1, 0.02, SMOOTH).unwrap();
        let sight = eye.see(&first, Duration::from_secs(20)).unwrap();
        let top = sight.surprises(1)[0];
        assert_eq!((top.0, top.1), (24, 20), "{top:?}");
        // the square moves: what changed is where it was and where it is
        let second = scene(32, 32, (16, 4));
        let sight = eye.see(&second, Duration::from_secs(20)).unwrap();
        let change = sight.change.unwrap();
        assert!(
            change.at(6, 6, 0) > 5.0 && change.at(18, 6, 0) > 5.0,
            "{} {}",
            change.at(6, 6, 0),
            change.at(18, 6, 0)
        );
        assert!(change.at(2, 28, 0) < 3.0, "{}", change.at(2, 28, 0));
    }

    #[test]
    fn a_big_picture_is_made_smaller_keeping_its_means() {
        let p = Picture { width: 400, height: 200, channels: 1, data: (0..80_000).map(|k| (k % 400) as f32).collect() };
        let q = p.shrunk(100);
        assert_eq!((q.width, q.height), (100, 50));
        // each new pixel covers four columns: their mean
        assert!((q.data[0] - 1.5).abs() < 1e-6 && (q.data[99] - 397.5).abs() < 1e-6);
        assert_eq!(p.shrunk(500).width, 400);
    }
}
