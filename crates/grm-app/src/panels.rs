// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The side panel: the inspector, the sandboxes, the sources and the quarantine, the journal, the eye, the sound.

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Shape, Stroke};

use crate::app::{App, Ink, Tab, TABS};
use crate::text::t;
use crate::view::reading;

/// Each tab's sign.
pub fn sign_of(tab: Tab) -> &'static str {
    match tab {
        Tab::Inspector => "search",
        Tab::Sandboxes => "hammer",
        Tab::Sources => "waypoints",
        Tab::Journal => "scroll-text",
        Tab::Eye => "eye",
        Tab::Sound => "audio-lines",
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.settings.lang;
    ui.horizontal_wrapped(|ui| {
        if crate::icons::sign(ui, "panel-right-close", t(lang, "fold_side")).clicked() {
            app.side_open = false;
        }
        for (k, (tab, key)) in TABS.iter().enumerate() {
            let r = crate::icons::toggle(ui, sign_of(*tab), t(lang, key), app.tab == *tab)
                .on_hover_text(format!("Ctrl+{}", k + 1));
            if r.clicked() {
                app.tab = *tab;
            }
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.tab {
        Tab::Inspector => crate::tree::show(app, ui),
        Tab::Sandboxes => sandboxes(app, ui),
        Tab::Sources => sources(app, ui),
        Tab::Journal => journal(app, ui),
        Tab::Eye => eye(app, ui),
        Tab::Sound => sound(app, ui),
    });
}

/// The side folded: a rail of signs (each unfolds the side on its tab).
pub fn rail(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.settings.lang;
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        if crate::icons::sign(ui, "panel-right-open", t(lang, "unfold_side")).clicked() {
            app.side_open = true;
        }
        ui.add_space(10.0);
        for (tab, key) in TABS {
            let ink = Ink::of(ui.ctx());
            let colour = if app.tab == tab { ink.accent } else { ink.text };
            let b = egui::Button::image(crate::icons::image(sign_of(tab), 18.0, colour)).frame(false);
            if ui.add(b).on_hover_text(t(lang, key)).clicked() {
                app.tab = tab;
                app.side_open = true;
            }
            ui.add_space(8.0);
        }
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

/// What a node is, unfolded.
pub fn sheet(app: &mut App, ui: &mut egui::Ui, name: &str, pic: &crate::model::Picture) {
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
    let playing = app.mixer.lock().map(|m| m.playing().contains(&name)).unwrap_or(false);
    ui.horizontal_wrapped(|ui| {
        if !n.tracked && crate::icons::button(ui, "timer", t(lang, "track")).on_hover_text(t(lang, "track_d")).clicked()
        {
            app.say(format!("track {name}"));
        }
        if n.tracked {
            if crate::icons::button(ui, "trending-up", t(lang, "forecast"))
                .on_hover_text(t(lang, "forecast_d"))
                .clicked()
            {
                app.say(format!("forecast {name} 12"));
            }
            if crate::icons::button(ui, "brain-circuit", t(lang, "embody")).on_hover_text(t(lang, "embody_d")).clicked()
            {
                app.say(format!("embody {name}"));
            }
        }
        // play becomes stop while it is heard
        if playing {
            if crate::icons::button(ui, "square", t(lang, "stop")).on_hover_text(t(lang, "t_stop_node")).clicked() {
                app.say(format!("stop {name}"));
            }
        } else if crate::icons::button(ui, "play", t(lang, "play")).on_hover_text(t(lang, "t_play_node")).clicked() {
            app.say(format!("play {name}"));
        }
    });
    // how it moved lately (or its wave, while it plays)
    let scope = app.mixer.lock().ok().and_then(|m| m.scope(&name));
    match scope {
        Some(s) => {
            let show: Vec<f32> = s[s.len().saturating_sub(512)..].to_vec();
            crate::hover::curve(ui, &show, ink.accent, &ink, t(lang, "as_it_sounds"));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
        None => {
            let k = app.colours.get(&n.source).copied().unwrap_or(0);
            let v: Vec<f32> = app.traces.get(&name).map(|t| t.iter().copied().collect()).unwrap_or_default();
            crate::hover::curve(ui, &v, ink.source(k), &ink, t(lang, "lately"));
        }
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
    // the workshops: where networks are made apart
    ui.horizontal(|ui| {
        crate::icons::show(ui, "hammer", 15.0, ink.accent);
        ui.label(RichText::new(t(lang, "workshops")).strong());
    });
    ui.label(RichText::new(t(lang, "workshop_d")).small().color(ink.muted));
    if crate::icons::button(ui, "circle-plus", t(lang, "ws_new")).clicked() {
        app.new_workshop_open = true;
    }
    for (name, nodes, rels) in app.picture.workshops.clone() {
        ui.horizontal(|ui| {
            let kind = crate::space::Kind::Workshop(name.clone());
            let on = app.zone_center.as_ref() == Some(&kind);
            if crate::icons::toggle(ui, "hammer", &name, on).on_hover_text(t(lang, "space_full")).clicked() {
                app.space.put(kind.clone());
                app.zone_center = Some(kind);
                app.space_on = true;
            }
            ui.label(RichText::new(format!("{nodes} · {rels}")).small().color(ink.muted));
            if crate::icons::sign(ui, "door-open", t(lang, "ws_let_in_d")).clicked() {
                crate::airlock::open_for_workshop(app, &name);
            }
        });
    }
    ui.add_space(8.0);
    ui.separator();
    // the airlock: what waits to come into the main network
    ui.horizontal(|ui| {
        crate::icons::show(ui, "door-open", 15.0, ink.accent);
        ui.label(RichText::new(t(lang, "airlock")).strong());
    });
    ui.label(RichText::new(t(lang, "airlock_tab_d")).small().color(ink.muted));
    let pic = app.picture.clone();
    if pic.sandboxes.is_empty() {
        ui.label(RichText::new(t(lang, "airlock_empty")).small().color(ink.muted));
    }
    for s in &pic.sandboxes {
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(&s.name).strong());
            crate::style::terminal(ui, &s.lines);
            ui.horizontal_wrapped(|ui| {
                if crate::icons::toggle(
                    ui,
                    "eye",
                    t(lang, "show_it"),
                    app.sandbox_shown.as_deref() == Some(s.name.as_str()),
                )
                .clicked()
                {
                    app.sandbox_shown = Some(s.name.clone());
                    app.current_file = None;
                    app.space_on = false;
                }
                if crate::icons::button(ui, "door-open", t(lang, "airlock_try"))
                    .on_hover_text(t(lang, "airlock_try_d"))
                    .clicked()
                {
                    crate::airlock::open_for_sandbox(app, &s.name);
                }
                if crate::icons::button(ui, "log-in", t(lang, "airlock_let_in"))
                    .on_hover_text(t(lang, "airlock_let_in_d"))
                    .clicked()
                {
                    app.say(format!("merge {}", s.name));
                }
                if crate::icons::button(ui, "ban", t(lang, "airlock_refuse"))
                    .on_hover_text(t(lang, "airlock_refuse_d"))
                    .clicked()
                {
                    app.say(format!("refuse {}", s.name));
                }
            });
            // a line more, written straight in it (for who knows the language)
            let field = app.sandbox_lines.entry(s.name.clone()).or_default();
            let r = ui.add(
                egui::TextEdit::singleline(field)
                    .margin(crate::style::FIELD)
                    .hint_text(crate::style::hint(t(lang, "say_in")))
                    .font(egui::TextStyle::Monospace),
            );
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let line = std::mem::take(field);
                if !line.trim().is_empty() {
                    app.say(format!("in {}: {}", s.name, line.trim()));
                }
            }
        });
    }
    if !pic.integrating.is_empty() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            crate::icons::show(ui, "trending-up", 14.0, ink.muted);
            ui.label(RichText::new(t(lang, "integrating")).color(ink.muted));
        });
        for i in &pic.integrating {
            ui.label(RichText::new(i).small().color(ink.muted));
        }
    }
    // a draft by hand (for who knows the language)
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_sandbox)
                .margin(crate::style::FIELD)
                .hint_text(crate::style::hint(t(lang, "new_sandbox")))
                .desired_width(160.0),
        );
        if ui.button(t(lang, "open")).clicked() && !app.new_sandbox.trim().is_empty() {
            let name = std::mem::take(&mut app.new_sandbox);
            app.say(format!("sandbox {}", name.trim()));
        }
    });
}

