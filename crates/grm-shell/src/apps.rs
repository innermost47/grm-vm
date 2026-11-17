// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Apps: a network and its surface in one file of the language. What an app file holds:
//!
//! ```text
//! app Oscillator bench                         its title
//! icon audio-waveform                          its sign
//! use osc.grm                                  networks written elsewhere (`network NAME(p) ... end`)
//! call osc(name=lead, step=0.0576)             a network called, its names made from what it is given
//! node volume = 0.5                            the language, as everywhere
//! widget vol: slider volume 0 .. 1 at 0 0 size 4 1     its surface: what the human sees and touches
//! api weather: get https://... every 600       an API that lives (see grm-connect)
//! read weather current.temperature_2m -> temp ± 0.5
//! play lead_s 0.4                              a command, run when the app opens
//! ```
//!
//! Here the file is read: `use` and `call` are unfolded into plain statements (so every name the network holds is
//! written somewhere a human can read), the surface, the APIs and the commands are set apart, and an app's names
//! can be given a prefix (an app opened while another running one already holds the same network: it runs its own
//! copy).

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use grm_connect::api::{parse_api, parse_read, Api};
use grm_engine::lang::is_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetKind {
    /// Sets a node by dragging (an input).
    Slider,
    /// 0 or 1.
    Toggle,
    /// 1 while held, 0 otherwise (an event).
    Button,
    /// A node's value as a bar, with its spread.
    Meter,
    /// A node's value ± sigma, as a number.
    Value,
    /// A node through time.
    Plot,
    /// Words.
    Text,
    /// What an eye saw.
    Picture,
    /// Two nodes set at once by a point on a square.
    Pad,
    /// A keyboard of notes: the frequency of the key held, and 1 while it is held.
    Keys,
    /// Plays a node (and stops it).
    Play,
    /// What a node that plays sounds like, as a wave.
    Scope,
    /// A round knob (a potentiometer): turned by dragging up and down.
    Knob,
    /// A slider standing up (a fader).
    Fader,
    /// A row of lights, one lit: which step a node is on (lo .. hi, a light each).
    Steps,
    /// An on/off switch with its light (a node at 0 or 1).
    Switch,
    /// A band with a title, framing the widgets under it (a section of a panel).
    Section,
    /// What the machine plays by ear, as a piano roll (the last seconds, the bass and the melody).
    Roll,
    /// A step of a sequencer: a fader for its pitch, its note's name above, lit when the sequencer is on it
    /// (nodes: the pitch, the step the sequencer is on; its words: its number, from 1).
    Step,
    /// Records what is played and turned (notes, knobs, by hand or by her), to play it again and to keep as MIDI.
    Rec,
}

impl WidgetKind {
    pub fn of(word: &str) -> Option<WidgetKind> {
        Some(match word {
            "slider" => WidgetKind::Slider,
            "toggle" => WidgetKind::Toggle,
            "button" => WidgetKind::Button,
            "meter" => WidgetKind::Meter,
            "value" => WidgetKind::Value,
            "plot" => WidgetKind::Plot,
            "text" => WidgetKind::Text,
            "picture" => WidgetKind::Picture,
            "pad" => WidgetKind::Pad,
            "keys" => WidgetKind::Keys,
            "play" => WidgetKind::Play,
            "scope" => WidgetKind::Scope,
            "knob" => WidgetKind::Knob,
            "fader" | "vslider" => WidgetKind::Fader,
            "steps" | "leds" => WidgetKind::Steps,
            "switch" => WidgetKind::Switch,
            "section" => WidgetKind::Section,
            "step" => WidgetKind::Step,
            "roll" => WidgetKind::Roll,
            "rec" | "record" => WidgetKind::Rec,
            _ => return None,
        })
    }

