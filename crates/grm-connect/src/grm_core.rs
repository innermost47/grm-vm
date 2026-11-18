// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A network of grm-core (its JSON store: nodes with their guess, their solved value, their dimension; relations of
//! products in log10, sums; measurements) said in GRML, so that it lives in the machine like anything else.
//!
//! Its relations of products `{terms: {node: power}, rhs}` mean  sum(power x log10 node) = rhs;  its sums
//! `{terms: {node: coef}, pow: {node: power}, K}` mean  sum(coef x node^power) = K. Its relations of type `fn` are
//! functions written in Python: they are not brought (said in a comment).

use serde_json::Value;

use grm_engine::error::{GrmError, Result};

/// What a grm-core network gives: its GRML, and what was left out.
pub struct Said {
    pub grml: String,
    pub nodes: usize,
    pub relations: usize,
    pub left_out: Vec<String>,
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// A number written so that GRML reads it back exactly.
fn g(x: f64) -> String {
    if x == 0.0 {
        "0".into()
    } else if (1e-4..1e6).contains(&x.abs()) {
        format!("{x}")
    } else {
        format!("{x:e}")
    }
}

/// The GRML of a grm-core store: the network chosen (its id), else the active one; every name given a prefix.
pub fn to_grml(json: &str, network: Option<&str>, prefix: &str) -> Result<Said> {
    let store: Value = serde_json::from_str(json).map_err(|e| GrmError::new(format!("not a grm-core store: {e}")))?;
    let networks = store["networks"].as_array().ok_or_else(|| GrmError::new("no networks in it"))?;
    let wanted = network.map(String::from).or_else(|| store["activeId"].as_str().map(String::from));
    let net = networks
        .iter()
        .find(|n| wanted.as_deref().is_some_and(|w| n["id"].as_str() == Some(w)))
        .or_else(|| networks.first())
        .ok_or_else(|| GrmError::new("an empty store"))?;
    let name = |n: &str| format!("{prefix}{n}");
    let mut out: Vec<String> = vec![];
    let mut left_out: Vec<String> = vec![];
    let nodes = net["nodes"].as_object().ok_or_else(|| GrmError::new("a network without nodes"))?;
    let lin = |n: &str| nodes.get(n).is_some_and(|s| s["scale"].as_str() == Some("lin"));
    out.push(format!(
        "# {} ({}): {} nodes, from grm-core",
        net["name"].as_str().unwrap_or("?"),
        net["id"].as_str().unwrap_or("?"),
        nodes.len()
    ));
    for (n, spec) in nodes {
        let v = num(&spec["solved"]).or_else(|| num(&spec["guess"])).unwrap_or(1.0);
        let signed = lin(n) || v <= 0.0;
        let mut line = format!("node {} = {}", name(n), g(if signed { v } else { v.abs() }));
        if spec["fixed"].as_bool() == Some(true) {
            line.push_str(" fixed");
        }
        if signed {
            line.push_str(" signed");
        }
        if let Some(d) = spec["dim"].as_array().filter(|d| d.len() == 5) {
            let d: Vec<String> = d.iter().map(|x| g(num(x).unwrap_or(0.0))).collect();
            line.push_str(&format!(" [{}]", d.join(" ")));
        }
        out.push(line);
    }
    // its relations: those of the network, or (an enriched one) its own
    let mut relations: Vec<&Value> = net["relations"].as_array().map(|r| r.iter().collect()).unwrap_or_default();
    if relations.is_empty() {
        relations = net["customRelations"].as_array().map(|r| r.iter().collect()).unwrap_or_default();
    }
    let mut said = 0;
    for r in relations {
        let rname = r["name"].as_str().unwrap_or("r");
        let sigma = num(&r["sigma"]).unwrap_or(1e-7);
        match r["type"].as_str().unwrap_or("log") {
            "log" => {
                let Some(terms) = r["terms"].as_object() else { continue };
                let rhs = num(&r["rhs"]).unwrap_or(0.0);
                let left: Vec<String> = terms
                    .iter()
                    .filter_map(|(n, p)| num(p).filter(|p| *p != 0.0).map(|p| (n, p)))
                    .map(|(n, p)| if p == 1.0 { name(n) } else { format!("{}^{}", name(n), g(p)) })
                    .collect();
                if left.is_empty() {
                    continue;
                }
                let right = if rhs == 0.0 { "1".to_string() } else { g(10f64.powf(rhs)) };
                out.push(format!("relate {}: {} = {right} ± {} dex", name(rname), left.join(" * "), g(sigma)));
                said += 1;
            }
            "sum" => {
                let Some(terms) = r["terms"].as_object() else { continue };
                let k = num(&r["K"]).unwrap_or(0.0);
                let pow = |n: &str| r["pow"][n].as_f64().unwrap_or(1.0);
                let parts: Vec<(f64, String)> = terms
                    .iter()
                    .filter_map(|(n, c)| num(c).filter(|c| *c != 0.0).map(|c| (n, c)))
                    .map(|(n, c)| {
                        let p = pow(n);
                        let x = if p == 1.0 { name(n) } else { format!("{}^{}", name(n), g(p)) };
                        (c, x)
                    })
                    .collect();
                match parts.len() {
                    0 => continue,
                    1 => {
                        // a sum of one term is a value: c x^p = K
                        let (c, x) = &parts[0];
                        let n = x.split('^').next().unwrap_or(x);
                        let p = pow(n.trim_start_matches(prefix));
                        if p == 1.0 {
                            out.push(format!("observe {n} = {} ± {}", g(k / c), g(sigma)));
                            said += 1;
                        } else {
                            left_out.push(format!("{rname} (one term to a power)"));
                        }
                    }
                    _ => {
                        let mut text = String::new();
                        for (i, (c, x)) in parts.iter().enumerate() {
                            let term = if c.abs() == 1.0 { x.clone() } else { format!("{} * {x}", g(c.abs())) };
                            if i == 0 {
                                text.push_str(&if *c < 0.0 { format!("-{term}") } else { term });
                            } else {
                                text.push_str(&format!(" {} {term}", if *c < 0.0 { "-" } else { "+" }));
                            }
                        }
                        out.push(format!("relate {}: {text} = {} ± {}", name(rname), g(k), g(sigma)));
                        said += 1;
                    }
                }
            }
            other => left_out.push(format!("{rname} ({other})")),
        }
    }
    // what was measured
    if let Some(m) = net["measured"].as_object() {
        for (n, v) in m {
            if !nodes.contains_key(n) || nodes[n]["fixed"].as_bool() == Some(true) {
                continue;
            }
            let (value, sigma) = match v {
                Value::Array(a) => (a.first().and_then(num), a.get(1).and_then(num)),
                other => (num(other), None),
            };
            let Some(value) = value else { continue };
            let sigma = sigma.unwrap_or(1e-4);
            if lin(n) || value <= 0.0 {
                out.push(format!("observe {} = {} ± {}", name(n), g(value), g(sigma.max(1e-12))));
            } else {
                out.push(format!("observe {} = {} ± {} dex", name(n), g(value), g(sigma.max(1e-12))));
            }
        }
    }
    if !left_out.is_empty() {
        out.push(format!("# not brought (functions of grm-core): {}", left_out.join(", ")));
    }
    Ok(Said { grml: out.join("\n") + "\n", nodes: nodes.len(), relations: said, left_out })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_universe_of_grm_core_is_said_whole_and_holds_its_values() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/universe/grm_network.json");
        let json = std::fs::read_to_string(path).unwrap();
        let said = to_grml(&json, None, "u_").unwrap();
        assert!(said.relations > 100, "{}", said.relations);
        let mut w = grm_engine::world::World::new();
        let origin = grm_engine::Provenance::of("app:universe");
        let mut errors = vec![];
        for line in said.grml.lines().filter(|l| !l.starts_with('#')) {
            if let Err(e) = w.say(line, &origin) {
                errors.push(format!("{line}: {e}"));
            }
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        for _ in 0..200 {
            w.tick(false);
        }
        let v = |n: &str| w.machine.value_si(n);
        // what it holds: the equation of the horizon, at its solved values
        let r = v("u_S") * v("u_Equant") / v("u_Estock");
        assert!((r - 1.0).abs() < 1e-3, "{r}");
        assert!((v("u_lP") / 1.616255e-35 - 1.0).abs() < 1e-4, "{}", v("u_lP"));
        // what it finds by itself: nothing imposed, the equation of the horizon among what it holds
        let t0 = std::time::Instant::now();
        let u = w.universe("app:universe", Some("u_Hend"));
        let name = |i: usize| u.names[i].trim_start_matches("u_").to_string();
        let texts: Vec<String> = u.found.iter().map(|f| f.text(&name)).collect();
        assert!(t0.elapsed() < std::time::Duration::from_secs(10));
        assert_eq!(texts[0], "Equant · S = Estock", "{texts:?}");
        assert!(texts.iter().any(|t| t == "Rs = Rh"), "the horizon is a black hole's: {texts:?}");
        let horizon = u.found.iter().find(|f| {
            let mut t: Vec<(String, f64)> = f.terms.iter().map(|(i, p)| (name(*i), *p)).collect();
            t.sort_by(|a, b| a.0.cmp(&b.0));
            t == vec![("Equant".to_string(), 1.0), ("Estock".to_string(), -1.0), ("S".to_string(), 1.0)]
                || t == vec![("Equant".to_string(), -1.0), ("Estock".to_string(), 1.0), ("S".to_string(), -1.0)]
        });
        let Some(h) = horizon else { panic!("S · Equant = Estock not found") };
        assert!((h.number - 1.0).abs() < 1e-3, "{}", h.number);
        let steps: Vec<&str> = h.path.iter().map(|p| p.1.as_str()).collect();
        // seven relations of the network, the paper's derivation (lP, H_end, rho_vacuum, Estock, T, Equant, S)
        assert_eq!(steps.len(), 7, "{steps:?}");
        for r in ["u_i_S", "u_i_Equant", "u_i_Tfloor", "u_i_Estock"] {
            assert!(steps.contains(&r), "{r} not in {steps:?}");
        }
        // the constants on 0 of Planck's scale, and the seed's family
        let at = |n: &str| u.names.iter().position(|x| x == n).unwrap();
        for c in ["u_c", "u_G", "u_hbar", "u_kB"] {
            assert!(u.planck[at(c)].abs() < 1e-4, "{c}: {} (value {})", u.planck[at(c)], u.value[at(c)]);
        }
        assert!((u.exponent[at("u_S")].unwrap() + 2.0).abs() < 1e-9);
        assert!((u.exponent[at("u_Estock")].unwrap() + 1.0).abs() < 1e-9);
    }
}