/// The trash: apps put away, each can come back.
fn trash(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let dir = grm_shell::runtime::apps_dir(&app.memory).join(".trash");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string())).collect())
        .unwrap_or_default();
    names.sort();
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        crate::icons::show(ui, "trash-2", 15.0, ink.muted);
        ui.label(RichText::new(t(lang, "trash")).strong());
    });
    if names.is_empty() {
        ui.label(RichText::new(t(lang, "trash_empty")).small().color(ink.muted));
    }
    for n in names {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&n).monospace());
            if crate::icons::button(ui, "archive-restore", t(lang, "restore")).clicked() {
                app.say(format!("app restore {n}"));
            }
        });
    }
}

/// Where everything comes from (and what is in quarantine), then the trash.
fn sources(app: &mut App, ui: &mut egui::Ui) {
    sources_list(app, ui);
    trash(app, ui);
}

fn sources_list(app: &mut App, ui: &mut egui::Ui) {
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

/// What the machine feels, as it is now and as it went: pleasure and displeasure, flourishing, progress; and what
/// moved it (all of it real quantities: its surprise against what it expected, its room, its learning).
pub fn feelings(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let m = app.picture.mood.clone();
    ui.horizontal(|ui| {
        crate::icons::show(ui, "heart-pulse", 15.0, ink.accent);
        ui.label(RichText::new(t(lang, "feel")).strong());
    });
    let gauge = |ui: &mut egui::Ui, sign: &str, name: &str, what: &str, v: f64, signed: bool, colour: Color32| {
        ui.horizontal(|ui| {
            crate::icons::show(ui, sign, 14.0, colour);
            ui.label(RichText::new(name).small()).on_hover_text(what);
            let (rect, _) = ui.allocate_exact_size(vec2(120.0, 8.0), Sense::hover());
            ui.painter().rect_filled(rect, 4.0, ink.faint.gamma_multiply(0.5));
            let (a, b) = if signed {
                let mid = rect.center().x;
                let x = mid + (v.clamp(-1.0, 1.0) as f32) * rect.width() / 2.0;
                (mid.min(x), mid.max(x))
            } else {
                (rect.left(), rect.left() + v.clamp(0.0, 1.0) as f32 * rect.width())
            };
            ui.painter().rect_filled(egui::Rect::from_x_y_ranges(a..=b.max(a + 1.0), rect.y_range()), 4.0, colour);
            ui.label(RichText::new(format!("{v:+.2}")).small().monospace().color(ink.muted));
        });
    };
    let pleasure = if m.dopamine >= 0.0 { ink.source(2) } else { ink.amber };
    gauge(ui, "smile", t(lang, "pleasure"), t(lang, "pleasure_d"), m.dopamine, true, pleasure);
    gauge(ui, "sprout", t(lang, "flourishing"), t(lang, "flourishing_d"), m.flourishing, false, ink.accent);
    gauge(ui, "trending-up", t(lang, "progress"), t(lang, "progress_d"), m.progress, true, ink.source(4));
    ui.label(
        RichText::new(format!("{} {:.2} · {} {:.2}", t(lang, "accuracy"), m.accuracy, t(lang, "room_left"), m.room))
            .small()
            .color(ink.muted),
    );
    // as it went
    if m.past.len() > 1 {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(320.0), 70.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, ink.surface);
        let n = m.past.len();
        for (k, colour, signed) in [(0, pleasure, true), (1, ink.accent, false), (2, ink.source(4), true)] {
            let pts: Vec<egui::Pos2> = m
                .past
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let x = [v.0, v.1, v.2][k];
                    let y = if signed { (x + 1.0) / 2.0 } else { x };
                    egui::pos2(
                        rect.left() + i as f32 / (n - 1) as f32 * rect.width(),
                        rect.bottom() - 4.0 - y.clamp(0.0, 1.0) as f32 * (rect.height() - 8.0),
                    )
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.4_f32, colour)));
        }
    }
    for (tick, amount, why) in m.moments.iter().rev().take(5) {
        ui.horizontal(|ui| {
            let (sign, colour) = if *amount >= 0.0 { ("smile", ink.source(2)) } else { ("frown", ink.amber) };
            crate::icons::show(ui, sign, 12.0, colour);
            ui.add(egui::Label::new(RichText::new(format!("[{tick}] {why}")).small().color(ink.muted)).truncate());
        });
    }
    ui.separator();
}

