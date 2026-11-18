// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Where each node and each relation sits in the network view: a calm force layout (springs along the links,
//! a soft push between everything, a slow pull to the middle). Movements are capped and damped, so nothing jumps;
//! it stops by itself when it is at rest. It knows nothing of how it is drawn.

use std::collections::{HashMap, HashSet};

/// The most a point moves in one step (in view units): slow, never a jump.
const MAX_STEP: f32 = 1.2;
const REST: f32 = 70.0;

#[derive(Default)]
pub struct Graph {
    pub pos: HashMap<String, [f32; 2]>,
    vel: HashMap<String, [f32; 2]>,
    /// Points the human put somewhere: they stay there.
    pub pinned: HashSet<String>,
    /// Points near the human's pointer: still while it is there (they can be taken without a fight).
    pub held: HashSet<String>,
    /// How much it moved at the last step (0: at rest).
    pub motion: f32,
    /// Steps left to go back quickly to a free layout (after a flow: the points far from where they settle).
    pub hurry: u32,
    /// How warm it is (1: just changed; it cools at each step, and at nothing it rests: a network of thousands
    /// finds its place in a few seconds, then costs nothing).
    warmth: f32,
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

impl Graph {
    /// Woken (a point moved by hand, a layout given back): it moves again, warm, and cools.
    pub fn wake(&mut self) {
        self.motion = 1.0;
        self.warmth = self.warmth.max(0.6);
    }

    /// One step. `items`: every point (nodes and relations); `links`: (a, b) pairs.
    pub fn step(&mut self, items: &[String], links: &[(String, String)]) {
        // the new ones: beside what they are linked to, or on a spiral (deterministic: the same each time)
        let present: HashSet<&String> = items.iter().collect();
        // a point put somewhere by hand stays known, even before what it stands for is there
        let pinned = self.pinned.clone();
        self.pos.retain(|k, _| present.contains(k) || pinned.contains(k));
        self.vel.retain(|k, _| present.contains(k));
        // the new ones, beside what they are linked to: their neighbours found once (not every link searched for
        // every new point: a network of thousands came in in one go)
        if items.iter().any(|i| !self.pos.contains_key(i)) {
            self.warmth = 1.0;
            let mut near_of: HashMap<&String, Vec<&String>> = HashMap::new();
            for (a, b) in links {
                near_of.entry(a).or_default().push(b);
                near_of.entry(b).or_default().push(a);
            }
            for (k, item) in items.iter().enumerate() {
                if self.pos.contains_key(item) {
                    continue;
                }
                let near: Vec<[f32; 2]> = near_of
                    .get(item)
                    .map(|v| v.iter().filter_map(|o| self.pos.get(*o).copied()).collect())
                    .unwrap_or_default();
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
        // a soft push between everything (only near ones count, beyond 4 rest lengths it is nothing): each point
        // meets only those in its square of the grid and the squares around (not all the others: thousands of
        // points cost thousands, not millions)
        // (a big network: the push reaches half as far)
        let reach = if n > 2000 { 2.0 * REST } else { 4.0 * REST };
        let cell = reach;
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, p) in pts.iter().enumerate() {
            grid.entry(((p[0] / cell).floor() as i32, (p[1] / cell).floor() as i32)).or_default().push(k);
        }
        for a in 0..n {
            let (cx, cy) = ((pts[a][0] / cell).floor() as i32, (pts[a][1] / cell).floor() as i32);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(others) = grid.get(&(cx + dx, cy + dy)) else { continue };
                    for &b in others {
                        if b <= a {
                            continue;
                        }
                        let dx = pts[a][0] - pts[b][0];
                        let dy = pts[a][1] - pts[b][1];
                        let d2 = (dx * dx + dy * dy).max(1.0);
                        if d2 > reach * reach {
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
            if self.held.contains(item) {
                self.vel.insert(item.clone(), [0.0, 0.0]);
                continue;
            }
            let v = self.vel.get_mut(item).unwrap();
            v[0] = 0.6 * v[0] + 0.05 * force[k][0];
            v[1] = 0.6 * v[1] + 0.05 * force[k][1];
            let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
            // calm, except just after a flow, when the points have far to go
            let most = if self.hurry > 0 { MAX_STEP * 8.0 } else { MAX_STEP * (0.3 + 2.0 * self.warmth) };
            if speed > most {
                v[0] *= most / speed;
                v[1] *= most / speed;
            }
            if speed < 0.02 {
                *v = [0.0, 0.0];
            }
            let p = self.pos.get_mut(item).unwrap();
            p[0] += v[0];
            p[1] += v[1];
            motion = motion.max(speed.min(most));
        }
        self.warmth *= 0.99;
        self.motion = if self.warmth < 0.03 && self.hurry == 0 { 0.0 } else { motion };
        self.hurry = self.hurry.saturating_sub(1);
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
