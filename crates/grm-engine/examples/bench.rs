// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Times the engine on a GRML file: cargo run --release --example bench -- FILE.grm [THREADS] [ROUNDS]

use std::time::Instant;

use grm_engine::lang::statement;
use grm_engine::machine::Machine;
use grm_engine::{Network, Provenance};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("a GRML file");
    let threads = args.get(2).and_then(|t| t.parse().ok());
    let t0 = Instant::now();
    let mut net = Network::new();
    for line in text.lines() {
        statement(&mut net, line, None, &Provenance::default()).unwrap();
    }
    let t1 = Instant::now();
    let mut m = Machine::new(net).unwrap();
    m.set_threads(threads);
    let t2 = Instant::now();
    let rounds = m.run(args.get(3).and_then(|r| r.parse().ok()).unwrap_or(100000));
    let t3 = Instant::now();
    println!("rust ({} threads): {} nodes, {} processors; read {:.3} s; compile {:.3} s; settle {:.3} s in {} rounds, {} messages",
             threads.map(|t| t.to_string()).unwrap_or("all".into()), m.var_names.len(), m.factors.len(),
             (t1 - t0).as_secs_f64(), (t2 - t1).as_secs_f64(), (t3 - t2).as_secs_f64(), rounds, m.messages);
}
