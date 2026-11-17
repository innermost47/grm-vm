// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The studio: one app where the human adds instruments (synthesisers, drum machines), a transport that holds the
//! time for all of them (sticky at the top: bars going by, a loop), and a mixer at the bottom (a strip each: level,
//! pan, sends to one echo and one reverb shared by all, a meter). The instruments are tools, written in GRML
//! (`networks/synth.grm`, `networks/drums.grm`): she is not one of them. Each has its hands (`hand NAME when
//! SWITCH: ...`): the human gives her a part (a synthesiser, one drum of the machine) by its switch, and she plays it
//! through its hands only.
//!
//! The song (its tracks) is kept beside the memory; the app's file is written from it (`apps/studio.grm`) each time
//! a track comes or goes.

use eframe::egui::{self, pos2, vec2, RichText, Sense, Stroke};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::app::Ink;
use crate::text::t;

pub const APP: &str = "studio";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Synth,
    Drums,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: u32,
    pub kind: Kind,
    pub label: String,
    /// Hers when it is made (a synthesiser she plays from the start).
    #[serde(default)]
    pub hers: bool,
    /// On: it is in the song's network and sounds; off: it is not computed at all (it costs nothing).
    #[serde(default = "on")]
    pub power: bool,
}

fn on() -> bool {
    true
}

/// A song: its tracks, its echo's time (in beats), and its own name (her learning, fine-tuned to it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub title: String,
    pub tracks: Vec<Track>,
    pub echo_beats: f64,
    pub bpm: f64,
    /// Which song it is for her (1 .. 4: what she learned in it is kept apart).
    #[serde(default = "first")]
    pub number: u32,
    /// The names of those who played with her here (she keeps apart what she learned with each, nothing of them).
    #[serde(default)]
    pub humans: Vec<String>,
}

fn first() -> u32 {
    1
}

impl Default for Song {
    fn default() -> Self {
        Song {
            title: "first song".into(),
            tracks: vec![
                Track { id: 1, kind: Kind::Synth, label: "synth".into(), hers: false, power: true },
                Track { id: 2, kind: Kind::Synth, label: "her synth".into(), hers: true, power: true },
                Track { id: 3, kind: Kind::Drums, label: "drums".into(), hers: false, power: true },
            ],
            echo_beats: 0.75,
            bpm: 120.0,
            number: 1,
            humans: vec![],
        }
    }
}

/// The drums: (letter, name).
pub const DRUMS: [(&str, &str); 4] = [("k", "kick"), ("s", "snare"), ("h", "hat"), ("t", "tom")];

impl Song {
    pub fn path(memory: &std::path::Path) -> std::path::PathBuf {
        memory.with_extension("studio.json")
    }

