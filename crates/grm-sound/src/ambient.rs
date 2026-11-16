// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine's own voice: what it feels (how much it computes, how well its relations hold, what weighs most
//! in it) heard as a calm sound. Three kinds of voice (a pad, an engine, the wind), a drone under it, bells when
//! it finds something, a soft swell when it is surprised, and a room around it.
//!
//! Calm by construction: it starts silent and rises slowly, everything it follows glides (seconds, never a jump),
//! nothing is struck hard, and it never goes above CAP.

use std::f64::consts::TAU;

/// The loudest it can ever be (the mixer's limit is above it).
pub const CAP: f64 = 0.5;
/// How long what it follows takes to glide (seconds): levels, brightness.
pub const SLOW: f64 = 3.0;
/// How long a pitch takes to glide (seconds).
pub const GLIDE: f64 = 5.0;
/// The roots it can sing on (Hz): a key is chosen among them.
pub const KEYS: [f64; 5] = [110.0, 98.0, 130.81, 146.83, 123.47];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pad,
    Engine,
    Wind,
}

/// What the machine feels, each in 0 .. 1 (pleasure in -1 .. 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feeling {
    /// How much it computes.
    pub activity: f64,
    /// How well its relations hold.
    pub harmony: f64,
    /// Good or bad news (-1 .. 1).
    pub pleasure: f64,
    /// Which root (an index into KEYS, rounded).
    pub key: f64,
}

impl Default for Feeling {
    fn default() -> Self {
        Feeling { activity: 0.0, harmony: 1.0, pleasure: 0.0, key: 0.0 }
    }
}

/// A relation that weighs: how much (0 .. 1), and how far it is from holding (in sigmas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voice {
    pub weight: f64,
    pub strain: f64,
}

/// How loud each layer is (0 .. 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layers {
    pub main: f64,
    pub drone: f64,
    pub bells: f64,
    pub reverb: f64,
}

impl Default for Layers {
    fn default() -> Self {
        Layers { main: 1.0, drone: 0.6, bells: 0.7, reverb: 0.35 }
    }
}

/// A value that glides to its target, never jumps.
#[derive(Debug, Clone, Copy)]
struct Glide {
    now: f64,
    target: f64,
    rate: f64,
}

impl Glide {
    fn new(v: f64, seconds: f64, sample_rate: f64) -> Glide {
        Glide { now: v, target: v, rate: 1.0 - (-1.0 / (seconds * sample_rate)).exp() }
    }
    fn next(&mut self) -> f64 {
        self.now += (self.target - self.now) * self.rate;
        self.now
    }
}

/// A comb filter with a damped feedback (a room's echo).
#[derive(Debug, Clone)]
struct Comb {
    buf: Vec<f64>,
    at: usize,
    store: f64,
}

impl Comb {
    fn new(len: usize) -> Comb {
        Comb { buf: vec![0.0; len.max(1)], at: 0, store: 0.0 }
    }
    fn next(&mut self, x: f64, feedback: f64, damp: f64) -> f64 {
        let y = self.buf[self.at];
        self.store = y * (1.0 - damp) + self.store * damp;
        self.buf[self.at] = x + self.store * feedback;
        self.at = (self.at + 1) % self.buf.len();
        y
    }
}

#[derive(Debug, Clone)]
struct Allpass {
    buf: Vec<f64>,
    at: usize,
}

impl Allpass {
    fn new(len: usize) -> Allpass {
        Allpass { buf: vec![0.0; len.max(1)], at: 0 }
    }
    fn next(&mut self, x: f64) -> f64 {
        let b = self.buf[self.at];
        let y = -x + b;
        self.buf[self.at] = x + b * 0.5;
        self.at = (self.at + 1) % self.buf.len();
        y
    }
}

/// A room (Freeverb-like: combs in parallel, allpasses in series, a little different on each side).
#[derive(Debug, Clone)]
struct Room {
    combs: [Vec<Comb>; 2],
    passes: [Vec<Allpass>; 2],
}

