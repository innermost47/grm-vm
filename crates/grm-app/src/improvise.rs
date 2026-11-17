// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Improvising by ear: when an instrument's sequencer is on and its `auto_on` node is 1, the machine writes the
//! next notes itself, and listens to what comes out. No scale is given: it starts from every interval alike, hears
//! how clear the sound is while each note plays (how periodic what comes out is: two notes that agree sound as one
//! clearer wave, two that clash do not; the echo and the room carry the note before into the next), and leans
//! towards the intervals that sounded clear, away from the ones that did not, a little at each step. What it learned
//! is shown, interval by interval: nothing is hidden.

use std::collections::{BTreeMap, VecDeque};

use crate::music::Teacher;

/// Intervals it may take, in semitones (an octave down to an octave up).
pub const SPAN: i32 = 12;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Ear {
    /// How much it likes each interval (index: interval + SPAN).
    pub taste: Vec<f64>,
    /// What it usually hears (the reward is measured against it).
    pub usual: f64,
    /// How clear what it hears now is (0 .. 1), and the clarity heard while the current step plays.
    pub clarity: f64,
    heard: Vec<f64>,
    /// The step the sequencer was on, the note written for each step (semitones from A1 = 55 Hz), and the
    /// interval that made it.
    last_step: Option<i64>,
    notes: [i32; 8],
    made_by: [Option<i32>; 8],
    /// The last steps judged, for the window: (interval, what it heard, the reward).
    pub judged: VecDeque<(i32, f64, f64)>,
    /// A knob she is playing: its gesture (node, from, to, when it starts, how long), and what she heard while it
    /// moved and after; kept if it sounded clearer, turned back if not.
    gesture: Option<(String, f64, f64, f64, f64)>,
    gesture_heard: Vec<f64>,
    /// What she did with the knobs lately, for the window: (node, kept or not).
    pub turned: VecDeque<(String, bool)>,
    /// The last note the human played (semitones from A1), and how many intervals she learned from them.
    human_note: Option<i32>,
    pub from_human: usize,
    /// How bright (the zero crossings, 0 .. 1) and how loud (its RMS) what she hears now is.
    pub brightness: f64,
    pub loudness: f64,
    /// What she knows of each knob: how clarity, brightness and loudness change when it turns (a whole turn), as
    /// she heard it when she or the human turned it.
    pub knows: BTreeMap<String, [f64; 3]>,
    /// Where the human likes each knob (0 .. 1 of its turn), from watching them turn it: their lessons.
    pub prefer: BTreeMap<String, f64>,
    /// How many times she watched the human turn a knob.
    pub taught: usize,
    /// A knob the human turns now: (node, from, to, what she heard before, when it last moved).
    #[serde(skip)]
    watching: Option<(String, f64, f64, [f64; 3], f64)>,
    #[serde(skip)]
    gesture_before: [f64; 3],
    /// The knob she studies (in a lesson on the synthesiser).
    #[serde(skip)]
    studied: usize,
    seed: u64,
}

impl Default for Ear {
    fn default() -> Self {
        Ear {
            taste: vec![0.0; (2 * SPAN + 1) as usize],
            usual: 0.5,
            clarity: 0.0,
            heard: vec![],
            last_step: None,
            notes: [24; 8],
            made_by: [None; 8],
            judged: VecDeque::new(),
            gesture: None,
            gesture_heard: vec![],
            turned: VecDeque::new(),
            human_note: None,
            from_human: 0,
            brightness: 0.0,
            loudness: 0.0,
            knows: BTreeMap::new(),
            prefer: BTreeMap::new(),
            taught: 0,
            watching: None,
            gesture_before: [0.0; 3],
            studied: 0,
            seed: 0x9e37_79b9_7f4a_7c15,
        }
    }
}

/// How periodic a sound is (0: noise or a clash, 1: one clear wave): the highest normalised autocorrelation over
/// the lags of 50 Hz to 1 kHz.
pub fn clarity(samples: &[f32], rate: f64) -> f64 {
    let n = samples.len();
    if n < 256 {
        return 0.0;
    }
    let mean = samples.iter().map(|x| *x as f64).sum::<f64>() / n as f64;
    let x: Vec<f64> = samples.iter().map(|v| *v as f64 - mean).collect();
    let energy: f64 = x.iter().map(|v| v * v).sum();
    if energy < 1e-9 {
        return 0.0;
    }
    let (lo, hi) = ((rate / 1000.0) as usize, ((rate / 50.0) as usize).min(n / 2));
    let mut best: f64 = 0.0;
    for lag in lo.max(1)..hi {
        let mut c = 0.0;
        let mut e1 = 0.0;
        let mut e2 = 0.0;
        for i in (0..n - lag).step_by(2) {
            c += x[i] * x[i + lag];
            e1 += x[i] * x[i];
            e2 += x[i + lag] * x[i + lag];
        }
        let r = c / (e1 * e2).sqrt().max(1e-12);
        best = best.max(r);
    }
    best.clamp(0.0, 1.0)
}

impl Ear {
    /// What she hears now: how clear, how bright, how loud.
    pub fn feel(&mut self, samples: &[f32], rate: f64) {
        self.clarity = clarity(samples, rate);
        let n = samples.len().max(1) as f64;
        let rms = (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / n).sqrt();
        let crossings = samples.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count() as f64;
        self.brightness = (crossings / 2.0 * rate / n / 4000.0).clamp(0.0, 1.0);
        self.loudness = rms.clamp(0.0, 1.0);
    }

    pub fn features(&self) -> [f64; 3] {
        [self.clarity, self.brightness, self.loudness]
    }

    /// A knob turned by `turn` (a part of its whole turn) while she heard `before` then what she hears now: what
    /// she knows of it moves towards what she heard. Returns how much it surprised her (what she heard against what
    /// she expected).
    fn learn_knob(&mut self, name: &str, turn: f64, before: [f64; 3]) -> f64 {
        if turn.abs() < 0.02 {
            return 0.0;
        }
        let after = self.features();
        let k = self.knows.entry(name.to_string()).or_insert([0.0; 3]);
        let mut surprise = 0.0;
        for i in 0..3 {
            let seen = ((after[i] - before[i]) / turn).clamp(-5.0, 5.0);
            surprise += ((seen - k[i]) * turn).abs();
            k[i] += 0.3 * (seen - k[i]);
        }
        surprise
    }

    /// The human turns a knob (`from` .. `to`, parts of its whole turn): she watches, and hears what it does.
    pub fn human_turns(&mut self, name: &str, from: f64, to: f64, now: f64) {
        match &mut self.watching {
            Some(w) if w.0 == name => {
                w.2 = to;
                w.4 = now;
            }
            _ => self.watching = Some((name.to_string(), from, to, self.features(), now)),
        }
    }

