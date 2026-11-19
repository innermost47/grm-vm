// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! MIDI in: every controller plugged is listened to (as many as there are), and anything of the machine can be
//! bound to a knob, a fader, a pad or a key by learning it (MIDI learn): a node (a slider of an app, a parameter of
//! a network, music or not), the keys of an instrument (a pitch and a gate), or a line said at the prompt (panic,
//! sleep, a tab...). What comes in is an observation like any other: the network learns from it too.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use serde::{Deserialize, Serialize};

/// What a controller sent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Signal {
    Cc { ch: u8, num: u8, value: u8 },
    NoteOn { ch: u8, note: u8, velocity: u8 },
    NoteOff { ch: u8, note: u8 },
}

/// What a binding listens to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Control {
    /// A knob or a fader.
    Cc { ch: u8, num: u8 },
    /// One pad or key.
    Note { ch: u8, num: u8 },
    /// Every key of a channel (a keyboard).
    Keys { ch: u8 },
}

/// What it drives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// A node, from lo (0) to hi (127).
    Node { name: String, lo: f64, hi: f64 },
    /// An instrument's keys: its pitch (Hz) and its gate (1 while a key is held).
    Keys { pitch: String, hold: String },
    /// A line said at the prompt (when the pad is struck, or the knob passes the middle).
    Line(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    /// The controller's name (empty: any).
    pub port: String,
    pub control: Control,
    pub target: Target,
}

impl Binding {
    /// In a few words, for a list.
    pub fn words(&self) -> String {
        let from = match &self.control {
            Control::Cc { ch, num } => format!("CC {num} (ch {})", ch + 1),
            Control::Note { ch, num } => format!("note {num} (ch {})", ch + 1),
            Control::Keys { ch } => format!("keys (ch {})", ch + 1),
        };
        let to = match &self.target {
            Target::Node { name, lo, hi } => {
                format!("{name} [{} .. {}]", grm_engine::lang::g(*lo), grm_engine::lang::g(*hi))
            }
            Target::Keys { pitch, hold } => format!("{pitch} · {hold}"),
            Target::Line(l) => format!("› {l}"),
        };
        if self.port.is_empty() {
            format!("{from} → {to}")
        } else {
            format!("{} · {from} → {to}", self.port)
        }
    }
}

/// A message read: what it is (None: what is not listened to: clock, sysex...).
pub fn parse(msg: &[u8]) -> Option<Signal> {
    let (&status, rest) = msg.split_first()?;
    let ch = status & 0x0f;
    match (status & 0xf0, rest) {
        (0xb0, [num, value, ..]) => Some(Signal::Cc { ch, num: *num, value: *value }),
        (0x90, [note, 0, ..]) => Some(Signal::NoteOff { ch, note: *note }),
        (0x90, [note, velocity, ..]) => Some(Signal::NoteOn { ch, note: *note, velocity: *velocity }),
        (0x80, [note, ..]) => Some(Signal::NoteOff { ch, note: *note }),
        _ => None,
    }
}

/// A key's pitch, in Hz (A4 = 69 = 440 Hz).
pub fn hz(note: u8) -> f64 {
    440.0 * 2f64.powf((note as f64 - 69.0) / 12.0)
}

/// What a signal does under the bindings: the nodes to observe (their latest value) and the lines to say.
pub fn apply(
    bindings: &[Binding],
    port: &str,
    s: Signal,
    held: &mut HashMap<String, Vec<u8>>,
) -> (Vec<(String, f64)>, Vec<String>) {
    let mut nodes = vec![];
    let mut lines = vec![];
    for b in bindings.iter().filter(|b| b.port.is_empty() || b.port == port) {
        match (&b.control, s, &b.target) {
            (Control::Cc { ch, num }, Signal::Cc { ch: c, num: n, value }, t) if *ch == c && *num == n => match t {
                Target::Node { name, lo, hi } => nodes.push((name.clone(), lo + (hi - lo) * value as f64 / 127.0)),
                Target::Line(l) if value >= 64 => lines.push(l.clone()),
                _ => {}
            },
            (Control::Note { ch, num }, Signal::NoteOn { ch: c, note, .. }, t) if *ch == c && *num == note => match t {
                Target::Node { name, hi, .. } => nodes.push((name.clone(), *hi)),
                Target::Line(l) => lines.push(l.clone()),
                _ => {}
            },
            (Control::Note { ch, num }, Signal::NoteOff { ch: c, note }, Target::Node { name, lo, .. })
                if *ch == c && *num == note =>
            {
                nodes.push((name.clone(), *lo))
            }
            (Control::Keys { ch }, Signal::NoteOn { ch: c, note, .. }, Target::Keys { pitch, hold }) if *ch == c => {
                let keys = held.entry(hold.clone()).or_default();
                keys.retain(|k| *k != note);
                keys.push(note);
                nodes.push((pitch.clone(), hz(note)));
                nodes.push((hold.clone(), 1.0));
            }
            (Control::Keys { ch }, Signal::NoteOff { ch: c, note }, Target::Keys { pitch, hold }) if *ch == c => {
                let keys = held.entry(hold.clone()).or_default();
                keys.retain(|k| *k != note);
                // the last key still held sounds again; none: the gate closes
                match keys.last() {
                    Some(k) => nodes.push((pitch.clone(), hz(*k))),
                    None => nodes.push((hold.clone(), 0.0)),
                }
            }
            _ => {}
        }
    }
    (nodes, lines)
}

