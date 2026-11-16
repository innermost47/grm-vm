// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Looks: style as a small GRM network of its own, not as classes.
//!
//! Each property of a look (its accent, its background, its text, as hue, saturation and lightness; its
//! roundness; its size) is a node. The machine's palette is measured into `vm_*`. An app's look (`app_*`) is held
//! to the machine's by soft relations, and a named look (`big_*`) to the app's: what is not said is inherited, the
//! way a cascade does it, by how sure each relation is (said: ± 0.001; inherited: ± 1). A look said with a
//! condition holds only while it holds (`look calm: accent = #6fa8dc when hour in 19 .. 24`), and `style` lines are
//! any statement of the language into this network (`style relate breathe: big_size = 1 + 0.1 * warmth`).
//! The hour comes in as an input: with "follow the hour", the evening warms the accent and dims the background.
//!
//! ```text
//! look app: accent = #7fb8f0, round = 12
//! look big: size = 1.4
//! widget go: play out "sound" look big
//! ```

use std::collections::HashMap;

use eframe::egui::Color32;
use grm_engine::world::World;
use grm_engine::Provenance;

use crate::style::Palette;

/// What a widget is drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub accent: Color32,
    pub background: Color32,
    pub text: Color32,
    pub round: f32,
    pub size: f32,
}

const COLOURS: [&str; 3] = ["accent", "bg", "text"];
const NUMBERS: [&str; 2] = ["round", "size"];