fn journal(app: &mut App, ui: &mut egui::Ui) {
    feelings(app, ui);
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
    // what it does, in four signs
    egui::Frame::new().fill(ink.raised).corner_radius(8.0).inner_margin(egui::Margin::symmetric(10, 8)).show(
        ui,
        |ui| {
            for (sign, key) in
                [("image", "eye_how1"), ("sparkles", "eye_how2"), ("scan-eye", "eye_how3"), ("network", "eye_how4")]
            {
                ui.horizontal(|ui| {
                    crate::icons::show(ui, sign, 15.0, ink.accent);
                    ui.label(RichText::new(t(lang, key)).small());
                });
            }
        },
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.eye_path)
                .margin(crate::style::FIELD)
                .hint_text(crate::style::hint(t(lang, "picture_path")))
                .desired_width(150.0),
        );
        if crate::icons::sign(ui, "folder-open", t(lang, "choose")).clicked() {
            app.pick(crate::app::Pick::Eye);
        }
        if ui.button(t(lang, "see")).clicked() && !app.eye_path.trim().is_empty() {
            let path = std::path::PathBuf::from(app.eye_path.trim().trim_matches('"'));
            app.see(&path);
            // seen (or being seen): the field is empty again
            app.eye_path.clear();
        }
    });
    // what is being seen: one after another
    if !app.seeing.is_empty() {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(16.0).color(ink.accent));
            ui.label(RichText::new(format!("{} ({})", t(lang, "eye_looking"), app.seeing.len())).color(ink.muted));
        });
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
    if app.eyes.is_empty() {
        ui.label(RichText::new(t(lang, "no_sight")).color(ink.muted));
        return;
    }
    let width = ui.available_width();
    let eyes: Vec<(String, crate::app::Eye)> = app.eyes.iter().map(|(n, e)| (n.clone(), e.clone())).collect();
    for (name, e) in &eyes {
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
        let mut open: Option<usize> = None;
        ui.horizontal(|ui| {
            for (k, (tag, tex)) in e.textures.iter().enumerate() {
                ui.vertical(|ui| {
                    let size = tex.size_vec2();
                    let scale = side / size.x.max(size.y).max(1.0);
                    // a click: bigger, and the others one after another
                    let r = ui
                        .add(
                            egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), size * scale))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text(t(lang, "eye_bigger"));
                    if r.clicked() {
                        open = Some(k);
                    }
                    ui.label(RichText::new(t(lang, tag)).small().color(ink.muted));
                });
            }
        });
        if let Some(k) = open {
            app.lightbox = Some((name.clone(), k));
        }
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
        // the nodes to choose from (those that play already are not offered)
        let playing: Vec<String> = views.iter().map(|v| v.name.clone()).collect();
        let shown = if app.play_field.is_empty() { t(lang, "play_node").to_string() } else { app.play_field.clone() };
        egui::ComboBox::from_id_salt("play-node").width(170.0).selected_text(shown).show_ui(ui, |ui| {
            for n in app.picture.nodes.iter().filter(|n| !n.name.contains("__next") && !playing.contains(&n.name)) {
                let k = app.colours.get(&n.source).copied().unwrap_or(0);
                let label = RichText::new(&n.name).color(ink.source(k));
                if ui.selectable_label(app.play_field == n.name, label).on_hover_text(&n.source).clicked() {
                    app.play_field = n.name.clone();
                }
            }
        });
        if crate::icons::button(ui, "play", t(lang, "play")).clicked() && !app.play_field.trim().is_empty() {
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