    pub fn load(memory: &std::path::Path) -> Song {
        std::fs::read_to_string(Self::path(memory)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, memory: &std::path::Path) {
        let _ = std::fs::write(Self::path(memory), serde_json::to_string_pretty(self).unwrap_or_default());
    }

    pub fn add(&mut self, kind: Kind) {
        let id = self.tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let label = match kind {
            Kind::Synth => "synth",
            Kind::Drums => "drums",
        };
        self.tracks.push(Track { id, kind, label: label.into(), hers: false, power: true });
    }

    /// Who plays with her now, as she knows them apart (1 .. 4; a name not given is "someone", the first).
    pub fn human(&mut self, name: &str) -> u32 {
        let name = if name.trim().is_empty() { "someone" } else { name.trim() };
        if let Some(k) = self.humans.iter().position(|h| h == name) {
            return k as u32 + 1;
        }
        if self.humans.len() < 4 {
            self.humans.push(name.to_string());
            return self.humans.len() as u32;
        }
        4
    }

    /// The app's file, written from the song (`human`: who plays with her, as she knows them apart).
    pub fn grm(&self, human: u32) -> String {
        let mut l: Vec<String> = vec![];
        let mut a = |s: String| l.push(s);
        a("app Studio".into());
        a("icon audio-lines".into());
        a("# Written by the studio from its song: its tracks are changed there (+ synth, + drums), not here.".into());
        a(format!("# Song: {}", self.title));
        a(format!("input bpm = {} positive", self.bpm));
        a("input playing = 1".into());
        a("input metro_on = 0".into());
        a("input metro_level = 0.3".into());
        a("input master = 0.8".into());
        a("input echo_back = 0.35".into());
        a("input echo_return = 0.5".into());
        a("input room = 0.75".into());
        a("input rev_return = 0.6".into());
        // where she is: this song, this human (what she learned in each is kept apart)
        a(format!("input elle_ctx_song = {}", self.number.clamp(1, 4)));
        a(format!("input elle_ctx_human = {}", human.clamp(1, 4)));
        // her lessons: on, unless the human turns them off
        a("input elle_lessons = 1".into());
        a("node tr_step".into());
        a("relate sixteenths: tr_step = 0.00000138888889 * bpm".into());
        for n in [
            "lfo",
            "saw",
            "square",
            "ladder",
            "adsr",
            "noise",
            "voice",
            "transport",
            "arpeggiator",
            "synth",
            "drums",
            "echo",
            "reverb",
        ] {
            a(format!("use networks/{n}.grm"));
        }
        a("call transport(name=tr, step=tr_step, run=playing)".into());
        // what she hears and what she plays, named before the instruments read them
        a("node elle_ear signed".into());
        a("var elle_pitch = 440 positive".into());
        a("var elle_gate = 0".into());
        for d in 1..=4 {
            a(format!("var elle_dhit{d} = 0"));
        }
        // what she plays sounds only while the song plays (stopped, her note and her strikes are let go)
        a("node elle_sounding signed".into());
        a("relate elle_while_playing: elle_sounding = playing * elle_gate".into());
        for d in 1..=4 {
            a(format!("node elle_strike{d} signed"));
            a(format!("relate elle_strike_while{d}: elle_strike{d} = playing * elle_dhit{d}"));
        }
        for t in self.tracks.iter().filter(|t| t.power) {
            let n = format!("t{}", t.id);
            match t.kind {
                Kind::Synth => {
                    a(format!(
                        "call synth(name={n}, clock_i=tr_i, clock_f=tr_f, hers={}, her_pitch=elle_pitch, her_gate=elle_sounding)",
                        if t.hers { 1 } else { 0 }
                    ))
                }
                Kind::Drums => a(format!(
                    "call drums(name={n}, clock_i=tr_i, clock_f=tr_f, her_k=elle_strike1, her_s=elle_strike2, her_h=elle_strike3, her_t=elle_strike4)"
                )),
            }
            // its strip on the mixer: level, pan (0 left .. 1 right), sends, on
            let level = if t.kind == Kind::Drums { 0.9 } else { 0.6 };
            a(format!("input {n}_level = {level}"));
            a(format!("input {n}_pan = {}", if t.hers { 0.62 } else { 0.42 }));
            a(format!("input {n}_echo = {}", if t.kind == Kind::Synth { 0.2 } else { 0 as f64 }));
            a(format!("input {n}_rev = {}", if t.kind == Kind::Synth { 0.25 } else { 0.08 }));
            a(format!("input {n}_on = 1"));
            a(format!("node {n}_l signed"));
            a(format!("relate {n}_left: {n}_l = 1.4 * {n}_on * {n}_level * {n}_out - 1.4 * {n}_on * {n}_level * {n}_pan * {n}_out"));
            a(format!("node {n}_r signed"));
            a(format!("relate {n}_right: {n}_r = 1.4 * {n}_on * {n}_level * {n}_pan * {n}_out"));
        }
        // her: the same network in every app; here she hears the song, sees the first synthesiser's keys
        a("use ../mind/elle.grm".into());
        let other =
            self.tracks.iter().find(|t| t.kind == Kind::Synth && !t.hers && t.power).map(|t| format!("t{}", t.id));
        let (op, og) = match &other {
            Some(o) => (format!("{o}_pitch"), format!("{o}_hold")),
            None => ("elle_none_pitch".into(), "elle_none_gate".into()),
        };
        if other.is_none() {
            a("input elle_none_pitch = 220 positive".into());
            a("input elle_none_gate = 0".into());
        }
        a(format!(
            "call elle(name=elle, ear=elle_ear, clock_i=tr_i, clock_f=tr_f, other_pitch={op}, other_gate={og}, ctx_song=elle_ctx_song, ctx_human=elle_ctx_human, lessons=elle_lessons)"
        ));
        let ids: Vec<String> = self.tracks.iter().filter(|t| t.power).map(|t| format!("t{}", t.id)).collect();
        let sum = |f: &dyn Fn(&str) -> String| -> String {
            if ids.is_empty() {
                "0".into()
            } else {
                ids.iter().map(|n| f(n)).collect::<Vec<_>>().join(" + ")
            }
        };
        // the sends: one echo, one reverb, for every track (each sends what it wants to them)
        a("node echo_in signed".into());
        a(format!("relate echo_send: echo_in = {}", sum(&|n| format!("{n}_echo * {n}_on * {n}_out"))));
        let seconds = (self.echo_beats * 60.0 / self.bpm.max(10.0)).clamp(0.02, 2.0);
        a(format!("call echo(name=fxe, input=echo_in, time={seconds:.4}, feedback=echo_back)"));
        a("node rev_in signed".into());
        a(format!("relate rev_send: rev_in = {} + 0.3 * fxe_wet", sum(&|n| format!("{n}_rev * {n}_on * {n}_out"))));
        a("call reverb(name=fxr, input=rev_in, decay=room)".into());
        // what she hears: every track and the returns, before the master
        a("node elle_ear signed".into());
        a(format!(
            "relate elle_hearing: elle_ear = {} + 0.5 * fxe_wet + 0.5 * fxr_out",
            sum(&|n| format!("{n}_l + {n}_r"))
        ));
        a("node out signed".into());
        a(format!(
            "relate master_left: out = {} + master * echo_return * fxe_wet + master * rev_return * fxr_out + metro_on * metro_level * tr_click",
            sum(&|n| format!("master * {n}_l"))
        ));
        a("node out_r signed".into());
        a(format!(
            "relate master_right: out_r = {} + master * echo_return * fxe_wet + master * rev_return * fxr_out + metro_on * metro_level * tr_click",
            sum(&|n| format!("master * {n}_r"))
        ));
        // the surface: what she plays, then each instrument's knobs (the transport and the mixer are the window's)
        a("widget roll: roll at 0 0 size 12 3 \"what she plays\"".into());
        let mut row = 3;
        for t in self.tracks.iter().filter(|t| t.power) {
            let n = format!("t{}", t.id);
            match t.kind {
                Kind::Synth => {
                    a(format!(
                        "widget {n}sec: section at 0 {row} size 12 12 \"{} · t{}\" icon audio-waveform",
                        t.label, t.id
                    ));
                    let k1 = [
                        ("shape", "0 .. 1", "saw · square"),
                        ("o1_level", "0 .. 1", "osc 1"),
                        ("o2_level", "0 .. 1", "osc 2"),
                        ("o2_ratio", "0.25 .. 4", "2: tune"),
                        ("o3_level", "0 .. 1", "osc 3"),
                        ("o3_ratio", "0.5 .. 8", "3: ratio"),
                        ("fm", "0 .. 1", "fm"),
                        ("noise", "0 .. 1", "noise"),
                        ("cutoff", "0.005 .. 0.32", "cutoff"),
                        ("env_amount", "0 .. 0.3", "env"),
                        ("resonance", "0 .. 3.5", "resonance"),
                        ("glide", "0.001 .. 2", "glide"),
                    ];
                    let k2 = [
                        ("aa_time", "0.001 .. 4", "attack"),
                        ("ad_time", "0.001 .. 4", "decay"),
                        ("a_sustain", "0 .. 1", "sustain"),
                        ("ar_time", "0.001 .. 4", "release"),
                        ("fa_time", "0.001 .. 4", "f attack"),
                        ("fd_time", "0.001 .. 4", "f decay"),
                        ("f_sustain", "0 .. 1", "f sustain"),
                        ("fr_time", "0.001 .. 4", "f release"),
                        ("pitch_env", "0 .. 4", "to pitch"),
                        ("lfo_rate", "0.05 .. 12", "lfo speed"),
                        ("lfo_cut", "0 .. 0.5", "lfo filter"),
                        ("lfo_pitch", "0 .. 0.03", "vibrato"),
                    ];
                    for (k, (node, range, words)) in k1.iter().enumerate() {
                        a(format!("widget {n}a{k}: knob {n}_{node} {range} at {k} {} size 1 2 \"{words}\"", row + 1));
                    }
                    for (k, (node, range, words)) in k2.iter().enumerate() {
                        a(format!("widget {n}b{k}: knob {n}_{node} {range} at {k} {} size 1 2 \"{words}\"", row + 3));
                    }
                    let switches = [
                        ("hers", "hers", "hand"),
                        ("seq_on", "sequencer", "layout-grid"),
                        ("arp_on", "arpeggiator", "list-music"),
                        ("arp_latch", "latch", "lock"),
                        ("seq_exact", "exact notes", "magnet"),
                    ];
                    for (k, (node, words, icon)) in switches.iter().enumerate() {
                        a(format!(
                            "widget {n}w{k}: switch {n}_{node} at {} {} size 2 1 \"{words}\" icon {icon}",
                            2 * k,
                            row + 5
                        ));
                    }
                    for k in 0..8 {
                        a(format!(
                            "widget {n}st{k}: step {n}_s_n{} {n}_at 55 .. 880 at {k} {} size 1 3 \"{}\"",
                            k + 1,
                            row + 6,
                            k + 1
                        ));
                        a(format!("widget {n}so{k}: switch {n}_s_on{} at {k} {} size 1 1 \"on\"", k + 1, row + 9));
                    }
                    a(format!("widget {n}am: knob {n}_arp_mode 0 .. 1 at 8 {} size 1 2 \"up · down\"", row + 6));
                    a(format!("widget {n}ao: knob {n}_arp_oct 1 .. 3 at 9 {} size 1 2 \"octaves\"", row + 6));
                    a(format!("widget {n}ar: knob {n}_arp_rate 0.25 .. 4 at 10 {} size 1 2 \"arp speed\"", row + 6));
                    a(format!("widget {n}la: knob {n}_lfo_amp 0 .. 1 at 11 {} size 1 2 \"tremolo\"", row + 6));
                    a(format!("widget {n}keys: keys {n}_pitch {n}_hold at 0 {} size 12 2", row + 10));
                    row += 12;
                }
                Kind::Drums => {
                    a(format!("widget {n}sec: section at 0 {row} size 12 4 \"{} · t{}\" icon drum", t.label, t.id));
                    for (k, (x, name)) in DRUMS.iter().enumerate() {
                        let (tlo, thi) = match *x {
                            "k" => ("30", "120"),
                            "s" => ("100", "400"),
                            "h" => ("0.5", "2"),
                            _ => ("60", "300"),
                        };
                        a(format!(
                            "widget {n}{x}l: knob {n}_{x}_level 0 .. 1 at {} {} size 1 2 \"{name}\"",
                            3 * k,
                            row + 1
                        ));
                        a(format!(
                            "widget {n}{x}t: knob {n}_{x}_tune {tlo} .. {thi} at {} {} size 1 2 \"pitch\"",
                            3 * k + 1,
                            row + 1
                        ));
                        a(format!(
                            "widget {n}{x}d: knob {n}_{x}_decay 0.01 .. 1.5 at {} {} size 1 2 \"decay\"",
                            3 * k + 2,
                            row + 1
                        ));
                        a(format!(
                            "widget {n}{x}w: switch {n}_{x}_hers at {} {} size 3 1 \"hers: {name}\" icon hand",
                            3 * k,
                            row + 3
                        ));
                    }
                    row += 4;
                }
            }
        }
        l.join("\n") + "\n"
    }
}

/// The studio's window: the transport at the top, the instruments (the drum machines' grids, then the knobs), the
/// mixer at the bottom.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    transport(app, ui, &ink);
    ui.separator();
    her(app, ui, &ink);
    ui.separator();
    let mixer_h = 236.0;
    let middle = (ui.available_height() - mixer_h - 12.0).max(120.0);
    egui::ScrollArea::vertical().id_salt("studio-tracks").max_height(middle).auto_shrink([false, false]).show(
        ui,
        |ui| {
            let drums: Vec<Track> =
                app.song.tracks.iter().filter(|t| t.kind == Kind::Drums && t.power).cloned().collect();
            for t in drums {
                drum_grid(app, ui, &t, &ink);
                ui.add_space(6.0);
            }
            crate::surface::show(app, ui, APP);
        },
    );
    ui.separator();
    egui::ScrollArea::horizontal().id_salt("studio-mixer").show(ui, |ui| {
        ui.horizontal(|ui| {
            let tracks = app.song.tracks.clone();
            for t in &tracks {
                strip(app, ui, t, &ink);
            }
            master(app, ui, &ink);
        });
    });
    let _ = lang;
}

