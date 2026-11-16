// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The side panel: the inspector, the sandboxes, the sources and the quarantine, the journal, the eye, the sound.

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Shape, Stroke};

use crate::app::{App, Ink, Tab, TABS};
use crate::text::t;
use crate::view::reading;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.settings.lang;
    ui.horizontal_wrapped(|ui| {
        for (k, (tab, key)) in TABS.iter().enumerate() {
            let r = ui.selectable_label(app.tab == *tab, t(lang, key)).on_hover_text(format!("Ctrl+{}", k + 1));
            if r.clicked() {
                app.tab = *tab;
            }
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.tab {
        Tab::Inspector => inspector(app, ui),
        Tab::Sandboxes => sandboxes(app, ui),
        Tab::Sources => sources(app, ui),
        Tab::Journal => journal(app, ui),
        Tab::Eye => eye(app, ui),
        Tab::Sound => sound(app, ui),
    });
}

fn chip(ui: &mut egui::Ui, text: &str, colour: Color32) {
    egui::Frame::new().stroke(Stroke::new(1.0_f32, colour)).corner_radius(8.0).inner_margin(vec2(6.0, 1.0)).show(
        ui,
        |ui| {
            ui.label(RichText::new(text).small().color(colour));
        },
    );
}

/// The nodes, each a line; a click unfolds what it is in place (an accordion: several can be open; a node chosen in
/// the network opens there, and the list goes to it).
fn inspector(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let pic = app.picture.clone();
    ui.add(egui::TextEdit::singleline(&mut app.filter).hint_text(t(lang, "filter")).desired_width(f32::INFINITY));
    if app.selected.len() > 1 {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", app.selected.len(), t(lang, "selected_many")));
            if ui.button(t(lang, "joint")).clicked() {
                let names = app.selected.join(" ");
                app.say(format!("joint {names}"));
            }
            if ui.button(t(lang, "clear")).clicked() {
                app.selected.clear();
            }
        });
    }
    ui.add_space(4.0);
    let memory = egui::Id::new("inspector-open");
    let was_open: Vec<String> = ui.ctx().data(|d| d.get_temp(memory)).unwrap_or_default();
    let filter = app.filter.to_lowercase();
    let mut shown = 0;
    for n in &pic.nodes {
        let open = app.selected.contains(&n.name);
        if !open && (shown >= 400 || !(filter.is_empty() || n.name.to_lowercase().contains(&filter))) {
            continue;
        }
        if n.name.contains("__next") {
            continue;
        }
        shown += 1;
        let k = app.colours.get(&n.source).copied().unwrap_or(0);
        let r = ui
            .horizontal(|ui| {
                crate::icons::show(ui, if open { "chevron-down" } else { "chevron-right" }, 14.0, ink.muted);
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, ink.source(k));
                let name = RichText::new(&n.name).color(if open { ink.accent } else { ink.text });
                let name = if open { name.strong() } else { name };
                ui.label(name);
                ui.label(RichText::new(reading(n.value, n.sigma, n.signed)).color(ink.muted).small());
            })
            .response;
        let r = ui.interact(r.rect, egui::Id::new(("inspector-row", &n.name)), Sense::click());
        if r.hovered() {
            ui.painter().rect_filled(r.rect.expand(1.0), 3.0, ink.faint.gamma_multiply(0.25));
        }
        if r.clicked() {
            app.select(&n.name, true);
        }
        if open && !was_open.contains(&n.name) {
            r.scroll_to_me(Some(egui::Align::TOP));
        }
        if open {
            let name = n.name.clone();
            egui::Frame::new()
                .inner_margin(egui::Margin { left: 18, right: 4, top: 4, bottom: 10 })
                .show(ui, |ui| sheet(app, ui, &name, &pic));
            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, ink.faint);
        }
    }
    let open_now = app.selected.clone();
    ui.ctx().data_mut(|d| d.insert_temp(memory, open_now));
}

