// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! An app's surface: its widgets on a grid of 12 columns (the width is shared, whatever the window), each bound to
//! nodes of the network. Touching a widget measures its node; a widget that shows reads its node's value and spread.
//!
//! Edit mode (Ctrl+E, or the button): the app keeps running and sounding; widgets are moved and resized by
//! dragging (snapped to the grid), chosen to change their words, added, taken away. Every change is a new line of
//! the app's file, said at once and written at once.
//! X-ray (Ctrl+R): over each widget, the node it is bound to, its value ± sigma and where it comes from.

use std::collections::{HashMap, HashSet, VecDeque};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke};

use grm_shell::apps::{Widget, WidgetKind, COLUMNS};
use grm_shell::runtime::AppView;

use crate::app::{App, Ink};
use crate::text::t;
use crate::view::reading;

/// Rows of the grid are this high (points).
pub const ROW: f32 = 46.0;
const GAP: f32 = 6.0;
/// The keys of a computer keyboard that play notes (from the lowest), as on many music programs.
const PIANO: [(egui::Key, i32); 13] = [
    (egui::Key::A, 0),
    (egui::Key::W, 1),
    (egui::Key::S, 2),
    (egui::Key::E, 3),
    (egui::Key::D, 4),
    (egui::Key::F, 5),
    (egui::Key::T, 6),
    (egui::Key::G, 7),
    (egui::Key::Y, 8),
    (egui::Key::H, 9),
    (egui::Key::U, 10),
    (egui::Key::J, 11),
    (egui::Key::K, 12),
];

#[derive(Clone, Copy, PartialEq)]
enum Grab {
    Move,
    Resize,
}

/// A widget held in edit mode: its name, what is done to it, where it was, its size, where the pointer took it.
type Grabbed = (String, Grab, (i32, i32), (i32, i32), Pos2);

#[derive(Default)]
pub struct State {
    /// Each plot's recent values: (frame, value, sigma).
    pub plots: HashMap<String, VecDeque<(f64, f64)>>,
    seen_frame: u64,
    grab: Option<Grabbed>,
    /// Where a grabbed widget is being put (column, row, columns, rows).
    ghost: Option<(i32, i32, i32, i32)>,
    pub chosen: Option<String>,
    held: HashMap<String, bool>,
    keys_down: HashSet<i32>,
    pub add_kind: Option<WidgetKind>,
    pub add_nodes: [String; 2],
    pub add_name: String,
    pub edit_line: String,
}

fn freq(midi: f64) -> f64 {
    440.0 * 2f64.powf((midi - 69.0) / 12.0)
}

/// The text of the app with the widget's line said again (its names as the file writes them).
fn rewritten(view: &AppView, w: &Widget) -> String {
    let mut original = w.clone();
    for n in &mut original.nodes {
        if let Some((k, _)) = view.prefix.iter().find(|(_, v)| *v == n) {
            *n = k.clone();
        }
    }
    original.placed = true;
    grm_shell::apps::with_line(&view.text, w.line, Some(&original.statement()))
}

fn observe(app: &App, node: &str, v: f64) {
    let line = format!("observe {node} = {v} ± 0.000001");
    app.engine.send(move |w| w.say(&line, &grm_engine::Provenance::of("surface")).map(|_| String::new()));
}