fn hsl(rgb: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (rgb[0] as f64 / 255.0, rgb[1] as f64 / 255.0, rgb[2] as f64 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    (h, s, l)
}

fn rgb(h: f64, s: f64, l: f64) -> Color32 {
    let h = h.rem_euclid(360.0) / 360.0;
    let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    Color32::from_rgb(f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

fn hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub struct Looks {
    world: World,
    pub key: String,
    names: Vec<String>,
    pub resolved: HashMap<String, Look>,
    /// Lines it could not say: (line of the app, why).
    pub errors: Vec<(usize, String)>,
    hour_seen: i64,
}

impl Looks {
    pub fn build(palette: &Palette, lines: &[(usize, String)], follow_hour: bool) -> Looks {
        let key = format!("{palette:?}{lines:?}{follow_hour}");
        let mut world = World::new();
        world.machine.set_threads(Some(1));
        let origin = Provenance::of("look");
        let say = |w: &mut World, l: &str| w.say(l, &origin).map(|_| ()).map_err(|e| e.message);
        let mut names = vec!["app".to_string()];
        let mut errors = vec![];
        // the machine's palette, measured
        let base = [("accent", palette.accent), ("bg", palette.background), ("text", palette.text)];
        let mut lines_vm = vec!["input hour = 12".to_string(), "node warmth signed".to_string()];
        for (p, c) in base {
            let (h, s, l) = hsl(c);
            for (part, v) in [("h", h), ("s", s), ("l", l)] {
                lines_vm.push(format!("input vm_{p}_{part} = {v}"));
            }
        }
        lines_vm.push("input vm_round = 6".into());
        lines_vm.push("input vm_size = 1".into());
        // the hour: the evening warms, the night dims (when followed)
        if follow_hour {
            lines_vm.push("relate day: warmth = 0 ± 0.001 when hour in 7 .. 19".into());
            lines_vm.push("relate evening: warmth = 1 ± 0.001 when hour in 19 .. 24".into());
            lines_vm.push("relate night: warmth = 1 ± 0.001 when hour in 0 .. 7".into());
        } else {
            lines_vm.push("relate still: warmth = 0 ± 0.001".into());
        }
        // the app's look, held to the machine's (softly)
        for p in COLOURS {
            for part in ["h", "s", "l"] {
                lines_vm.push(format!("node app_{p}_{part} signed"));
            }
        }
        // (the cascade goes one way: a parent is held to its own parent a hundred times more surely than a child
        // is held to it, so what a child says barely touches it)
        lines_vm.push("relate inherit_app_accent_h: app_accent_h = vm_accent_h - 25 * warmth ± 0.01".into());
        lines_vm.push("relate inherit_app_accent_s: app_accent_s = vm_accent_s ± 0.01".into());
        lines_vm.push("relate inherit_app_accent_l: app_accent_l = vm_accent_l ± 0.01".into());
        lines_vm.push("relate inherit_app_bg_h: app_bg_h = vm_bg_h ± 0.01".into());
        lines_vm.push("relate inherit_app_bg_s: app_bg_s = vm_bg_s ± 0.01".into());
        lines_vm.push("relate inherit_app_bg_l: app_bg_l = vm_bg_l - 0.03 * warmth ± 0.01".into());
        for part in ["h", "s", "l"] {
            lines_vm.push(format!("relate inherit_app_text_{part}: app_text_{part} = vm_text_{part} ± 0.01"));
        }
        for p in NUMBERS {
            lines_vm.push(format!("node app_{p} signed"));
            lines_vm.push(format!("relate inherit_app_{p}: app_{p} = vm_{p} ± 0.01"));
        }
        for l in &lines_vm {
            let _ = say(&mut world, l);
        }
        // the app's own looks
        let mut k = 0;
        for (line, text) in lines {
            if let Some(st) = text.strip_prefix("style ") {
                if let Err(e) = say(&mut world, st) {
                    errors.push((*line, e));
                }
                continue;
            }
            let Some(rest) = text.strip_prefix("look ") else { continue };
            let Some((name, body)) = rest.split_once(':') else {
                errors.push((
                    *line,
                    "look NAME: accent = #rrggbb, background = .., text = .., round = N, size = N [when ...]".into(),
                ));
                continue;
            };
            let name = name.trim().to_string();
            if !grm_engine::lang::is_name(&name) {
                errors.push((*line, format!("{name} is not a name")));
                continue;
            }
            if !names.contains(&name) {
                names.push(name.clone());
                // a named look is held to the app's
                for p in COLOURS {
                    for part in ["h", "s", "l"] {
                        let _ = say(&mut world, &format!("node {name}_{p}_{part} signed"));
                        let _ = say(
                            &mut world,
                            &format!("relate inherit_{name}_{p}_{part}: {name}_{p}_{part} = app_{p}_{part} ± 1"),
                        );
                    }
                }
                for p in NUMBERS {
                    let _ = say(&mut world, &format!("node {name}_{p} signed"));
                    let _ = say(&mut world, &format!("relate inherit_{name}_{p}: {name}_{p} = app_{p} ± 1"));
                }
            }
            let (body, when) = match body.split_once(" when ") {
                Some((b, w)) => (b, format!(" when {}", w.trim())),
                None => (body, String::new()),
            };
            for part in body.split(',') {
                let Some((prop, value)) = part.split_once('=') else { continue };
                let (prop, value) = (prop.trim(), value.trim());
                let p = match prop {
                    "accent" => "accent",
                    "background" | "bg" => "bg",
                    "text" => "text",
                    "round" => "round",
                    "size" => "size",
                    other => {
                        errors.push((*line, format!("{other}: accent, background, text, round or size")));
                        continue;
                    }
                };
                k += 1;
                if NUMBERS.contains(&p) {
                    let Ok(v) = value.parse::<f64>() else {
                        errors.push((*line, format!("{value} is not a number")));
                        continue;
                    };
                    if let Err(e) = say(&mut world, &format!("relate said_{k}: {name}_{p} = {v} ± 0.0001{when}")) {
                        errors.push((*line, e));
                    }
                } else {
                    let Some(c) = hex(value) else {
                        errors.push((*line, format!("{value}: a colour is written #rrggbb")));
                        continue;
                    };
                    let (h, s, l) = hsl(c);
                    for (part, v) in [("h", h), ("s", s), ("l", l)] {
                        if let Err(e) =
                            say(&mut world, &format!("relate said_{k}_{part}: {name}_{p}_{part} = {v} ± 0.0001{when}"))
                        {
                            errors.push((*line, e));
                        }
                    }
                }
            }
        }
        // a machine made anew from the whole network: every relation awake from the start
        if let Ok(m) = grm_engine::machine::Machine::new(world.machine.net.clone()) {
            world.machine = m;
            world.machine.set_threads(Some(1));
        }
        let mut looks = Looks { world, key, names, resolved: HashMap::new(), errors, hour_seen: -1 };
        looks.settle(true);
        looks
    }

    /// The hour comes in (each minute), and every look is read again.
    pub fn settle(&mut self, force: bool) {
        let now = local_minutes();
        if !force && now == self.hour_seen {
            return;
        }
        self.hour_seen = now;
        let _ = self.world.say(&format!("observe hour = {} ± 0.0001", now as f64 / 60.0), &Provenance::of("clock"));
        self.world.machine.run(20000);
        let m = &self.world.machine;
        let v = |n: &str| m.value_si(n);
        for name in &self.names {
            let colour =
                |p: &str| rgb(v(&format!("{name}_{p}_h")), v(&format!("{name}_{p}_s")), v(&format!("{name}_{p}_l")));
            self.resolved.insert(
                name.clone(),
                Look {
                    accent: colour("accent"),
                    background: colour("bg"),
                    text: colour("text"),
                    round: v(&format!("{name}_round")).clamp(0.0, 40.0) as f32,
                    size: v(&format!("{name}_size")).clamp(0.5, 3.0) as f32,
                },
            );
        }
    }

    pub fn get(&self, name: Option<&str>) -> Look {
        name.and_then(|n| self.resolved.get(n)).or_else(|| self.resolved.get("app")).copied().unwrap_or(Look {
            accent: Color32::from_rgb(0xE8, 0xA8, 0x3D),
            background: Color32::from_rgb(0x17, 0x18, 0x1B),
            text: Color32::WHITE,
            round: 6.0,
            size: 1.0,
        })
    }
}

/// Minutes since midnight, local time (from the system's offset when it can be read; UTC otherwise).
fn local_minutes() -> i64 {
    let utc =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let offset =
        std::env::var("GRM_UTC_OFFSET_MINUTES").ok().and_then(|s| s.parse::<i64>().ok()).unwrap_or_else(local_offset);
    ((utc / 60 + offset) % 1440 + 1440) % 1440
}

/// The local offset from UTC, in minutes, as `date` says it (0 when it cannot be known).
fn local_offset() -> i64 {
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        std::process::Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                let s = s.trim();
                let sign = if s.starts_with('-') { -1 } else { 1 };
                let d = s.trim_start_matches(['+', '-']);
                let (h, m) = (d.get(..2)?.parse::<i64>().ok()?, d.get(2..4)?.parse::<i64>().ok()?);
                Some(sign * (h * 60 + m))
            })
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_inherit_unless_said_and_follow_their_conditions() {
        let p = Palette::default();
        let lines = vec![
            (1, "look app: round = 12".to_string()),
            (2, "look big: size = 1.5, accent = #3366cc".to_string()),
            (3, "look calm: accent = #00ff00 when hour in 0 .. 24".to_string()),
        ];
        let l = Looks::build(&p, &lines, false);
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let app = l.get(None);
        assert!((app.round - 12.0).abs() < 0.01 && (app.size - 1.0).abs() < 0.01, "{app:?}");
        // the app's accent is the machine's
        let a = crate::style::c(p.accent);
        assert!((app.accent.r() as i32 - a.r() as i32).abs() <= 2, "{:?} vs {:?}", app.accent, a);
        let big = l.get(Some("big"));
        assert!(
            (big.size - 1.5).abs() < 0.01 && (big.round - 12.0).abs() < 0.01,
            "inherits the app's roundness: {big:?}"
        );
        assert!((big.accent.b() as i32 - 0xcc).abs() <= 2, "{:?}", big.accent);
        let calm = l.get(Some("calm"));
        assert!(calm.accent.g() > 240 && calm.accent.r() < 10, "{:?}", calm.accent);
    }
}
