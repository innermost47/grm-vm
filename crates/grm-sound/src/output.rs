// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The computer's sound device, playing the mixer. The sound is computed ahead, by a thread of its own, into a
//! short reserve (about sixteen milliseconds): the device only takes from it, and never waits for the mixer. A moment
//! where the mixer is held (the window reading it, the network made again) or slower to compute is absorbed by the
//! reserve; if the reserve runs dry, the sound fades to silence and back rather than clicking, and it is counted.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::mixer::Mixer;

/// While it lives, the device plays.
pub struct Output {
    _stream: cpal::Stream,
    pub rate: u32,
    pub device: String,
    running: Arc<AtomicBool>,
}

impl Drop for Output {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// How much is computed at once by the sound's thread (frames).
const BLOCK: usize = 128;

/// Opens the default output device and plays the mixer through it. The mixer takes the device's rate.
pub fn open(mixer: Arc<Mutex<Mixer>>) -> Result<Output, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no sound device")?;
    let name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "a sound device".into());
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let rate = config.sample_rate();
    let channels = config.channels() as usize;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("{name} does not take 32-bit floats"));
    }
    {
        let mut m = mixer.lock().map_err(|_| "the mixer")?;
        if m.rate != rate {
            let (on, gain, master, kind, volume) =
                (m.ambient_on, m.ambient_gain, m.master, m.ambient.kind, m.ambient.volume);
            *m = Mixer::new(rate);
            m.ambient_on = on;
            m.ambient_gain = gain;
            m.master = master;
            m.ambient = crate::ambient::Ambient::new(kind, volume, rate);
            if on {
                m.ambient.fade_in();
            }
        }
    }
    // the reserve: stereo frames computed ahead, taken by the device
    let reserve: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(16_384)));
    let running = Arc::new(AtomicBool::new(true));
    let ahead = (rate as usize * 16 / 1000).max(2 * BLOCK);
    {
        let (reserve, running, mixer) = (reserve.clone(), running.clone(), mixer.clone());
        std::thread::Builder::new()
            .name("grm-sound".into())
            .spawn(move || compute_ahead(&mixer, &reserve, &running, rate, ahead))
            .map_err(|e| e.to_string())?;
    }
    let mut stereo: Vec<f32> = vec![];
    // the last sample played, and whether the block before ran dry (to come back without a click)
    let mut last = [0.0f32; 2];
    let mut lost = false;
    let dry = mixer.clone();
    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let frames = data.len() / channels.max(1);
                stereo.resize(2 * frames, 0.0);
                const RAMP: usize = 96;
                // what the reserve holds (it is held by the sound's thread only while it adds a block)
                let mut got = 0;
                if let Ok(mut r) = reserve.lock() {
                    got = (r.len() / 2).min(frames);
                    for (k, x) in r.drain(..2 * got).enumerate() {
                        stereo[k] = x;
                    }
                }
                if got == frames {
                    if lost {
                        // back after running dry: it rises again instead of jumping (a jump is a click)
                        for k in 0..frames.min(RAMP) {
                            let g = k as f32 / RAMP as f32;
                            stereo[2 * k] *= g;
                            stereo[2 * k + 1] *= g;
                        }
                        lost = false;
                    }
                } else {
                    // dry: what there was, then it falls softly from where it was to silence, never at once
                    let from = if got > 0 { [stereo[2 * got - 2], stereo[2 * got - 1]] } else { last };
                    for k in got..frames {
                        let g = if k - got < RAMP { 1.0 - (k - got) as f32 / RAMP as f32 } else { 0.0 };
                        stereo[2 * k] = from[0] * g;
                        stereo[2 * k + 1] = from[1] * g;
                    }
                    lost = true;
                    if let Ok(mut m) = dry.try_lock() {
                        m.dry += 1;
                    }
                }
                if frames > 0 {
                    last = [stereo[2 * frames - 2], stereo[2 * frames - 1]];
                }
                for (k, frame) in data.chunks_exact_mut(channels.max(1)).enumerate() {
                    for (c, x) in frame.iter_mut().enumerate() {
                        *x = if c < 2 { stereo[2 * k + c] } else { 0.0 };
                    }
                }
            },
            |e| eprintln!("sound: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Output { _stream: stream, rate, device: name, running })
}

/// The sound's own thread: it keeps the reserve full, a block at a time, and measures how long a block takes
/// against how long it lasts (the load: above 100 %, the sound cannot keep up).
fn compute_ahead(
    mixer: &Arc<Mutex<Mixer>>,
    reserve: &Arc<Mutex<VecDeque<f32>>>,
    running: &AtomicBool,
    rate: u32,
    ahead: usize,
) {
    let mut block = vec![0.0f32; 2 * BLOCK];
    let lasts = BLOCK as f64 / rate.max(1) as f64;
    while running.load(Ordering::Relaxed) {
        let held = reserve.lock().map(|r| r.len() / 2).unwrap_or(0);
        if held >= ahead {
            std::thread::sleep(std::time::Duration::from_micros(500));
            continue;
        }
        let t0 = std::time::Instant::now();
        {
            let Ok(mut m) = mixer.lock() else { return };
            m.render(&mut block);
            // how hard it works: what a block took over what it lasts, smoothed
            let load = t0.elapsed().as_secs_f64() / lasts;
            m.load += 0.02 * (load as f32 - m.load);
        }
        if let Ok(mut r) = reserve.lock() {
            r.extend(block.iter().copied());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tone(f64);

    impl crate::mixer::Source for Tone {
        fn next(&mut self) -> f64 {
            self.0 += 0.01;
            self.0.sin() * 0.3
        }
    }

    #[test]
    fn the_sound_is_computed_ahead_and_its_load_measured() {
        let mixer = Arc::new(Mutex::new(Mixer::new(48_000)));
        mixer.lock().unwrap().play("tone", Box::new(Tone(0.0)), 1.0);
        let reserve = Arc::new(Mutex::new(VecDeque::new()));
        let running = Arc::new(AtomicBool::new(true));
        let ahead = 768;
        let worker = {
            let (m, r, run) = (mixer.clone(), reserve.clone(), running.clone());
            std::thread::spawn(move || compute_ahead(&m, &r, &run, 48_000, ahead))
        };
        // it fills its reserve to what it keeps ahead, and no further
        let t0 = std::time::Instant::now();
        while reserve.lock().unwrap().len() / 2 < ahead && t0.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::yield_now();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        let held = reserve.lock().unwrap().len() / 2;
        assert!(held >= ahead && held < ahead + BLOCK + 1, "{held}");
        // taken from, it fills again; the window holding the mixer a moment only delays it
        reserve.lock().unwrap().drain(..2 * 512);
        {
            let _held = mixer.lock().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let t0 = std::time::Instant::now();
        while reserve.lock().unwrap().len() / 2 < ahead && t0.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::yield_now();
        }
        assert!(reserve.lock().unwrap().len() / 2 >= ahead);
        let load = mixer.lock().unwrap().load;
        assert!(load > 0.0 && load < 1.0, "{load}");
        assert!(reserve.lock().unwrap().iter().any(|x| x.abs() > 0.01), "it sounds");
        running.store(false, Ordering::Relaxed);
        worker.join().unwrap();
    }
}