    pub fn word(self) -> &'static str {
        match self {
            WidgetKind::Slider => "slider",
            WidgetKind::Toggle => "toggle",
            WidgetKind::Button => "button",
            WidgetKind::Meter => "meter",
            WidgetKind::Value => "value",
            WidgetKind::Plot => "plot",
            WidgetKind::Text => "text",
            WidgetKind::Picture => "picture",
            WidgetKind::Pad => "pad",
            WidgetKind::Keys => "keys",
            WidgetKind::Play => "play",
            WidgetKind::Scope => "scope",
            WidgetKind::Knob => "knob",
            WidgetKind::Fader => "fader",
            WidgetKind::Steps => "steps",
            WidgetKind::Switch => "switch",
            WidgetKind::Section => "section",
            WidgetKind::Step => "step",
            WidgetKind::Roll => "roll",
            WidgetKind::Rec => "rec",
        }
    }

    /// How many nodes it is bound to.
    pub fn nodes(self) -> usize {
        match self {
            WidgetKind::Text | WidgetKind::Section | WidgetKind::Roll | WidgetKind::Rec => 0,
            WidgetKind::Pad | WidgetKind::Keys | WidgetKind::Step => 2,
            _ => 1,
        }
    }

    /// Its size when none is given (columns, rows of the surface's grid).
    pub fn size(self) -> (i32, i32) {
        match self {
            WidgetKind::Slider => (4, 1),
            WidgetKind::Toggle | WidgetKind::Button | WidgetKind::Play | WidgetKind::Rec => (2, 1),
            WidgetKind::Meter => (3, 1),
            WidgetKind::Value => (3, 1),
            WidgetKind::Plot => (6, 3),
            WidgetKind::Text => (6, 1),
            WidgetKind::Picture => (4, 4),
            WidgetKind::Pad => (3, 3),
            WidgetKind::Keys => (8, 2),
            WidgetKind::Scope => (6, 3),
            WidgetKind::Knob => (2, 2),
            WidgetKind::Fader => (1, 3),
            WidgetKind::Steps => (8, 1),
            WidgetKind::Switch => (2, 1),
            WidgetKind::Section => (6, 3),
            WidgetKind::Step => (1, 4),
            WidgetKind::Roll => (12, 3),
        }
    }

    pub const ALL: [WidgetKind; 20] = [
        WidgetKind::Slider,
        WidgetKind::Toggle,
        WidgetKind::Button,
        WidgetKind::Meter,
        WidgetKind::Value,
        WidgetKind::Plot,
        WidgetKind::Text,
        WidgetKind::Picture,
        WidgetKind::Pad,
        WidgetKind::Keys,
        WidgetKind::Play,
        WidgetKind::Scope,
        WidgetKind::Knob,
        WidgetKind::Fader,
        WidgetKind::Steps,
        WidgetKind::Switch,
        WidgetKind::Section,
        WidgetKind::Step,
        WidgetKind::Roll,
        WidgetKind::Rec,
    ];
}

/// The columns of a surface (its width is shared between them, whatever the window).
pub const COLUMNS: i32 = 12;

/// A thing on the surface, bound to nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    pub name: String,
    pub kind: WidgetKind,
    pub nodes: Vec<String>,
    pub lo: f64,
    pub hi: f64,
    /// (column, row) of its top left corner, and its size in columns and rows (placed where there is room when
    /// not said).
    pub at: (i32, i32),
    pub size: (i32, i32),
    pub placed: bool,
    /// Its words (a text, or a label).
    pub text: String,
    /// The line of the app's file it is written on (1 = the first).
    pub line: usize,
    /// Its look (None: the app's).
    #[serde(default)]
    pub look: Option<String>,
    /// `icon NAME`: a sign beside its words (a Lucide icon's name).
    #[serde(default)]
    pub icon: Option<String>,
}

