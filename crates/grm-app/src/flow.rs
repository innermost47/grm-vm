// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The network as a flow: what comes in (the inputs: knobs, keys, switches) on the left, what goes out (what is
//! played, the outputs) on the right, and between them, in columns, each node as far from the inputs as the
//! relations that make it (a relation reads its right side and writes its left one: `relate r: y = a * x` goes from
//! a and x to y). The nodes of one network called (a voice, a filter) stay together in their column. While it plays,
//! what passes through each link is read from the sound itself and the link brightens and thickens with it: a note
//! is seen crossing the network, from the key to the speakers. Nothing flashes: it rises and falls slowly.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::Picture;

#[derive(Default)]
pub struct Flow {
    /// Laid out as a flow (or as the usual calm force layout).
    pub on: bool,
    /// How much passes through each node now (0 .. 1, against what passed through it lately at most).
    pub live: HashMap<String, f32>,
    level: HashMap<String, f32>,
    peak: HashMap<String, f32>,
}

/// What a relation's text writes (the names left of its `=`) and reads (the names right of it).
fn sides(text: &str) -> (Vec<String>, Vec<String>) {
    let body = text.split_once(':').map(|(_, b)| b).unwrap_or(text);
    let body = body.split(" when ").next().unwrap_or(body);
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
            .filter(|w| !w.is_empty() && w.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_'))
            .map(|w| w.to_string())
            .collect()
    };
    match body.split_once('=') {
        Some((l, r)) => (words(l), words(r)),
        None => (vec![], words(body)),
    }
}

/// The group a node belongs to, for keeping it beside its kin (`v1_f_y4` with `v1_...`).
fn group(name: &str) -> &str {
    let short = name.rsplit("__").next().unwrap_or(name);
    short.split('_').next().unwrap_or(short)
}

/// Where each item (`n:node`, `r:relation`) goes in a flow: (x, y). `played`: what is heard (on the right).
pub fn layout(pic: &Picture, played: &HashSet<String>) -> HashMap<String, [f32; 2]> {
    let names: HashSet<&str> = pic.nodes.iter().map(|n| n.name.as_str()).collect();
    // directed: from what a relation reads to the relation, from the relation to what it writes
    let mut writes: HashMap<String, Vec<String>> = HashMap::new();
    let mut reads: HashMap<String, Vec<String>> = HashMap::new();
    for r in &pic.relations {
        let (l, rr) = sides(&r.text);
        let keep = |v: Vec<String>| -> Vec<String> {
            v.into_iter().filter(|w| names.contains(w.as_str()) && r.nodes.contains(w)).collect()
        };
        let (mut l, mut rr) = (keep(l), keep(rr));
        if l.is_empty() && rr.is_empty() {
            rr = r.nodes.clone();
        }
        l.dedup();
        rr.dedup();
        writes.insert(r.name.clone(), l);
        reads.insert(r.name.clone(), rr);
    }
    // the distance from the inputs, in steps (a node, a relation, a node...), the shortest way
    let mut depth: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    for n in &pic.nodes {
        if n.input || (n.observed && !n.var) {
            depth.insert(format!("n:{}", n.name), 0);
            queue.push_back(n.name.clone());
        }
    }
    let read_by: HashMap<&str, Vec<&String>> = {
        let mut m: HashMap<&str, Vec<&String>> = HashMap::new();
        for (r, ns) in &reads {
            for n in ns {
                m.entry(n.as_str()).or_default().push(r);
            }
        }
        m
    };
    while let Some(n) = queue.pop_front() {
        let d = depth[&format!("n:{n}")];
        for r in read_by.get(n.as_str()).cloned().unwrap_or_default() {
            let rk = format!("r:{r}");
            if depth.contains_key(&rk) {
                continue;
            }
            depth.insert(rk, d + 1);
            for w in writes.get(r).cloned().unwrap_or_default() {
                let wk = format!("n:{w}");
                if let std::collections::hash_map::Entry::Vacant(e) = depth.entry(wk) {
                    e.insert(d + 2);
                    queue.push_back(w);
                }
            }
        }
    }
    let deepest = depth.values().copied().max().unwrap_or(0) + 2;
    // what is heard, and what nothing reads, on the right
    let mut column: HashMap<String, usize> = HashMap::new();
    for n in &pic.nodes {
        let k = format!("n:{}", n.name);
        let heard = played.contains(&n.name) || n.output;
        let c = if heard { deepest } else { depth.get(&k).copied().unwrap_or(1) };
        column.insert(k, c);
    }
    for r in &pic.relations {
        let k = format!("r:{}", r.name);
        let c = depth.get(&k).copied().unwrap_or_else(|| {
            // a relation not reached: beside the first node it touches
            r.nodes.iter().filter_map(|n| column.get(&format!("n:{n}"))).min().copied().unwrap_or(1)
        });
        column.insert(k, c);
    }
    // each column, its items by group then name (a voice's nodes together), centred
    let mut columns: HashMap<usize, Vec<String>> = HashMap::new();
    for (k, c) in &column {
        columns.entry(*c).or_default().push(k.clone());
    }
    let key = |k: &String| {
        let name = &k[2..];
        (group(name).to_string(), name.to_string())
    };
    // a tall column is folded into several side by side (so that the whole reads on a screen wider than tall)
    const ROWS: usize = 24;
    let mut out = HashMap::new();
    let mut order: Vec<usize> = columns.keys().copied().collect();
    order.sort();
    let mut x0 = 0.0f32;
    for c in order {
        let mut items = columns.remove(&c).unwrap_or_default();
        items.sort_by_key(key);
        let chunks = items.len().div_ceil(ROWS).max(1);
        for (i, k) in items.iter().enumerate() {
            let (j, row) = (i / ROWS, i % ROWS);
            let in_chunk = if j + 1 == chunks { items.len() - j * ROWS } else { ROWS } as f32;
            let x = x0 + j as f32 * 70.0;
            let y = (row as f32 - (in_chunk - 1.0) / 2.0) * 30.0;
            out.insert(k.clone(), [x, y]);
        }
        x0 += 70.0 * chunks as f32 + 60.0;
    }
    out
}

