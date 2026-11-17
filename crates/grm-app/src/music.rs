// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Teaching her music: lessons that bind her improvising with rules (a mode or a scale, chords, a metre and its
//! rhythm, the twelve tones), the rule held by a bridle that loosens as she masters it, until she plays free.
//! Every lesson is improvised: the rule says which notes and lengths are allowed, never which to play. Each lesson
//! is guided first (the bridle tight), accompanied (the sequencer plays its chords under her), then free; she may
//! leave a lesson for another when she stops learning in it (her curiosity: how fast what she hears improves), or
//! rest. What she lived in each lesson is kept between sessions (beside the memory), with her ear and her tastes.
//!
//! The harmony's mathematics come with it: each interval has a ratio (an octave 2/1, a fifth 3/2, a fourth 4/3, a
//! major third 5/4...), and she can see how her taste follows (or not) the simplicity of the ratio.

use serde::{Deserialize, Serialize};

/// A set of pitch classes (0 = the tonic, A), by name.
pub const SCALES: &[(&str, &str, &[i32])] = &[
    ("ionian", "major (ionian)", &[0, 2, 4, 5, 7, 9, 11]),
    ("dorian", "dorian", &[0, 2, 3, 5, 7, 9, 10]),
    ("phrygian", "phrygian", &[0, 1, 3, 5, 7, 8, 10]),
    ("lydian", "lydian", &[0, 2, 4, 6, 7, 9, 11]),
    ("mixolydian", "mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("aeolian", "minor (aeolian)", &[0, 2, 3, 5, 7, 8, 10]),
    ("locrian", "locrian", &[0, 1, 3, 5, 6, 8, 10]),
    ("pentatonic", "pentatonic major", &[0, 2, 4, 7, 9]),
    ("pentatonic_minor", "pentatonic minor", &[0, 3, 5, 7, 10]),
    ("blues", "blues", &[0, 3, 5, 6, 7, 10]),
    ("harmonic_minor", "harmonic minor", &[0, 2, 3, 5, 7, 8, 11]),
    ("hijaz", "hijaz (maqam, tempered)", &[0, 1, 4, 5, 7, 8, 10]),
    ("bhairav", "bhairav (raga, tempered)", &[0, 1, 4, 5, 7, 8, 11]),
    ("in", "in (Japan)", &[0, 1, 5, 7, 8]),
    ("pelog", "pelog (Java, tempered)", &[0, 1, 3, 7, 8]),
    ("whole_tone", "whole tone", &[0, 2, 4, 6, 8, 10]),
    ("chromatic", "chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
];

/// A metre and its rhythm: lengths in beats, one after another (a cycle), with its name.
pub const RHYTHMS: &[(&str, &str, &[f64])] = &[
    ("wholes", "wholes and halves", &[4.0, 2.0, 2.0]),
    ("four", "4/4, quarters", &[1.0, 1.0, 1.0, 1.0]),
    ("eighths", "eighths", &[0.5, 0.5]),
    ("dotted_quarter", "a dotted quarter and an eighth", &[1.5, 0.5]),
    ("dotted_eighth", "a dotted eighth and a sixteenth", &[0.75, 0.25]),
    ("triplets", "triplets", &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]),
    ("sixteenths", "sixteenths and an eighth", &[0.25, 0.25, 0.5]),
    ("every_value", "every value in a bar", &[2.0, 1.0, 0.5, 0.25, 0.25]),
    ("waltz", "3/4, a waltz", &[2.0, 1.0]),
    ("swing", "swing (long, short)", &[0.67, 0.33]),
    ("tresillo", "tresillo (3+3+2)", &[1.5, 1.5, 1.0]),
    ("son_clave", "son clave 3-2", &[1.5, 1.5, 1.0, 1.0, 1.0, 2.0]),
    ("five", "5/4 (3+2)", &[1.5, 1.0, 1.5, 1.0]),
    ("seven_eight", "7/8 (2+2+3)", &[1.0, 1.0, 1.5]),
    ("aksak", "9/8 aksak (2+2+2+3)", &[1.0, 1.0, 1.0, 1.5]),
    ("twelve_eight", "12/8 bembé", &[1.0, 0.5, 1.0, 1.0, 0.5, 1.0, 1.0]),
    ("free", "free time", &[]),
];

/// A progression of chords: their roots (from the tonic) and their tones (from the root), a bar each.
/// A chord: its root (from the tonic) and its tones (from the root).
pub type Chord = (i32, &'static [i32]);

/// A note played: when it began (seconds), how long, the note from A1, its voice (0 the bass, 1 the melody, 2 the
/// human).
pub type Note = (f64, f64, i32, u8);

pub const PROGRESSIONS: &[(&str, &str, &[Chord])] = &[
    ("ii_v_i", "ii-V-I (jazz)", &[(2, &[0, 3, 7, 10]), (7, &[0, 4, 7, 10]), (0, &[0, 4, 7, 11]), (0, &[0, 4, 7, 11])]),
    ("pop", "I-V-vi-IV", &[(0, &[0, 4, 7]), (7, &[0, 4, 7]), (9, &[0, 3, 7]), (5, &[0, 4, 7])]),
    (
        "blues12",
        "a blues (I-IV-I-V)",
        &[(0, &[0, 4, 7, 10]), (5, &[0, 4, 7, 10]), (0, &[0, 4, 7, 10]), (7, &[0, 4, 7, 10])],
    ),
    ("modal_vamp", "a modal vamp (i-IV)", &[(0, &[0, 3, 7, 10]), (5, &[0, 4, 7, 10])]),
    ("andalusian", "andalusian (i-VII-VI-V)", &[(0, &[0, 3, 7]), (10, &[0, 4, 7]), (8, &[0, 4, 7]), (7, &[0, 4, 7])]),
];

/// Jazz progressions, chord to chord: each chord its root (from the tonic) and its quality; each quality has its
/// tones and its own scale (the chord-scale way: a minor seventh dorian, a dominant mixolydian, a major seventh
/// ionian, a half-diminished locrian, an altered dominant the altered scale). A chord a bar.
/// A jazz chord: its root (from the tonic) and its quality.
pub type JazzChord = (i32, &'static str);

pub const JAZZ: &[(&str, &str, &[JazzChord])] = &[
    (
        "ii_v_cycle",
        "ii-V's down by whole tones",
        &[
            (2, "m7"),
            (7, "7"),
            (0, "m7"),
            (5, "7"),
            (10, "m7"),
            (3, "7"),
            (8, "m7"),
            (1, "7"),
            (6, "maj7"),
            (6, "maj7"),
        ],
    ),
    (
        "rhythm_changes",
        "rhythm changes (A section)",
        &[
            (0, "maj7"),
            (9, "7"),
            (2, "m7"),
            (7, "7"),
            (4, "m7"),
            (9, "7"),
            (2, "m7"),
            (7, "7"),
            (0, "maj7"),
            (0, "7"),
            (5, "maj7"),
            (5, "m7"),
            (4, "m7"),
            (9, "7"),
            (2, "m7"),
            (7, "7"),
        ],
    ),
    ("minor_ii_v", "minor ii-V-i, altered", &[(2, "m7b5"), (7, "7alt"), (0, "m7"), (0, "m7")]),
    (
        "coltrane",
        "Coltrane changes (major thirds)",
        &[
            (0, "maj7"),
            (3, "7"),
            (8, "maj7"),
            (11, "7"),
            (4, "maj7"),
            (10, "m7"),
            (3, "7"),
            (8, "maj7"),
            (11, "7"),
            (4, "maj7"),
            (7, "7"),
            (0, "maj7"),
            (6, "m7"),
            (11, "7"),
            (4, "maj7"),
            (10, "m7"),
        ],
    ),
];

/// A chord quality: its tones and its scale (from its root).
pub fn quality(q: &str) -> (&'static [i32], &'static [i32]) {
    match q {
        "m7" => (&[0, 3, 7, 10], &[0, 2, 3, 5, 7, 9, 10]),
        "7" => (&[0, 4, 7, 10], &[0, 2, 4, 5, 7, 9, 10]),
        "maj7" => (&[0, 4, 7, 11], &[0, 2, 4, 5, 7, 9, 11]),
        "m7b5" => (&[0, 3, 6, 10], &[0, 1, 3, 5, 6, 8, 10]),
        "7alt" => (&[0, 4, 10, 3, 8], &[0, 1, 3, 4, 6, 8, 10]),
        _ => (&[0, 4, 7], &[0, 2, 4, 5, 7, 9, 11]),
    }
}

/// The names of the twelve pitch classes, from A.
pub const PITCHES: [&str; 12] = ["A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#"];

/// Where a pitch class sits on the circle of fifths (A 0, E 1, B 2...).
pub fn fifths(pc: i32) -> i32 {
    (pc * 7).rem_euclid(12)
}

/// How far two keys are on the circle of fifths (0 .. 6): neighbours share all their notes but one.
pub fn fifth_distance(a: i32, b: i32) -> i32 {
    let d = (fifths(a) - fifths(b)).rem_euclid(12);
    d.min(12 - d)
}

/// How many fifths a move of tonic is, signed (-5 .. 6): a fifth up +1, a fourth up -1...
pub fn fifths_of(step: i32) -> i32 {
    let f = fifths(step);
    if f > 6 {
        f - 12
    } else {
        f
    }
}

/// The key heard in a count of pitch classes (from A), by Krumhansl and Kessler's profiles (how well each degree
/// was heard to fit a key, by listeners): the tonic, major or not, and how well it fits (a correlation).
pub fn key_of(heard: &[f64; 12]) -> (i32, bool, f64) {
    const MAJOR: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
    const MINOR: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
    let corr = |profile: &[f64; 12], tonic: usize| {
        let x: Vec<f64> = (0..12).map(|k| heard[(k + tonic) % 12]).collect();
        let (mx, my) = (x.iter().sum::<f64>() / 12.0, profile.iter().sum::<f64>() / 12.0);
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for k in 0..12 {
            sxy += (x[k] - mx) * (profile[k] - my);
            sxx += (x[k] - mx).powi(2);
            syy += (profile[k] - my).powi(2);
        }
        if sxx <= 0.0 {
            0.0
        } else {
            sxy / (sxx * syy).sqrt()
        }
    };
    let mut best = (0, true, f64::MIN);
    for t in 0..12 {
        for (major, p) in [(true, &MAJOR), (false, &MINOR)] {
            let r = corr(p, t);
            if r > best.2 {
                best = (t as i32, major, r);
            }
        }
    }
    best
}

/// The Pythagorean comma, in cents: twelve just fifths (3/2 each) against seven octaves (531441/524288).
pub fn comma() -> f64 {
    1200.0 * (531441.0f64 / 524288.0).log2()
}

/// What a just fifth is beside a tempered one, in cents (about 1.955).
pub fn fifth_cents() -> f64 {
    1200.0 * 1.5f64.log2() - 700.0
}

/// Sounds to reach on the synthesiser, by how they are heard: (name, clarity, brightness, loudness).
pub const TARGETS: &[(&str, [f64; 3])] = &[
    ("a soft pad", [0.85, 0.12, 0.15]),
    ("a bright lead", [0.7, 0.55, 0.3]),
    ("a dark bass", [0.85, 0.05, 0.3]),
    ("a bell", [0.55, 0.45, 0.2]),
    ("a quiet murmur", [0.8, 0.08, 0.05]),
];

/// What a knob does, in words: the two greatest of what she heard it do (per whole turn).
pub fn knob_words(name: &str, k: &[f64; 3]) -> String {
    let mut parts: Vec<(f64, String)> = [("clearer", "blurrier"), ("brighter", "darker"), ("louder", "softer")]
        .iter()
        .zip(k.iter())
        .map(|((up, down), v)| (v.abs(), format!("{} {:+.2}", if *v >= 0.0 { up } else { down }, v)))
        .collect();
    parts.sort_by(|a, b| b.0.total_cmp(&a.0));
    let short = name.rsplit('.').next().unwrap_or(name);
    format!("{short}, turned up: {}", parts.iter().take(2).map(|p| p.1.clone()).collect::<Vec<_>>().join(", "))
}

/// What a lesson binds: notes to a scale, to the chord sounding, to the twelve tones in turn, or nothing; lengths
/// to a rhythm or nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lesson {
    pub name: String,
    pub scale: Option<String>,
    pub chords: Option<String>,
    pub rhythm: Option<String>,
    /// Atonal: the twelve tones each before any comes back (a row she makes as she goes).
    pub twelve: bool,
    /// What changes on its own, every four bars: "mode" (the mode, on the same tonic), "key" (the tonic, the same
    /// mode), "both" (the two at once).
    pub changes: Option<String>,
    /// A jazz progression (chord to chord, each chord its scale).
    pub jazz: Option<String>,
    /// A switch the lesson turns on while it lasts (the sequencer, the arpeggiator).
    pub switch: Option<String>,
    /// A lesson on the instrument itself: the knobs she studies (what each does to the sound), her melody simple
    /// meanwhile.
    pub knobs: Option<Vec<String>>,
}

/// The lessons, from the bound to the free.
pub fn lessons() -> Vec<Lesson> {
    let l = |name: &str, scale: Option<&str>, chords: Option<&str>, rhythm: Option<&str>, twelve: bool| Lesson {
        name: name.to_string(),
        scale: scale.map(String::from),
        chords: chords.map(String::from),
        rhythm: rhythm.map(String::from),
        twelve,
        changes: None,
        jazz: None,
        knobs: None,
        switch: None,
    };
    let synth = |name: &str, knobs: &[&str]| Lesson {
        knobs: Some(knobs.iter().map(|k| k.to_string()).collect()),
        ..l(name, Some("pentatonic"), None, Some("four"), false)
    };
    let changing = |name: &str, scale: &str, changes: &str, rhythm: &str| Lesson {
        changes: Some(changes.to_string()),
        ..l(name, Some(scale), None, Some(rhythm), false)
    };
    let jazz =
        |name: &str, jazz: &str| Lesson { jazz: Some(jazz.to_string()), ..l(name, None, None, Some("swing"), false) };
    // from the simple to the free: long notes first, then shorter, dotted, in threes; modes, chords, the world's
    // scales, changing key, jazz; the twelve tones; her ears alone
    vec![
        l("pentatonic, wholes and halves", Some("pentatonic"), None, Some("wholes"), false),
        l("pentatonic, quarters", Some("pentatonic"), None, Some("four"), false),
        Lesson {
            switch: Some("seq_on".into()),
            ..l("the sequencer: a loop of eight steps", Some("pentatonic"), None, Some("four"), false)
        },
        l("major, eighths", Some("ionian"), None, Some("eighths"), false),
        l("major, a waltz", Some("ionian"), None, Some("waltz"), false),
        synth("the synthesiser: the filter", &["cutoff", "resonance", "env_amount"]),
        l("minor, dotted quarters", Some("aeolian"), None, Some("dotted_quarter"), false),
        synth("the synthesiser: the envelopes", &["aa_time", "ad_time", "a_sustain", "ar_time", "fd_time"]),
        l("dorian over a modal vamp", Some("dorian"), Some("modal_vamp"), Some("four"), false),
        l("pop chords, swing", None, Some("pop"), Some("swing"), false),
        Lesson {
            switch: Some("arp_on".into()),
            ..synth("the arpeggiator: its chords one note at a time", &["arp_mode", "arp_oct", "arp_rate"])
        },
        l("mixolydian, triplets", Some("mixolydian"), None, Some("triplets"), false),
        synth("the synthesiser: the oscillators", &["shape", "o2_level", "o2_ratio"]),
        l("minor, andalusian", Some("aeolian"), Some("andalusian"), Some("tresillo"), false),
        l("harmonic minor, dotted eighths", Some("harmonic_minor"), None, Some("dotted_eighth"), false),
        l("blues", Some("blues"), Some("blues12"), Some("swing"), false),
        synth("the synthesiser: the LFO", &["lfo_rate", "lfo_cut", "lfo_pitch", "lfo_amp", "lfo_shape"]),
        l("pentatonic minor, sixteenths", Some("pentatonic_minor"), None, Some("sixteenths"), false),
        l("phrygian, son clave", Some("phrygian"), None, Some("son_clave"), false),
        l("lydian, 5/4", Some("lydian"), None, Some("five"), false),
        synth("the synthesiser: FM", &["o3_level", "o3_ratio", "fm"]),
        synth("the synthesiser: the effects", &["echo_back", "room", "wet"]),
        l("major, every value", Some("ionian"), None, Some("every_value"), false),
        l("jazz: ii-V-I, swing", None, Some("ii_v_i"), Some("swing"), false),
        changing("modes on one tonic", "ionian", "mode", "four"),
        changing("changing key (the same mode)", "ionian", "key", "waltz"),
        l("hijaz, 9/8 aksak", Some("hijaz"), None, Some("aksak"), false),
        l("bhairav, 7/8", Some("bhairav"), None, Some("seven_eight"), false),
        l("in (Japan), free time", Some("in"), None, Some("free"), false),
        l("pelog, 12/8", Some("pelog"), None, Some("twelve_eight"), false),
        changing("mode and key at once", "dorian", "both", "swing"),
        jazz("jazz: ii-V's down by whole tones", "ii_v_cycle"),
        jazz("jazz: rhythm changes", "rhythm_changes"),
        jazz("jazz: minor ii-V-i, altered", "minor_ii_v"),
        jazz("jazz: Coltrane changes", "coltrane"),
        l("whole tone, free time", Some("whole_tone"), None, Some("free"), false),
        l("twelve tones (atonal)", None, None, Some("free"), true),
        l("free: only her ears", None, None, None, false),
    ]
}

pub fn scale(name: &str) -> Option<&'static [i32]> {
    SCALES.iter().find(|s| s.0 == name).map(|s| s.2)
}

pub fn rhythm(name: &str) -> Option<&'static [f64]> {
    RHYTHMS.iter().find(|r| r.0 == name).map(|r| r.2)
}