impl Room {
    fn new(rate: f64) -> Room {
        let scale = rate / 44100.0;
        let combs = [1116, 1188, 1277, 1356, 1422, 1491];
        let passes = [556, 441, 341];
        let side = |spread: usize| -> (Vec<Comb>, Vec<Allpass>) {
            (
                combs.iter().map(|&c| Comb::new(((c + spread) as f64 * scale) as usize)).collect(),
                passes.iter().map(|&p| Allpass::new(((p + spread) as f64 * scale) as usize)).collect(),
            )
        };
        let (cl, pl) = side(0);
        let (cr, pr) = side(23);
        Room { combs: [cl, cr], passes: [pl, pr] }
    }
    fn next(&mut self, x: f64) -> [f64; 2] {
        let mut out = [0.0; 2];
        for (o, (combs, passes)) in out.iter_mut().zip(self.combs.iter_mut().zip(self.passes.iter_mut())) {
            let mut y: f64 = combs.iter_mut().map(|c| c.next(x, 0.84, 0.3)).sum::<f64>() / 6.0;
            for a in passes.iter_mut() {
                y = a.next(y);
            }
            *o = y;
        }
        out
    }
}

/// A bell: inharmonic partials dying away.
#[derive(Debug, Clone)]
struct Bell {
    freq: f64,
    phase: [f64; 3],
    level: f64,
    attack: f64,
}

pub struct Ambient {
    pub kind: Kind,
    pub volume: f64,
    rate: f64,
    feeling: Feeling,
    layers: Layers,
    /// The fade of the whole voice (0: silent).
    fade: Glide,
    level: Glide,
    bright: Glide,
    root: Glide,
    tones: Vec<(Glide, Glide, f64)>, // (ratio, level, phase)
    drone_phase: f64,
    engine_phase: f64,
    noise: u64,
    low: [f64; 2],
    band: [f64; 2],
    swell: Glide,
    bells: Vec<Bell>,
    room: Room,
}

/// The chord tones (ratios to the root): open and consonant when it holds, suspended when it strains.
fn chord(harmony: f64, pleasure: f64) -> [f64; 4] {
    if harmony > 0.8 {
        if pleasure >= 0.0 {
            [1.0, 1.25, 1.5, 2.0]
        } else {
            [1.0, 1.2, 1.5, 2.0]
        }
    } else if harmony > 0.5 {
        [1.0, 4.0 / 3.0, 1.5, 2.0]
    } else {
        [1.0, 9.0 / 8.0, 1.5, 15.0 / 8.0]
    }
}

impl Ambient {
    pub fn new(kind: Kind, volume: f64, rate: u32) -> Ambient {
        let r = rate.max(1) as f64;
        let c = chord(1.0, 0.0);
        Ambient {
            kind,
            volume: volume.clamp(0.0, 1.0),
            rate: r,
            feeling: Feeling::default(),
            layers: Layers::default(),
            fade: Glide::new(0.0, 4.0, r),
            level: Glide::new(0.3, SLOW, r),
            bright: Glide::new(0.3, SLOW, r),
            root: Glide::new(KEYS[0], GLIDE, r),
            tones: c.iter().map(|&q| (Glide::new(q, GLIDE, r), Glide::new(0.5, SLOW, r), 0.0)).collect(),
            drone_phase: 0.0,
            engine_phase: 0.0,
            noise: 0x9E3779B97F4A7C15,
            low: [0.0; 2],
            band: [0.0; 2],
            swell: Glide::new(0.0, 1.5, r),
            bells: vec![],
            room: Room::new(r),
        }
    }

