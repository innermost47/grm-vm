// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Sound files read (WAV: 8, 16, 24 or 32-bit integers, 32-bit floats; any rate, any number of channels), as one
//! channel at 48 000 samples a second, kept once read (a network played again does not read its file again).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The samples of a file, one channel, 48 000 a second (read once, then kept).
pub fn load(path: &Path) -> Option<Arc<Vec<f32>>> {
    static KEPT: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<f32>>>>> = OnceLock::new();
    let kept = KEPT.get_or_init(Default::default);
    if let Some(d) = kept.lock().ok()?.get(path) {
        return Some(d.clone());
    }
    let data = Arc::new(read(&std::fs::read(path).ok()?)?);
    kept.lock().ok()?.insert(path.to_path_buf(), data.clone());
    Some(data)
}

/// A WAV file's bytes as one channel at 48 000 samples a second.
pub fn read(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut at = 12;
    let mut data: Option<&[u8]> = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = bytes.get(at + 8..(at + 8 + size).min(bytes.len()))?;
        if id == b"fmt " && body.len() >= 16 {
            format = u16::from_le_bytes([body[0], body[1]]);
            channels = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
            bits = u16::from_le_bytes([body[14], body[15]]);
            // WAVE_FORMAT_EXTENSIBLE: the real format is in its sub-format
            if format == 0xFFFE && body.len() >= 26 {
                format = u16::from_le_bytes([body[24], body[25]]);
            }
        } else if id == b"data" {
            data = Some(body);
        }
        at += 8 + size + (size & 1);
    }
    let data = data?;
    if channels == 0 || rate == 0 {
        return None;
    }
    let width = (bits / 8) as usize;
    let frame = width * channels as usize;
    if width == 0 || frame == 0 {
        return None;
    }
    let sample = |b: &[u8]| -> f32 {
        match (format, bits) {
            (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            (1, 8) => (b[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
            (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            _ => 0.0,
        }
    };
    let mono: Vec<f32> =
        data.chunks_exact(frame).map(|f| f.chunks_exact(width).map(sample).sum::<f32>() / channels as f32).collect();
    if rate == 48_000 {
        return Some(mono);
    }
    // to 48 000 a second, each new sample between the two it falls between
    let ratio = rate as f64 / 48_000.0;
    let n = (mono.len() as f64 / ratio) as usize;
    Some(
        (0..n)
            .map(|k| {
                let x = k as f64 * ratio;
                let i = x as usize;
                let f = (x - i as f64) as f32;
                let a = mono[i.min(mono.len() - 1)];
                let b = mono[(i + 1).min(mono.len() - 1)];
                a + (b - a) * f
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_is_read_as_one_channel_at_48000() {
        // two channels, 24 000 a second, 16 bits: four frames
        let mut b = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(24_000u32.to_le_bytes());
        b.extend((24_000u32 * 4).to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend(16u32.to_le_bytes());
        for v in [0i16, 0, 16384, 16384, 0, 0, -16384, -16384] {
            b.extend(v.to_le_bytes());
        }
        let s = read(&b).unwrap();
        assert_eq!(s.len(), 8);
        assert!((s[2] - 0.5).abs() < 1e-6 && (s[1] - 0.25).abs() < 1e-6);
    }
}