pub fn jazz(name: &str) -> Option<&'static [(i32, &'static str)]> {
    JAZZ.iter().find(|p| p.0 == name).map(|p| p.2)
}

pub fn progression(name: &str) -> Option<&'static [(i32, &'static [i32])]> {
    PROGRESSIONS.iter().find(|p| p.0 == name).map(|p| p.2)
}

/// The nearest note to `note` (semitones from A1) whose pitch class is in `allowed` (relative to `root`).
pub fn snap(note: i32, root: i32, allowed: &[i32]) -> i32 {
    if allowed.is_empty() {
        return note;
    }
    for d in 0..12 {
        for cand in [note + d, note - d] {
            if allowed.contains(&(cand - root).rem_euclid(12)) {
                return cand;
            }
        }
    }
    note
}

/// The ratio of an interval in just intonation (an octave folded): its numerator and denominator.
pub fn ratio(semitones: i32) -> (u32, u32) {
    const R: [(u32, u32); 12] =
        [(1, 1), (16, 15), (9, 8), (6, 5), (5, 4), (4, 3), (45, 32), (3, 2), (8, 5), (5, 3), (9, 5), (15, 8)];
    R[semitones.rem_euclid(12) as usize]
}

/// How simple a ratio is (Tenney's height: log2 of p times q; small is simple).
pub fn height(semitones: i32) -> f64 {
    let (p, q) = ratio(semitones);
    ((p * q) as f64).log2()
}

