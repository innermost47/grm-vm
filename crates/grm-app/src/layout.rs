// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Where the panels go, decided by a small GRM network of its own (not the human's): the size of the window comes
//! in as two inputs, the panels are nodes, the wishes are soft relations, the limits are `keep`s, and a narrow
//! window switches regime (`when W in ...`): the side panel goes under the network.
//!
//! It knows nothing of how it is drawn: it gives sizes, the window draws them.

use grm_engine::world::World;
use grm_engine::Provenance;

const BASE: &[&str] = &[
    "node W = 1280",
    "node H = 800",
    "node side = 380",
    "node bottom = 220",
    "node band = 330",
    // the soft wishes: a share of the window
    "relate side_wide: side = 0.3 * W ± 0.03 when W in 860 .. 100000",
    "relate bottom_soft: bottom = 0.28 * H ± 0.03",
    "relate band_soft: band = 0.42 * H ± 0.03",
    // a narrow window: the side panel takes the whole width, under the network
    "relate side_narrow: side = W ± 0.001 when W in 0 .. 860",
    // the limits
    "keep side_min: side >= 300",
    "keep side_max: side <= 520 when W in 860 .. 100000",
    "keep bottom_min: bottom >= 150",
    "keep bottom_max: bottom <= 320",
    "keep band_min: band >= 200",
    "keep band_max: band <= 440",
];

/// The sizes, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    pub side: f32,
    pub bottom: f32,
    /// The side panel under the network (a narrow window), `band` high.
    pub stacked: bool,
    pub band: f32,
    /// How well the wishes hold together (1: all of them).
    pub harmony: f64,
}

pub struct Layout {
    world: World,
    seen: (i32, i32),
    wished: bool,
    pub sizes: Sizes,
    /// The human's wishes (dragged edges), to keep them for next time.
    pub side_wish: Option<f32>,
    pub bottom_wish: Option<f32>,
}

impl Layout {
    pub fn new() -> Layout {
        let mut world = World::new();
        world.machine.set_threads(Some(1));
        let origin = Provenance::of("layout");
        for line in BASE {
            world.say(line, &origin).expect("the layout network");
        }
        let mut l = Layout {
            world,
            seen: (0, 0),
            wished: false,
            side_wish: None,
            bottom_wish: None,
            sizes: Sizes { side: 380.0, bottom: 220.0, stacked: false, band: 330.0, harmony: 1.0 },
        };
        l.window(1280.0, 800.0);
        l
    }

    /// The window has this size: the panels follow (only when it changed).
    pub fn window(&mut self, w: f32, h: f32) -> Sizes {
        let key = (w.round() as i32, h.round() as i32);
        if key != self.seen {
            self.seen = key;
            let origin = Provenance::of("window");
            let _ = self.world.say(&format!("observe W = {} ± 0.001", key.0.max(1)), &origin);
            let _ = self.world.say(&format!("observe H = {} ± 0.001", key.1.max(1)), &origin);
            self.settle();
        }
        self.sizes
    }

    /// The human dragged the side panel's edge: a wish, stronger than the share, still within the limits.
    pub fn wish_side(&mut self, side: f32) {
        self.side_wish = Some(side);
        let origin = Provenance::of("human");
        if !self.wished {
            let _ = self.world.say("node side_wish = 380", &origin);
            let _ = self.world.say("relate wish_side: side = side_wish ± 0.003 when W in 860 .. 100000", &origin);
            self.wished = true;
        }
        let _ = self.world.say(&format!("observe side_wish = {} ± 0.001", side.max(1.0)), &origin);
        self.settle();
    }

    /// The human dragged the prompt's edge.
    pub fn wish_bottom(&mut self, bottom: f32) {
        self.bottom_wish = Some(bottom);
        let origin = Provenance::of("human");
        if self.world.net().nodes.get("bottom_wish").is_none() {
            let _ = self.world.say("node bottom_wish = 220", &origin);
            let _ = self.world.say("relate wish_bottom: bottom = bottom_wish ± 0.003", &origin);
        }
        let _ = self.world.say(&format!("observe bottom_wish = {} ± 0.001", bottom.max(1.0)), &origin);
        self.settle();
    }

    /// Forgets the wishes: the shares again.
    pub fn reset(&mut self) {
        let seen = self.seen;
        *self = Layout::new();
        self.seen = (0, 0);
        self.window(seen.0 as f32, seen.1 as f32);
    }

    fn settle(&mut self) {
        self.world.machine.run(4000);
        let m = &self.world.machine;
        let w = m.value_si("W") as f32;
        let side = m.value_si("side") as f32;
        self.sizes = Sizes {
            side,
            bottom: m.value_si("bottom") as f32,
            stacked: side >= w - 1.0,
            band: m.value_si("band") as f32,
            harmony: m.harmony(),
        };
    }

    /// The statements of the layout network, as said (to show the human what decides).
    pub fn statements(&self) -> Vec<String> {
        self.world.net().relations.iter().map(|r| format!("{}: {}", r.name, r.text)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panels_follow_the_window_within_their_limits() {
        let mut l = Layout::new();
        let s = l.window(1280.0, 800.0);
        assert!((s.side - 384.0).abs() < 2.0 && !s.stacked, "{s:?}");
        assert!((s.bottom - 224.0).abs() < 2.0, "{s:?}");
        let s = l.window(2400.0, 1400.0);
        assert!((s.side - 520.0).abs() < 2.0 && (s.bottom - 320.0).abs() < 2.0, "{s:?}");
        let s = l.window(700.0, 900.0);
        assert!(s.stacked && (s.band - 378.0).abs() < 3.0, "{s:?}");
        let s = l.window(1280.0, 800.0);
        assert!(!s.stacked);
        l.wish_side(450.0);
        assert!((l.sizes.side - 450.0).abs() < 3.0, "{:?}", l.sizes);
        l.wish_side(900.0);
        assert!((l.sizes.side - 520.0).abs() < 3.0, "{:?}", l.sizes);
        l.reset();
        assert!((l.sizes.side - 384.0).abs() < 2.0, "{:?}", l.sizes);
    }
}