impl Flow {
    /// What passes now through each node of what plays, read from the sound itself: how far it is from nothing,
    /// against what passed through it lately at most; it rises and falls slowly (nothing flashes).
    pub fn listen(&mut self, values: &[(String, f64)]) {
        for (name, v) in values {
            let a = v.abs() as f32;
            let level = self.level.entry(name.clone()).or_insert(0.0);
            *level += 0.15 * (a - *level);
            let peak = self.peak.entry(name.clone()).or_insert(1e-6);
            *peak = (*peak * 0.995).max(*level).max(1e-6);
            let live = (*level / *peak).clamp(0.0, 1.0);
            let shown = self.live.entry(name.clone()).or_insert(0.0);
            *shown += 0.2 * (live - *shown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeView, RelationView};

    #[test]
    fn inputs_go_left_what_is_heard_goes_right() {
        let node = |name: &str, input: bool| NodeView { name: name.into(), input, ..Default::default() };
        let rel = |name: &str, text: &str, nodes: &[&str]| RelationView {
            name: name.into(),
            text: text.into(),
            nodes: nodes.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let pic = Picture {
            nodes: vec![node("pitch", true), node("step", false), node("wave", false), node("out", false)],
            relations: vec![
                rel("rate", "relation rate: step = 0.00004 * pitch", &["step", "pitch"]),
                rel("osc", "set wave = wave + step", &["wave", "step"]),
                rel("level", "relation level: out = 0.5 * wave", &["out", "wave"]),
            ],
            ..Default::default()
        };
        let played: HashSet<String> = ["out".to_string()].into_iter().collect();
        let at = layout(&pic, &played);
        let x = |k: &str| at[k][0];
        assert!(x("n:pitch") < x("r:rate") && x("r:rate") < x("n:step"));
        assert!(x("n:step") < x("n:wave") && x("n:wave") < x("n:out"));
        assert_eq!(x("n:out"), at.values().map(|p| p[0]).fold(f32::MIN, f32::max));
    }
}
