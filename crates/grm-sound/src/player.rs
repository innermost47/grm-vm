// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A music file played: read and decoded beside the sound (MP3, FLAC, OGG Vorbis, WAV, AAC in MP4, by Symphonia),
//! made stereo at the mixer's rate, given to the mixer as a channel. What the window needs to know of it (where it
//! is, how long it is, its title, paused, ended) is shared with it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::mixer::Source;

/// The files a player reads, by their extension.
pub const EXTENSIONS: [&str; 7] = ["mp3", "flac", "ogg", "oga", "wav", "m4a", "mp4"];

/// Whether a file is one a player reads.
pub fn is_music(path: &Path) -> bool {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).is_some_and(|e| EXTENSIONS.contains(&e.as_str()))
}

/// What the window and the sound share of a file that plays.
#[derive(Default)]
pub struct Playing {
    pub path: PathBuf,
    pub paused: AtomicBool,
    /// Frames played (at the mixer's rate).
    pub at: AtomicU64,
    /// Frames in all (0: not known).
    pub length: AtomicU64,
    /// Read to its end and heard to its end.
    pub ended: AtomicBool,
    /// Its title and who made it, when the file says.
    pub title: Mutex<Option<String>>,
    pub error: Mutex<Option<String>>,
    /// Where to go (in frames), asked by the window and not yet done by the reader.
    seek: Mutex<Option<u64>>,
    /// Which reading the frames belong to (a seek starts a new one: what was read before is not heard).
    generation: AtomicU64,
    read_all: AtomicBool,
}

impl Playing {
    pub fn seconds(&self, rate: u32) -> (f64, f64) {
        let r = rate.max(1) as f64;
        (self.at.load(Ordering::Relaxed) as f64 / r, self.length.load(Ordering::Relaxed) as f64 / r)
    }

    /// Goes to this second of the file.
    pub fn seek(&self, seconds: f64, rate: u32) {
        let frame = (seconds.max(0.0) * rate as f64) as u64;
        if let Ok(mut s) = self.seek.lock() {
            *s = Some(frame);
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.at.store(frame, Ordering::Relaxed);
        self.ended.store(false, Ordering::Relaxed);
        self.read_all.store(false, Ordering::Relaxed);
    }
}

/// A file as a mixer's source: its frames come from the reader, ahead of the sound.
pub struct FileSource {
    pub playing: Arc<Playing>,
    frames: Receiver<(u64, Vec<f32>)>,
    chunk: Vec<f32>,
    pos: usize,
}

impl FileSource {
    /// The file, read from its start, at this rate. Its reader runs beside the sound until the source is dropped.
    pub fn open(path: &Path, rate: u32) -> FileSource {
        let playing = Arc::new(Playing { path: path.to_path_buf(), ..Default::default() });
        let (tx, rx) = sync_channel::<(u64, Vec<f32>)>(16);
        let shared = playing.clone();
        let path = path.to_path_buf();
        let _ = std::thread::Builder::new().name("grm-player".into()).spawn(move || {
            if let Err(e) = read(&path, rate, &shared, &tx) {
                if let Ok(mut err) = shared.error.lock() {
                    *err = Some(e);
                }
                shared.read_all.store(true, Ordering::Relaxed);
            }
        });
        FileSource { playing, frames: rx, chunk: vec![], pos: 0 }
    }
}

impl Source for FileSource {
    fn next(&mut self) -> f64 {
        let (l, r) = self.next_stereo();
        (l + r) / 2.0
    }

    fn next_stereo(&mut self) -> (f64, f64) {
        let p = &self.playing;
        if p.paused.load(Ordering::Relaxed) || p.ended.load(Ordering::Relaxed) {
            return (0.0, 0.0);
        }
        let generation = p.generation.load(Ordering::SeqCst);
        while self.pos + 1 >= self.chunk.len() {
            match self.frames.try_recv() {
                Ok((g, c)) if g == generation => {
                    self.chunk = c;
                    self.pos = 0;
                }
                // read before a seek: not heard
                Ok(_) => continue,
                Err(_) => {
                    // nothing read yet: silence (the reader is behind, or it has read everything)
                    if p.read_all.load(Ordering::Relaxed) {
                        p.ended.store(true, Ordering::Relaxed);
                    }
                    return (0.0, 0.0);
                }
            }
        }
        let (l, r) = (self.chunk[self.pos] as f64, self.chunk[self.pos + 1] as f64);
        self.pos += 2;
        p.at.fetch_add(1, Ordering::Relaxed);
        (l, r)
    }
}

/// From the file's rate to the mixer's, a straight line between two frames (enough for listening).
struct Resampler {
    step: f64,
    t: f64,
    prev: (f32, f32),
    started: bool,
}

impl Resampler {
    fn new(from: u32, to: u32) -> Resampler {
        Resampler { step: from.max(1) as f64 / to.max(1) as f64, t: 0.0, prev: (0.0, 0.0), started: false }
    }

