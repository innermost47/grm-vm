// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A line of the language said in words (French or English): what the workshop, its editor and its lines show
//! under each line, so that the language is learned by reading what it says.

use crate::math::Expr;
use crate::text::Lang;

fn short(n: &str) -> String {
    n.parse::<f64>().map(grm_engine::lang::g).unwrap_or(n.to_string())
}

/// An expression said in words.
pub fn words(e: &Expr, fr: bool) -> String {
    let w = |e: &Expr| words(e, fr);
    match e {
        Expr::Num(n) => short(n),
        Expr::Var(v) => v.clone(),
        Expr::Neg(x) => format!("{} {}", if fr { "moins" } else { "minus" }, w(x)),
        Expr::Sum(terms) => {
            let mut s = String::new();
            for (k, (minus, x)) in terms.iter().enumerate() {
                if k > 0 || *minus {
                    s.push_str(match (minus, fr) {
                        (true, true) => " moins ",
                        (true, false) => " minus ",
                        (false, true) => " plus ",
                        (false, false) => " plus ",
                    });
                }
                s.push_str(&w(x));
            }
            s.trim().to_string()
        }
        Expr::Mul(f) => f.iter().map(w).collect::<Vec<_>>().join(if fr { " fois " } else { " times " }),
        Expr::Div(a, b) => format!("{} {} {}", w(a), if fr { "divisé par" } else { "divided by" }, w(b)),
        Expr::Pow(a, b) => match b.as_ref() {
            Expr::Num(n) if n == "2" => format!("{} {}", w(a), if fr { "au carré" } else { "squared" }),
            Expr::Num(n) if n == "3" => format!("{} {}", w(a), if fr { "au cube" } else { "cubed" }),
            Expr::Neg(x) if **x == Expr::Num("1".into()) => {
                format!("{} {}", if fr { "l'inverse de" } else { "one over" }, w(a))
            }
            _ => format!("{} {} {}", w(a), if fr { "puissance" } else { "to the power" }, w(b)),
        },
        Expr::Call(f, args) => {
            let inner = args.iter().map(w).collect::<Vec<_>>().join(", ");
            match (f.as_str(), fr) {
                ("ln", true) => format!("le logarithme de {inner}"),
                ("ln", false) => format!("the logarithm of {inner}"),
                ("exp", true) => format!("e puissance {inner}"),
                ("exp", false) => format!("e to the power {inner}"),
                ("sqrt", true) => format!("la racine de {inner}"),
                ("sqrt", false) => format!("the square root of {inner}"),
                _ => format!("{f}({inner})"),
            }
        }
        Expr::Paren(x) => format!("({})", w(x)),
        Expr::Rel(sides, ops) => {
            let mut s = w(&sides[0]);
            for (op, side) in ops.iter().zip(&sides[1..]) {
                let op = match (op.as_str(), fr) {
                    ("=", true) => "est égal à",
                    ("=", false) => "is",
                    (">=", true) => "vaut au moins",
                    (">=", false) => "is at least",
                    ("<=", true) => "vaut au plus",
                    ("<=", false) => "is at most",
                    (o, _) => o,
                };
                s.push_str(&format!(" {op} {}", w(side)));
            }
            s
        }
    }
}

/// A line of the language said in words (empty when it is not understood).
pub fn explain(line: &str, lang: Lang) -> String {
    let fr = lang == Lang::Fr;
    let line = line.trim();
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.trim();
    let name_value = |r: &str| -> (String, Option<String>) {
        let r =
            r.split_whitespace().filter(|w| !["signed", "positive", "fixed"].contains(w)).collect::<Vec<_>>().join(" ");
        match r.split_once('=') {
            Some((n, v)) => (n.trim().to_string(), Some(v.trim().to_string())),
            None => (r.trim().to_string(), None),
        }
    };
    let expr = |s: &str| crate::math::parse(s).map(|e| words(&e, fr)).unwrap_or(s.to_string());
    match word {
        "node" => {
            let (n, v) = name_value(rest);
            let signed = rest.contains("signed");
            let kind = match (signed, fr) {
                (true, true) => "une quantité qui peut être négative",
                (false, true) => "une quantité (jamais négative)",
                (true, false) => "a quantity that can be negative",
                (false, false) => "a quantity (never negative)",
            };
            match (v, fr) {
                (Some(v), true) => format!("« {n} » est {kind}, qui part de {v}."),
                (Some(v), false) => format!("“{n}” is {kind}, starting from {v}."),
                (None, true) => format!("« {n} » est {kind}, encore inconnue."),
                (None, false) => format!("“{n}” is {kind}, unknown yet."),
            }
        }
        "input" => {
            let (n, v) = name_value(rest);
            let v = v.map(|v| if fr { format!(", d'abord {v}") } else { format!(", first {v}") }).unwrap_or_default();
            if fr {
                format!("« {n} » vient de dehors (une entrée{v}).")
            } else {
                format!("“{n}” comes from outside (an input{v}).")
            }
        }
        "output" => {
            if fr {
                format!("On veut lire (ou entendre) « {rest} ».")
            } else {
                format!("“{rest}” is to be read (or heard).")
            }
        }
        "var" | "learn" => {
            let (n, v) = name_value(rest);
            let v = v.unwrap_or_default();
            match (word, fr) {
                ("var", true) => format!("« {n} » est un état qui change à chaque pas ; il part de {v}."),
                ("var", false) => format!("“{n}” is a state that changes at each step, from {v}."),
                (_, true) => format!("« {n} » est un poids que la machine apprend ; elle part de {v}."),
                (_, false) => format!("“{n}” is a weight the machine learns, from {v}."),
            }
        }
        "relate" | "keep" => {
            let (name, body) = rest.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or(("", rest));
            let body = body.split(" when ").next().unwrap_or(body);
            if fr {
                format!("{name} : {}.", expr(body))
            } else {
                format!("{name}: {}.", expr(body))
            }
        }
        "observe" => {
            let (n, v) = name_value(rest);
            let v = v.unwrap_or_default();
            if fr {
                format!("On a mesuré « {n} » : {v}.")
            } else {
                format!("“{n}” was measured: {v}.")
            }
        }
        "forget" => {
            if fr {
                format!("La mesure de « {rest} » est retirée : il redevient libre.")
            } else {
                format!("The measurement of “{rest}” is taken back: it is free again.")
            }
        }
        "set" => {
            let (n, e) = rest.split_once('=').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((rest, ""));
            if fr {
                format!("À chaque pas, « {n} » devient {}.", expr(e))
            } else {
                format!("At each step, “{n}” becomes {}.", expr(e))
            }
        }
        "carry" => {
            if fr {
                format!("D'un pas à l'autre : {}.", rest.replace('=', "suit"))
            } else {
                format!("From step to step: {}.", rest.replace('=', "follows"))
            }
        }
        "ask" => {
            if fr {
                format!("Que vaut « {rest} », avec quelle certitude, et pourquoi ?")
            } else {
                format!("What is “{rest}”, how sure, and why?")
            }
        }
        _ => String::new(),
    }
}