    /// A beat after the human let go of a knob, she has heard what it did: she learns it, and where they like it.
    /// Returns how much it surprised her.
    pub fn settle(&mut self, now: f64, beat: f64) -> Option<f64> {
        let (name, from, to, before, last) = self.watching.clone()?;
        if now < last + beat.max(0.25) {
            return None;
        }
        self.watching = None;
        let surprise = self.learn_knob(&name, to - from, before);
        let p = self.prefer.entry(name.clone()).or_insert(to);
        *p += 0.5 * (to - *p);
        self.taught += 1;
        self.turned.push_front((format!("{name} (human)"), true));
        self.turned.truncate(40);
        Some(surprise)
    }

    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The next interval: drawn by its taste (softly: it still tries the others), a note it has just played less
    /// likely, and kept within the range (A1 .. A5).
    fn choose(&mut self, from: i32) -> i32 {
        let weights: Vec<f64> = (-SPAN..=SPAN)
            .map(|i| {
                let to = from + i;
                if !(0..=48).contains(&to) {
                    return 0.0;
                }
                let again = if i == 0 { 0.3 } else { 1.0 };
                (self.taste[(i + SPAN) as usize] * 3.0).exp() * again
            })
            .collect();
        let total: f64 = weights.iter().sum();
        let mut r = self.random() * total;
        for (k, w) in weights.iter().enumerate() {
            r -= w;
            if r <= 0.0 {
                return k as i32 - SPAN;
            }
        }
        0
    }

    /// A note the human plays (Hz): the interval from their last note leans her towards it (she learns by
    /// playing with them, as one learns from another musician).
    pub fn hear_human(&mut self, hz: f64) {
        if hz.is_nan() || hz <= 20.0 {
            return;
        }
        let note = (12.0 * (hz / 55.0).log2()).round() as i32;
        if self.human_note == Some(note) {
            return;
        }
        if let Some(before) = self.human_note {
            let i = (note - before).clamp(-SPAN, SPAN);
            self.taste[(i + SPAN) as usize] = (self.taste[(i + SPAN) as usize] + 0.25).min(2.0);
            self.from_human += 1;
        }
        self.human_note = Some(note);
    }

    /// Her hands on the knobs (the cutoff, the resonance, the envelopes, the echo, the room...; not the loudness
    /// nor the tempo): now and then she takes one and moves it, slowly or fast, a little or far (a sweep of the
    /// filter, a room that opens), as a gesture over a few beats; she listens while it moves and a beat after; clearer
    /// than usual, she leaves it there; not, she brings it back. `knobs`: (node, lo, hi, now). Called every frame,
    /// with or without the sequencer; returns the values to write now.
    pub fn play_knobs(&mut self, knobs: &[(String, f64, f64, f64)], now: f64, beat: f64) -> Vec<(String, f64)> {
        let mut out = vec![];
        if let Some((name, from, to, t0, dur)) = self.gesture.clone() {
            let f = ((now - t0) / dur).clamp(0.0, 1.0);
            // a smooth gesture (slow at both ends)
            let e = f * f * (3.0 - 2.0 * f);
            if f < 1.0 {
                out.push((name.clone(), from + (to - from) * e));
            }
            self.gesture_heard.push(self.clarity);
            if now >= t0 + dur + beat {
                let heard = self.gesture_heard.iter().sum::<f64>() / self.gesture_heard.len().max(1) as f64;
                let kept = heard >= self.usual;
                if let Some(k) = knobs.iter().find(|k| k.0 == name) {
                    let span = (k.2 - k.1).abs().max(1e-12);
                    let before = self.gesture_before;
                    self.learn_knob(&name, (to - from) / span, before);
                }
                out.push((name.clone(), if kept { to } else { from }));
                self.turned.push_front((name, kept));
                self.turned.truncate(40);
                self.gesture = None;
                self.gesture_heard.clear();
            }
            return out;
        }
        // she reaches for a knob, sometimes (about once every eight beats)
        // not while the human is turning one: she watches
        if knobs.is_empty() || self.watching.is_some() || self.random() > 1.0 / (8.0 * beat.max(0.05) * 25.0) {
            return out;
        }
        let k = (self.random() * knobs.len() as f64) as usize % knobs.len();
        let (name, lo, hi, v) = knobs[k].clone();
        let span = hi - lo;
        // a little or far: most gestures small, some wide (a sweep)
        let reach = if self.random() < 0.25 { 0.6 } else { 0.15 };
        let mut to = v + (self.random() * 2.0 - 1.0) * reach * span;
        // sometimes towards where the human likes it, sometimes the way she knows makes it clearer
        let r = self.random();
        if let (Some(p), true) = (self.prefer.get(&name).copied(), r < 0.4) {
            to = lo + span * (p + (self.random() - 0.5) * 0.1);
        } else if let (Some(k), true) = (self.knows.get(&name).copied(), r < 0.7) {
            if k[0].abs() > 0.01 {
                to = v + k[0].signum() * reach * span.abs();
            }
        }
        let to = to.clamp(lo.min(hi), hi.max(lo));
        self.gesture_before = self.features();
        let dur = beat * [0.5, 1.0, 2.0, 4.0][(self.random() * 4.0) as usize % 4];
        self.gesture = Some((name, v, to, now, dur));
        out
    }

    /// A lesson on the synthesiser: she studies its knobs one after another. Exploring, she turns each over its
    /// whole course, slowly, and listens (what she hears it do is learned, and said); looking for a sound (`want`:
    /// clarity, brightness, loudness), she turns the knob that, by what she knows, brings her nearest. Returns the
    /// values to write, and the knob she has just finished studying.
    pub fn study(
        &mut self,
        knobs: &[(String, f64, f64, f64)],
        now: f64,
        beat: f64,
        want: Option<[f64; 3]>,
    ) -> (Vec<(String, f64)>, Option<String>) {
        if self.gesture.is_some() {
            let name = self.gesture.as_ref().map(|g| g.0.clone());
            let out = self.play_knobs(knobs, now, beat);
            let done = if self.gesture.is_none() { name } else { None };
            return (out, done);
        }
        if knobs.is_empty() || self.watching.is_some() {
            return (vec![], None);
        }
        self.studied = (self.studied + 1) % knobs.len();
        let (name, lo, hi, v) = knobs[self.studied].clone();
        let span = hi - lo;
        let to = match want {
            None => {
                // its whole course: to the far end from where it is
                if (v - lo).abs() < (hi - v).abs() {
                    hi
                } else {
                    lo
                }
            }
            Some(want) => {
                // by what she knows of it: the turn that brings what she hears nearest to what she wants
                let k = self.knows.get(&name).copied().unwrap_or([0.0; 3]);
                let need: Vec<f64> = (0..3).map(|i| want[i] - self.features()[i]).collect();
                let kk: f64 = k.iter().map(|x| x * x).sum();
                let turn = if kk < 1e-6 {
                    (self.random() - 0.5) * 0.4
                } else {
                    ((0..3).map(|i| k[i] * need[i]).sum::<f64>() / kk).clamp(-0.3, 0.3)
                };
                v + turn * span
            }
        };
        let to = to.clamp(lo.min(hi), hi.max(lo));
        let dur = beat * if want.is_none() { 8.0 } else { 2.0 };
        self.gesture_before = self.features();
        self.gesture = Some((name, v, to, now, dur));
        (vec![], None)
    }