fn value(app: &App, node: &str) -> f64 {
    app.picture.node(node).map(|n| n.value).unwrap_or(0.0)
}

/// Her network as it is now (read from the sound): what she feels, and each of her wants, the one she holds lit.
/// Nothing here is computed by the window: these are her nodes.
fn her(app: &mut App, ui: &mut egui::Ui, ink: &Ink) {
    let lang = app.settings.lang;
    let mixer = app.mixer.clone();
    let read = |n: &str| mixer.lock().ok().and_then(|m| m.value("out", n));
    let Some(choice) = read("elle_choice") else {
        ui.label(RichText::new(t(lang, "elle_asleep")).small().color(ink.muted));
        return;
    };
    ui.horizontal(|ui| {
        crate::icons::show(ui, "brain", 14.0, ink.accent);
        ui.label(RichText::new(t(lang, "elle_name")).strong());
        ui.add_space(8.0);
        for (node, key, lo, hi) in [
            ("elle_pleasure", "elle_pleasure", -1.0, 1.0),
            ("elle_sur", "elle_surprise", 0.0, 1.0),
            ("elle_unc", "elle_uncertainty", 0.0, 1.0),
            ("elle_bore", "elle_boredom", 0.0, 1.0),
            ("elle_curious", "elle_curiosity", -0.3, 0.3),
            ("elle_clar", "elle_clarity", 0.0, 0.5),
        ] {
            let v = read(node).unwrap_or(0.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(t(lang, key)).size(9.0).color(ink.muted));
                let (rect, r) = ui.allocate_exact_size(vec2(70.0, 6.0), Sense::hover());
                ui.painter().rect_filled(rect, 2.0, ink.panel);
                let f = ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
                let bar = egui::Rect::from_min_size(rect.min, vec2(rect.width() * f, rect.height()));
                ui.painter().rect_filled(bar, 2.0, ink.accent.gamma_multiply(0.8));
                r.on_hover_text(format!("{node} = {v:.3}"));
            });
        }
        ui.add_space(12.0);
        // her lesson: which, and whether she has lessons (the human's switch)
        let on = read("elle_lessons").unwrap_or(1.0) > 0.5;
        let mut keep = on;
        if ui.checkbox(&mut keep, t(lang, "elle_lessons")).on_hover_text(t(lang, "elle_lessons_d")).changed() {
            app.write("elle_lessons", if keep { 1.0 } else { 0.0 });
        }
        if on {
            let k = read("elle_lesson").unwrap_or(1.0).round().clamp(1.0, 6.0) as usize;
            let key = ["lesson_e1", "lesson_e2", "lesson_e3", "lesson_e4", "lesson_e5", "lesson_e6"][k - 1];
            let time = read("elle_ltime").unwrap_or(0.0);
            ui.label(RichText::new(format!("{k}/6 · {}", t(lang, key))).small().color(ink.accent))
                .on_hover_text(format!("{} ({time:.0})", t(lang, "elle_lesson_d")));
        }
        ui.add_space(12.0);
        // her wants: fifteen intervals (-7 .. +7 semitones) and a rest
        let wants: Vec<f64> = (1..=16).map(|k| read(&format!("elle_want{k}")).unwrap_or(0.0)).collect();
        let lo = wants.iter().cloned().fold(f64::MAX, f64::min);
        let hi = wants.iter().cloned().fold(f64::MIN, f64::max).max(lo + 1e-6);
        let (rect, r) = ui.allocate_exact_size(vec2(16.0 * 9.0, 26.0), Sense::hover());
        for (k, w) in wants.iter().enumerate() {
            let h = 4.0 + 20.0 * ((w - lo) / (hi - lo)) as f32;
            let x = rect.left() + k as f32 * 9.0;
            let col = if (choice.round() as usize) == k + 1 { ink.accent } else { ink.muted.gamma_multiply(0.6) };
            ui.painter().rect_filled(
                egui::Rect::from_min_max(pos2(x + 1.0, rect.bottom() - h), pos2(x + 8.0, rect.bottom())),
                1.5,
                col,
            );
        }
        let hover: Vec<String> = wants
            .iter()
            .enumerate()
            .map(|(k, w)| if k == 15 { format!("rest {w:.2}") } else { format!("{:+} {w:.2}", k as i64 - 7) })
            .collect();
        r.on_hover_text(format!("{}\n{}", t(lang, "elle_wants"), hover.join("  ")));
    });
}

