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
    /// What it learned while it played (its network's `memory` nodes).
    fn memories(&self) -> IndexMap<String, f64> {
        IndexMap::new()
    }
    fn value(&self, _node: &str) -> Option<f64> {
        None
    }
}

/// Beyond this, a sample is no sound any more but a network that ran away (a sound is about -1 .. 1).
const RUNAWAY: f64 = 64.0;

/// A node of a network played at the speed of sound: its reflex runs its sets, sample after sample.
pub struct ReflexSource {
    pub reflex: Reflex,
    /// The world shared among the processor's cores (its instruments, her, each on one), when it has parts heavy
    /// enough; for what nodes the channels listen to.
    parts: Option<grm_engine::reflex::Parts>,
    parts_taps: Vec<usize>,
    /// How many cores it may take (1: one, as before).
    cores: usize,
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
        Some(ReflexSource { reflex, slot, right, fused: 0, parts: None, parts_taps: vec![], cores: 1 })
    }

    /// The whole of a world's network, played once for every channel that listens to one of its nodes (and for
    /// what runs in it even when nothing is heard: her).
    pub fn whole(reflex: Reflex) -> ReflexSource {
        // the cores there are (but four at most: beyond, the parts are too small for it to pay)
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(4);
        let cores = std::env::var("GRM_CORES").ok().and_then(|c| c.parse().ok()).unwrap_or(cores);
        ReflexSource { reflex, slot: 0, right: None, fused: 0, parts: None, parts_taps: vec![], cores }
    }

    /// Its parts given back to one reflex (before it is shared out again otherwise).
    fn join(&mut self) {
        if let Some(p) = self.parts.take() {
            self.reflex.values = p.values();
            self.reflex.touch();
        }
    }

    /// How many cores compute it now (1: not shared).
    pub fn cores_used(&self) -> usize {
        self.parts.as_ref().map(|p| p.count()).unwrap_or(1)
    }

    /// A block of the world: `room` what the room heard (one a sample), `taps` what the channels listen to. Each
    /// channel's samples written to `tapped` (one list a tap, in their order); true when it ran away (and started
    /// again).
    pub fn block(
        &mut self,
        frames: usize,
        room: &[(f32, f32)],
        room_slots: (Option<usize>, Option<usize>),
        taps: &[(usize, Option<usize>)],
        tapped: &mut [Vec<(f64, f64)>],
    ) -> Option<String> {
        let mut slots: Vec<usize> = taps.iter().flat_map(|(l, r)| std::iter::once(*l).chain(*r)).collect();
        slots.sort();
        slots.dedup();
        // (shared out again only when what the channels listen to changed: not tried again at every block)
        if self.cores > 1 && (self.parts_taps != slots || self.parts_taps.is_empty() && self.parts.is_none()) {
            self.join();
            self.parts = self.reflex.parts(self.cores, &slots);
            self.parts_taps = slots;
        }
        let heard = |k: usize| room.get(k).copied().unwrap_or((0.0, 0.0));
        let mut ran_away = false;
        let mut culprit: Option<String> = None;
        if let Some(p) = self.parts.as_mut() {
            let inputs: Vec<Vec<(usize, f64)>> = (0..frames)
                .map(|k| {
                    let h = heard(k);
                    let mut v = vec![];
                    if let Some(i) = room_slots.0 {
                        v.push((i, h.0 as f64));
                    }
                    if let Some(i) = room_slots.1 {
                        v.push((i, h.1 as f64));
                    }
                    v
                })
                .collect();
            p.block(frames, &inputs);
            for k in 0..frames {
                for (t, (l, r)) in taps.iter().enumerate() {
                    let a = p.tapped(*l, k).unwrap_or(0.0);
                    let b = r.and_then(|r| p.tapped(r, k)).unwrap_or(a);
                    if !(a.is_finite() && b.is_finite() && a.abs() < RUNAWAY && b.abs() < RUNAWAY) {
                        ran_away = true;
                    }
                    tapped[t].push(if ran_away { (0.0, 0.0) } else { (a, b) });
                }
            }
            if ran_away {
                // what ran away, named (the wildest of every part) before all starts again from nothing
                let wild = p
                    .all_mut()
                    .filter_map(|r| r.wildest())
                    .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
                for r in p.all_mut() {
                    r.reset();
                }
                return Some(wild.map(|w| w.0).unwrap_or_default());
            }
            return None;
        }
        for k in 0..frames {
            let h = heard(k);
            if let Some(i) = room_slots.0 {
                self.reflex.values[i] = h.0 as f64;
            }
            if let Some(i) = room_slots.1 {
                self.reflex.values[i] = h.1 as f64;
            }
            self.reflex.step();
            let mut away = false;
            for (t, (l, r)) in taps.iter().enumerate() {
                let v = (self.reflex.values[*l], self.reflex.values[r.unwrap_or(*l)]);
                if !(v.0.is_finite() && v.1.is_finite() && v.0.abs() < RUNAWAY && v.1.abs() < RUNAWAY) {
                    away = true;
                }
                tapped[t].push(v);
            }
            if away {
                // the fuse: it starts again from nothing (what it learned kept), silent for this sample; what ran
                // away named first
                if !ran_away {
                    culprit = self.reflex.wildest().map(|w| w.0);
                }
                self.reflex.reset();
                ran_away = true;
                for t in tapped.iter_mut() {
                    if let Some(v) = t.last_mut() {
                        *v = (0.0, 0.0);
                    }
                }
            }
        }
        ran_away.then(|| culprit.unwrap_or_default())
    }
}