/// How her taste follows the simplicity of the ratios: the correlation between her taste for each interval and
/// minus its height (1: she likes exactly the simplest; 0: no link; below: the opposite).
pub fn follows_simplicity(taste: &[f64], span: i32) -> f64 {
    let pts: Vec<(f64, f64)> =
        (-span..=span).filter(|i| *i != 0).map(|i| (-height(i), taste[(i + span) as usize])).collect();
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in &pts {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return 0.0;
    }
    sxy / (sxx * syy).sqrt()
}

/// Her mood, between -1 and 1 (boredom and curiosity 0 .. 1).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Mood {
    pub pleasure: f64,
    pub surprise: f64,
    pub boredom: f64,
    pub curiosity: f64,
}

/// Where she is in her lessons, and what she lived in each.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Teacher {
    /// On: lessons bind her; off: she plays free (by ear alone).
    pub on: bool,
    /// Who chooses the lesson: she (by her curiosity) or the human.
    pub she_chooses: bool,
    pub lesson: usize,
    /// How tight the rule holds (1: every note bound; 0: free), for each lesson.
    pub bridle: Vec<f64>,
    /// What she lived in each lesson: (how clear on average, for how long in seconds, how fast it improved lately).
    pub lived: Vec<(f64, f64, f64)>,
    /// Where she is in the rhythm's cycle, the bar of the progression, the twelve tones used.
    #[serde(skip)]
    pub beat_at: usize,
    #[serde(skip)]
    pub bar: usize,
    #[serde(skip)]
    pub row: Vec<i32>,
    /// What she decided lately, for the window.
    #[serde(skip)]
    pub said: Vec<String>,
    /// How much she has lived since she last slept (the pressure to sleep: it rises with what is new, as Borbély's
    /// sleep pressure rises with waking); at 1 she asks to sleep.
    pub pressure: f64,
    /// Her mood, kept: how it pleases her (what she hears against what she usually hears, and the new), how it
    /// surprises her (what she hears against what she expected in this lesson), how bored she is (the same, again:
    /// the pleasure of the same fades), how curious (how fast she learns, here).
    pub mood: Mood,
    /// The lessons she found too hard for now, and left for later (no shame in it).
    pub later: Vec<usize>,
    /// Resting: until when (seconds), and why.
    #[serde(skip)]
    pub resting_until: f64,
    /// Her tonic now (a pitch class from A: hers to choose), and the mode that replaces the lesson's when it changes.
    pub tonic: i32,
    pub mode_now: Option<usize>,
    /// How long she lived each key, in each lesson (seconds).
    pub keys_lived: Vec<Vec<f64>>,
    /// Her taste for each move of tonic (0 .. 11 semitones up) and for each mode she moved to (by its place in
    /// SCALES), judged by what she heard after.
    pub moves: Vec<f64>,
    pub modes_taste: Vec<f64>,
    /// How many fifths she has moved, all her changes of key together (the comma, if each were tuned just).
    pub walk: i32,
    /// A change still to judge: (the move, the mode, what she heard before, notes since).
    #[serde(skip)]
    pub pending: Option<(i32, Option<usize>, f64, usize)>,
    /// What she heard lately (fast), to judge a change by.
    #[serde(skip)]
    pub recent: f64,
    #[serde(skip)]
    pub last_bar: usize,
    /// The lessons' names when this was kept (what she lived follows each lesson by its name, wherever it moves).
    pub order: Vec<String>,
    /// How free she is to choose her lessons (0: they follow one another, each when the one before is learned; 1:
    /// hers to choose): it grows with the lessons she has learned.
    pub freedom: f64,
    /// The sound she looks for on the synthesiser (in a lesson on it), by its place in TARGETS.
    #[serde(skip)]
    pub target: Option<usize>,
    /// How far what she hears usually strays from what she expects (her surprise is measured against it).
    pub spread: f64,
    /// How much each lesson pleased her (her pleasure while in it, kept).
    pub liked: Vec<f64>,
    /// Her notes held to the rules of two notes together, and those the rules moved.
    #[serde(skip)]
    pub harmony: (u64, u64),
    /// The key she heard the human play in: (tonic, major, how well it fits).
    #[serde(skip)]
    pub human_key: Option<(i32, bool, f64)>,
}

