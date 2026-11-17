// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Improvising by ear: when an instrument's sequencer is on and its `auto_on` node is 1, the machine writes the
//! next notes itself, and listens to what comes out. No scale is given: it starts from every interval alike, hears
//! how clear the sound is while each note plays (how periodic what comes out is: two notes that agree sound as one
//! clearer wave, two that clash do not; the echo and the room carry the note before into the next), and leans
//! towards the intervals that sounded clear, away from the ones that did not, a little at each step. What it learned
//! is shown, interval by interval: nothing is hidden.

use std::collections::VecDeque;

/// Intervals it may take, in semitones (an octave down to an octave up).
pub const SPAN: i32 = 12;

#[derive(Debug, Clone)]
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
    /// A knob she is trying: (node, its value before, the step she turned it on); kept if it sounded clearer.
    trying: Option<(String, f64, i64)>,
    /// What she did with the knobs lately, for the window: (node, kept or not).
    pub turned: VecDeque<(String, bool)>,
    /// The last note the human played (semitones from A1), and how many intervals she learned from them.
    human_note: Option<i32>,
    pub from_human: usize,
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
            trying: None,
            turned: VecDeque::new(),
            human_note: None,
            from_human: 0,
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

    /// Between notes, now and then, she turns one of the instrument's knobs a little (`knobs`: node, lo, hi, now),
    /// and listens: clearer than usual, she keeps it; not, she turns it back. Returns the values to write.
    pub fn play_knobs(&mut self, knobs: &[(String, f64, f64, f64)], step: i64) -> Vec<(String, f64)> {
        let mut out = vec![];
        // a knob tried two steps ago is judged by what was heard since
        if let Some((name, before, at)) = self.trying.clone() {
            if step - at >= 2 {
                let heard = self.judged.iter().take(2).map(|j| j.1).sum::<f64>() / 2.0;
                let kept = heard >= self.usual;
                if !kept {
                    out.push((name.clone(), before));
                }
                self.turned.push_front((name, kept));
                self.turned.truncate(8);
                self.trying = None;
            }
            return out;
        }
        if knobs.is_empty() || self.random() > 0.25 {
            return out;
        }
        let k = (self.random() * knobs.len() as f64) as usize % knobs.len();
        let (name, lo, hi, now) = knobs[k].clone();
        let span = hi - lo;
        let to = (now + (self.random() * 2.0 - 1.0) * 0.12 * span).clamp(lo.min(hi), hi.max(lo));
        self.trying = Some((name.clone(), now, step));
        out.push((name, to));
        out
    }

    /// One frame: what it hears now, the step the sequencer is on. Returns the notes to write: (step 0..7, Hz).
    pub fn listen(&mut self, samples: &[f32], rate: f64, step: i64) -> Vec<(usize, f64)> {
        self.clarity = clarity(samples, rate);
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
        self.notes[k] = (before + i).clamp(0, 48);
        self.made_by[k] = Some(i);
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
            ear.listen(&s, 48000.0, step);
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
}

/// Playing freely: a melody over the bass (or the sequence), and, the sequencer off, the bass too; now and then
/// she turns the sequencer on or off herself. Each note is chosen by her taste (the intervals in time, as the ear
/// learns them, and the intervals with the bass at the same moment, learned here), held for a length she chooses,
/// and judged by how clear what comes out was while it sounded.
#[derive(Debug, Clone)]
pub struct Band {
    /// Her taste for each interval with the bass at the same moment (0 .. 11 semitones, an octave folded).
    pub together: Vec<f64>,
    lead: i32,
    lead_until: f64,
    lead_on: bool,
    lead_heard: Vec<f64>,
    lead_with: Option<(i32, i32)>,
    bass: i32,
    bass_until: f64,
    /// What she heard in each way of playing, with or without the sequencer: (how clear on average, how long she
    /// played so). She decides from it, nothing else: no clock tells her when.
    pub ways: [(f64, f64); 2],
    last_now: f64,
    /// What she did lately, in words for the window.
    pub said: VecDeque<String>,
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
            bass: 24,
            bass_until: 0.0,
            ways: [(0.5, 0.0); 2],
            last_now: 0.0,
            said: VecDeque::new(),
            seed: 0x2545_f491_4f6c_dd1d,
        }
    }
}

