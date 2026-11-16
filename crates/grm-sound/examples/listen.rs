// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Writes the machine's voice into WAV files, to listen to it without running the machine:
//! 30 seconds per kind, the machine moving from calm to busy and strained, then finding something, then calm again.
//!
//!   cargo run --release -p grm-sound --example listen -- [FOLDER]

use grm_sound::ambient::{Ambient, Feeling, Kind, Voice};

fn main() {
    let folder = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let rate = 48000;
    for (kind, name) in [(Kind::Pad, "pad"), (Kind::Engine, "engine"), (Kind::Wind, "wind")] {
        let mut a = Ambient::new(kind, 0.8, rate);
        a.fade_in();
        let mut frames = vec![];
        for second in 0..30 {
            let (activity, harmony) = match second {
                0..=9 => (0.1, 1.0),
                10..=19 => (0.8, 0.4),
                _ => (0.2, 0.95),
            };
            a.feel(
                Feeling { activity, harmony, pleasure: 2.0 * harmony - 1.0, key: 0.0 },
                &[Voice { weight: 1.0, strain: 2.0 * (1.0 - harmony) }, Voice { weight: 0.6, strain: 0.0 }],
            );
            if second == 20 {
                a.discovery(1.5);
            }
            if second == 12 {
                a.surprise();
            }
            let mut block = vec![0.0f32; 2 * rate as usize];
            a.render(&mut block);
            frames.extend(block);
        }
        let path = std::path::Path::new(&folder).join(format!("voice-{name}.wav"));
        grm_sound::write_wav(&path, &frames, rate).expect("the file");
        println!("{}", path.display());
    }
}