/// What a node is, unfolded.
fn sheet(app: &mut App, ui: &mut egui::Ui, name: &str, pic: &crate::model::Picture) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let name = name.to_string();
    let Some(n) = pic.node(&name).cloned() else { return };
    ui.label(RichText::new(reading(n.value, n.sigma, n.signed)).size(18.0).monospace());
    ui.horizontal_wrapped(|ui| {
        let k = app.colours.get(&n.source).copied().unwrap_or(0);
        chip(ui, &format!("{} {}", t(lang, "source"), n.source), ink.source(k));
        if pic.quarantine.contains(&n.source) {
            chip(ui, t(lang, "in_quarantine"), ink.amber);
        }
        chip(ui, if n.observed { t(lang, "observed") } else { t(lang, "computed") }, ink.muted);
        for (on, key) in [
            (n.fixed, "fixed"),
            (n.var, "var"),
            (n.learn, "learn"),
            (n.input, "input"),
            (n.output, "output"),
            (n.tracked, "tracked"),
        ] {
            if on {
                chip(ui, t(lang, key), ink.muted);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if !n.tracked && ui.button(t(lang, "track")).clicked() {
            app.say(format!("track {name}"));
        }
        if n.tracked {
            if ui.button(t(lang, "forecast")).clicked() {
                app.say(format!("forecast {name} 12"));
            }
            if ui.button(t(lang, "embody")).clicked() {
                app.say(format!("embody {name}"));
            }
        }
        if ui.button(t(lang, "play")).clicked() {
            app.say(format!("play {name}"));
        }
    });
    ui.add_space(6.0);
    ui.label(RichText::new(t(lang, "relations")).strong());
    for r in pic.relations.iter().filter(|r| r.nodes.contains(&name)) {
        let strain = app.tension.get(&r.name).copied().unwrap_or(0.0);
        let colour = if strain > 1.0 { ink.amber } else { ink.text };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}:", r.name)).strong().color(colour));
            ui.label(RichText::new(&r.text).monospace().color(colour));
        });
        let mut note = format!("{} · {} {:.1} σ", r.source, t(lang, "strain"), r.tension);
        if !r.awake {
            note.push_str(&format!(" · {}", t(lang, "asleep")));
        }
        if r.strength < 0.999 {
            note.push_str(&format!(" · {} ({:.0} %)", t(lang, "integrating"), r.strength * 100.0));
        }
        ui.label(RichText::new(note).small().color(ink.muted));
        if let Some((app_name, l)) = app.line_in_app(&r.name) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{app_name}, {} {l}", t(lang, "line"))).small().color(ink.muted));
                if ui.small_button(t(lang, "edit_line")).clicked() {
                    let text = app
                        .apps
                        .iter()
                        .find(|a| a.name == app_name)
                        .and_then(|a| a.text.lines().nth(l - 1))
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    app.line_draft = Some((Some(app_name.clone()), Some(l), text));
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            for other in r.nodes.iter().filter(|o| **o != name && !o.contains("__next")) {
                if ui.small_button(other).clicked() {
                    app.select(other, false);
                }
            }
        });
    }
    if n.tracked {
        ui.add_space(8.0);
        ui.label(RichText::new(t(lang, "time")).strong());
        if let Some(d) = app.detail.clone().filter(|d| d.node == name) {
            chart(ui, &d, &ink, lang);
            ui.add_space(4.0);
            ui.label(RichText::new(t(lang, "why")).strong());
            ui.label(RichText::new(&d.why).monospace().small());
        } else {
            ui.label(RichText::new("…").color(ink.muted));
        }
    }
}