    /// What it feels now, and the relations that weigh most: everything glides toward it.
    pub fn feel(&mut self, feeling: Feeling, voices: &[Voice]) {
        let f = Feeling {
            activity: feeling.activity.clamp(0.0, 1.0),
            harmony: feeling.harmony.clamp(0.0, 1.0),
            pleasure: feeling.pleasure.clamp(-1.0, 1.0),
            key: feeling.key,
        };
        self.feeling = f;
        self.level.target = 0.25 + 0.35 * f.activity;
        self.bright.target = 0.15 + 0.5 * f.activity + 0.2 * f.harmony;
        let k = (f.key.round().max(0.0) as usize) % KEYS.len();
        self.root.target = KEYS[k];
        let c = chord(f.harmony, f.pleasure);
        for (j, t) in self.tones.iter_mut().enumerate() {
            // a strained relation detunes its tone a little (a slow beating, never a clash)
            let v = voices.get(j).copied().unwrap_or(Voice { weight: 0.5, strain: 0.0 });
            t.0.target = c[j] * (1.0 + 0.004 * v.strain.clamp(0.0, 3.0));
            t.1.target = v.weight.clamp(0.0, 1.0);
        }
    }

    pub fn set_layers(&mut self, layers: Layers) {
        self.layers = Layers {
            main: layers.main.clamp(0.0, 1.0),
            drone: layers.drone.clamp(0.0, 1.0),
            bells: layers.bells.clamp(0.0, 1.0),
            reverb: layers.reverb.clamp(0.0, 1.0),
        };
    }

    /// It found something: a bell, soft, at this ratio of its root.
    pub fn discovery(&mut self, ratio: f64) {
        if self.bells.len() < 4 {
            self.bells.push(Bell {
                freq: self.root.target * 2.0 * ratio.clamp(0.5, 4.0),
                phase: [0.0; 3],
                level: 0.0,
                attack: 1.0,
            });
        }
    }

    /// Something surprised it: a soft swell of air (slow to come, slow to go), never a hit.
    pub fn surprise(&mut self) {
        self.swell.target = 1.0;
    }

    pub fn fade_out(&mut self) {
        self.fade.target = 0.0;
    }

    pub fn fade_in(&mut self) {
        self.fade.target = 1.0;
    }

    /// Where it is now: (level, brightness, fade), to show it.
    pub fn sea(&self) -> (f64, f64, f64) {
        (self.level.now, self.bright.now, self.fade.now)
    }

    fn noise(&mut self) -> f64 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 7;
        self.noise ^= self.noise << 17;
        (self.noise >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }

