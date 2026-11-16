// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The master equation: what the relations of products say together, once every node in between is taken away.
//! In log, a product relation is a straight line (sum of powers times logs = a constant); a chain of them is a
//! set of straight lines, and what is left when the nodes inside the chain are eliminated is one relation between
//! the nodes at its ends (what is measured, what goes in or out, what only one relation holds). Nobody wrote it:
//! the network holds it.

use indexmap::IndexMap;

use crate::network::Kind;
use crate::world::World;

/// A master equation: its powers, its constant (log10), how many decades its nodes span, how far it is from
/// holding now (dex).
#[derive(Debug, Clone, PartialEq)]
pub struct Master {
    pub powers: Vec<(String, f64)>,
    pub constant: f64,
    pub decades: f64,
    pub residual: f64,
    /// How many relations it gathers.
    pub relations: usize,
}

fn power_text(p: f64) -> String {
    // simple fractions are written as such
    for d in [1.0, 2.0, 3.0, 4.0] {
        let n = p * d;
        if (n - n.round()).abs() < 1e-6 {
            let n = n.round() as i64;
            return if d == 1.0 { format!("{n}") } else { format!("{n}/{}", d as i64) };
        }
    }
    crate::lang::g(p)
}

impl Master {
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = self
            .powers
            .iter()
            .map(|(n, p)| if (p - 1.0).abs() < 1e-9 { n.clone() } else { format!("{n}^{}", power_text(*p)) })
            .collect();
        if parts.is_empty() {
            parts.push("1".into());
        }
        let k = 10f64.powf(self.constant);
        let right = if (k - 1.0).abs() < 1e-9 { "1".to_string() } else { crate::lang::g(k) };
        format!("{} = {right}", parts.join(" · "))
    }
}

impl World {
    /// The master equation of the network (or of what one source holds): None when its relations of products do
    /// not gather into one.
    pub fn master(&self, source: Option<&str>) -> Option<Master> {
        let net = &self.machine.net;
        let rels: Vec<(IndexMap<String, f64>, f64)> = net
            .relations
            .iter()
            .filter(|r| source.is_none_or(|s| r.origin.source == s) && r.when.is_empty() && r.strength > 0.0)
            .filter_map(|r| match &r.kind {
                Kind::Product { terms, rhs } if terms.len() >= 2 => Some((terms.clone(), *rhs)),
                _ => None,
            })
            .collect();
        if rels.len() < 2 {
            return None;
        }
        let mut nodes: Vec<String> = vec![];
        let mut degree: IndexMap<String, usize> = IndexMap::new();
        for (t, _) in &rels {
            for n in t.keys() {
                if !nodes.contains(n) {
                    nodes.push(n.clone());
                }
                *degree.entry(n.clone()).or_insert(0) += 1;
            }
        }
        let outer = |n: &str| {
            net.measured.contains_key(n)
                || net.nodes.get(n).is_some_and(|s| s.input || s.output || s.fixed)
                || degree.get(n).copied().unwrap_or(0) == 1
        };
        // inner columns first, then outer, then the constant
        let inner: Vec<&String> = nodes.iter().filter(|n| !outer(n)).collect();
        let outs: Vec<&String> = nodes.iter().filter(|n| outer(n)).collect();
        if outs.len() < 2 {
            return None;
        }
        let cols: Vec<&String> = inner.iter().chain(outs.iter()).copied().collect();
        let mut m: Vec<Vec<f64>> = rels
            .iter()
            .map(|(t, rhs)| {
                let mut row: Vec<f64> = cols.iter().map(|c| t.get(*c).copied().unwrap_or(0.0)).collect();
                row.push(*rhs);
                row
            })
            .collect();
        // eliminate the inner nodes (Gauss, with the largest pivot)
        let mut used = vec![false; m.len()];
        for col in 0..inner.len() {
            let pivot = (0..m.len())
                .filter(|&r| !used[r] && m[r][col].abs() > 1e-12)
                .max_by(|&a, &b| m[a][col].abs().partial_cmp(&m[b][col].abs()).unwrap());
            let Some(p) = pivot else { continue };
            used[p] = true;
            for r in 0..m.len() {
                if r != p && m[r][col].abs() > 1e-12 {
                    let k = m[r][col] / m[p][col];
                    for c in 0..m[r].len() {
                        m[r][c] -= k * m[p][c];
                    }
                }
            }
        }
        // what is left: rows that no longer hold an inner node; the one that gathers most
        let mut best: Option<(usize, usize)> = None;
        for (r, row) in m.iter().enumerate() {
            if used[r] {
                continue;
            }
            if row[..inner.len()].iter().any(|x| x.abs() > 1e-9) {
                continue;
            }
            let n = row[inner.len()..cols.len()].iter().filter(|x| x.abs() > 1e-9).count();
            if n >= 2 && best.is_none_or(|(_, b)| n > b) {
                best = Some((r, n));
            }
        }
        let (r, _) = best?;
        let row = &m[r];
        // the first power made positive and the smallest one made 1 (when it gives simple numbers)
        let first = row[inner.len()..cols.len()].iter().find(|x| x.abs() > 1e-9).copied().unwrap_or(1.0);
        let smallest =
            row[inner.len()..cols.len()].iter().filter(|x| x.abs() > 1e-9).fold(f64::INFINITY, |a, x| a.min(x.abs()));
        let scale = first.signum() / smallest;
        let powers: Vec<(String, f64)> = outs
            .iter()
            .enumerate()
            .filter(|(k, _)| row[inner.len() + k].abs() > 1e-9)
            .map(|(k, n)| ((*n).clone(), row[inner.len() + k] * scale))
            .collect();
        let constant = row[cols.len()] * scale;
        let logs: Vec<f64> = powers.iter().map(|(n, _)| self.machine.value_si(n).abs().max(1e-300).log10()).collect();
        let decades =
            logs.iter().fold(f64::NEG_INFINITY, |a, x| a.max(*x)) - logs.iter().fold(f64::INFINITY, |a, x| a.min(*x));
        let residual = (powers.iter().zip(&logs).map(|((_, p), l)| p * l).sum::<f64>() - constant).abs();
        Some(Master { powers, constant, decades, residual, relations: rels.len() })
    }
}

