// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What comes into the network and what goes out of it: typed ports.
//!
//! Everything that reaches the machine is a signal on a port: a value, a vector, an event (a click, a key), an
//! image, a sound. A port is bound to the nodes it feeds or reads. The machine only ever sees measurements of its
//! nodes; the port is what turns a signal into them.
//!
//! In this milestone values, vectors and events are carried; images and sounds have their type and their place,
//! and their adapters (a pyramid of patches, as the retina and the visual cortex; bands of frequencies, as the
//! cochlea) come with the senses.

use std::time::Instant;

use crate::bail;
use crate::error::Result;
use crate::world::World;

/// A signal arriving on a port, or leaving by one.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Value(f64),
    Vector(Vec<f64>),
    /// Something that happens: a click, a key, a touch; `value` 1 while it is on, 0 when it is off.
    Event {
        name: String,
        value: f64,
    },
    /// An image: width x height x channels, row by row, each value in 0 .. 1.
    Image {
        width: usize,
        height: usize,
        channels: usize,
        data: Vec<f32>,
    },
    /// A sound: samples in -1 .. 1, at `rate` a second.
    Audio {
        rate: u32,
        samples: Vec<f32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortKind {
    Value,
    Vector,
    Event,
    Image,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

/// A port: a name, a kind, a direction, and the nodes it is bound to.
#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    pub name: String,
    pub kind: PortKind,
    pub direction: Direction,
    /// The node of a value; the nodes of a vector's components (NAME_0, NAME_1...); the node of an event.
    pub nodes: Vec<String>,
    /// How much a value is trusted: its sigma (None: as the senses do, a thousandth of itself).
    pub sigma: Option<f64>,
}

impl Port {
    pub fn value(name: &str, direction: Direction) -> Port {
        Port { name: name.into(), kind: PortKind::Value, direction, nodes: vec![name.into()], sigma: None }
    }

    pub fn vector(name: &str, dim: usize, direction: Direction) -> Port {
        Port {
            name: name.into(),
            kind: PortKind::Vector,
            direction,
            nodes: (0..dim).map(|k| format!("{name}_{k}")).collect(),
            sigma: None,
        }
    }

    pub fn event(name: &str) -> Port {
        Port {
            name: name.into(),
            kind: PortKind::Event,
            direction: Direction::In,
            nodes: vec![format!("event_{name}")],
            sigma: Some(1e-6),
        }
    }

    pub fn image(name: &str, direction: Direction) -> Port {
        Port { name: name.into(), kind: PortKind::Image, direction, nodes: vec![], sigma: None }
    }

    pub fn audio(name: &str, direction: Direction) -> Port {
        Port { name: name.into(), kind: PortKind::Audio, direction, nodes: vec![], sigma: None }
    }

    /// The statements that make its nodes (an input or an output each).
    pub fn statements(&self) -> Vec<String> {
        let word = if self.direction == Direction::In { "input" } else { "output" };
        self.nodes.iter().map(|n| format!("{word} {n}")).collect()
    }
}

/// When a signal came in (for the senses and the logbook: everything is real time).
#[derive(Debug, Clone)]
pub struct Arrival {
    pub port: String,
    pub at: Instant,
}

impl World {
    /// Makes the nodes of a port.
    pub fn open_port(&mut self, port: &Port) -> Result<()> {
        let lines = port.statements();
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        self.say_all(&refs, &crate::network::Provenance::of(format!("port:{}", port.name)))?;
        Ok(())
    }

    /// A signal on an input port: it becomes measurements of its nodes (only they wake).
    pub fn feed(&mut self, port: &Port, signal: &Signal) -> Result<Arrival> {
        if port.direction != Direction::In {
            bail!("{} is an output: it is read, not fed", port.name);
        }
        if let (PortKind::Image, Signal::Image { width, height, channels, data }) = (port.kind, signal) {
            let picture =
                crate::vision::Picture { width: *width, height: *height, channels: *channels, data: data.clone() };
            self.see(&port.name, &picture, std::time::Duration::from_secs(5))?;
            return Ok(Arrival { port: port.name.clone(), at: Instant::now() });
        }
        let m = &mut self.machine;
        match (port.kind, signal) {
            (PortKind::Value, Signal::Value(v)) => m.observe(&port.nodes[0], *v, port.sigma)?,
            (PortKind::Vector, Signal::Vector(v)) => {
                if v.len() != port.nodes.len() {
                    bail!("{} holds {} values, not {}", port.name, port.nodes.len(), v.len());
                }
                for (n, x) in port.nodes.iter().zip(v) {
                    m.observe(n, *x, port.sigma)?;
                }
            }
            (PortKind::Event, Signal::Event { value, .. }) => m.observe(&port.nodes[0], *value, port.sigma)?,
            (PortKind::Audio, Signal::Audio { .. }) => {
                bail!("{}: sounds come in with the ear (a later milestone); its port is ready", port.name)
            }
            _ => bail!("{} does not take that kind of signal", port.name),
        }
        Ok(Arrival { port: port.name.clone(), at: Instant::now() })
    }

    /// What an output port says now.
    pub fn read(&self, port: &Port) -> Result<Signal> {
        let m = &self.machine;
        for n in &port.nodes {
            if !m.index.contains_key(n) && !m.fixed.contains_key(n) {
                bail!("{n} is not a node of the network");
            }
        }
        Ok(match port.kind {
            PortKind::Value => Signal::Value(m.value_si(&port.nodes[0])),
            PortKind::Vector => Signal::Vector(port.nodes.iter().map(|n| m.value_si(n)).collect()),
            PortKind::Event => Signal::Event { name: port.name.clone(), value: m.value_si(&port.nodes[0]) },
            PortKind::Image | PortKind::Audio => {
                bail!("{}: images and sounds go out with the senses (a later milestone)", port.name)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::Provenance;

    #[test]
    fn a_click_and_a_vector_come_in_and_a_value_goes_out() {
        let mut w = World::new();
        let click = Port::event("button");
        let pos = Port::vector("mouse", 2, Direction::In);
        let out = Port::value("light", Direction::Out);
        for p in [&click, &pos, &out] {
            w.open_port(p).unwrap();
        }
        w.say("relate glow: light = event_button * 10 + mouse_0", &Provenance::default()).unwrap();
        w.feed(&click, &Signal::Event { name: "button".into(), value: 1.0 }).unwrap();
        w.feed(&pos, &Signal::Vector(vec![0.5, 0.25])).unwrap();
        w.machine.run(1000);
        let Signal::Value(v) = w.read(&out).unwrap() else { panic!() };
        assert!((v - 10.5).abs() < 1e-6, "{v}");
        let image = Port::image("eye", Direction::In);
        w.feed(&image, &Signal::Image { width: 4, height: 4, channels: 1, data: vec![0.5; 16] }).unwrap();
        assert!(w.net().nodes.contains_key("eye_glance_0_0"));
        let ear = Port::audio("ear", Direction::In);
        assert!(w.feed(&ear, &Signal::Audio { rate: 48000, samples: vec![0.0] }).is_err());
    }
}