impl Widget {
    /// The line that says it (as it would be written in the file).
    pub fn statement(&self) -> String {
        let mut s = format!("widget {}: {}", self.name, self.kind.word());
        for n in &self.nodes {
            s.push(' ');
            s.push_str(n);
        }
        if matches!(
            self.kind,
            WidgetKind::Slider
                | WidgetKind::Meter
                | WidgetKind::Plot
                | WidgetKind::Pad
                | WidgetKind::Scope
                | WidgetKind::Knob
                | WidgetKind::Fader
                | WidgetKind::Steps
                | WidgetKind::Step
        ) {
            s.push_str(&format!(" {} .. {}", grm_engine::lang::g(self.lo), grm_engine::lang::g(self.hi)));
        }
        s.push_str(&format!(" at {} {} size {} {}", self.at.0, self.at.1, self.size.0, self.size.1));
        if !self.text.is_empty() {
            s.push_str(&format!(" \"{}\"", self.text));
        }
        if let Some(l) = &self.look {
            s.push_str(&format!(" look {l}"));
        }
        if let Some(i) = &self.icon {
            s.push_str(&format!(" icon {i}"));
        }
        s
    }
}

/// `widget NAME: KIND [NODES] [LO .. HI] [at C R] [size W H] ["words"]`
pub fn parse_widget(rest: &str, line: usize) -> Result<Widget, String> {
    let usage = "widget NAME: KIND [NODE..] [LO .. HI] [at COLUMN ROW] [size COLUMNS ROWS] [\"words\"]";
    let Some((name, body)) = rest.split_once(':') else { return Err(usage.into()) };
    let name = name.trim().to_string();
    if !is_name(&name) {
        return Err(format!("{name} is not a name"));
    }
    // its words, between quotes, may stand anywhere on the line
    let (body, text) = match body.find('"') {
        Some(k) => {
            let after = &body[k + 1..];
            let end = after.find('"').unwrap_or(after.len());
            (format!("{} {}", &body[..k], after.get(end + 1..).unwrap_or("")), after[..end].to_string())
        }
        None => (body.to_string(), String::new()),
    };
    let words: Vec<&str> = body.split_whitespace().collect();
    let Some(kind) = words.first().and_then(|w| WidgetKind::of(w)) else {
        return Err(format!(
            "a widget is one of: {}",
            WidgetKind::ALL.iter().map(|k| k.word()).collect::<Vec<_>>().join(", ")
        ));
    };
    let mut nodes = vec![];
    let (mut lo, mut hi) = (0.0, 1.0);
    let mut at = None;
    let mut size = None;
    let mut look = None;
    let mut icon = None;
    let num = |s: Option<&&str>| -> Result<f64, String> {
        s.ok_or_else(|| usage.to_string())?.parse::<f64>().map_err(|_| format!("{} is not a number", s.unwrap()))
    };
    let mut k = 1;
    while k < words.len() {
        match words[k] {
            "at" => {
                at = Some((num(words.get(k + 1))? as i32, num(words.get(k + 2))? as i32));
                k += 3;
            }
            "size" => {
                size = Some(((num(words.get(k + 1))? as i32).max(1), (num(words.get(k + 2))? as i32).max(1)));
                k += 3;
            }
            "look" => {
                look = words.get(k + 1).map(|w| w.to_string());
                k += 2;
            }
            "icon" => {
                icon = words.get(k + 1).map(|w| w.to_string());
                k += 2;
            }
            w if words.get(k + 1) == Some(&"..") => {
                lo = w.parse().map_err(|_| format!("{w} is not a number"))?;
                hi = num(words.get(k + 2))?;
                k += 3;
            }
            w if is_name(w) => {
                nodes.push(w.to_string());
                k += 1;
            }
            w => return Err(format!("{w}: {usage}")),
        }
    }
    if nodes.len() < kind.nodes() {
        return Err(format!("a {} is bound to {} node(s)", kind.word(), kind.nodes()));
    }
    if kind == WidgetKind::Keys && lo == 0.0 && hi == 1.0 {
        // the notes it spans (MIDI numbers): two octaves from C3
        lo = 48.0;
        hi = 72.0;
    }
    Ok(Widget {
        name,
        kind,
        nodes,
        lo,
        hi,
        at: at.unwrap_or((0, 0)),
        size: size.unwrap_or(kind.size()),
        placed: at.is_some(),
        text,
        line,
        look,
        icon,
    })
}

