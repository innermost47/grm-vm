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
    /// The sky, as a weather code says it (WMO: clear, clouds, fog, drizzle, rain, snow, storm), by day or night
    /// (nodes: the code, and 1 by day / 0 by night).
    Sky,
    /// A curve through its nodes, one after another (the next hours, say), on LO .. HI.
    Series,
    /// Its nodes two by two (a low and a high: a day's coldest and warmest), each pair a bar on LO .. HI.
    Range,
    /// A drum machine's steps (its words: the machine's name, `t3`): a row per drum, its pad, its steps on the
    /// transport, the one playing lit.
    Drums,
    /// Bars side by side, each as tall as its node is among the others; the first node says which one is lit (from
    /// 1): `bars elle_choice elle_want1 .. elle_want16`.
    Bars,
    /// One among a few, its words the choices (`"low|middle|high"`): the node is the one chosen (0, 1, 2...).
    Choice,
    /// A number written by hand (`9,81` as well as `9.81`): the node takes it when Enter is pressed.
    Field,
    // what holds other widgets (written `widget NAME: KIND ...`, the widgets it holds under it, then `end`)
    /// Side by side, each as wide as its size says.
    Row,
    /// One under another, each as tall as its size says.
    Column,
    /// In a grid of N columns (`grid 3`), cell after cell.
    Grid,
    /// One over another, all in the same place (layers).
    Stack,
    /// One under another, and what does not fit scrolls (the bar never covers them).
    Scroll,
    /// One at a time, chosen by its tab (each tab says its widget's words).
    Tabs,
    /// A framed card, its words as its title, what it holds under them.
    Card,
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
            "sky" => WidgetKind::Sky,
            "series" => WidgetKind::Series,
            "range" => WidgetKind::Range,
            "drums" => WidgetKind::Drums,
            "choice" => WidgetKind::Choice,
            "bars" => WidgetKind::Bars,
            "field" => WidgetKind::Field,
            "row" => WidgetKind::Row,
            "column" => WidgetKind::Column,
            "grid" => WidgetKind::Grid,
            "stack" => WidgetKind::Stack,
            "scroll" => WidgetKind::Scroll,
            "tabs" => WidgetKind::Tabs,
            "card" => WidgetKind::Card,
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
            WidgetKind::Sky => "sky",
            WidgetKind::Series => "series",
            WidgetKind::Range => "range",
            WidgetKind::Drums => "drums",
            WidgetKind::Choice => "choice",
            WidgetKind::Bars => "bars",
            WidgetKind::Field => "field",
            WidgetKind::Row => "row",
            WidgetKind::Column => "column",
            WidgetKind::Grid => "grid",
            WidgetKind::Stack => "stack",
            WidgetKind::Scroll => "scroll",
            WidgetKind::Tabs => "tabs",
            WidgetKind::Card => "card",
        }
    }

    /// Whether it holds other widgets (its lines end with `end`).
    pub fn holds(self) -> bool {
        matches!(
            self,
            WidgetKind::Row
                | WidgetKind::Column
                | WidgetKind::Grid
                | WidgetKind::Stack
                | WidgetKind::Scroll
                | WidgetKind::Tabs
                | WidgetKind::Card
        )
    }

    /// How many nodes it is bound to.
    pub fn nodes(self) -> usize {
        match self {
            WidgetKind::Text | WidgetKind::Section | WidgetKind::Roll | WidgetKind::Rec | WidgetKind::Drums => 0,
            k if k.holds() => 0,
            WidgetKind::Pad | WidgetKind::Keys | WidgetKind::Step | WidgetKind::Range => 2,
            _ => 1,
        }
    }

    /// Its size when none is given (columns, rows of the surface's grid).
    pub fn size(self) -> (i32, i32) {
        match self {
            WidgetKind::Row
            | WidgetKind::Column
            | WidgetKind::Grid
            | WidgetKind::Stack
            | WidgetKind::Scroll
            | WidgetKind::Tabs
            | WidgetKind::Card => (12, 4),
            WidgetKind::Choice | WidgetKind::Field => (3, 1),
            WidgetKind::Bars => (4, 2),
            WidgetKind::Slider => (4, 1),
            WidgetKind::Toggle | WidgetKind::Button | WidgetKind::Play | WidgetKind::Rec => (2, 1),
            WidgetKind::Meter => (3, 1),
            WidgetKind::Value => (3, 1),
            WidgetKind::Plot => (6, 3),
            WidgetKind::Text => (6, 1),
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
            WidgetKind::Sky => (3, 3),
            WidgetKind::Series => (12, 3),
            WidgetKind::Range => (12, 4),
            WidgetKind::Drums => (12, 4),
        }
    }

    pub const ALL: [WidgetKind; 33] = [
        WidgetKind::Slider,
        WidgetKind::Toggle,
        WidgetKind::Button,
        WidgetKind::Meter,
        WidgetKind::Value,
        WidgetKind::Plot,
        WidgetKind::Text,
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
        WidgetKind::Sky,
        WidgetKind::Series,
        WidgetKind::Range,
        WidgetKind::Drums,
        WidgetKind::Choice,
        WidgetKind::Bars,
        WidgetKind::Field,
        WidgetKind::Row,
        WidgetKind::Column,
        WidgetKind::Grid,
        WidgetKind::Stack,
        WidgetKind::Scroll,
        WidgetKind::Tabs,
        WidgetKind::Card,
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
    /// Whether its size is written (`size W H`); what holds others and says none is as tall as what it holds.
    #[serde(default)]
    pub sized: bool,
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
    /// `tip "words"`: what it does, said when the pointer rests on it.
    #[serde(default)]
    pub tip: Option<String>,
    /// `when NODE`: shown only while the node is not 0.
    #[serde(default)]
    pub when: Option<String>,
    /// `key K`: a key of the keyboard that presses it too (space, enter, a letter...), when nothing is being typed.
    #[serde(default)]
    pub key: Option<String>,
    /// `say "LINE"`: the line it says when it is pressed (`{app}` is its app's name), as typed at the prompt.
    #[serde(default)]
    pub say: Option<String>,
    /// `grid N`: its columns.
    #[serde(default)]
    pub columns: Option<i32>,
    /// What it holds (a row, a column, a card...), in their order.
    #[serde(default)]
    pub children: Vec<Widget>,
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
        if let Some(t) = &self.tip {
            s.push_str(&format!(" tip \"{t}\""));
        }
        if let Some(w) = &self.when {
            s.push_str(&format!(" when {w}"));
        }
        if let Some(k) = &self.key {
            s.push_str(&format!(" key {k}"));
        }
        if let Some(l) = &self.say {
            s.push_str(&format!(" say \"{l}\""));
        }
        if let Some(i) = &self.icon {
            s.push_str(&format!(" icon {i}"));
        }
        s
    }
}

