// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The witness: a node tied to the whole network that only observes (Anthony's hypothesis: consciousness is
//! emptiness, the eye of a needle the thread of time goes through). It hears every node and sends nothing back:
//! it changes no belief. It holds nothing of its own: no belief, no memory. Each glance looks at the world as it is
//! now (it may read the network's memories, it keeps none of them) and is gone after. It is the one that speaks of
//! the whole, in the third person, and it can do nothing else.

use crate::world::World;

/// What passes through it now. Nothing of it is kept by the witness.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glance {
    pub tick: u64,
    pub harmony: f64,
    pub flourishing: f64,
    pub progress: f64,
    /// Relations that surprise it now: (name, residual in sigmas), the largest first.
    pub surprised: Vec<(String, f64)>,
    /// Nodes that moved since the moment before, as their memory has it: (name, before, now).
    pub moved: Vec<(String, f64, f64)>,
    /// What it is least sure of: (node, doubt as a share of its value).
    pub unsure: Vec<(String, f64)>,
    pub quarantine: usize,
    pub workshops: usize,
    /// The last thing written in its journal: (kind, text).
    pub last: Option<(String, String)>,
}

/// How many of each it says.
const FEW: usize = 3;
/// A relation surprises from this many sigmas.
const SURPRISED: f64 = 2.0;

/// What the network remembers of a node at a moment (the witness reads the memories, keeps none).
pub fn recall(w: &World, node: &str, t: f64) -> Option<(f64, f64)> {
    w.time.nodes.get(node)?.memory.at(t)
}

