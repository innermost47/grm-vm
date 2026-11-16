// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Machines that talk to each other, without a centre: the foundation of the decentralised network.
//!
//! Two machines that share nodes (the same quantity of the world) exchange messages about them, as the processors
//! of one machine do: a precision and a precision-weighted mean (Gaussian belief propagation across machines, as
//! the Robot Web of Murai, Ortiz and Davison, 2022). They never exchange their data, only what they believe.
//!
//! - What a machine tells a peer leaves out what it heard from that peer (a belief is not echoed back to whoever
//!   gave it: no one is counted twice).
//! - What a peer says comes in as a node of its own (`peer_P__x`) tied to the shared node by a relation whose
//!   provenance is the peer: so a peer can be put in quarantine or recalled like any source.
//! - A peer is trusted only so far: the certainty accepted from it is capped (its sigma never below `min_sigma`);
//!   trust is earned later, with its record (the sas of a later milestone).
//!
//! The transport carries the messages: here in-process channels; over the internet in a later milestone (the
//! messages, the peers and the rules stay the same).

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::bail;
use crate::error::Result;
use crate::machine::FKind;
use crate::network::Provenance;
use crate::world::World;

/// A message from one machine to another about a shared node, in the machine's internal units (log10 for a
/// positive node, the node's unit for a signed one).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub node: String,
    pub lam: f64,
    pub eta: f64,
    /// Signed by its sender (checked by the sas of a later milestone).
    pub signature: Option<String>,
}

/// What a machine knows of a peer: the nodes they share, and how far it trusts it.
#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub name: String,
    pub shared: Vec<String>,
    /// The smallest sigma accepted from it (internal units): its certainty is capped there.
    pub min_sigma: f64,
}

impl Peer {
    pub fn new(name: &str, shared: &[&str], min_sigma: f64) -> Peer {
        Peer { name: name.into(), shared: shared.iter().map(|s| s.to_string()).collect(), min_sigma }
    }

    pub fn source(&self) -> String {
        format!("peer:{}", self.name)
    }

    fn node(&self, shared: &str) -> String {
        format!("peer_{}__{}", self.name, shared)
    }

    fn link(&self, shared: &str) -> String {
        format!("link_{}_{}", self.name, shared)
    }
}

/// How messages travel: to a peer by its name; what has come for this machine.
pub trait Transport {
    fn send(&self, to: &str, message: Message) -> Result<()>;
    fn receive(&self) -> Vec<Message>;
}

/// Machines of one process, linked by channels (the same messages the internet will carry).
pub struct LocalTransport {
    inbox: Receiver<Message>,
    outboxes: HashMap<String, Sender<Message>>,
}

/// Makes the transports of machines that can all reach each other.
pub fn local_mesh(names: &[&str]) -> HashMap<String, LocalTransport> {
    let channels: Vec<(String, Sender<Message>, Receiver<Message>)> = names
        .iter()
        .map(|n| {
            let (tx, rx) = channel();
            (n.to_string(), tx, rx)
        })
        .collect();
    let senders: HashMap<String, Sender<Message>> = channels.iter().map(|(n, tx, _)| (n.clone(), tx.clone())).collect();
    channels
        .into_iter()
        .map(|(n, _, rx)| {
            let outboxes = senders.iter().filter(|(m, _)| **m != n).map(|(m, tx)| (m.clone(), tx.clone())).collect();
            (n, LocalTransport { inbox: rx, outboxes })
        })
        .collect()
}

impl Transport for LocalTransport {
    fn send(&self, to: &str, message: Message) -> Result<()> {
        match self.outboxes.get(to) {
            Some(tx) => tx.send(message).map_err(|_| crate::error::GrmError::new(format!("{to} is no longer there"))),
            None => bail!("no way to {to}"),
        }
    }

    fn receive(&self) -> Vec<Message> {
        self.inbox.try_iter().collect()
    }
}

impl World {
    /// What this machine tells a peer about the nodes they share: its belief, without what it heard from that peer.
    pub fn tell(&self, me: &str, peer: &Peer) -> Vec<Message> {
        let m = &self.machine;
        let mut out = vec![];
        for x in &peer.shared {
            let Some(&i) = m.index.get(x) else { continue };
            let (mut lam, mut eta) = (m.lam[i], m.eta[i]);
            // leave out the peer's own message (it reaches x through the link relation)
            if let Some(f) = m.factors.iter().find(|f| f.kind == FKind::Relation && f.name == peer.link(x)) {
                if let Some(s) = f.vars.iter().position(|v| *v == i) {
                    lam -= f.out_lam[s];
                    eta -= f.out_eta[s];
                }
            }
            if lam > 0.0 {
                out.push(Message { from: me.to_string(), node: x.clone(), lam, eta, signature: None });
            }
        }
        out
    }