pub fn show(app: &mut App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(view) = app.apps.iter().find(|a| a.name == name).cloned() else { return };
    // the plots hear each new picture of the world
    if app.picture_frame != app.surface.seen_frame {
        app.surface.seen_frame = app.picture_frame;
        for w in view.widgets.iter().filter(|w| w.kind == WidgetKind::Plot) {
            if let Some(n) = app.picture.node(&w.nodes[0]) {
                let buf = app.surface.plots.entry(format!("{name}/{}", w.name)).or_default();
                let sigma = if n.signed { n.sigma } else { n.value * (10f64.powf(n.sigma.min(3.0)) - 1.0) };
                // a spread wider than what the plot shows says nothing: it is not drawn
                let sigma = if sigma.is_finite() && sigma < (w.hi - w.lo) { sigma } else { 0.0 };
                buf.push_back((n.value, sigma));
                while buf.len() > 240 {
                    buf.pop_front();
                }
            }
        }
    }
    let editing = app.editing;
    let area = ui.available_rect_before_wrap();
    let col = (area.width() - GAP) / COLUMNS as f32;
    let rows = view.widgets.iter().map(|w| w.at.1 + w.size.1).max().unwrap_or(0).max(1);
    let height = (rows as f32 * ROW + GAP).max(area.height());
    let (whole, _) = ui.allocate_exact_size(vec2(area.width(), height), Sense::hover());
    let painter = ui.painter_at(whole);
    // its looks: a small network of their own (see looks.rs)
    let (app_look, widget_looks) = {
        let looks = app.looks_of(&view);
        let each: std::collections::HashMap<String, crate::looks::Look> =
            view.widgets.iter().map(|w| (w.name.clone(), looks.get(w.look.as_deref()))).collect();
        (looks.get(None), each)
    };
    painter.rect_filled(whole, 0.0, app_look.background);
    let cell = |c: i32, r: i32, w: i32, h: i32| {
        Rect::from_min_size(
            pos2(whole.left() + GAP + c as f32 * col, whole.top() + GAP + r as f32 * ROW),
            vec2(w as f32 * col - GAP, h as f32 * ROW - GAP),
        )
    };
    if editing {
        // the grid, quiet
        for c in 0..=COLUMNS {
            let x = whole.left() + GAP / 2.0 + c as f32 * col;
            painter.line_segment([pos2(x, whole.top()), pos2(x, whole.bottom())], Stroke::new(0.5_f32, ink.faint));
        }
        let mut y = whole.top() + GAP / 2.0;
        while y < whole.bottom() {
            painter.line_segment([pos2(whole.left(), y), pos2(whole.right(), y)], Stroke::new(0.5_f32, ink.faint));
            y += ROW;
        }
    }
    if view.widgets.is_empty() {
        painter.text(
            whole.center_top() + vec2(0.0, 40.0),
            egui::Align2::CENTER_CENTER,
            t(lang, "empty_surface"),
            egui::FontId::proportional(14.0),
            ink.muted,
        );
    }
    let mut new_text: Option<String> = None;
    let outer_ink = ink;
    for w in &view.widgets {
        let rect = cell(w.at.0, w.at.1, w.size.0.min(COLUMNS), w.size.1);
        // the widget drawn in its look
        let lk = widget_looks.get(&w.name).copied().unwrap_or(app_look);
        let mut ink = outer_ink;
        ink.accent = lk.accent;
        ink.text = lk.text;
        let saved_style = ui.style().clone();
        {
            let st = ui.style_mut();
            let r = eframe::egui::CornerRadius::same(lk.round.round() as u8);
            st.visuals.selection.bg_fill = lk.accent;
            let l = 0.299 * lk.accent.r() as f32 + 0.587 * lk.accent.g() as f32 + 0.114 * lk.accent.b() as f32;
            let on = if l > 140.0 {
                eframe::egui::Color32::from_rgb(0x14, 0x14, 0x16)
            } else {
                eframe::egui::Color32::WHITE
            };
            st.visuals.selection.stroke = Stroke::new(1.0_f32, on);
            st.visuals.override_text_color = Some(lk.text);
            for w in [
                &mut st.visuals.widgets.inactive,
                &mut st.visuals.widgets.hovered,
                &mut st.visuals.widgets.active,
                &mut st.visuals.widgets.open,
            ] {
                w.corner_radius = r;
            }
            st.visuals.widgets.active.bg_fill = lk.accent;
            st.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, lk.accent);
            for f in st.text_styles.values_mut() {
                f.size *= lk.size;
            }
        }
        let id = egui::Id::new(("widget", name, &w.name));
        let node = |k: usize| w.nodes.get(k).and_then(|n| app.picture.node(n)).cloned();
        if editing {
            let resp = ui.interact(rect, id.with("edit"), Sense::click_and_drag());
            let chosen = app.surface.chosen.as_deref() == Some(w.name.as_str());
            painter.rect_filled(rect, 4.0, ink.faint.gamma_multiply(0.25));
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(if chosen { 2.0_f32 } else { 1.0_f32 }, if chosen { ink.accent } else { ink.muted }),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.left_top() + vec2(6.0, 4.0),
                egui::Align2::LEFT_TOP,
                format!("{} · {} {}", w.name, w.kind.word(), w.nodes.join(" ")),
                egui::FontId::proportional(12.0),
                ink.text,
            );
            let handle = Rect::from_min_size(rect.right_bottom() - vec2(14.0, 14.0), vec2(14.0, 14.0));
            painter.rect_filled(handle, 2.0, ink.accent);
            if resp.clicked() {
                app.surface.chosen = Some(w.name.clone());
                app.surface.edit_line = view.text.lines().nth(w.line - 1).unwrap_or("").to_string();
            }
            if resp.drag_started() {
                let p = resp.interact_pointer_pos().unwrap_or(rect.center());
                let grab = if handle.expand(4.0).contains(p) { Grab::Resize } else { Grab::Move };
                app.surface.grab = Some((w.name.clone(), grab, w.at, w.size, p));
                app.surface.chosen = Some(w.name.clone());
                app.surface.edit_line = view.text.lines().nth(w.line - 1).unwrap_or("").to_string();
            }
            if let (Some((g, grab, at, size, from)), Some(p)) = (&app.surface.grab, resp.interact_pointer_pos()) {
                if g == &w.name {
                    let dc = ((p.x - from.x) / col).round() as i32;
                    let dr = ((p.y - from.y) / ROW).round() as i32;
                    let ghost = match grab {
                        Grab::Move => {
                            ((at.0 + dc).clamp(0, COLUMNS - size.0.min(COLUMNS)), (at.1 + dr).max(0), size.0, size.1)
                        }
                        Grab::Resize => (at.0, at.1, (size.0 + dc).clamp(1, COLUMNS - at.0), (size.1 + dr).max(1)),
                    };
                    app.surface.ghost = Some(ghost);
                    // a widget does not go over another: where it would, the ghost is amber and it stays put
                    let free = !view.widgets.iter().any(|o| {
                        o.name != w.name
                            && ghost.0 < o.at.0 + o.size.0
                            && o.at.0 < ghost.0 + ghost.2
                            && ghost.1 < o.at.1 + o.size.1
                            && o.at.1 < ghost.1 + ghost.3
                    });
                    let g = cell(ghost.0, ghost.1, ghost.2, ghost.3);
                    painter.rect_stroke(
                        g,
                        4.0,
                        Stroke::new(2.0_f32, if free { ink.accent } else { ink.amber }),
                        egui::StrokeKind::Inside,
                    );
                    if resp.drag_stopped() && !free {
                        app.surface.grab = None;
                        app.surface.ghost = None;
                    } else if resp.drag_stopped() {
                        let mut moved = w.clone();
                        moved.at = (ghost.0, ghost.1);
                        moved.size = (ghost.2, ghost.3);
                        new_text = Some(rewritten(&view, &moved));
                        app.surface.grab = None;
                        app.surface.ghost = None;
                    }
                }
            }
            resp.on_hover_cursor(egui::CursorIcon::Grab);
            ui.set_style(saved_style.clone());
            continue;
        }
        // the widget at work
        let label = if w.text.is_empty() { w.name.clone() } else { w.text.clone() };
        match w.kind {
            WidgetKind::Slider => {
                if let Some(n) = node(0) {
                    let mut v = n.value;
                    let mut inner = rect;
                    inner.set_height(inner.height().min(28.0));
                    inner = inner.translate(vec2(0.0, (rect.height() - inner.height()) / 2.0));
                    let r = ui.put(
                        inner,
                        egui::Slider::new(&mut v, w.lo..=w.hi).text(&label).clamping(egui::SliderClamping::Always),
                    );
                    if r.changed() {
                        observe(app, &n.name, v);
                    }
                }
            }
            WidgetKind::Toggle => {
                if let Some(n) = node(0) {
                    let mut on = n.value > 0.5;
                    if ui.put(rect, egui::Checkbox::new(&mut on, &label)).changed() {
                        observe(app, &n.name, if on { 1.0 } else { 0.0 });
                    }
                }
            }
            WidgetKind::Button => {
                if let Some(n) = node(0) {
                    let r = ui.put(rect, egui::Button::new(RichText::new(&label).size(15.0 * lk.size)));
                    let down = r.is_pointer_button_down_on();
                    let key = format!("{name}/{}", w.name);
                    let was = app.surface.held.get(&key).copied().unwrap_or(false);
                    if down != was {
                        observe(app, &n.name, if down { 1.0 } else { 0.0 });
                        app.surface.held.insert(key, down);
                    }
                }
            }
            WidgetKind::Play => {
                let playing = app.mixer.lock().map(|m| m.playing().contains(&w.nodes[0])).unwrap_or(false);
                let text = if playing { format!("■ {label}") } else { format!("▶ {label}") };
                if ui.put(rect, egui::Button::new(RichText::new(text).size(15.0 * lk.size)).selected(playing)).clicked()
                {
                    app.say(if playing { format!("stop {}", w.nodes[0]) } else { format!("play {}", w.nodes[0]) });
                }
            }
            WidgetKind::Meter => {
                if let Some(n) = node(0) {
                    let r = ui.interact(rect, id, Sense::hover());
                    let bar = Rect::from_min_size(rect.left_center() + vec2(0.0, 2.0), vec2(rect.width(), 10.0));
                    painter.rect_filled(bar, 5.0, ink.faint);
                    let k = ((n.value - w.lo) / (w.hi - w.lo)).clamp(0.0, 1.0) as f32;
                    let mut full = bar;
                    full.set_width(bar.width() * k);
                    painter.rect_filled(full, 5.0, ink.accent);
                    painter.text(
                        rect.left_top(),
                        egui::Align2::LEFT_TOP,
                        &label,
                        egui::FontId::proportional(12.0),
                        ink.muted,
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::ProgressIndicator,
                            true,
                            format!("{label}: {}", reading(n.value, n.sigma, n.signed)),
                        )
                    });
                }
            }
            WidgetKind::Value => {
                if let Some(n) = node(0) {
                    let r = ui.interact(rect, id, Sense::hover());
                    painter.text(
                        rect.left_top(),
                        egui::Align2::LEFT_TOP,
                        &label,
                        egui::FontId::proportional(12.0),
                        ink.muted,
                    );
                    painter.text(
                        rect.left_bottom(),
                        egui::Align2::LEFT_BOTTOM,
                        reading(n.value, n.sigma, n.signed),
                        egui::FontId::monospace(16.0),
                        ink.text,
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Label,
                            true,
                            format!("{label}: {}", reading(n.value, n.sigma, n.signed)),
                        )
                    });
                }
            }
            WidgetKind::Plot => {
                let r = ui.interact(rect, id, Sense::hover());
                painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ink.faint), egui::StrokeKind::Inside);
                painter.text(
                    rect.left_top() + vec2(6.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    egui::FontId::proportional(12.0),
                    ink.muted,
                );
                let inner = rect.shrink2(vec2(6.0, 18.0)).translate(vec2(0.0, 6.0));
                if let Some(buf) = app.surface.plots.get(&format!("{name}/{}", w.name)) {
                    let n = buf.len().max(2) as f32 - 1.0;
                    let y = |v: f64| {
                        inner.bottom() - (((v - w.lo) / (w.hi - w.lo)) as f32).clamp(-0.05, 1.05) * inner.height()
                    };
                    let x = |k: usize| inner.left() + k as f32 / n * inner.width();
                    for (k, (v, s)) in buf.iter().enumerate() {
                        if s.is_finite() && *s > 0.0 {
                            painter.line_segment(
                                [pos2(x(k), y(v + s)), pos2(x(k), y(v - s))],
                                Stroke::new(2.0_f32, ink.accent.gamma_multiply(0.25)),
                            );
                        }
                    }
                    let line: Vec<Pos2> = buf.iter().enumerate().map(|(k, (v, _))| pos2(x(k), y(*v))).collect();
                    if line.len() > 1 {
                        painter.add(egui::Shape::line(line, Stroke::new(2.0_f32, ink.accent)));
                    }
                    if let Some((v, _)) = buf.back() {
                        painter.text(
                            rect.right_top() + vec2(-6.0, 4.0),
                            egui::Align2::RIGHT_TOP,
                            grm_engine::lang::g(*v),
                            egui::FontId::monospace(12.0),
                            ink.text,
                        );
                    }
                }
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("plot of {}", w.nodes[0]))
                });
            }
            WidgetKind::Text => {
                ui.put(rect, egui::Label::new(RichText::new(&w.text).size(15.0 * lk.size)).wrap());
            }
            WidgetKind::Picture => {
                let eye = &w.nodes[0];
                match app.eyes.get(eye).and_then(|e| e.textures.first()) {
                    Some((_, tex)) => {
                        let size = tex.size_vec2();
                        let k = (rect.width() / size.x).min(rect.height() / size.y);
                        let r = Rect::from_center_size(rect.center(), size * k);
                        painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ink.faint), egui::StrokeKind::Inside);
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{eye}: —"),
                            egui::FontId::proportional(12.0),
                            ink.muted,
                        );
                    }
                }
            }
            WidgetKind::Pad => {
                if let (Some(nx), Some(ny)) = (node(0), node(1)) {
                    let side = rect.width().min(rect.height());
                    let sq = Rect::from_min_size(rect.min, vec2(side, side));
                    let r = ui.interact(sq, id, Sense::click_and_drag());
                    painter.rect_filled(sq, 4.0, ink.faint.gamma_multiply(0.4));
                    painter.rect_stroke(sq, 4.0, Stroke::new(1.0_f32, ink.muted), egui::StrokeKind::Inside);
                    let kx = ((nx.value - w.lo) / (w.hi - w.lo)).clamp(0.0, 1.0) as f32;
                    let ky = ((ny.value - w.lo) / (w.hi - w.lo)).clamp(0.0, 1.0) as f32;
                    painter.circle_filled(pos2(sq.left() + kx * side, sq.bottom() - ky * side), 7.0, ink.accent);
                    if r.dragged() || r.clicked() {
                        if let Some(p) = r.interact_pointer_pos() {
                            let x = w.lo + ((p.x - sq.left()) / side).clamp(0.0, 1.0) as f64 * (w.hi - w.lo);
                            let y = w.lo + ((sq.bottom() - p.y) / side).clamp(0.0, 1.0) as f64 * (w.hi - w.lo);
                            observe(app, &nx.name, x);
                            observe(app, &ny.name, y);
                        }
                    }
                    painter.text(
                        sq.left_top() + vec2(4.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        &label,
                        egui::FontId::proportional(11.0),
                        ink.muted,
                    );
                }
            }
            WidgetKind::Keys => keys(app, ui, &painter, rect, id, w, &ink),
            WidgetKind::Scope => {
                let r = ui.interact(rect, id, Sense::hover());
                painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ink.faint), egui::StrokeKind::Inside);
                painter.text(
                    rect.left_top() + vec2(6.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    egui::FontId::proportional(12.0),
                    ink.muted,
                );
                let inner = rect.shrink2(vec2(6.0, 18.0)).translate(vec2(0.0, 6.0));
                let mid = inner.center().y;
                painter
                    .line_segment([pos2(inner.left(), mid), pos2(inner.right(), mid)], Stroke::new(0.5_f32, ink.faint));
                match app.mixer.lock().ok().and_then(|m| m.scope(&w.nodes[0])) {
                    Some(samples) => {
                        // from the last rising zero crossing: the wave stands still when it repeats
                        let n = samples.len();
                        let show = (n / 2).min(1024);
                        let mut start = n - show;
                        for k in (1..n - show).rev() {
                            if samples[k - 1] < 0.0 && samples[k] >= 0.0 {
                                start = k;
                                break;
                            }
                        }
                        let span = (w.hi - w.lo) as f32;
                        let line: Vec<Pos2> = (0..show)
                            .map(|k| {
                                let v = samples[(start + k).min(n - 1)];
                                pos2(
                                    inner.left() + k as f32 / show as f32 * inner.width(),
                                    inner.bottom() - ((v - w.lo as f32) / span).clamp(-0.05, 1.05) * inner.height(),
                                )
                            })
                            .collect();
                        painter.add(egui::Shape::line(line, Stroke::new(1.5_f32, ink.accent)));
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                    }
                    None => {
                        painter.text(
                            inner.center(),
                            egui::Align2::CENTER_CENTER,
                            t(lang, "not_playing"),
                            egui::FontId::proportional(12.0),
                            ink.muted,
                        );
                    }
                }
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("scope of {}", w.nodes[0]))
                });
            }
        }
        ui.set_style(saved_style);
        if app.xray {
            let words: Vec<String> = w
                .nodes
                .iter()
                .map(|n| match app.picture.node(n) {
                    Some(v) => format!("→ {n} = {} · {}", reading(v.value, v.sigma, v.signed), v.source),
                    None => format!("→ {n} (not in the network)"),
                })
                .collect();
            let text = format!("{} · line {}\n{}", w.name, w.line, words.join("\n"));
            let galley = painter.layout(text, egui::FontId::monospace(11.0), ink.text, rect.width().max(160.0));
            let back = Rect::from_min_size(rect.left_top(), galley.size() + vec2(8.0, 6.0));
            painter.rect_filled(back, 3.0, ink.surface.gamma_multiply(0.92));
            painter.rect_stroke(back, 3.0, Stroke::new(1.0_f32, ink.amber), egui::StrokeKind::Inside);
            painter.galley(back.min + vec2(4.0, 3.0), galley, ink.text);
        }
    }
    if let Some(text) = new_text {
        app.edit_app_text(name, text);
    }
}

