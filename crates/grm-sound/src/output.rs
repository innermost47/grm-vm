// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The computer's sound device, playing the mixer. The device's thread never waits: if the mixer is busy (the
//! window is changing a gain), that block is silence rather than a stall.

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
    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                let frames = data.len() / channels.max(1);
                stereo.resize(2 * frames, 0.0);
                match mixer.try_lock() {
                    Ok(mut m) => m.render(&mut stereo),
                    Err(_) => stereo.iter_mut().for_each(|x| *x = 0.0),
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