/// Its memory (a line, its spread around it), then what it will be (dashed, its sigma, the range of the draws).
fn chart(ui: &mut egui::Ui, d: &crate::model::Detail, ink: &Ink, lang: crate::text::Lang) {
    if d.history.is_empty() && d.forecast.is_empty() {
        ui.label(RichText::new(t(lang, "nothing_remembered")).small().color(ink.muted));
        return;
    }
    let w = ui.available_width().max(120.0);
    let (resp, painter) = ui.allocate_painter(vec2(w, 170.0), Sense::hover());
    let rect = resp.rect.shrink2(vec2(4.0, 14.0));
    let hist: Vec<(f64, f64, f64)> = d.history.iter().map(|h| ((h.0 + h.1) / 2.0, h.2, h.3)).collect();
    if hist.is_empty() && d.forecast.is_empty() {
        return;
    }
    let mut t0 = f64::INFINITY;
    let mut t1 = f64::NEG_INFINITY;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &(t, m, s) in &hist {
        t0 = t0.min(t);
        t1 = t1.max(t);
        lo = lo.min(m - s);
        hi = hi.max(m + s);
    }
    for &(t, m, s, a, b) in &d.forecast {
        t0 = t0.min(t);
        t1 = t1.max(t);
        lo = lo.min(a.min(m - s));
        hi = hi.max(b.max(m + s));
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return;
    }
    if hi - lo < 1e-12 {
        hi += 1.0;
        lo -= 1.0;
    }
    if t1 - t0 < 1e-12 {
        t1 += 1.0;
    }
    let x = |t: f64| rect.left() + ((t - t0) / (t1 - t0)) as f32 * rect.width();
    let y = |v: f64| rect.bottom() - ((v - lo) / (hi - lo)) as f32 * rect.height();
    // the grid, quiet
    for k in 0..3 {
        let v = lo + (hi - lo) * k as f64 / 2.0;
        painter.line_segment([pos2(rect.left(), y(v)), pos2(rect.right(), y(v))], Stroke::new(0.5_f32, ink.faint));
        painter.text(
            pos2(rect.left(), y(v) - 1.0),
            egui::Align2::LEFT_BOTTOM,
            grm_engine::lang::g(v),
            egui::FontId::proportional(10.0),
            ink.muted,
        );
    }
    let band = |pts: &[(f32, f32, f32)], colour: Color32| {
        // a band as quads between successive points (no concave polygon)
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let quad = vec![pos2(a.0, a.1), pos2(b.0, b.1), pos2(b.0, b.2), pos2(a.0, a.2)];
            painter.add(Shape::convex_polygon(quad, colour, Stroke::NONE));
        }
    };
    let line_colour = ink.accent;
    let soft = Color32::from_rgba_unmultiplied(line_colour.r(), line_colour.g(), line_colour.b(), 40);
    let softer = Color32::from_rgba_unmultiplied(line_colour.r(), line_colour.g(), line_colour.b(), 20);
    let hb: Vec<(f32, f32, f32)> = hist.iter().map(|&(t, m, s)| (x(t), y(m + s), y(m - s))).collect();
    band(&hb, soft);
    let hl: Vec<egui::Pos2> = hist.iter().map(|&(t, m, _)| pos2(x(t), y(m))).collect();
    if hl.len() > 1 {
        painter.add(Shape::line(hl.clone(), Stroke::new(2.0_f32, line_colour)));
    }
    let range: Vec<(f32, f32, f32)> = d.forecast.iter().map(|&(t, _, _, a, b)| (x(t), y(b), y(a))).collect();
    band(&range, softer);
    let sig: Vec<(f32, f32, f32)> = d.forecast.iter().map(|&(t, m, s, _, _)| (x(t), y(m + s), y(m - s))).collect();
    band(&sig, soft);
    let mut fl: Vec<egui::Pos2> = hl.last().copied().into_iter().collect();
    fl.extend(d.forecast.iter().map(|&(t, m, _, _, _)| pos2(x(t), y(m))));
    if fl.len() > 1 {
        for w in fl.windows(2) {
            painter.extend(Shape::dashed_line(&[w[0], w[1]], Stroke::new(2.0_f32, line_colour), 5.0, 4.0));
        }
    }
    if d.now >= t0 && d.now <= t1 {
        let xn = x(d.now);
        painter.line_segment([pos2(xn, rect.top()), pos2(xn, rect.bottom())], Stroke::new(1.0_f32, ink.muted));
        painter.text(
            pos2(xn + 3.0, rect.top()),
            egui::Align2::LEFT_TOP,
            t(lang, "now"),
            egui::FontId::proportional(10.0),
            ink.muted,
        );
    }
    painter.text(
        pos2(rect.left(), resp.rect.bottom()),
        egui::Align2::LEFT_BOTTOM,
        format!("{} · {} ({})", t(lang, "remembered"), t(lang, "foreseen"), d.unit),
        egui::FontId::proportional(10.0),
        ink.muted,
    );
}

fn sandboxes(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_sandbox).hint_text(t(lang, "new_sandbox")).desired_width(160.0));
        if ui.button(t(lang, "open")).clicked() && !app.new_sandbox.trim().is_empty() {
            let name = std::mem::take(&mut app.new_sandbox);
            app.say(format!("sandbox {}", name.trim()));
        }
    });
    let pic = app.picture.clone();
    if pic.sandboxes.is_empty() {
        ui.label(RichText::new(t(lang, "no_sandbox")).color(ink.muted));
    }
    for s in &pic.sandboxes {
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(&s.name).strong());
            for l in &s.lines {
                ui.label(RichText::new(l).monospace().small());
            }
            ui.horizontal(|ui| {
                let field = app.sandbox_lines.entry(s.name.clone()).or_default();
                let r = ui.add(
                    egui::TextEdit::singleline(field).hint_text(t(lang, "say_in")).font(egui::TextStyle::Monospace),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let line = std::mem::take(field);
                    if !line.trim().is_empty() {
                        app.say(format!("in {}: {}", s.name, line.trim()));
                    }
                }
            });
            ui.horizontal(|ui| {
                if crate::style::chosen(ui, app.sandbox_shown.as_deref() == Some(s.name.as_str()), t(lang, "show_it"))
                    .clicked()
                {
                    app.sandbox_shown = Some(s.name.clone());
                    app.current_file = None;
                }
                for (key, word) in [("run", "run"), ("trial", "trial"), ("merge", "merge"), ("refuse", "refuse")] {
                    if ui.button(t(lang, key)).clicked() {
                        app.say(format!("{word} {}", s.name));
                    }
                }
            });
        });
    }
    if !pic.integrating.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new(t(lang, "integrating")).color(ink.muted));
        for i in &pic.integrating {
            ui.label(RichText::new(i).small().color(ink.muted));
        }
    }
}

