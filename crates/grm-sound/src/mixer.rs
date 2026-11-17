// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The mixer: the machine's own voice on one channel, and every sound the human makes with a network (a string,
//! an oscillator, a whole DAW) on channels of its own, each with its gain, its place, mute and solo.
//!
//! The human decides what is heard; the mixer only makes sure it never hurts: a channel always rises from silence
//! and leaves into silence (FADE), what a network gives that is not a number is silence, a constant offset is
//! removed (DC), and the whole never goes above LIMIT (a soft limit). `panic` takes everything down in a breath.

use indexmap::IndexMap;

use grm_engine::reflex::Reflex;

use crate::ambient::{Ambient, Kind};

/// The loudest the mixer gives (a soft limit: tanh).
pub const LIMIT: f64 = 0.8;
/// How long a channel takes to rise or leave (seconds).
pub const FADE: f64 = 0.03;

/// Something that makes sound, one sample at a time (in -1 .. 1, or it is limited).
pub trait Source: Send {
    fn next(&mut self) -> f64;
    /// Its next sample, left and right (a source that has only one says it twice).
    fn next_stereo(&mut self) -> (f64, f64) {
        let s = self.next();
        (s, s)
    }
    /// The nodes it listens to (the machine above tunes them).
    fn params(&self) -> Vec<String> {
        vec![]
    }
    /// Their values now, in SI.
    fn tune(&mut self, _values: &IndexMap<String, f64>) {}
    /// What it is living through (to go on from it when it is replaced by a new version of itself).
    fn state(&self) -> IndexMap<String, f64> {
        IndexMap::new()
    }
    fn set_state(&mut self, _state: &IndexMap<String, f64>) {}
    /// Starts again from nothing (what it wrote itself), its parameters kept.
    fn reset(&mut self) {}
    /// How many times it ran away and started again by itself.
    fn fused(&self) -> u64 {
        0
    }
    /// A node's value as it is now, at the speed of sound (what the machine above only sees slowly).
    fn value(&self, _node: &str) -> Option<f64> {
        None
    }
}

/// Beyond this, a sample is no sound any more but a network that ran away (a sound is about -1 .. 1).
const RUNAWAY: f64 = 64.0;

/// A node of a network played at the speed of sound: its reflex runs its sets, sample after sample.
pub struct ReflexSource {
    pub reflex: Reflex,
    slot: usize,
    /// Its right side, when it plays in stereo.
    right: Option<usize>,
    /// How many times it ran away (a value not a number, or far beyond a sound) and started again: a fuse, as an
    /// amplifier has, so that a filter pushed past what it holds does not stay screaming.
    fused: u64,
}

impl ReflexSource {
    pub fn new(reflex: Reflex, node: &str) -> Option<ReflexSource> {
        let slot = reflex.slot_of(node)?;
        // a node NAME with a NAME_r beside it plays in stereo: NAME on the left, NAME_r on the right
        let right = reflex.slot_of(&format!("{node}_r"));
        Some(ReflexSource { reflex, slot, right, fused: 0 })
    }
}

impl Source for ReflexSource {
    fn next(&mut self) -> f64 {
        self.reflex.step_read(self.slot)
    }
    fn next_stereo(&mut self) -> (f64, f64) {
        let l = self.reflex.step_read(self.slot);
        let r = self.right.map(|r| self.reflex.values[r]).unwrap_or(l);
        if !(l.is_finite() && r.is_finite() && l.abs() < RUNAWAY && r.abs() < RUNAWAY) {
            self.reflex.reset();
            self.fused += 1;
            return (0.0, 0.0);
        }
        (l, r)
    }
    fn reset(&mut self) {
        self.reflex.reset();
    }
    fn fused(&self) -> u64 {
        self.fused
    }
    fn params(&self) -> Vec<String> {
        self.reflex.params()
    }
    fn tune(&mut self, values: &IndexMap<String, f64>) {
        self.reflex.tune_values(values)
    }
    fn state(&self) -> IndexMap<String, f64> {
        self.reflex.names.iter().map(|(n, &i)| (n.clone(), self.reflex.values[i])).collect()
    }
    fn value(&self, node: &str) -> Option<f64> {
        self.reflex.value(node)
    }
    fn set_state(&mut self, state: &IndexMap<String, f64>) {
        // what it writes itself goes on from where it was (its parameters come from the machine)
        let params = self.reflex.params();
        for (n, v) in state {
            if let Some(&i) = self.reflex.names.get(n) {
                if !params.contains(n) && v.is_finite() && v.abs() < RUNAWAY {
                    self.reflex.values[i] = *v;
                }
            }
        }
    }
}