impl Default for Teacher {
    fn default() -> Self {
        let n = lessons().len();
        Teacher {
            on: false,
            she_chooses: true,
            lesson: 0,
            bridle: vec![1.0; n],
            lived: vec![(0.5, 0.0, 0.0); n],
            beat_at: 0,
            bar: 0,
            row: vec![],
            said: vec![],
            pressure: 0.0,
            mood: Mood::default(),
            later: vec![],
            resting_until: 0.0,
            tonic: 0,
            mode_now: None,
            keys_lived: vec![vec![0.0; 12]; n],
            moves: vec![0.0; 12],
            modes_taste: vec![0.0; SCALES.len()],
            walk: 0,
            pending: None,
            recent: 0.5,
            last_bar: 0,
            human_key: None,
            order: lessons().iter().map(|l| l.name.clone()).collect(),
            freedom: 0.0,
            harmony: (0, 0),
            liked: vec![0.0; n],
            spread: 0.1,
            target: None,
        }
    }
}

impl Teacher {
    /// Its lists as long as the lessons are (a memory kept before lessons were added).
    pub fn fit(&mut self) {
        let names: Vec<String> = lessons().iter().map(|l| l.name.clone()).collect();
        // what she lived follows each lesson by its name (the lessons may have moved, or new ones come)
        if !self.order.is_empty() && self.order != names {
            let at = |name: &String| self.order.iter().position(|o| o == name);
            let bridle = names.iter().map(|n| at(n).and_then(|k| self.bridle.get(k).copied()).unwrap_or(1.0)).collect();
            let lived = names
                .iter()
                .map(|n| at(n).and_then(|k| self.lived.get(k).copied()).unwrap_or((0.5, 0.0, 0.0)))
                .collect();
            let keys = names
                .iter()
                .map(|n| at(n).and_then(|k| self.keys_lived.get(k).cloned()).unwrap_or_else(|| vec![0.0; 12]))
                .collect();
            let later = self
                .later
                .iter()
                .filter_map(|k| self.order.get(*k))
                .filter_map(|n| names.iter().position(|x| x == n))
                .collect();
            let liked = names.iter().map(|n| at(n).and_then(|k| self.liked.get(k).copied()).unwrap_or(0.0)).collect();
            let lesson = self.order.get(self.lesson).and_then(|n| names.iter().position(|x| x == n)).unwrap_or(0);
            self.liked = liked;
            self.bridle = bridle;
            self.lived = lived;
            self.keys_lived = keys;
            self.later = later;
            self.lesson = lesson;
        }
        self.order = names;
        let n = lessons().len();
        self.bridle.resize(n, 1.0);
        self.lived.resize(n, (0.5, 0.0, 0.0));
        self.keys_lived.resize(n, vec![0.0; 12]);
        self.liked.resize(n, 0.0);
        for k in &mut self.keys_lived {
            k.resize(12, 0.0);
        }
        self.moves.resize(12, 0.0);
        self.modes_taste.resize(SCALES.len(), 0.0);
        self.lesson = self.lesson.min(n - 1);
        self.tonic = self.tonic.rem_euclid(12);
    }

    /// The key now, in words: its tonic and its mode.
    pub fn key_name(&self) -> String {
        let mode = self
            .mode_now
            .map(|m| SCALES[m].1.to_string())
            .or_else(|| {
                self.current().scale.as_deref().and_then(|s| SCALES.iter().find(|x| x.0 == s)).map(|x| x.1.to_string())
            })
            .unwrap_or_default();
        format!("{} {mode}", PITCHES[self.tonic.rem_euclid(12) as usize]).trim().to_string()
    }

