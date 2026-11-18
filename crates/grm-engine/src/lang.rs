// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! GRML, the Global Relational Model Language: one statement per line (reference: docs/GRML.md of grm-core).
//!
//! ```text
//! node H = 2.2e-18             a positive quantity (solved in log10); `fixed`, `signed`
//! relate r: S * H^2 = pi       a product relation; with + and -, a sum (kept as a product of exponentials)
//! relate r: L = ln(y)          the converter between a quantity and its exponent
//! observe H = 2.18e-18 ± 1%    a measurement;  forget H
//! keep land: y >= 2 when x in 3 .. 8        a contact
//! var / learn / input / output NAME [= v] [positive] [M L T Q K]
//! set x = x + 1 [when ...]     written at each tick
//! fuse A B [inverse]           B was A under another name (or its inverse)
//! network NAME(p, q) ... end   a network that can be called;  call NAME(p=1, q=2);  use FILE.grm
//! ```
//!
//! Everything said carries a provenance (who said it, from where): see network.rs.

use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::bail;
use crate::error::{GrmError, Result};
use crate::network::{
    CarryGroup, Cond, KeepOp, Kind, Network, NodeSpec, Prior, Provenance, Relation, SetRule, Template,
};

pub const DEFAULT_RELATION_SIGMA: f64 = 1e-9; // dex: a product relation written without "±" is exact
pub const DEFAULT_SUM_SIGMA: f64 = 1e-9; // in the relation's unit: a sum written without "±" is exact
pub const DEFAULT_MEASURE_SIGMA: f64 = 0.01; // dex
pub const DEFAULT_SIGNED_SIGMA: f64 = 1e-3; // in the node's unit
pub const VAR_SIGMA: f64 = 1e-9; // a variable holds its value
pub const LEARN_PRIOR: f64 = 1e3; // what a weight is before it has seen anything

const NAME: &str = r"[A-Za-z_~][A-Za-z0-9_]*";
const NUMBER: &str = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?";

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("a valid pattern"))
}

macro_rules! pattern {
    ($name:ident, $body:expr) => {
        fn $name() -> &'static Regex {
            static CELL: OnceLock<Regex> = OnceLock::new();
            re(&CELL, || $body)
        }
    };
}

pattern!(re_name_full, format!("^{NAME}$"));
pattern!(re_name, NAME.to_string());
pattern!(re_number_full, format!("^{NUMBER}$"));
pattern!(re_condition, format!(r"^({NAME})\s+in\s+(\S+)\s*\.\.\s*(\S+)$"));
pattern!(re_and, r"\s+and\s+".to_string());
pattern!(re_dim, r"\[([^\]]*)\]\s*$".to_string());
pattern!(re_ln, format!(r"^\s*({NAME})\s*=\s*ln\(\s*({NAME})\s*\)\s*$"));
pattern!(re_keep, format!(r"^\s*({NAME})\s*(>=|<=)\s*(\S+)\s*$"));
pattern!(re_node, format!(r"^({NAME})\s*(?:=\s*(\S+))?((?:\s+(?:fixed|signed))*)$"));
pattern!(re_observe, format!(r"^({NAME})\s*=\s*([^±]+?)\s*(?:±\s*(.+))?$"));
pattern!(re_call, format!(r"^({NAME})\s*\(([^)]*)\)$"));
pattern!(re_role, format!(r"^({NAME})\s*(?:=\s*(\S+))?(\s+positive)?$"));
pattern!(re_set, format!(r"^({NAME})\s*=\s*(.+)$"));
pattern!(re_side_split, r"\s*([*/])\s*".to_string());
pattern!(re_tight, r"([*/^(])\s+".to_string());
pattern!(re_sum_factor, format!(r"(?:([+-])\s*)?(?:({NUMBER})\s*\*\s*)?$"));
pattern!(re_arith, r"^[0-9eE.+\-*/() ]+$".to_string());

pub fn is_name(s: &str) -> bool {
    re_name_full().is_match(s)
}