/// A plain statement of the language, with where it was written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flat {
    pub text: String,
    /// The line of the app's file it comes from (1 = the first).
    pub line: usize,
    /// When it was unfolded from elsewhere: the network called, or the file used, and its line there.
    pub from: Option<String>,
}

/// `hand NAME when SWITCH: NODE NODE ...`: a part of an instrument she may take in hand. While SWITCH is 1, those
/// nodes are hers to write (and only hers: the human sees them move); the switch itself stays the human's. What she
/// writes goes through hands only: she is not the instrument, she plays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hand {
    pub name: String,
    pub switch: String,
    pub nodes: Vec<String>,
    pub line: usize,
}

/// `hand NAME when SWITCH: NODE ...`
pub fn parse_hand(rest: &str, line: usize) -> Result<Hand, String> {
    let usage = "hand NAME when SWITCH: NODE NODE ...";
    let (head, nodes) = rest.split_once(':').ok_or(usage)?;
    let mut words = head.split_whitespace();
    let (Some(name), Some("when"), Some(switch)) = (words.next(), words.next(), words.next()) else {
        return Err(usage.into());
    };
    let nodes: Vec<String> = nodes.split_whitespace().map(|s| s.to_string()).collect();
    if !is_name(name) || !is_name(switch) || nodes.iter().any(|n| !is_name(n)) {
        return Err(usage.into());
    }
    Ok(Hand { name: name.to_string(), switch: switch.to_string(), nodes, line })
}

/// An app file, read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    /// The parts she may take in hand.
    pub hands: Vec<Hand>,
    pub title: Option<String>,
    /// `icon NAME`: its sign (a Lucide icon's name).
    pub icon: Option<String>,
    /// `clock S`: the world ticks every S seconds while it runs.
    pub clock: Option<f64>,
    pub flat: Vec<Flat>,
    pub widgets: Vec<Widget>,
    pub apis: Vec<Api>,
    /// Commands to run when it opens (or when they are written), with their line.
    pub commands: Vec<(usize, String)>,
    /// Lines it could not read: (line, why).
    pub errors: Vec<(usize, String)>,
    /// Its looks and the lines of its style network (`look ...`, `style ...`), with their line.
    pub looks: Vec<(usize, String)>,
}

pub const COMMANDS: &[&str] =
    &["track", "play", "sound", "voice", "volume", "see", "embody", "tick", "mute", "solo", "gain", "pan"];

type Templates = IndexMap<String, (Vec<String>, Vec<String>)>;

/// Where a comment begins: a `#` that is not a colour (`#7fb8f0`).
pub fn comment_at(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    (0..b.len()).find(|&i| {
        if b[i] != b'#' {
            return false;
        }
        let hex = b.get(i + 1..i + 7).is_some_and(|h| h.iter().all(|c| c.is_ascii_hexdigit()));
        let ends = b.get(i + 7).is_none_or(|c| !c.is_ascii_alphanumeric());
        !(hex && ends)
    })
}

fn strip(line: &str) -> &str {
    // a comment begins with # (a colour is not one; inside a URL it would be: a URL with # is written without it)
    match comment_at(line) {
        Some(i) => line[..i].trim(),
        None => line.trim(),
    }
}