    /// A long note (a pad): her hands move a knob with it, slowly, the note's whole length (the sound and the note
    /// together: what makes a pad breathe). The filter first, when there is one.
    pub fn sweep(&mut self, knobs: &[(String, f64, f64, f64)], now: f64, seconds: f64) {
        if self.gesture.is_some() || self.watching.is_some() || knobs.is_empty() {
            return;
        }
        let k = knobs
            .iter()
            .position(|k| k.0.ends_with("cutoff") && self.random() < 0.6)
            .unwrap_or((self.random() * knobs.len() as f64) as usize % knobs.len());
        let (name, lo, hi, v) = knobs[k].clone();
        let to = (v + (self.random() * 2.0 - 1.0) * 0.4 * (hi - lo)).clamp(lo.min(hi), hi.max(lo));
        self.gesture_before = self.features();
        self.gesture = Some((name, v, to, now, seconds.max(0.5)));
    }

    /// One frame: what it hears now, the step the sequencer is on. Returns the notes to write: (step 0..7, Hz).
    pub fn listen(
        &mut self,
        samples: &[f32],
        rate: f64,
        step: i64,
        bind: &mut dyn FnMut(i32) -> i32,
    ) -> Vec<(usize, f64)> {
        self.feel(samples, rate);
        let mut out = vec![];
        if self.last_step == Some(step) {
            self.heard.push(self.clarity);
            return out;
        }
        // the step that just ended is judged by what was heard while it played
        if let Some(prev) = self.last_step {
            let k = prev.rem_euclid(8) as usize;
            if let Some(i) = self.made_by[k] {
                let heard =
                    if self.heard.is_empty() { 0.0 } else { self.heard.iter().sum::<f64>() / self.heard.len() as f64 };
                let reward = heard - self.usual;
                self.taste[(i + SPAN) as usize] = (self.taste[(i + SPAN) as usize] + 0.6 * reward).clamp(-2.0, 2.0);
                self.usual += 0.1 * (heard - self.usual);
                self.judged.push_front((i, heard, reward));
                self.judged.truncate(16);
            }
        }
        self.heard.clear();
        self.heard.push(self.clarity);
        self.last_step = Some(step);
        // the note two steps ahead is written now (so it is there before it is played)
        let k = (step + 2).rem_euclid(8) as usize;
        let before = self.notes[(k + 7) % 8];
        let i = self.choose(before);
        self.notes[k] = bind((before + i).clamp(0, 48)).clamp(0, 48);
        self.made_by[k] = Some((self.notes[k] - before).clamp(-SPAN, SPAN));
        out.push((k, 55.0 * 2f64.powf(self.notes[k] as f64 / 12.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn she_learns_from_what_the_human_plays() {
        let mut ear = Ear::default();
        // A, E, A, E...: fifths up and fourths down
        for k in 0..10 {
            ear.hear_human(if k % 2 == 0 { 220.0 } else { 329.63 });
        }
        assert_eq!(ear.from_human, 9);
        assert!(ear.taste[(7 + SPAN) as usize] >= 1.0 && ear.taste[(-7 + SPAN) as usize] >= 1.0);
        assert_eq!(ear.taste[(1 + SPAN) as usize], 0.0);
    }

    #[test]
    fn a_clear_wave_is_clearer_than_a_clash() {
        let rate = 48000.0;
        let wave = |f: &[f64]| -> Vec<f32> {
            (0..2048)
                .map(|t| f.iter().map(|f| (std::f64::consts::TAU * f * t as f64 / rate).sin()).sum::<f64>() as f32)
                .collect()
        };
        let one = clarity(&wave(&[220.0]), rate);
        let fifth = clarity(&wave(&[220.0, 330.0]), rate);
        let clash = clarity(&wave(&[220.0, 233.08]), rate);
        assert!(one > 0.95, "{one}");
        assert!(fifth > clash, "a fifth {fifth}, a minor second {clash}");
    }

    #[test]
    fn it_leans_towards_what_sounded_clear() {
        let mut ear = Ear::default();
        // whatever it plays, the fifth (7) is heard clear and the rest not: its taste for 7 grows
        for step in 0..400i64 {
            let clear = matches!(ear.made_by[(step as usize).rem_euclid(8)], Some(7) | Some(-7));
            let s: Vec<f32> = if clear {
                (0..1024).map(|t| (t as f32 * 0.05).sin()).collect()
            } else {
                let mut x: u64 = 88172645463325252 ^ step as u64;
                (0..1024)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
                    })
                    .collect()
            };
            ear.listen(&s, 48000.0, step, &mut |n| n);
        }
        let fifth = ear.taste[(7 + SPAN) as usize].max(ear.taste[(-7 + SPAN) as usize]);
        let second = ear.taste[(1 + SPAN) as usize];
        assert!(fifth > second, "fifth {fifth}, second {second}");
    }
}

/// What the band writes: the melody's pitch and gate, the bass's (when the sequencer is off), the sequencer on/off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    LeadPitch,
    LeadGate,
    BassPitch,
    BassGate,
    SeqOn,
    /// The metronome on or off (lessons are learned with it first, then without).
    Metro,
    /// A chord's tone (the third voice, the fourth), and whether it sounds.
    ChordPitch(u8),
    ChordGate(u8),
}

/// Playing freely: a melody over the bass (or the sequence), and, the sequencer off, the bass too; now and then
/// she turns the sequencer on or off herself. Each note is chosen by her taste (the intervals in time, as the ear
/// learns them, and the intervals with the bass at the same moment, learned here), held for a length she chooses,
/// and judged by how clear what comes out was while it sounded.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Band {
    /// Her taste for each interval with the bass at the same moment (0 .. 11 semitones, an octave folded).
    pub together: Vec<f64>,
    lead: i32,
    lead_until: f64,
    lead_on: bool,
    lead_heard: Vec<f64>,
    lead_with: Option<(i32, i32)>,
    /// Her taste for each length (an eighth, a quarter, a half, a whole, two wholes) and for silence; the length
    /// and whether the last one was a rest (judged as the notes are).
    pub lengths: Vec<f64>,
    pub rest: f64,
    last_len: Option<(usize, bool)>,
    /// What she played lately, for the piano roll: (when it began, how long, the note from A1, 0 the bass 1 the
    /// melody).
    pub roll: VecDeque<(f64, f64, i32, u8)>,
    bass: i32,
    bass_until: f64,
    /// What she heard in each way of playing, with or without the sequencer: (how clear on average, how long she
    /// played so). She decides from it, nothing else: no clock tells her when.
    pub ways: [(f64, f64); 2],
    last_now: f64,
    /// What she did lately, in words for the window.
    pub said: VecDeque<String>,
    /// The human's phrase on the keys (when each note began, the note from A1), and when they last played: she
    /// listens while they play, and answers when they stop.
    #[serde(skip)]
    phrase: Vec<(f64, i32)>,
    #[serde(skip)]
    last_human: f64,
    /// Her answer, still to play: (length in beats, the note).
    #[serde(skip)]
    answer: VecDeque<(f64, i32)>,
    /// What is recorded now (every note, both voices and the human's), when recording.
    #[serde(skip)]
    pub recording: Option<Vec<(f64, f64, i32, u8)>>,
    /// How long her note lasts (seconds), for the lesson.
    #[serde(skip)]
    note_seconds: f64,
    /// What was heard lately, by pitch class (from A), fading: the key it is in.
    pub heard: [f64; 12],
    /// Each on their own instrument: she plays on while the human plays.
    #[serde(skip)]
    pub duet: bool,
    /// The bar whose chord sounds (none: no chord).
    #[serde(skip)]
    chord_bar: Option<usize>,
    /// Where the beat is now (0 .. 1, the metronome's phase), and whether its click is heard.
    #[serde(skip)]
    pub beat_phase: Option<f64>,
    #[serde(skip)]
    pub metro_heard: bool,
    /// Her sense of time: how much she places her notes on the beat (0 .. 1, it grows as she practises with the
    /// click), and how far from it her notes began lately (milliseconds).
    pub timing: f64,
    pub off_ms: f64,
    /// The metronome as the lesson wants it (on while it is guided), last asked.
    #[serde(skip)]
    metro_asked: Option<bool>,
    /// When her bass note and her chord are let go.
    #[serde(skip)]
    bass_off: Option<f64>,
    #[serde(skip)]
    chord_off: Option<f64>,
    /// The chord she holds now (notes from A1), for the arpeggiator.
    #[serde(skip)]
    pub chord_notes: Vec<i32>,
    /// Her taste for a left hand (chords of her own under the melody), whether it plays now, what it heard.
    pub left: f64,
    #[serde(skip)]
    left_on: bool,
    #[serde(skip)]
    left_heard: Vec<f64>,
    /// Her last note and the bass under it (for the rules of two notes together).
    #[serde(skip)]
    pair: Option<(i32, i32)>,
    /// A long note just begun (seconds): her hands may make it a pad.
    #[serde(skip)]
    pub long_note: Option<f64>,
    seed: u64,
}