/// The transport: play or stop, the tempo, where it is (bar and beat), the click, the echo's time, the tracks.
fn transport(app: &mut App, ui: &mut egui::Ui, ink: &Ink) {
    let lang = app.settings.lang;
    ui.horizontal_wrapped(|ui| {
        let sounding = app.mixer.lock().map(|m| m.playing().iter().any(|p| p == "out")).unwrap_or(false);
        let running = value(app, "playing") > 0.5;
        let on = sounding && running;
        let (icon, word) = if on { ("square", t(lang, "stop")) } else { ("play", t(lang, "play")) };
        if crate::icons::button(ui, icon, word).clicked() {
            if on {
                app.write("playing", 0.0);
            } else {
                if !sounding {
                    app.say("play out");
                }
                app.write("playing", 1.0);
            }
        }
        // where it is: the bar, the beat
        let (i, bar) = app
            .mixer
            .lock()
            .ok()
            .and_then(|m| Some((m.value("out", "tr_i")?, m.value("out", "tr_bar")?)))
            .unwrap_or((0.0, 0.0));
        ui.label(
            RichText::new(format!("{:>3} . {}", bar as i64 + 1, (i / 4.0).floor() as i64 + 1))
                .monospace()
                .size(18.0)
                .color(ink.accent),
        );
        ui.separator();
        // the tempo: only once the machine has it (a drag clamped on nothing would write its least)
        let mut bpm = app.picture.node("bpm").map(|n| n.value).unwrap_or(app.song.bpm);
        let drag = egui::DragValue::new(&mut bpm).range(30.0..=240.0).clamp_existing_to_range(false).speed(0.3);
        if ui.add(drag.suffix(" bpm")).changed() && app.picture.node("bpm").is_some() {
            app.write("bpm", bpm.round());
            app.song.bpm = bpm.round();
        }
        let mut click = value(app, "metro_on") > 0.5;
        if ui.checkbox(&mut click, t(lang, "metronome")).changed() {
            app.write("metro_on", if click { 1.0 } else { 0.0 });
        }
        let mut level = value(app, "metro_level");
        if ui
            .add(egui::Slider::new(&mut level, 0.0..=1.0).show_value(false))
            .on_hover_text(t(lang, "click_level"))
            .changed()
        {
            app.write("metro_level", level);
        }
        ui.separator();
        // the echo's time, in beats (written into the song's file)
        let beats = app.song.echo_beats;
        egui::ComboBox::from_id_salt("echo-time")
            .selected_text(format!("{} {}", t(lang, "echo"), beats_name(beats)))
            .show_ui(ui, |ui| {
                for b in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0] {
                    if ui.selectable_label(b == beats, beats_name(b)).clicked() {
                        app.song.echo_beats = b;
                        app.studio_rewrite();
                    }
                }
            });
        ui.separator();
        if crate::icons::button(ui, "plus", t(lang, "add_synth")).clicked() {
            app.song.add(Kind::Synth);
            app.studio_rewrite();
        }
        if crate::icons::button(ui, "plus", t(lang, "add_drums")).clicked() {
            app.song.add(Kind::Drums);
            app.studio_rewrite();
        }
        if crate::icons::button(ui, "rotate-ccw", t(lang, "reset_sound"))
            .on_hover_text(t(lang, "reset_sound_d"))
            .clicked()
        {
            app.reset_sound();
        }
    });
}