    /// Adds its sound to `out` (stereo, interleaved).
    pub fn render(&mut self, out: &mut [f32]) {
        let dt = 1.0 / self.rate;
        for frame in out.chunks_exact_mut(2) {
            let fade = self.fade.next();
            let level = self.level.next();
            let bright = self.bright.next();
            let root = self.root.next();
            let swell = self.swell.next();
            if self.swell.now > 0.98 {
                self.swell.target = 0.0;
            }
            if fade < 1e-6 && self.fade.target == 0.0 {
                continue;
            }
            let mut main = [0.0f64; 2];
            match self.kind {
                Kind::Pad => {
                    let n = self.tones.len() as f64;
                    for (j, t) in self.tones.iter_mut().enumerate() {
                        let ratio = t.0.next();
                        let l = t.1.next();
                        t.2 = (t.2 + TAU * root * 2.0 * ratio * dt) % TAU;
                        // a soft tone: the fundamental and a little of its octave, more when it is bright
                        let s = t.2.sin() + 0.25 * bright * (2.0 * t.2).sin();
                        let pan = (j as f64 / (n - 1.0).max(1.0)) * 0.6 + 0.2;
                        main[0] += s * l * (1.0 - pan) / n;
                        main[1] += s * l * pan / n;
                    }
                }
                Kind::Engine => {
                    let speed = 0.5 + self.feeling.activity;
                    self.engine_phase = (self.engine_phase + TAU * root * 0.5 * speed * dt) % TAU;
                    let p = self.engine_phase;
                    let s = p.sin() + 0.4 * bright * (2.0 * p).sin() + 0.15 * bright * (3.0 * p).sin();
                    let hum = 0.6 + 0.4 * (p * 0.01).sin();
                    main = [s * hum * 0.5, s * hum * 0.5];
                }
                Kind::Wind => {
                    let a = 0.002 + 0.03 * bright;
                    for side in 0..2 {
                        let x = self.noise();
                        self.low[side] += a * (x - self.low[side]);
                    }
                    main = [self.low[0] * 4.0, self.low[1] * 4.0];
                }
            }
            // the drone: the root, an octave under, very soft
            self.drone_phase = (self.drone_phase + TAU * root * dt) % TAU;
            let drone = self.drone_phase.sin() * 0.3 * self.layers.drone;
            // the swell of a surprise: air, band-limited
            let mut air = [0.0; 2];
            if swell > 1e-4 {
                for (side, a) in air.iter_mut().enumerate() {
                    let x = self.noise();
                    self.band[side] += 0.01 * (x - self.band[side]);
                    *a = self.band[side] * 3.0 * swell * 0.3;
                }
            }
            // the bells
            let mut bell = 0.0;
            for b in &mut self.bells {
                if b.attack > 0.0 {
                    b.level += (1.0 - b.level) * 0.002;
                    if b.level > 0.95 {
                        b.attack = 0.0;
                    }
                } else {
                    b.level *= 1.0 - 1.0 / (2.5 * self.rate);
                }
                let ratios = [1.0, 2.76, 5.4];
                let mut s = 0.0;
                for (k, r) in ratios.iter().enumerate() {
                    b.phase[k] = (b.phase[k] + TAU * b.freq * r * dt) % TAU;
                    s += b.phase[k].sin() / (k as f64 + 1.0).powi(2);
                }
                bell += s * b.level * 0.25;
            }
            self.bells.retain(|b| b.attack > 0.0 || b.level > 1e-4);
            bell *= self.layers.bells;
            let dry = [
                main[0] * self.layers.main * level + drone + air[0] + bell * 0.6,
                main[1] * self.layers.main * level + drone + air[1] + bell * 0.4,
            ];
            let wet = self.room.next((dry[0] + dry[1]) * 0.5);
            for side in 0..2 {
                let y = (dry[side] * (1.0 - 0.5 * self.layers.reverb) + wet[side] * self.layers.reverb)
                    * fade
                    * self.volume;
                // never above CAP: a soft limit
                let y = CAP * (y / CAP).tanh();
                frame[side] += y as f32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_silent_rises_slowly_and_stays_under_its_cap() {
        for kind in [Kind::Pad, Kind::Engine, Kind::Wind] {
            let mut a = Ambient::new(kind, 1.0, 48000);
            let mut buf = vec![0.0f32; 2 * 4800];
            a.render(&mut buf);
            assert!(buf.iter().all(|x| *x == 0.0), "{kind:?}: silent before it is asked to rise");
            a.fade_in();
            a.feel(
                Feeling { activity: 1.0, harmony: 0.2, pleasure: -1.0, key: 3.0 },
                &[Voice { weight: 1.0, strain: 9.0 }],
            );
            a.discovery(1.5);
            a.surprise();
            let mut first = vec![0.0f32; 2 * 480];
            a.render(&mut first);
            let early = first.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let mut peak = 0.0f32;
            for _ in 0..20 {
                buf.iter_mut().for_each(|x| *x = 0.0);
                a.render(&mut buf);
                peak = buf.iter().fold(peak, |m, x| m.max(x.abs()));
            }
            assert!(peak <= CAP as f32 + 1e-6, "{kind:?}: {peak}");
            assert!(peak > 0.01, "{kind:?}: it is heard ({peak})");
            assert!(early < peak * 0.2, "{kind:?}: it rises slowly ({early} then {peak})");
        }
    }
}
