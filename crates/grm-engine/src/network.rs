// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What has been said to the machine: nodes, relations, measurements, the networks that can be called, and where
//! each of them comes from (its provenance: what the immune system of the machine looks at).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Where a node or a relation comes from. Everything said to the machine carries one, so that what came from a
/// source can be found again, weakened, put in quarantine, or taken away at once, everywhere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// "typed", "base", "file:NAME", "sandbox:NAME", "found", "peer:ID"...
    pub source: String,
    /// Who said it, when known (a person, a peer).
    pub author: Option<String>,
    /// A signature of what was said, when it came signed (checked by the sas of a later milestone).
    pub signature: Option<String>,
    pub version: Option<String>,
}

impl Provenance {
    pub fn of(source: impl Into<String>) -> Self {
        Provenance { source: source.into(), author: None, signature: None, version: None }
    }
}

impl Default for Provenance {
    fn default() -> Self {
        Provenance::of("typed")
    }
}

/// A node: a quantity of the world. Positive nodes are solved in log10, signed ones as they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub guess: f64,
    pub fixed: bool,
    pub signed: bool,
    #[serde(default)]
    pub var: bool,
    #[serde(default)]
    pub learn: bool,
    #[serde(default)]
    pub input: bool,
    #[serde(default)]
    pub output: bool,
    /// Its dimension [M L T Q K]: given and read in SI, counted inside in the machine's units.
    #[serde(default)]
    pub dim: Option<[f64; 5]>,
    #[serde(default)]
    pub origin: Provenance,
}

impl NodeSpec {
    pub fn new(guess: f64, signed: bool) -> Self {
        NodeSpec {
            guess,
            fixed: false,
            signed,
            var: false,
            learn: false,
            input: false,
            output: false,
            dim: None,
            origin: Provenance::default(),
        }
    }
}

/// A condition a relation watches: it holds only while `node` is in `lo .. hi` (SI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cond {
    pub node: String,
    pub lo: f64,
    pub hi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeepOp {
    AtLeast,
    AtMost,
}

/// The kinds of relations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    /// Products and powers, straight in log: prod n^p = 10^rhs (sigma in dex).
    Product { terms: IndexMap<String, f64>, rhs: f64 },
    /// A sum of terms = 0, each a coefficient times a product of nodes (kept as a product of exponentials).
    Sum { terms: Vec<(f64, IndexMap<String, f64>)> },
    /// L = ln(y): the converter between a quantity and its exponent.
    Log { log: String, node: String },
    /// A contact: node >= bound (or <=), pushing only while its conditions hold.
    Keep { node: String, op: KeepOp, bound: f64 },
}

/// A relation between nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub name: String,
    pub kind: Kind,
    pub sigma: f64,
    pub when: Vec<Cond>,
    pub text: String,
    #[serde(default)]
    pub origin: Provenance,
    /// How strongly it holds, from 0 (not at all: in quarantine) to 1: a relation being integrated little by
    /// little rises from 0 to 1 (its precision is multiplied by it).
    #[serde(default = "one")]
    pub strength: f64,
}

fn one() -> f64 {
    1.0
}

impl Relation {
    /// The nodes it holds (conditions included).
    pub fn nodes(&self) -> Vec<String> {
        let mut out: Vec<String> = match &self.kind {
            Kind::Product { terms, .. } => terms.keys().cloned().collect(),
            Kind::Sum { terms } => terms.iter().flat_map(|(_, t)| t.keys().cloned()).collect(),
            Kind::Log { log, node } => return vec![log.clone(), node.clone()],
            Kind::Keep { node, .. } => vec![node.clone()],
        };
        out.extend(self.when.iter().map(|c| c.node.clone()));
        out
    }
}

/// `set x = EXPR [when ...]`: at each tick x takes the value of its source (computed by the network).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetRule {
    pub target: String,
    pub source: String,
    pub when: Vec<Cond>,
    pub text: String,
    /// `carry`: the target takes the source's value and its uncertainty (a state carried from one tick to the
    /// next, as sure as it was), not only its value.
    #[serde(default)]
    pub carry: bool,
}

/// A belief of several nodes together: a mean and an information matrix (the inverse of their covariance), in the
/// machine's internal units (log10 for a positive node, its own unit for a signed one).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prior {
    pub name: String,
    pub names: Vec<String>,
    pub mean: Vec<f64>,
    pub info: Vec<Vec<f64>>,
}