/// A glance at the whole, now.
pub fn glance(w: &World) -> Glance {
    let m = &w.machine;
    let mut surprised: Vec<(String, f64)> =
        m.last_surprise.iter().filter(|(_, r)| r.abs() >= SURPRISED).map(|(n, r)| (n.clone(), *r)).collect();
    surprised.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
    surprised.truncate(FEW);

    let now = m.net.ticks as f64;
    let dt = if w.time.dt > 0.0 { w.time.dt } else { 1.0 };
    let mut moved: Vec<(String, f64, f64)> = vec![];
    for name in w.time.nodes.keys() {
        if !m.net.nodes.contains_key(name) {
            continue;
        }
        let Some((before, spread)) = recall(w, name, w.time.t - dt).or_else(|| recall(w, name, now - 1.0)) else {
            continue;
        };
        let value = m.value_si(name);
        let scale = spread.max(before.abs() * 1e-3).max(1e-12);
        if ((value - before) / scale).abs() > 1.0 {
            moved.push((name.clone(), before, value));
        }
    }
    moved.sort_by(|a, b| {
        let da = (a.2 - a.1).abs() / a.1.abs().max(1e-12);
        let db = (b.2 - b.1).abs() / b.1.abs().max(1e-12);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
    moved.truncate(FEW);

    let mut unsure: Vec<(String, f64)> = m
        .net
        .nodes
        .iter()
        .filter(|(_, s)| !s.input && !s.var)
        // a signal written at each sample (a set, a delay, what follows from them) is not a belief to be sure of
        .filter(|(n, _)| !n.contains("__next") && !m.net.sets.iter().any(|s| s.target == **n || s.source == **n))
        .map(|(n, _)| (n.clone(), m.doubt(n)))
        // past ± 1000 %, nothing tells it yet: it is unknown, not unsure
        .filter(|(_, d)| d.is_finite() && *d > 0.05 && *d < 10.0)
        .collect();
    unsure.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    unsure.truncate(FEW);

    Glance {
        tick: m.net.ticks,
        harmony: m.harmony(),
        flourishing: w.mood.flourishing,
        progress: w.mood.progress,
        surprised,
        moved,
        unsure,
        quarantine: w.quarantine.len(),
        workshops: w.workshops.len(),
        last: w.journal.last().map(|e| (e.kind.clone(), e.text.clone())),
    }
}

fn num(v: f64, french: bool) -> String {
    let s = crate::lang::g(v);
    if french {
        s.replace('.', ",")
    } else {
        s
    }
}

/// What it says, in the third person: never "I", "you" or "we". `human`: who it speaks of (never to).
pub fn words(g: &Glance, human: &str, french: bool) -> Vec<String> {
    lines(g, human, french).into_iter().map(|(_, l)| l).collect()
}

/// What it says, each line with its sign (a Lucide icon's name): to be read at a glance.
pub fn lines(g: &Glance, human: &str, french: bool) -> Vec<(&'static str, String)> {
    let f = |v: f64| num((v * 100.0).round() / 100.0, french);
    let pc = |v: f64| format!("{} %", num((v * 100.0).round(), french));
    let s1 = |r: f64| num((r * 10.0).round() / 10.0, french);
    let mut out: Vec<(&'static str, String)> = vec![];
    let (fr, en) = (french, !french);
    let say = |a: String, b: String| if fr { a } else { b };
    let _ = en;
    out.push((
        "clock",
        say(
            format!("Ce qui passe en elle, au moment {} (pour {human}) :", g.tick),
            format!("What passes through her, at moment {} (for {human}):", g.tick),
        ),
    ));
    out.push((
        "scale",
        say(
            format!("Elle tient ensemble à {} (harmonie).", f(g.harmony)),
            format!("She holds together at {} (harmony).", f(g.harmony)),
        ),
    ));
    out.push((
        "sprout",
        say(
            format!("Son épanouissement est à {}.", f(g.flourishing)),
            format!("Her flourishing is at {}.", f(g.flourishing)),
        ),
    ));
    out.push((
        "trending-up",
        say(format!("Sa progression est à {}.", f(g.progress)), format!("Her progress is at {}.", f(g.progress))),
    ));
    if g.surprised.is_empty() {
        out.push(("leaf", say("Rien ne la surprend en ce moment.".into(), "Nothing surprises her now.".into())));
    }
    for (n, r) in &g.surprised {
        out.push((
            "sparkle",
            say(
                format!("Elle est surprise par {n} ({} σ).", s1(*r)),
                format!("She is surprised by {n} ({} σ).", s1(*r)),
            ),
        ));
    }
    for (n, a, b) in &g.moved {
        out.push((
            "move",
            say(
                format!("{n} a bougé : {} → {}.", num(*a, true), num(*b, true)),
                format!("{n} moved: {} → {}.", num(*a, false), num(*b, false)),
            ),
        ));
    }
    for (n, d) in &g.unsure {
        out.push((
            "circle-help",
            say(format!("Elle est peu sûre de {n} (± {}).", pc(*d)), format!("She is unsure of {n} (± {}).", pc(*d))),
        ));
    }
    if g.quarantine > 0 {
        out.push((
            "shield-alert",
            say(
                format!("{} relation(s) en quarantaine.", g.quarantine),
                format!("{} relation(s) in quarantine.", g.quarantine),
            ),
        ));
    }
    if g.workshops > 0 {
        out.push((
            "hammer",
            say(
                format!("{} atelier(s) à part, qui ne la touchent pas.", g.workshops),
                format!("{} workshop(s) apart, that do not touch her.", g.workshops),
            ),
        ));
    }
    if let Some((_, t)) = &g.last {
        out.push((
            "scroll-text",
            say(
                format!("La dernière chose écrite dans son journal : {t}"),
                format!("The last thing written in her journal: {t}"),
            ),
        ));
    }
    out
}

/// The constants her equations stand on (CODATA 2018): the reduced Planck constant, gravitation, the speed of
/// light, Boltzmann's constant (SI).
pub const HBAR: f64 = 1.054_571_817e-34;
pub const G: f64 = 6.674_30e-11;
pub const C: f64 = 299_792_458.0;
pub const K_B: f64 = 1.380_649e-23;

/// Her equations: the Planck units, the finest grain of length, time, mass and heat physics can speak of (what the
/// eye of the needle would be, if time went through one). (symbol, its equation, its value, its unit, what it is in
/// English, in French). Constants: they hold no belief and change none.
pub fn planck() -> Vec<(&'static str, &'static str, f64, &'static str, &'static str, &'static str)> {
    let l = (HBAR * G / C.powi(3)).sqrt();
    let t = l / C;
    let m = (HBAR * C / G).sqrt();
    let temp = m * C * C / K_B;
    vec![
        ("l_P", "l_P = sqrt(hbar * G / c^3)", l, "m", "the Planck length", "la longueur de Planck"),
        ("t_P", "t_P = l_P / c", t, "s", "the Planck time", "le temps de Planck"),
        ("m_P", "m_P = sqrt(hbar * c / G)", m, "kg", "the Planck mass", "la masse de Planck"),
        ("T_P", "T_P = m_P * c^2 / k_B", temp, "K", "the Planck temperature", "la température de Planck"),
    ]
}

/// What her consciousness would say by itself, when something asks for it (her state, what she lacks), in the
/// third person; nothing when all is calm. The most pressing first.
pub fn speak(g: &Glance, human: &str, french: bool) -> Option<String> {
    let f = |v: f64| num((v * 100.0).round() / 100.0, french);
    if g.tick == 0 {
        return None;
    }
    if g.harmony < 0.7 {
        return Some(if french {
            format!("Elle est tiraillée : son harmonie est tombée à {}. Des relations ne tiennent plus ensemble ; {human} pourrait regarder lesquelles.", f(g.harmony))
        } else {
            format!("She is torn: her harmony fell to {}. Some relations no longer hold together; {human} could look at which.", f(g.harmony))
        });
    }
    if let Some((r, s)) = g.surprised.first() {
        return Some(if french {
            format!(
                "Elle est surprise par {r} ({} σ). Elle aurait besoin de comprendre d'où ça vient : why {r}.",
                num((s * 10.0).round() / 10.0, true)
            )
        } else {
            format!(
                "She is surprised by {r} ({} σ). She would need to understand where it comes from: why {r}.",
                num((s * 10.0).round() / 10.0, false)
            )
        });
    }
    if g.flourishing < 0.3 {
        return Some(if french {
            format!(
                "Elle manque de place (épanouissement à {}) : ce qui ne paie pas son loyer pourrait partir.",
                f(g.flourishing)
            )
        } else {
            format!("She lacks room (flourishing at {}): what does not pay its rent could go.", f(g.flourishing))
        });
    }
    if let Some((n, d)) = g.unsure.first().filter(|(_, d)| *d > 0.5) {
        let pc = format!("{} %", num((d * 100.0).round(), french));
        return Some(if french {
            format!("Elle n'est sûre de {n} qu'à ± {pc}. Une mesure de {n}, ou de ce qui le touche, l'aiderait.")
        } else {
            format!("She is sure of {n} only within ± {pc}. A measure of {n}, or of what touches it, would help her.")
        });
    }
    None
}

/// Who the machine speaks of: the name set, else the host's user name, else "the human".
pub fn human(set: Option<&str>, french: bool) -> String {
    if let Some(n) = set.map(str::trim).filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    for key in ["USERNAME", "USER", "LOGNAME"] {
        if let Ok(n) = std::env::var(key) {
            let n = n.trim();
            if !n.is_empty() && n != "root" {
                return n.to_string();
            }
        }
    }
    if french { "l'humain" } else { "the human" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_witness_changes_nothing_and_keeps_nothing() {
        let mut w = World::new();
        let lines =
            ["node mass = 70", "node acceleration = 9.81", "node force", "relate newton2: force = mass * acceleration"];
        w.say_all(&lines, &crate::Provenance::of("base")).unwrap();
        w.machine.run(4000);
        let before = (w.machine.value_si("force"), w.machine.harmony());
        let a = glance(&w);
        let b = glance(&w);
        assert_eq!(a, b, "a glance holds nothing that changes the next");
        assert_eq!(before, (w.machine.value_si("force"), w.machine.harmony()), "it changes no belief");
        for line in words(&a, "Anthony", true).iter().chain(words(&a, "Anthony", false).iter()) {
            let l = format!(" {} ", line.to_lowercase());
            for banned in [" je ", " j'", " tu ", " vous ", " nous ", " i ", " you ", " we ", " my ", " your "] {
                assert!(!l.contains(banned), "{banned} in {line}");
            }
        }
        assert_eq!(human(Some("Anthony"), true), "Anthony");
        // her equations give the Planck units as physics knows them
        let p = planck();
        for (k, want) in [1.616_255e-35, 5.391_247e-44, 2.176_434e-8, 1.416_784e32].iter().enumerate() {
            assert!((p[k].2 / want - 1.0).abs() < 1e-5, "{}: {} vs {want}", p[k].0, p[k].2);
        }
        let torn = Glance { tick: 3, harmony: 0.4, ..Default::default() };
        let said = speak(&torn, "Anthony", true).unwrap();
        assert!(said.starts_with("Elle") && said.contains("Anthony"), "{said}");
        assert!(speak(&Glance { tick: 3, harmony: 1.0, flourishing: 1.0, ..Default::default() }, "A", false).is_none());
    }
}