impl Default for Band {
    fn default() -> Self {
        Band {
            together: vec![0.0; 12],
            lead: 36,
            lead_until: 0.0,
            lead_on: false,
            lead_heard: vec![],
            lead_with: None,
            lengths: vec![0.0; Band::LENGTHS.len()],
            rest: 0.0,
            last_len: None,
            roll: VecDeque::new(),
            bass: 24,
            bass_until: 0.0,
            ways: [(0.5, 0.0); 2],
            last_now: 0.0,
            said: VecDeque::new(),
            phrase: vec![],
            last_human: f64::MIN,
            answer: VecDeque::new(),
            recording: None,
            note_seconds: 0.0,
            long_note: None,
            pair: None,
            chord_bar: None,
            duet: false,
            left: 0.0,
            chord_notes: vec![],
            bass_off: None,
            beat_phase: None,
            metro_heard: false,
            timing: 0.0,
            off_ms: 0.0,
            metro_asked: None,
            chord_off: None,
            left_on: false,
            left_heard: vec![],
            heard: [0.0; 12],
            seed: 0x2545_f491_4f6c_dd1d,
        }
    }
}

fn hz(note: i32) -> f64 {
    55.0 * 2f64.powf(note as f64 / 12.0)
}

pub fn note_of(hz: f64) -> i32 {
    (12.0 * (hz / 55.0).log2()).round() as i32
}

