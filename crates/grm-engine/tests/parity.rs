// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The Rust engine gives the values the Python machine of grm-core gives (tests/parity/golden.json, written by
//! tools/golden.py), case by case, tick by tick; and the same numbers whatever the number of threads.

use grm_engine::lang::statement;
use grm_engine::machine::Machine;
use grm_engine::{Network, Provenance};
use serde_json::Value;

fn load(name: &str) -> Value {
    let path = format!("{}/../../tests/parity/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Runs a case; returns the values of every node after each tick.
fn run(case: &Value, threads: Option<usize>, cap: Option<usize>) -> (Vec<Vec<(String, f64)>>, Machine) {
    let mut net = Network::new();
    for line in case["lines"].as_array().unwrap() {
        statement(&mut net, line.as_str().unwrap(), None, &Provenance::default()).unwrap();
    }
    let mut m = Machine::new(net).unwrap();
    m.set_threads(threads);
    m.budget.cap = cap;
    let mut ticks = vec![];
    let snap = |m: &Machine| m.net.nodes.keys().map(|n| (n.clone(), m.value_si(n))).collect::<Vec<_>>();
    for a in case["actions"].as_array().unwrap() {
        match a[0].as_str().unwrap() {
            "tick" => {
                m.tick(1000, false);
                ticks.push(snap(&m));
            }
            "ticks" => {
                for _ in 0..a[1].as_u64().unwrap() {
                    m.tick(1000, false);
                    ticks.push(snap(&m));
                }
            }
            "observe" => m.observe(a[1].as_str().unwrap(), a[2].as_f64().unwrap(), a[3].as_f64()).unwrap(),
            other => panic!("unknown action {other}"),
        }
    }
    (ticks, m)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn every_case_gives_the_values_of_the_python_machine() {
    let cases = load("cases.json");
    let golden = load("golden.json");
    let mut failures = vec![];
    for case in cases.as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let want = &golden[name]["ticks"];
        let (got, m) = run(case, Some(1), None);
        assert_eq!(got.len(), want.as_array().unwrap().len(), "{name}: ticks");
        // a loop that never settles (1000 rounds a tick) is compared loosely: it swings around its answer
        let tol = if golden[name]["rounds"].as_u64().unwrap() >= 1000 { 1e-3 } else { 1e-9 };
        for (t, (g, w)) in got.iter().zip(want.as_array().unwrap()).enumerate() {
            for (node, v) in g {
                let wv = w[node].as_f64().unwrap_or(f64::INFINITY);
                if !close(*v, wv, tol) {
                    failures.push(format!("{name} tick {t} {node}: rust {v} python {wv}"));
                }
            }
        }
        assert_eq!(m.rounds, golden[name]["rounds"].as_u64().unwrap(), "{name}: rounds");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_numbers_do_not_depend_on_the_threads() {
    for case in load("cases.json").as_array().unwrap() {
        let (one, _) = run(case, Some(1), None);
        let (four, _) = run(case, Some(4), None);
        assert_eq!(one, four, "{}", case["name"]);
    }
}

/// A grid big enough that each half round is shared out between threads.
fn grid(n: usize) -> Network {
    let mut net = Network::new();
    let mut say = |l: String| {
        statement(&mut net, &l, None, &Provenance::default()).unwrap();
    };
    for i in 0..n {
        for j in 0..n {
            say(format!("node u_{i}_{j} signed"));
        }
    }
    for i in 0..n {
        for j in 0..n {
            if j + 1 < n {
                say(format!("relate h_{i}_{j}: u_{i}_{j} - u_{i}_{} = 0 ± 1", j + 1));
            }
            if i + 1 < n {
                say(format!("relate v_{i}_{j}: u_{i}_{j} - u_{}_{j} = 0 ± 1", i + 1));
            }
        }
    }
    say("observe u_0_0 = 5 ± 0.1".to_string());
    say(format!("observe u_{}_{} = -5 ± 0.1", n - 1, n - 1));
    net
}

#[test]
fn a_big_network_gives_the_same_numbers_on_one_thread_or_four() {
    let mut results = vec![];
    for threads in [Some(1), Some(2), Some(4), None] {
        let mut m = Machine::new(grid(30)).unwrap();
        m.set_threads(threads);
        m.run(60);
        results.push((m.x.clone(), m.lam.clone(), m.messages));
    }
    for r in &results[1..] {
        assert_eq!(r, &results[0]);
    }
}
