// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A workshop: a network made apart, and the place where the language is learned by writing it.
//!
//! On the left, the words of the language, by families, each with its sign and a sentence; a click on one opens a
//! small form: what it needs, the line written as it is filled (in colour), what it would do in the network, and
//! what it means in words. In the middle, the network as it grows. Below, the lines written, each said in words,
//! each that can be taken back. A path goes step by step: from a blank page (the questions), or from an example
//! (a falling body, Newton's law, an oscillator that is heard). Three levels: first steps, more, everything.
//! When it is ready, the airlock: what it would change in the main network, and two clear choices.

use std::collections::HashMap;

use eframe::egui::{self, vec2, RichText};

use crate::app::{App, Ink};
use crate::math::Expr;
use crate::text::{t, Lang};

/// What a field of a form asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ask {
    /// A new name.
    Name,
    /// A node already there.
    Node,
    /// A number (it may be left empty when `optional`).
    Number,
    /// An equation (its nodes offered as buttons).
    Equation,
    /// An expression (what a node becomes).
    Expression,
    /// At least or at most.
    Bound,
    /// Yes or no: the word added when yes.
    Flag(&'static str),
}

pub struct Field {
    pub key: &'static str,
    pub ask: Ask,
    pub optional: bool,
    pub label: (&'static str, &'static str),
}

/// A word of the language, as the palette shows it.
pub struct Word {
    pub word: &'static str,
    /// 0 what there is, 1 what links, 2 what is known, 3 time.
    pub family: usize,
    /// 1 first steps, 2 more, 3 everything.
    pub level: u8,
    pub sign: &'static str,
    pub what: (&'static str, &'static str),
    pub example: &'static str,
    pub fields: &'static [Field],
}

const fn f(key: &'static str, ask: Ask, optional: bool, en: &'static str, fr: &'static str) -> Field {
    Field { key, ask, optional, label: (en, fr) }
}