/// The binding a signal makes when something is being learned.
pub fn learn(port: &str, s: Signal, target: &Target) -> Option<Binding> {
    let control = match (s, target) {
        (Signal::NoteOn { ch, .. }, Target::Keys { .. }) => Control::Keys { ch },
        (_, Target::Keys { .. }) => return None,
        (Signal::Cc { ch, num, .. }, _) => Control::Cc { ch, num },
        (Signal::NoteOn { ch, note, .. }, _) => Control::Note { ch, num: note },
        (Signal::NoteOff { .. }, _) => return None,
    };
    Some(Binding { port: port.to_string(), control, target: target.clone() })
}

/// Every controller plugged, listened to.
pub struct Midi {
    pub ports: Vec<String>,
    connections: Vec<midir::MidiInputConnection<()>>,
    tx: Sender<(String, Signal)>,
    rx: Receiver<(String, Signal)>,
    pub error: Option<String>,
    /// Every port the system shows, listened to or not (a change: listened to again).
    seen: Vec<String>,
    looked: std::time::Instant,
}

impl Midi {
    pub fn start() -> Midi {
        let (tx, rx) = channel();
        let mut m = Midi {
            ports: vec![],
            connections: vec![],
            tx,
            rx,
            error: None,
            seen: vec![],
            looked: std::time::Instant::now(),
        };
        m.rescan();
        m
    }

    /// Every two seconds, the ports the system shows: one plugged or unplugged since, and all are listened to again
    /// (a keyboard plugged in USB once the window is open is heard).
    pub fn watch(&mut self) {
        if self.looked.elapsed() < std::time::Duration::from_secs(2) {
            return;
        }
        self.looked = std::time::Instant::now();
        let Ok(probe) = midir::MidiInput::new("grm-vm-probe") else { return };
        let names: Vec<String> = probe.ports().iter().filter_map(|p| probe.port_name(p).ok()).collect();
        if names != self.seen {
            self.rescan();
        }
    }

    /// Listens again to every controller there is now (one plugged since is heard).
    pub fn rescan(&mut self) {
        self.connections.clear();
        self.ports.clear();
        self.error = None;
        let probe = match midir::MidiInput::new("grm-vm") {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let names: Vec<String> = probe.ports().iter().filter_map(|p| probe.port_name(p).ok()).collect();
        self.seen = names.clone();
        self.looked = std::time::Instant::now();
        let mut busy = vec![];
        for (k, name) in names.into_iter().enumerate() {
            let Ok(input) = midir::MidiInput::new("grm-vm") else { continue };
            let ports = input.ports();
            let Some(port) = ports.get(k) else { continue };
            let tx = self.tx.clone();
            let who = name.clone();
            match input.connect(
                port,
                "grm-vm-in",
                move |_, msg, _| {
                    if let Some(s) = parse(msg) {
                        let _ = tx.send((who.clone(), s));
                    }
                },
                (),
            ) {
                Ok(c) => {
                    self.connections.push(c);
                    self.ports.push(name);
                }
                // on Windows a port is one program's at a time: another (a DAW) may hold it
                Err(e) => busy.push(format!("{name}: {e}")),
            }
        }
        if !busy.is_empty() {
            self.error =
                Some(format!("{} — {}", busy.join(" · "), "busy: another program may hold it (close it, then wait)"));
        }
    }

    /// What came since the last look.
    pub fn drain(&self) -> Vec<(String, Signal)> {
        self.rx.try_iter().collect()
    }
}

/// The MIDI part of the settings: the controllers heard, what each drives, and learning.
pub fn settings(app: &mut crate::app::App, ui: &mut eframe::egui::Ui) {
    use crate::text::t;
    use eframe::egui::RichText;
    let lang = app.settings.lang;
    let ink = crate::app::Ink::of(ui.ctx());
    ui.horizontal(|ui| {
        crate::icons::show(ui, "piano", 15.0, ink.accent);
        ui.label(RichText::new("MIDI").strong());
        if crate::icons::sign(ui, "refresh-cw", t(lang, "midi_rescan")).clicked() {
            app.midi.rescan();
        }
    });
    if app.midi.ports.is_empty() {
        ui.label(RichText::new(t(lang, "midi_none")).small().color(ink.muted));
    } else {
        ui.label(RichText::new(app.midi.ports.join(" · ")).small().color(ink.muted));
    }
    if let Some(e) = &app.midi.error {
        ui.label(RichText::new(e).small().color(ink.amber));
    }
    if let Some(target) = &app.midi_learn {
        let what = Binding { port: String::new(), control: Control::Keys { ch: 0 }, target: target.clone() }.words();
        let what = what.split("→ ").nth(1).unwrap_or("").to_string();
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(format!("{} {what}", t(lang, "midi_waiting"))).color(ink.accent));
            if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                app.midi_learn = None;
            }
        });
    }
    let mut gone = None;
    for (k, b) in app.settings.midi.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(b.words()).small());
            if crate::icons::sign(ui, "x", t(lang, "midi_unbind")).clicked() {
                gone = Some(k);
            }
        });
    }
    if let Some(k) = gone {
        app.settings.midi.remove(k);
    }
    ui.horizontal_wrapped(|ui| {
        if crate::icons::button(ui, "keyboard-music", t(lang, "midi_learn_keys")).clicked() {
            app.midi_learn = Some(Target::Keys { pitch: "pitch".into(), hold: "hold".into() });
        }
        for (sign, key, line) in [
            ("octagon-x", "midi_learn_panic", "panic"),
            ("moon", "midi_learn_sleep", "sleep"),
            ("step-forward", "midi_learn_tick", "tick"),
        ] {
            if crate::icons::button(ui, sign, t(lang, key)).clicked() {
                app.midi_learn = Some(Target::Line(line.into()));
            }
        }
    });
    ui.label(RichText::new(t(lang, "midi_d")).small().color(ink.muted));
}