impl World {
    /// Why a node has its value: what each of its relations (and its measurement) tells it, each weighed by how
    /// sure it is (their precisions add up), the strongest first.
    pub fn why_value(&self, name: &str) -> crate::error::Result<String> {
        use crate::machine::FKind;
        let m = &self.machine;
        let Some(&i) = m.index.get(name) else {
            if m.fixed.contains_key(name) {
                return Ok(format!("{name} is fixed: {}", crate::lang::g(m.value_si(name))));
            }
            crate::bail!("no node {name}")
        };
        let signed = m.var_signed[i];
        let shift = 10f64.powf(m.shift_of(name));
        let mut parts: Vec<(f64, f64, String)> = vec![];
        for f in &m.factors {
            for (s, &v) in f.vars.iter().enumerate() {
                if v != i || f.out_lam[s] <= 0.0 {
                    continue;
                }
                let label = match f.kind {
                    FKind::Anchor => "its measurement (or what it was given)".to_string(),
                    FKind::Keep => format!("the limit {}: {}", f.name, f.text),
                    FKind::Joint => format!("a belief held with others ({})", f.text),
                    _ => format!("relation {}: {}", f.name, f.text),
                };
                parts.push((f.out_lam[s], f.out_eta[s], label));
            }
        }
        let total: f64 = parts.iter().map(|p| p.0).sum();
        if total <= 0.0 {
            return Ok(format!("{name}: nothing tells it anything yet"));
        }
        parts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let (v, s) = m.reading(name);
        let mut out = vec![format!(
            "why {name} = {} (± {}{})",
            crate::lang::g(v),
            crate::lang::g(s),
            if signed { "" } else { " dex" }
        )];
        let mut quiet = 0;
        for (lam, eta, label) in &parts {
            let share = lam / total;
            if share < 0.005 {
                quiet += 1;
                continue;
            }
            let x = eta / lam;
            let says = if signed { x * shift } else { 10f64.powf(x.min(300.0)) * shift };
            out.push(format!("  {:>5.1} %  {label}\n          says {}", share * 100.0, crate::lang::g(says)));
        }
        if quiet > 0 {
            out.push(format!("  and {quiet} that say almost nothing of it"));
        }
        Ok(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use crate::network::Provenance;
    use crate::world::World;

    #[test]
    fn a_chain_of_products_gives_its_master_equation() {
        let mut w = World::new();
        w.say_all(
            &[
                "observe distance = 100",
                "observe time = 9.58",
                "relate run: speed * time = distance",
                "output speed",
                "observe m1 = 2",
                "observe m2 = 3",
                "relate pot: energy * distance = m1 * m2",
                "relate gauge: epp = energy",
                "output epp",
            ],
            &Provenance::of("typed"),
        )
        .unwrap();
        w.machine.run(4000);
        let m = w.master(None).unwrap();
        // energy goes: epp · distance = m1 · m2 (as epp · distance · m1^-1 · m2^-1 = 1)
        let text = m.text();
        assert!(text.contains("epp") && text.contains("m1^-1") && !text.contains("energy"), "{text}");
        assert!(m.residual < 1e-6, "{m:?}");
        assert!(w.master(Some("nobody")).is_none());
    }
}
