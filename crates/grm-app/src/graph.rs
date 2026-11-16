// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Where each node and each relation sits in the network view: a calm force layout (springs along the links,
//! a soft push between everything, a slow pull to the middle). Movements are capped and damped, so nothing jumps;
//! it stops by itself when it is at rest. It knows nothing of how it is drawn.

use std::collections::{HashMap, HashSet};

/// The most a point moves in one step (in view units): slow, never a jump.
const MAX_STEP: f32 = 3.0;
const REST: f32 = 70.0;

#[derive(Default)]
pub struct Graph {
    pub pos: HashMap<String, [f32; 2]>,
    vel: HashMap<String, [f32; 2]>,
    /// Points the human put somewhere: they stay there.
    pub pinned: HashSet<String>,
    /// How much it moved at the last step (0: at rest).
    pub motion: f32,
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

impl Graph {
    /// One step. `items`: every point (nodes and relations); `links`: (a, b) pairs.
    pub fn step(&mut self, items: &[String], links: &[(String, String)]) {
        // the new ones: beside what they are linked to, or on a spiral (deterministic: the same each time)
        let present: HashSet<&String> = items.iter().collect();
        // a point put somewhere by hand stays known, even before what it stands for is there
        let pinned = self.pinned.clone();
        self.pos.retain(|k, _| present.contains(k) || pinned.contains(k));
        self.vel.retain(|k, _| present.contains(k));
        for (k, item) in items.iter().enumerate() {
            if self.pos.contains_key(item) {
                continue;
            }
            let near: Vec<[f32; 2]> = links
                .iter()
                .filter_map(|(a, b)| {
                    if a == item {
                        Some(b)
                    } else if b == item {
                        Some(a)
                    } else {
                        None
                    }
                })
                .filter_map(|o| self.pos.get(o).copied())
                .collect();
            let h = hash(item);
            let angle = (h % 6283) as f32 / 1000.0;
            let p = if near.is_empty() {
                let r = 40.0 * (k as f32 + 1.0).sqrt();
                [r * angle.cos(), r * angle.sin()]
            } else {
                let n = near.len() as f32;
                let c = near.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
                [c[0] + 30.0 * angle.cos(), c[1] + 30.0 * angle.sin()]
            };
            self.pos.insert(item.clone(), p);
            self.vel.insert(item.clone(), [0.0, 0.0]);
        }
        let n = items.len();
        if n == 0 {
            self.motion = 0.0;
            return;
        }
        for i in items {
            self.vel.entry(i.clone()).or_insert([0.0, 0.0]);
        }
        let pts: Vec<[f32; 2]> = items.iter().map(|i| self.pos[i]).collect();
        let index: HashMap<&String, usize> = items.iter().enumerate().map(|(k, i)| (i, k)).collect();
        let mut force = vec![[0.0f32; 2]; n];
        // a soft push between everything (only near ones count, beyond 4 rest lengths it is nothing)
        if n <= 1500 {
            for a in 0..n {
                for b in (a + 1)..n {
                    let dx = pts[a][0] - pts[b][0];
                    let dy = pts[a][1] - pts[b][1];
                    let d2 = (dx * dx + dy * dy).max(1.0);
                    if d2 > 16.0 * REST * REST {
                        continue;
                    }
                    let f = REST * REST / d2;
                    let d = d2.sqrt();
                    let (fx, fy) = (f * dx / d, f * dy / d);
                    force[a][0] += fx;
                    force[a][1] += fy;
                    force[b][0] -= fx;
                    force[b][1] -= fy;
                }
            }
        }
        // springs along the links
        for (a, b) in links {
            let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) else { continue };
            let dx = pts[ib][0] - pts[ia][0];
            let dy = pts[ib][1] - pts[ia][1];
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let f = 0.08 * (d - REST * 0.6);
            force[ia][0] += f * dx / d;
            force[ia][1] += f * dy / d;
            force[ib][0] -= f * dx / d;
            force[ib][1] -= f * dy / d;
        }
        // a slow pull to the middle
        for (k, p) in pts.iter().enumerate() {
            force[k][0] -= 0.004 * p[0];
            force[k][1] -= 0.004 * p[1];
        }
        let mut motion = 0.0f32;
        for (k, item) in items.iter().enumerate() {
            if self.pinned.contains(item) {
                continue;
            }
            let v = self.vel.get_mut(item).unwrap();
            v[0] = 0.6 * v[0] + 0.05 * force[k][0];
            v[1] = 0.6 * v[1] + 0.05 * force[k][1];
            let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
            if speed > MAX_STEP {
                v[0] *= MAX_STEP / speed;
                v[1] *= MAX_STEP / speed;
            }
            if speed < 0.02 {
                *v = [0.0, 0.0];
            }
            let p = self.pos.get_mut(item).unwrap();
            p[0] += v[0];
            p[1] += v[1];
            motion = motion.max(speed.min(MAX_STEP));
        }
        self.motion = motion;
    }

    /// The box around everything: (min x, min y, max x, max y).
    pub fn bounds(&self) -> Option<[f32; 4]> {
        let mut it = self.pos.values();
        let first = it.next()?;
        Some(it.fold([first[0], first[1], first[0], first[1]], |b, p| {
            [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_comes_to_rest_and_keeps_linked_ones_close() {
        let mut g = Graph::default();
        let items: Vec<String> = ["a", "b", "c", "r1", "r2"].iter().map(|s| s.to_string()).collect();
        let links = vec![
            ("a".into(), "r1".into()),
            ("b".into(), "r1".into()),
            ("b".into(), "r2".into()),
            ("c".into(), "r2".into()),
        ];
        for _ in 0..2000 {
            g.step(&items, &links);
        }
        assert!(g.motion < 0.05, "{}", g.motion);
        let d = |x: &str, y: &str| {
            let (p, q) = (g.pos[x], g.pos[y]);
            ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
        };
        assert!(d("a", "r1") < d("a", "c"));
    }
}