fn beats_name(b: f64) -> &'static str {
    match (b * 4.0).round() as i64 {
        1 => "1/16",
        2 => "1/8",
        3 => "1/8.",
        4 => "1/4",
        6 => "1/4.",
        _ => "1/2",
    }
}

/// A drum machine's grid: a row per drum, sixteen steps, the one playing lit; the human's pads beside; a drum she
/// holds is hers (its steps move by her: the human sees them, cannot touch them).
fn drum_grid(app: &mut App, ui: &mut egui::Ui, track: &Track, ink: &Ink) {
    let lang = app.settings.lang;
    let n = format!("t{}", track.id);
    let now_i = app.mixer.lock().ok().and_then(|m| m.value("out", "tr_i")).unwrap_or(-1.0).round() as i64;
    ui.label(RichText::new(format!("{} · {n}", track.label)).strong());
    for (x, name) in DRUMS {
        let hers = value(app, &format!("{n}_{x}_hers")) > 0.5;
        ui.horizontal(|ui| {
            ui.add_sized([70.0, 22.0], egui::Label::new(RichText::new(name).small()));
            // the human's pad: it sounds while pressed
            let pad = format!("{n}_{x}_pad");
            let (rect, r) = ui.allocate_exact_size(vec2(44.0, 22.0), Sense::click_and_drag());
            let down = r.is_pointer_button_down_on();
            ui.painter().rect_filled(rect, 3.0, if down { ink.accent } else { ink.surface });
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0_f32, ink.faint), egui::StrokeKind::Inside);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "pad",
                egui::FontId::proportional(10.0),
                ink.text,
            );
            let was = value(app, &pad) > 0.5;
            if down != was {
                app.write(&pad, if down { 1.0 } else { 0.0 });
            }
            ui.add_space(6.0);
            for k in 0..16 {
                let node = format!("{n}_{x}_s{}", k + 1);
                let on = value(app, &node) > 0.5;
                let (rect, r) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
                let beat = k % 4 == 0;
                let fill = if on {
                    if hers {
                        ink.accent.gamma_multiply(0.75)
                    } else {
                        ink.accent
                    }
                } else if beat {
                    ink.surface.gamma_multiply(1.6)
                } else {
                    ink.surface
                };
                ui.painter().rect_filled(rect.shrink(1.5), 3.0, fill);
                if k as i64 == now_i {
                    ui.painter().rect_stroke(
                        rect.shrink(0.5),
                        3.0,
                        Stroke::new(1.5_f32, ink.text),
                        egui::StrokeKind::Inside,
                    );
                }
                let r = if hers { r.on_hover_text(t(lang, "drum_hers")) } else { r };
                if r.clicked() {
                    app.write(&node, if on { 0.0 } else { 1.0 });
                }
                if k % 4 == 3 {
                    ui.add_space(4.0);
                }
            }
            ui.add_space(6.0);
            if hers {
                crate::icons::show(ui, "hand", 12.0, ink.accent);
                ui.label(RichText::new(t(lang, "hers_word")).small().color(ink.muted));
            }
        });
    }
}