/// The range a node is moved in by a knob: the slider's that shows it, else around its value.
pub fn range_of(app: &crate::app::App, node: &str) -> (f64, f64) {
    for a in &app.apps {
        for w in &a.widgets {
            if w.nodes.first().is_some_and(|n| n == node || a.prefix.get(n).is_some_and(|p| p == node)) && w.hi > w.lo {
                return (w.lo, w.hi);
            }
        }
    }
    match app.picture.node(node) {
        Some(n) if n.value > 0.0 && !n.signed => (0.0, 2.0 * n.value),
        Some(n) if n.value != 0.0 => (-2.0 * n.value.abs(), 2.0 * n.value.abs()),
        _ => (0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_knob_a_pad_and_a_keyboard() {
        assert_eq!(parse(&[0xb1, 7, 127]), Some(Signal::Cc { ch: 1, num: 7, value: 127 }));
        assert_eq!(parse(&[0x90, 60, 0]), Some(Signal::NoteOff { ch: 0, note: 60 }));
        assert!((hz(69) - 440.0).abs() < 1e-9 && (hz(81) - 880.0).abs() < 1e-9);
        let knob = learn(
            "",
            Signal::Cc { ch: 0, num: 74, value: 3 },
            &Target::Node { name: "cutoff".into(), lo: 0.0, hi: 0.5 },
        )
        .unwrap();
        let pad = learn("", Signal::NoteOn { ch: 9, note: 36, velocity: 90 }, &Target::Line("panic".into())).unwrap();
        let keys = learn(
            "",
            Signal::NoteOn { ch: 0, note: 60, velocity: 90 },
            &Target::Keys { pitch: "pitch".into(), hold: "hold".into() },
        )
        .unwrap();
        let all = vec![knob, pad, keys];
        let mut held = HashMap::new();
        let (n, _) = apply(&all, "x", Signal::Cc { ch: 0, num: 74, value: 127 }, &mut held);
        assert_eq!(n, vec![("cutoff".to_string(), 0.5)]);
        let (_, l) = apply(&all, "x", Signal::NoteOn { ch: 9, note: 36, velocity: 1 }, &mut held);
        assert_eq!(l, vec!["panic".to_string()]);
        let (n, _) = apply(&all, "x", Signal::NoteOn { ch: 0, note: 69, velocity: 1 }, &mut held);
        assert_eq!(n, vec![("pitch".to_string(), 440.0), ("hold".to_string(), 1.0)]);
        apply(&all, "x", Signal::NoteOn { ch: 0, note: 72, velocity: 1 }, &mut held);
        let (n, _) = apply(&all, "x", Signal::NoteOff { ch: 0, note: 72 }, &mut held);
        assert_eq!(n, vec![("pitch".to_string(), 440.0)], "the key still held sounds again");
        let (n, _) = apply(&all, "x", Signal::NoteOff { ch: 0, note: 69 }, &mut held);
        assert_eq!(n, vec![("hold".to_string(), 0.0)]);
    }
}