    fn push(&mut self, l: f32, r: f32, out: &mut Vec<f32>) {
        if !self.started {
            self.prev = (l, r);
            self.started = true;
        }
        while self.t < 1.0 {
            let t = self.t as f32;
            out.push(self.prev.0 + (l - self.prev.0) * t);
            out.push(self.prev.1 + (r - self.prev.1) * t);
            self.t += self.step;
        }
        self.t -= 1.0;
        self.prev = (l, r);
    }
}

/// Reads the file to its end (or until no one listens), sending its frames ahead of the sound.
fn read(path: &Path, rate: u32, p: &Playing, tx: &SyncSender<(u64, Vec<f32>)>) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("{}: no sound in it", path.display()))?;
    let id = track.id;
    let file_rate = track.codec_params.sample_rate.unwrap_or(rate);
    if let Some(n) = track.codec_params.n_frames {
        p.length.store((n as f64 * rate as f64 / file_rate.max(1) as f64) as u64, Ordering::Relaxed);
    }
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // its title, and who made it (from the file's tags, wherever it keeps them)
    let mut title: Option<String> = None;
    let mut artist: Option<String> = None;
    let mut look = |rev: &symphonia::core::meta::MetadataRevision| {
        for tag in rev.tags() {
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => title = Some(tag.value.to_string()),
                Some(StandardTagKey::Artist) | Some(StandardTagKey::AlbumArtist) if artist.is_none() => {
                    artist = Some(tag.value.to_string())
                }
                _ => {}
            }
        }
    };
    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current().cloned()) {
        look(&rev);
    }
    if let Some(rev) = format.metadata().current() {
        look(rev);
    }
    if let Ok(mut t) = p.title.lock() {
        *t = match (artist, title) {
            (Some(a), Some(t)) => Some(format!("{a} — {t}")),
            (None, Some(t)) => Some(t),
            _ => None,
        };
    }
    let mut resampler = Resampler::new(file_rate, rate);
    let mut out: Vec<f32> = Vec::with_capacity(8192);
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        // the window asked for another place in the file
        let asked = p.seek.lock().ok().and_then(|mut s| s.take());
        if let Some(frame) = asked {
            let seconds = frame as f64 / rate.max(1) as f64;
            let _ = format.seek(SeekMode::Coarse, SeekTo::Time { time: Time::from(seconds), track_id: Some(id) });
            decoder.reset();
            resampler = Resampler::new(file_rate, rate);
            out.clear();
        }
        let generation = p.generation.load(Ordering::SeqCst);
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => {
                // its end: what is left is sent, then the reader waits (a seek may come back to it)
                if !out.is_empty() {
                    let _ = tx.send((generation, std::mem::take(&mut out)));
                }
                p.read_all.store(true, Ordering::Relaxed);
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if p.seek.lock().map(|s| s.is_some()).unwrap_or(false) {
                        break;
                    }
                    // no one listens any more
                    if let Err(TrySendError::Disconnected(_)) = tx.try_send((u64::MAX, vec![])) {
                        return Ok(());
                    }
                }
                continue;
            }
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let need = decoded.capacity() as u64;
        if buffer.as_ref().is_none_or(|b| b.capacity() < need as usize * channels) {
            buffer = Some(SampleBuffer::<f32>::new(need, spec));
        }
        let b = buffer.as_mut().unwrap();
        b.copy_interleaved_ref(decoded);
        for frame in b.samples().chunks_exact(channels) {
            let (l, r) = if channels == 1 { (frame[0], frame[0]) } else { (frame[0], frame[1]) };
            resampler.push(l, r, &mut out);
        }
        if out.len() >= 8192 {
            if tx.send((generation, std::mem::take(&mut out))).is_err() {
                return Ok(());
            }
            out.reserve(8192);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_file_is_played_at_the_mixer_rate_to_its_end() {
        // a second of a 440 Hz tone at 24 kHz, mono: heard as a second at 48 kHz, on both sides
        let dir = std::env::temp_dir().join(format!("grm-player-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("tone.wav");
        let rate = 24000u32;
        let mut data: Vec<u8> = vec![];
        let n = rate as usize;
        let samples: Vec<i16> =
            (0..n).map(|k| ((k as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 16000.0) as i16).collect();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(36 + 2 * n as u32).to_le_bytes());
        data.extend_from_slice(b"WAVEfmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&rate.to_le_bytes());
        data.extend_from_slice(&(rate * 2).to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&(2 * n as u32).to_le_bytes());
        for s in samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(&file, data).unwrap();
        let mut src = FileSource::open(&file, 48000);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(src.playing.length.load(Ordering::Relaxed), 48000);
        let mut heard = 0usize;
        let mut loudest = 0.0f64;
        for _ in 0..200 {
            for _ in 0..4800 {
                let (l, r) = src.next_stereo();
                if l != 0.0 {
                    heard += 1;
                }
                assert_eq!(l, r);
                loudest = loudest.max(l.abs());
            }
            if src.playing.ended.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(src.playing.ended.load(Ordering::Relaxed));
        assert!((47000..=48001).contains(&heard), "{heard}");
        assert!(loudest > 0.45 && loudest < 0.5, "{loudest}");
        // played again from its middle
        src.playing.seek(0.5, 48000);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut again = 0;
        for _ in 0..100 {
            for _ in 0..4800 {
                if src.next_stereo().0 != 0.0 {
                    again += 1;
                }
            }
            if src.playing.ended.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!((20000..=26000).contains(&again), "{again}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
