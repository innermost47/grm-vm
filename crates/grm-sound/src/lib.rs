// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The GRM machine heard: its own calm voice (`ambient`), the sounds the human's networks make, mixed so that they
//! never hurt (`mixer`), played by the computer's sound device (`output`) or written into a file (`write_wav`).

pub mod ambient;
pub mod mixer;
#[cfg(feature = "device")]
pub mod output;
pub mod player;

use std::io::Write;
use std::path::Path;

/// Writes stereo interleaved frames (in -1 .. 1) as a 16-bit WAV file.
pub fn write_wav(path: &Path, frames: &[f32], rate: u32) -> std::io::Result<()> {
    let data = (frames.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data.to_le_bytes())?;
    for x in frames {
        f.write_all(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    f.flush()
}

/// Without a sound device, the mixer still runs at the speed of time (into nothing): its meters, its scopes and the
/// sources' own state go on as if it were heard. It stops when dropped.
pub struct Silent {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Silent {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn silent(mixer: std::sync::Arc<std::sync::Mutex<mixer::Mixer>>) -> Silent {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("grm-silent-sound".into())
        .spawn(move || {
            let frames = 512usize;
            let mut block = vec![0.0f32; 2 * frames];
            let start = std::time::Instant::now();
            let mut done = 0u64;
            while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                let rate = match mixer.lock() {
                    Ok(mut m) => {
                        m.render(&mut block);
                        m.rate
                    }
                    Err(_) => return,
                };
                done += frames as u64;
                let due = std::time::Duration::from_secs_f64(done as f64 / rate as f64);
                if let Some(wait) = due.checked_sub(start.elapsed()) {
                    std::thread::sleep(wait);
                }
            }
        })
        .expect("the silent sound's thread");
    Silent { stop }
}