impl Band {
    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The lengths she can give a note, in beats (a quarter note a beat): a sixteenth, an eighth of a triplet, an
    /// eighth, a dotted eighth, a quarter, a dotted quarter, a half, a dotted half, a whole.
    pub const LENGTHS: [f64; 9] = [0.25, 1.0 / 3.0, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
    /// Their names, as a musician writes them.
    pub const LENGTH_NAMES: [&'static str; 9] = ["1/16", "1/8 (3)", "1/8", "1/8.", "1/4", "1/4.", "1/2", "1/2.", "1"];

    /// Its lists as long as they should be (a memory kept before).
    pub fn fit(&mut self) {
        if self.lengths.len() != Self::LENGTHS.len() {
            self.lengths = vec![0.0; Self::LENGTHS.len()];
        }
    }

    /// A length, by her taste (with a little chance: she keeps trying the others).
    fn length(&mut self) -> (usize, f64) {
        let mut best = (f64::MIN, 1);
        for k in 0..Self::LENGTHS.len() {
            let score = self.lengths.get(k).copied().unwrap_or(0.0) + self.random() * 1.0;
            if score > best.0 {
                best = (score, k);
            }
        }
        (best.1, Self::LENGTHS[best.1])
    }

    /// A note heard, for the piano roll (the window keeps the last twenty seconds).
    pub fn played(&mut self, at: f64, len: f64, note: i32, voice: u8) {
        for h in self.heard.iter_mut() {
            *h *= 0.97;
        }
        self.heard[note.rem_euclid(12) as usize] += len.clamp(0.05, 4.0);
        if let Some(r) = &mut self.recording {
            r.push((at, len, note, voice));
        }
        self.roll.push_back((at, len, note, voice));
        while self.roll.front().is_some_and(|n| n.0 + n.1 < at - 20.0) {
            self.roll.pop_front();
        }
    }

    fn say(&mut self, s: String) {
        self.said.push_front(s);
        self.said.truncate(6);
    }

    /// A note the human plays on the keys (from A1): she listens (on the roll, in the human's colour).
    /// `show`: drawn on the roll here (a polyphonic instrument draws the human's notes when they are let go, with
    /// their length).
    pub fn human_played(&mut self, now: f64, note: i32, show: bool) {
        self.phrase.push((now, note));
        self.phrase.truncate(16);
        self.last_human = now;
        if show {
            self.played(now, 0.3, note, 2);
        }
    }

    /// Her answer to the human's phrase: its shape kept (its intervals, its rhythm), moved by the interval she
    /// likes best with what they played, its last note her own.
    fn answer_phrase(&mut self, ear: &mut Ear, beat: f64, teacher: &mut Teacher) {
        let phrase = std::mem::take(&mut self.phrase);
        if phrase.is_empty() {
            return;
        }
        // the key they played in (Krumhansl's profiles): in a lesson, she joins it when she hears it well
        let mut counts = [0.0; 12];
        for (k, (at, note)) in phrase.iter().enumerate() {
            let next = phrase.get(k + 1).map(|n| n.0).unwrap_or(at + beat);
            counts[note.rem_euclid(12) as usize] += (next - at).clamp(0.1, 2.0);
        }
        if phrase.len() >= 4 {
            let (tonic, major, fit) = crate::music::key_of(&counts);
            teacher.human_key = Some((tonic, major, fit));
            if teacher.on && fit > 0.6 && tonic != teacher.tonic {
                teacher.walk += crate::music::fifths_of(tonic - teacher.tonic);
                teacher.tonic = tonic;
                let name = crate::music::PITCHES[tonic as usize];
                self.say(format!(
                    "she hears the human in {name} {}: she joins their key",
                    if major { "major" } else { "minor" }
                ));
            }
        }
        let shifts = [0, 7, 5, -5, 3, 4, -3, 12, -12];
        let mut shift = 0;
        for i in shifts {
            if ear.taste[(i + SPAN) as usize] > ear.taste[(shift + SPAN) as usize] {
                shift = i;
            }
        }
        // the whole phrase moved by octaves to stay in the melody's range (its shape kept)
        let lo = phrase.iter().map(|p| p.1).min().unwrap_or(24) + shift;
        let hi = phrase.iter().map(|p| p.1).max().unwrap_or(24) + shift;
        let mut octave = 0;
        while lo + octave < 24 && hi + octave + 12 <= 56 {
            octave += 12;
        }
        while hi + octave > 56 && lo + octave - 12 >= 24 {
            octave -= 12;
        }
        for (k, (at, note)) in phrase.iter().enumerate() {
            let next = phrase.get(k + 1).map(|n| n.0).unwrap_or(at + beat);
            let beats = (((next - at) / beat.max(1e-3)) * 2.0).round().clamp(1.0, 8.0) / 2.0;
            self.answer.push_back((beats, (note + shift + octave).clamp(24, 56)));
        }
        if let Some(last) = self.answer.back_mut() {
            let i = ear.choose_from(last.1, 24, 56);
            last.1 = (last.1 + i).clamp(24, 56);
        }
        let n = phrase.len();
        self.say(format!("she answers the human's phrase ({n} notes), moved by {shift:+} semitones"));
    }

    /// The next melody note: by its taste in time (from her ear) and with the bass (learned here).
    fn choose_lead(&mut self, ear: &Ear, bass: i32) -> i32 {
        let mut best = (f64::MIN, self.lead);
        for i in -7..=7 {
            let to = self.lead + i;
            if !(24..=56).contains(&to) {
                continue;
            }
            let time = ear.taste[(i + SPAN) as usize];
            let with = self.together[(to - bass).rem_euclid(12) as usize];
            let again = if i == 0 { -0.6 } else { 0.0 };
            // a little chance in each choice: she keeps trying
            let score = time + with + again + self.random() * 1.2;
            if score > best.0 {
                best = (score, to);
            }
        }
        best.1
    }

    /// One frame. `now`: seconds; `beat`: seconds a beat; `clarity`: heard now; `bass_hz`: the bass sounding now
    /// (the sequence's), None when she plays it herself. Returns what to write.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        ear: &mut Ear,
        now: f64,
        beat: f64,
        clarity: f64,
        bass_hz: Option<f64>,
        seq_on: bool,
        teacher: &mut Teacher,
    ) -> Vec<(Part, f64)> {
        let mut out = vec![];
        let (r1, r2) = (self.random(), self.random());
        teacher.at_bar((now / (4.0 * beat.max(1e-3))).max(0.0) as usize, r1, r2);
        // the chords of the lesson, held under her, a bar each (the third and fourth voices)
        let chord = if teacher.on { teacher.chord() } else { None };
        match chord {
            Some((root, tones)) if tones.len() >= 3 && self.chord_bar != Some(teacher.bar) => {
                self.chord_bar = Some(teacher.bar);
                let base = 36 + (teacher.tonic + root).rem_euclid(12);
                let (a, b) = (base + tones[1], base + tones[if tones.len() > 3 { 3 } else { 2 }]);
                self.chord_notes = vec![base, a, b];
                for (k, n) in [(0u8, a), (1, b)] {
                    out.push((Part::ChordPitch(k), hz(n)));
                    out.push((Part::ChordGate(k), 1.0));
                    self.played(now, 3.6 * beat, n, 3);
                }
                self.chord_off = Some(now + 3.6 * beat);
            }
            _ => {}
        }
        // a lesson with a metre is learned with the click first (guided), then without: she keeps the time herself
        let lesson = teacher.current();
        let wants = teacher.on
            && teacher.phase() == "guided"
            && lesson.rhythm.as_deref().is_some_and(|r| r != "free")
            && lesson.knobs.is_none();
        if self.metro_asked != Some(wants) && teacher.on {
            self.metro_asked = Some(wants);
            out.push((Part::Metro, if wants { 1.0 } else { 0.0 }));
            self.say(if wants {
                "the click on: the lesson is guided".into()
            } else {
                "the click off: she keeps the time herself".into()
            });
        }
        // the human plays: she listens, silent; they stop: she answers. Each on their own instrument, she plays on
        // with them (improvising together), and answers their phrase now and then
        let human_playing = now - self.last_human < 1.5 * beat;
        if self.duet && !human_playing && !self.phrase.is_empty() && self.random() > 0.35 {
            self.phrase.clear();
        }
        if human_playing && !self.duet {
            if self.lead_on {
                self.lead_on = false;
                out.push((Part::LeadGate, 0.0));
            }
            self.lead_until = now + beat * 0.5;
            self.last_len = None;
            self.lead_with = None;
            self.lead_heard.clear();
            return out;
        }
        if !self.phrase.is_empty() {
            self.answer_phrase(ear, beat, teacher);
            self.lead_until = now;
        }
        // resting (her lessons): silent, the time it takes
        if teacher.on && teacher.resting(now) {
            if self.lead_on {
                self.lead_on = false;
                out.push((Part::LeadGate, 0.0));
            }
            self.lead_until = now;
            self.lead_heard.clear();
            return out;
        }
        // what she hears while her note sounds, or while she is silent
        self.lead_heard.push(clarity);
        // what she hears, counted for the way she plays now
        let dt = (now - self.last_now).clamp(0.0, 0.5);
        self.last_now = now;
        let way = &mut self.ways[seq_on as usize];
        way.1 += dt;
        way.0 += (clarity - way.0) * (dt / (way.1 + 1.0)).min(0.2);
        // the bass: the sequence's, or her own (the sequencer off), a long note at a time
        // her bass and her chords let go before the next (a note held for ever is a drone)
        if self.bass_off.is_some_and(|t| now >= t) {
            self.bass_off = None;
            out.push((Part::BassGate, 0.0));
        }
        if self.chord_off.is_some_and(|t| now >= t) {
            self.chord_off = None;
            out.push((Part::ChordGate(0), 0.0));
            out.push((Part::ChordGate(1), 0.0));
        }
        let bass = match bass_hz {
            Some(h) if seq_on && h > 20.0 => {
                // the sequencer has the bass: hers is let go
                if self.bass_off.take().is_some() {
                    out.push((Part::BassGate, 0.0));
                }
                note_of(h)
            }
            _ => {
                if now >= self.bass_until {
                    let chord = if teacher.on { teacher.chord() } else { None };
                    match chord {
                        // the lesson's chords: the bass plays their roots (the accompaniment)
                        Some(_) => self.bass = 12 + teacher.bass_root().unwrap_or(0),
                        None => {
                            let i = ear.choose_from(self.bass, 0, 36);
                            let r = self.random();
                            self.bass = if teacher.on { teacher.bind(self.bass + i, 12, r) } else { self.bass + i };
                            self.bass = self.bass.clamp(0, 36);
                        }
                    }
                    let len = if chord.is_some() {
                        // to the next bar
                        let bar = 4.0 * beat;
                        (bar - now.rem_euclid(bar)).max(beat * 0.5)
                    } else {
                        beat * 2.0 * self.length().1.max(1.0)
                    };
                    self.bass_until = now + len;
                    let b = self.bass;
                    self.played(now, len, b, 0);
                    out.push((Part::BassPitch, hz(self.bass)));
                    out.push((Part::BassGate, 1.0));
                    self.bass_off = Some(now + 0.85 * len);
                }
                self.bass
            }
        };
        // her left hand, outside the lessons' chords: at each bar, a chord over the bass now and then (two tones she
        // likes with it), by her taste for it, judged by what she heard while it was held
        if chord.is_none() {
            if self.left_on {
                self.left_heard.push(clarity);
            }
            if self.chord_bar != Some(teacher.bar) {
                self.chord_bar = Some(teacher.bar);
                if self.left_on && !self.left_heard.is_empty() {
                    let heard = self.left_heard.iter().sum::<f64>() / self.left_heard.len() as f64;
                    self.left = (self.left + 0.3 * (heard - ear.usual)).clamp(-2.0, 2.0);
                }
                self.left_heard.clear();
                let p = (0.25 * (self.left * 1.5).exp()).clamp(0.05, 0.8);
                if self.random() < p {
                    let mut ks: Vec<(f64, i32)> =
                        (1..12).map(|k| (self.together[k as usize] + self.random() * 0.6, k)).collect();
                    ks.sort_by(|a, b| b.0.total_cmp(&a.0));
                    let root = 36 + bass.rem_euclid(12);
                    self.chord_notes = std::iter::once(root).chain(ks.iter().take(2).map(|k| root + k.1)).collect();
                    for (j, (_, k)) in ks.iter().take(2).enumerate() {
                        let n = root + k;
                        out.push((Part::ChordPitch(j as u8), hz(n)));
                        out.push((Part::ChordGate(j as u8), 1.0));
                        self.played(now, 3.6 * beat, n, 3);
                    }
                    self.chord_off = Some(now + 3.6 * beat);
                    self.left_on = true;
                } else if self.left_on {
                    self.chord_notes.clear();
                    out.push((Part::ChordGate(0), 0.0));
                    out.push((Part::ChordGate(1), 0.0));
                    self.left_on = false;
                }
            }
        }
        // the melody: when a note ends, it is judged, and the next chosen (or a rest)
        let ended = now >= self.lead_until;
        if ended {
            let judged = self.last_len.is_some();
            let heard = if self.lead_heard.is_empty() {
                ear.usual
            } else {
                self.lead_heard.iter().sum::<f64>() / self.lead_heard.len() as f64
            };
            // the length she chose, and whether it was a silence, judged too
            if let Some((k, was_rest)) = self.last_len.take() {
                let reward = heard - ear.usual;
                if let Some(l) = self.lengths.get_mut(k) {
                    *l = (*l + 0.4 * reward).clamp(-2.0, 2.0);
                }
                if was_rest {
                    self.rest = (self.rest + 0.4 * reward).clamp(-2.0, 2.0);
                }
            }
            if let Some((with, step)) = self.lead_with.take() {
                if !self.lead_heard.is_empty() {
                    let reward = heard - ear.usual;
                    let k = with.rem_euclid(12) as usize;
                    self.together[k] = (self.together[k] + 0.5 * reward).clamp(-2.0, 2.0);
                    ear.taste[(step.clamp(-SPAN, SPAN) + SPAN) as usize] =
                        (ear.taste[(step.clamp(-SPAN, SPAN) + SPAN) as usize] + 0.3 * reward).clamp(-2.0, 2.0);
                }
            }
            // what she lived, for her lesson (and her mood)
            // out of her lessons, she feels all the same (her mood is not the lessons')
            if !teacher.on && self.note_seconds > 0.0 && judged {
                teacher.felt(heard, self.note_seconds, ear.usual);
            }
            if teacher.on && self.note_seconds > 0.0 && judged {
                teacher.lived_note(heard, self.note_seconds, ear.usual);
                let r = self.random();
                teacher.consider(now, r);
                if let Some(s) = teacher.said.first().cloned() {
                    if self.said.front() != Some(&s) {
                        self.say(s);
                    }
                }
            }
            self.lead_heard.clear();
            let (mut k, mut len) = self.length();
            // the lesson's rhythm (by its bridle)
            if teacher.on {
                let r = self.random();
                if let Some(l) = teacher.length(r) {
                    len = l;
                    k = Self::LENGTHS
                        .iter()
                        .enumerate()
                        .min_by(|a, b| (a.1 - l).abs().total_cmp(&(b.1 - l).abs()))
                        .map(|(k, _)| k)
                        .unwrap_or(1);
                }
            }
            // her answer to the human: its own rhythm
            if let Some((beats, _)) = self.answer.front() {
                len = *beats;
            }
            self.note_seconds = beat * len;
            self.long_note = (len >= 4.0).then_some(beat * len);
            // a silence, by her taste for silence (never all the time, never never)
            let p_rest = (0.15 * (self.rest * 1.5).exp()).clamp(0.03, 0.6);
            let resting = self.answer.is_empty() && self.random() < p_rest;
            self.last_len = Some((k, resting));
            if resting {
                // a rest: silence is part of it
                self.lead_on = false;
                out.push((Part::LeadGate, 0.0));
            } else {
                let mut to = match self.answer.pop_front() {
                    Some((_, n)) => n,
                    None => self.choose_lead(ear, bass),
                };
                if teacher.on {
                    let r = self.random();
                    to = teacher.bind(to, 24, r).clamp(24, 56);
                    // two notes together by the rules (counterpoint's), as tight as the bridle: on a strong beat a
                    // consonance with the bass, a dissonance only by step, no fifths nor octaves in a row
                    let pos = now / beat.max(1e-3);
                    let on_beat = pos.fract() < 0.12 || pos.fract() > 0.88;
                    let strong = len >= 1.0 || (on_beat && (pos.round() as i64) % 2 == 0);
                    let r = self.random();
                    to = teacher.counterpoint(to, self.pair, bass, strong, r, 24);
                }
                self.pair = Some((to, bass));
                self.lead_with = Some((to - bass, to - self.lead));
                self.lead = to;
                self.lead_on = true;
                // free, she may tune the interval with the bass by its ratio (just, not tempered): she plays
                // frequencies, not only semitones
                let mut f = hz(to);
                if teacher.on && teacher.phase() == "free" && self.random() < 0.4 {
                    let (p, q) = crate::music::ratio(to - bass);
                    let octaves = (to - bass).div_euclid(12);
                    f = hz(bass) * p as f64 / q as f64 * 2f64.powi(octaves);
                }
                out.push((Part::LeadPitch, f));
                out.push((Part::LeadGate, 1.0));
                self.played(now, beat * len, to, 1);
            }
            self.lead_until = now + beat * len;
            // on the beat: where this note began against the beat (the nearest sixteenth), heard; and the next
            // placed nearer the grid, as much as her sense of time already holds
            if let Some(p) = self.beat_phase {
                let off = ((p * 4.0).round() - p * 4.0) / 4.0;
                self.off_ms += 0.1 * ((off * beat).abs() * 1000.0 - self.off_ms);
                if self.metro_heard {
                    // with the click to hear, she practises it
                    self.timing += 0.01 * (1.0 - self.timing);
                }
                let end = p + len;
                let grid = (end * 4.0).round() / 4.0;
                // guided, on the beat whatever her sense of time (as tight as the bridle); then hers
                let held = if teacher.on && teacher.phase() == "guided" { teacher.bridle_now() } else { 0.0 };
                self.lead_until += self.timing.max(held) * (grid - end) * beat;
            }
        }
        // with or without the sequencer: hers to decide. At the end of each of her notes she weighs both ways by
        // what she heard in each, the one she knows less counting a little more (it might be better: she tries it);
        // she changes only when the other way weighs more than this one
        if ended
            && self.ways[0].1 + self.ways[1].1 > 2.0
            && !(teacher.on && (teacher.chord().is_some() || teacher.current().switch.is_some()))
        {
            let total = self.ways[0].1 + self.ways[1].1;
            let weigh = |w: (f64, f64)| w.0 + 0.15 * (total.ln().max(0.0) / (w.1 + 1.0)).sqrt();
            let (here, there) = (weigh(self.ways[seq_on as usize]), weigh(self.ways[!seq_on as usize]));
            if there > here + 0.02 {
                out.push((Part::SeqOn, if seq_on { 0.0 } else { 1.0 }));
                self.said.push_front(if seq_on {
                    format!(
                        "sequencer off ({:.2} heard with it, {:.2} without): she plays the bass",
                        self.ways[1].0, self.ways[0].0
                    )
                } else {
                    format!("sequencer on ({:.2} heard without it, {:.2} with)", self.ways[0].0, self.ways[1].0)
                });
                self.said.truncate(6);
            }
        }
        out
    }
}

impl Ear {
    /// An interval from a note, by taste, kept within lo .. hi.
    pub fn choose_from(&mut self, from: i32, lo: i32, hi: i32) -> i32 {
        let i = self.choose(from);
        if (lo..=hi).contains(&(from + i)) {
            i
        } else {
            -i
        }
    }
}

#[cfg(test)]
mod band_tests {
    use super::*;