/// `widget NAME: KIND [NODES] [LO .. HI] [at C R] [size W H] ["words"]`
pub fn parse_widget(rest: &str, line: usize) -> Result<Widget, String> {
    let usage = "widget NAME: KIND [NODE..] [LO .. HI] [at COLUMN ROW] [size COLUMNS ROWS] [\"words\"] [tip \"words\"] [when NODE]";
    let Some((name, body)) = rest.split_once(':') else { return Err(usage.into()) };
    // `tip "words"` first (its words are not the widget's own)
    let mut tip = None;
    let mut body = body.to_string();
    if let Some(k) = body.find("tip \"") {
        let after = &body[k + 5..];
        let end = after.find('"').ok_or("tip \"words\": its words end with a quote")?;
        tip = Some(after[..end].to_string());
        body = format!("{}{}", &body[..k], &after[end + 1..]);
    }
    let mut say = None;
    if let Some(k) = body.find("say \"") {
        let after = body[k + 5..].to_string();
        let end = after.find('"').ok_or("say \"line\": its line ends with a quote")?;
        say = Some(after[..end].to_string());
        body = format!("{}{}", &body[..k], &after[end + 1..]);
    }
    let body = body.as_str();
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
    let mut when = None;
    let mut key = None;
    let mut columns = None;
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
            // `key K`: only a key's name (a node may be called key)
            "key" if words.get(k + 1).is_some_and(|w| is_key(w)) => {
                key = words.get(k + 1).map(|w| w.to_string());
                k += 2;
            }
            "when" => {
                when = Some(words.get(k + 1).filter(|w| is_name(w)).ok_or("when NODE")?.to_string());
                k += 2;
            }
            w if k == 1 && kind == WidgetKind::Grid && w.parse::<i32>().is_ok() => {
                columns = w.parse::<i32>().ok().map(|c| c.clamp(1, 24));
                k += 1;
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
    // a button that says a line needs no node: what it does is its line
    let needs = if kind == WidgetKind::Button && say.is_some() { 0 } else { kind.nodes() };
    if nodes.len() < needs {
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
        sized: size.is_some(),
        size: size.unwrap_or(kind.size()),
        placed: at.is_some(),
        text,
        line,
        look,
        icon,
        tip,
        when,
        key,
        say,
        columns,
        children: vec![],
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
    /// Its kinds of widget (`kind NAME(param, ...): KIND ... end`): their parameters, their first line, their body.
    pub kinds: IndexMap<String, (Vec<String>, String, Vec<String>)>,
    /// Its lists (`list tracks = synths drums`: a list in a list is its items), their items in their order, and the
    /// line each is written on.
    pub lists: IndexMap<String, (Vec<String>, usize)>,
    /// The widgets that hold others, open while their lines are read (closed by `end`).
    open: Vec<Widget>,
    /// While a kind is unfolded: the line of the app's file everything comes from.
    at_line: Option<usize>,
}

/// The keys a widget can be pressed by: space, enter, escape, tab, a letter or a digit, f1 .. f12.
pub fn is_key(w: &str) -> bool {
    matches!(w, "space" | "enter" | "escape" | "tab")
        || (w.len() == 1 && w.chars().all(|c| c.is_ascii_alphanumeric()))
        || w.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n))
}