/// A strip of the mixer: its meter, its fader, its pan, its sends, on or off.
fn strip(app: &mut App, ui: &mut egui::Ui, track: &Track, ink: &Ink) {
    let lang = app.settings.lang;
    let (label, n) = (track.label.as_str(), format!("t{}", track.id));
    let n = n.as_str();
    egui::Frame::new().fill(ink.surface).corner_radius(3).inner_margin(6.0).show(ui, |ui| {
        ui.set_width(96.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                // its power: off, it is not computed at all
                let colour = if track.power { ink.accent } else { ink.muted };
                let r = ui
                    .add(egui::Button::image(crate::icons::image("power", 12.0, colour)).frame(false))
                    .on_hover_text(t(lang, if track.power { "power_off" } else { "power_on" }));
                if r.clicked() {
                    if let Some(x) = app.song.tracks.iter_mut().find(|x| x.id == track.id) {
                        x.power = !x.power;
                    }
                    app.studio_rewrite();
                }
                ui.label(RichText::new(label).small().strong());
            });
            ui.label(RichText::new(n).small().color(ink.muted));
            if !track.power {
                ui.label(RichText::new(t(lang, "powered_off")).small().color(ink.muted));
                return;
            }
            ui.horizontal(|ui| {
                // the meter: what passes, slowly (it rises and falls, it does not flash)
                let level =
                    app.mixer.lock().ok().and_then(|m| m.value("out", &format!("{n}_out"))).unwrap_or(0.0).abs();
                let vu = app.studio_vu.entry(n.to_string()).or_insert(0.0);
                *vu = (*vu * 0.9).max(level as f32);
                let shown = *vu;
                let (rect, _) = ui.allocate_exact_size(vec2(10.0, 110.0), Sense::hover());
                ui.painter().rect_filled(rect, 2.0, ink.panel);
                let h = rect.height() * shown.clamp(0.0, 1.0);
                let bar = egui::Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.max);
                ui.painter().rect_filled(bar, 2.0, if shown > 0.9 { ink.amber } else { ink.accent });
                let mut level = value(app, &format!("{n}_level"));
                if ui.add(egui::Slider::new(&mut level, 0.0..=1.0).vertical().show_value(false)).changed() {
                    app.write(&format!("{n}_level"), level);
                }
                ui.vertical(|ui| {
                    for (k, word) in [("pan", "pan"), ("echo", "echo"), ("rev", "rev")] {
                        let mut v = value(app, &format!("{n}_{k}"));
                        ui.label(RichText::new(word).size(9.0).color(ink.muted));
                        if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false)).changed() {
                            app.write(&format!("{n}_{k}"), v);
                        }
                    }
                });
            });
            let mut on = value(app, &format!("{n}_on")) > 0.5;
            if ui.checkbox(&mut on, t(lang, "on_word")).changed() {
                app.write(&format!("{n}_on"), if on { 1.0 } else { 0.0 });
            }
        });
    });
}