pub const WORDS: &[Word] = &[
    Word {
        word: "node",
        family: 0,
        level: 1,
        sign: "circle-dot",
        what: (
            "a quantity: a mass, a length, a price, a pitch...",
            "une quantité : une masse, une longueur, un prix, une hauteur de note…",
        ),
        example: "node mass = 70",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f("value", Ask::Number, true, "where it starts (empty: unknown)", "d'où il part (vide : inconnu)"),
            f("signed", Ask::Flag("signed"), true, "it can be negative", "il peut être négatif"),
        ],
    },
    Word {
        word: "relate",
        family: 1,
        level: 1,
        sign: "spline",
        what: (
            "an equation between quantities: a law, a definition, a rule",
            "une équation entre des quantités : une loi, une définition, une règle",
        ),
        example: "relate newton: force = mass * acceleration",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f(
                "equation",
                Ask::Equation,
                false,
                "the equation (+ - * / ^, ln, exp, sqrt)",
                "l'équation (+ - * / ^, ln, exp, sqrt)",
            ),
        ],
    },
    Word {
        word: "observe",
        family: 2,
        level: 1,
        sign: "gauge",
        what: ("a measurement: what was seen of a quantity", "une mesure : ce qu'on a vu d'une quantité"),
        example: "observe mass = 70",
        fields: &[
            f("node", Ask::Node, false, "what was measured", "ce qui a été mesuré"),
            f("value", Ask::Number, false, "its value", "sa valeur"),
        ],
    },
    Word {
        word: "ask",
        family: 2,
        level: 1,
        sign: "circle-help",
        what: (
            "a question: what a quantity is worth, how sure, and why",
            "une question : ce que vaut une quantité, avec quelle certitude, et pourquoi",
        ),
        example: "ask force",
        fields: &[f("node", Ask::Node, false, "what is asked", "ce qui est demandé")],
    },
    Word {
        word: "input",
        family: 0,
        level: 2,
        sign: "log-in",
        what: (
            "a quantity that comes from outside: a slider, a sensor, a table",
            "une quantité qui vient de dehors : un curseur, un capteur, un tableau",
        ),
        example: "input pitch = 220 positive",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f("value", Ask::Number, true, "its first value", "sa première valeur"),
            f(
                "positive",
                Ask::Flag("positive"),
                true,
                "never below zero (a quantity)",
                "jamais sous zéro (une grandeur)",
            ),
        ],
    },
    Word {
        word: "output",
        family: 0,
        level: 2,
        sign: "log-out",
        what: ("what is read, shown or heard", "ce qu'on lit, montre ou entend"),
        example: "output force",
        fields: &[f("node", Ask::Node, false, "what is read", "ce qu'on lit")],
    },
    Word {
        word: "keep",
        family: 1,
        level: 2,
        sign: "fence",
        what: (
            "a limit: a quantity kept at least, or at most, at a value",
            "une limite : une quantité gardée au moins, ou au plus, à une valeur",
        ),
        example: "keep floor: height >= 0",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f("node", Ask::Node, false, "what is kept", "ce qui est gardé"),
            f("bound", Ask::Bound, false, "at least or at most", "au moins ou au plus"),
            f("value", Ask::Number, false, "the value", "la valeur"),
        ],
    },
    Word {
        word: "forget",
        family: 2,
        level: 2,
        sign: "eraser",
        what: (
            "a measurement taken back: the quantity is free again",
            "une mesure retirée : la quantité redevient libre",
        ),
        example: "forget mass",
        fields: &[f("node", Ask::Node, false, "what is forgotten", "ce qu'on oublie")],
    },
    Word {
        word: "var",
        family: 3,
        level: 3,
        sign: "variable",
        what: (
            "a state that changes at each step (what `set` writes)",
            "un état qui change à chaque pas (ce que `set` écrit)",
        ),
        example: "var position = 0",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f("value", Ask::Number, false, "where it starts", "d'où il part"),
        ],
    },
    Word {
        word: "learn",
        family: 0,
        level: 3,
        sign: "brain",
        what: ("a weight the machine learns from what it sees", "un poids que la machine apprend de ce qu'elle voit"),
        example: "learn k = 1",
        fields: &[
            f("name", Ask::Name, false, "its name", "son nom"),
            f("value", Ask::Number, false, "where it starts", "d'où il part"),
        ],
    },
    Word {
        word: "set",
        family: 3,
        level: 3,
        sign: "step-forward",
        what: ("at each step, what a state becomes", "à chaque pas, ce que devient un état"),
        example: "set position = position + speed",
        fields: &[
            f("node", Ask::Node, false, "the state", "l'état"),
            f("expression", Ask::Expression, false, "what it becomes", "ce qu'il devient"),
        ],
    },
    Word {
        word: "carry",
        family: 3,
        level: 3,
        sign: "arrow-right-left",
        what: (
            "from step to step, a quantity carried with how sure it is",
            "d'un pas à l'autre, une quantité portée avec sa certitude",
        ),
        example: "carry x = y",
        fields: &[
            f("node", Ask::Node, false, "what is carried", "ce qui est porté"),
            f("from", Ask::Node, false, "from what", "depuis quoi"),
        ],
    },
];

const FAMILIES: [(&str, &str, &str); 4] = [
    ("box", "What there is", "Ce qui existe"),
    ("link", "What links them", "Ce qui les lie"),
    ("eye", "What is known", "Ce qu'on sait"),
    ("timer", "Time", "Le temps"),
];

fn tr(lang: Lang, pair: (&'static str, &'static str)) -> &'static str {
    if lang == Lang::Fr {
        pair.1
    } else {
        pair.0
    }
}

pub fn word(w: &str) -> Option<&'static Word> {
    WORDS.iter().find(|x| x.word == w)
}

/// The line a form makes.
pub fn line_of(w: &Word, v: &HashMap<String, String>) -> String {
    let get = |k: &str| v.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
    let on = |k: &str| v.get(k).is_some_and(|s| s == "yes");
    match w.word {
        "node" | "input" => {
            let mut l = format!("{} {}", w.word, get("name"));
            if !get("value").is_empty() {
                l.push_str(&format!(" = {}", get("value")));
            }
            for fl in
                w.fields.iter().filter_map(|f| if let Ask::Flag(word) = f.ask { Some((f.key, word)) } else { None })
            {
                if on(fl.0) {
                    l.push_str(&format!(" {}", fl.1));
                }
            }
            l
        }
        "relate" => format!("relate {}: {}", get("name"), get("equation")),
        "observe" => format!("observe {} = {}", get("node"), get("value")),
        "ask" => format!("ask {}", get("node")),
        "output" | "forget" => format!("{} {}", w.word, get("node")),
        "keep" => format!(
            "keep {}: {} {} {}",
            get("name"),
            get("node"),
            if get("bound") == "<=" { "<=" } else { ">=" },
            get("value")
        ),
        "var" | "learn" => format!("{} {} = {}", w.word, get("name"), get("value")),
        "set" => format!("set {} = {}", get("node"), get("expression")),
        "carry" => format!("carry {} = {}", get("node"), get("from")),
        _ => String::new(),
    }
}