/// Reads an app's text. `base`: the folder its `use`s read from.
pub fn parse(text: &str, base: &Path) -> Parsed {
    let mut p = Parsed::default();
    let mut templates: Templates = IndexMap::new();
    let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    unfold(&lines, base, &mut templates, &mut p, None, 0);
    // a read goes to its api
    let mut reads = vec![];
    p.flat.retain(|f| {
        if let Some(rest) = f.text.strip_prefix("read ") {
            reads.push((f.line, rest.to_string()));
            false
        } else {
            true
        }
    });
    for (line, rest) in reads {
        match parse_read(&rest) {
            Ok((api, r)) => match p.apis.iter_mut().find(|a| a.name == api) {
                Some(a) => a.reads.push(r),
                None => p.errors.push((line, format!("no api {api} (say it before its reads)"))),
            },
            Err(e) => p.errors.push((line, e.message)),
        }
    }
    place(&mut p.widgets);
    p
}

/// The lines, `use`, `network ... end` and `call` unfolded. `outer`: the line of the app's own file everything
/// here comes from (None: these are the app's own lines).
fn unfold(
    lines: &[String],
    base: &Path,
    templates: &mut Templates,
    p: &mut Parsed,
    outer: Option<(usize, String)>,
    depth: usize,
) {
    if depth > 16 {
        if let Some((l, _)) = &outer {
            p.errors.push((*l, "too deep: a network that calls itself?".into()));
        }
        return;
    }
    let mut recording: Option<(String, Vec<String>, Vec<String>)> = None;
    for (k, raw) in lines.iter().enumerate() {
        let line_no = outer.as_ref().map(|o| o.0).unwrap_or(k + 1);
        let from = outer.as_ref().map(|o| format!("{} line {}", o.1, k + 1));
        let text = strip(raw);
        if let Some((name, params, mut body)) = recording.take() {
            if text.eq_ignore_ascii_case("end") {
                templates.insert(name, (params, body));
            } else {
                if !text.is_empty() {
                    body.push(text.to_string());
                }
                recording = Some((name, params, body));
            }
            continue;
        }
        if text.is_empty() {
            continue;
        }
        let (word, rest) = text.split_once(char::is_whitespace).map(|(a, b)| (a, b.trim())).unwrap_or((text, ""));
        match word.to_lowercase().as_str() {
            "app" if outer.is_none() => p.title = Some(rest.to_string()),
            "icon" if outer.is_none() => match rest.split_whitespace().next() {
                Some(i) => p.icon = Some(i.to_lowercase()),
                None => p.errors.push((line_no, "icon NAME (a sign's name: play, music, cloud-sun...)".into())),
            },
            "clock" if outer.is_none() => match rest.trim().parse::<f64>() {
                Ok(s) if s >= 0.01 => p.clock = Some(s),
                _ => p.errors.push((line_no, "clock SECONDS (the world ticks every so often while it runs)".into())),
            },
            "network" | "net" => {
                let (name, params) = match rest.split_once('(') {
                    Some((n, ps)) => (
                        n.trim().to_string(),
                        ps.trim_end_matches(')')
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    ),
                    None => (rest.trim().to_string(), vec![]),
                };
                if !is_name(&name) {
                    p.errors.push((line_no, format!("network NAME(param, ...) ... end: {name} is not a name")));
                    continue;
                }
                recording = Some((name, params, vec![]));
            }
            "call" | "cl" => {
                let Some((name, args)) = rest.split_once('(') else {
                    p.errors.push((line_no, "call NAME(param=value, ...)".into()));
                    continue;
                };
                let name = name.trim();
                let Some((params, body)) = templates.get(name).cloned() else {
                    p.errors.push((line_no, format!("no network {name} (written or used before it is called)")));
                    continue;
                };
                let mut given: IndexMap<String, String> = IndexMap::new();
                for kv in args.trim_end_matches(')').split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        given.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
                let missing: Vec<&String> = params.iter().filter(|q| !given.contains_key(*q)).collect();
                if !missing.is_empty() {
                    p.errors.push((
                        line_no,
                        format!(
                            "call {name}: missing {}",
                            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                        ),
                    ));
                    continue;
                }
                let lines: Vec<String> = body
                    .iter()
                    .map(|l| given.iter().fold(l.clone(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v)))
                    .collect();
                let said = given.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ");
                let label = match &outer {
                    Some((_, o)) => format!("{o} > {name}({said})"),
                    None => format!("{name}({said})"),
                };
                unfold(&lines, base, templates, p, Some((line_no, label)), depth + 1);
            }
            "use" => {
                let mut path = PathBuf::from(rest);
                if !path.is_absolute() {
                    path = base.join(rest);
                }
                match std::fs::read_to_string(&path) {
                    Ok(t) => {
                        let lines: Vec<String> = t.lines().map(|s| s.to_string()).collect();
                        let folder = path.parent().map(|f| f.to_path_buf()).unwrap_or_else(|| base.to_path_buf());
                        unfold(&lines, &folder, templates, p, Some((line_no, rest.to_string())), depth + 1);
                    }
                    Err(e) => p.errors.push((line_no, format!("{rest}: {e}"))),
                }
            }
            "widget" if outer.is_none() => match parse_widget(rest, line_no) {
                Ok(w) => {
                    if p.widgets.iter().any(|x| x.name == w.name) {
                        p.errors.push((line_no, format!("two widgets called {}", w.name)));
                    } else {
                        p.widgets.push(w);
                    }
                }
                Err(e) => p.errors.push((line_no, e)),
            },
            "look" | "style" if outer.is_none() => p.looks.push((line_no, text.to_string())),
            // a line said for each k (widgets, hands and calls too: unfolded as the lines it makes)
            "repeat" => match grm_engine::lang::repeat(rest) {
                Ok(lines) => match &outer {
                    Some(o) => unfold(&lines, base, templates, p, Some((line_no, o.1.clone())), depth + 1),
                    None => {
                        // at the top, what it makes is the app's own (its widgets too), all from this line
                        let (errors, flat) = (p.errors.len(), p.flat.len());
                        unfold(&lines, base, templates, p, None, depth + 1);
                        p.errors[errors..].iter_mut().for_each(|e| e.0 = line_no);
                        p.flat[flat..].iter_mut().for_each(|f| f.line = line_no);
                    }
                },
                Err(e) => p.errors.push((line_no, e.message)),
            },
            "hand" => match parse_hand(rest, line_no) {
                Ok(h) => p.hands.push(h),
                Err(e) => p.errors.push((line_no, e)),
            },
            "api" => match parse_api(rest) {
                Ok(a) => {
                    p.apis.retain(|x| x.name != a.name);
                    p.apis.push(a);
                }
                Err(e) => p.errors.push((line_no, e.message)),
            },
            w if COMMANDS.contains(&w) => p.commands.push((line_no, text.to_string())),
            _ => p.flat.push(Flat { text: text.to_string(), line: line_no, from }),
        }
    }
    if let Some((name, _, _)) = recording {
        p.errors.push((outer.map(|o| o.0).unwrap_or(lines.len()), format!("network {name}: no end")));
    }
}