fn master(app: &mut App, ui: &mut egui::Ui, ink: &Ink) {
    let lang = app.settings.lang;
    egui::Frame::new().fill(ink.surface).corner_radius(3).inner_margin(6.0).show(ui, |ui| {
        ui.set_width(150.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(t(lang, "master")).small().strong());
            for (node, word) in [
                ("master", "master"),
                ("echo_return", "echo"),
                ("echo_back", "echo feedback"),
                ("rev_return", "reverb"),
                ("room", "room"),
            ] {
                let mut v = value(app, node);
                ui.label(RichText::new(word).size(9.0).color(ink.muted));
                let hi = if node == "room" || node == "echo_back" { 0.97 } else { 1.0 };
                if ui.add(egui::Slider::new(&mut v, 0.0..=hi).show_value(false)).changed() {
                    app.write(node, v);
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_song_writes_an_app_with_its_tracks_and_their_hands() {
        let song = Song::default();
        let text = song.grm(1);
        assert!(text.contains(
            "call synth(name=t1, clock_i=tr_i, clock_f=tr_f, hers=0, her_pitch=elle_pitch, her_gate=elle_sounding"
        ));
        assert!(text.contains("call synth(name=t2, clock_i=tr_i, clock_f=tr_f, hers=1, her_pitch=elle_pitch"));
        assert!(text.contains("call elle(name=elle, ear=elle_ear"));
        assert!(text.contains("call drums(name=t3"));
        assert!(text.contains("relate echo_send: echo_in = t1_echo * t1_on * t1_out"));
        assert!(text.contains("widget t1keys: keys t1_pitch t1_hold"));
    }
}
