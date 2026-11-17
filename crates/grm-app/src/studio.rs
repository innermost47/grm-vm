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

use std::collections::HashMap;

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
}

/// A song: its tracks, its echo's time (in beats), and its own name (her learning, fine-tuned to it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub title: String,
    pub tracks: Vec<Track>,
    pub echo_beats: f64,
    pub bpm: f64,
}

impl Default for Song {
    fn default() -> Self {
        Song {
            title: "first song".into(),
            tracks: vec![
                Track { id: 1, kind: Kind::Synth, label: "synth".into(), hers: false },
                Track { id: 2, kind: Kind::Synth, label: "her synth".into(), hers: true },
                Track { id: 3, kind: Kind::Drums, label: "drums".into(), hers: false },
            ],
            echo_beats: 0.75,
            bpm: 120.0,
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
        self.tracks.push(Track { id, kind, label: label.into(), hers: false });
    }

    /// The app's file, written from the song.
    pub fn grm(&self) -> String {
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
            "synth",
            "drums",
            "echo",
            "reverb",
        ] {
            a(format!("use networks/{n}.grm"));
        }
        a("call transport(name=tr, step=tr_step, run=playing)".into());
        for t in &self.tracks {
            let n = format!("t{}", t.id);
            match t.kind {
                Kind::Synth => {
                    a(format!("call synth(name={n}, clock_i=tr_i, clock_f=tr_f, hers={})", if t.hers { 1 } else { 0 }))
                }
                Kind::Drums => a(format!("call drums(name={n}, clock_i=tr_i, clock_f=tr_f)")),
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
        let ids: Vec<String> = self.tracks.iter().map(|t| format!("t{}", t.id)).collect();
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
        a("widget roll: roll at 0 0 size 12 3 \"what is played: hers, the human's, the drums\"".into());
        let mut row = 3;
        for t in &self.tracks {
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
                        ("auto_on", "by ear", "ear"),
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
                    a(format!("widget {n}am: knob {n}_arp_mode 0 .. 3 at 8 {} size 1 2 \"arp mode\"", row + 6));
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

/// Her hand on a drum machine: for each drum she holds, at each bar she may move one step, kept when what she heard
/// after pleased her more (clearer, and the groove nearer its sweet spot: Witek et al., 2014, found the wish to move
/// and the pleasure highest at a middle amount of syncopation, an inverted U).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DrumHand {
    /// Her taste for a hit on each sixteenth, per drum (k, s, h, t).
    pub taste: HashMap<String, [f64; 16]>,
    /// The step she moved last, per track and drum: (node, its value before, what she heard before).
    #[serde(skip)]
    pending: HashMap<String, (String, f64, f64)>,
    #[serde(skip)]
    last_bar: i64,
    #[serde(skip)]
    seed: u64,
}

/// How syncopated a bar of sixteen steps is: hits on weak sixteenths not followed by one on the stronger next
/// (Longuet-Higgins and Lee's idea, simply), 0 .. 1.
pub fn syncopation(steps: &[bool; 16]) -> f64 {
    const WEIGHT: [f64; 16] = [4.0, 1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0];
    let mut s = 0.0;
    for k in 0..16 {
        let next = (k + 1) % 16;
        if steps[k] && !steps[next] && WEIGHT[next] > WEIGHT[k] {
            s += WEIGHT[next] - WEIGHT[k];
        }
    }
    (s / 12.0).min(1.0)
}

impl DrumHand {
    fn random(&mut self) -> f64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// One frame: at each new bar, what she moved is judged, and she may move another step of a drum she holds.
    /// Returns the steps to write (node, 0 or 1).
    pub fn tick(&mut self, bar: i64, held: &[(String, String, [bool; 16])], clarity: f64) -> Vec<(String, f64)> {
        if bar == self.last_bar {
            return vec![];
        }
        self.last_bar = bar;
        if self.seed == 0 {
            self.seed = 0x5851_f42d_4c95_7f2d;
        }
        let mut out = vec![];
        for (prefix, drum, steps) in held {
            let key = format!("{prefix}{drum}");
            let groove = 1.0 - (syncopation(steps) - 0.35).abs() * 2.0;
            let heard = clarity + 0.3 * groove;
            // judged: what she moved, by what she heard since, against before
            if let Some((node, before, was)) = self.pending.remove(&key) {
                let k: usize = node.rsplit("_s").next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1) - 1;
                let taste = self.taste.entry(drum.clone()).or_insert([0.0; 16]);
                let better = heard - was;
                let now_on = before < 0.5;
                taste[k.min(15)] += if now_on { 0.5 * better } else { -0.5 * better };
                if better < -0.02 {
                    out.push((node, before));
                }
            }
            // a step moved, now and then (one bar in two)
            if self.random() < 0.5 {
                let taste = self.taste.get(drum).copied().unwrap_or([0.0; 16]);
                let mut best = (f64::MIN, 0usize);
                for k in 0..16 {
                    let want_on = taste[k] + self.random() * 0.8 - 0.4;
                    let flip = if steps[k] { -want_on } else { want_on };
                    if flip > best.0 {
                        best = (flip, k);
                    }
                }
                let k = best.1;
                let node = format!("{prefix}{drum}_s{}", k + 1);
                let before = if steps[k] { 1.0 } else { 0.0 };
                out.push((node.clone(), 1.0 - before));
                self.pending.insert(key, (node, before, heard));
            }
        }
        out
    }
}

/// The studio's window: the transport at the top, the instruments (the drum machines' grids, then the knobs), the
/// mixer at the bottom.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    transport(app, ui, &ink);
    ui.separator();
    let mixer_h = 236.0;
    let middle = (ui.available_height() - mixer_h - 12.0).max(120.0);
    egui::ScrollArea::vertical().id_salt("studio-tracks").max_height(middle).auto_shrink([false, false]).show(
        ui,
        |ui| {
            let drums: Vec<Track> = app.song.tracks.iter().filter(|t| t.kind == Kind::Drums).cloned().collect();
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
                strip(app, ui, &t.label, &format!("t{}", t.id), &ink);
            }
            master(app, ui, &ink);
        });
    });
    let _ = lang;
}

fn value(app: &App, node: &str) -> f64 {
    app.picture.node(node).map(|n| n.value).unwrap_or(0.0)
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
                if r.clicked() && !hers {
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
fn strip(app: &mut App, ui: &mut egui::Ui, label: &str, n: &str, ink: &Ink) {
    let lang = app.settings.lang;
    egui::Frame::new().fill(ink.surface).corner_radius(3).inner_margin(6.0).show(ui, |ui| {
        ui.set_width(96.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(label).small().strong());
            ui.label(RichText::new(n).small().color(ink.muted));
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
        let text = song.grm();
        assert!(text.contains("call synth(name=t1, clock_i=tr_i, clock_f=tr_f, hers=0)"));
        assert!(text.contains("call synth(name=t2, clock_i=tr_i, clock_f=tr_f, hers=1)"));
        assert!(text.contains("call drums(name=t3"));
        assert!(text.contains("relate echo_send: echo_in = t1_echo * t1_on * t1_out"));
        assert!(text.contains("widget t1keys: keys t1_pitch t1_hold"));
    }

    #[test]
    fn a_groove_has_its_syncopation() {
        let mut four = [false; 16];
        for k in [0, 4, 8, 12] {
            four[k] = true;
        }
        assert_eq!(syncopation(&four), 0.0);
        let mut off = [false; 16];
        for k in [3, 7, 11, 15] {
            off[k] = true;
        }
        assert!(syncopation(&off) > 0.3);
    }
}