    #[test]
    fn her_hands_move_a_knob_smoothly_and_bring_it_back_when_it_clashed() {
        let mut ear = Ear::default();
        let knobs = vec![("cutoff".to_string(), 0.0, 1.0, 0.5)];
        let (mut t, mut seen, mut back) = (0.0, vec![], false);
        ear.usual = 0.8;
        ear.clarity = 0.2; // whatever she does, it sounds worse than usual
        for _ in 0..20000 {
            for (n, v) in ear.play_knobs(&knobs, t, 0.25) {
                assert_eq!(n, "cutoff");
                seen.push(v);
            }
            if ear.turned.front().is_some_and(|(_, kept)| !kept) {
                back = true;
                break;
            }
            t += 0.01;
        }
        assert!(seen.len() > 3, "a gesture, not a jump: {seen:?}");
        assert!(back && (seen.last().copied().unwrap_or(0.0) - 0.5).abs() < 1e-9, "brought back to where it was");
    }

    #[test]
    fn she_chooses_the_way_that_sounded_clearer() {
        // with the sequencer she hears clearer: she ends up playing with it most of the time, by herself
        let (mut ear, mut band) = (Ear::default(), Band::default());
        let mut seq = false;
        let (mut with, mut without) = (0.0, 0.0);
        let mut t = 0.0;
        for _ in 0..60000 {
            let clear = if seq { 0.9 } else { 0.3 };
            for (part, v) in band.tick(&mut ear, t, 0.25, clear, Some(110.0), seq, &mut Teacher::default()) {
                if part == Part::SeqOn {
                    seq = v > 0.5;
                }
            }
            if seq {
                with += 1.0
            } else {
                without += 1.0
            }
            t += 0.01;
        }
        assert!(with > 3.0 * without, "with {with}, without {without}");
    }