/// How many rows what holds others takes, from what it holds (one under another: their sum; side by side: the
/// tallest; a grid: the tallest of each of its rows; a card's title and tabs' tabs: one more).
fn natural_rows(w: &Widget) -> i32 {
    let rows = |c: &Widget| c.size.1.max(1);
    let shown = &w.children;
    match w.kind {
        WidgetKind::Column | WidgetKind::Scroll => shown.iter().map(rows).sum(),
        WidgetKind::Card => shown.iter().map(rows).sum::<i32>() + i32::from(!w.text.is_empty()),
        WidgetKind::Row | WidgetKind::Stack => shown.iter().map(rows).max().unwrap_or(1),
        WidgetKind::Tabs => shown.iter().map(rows).max().unwrap_or(1) + 1,
        WidgetKind::Grid => {
            let n = w.columns.unwrap_or(2).max(1) as usize;
            shown.chunks(n).map(|r| r.iter().map(rows).max().unwrap_or(1)).sum()
        }
        _ => w.size.1,
    }
}

/// Every widget's name, those held included.
pub fn all_widgets(ws: &[Widget]) -> Vec<&Widget> {
    let mut out = vec![];
    for w in ws {
        out.push(w);
        out.extend(all_widgets(&w.children));
    }
    out
}

impl Parsed {
    /// A widget read: held by the one open, or on the surface.
    fn put_widget(&mut self, w: Widget, line_no: usize) {
        let taken = all_widgets(&self.widgets).iter().any(|x| x.name == w.name)
            || self.open.iter().any(|o| o.name == w.name || all_widgets(&o.children).iter().any(|x| x.name == w.name));
        if taken {
            self.errors.push((line_no, format!("two widgets called {}", w.name)));
            return;
        }
        if w.kind.holds() {
            self.open.push(w);
        } else {
            self.held(w);
        }
    }

    fn held(&mut self, mut w: Widget) {
        // what holds others and says no size: as tall as what it holds (a list that grows grows it)
        if w.kind.holds() && !w.sized {
            w.size.1 = natural_rows(&w).max(1);
        }
        match self.open.last_mut() {
            Some(o) => o.children.push(w),
            None => self.widgets.push(w),
        }
    }

    /// `end`: the widget open is closed (what it holds is read).
    fn close_widget(&mut self, line_no: usize) {
        match self.open.pop() {
            Some(w) => self.held(w),
            None => self.errors.push((line_no, "end: no widget that holds others is open".into())),
        }
    }
}

pub const COMMANDS: &[&str] =
    &["track", "play", "sound", "voice", "volume", "embody", "tick", "mute", "solo", "gain", "pan"];

type Templates = IndexMap<String, (Vec<String>, Vec<String>)>;