pub struct Channel {
    pub source: Box<dyn Source>,
    pub gain: f64,
    /// -1: left, 1: right.
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    envelope: f64,
    smooth_gain: f64,
    stopping: bool,
    dc: (f64, f64),
    dc_r: (f64, f64),
    pub peak: f32,
    /// The last samples it gave (what a scope shows), and where the next one goes.
    scope: Vec<f32>,
    scope_at: usize,
}

/// How many samples a channel keeps for a scope.
pub const SCOPE: usize = 2048;

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelView {
    pub name: String,
    pub gain: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub peak: f32,
}

pub struct Mixer {
    pub rate: u32,
    pub ambient: Ambient,
    pub ambient_on: bool,
    pub ambient_gain: f64,
    ambient_envelope: f64,
    pub channels: IndexMap<String, Channel>,
    pub master: f64,
    smooth_master: f64,
    /// The loudest it gave lately (it falls slowly).
    pub peak: f32,
    scratch: Vec<f32>,
    /// Parameters written straight to the sound lately, and when.
    direct: std::collections::HashMap<String, std::time::Instant>,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer {
            rate: rate.max(1),
            ambient: Ambient::new(Kind::Pad, 0.8, rate.max(1)),
            ambient_on: false,
            ambient_gain: 0.6,
            ambient_envelope: 0.0,
            channels: IndexMap::new(),
            master: 0.7,
            smooth_master: 0.7,
            peak: 0.0,
            scratch: vec![],
            direct: Default::default(),
        }
    }

    /// A source on a channel (another one of that name is replaced, after it has left).
    pub fn play(&mut self, name: &str, source: Box<dyn Source>, gain: f64) {
        let ch = Channel {
            source,
            gain: gain.clamp(0.0, 2.0),
            pan: 0.0,
            mute: false,
            solo: false,
            envelope: 0.0,
            smooth_gain: gain.clamp(0.0, 2.0),
            stopping: false,
            dc: (0.0, 0.0),
            dc_r: (0.0, 0.0),
            peak: 0.0,
            scope: vec![0.0; SCOPE],
            scope_at: 0,
        };
        self.channels.insert(name.to_string(), ch);
    }

    /// A new version of a channel's source (its network was edited while it plays): it goes on from where the old
    /// one was, without a fade, without a click. False: no such channel.
    pub fn swap(&mut self, name: &str, mut source: Box<dyn Source>) -> bool {
        match self.channels.get_mut(name) {
            Some(c) if !c.stopping => {
                source.set_state(&c.source.state());
                c.source = source;
                true
            }
            _ => false,
        }
    }

    /// What a channel gave lately, the oldest first (for a scope).
    pub fn scope(&self, name: &str) -> Option<Vec<f32>> {
        let c = self.channels.get(name)?;
        Some(c.scope[c.scope_at..].iter().chain(c.scope[..c.scope_at].iter()).copied().collect())
    }

    /// A node's value in a channel that plays, as its reflex has it now (a sequencer's step, say).
    pub fn value(&self, channel: &str, node: &str) -> Option<f64> {
        self.channels.get(channel).and_then(|c| c.source.value(node))
    }

    /// The names of the channels that play.
    pub fn playing(&self) -> Vec<String> {
        self.channels.iter().filter(|(_, c)| !c.stopping).map(|(n, _)| n.clone()).collect()
    }

    /// The channel leaves (into silence). False: no such channel.
    pub fn stop(&mut self, name: &str) -> bool {
        match self.channels.get_mut(name) {
            Some(c) => {
                c.stopping = true;
                true
            }
            None => false,
        }
    }

    /// Everything down, in a breath: every channel leaves, the voice too.
    pub fn panic(&mut self) {
        for c in self.channels.values_mut() {
            c.stopping = true;
            // and what it held starts again from nothing (a filter that ran away does not come back screaming)
            c.source.reset();
        }
        self.ambient_on = false;
    }

    /// Starts every channel's sound again from nothing, keeping them playing (when it ran away).
    pub fn reset(&mut self) {
        for c in self.channels.values_mut() {
            c.source.reset();
            c.dc = (0.0, 0.0);
            c.dc_r = (0.0, 0.0);
        }
    }

    /// How many times the channels ran away and started again by themselves, all together.
    pub fn fused(&self) -> u64 {
        self.channels.values().map(|c| c.source.fused()).sum()
    }

    pub fn views(&self) -> Vec<ChannelView> {
        self.channels
            .iter()
            .filter(|(_, c)| !c.stopping)
            .map(|(n, c)| ChannelView {
                name: n.clone(),
                gain: c.gain,
                pan: c.pan,
                mute: c.mute,
                solo: c.solo,
                peak: c.peak,
            })
            .collect()
    }

    /// What the channels listen to (the nodes to tune them with).
    pub fn params(&self) -> Vec<String> {
        let mut all: Vec<String> = vec![];
        for c in self.channels.values() {
            for p in c.source.params() {
                if !all.contains(&p) {
                    all.push(p);
                }
            }
        }
        all
    }

    pub fn tune(&mut self, values: &IndexMap<String, f64>) {
        // what was just written straight to the sound is newer than what the machine says of it
        let now = std::time::Instant::now();
        self.direct.retain(|_, at| now.duration_since(*at).as_secs_f64() < 1.0);
        let values: IndexMap<String, f64> =
            values.iter().filter(|(k, _)| !self.direct.contains_key(*k)).map(|(k, v)| (k.clone(), *v)).collect();
        for c in self.channels.values_mut() {
            c.source.tune(&values);
        }
    }

    /// A parameter written now, straight to the sound (a key, a note, a knob): heard at the next block, not when the
    /// machine next says it (a short note would be over before). For a second, the machine's older word on it
    /// is not taken.
    pub fn set_now(&mut self, node: &str, v: f64) {
        self.direct.insert(node.to_string(), std::time::Instant::now());
        let one: IndexMap<String, f64> = [(node.to_string(), v)].into_iter().collect();
        for c in self.channels.values_mut() {
            c.source.tune(&one);
        }
    }

    /// The next block (stereo, interleaved): written over `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        out.iter_mut().for_each(|x| *x = 0.0);
        let step = 1.0 / (FADE * self.rate as f64);
        // the machine's voice
        if self.ambient_on || self.ambient_envelope > 0.0 {
            self.scratch.resize(out.len(), 0.0);
            self.scratch.iter_mut().for_each(|x| *x = 0.0);
            self.ambient.render(&mut self.scratch);
            let target = if self.ambient_on { 1.0 } else { 0.0 };
            for (o, s) in out.chunks_exact_mut(2).zip(self.scratch.chunks_exact(2)) {
                self.ambient_envelope += (target - self.ambient_envelope).clamp(-step * 0.1, step * 0.1);
                let g = self.ambient_envelope * self.ambient_gain;
                o[0] += s[0] * g as f32;
                o[1] += s[1] * g as f32;
            }
        }
        // the human's channels
        let any_solo = self.channels.values().any(|c| c.solo && !c.stopping);
        let fall = (-(out.len() as f64 / 2.0) / (0.3 * self.rate as f64)).exp() as f32;
        for c in self.channels.values_mut() {
            let heard = !c.stopping && !c.mute && (!any_solo || c.solo);
            let target = if heard { 1.0 } else { 0.0 };
            let angle = (c.pan.clamp(-1.0, 1.0) + 1.0) * std::f64::consts::FRAC_PI_4;
            let (left, right) = (angle.cos(), angle.sin());
            let mut peak = c.peak * fall;
            for o in out.chunks_exact_mut(2) {
                let (mut x, mut xr) = c.source.next_stereo();
                if !x.is_finite() {
                    x = 0.0;
                }
                if !xr.is_finite() {
                    xr = 0.0;
                }
                // a constant offset is not heard: it is removed (on each side)
                let y = x - c.dc.0 + 0.995 * c.dc.1;
                c.dc = (x, y);
                let yr = xr - c.dc_r.0 + 0.995 * c.dc_r.1;
                c.dc_r = (xr, yr);
                c.envelope += (target - c.envelope).clamp(-step, step);
                c.smooth_gain += (c.gain - c.smooth_gain) * 0.001;
                let g = c.envelope * c.smooth_gain;
                let clean = |v: f64| if v.is_finite() { v.clamp(-4.0, 4.0) } else { 0.0 };
                let (v, vr) = (clean(y * g), clean(yr * g));
                peak = peak.max(v.abs().max(vr.abs()) as f32);
                c.scope[c.scope_at] = ((v + vr) / 2.0) as f32;
                c.scope_at = (c.scope_at + 1) % SCOPE;
                o[0] += (v * left) as f32;
                o[1] += (vr * right) as f32;
            }
            c.peak = peak;
        }
        self.channels.retain(|_, c| !(c.stopping && c.envelope <= 0.0));
        // the whole: the master, then the soft limit
        let mut peak = self.peak * fall;
        for o in out.iter_mut() {
            self.smooth_master += (self.master - self.smooth_master) * 0.0005;
            let v = *o as f64 * self.smooth_master;
            let v = LIMIT * (v / LIMIT).tanh();
            *o = v as f32;
            peak = peak.max(o.abs());
        }
        self.peak = peak;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grm_engine::reflex::{Clock, Reflex};
    use grm_engine::world::World;
    use grm_engine::Provenance;

    #[test]
    fn a_step_is_read_as_the_sound_has_it() {
        // a counter written at each sample: the machine above sees it only slowly, the mixer at once
        let mut w = World::new();
        w.say_all(&["var k = 0", "set k = k + 1", "node out signed", "relate o: out = 0 * k"], &Provenance::default())
            .unwrap();
        w.machine.run(100);
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        r.follow(&w.machine, "out").unwrap();
        let mut m = Mixer::new(48000);
        m.play("out", Box::new(ReflexSource::new(r, "out").unwrap()), 1.0);
        let mut block = vec![0.0f32; 2 * 480];
        m.render(&mut block);
        assert_eq!(m.value("out", "k"), Some(480.0));
    }

    #[test]
    fn a_note_written_now_is_heard_now_and_the_machine_older_word_waits() {
        let mut w = World::new();
        w.say_all(&["input g = 0", "node out signed", "relate o: out = 0.5 * g"], &Provenance::default()).unwrap();
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        r.follow(&w.machine, "out").unwrap();
        let mut m = Mixer::new(48000);
        m.play("out", Box::new(ReflexSource::new(r, "out").unwrap()), 1.0);
        m.set_now("g", 1.0);
        let mut block = vec![0.0f32; 2 * 64];
        m.render(&mut block);
        assert_eq!(m.value("out", "g"), Some(1.0));
        // the machine, a little behind, still says 0: not taken for now
        m.tune(&[("g".to_string(), 0.0)].into_iter().collect());
        assert_eq!(m.value("out", "g"), Some(1.0));
    }

    #[test]
    fn a_network_that_runs_away_starts_again_from_nothing() {
        // a value that doubles at each sample: it runs away, the fuse starts it again from nothing, and so on
        let mut w = World::new();
        w.say_all(&["var k = 1", "set k = 2 * k + 1", "node out signed", "relate o: out = k"], &Provenance::default())
            .unwrap();
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        r.follow(&w.machine, "out").unwrap();
        let mut m = Mixer::new(48000);
        m.play("out", Box::new(ReflexSource::new(r, "out").unwrap()), 1.0);
        let mut block = vec![0.0f32; 2 * 480];
        m.render(&mut block);
        assert!(m.fused() >= 1);
        assert!(block.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(m.value("out", "k").unwrap() < 64.0);
        // and started again by hand
        m.reset();
        assert_eq!(m.value("out", "k"), Some(0.0));
    }

    #[test]
    fn a_network_played_loud_stays_within_the_limit_and_panic_empties() {
        let step = std::f64::consts::TAU * 440.0 / 48000.0;
        let mut w = World::new();
        w.say_all(
            &[
                &format!("node step = {step} fixed"),
                "var c = 1",
                "var s = 0",
                "node c_new signed",
                "relate turn: c_new = c - step * s",
                "set c = c_new",
                "set s = s + step * c_new",
                "node out signed",
                "relate offset: out = s + 2",
            ],
            &Provenance::default(),
        )
        .unwrap();
        w.machine.run(1000);
        let mut r = Reflex::of(&w.machine, Clock::Audio { rate: 48000 }).unwrap();
        r.follow(&w.machine, "c_new").unwrap();
        r.follow(&w.machine, "out").unwrap();
        let mut m = Mixer::new(48000);
        m.master = 1.0;
        m.play("out", Box::new(ReflexSource::new(r, "out").unwrap()), 2.0);
        m.channels["out"].gain = 2.0;
        let mut buf = vec![0.0f32; 2 * 48000];
        m.render(&mut buf);
        assert!(buf.iter().all(|x| x.abs() <= LIMIT as f32 + 1e-6));
        // it rose from silence
        assert!(buf[0].abs() < 0.01);
        // the offset of 2 is gone: the last half second averages near 0
        let tail = &buf[48000..];
        let mean: f32 = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(mean.abs() < 0.05, "{mean}");
        assert!(m.peak > 0.3);
        m.panic();
        for _ in 0..10 {
            m.render(&mut buf[..2 * 4800]);
        }
        assert!(m.channels.is_empty());
        assert!(buf[2 * 4800 - 2..2 * 4800].iter().all(|x| x.abs() < 1e-4));
    }
}