fn keys(app: &mut App, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect, id: egui::Id, w: &Widget, ink: &Ink) {
    let (lo, hi) = (w.lo.round() as i32, w.hi.round() as i32);
    let white: Vec<i32> = (lo..=hi).filter(|m| ![1, 3, 6, 8, 10].contains(&m.rem_euclid(12))).collect();
    let kw = rect.width() / white.len().max(1) as f32;
    let r = ui.interact(rect, id, Sense::click_and_drag());
    let mut boxes: Vec<(i32, Rect, bool)> = vec![];
    for (k, m) in white.iter().enumerate() {
        boxes.push((
            *m,
            Rect::from_min_size(pos2(rect.left() + k as f32 * kw, rect.top()), vec2(kw - 1.0, rect.height())),
            false,
        ));
    }
    for m in lo..=hi {
        if [1, 3, 6, 8, 10].contains(&m.rem_euclid(12)) {
            let left = white.iter().position(|x| *x == m - 1).unwrap_or(0) as f32;
            boxes.push((
                m,
                Rect::from_min_size(
                    pos2(rect.left() + (left + 0.7) * kw, rect.top()),
                    vec2(kw * 0.6, rect.height() * 0.6),
                ),
                true,
            ));
        }
    }
    // which note the pointer holds (black keys are on top), and the computer keyboard's
    let pointer = if r.is_pointer_button_down_on() { r.interact_pointer_pos() } else { None };
    let mut held: Option<i32> =
        pointer.and_then(|p| boxes.iter().rev().find(|(_, b, _)| b.contains(p)).map(|(m, _, _)| *m));
    // the computer's keys play only alone (Ctrl+E is not an E), and not while something is typed
    let typing = ui.ctx().memory(|m| m.focused().is_some()) || ui.input(|i| i.modifiers.any());
    if !typing && !app.editing {
        ui.input(|i| {
            for (key, k) in PIANO {
                let m = lo + k;
                if i.key_down(key) && m <= hi {
                    held = Some(m);
                }
            }
        });
    }
    for (m, b, black) in &boxes {
        let on = held == Some(*m);
        // a keyboard looks like one, whatever the theme
        let fill = if on {
            ink.accent
        } else if *black {
            Color32::from_rgb(0x26, 0x27, 0x2B)
        } else {
            Color32::from_rgb(0xEC, 0xEC, 0xE8)
        };
        painter.rect_filled(*b, 2.0, fill);
        painter.rect_stroke(*b, 2.0, Stroke::new(1.0_f32, ink.muted), egui::StrokeKind::Inside);
    }
    let key = format!("keys/{}", w.name);
    let was = app.surface.keys_down.iter().next().copied();
    let now_held = held;
    if now_held != was {
        app.surface.keys_down.clear();
        if let Some(m) = now_held {
            app.surface.keys_down.insert(m);
            observe(app, &w.nodes[0], freq(m as f64));
            observe(app, &w.nodes[1], 1.0);
        } else {
            observe(app, &w.nodes[1], 0.0);
        }
        ui.ctx().request_repaint();
    }
    let _ = key;
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("keys: {} and {}", w.nodes[0], w.nodes[1]))
    });
    if held.is_some() {
        ui.ctx().request_repaint();
    }
}

