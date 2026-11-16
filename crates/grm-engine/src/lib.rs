// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! grm-engine: the heart of the GRM machine.
//!
//! A network of nodes and relations (network.rs), said in GRML (lang.rs), that settles by Gaussian belief
//! propagation (machine.rs): every node and every relation a small processor that only talks to its neighbours,
//! and only works when a neighbour has something new to say.

// Comparisons written `!(a > b)` are meant: they are true for NaN too. Index loops follow the formulas.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::needless_range_loop)]

pub mod discover;
pub mod engine;
pub mod error;
pub mod hidden;
pub mod lang;
pub mod machine;
pub mod master;
pub mod mind;
pub mod network;
pub mod peers;
pub mod ports;
pub mod reflex;
pub mod time;
pub mod units;
pub mod vision;
pub mod world;

pub use error::{GrmError, Result};
pub use network::{Network, Provenance};