    #[test]
    fn she_plays_a_melody_and_learns_what_sounds_together() {
        let mut ear = Ear::default();
        let mut band = Band::default();
        let mut wrote = 0;
        // clear when the melody is a fifth (7) over the bass, cloudy otherwise
        let mut t = 0.0;
        let mut clear = 0.5;
        for _ in 0..20000 {
            let out = band.tick(&mut ear, t, 0.25, clear, None, false, &mut Teacher::default());
            wrote += out.len();
            if let Some((with, _)) = band.lead_with {
                clear = if with.rem_euclid(12) == 7 { 0.95 } else { 0.3 };
            }
            t += 0.01;
        }
        assert!(wrote > 50);
        let fifth = band.together[7];
        let worst = band.together.iter().copied().fold(f64::MAX, f64::min);
        assert!(fifth > worst + 0.3, "{:?}", band.together);
        // the lengths she gave her notes were judged too, and what she played is on the roll
        assert!(band.lengths.iter().any(|l| *l != 0.0), "{:?}", band.lengths);
        assert!(!band.roll.is_empty());
    }

    #[test]
    fn in_a_lesson_her_notes_keep_to_its_scale_until_the_bridle_loosens() {
        let mut ear = Ear::default();
        let mut band = Band::default();
        let mut teacher = Teacher { on: true, she_chooses: false, ..Default::default() };
        // the pentatonic: A B C# E F# (from A: 0 2 4 7 9)
        let mut t = 0.0;
        let mut seen = vec![];
        for _ in 0..4000 {
            for (part, v) in band.tick(&mut ear, t, 0.25, 0.6, None, false, &mut teacher) {
                if part == Part::LeadPitch {
                    seen.push(note_of(v));
                }
            }
            t += 0.01;
        }
        assert!(seen.len() > 20);
        let inside = seen.iter().filter(|n| [0, 2, 4, 7, 9].contains(&n.rem_euclid(12))).count();
        assert!(inside as f64 >= 0.8 * seen.len() as f64, "{seen:?}");
        // what she lived in it was counted, and the bridle has loosened a little
        assert!(teacher.lived[0].1 > 5.0 && teacher.bridle[0] < 1.0);
    }