fn sources(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let pic = app.picture.clone();
    if pic.sources.is_empty() {
        ui.label(RichText::new(t(lang, "no_source")).color(ink.muted));
    }
    for (s, count) in &pic.sources {
        let q = pic.quarantine.contains(s);
        let k = app.colours.get(s).copied().unwrap_or(0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 5.0, ink.source(k));
            ui.label(RichText::new(s).strong());
            ui.label(RichText::new(format!("{count} {}", t(lang, "relations_n"))).color(ink.muted));
            if q {
                ui.label(RichText::new(t(lang, "in_quarantine")).color(ink.amber));
            }
        });
        ui.horizontal(|ui| {
            if q {
                if ui.small_button(t(lang, "release")).clicked() {
                    app.say(format!("release {s}"));
                }
            } else if ui.small_button(t(lang, "quarantine")).clicked() {
                app.say(format!("quarantine {s}"));
            }
            if ui.small_button(t(lang, "recall")).clicked() {
                app.say(format!("recall {s}"));
            }
        });
        ui.add_space(4.0);
    }
}

fn journal(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    // the window's changes: each can be taken back (Ctrl+Z), in the order they were made
    if !app.journey.undo.is_empty() || !app.journey.redo.is_empty() {
        ui.label(RichText::new(t(lang, "changes")).strong());
        for e in app.journey.redo.iter().take(5) {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "redo-2", 13.0, ink.faint);
                ui.label(RichText::new(&e.what).small().color(ink.faint)).on_hover_text(t(lang, "taken_back"));
            });
        }
        for e in app.journey.undo.iter().rev().take(12) {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "undo-2", 13.0, ink.muted);
                ui.label(RichText::new(&e.what).small().monospace());
            });
        }
        ui.separator();
    }
    for (tick, kind, text) in app.picture.journal.iter().rev() {
        let colour =
            if matches!(kind.as_str(), "quarantine" | "revised" | "recall" | "refused") { ink.amber } else { ink.text };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("[{tick}]")).monospace().small().color(ink.muted));
            ui.label(RichText::new(kind).small().strong().color(colour));
            ui.label(RichText::new(text).small());
        });
    }
}

fn eye(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.eye_path).hint_text(t(lang, "picture_path")).desired_width(150.0));
        if crate::icons::sign(ui, "folder-open", t(lang, "choose")).clicked() {
            app.pick(crate::app::Pick::Eye);
        }
        ui.add(egui::TextEdit::singleline(&mut app.eye_name).hint_text(t(lang, "eye_name")).desired_width(60.0));
        if ui.button(t(lang, "see")).clicked() && !app.eye_path.trim().is_empty() {
            let line = format!("see {} {}", app.eye_path.trim(), app.eye_name.trim());
            app.say(line);
        }
    });
    if app.eyes.is_empty() {
        ui.label(RichText::new(t(lang, "no_sight")).color(ink.muted));
        return;
    }
    let width = ui.available_width();
    for (name, e) in &app.eyes {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{name}: {}x{} · {} rounds · {:.2?}",
                e.sight.clean.width, e.sight.clean.height, e.sight.rounds, e.sight.elapsed
            ))
            .strong(),
        );
        let n = e.textures.len().max(1) as f32;
        let side = ((width - 8.0 * n) / n).clamp(60.0, 240.0);
        ui.horizontal(|ui| {
            for (tag, tex) in &e.textures {
                ui.vertical(|ui| {
                    let size = tex.size_vec2();
                    let scale = side / size.x.max(size.y).max(1.0);
                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), size * scale)));
                    ui.label(RichText::new(t(lang, tag)).small().color(ink.muted));
                });
            }
        });
        let top: Vec<String> = e.sight.surprises(3).iter().map(|(x, y, s)| format!("({x}, {y}) {s:.1} σ")).collect();
        ui.label(RichText::new(format!("{}: {}", t(lang, "surprise"), top.join(", "))).small());
    }
}