/// A kind of widget being read: its name, its parameters, its first line, its body, how deep in what it holds.
type KindRecord = (String, Vec<String>, String, Vec<String>, usize);

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
    // a kind of widget being written: (name, params, its first line, its body, how deep in what it holds)
    let mut kind_rec: Option<KindRecord> = None;
    for (k, raw) in lines.iter().enumerate() {
        let line_no = outer.as_ref().map(|o| o.0).or(p.at_line).unwrap_or(k + 1);
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
        if let Some((name, params, head, mut body, depth)) = kind_rec.take() {
            let opens = text.starts_with("widget ")
                && text
                    .split_once(':')
                    .and_then(|(_, b)| b.split_whitespace().next())
                    .and_then(WidgetKind::of)
                    .is_some_and(|k| k.holds());
            if text.eq_ignore_ascii_case("end") && depth == 0 {
                p.kinds.insert(name, (params, head, body));
            } else {
                let depth = if text.eq_ignore_ascii_case("end") { depth - 1 } else { depth + opens as usize };
                if !text.is_empty() {
                    body.push(text.to_string());
                }
                kind_rec = Some((name, params, head, body, depth));
            }
            continue;
        }
        if text.is_empty() {
            continue;
        }
        // a sum over a list is the sum of its terms, written out
        let expanded;
        let text = if text.contains(" in ") && text.contains("sum(") {
            match sums_over_lists(text, &p.lists) {
                Ok(t) => {
                    expanded = t;
                    expanded.as_str()
                }
                Err(e) => {
                    p.errors.push((line_no, e));
                    continue;
                }
            }
        } else {
            text
        };
        let (word, rest) = text.split_once(char::is_whitespace).map(|(a, b)| (a, b.trim())).unwrap_or((text, ""));
        // `repeat t in LIST: line`: the line said for each of its items ({t} the item, {t.k} its place from 1, {t.i} from 0)
        if word.eq_ignore_ascii_case("repeat") {
            if let Some(lines) = repeat_over_list(rest, &p.lists) {
                match lines {
                    Ok(lines) => {
                        let (errors, flat) = (p.errors.len(), p.flat.len());
                        let before = p.at_line.replace(line_no);
                        unfold(&lines, base, templates, p, outer.clone(), depth + 1);
                        p.at_line = before;
                        p.errors[errors..].iter_mut().for_each(|e| e.0 = line_no);
                        p.flat[flat..].iter_mut().for_each(|f| f.line = line_no);
                    }
                    Err(e) => p.errors.push((line_no, e)),
                }
                continue;
            }
        }
        match word.to_lowercase().as_str() {
            "list" => match parse_list(rest, &p.lists) {
                Ok((name, items)) => {
                    p.lists.insert(name, (items, line_no));
                }
                Err(e) => p.errors.push((line_no, e)),
            },
            // kinds are made anywhere (an app's own, or a library's: `use widgets/mixer.grm`)
            "kind" => match parse_kind_head(rest) {
                Ok((name, params, head)) => kind_rec = Some((name, params, head, vec![], 0)),
                Err(e) => p.errors.push((line_no, e)),
            },
            "end" if outer.is_none() => p.close_widget(line_no),
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
                // `use FILE (k=v, ...)`: the file said with each {k} written v (a lesson: her name, where it holds)
                let (rest, given) = match rest.split_once(" (") {
                    Some((f, args)) if args.ends_with(')') => (f.trim(), args.trim_end_matches(')').to_string()),
                    _ => (rest, String::new()),
                };
                let given: Vec<(String, String)> = given
                    .split(',')
                    .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                    .collect();
                let mut path = PathBuf::from(rest);
                if !path.is_absolute() {
                    path = base.join(rest);
                    // the widgets' library stands beside the apps (`widgets/` next to `apps/`)
                    if !path.exists() && rest.starts_with("widgets/") {
                        if let Some(up) = base.parent() {
                            path = up.join(rest);
                        }
                    }
                }
                match std::fs::read_to_string(&path) {
                    Ok(t) => {
                        let said =
                            |l: &str| given.iter().fold(l.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v));
                        let lines: Vec<String> = t.lines().map(said).collect();
                        let folder = path.parent().map(|f| f.to_path_buf()).unwrap_or_else(|| base.to_path_buf());
                        if given.is_empty() || outer.is_some() {
                            unfold(&lines, &folder, templates, p, Some((line_no, rest.to_string())), depth + 1);
                        } else {
                            // said with its words given: the app's own lines, written here (not a network it shares)
                            let before = p.at_line.replace(line_no);
                            unfold(&lines, &folder, templates, p, None, depth + 1);
                            p.at_line = before;
                        }
                    }
                    Err(e) => p.errors.push((line_no, format!("{rest}: {e}"))),
                }
            }
            "widget" if outer.is_none() => match kind_called(rest, &p.kinds) {
                // a kind of one's own: its lines, said again for this one (its widgets named after it)
                Some(Ok(lines)) => {
                    let before = p.at_line.replace(line_no);
                    unfold(&lines, base, templates, p, None, depth + 1);
                    p.at_line = before;
                }
                Some(Err(e)) => p.errors.push((line_no, e)),
                None => match parse_widget(rest, line_no) {
                    Ok(w) => p.put_widget(w, line_no),
                    Err(e) => p.errors.push((line_no, e)),
                },
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
        p.errors.push((outer.as_ref().map(|o| o.0).unwrap_or(lines.len()), format!("network {name}: no end")));
    }
    if let Some((name, ..)) = kind_rec {
        p.errors.push((p.at_line.unwrap_or(lines.len()), format!("kind {name}: no end")));
    }
    // the app's own lines read to their last: what is still open was not closed
    if depth == 0 && outer.is_none() {
        while let Some(w) = p.open.last() {
            p.errors.push((w.line, format!("widget {}: no end (what holds others ends with `end`)", w.name)));
            p.close_widget(0);
        }
    }
}