/// Whether a form says enough to be written.
fn complete(w: &Word, v: &HashMap<String, String>) -> bool {
    w.fields.iter().all(|f| {
        let s = v.get(f.key).map(|s| s.trim().to_string()).unwrap_or_default();
        match f.ask {
            Ask::Flag(_) | Ask::Bound => true,
            _ if f.optional && s.is_empty() => true,
            Ask::Name | Ask::Node => grm_engine::lang::is_name(&s),
            Ask::Number => s.parse::<f64>().is_ok(),
            Ask::Equation | Ask::Expression => crate::math::parse(&s).is_some(),
        }
    })
}

// ---------------------------------------------------------------------- in words

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

// ---------------------------------------------------------------------- paths

/// A step of a path: what is said about it, and the line it writes (empty: a question to think about).
pub struct Step {
    pub say: (&'static str, &'static str),
    pub line: &'static str,
}

pub struct Path {
    pub sign: &'static str,
    pub title: (&'static str, &'static str),
    pub steps: &'static [Step],
}

const fn s(en: &'static str, fr: &'static str, line: &'static str) -> Step {
    Step { say: (en, fr), line }
}

pub const PATHS: &[Path] = &[
    Path {
        sign: "file",
        title: ("A blank page", "Une page blanche"),
        steps: &[
            s(
                "What are the quantities? Each is a node: a name, a value if it is known (Node, on the left).",
                "Quelles sont les quantités ? Chacune est un nœud : un nom, une valeur si on la connaît (Nœud, à gauche).",
                "",
            ),
            s(
                "How are they linked? Each law is a relation: an equation between them (Relate).",
                "Comment sont-elles liées ? Chaque loi est une relation : une équation entre elles (Relier).",
                "",
            ),
            s(
                "What was measured? Each measurement is an observation (Observe): the network settles around it.",
                "Qu'est-ce qui a été mesuré ? Chaque mesure est une observation (Observer) : le réseau s'ajuste autour.",
                "",
            ),
            s(
                "Ask: what is a quantity worth, how sure, and why (Ask). Then, when it is ready: let it into the main network.",
                "Demander : ce que vaut une quantité, avec quelle certitude, et pourquoi (Demander). Puis, quand il est prêt : le faire entrer dans le réseau principal.",
                "",
            ),
        ],
    },
    Path {
        sign: "apple",
        title: ("Newton's law", "La loi de Newton"),
        steps: &[
            s("A mass: a person of 70 kg (a node).", "Une masse : une personne de 70 kg (un nœud).", "node mass = 70"),
            s("The acceleration of gravity, 9.81 m/s².", "L'accélération de la pesanteur, 9,81 m/s².", "node acceleration = 9.81"),
            s("The force: not known yet.", "La force : pas encore connue.", "node force"),
            s(
                "The law that links them: the force is the mass times the acceleration (a relation).",
                "La loi qui les lie : la force est la masse fois l'accélération (une relation).",
                "relate newton: force = mass * acceleration",
            ),
            s(
                "A measurement: the mass was weighed (an observation). The network settles: the force follows.",
                "Une mesure : la masse a été pesée (une observation). Le réseau s'ajuste : la force suit.",
                "observe mass = 70",
            ),
            s(
                "The question: what is the force? About 687 newtons, with how sure the machine is.",
                "La question : que vaut la force ? Environ 687 newtons, avec la certitude de la machine.",
                "ask force",
            ),
        ],
    },
    Path {
        sign: "arrow-down-to-line",
        title: ("A falling body", "La chute d'un corps"),
        steps: &[
            s("Time, in seconds: it comes from outside (an input).", "Le temps, en secondes : il vient de dehors (une entrée).", "input t positive"),
            s("The height fallen, in metres: not known yet.", "La hauteur parcourue, en mètres : pas encore connue.", "node h"),
            s(
                "The law of a fall: the height is 4.9 times the time squared (half of g).",
                "La loi de la chute : la hauteur vaut 4,9 fois le temps au carré (la moitié de g).",
                "relate fall: h = 4.9 * t^2",
            ),
            s("Three seconds have passed.", "Trois secondes ont passé.", "observe t = 3"),
            s(
                "How far has it fallen? About 44 metres. (A table of a real fall, loaded here, finds this law by itself.)",
                "De combien est-il tombé ? Environ 44 mètres. (Un tableau d'une vraie chute, chargé ici, trouve cette loi tout seul.)",
                "ask h",
            ),
        ],
    },
    Path {
        sign: "audio-waveform",
        title: ("An oscillator, heard", "Un oscillateur, qu'on entend"),
        steps: &[
            s("A pitch, in hertz: it comes from outside (a slider, a key).", "Une hauteur, en hertz : elle vient de dehors (un curseur, une touche).", "input pitch = 220 positive"),
            s("How far it turns at each of the 48 000 samples of a second.", "De combien il tourne à chacun des 48 000 échantillons d'une seconde.", "node step = 0.0288"),
            s(
                "The step follows the pitch (2 π divided by 48 000, times the pitch).",
                "Le pas suit la hauteur (2 π divisé par 48 000, fois la hauteur).",
                "relate turn_rate: step = 0.000130899693899575 * pitch",
            ),
            s("Two states that turn: a cosine...", "Deux états qui tournent : un cosinus…", "var lead_c = 1"),
            s("... and a sine (this one is heard).", "… et un sinus (c'est lui qu'on entend).", "var lead_s = 0"),
            s("Where the cosine goes next.", "Où va le cosinus ensuite.", "node lead_new signed"),
            s("It turns a little.", "Il tourne un peu.", "relate lead_turn: lead_new = lead_c - step * lead_s"),
            s("At each step, the cosine becomes where it was going.", "À chaque pas, le cosinus devient ce vers quoi il allait.", "set lead_c = lead_new"),
            s(
                "And the sine follows it (they never drift: its loudness holds for ever).",
                "Et le sinus le suit (ils ne dérivent jamais : son volume tient pour toujours).",
                "set lead_s = lead_s + step * lead_new",
            ),
            s("Listen: the sine, played 48 000 times a second.", "Écouter : le sinus, joué 48 000 fois par seconde.", "play lead_s"),
        ],
    },
];