    /// The scale now (the lesson's, or the mode it changed to).
    fn scale_now(&self) -> Option<&'static [i32]> {
        match self.mode_now {
            Some(m) => Some(SCALES[m].2),
            None => self.current().scale.as_deref().and_then(scale),
        }
    }

    /// The bar she is in (the chords and the changes follow it). On every fourth bar, a lesson that changes
    /// changes: guided, each time; accompanied, one time in two; free, when she wants it (bored, or for the new).
    pub fn at_bar(&mut self, bar: usize, random: f64, random2: f64) {
        self.bar = bar;
        if bar == self.last_bar {
            return;
        }
        self.last_bar = bar;
        let Some(kind) = self.current().changes.filter(|_| self.on && bar.is_multiple_of(4)) else { return };
        let wants = match self.phase() {
            "guided" => true,
            "accompanied" => random < 0.5,
            _ => self.mood.boredom > 0.5 || random < 0.15,
        };
        if wants {
            self.change(&kind, random2);
        }
    }

    /// A change: of mode, of key, or both, chosen by her taste for each (with a little chance: she keeps trying).
    fn change(&mut self, kind: &str, random: f64) {
        let mut noise = random;
        let mut next = || {
            noise = (noise * 9301.0 + 0.4927).fract();
            noise
        };
        let mut step = 0;
        if kind == "key" || kind == "both" {
            let mut best = (f64::MIN, 7);
            for i in 1..12 {
                let score = self.moves[i as usize] + next() * 0.8;
                if score > best.0 {
                    best = (score, i);
                }
            }
            step = best.1;
        }
        let mut mode = None;
        if kind == "mode" || kind == "both" {
            let now = self
                .mode_now
                .or_else(|| self.current().scale.as_deref().and_then(|s| SCALES.iter().position(|x| x.0 == s)));
            // the seven modes, and the harmonic minor
            let mut best = (f64::MIN, 0);
            for m in [0usize, 1, 2, 3, 4, 5, 6, 10] {
                if Some(m) == now {
                    continue;
                }
                let score = self.modes_taste[m] + next() * 0.8;
                if score > best.0 {
                    best = (score, m);
                }
            }
            mode = Some(best.1);
        }
        let from = self.key_name();
        self.tonic = (self.tonic + step).rem_euclid(12);
        self.walk += fifths_of(step);
        if mode.is_some() {
            self.mode_now = mode;
        }
        self.pending = Some((step, mode, self.recent, 0));
        let to = self.key_name();
        let how = match fifths_of(step) {
            0 => String::new(),
            1 => " (a fifth up)".into(),
            -1 => " (a fourth up)".into(),
            f => format!(" ({f:+} fifths)"),
        };
        self.say(format!("{from} → {to}{how}"));
    }

    /// Where she goes when she leaves a key: a key she knows less in this lesson, near on the circle of fifths while
    /// the bridle is tight, anywhere when it is loose.
    fn next_key(&self, random: f64) -> i32 {
        let k = self.lesson.min(self.keys_lived.len() - 1);
        let lived = &self.keys_lived[k];
        let reach = 1 + ((1.0 - self.bridle[k]) * 5.0).round() as i32;
        let total = lived.iter().sum::<f64>() + 1.0;
        let mut best = (f64::MIN, self.tonic);
        for t in 0..12 {
            let d = fifth_distance(t, self.tonic);
            if t == self.tonic || d > reach {
                continue;
            }
            let score = (total.ln().max(0.0) / (lived[t as usize] + 1.0)).sqrt() - 0.05 * d as f64
                + ((random * 7.0 + t as f64 * 0.37).fract()) * 0.05;
            if score > best.0 {
                best = (score, t);
            }
        }
        best.1
    }

    pub fn current(&self) -> Lesson {
        lessons()[self.lesson.min(lessons().len() - 1)].clone()
    }

    fn say(&mut self, s: String) {
        self.said.insert(0, s);
        self.said.truncate(6);
    }

    /// A note she heard while in the lesson: how clear, for how long. The bridle loosens as what she hears stays
    /// at her own level in this lesson or above (a goal within her reach, step by step: her level, not the best
    /// one), never at once; a note below it is not punished: the bridle simply stays.
    pub fn lived_note(&mut self, clarity: f64, seconds: f64, usual: f64) {
        let k = self.lesson.min(self.lived.len() - 1);
        let (mean, time, _) = self.lived[k];
        let w = (seconds / (time + seconds + 1.0)).clamp(0.02, 0.2);
        let new_mean = mean + (clarity - mean) * w;
        // how fast it improves: her curiosity for this lesson
        let progress = 0.9 * self.lived[k].2 + 0.1 * (new_mean - mean) / seconds.max(1e-3);
        self.lived[k] = (new_mean, time + seconds, progress);
        // she learns faster when it pleases her, slower when she is bored (her mood is the pace, not a clock)
        if clarity >= mean - 0.02 {
            let pace = self.pace();
            self.bridle[k] = (self.bridle[k] - 0.004 * pace).max(0.0);
        }
        if let Some(l) = self.liked.get_mut(k) {
            *l += 0.05 * (self.mood.pleasure - *l);
        }
        let key = self.tonic.rem_euclid(12) as usize;
        if let Some(keys) = self.keys_lived.get_mut(k) {
            keys[key] += seconds;
        }
        self.recent += 0.3 * (clarity - self.recent);
        // a change is judged eight notes after: what she heard since, against what she heard before
        if let Some((step, mode, before, n)) = self.pending {
            if n + 1 >= 8 {
                let reward = self.recent - before;
                if step != 0 {
                    let m = &mut self.moves[step.rem_euclid(12) as usize];
                    *m = (*m + 0.5 * reward).clamp(-2.0, 2.0);
                }
                if let Some(m) = mode {
                    self.modes_taste[m] = (self.modes_taste[m] + 0.5 * reward).clamp(-2.0, 2.0);
                }
                self.pending = None;
            } else {
                self.pending = Some((step, mode, before, n + 1));
            }
        }
        self.feel(k, clarity, mean, progress, seconds, usual);
    }

    /// What she feels on a note heard (in a lesson or not): her mood.
    pub fn felt(&mut self, clarity: f64, seconds: f64, usual: f64) {
        let k = self.lesson.min(self.lived.len() - 1);
        let (mean, _, progress) = self.lived[k];
        self.feel(k, clarity, mean, progress, seconds, usual);
    }

    fn feel(&mut self, k: usize, clarity: f64, mean: f64, progress: f64, seconds: f64, usual: f64) {
        // her mood: the surprise (against what she expected here), the pleasure (against what she usually hears,
        // and the new pleases), fading when it is the same again (boredom)
        // surprise: against how far what she hears usually strays from what she expects here (an ordinary note is
        // no surprise, however much notes vary); novelty fades too with the time spent in the same lesson and key
        let dev = (clarity - mean).abs();
        self.spread += 0.05 * (dev - self.spread);
        let z = dev / (self.spread + 0.02);
        let key_time = self.keys_lived.get(k).map(|v| v[self.tonic.rem_euclid(12) as usize]).unwrap_or(0.0);
        let novelty = ((z - 1.0) / 2.0).clamp(0.0, 1.0) / (1.0 + key_time / 120.0);
        let m = &mut self.mood;
        m.surprise += 0.2 * ((z / 3.0).min(1.0) - m.surprise);
        m.boredom = (m.boredom + 0.03 * ((1.0 - novelty) - m.boredom) * 2.0).clamp(0.0, 1.0);
        let joy = ((clarity - usual) * 4.0 + 0.5 * novelty) * (1.0 - m.boredom);
        m.pleasure = (m.pleasure + 0.1 * (joy.clamp(-1.0, 1.0) - m.pleasure)).clamp(-1.0, 1.0);
        m.curiosity = (m.curiosity + 0.1 * ((progress.abs() * 200.0).min(1.0) - m.curiosity)).clamp(0.0, 1.0);
        self.pressure += seconds / 900.0 * (0.5 + novelty);
    }

    /// She asks to sleep when what she lived weighs enough (once: the pressure falls when she has slept).
    pub fn wants_sleep(&self) -> bool {
        self.pressure >= 1.0
    }

    /// She slept: what she lived settles, the pressure falls, and she wakes rested.
    pub fn slept(&mut self, now: f64) {
        self.pressure = 0.0;
        self.mood.boredom *= 0.3;
        self.resting_until = now + 20.0;
        self.say("she slept: what she lived settles".into());
    }

    /// The phase of the lesson, from its bridle: guided, accompanied, free.
    /// How fast she learns now: faster when it pleases her, slower when she is bored.
    pub fn pace(&self) -> f64 {
        (1.0 + self.mood.pleasure.max(0.0)) * (1.0 - 0.5 * self.mood.boredom)
    }

    /// The bridle of the lesson now (1: every note bound; 0: free).
    pub fn bridle_now(&self) -> f64 {
        self.bridle.get(self.lesson).copied().unwrap_or(0.0)
    }

    pub fn phase(&self) -> &'static str {
        let b = self.bridle[self.lesson.min(self.bridle.len() - 1)];
        if b > 0.66 {
            "guided"
        } else if b > 0.33 {
            "accompanied"
        } else {
            "free"
        }
    }

    /// At the end of each of her notes she may change lesson (when she chooses): towards where she learns the
    /// fastest, the lessons she knows less counting a little more (Oudeyer's learning progress); or rest, when she
    /// is bored and learns nowhere. A lesson long below her own level, where she does not progress, she leaves for
    /// later, without shame. Never by a clock.
    pub fn consider(&mut self, now: f64, random: f64) {
        if !self.on || now < self.resting_until {
            return;
        }
        let k = self.lesson;
        let (mean, time, progress) = self.lived[k];
        let others = self.lived.iter().map(|l| l.0).sum::<f64>() / self.lived.len() as f64;
        if time > 120.0 && progress.abs() < 1e-4 && mean < others - 0.08 && !self.later.contains(&k) {
            self.later.push(k);
            self.say(format!("{}: too hard for now, she keeps it for later", self.current().name));
        }
        // bored, learning nowhere: a rest, to let it settle
        if self.mood.boredom > 0.8 && self.mood.curiosity < 0.2 && time > 30.0 {
            self.resting_until = now + 15.0;
            self.mood.boredom = 0.4;
            self.say("a rest: the same again, nothing new; she lets it settle".into());
            return;
        }
        if time < 20.0 {
            return;
        }
        // another key: when she has lived this one a while here, and it has stopped teaching her
        let key_time = self.keys_lived[k][self.tonic.rem_euclid(12) as usize];
        if self.current().changes.is_none() && key_time > 30.0 && (progress.abs() < 2e-4 || self.mood.boredom > 0.6) {
            let to = self.next_key(random);
            if to != self.tonic {
                let from = PITCHES[self.tonic as usize];
                self.walk += fifths_of(to - self.tonic);
                self.tonic = to;
                self.say(format!("to {} from {from}: a key she knows less", PITCHES[to as usize]));
                return;
            }
        }
        // how free she is: it grows with the lessons she has learned (a bridle loose by half)
        let learned = self.bridle.iter().filter(|b| **b < 0.5).count();
        self.freedom = (learned as f64 / 8.0).min(1.0);
        let second = (random * 7919.0).fract();
        if !self.she_chooses || second >= self.freedom {
            // the lessons follow one another: the next not yet learned, once this one is; a little later while it
            // pleases her and is still new, a little sooner when she is bored of it (it comes back later)
            let m = self.mood;
            let enjoying = m.pleasure > 0.5 && m.boredom < 0.3;
            let weary = m.boredom > 0.7 && m.pleasure < 0.0 && self.bridle[k] < 0.8;
            if (self.bridle[k] < 0.5 && !enjoying) || weary {
                let n = self.bridle.len();
                if let Some(next) = (1..n).map(|d| (k + d) % n).find(|j| self.bridle[*j] >= 0.5) {
                    let from = self.current().name;
                    self.choose(next);
                    let to = self.current().name;
                    self.say(if weary {
                        format!("{from}: bored of it for now, on to {to} (it comes back later)")
                    } else {
                        format!("{from} learned: on to {to}")
                    });
                }
            }
            return;
        }
        let total: f64 = self.lived.iter().map(|l| l.1).sum::<f64>() + 1.0;
        let later = self.later.clone();
        let liked = self.liked.clone();
        let weigh = |j: usize, l: (f64, f64, f64)| {
            let away = if later.contains(&j) { 0.5 } else { 1.0 };
            (l.2.abs() * 50.0 + 0.2 * (total.ln().max(0.0) / (l.1 + 1.0)).sqrt()) * away
                + 0.2 * liked.get(j).copied().unwrap_or(0.0)
        };
        // bored, the lesson here counts less
        let here = weigh(k, self.lived[k]) * (1.0 - 0.5 * self.mood.boredom);
        let (best, best_w) = self
            .lived
            .iter()
            .enumerate()
            .map(|(j, l)| (j, weigh(j, *l) + random * 0.02))
            .fold((k, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
        if best != k && best_w > here + 0.05 {
            let from = self.current().name;
            self.choose(best);
            let to = self.current().name;
            self.say(format!("from {from} to {to}: she learns more there"));
        }
    }

    /// In a lesson on the synthesiser: how near what she hears is to the sound she looks for; reached, she says
    /// so, the bridle loosens, and she looks for another. Guided, she only explores (no sound to look for yet).
    pub fn seek(&mut self, heard: [f64; 3], random: f64) -> Option<[f64; 3]> {
        if self.current().knobs.is_none() || self.phase() == "guided" {
            self.target = None;
            return None;
        }
        let t = *self.target.get_or_insert((random * TARGETS.len() as f64) as usize % TARGETS.len());
        let (name, want) = TARGETS[t];
        let d = (0..3).map(|i| (heard[i] - want[i]).powi(2)).sum::<f64>().sqrt();
        if d < 0.12 {
            let k = self.lesson;
            self.bridle[k] = (self.bridle[k] - 0.05).max(0.0);
            self.say(format!("found {name} on the synthesiser"));
            self.target = Some((t + 1 + (random * 3.0) as usize) % TARGETS.len());
        }
        Some(want)
    }

    /// The sound she looks for now, in words.
    pub fn target_name(&self) -> Option<&'static str> {
        self.target.map(|t| TARGETS[t].0)
    }

    pub fn say_out(&mut self, s: String) {
        self.say(s);
    }

    /// A lesson chosen (by her or by the human).
    pub fn choose(&mut self, lesson: usize) {
        self.lesson = lesson.min(lessons().len() - 1);
        self.beat_at = 0;
        self.row.clear();
        self.mode_now = None;
        self.pending = None;
        self.mood.boredom *= 0.5;
    }

    pub fn resting(&self, now: f64) -> bool {
        now < self.resting_until
    }

    /// The next length (in beats) the lesson allows, or None (free), by its bridle.
    pub fn length(&mut self, random: f64) -> Option<f64> {
        let lesson = self.current();
        let pattern = lesson.rhythm.as_deref().and_then(rhythm)?;
        if pattern.is_empty() || random > self.bridle[self.lesson] {
            return None;
        }
        let l = pattern[self.beat_at % pattern.len()];
        self.beat_at += 1;
        Some(l)
    }

    /// The chord sounding now (root from the tonic, tones from the root), if the lesson has chords.
    pub fn chord(&self) -> Option<(i32, &'static [i32])> {
        let lesson = self.current();
        if let Some(p) = lesson.jazz.as_deref().and_then(jazz) {
            let (root, q) = p[self.bar % p.len()];
            return Some((root, quality(q).0));
        }
        let p = lesson.chords.as_deref().and_then(progression)?;
        Some(p[self.bar % p.len()])
    }

    /// The chord now, in words (for the window).
    pub fn chord_name(&self) -> Option<String> {
        let lesson = self.current();
        let p = lesson.jazz.as_deref().and_then(jazz)?;
        let (root, q) = p[self.bar % p.len()];
        Some(format!("{}{}", PITCHES[(self.tonic + root).rem_euclid(12) as usize], q))
    }

    /// The bass's note for the chord now (a pitch class from A), if the lesson has chords.
    pub fn bass_root(&self) -> Option<i32> {
        self.chord().map(|(root, _)| (self.tonic + root).rem_euclid(12))
    }

    /// A note bound by the lesson (or left as she chose, by the bridle). `base`: the octave's A (from A1) the tonic
    /// is counted from; the tonic is hers.
    pub fn bind(&mut self, note: i32, base: i32, random: f64) -> i32 {
        let lesson = self.current();
        let tonic = base + self.tonic;
        if random > self.bridle[self.lesson] {
            return note;
        }
        if lesson.twelve {
            // the twelve tones, each before any comes back
            let pc = (note - tonic).rem_euclid(12);
            let unused: Vec<i32> = (0..12).filter(|p| !self.row.contains(p)).collect();
            let chosen = if unused.contains(&pc) { note } else { snap(note, tonic, &unused) };
            self.row.push((chosen - tonic).rem_euclid(12));
            if self.row.len() >= 12 {
                self.row.clear();
            }
            return chosen;
        }
        match self.rule(base) {
            Some((root, set)) => snap(note, root, set),
            None => note,
        }
    }

    /// What the lesson allows now: the notes of a set (pitch classes from a root, as a note from A1), or anything.
    /// Chord to chord: the chord's tones when guided, its scale when it loosens; with chords: their tones when
    /// guided (or when there is no scale), the scale when it loosens.
    fn rule(&self, base: i32) -> Option<(i32, &'static [i32])> {
        let lesson = self.current();
        let tonic = base + self.tonic;
        if lesson.twelve {
            return None;
        }
        if let Some(p) = lesson.jazz.as_deref().and_then(jazz) {
            let (root, q) = p[self.bar % p.len()];
            let (tones, sc) = quality(q);
            return Some((tonic + root, if self.phase() == "guided" { tones } else { sc }));
        }
        if let Some((root, tones)) = self.chord() {
            if self.phase() == "guided" || lesson.scale.is_none() {
                return Some((tonic + root, tones));
            }
        }
        self.scale_now().map(|s| (tonic, s))
    }

    /// Two notes together by the rules counterpoint teaches, as tight as the bridle (strict at first): on a strong
    /// beat a consonance with the bass (the unison or octave, a third, a fifth, a sixth); on a weak beat a
    /// dissonance only when reached by step (a passing note); never two fifths nor two octaves in a row with both
    /// voices going the same way. The note moves to the nearest one that keeps both the rules and the lesson.
    /// `pair`: her note before and the bass under it.
    pub fn counterpoint(
        &mut self,
        note: i32,
        pair: Option<(i32, i32)>,
        bass: i32,
        strong: bool,
        random: f64,
        base: i32,
    ) -> i32 {
        let lesson = self.current();
        if lesson.twelve || lesson.rhythm.is_none() || random > self.bridle[self.lesson] {
            return note;
        }
        const CONSONANT: [i32; 6] = [0, 3, 4, 7, 8, 9];
        // guided, her leaps are imposed: a third, a fourth, a fifth, a sixth, a seventh, an octave, a ninth (no
        // wandering by small steps yet), and every note agrees with the bass
        const LEAPS: [i32; 10] = [3, 4, 5, 7, 8, 9, 10, 11, 12, 14];
        let guided = self.phase() == "guided";
        let rule = self.rule(base);
        let ok = |c: i32| {
            let iv = (c - bass).rem_euclid(12);
            let consonant = CONSONANT.contains(&iv);
            if (strong || guided) && !consonant {
                return false;
            }
            if guided && pair.is_some_and(|(p, _)| !LEAPS.contains(&(c - p).abs())) {
                return false;
            }
            if !consonant && !pair.is_some_and(|(p, _)| (1..=2).contains(&(c - p).abs())) {
                return false;
            }
            if let Some((p, pb)) = pair {
                let before = (p - pb).rem_euclid(12);
                let same_way = (c - p).signum() == (bass - pb).signum() && c != p;
                if same_way && ((iv == 7 && before == 7) || (iv == 0 && before == 0)) {
                    return false;
                }
            }
            rule.is_none_or(|(root, set)| set.contains(&(c - root).rem_euclid(12)))
        };
        self.harmony.0 += 1;
        for d in 0..13 {
            for c in [note + d, note - d] {
                if (24..=56).contains(&c) && ok(c) {
                    if c != note {
                        self.harmony.1 += 1;
                    }
                    return c;
                }
            }
        }
        note
    }
}