/// `list NAME = ITEM ...`: its name and its items (a list named among them gives its own items).
fn parse_list(rest: &str, lists: &IndexMap<String, (Vec<String>, usize)>) -> Result<(String, Vec<String>), String> {
    let usage = "list NAME = ITEM ITEM ... (a list among them gives its items)";
    let (name, items) = rest.split_once('=').ok_or(usage)?;
    let name = name.trim();
    if !is_name(name) {
        return Err(format!("{usage}: {name} is not a name"));
    }
    let mut out = vec![];
    for w in items.split_whitespace() {
        // an item is a word (letters, digits, _: a file's name may begin with a digit, 01_one_note)
        if w.is_empty() || !w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("{usage}: {w} is not a word"));
        }
        match lists.get(w) {
            Some((inner, _)) => out.extend(inner.iter().cloned()),
            None => out.push(w.to_string()),
        }
    }
    Ok((name.to_string(), out))
}

/// `repeat t in LIST: line` (None: not over a list; `repeat k in 1 .. 3` is the language's).
fn repeat_over_list(rest: &str, lists: &IndexMap<String, (Vec<String>, usize)>) -> Option<Result<Vec<String>, String>> {
    let (head, line) = rest.split_once(':')?;
    let mut w = head.split_whitespace();
    let (var, inw, list) = (w.next()?, w.next()?, w.next()?);
    if inw != "in" || w.next().is_some() || !is_name(list) {
        return None;
    }
    let Some((items, _)) = lists.get(list) else {
        return Some(Err(format!("repeat {var} in {list}: no list {list} (a list is written before it is used)")));
    };
    let one = |k: usize, it: &String| {
        line.trim()
            .replace(&format!("{{{var}.k}}"), &(k + 1).to_string())
            .replace(&format!("{{{var}.i}}"), &k.to_string())
            .replace(&format!("{{{var}}}"), it)
    };
    Some(Ok(items.iter().enumerate().map(|(k, it)| one(k, it)).collect()))
}