    #[test]
    fn she_listens_to_the_human_and_answers_their_phrase() {
        let mut ear = Ear::default();
        let mut band = Band::default();
        let mut teacher = Teacher::default();
        // the human plays A C# E (from A1: 24 28 31), a beat each
        for (k, n) in [24, 28, 31].iter().enumerate() {
            band.human_played(k as f64 * 0.25, *n, true);
            let out = band.tick(&mut ear, k as f64 * 0.25 + 0.01, 0.25, 0.6, None, false, &mut teacher);
            assert!(!out.iter().any(|(p, v)| *p == Part::LeadGate && *v > 0.5), "silent while they play");
        }
        // they stop: she answers with its shape (the intervals 4 then 3)
        let mut answer = vec![];
        let mut t = 1.2;
        while answer.len() < 3 && t < 10.0 {
            for (part, v) in band.tick(&mut ear, t, 0.25, 0.6, None, false, &mut teacher) {
                if part == Part::LeadPitch {
                    answer.push(note_of(v));
                }
            }
            t += 0.01;
        }
        assert_eq!(answer.len(), 3, "{answer:?}");
        assert_eq!(answer[1] - answer[0], 4, "{answer:?}");
        assert!(band.said.iter().any(|s| s.contains("answers")));
    }

    #[test]
    fn she_learns_a_knob_by_watching_the_human_turn_it() {
        let mut ear = Ear { brightness: 0.2, ..Default::default() };
        ear.human_turns("cutoff", 0.2, 0.5, 0.0);
        ear.human_turns("cutoff", 0.2, 0.8, 0.1);
        // the sound got brighter as they turned it
        ear.brightness = 0.6;
        assert!(ear.settle(0.2, 0.5).is_none(), "still watching");
        let surprise = ear.settle(1.0, 0.5).expect("learned");
        assert!(surprise > 0.0);
        assert!(ear.knows["cutoff"][1] > 0.1, "{:?}", ear.knows);
        assert!((ear.prefer["cutoff"] - 0.8).abs() < 1e-9 && ear.taught == 1);
    }

    #[test]
    fn what_she_learned_stays_between_sessions() {
        let dir = std::env::temp_dir().join(format!("grm-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let memory = dir.join("space.grm");
        let mut kept = crate::music::Kept::default();
        kept.ear.taste[19] = 1.5;
        kept.teacher.bridle[3] = 0.4;
        kept.band.together[7] = 0.9;
        kept.save(&memory).unwrap();
        let back = crate::music::Kept::load(&memory).unwrap();
        assert_eq!(back.ear.taste[19], 1.5);
        assert_eq!(back.teacher.bridle[3], 0.4);
        assert_eq!(back.band.together[7], 0.9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_a_lesson_with_chords_two_voices_hold_them() {
        let mut ear = Ear::default();
        let mut band = Band::default();
        let k = crate::music::lessons().iter().position(|l| l.chords.as_deref() == Some("pop")).unwrap();
        let mut teacher = Teacher { on: true, ..Default::default() };
        teacher.choose(k);
        let mut chords = vec![];
        let mut t = 0.0;
        while t < 5.0 {
            for (part, v) in band.tick(&mut ear, t, 0.25, 0.6, None, false, &mut teacher) {
                if let Part::ChordPitch(_) = part {
                    chords.push(note_of(v));
                }
            }
            t += 0.01;
        }
        // a bar a second: about five chords, two tones each; the first (I: A major) its third and fifth
        assert!(chords.len() >= 8, "{chords:?}");
        assert_eq!((chords[0] - 36, chords[1] - 36), (4, 7));
        assert!(band.roll.iter().any(|n| n.3 == 3));
    }

    #[test]
    fn in_a_lesson_on_the_synthesiser_she_turns_each_knob_its_whole_course_and_says_what_it_does() {
        let mut ear = Ear::default();
        let knobs = vec![("cutoff".to_string(), 0.0, 1.0, 0.2)];
        // exploring: the knob goes to its far end over eight beats; the sound brightens meanwhile
        let (out, done) = ear.study(&knobs, 0.0, 0.25, None);
        assert!(out.is_empty() && done.is_none());
        let mut last = 0.2;
        let mut finished = None;
        let mut t = 0.0;
        while finished.is_none() && t < 10.0 {
            t += 0.04;
            ear.brightness = 0.1 + 0.5 * last;
            let (out, done) = ear.study(&knobs, t, 0.25, None);
            if let Some((_, v)) = out.first() {
                last = *v;
            }
            finished = done;
        }
        assert_eq!(finished.as_deref(), Some("cutoff"));
        assert!(ear.knows["cutoff"][1] > 0.1, "{:?}", ear.knows);
        let words = crate::music::knob_words("cutoff", &ear.knows["cutoff"]);
        assert!(words.contains("brighter"), "{words}");
    }

    #[test]
    fn a_turn_of_the_hand_counts_by_ratios_on_a_long_range() {
        use crate::surface::{turn_of, value_of};
        // a time from a millisecond to four seconds: the middle of the turn is about 63 ms
        let mid = value_of(0.001, 4.0, 0.5);
        assert!((mid - 0.0632).abs() < 0.001, "{mid}");
        assert!((turn_of(0.001, 4.0, mid) - 0.5).abs() < 1e-9);
        // a short range stays even
        assert!((value_of(0.0, 1.0, 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn her_left_hand_holds_chords_of_her_own_and_learns_from_them() {
        let mut ear = Ear::default();
        let mut band = Band { left: 1.5, ..Default::default() };
        let mut teacher = Teacher::default();
        let mut chords = 0;
        let mut t = 0.0;
        while t < 8.0 {
            for (part, _) in band.tick(&mut ear, t, 0.25, 0.9, None, false, &mut teacher) {
                if let Part::ChordPitch(_) = part {
                    chords += 1;
                }
            }
            t += 0.01;
        }
        assert!(chords >= 4, "{chords}");
        assert!(band.left > 1.5, "clear while it was held: she likes it more ({})", band.left);
    }

    #[test]
    fn her_bass_is_let_go_before_the_next_note_and_when_the_sequencer_takes_it() {
        let mut ear = Ear::default();
        let mut band = Band::default();
        let mut teacher = Teacher::default();
        let (mut on, mut off) = (0, 0);
        let mut t = 0.0;
        while t < 6.0 {
            for (part, v) in band.tick(&mut ear, t, 0.25, 0.6, None, false, &mut teacher) {
                if part == Part::BassGate {
                    if v > 0.5 {
                        on += 1
                    } else {
                        off += 1
                    }
                }
            }
            t += 0.01;
        }
        assert!(on >= 2 && off >= on - 1, "{on} on, {off} off");
        // the sequencer on: her bass is let go at once
        band.bass_off = Some(t + 100.0);
        let out = band.tick(&mut ear, t, 0.25, 0.6, Some(110.0), true, &mut teacher);
        assert!(out.iter().any(|(p, v)| *p == Part::BassGate && *v == 0.0));
    }

    #[test]
    fn with_the_click_she_learns_to_place_her_notes_on_the_beat() {
        let mut ear = Ear::default();
        let mut band = Band { metro_heard: true, ..Default::default() };
        let mut teacher = Teacher::default();
        let beat: f64 = 0.5;
        let mut t: f64 = 0.013;
        while t < 30.0 {
            band.beat_phase = Some((t / beat).fract());
            band.tick(&mut ear, t, beat, 0.6, None, false, &mut teacher);
            t += 0.01;
        }
        assert!(band.timing > 0.2, "{}", band.timing);
        assert!(band.off_ms < 40.0, "{}", band.off_ms);
    }
}