fn sound(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    match &app.device {
        Some(d) => ui.label(RichText::new(format!("{}: {d}", t(lang, "device"))).color(ink.muted)),
        None => ui.label(RichText::new(t(lang, "no_device")).color(ink.amber)),
    };
    ui.label(RichText::new(t(lang, "sound_note")).small().color(ink.muted));
    ui.add_space(6.0);
    if ui.button(RichText::new(format!("{}  (Ctrl+.)", t(lang, "panic"))).color(ink.amber)).clicked() {
        app.say("panic");
    }
    ui.add_space(6.0);
    // the machine's own voice
    ui.label(RichText::new(t(lang, "its_voice")).strong());
    let mut changed_voice = None;
    let (mut on, mut level, mut master, peak, sea, views) = match app.mixer.lock() {
        Ok(m) => (m.ambient_on, m.ambient_gain, m.master, m.peak, m.ambient.sea(), m.views()),
        Err(_) => return,
    };
    let was_on = on;
    ui.horizontal(|ui| {
        ui.checkbox(&mut on, t(lang, "voice_on"));
        ui.label(t(lang, "voice_kind"));
        for kind in ["pad", "engine", "wind"] {
            if ui.selectable_label(app.settings.voice == kind, kind).clicked() && app.settings.voice != kind {
                changed_voice = Some(kind.to_string());
            }
        }
    });
    ui.add(egui::Slider::new(&mut level, 0.0..=1.0).text(t(lang, "level")));
    ui.add(egui::Slider::new(&mut master, 0.0..=1.0).text(t(lang, "master")));
    meter(ui, peak, &ink);
    ui.label(
        RichText::new(format!(
            "{} {:.2} · {} {:.2} · {} {:.2}",
            t(lang, "level"),
            sea.0,
            t(lang, "brightness"),
            sea.1,
            t(lang, "fade"),
            sea.2
        ))
        .small()
        .color(ink.muted),
    );
    if on != was_on {
        app.say(if on { "sound on" } else { "sound off" });
        app.settings.voice_on = on;
    }
    if let Some(v) = changed_voice {
        app.say(format!("voice {v}"));
        app.settings.voice = v;
    }
    if let Ok(mut m) = app.mixer.lock() {
        m.ambient_gain = level;
        m.master = master;
    }
    app.settings.voice_level = level;
    app.settings.volume = master;
    ui.add_space(8.0);
    // the human's channels
    ui.label(RichText::new(t(lang, "channels")).strong());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.play_field).hint_text(t(lang, "play_node")).desired_width(150.0));
        if ui.button(t(lang, "play")).clicked() && !app.play_field.trim().is_empty() {
            let n = std::mem::take(&mut app.play_field);
            app.say(format!("play {}", n.trim()));
        }
    });
    if views.is_empty() {
        ui.label(RichText::new(t(lang, "no_channel")).small().color(ink.muted));
    }
    for v in views {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&v.name).strong());
                let mut mute = v.mute;
                let mut solo = v.solo;
                if ui.toggle_value(&mut mute, t(lang, "mute")).on_hover_text("mute").changed() {
                    if let Ok(mut m) = app.mixer.lock() {
                        if let Some(c) = m.channels.get_mut(&v.name) {
                            c.mute = mute;
                        }
                    }
                }
                if ui.toggle_value(&mut solo, t(lang, "solo")).on_hover_text("solo").changed() {
                    if let Ok(mut m) = app.mixer.lock() {
                        if let Some(c) = m.channels.get_mut(&v.name) {
                            c.solo = solo;
                        }
                    }
                }
                if ui.small_button(t(lang, "stop")).clicked() {
                    app.say(format!("stop {}", v.name));
                }
            });
            let mut gain = v.gain;
            let mut pan = v.pan;
            let g = ui.add(egui::Slider::new(&mut gain, 0.0..=2.0).text(t(lang, "gain")));
            let p = ui.add(egui::Slider::new(&mut pan, -1.0..=1.0).text(t(lang, "pan")));
            if g.changed() || p.changed() {
                if let Ok(mut m) = app.mixer.lock() {
                    if let Some(c) = m.channels.get_mut(&v.name) {
                        c.gain = gain;
                        c.pan = pan;
                    }
                }
            }
            meter(ui, v.peak, &ink);
        });
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
}

/// A level: a bar that falls slowly, amber near the limit.
fn meter(ui: &mut egui::Ui, peak: f32, ink: &Ink) {
    let w = ui.available_width().min(260.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, 6.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, ink.faint);
    let mut f = r;
    f.set_width(r.width() * (peak / grm_sound::mixer::LIMIT as f32).clamp(0.0, 1.0));
    ui.painter().rect_filled(f, 3.0, if peak > 0.7 { ink.amber } else { ink.accent });
}