/// Where the widgets go: those placed stay; the others take the first room, row by row.
pub fn place(widgets: &mut [Widget]) {
    let mut taken: Vec<(i32, i32, i32, i32)> =
        widgets.iter().filter(|w| w.placed).map(|w| (w.at.0, w.at.1, w.size.0, w.size.1)).collect();
    let overlaps = |taken: &[(i32, i32, i32, i32)], c: i32, r: i32, w: i32, h: i32| {
        taken.iter().any(|&(x, y, tw, th)| c < x + tw && x < c + w && r < y + th && y < r + h)
    };
    for wd in widgets.iter_mut().filter(|w| !w.placed) {
        let (w, h) = (wd.size.0.min(COLUMNS), wd.size.1);
        'search: for r in 0..200 {
            for c in 0..=(COLUMNS - w) {
                if !overlaps(&taken, c, r, w, h) {
                    wd.at = (c, r);
                    taken.push((c, r, w, h));
                    break 'search;
                }
            }
        }
    }
}

/// The names its statements make (nodes, relations): what could meet another app's.
pub fn defined(flat: &[Flat]) -> Vec<String> {
    let mut out = vec![];
    for f in flat {
        let mut words = f.text.split(|c: char| c.is_whitespace() || c == ':' || c == '=' || c == '[');
        let word = words.next().unwrap_or("").to_lowercase();
        // a delay is a node too: two copies sharing one would pour the sound of one into the other's echo
        if matches!(
            word.as_str(),
            "node" | "var" | "learn" | "input" | "output" | "memory" | "relate" | "keep" | "rel" | "kp" | "delay"
        ) {
            if let Some(name) = words.find(|w| !w.is_empty()) {
                if is_name(name) && !out.iter().any(|o| o == name) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod defined_tests {
    use super::*;

    #[test]
    fn a_delay_is_defined_as_a_node_is() {
        let flat: Vec<Flat> = ["node a signed", "delay b = a by 0.3", "relate r: a = 2 * b"]
            .iter()
            .map(|t| Flat { text: t.to_string(), line: 1, from: None })
            .collect();
        let d = defined(&flat);
        assert!(d.contains(&"b".to_string()) && d.contains(&"a".to_string()), "{d:?}");
    }

    #[test]
    fn a_hand_names_its_switch_and_the_nodes_it_plays() {
        let h = parse_hand("kick when t3_k_hers: t3_k_s1 t3_k_s2 t3_k_level", 4).unwrap();
        assert_eq!(h.name, "kick");
        assert_eq!(h.switch, "t3_k_hers");
        assert_eq!(h.nodes, vec!["t3_k_s1", "t3_k_s2", "t3_k_level"]);
        assert!(parse_hand("kick t3_k_s1", 4).is_err());
    }
}

/// Every whole word `from` becomes `to`.
pub fn rename_words(text: &str, map: &IndexMap<String, String>) -> String {
    if map.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        match map.get(word.as_str()) {
            Some(to) => out.push_str(to),
            None => out.push_str(word),
        }
        word.clear();
    };
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted && (c.is_ascii_alphanumeric() || c == '_' || c == '~') {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

impl Parsed {
    /// Its names renamed (its own copy of a network another running app holds).
    pub fn renamed(&self, map: &IndexMap<String, String>) -> Parsed {
        if map.is_empty() {
            return self.clone();
        }
        let mut p = self.clone();
        for f in &mut p.flat {
            f.text = rename_words(&f.text, map);
        }
        for w in &mut p.widgets {
            for n in &mut w.nodes {
                if let Some(to) = map.get(n) {
                    *n = to.clone();
                }
            }
        }
        for h in &mut p.hands {
            if let Some(to) = map.get(&h.switch) {
                h.switch = to.clone();
            }
            for n in &mut h.nodes {
                if let Some(to) = map.get(n) {
                    *n = to.clone();
                }
            }
        }
        for a in &mut p.apis {
            for r in &mut a.reads {
                if let Some(to) = map.get(&r.node) {
                    r.node = to.clone();
                }
            }
            match &mut a.way {
                grm_connect::api::Way::Post { with, .. } => with.iter_mut().for_each(|n| *n = rename_words(n, map)),
                grm_connect::api::Way::Websocket { send, .. } => {
                    send.iter_mut().for_each(|n| *n = rename_words(n, map))
                }
                _ => {}
            }
        }
        for (_, c) in &mut p.commands {
            *c = rename_words(c, map);
        }
        p
    }
}

/// The line of an app's text that says a relation, a node or a widget of this name (1 = the first), if the app
/// writes it itself (not through a network it calls).
pub fn line_of(text: &str, name: &str) -> Option<usize> {
    for (k, raw) in text.lines().enumerate() {
        let t = strip(raw);
        let mut words = t.split(|c: char| c.is_whitespace() || c == ':' || c == '=' || c == '[');
        let word = words.next().unwrap_or("").to_lowercase();
        if matches!(word.as_str(), "node" | "var" | "learn" | "input" | "output" | "relate" | "keep" | "widget")
            && words.find(|w| !w.is_empty()) == Some(name)
        {
            return Some(k + 1);
        }
    }
    None
}

/// The text with line `n` (1 = the first) replaced (None: taken away).
pub fn with_line(text: &str, n: usize, new: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if n >= 1 && n <= lines.len() {
        match new {
            Some(l) => lines[n - 1] = l.to_string(),
            None => {
                lines.remove(n - 1);
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The text with a line added at its end.
pub fn with_added(text: &str, line: &str) -> String {
    let mut out = text.trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_app_is_read_unfolded_and_set_apart() {
        let dir = std::env::temp_dir().join("grm-apps-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("osc.grm"),
            "network osc(name, step)\n  var {name}_c = 1\n  var {name}_s = 0\n  node {name}_new signed\n  relate {name}_turn: {name}_new = {name}_c - {step} * {name}_s\n  set {name}_c = {name}_new\n  set {name}_s = {name}_s + {step} * {name}_new\nend\n",
        )
        .unwrap();
        let text = "app Bench\nuse osc.grm\ncall osc(name=lead, step=0.0576)\nnode volume = 0.5 # how loud\nwidget vol: slider volume 0 .. 1\nwidget scope: plot lead_s -1 .. 1 at 0 2 size 12 3\napi w: get http://x every 60\nread w a.b -> volume ± 0.1\nplay lead_s 0.4\nwidget bad: fly\n";
        let p = parse(text, &dir);
        assert_eq!(p.title.as_deref(), Some("Bench"));
        assert_eq!(p.flat.len(), 7, "{:?}", p.flat);
        assert!(p.flat[0].text == "var lead_c = 1" && p.flat[0].line == 3);
        assert!(p.flat[0].from.as_deref().unwrap().starts_with("osc(name=lead"));
        assert_eq!(p.widgets.len(), 2);
        assert_eq!(p.widgets[0].at, (0, 0), "placed where there is room");
        assert_eq!(p.widgets[1].at, (0, 2));
        assert_eq!(p.apis[0].reads[0].node, "volume");
        assert_eq!(p.commands, vec![(9, "play lead_s 0.4".to_string())]);
        assert_eq!(p.errors.len(), 1);
        assert_eq!(p.errors[0].0, 10);
        let d = defined(&p.flat);
        assert!(
            d.contains(&"lead_c".to_string())
                && d.contains(&"lead_turn".to_string())
                && d.contains(&"volume".to_string())
        );
        let map: IndexMap<String, String> = [("lead_s".to_string(), "b__lead_s".to_string())].into_iter().collect();
        let r = p.renamed(&map);
        assert!(r.flat.iter().any(|f| f.text == "set b__lead_s = b__lead_s + 0.0576 * lead_new"));
        assert_eq!(r.widgets[1].nodes[0], "b__lead_s");
        assert_eq!(r.commands[0].1, "play b__lead_s 0.4");
        assert_eq!(line_of(text, "vol"), Some(5));
        assert_eq!(line_of(text, "volume"), Some(4));
        let w = parse_widget("about: text \"a, b\" at 0 0 size 12 1", 1).unwrap();
        assert_eq!((w.text.as_str(), w.size, w.placed), ("a, b", (12, 1), true));
        let w = parse_widget("k: keys note gate", 1).unwrap();
        assert_eq!((w.lo, w.hi, w.nodes.len()), (48.0, 72.0, 2));
        assert_eq!(parse_widget(&w.statement()["widget ".len()..], 1).unwrap().nodes, w.nodes);
        assert_eq!(with_line("a\nb\nc\n", 2, None), "a\nc\n");
        assert_eq!(with_added("a", "b"), "a\nb\n");
    }
}