/// A number as `%g` writes it (6 significant digits unless asked).
pub fn fmt_g(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let p = precision.max(1);
    let exp = v.abs().log10().floor() as i32;
    // round first, the exponent may change (9.9999995 -> 10)
    let sci = format!("{:.*e}", p - 1, v);
    let (mant, e) = sci.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap_or(exp);
    if e < -4 || e >= p as i32 {
        let mant = trim_zeros(mant);
        format!("{}e{}{:02}", mant, if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        let decimals = (p as i32 - 1 - e).max(0) as usize;
        trim_zeros(&format!("{:.*}", decimals, v))
    }
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

pub fn g(v: f64) -> String {
    fmt_g(v, 6)
}

/// The mathematical constants, known everywhere the same way (a number, a node's value, a relation, what
/// follows); no node is called as one of them is.
pub const CONSTANTS: [(&str, f64); 9] = [
    ("pi", std::f64::consts::PI),
    ("tau", std::f64::consts::TAU),
    ("e", std::f64::consts::E),
    ("phi", 1.618_033_988_749_895),
    ("sqrt2", std::f64::consts::SQRT_2),
    ("ln2", std::f64::consts::LN_2),
    ("ln10", std::f64::consts::LN_10),
    ("c0", 299_792_458.0),
    ("inf", f64::INFINITY),
];

/// A constant's value, by its name.
pub fn constant(t: &str) -> Option<f64> {
    CONSTANTS.iter().find(|(n, _)| *n == t).map(|(_, v)| *v)
}

/// A node's name is its own: not a constant's.
fn own_name(name: &str) -> Result<()> {
    if constant(name).is_some() {
        bail!("{name} is a constant ({}): a node is called otherwise", g(constant(name).unwrap_or(0.0)));
    }
    Ok(())
}

/// A number: a literal, pi / e / tau / inf, or a little arithmetic on numbers (as a call writes "{top} - 0.25").
pub fn number(text: &str) -> Result<f64> {
    let t = text.trim();
    if let Some(v) = constant(t) {
        return Ok(v);
    }
    let lower = t.to_lowercase();
    match lower.as_str() {
        "-inf" => return Ok(f64::NEG_INFINITY),
        "+inf" => return Ok(f64::INFINITY),
        _ => {}
    }
    if re_number_full().is_match(&lower) {
        if let Ok(v) = lower.parse::<f64>() {
            return Ok(v);
        }
    }
    if re_arith().is_match(&lower) && lower.chars().any(|c| c.is_ascii_digit()) {
        if let Some(v) = Arith::new(&lower).eval() {
            return Ok(v);
        }
    }
    bail!("not a number: '{}'", text)
}

/// A tiny evaluator of + - * / and parentheses on numbers.
struct Arith<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Arith<'a> {
    fn new(s: &'a str) -> Self {
        Arith { s: s.as_bytes(), i: 0 }
    }
    fn eval(mut self) -> Option<f64> {
        let v = self.sum()?;
        self.space();
        if self.i == self.s.len() {
            Some(v)
        } else {
            None
        }
    }
    fn space(&mut self) {
        while self.i < self.s.len() && self.s[self.i] == b' ' {
            self.i += 1;
        }
    }
    fn sum(&mut self) -> Option<f64> {
        let mut v = self.product()?;
        loop {
            self.space();
            match self.s.get(self.i) {
                Some(b'+') => {
                    self.i += 1;
                    v += self.product()?;
                }
                Some(b'-') => {
                    self.i += 1;
                    v -= self.product()?;
                }
                _ => return Some(v),
            }
        }
    }
    fn product(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            self.space();
            match self.s.get(self.i) {
                Some(b'*') => {
                    self.i += 1;
                    v *= self.unary()?;
                }
                Some(b'/') => {
                    self.i += 1;
                    v /= self.unary()?;
                }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        self.space();
        match self.s.get(self.i) {
            Some(b'-') => {
                self.i += 1;
                Some(-self.unary()?)
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            Some(b'(') => {
                self.i += 1;
                let v = self.sum()?;
                self.space();
                if self.s.get(self.i) == Some(&b')') {
                    self.i += 1;
                    Some(v)
                } else {
                    None
                }
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    let exp_sign =
                        (c == b'-' || c == b'+') && self.i > start && matches!(self.s[self.i - 1], b'e' | b'E');
                    if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || exp_sign {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
            }
        }
    }
}

/// '1%' -> dex; '0.004 dex' or '0.004' -> dex.
pub fn sigma_dex(text: Option<&str>, default: f64) -> Result<f64> {
    let Some(text) = text else { return Ok(default) };
    let t: String = text.trim().to_lowercase().chars().filter(|c| *c != ' ').collect();
    if let Some(p) = t.strip_suffix('%') {
        return Ok((1.0 + number(p)? / 100.0).log10());
    }
    number(t.strip_suffix("dex").unwrap_or(&t))
}

/// 'S * H^2 / tP^-1 * 2' -> ({node: power}, log10 of the numbers), or with `signed_ok` the numbers as a coefficient.
pub fn parse_side(text: &str, signed_ok: bool) -> Result<(IndexMap<String, f64>, f64)> {
    let text = text.trim();
    if text.is_empty() {
        bail!("an empty side of a relation");
    }
    let mut terms: IndexMap<String, f64> = IndexMap::new();
    let (mut log_const, mut coef) = (0.0, 1.0);
    let mut sign = 1.0;
    let mut parts: Vec<&str> = Vec::new();
    let mut last = 0;
    for m in re_side_split().captures_iter(text) {
        let whole = m.get(0).unwrap();
        parts.push(&text[last..whole.start()]);
        parts.push(m.get(1).unwrap().as_str());
        last = whole.end();
    }
    parts.push(&text[last..]);
    for part in parts {
        if part == "*" || part == "/" {
            sign = if part == "*" { 1.0 } else { -1.0 };
            continue;
        }
        if part.is_empty() {
            continue;
        }
        let (base, exp) = match part.split_once('^') {
            Some((b, e)) => (b, Some(e)),
            None => (part, None),
        };
        let power = sign
            * match exp {
                Some(e) => number(e.trim_matches(|c| c == '(' || c == ')' || c == ' '))?,
                None => 1.0,
            };
        let base = base.trim_matches(|c| c == '(' || c == ')' || c == ' ');
        if re_number_full().is_match(base) || constant(base).is_some() {
            let value = number(base)?;
            if signed_ok {
                coef *= value.powf(power);
            } else {
                if value <= 0.0 {
                    bail!("a product relation multiplies positive quantities: '{}'", base);
                }
                log_const += power * value.log10();
            }
        } else if is_name(base) {
            *terms.entry(base.to_string()).or_insert(0.0) += power;
        } else {
            bail!("cannot read '{}'", part);
        }
        sign = 1.0;
    }
    Ok((terms, if signed_ok { coef } else { log_const }))
}

/// 'x0 + v * dt - 3' -> [(+1, 'x0'), (+1, 'v * dt'), (-1, '3')]: + and - at the top level (not in 1e-5, nor in a
/// power ^-2, nor after * / ( ).
pub fn split_sum(text: &str) -> Vec<(f64, String)> {
    let text = re_tight().replace_all(text.trim(), "$1").to_string();
    let chars: Vec<char> = text.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '+' || c == '-' {
            // where the separator would start: the spaces before it belong to it
            let mut k = i;
            while k > 0 && chars[k - 1].is_whitespace() {
                k -= 1;
            }
            let after_op = k >= 1 && "*/^(".contains(chars[k - 1]);
            // a number's exponent (2e-3): the sign right after its e, and the e after the digits of a number, not
            // after a name's (x2e - y is a difference)
            let exponent = k == i && k >= 2 && (chars[k - 1] == 'e' || chars[k - 1] == 'E') && {
                let mut d = k - 1;
                while d > 0 && (chars[d - 1].is_ascii_digit() || chars[d - 1] == '.') {
                    d -= 1;
                }
                d < k - 1 && (d == 0 || !(chars[d - 1].is_alphanumeric() || chars[d - 1] == '_'))
            };
            if !after_op && !exponent {
                pieces.push(current.trim_end().to_string());
                pieces.push(c.to_string());
                current = String::new();
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                continue;
            }
        }
        current.push(c);
        i += 1;
    }
    pieces.push(current);
    let mut out = Vec::new();
    let mut sign = 1.0;
    for p in pieces {
        if p == "+" {
            continue;
        }
        if p == "-" {
            sign = -sign;
            continue;
        }
        if !p.trim().is_empty() {
            out.push((sign, p.trim().to_string()));
        }
        sign = 1.0;
    }
    out
}

fn conditions(net: &mut Network, when: &str, origin: &Provenance) -> Result<Vec<Cond>> {
    let mut conds = Vec::new();
    if when.trim().is_empty() {
        return Ok(conds);
    }
    for c in re_and().split(when.trim()) {
        let Some(m) = re_condition().captures(c.trim()) else {
            bail!("cannot read the condition '{}' (node in lo .. hi)", c);
        };
        let node = m[1].to_string();
        conds.push(Cond { node: node.clone(), lo: number(&m[2])?, hi: number(&m[3])? });
        ensure_node(net, &node, 0.0, true, origin);
    }
    Ok(conds)
}

fn ensure_node(net: &mut Network, name: &str, guess: f64, signed: bool, origin: &Provenance) {
    if !net.nodes.contains_key(name) {
        let mut spec = NodeSpec::new(guess, signed);
        spec.origin = origin.clone();
        net.nodes.insert(name.to_string(), spec);
    }
}

/// '[0 1 -2 0 0]' -> the dimension (mass, length, time, charge, temperature), and the text without it.
fn dim_of(text: &str) -> Result<(Option<[f64; 5]>, String)> {
    let Some(m) = re_dim().captures(text) else { return Ok((None, text.to_string())) };
    let parts: Vec<&str> = m[1].split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    if parts.len() != 5 {
        bail!("a dimension is [M L T Q K]: five powers");
    }
    let mut d = [0.0; 5];
    for (k, p) in parts.iter().enumerate() {
        d[k] = p.parse().map_err(|_| GrmError::new(format!("not a number: '{p}'")))?;
    }
    Ok((Some(d), text[..m.get(0).unwrap().start()].trim().to_string()))
}

/// A name fused as the inverse of a node is that node to the power -1.
fn inverted(net: &Network, terms: IndexMap<String, f64>) -> IndexMap<String, f64> {
    if net.inverses.is_empty() {
        return terms;
    }
    let mut out: IndexMap<String, f64> = IndexMap::new();
    for (n, p) in terms {
        let (n, p) = match net.inverses.get(&n) {
            Some(k) => (k.clone(), -p),
            None => (n, p),
        };
        *out.entry(n).or_insert(0.0) += p;
    }
    out.retain(|_, p| p.abs() > 1e-15);
    out
}

fn is_sum(net: &Network, left: &str, right: &str) -> bool {
    let (l, r) = (split_sum(left), split_sum(right));
    if l.len() > 1 || r.len() > 1 || left.trim().starts_with('-') || right.trim().starts_with('-') {
        return true;
    }
    if matches!(right.trim(), "0" | "0.0") || matches!(left.trim(), "0" | "0.0") {
        return true;
    }
    for side in [left, right] {
        for m in re_name().find_iter(side) {
            if net.nodes.get(m.as_str()).is_some_and(|s| s.signed) {
                return true;
            }
        }
    }
    false
}

fn relation(name: String, kind: Kind, sigma: f64, when: Vec<Cond>, text: String, origin: &Provenance) -> Relation {
    Relation { name, kind, sigma, when, text, origin: origin.clone(), strength: 1.0 }
}

fn relate(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let (name, body) = match rest.split_once(':') {
        Some((n, b)) => (n.trim().to_string(), b),
        None => (format!("r{}", net.relations.len() + 1), rest),
    };
    let (body, when) = body.split_once(" when ").unwrap_or((body, ""));
    let (body, sig) = body.split_once('±').unwrap_or((body, ""));
    if !body.contains('=') {
        bail!("relate NAME: left = right [+/- sigma] [when node in lo .. hi and ...]");
    }
    let conds = conditions(net, when, origin)?;
    if let Some(m) = re_ln().captures(body) {
        let (l, y) = (m[1].to_string(), m[2].to_string());
        ensure_node(net, &l, 0.0, true, origin);
        ensure_node(net, &y, 1.0, false, origin);
        if net.nodes[&y].signed {
            bail!("ln({y}): {y} is signed; a logarithm takes a positive quantity");
        }
        if !net.nodes[&l].signed {
            bail!("{l} = ln({y}): {l} must be signed (a logarithm can be negative)");
        }
        net.put(relation(
            name.clone(),
            Kind::Log { log: l.clone(), node: y.clone() },
            0.0,
            vec![],
            body.trim().to_string(),
            origin,
        ));
        return Ok(format!("relation {name}: {l} = ln({y}) (the converter between a quantity and its exponent)"));
    }
    let (left, right) = body.split_once('=').unwrap();
    let text =
        format!("{}{}", body.trim(), if conds.is_empty() { String::new() } else { format!(" when {}", when.trim()) });
    if is_sum(net, left, right) {
        let mut terms: Vec<(f64, IndexMap<String, f64>)> = Vec::new();
        for (side, s0) in [(left, 1.0), (right, -1.0)] {
            for (s, piece) in split_sum(side) {
                let (t, c) = parse_side(&piece, true)?;
                if c != 0.0 {
                    terms.push((s0 * s * c, inverted(net, t)));
                }
            }
        }
        if !terms.iter().any(|(_, t)| !t.is_empty()) {
            bail!("a relation needs at least one node");
        }
        for (_, t) in &terms {
            for n in t.keys() {
                ensure_node(net, n, 0.0, true, origin);
            }
        }
        let sigma = if sig.trim().is_empty() { DEFAULT_SUM_SIGMA } else { number(sig)? };
        net.put(relation(name.clone(), Kind::Sum { terms }, sigma, conds, text.clone(), origin));
        return Ok(format!("relation {name} (a sum, kept as a product of exponentials): {text}"));
    }
    let (lt, lc) = parse_side(left, false)?;
    let (rt, rc) = parse_side(right, false)?;
    let (lt, rt) = (inverted(net, lt), inverted(net, rt));
    let mut terms = lt;
    for (n, p) in rt {
        *terms.entry(n).or_insert(0.0) -= p;
    }
    terms.retain(|_, p| p.abs() > 1e-15);
    if terms.is_empty() {
        bail!("a relation needs at least one node");
    }
    for n in terms.keys() {
        ensure_node(net, n, 1.0, false, origin);
        if net.nodes[n].signed {
            bail!("{n} is signed: a product relation needs positive quantities");
        }
    }
    let sigma = sigma_dex(if sig.is_empty() { None } else { Some(sig) }, DEFAULT_RELATION_SIGMA)?;
    net.put(relation(name.clone(), Kind::Product { terms, rhs: rc - lc }, sigma, conds, text.clone(), origin));
    Ok(format!("relation {name}: {text}"))
}

fn keep(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let (name, body) = match rest.split_once(':') {
        Some((n, b)) => (n.trim().to_string(), b),
        None => (format!("k{}", net.relations.len() + 1), rest),
    };
    let (cond, when) = body.split_once(" when ").unwrap_or((body, ""));
    let Some(m) = re_keep().captures(cond) else {
        bail!("keep NAME: node >= value [when node in lo .. hi and ...]");
    };
    let conds = conditions(net, when, origin)?;
    let node = m[1].to_string();
    ensure_node(net, &node, 0.0, true, origin);
    let op = if &m[2] == ">=" { KeepOp::AtLeast } else { KeepOp::AtMost };
    let text = body.trim().to_string();
    net.put(relation(name.clone(), Kind::Keep { node, op, bound: number(&m[3])? }, 0.0, conds, text.clone(), origin));
    Ok(format!("contact {name}: {text}"))
}

/// The short forms of the statements (first word of a line only).
pub fn short_form(word: &str) -> Option<&'static str> {
    Some(match word {
        "nd" => "node",
        "rel" => "relate",
        "obs" => "observe",
        "fgt" => "forget",
        "kp" => "keep",
        "net" => "network",
        "cl" => "call",
        "lrn" => "learn",
        "inp" => "input",
        "out" => "output",
        _ => return None,
    })
}

/// A line as the language reads it: a short form at its head made whole, `+/-` written `±`.
pub fn normal(line: &str) -> String {
    let line = line.replace("+/-", "±");
    let head = line.trim_start();
    let indent = &line[..line.len() - head.len()];
    let (word, rest) = match head.split_once(' ') {
        Some((w, r)) => (w, Some(r)),
        None => (head, None),
    };
    match short_form(&word.to_lowercase()) {
        Some(full) => format!("{indent}{full}{}", rest.map(|r| format!(" {r}")).unwrap_or_default()),
        None => line.clone(),
    }
}

/// A name fused into another node means that node.
fn with_aliases(net: &Network, line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for m in re_name().find_iter(line) {
        let before = line[..m.start()].chars().last();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '~') {
            continue;
        }
        if let Some(target) = net.aliases.get(m.as_str()) {
            out.push_str(&line[last..m.start()]);
            out.push_str(target);
            last = m.end();
        }
    }
    out.push_str(&line[last..]);
    out
}

/// Reads one statement into the network. Returns what was done (empty for nothing), or None if the line is not a
/// statement of the language (a question for the interpreter). `base`: the folder `use` reads from. `origin`:
/// where what it says comes from.
pub fn statement(
    net: &mut Network,
    line: &str,
    base: Option<&std::path::Path>,
    origin: &Provenance,
) -> Result<Option<String>> {
    let line = normal(line.split('#').next().unwrap_or("").trim_end());
    if let Some((name, params, mut lines)) = net.recording.take() {
        if line.trim().eq_ignore_ascii_case("end") {
            let n = lines.len();
            net.networks.insert(name.clone(), Template { params: params.clone(), lines });
            return Ok(Some(format!("network {name}({}): {n} lines", params.join(", "))));
        }
        if !line.trim().is_empty() {
            lines.push(line.trim().to_string());
        }
        net.recording = Some((name, params, lines));
        return Ok(Some(String::new()));
    }
    let mut line = line.trim().to_string();
    if line.is_empty() {
        return Ok(Some(String::new()));
    }
    if !net.aliases.is_empty() {
        line = with_aliases(net, &line);
    }
    if line.contains("sum(") && !line.trim_start().to_lowercase().starts_with("repeat") {
        line = sums(&line)?;
    }
    let (word, rest) = line.split_once(' ').unwrap_or((&line, ""));
    let word = word.to_lowercase();
    let rest = rest.trim();
    // vectors: a node of several components, said once; a statement about vectors, said for each component
    if let Some(said) = vector_declaration(net, &word, rest, base, origin) {
        return said.map(Some);
    }
    if let Some(lines) = vector_expansion(net, &word, rest)? {
        let mut said = vec![];
        for l in &lines {
            if let Some(s) = statement(net, l, base, origin)? {
                said.push(s);
            }
        }
        return Ok(Some(format!("{} ({} components)", said.first().cloned().unwrap_or_default(), lines.len())));
    }
    let said = match word.as_str() {
        "var" | "learn" | "input" | "output" | "memory" => role(net, &word, rest, origin)?,
        "repeat" => {
            let mut said = 0;
            for l in repeat(rest)? {
                if statement(net, &l, base, origin)?.is_some_and(|s| !s.is_empty()) {
                    said += 1;
                }
            }
            format!("repeated: {said} statements")
        }
        "set" => set(net, rest, origin)?,
        "rail" => rail(net, rest)?,
        "delay" => delay(net, rest, origin)?,
        "audio" => audio(net, rest, base.unwrap_or(std::path::Path::new(".")), origin)?,
        "carry" => carry(net, rest, origin)?,
        "observe-joint" => observe_joint(net, rest)?,
        "fuse" => {
            let w: Vec<&str> = rest.split_whitespace().collect();
            fuse(net, &w)?
        }
        "node" => node(net, rest, origin)?,
        "relate" => relate(net, rest, origin)?,
        "keep" => keep(net, rest, origin)?,
        "observe" => observe(net, rest)?,
        "forget" if net.priors.iter().any(|p| p.name == format!("observe {rest}")) => {
            net.priors.retain(|p| p.name != format!("observe {rest}"));
            format!("forgot the measurement of {rest}")
        }
        "forget" => {
            if net.measured.shift_remove(rest).is_none() {
                bail!("{rest} is not measured");
            }
            format!("forgot the measurement of {rest}")
        }
        "network" => {
            let Some(m) = re_call().captures(rest) else { bail!("network NAME(param, param...) ... end") };
            let params = m[2].split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
            net.recording = Some((m[1].to_string(), params, Vec::new()));
            String::new()
        }
        "call" => call(net, rest, base, origin)?,
        "use" => use_file(net, rest, base, origin)?,
        _ => return Ok(None),
    };
    Ok(Some(said))
}

pattern!(re_vector_decl, format!(r"^({NAME})\[(\d+)\](.*)$"));
pattern!(re_dot, format!(r"dot\(\s*({NAME})\s*,\s*({NAME})\s*\)"));
pattern!(re_list, r"^\[([^\]]*)\](.*)$".to_string());

/// `node v[3] ...`, `var p[2] = 0`, `input pos[2]`...: a vector, its components made one by one.
fn vector_declaration(
    net: &mut Network,
    word: &str,
    rest: &str,
    base: Option<&std::path::Path>,
    origin: &Provenance,
) -> Option<Result<String>> {
    if !matches!(word, "node" | "var" | "learn" | "input" | "output" | "memory") {
        return None;
    }
    let m = re_vector_decl().captures(rest)?;
    let (name, tail) = (m[1].to_string(), m[3].to_string());
    let dim: usize = match m[2].parse() {
        Ok(d) if (1..=4096).contains(&d) => d,
        _ => return Some(Err(GrmError::new(format!("{name}: a vector has 1 to 4096 components")))),
    };
    for k in 0..dim {
        if let Err(e) = statement(net, &format!("{word} {name}_{k}{tail}"), base, origin) {
            return Some(Err(e));
        }
    }
    net.vectors.insert(name.clone(), dim);
    Some(Ok(format!("vector {name}[{dim}]: {name}_0 .. {name}_{}", dim - 1)))
}

/// A statement that names vectors, said for each component: `relate r: w = 2 * v` is `r_k: w_k = 2 * v_k`;
/// `dot(v, w)` is `v_0 * w_0 + v_1 * w_1 + ...`; `observe v = [1, 2, 3]`; `forget v`. A scalar is the same for every
/// component. None when the statement names no vector.
fn vector_expansion(net: &Network, word: &str, rest: &str) -> Result<Option<Vec<String>>> {
    if net.vectors.is_empty() || !matches!(word, "relate" | "keep" | "set" | "observe" | "forget") {
        return Ok(None);
    }
    if word == "forget" && net.priors.iter().any(|p| p.name == format!("observe {}", rest.trim())) {
        return Ok(None);
    }
    let dim_of = |n: &str| net.vectors.get(n).copied();
    // dot products first: a sum of products (a "-" before it goes to each of its terms)
    let mut body = rest.to_string();
    while let Some(m) = re_dot().captures(&body) {
        let (a, b) = (m[1].to_string(), m[2].to_string());
        let (Some(da), Some(db)) = (dim_of(&a), dim_of(&b)) else { bail!("dot({a}, {b}): both must be vectors") };
        if da != db {
            bail!("dot({a}, {b}): {da} and {db} components");
        }
        let whole = m.get(0).unwrap();
        let negative = body[..whole.start()].trim_end().ends_with('-');
        let join = if negative { " - " } else { " + " };
        let terms: Vec<String> = (0..da).map(|k| format!("{a}_{k} * {b}_{k}")).collect();
        body = format!("{}{}{}", &body[..whole.start()], terms.join(join), &body[whole.end()..]);
    }
    if word == "observe" {
        // observe v = [1, 2, 3] [± s]
        if let Some((name, value)) = body.split_once('=') {
            let name = name.trim();
            if let Some(d) = dim_of(name) {
                let Some(m) = re_list().captures(value.trim()) else {
                    bail!("observe {name} = [v0, v1, ...]: {name} is a vector")
                };
                let vals: Vec<&str> = m[1].split(',').map(|v| v.trim()).filter(|v| !v.is_empty()).collect();
                if vals.len() != d {
                    bail!("{name} has {d} components, not {}", vals.len());
                }
                let tail = m[2].trim();
                if tail.starts_with("cov") {
                    // measured together: one prior over its components, from the covariance said
                    return Ok(Some(vec![format!("observe-joint {name} {} {tail}", m[1].trim())]));
                }
                return Ok(Some(
                    vals.iter()
                        .enumerate()
                        .map(|(k, v)| format!("observe {name}_{k} = {v} {tail}").trim().to_string())
                        .collect(),
                ));
            }
        }
    }
    // the vectors it names, all of one dimension
    let dotted = body != rest;
    let whole = body.clone();
    let (label, body) = match word {
        "relate" | "keep" => match body.split_once(':') {
            Some((n, b)) if is_name(n.trim()) => (Some(n.trim().to_string()), b.to_string()),
            _ => (None, body.clone()),
        },
        _ => (None, body.clone()),
    };
    let mut dims = vec![];
    for m in re_name().find_iter(&body) {
        let before = body[..m.start()].chars().last();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            continue;
        }
        if let Some(d) = dim_of(m.as_str()) {
            dims.push(d);
        }
    }
    if dims.is_empty() {
        return Ok(if dotted { Some(vec![format!("{word} {whole}")]) } else { None });
    }
    let d = dims[0];
    if dims.iter().any(|x| *x != d) {
        bail!(
            "{word}: vectors of {} components together",
            dims.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    let mut lines = vec![];
    for k in 0..d {
        let mut out = String::new();
        let mut last = 0;
        for m in re_name().find_iter(&body) {
            let before = body[..m.start()].chars().last();
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            if dim_of(m.as_str()).is_some() {
                out.push_str(&body[last..m.start()]);
                out.push_str(&format!("{}_{k}", m.as_str()));
                last = m.end();
            }
        }
        out.push_str(&body[last..]);
        lines.push(match &label {
            Some(l) => format!("{word} {l}_{k}:{out}"),
            None => format!("{word} {}", out.trim()),
        });
    }
    Ok(Some(lines))
}

fn node(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let (dim, rest) = dim_of(rest)?;
    let Some(m) = re_node().captures(&rest) else { bail!("node NAME [= value] [fixed] [signed]") };
    let name = m[1].to_string();
    own_name(&name)?;
    let flags: Vec<&str> = m.get(3).map(|f| f.as_str().split_whitespace().collect()).unwrap_or_default();
    let old = net.nodes.get(&name).cloned();
    let signed = flags.contains(&"signed") || old.as_ref().is_some_and(|o| o.signed);
    let guess = match m.get(2) {
        Some(v) => number(v.as_str())?,
        None => old.as_ref().map(|o| o.guess).unwrap_or(if signed { 0.0 } else { 1.0 }),
    };
    if !signed && guess <= 0.0 {
        bail!("a node is a positive quantity (solved in log10): say `signed` for a signed one");
    }
    let fixed = flags.contains(&"fixed");
    let mut spec = old.unwrap_or_else(|| {
        let mut s = NodeSpec::new(guess, signed);
        s.origin = origin.clone();
        s
    });
    spec.guess = guess;
    spec.fixed = fixed;
    spec.signed = signed;
    if dim.is_some() {
        spec.dim = dim;
    }
    net.nodes.insert(name.clone(), spec);
    Ok(format!(
        "node {name} = {}{}{}",
        g(guess),
        if fixed { " (fixed)" } else { "" },
        if signed { " signed" } else { "" }
    ))
}

fn observe(net: &mut Network, rest: &str) -> Result<String> {
    let Some(m) = re_observe().captures(rest) else { bail!("observe NAME = value [+/- 1% | +/- 0.004 dex]") };
    let name = m[1].to_string();
    let value = number(&m[2])?;
    let sg = m.get(3).map(|s| s.as_str());
    if net.nodes.get(&name).is_some_and(|n| n.signed) {
        let s = match sg {
            Some(s) if s.trim().ends_with('%') => value.abs() * number(&s.trim()[..s.trim().len() - 1])? / 100.0,
            Some(s) => number(s)?,
            // as the senses do (Weber's law): known to a thousandth of itself, never better than a thousandth of a unit
            None => DEFAULT_SIGNED_SIGMA * value.abs().max(1.0),
        };
        if !(s > 0.0) {
            bail!("a measurement needs a positive sigma");
        }
        net.measured.insert(name.clone(), (value, s));
        return Ok(format!("observed {name} = {} ± {}", g(value), fmt_g(s, 3)));
    }
    if value <= 0.0 {
        bail!("{name} is a positive quantity: say `node {name} signed` first for a signed one");
    }
    if !net.nodes.contains_key(&name) {
        net.nodes.insert(name.clone(), NodeSpec::new(value, false));
    }
    let s = sigma_dex(sg, DEFAULT_MEASURE_SIGMA)?;
    net.measured.insert(name.clone(), (value, s));
    Ok(format!("observed {name} = {} ± {} dex", g(value), fmt_g(s, 3)))
}

fn call(net: &mut Network, rest: &str, base: Option<&std::path::Path>, origin: &Provenance) -> Result<String> {
    let m = re_call().captures(rest);
    let Some(m) = m.filter(|m| net.networks.contains_key(&m[1])) else {
        bail!("call NETWORK(param=value, ...): no network '{}'", rest.split('(').next().unwrap_or("").trim());
    };
    let name = m[1].to_string();
    let template = net.networks[&name].clone();
    let mut given: IndexMap<String, String> = IndexMap::new();
    for kv in m[2].split(',') {
        if !kv.trim().is_empty() {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            given.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let missing: Vec<&String> = template.params.iter().filter(|p| !given.contains_key(*p)).collect();
    if !missing.is_empty() {
        bail!("call {name}: missing {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    for ln in &template.lines {
        let mut text = ln.clone();
        for (k, v) in &given {
            text = text.replace(&format!("{{{k}}}"), v);
        }
        statement(net, &text, base, origin)?;
    }
    let said = given.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ");
    net.calls.push((name.clone(), given));
    Ok(format!("called {name}({said})"))
}

fn use_file(net: &mut Network, rest: &str, base: Option<&std::path::Path>, origin: &Provenance) -> Result<String> {
    let rest = rest.trim().trim_matches('"');
    let mut path = std::path::PathBuf::from(rest);
    if let Some(b) = base {
        if !path.is_absolute() && !path.exists() {
            path = b.join(rest);
        }
    }
    let Ok(text) = std::fs::read_to_string(&path) else { bail!("no file {rest}") };
    let folder = path.canonicalize().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let mut count = 0;
    for (k, ln) in text.lines().enumerate() {
        match statement(net, ln, folder.as_deref(), origin) {
            Ok(Some(s)) if !s.is_empty() => count += 1,
            Ok(_) => {}
            Err(e) if e.located => return Err(e),
            Err(e) => {
                let mut err = GrmError::new(format!("in {file}, line {}: {}\n    {}", k + 1, ln.trim(), e.message));
                err.located = true;
                return Err(err);
            }
        }
    }
    Ok(format!("used {file}: {count} statements"))
}

const ROLES: [(&str, &str); 5] = [
    ("var", "a variable"),
    ("learn", "a weight it learns"),
    ("input", "an input"),
    ("output", "an output"),
    ("memory", "a memory"),
];

/// var / learn / input / output NAME [= value] [positive] [dim]: a signed node with a role.
/// `rail NAME R`: what NAME can never go beyond (in SI, either way): its potential. A node near its rail bends
/// softly towards it, as a circuit saturates at its supply; never past it, never to the infinite.
fn rail(net: &mut Network, rest: &str) -> Result<String> {
    let words: Vec<&str> = rest.split(|c: char| c.is_whitespace() || c == '=').filter(|w| !w.is_empty()).collect();
    let [name, r] = words[..] else { bail!("rail NAME R") };
    let r = number(r)?;
    if !(r > 0.0 && r.is_finite()) {
        bail!("a rail is a positive number");
    }
    let Some(spec) = net.nodes.get_mut(name) else { bail!("rail {name}: no node {name} (say it first)") };
    spec.rail = Some(r);
    Ok(format!("rail: {name} within ±{}", g(r)))
}

fn role(net: &mut Network, word: &str, rest: &str, origin: &Provenance) -> Result<String> {
    let (dim, rest) = dim_of(rest)?;
    let Some(m) = re_role().captures(&rest) else { bail!("{word} NAME [= value] [positive] [M L T Q K]") };
    let name = m[1].to_string();
    own_name(&name)?;
    let old = net.nodes.get(&name).cloned();
    let positive = m.get(3).is_some() || old.as_ref().is_some_and(|o| !o.signed);
    let guess = match m.get(2) {
        Some(v) => number(v.as_str())?,
        None => old.as_ref().map(|o| o.guess).unwrap_or(if positive { 1.0 } else { 0.0 }),
    };
    if positive && guess <= 0.0 {
        bail!("{name} is positive: its value must be above 0");
    }
    let mut spec = old.unwrap_or_else(|| {
        let mut s = NodeSpec::new(guess, !positive);
        s.origin = origin.clone();
        s
    });
    spec.guess = guess;
    spec.fixed = false;
    spec.signed = !positive;
    match word {
        "var" => spec.var = true,
        "memory" => {
            spec.var = true;
            spec.memory = true;
        }
        "learn" => spec.learn = true,
        "input" => spec.input = true,
        _ => spec.output = true,
    }
    if dim.is_some() {
        spec.dim = dim;
    }
    net.nodes.insert(name.clone(), spec);
    // a memory said again keeps what it holds (its value is only where it starts, the first time)
    if word == "memory" && !net.measured.contains_key(&name) {
        net.measured.insert(name.clone(), (guess, VAR_SIGMA));
    }
    if (word == "var" || word == "input") && (m.get(2).is_some() || !net.measured.contains_key(&name)) {
        net.measured.insert(name.clone(), (guess, if positive { 1e-6 } else { VAR_SIGMA }));
    }
    if word == "learn" {
        net.measured.insert(name.clone(), (guess, if positive { 3.0 } else { LEARN_PRIOR }));
    }
    let label = ROLES.iter().find(|(w, _)| *w == word).map(|(_, l)| *l).unwrap_or(word);
    Ok(format!("{label}: {name} = {}", g(guess)))
}

/// `repeat k in A .. B: LINE`: the line said once for each whole k from A to B, each `{EXPRESSION}` in it computed
/// (with k, numbers, + - * / ^, parentheses, sin, cos, sqrt, ln, exp, abs, floor, round). Repeats can be nested:
/// `repeat k in 1 .. 4: repeat j in 1 .. 3: var w{k}_{j} = {0.1 * j}`. An expression it cannot compute (a
/// parameter of a network, `{name}`) is left as it is.
pub fn repeat(rest: &str) -> Result<Vec<String>> {
    repeat_in(rest, &[])
}

fn repeat_in(rest: &str, outer: &[(String, f64)]) -> Result<Vec<String>> {
    let Some((head, line)) = rest.split_once(':') else { bail!("repeat NAME in A .. B: LINE") };
    let Some(m) = re_condition().captures(head.trim()) else { bail!("repeat NAME in A .. B: LINE") };
    let var = m[1].to_string();
    let (a, b) = (number(&m[2])?, number(&m[3])?);
    if (b - a).abs() > 4096.0 {
        bail!("repeat: at most 4096 times");
    }
    let mut out = vec![];
    let mut k = a.round() as i64;
    while k as f64 <= b + 1e-9 {
        let mut env = outer.to_vec();
        env.push((var.clone(), k as f64));
        let line = line.trim();
        match line.split_once(char::is_whitespace) {
            Some((w, r)) if w.eq_ignore_ascii_case("repeat") => out.extend(repeat_in(r, &env)?),
            _ => out.push(substitute(line, &env).trim().to_string()),
        }
        k += 1;
    }
    Ok(out)
}

/// `sum(j in A .. B: TERM)`: TERM written for each whole j, joined by +, each `{...}` computed with j (a sum of
/// terms in one line: `relate r: y = sum(j in 1 .. 6: w{j} * s{j})`).
pub fn sums(line: &str) -> Result<String> {
    let mut line = line.to_string();
    while let Some(start) = line.find("sum(") {
        // its closing parenthesis (parentheses inside it counted)
        let mut depth = 0;
        let mut end = None;
        for (i, c) in line[start + 3..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + 3 + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { bail!("sum(j in A .. B: TERM): no closing parenthesis") };
        let inner = &line[start + 4..end];
        let Some((head, term)) = inner.split_once(':') else { bail!("sum(j in A .. B: TERM)") };
        let Some(m) = re_condition().captures(head.trim()) else { bail!("sum(j in A .. B: TERM)") };
        let var = m[1].to_string();
        let (a, b) = (number(&m[2])?.round() as i64, number(&m[3])?.round() as i64);
        if b < a || b - a > 4096 {
            bail!("sum: from A to B, at most 4096 terms");
        }
        // what stands before it (a sign, a factor) goes to each of its terms
        let before = &line[..start];
        let (head, sign, factor) = match re_sum_factor().captures(before) {
            Some(m) => (
                &before[..m.get(0).unwrap().start()],
                m.get(1).map(|s| s.as_str()).unwrap_or(""),
                m.get(2).map(|f| format!("{} * ", f.as_str())).unwrap_or_default(),
            ),
            None => (before, "", String::new()),
        };
        let join = if sign == "-" { " - " } else { " + " };
        let terms: Vec<String> =
            (a..=b).map(|k| format!("{factor}{}", substitute(term.trim(), &[(var.clone(), k as f64)]))).collect();
        let lead = if sign.is_empty() { String::new() } else { format!("{sign} ") };
        line = format!("{head}{lead}{}{}", terms.join(join), &line[end + 1..]);
    }
    Ok(line)
}

/// Each `{...}` that names only these (and numbers) computed with them.
fn substitute(line: &str, env: &[(String, f64)]) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &rest[open + 1..open + close];
        match arithmetic(inner, env) {
            // a negative value after a sign: the sign turns (`- {x}` with x = -2 is `+ 2`)
            Some(v) if v < 0.0 && (out.trim_end().ends_with('+') || out.trim_end().ends_with('-')) => {
                let t = out.trim_end().len();
                let turned = if out[..t].ends_with('+') { '-' } else { '+' };
                out.replace_range(t - 1..t, &turned.to_string());
                out.push_str(&fmt_number(-v));
            }
            Some(v) => out.push_str(&fmt_number(v)),
            None => out.push_str(&rest[open..=open + close]),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

fn fmt_number(v: f64) -> String {
    if v == v.round() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// A small expression: numbers, one variable, + - * / ^, parentheses, a few functions. None when it names anything
/// else (it is not this repeat's to compute).
pub fn arithmetic(text: &str, env: &[(String, f64)]) -> Option<f64> {
    struct P<'a> {
        s: &'a [u8],
        at: usize,
        env: &'a [(String, f64)],
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.at < self.s.len() && self.s[self.at] == b' ' {
                self.at += 1;
            }
        }
        fn sum(&mut self) -> Option<f64> {
            let mut v = self.product()?;
            loop {
                self.ws();
                match self.s.get(self.at) {
                    Some(b'+') => {
                        self.at += 1;
                        v += self.product()?;
                    }
                    Some(b'-') => {
                        self.at += 1;
                        v -= self.product()?;
                    }
                    _ => return Some(v),
                }
            }
        }
        fn product(&mut self) -> Option<f64> {
            let mut v = self.power()?;
            loop {
                self.ws();
                match self.s.get(self.at) {
                    Some(b'*') => {
                        self.at += 1;
                        v *= self.power()?;
                    }
                    Some(b'/') => {
                        self.at += 1;
                        v /= self.power()?;
                    }
                    _ => return Some(v),
                }
            }
        }
        fn power(&mut self) -> Option<f64> {
            let base = self.unary()?;
            self.ws();
            if self.s.get(self.at) == Some(&b'^') {
                self.at += 1;
                let e = self.power()?;
                return Some(base.powf(e));
            }
            Some(base)
        }
        fn unary(&mut self) -> Option<f64> {
            self.ws();
            match self.s.get(self.at) {
                Some(b'-') => {
                    self.at += 1;
                    Some(-self.unary()?)
                }
                Some(b'(') => {
                    self.at += 1;
                    let v = self.sum()?;
                    self.ws();
                    if self.s.get(self.at) != Some(&b')') {
                        return None;
                    }
                    self.at += 1;
                    Some(v)
                }
                Some(c) if c.is_ascii_digit() || *c == b'.' => {
                    let start = self.at;
                    while self.at < self.s.len()
                        && (self.s[self.at].is_ascii_digit()
                            || self.s[self.at] == b'.'
                            || ((self.s[self.at] == b'e' || self.s[self.at] == b'E')
                                && self.s.get(self.at + 1).is_some_and(|c| c.is_ascii_digit() || *c == b'-')))
                    {
                        if self.s[self.at] == b'e' || self.s[self.at] == b'E' {
                            self.at += 1;
                        }
                        self.at += 1;
                    }
                    std::str::from_utf8(&self.s[start..self.at]).ok()?.parse().ok()
                }
                Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {
                    let start = self.at;
                    while self.at < self.s.len() && (self.s[self.at].is_ascii_alphanumeric() || self.s[self.at] == b'_')
                    {
                        self.at += 1;
                    }
                    let word = std::str::from_utf8(&self.s[start..self.at]).ok()?;
                    if let Some((_, v)) = self.env.iter().rev().find(|(n, _)| n == word) {
                        return Some(*v);
                    }
                    if let Some(v) = constant(word) {
                        return Some(v);
                    }
                    let f: fn(f64) -> f64 = match word {
                        "sin" => f64::sin,
                        "cos" => f64::cos,
                        "sqrt" => f64::sqrt,
                        "ln" => f64::ln,
                        "exp" => f64::exp,
                        "abs" => f64::abs,
                        "floor" => f64::floor,
                        "round" => f64::round,
                        _ => return None,
                    };
                    self.ws();
                    if self.s.get(self.at) != Some(&b'(') {
                        return None;
                    }
                    self.at += 1;
                    let v = self.sum()?;
                    self.ws();
                    if self.s.get(self.at) != Some(&b')') {
                        return None;
                    }
                    self.at += 1;
                    Some(f(v))
                }
                _ => None,
            }
        }
    }
    let mut p = P { s: text.as_bytes(), at: 0, env };
    let v = p.sum()?;
    p.ws();
    (p.at == p.s.len() && v.is_finite()).then_some(v)
}

/// set x = EXPR [when ...]: at each tick x takes the value of EXPR (computed by the network).
fn set(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let (body, when) = rest.split_once(" when ").unwrap_or((rest, ""));
    let Some(m) = re_set().captures(body.trim()) else {
        bail!("set NAME = expression [when node in lo .. hi and ...]")
    };
    let (target, expr) = (m[1].to_string(), m[2].trim().to_string());
    if !net.nodes.contains_key(&target) {
        role(net, "var", &target, origin)?;
    }
    let conds = conditions(net, when, origin)?;
    let source = if is_name(&expr) && net.nodes.contains_key(&expr) {
        expr.clone()
    } else {
        let k = net.sets.iter().filter(|s| s.target == target).count() + 1;
        let source = format!("{target}__next{k}");
        let spec = &net.nodes[&target];
        let mut s = NodeSpec::new(spec.guess, spec.signed);
        s.origin = origin.clone();
        net.nodes.insert(source.clone(), s);
        relate(net, &format!("set_{target}_{k}: {source} = {expr}"), origin)?;
        source
    };
    let text = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    net.sets.push(SetRule { target: target.clone(), source, when: conds, text, carry: false });
    Ok(format!(
        "at each tick {target} <- {expr}{}",
        if when.trim().is_empty() { String::new() } else { format!(" when {}", when.trim()) }
    ))
}

/// delay NAME = SOURCE by SECONDS: NAME is what SOURCE was SECONDS ago (a line that remembers). At the speed of
/// sound it is exact to the sample; at the machine's pace it is one tick late. An echo is one delay and one
/// relation that brings it back; a reverb a few of them feeding each other.
fn delay(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let usage = "delay NAME = SOURCE by SECONDS (or by NODE up to SECONDS: as far back as the node says)";
    // `by NODE up to SECONDS`: a delay that follows a node (an echo on the tempo)
    let (rest, follows) = match rest.split_once(" up to ") {
        Some((r, most)) => match r.rsplit_once(" by ") {
            Some((body, node)) if is_name(node.trim()) => {
                (format!("{body} by {}", most.trim()), Some(node.trim().to_string()))
            }
            _ => bail!("{usage}"),
        },
        None => (rest.to_string(), None),
    };
    let rest = rest.as_str();
    let Some((body, by)) = rest.rsplit_once(" by ") else { bail!("{usage}") };
    let Some(m) = re_set().captures(body.trim()) else { bail!("delay NAME = SOURCE by SECONDS") };
    let (target, source) = (m[1].to_string(), m[2].trim().to_string());
    if !is_name(&source) {
        bail!("delay {target} = {source}: what is delayed is a node");
    }
    let seconds =
        seconds_of(by).ok_or_else(|| GrmError::new(format!("delay: {by} is not a time (0.3, 300ms, 0.3s)")))?;
    if !(seconds > 0.0 && seconds <= 10.0) {
        bail!("delay: between 0 and 10 seconds");
    }
    if !net.nodes.contains_key(&source) {
        let mut s = NodeSpec::new(0.0, true);
        s.origin = origin.clone();
        net.nodes.insert(source.clone(), s);
    }
    if !net.nodes.contains_key(&target) {
        role(net, "var", &format!("{target} = 0"), origin)?;
    }
    net.sets.retain(|st| !(st.target == target && st.text.starts_with("delay ")));
    let text = match &follows {
        Some(n) => {
            if !net.nodes.contains_key(n) {
                role(net, "var", &format!("{n} = {}", crate::lang::g(seconds)), origin)?;
            }
            format!("delay {target} = {source} by {n} up to {}", crate::lang::g(seconds))
        }
        None => format!("delay {target} = {source} by {}", crate::lang::g(seconds)),
    };
    net.sets.push(SetRule { target: target.clone(), source: source.clone(), when: vec![], text, carry: false });
    Ok(format!("{target} is {source} as it was {} s before", crate::lang::g(seconds)))
}

/// `audio NAME from FILE [when SWITCH]`: a sound file (WAV) played as a node, its samples one each sample while
/// the switch is on (from where it was left), round again at its end; the file read where the line was written.
fn audio(net: &mut Network, rest: &str, base: &std::path::Path, origin: &Provenance) -> Result<String> {
    let Some((name, after)) = rest.split_once(" from ") else { bail!("audio NAME from FILE [when SWITCH]") };
    let name = name.trim().to_string();
    if !is_name(&name) {
        bail!("audio: {name} is not a name");
    }
    let (file, run) = match after.rsplit_once(" when ") {
        Some((f, r)) => (f.trim(), Some(r.trim().to_string())),
        None => (after.trim(), None),
    };
    let file = file.trim_matches('"');
    let path = if std::path::Path::new(file).is_absolute() { std::path::PathBuf::from(file) } else { base.join(file) };
    if crate::wav::load(&path).is_none() {
        bail!("audio: cannot read {} (a WAV file)", path.display());
    }
    if let Some(r) = &run {
        if !is_name(r) {
            bail!("audio: when {r}: a node");
        }
        if !net.nodes.contains_key(r) {
            role(net, "input", &format!("{r} = 1"), origin)?;
        }
    }
    if !net.nodes.contains_key(&name) {
        role(net, "var", &format!("{name} = 0"), origin)?;
        if let Some(s) = net.nodes.get_mut(&name) {
            s.signed = true;
        }
    }
    net.sets.retain(|st| !(st.target == name && st.text.starts_with("audio ")));
    let text = format!(
        "audio {name} from {}{}",
        path.display(),
        run.as_ref().map(|r| format!(" when {r}")).unwrap_or_default()
    );
    let source = run.clone().unwrap_or_else(|| name.clone());
    net.sets.push(SetRule { target: name.clone(), source, when: vec![], text, carry: false });
    Ok(format!("{name} plays {}", path.display()))
}

/// A time in seconds: "0.3", "0.3s", "300ms".
pub fn seconds_of(t: &str) -> Option<f64> {
    let t = t.trim();
    if let Some(ms) = t.strip_suffix("ms") {
        return ms.trim().parse::<f64>().ok().map(|v| v / 1000.0);
    }
    t.strip_suffix('s').unwrap_or(t).trim().parse::<f64>().ok()
}

/// carry x = y [when ...]: at each tick x takes y's value and how sure of it the network is (a state carried from
/// one tick to the next: its uncertainty goes with it). y is a node.
fn carry(net: &mut Network, rest: &str, origin: &Provenance) -> Result<String> {
    let (body, when) = rest.split_once(" when ").unwrap_or((rest, ""));
    if body.contains(',') {
        return carry_group(net, rest, body, when, origin);
    }
    let Some(m) = re_set().captures(body.trim()) else { bail!("carry NAME = NODE [when node in lo .. hi and ...]") };
    let (target, source) = (m[1].to_string(), m[2].trim().to_string());
    if !is_name(&source) || !net.nodes.contains_key(&source) {
        bail!("carry {target} = {source}: what is carried is a node");
    }
    if !net.nodes.contains_key(&target) {
        role(net, "var", &target, origin)?;
    }
    let conds = conditions(net, when, origin)?;
    let text = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    net.sets.push(SetRule { target: target.clone(), source: source.clone(), when: conds, text, carry: true });
    Ok(format!("at each tick {target} <- {source}, with its uncertainty"))
}

/// The internal value and sigma of a node as said (SI value; sigma in SI for a signed node, in dex for a positive one).
fn internal(spec: &NodeSpec, value: f64, sigma: f64) -> (f64, f64) {
    let shift = spec.dim.as_ref().map(crate::units::unit_log10).unwrap_or(0.0);
    if spec.signed {
        (value / 10f64.powf(shift), sigma / 10f64.powf(shift))
    } else {
        (value.max(f64::MIN_POSITIVE).log10() - shift, sigma)
    }
}

/// observe v = [1, 2] cov [[a, b], [b, c]]: a vector measured together (its components move together), said
/// once as `observe-joint v 1, 2 cov [[...]]`.
fn observe_joint(net: &mut Network, rest: &str) -> Result<String> {
    let (name, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let Some(&d) = net.vectors.get(name) else { bail!("{name} is not a vector") };
    let (vals, cov) = rest.split_once("cov").unwrap_or((rest, ""));
    let values: Vec<f64> = vals.split(',').map(number).collect::<Result<_>>()?;
    let rows: Vec<Vec<f64>> = cov
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(']')
        .map(|r| r.trim_start_matches([',', ' ', '[']).trim())
        .filter(|r| !r.is_empty())
        .map(|r| r.split(',').map(number).collect::<Result<Vec<f64>>>())
        .collect::<Result<_>>()?;
    if values.len() != d || rows.len() != d || rows.iter().any(|r| r.len() != d) {
        bail!("observe {name} = [{d} values] cov [[{d} x {d}]]");
    }
    let names: Vec<String> = (0..d).map(|k| format!("{name}_{k}")).collect();
    if names.iter().any(|n| !net.nodes.get(n).is_some_and(|s| s.signed && s.dim.is_none())) {
        bail!("{name}: a vector measured together is signed, without dimension (for now)");
    }
    let Some(info) = crate::time::inverse_spd(&rows) else {
        bail!("{name}: its covariance must be symmetric and positive")
    };
    let prior = format!("observe {name}");
    net.priors.retain(|p| p.name != prior);
    net.priors.push(Prior { name: prior, names: names.clone(), mean: values.clone(), info });
    for n in &names {
        net.measured.shift_remove(n);
    }
    Ok(format!("observed {name} = [{}] together", values.iter().map(|v| g(*v)).collect::<Vec<_>>().join(", ")))
}

/// carry a, b = c, d [when ...]: the sources' joint belief (their means and how they move together) becomes the
/// targets' prior at each tick: a state of several parts carried as one.
fn carry_group(net: &mut Network, rest: &str, body: &str, when: &str, origin: &Provenance) -> Result<String> {
    let Some((left, right)) = body.split_once('=') else { bail!("carry A, B = C, D [when ...]") };
    let targets: Vec<String> = left.split(',').map(|t| t.trim().to_string()).collect();
    let sources: Vec<String> = right.split(',').map(|t| t.trim().to_string()).collect();
    if targets.len() != sources.len() || targets.iter().chain(&sources).any(|n| !is_name(n)) {
        bail!("carry A, B = C, D: as many targets as sources, each a node");
    }
    for s in &sources {
        if !net.nodes.contains_key(s) {
            bail!("carry: {s} is not a node");
        }
    }
    for t in &targets {
        if !net.nodes.contains_key(t) {
            role(net, "var", t, origin)?;
        }
    }
    let conds = conditions(net, when, origin)?;
    let name = format!("carry {}", targets.join(" "));
    // its first prior: what is said of each target, alone
    let k = targets.len();
    let mut mean = vec![0.0; k];
    let mut info = vec![vec![0.0; k]; k];
    for (i, t) in targets.iter().enumerate() {
        let spec = &net.nodes[t];
        let (v, sg) = net.measured.get(t).copied().unwrap_or((spec.guess, if spec.signed { 1e6 } else { 1e3 }));
        let (u, su) = internal(spec, v, sg);
        mean[i] = u;
        info[i][i] = 1.0 / su.max(1e-12).powi(2);
    }
    net.priors.retain(|p| p.name != name);
    net.priors.push(Prior { name: name.clone(), names: targets.clone(), mean, info });
    net.carries.retain(|c| c.prior != name);
    let text = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    net.carries.push(CarryGroup { prior: name, targets: targets.clone(), sources: sources.clone(), when: conds, text });
    Ok(format!("at each tick {} <- {}, together, with how they move together", targets.join(", "), sources.join(", ")))
}

fn rename(t: &IndexMap<String, f64>, old: &str, new: &str) -> IndexMap<String, f64> {
    t.iter().map(|(k, v)| (if k == old { new.to_string() } else { k.clone() }, *v)).collect()
}

fn swap(n: &mut String, gone: &str, keep: &str) {
    if n == gone {
        *n = keep.to_string();
    }
}

/// fuse A B: B was A under another name (one node, every relation of B now on A). fuse A B inverse: B was A^-1.
pub fn fuse(net: &mut Network, words: &[&str]) -> Result<String> {
    if words.len() == 3 && words[2] == "inverse" {
        return fuse_inverse(net, words[0], words[1]);
    }
    if words.len() != 2 {
        bail!("fuse A B [inverse]: B becomes A (or its inverse)");
    }
    let (keep, gone) = (words[0], words[1]);
    for n in [keep, gone] {
        if !net.nodes.contains_key(n) {
            bail!("no node {n}");
        }
    }
    if keep == gone {
        return Ok(format!("{keep} is already one node"));
    }
    let (ka, kb) = (net.nodes[keep].signed, net.nodes[gone].signed);
    if ka && !kb {
        bail!("fuse {keep} {gone}: keep the positive one ({gone}) and fuse the signed one into it");
    }
    if !ka && kb {
        if let Some(&(v, sg)) = net.measured.get(gone) {
            if v <= 0.0 {
                bail!("fuse {keep} {gone}: {gone} has been {}, {keep} is positive", g(v));
            }
            net.measured.insert(gone.to_string(), (v, (sg / (v * std::f64::consts::LN_10)).max(1e-9)));
        }
    }
    for r in &mut net.relations {
        match &mut r.kind {
            Kind::Product { terms, .. } => {
                let mut t: IndexMap<String, f64> = IndexMap::new();
                for (n, p) in terms.iter() {
                    *t.entry(if n == gone { keep.to_string() } else { n.clone() }).or_insert(0.0) += p;
                }
                t.retain(|_, p| p.abs() > 1e-15);
                *terms = t;
            }
            Kind::Sum { terms } => {
                for (_, t) in terms.iter_mut() {
                    *t = rename(t, gone, keep);
                }
            }
            Kind::Log { log, node } => {
                swap(node, gone, keep);
                swap(log, gone, keep);
            }
            Kind::Keep { node, .. } => swap(node, gone, keep),
        }
        for c in &mut r.when {
            swap(&mut c.node, gone, keep);
        }
    }
    for st in &mut net.sets {
        swap(&mut st.target, gone, keep);
        swap(&mut st.source, gone, keep);
        for c in &mut st.when {
            swap(&mut c.node, gone, keep);
        }
    }
    if let Some((vb, sb)) = net.measured.shift_remove(gone) {
        if let Some(&(va, sa)) = net.measured.get(keep) {
            let signed = net.nodes[keep].signed;
            let (va, vb) = if signed { (va, vb) } else { (va.log10(), vb.log10()) };
            let (w1, w2) = (1.0 / (sa * sa), 1.0 / (sb * sb));
            let v = (w1 * va + w2 * vb) / (w1 + w2);
            net.measured.insert(keep.to_string(), (if signed { v } else { 10f64.powf(v) }, (w1 + w2).powf(-0.5)));
        } else {
            net.measured.insert(keep.to_string(), (vb, sb));
        }
    }
    let g_spec = net.nodes[gone].clone();
    {
        let k = net.nodes.get_mut(keep).unwrap();
        k.var |= g_spec.var;
        k.learn |= g_spec.learn;
        k.input |= g_spec.input;
        k.output |= g_spec.output;
    }
    net.nodes.shift_remove(gone);
    for link in &mut net.links {
        for n in &mut link.names {
            swap(n, gone, keep);
        }
    }
    for (_, b) in net.aliases.iter_mut() {
        swap(b, gone, keep);
    }
    net.aliases.insert(gone.to_string(), keep.to_string());
    Ok(format!("fused: {gone} is {keep} (one node)"))
}

fn fuse_inverse(net: &mut Network, keep: &str, gone: &str) -> Result<String> {
    for n in [keep, gone] {
        if !net.nodes.contains_key(n) {
            bail!("no node {n}");
        }
        if let Some(&(v, _)) = net.measured.get(n) {
            if v <= 0.0 {
                bail!("fuse {keep} {gone} inverse: {n} has been {}: an inverse is positive", g(v));
            }
        }
    }
    for r in &net.relations {
        if matches!(r.kind, Kind::Keep { .. } | Kind::Log { .. }) && r.nodes().iter().any(|n| n == gone) {
            bail!("fuse {keep} {gone} inverse: {gone} is in {}", r.name);
        }
        if r.when.iter().any(|c| c.node == gone) {
            bail!("fuse {keep} {gone} inverse: {gone} is a condition of {}", r.name);
        }
    }
    for st in &net.sets {
        if st.target == gone || st.source == gone || st.when.iter().any(|c| c.node == gone) {
            bail!("fuse {keep} {gone} inverse: {gone} is written at each tick");
        }
    }
    for name in [keep, gone] {
        let measured = net.measured.get(name).copied();
        let spec = net.nodes.get_mut(name).unwrap();
        if spec.signed {
            spec.signed = false;
            if let Some((v, sg)) = measured {
                net.measured.insert(name.to_string(), (v, (sg / (v * std::f64::consts::LN_10)).max(1e-9)));
            }
            if spec.guess <= 0.0 {
                spec.guess = measured.map(|m| m.0).unwrap_or(1.0);
            }
        }
    }
    for r in &mut net.relations {
        match &mut r.kind {
            Kind::Product { terms, .. } => {
                let mut t: IndexMap<String, f64> = IndexMap::new();
                for (n, p) in terms.iter() {
                    let (n, p) = if n == gone { (keep.to_string(), -p) } else { (n.clone(), *p) };
                    *t.entry(n).or_insert(0.0) += p;
                }
                t.retain(|_, p| p.abs() > 1e-15);
                *terms = t;
            }
            Kind::Sum { terms } => {
                for (_, t) in terms.iter_mut() {
                    let mut t2: IndexMap<String, f64> = IndexMap::new();
                    for (n, p) in t.iter() {
                        let (n, p) = if n == gone { (keep.to_string(), -p) } else { (n.clone(), *p) };
                        *t2.entry(n).or_insert(0.0) += p;
                    }
                    t2.retain(|_, p| p.abs() > 1e-15);
                    *t = t2;
                }
            }
            _ => {}
        }
    }
    if let Some((vb, sb)) = net.measured.shift_remove(gone) {
        if let Some(&(va, sa)) = net.measured.get(keep) {
            let (la, lb) = (va.log10(), -vb.log10());
            let (w1, w2) = (1.0 / (sa * sa), 1.0 / (sb * sb));
            net.measured.insert(keep.to_string(), (10f64.powf((w1 * la + w2 * lb) / (w1 + w2)), (w1 + w2).powf(-0.5)));
        } else {
            net.measured.insert(keep.to_string(), (1.0 / vb, sb));
        }
    }
    let gs = net.nodes[gone].clone();
    {
        let k = net.nodes.get_mut(keep).unwrap();
        k.input |= gs.input;
        k.output |= gs.output;
    }
    net.nodes.shift_remove(gone);
    net.inverses.insert(gone.to_string(), keep.to_string());
    for (_, b) in net.inverses.iter_mut() {
        swap(b, gone, keep);
    }
    Ok(format!("fused: {gone} is {keep}^-1 (one node, its inverse included)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn say(net: &mut Network, line: &str) -> String {
        statement(net, line, None, &Provenance::default()).unwrap().unwrap()
    }

    #[test]
    fn numbers_are_read_as_python_reads_them() {
        assert_eq!(number("2 - 0.25").unwrap(), 1.75);
        assert_eq!(number("1e-3").unwrap(), 1e-3);
        assert_eq!(number("pi").unwrap(), std::f64::consts::PI);
        assert!(number("abc").is_err());
        assert_eq!(g(0.001), "0.001");
        assert_eq!(g(1234567.0), "1.23457e+06");
        assert_eq!(g(1e-5), "1e-05");
        assert_eq!(fmt_g(0.0105, 3), "0.0105");
    }

    #[test]
    fn a_sum_is_split_at_its_top_level() {
        let s = split_sum("x0 + v * dt - 3e-5 * y^-2");
        assert_eq!(s, vec![(1.0, "x0".into()), (1.0, "v *dt".into()), (-1.0, "3e-5 *y^-2".into())]);
        assert_eq!(split_sum("-x + 2"), vec![(-1.0, "x".into()), (1.0, "2".into())]);
    }

    #[test]
    fn a_repeat_says_its_line_for_each_k_with_its_arithmetic() {
        assert_eq!(
            sums("relate r: y = sum(j in 1 .. 3: w{j} * s{j}) + 1").unwrap(),
            "relate r: y = w1 * s1 + w2 * s2 + w3 * s3 + 1"
        );
        assert_eq!(
            sums("relate r: y = 1 - 2 * sum(j in 1 .. 2: p{j} * p{j})").unwrap(),
            "relate r: y = 1 - 2 * p1 * p1 - 2 * p2 * p2"
        );
        assert_eq!(
            repeat("k in 1 .. 1: relate r: y = x - {k - 3} * z + {k - 2}").unwrap()[0],
            "relate r: y = x + 2 * z - 1"
        );
        let lines = repeat("k in 1 .. 3: repeat j in 0 .. 1: var w{k}_{j} = {k * 10 + j / 2}").unwrap();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], "var w1_0 = 10");
        assert_eq!(lines[5], "var w3_1 = 30.5");
        // what it cannot compute (a network's parameter) stays as it is
        assert_eq!(repeat("k in 1 .. 1: set {name}_x{k} = {name}_y").unwrap()[0], "set {name}_x1 = {name}_y");
        assert_eq!(arithmetic("2^(1/12) * 110", &[]).map(|v| (v * 1000.0).round()), Some(116541.0));
        let mut net = Network::default();
        say(&mut net, "repeat k in 1 .. 4: var v{k} = {k}");
        assert!(net.nodes.contains_key("v4"));
    }

    #[test]
    fn a_memory_said_again_keeps_what_it_holds() {
        let mut net = Network::default();
        say(&mut net, "memory w = 0.5");
        assert!(net.nodes["w"].memory);
        net.measured.insert("w".into(), (0.9, VAR_SIGMA));
        say(&mut net, "memory w = 0.5");
        assert_eq!(net.measured["w"].0, 0.9);
    }

    #[test]
    fn statements_build_the_network() {
        let mut net = Network::new();
        say(&mut net, "node c = 299792458 fixed");
        say(&mut net, "relate grains: S * H^2 * tP^2 = pi");
        say(&mut net, "observe H = 2.18e-18 ± 1%");
        say(&mut net, "var x = 0");
        say(&mut net, "set x = x + 1");
        let Kind::Product { terms, .. } = &net.relation("grains").unwrap().kind else { panic!() };
        assert_eq!(terms["H"], 2.0);
        assert!(net.nodes.contains_key("x__next1"));
        assert!(matches!(net.relation("set_x_1").unwrap().kind, Kind::Sum { .. }));
        assert!((net.measured["H"].1 - (1.01f64).log10()).abs() < 1e-15);
        assert!(statement(&mut net, "why x", None, &Provenance::default()).unwrap().is_none());
    }

    #[test]
    fn a_network_is_called_and_a_name_fused() {
        let mut net = Network::new();
        say(&mut net, "network plat(id, top)");
        say(&mut net, "keep land_{id}: y >= {top} when x in {top}-0.25 .. 8");
        say(&mut net, "end");
        say(&mut net, "call plat(id=3, top=2)");
        let r = net.relation("land_3").unwrap();
        assert!(matches!(r.kind, Kind::Keep { bound, .. } if bound == 2.0));
        assert_eq!(r.when[0].lo, 1.75);
        say(&mut net, "relate a: p = 2 * q");
        say(&mut net, "relate b: r = 3 * s");
        say(&mut net, "fuse q s");
        assert!(!net.nodes.contains_key("s"));
        say(&mut net, "observe s = 4");
        assert!(net.measured.contains_key("q"));
    }

    #[test]
    fn vectors_are_said_once_and_hold_for_each_component() {
        let mut net = Network::new();
        say(&mut net, "node v[3] signed");
        say(&mut net, "node w[3] signed");
        assert_eq!(net.vectors["v"], 3);
        assert!(net.nodes.contains_key("v_2"));
        say(&mut net, "relate r: w = 2 * v + 1");
        assert!(net.relation("r_0").is_some() && net.relation("r_2").is_some());
        say(&mut net, "observe v = [1, 2, 3] ± 0.001");
        assert_eq!(net.measured["v_1"].0, 2.0);
        say(&mut net, "relate e: energy = dot(v, v)");
        let Kind::Sum { terms } = &net.relation("e").unwrap().kind else { panic!() };
        assert_eq!(terms.len(), 4);
        say(&mut net, "var p[3] = 0");
        say(&mut net, "set p = p + v");
        assert_eq!(net.sets.len(), 3);
        assert!(statement(&mut net, "observe v = [1, 2]", None, &Provenance::default()).is_err());
        assert!(statement(&mut net, "node u[2] signed", None, &Provenance::default()).is_ok());
        assert!(statement(&mut net, "relate bad: u = v", None, &Provenance::default()).is_err());
        say(&mut net, "forget v");
        assert!(!net.measured.contains_key("v_0"));
    }
}