/// Nothing (a channel that takes its sound from the world's network has no source of its own).
struct Nothing;

impl Source for Nothing {
    fn next(&mut self) -> f64 {
        0.0
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
        if let Some(p) = self.parts.as_mut() {
            for r in p.all_mut() {
                r.reset();
            }
        }
    }
    fn fused(&self) -> u64 {
        self.fused
    }
    fn params(&self) -> Vec<String> {
        self.reflex.params().to_vec()
    }
    fn tune(&mut self, values: &IndexMap<String, f64>) {
        self.reflex.tune_values(values);
        if let Some(p) = self.parts.as_mut() {
            for r in p.all_mut() {
                r.tune_values(values);
            }
        }
    }
    fn state(&self) -> IndexMap<String, f64> {
        match &self.parts {
            Some(p) => self.reflex.names.iter().map(|(n, &i)| (n.clone(), p.value_at(i))).collect(),
            None => self.reflex.names.iter().map(|(n, &i)| (n.clone(), self.reflex.values[i])).collect(),
        }
    }
    fn value(&self, node: &str) -> Option<f64> {
        match &self.parts {
            Some(p) => self.reflex.slot_of(node).map(|i| p.value_at(i)),
            None => self.reflex.value(node),
        }
    }
    fn memories(&self) -> IndexMap<String, f64> {
        let kept = self.reflex.memories();
        match &self.parts {
            Some(p) => kept.keys().filter_map(|n| self.reflex.slot_of(n).map(|i| (n.clone(), p.value_at(i)))).collect(),
            None => kept,
        }
    }
    fn set_state(&mut self, state: &IndexMap<String, f64>) {
        // what it writes itself goes on from where it was (its parameters come from the machine)
        let base = &self.reflex;
        let writes: Vec<(usize, f64)> = state
            .iter()
            .filter_map(|(n, v)| base.names.get(n).map(|&i| (i, *v)))
            .filter(|(i, v)| !base.is_param(*i) && v.is_finite() && v.abs() < RUNAWAY)
            .collect();
        let parts = self.parts.as_mut().map(|p| p.all_mut().collect::<Vec<_>>()).unwrap_or_default();
        for r in std::iter::once(&mut self.reflex).chain(parts) {
            for &(i, v) in &writes {
                r.values[i] = v;
            }
            r.touch();
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
    /// A node of the world's network it plays (and its right side), instead of a source of its own.
    tap: Option<(usize, Option<usize>)>,
    tapped: Vec<(f64, f64)>,
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
    /// What ran away last (the node, most likely), when the sound did.
    pub ran_away: Option<String>,
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
    /// The world's network, at the speed of sound: one for all its channels, one time for all (it runs even when
    /// none of them is heard, while something lives in it).
    world: Option<ReflexSource>,
    world_fused: u64,
    /// How hard the sound works: what a block takes to compute over how long it lasts (1: just in time), and how
    /// many times the device found the reserve dry. Written by the sound's thread.
    pub load: f32,
    pub dry: u64,
    /// What the room heard at the last block, left and right (what the world hears by `room_l`, `room_r`).
    room: Vec<(f32, f32)>,
    room_slots: (Option<usize>, Option<usize>),
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer {
            rate: rate.max(1),
            ambient: Ambient::new(Kind::Pad, 0.8, rate.max(1)),
            ambient_on: false,
            ran_away: None,
            ambient_gain: 0.6,
            ambient_envelope: 0.0,
            channels: IndexMap::new(),
            master: 0.7,
            smooth_master: 0.7,
            peak: 0.0,
            scratch: vec![],
            direct: Default::default(),
            world: None,
            world_fused: 0,
            load: 0.0,
            dry: 0,
            room: vec![],
            room_slots: (None, None),
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
            tap: None,
            tapped: vec![],
            scope: vec![0.0; SCOPE],
            scope_at: 0,
        };
        self.channels.insert(name.to_string(), ch);
    }

    /// The world's network as it is now written (it goes on from where the one before was). Its channels find their
    /// nodes in it again; one whose node is gone leaves. None: nothing runs in the world any more.
    pub fn set_world(&mut self, world: Option<ReflexSource>) {
        let mut world = world;
        if let (Some(new), Some(old)) = (world.as_mut(), self.world.as_ref()) {
            new.set_state(&old.state());
        }
        self.world = world;
        let w = self.world.as_ref();
        self.room_slots = (w.and_then(|w| w.reflex.slot_of("room_l")), w.and_then(|w| w.reflex.slot_of("room_r")));
        // what the room hears comes in at every sample: what reads it is never computed slowly
        if let Some(w) = self.world.as_mut() {
            for s in [self.room_slots.0, self.room_slots.1].into_iter().flatten() {
                w.reflex.audio_input(s);
            }
        }
        let w = self.world.as_ref();
        for (name, c) in self.channels.iter_mut() {
            if c.tap.is_some() {
                match w.and_then(|w| w.reflex.slot_of(name)) {
                    Some(l) => c.tap = Some((l, w.and_then(|w| w.reflex.slot_of(&format!("{name}_r"))))),
                    None => c.stopping = true,
                }
            }
        }
    }

    /// How big the world's network is (its sets, what follows, its delays, its nodes).
    pub fn world_sizes(&self) -> Option<(usize, usize, usize, usize)> {
        self.world.as_ref().map(|w| w.reflex.sizes())
    }

    /// How much of the world's network is slow (sets, what follows): computed only when a parameter moves.
    pub fn world_slow(&self) -> Option<(usize, usize)> {
        self.world.as_ref().map(|w| w.reflex.slow_sizes())
    }

    /// How many cores compute the world's network now.
    pub fn world_cores(&self) -> usize {
        self.world.as_ref().map(|w| w.cores_used()).unwrap_or(0)
    }

    /// Whether the world's network runs.
    pub fn has_world(&self) -> bool {
        self.world.is_some()
    }

    /// A node of the world's network on a channel of its own (the world plays it; another channel of that name is
    /// replaced). False: no such node in it, or no world.
    pub fn tap(&mut self, name: &str, gain: f64) -> bool {
        let Some(w) = self.world.as_ref() else { return false };
        let Some(l) = w.reflex.slot_of(name) else { return false };
        let r = w.reflex.slot_of(&format!("{name}_r"));
        self.play(name, Box::new(Nothing), gain);
        if let Some(c) = self.channels.get_mut(name) {
            c.tap = Some((l, r));
        }
        true
    }

    /// The channels the world plays.
    pub fn taps(&self) -> Vec<String> {
        self.channels.iter().filter(|(_, c)| c.tap.is_some() && !c.stopping).map(|(n, _)| n.clone()).collect()
    }

    /// A node as the sound has it now, wherever it is computed (the world's network, or a channel's).
    pub fn sounding(&self, node: &str) -> Option<f64> {
        self.world_value(node).or_else(|| self.channels.values().find_map(|c| c.source.value(node)))
    }

    /// A node of the world's network, as the sound has it now.
    pub fn world_value(&self, node: &str) -> Option<f64> {
        self.world.as_ref().and_then(|w| w.value(node))
    }

    /// Several nodes of the world's network at once (one look, one lock).
    pub fn world_values(&self, nodes: &[&str]) -> Vec<Option<f64>> {
        nodes.iter().map(|n| self.world_value(n)).collect()
    }

    /// What the world's network learned (its `memory` nodes).
    pub fn world_memories(&self) -> IndexMap<String, f64> {
        self.world.as_ref().map(|w| w.memories()).unwrap_or_default()
    }

    /// Gives the world's network back what it had learned.
    pub fn world_restore(&mut self, values: &IndexMap<String, f64>) {
        if let Some(w) = self.world.as_mut() {
            w.set_state(values);
        }
    }

    /// A new version of a channel's source (its network was edited while it plays): it goes on from where the old
    /// one was, without a fade, without a click. False: no such channel.
    pub fn swap(&mut self, name: &str, mut source: Box<dyn Source>) -> bool {
        match self.channels.get_mut(name) {
            Some(c) if !c.stopping && c.tap.is_none() => {
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
        match self.channels.get(channel) {
            Some(c) if c.tap.is_some() => self.world_value(node),
            Some(c) => c.source.value(node),
            None => None,
        }
    }

    /// What a channel learned while it played (its `memory` nodes), to be kept.
    pub fn memories(&self, channel: &str) -> IndexMap<String, f64> {
        match self.channels.get(channel) {
            Some(c) if c.tap.is_some() => self.world_memories(),
            Some(c) => c.source.memories(),
            None => IndexMap::new(),
        }
    }

    /// Gives a channel back what it had learned (its `memory` nodes, and anything else it writes itself).
    pub fn restore(&mut self, channel: &str, values: &IndexMap<String, f64>) {
        match self.channels.get_mut(channel) {
            Some(c) if c.tap.is_some() => self.world_restore(values),
            Some(c) => c.source.set_state(values),
            None => {}
        }
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
        if let Some(w) = self.world.as_mut() {
            w.reset();
        }
    }

    /// How many times the channels ran away and started again by themselves, all together.
    pub fn fused(&self) -> u64 {
        self.channels.values().map(|c| c.source.fused()).sum::<u64>() + self.world_fused
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
        let mut all: indexmap::IndexSet<String> = indexmap::IndexSet::new();
        for c in self.channels.values() {
            all.extend(c.source.params());
        }
        // the world's network listens even when no channel does (she runs on)
        if let Some(w) = self.world.as_ref() {
            all.extend(w.reflex.params().iter().cloned());
        }
        all.into_iter().collect()
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
        if let Some(w) = self.world.as_mut() {
            w.tune(&values);
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
        if let Some(w) = self.world.as_mut() {
            w.tune(&one);
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
        // the world's network, one sample after another for the whole block: what the room heard comes in, what
        // its channels play goes out to them
        let frames = out.len() / 2;
        if let Some(w) = self.world.as_mut() {
            for c in self.channels.values_mut() {
                c.tapped.clear();
            }
            let taps: Vec<(usize, Option<usize>)> = self.channels.values().filter_map(|c| c.tap).collect();
            let mut tapped: Vec<Vec<(f64, f64)>> = vec![Vec::with_capacity(frames); taps.len()];
            if let Some(node) = w.block(frames, &self.room, self.room_slots, &taps, &mut tapped) {
                self.world_fused += 1;
                self.ran_away = Some(node);
            }
            let mut it = tapped.into_iter();
            for c in self.channels.values_mut() {
                if c.tap.is_some() {
                    c.tapped = it.next().unwrap_or_default();
                }
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
            for (k, o) in out.chunks_exact_mut(2).enumerate() {
                let (mut x, mut xr) = match c.tap {
                    Some(_) => c.tapped.get(k).copied().unwrap_or((0.0, 0.0)),
                    None => c.source.next_stereo(),
                };
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
        // what the room heard (every channel, before the master), for the world to hear at the next block
        self.room.clear();
        self.room.extend(out.chunks_exact(2).map(|o| (o[0], o[1])));
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