// ---------------------------------------------------------------------- the window

/// What a workshop's window keeps between frames.
#[derive(Default)]
pub struct State {
    pub level: u8,
    /// The word whose form is open, and what it says so far.
    pub form: Option<(&'static str, HashMap<String, String>)>,
    /// The path followed, and the step it is at.
    pub path: Option<(usize, usize)>,
    /// The answer to the last question asked.
    pub answer: Option<String>,
}

/// Says a line in the workshop (a question is asked of it, a sound played from it). A question gives back its
/// answer.
pub fn write(app: &mut App, ws: &str, line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(n) = line.strip_prefix("ask ") {
        let (n, w) = (n.trim().to_string(), ws.to_string());
        let asked = n.clone();
        let lang = app.settings.lang;
        let answer = app
            .engine
            .query(move |main| {
                main.workshops.get(&w).map(|x| {
                    let (v, s) = x.machine.reading(&n);
                    let signed = x.machine.net.nodes.get(&n).is_some_and(|s| s.signed);
                    let why = x.why_value(&n).unwrap_or_default();
                    (crate::view::reading(v, s, signed), why)
                })
            })
            .flatten();
        let _ = lang;
        let text = answer.map(|(r, why)| format!("{asked} = {r}\n{why}"));
        // asked from outside the window (a menu): it waits there
        if let Some(st) = app.workshop_state.get_mut(ws) {
            st.answer = text.clone();
        }
        return text;
    }
    if let Some(n) = line.strip_prefix("play ") {
        app.say(format!("workshop play {ws} {}", n.trim()));
        return None;
    }
    app.say(format!("workshop say {ws} {line}"));
    None
}

/// The workshop's window: the palette, the network, the form or the step, the lines.
pub fn show(app: &mut App, ui: &mut egui::Ui, ws: &str, view: &mut crate::view::View, small: bool) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    if !app.picture.workshops.iter().any(|w| w.0 == ws) {
        ui.label(RichText::new(t(lang, "no_workshop")).color(ink.muted));
        if crate::icons::button(ui, "hammer", t(lang, "ws_reopen")).clicked() {
            app.say(format!("workshop new {ws}"));
        }
        return;
    }
    // its own picture, pulled now and then
    let name = ws.to_string();
    let entry = app
        .workshop_pulls
        .entry(name.clone())
        .or_insert_with(|| (crate::model::Pull::new(std::time::Duration::from_millis(300)), None));
    let engine = app.engine.clone();
    if let Some(p) = entry.0.poll(&engine, move |w| crate::model::Picture::of_workshop(w, &name)) {
        entry.1 = p;
    }
    let picture = entry.1.clone();
    let written: Vec<String> = picture.as_ref().map(|p| p.written.clone()).unwrap_or_default();
    let mut state = app.workshop_state.remove(ws).unwrap_or_else(|| State { level: 1, ..Default::default() });
    // the bar: its levels, its paths, a table, the sound, the airlock
    ui.horizontal_wrapped(|ui| {
        for (lv, key) in [(1u8, "ws_level1"), (2, "ws_level2"), (3, "ws_level3")] {
            if crate::icons::toggle(ui, ["sprout", "leaf", "trees"][lv as usize - 1], t(lang, key), state.level == lv)
                .on_hover_text(t(lang, "ws_level_d"))
                .clicked()
            {
                state.level = lv;
            }
        }
        ui.separator();
        egui::containers::menu::MenuButton::from_button(egui::Button::image_and_text(
            crate::icons::image("footprints", 15.0, ui.visuals().text_color()),
            t(lang, "ws_paths"),
        ))
        .ui(ui, |ui| {
            ui.set_min_width(260.0);
            for (k, p) in PATHS.iter().enumerate() {
                if crate::menu::item(
                    ui,
                    p.sign,
                    tr(lang, p.title),
                    &format!("{} {}", p.steps.len(), t(lang, "ws_steps")),
                ) {
                    state.path = Some((k, 0));
                    state.form = None;
                    ui.close();
                }
            }
        });
        if crate::icons::button(ui, "sheet", t(lang, "ws_table")).on_hover_text(t(lang, "ws_table_d")).clicked() {
            app.workshop_loading = Some(ws.to_string());
            app.pick(crate::app::Pick::Open);
        }
        let outputs: Vec<String> = picture
            .as_ref()
            .map(|p| p.nodes.iter().filter(|n| n.output || n.name.ends_with("_s")).map(|n| n.name.clone()).collect())
            .unwrap_or_default();
        if !outputs.is_empty() {
            egui::containers::menu::MenuButton::from_button(egui::Button::image_and_text(
                crate::icons::image("play", 15.0, ui.visuals().text_color()),
                t(lang, "play"),
            ))
            .ui(ui, |ui| {
                for o in &outputs {
                    if ui.button(o).clicked() {
                        app.say(format!("workshop play {ws} {o}"));
                        ui.close();
                    }
                }
            });
        }
        let ready = !written.is_empty();
        if ui
            .add_enabled(
                ready,
                egui::Button::image_and_text(
                    crate::icons::image("door-open", 15.0, ink.on_accent()),
                    RichText::new(t(lang, "ws_let_in")).color(ink.on_accent()),
                )
                .fill(ink.accent),
            )
            .on_hover_text(t(lang, "ws_let_in_d"))
            .clicked()
        {
            crate::airlock::open_for_workshop(app, ws);
        }
    });
    ui.separator();
    let avail = ui.available_size();
    let palette_w = if small { 0.0 } else { 190.0_f32.min(avail.x * 0.3) };
    ui.horizontal_top(|ui| {
        if palette_w > 0.0 {
            ui.allocate_ui_with_layout(vec2(palette_w, avail.y), egui::Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().id_salt(("ws-palette", ws)).show(ui, |ui| palette(ui, lang, &mut state));
            });
            ui.separator();
        }
        ui.vertical(|ui| {
            // the form, or the step of the path, or what was asked
            card(app, ui, ws, &mut state, picture.as_ref(), lang);
            // the network as it grows
            let h = (ui.available_height() * 0.55).max(140.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), h), egui::Layout::top_down(egui::Align::Min), |ui| {
                match picture.clone() {
                    Some(p) if !p.nodes.is_empty() => {
                        let master = (p.master.clone(), None);
                        let saved = std::mem::replace(&mut app.picture, std::sync::Arc::new(p));
                        let saved_master = std::mem::replace(&mut app.master, master);
                        std::mem::swap(&mut app.view, view);
                        app.view.passive = small;
                        app.write_into = Some(ws.to_string());
                        crate::view::show(app, ui, None);
                        app.write_into = None;
                        // a form asked from its menu (right button: a new relation) is this workshop's
                        if let Some(asked) = app.workshop_state.remove(ws).and_then(|st| st.form) {
                            state.form = Some(asked);
                            state.path = None;
                        }
                        app.view.passive = false;
                        std::mem::swap(&mut app.view, view);
                        app.picture = saved;
                        app.master = saved_master;
                    }
                    _ => {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            crate::icons::show(ui, "hammer", 28.0, ink.faint);
                            ui.label(RichText::new(t(lang, "ws_empty")).color(ink.muted));
                        });
                    }
                }
            });
            // the node chosen in it, in hand; and in a simulation, what moved
            if let Some(p) = picture.as_ref() {
                let chosen: Vec<crate::model::NodeView> = p
                    .nodes
                    .iter()
                    .filter(|n| app.ws_selected.as_ref().is_some_and(|(w, s)| w == ws && *s == n.name))
                    .take(1)
                    .cloned()
                    .collect();
                for n in chosen {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&n.name).strong().monospace());
                    });
                    crate::insights::control(app, ui, &n, Some(ws));
                }
                crate::insights::differences(app, ui, ws, p);
            }
            // the lines written, each in words, each can be taken back
            ui.separator();
            egui::ScrollArea::vertical().id_salt(("ws-lines", ws)).show(ui, |ui| {
                for (k, l) in written.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(crate::code::highlight_line(&ink, l));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::icons::sign(ui, "undo-2", t(lang, "ws_unsay")).clicked() {
                                app.say(format!("workshop unsay {ws} {k}"));
                            }
                        });
                    });
                    let words = explain(l, lang);
                    if !words.is_empty() {
                        ui.label(RichText::new(words).small().italics().color(ink.muted));
                    }
                }
            });
        });
    });
    app.workshop_state.insert(ws.to_string(), state);
}