/// A take: its notes (for a MIDI file), and everything written while it was recorded (seconds from its start,
/// the node, its value): notes and knobs, by hand or by her, to play it again as it was.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Take {
    pub notes: Vec<Note>,
    #[serde(default)]
    pub events: Vec<(f64, String, f64)>,
}

/// Her musical self, as kept between sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kept {
    pub ear: crate::improvise::Ear,
    pub band: crate::improvise::Band,
    pub teacher: Teacher,
    /// Her tastes on the drum machines.
    #[serde(default)]
    pub drums: crate::studio::DrumHand,
}

impl Kept {
    pub fn path(memory: &std::path::Path) -> std::path::PathBuf {
        memory.with_extension("music.json")
    }

    pub fn load(memory: &std::path::Path) -> Option<Kept> {
        let mut kept: Kept = serde_json::from_str(&std::fs::read_to_string(Self::path(memory)).ok()?).ok()?;
        kept.teacher.fit();
        kept.band.fit();
        Some(kept)
    }

    pub fn save(&self, memory: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(Self::path(memory), serde_json::to_string(self).unwrap_or_default())
    }
}

/// Notes as a standard MIDI file (format 0, one track): (when it began in seconds, how long, the note from A1,
/// its voice: the bass on channel 1, the melody on channel 2), at `bpm` beats a minute.
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
    fn lessons_bind_and_the_bridle_loosens() {
        let mut t = Teacher { on: true, lesson: 0, ..Default::default() };
        // pentatonic: a major third stays, a minor second goes to a neighbour in the scale
        assert_eq!(t.bind(24 + 4, 24, 0.0), 28);
        assert!([0, 2].contains(&(t.bind(24 + 1, 24, 0.0) - 24)));
        let before = t.bridle[0];
        for _ in 0..100 {
            t.lived_note(0.9, 0.5, 0.5);
        }
        assert!(t.bridle[0] < before, "it loosens as she hears it clear");
        // twelve tones: all twelve before one comes back
        let k = lessons().iter().position(|l| l.twelve).unwrap();
        let mut t = Teacher { on: true, lesson: k, ..Default::default() };
        let row: Vec<i32> = (0..12).map(|_| t.bind(24, 24, 0.0).rem_euclid(12)).collect();
        let mut sorted = row.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 12, "{row:?}");
    }

    #[test]
    fn the_simple_ratios_are_simple() {
        assert!(height(7) < height(6) && height(12) < height(1));
        let mut taste = vec![0.0; 25];
        for i in -12..=12i32 {
            taste[(i + 12) as usize] = -height(i);
        }
        assert!(follows_simplicity(&taste, 12) > 0.99);
    }

    #[test]
    fn a_midi_file_is_written() {
        let m = midi(&[(0.0, 0.5, 24, 0), (0.5, 0.5, 31, 1)], 120.0);
        assert_eq!(&m[..4], b"MThd");
        assert!(m.windows(4).any(|w| w == b"MTrk"));
    }

    #[test]
    fn she_hears_the_key_a_melody_is_in() {
        // C major (C is 3 from A): C D E F G A B, the tonic and the fifth longer
        let mut heard = [0.0; 12];
        for (pc, w) in [(3, 3.0), (5, 1.0), (7, 2.0), (8, 1.0), (10, 2.5), (0, 1.0), (2, 1.0)] {
            heard[pc] = w;
        }
        let (tonic, major, fit) = key_of(&heard);
        assert_eq!((tonic, major), (3, true), "{fit}");
        assert!(fit > 0.7);
    }

    #[test]
    fn a_changing_lesson_changes_key_and_judges_the_move() {
        let k = lessons().iter().position(|l| l.changes.as_deref() == Some("key")).unwrap();
        let mut t = Teacher { on: true, she_chooses: false, ..Default::default() };
        t.choose(k);
        // guided: every fourth bar the tonic moves
        t.at_bar(1, 0.3, 0.3);
        assert_eq!(t.tonic, 0);
        t.at_bar(4, 0.3, 0.3);
        assert_ne!(t.tonic, 0);
        let step = t.tonic;
        assert_eq!(t.walk, fifths_of(step));
        // what she heard after was clearer: she likes that move a little more
        t.recent = 0.4;
        t.pending = Some((step, None, 0.4, 0));
        for _ in 0..8 {
            t.lived_note(0.9, 0.5, 0.5);
        }
        assert!(t.moves[step as usize] > 0.0, "{:?}", t.moves);
        assert!(t.said.iter().any(|s| s.contains('→')));
    }

    #[test]
    fn in_jazz_each_chord_has_its_scale() {
        let k = lessons().iter().position(|l| l.jazz.as_deref() == Some("minor_ii_v")).unwrap();
        let mut t = Teacher { on: true, ..Default::default() };
        t.choose(k);
        t.tonic = 3; // C minor: D half-diminished, G altered, C minor
        t.at_bar(1, 0.9, 0.9);
        assert_eq!(t.chord_name().as_deref(), Some("G7alt"));
        // guided: on the chord's tones (G B F Bb Eb), from G (10)
        for n in 20..40 {
            let b = t.bind(n, 24, 0.0);
            assert!([0, 4, 10, 3, 8].contains(&(b - 24 - 3 - 7).rem_euclid(12)), "{n} -> {b}");
        }
        assert_eq!(t.bass_root(), Some(10));
    }

    #[test]
    fn a_memory_from_before_fits_the_lessons_now() {
        let mut t = Teacher { bridle: vec![0.5; 3], lived: vec![], keys_lived: vec![], ..Default::default() };
        t.fit();
        assert_eq!(t.bridle.len(), lessons().len());
        assert_eq!(t.bridle[0], 0.5);
        assert_eq!(t.keys_lived[5].len(), 12);
        assert!((comma() - 23.46).abs() < 0.01 && (fifth_cents() - 1.955).abs() < 0.001);
    }

    #[test]
    fn two_notes_together_keep_the_rules_while_the_bridle_is_tight() {
        let mut t = Teacher { on: true, ..Default::default() };
        t.choose(lessons().iter().position(|l| l.name == "major, eighths").unwrap());
        // over A (24), a B♭ (25) on a strong beat moves to a consonance in A major
        let c = t.counterpoint(25 + 12, None, 24, true, 0.0, 24);
        assert!([0, 3, 4, 7, 8, 9].contains(&(c - 24).rem_euclid(12)), "{c}");
        // a fifth after a fifth, both going up: not allowed
        let c = t.counterpoint(31 + 2 + 12, Some((31 + 12, 24)), 26, false, 0.0, 24);
        assert_ne!((c - 26).rem_euclid(12), 7, "{c}");
        // guided, a leap is imposed: from C#, never a step
        let c = t.counterpoint(38, Some((40, 24)), 24, false, 0.0, 24);
        assert!((c - 40).abs() >= 3, "{c}");
        // looser (accompanied), a dissonance by step stays on a weak beat (B over A, from C#)
        let k = t.lesson;
        t.bridle[k] = 0.5;
        assert_eq!(t.counterpoint(38, Some((40, 24)), 24, false, 0.0, 24), 38);
        assert!(t.harmony.0 == 4 && t.harmony.1 >= 1);
        // the bridle loosened: now and then left as she chose
        assert_eq!(t.counterpoint(25 + 12, None, 24, true, 0.99, 24), 37);
    }

    #[test]
    fn the_lessons_follow_one_another_until_she_is_free_to_choose() {
        let mut t = Teacher { on: true, ..Default::default() };
        t.lived[0] = (0.6, 30.0, 0.0);
        t.bridle[0] = 0.45;
        t.consider(100.0, 0.5);
        assert_eq!(t.lesson, 1, "{:?}", t.said);
        assert!(t.said[0].contains("learned"));
        assert!(t.freedom > 0.0 && t.freedom < 0.5);
    }

    #[test]
    fn what_she_lived_follows_each_lesson_by_its_name() {
        let mut t = Teacher::default();
        let names: Vec<String> = lessons().iter().map(|l| l.name.clone()).collect();
        // kept when the lessons were in another order (the second one first)
        t.order = vec![names[1].clone(), names[0].clone()];
        t.bridle = vec![0.3, 0.9];
        t.lived = vec![(0.7, 50.0, 0.0), (0.5, 10.0, 0.0)];
        t.keys_lived = vec![];
        t.lesson = 0;
        t.fit();
        assert_eq!(t.bridle[1], 0.3);
        assert_eq!(t.bridle[0], 0.9);
        assert_eq!(t.lived[1].1, 50.0);
        assert_eq!(t.lesson, 1);
        assert_eq!(t.bridle.len(), names.len());
    }

    #[test]
    fn the_same_again_bores_her_and_the_new_surprises_her() {
        let mut t = Teacher { on: true, ..Default::default() };
        // notes that vary as usual, again and again, in the same lesson and key
        for k in 0..400 {
            let c = 0.5 + if k % 2 == 0 { 0.15 } else { -0.15 };
            t.lived_note(c, 0.5, 0.5);
        }
        assert!(t.mood.boredom > 0.5, "{:?}", t.mood);
        assert!(t.pace() < 1.0);
        // something far from all she knew: a surprise
        let before = t.mood.surprise;
        t.lived_note(1.0, 0.5, 0.5);
        t.lived_note(1.0, 0.5, 0.5);
        assert!(t.mood.surprise > before, "{} -> {}", before, t.mood.surprise);
    }
}