/// `carry a, b = c, d [when ...]`: the sources' joint belief becomes the targets' prior at each tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarryGroup {
    /// The prior it writes.
    pub prior: String,
    pub targets: Vec<String>,
    pub sources: Vec<String>,
    pub when: Vec<Cond>,
    pub text: String,
}

/// A network that can be called: `network NAME(params) ... end`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub params: Vec<String>,
    pub lines: Vec<String>,
}

/// Everything said: the description of the network (what the machine compiles and runs).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Network {
    /// The links it may hold, set by hand (`capacity N`); None: what the host's memory allows.
    #[serde(default)]
    pub capacity: Option<usize>,
    pub nodes: IndexMap<String, NodeSpec>,
    pub relations: Vec<Relation>,
    /// name -> (value, sigma): dex for a positive node, its own unit for a signed one.
    pub measured: IndexMap<String, (f64, f64)>,
    pub networks: IndexMap<String, Template>,
    #[serde(skip)]
    pub recording: Option<(String, Vec<String>, Vec<String>)>,
    pub calls: Vec<(String, IndexMap<String, String>)>,
    pub sets: Vec<SetRule>,
    pub ticks: u64,
    /// A name that became another node by a fusion -> that node.
    pub aliases: IndexMap<String, String>,
    /// A name that became the inverse of another node -> that node.
    pub inverses: IndexMap<String, String>,
    /// What groups of weights have learned together, kept by the source it was learned from, so that what a
    /// source taught can be taken back.
    pub links: Vec<Link>,
    /// The vectors: a name -> its dimension (its components are NAME_0, NAME_1...).
    #[serde(default)]
    pub vectors: IndexMap<String, usize>,
    /// Beliefs of several nodes together (a state carried with its covariance, a vector measured with its
    /// covariance): each a Gaussian over its nodes, in the machine's internal units.
    #[serde(default)]
    pub priors: Vec<Prior>,
    /// `carry a, b = c, d`: at each tick, what the network believes of the sources together becomes the prior of
    /// the targets together.
    #[serde(default)]
    pub carries: Vec<CarryGroup>,
    /// Where each relation is in `relations` (rebuilt when it no longer says the truth).
    #[serde(skip)]
    rel_index: std::collections::HashMap<String, usize>,
}

/// What a group of weights has learned from one source: an information matrix and vector (a Gaussian).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub names: Vec<String>,
    pub source: String,
    #[serde(rename = "L")]
    pub l: Vec<Vec<f64>>,
    pub h: Vec<f64>,
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    fn position(&self, name: &str) -> Option<usize> {
        if let Some(&i) = self.rel_index.get(name) {
            if self.relations.get(i).is_some_and(|r| r.name == name) {
                return Some(i);
            }
        }
        if self.rel_index.len() == self.relations.len() && !self.rel_index.contains_key(name) {
            return None;
        }
        self.relations.iter().position(|r| r.name == name)
    }

    fn reindex(&mut self) {
        self.rel_index = self.relations.iter().enumerate().map(|(i, r)| (r.name.clone(), i)).collect();
    }

    pub fn relation(&self, name: &str) -> Option<&Relation> {
        self.position(name).map(|i| &self.relations[i])
    }

    pub fn relation_mut(&mut self, name: &str) -> Option<&mut Relation> {
        self.position(name).map(|i| &mut self.relations[i])
    }

    /// Adds a relation, or replaces the one of the same name.
    pub fn put(&mut self, rel: Relation) {
        if self.rel_index.len() != self.relations.len() {
            self.reindex();
        }
        if let Some(i) = self.position(&rel.name) {
            self.relations[i] = rel;
        } else {
            self.rel_index.insert(rel.name.clone(), self.relations.len());
            self.relations.push(rel);
        }
    }

    /// Takes a relation away (by name). Returns it.
    pub fn take_relation(&mut self, name: &str) -> Option<Relation> {
        let i = self.position(name)?;
        let r = self.relations.remove(i);
        self.reindex();
        Some(r)
    }

    pub fn degree(&self, node: &str) -> usize {
        self.relations.iter().filter(|r| r.nodes().iter().any(|n| n == node)).count()
    }

    /// Every node and relation that came from a source.
    pub fn from_source(&self, source: &str) -> (Vec<String>, Vec<String>) {
        let nodes = self.nodes.iter().filter(|(_, s)| s.origin.source == source).map(|(n, _)| n.clone()).collect();
        let rels = self.relations.iter().filter(|r| r.origin.source == source).map(|r| r.name.clone()).collect();
        (nodes, rels)
    }
}
