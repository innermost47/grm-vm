// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine in a thread of its own: whoever talks to it (a prompt, a window, a peer) sends it orders and reads
//! snapshots of it; nobody ever waits for it to compute.
//!
//! The engine works in frames: it takes the orders that have come, then computes for at most a frame's budget
//! (whatever it has reached is usable, even before it has settled), then publishes a snapshot. When nothing is new,
//! it sleeps until an order comes.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::error::{GrmError, Result};
use crate::network::Provenance;
use crate::world::World;

/// What a snapshot holds of the machine at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// name -> (value in SI, sigma: SI for a signed node, dex for a positive one).
    pub values: IndexMap<String, (f64, f64)>,
    pub harmony: f64,
    pub rounds: u64,
    pub messages: u64,
    pub ticks: u64,
    pub settled: bool,
    /// How many snapshots came before this one.
    pub frame: u64,
    /// How long the last frame computed.
    pub compute: Duration,
    pub quarantine: Vec<String>,
}

impl Snapshot {
    fn of(world: &World, frame: u64, compute: Duration) -> Snapshot {
        let m = &world.machine;
        let values = m.net.nodes.keys().filter(|n| !n.contains("__next")).map(|n| (n.clone(), m.reading(n))).collect();
        Snapshot {
            values,
            harmony: m.harmony(),
            rounds: m.rounds,
            messages: m.messages,
            ticks: m.net.ticks,
            settled: m.quiet(),
            frame,
            compute,
            quarantine: world.quarantine.clone(),
        }
    }
}

type Job = Box<dyn FnOnce(&mut World) -> Result<String> + Send>;

enum Order {
    Do(Job, Option<Sender<Result<String>>>),
    Stop,
}

/// The handle of a machine running in its own thread.
pub struct Engine {
    orders: Sender<Order>,
    snapshot: Arc<RwLock<Arc<Snapshot>>>,
    thread: Option<JoinHandle<World>>,
}

impl Engine {
    /// Starts a machine in its own thread, computing at most `frame` at a time between two snapshots.
    pub fn start(world: World, frame: Duration) -> Engine {
        let (tx, rx) = channel::<Order>();
        let snapshot = Arc::new(RwLock::new(Arc::new(Snapshot::of(&world, 0, Duration::ZERO))));
        let shared = snapshot.clone();
        let thread = std::thread::Builder::new()
            .name("grm-engine".into())
            .spawn(move || run(world, rx, shared, frame))
            .expect("a thread for the engine");
        Engine { orders: tx, snapshot, thread: Some(thread) }
    }

    /// The last snapshot (never waits for the machine).
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// Sends an order and goes on: the machine does it at its next frame.
    pub fn send(&self, job: impl FnOnce(&mut World) -> Result<String> + Send + 'static) {
        let _ = self.orders.send(Order::Do(Box::new(job), None));
    }

    /// Sends an order and waits for what the machine answers (it answers at its next frame, not after settling).
    pub fn ask(&self, job: impl FnOnce(&mut World) -> Result<String> + Send + 'static) -> Result<String> {
        let (tx, rx) = channel();
        self.orders.send(Order::Do(Box::new(job), Some(tx))).map_err(|_| GrmError::new("the engine has stopped"))?;
        rx.recv().map_err(|_| GrmError::new("the engine has stopped"))?
    }

    /// Asks the world something and waits for the answer (at the machine's next frame). None: the engine stopped.
    pub fn query<T: Send + 'static>(&self, job: impl FnOnce(&mut World) -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = channel();
        self.send(move |w| {
            let _ = tx.send(job(w));
            Ok(String::new())
        });
        rx.recv().ok()
    }

    /// Says a statement to the global network.
    pub fn say(&self, line: &str, origin: Provenance) -> Result<String> {
        let line = line.to_string();
        self.ask(move |w| Ok(w.say(&line, &origin)?.unwrap_or_default()))
    }

    /// Waits until the machine has settled after what was sent before (or `timeout`), and returns its snapshot.
    pub fn settled(&self, timeout: Duration) -> Arc<Snapshot> {
        let t0 = Instant::now();
        let since = self.snapshot().frame;
        loop {
            let s = self.snapshot();
            if (s.settled && s.frame > since) || t0.elapsed() > timeout {
                return s;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Stops the machine and gives its world back.
    pub fn stop(mut self) -> World {
        let _ = self.orders.send(Order::Stop);
        self.thread.take().expect("a running engine").join().expect("the engine thread")
    }
}

fn run(mut world: World, orders: Receiver<Order>, snapshot: Arc<RwLock<Arc<Snapshot>>>, frame: Duration) -> World {
    let mut frames = 0u64;
    loop {
        // the orders that have come (when the machine is quiet, it sleeps until one comes)
        let first = if world.machine.quiet() {
            match orders.recv_timeout(Duration::from_millis(250)) {
                Ok(o) => Some(o),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return world,
            }
        } else {
            orders.try_recv().ok()
        };
        let mut pending: Vec<Order> = first.into_iter().collect();
        pending.extend(orders.try_iter());
        for order in pending {
            match order {
                Order::Stop => return world,
                Order::Do(job, reply) => {
                    let answer = job(&mut world);
                    if let Some(r) = reply {
                        let _ = r.send(answer);
                    }
                }
            }
        }
        // one frame of work: whatever it reaches is shown, settled or not
        let t0 = Instant::now();
        if !world.machine.quiet() {
            world.machine.run_for(frame);
        }
        let compute = t0.elapsed();
        frames += 1;
        let s = Arc::new(Snapshot::of(&world, frames, compute));
        if let Ok(mut w) = snapshot.write() {
            *w = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_answers_while_it_computes() {
        let engine = Engine::start(World::new(), Duration::from_millis(8));
        engine.say("node x signed", Provenance::default()).unwrap();
        engine.say("relate r: y = 3 * x", Provenance::default()).unwrap();
        engine.say("observe x = 2", Provenance::default()).unwrap();
        let s = engine.settled(Duration::from_secs(5));
        assert!(s.settled);
        assert!((s.values["y"].0 - 6.0).abs() < 1e-6);
        let answer = engine.ask(|w| Ok(format!("{}", w.machine.value("y"))));
        assert!((answer.unwrap().parse::<f64>().unwrap() - 6.0).abs() < 1e-6);
        let world = engine.stop();
        assert!(world.net().nodes.contains_key("y"));
    }

    #[test]
    fn a_frame_never_computes_much_longer_than_its_budget() {
        // a long chain that takes many rounds: each frame stops at its budget, and shows where it has got to
        let mut w = World::new();
        let lines: Vec<String> = (0..400).map(|k| format!("relate c{k}: a{} = a{k} + 1", k + 1)).collect();
        let mut refs: Vec<&str> = vec!["node a0 signed", "observe a0 = 0"];
        refs.extend(lines.iter().map(|s| s.as_str()));
        w.say_all(&refs, &Provenance::default()).unwrap();
        let engine = Engine::start(w, Duration::from_millis(1));
        engine.send(|_| Ok(String::new()));
        std::thread::sleep(Duration::from_millis(30));
        let s = engine.snapshot();
        assert!(s.frame > 1);
        assert!(s.compute < Duration::from_millis(50), "{:?}", s.compute);
        let s = engine.settled(Duration::from_secs(30));
        assert!((s.values["a400"].0 - 400.0).abs() < 1e-3);
        engine.stop();
    }
}
