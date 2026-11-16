// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What a relation is worth: the information it brings, in bits, against the rent of its links (ported from the
//! Python machine's mind).
//!
//! Every relation brings information: how much it narrows what its nodes can be. For Gaussian beliefs it is exact:
//! without the relation's message a node's precision would be lam - l instead of lam, its entropy higher by
//! 1/2 log2(lam / (lam - l)) bits. A relation that says what others already say (an echo) brings almost nothing; one
//! that alone fixes a node brings a lot (at most MAX_BITS).
//!
//! Holding a link (one node in one relation: a synapse) costs: the rent is PRICE bits per link and per tick at half
//! the room taken, a price that grows as room runs out (occupancy / (1 - occupancy)): nothing in an empty network,
//! more and more in a full one (Rissanen's minimum description length, with a price set by scarcity). The room is
//! the host's: a share of the memory available now, about BYTES_PER_LINK a link (or set by hand).

use indexmap::IndexMap;

use crate::machine::Machine;
use crate::network::Network;

/// Links when the host cannot be measured.
pub const CAPACITY: usize = 10_000;
/// Never less room than this.
pub const MIN_CAPACITY: usize = 500;
/// Bits a link costs each tick when half the room is taken.
pub const PRICE: f64 = 0.1;
/// What a relation can tell of one node, at most (a node it alone fixes).
pub const MAX_BITS: f64 = 32.0;
/// What a link takes in memory (its messages and history included), about.
pub const BYTES_PER_LINK: u64 = 4_000;
/// Of the memory available now, the share the network may take.
pub const MEMORY_SHARE: f64 = 0.25;

/// The links the network holds: each node of each relation.
pub fn links(net: &Network) -> usize {
    net.relations
        .iter()
        .map(|r| {
            let mut n = r.nodes();
            n.sort();
            n.dedup();
            n.len()
        })
        .sum()
}

/// The links the host's memory allows (None: it cannot be read).
pub fn host_capacity(available_bytes: Option<u64>) -> usize {
    match available_bytes {
        Some(b) => ((b as f64 * MEMORY_SHARE / BYTES_PER_LINK as f64) as usize).max(MIN_CAPACITY),
        None => CAPACITY,
    }
}

/// The links the network may hold: set by hand, or what the host allows.
pub fn capacity(net: &Network, host: usize) -> usize {
    net.capacity.unwrap_or(host).max(1)
}

/// Bits a link costs each tick: nothing in an empty network, PRICE at half the room, more and more as it fills.
pub fn price(links: usize, capacity: usize) -> f64 {
    let rho = (links as f64 / capacity.max(1) as f64).min(0.99);
    PRICE * rho / (1.0 - rho)
}

/// {relation: bits}: how much each relation narrows what its nodes can be, now.
pub fn information(m: &Machine) -> IndexMap<String, f64> {
    let names: std::collections::HashSet<&str> = m.net.relations.iter().map(|r| r.name.as_str()).collect();
    let mut out: IndexMap<String, f64> = IndexMap::new();
    for f in &m.factors {
        if !names.contains(f.name.as_str()) {
            continue;
        }
        let mut bits = 0.0;
        for (s, &i) in f.vars.iter().enumerate() {
            let l = f.out_lam.get(s).copied().unwrap_or(0.0);
            let lam = m.lam.get(i).copied().unwrap_or(0.0);
            if !(l > 0.0 && lam > 0.0) {
                continue;
            }
            let rest = lam - l;
            bits += if rest <= lam * 2f64.powf(-2.0 * MAX_BITS) {
                MAX_BITS
            } else {
                (0.5 * (lam / rest).log2()).min(MAX_BITS)
            };
        }
        *out.entry(f.name.clone()).or_insert(0.0) += bits;
    }
    out
}

/// One relation's account.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub name: String,
    /// What it brings each tick.
    pub bits: f64,
    pub links: usize,
    /// What its links cost each tick.
    pub rent: f64,
}

impl Account {
    /// What it brings, minus its rent: below zero, it does not pay for its links.
    pub fn worth(&self) -> f64 {
        self.bits - self.rent
    }
}

/// The accounts of the network: (links held, room, price of a link, each relation's account).
pub fn accounts(m: &Machine, host: usize) -> (usize, usize, f64, Vec<Account>) {
    let held = links(&m.net);
    let room = capacity(&m.net, host);
    let p = price(held, room);
    let info = information(m);
    let out = m
        .net
        .relations
        .iter()
        .map(|r| {
            let mut n = r.nodes();
            n.sort();
            n.dedup();
            let bits = info.get(&r.name).copied().unwrap_or(0.0);
            Account { name: r.name.clone(), bits, links: n.len(), rent: p * n.len() as f64 }
        })
        .collect();
    (held, room, p, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use crate::Provenance;

    #[test]
    fn a_relation_that_fixes_a_node_brings_more_than_an_echo_and_rent_grows_with_scarcity() {
        let mut w = World::new();
        let o = Provenance::of("typed");
        w.say_all(
            &[
                "node a = 2",
                "observe a = 2",
                "node b",
                "node c",
                "relate fixes: b = 3 * a",
                "relate echo: c = b",
                "relate echo2: c = b",
            ],
            &o,
        )
        .unwrap();
        w.machine.run(10_000);
        let (held, _, p, acc) = accounts(&w.machine, 1_000_000);
        assert_eq!(held, 6);
        assert!(p < 1e-3, "an empty room costs almost nothing: {p}");
        let bits = |n: &str| acc.iter().find(|a| a.name == n).unwrap().bits;
        assert!(bits("fixes") > 1.0, "{}", bits("fixes"));
        // an echo says what another already says
        assert!(bits("echo") < bits("fixes"));
        // half the room: PRICE a link; nearly full: much more
        assert!((price(50, 100) - PRICE).abs() < 1e-12);
        assert!(price(98, 100) > 40.0 * PRICE);
        assert_eq!(host_capacity(Some(4_000 * 4 * 1000)), 1000);
        assert_eq!(host_capacity(Some(0)), MIN_CAPACITY);
        assert_eq!(host_capacity(None), CAPACITY);
    }
}
