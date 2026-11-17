// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What is played, as the window shows and keeps it: the notes lately (for the piano roll and the keys), a take
//! being recorded, takes written as standard MIDI files. Nothing here plays or decides: she is her network
//! (mind/elle.grm); this only remembers what sounded.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// A note: when it began (seconds), how long, which (semitones from A1 = 55 Hz), whose (0 the bass, 1 a melody,
/// 2 the human, 4 hers).
pub type Note = (f64, f64, i32, u8);

/// The notes lately, and the take being recorded.
#[derive(Debug, Clone, Default)]
pub struct Played {
    pub roll: VecDeque<Note>,
    pub recording: Option<Vec<Note>>,
}

impl Played {
    /// A note heard: on the roll (the last twenty seconds), and in the take if one is recorded.
    pub fn played(&mut self, at: f64, len: f64, note: i32, voice: u8) {
        if let Some(r) = &mut self.recording {
            r.push((at, len, note, voice));
        }
        self.roll.push_back((at, len, note, voice));
        while self.roll.front().is_some_and(|n| n.0 + n.1 < at - 20.0) {
            self.roll.pop_front();
        }
    }
}

/// A take: its notes, and what was turned while it played.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Take {
    pub notes: Vec<Note>,
    #[serde(default)]
    pub events: Vec<(f64, String, f64)>,
}

/// What she learned, kept beside the memory between sessions (her network's `memory` nodes).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default)]
    pub elle: indexmap::IndexMap<String, f64>,
}

impl Kept {
    pub fn path(memory: &std::path::Path) -> std::path::PathBuf {
        memory.with_extension("music.json")
    }

    pub fn load(memory: &std::path::Path) -> Option<Kept> {
        serde_json::from_str(&std::fs::read_to_string(Self::path(memory)).ok()?).ok()
    }

    pub fn save(&self, memory: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(Self::path(memory), serde_json::to_string(self).unwrap_or_default())
    }
}

/// A standard MIDI file of these notes (one track, each voice its channel).
pub fn midi(notes: &[(f64, f64, i32, u8)], bpm: f64) -> Vec<u8> {
    const TPQ: u32 = 480;
    let ticks = |s: f64| (s * bpm / 60.0 * TPQ as f64).max(0.0) as u32;
    let t0 = notes.iter().map(|n| n.0).fold(f64::MAX, f64::min);
    let t0 = if t0.is_finite() { t0 } else { 0.0 };
    // the events: (tick, on or off, channel, key)
    let mut ev: Vec<(u32, bool, u8, u8)> = vec![];
    for (at, len, note, voice) in notes {
        let key = (33 + note).clamp(0, 127) as u8; // A1 = MIDI 33
        let ch = (*voice).min(15);
        ev.push((ticks(at - t0), true, ch, key));
        ev.push((ticks(at - t0 + len.max(0.05)), false, ch, key));
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let mut track: Vec<u8> = vec![];
    let vlq = |mut v: u32, out: &mut Vec<u8>| {
        let mut bytes = vec![(v & 0x7f) as u8];
        v >>= 7;
        while v > 0 {
            bytes.push(((v & 0x7f) as u8) | 0x80);
            v >>= 7;
        }
        bytes.reverse();
        out.extend(bytes);
    };
    // the tempo
    let us = (60_000_000.0 / bpm.max(1.0)) as u32;
    track.extend([0x00, 0xff, 0x51, 0x03, (us >> 16) as u8, (us >> 8) as u8, us as u8]);
    let mut last = 0;
    for (t, on, ch, key) in ev {
        vlq(t - last, &mut track);
        last = t;
        track.extend([if on { 0x90 | ch } else { 0x80 | ch }, key, if on { 90 } else { 0 }]);
    }
    track.extend([0x00, 0xff, 0x2f, 0x00]);
    let mut out = b"MThd".to_vec();
    out.extend([0, 0, 0, 6, 0, 0, 0, 1, (TPQ >> 8) as u8, TPQ as u8]);
    out.extend(b"MTrk");
    out.extend((track.len() as u32).to_be_bytes());
    out.extend(track);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_is_a_standard_midi_file() {
        let m = midi(&[(0.0, 0.5, 24, 0), (0.5, 0.5, 31, 1)], 120.0);
        assert_eq!(&m[..4], b"MThd");
        assert!(m.windows(4).any(|w| w == b"MTrk"));
    }

    #[test]
    fn the_roll_keeps_twenty_seconds() {
        let mut p = Played::default();
        p.played(0.0, 0.5, 24, 4);
        p.played(30.0, 0.5, 26, 4);
        assert_eq!(p.roll.len(), 1);
    }
}
