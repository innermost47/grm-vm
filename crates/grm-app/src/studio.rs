// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The studio: one app where the human adds instruments (synthesisers, drum machines), a transport that holds the
//! time for all of them (sticky at the top: bars going by, a loop), and a mixer at the bottom (a strip each: level,
//! pan, sends to one echo and one reverb shared by all, a meter). The instruments are tools, written in GRML
//! (`networks/synth.grm`, `networks/drums.grm`): she is not one of them. Each has its hands (`hand NAME when
//! SWITCH: ...`): the human gives her a part (a synthesiser, one drum of the machine) by its switch, and she plays it
//! through its hands only.
//!
//! The song (its tracks) is kept beside the memory; the app's file is written from it (`apps/studio.grm`) each time
//! a track comes or goes.

use eframe::egui::{self, vec2, RichText, Sense, Stroke};

use crate::app::App;
use crate::app::Ink;
use crate::text::t;

pub const APP: &str = "studio";

/// Where her mind is (her network, her lessons, what she starts knowing): beside the memory, or here.
pub fn mind_dir(memory: &std::path::Path) -> Option<std::path::PathBuf> {
    [memory.parent().map(|p| p.join("mind")), Some(std::path::PathBuf::from("mind"))]
        .into_iter()
        .flatten()
        .find(|d| d.join("elle.grm").exists())
}

/// The drums: (letter, name).
pub const DRUMS: [(&str, &str); 4] = [("k", "kick"), ("s", "snare"), ("h", "hat"), ("t", "tom")];

fn value(app: &App, node: &str) -> f64 {
    app.picture.node(node).map(|n| n.value).unwrap_or(0.0)
}

/// A drum machine's grid: a row per drum, its steps (two bars of sixteen, or one), the one playing lit; the
/// human's pads beside; a drum she holds is hers (she strikes it too: the human sees it). Drawn inside the drum
/// machine's section of the studio (a `drums` widget).
pub fn drum_grid(app: &mut App, ui: &mut egui::Ui, n: &str, ink: &Ink) {
    let lang = app.settings.lang;
    let long = value(app, &format!("{n}_long")) > 0.5;
    let steps = if long { 32 } else { 16 };
    let (now_i, half) = app
        .mixer
        .lock()
        .ok()
        .and_then(|m| Some((m.value("out", "tr_i")?, m.value("out", "tr_h").unwrap_or(0.0))))
        .unwrap_or((-1.0, 0.0));
    let now = now_i.round() as i64 + if long { 16 * half.round() as i64 } else { 0 };
    // its length: one bar, or two
    ui.horizontal(|ui| {
        ui.label(RichText::new(t(lang, "drum_steps")).small().color(ink.muted));
        for (word, on) in [("16", false), ("32", true)] {
            if ui.selectable_label(long == on, RichText::new(word).small()).clicked() && long != on {
                app.write(&format!("{n}_long"), if on { 1.0 } else { 0.0 });
            }
        }
    });
    // the steps share the width there is (never narrower than a finger's tip, never wider than they were)
    let room = ui.available_width() - 70.0 - 44.0 - 16.0 - 60.0 - 4.0 * (steps / 4) as f32;
    let w = (room / steps as f32).clamp(11.0, 24.0);
    for (x, name) in DRUMS {
        let hers = value(app, &format!("{n}_{x}_hers")) > 0.5;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_sized([70.0, 22.0], egui::Label::new(RichText::new(name).small()));
            // the human's pad: it sounds while pressed
            let pad = format!("{n}_{x}_pad");
            let (rect, r) = ui.allocate_exact_size(vec2(44.0, 22.0), Sense::click_and_drag());
            let down = r.is_pointer_button_down_on();
            ui.painter().rect_filled(rect, crate::style::corner(), if down { ink.accent } else { ink.surface });
            ui.painter().rect_stroke(
                rect,
                crate::style::corner(),
                Stroke::new(1.0_f32, ink.faint),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "pad",
                crate::style::font(crate::style::Size::Small),
                ink.text,
            );
            let was = value(app, &pad) > 0.5;
            if down != was {
                app.write(&pad, if down { 1.0 } else { 0.0 });
            }
            ui.add_space(16.0);
            for k in 0..steps {
                let node = format!("{n}_{x}_s{}", k + 1);
                let on = value(app, &node) > 0.5;
                let (rect, r) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
                // how strong its place in the bar is: the first beat, the other beats, the eighths between, the
                // sixteenths (shades of one grey: what is lit stays what is seen first)
                let weight = match k % 16 {
                    0 => 0.34,
                    4 | 8 | 12 => 0.24,
                    2 | 6 | 10 | 14 => 0.13,
                    _ => 0.04,
                };
                let fill = if on {
                    if hers {
                        ink.accent.gamma_multiply(0.75)
                    } else {
                        ink.accent
                    }
                } else {
                    crate::view::mix(ink.surface, ink.text, weight)
                };
                ui.painter().rect_filled(rect.shrink(1.5), crate::style::corner(), fill);
                if k as i64 == now {
                    ui.painter().rect_stroke(
                        rect.shrink(0.5),
                        3.0,
                        Stroke::new(1.5_f32, ink.text),
                        egui::StrokeKind::Inside,
                    );
                }
                let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                let r = if hers { r.on_hover_text(t(lang, "drum_hers")) } else { r };
                if r.clicked() {
                    app.write(&node, if on { 0.0 } else { 1.0 });
                }
                if k % 4 == 3 {
                    ui.add_space(if k % 16 == 15 { 10.0 } else { 4.0 });
                }
            }
            ui.add_space(6.0);
            if hers {
                crate::icons::show(ui, "hand", 12.0, ink.accent);
                ui.label(RichText::new(t(lang, "hers_word")).small().color(ink.muted));
            }
        });
    }
}