fn hz(note: i32) -> f64 {
    55.0 * 2f64.powf(note as f64 / 12.0)
}

fn note_of(hz: f64) -> i32 {
    (12.0 * (hz / 55.0).log2()).round() as i32
}

impl Band {
    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A length, in beats: an eighth, a quarter, a half, a whole.
    fn length(&mut self) -> f64 {
        [0.5, 1.0, 1.0, 2.0, 4.0][(self.random() * 5.0) as usize % 5]
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
    pub fn tick(
        &mut self,
        ear: &mut Ear,
        now: f64,
        beat: f64,
        clarity: f64,
        bass_hz: Option<f64>,
        seq_on: bool,
    ) -> Vec<(Part, f64)> {
        let mut out = vec![];
        if self.lead_on {
            self.lead_heard.push(clarity);
        }
        // what she hears, counted for the way she plays now
        let dt = (now - self.last_now).clamp(0.0, 0.5);
        self.last_now = now;
        let way = &mut self.ways[seq_on as usize];
        way.1 += dt;
        way.0 += (clarity - way.0) * (dt / (way.1 + 1.0)).min(0.2);
        // the bass: the sequence's, or her own (the sequencer off), a long note at a time
        let bass = match bass_hz {
            Some(h) if seq_on && h > 20.0 => note_of(h),
            _ => {
                if now >= self.bass_until {
                    let i = ear.choose_from(self.bass, 0, 36);
                    self.bass = (self.bass + i).clamp(0, 36);
                    self.bass_until = now + beat * 2.0 * self.length().max(1.0);
                    out.push((Part::BassPitch, hz(self.bass)));
                    out.push((Part::BassGate, 1.0));
                }
                self.bass
            }
        };
        // the melody: when a note ends, it is judged, and the next chosen (or a rest)
        let ended = now >= self.lead_until;
        if ended {
            if let Some((with, step)) = self.lead_with.take() {
                if !self.lead_heard.is_empty() {
                    let heard = self.lead_heard.iter().sum::<f64>() / self.lead_heard.len() as f64;
                    let reward = heard - ear.usual;
                    let k = with.rem_euclid(12) as usize;
                    self.together[k] = (self.together[k] + 0.5 * reward).clamp(-2.0, 2.0);
                    ear.taste[(step.clamp(-SPAN, SPAN) + SPAN) as usize] =
                        (ear.taste[(step.clamp(-SPAN, SPAN) + SPAN) as usize] + 0.3 * reward).clamp(-2.0, 2.0);
                }
            }
            self.lead_heard.clear();
            if self.random() < 0.18 {
                // a rest: silence is part of it
                self.lead_on = false;
                out.push((Part::LeadGate, 0.0));
            } else {
                let to = self.choose_lead(ear, bass);
                self.lead_with = Some((to - bass, to - self.lead));
                self.lead = to;
                self.lead_on = true;
                out.push((Part::LeadPitch, hz(to)));
                out.push((Part::LeadGate, 1.0));
            }
            self.lead_until = now + beat * self.length();
        }
        // with or without the sequencer: hers to decide. At the end of each of her notes she weighs both ways by
        // what she heard in each, the one she knows less counting a little more (it might be better: she tries it);
        // she changes only when the other way weighs more than this one
        if ended && self.ways[0].1 + self.ways[1].1 > 2.0 {
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
    fn she_chooses_the_way_that_sounded_clearer() {
        // with the sequencer she hears clearer: she ends up playing with it most of the time, by herself
        let (mut ear, mut band) = (Ear::default(), Band::default());
        let mut seq = false;
        let (mut with, mut without) = (0.0, 0.0);
        let mut t = 0.0;
        for _ in 0..60000 {
            let clear = if seq { 0.9 } else { 0.3 };
            for (part, v) in band.tick(&mut ear, t, 0.25, clear, Some(110.0), seq) {
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
            let out = band.tick(&mut ear, t, 0.25, clear, None, false);
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
    }
}
