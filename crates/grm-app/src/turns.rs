// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Her hands on what the human gives her to turn (`hand turn_NAME when SWITCH: NODE ...` in an app): at most two
//! things at once, each slowly turned (a knob, a slider) or flipped (a switch), listened to, kept when it pleases
//! her and put back when it does not. Where she goes is never drawn by chance: each thing goes round its range by the
//! golden ratio, so she tries everywhere, evenly. She turns only while the sound plays.
//!
//! Written in Rust for now, as the window writes what a hand plays (the language does not yet let a network turn
//! an `input`): to move into GRML when it can.

use std::collections::HashMap;

use grm_shell::apps::WidgetKind;

use crate::app::App;

/// At most this many things in her hands at once (she has two hands, not fifty).
pub const AT_ONCE: usize = 2;
/// Seconds to turn one, then to listen to it.
const GLIDE: f64 = 1.5;
const LISTEN: f64 = 3.0;

#[derive(Debug, Clone)]
struct Turn {
    node: String,
    from: f64,
    to: f64,
    began: f64,
    pleasure: f64,
    back: bool,
    written: f64,
}

/// What her hands hold, and where each thing she may turn is in its round.
#[derive(Debug, Default)]
pub struct Turns {
    moving: Vec<Turn>,
    next_at: f64,
    k: usize,
    rounds: HashMap<String, f64>,
}

/// What she may turn now: the nodes of each `hand turn_...` whose switch is on, shown by a knob, a slider or a
/// switch (its range read there).
pub fn allowed(app: &App) -> Vec<(String, f64, f64, bool)> {
    let mut out: Vec<(String, f64, f64, bool)> = vec![];
    for a in &app.apps {
        let widgets = grm_shell::apps::all_widgets(&a.widgets);
        for h in a.hands.iter().filter(|h| h.name.starts_with("turn_")) {
            if !app.picture.node(&h.switch).is_some_and(|n| n.value > 0.5) {
                continue;
            }
            for w in &widgets {
                let Some(node) = w.nodes.first().filter(|n| h.nodes.contains(n)) else { continue };
                if out.iter().any(|o| &o.0 == node) {
                    continue;
                }
                match w.kind {
                    WidgetKind::Knob | WidgetKind::Slider => out.push((node.clone(), w.lo, w.hi, false)),
                    WidgetKind::Switch | WidgetKind::Toggle => out.push((node.clone(), 0.0, 1.0, true)),
                    _ => {}
                }
            }
        }
    }
    out
}

impl Turns {
    /// What her hands hold now.
    #[cfg(test)]
    pub fn moving(&self) -> Vec<String> {
        self.moving.iter().map(|t| t.node.clone()).collect()
    }
}

impl App {
    /// Her hands, once a frame.
    pub(crate) fn turns(&mut self) {
        let now = self.ear_now();
        let may = allowed(self);
        let value = |app: &App, n: &str| app.picture.node(n).map(|x| x.value);
        let pleasure = value(self, "elle_pleasure").unwrap_or(0.0);
        let bore = value(self, "elle_bore").unwrap_or(0.2).clamp(0.0, 1.0);
        // she turns only while it is heard
        let heard = value(self, "playing").is_none_or(|v| v > 0.5);
        let mut turns = std::mem::take(&mut self.turns);
        // what she no longer may: let go where it is
        turns.moving.retain(|t| may.iter().any(|m| m.0 == t.node));
        let mut done = vec![];
        for (i, t) in turns.moving.iter_mut().enumerate() {
            let part = ((now - t.began) / GLIDE).clamp(0.0, 1.0);
            let v = t.from + (t.to - t.from) * part;
            // written ten times a second while it turns, and once where it ends
            if (now - t.written >= 0.1 || part >= 1.0) && part <= 1.0 && t.written < t.began + GLIDE {
                self.by_ear(&t.node, v);
                t.written = now;
            }
            if now - t.began >= GLIDE + LISTEN {
                // listened to: a turn that pleased less is put back (once), the rest is kept
                if !t.back && pleasure < t.pleasure - 0.05 {
                    *t = Turn {
                        node: t.node.clone(),
                        from: t.to,
                        to: t.from,
                        began: now,
                        pleasure,
                        back: true,
                        written: now - 1.0,
                    };
                } else {
                    done.push(i);
                }
            }
        }
        for i in done.into_iter().rev() {
            turns.moving.remove(i);
        }
        // a new one: her hands not full, it is time (sooner when she is bored), something she may turn
        if heard && turns.moving.len() < AT_ONCE && now >= turns.next_at && !may.is_empty() {
            for _ in 0..may.len() {
                let (node, lo, hi, switch) = may[turns.k % may.len()].clone();
                turns.k += 1;
                if turns.moving.iter().any(|t| t.node == node) {
                    continue;
                }
                let Some(here) = value(self, &node) else { continue };
                let to = if switch {
                    if here > 0.5 {
                        0.0
                    } else {
                        1.0
                    }
                } else {
                    let round = turns.rounds.entry(node.clone()).or_insert(((here - lo) / (hi - lo)).clamp(0.0, 1.0));
                    *round = (*round + 0.618_033_988_75).fract();
                    let aim = lo + (hi - lo) * *round;
                    // how far toward it: farther as she is bored
                    here + (aim - here) * (0.35 + 0.5 * bore)
                };
                turns.moving.push(Turn { node, from: here, to, began: now, pleasure, back: false, written: now - 1.0 });
                turns.next_at = now + 6.0 - 4.0 * bore;
                break;
            }
        }
        self.turns = turns;
    }
}

#[cfg(test)]
mod tests {
    /// Her hands go round a range by the golden ratio: everywhere, evenly, never by chance.
    #[test]
    fn her_hands_go_everywhere_evenly() {
        let mut r: f64 = 0.0;
        let mut seen = [false; 10];
        for _ in 0..20 {
            r = (r + 0.618_033_988_75).fract();
            seen[(r * 10.0) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "{seen:?}");
    }
}