#[cfg(test)]
mod tests {

    /// Her home and the studio opened as the window opens them, the studio's sound played: what is heard.
    fn studio_heard(seconds: usize, keys: bool) -> (f32, f32, usize, Vec<f64>) {
        use grm_engine::engine::Engine;
        use grm_engine::world::World;
        use std::sync::{Arc, Mutex};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("grm-studio-{keys}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let engine = Engine::start(World::new(), std::time::Duration::from_millis(4));
        let mixer = Arc::new(Mutex::new(grm_sound::mixer::Mixer::new(48000)));
        let mut sh = grm_shell::Shell::new(dir.join("w.world"), mixer.clone());
        sh.always = Some(root.join("mind/her.grm"));
        let out = sh.wake(&engine);
        assert!(out.iter().all(|l| l.kind != grm_shell::Kind::Error), "{out:?}");
        assert!(mixer.lock().unwrap().has_world(), "she is there before anything plays");
        let opened = std::time::Instant::now();
        let (out, _) = sh.run(&engine, &format!("app open {}", root.join("apps").join("studio.grm").display()));
        // thousands of lines said at once: in a moment, not in seconds
        assert!(opened.elapsed() < std::time::Duration::from_secs(10), "the studio opened in {:.2?}", opened.elapsed());
        assert!(out.iter().all(|l| l.kind != grm_shell::Kind::Error), "{out:?}");
        // the app said whole before it plays
        for _ in 0..300 {
            let (o, _) = sh.run(&engine, "show t3_out");
            if o.iter().any(|l| l.text.contains("t3_out =")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (out, _) = sh.run(&engine, "play out");
        assert!(out[0].text.contains("plays"), "{out:?}");
        if keys {
            let mut m = mixer.lock().unwrap();
            // a lesson of the major scale given to her
            m.set_now("elle_lesson_pick", 4.0);
            m.set_now("t1_pitch", 220.0);
            m.set_now("t1_hold", 1.0);
        }
        let mut buf = vec![0.0f32; 2 * 480];
        let (mut loud, mut t1) = (0.0f32, 0.0f32);
        let mut gates = 0;
        let mut her = vec![];
        let started = std::time::Instant::now();
        for k in 0..seconds * 100 {
            mixer.lock().unwrap().render(&mut buf);
            let m = mixer.lock().unwrap();
            loud = loud.max(buf.iter().fold(0.0f32, |a, x| a.max(x.abs())));
            t1 = t1.max(m.world_value("t1_out").unwrap_or(0.0).abs() as f32);
            if m.world_value("elle_gate").unwrap_or(0.0) > 0.5 {
                gates += 1;
            }
            if k % 10 == 0 {
                her.push(m.world_value("elle_pitch").unwrap_or(0.0));
            }
        }
        // the sound computed faster than it plays (with room to spare on a slower machine)
        assert!(
            started.elapsed() < std::time::Duration::from_secs(seconds as u64 * 2),
            "{seconds} s of sound in {:.2?}",
            started.elapsed()
        );
        engine.stop();
        let _ = std::fs::remove_dir_all(&dir);
        (loud, t1, gates, her)
    }

    #[test]
    fn the_human_plays_the_studio_synthesiser_and_she_plays_hers() {
        let (loud, t1, gates, her) = studio_heard(4, true);
        assert!(t1 > 0.01, "the human's synthesiser is silent: {t1}");
        assert!(loud > 0.01, "nothing heard: {loud}");
        assert!(gates > 0, "she does not play");
        let mut moved = her.clone();
        moved.dedup();
        assert!(moved.len() > 1, "her note never moves: {her:?}");
    }

    /// Every lesson in her mind is said by the studio (its list `lessons`, in the order of her files).
    #[test]
    fn the_studio_says_every_lesson_of_hers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join("apps/studio.grm")).unwrap();
        let parsed = grm_shell::apps::parse(&text, &root.join("apps"));
        let listed = parsed.lists["lessons"].0.clone();
        let mut files: Vec<String> = std::fs::read_dir(root.join("mind/lessons"))
            .unwrap()
            .flatten()
            .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string()))
            .collect();
        files.sort();
        assert_eq!(listed, files, "a lesson of hers the studio does not say");
        // and each can be given to her by its name, in the same order
        let lesson = parsed.widgets.iter().find(|w| w.name == "lesson").expect("the lesson's choice");
        assert_eq!(lesson.text.split('|').count(), files.len() + 1);
    }

    /// Played again: « c'est écrit heard clearly avec la barre à fond alors qu'elle n'est pas supposée s'entendre »:
    /// her lines of clarity (`mind/elle.grm`: the sum of squares over the square of the sum, each with its floor)
    /// say that silence is not heard clearly: the floor of the squares far below the square of the sum's.
    #[test]
    fn silence_is_not_heard_clearly() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../mind/elle.grm")).unwrap();
        let floor = |key: &str| -> f64 {
            let line = text.lines().find(|l| l.contains(key)).expect(key);
            line.rsplit('+').next().unwrap().trim().parse().expect(line)
        };
        let (tot, sq) = (floor("relate {name}_all:"), floor("relate {name}_squares:"));
        // silence: every band empty, only the floors
        let clar = sq / (tot * tot);
        assert!(clar < 0.01, "silence heard clearly: {clar}");
    }
}
