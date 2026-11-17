// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The computer's sound device, playing the mixer. The device's thread barely waits: if the mixer stays busy (the
//! window is changing a gain), that block fades to silence and the next fades back in, rather than a stall or a
//! click.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::mixer::Mixer;

/// While it lives, the device plays.
pub struct Output {
    _stream: cpal::Stream,
    pub rate: u32,
    pub device: String,
}

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
    let mut stereo: Vec<f32> = vec![];
    // the last sample played, and whether the block before was lost (to come back without a click)
    let mut last = [0.0f32; 2];
    let mut lost = false;
    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let frames = data.len() / channels.max(1);
                stereo.resize(2 * frames, 0.0);
                // the mixer may be held a moment by the window (a scope read, a gain changed): it is waited for a
                // little (never more than half a millisecond: the device must not wait)
                let t0 = std::time::Instant::now();
                let mut got = mixer.try_lock();
                while got.is_err() && t0.elapsed() < std::time::Duration::from_micros(500) {
                    std::hint::spin_loop();
                    got = mixer.try_lock();
                }
                const RAMP: usize = 96;
                match got {
                    Ok(mut m) => {
                        m.render(&mut stereo);
                        drop(m);
                        if lost {
                            // back after a lost block: it rises again instead of jumping (a jump is a click)
                            for k in 0..frames.min(RAMP) {
                                let g = k as f32 / RAMP as f32;
                                stereo[2 * k] *= g;
                                stereo[2 * k + 1] *= g;
                            }
                            lost = false;
                        }
                    }
                    Err(_) => {
                        // lost: it falls softly from where it was to silence, never at once
                        for k in 0..frames {
                            let g = if k < RAMP { 1.0 - k as f32 / RAMP as f32 } else { 0.0 };
                            stereo[2 * k] = last[0] * g;
                            stereo[2 * k + 1] = last[1] * g;
                        }
                        lost = true;
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
    Ok(Output { _stream: stream, rate, device: name })
}