/// Edit mode's own panel: the widget chosen (its line, to change), adding one, taking one away.
pub fn edit_panel(app: &mut App, ui: &mut egui::Ui, name: &str) {
    let lang = app.settings.lang;
    let Some(view) = app.apps.iter().find(|a| a.name == name).cloned() else { return };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(t(lang, "add_widget")).strong());
        egui::ComboBox::from_id_salt("add-kind")
            .selected_text(app.surface.add_kind.map(|k| k.word()).unwrap_or("…"))
            .show_ui(ui, |ui| {
                for k in WidgetKind::ALL {
                    ui.selectable_value(&mut app.surface.add_kind, Some(k), k.word());
                }
            });
        let need = app.surface.add_kind.map(|k| k.nodes()).unwrap_or(1);
        let nodes: Vec<String> = app.picture.nodes.iter().map(|n| n.name.clone()).collect();
        for k in 0..need {
            egui::ComboBox::from_id_salt(("add-node", k))
                .selected_text(if app.surface.add_nodes[k].is_empty() {
                    "node…"
                } else {
                    app.surface.add_nodes[k].as_str()
                })
                .show_ui(ui, |ui| {
                    for n in &nodes {
                        ui.selectable_value(&mut app.surface.add_nodes[k], n.clone(), n);
                    }
                });
        }
        ui.add(
            egui::TextEdit::singleline(&mut app.surface.add_name).hint_text(t(lang, "widget_name")).desired_width(90.0),
        );
        if ui.button(t(lang, "add")).clicked() {
            if let Some(kind) = app.surface.add_kind {
                let mut wname = app.surface.add_name.trim().to_string();
                if !grm_engine::lang::is_name(&wname) {
                    wname = format!("{}{}", kind.word(), view.widgets.len() + 1);
                }
                let mut line = format!("widget {wname}: {}", kind.word());
                for k in 0..kind.nodes() {
                    let n = &app.surface.add_nodes[k];
                    let original =
                        view.prefix.iter().find(|(_, v)| *v == n).map(|(k, _)| k.clone()).unwrap_or(n.clone());
                    line.push(' ');
                    line.push_str(&original);
                }
                if kind == WidgetKind::Text {
                    line.push_str(" \"…\"");
                }
                if matches!(kind, WidgetKind::Slider | WidgetKind::Meter | WidgetKind::Plot | WidgetKind::Pad) {
                    if let Some(n) = app.picture.node(&app.surface.add_nodes[0]) {
                        let span = n.value.abs().max(1.0);
                        line.push_str(&format!(
                            " {} .. {}",
                            grm_engine::lang::g(if n.signed { -span * 2.0 } else { 0.0 }),
                            grm_engine::lang::g(span * 2.0)
                        ));
                    }
                }
                let text = grm_shell::apps::with_added(&view.text, &line);
                app.edit_app_text(name, text);
                app.surface.add_name.clear();
            }
        }
    });
    // the line of the widget chosen: always there (the surface below does not move when one is chosen)
    let chosen = app.surface.chosen.clone().and_then(|c| view.widgets.iter().find(|w| w.name == c).cloned());
    if chosen.is_none() {
        ui.horizontal(|ui| {
            ui.add_enabled(
                false,
                egui::TextEdit::singleline(&mut String::new())
                    .hint_text(t(lang, "choose_widget"))
                    .desired_width(ui.available_width() - 160.0),
            );
            ui.add_enabled(false, egui::Button::new(t(lang, "apply")));
            ui.add_enabled(false, egui::Button::new(t(lang, "remove_widget")));
        });
    }
    if let Some(chosen) = chosen.map(|w| w.name) {
        if let Some(w) = view.widgets.iter().find(|w| w.name == chosen) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} (line {})", w.name, w.line)).strong());
                let r = ui.add(
                    egui::TextEdit::singleline(&mut app.surface.edit_line)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width() - 160.0),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button(t(lang, "apply")).clicked() || enter {
                    let text = grm_shell::apps::with_line(&view.text, w.line, Some(app.surface.edit_line.trim()));
                    app.edit_app_text(name, text);
                }
                if ui.button(t(lang, "remove_widget")).clicked() {
                    let text = grm_shell::apps::with_line(&view.text, w.line, None);
                    app.edit_app_text(name, text);
                    app.surface.chosen = None;
                }
            });
        }
    }
}