/// Each `sum(t in LIST: term)` of a line written out: `(term1 + term2 ...)`, `0` for an empty list.
pub fn sums_over_lists(text: &str, lists: &IndexMap<String, (Vec<String>, usize)>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(k) = rest.find("sum(") {
        let after = &rest[k + 4..];
        let head = after.split(':').next().unwrap_or("");
        let mut w = head.split_whitespace();
        let over = match (w.next(), w.next(), w.next(), w.next()) {
            (Some(var), Some("in"), Some(list), None) if lists.contains_key(list) => Some((var, list)),
            _ => None,
        };
        let Some((var, list)) = over else {
            out.push_str(&rest[..k + 4]);
            rest = after;
            continue;
        };
        // its term, to the parenthesis that closes the sum
        let body_at = head.len() + 1;
        let mut depth = 1;
        let mut end = None;
        for (i, c) in after[body_at..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(body_at + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or(format!("sum({var} in {list}: ...): its parenthesis is not closed"))?;
        let term = after[body_at..end].trim();
        let items = &lists[list].0;
        let written = if items.is_empty() {
            "0".to_string()
        } else {
            let terms: Vec<String> = items.iter().map(|it| term.replace(&format!("{{{var}}}"), it)).collect();
            format!("({})", terms.join(" + "))
        };
        out.push_str(&rest[..k]);
        out.push_str(&written);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// A list of an app's file changed: `add ITEM`, `add next [PREFIX]` (the next name free among all its lists:
/// t1 t2 t3 → t4), `remove ITEM`. The file's text after it, and what was done.
pub fn change_list(text: &str, base: &Path, list: &str, how: &str) -> Result<(String, String), String> {
    let p = parse(text, base);
    let Some((items, line)) = p.lists.get(list).cloned() else { return Err(format!("no list {list}")) };
    let written_line = text.lines().nth(line - 1).unwrap_or("");
    let own: Vec<String> = written_line
        .split_once('=')
        .map(|(_, r)| r.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    let words: Vec<&str> = how.split_whitespace().collect();
    let (new, said) = match words.as_slice() {
        ["add", "next", prefix @ ..] => {
            let prefix = match prefix.first() {
                Some(p) => p.to_string(),
                None => items
                    .first()
                    .map(|i| i.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
                    .ok_or(format!("list {list} is empty: add next PREFIX (t: t1, t2...)"))?,
            };
            let all = p.lists.values().flat_map(|(i, _)| i.iter());
            let top = all.filter_map(|i| i.strip_prefix(prefix.as_str())?.parse::<u32>().ok()).max().unwrap_or(0);
            let item = format!("{prefix}{}", top + 1);
            let mut o = own.clone();
            o.push(item.clone());
            (o, format!("{item} added to {list}"))
        }
        ["add", item] if is_name(item) => {
            if items.iter().any(|i| i == item) {
                return Err(format!("{item} is already in {list}"));
            }
            let mut o = own.clone();
            o.push(item.to_string());
            (o, format!("{item} added to {list}"))
        }
        ["remove", item] => {
            if !own.iter().any(|i| i == item) {
                return Err(format!("{item} is not written in {list}"));
            }
            (own.iter().filter(|i| i != item).cloned().collect(), format!("{item} taken from {list}"))
        }
        _ => return Err("list APP LIST add ITEM | add next [PREFIX] | remove ITEM".into()),
    };
    let indent: String = written_line.chars().take_while(|c| c.is_whitespace()).collect();
    let line_now = format!("{indent}list {list} = {}", new.join(" "));
    Ok((with_line(text, line, Some(&line_now)), said))
}

/// `kind NAME(param, ...): KIND ...`: its name, its parameters, its first line (what it is, its words, its look).
fn parse_kind_head(rest: &str) -> Result<(String, Vec<String>, String), String> {
    let usage = "kind NAME(param, ...): KIND ... (the widgets it holds) end";
    let (left, head) = rest.split_once(':').ok_or(usage)?;
    let (name, params) = match left.split_once('(') {
        Some((n, ps)) => (
            n.trim().to_string(),
            ps.trim_end()
                .trim_end_matches(')')
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        ),
        None => (left.trim().to_string(), vec![]),
    };
    if !is_name(&name) || WidgetKind::of(&name).is_some() {
        return Err(format!("{usage}: {name} is not a name of its own"));
    }
    let first = head.split_whitespace().next().and_then(WidgetKind::of).ok_or(format!("{usage}: KIND"))?;
    if !first.holds() {
        return Err(format!("kind {name}: a kind holds widgets (row, column, grid, stack, scroll, tabs, card)"));
    }
    Ok((name, params, head.trim().to_string()))
}

/// `widget NAME: KIND(param=value, ...) ...` for a kind of one's own: the lines it makes (None: not one).
fn kind_called(
    rest: &str,
    kinds: &IndexMap<String, (Vec<String>, String, Vec<String>)>,
) -> Option<Result<Vec<String>, String>> {
    let (name, body) = rest.split_once(':')?;
    let body = body.trim();
    let called = body.split(|c: char| c == '(' || c.is_whitespace()).next()?;
    let (params, head, lines) = kinds.get(called)?;
    let name = name.trim();
    let (args, after) = match body[called.len()..].trim_start().strip_prefix('(') {
        Some(a) => match a.split_once(')') {
            Some((inside, after)) => (inside, after),
            None => return Some(Err(format!("widget {name}: {called}(param=value, ...)"))),
        },
        None => ("", &body[called.len()..]),
    };
    let mut given: IndexMap<String, String> = IndexMap::new();
    for kv in args.split(',').filter(|s| !s.trim().is_empty()) {
        match kv.split_once('=') {
            Some((k, v)) => {
                given.insert(k.trim().to_string(), v.trim().to_string());
            }
            None => return Some(Err(format!("widget {name}: {called}(param=value, ...)"))),
        }
    }
    let missing: Vec<&str> = params.iter().filter(|q| !given.contains_key(*q)).map(String::as_str).collect();
    if !missing.is_empty() {
        return Some(Err(format!("widget {name}: {called} misses {}", missing.join(", "))));
    }
    given.insert("name".into(), name.to_string());
    let said = |l: &str| given.iter().fold(l.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v));
    // its widgets are named after it: `widget gain: ...` in a strip called t1 is t1_gain
    let inner = |l: String| match l.strip_prefix("widget ").and_then(|r| r.split_once(':')) {
        Some((n, b)) => format!("widget {name}_{}:{b}", n.trim()),
        None => l,
    };
    let mut out = vec![format!("widget {name}: {} {}", said(head), after.trim())];
    out.extend(lines.iter().map(|l| inner(said(l))));
    out.push("end".into());
    Some(Ok(out))
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
            "node"
                | "var"
                | "learn"
                | "input"
                | "output"
                | "memory"
                | "relate"
                | "keep"
                | "rel"
                | "kp"
                | "delay"
                | "audio"
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
    fn the_weather_app_reads_its_hours_and_its_week() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
        let text = std::fs::read_to_string(root.join("weather.grm")).unwrap();
        let p = parse(&text, &root);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let reads: usize = p.apis.iter().map(|a| a.reads.len()).sum();
        assert_eq!(reads, 11 + 24 * 2 + 7 * 3);
        assert!(p.widgets.iter().any(|w| w.kind == WidgetKind::Range && w.nodes.len() == 14));
        assert!(p.widgets.iter().any(|w| w.kind == WidgetKind::Sky));
    }

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
    fn widgets_hold_widgets_and_kinds_of_ones_own_are_made_like_networks() {
        let text = "app T\n\
            kind strip(ch): column \"{ch}\" tip \"a channel\"\n\
              widget gain: fader {ch}_gain 0 .. 1 tip \"how loud\"\n\
              widget pan: knob {ch}_pan -1 .. 1\n\
            end\n\
            widget mixer: row at 0 0 size 12 4\n\
              widget t1: strip(ch=t1)\n\
              widget t2: strip(ch=t2) when t2_on\n\
              widget more: tabs\n\
                widget a: text \"one\"\n\
                widget b: grid 3\n\
                  widget c: value t1_gain\n\
                end\n\
              end\n\
            end\n\
            widget alone: text \"under\"\n";
        let p = parse(text, Path::new("."));
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.widgets.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["mixer", "alone"]);
        let mixer = &p.widgets[0];
        assert_eq!(mixer.kind, WidgetKind::Row);
        let names: Vec<&str> = mixer.children.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["t1", "t2", "more"]);
        let t1 = &mixer.children[0];
        assert_eq!((t1.kind, t1.text.as_str(), t1.tip.as_deref()), (WidgetKind::Column, "t1", Some("a channel")));
        assert_eq!(t1.children[0].name, "t1_gain");
        assert_eq!(t1.children[0].nodes, ["t1_gain"]);
        assert_eq!(t1.children[0].tip.as_deref(), Some("how loud"));
        assert_eq!(t1.children[1].lo, -1.0);
        assert_eq!(t1.line, 7, "a kind's widgets are written on the line that calls it");
        assert_eq!(mixer.children[1].when.as_deref(), Some("t2_on"));
        let b = &mixer.children[2].children[1];
        assert_eq!((b.kind, b.columns, b.children.len()), (WidgetKind::Grid, Some(3), 1));
        assert_eq!(all_widgets(&p.widgets).len(), 12);
    }

    /// The widgets' library (`widgets/`, beside `apps/`): its kinds are used by any app, with no error.
    #[test]
    fn the_widgets_library_is_used_from_an_app() {
        let apps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
        let text = "app M\nuse widgets/mixer.grm\nwidget mixer: row at 0 0 size 12 7\n  widget a: strip(ch=t1, label=synth)\n  widget out: master\nend\n";
        let p = parse(text, &apps);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let names: Vec<&str> = all_widgets(&p.widgets).iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"a_level") && names.contains(&"a_pan") && names.contains(&"out_room"), "{names:?}");
    }

    /// Lists: one in another gives its items; a line repeated for each item, a sum over them written out; a button
    /// that says a line; the list changed in the file (the next name free among all the lists).
    #[test]
    fn lists_make_lines_sums_and_widgets_and_change_in_the_file() {
        let text = "app S\n\
            list synths = t1 t2\n\
            list drums = t3\n\
            list tracks = synths drums\n\
            repeat t in tracks: node {t}_out = {t.k}\n\
            node out\n\
            relate mix: out = sum(t in tracks: 2 * {t}_out) + sum(j in 1 .. 2: j)\n\
            widget col: column at 0 0 size 12 4\n\
              repeat t in synths: widget {t}v: value {t}_out\n\
            end\n\
            widget add: button \"+ synth\" icon plus say \"list {app} synths add next\" at 0 5 size 2 1\n";
        let p = parse(text, Path::new("."));
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.lists["tracks"].0, ["t1", "t2", "t3"]);
        let lines: Vec<&str> = p.flat.iter().map(|f| f.text.as_str()).collect();
        assert!(lines.contains(&"node t3_out = 3"), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("(2 * t1_out + 2 * t2_out + 2 * t3_out) + sum(j in 1 .. 2: j)")),
            "{lines:?}"
        );
        assert_eq!(p.widgets[0].children.len(), 2);
        assert_eq!(p.widgets[1].say.as_deref(), Some("list {app} synths add next"));
        let (after, said) = change_list(text, Path::new("."), "synths", "add next").unwrap();
        assert!(after.contains("list synths = t1 t2 t4"), "t3 is taken by the drums: {after}");
        assert_eq!(said, "t4 added to synths");
        let (after, _) = change_list(&after, Path::new("."), "synths", "remove t1").unwrap();
        assert!(after.contains("list synths = t2 t4"));
        assert!(change_list(&after, Path::new("."), "tracks", "remove t2").is_err(), "t2 is written in synths");
        let p = parse("repeat t in nothing: node {t}\n", Path::new("."));
        assert!(p.errors[0].1.contains("no list nothing"), "{:?}", p.errors);
    }

    /// The studio is its file: its lists make its instruments, their panels, the mixer, the mix; no error.
    #[test]
    fn the_studio_is_written_in_grml_with_no_error() {
        let apps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
        let text = std::fs::read_to_string(apps.join("studio.grm")).unwrap();
        let p = parse(&text, &apps);
        assert!(p.errors.is_empty(), "{:?}", &p.errors[..p.errors.len().min(5)]);
        assert_eq!(p.lists["tracks"].0, ["t1", "t2", "t3"]);
        let names: Vec<&str> = all_widgets(&p.widgets).iter().map(|w| w.name.as_str()).collect();
        for n in ["t1sec", "t2sec_keys", "t3sec_grid", "t3mix_level", "out_room"] {
            assert!(names.contains(&n), "{n} not in {names:?}");
        }
        let flat: Vec<&str> = p.flat.iter().map(|f| f.text.as_str()).collect();
        assert!(flat
            .iter()
            .any(|l| l.starts_with("relate master_left: out = (master * t1_l + master * t2_l + master * t3_l)")));
        assert!(flat.iter().any(|l| l.contains("elle_lesson in 27.5 .. 28.5")), "the lessons, each where it holds");
    }

    #[test]
    fn what_holds_widgets_says_when_it_is_not_closed_or_named_twice() {
        let p = parse("widget a: row\nwidget b: text \"x\"\nwidget b: text \"y\"\n", Path::new("."));
        let said: Vec<&str> = p.errors.iter().map(|e| e.1.as_str()).collect();
        assert!(said.iter().any(|e| e.contains("two widgets called b")), "{said:?}");
        assert!(said.iter().any(|e| e.contains("a: no end")), "{said:?}");
        assert_eq!(p.widgets[0].children.len(), 1, "what it held is kept all the same");
        let p = parse("end\nwidget s: strip(ch=t1)\n", Path::new("."));
        assert!(p.errors.iter().any(|e| e.0 == 1 && e.1.contains("end")), "{:?}", p.errors);
        assert!(p.errors.iter().any(|e| e.0 == 2), "an unknown kind is said: {:?}", p.errors);
    }

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