    /// What a peer said comes in: as its own node, tied to the shared one by a relation of the peer's provenance,
    /// its certainty capped by the trust given to it.
    pub fn hear(&mut self, peer: &Peer, messages: &[Message]) -> Result<usize> {
        let origin =
            Provenance { source: peer.source(), author: Some(peer.name.clone()), signature: None, version: None };
        let mut heard = 0;
        for msg in messages.iter().filter(|m| m.from == peer.name && peer.shared.contains(&m.node)) {
            if !(msg.lam > 0.0) || !msg.eta.is_finite() {
                continue;
            }
            let x = &msg.node;
            let Some(&i) = self.machine.index.get(x) else { continue };
            let signed = self.machine.var_signed[i];
            let p = peer.node(x);
            let shift = self.machine.var_shift[i];
            let (u, s) = (msg.eta / msg.lam, (1.0 / msg.lam.sqrt()).max(peer.min_sigma));
            // in the words of the language: SI for a signed node, a value and dex for a positive one
            let (value, sigma) =
                if signed { (u * 10f64.powf(shift), s * 10f64.powf(shift)) } else { (10f64.powf(u + shift), s) };
            if !self.machine.index.contains_key(&p) {
                let kind = if signed { " signed" } else { "" };
                let dim = self.machine.net.nodes[x]
                    .dim
                    .map(|d| format!(" [{} {} {} {} {}]", d[0], d[1], d[2], d[3], d[4]))
                    .unwrap_or_default();
                let link = format!("relate {}: {x} = {p}", peer.link(x));
                self.say_all(
                    &[
                        &format!("node {p} = {}{kind}{dim}", if signed { value } else { value.max(f64::MIN_POSITIVE) }),
                        &link,
                    ],
                    &origin,
                )?;
            }
            self.machine.observe(&p, value, Some(sigma))?;
            heard += 1;
        }
        Ok(heard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(lines: &[&str]) -> World {
        let mut w = World::new();
        w.say_all(lines, &Provenance::of("base")).unwrap();
        w.machine.run(1000);
        w
    }

    #[test]
    fn two_machines_share_what_they_believe_and_nobody_is_counted_twice() {
        let mesh = local_mesh(&["a", "b"]);
        let mut a = world(&["node x signed", "observe x = 3 ± 0.01"]);
        let mut b = world(&["node x signed", "relate r: y = 2 * x"]);
        let (pa, pb) = (Peer::new("b", &["x"], 1e-9), Peer::new("a", &["x"], 1e-9));
        for _ in 0..5 {
            for m in a.tell("a", &pa) {
                mesh["a"].send("b", m).unwrap();
            }
            for m in b.tell("b", &pb) {
                mesh["b"].send("a", m).unwrap();
            }
            let to_b = mesh["b"].receive();
            b.hear(&pb, &to_b).unwrap();
            let to_a = mesh["a"].receive();
            a.hear(&pa, &to_a).unwrap();
            a.machine.run(1000);
            b.machine.run(1000);
        }
        assert!((b.machine.value("y") - 6.0).abs() < 1e-3, "{}", b.machine.value("y"));
        // b knew nothing of x but what a told it: what it tells a back is (almost) nothing, a's certainty is its own
        assert!((a.machine.sigma("x") / 0.01 - 1.0).abs() < 1e-3, "{}", a.machine.sigma("x"));
    }

    #[test]
    fn a_peer_too_sure_of_itself_is_capped_and_can_be_put_in_quarantine() {
        let mut b = world(&["node x signed", "observe x = 3 ± 0.01"]);
        let liar = Peer::new("m", &["x"], 1.0);
        let lie = Message { from: "m".into(), node: "x".into(), lam: 1e18, eta: 1e20, signature: None }; // x = 100 ± 1e-9
        b.hear(&liar, &[lie]).unwrap();
        b.machine.run(1000);
        // capped at sigma 1: it moves x by (100 - 3) / (1 + 1/0.01^2), about 0.01
        assert!((b.machine.value("x") - 3.0).abs() < 0.02, "{}", b.machine.value("x"));
        b.quarantine("peer:m", "too sure of itself").unwrap();
        b.machine.run(1000);
        assert!((b.machine.value("x") - 3.0).abs() < 1e-6);
        b.recall("peer:m").unwrap();
        assert!(!b.net().nodes.contains_key("peer_m__x"));
    }
}
