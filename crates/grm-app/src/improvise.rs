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
