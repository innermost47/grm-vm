// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The prompt runs in a thread of its own: a command that takes time (an eye seeing a big picture, a file being
//! fed, an app said again) never freezes the window. Between commands, a few times a second, what the machine
//! feels moves its voice, and what the APIs read goes into the machine.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use grm_engine::engine::Engine;
use grm_engine::vision::Sight;
use grm_shell::runtime::AppView;
use grm_shell::{Line, Shell};

pub enum Request {
    /// A line, as typed.
    Line(String),
    /// A new text for a running app (its code edited, a widget moved, a relation drawn).
    Edit(String, String),
}

#[derive(Default)]
pub struct Answer {
    pub typed: String,
    pub lines: Vec<Line>,
    pub quit: bool,
    pub asking: bool,
    pub sight: Option<(String, Sight)>,
    /// The running apps, when they changed.
    pub apps: Option<Vec<AppView>>,
    /// The app files that can be opened, when the apps changed.
    pub files: Option<Vec<std::path::PathBuf>>,
    /// Whether this answers a request (or came by itself).
    pub answered: bool,
    /// What the prompt asks of the window.
    pub orders: Vec<grm_shell::Order>,
    /// Whether the world was kept before this line changed it (it can be taken back).
    pub kept: bool,
}

pub struct Worker {
    tx: Option<Sender<Request>>,
    rx: Receiver<Answer>,
    thread: Option<JoinHandle<()>>,
    /// Requests sent, not yet answered.
    pub waiting: usize,
}

fn news(shell: &mut Shell, mut a: Answer) -> Answer {
    a.lines.extend(shell.take_pending());
    a.orders = shell.take_orders();
    if shell.apps_changed() {
        a.apps = Some(shell.app_views());
        a.files = Some(shell.app_files());
    }
    a
}

impl Worker {
    pub fn start(engine: Arc<Engine>, mut shell: Shell) -> Worker {
        let (tx, requests) = channel::<Request>();
        let (answers, rx) = channel::<Answer>();
        let thread = std::thread::Builder::new()
            .name("grm-prompt".into())
            .spawn(move || {
                // the desk as it was left
                let lines = shell.restore_desk(&engine);
                let mut first = news(&mut shell, Answer { lines, ..Default::default() });
                first.apps = Some(shell.app_views());
                first.files = Some(shell.app_files());
                if answers.send(first).is_err() {
                    return;
                }
                loop {
                    match requests.recv_timeout(Duration::from_millis(100)) {
                        Ok(Request::Line(typed)) => {
                            let seen = shell.sights.len();
                            let last = shell.sights.last().map(|(k, s)| (k.clone(), s.rounds, s.elapsed));
                            let before = shell.snapshots;
                            let (lines, quit) = shell.run(&engine, &typed);
                            let kept = shell.snapshots != before;
                            let now = shell.sights.last().map(|(k, s)| (k.clone(), s.rounds, s.elapsed));
                            let sight = if shell.sights.len() != seen || now != last {
                                shell.sights.last().map(|(k, s)| (k.clone(), s.clone()))
                            } else {
                                None
                            };
                            let asking = shell.asking();
                            let a = Answer {
                                typed,
                                lines,
                                quit,
                                asking,
                                sight,
                                kept,
                                answered: true,
                                ..Default::default()
                            };
                            if answers.send(news(&mut shell, a)).is_err() {
                                return;
                            }
                        }
                        Ok(Request::Edit(app, text)) => {
                            let lines = shell.edit_app(&engine, &app, text);
                            let a = Answer { lines, answered: true, asking: shell.asking(), ..Default::default() };
                            if answers.send(news(&mut shell, a)).is_err() {
                                return;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            shell.feel(&engine);
                            let asking = shell.asking();
                            let a = news(&mut shell, Answer { asking, ..Default::default() });
                            if (!a.lines.is_empty() || a.apps.is_some() || !a.orders.is_empty())
                                && answers.send(a).is_err()
                            {
                                return;
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            shell.save_desk();
                            return;
                        }
                    }
                }
            })
            .expect("the prompt's thread");
        Worker { tx: Some(tx), rx, thread: Some(thread), waiting: 0 }
    }

    pub fn send(&mut self, line: impl Into<String>) {
        self.request(Request::Line(line.into()));
    }

    pub fn request(&mut self, r: Request) {
        if let Some(tx) = &self.tx {
            if tx.send(r).is_ok() {
                self.waiting += 1;
            }
        }
    }

    pub fn answers(&mut self) -> Vec<Answer> {
        let got: Vec<Answer> = self.rx.try_iter().collect();
        let answered = got.iter().filter(|a| a.answered).count();
        self.waiting = self.waiting.saturating_sub(answered);
        got
    }

    /// Stops the thread (after what it is doing): it keeps the desk as it is.
    pub fn stop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