fn palette(ui: &mut egui::Ui, lang: Lang, state: &mut State) {
    let ink = Ink::of(ui.ctx());
    for (k, (sign, en, fr)) in FAMILIES.iter().enumerate() {
        let words: Vec<&Word> = WORDS.iter().filter(|w| w.family == k && w.level <= state.level).collect();
        if words.is_empty() {
            continue;
        }
        ui.horizontal(|ui| {
            crate::icons::show(ui, sign, 13.0, ink.muted);
            ui.label(RichText::new(if lang == Lang::Fr { *fr } else { *en }).small().color(ink.muted));
        });
        for w in words {
            let on = state.form.as_ref().is_some_and(|f| f.0 == w.word);
            let r = crate::icons::toggle(ui, w.sign, RichText::new(w.word).monospace(), on).on_hover_text(format!(
                "{}\n\n{}",
                tr(lang, w.what),
                w.example
            ));
            if r.clicked() {
                state.form = Some((w.word, HashMap::new()));
                state.path = None;
            }
        }
        ui.add_space(6.0);
    }
}

/// The card above the network: a form, a step, or an answer.
fn card(
    app: &mut App,
    ui: &mut egui::Ui,
    ws: &str,
    state: &mut State,
    picture: Option<&crate::model::Picture>,
    lang: Lang,
) {
    let ink = Ink::of(ui.ctx());
    let nodes: Vec<String> = picture.map(|p| p.nodes.iter().map(|n| n.name.clone()).collect()).unwrap_or_default();
    let frame = egui::Frame::new().fill(ink.raised).corner_radius(3.0).inner_margin(egui::Margin::symmetric(12, 10));
    if let Some(a) = state.answer.clone() {
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "circle-help", 16.0, ink.accent);
                let mut lines = a.lines();
                ui.label(RichText::new(lines.next().unwrap_or("")).strong().monospace().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                        state.answer = None;
                    }
                });
            });
            let rest: Vec<String> = a.lines().skip(1).map(String::from).collect();
            if !rest.is_empty() {
                crate::style::terminal(ui, &rest);
            }
        });
        ui.add_space(6.0);
    }
    if let Some((p, k)) = state.path {
        let path = &PATHS[p];
        let step = &path.steps[k.min(path.steps.len() - 1)];
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                crate::icons::show(ui, path.sign, 16.0, ink.accent);
                ui.label(RichText::new(tr(lang, path.title)).strong());
                ui.label(RichText::new(format!("{} / {}", k + 1, path.steps.len())).small().color(ink.muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                        state.path = None;
                    }
                });
            });
            // the steps, as dots
            ui.horizontal(|ui| {
                for i in 0..path.steps.len() {
                    let c = if i < k {
                        ink.accent
                    } else if i == k {
                        ink.text
                    } else {
                        ink.faint
                    };
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, c);
                }
            });
            ui.label(RichText::new(tr(lang, step.say)).size(14.0));
            if !step.line.is_empty() {
                crate::style::terminal(ui, &[step.line.to_string()]);
                let words = explain(step.line, lang);
                if !words.is_empty() {
                    ui.label(RichText::new(words).small().italics().color(ink.muted));
                }
            }
            ui.horizontal(|ui| {
                if k > 0 && crate::icons::button(ui, "arrow-left", t(lang, "ws_prev")).clicked() {
                    state.path = Some((p, k - 1));
                }
                let last = k + 1 >= path.steps.len();
                let (sign, key) = if step.line.is_empty() {
                    ("arrow-right", if last { "ws_done" } else { "ws_next" })
                } else {
                    ("pen-line", if last { "ws_write_done" } else { "ws_write_next" })
                };
                let b = egui::Button::image_and_text(
                    crate::icons::image(sign, 15.0, ink.on_accent()),
                    RichText::new(t(lang, key)).color(ink.on_accent()),
                )
                .fill(ink.accent);
                if ui.add(b).clicked() {
                    if !step.line.is_empty() {
                        if let Some(a) = write(app, ws, step.line) {
                            state.answer = Some(a);
                        }
                    }
                    state.path = if last { None } else { Some((p, k + 1)) };
                }
                if step.line.is_empty() {
                    ui.label(RichText::new(t(lang, "ws_blank_hint")).small().color(ink.muted));
                }
            });
        });
        ui.add_space(6.0);
    }
    let Some((w, mut values)) = state.form.clone() else { return };
    let Some(word) = word(w) else { return };
    let mut close = false;
    let mut wrote = false;
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            crate::icons::show(ui, word.sign, 16.0, ink.accent);
            ui.label(RichText::new(word.word).strong().monospace().size(15.0));
            ui.label(RichText::new(tr(lang, word.what)).small().color(ink.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                    close = true;
                }
            });
        });
        egui::Grid::new(("ws-form", ws, w)).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            for f in word.fields {
                ui.label(RichText::new(tr(lang, f.label)).small());
                let v = values.entry(f.key.to_string()).or_default();
                match f.ask {
                    Ask::Flag(_) => {
                        let mut on = v == "yes";
                        if ui.checkbox(&mut on, "").changed() {
                            *v = if on { "yes".into() } else { String::new() };
                        }
                    }
                    Ask::Bound => {
                        ui.horizontal(|ui| {
                            for (op, key) in [(">=", "ws_at_least"), ("<=", "ws_at_most")] {
                                let on = if v.is_empty() { op == ">=" } else { v == op };
                                if crate::style::chosen(ui, on, t(lang, key)).clicked() {
                                    *v = op.to_string();
                                }
                            }
                        });
                    }
                    Ask::Node => {
                        ui.horizontal_wrapped(|ui| {
                            for n in nodes.iter().filter(|n| !n.contains("__")) {
                                if crate::style::chosen(ui, v == n, n).clicked() {
                                    *v = n.clone();
                                }
                            }
                            if nodes.is_empty() {
                                ui.label(RichText::new(t(lang, "ws_no_node")).small().color(ink.muted));
                            }
                        });
                    }
                    Ask::Equation | Ask::Expression => {
                        ui.vertical(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(v)
                                    .margin(crate::style::FIELD)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(260.0)
                                    .hint_text(crate::style::hint(if f.ask == Ask::Equation {
                                        "force = mass * acceleration"
                                    } else {
                                        "x + speed"
                                    })),
                            );
                            // the nodes there are, to put in it
                            ui.horizontal_wrapped(|ui| {
                                for n in nodes.iter().filter(|n| !n.contains("__")) {
                                    if ui.small_button(n).clicked() {
                                        if !v.is_empty() && !v.ends_with(' ') {
                                            v.push(' ');
                                        }
                                        v.push_str(n);
                                    }
                                }
                                for op in ["=", "+", "-", "*", "/", "^", "ln("] {
                                    if ui.small_button(RichText::new(op).monospace()).clicked() {
                                        v.push_str(&format!(" {op} ").replace("ln( ", "ln("));
                                    }
                                }
                            });
                        });
                    }
                    Ask::Name | Ask::Number => {
                        ui.add(
                            egui::TextEdit::singleline(v)
                                .margin(crate::style::FIELD)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(160.0),
                        );
                    }
                }
                ui.end_row();
            }
        });
        // the line it makes, in colour; in mathematics; in words
        let line = line_of(word, &values);
        crate::style::terminal(ui, std::slice::from_ref(&line));
        // its mathematics and its words only once it says something (not a lone ": ." while it is being written)
        let fine = complete(word, &values);
        if fine {
            if let Some(eq) = line.split_once(':').map(|x| x.1).filter(|_| w == "relate" || w == "keep") {
                crate::math::show(ui, eq.trim(), 15.0, lang);
            }
            let words = explain(&line, lang);
            if !words.is_empty() {
                ui.label(RichText::new(words).italics().color(ink.muted));
            }
        }
        ui.horizontal(|ui| {
            let key = if w == "ask" { "ws_ask" } else { "ws_write" };
            let b = egui::Button::image_and_text(
                crate::icons::image("pen-line", 15.0, ink.on_accent()),
                RichText::new(t(lang, key)).color(ink.on_accent()),
            )
            .fill(ink.accent);
            if ui.add_enabled(fine, b).clicked() {
                if let Some(a) = write(app, ws, &line) {
                    state.answer = Some(a);
                }
                wrote = true;
            }
            if !fine {
                ui.label(RichText::new(t(lang, "ws_not_yet")).small().color(ink.muted));
            }
        });
    });
    ui.add_space(6.0);
    state.form = if close {
        None
    } else if wrote {
        // written: the same word again, empty (another node, another relation)
        Some((w, HashMap::new()))
    } else {
        Some((w, values))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_make_lines_and_lines_are_said_in_words() {
        let mut v = HashMap::new();
        v.insert("name".to_string(), "mass".to_string());
        v.insert("value".to_string(), "70".to_string());
        assert_eq!(line_of(word("node").unwrap(), &v), "node mass = 70");
        v.insert("signed".to_string(), "yes".to_string());
        assert_eq!(line_of(word("node").unwrap(), &v), "node mass = 70 signed");
        let mut r = HashMap::new();
        r.insert("name".to_string(), "newton".to_string());
        r.insert("equation".to_string(), "force = mass * acceleration".to_string());
        assert!(complete(word("relate").unwrap(), &r));
        assert_eq!(
            explain("relate newton: force = mass * acceleration", Lang::Fr),
            "newton : force est égal à mass fois acceleration."
        );
        assert_eq!(explain("relate fall: h = 4.9 * t^2", Lang::En), "fall: h is 4.9 times t squared.");
        assert_eq!(explain("node mass = 70", Lang::Fr), "« mass » est une quantité (jamais négative), qui part de 70.");
        assert!(explain("set lead_s = lead_s + step * lead_new", Lang::Fr).starts_with("À chaque pas"));
        r.insert("equation".to_string(), "force = = mass".to_string());
        assert!(!complete(word("relate").unwrap(), &r));
        // every line of every path is understood
        for p in PATHS {
            for s in p.steps.iter().filter(|s| !s.line.is_empty() && !s.line.starts_with("play")) {
                assert!(!explain(s.line, Lang::Fr).is_empty(), "{}", s.line);
            }
        }
    }
}
