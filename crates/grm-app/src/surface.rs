// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! An app's surface: its widgets on a grid of 12 columns (the width is shared, whatever the window), each bound to
//! nodes of the network. Touching a widget measures its node; a widget that shows reads its node's value and spread.
//!
//! Edit mode (Ctrl+E, or the button): the app keeps running and sounding; widgets are moved and resized by
//! dragging (snapped to the grid), chosen to change their words, added, taken away. Every change is a new line of
//! the app's file, said at once and written at once.
//! X-ray (Ctrl+R): over each widget, the node it is bound to, its value ± sigma and where it comes from.

use std::collections::{HashMap, VecDeque};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke};

use grm_shell::apps::{Widget, WidgetKind, COLUMNS};
use grm_shell::runtime::AppView;

use crate::app::{App, Ink};
use crate::text::t;

/// Her colour: her notes, on the keys and on the roll (a soft lavender, calm in both themes).
pub const HERS: Color32 = Color32::from_rgb(0xB4, 0xA7, 0xE6);
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
    /// The keys held on each keyboard (by its pitch node): each its own, so that one keyboard let alone does not
    /// let go of what is held on another (her synthesiser's, drawn after the human's, let go of every key at once).
    keys_down: HashMap<String, std::collections::BTreeSet<i32>>,
    /// The keys held on a polyphonic instrument (MIDI numbers, the octave counted), and its prefix: the window
    /// gives each a voice.
    pub keys_held: std::collections::BTreeSet<i32>,
    /// The knob being turned and where the hand has it now (the machine's answer comes later: the hand does not
    /// wait for it, or most of its gesture would be lost).
    pub turning: Option<(String, f64)>,
    pub poly: Option<String>,
    pub add_kind: Option<WidgetKind>,
    pub add_nodes: [String; 2],
    pub add_name: String,
    pub edit_line: String,
    /// The notes the human played on the keys (MIDI numbers), for her to hear; the knobs they turned (node, from,
    /// to: parts of a whole turn), for her to learn from.
    pub played: Vec<i32>,
    pub turned: Vec<(String, f64, f64)>,
    /// The keys' octave, moved up or down.
    pub octave: i32,
    /// The computer's piano keys held, by where they are (semitones from the keyboard's lowest note).
    pub phys_down: std::collections::BTreeSet<i32>,
    /// Each set of tabs' chosen one, and how far each scroll is down (by app and widget).
    pub tabs: HashMap<String, usize>,
    pub scrolls: HashMap<String, f32>,
    /// What each meter of a sound shows (it falls slowly), by app and widget.
    pub meters: HashMap<String, f64>,
    /// What is being written in each field (by app and widget), until Enter gives it to its node.
    pub fields: HashMap<String, String>,
    /// How each 3D view looks at its world (by app and widget).
    pub cameras: HashMap<String, crate::view3d::Camera>,
}

/// What a widget that holds others draws itself (what it holds is drawn as any widget is).
enum Frame {
    Card { rect: Rect, title: String, icon: Option<String> },
    Tabs { rect: Rect, key: String, titles: Vec<String>, chosen: usize },
    Scroll { rect: Rect, key: String, offset: f32, content: f32 },
}

/// Where each widget is drawn: those on the surface in their cells, those held where what holds them puts them
/// (side by side, one under another, in a grid, one over another, scrolled, one tab at a time, in a card). A
/// widget `when NODE` while the node is 0 is not drawn and takes no room.
#[allow(clippy::too_many_arguments)]
fn lay<'a>(
    app: &App,
    key: &str,
    w: &'a Widget,
    rect: Rect,
    out: &mut Vec<(&'a Widget, Rect, bool)>,
    frames: &mut Vec<Frame>,
    top: bool,
) {
    // what holds others, held itself, keeps a little room from its neighbours
    let rect = if !top && w.kind.holds() { rect.shrink(GAP / 2.0) } else { rect };
    out.push((w, rect, top));
    if !w.kind.holds() {
        return;
    }
    let shown: Vec<&Widget> = w.children.iter().filter(|c| shown(app, c)).collect();
    let tall = |c: &Widget| c.size.1.max(1) as f32 * ROW;
    let inner = rect.shrink(GAP / 2.0);
    let me = format!("{key}/{}", w.name);
    match w.kind {
        WidgetKind::Row => {
            let whole: i32 = shown.iter().map(|c| c.size.0.max(1)).sum::<i32>().max(1);
            let mut x = inner.left();
            for c in shown {
                let wd = inner.width() * c.size.0.max(1) as f32 / whole as f32;
                let h = if c.kind.holds() { inner.height() } else { inner.height().min(tall(c)) };
                lay(app, key, c, Rect::from_min_size(pos2(x, inner.top()), vec2(wd, h)), out, frames, false);
                x += wd;
            }
        }
        WidgetKind::Column | WidgetKind::Card => {
            let mut y = inner.top();
            if w.kind == WidgetKind::Card {
                frames.push(Frame::Card { rect, title: w.text.clone(), icon: w.icon.clone() });
                if !w.text.is_empty() {
                    y += ROW * 0.6;
                }
            }
            for c in shown {
                let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), tall(c)));
                lay(app, key, c, r, out, frames, false);
                y += tall(c);
            }
        }
        WidgetKind::Grid => {
            let n = w.columns.unwrap_or(2).max(1) as usize;
            let cw = inner.width() / n as f32;
            let mut y = inner.top();
            for row in shown.chunks(n) {
                let h = row.iter().map(|c| tall(c)).fold(ROW, f32::max);
                for (k, c) in row.iter().enumerate() {
                    let r = Rect::from_min_size(pos2(inner.left() + k as f32 * cw, y), vec2(cw, h));
                    lay(app, key, c, r, out, frames, false);
                }
                y += h;
            }
        }
        WidgetKind::Stack => {
            for c in shown {
                lay(app, key, c, inner, out, frames, false);
            }
        }
        WidgetKind::Tabs => {
            let chosen = app.surface.tabs.get(&me).copied().unwrap_or(0).min(shown.len().saturating_sub(1));
            let titles =
                shown.iter().map(|c| if c.text.is_empty() { c.name.clone() } else { c.text.clone() }).collect();
            frames.push(Frame::Tabs { rect, key: me, titles, chosen });
            if let Some(c) = shown.get(chosen) {
                let r = Rect::from_min_max(pos2(inner.left(), inner.top() + ROW * 0.7), inner.max);
                lay(app, key, c, r, out, frames, false);
            }
        }
        WidgetKind::Scroll => {
            // a gutter of its own for the bar: what scrolls is never under it
            let room = Rect::from_min_max(inner.min, pos2(inner.right() - 10.0, inner.bottom()));
            let content: f32 = shown.iter().map(|c| tall(c)).sum();
            let offset =
                app.surface.scrolls.get(&me).copied().unwrap_or(0.0).clamp(0.0, (content - room.height()).max(0.0));
            frames.push(Frame::Scroll { rect: inner, key: me, offset, content });
            let mut y = room.top() - offset;
            for c in shown {
                let r = Rect::from_min_size(pos2(room.left(), y), vec2(room.width(), tall(c)));
                // only what is wholly in view is drawn (nothing cut)
                if r.top() >= room.top() - 0.5 && r.bottom() <= room.bottom() + 0.5 {
                    lay(app, key, c, r, out, frames, false);
                }
                y += tall(c);
            }
        }
        _ => {}
    }
}

/// Whether a widget's key (`key space`) was pressed now: taken, so that nothing else takes it too; never while words
/// are being typed.
fn pressed_by(ui: &egui::Ui, w: &Widget) -> bool {
    let Some(k) = w.key.as_deref() else { return false };
    let key = match k {
        "space" => Some(egui::Key::Space),
        "enter" => Some(egui::Key::Enter),
        "escape" => Some(egui::Key::Escape),
        "tab" => Some(egui::Key::Tab),
        other => egui::Key::from_name(&other.to_uppercase()),
    };
    let Some(key) = key else { return false };
    !ui.ctx().wants_keyboard_input() && ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

/// What a node is now: as the sound has it when the sound computes it (a transport's bar), else as the world has it.
fn now_of(app: &App, n: &crate::model::NodeView) -> f64 {
    app.mixer.lock().ok().and_then(|m| m.sounding(&n.name)).unwrap_or(n.value)
}

/// Whether a widget is shown (`when NODE`: while the node is not 0).
fn shown(app: &App, w: &Widget) -> bool {
    w.when.as_ref().is_none_or(|n| app.picture.node(n).is_some_and(|v| v.value.abs() > 1e-9))
}

/// What holds widgets: its card, its tabs (chosen by a click), its scroll bar (and the wheel over it).
fn frames(app: &mut App, ui: &mut egui::Ui, painter: &egui::Painter, frames: Vec<Frame>, ink: &Ink) {
    for f in frames {
        match f {
            Frame::Card { rect, title, icon } => {
                painter.rect_filled(rect, crate::style::corner(), ink.raised.gamma_multiply(0.35));
                painter.rect_stroke(
                    rect,
                    crate::style::corner(),
                    Stroke::new(1.0_f32, ink.faint),
                    egui::StrokeKind::Inside,
                );
                if !title.is_empty() {
                    caption(ui, painter, pos2(rect.center().x, rect.top() + ROW * 0.45), &title, icon.as_deref(), ink);
                }
            }
            Frame::Tabs { rect, key, titles, chosen } => {
                let mut x = rect.left() + GAP;
                for (k, title) in titles.iter().enumerate() {
                    let font = crate::style::font(crate::style::Size::Text);
                    let galley = painter.layout_no_wrap(title.clone(), font, ink.text);
                    let r = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(galley.size().x + 20.0, ROW * 0.55));
                    let resp = ui.interact(r, egui::Id::new(("tab", &key, k)), Sense::click());
                    let on = k == chosen;
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, title));
                    if on || resp.hovered() {
                        let fill = if on { ink.chosen } else { ink.faint.gamma_multiply(0.5) };
                        painter.rect_filled(r, crate::style::corner(), fill);
                    }
                    painter.galley(
                        r.center() - galley.size() / 2.0,
                        galley,
                        if on { ink.on_chosen() } else { ink.text },
                    );
                    if resp.clicked() {
                        app.surface.tabs.insert(key.clone(), k);
                    }
                    x = r.right() + 4.0;
                }
            }
            Frame::Scroll { rect, key, offset, content } => {
                let room = rect.height();
                if content > room {
                    let bar = Rect::from_min_max(pos2(rect.right() - 6.0, rect.top()), rect.right_bottom());
                    painter.rect_filled(bar, crate::style::corner(), ink.faint.gamma_multiply(0.4));
                    let h = (room / content * room).max(16.0);
                    let y = rect.top() + offset / (content - room) * (room - h);
                    let thumb = Rect::from_min_size(pos2(bar.left(), y), vec2(bar.width(), h));
                    painter.rect_filled(thumb, crate::style::corner(), ink.muted);
                    if ui.rect_contains_pointer(rect) {
                        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
                        if wheel != 0.0 {
                            app.surface.scrolls.insert(key, (offset - wheel).clamp(0.0, content - room));
                        }
                    }
                }
            }
        }
    }
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

/// A pitch's name as a musician says it (A4 = 440 Hz), the nearest note.
fn note_name(hz: f64) -> String {
    if hz.is_nan() || hz <= 0.0 {
        return "·".into();
    }
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let n = (69.0 + 12.0 * (hz / 440.0).log2()).round() as i64;
    format!("{}{}", NAMES[n.rem_euclid(12) as usize], n.div_euclid(12) - 1)
}

/// A widget's words under it, in capitals, its sign before them when it has one.
fn caption(ui: &egui::Ui, painter: &egui::Painter, bottom: Pos2, label: &str, icon: Option<&str>, ink: &Ink) {
    // it fits its place: smaller words, then no sign, rather than cut
    let room = painter.clip_rect().width();
    let mut g = painter.layout_no_wrap(label.to_uppercase(), crate::style::font(crate::style::Size::Small), ink.text);
    if g.size().x + 15.0 > room {
        g = painter.layout_no_wrap(label.to_uppercase(), crate::style::font(crate::style::Size::Tiny), ink.text);
    }
    let icon = icon.filter(|_| g.size().x + 15.0 <= room);
    let (w, h) = (g.size().x, g.size().y);
    let icon_w = if icon.is_some() { 15.0 } else { 0.0 };
    let left = bottom.x - (w + icon_w) / 2.0;
    if let Some(i) = icon {
        crate::icons::image(i, 11.0, ink.accent)
            .paint_at(ui, Rect::from_min_size(pos2(left, bottom.y - h + 1.0), vec2(11.0, 11.0)));
    }
    painter.galley(pos2(left + icon_w, bottom.y - h), g, ink.text);
}

/// How far a knob is turned (0 .. 1) for a value: evenly, or by ratios when its range spans a hundredfold or more
/// (a time from a millisecond to seconds: each turn of the hand counts as much at the short end as at the long).
pub fn turn_of(lo: f64, hi: f64, v: f64) -> f64 {
    let t = if lo > 0.0 && hi / lo >= 100.0 {
        (v.max(lo).ln() - lo.ln()) / (hi.ln() - lo.ln())
    } else {
        (v - lo) / (hi - lo)
    };
    if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The value a knob turned so far (0 .. 1) gives.
pub fn value_of(lo: f64, hi: f64, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if lo > 0.0 && hi / lo >= 100.0 {
        (lo.ln() + t * (hi.ln() - lo.ln())).exp()
    } else {
        lo + t * (hi - lo)
    }
}

/// A double click on a control: back to where the app first puts it.
fn reset_on_double_click(app: &App, r: &egui::Response, node: &str) {
    if r.double_clicked() {
        if let Some(d) = app.default_of(node) {
            observe(app, node, d);
        }
    }
}

pub(crate) fn observe(app: &App, node: &str, v: f64) {
    // her own synthesiser: only she plays it (its loudness and its switch to play by ear excepted)
    if app.locked(node) {
        return;
    }
    app.taped(node, v);
    app.sound_now(node, v);
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
            crate::style::font(crate::style::Size::Subtitle),
            ink.muted,
        );
    }
    let mut new_text: Option<String> = None;
    let outer_ink = ink;
    // where each is: those on the surface in their cells, those held where what holds them puts them
    let mut laid = vec![];
    let mut held = vec![];
    for w in view.widgets.iter().filter(|w| shown(app, w)) {
        let rect = cell(w.at.0, w.at.1, w.size.0.min(COLUMNS), w.size.1);
        lay(app, name, w, rect, &mut laid, &mut held, true);
    }
    frames(app, ui, &painter, held, &ink);
    let tips_on = app.tips_for(&view);
    for (w, rect, top) in laid {
        // inside its cell, a little room on every side: nothing sits on a frame's border
        let rect = if w.kind == WidgetKind::Section || w.kind.holds() { rect } else { rect.shrink2(vec2(6.0, 3.0)) };
        // the widget drawn in its look
        let lk = widget_looks.get(&w.name).copied().unwrap_or(app_look);
        let mut ink = outer_ink;
        ink.accent = lk.accent;
        ink.chosen = crate::style::under_words(lk.accent);
        ink.text = lk.text;
        let saved_style = ui.style().clone();
        {
            let st = ui.style_mut();
            let r = eframe::egui::CornerRadius::same(lk.round.round() as u8);
            // what is chosen: the look's accent made a ground for the dark words (never two colours close)
            st.visuals.selection.bg_fill = crate::style::under_words(lk.accent);
            st.visuals.selection.stroke = Stroke::new(1.0_f32, crate::style::ON_FILL);
            st.visuals.override_text_color = Some(lk.text);
            for w in [
                &mut st.visuals.widgets.inactive,
                &mut st.visuals.widgets.hovered,
                &mut st.visuals.widgets.active,
                &mut st.visuals.widgets.open,
            ] {
                w.corner_radius = r;
            }
            st.visuals.widgets.active.bg_fill = crate::style::under_words(lk.accent);
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
            painter.rect_filled(rect, crate::style::corner(), ink.faint.gamma_multiply(0.25));
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(if chosen { 2.0_f32 } else { 1.0_f32 }, if chosen { ink.accent } else { ink.muted }),
                egui::StrokeKind::Inside,
            );
            // its name above, what it is and its nodes under it: each fitted to its width (a knob is narrow)
            let fit = |words: String, size: crate::style::Size| {
                let mut g = painter.layout_no_wrap(words.clone(), crate::style::font(size), ink.text);
                if g.size().x > rect.width() - 6.0 {
                    g = painter.layout(
                        words,
                        crate::style::font(crate::style::Size::Tiny),
                        ink.text,
                        rect.width() - 6.0,
                    );
                }
                g
            };
            let top_words = fit(w.name.clone(), crate::style::Size::Small);
            painter.galley(pos2(rect.center().x - top_words.size().x / 2.0, rect.top() + 3.0), top_words, ink.text);
            let under = fit(format!("{} {}", w.kind.word(), w.nodes.join(" ")), crate::style::Size::Tiny);
            let at = pos2(rect.center().x - under.size().x / 2.0, rect.bottom() - under.size().y - 3.0);
            painter.galley(at, under, ink.muted);
            let handle = Rect::from_min_size(rect.right_bottom() - vec2(14.0, 14.0), vec2(14.0, 14.0));
            // what is held is moved with what holds it (only what stands on the surface has a place of its own)
            if top {
                painter.rect_filled(handle, crate::style::corner(), ink.accent);
            }
            if resp.clicked() {
                app.surface.chosen = Some(w.name.clone());
                app.surface.edit_line = view.text.lines().nth(w.line - 1).unwrap_or("").to_string();
            }
            if top && resp.drag_started() {
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
        // the widget at work, never out of its place: small, it says less (its words come on the pointer)
        let label = if w.text.is_empty() { w.name.clone() } else { w.text.clone() };
        let clip_before = ui.clip_rect();
        ui.set_clip_rect(rect.expand(1.0).intersect(clip_before));
        let painter = painter.with_clip_rect(rect.expand(1.0).intersect(clip_before));
        let small = rect.width() < 230.0;
        match w.kind {
            WidgetKind::Slider => {
                if let Some(n) = node(0) {
                    let mut v = n.value;
                    let mut inner = rect;
                    inner.set_height(inner.height().min(28.0));
                    inner = inner.translate(vec2(0.0, (rect.height() - inner.height()) / 2.0));
                    ui.spacing_mut().slider_width = (inner.width() - if small { 8.0 } else { 150.0 }).max(30.0);
                    let slider = egui::Slider::new(&mut v, w.lo..=w.hi).clamping(egui::SliderClamping::Always);
                    let slider = if small { slider.show_value(false) } else { slider.text(&label) };
                    let r = ui.put(inner, slider);
                    let r = if small { r.on_hover_text(format!("{label}: {}", grm_engine::lang::g(v))) } else { r };
                    if r.changed() {
                        observe(app, &n.name, v);
                    }
                    reset_on_double_click(app, &r, &n.name);
                    // any slider can be given to a knob of a controller
                    let (lo, hi, node_name) = (w.lo, w.hi, n.name.clone());
                    r.context_menu(|ui| {
                        if crate::icons::button(ui, "piano", crate::text::t(app.settings.lang, "midi_learn")).clicked()
                        {
                            app.midi_learn = Some(crate::midi::Target::Node { name: node_name.clone(), lo, hi });
                            app.settings_open = true;
                            ui.close();
                        }
                    });
                }
            }
            WidgetKind::View3d => {
                let lists = view.lists.clone();
                crate::view3d::show(app, ui, rect, w, name, &lists);
            }
            WidgetKind::Bars => {
                // each bar as tall as its node among the others; the one the first node says, lit
                let lit = node(0).map(|n| now_of(app, &n).round() as i64).unwrap_or(0);
                let values: Vec<f64> =
                    (1..w.nodes.len()).map(|k| node(k).map(|n| now_of(app, &n)).unwrap_or(0.0)).collect();
                let lo = values.iter().cloned().fold(f64::MAX, f64::min);
                let hi = values.iter().cloned().fold(f64::MIN, f64::max).max(lo + 1e-9);
                let words_h = if label.is_empty() { 0.0 } else { 14.0 };
                painter.text(
                    rect.left_top(),
                    egui::Align2::LEFT_TOP,
                    &label,
                    crate::style::font(crate::style::Size::Tiny),
                    ink.muted,
                );
                let area = Rect::from_min_max(rect.left_top() + vec2(0.0, words_h), rect.right_bottom());
                let wide = area.width() / values.len().max(1) as f32;
                for (k, v) in values.iter().enumerate() {
                    let h = 3.0 + (area.height() - 3.0) * ((v - lo) / (hi - lo)) as f32;
                    let x = area.left() + k as f32 * wide;
                    let bar = Rect::from_min_max(pos2(x + 1.0, area.bottom() - h), pos2(x + wide - 1.0, area.bottom()));
                    let colour = if lit == k as i64 + 1 { ink.accent } else { ink.muted.gamma_multiply(0.6) };
                    painter.rect_filled(bar, crate::style::corner(), colour);
                }
                let r = ui.interact(rect, id, Sense::hover());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &label));
            }
            WidgetKind::Choice => {
                if let Some(n) = node(0) {
                    // each choice its words, and its value when written (`1/8.=0.75`; else its place from 0)
                    let mut options = vec![];
                    let mut values = vec![];
                    for (k, o) in w.text.split('|').enumerate() {
                        match o.rsplit_once('=').and_then(|(l, v)| v.trim().parse::<f64>().ok().map(|v| (l, v))) {
                            Some((l, v)) => {
                                options.push(l.trim().to_string());
                                values.push(v);
                            }
                            None => {
                                options.push(o.trim().to_string());
                                values.push(k as f64);
                            }
                        }
                    }
                    // the one shown: the nearest to what the node is
                    let near = |v: f64| (v - n.value).abs();
                    let mut k =
                        (0..values.len()).min_by(|a, b| near(values[*a]).total_cmp(&near(values[*b]))).unwrap_or(0);
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    if crate::widgets::choice(&mut child, id, &mut k, &options, w.tip.as_deref().unwrap_or("")) {
                        observe(app, &n.name, values[k]);
                    }
                }
            }
            WidgetKind::Field => {
                if let Some(n) = node(0) {
                    let key = format!("{name}/{}", w.name);
                    let mut text =
                        app.surface.fields.get(&key).cloned().unwrap_or_else(|| grm_engine::lang::g(n.value));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    let example = if w.text.is_empty() { "9,81" } else { w.text.as_str() };
                    let need = crate::widgets::Need::Number;
                    let (fine, r) =
                        crate::widgets::field(&mut child, &mut text, need, example, rect.width() - 4.0, lang);
                    // given to its node with Enter; what is being written is kept until then
                    let enter = r.lost_focus() && child.input(|i| i.key_pressed(egui::Key::Enter));
                    if fine && enter {
                        if let Some(v) = crate::widgets::number(&text) {
                            observe(app, &n.name, v);
                        }
                        app.surface.fields.remove(&key);
                    } else if text != grm_engine::lang::g(n.value) {
                        app.surface.fields.insert(key, text);
                    } else {
                        app.surface.fields.remove(&key);
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
            // a button that says a line: pressed, its line is said as typed at the prompt (`{app}`: its app)
            WidgetKind::Button if w.say.is_some() => {
                let words = RichText::new(&label).size(crate::style::size(crate::style::Size::Text) * lk.size);
                let b = match &w.icon {
                    Some(i) => egui::Button::image_and_text(crate::icons::image(i, 15.0, ink.text), words),
                    None => egui::Button::new(words),
                };
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                if child.add(b).clicked() || pressed_by(ui, w) {
                    if let Some(line) = &w.say {
                        app.say(line.replace("{app}", name));
                    }
                }
            }
            WidgetKind::Button => {
                if let Some(n) = node(0) {
                    let r = ui.put(
                        rect,
                        egui::Button::new(
                            RichText::new(&label).size(crate::style::size(crate::style::Size::Subtitle) * lk.size),
                        ),
                    );
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
                // playing, it stands on the accent: its words and sign in the colour that reads on it
                let on_colour = if playing { ui.visuals().selection.stroke.color } else { lk.text };
                let sign = crate::icons::image(if playing { "square" } else { "play" }, 15.0 * lk.size, on_colour);
                let b = egui::Button::image_and_text(
                    sign,
                    RichText::new(&label)
                        .size(crate::style::size(crate::style::Size::Subtitle) * lk.size)
                        .color(on_colour),
                )
                .selected(playing);
                if ui.put(rect, b).clicked() || pressed_by(ui, w) {
                    app.say(if playing { format!("stop {}", w.nodes[0]) } else { format!("play {}", w.nodes[0]) });
                }
            }
            WidgetKind::Rec => {
                // recording what is played and turned: its light, and how long it has been going
                let recording = app.played.recording.is_some();
                let colour = if recording { ui.visuals().selection.stroke.color } else { lk.text };
                let sign = crate::icons::image(if recording { "square" } else { "circle-dot" }, 15.0 * lk.size, colour);
                let words = match app.recording_for() {
                    Some(s) => format!("{label} {:02}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32),
                    None => label.clone(),
                };
                let b = egui::Button::image_and_text(
                    sign,
                    RichText::new(words).size(crate::style::size(crate::style::Size::Subtitle) * lk.size).color(colour),
                )
                .selected(recording);
                if ui.put(rect, b).on_hover_text(crate::text::t(app.settings.lang, "rec_d")).clicked() {
                    app.record();
                }
                if recording {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
                }
            }
            WidgetKind::Meter => {
                if let Some(n) = node(0) {
                    let r = ui.interact(rect, id, Sense::hover());
                    // a node the sound computes is read from the sound (what passes now), and falls slowly: it
                    // rises and falls, it does not flash
                    let heard = app.mixer.lock().ok().and_then(|m| m.sounding(&n.name));
                    let now = match heard {
                        Some(v) => {
                            let key = format!("{name}/{}", w.name);
                            let was = app.surface.meters.get(&key).copied().unwrap_or(0.0);
                            let v = (was * 0.9).max(v.abs());
                            app.surface.meters.insert(key, v);
                            v
                        }
                        None => n.value,
                    };
                    let k = ((now - w.lo) / (w.hi - w.lo)).clamp(0.0, 1.0) as f32;
                    let colour = if heard.is_some() && k > 0.9 { ink.amber } else { ink.accent };
                    if rect.height() > rect.width() * 1.5 {
                        // standing: it fills from the bottom
                        let bar =
                            Rect::from_center_size(rect.center(), vec2(10.0_f32.min(rect.width()), rect.height()));
                        painter.rect_filled(bar, crate::style::corner(), ink.faint);
                        let full = Rect::from_min_max(pos2(bar.left(), bar.bottom() - bar.height() * k), bar.max);
                        painter.rect_filled(full, crate::style::corner(), colour);
                    } else {
                        let bar = Rect::from_min_size(rect.left_center() + vec2(0.0, 2.0), vec2(rect.width(), 10.0));
                        painter.rect_filled(bar, crate::style::corner(), ink.faint);
                        let mut full = bar;
                        full.set_width(bar.width() * k);
                        painter.rect_filled(full, crate::style::corner(), colour);
                        painter.text(
                            rect.left_top(),
                            egui::Align2::LEFT_TOP,
                            &label,
                            crate::style::font(crate::style::Size::Text),
                            ink.muted,
                        );
                    }
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::ProgressIndicator,
                            true,
                            format!("{label}: {}", reading(n.value, n.sigma, n.signed)),
                        )
                    });
                }
            }
            WidgetKind::Sky => {
                let code = node(0).map(|n| n.value.round() as i64).unwrap_or(-1);
                let day = node(1).is_none_or(|n| n.value > 0.5);
                let (icon, en, fr) = sky(code, day);
                let r = ui.interact(rect, id, Sense::hover());
                let side = (rect.height() - 22.0).min(rect.width()).max(16.0);
                let at = Rect::from_center_size(rect.center_top() + vec2(0.0, side / 2.0 + 2.0), vec2(side, side));
                crate::icons::image(icon, side, ink.accent).paint_at(ui, at);
                let words = if app.settings.lang == crate::text::Lang::Fr { fr } else { en };
                painter.text(
                    rect.center_bottom(),
                    egui::Align2::CENTER_BOTTOM,
                    words,
                    crate::style::font(crate::style::Size::Text),
                    ink.text,
                );
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, words));
            }
            WidgetKind::Series | WidgetKind::Range => {
                let r = ui.interact(rect, id, Sense::hover());
                painter.text(
                    rect.left_top(),
                    egui::Align2::LEFT_TOP,
                    &label,
                    crate::style::font(crate::style::Size::Text),
                    ink.muted,
                );
                let inner =
                    Rect::from_min_max(rect.left_top() + vec2(30.0, 20.0), rect.right_bottom() - vec2(6.0, 18.0));
                let vals: Vec<Option<f64>> = (0..w.nodes.len()).map(|k| node(k).map(|n| n.value)).collect();
                let y =
                    |v: f64| inner.bottom() - (((v - w.lo) / (w.hi - w.lo)) as f32).clamp(0.0, 1.0) * inner.height();
                // its scale: a few levels, faint
                for f in [0.0f64, 0.5, 1.0] {
                    let v = w.lo + f * (w.hi - w.lo);
                    painter.line_segment(
                        [pos2(inner.left(), y(v)), pos2(inner.right(), y(v))],
                        Stroke::new(0.5_f32, ink.faint),
                    );
                    painter.text(
                        pos2(inner.left() - 4.0, y(v)),
                        egui::Align2::RIGHT_CENTER,
                        grm_engine::lang::g(v),
                        crate::style::font(crate::style::Size::Small),
                        ink.muted,
                    );
                }
                let fr = app.settings.lang == crate::text::Lang::Fr;
                if w.kind == WidgetKind::Series {
                    let n = vals.len().max(2) as f32 - 1.0;
                    let x = |k: usize| inner.left() + k as f32 / n * inner.width();
                    let pts: Vec<Pos2> =
                        vals.iter().enumerate().filter_map(|(k, v)| v.map(|v| pos2(x(k), y(v)))).collect();
                    if pts.len() > 1 {
                        // a soft ground under the curve, then the curve
                        for pair in pts.windows(2) {
                            let quad = vec![
                                pair[0],
                                pair[1],
                                pos2(pair[1].x, inner.bottom()),
                                pos2(pair[0].x, inner.bottom()),
                            ];
                            painter.add(egui::Shape::convex_polygon(
                                quad,
                                ink.accent.gamma_multiply(0.12),
                                Stroke::NONE,
                            ));
                        }
                        painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0_f32, ink.accent)));
                    }
                    // every few, its value and how far ahead
                    let step = (vals.len() / 8).max(1);
                    for (k, v) in vals.iter().enumerate().step_by(step) {
                        if let Some(v) = v {
                            painter.text(
                                pos2(x(k), y(*v) - 4.0),
                                egui::Align2::CENTER_BOTTOM,
                                format!("{v:.0}"),
                                crate::style::font(crate::style::Size::Small),
                                ink.text,
                            );
                            let when = if k == 0 {
                                if fr {
                                    "maint.".to_string()
                                } else {
                                    "now".to_string()
                                }
                            } else {
                                format!("+{k}")
                            };
                            painter.text(
                                pos2(x(k), inner.bottom() + 3.0),
                                egui::Align2::CENTER_TOP,
                                when,
                                crate::style::font(crate::style::Size::Small),
                                ink.muted,
                            );
                        }
                    }
                    if let Some(p) = r.hover_pos() {
                        let k = (((p.x - inner.left()) / inner.width()) * n).round().clamp(0.0, n) as usize;
                        if let Some(Some(v)) = vals.get(k) {
                            painter.circle_filled(pos2(x(k), y(*v)), 3.5, ink.accent);
                            painter.text(
                                pos2(x(k), inner.top()),
                                egui::Align2::CENTER_BOTTOM,
                                format!("+{k}: {}", grm_engine::lang::g(*v)),
                                crate::style::font(crate::style::Size::Small),
                                ink.text,
                            );
                        }
                    }
                } else {
                    // pairs: each a bar from its low to its high, the day's name under it
                    let pairs = vals.len() / 2;
                    let slot = inner.width() / pairs.max(1) as f32;
                    let today = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() / 86_400)
                        .unwrap_or(0);
                    let names_fr = ["jeu.", "ven.", "sam.", "dim.", "lun.", "mar.", "mer."];
                    let names_en = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
                    for d in 0..pairs {
                        let (Some(a), Some(b)) = (vals[2 * d], vals[2 * d + 1]) else { continue };
                        let (lo, hi) = (a.min(b), a.max(b));
                        let cx = inner.left() + slot * (d as f32 + 0.5);
                        let bar = Rect::from_min_max(pos2(cx - slot * 0.18, y(hi)), pos2(cx + slot * 0.18, y(lo)));
                        painter.rect_filled(bar, crate::style::corner(), ink.accent.gamma_multiply(0.75));
                        painter.text(
                            pos2(cx, bar.top() - 2.0),
                            egui::Align2::CENTER_BOTTOM,
                            format!("{hi:.0}°"),
                            crate::style::font(crate::style::Size::Small),
                            ink.text,
                        );
                        painter.text(
                            pos2(cx, bar.bottom() + 2.0),
                            egui::Align2::CENTER_TOP,
                            format!("{lo:.0}°"),
                            crate::style::font(crate::style::Size::Small),
                            ink.muted,
                        );
                        let wd = ((today + d as u64) % 7) as usize;
                        let day = if d == 0 {
                            if fr {
                                "auj."
                            } else {
                                "today"
                            }
                        } else if fr {
                            names_fr[wd]
                        } else {
                            names_en[wd]
                        };
                        painter.text(
                            pos2(cx, rect.bottom()),
                            egui::Align2::CENTER_BOTTOM,
                            day,
                            crate::style::font(crate::style::Size::Small),
                            ink.muted,
                        );
                    }
                }
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &label));
            }
            WidgetKind::Value => {
                if let Some(n) = node(0) {
                    // what the sound computes is exact (no ± to it): a counter, a bar
                    let heard = app.mixer.lock().ok().and_then(|m| m.sounding(&n.name));
                    let now = heard.unwrap_or(n.value);
                    let shown = match heard {
                        Some(v) => grm_engine::lang::g(v),
                        None => reading(now, n.sigma, n.signed),
                    };
                    let r = ui.interact(rect, id, Sense::hover());
                    if rect.height() < 44.0 {
                        // short: its words and its value on one line
                        let words = painter.layout_no_wrap(
                            label.clone(),
                            crate::style::font(crate::style::Size::Small),
                            ink.muted,
                        );
                        let w0 = words.size().x;
                        painter.galley(pos2(rect.left(), rect.center().y - words.size().y / 2.0), words, ink.muted);
                        painter.text(
                            pos2(rect.left() + w0 + if w0 > 0.0 { 6.0 } else { 0.0 }, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &shown,
                            crate::style::font(crate::style::Size::Subtitle),
                            ink.text,
                        );
                    } else {
                        painter.text(
                            rect.left_top(),
                            egui::Align2::LEFT_TOP,
                            &label,
                            crate::style::font(crate::style::Size::Text),
                            ink.muted,
                        );
                        painter.text(
                            rect.left_bottom(),
                            egui::Align2::LEFT_BOTTOM,
                            &shown,
                            crate::style::font(crate::style::Size::Subtitle),
                            ink.text,
                        );
                    }
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("{label}: {shown}"))
                    });
                }
            }
            WidgetKind::Plot => {
                let r = ui.interact(rect, id, Sense::hover());
                painter.rect_stroke(
                    rect,
                    crate::style::corner(),
                    Stroke::new(1.0_f32, ink.faint),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    rect.left_top() + vec2(6.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    crate::style::font(crate::style::Size::Text),
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
                            crate::style::font(crate::style::Size::Text),
                            ink.text,
                        );
                    }
                }
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("plot of {}", w.nodes[0]))
                });
            }
            WidgetKind::Text => {
                ui.put(
                    rect,
                    egui::Label::new(
                        RichText::new(&w.text).size(crate::style::size(crate::style::Size::Subtitle) * lk.size),
                    )
                    .wrap(),
                );
            }
            WidgetKind::Pad => {
                if let (Some(nx), Some(ny)) = (node(0), node(1)) {
                    let side = rect.width().min(rect.height());
                    let sq = Rect::from_min_size(rect.min, vec2(side, side));
                    let r = ui.interact(sq, id, Sense::click_and_drag());
                    painter.rect_filled(sq, crate::style::corner(), ink.faint.gamma_multiply(0.4));
                    painter.rect_stroke(
                        sq,
                        crate::style::corner(),
                        Stroke::new(1.0_f32, ink.muted),
                        egui::StrokeKind::Inside,
                    );
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
                        crate::style::font(crate::style::Size::Small),
                        ink.muted,
                    );
                }
            }
            WidgetKind::Keys => keys(app, ui, &painter, rect, id, w, &ink),
            WidgetKind::Drums => {
                // a drum machine's steps, in its own place on the surface (inside the machine's section)
                let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(id));
                crate::studio::drum_grid(app, &mut inner, w.text.trim(), &ink);
            }
            WidgetKind::Knob => {
                if let Some(n) = node(0) {
                    // a round knob, as on a desk: its scale of ticks, its cap, its words under it; dragged up and
                    // down (the value shows while it moves, and on the pointer)
                    let r = ui.interact(rect, id, Sense::click_and_drag());
                    let mut v = n.value;
                    if r.drag_stopped() {
                        app.surface.turning = None;
                    }
                    if r.dragged() && !app.locked(&n.name) {
                        let dy = -r.drag_delta().y as f64;
                        // from where the hand has it, not from the machine's last word
                        if let Some((name, held)) = &app.surface.turning {
                            if *name == n.name {
                                v = *held;
                            }
                        }
                        let from = v;
                        // its whole course in 200 points; with Shift, four times finer
                        let fine = if ui.input(|i| i.modifiers.shift) { 4.0 } else { 1.0 };
                        v = value_of(w.lo, w.hi, (turn_of(w.lo, w.hi, v) + dy / (200.0 * fine)).clamp(0.0, 1.0));
                        app.surface.turning = Some((n.name.clone(), v));
                        observe(app, &n.name, v);
                        // she watches the human turn it
                        let part = |x: f64| (x - w.lo) / (w.hi - w.lo);
                        app.surface.turned.push((n.name.clone(), part(from), part(v)));
                    }
                    // a double click: back to where the app first puts it
                    if r.double_clicked() {
                        if let Some(d) = app.default_of(&n.name) {
                            observe(app, &n.name, d);
                            v = d;
                        }
                    }
                    // a knob says less only when it is really small
                    let small = rect.width() < 44.0 || rect.height() < 60.0;
                    let lab_h = if small { 0.0 } else { 16.0 };
                    let d = (rect.width().min(rect.height() - lab_h - 8.0) - 14.0).clamp(16.0, 58.0);
                    let c = pos2(rect.center().x, rect.top() + 6.0 + d / 2.0);
                    let rad = d / 2.0;
                    let a0 = std::f32::consts::PI * 0.75;
                    let sweep = std::f32::consts::PI * 1.5;
                    let share = turn_of(w.lo, w.hi, v) as f32;
                    let at = |a: f32, r: f32| c + vec2(a.cos(), a.sin()) * r;
                    // the scale: eleven ticks, the ones passed in the accent
                    for k in 0..=10 {
                        let f = k as f32 / 10.0;
                        let a = a0 + sweep * f;
                        let colour = if f <= share + 1e-4 { ink.accent } else { ink.faint };
                        let long = if k % 5 == 0 { 0.30 } else { 0.18 };
                        painter.line_segment([at(a, rad * (1.0 - long)), at(a, rad)], Stroke::new(1.2_f32, colour));
                    }
                    // the cap: light, a dark line where it points
                    let cap = rad * 0.66;
                    painter.circle_filled(c, cap + 1.5, ink.faint);
                    painter.circle_filled(c, cap, ink.text.gamma_multiply(0.78));
                    painter.circle_filled(c, cap * 0.55, ink.text.gamma_multiply(0.9));
                    let a = a0 + sweep * share;
                    painter.line_segment([at(a, cap * 0.25), at(a, cap * 0.95)], Stroke::new(2.2_f32, ink.surface));
                    if !small && rect.width() >= 80.0 {
                        let g = |x: f64| grm_engine::lang::g((x * 1000.0).round() / 1000.0);
                        painter.text(
                            at(a0, rad + 2.0),
                            egui::Align2::RIGHT_TOP,
                            g(w.lo),
                            crate::style::font(crate::style::Size::Tiny),
                            ink.muted,
                        );
                        painter.text(
                            at(a0 + sweep, rad + 2.0),
                            egui::Align2::LEFT_TOP,
                            g(w.hi),
                            crate::style::font(crate::style::Size::Tiny),
                            ink.muted,
                        );
                    }
                    // a knob of choices ("up · down"): the one chosen lit, the others quiet
                    let choices: Vec<&str> = label.split(" · ").collect();
                    let steps = w.hi - w.lo;
                    if !small && choices.len() > 1 && (steps - (choices.len() - 1) as f64).abs() < 1e-9 {
                        let chosen = ((v - w.lo).round() as usize).min(choices.len() - 1);
                        let font = crate::style::font(crate::style::Size::Small);
                        let parts: Vec<_> = choices
                            .iter()
                            .enumerate()
                            .map(|(k, word)| {
                                let colour = if k == chosen { ink.accent } else { ink.muted.gamma_multiply(0.7) };
                                painter.layout_no_wrap(word.to_uppercase(), font.clone(), colour)
                            })
                            .collect();
                        let gap = 8.0;
                        let total: f32 = parts.iter().map(|g| g.size().x).sum::<f32>() + gap * (parts.len() - 1) as f32;
                        let mut x = c.x - total / 2.0;
                        for g in parts {
                            let (w, h) = (g.size().x, g.size().y);
                            painter.galley(pos2(x, rect.bottom() - h), g, ink.text);
                            x += w + gap;
                        }
                    } else if !small {
                        caption(ui, &painter, pos2(c.x, rect.bottom()), &label, w.icon.as_deref(), &ink);
                    }
                    if r.dragged() || r.hovered() {
                        // the value, between its bounds under the knob (above it, it would be cut)
                        painter.text(
                            c + vec2(0.0, rad + 2.0),
                            egui::Align2::CENTER_TOP,
                            grm_engine::lang::g((v * 1000.0).round() / 1000.0),
                            crate::style::font(crate::style::Size::Small),
                            ink.accent,
                        );
                    }
                    let r = r
                        .on_hover_text(format!("{label}: {}", grm_engine::lang::g(v)))
                        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
                    let (lo, hi, node_name) = (w.lo, w.hi, n.name.clone());
                    r.context_menu(|ui| {
                        if crate::icons::button(ui, "piano", crate::text::t(app.settings.lang, "midi_learn")).clicked()
                        {
                            app.midi_learn = Some(crate::midi::Target::Node { name: node_name.clone(), lo, hi });
                            app.settings_open = true;
                            ui.close();
                        }
                    });
                }
            }
            WidgetKind::Switch => {
                if let Some(n) = node(0) {
                    // an on/off switch and its light
                    let on = n.value > 0.5;
                    let r = ui.interact(rect, id, Sense::click());
                    if r.clicked() || pressed_by(ui, w) {
                        observe(app, &n.name, if on { 0.0 } else { 1.0 });
                    }
                    let h = rect.height().min(22.0);
                    let track =
                        Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - h / 2.0), vec2(h * 1.8, h));
                    painter.rect_filled(track, crate::style::corner(), ink.surface);
                    painter.rect_stroke(
                        track,
                        crate::style::corner(),
                        Stroke::new(1.0_f32, ink.faint),
                        egui::StrokeKind::Inside,
                    );
                    let knob_w = track.width() / 2.0 - 2.0;
                    let x = if on { track.right() - knob_w - 2.0 } else { track.left() + 2.0 };
                    painter.rect_filled(
                        Rect::from_min_size(pos2(x, track.top() + 2.0), vec2(knob_w, h - 4.0)),
                        2.0,
                        ink.text.gamma_multiply(0.8),
                    );
                    let led = pos2(track.right() + 9.0, rect.center().y);
                    painter.circle_filled(led, 4.0, if on { ink.accent } else { ink.faint.gamma_multiply(0.6) });
                    if !small || rect.width() > 110.0 {
                        let tx = led.x + 9.0;
                        let fits = painter
                            .layout_no_wrap(
                                label.to_uppercase(),
                                crate::style::font(crate::style::Size::Small),
                                ink.text,
                            )
                            .size()
                            .x
                            + 17.0
                            + tx
                            <= rect.right();
                        let icon_kept = if fits { w.icon.as_deref() } else { None };
                        if let Some(i) = icon_kept {
                            crate::icons::image(i, 13.0, ink.muted)
                                .paint_at(ui, Rect::from_min_size(pos2(tx, rect.center().y - 6.5), vec2(13.0, 13.0)));
                            painter.text(
                                pos2(tx + 17.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label.to_uppercase(),
                                crate::style::font(crate::style::Size::Small),
                                ink.text,
                            );
                        } else {
                            painter.text(
                                pos2(tx, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label.to_uppercase(),
                                crate::style::font(crate::style::Size::Small),
                                ink.text,
                            );
                        }
                    }
                    r.on_hover_text(format!("{label}: {}", if on { "on" } else { "off" }))
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                }
            }
            WidgetKind::Step => {
                if let (Some(n), Some(at)) = (node(0), node(1)) {
                    // a step: its note's name in a small plate (lit when the sequencer is on it), its fader under it
                    let k: i64 = w.text.trim().parse().unwrap_or(1);
                    let here = (at.value.round() as i64) == k - 1;
                    let plate =
                        Rect::from_min_size(pos2(rect.left() + 3.0, rect.top() + 2.0), vec2(rect.width() - 6.0, 20.0));
                    painter.rect_filled(plate, crate::style::corner(), if here { ink.chosen } else { ink.surface });
                    painter.rect_stroke(
                        plate,
                        3.0,
                        Stroke::new(1.0_f32, if here { ink.accent } else { ink.faint }),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        plate.center(),
                        egui::Align2::CENTER_CENTER,
                        note_name(n.value),
                        crate::style::font(crate::style::Size::Small),
                        if here { crate::style::ON_FILL } else { ink.text },
                    );
                    let mut v = n.value;
                    let inner = Rect::from_min_max(
                        pos2(rect.left(), plate.bottom() + 4.0),
                        pos2(rect.right(), rect.bottom() - 14.0),
                    );
                    ui.spacing_mut().slider_width = (inner.height() - 10.0).max(30.0);
                    let r = ui.put(
                        inner,
                        egui::Slider::new(&mut v, w.lo..=w.hi)
                            .vertical()
                            .logarithmic(w.lo > 0.0)
                            .clamping(egui::SliderClamping::Always)
                            .show_value(false),
                    );
                    reset_on_double_click(app, &r, &n.name);
                    if r.changed() {
                        // exact notes: the step lands on a semitone, when the instrument says so
                        let exact = app.picture.node(&app.named("seq_exact")).is_some_and(|e| e.value > 0.5);
                        if exact && v > 0.0 {
                            v = 55.0 * 2f64.powf((12.0 * (v / 55.0).log2()).round() / 12.0);
                        }
                        observe(app, &n.name, (v * 100.0).round() / 100.0);
                    }
                    // a mark a semitone beside the rail, the octaves (the A's) longer, bold, and named
                    if w.lo > 0.0 && w.hi > w.lo {
                        let rail = r.rect;
                        let m = rail.width() / 2.5;
                        let (y0, y1) = (rail.bottom() - m, rail.top() + m);
                        let (l0, l1) = (w.lo.ln(), w.hi.ln());
                        let x = rail.center().x + 7.0;
                        let first = (12.0 * (w.lo / 55.0).log2()).ceil() as i64;
                        let last = (12.0 * (w.hi / 55.0).log2()).floor() as i64;
                        for semi in first..=last {
                            let hz = 55.0 * 2f64.powf(semi as f64 / 12.0);
                            let f = ((hz.ln() - l0) / (l1 - l0)) as f32;
                            let y = y0 + (y1 - y0) * f;
                            if semi.rem_euclid(12) == 0 {
                                painter.line_segment(
                                    [pos2(x, y), pos2(x + 9.0, y)],
                                    Stroke::new(2.0_f32, ink.text.gamma_multiply(0.8)),
                                );
                                painter.text(
                                    pos2(x + 11.0, y),
                                    egui::Align2::LEFT_CENTER,
                                    format!("A{}", 1 + semi / 12),
                                    crate::style::font(crate::style::Size::Tiny),
                                    ink.muted,
                                );
                            } else {
                                painter.line_segment([pos2(x, y), pos2(x + 4.0, y)], Stroke::new(0.8_f32, ink.faint));
                            }
                        }
                    }
                    painter.text(
                        pos2(rect.center().x, rect.bottom()),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{k}"),
                        crate::style::font(crate::style::Size::Small),
                        if here { ink.accent } else { ink.muted },
                    );
                    r.on_hover_text(format!(
                        "{} · {} Hz",
                        note_name(v),
                        grm_engine::lang::g((v * 10.0).round() / 10.0)
                    ));
                }
            }
            WidgetKind::Roll => {
                // the last ten seconds of what she played: time to the right, pitch up (A1 .. A5, the A's marked),
                // the bass quieter, the melody in the accent; it slides slowly to the left
                painter.rect_filled(rect, crate::style::corner(), ink.surface);
                painter.rect_stroke(
                    rect,
                    crate::style::corner(),
                    Stroke::new(1.0_f32, ink.faint),
                    egui::StrokeKind::Inside,
                );
                let inner = rect.shrink2(vec2(28.0, 6.0)).translate(vec2(10.0, 0.0));
                let now = app.roll_frozen.unwrap_or_else(|| app.ear_now());
                let span = 10.0;
                // the playhead: now, four fifths along; what she has already chosen shows after it
                let head = inner.left() + inner.width() * 0.8;
                let x_of = |t: f64| head + ((t - now) / span) as f32 * inner.width();
                let y_of = |n: i32| inner.bottom() - (n as f32 / 48.0).clamp(0.0, 1.0) * inner.height();
                for a in 0..=4 {
                    let y = y_of(a * 12);
                    painter
                        .line_segment([pos2(inner.left(), y), pos2(inner.right(), y)], Stroke::new(0.5_f32, ink.faint));
                    painter.text(
                        pos2(rect.left() + 4.0, y),
                        egui::Align2::LEFT_CENTER,
                        format!("A{}", a + 1),
                        crate::style::font(crate::style::Size::Tiny),
                        ink.muted,
                    );
                }
                // the rhythmic grid, from the beat itself: a line each beat, stronger at each bar, fainter eighths
                if let Some((read_at, phase, beat, bar)) = app.beat_clock {
                    let beat = beat.max(0.05);
                    let last = read_at - phase * beat;
                    let first_k = ((now - span * 0.8 - last) / beat).floor() as i64;
                    let last_k = ((now + span * 0.2 - last) / beat).ceil() as i64;
                    for k in first_k..=last_k {
                        for half in [0.0, 0.5] {
                            let t = last + (k as f64 + half) * beat;
                            let x = x_of(t);
                            if x < inner.left() || x > inner.right() {
                                continue;
                            }
                            let in_bar = (bar.round() as i64 + k).rem_euclid(4);
                            let stroke = if half > 0.0 {
                                Stroke::new(0.5_f32, ink.faint.gamma_multiply(0.5))
                            } else if in_bar == 0 {
                                Stroke::new(1.2_f32, ink.muted.gamma_multiply(0.8))
                            } else {
                                Stroke::new(0.7_f32, ink.faint)
                            };
                            painter.line_segment([pos2(x, inner.top()), pos2(x, inner.bottom())], stroke);
                        }
                    }
                }
                let h = (inner.height() / 48.0).max(2.0);
                for (at, len, note, voice) in app.played.roll.iter() {
                    let (x0, x1) = (x_of(*at), x_of(at + len));
                    if x1 < inner.left() || x0 > inner.right() {
                        continue;
                    }
                    let r = Rect::from_min_max(
                        pos2(x0.max(inner.left()), y_of(*note) - h),
                        pos2(x1.min(inner.right()), y_of(*note) + h * 0.2),
                    );
                    let colour = match voice {
                        4 => HERS,
                        5 => HERS.gamma_multiply(0.55),
                        6 => HERS.gamma_multiply(0.75),
                        1 => ink.accent,
                        2 => ink.amber,
                        3 => ink.accent.gamma_multiply(0.45),
                        _ => ink.text.gamma_multiply(0.45),
                    };
                    // a note sounding now: fuller, framed, a little taller, as long as it lasts (no flashing)
                    let sounding = *at <= now && now < at + len;
                    if sounding {
                        let r = r.expand2(vec2(0.0, 1.5));
                        painter.rect_filled(r, crate::style::corner(), colour.gamma_multiply(1.4));
                        painter.rect_stroke(
                            r,
                            crate::style::corner(),
                            Stroke::new(1.0_f32, ink.text),
                            egui::StrokeKind::Outside,
                        );
                    } else {
                        painter.rect_filled(r, crate::style::corner(), colour);
                    }
                }
                painter.line_segment(
                    [pos2(head, inner.top() - 2.0), pos2(head, inner.bottom() + 2.0)],
                    Stroke::new(1.0_f32, ink.accent.gamma_multiply(0.7)),
                );
                painter.text(
                    rect.right_top() + vec2(-6.0, 4.0),
                    egui::Align2::RIGHT_TOP,
                    &label,
                    crate::style::font(crate::style::Size::Small),
                    ink.muted,
                );
                if !app.played.roll.is_empty() {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                }
            }
            // what holds widgets draws its frame before them (see `frames`)
            WidgetKind::Row
            | WidgetKind::Column
            | WidgetKind::Grid
            | WidgetKind::Stack
            | WidgetKind::Scroll
            | WidgetKind::Tabs
            | WidgetKind::Card => {}
            WidgetKind::Section => {
                // a band with its title, and a frame around the widgets under it
                let band = Rect::from_min_size(rect.min, vec2(rect.width(), 22.0f32.min(rect.height())));
                let whole = rect;
                painter.rect_stroke(
                    whole,
                    crate::style::corner(),
                    Stroke::new(1.0_f32, ink.faint),
                    egui::StrokeKind::Inside,
                );
                painter.rect_filled(
                    band,
                    egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 },
                    ink.faint.gamma_multiply(0.55),
                );
                let mut x = band.center().x;
                let title = painter.layout_no_wrap(
                    label.to_uppercase(),
                    crate::style::font(crate::style::Size::Text),
                    ink.text,
                );
                if let Some(i) = w.icon.as_deref() {
                    x += 9.0;
                    crate::icons::image(i, 13.0, ink.accent).paint_at(
                        ui,
                        Rect::from_min_size(
                            pos2(x - title.size().x / 2.0 - 19.0, band.center().y - 6.5),
                            vec2(13.0, 13.0),
                        ),
                    );
                }
                painter.galley(pos2(x - title.size().x / 2.0, band.center().y - title.size().y / 2.0), title, ink.text);
            }
            WidgetKind::Fader => {
                if let Some(n) = node(0) {
                    // a slider standing up, its words under it
                    let mut v = n.value;
                    let lab_h = if rect.width() < 40.0 { 0.0 } else { 16.0 };
                    let inner = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y - lab_h));
                    ui.spacing_mut().slider_width = (inner.height() - 30.0).max(30.0);
                    let r = ui.put(
                        inner,
                        egui::Slider::new(&mut v, w.lo..=w.hi)
                            .vertical()
                            .clamping(egui::SliderClamping::Always)
                            .show_value(false),
                    );
                    if r.changed() {
                        observe(app, &n.name, v);
                    }
                    reset_on_double_click(app, &r, &n.name);
                    if lab_h > 0.0 {
                        painter.text(
                            pos2(rect.center().x, rect.bottom()),
                            egui::Align2::CENTER_BOTTOM,
                            &label,
                            crate::style::font(crate::style::Size::Small),
                            ink.text,
                        );
                    }
                    let r = r.on_hover_text(format!("{label}: {}", grm_engine::lang::g(v)));
                    let (lo, hi, node_name) = (w.lo, w.hi, n.name.clone());
                    r.context_menu(|ui| {
                        if crate::icons::button(ui, "piano", crate::text::t(app.settings.lang, "midi_learn")).clicked()
                        {
                            app.midi_learn = Some(crate::midi::Target::Node { name: node_name.clone(), lo, hi });
                            app.settings_open = true;
                            ui.close();
                        }
                    });
                }
            }
            WidgetKind::Steps => {
                if let Some(n) = node(0) {
                    // a light per step, the one it is on lit (steady: it moves with the steps, it does not blink)
                    let count = ((w.hi - w.lo).round() as i64 + 1).clamp(1, 64) as usize;
                    let now = (n.value - w.lo).round() as i64;
                    let gap = 4.0;
                    let lw = ((rect.width() - gap * (count as f32 - 1.0)) / count as f32).max(4.0);
                    let lh = rect.height().min(lw).clamp(6.0, 22.0);
                    for k in 0..count {
                        let x = rect.left() + k as f32 * (lw + gap);
                        let led = Rect::from_min_size(pos2(x, rect.center().y - lh / 2.0), vec2(lw, lh));
                        if k as i64 == now {
                            painter.rect_filled(led, crate::style::corner(), ink.accent);
                        } else {
                            painter.rect_stroke(
                                led,
                                crate::style::corner(),
                                Stroke::new(1.0_f32, ink.faint),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ui.interact(rect, id, Sense::hover()).on_hover_text(format!("{label}: {}", now + 1));
                }
            }
            WidgetKind::Scope => {
                let r = ui.interact(rect, id, Sense::hover());
                painter.rect_stroke(
                    rect,
                    crate::style::corner(),
                    Stroke::new(1.0_f32, ink.faint),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    rect.left_top() + vec2(6.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    crate::style::font(crate::style::Size::Text),
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
                        // small, it only says it is quiet
                        painter.text(
                            inner.center(),
                            egui::Align2::CENTER_CENTER,
                            if rect.width() < 360.0 { "·" } else { t(lang, "not_playing") },
                            crate::style::font(crate::style::Size::Text),
                            ink.muted,
                        );
                    }
                }
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("scope of {}", w.nodes[0]))
                });
            }
        }
        ui.set_clip_rect(clip_before);
        ui.set_style(saved_style);
        // what it does, said when the pointer rests on it (unless the human hid this kind of tips)
        if let Some(tip) = w.tip.as_ref().filter(|_| tips_on && !editing) {
            let still = ui.input(|i| i.pointer.time_since_last_movement()) > 0.5;
            if still && ui.rect_contains_pointer(rect) {
                let r = ui.interact(rect, id.with("tip"), Sense::hover());
                r.show_tooltip_text(tip.as_str());
            }
        }
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
            let galley =
                painter.layout(text, crate::style::font(crate::style::Size::Small), ink.text, rect.width().max(160.0));
            let back = Rect::from_min_size(rect.left_top(), galley.size() + vec2(8.0, 6.0));
            painter.rect_filled(back, crate::style::corner(), ink.surface.gamma_multiply(0.92));
            painter.rect_stroke(
                back,
                crate::style::corner(),
                Stroke::new(1.0_f32, ink.amber),
                egui::StrokeKind::Inside,
            );
            painter.galley(back.min + vec2(4.0, 3.0), galley, ink.text);
        }
    }
    if let Some(text) = new_text {
        app.edit_app_text(name, text);
    }
}

fn keys(app: &mut App, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect, id: egui::Id, w: &Widget, ink: &Ink) {
    // the octave, moved up or down (its buttons on the left; Z and X on the computer's keys)
    let side = Rect::from_min_size(rect.min, vec2(24.0, rect.height()));
    let rect = Rect::from_min_max(pos2(rect.left() + 28.0, rect.top()), rect.max);
    let third = side.height() / 3.0;
    let up = Rect::from_min_size(side.min, vec2(side.width(), third));
    let down = Rect::from_min_size(pos2(side.left(), side.bottom() - third), vec2(side.width(), third));
    for (b, icon, step) in [(up, "chevron-up", 1), (down, "chevron-down", -1)] {
        let r =
            ui.interact(b, id.with(icon), Sense::click()).on_hover_text(crate::text::t(app.settings.lang, "octave"));
        painter.rect_filled(b.shrink(1.0), crate::style::corner(), if r.hovered() { ink.faint } else { ink.surface });
        crate::icons::image(icon, 12.0, ink.text).paint_at(ui, Rect::from_center_size(b.center(), vec2(12.0, 12.0)));
        if r.clicked() {
            app.surface.octave = (app.surface.octave + step).clamp(-3, 3);
        }
    }
    let typing = ui.ctx().memory(|m| m.focused().is_some()) || ui.input(|i| i.modifiers.any());
    // the computer's keys by where they are, not by what they print: the same hand on QWERTY, AZERTY, QWERTZ...
    // (the row of A on QWERTY, of Q on AZERTY; the octave under it, Z and X on QWERTY, W and X on AZERTY)
    let focused = ui.input(|i| i.focused);
    if typing || app.editing || !focused {
        app.surface.phys_down.clear();
    } else {
        let mut octave = app.surface.octave;
        let mut down = std::mem::take(&mut app.surface.phys_down);
        ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, repeat, .. } = e {
                    let k = physical_key.unwrap_or(*key);
                    if let Some((_, semi)) = PIANO.iter().find(|(pk, _)| *pk == k) {
                        if *pressed {
                            down.insert(*semi);
                        } else {
                            down.remove(semi);
                        }
                    }
                    if *pressed && !repeat {
                        match k {
                            egui::Key::Z => octave = (octave - 1).max(-3),
                            egui::Key::X => octave = (octave + 1).min(3),
                            _ => {}
                        }
                    }
                }
            }
        });
        app.surface.phys_down = down;
        app.surface.octave = octave;
    }
    painter.text(
        side.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:+}", app.surface.octave),
        crate::style::font(crate::style::Size::Small),
        ink.text,
    );
    let shift = 12 * app.surface.octave;
    let (lo0, hi0) = (w.lo.round() as i32, w.hi.round() as i32);
    // a wide keyboard has more keys, not wider ones: whole octaves added above and below (a piano's 88 at most)
    let is_white = |m: &i32| ![1, 3, 6, 8, 10].contains(&m.rem_euclid(12));
    let whites = (lo0..=hi0).filter(is_white).count() as i32;
    let more = ((rect.width() / 24.0) as i32 - whites).max(0) / 7;
    let hi = (hi0 + 12 * ((more + 1) / 2)).min(108);
    let lo = (lo0 - 12 * (more / 2)).max(21);
    let white: Vec<i32> = (lo..=hi).filter(is_white).collect();
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
    // every key held: the pointer's, and the computer's (several at once: chords)
    let mut held: std::collections::BTreeSet<i32> = pointer
        .and_then(|p| boxes.iter().rev().find(|(_, b, _)| b.contains(p)).map(|(m, _, _)| *m))
        .into_iter()
        .collect();
    // the computer's keys play only alone (Ctrl+E is not an E), and not while something is typed
    // (they stay where they were, from the keyboard's first key as it is written)
    for k in &app.surface.phys_down {
        if lo0 + k <= hi {
            held.insert(lo0 + k);
        }
    }
    // her keyboard plays only by her
    if app.locked(&w.nodes[0]) {
        held.clear();
    }
    // the notes sounding now, lit softly on their keys. In the studio each keyboard shows its own instrument only,
    // read from the sound: its voices, the one she plays in her colour, and the notes its arpeggiator holds latched
    // (framed); elsewhere, what the roll says sounds
    let now = app.ear_now();
    let pre = w.nodes[0].strip_suffix("pitch").unwrap_or("").to_string();
    let read = |n: &str| app.mixer.lock().ok().and_then(|m| m.value("out", &format!("{pre}{n}")));
    let key_of = |hz: f64| (12.0 * (hz.max(1.0) / 55.0).log2()).round() as i32 + 33 - shift;
    let (sounding, hers_now, latched): (
        std::collections::BTreeSet<i32>,
        std::collections::BTreeSet<i32>,
        std::collections::BTreeSet<i32>,
    ) = if !pre.is_empty() && read("g1").is_some() {
        let hers = read("hers").unwrap_or(0.0) > 0.5;
        let mut mine = std::collections::BTreeSet::new();
        let mut her = std::collections::BTreeSet::new();
        for (k, (p, g)) in [("g1", "gate"), ("g2", "lg"), ("g3", "c1g"), ("g4", "c2g")].iter().enumerate() {
            if read(g).unwrap_or(0.0) > 0.5 {
                let key = key_of(read(p).unwrap_or(0.0));
                if hers && k >= 1 {
                    her.insert(key)
                } else {
                    mine.insert(key)
                };
            }
        }
        let mut held = std::collections::BTreeSet::new();
        if read("arp_on").unwrap_or(0.0) > 0.5 {
            for i in 1..=4 {
                if read(&format!("arp_lat{i}")).unwrap_or(0.0) > 0.5 {
                    held.insert(key_of(read(&format!("arp_lp{i}")).unwrap_or(0.0)));
                }
            }
        }
        (mine, her, held)
    } else {
        let s = app
            .played
            .roll
            .iter()
            .filter(|(at, len, _, _)| *at <= now && now < at + len)
            .map(|(_, _, n, _)| n + 33 - shift)
            .collect();
        (s, Default::default(), Default::default())
    };
    if !sounding.is_empty() || !hers_now.is_empty() || !latched.is_empty() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(40));
    }
    for (m, b, black) in &boxes {
        let on = held.contains(m);
        // a keyboard looks like one, whatever the theme
        let fill = if on {
            ink.accent
        } else if hers_now.contains(m) {
            HERS
        } else if sounding.contains(m) {
            ink.accent.gamma_multiply(0.55)
        } else if *black {
            Color32::from_rgb(0x26, 0x27, 0x2B)
        } else {
            Color32::from_rgb(0xEC, 0xEC, 0xE8)
        };
        painter.rect_filled(*b, crate::style::corner(), fill);
        painter.rect_stroke(*b, crate::style::corner(), Stroke::new(1.0_f32, ink.muted), egui::StrokeKind::Inside);
        // held by the latch: framed (it goes on sounding, by turns)
        if latched.contains(m) {
            painter.rect_stroke(
                b.shrink(2.0),
                crate::style::corner(),
                Stroke::new(2.0_f32, ink.accent),
                egui::StrokeKind::Inside,
            );
        }
    }
    // a polyphonic instrument (a `poly` node beside its pitch): each key held takes a voice (the window shares them
    // out); otherwise the last key held plays, alone
    let poly = app.is_poly(&w.nodes[0]);
    let before = app.surface.keys_down.get(&w.nodes[0]).cloned().unwrap_or_default();
    if held != before {
        for m in held.difference(&before) {
            app.surface.played.push(m + shift);
        }
        // the keys held, for the arpeggiator (and, on a polyphonic instrument, each its voice)
        app.surface.keys_held = held.iter().map(|m| m + shift).collect();
        if poly {
            app.surface.poly = Some(w.nodes[0].clone());
        } else {
            match held.iter().next_back() {
                Some(m) => {
                    observe(app, &w.nodes[0], freq((m + shift) as f64));
                    observe(app, &w.nodes[1], 1.0);
                }
                None => observe(app, &w.nodes[1], 0.0),
            }
        }
        app.surface.keys_down.insert(w.nodes[0].clone(), held.clone());
        ui.ctx().request_repaint();
    }
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("keys: {} and {}", w.nodes[0], w.nodes[1]))
    });
    if !held.is_empty() {
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
            egui::TextEdit::singleline(&mut app.surface.add_name)
                .margin(crate::style::field())
                .hint_text(crate::style::hint(t(lang, "widget_name")))
                .desired_width(90.0),
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
            // the buttons from the right, the field takes what is left (nothing goes under the side)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.add_enabled(false, egui::Button::new(t(lang, "remove_widget")));
                ui.add_enabled(false, egui::Button::new(t(lang, "apply")));
                ui.add_enabled(
                    false,
                    egui::TextEdit::singleline(&mut String::new())
                        .margin(crate::style::field())
                        .hint_text(crate::style::hint(t(lang, "choose_widget")))
                        .desired_width(ui.available_width() - 8.0),
                );
            });
        });
    }
    if let Some(chosen) = chosen.map(|w| w.name) {
        if let Some(w) = view.widgets.iter().find(|w| w.name == chosen) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} (line {})", w.name, w.line)).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if crate::icons::button(ui, "trash-2", t(lang, "remove_widget")).clicked() {
                        let text = grm_shell::apps::with_line(&view.text, w.line, None);
                        app.edit_app_text(name, text);
                        app.surface.chosen = None;
                    }
                    let apply = ui.button(t(lang, "apply")).clicked();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut app.surface.edit_line)
                            .margin(crate::style::field())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(ui.available_width() - 8.0),
                    );
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if apply || enter {
                        let text = grm_shell::apps::with_line(&view.text, w.line, Some(app.surface.edit_line.trim()));
                        app.edit_app_text(name, text);
                    }
                });
            });
        }
    }
}

/// The sky a weather code says (WMO, as Open-Meteo gives it): its sign, in English, in French.
pub fn sky(code: i64, day: bool) -> (&'static str, &'static str, &'static str) {
    match code {
        0 if day => ("sun", "clear", "dégagé"),
        0 => ("moon", "clear", "dégagé"),
        1 | 2 if day => ("cloud-sun", "partly cloudy", "quelques nuages"),
        1 | 2 => ("cloud-moon", "partly cloudy", "quelques nuages"),
        3 => ("cloud", "overcast", "couvert"),
        45 | 48 => ("cloud-fog", "fog", "brouillard"),
        51..=57 => ("cloud-drizzle", "drizzle", "bruine"),
        61..=67 | 80..=82 => ("cloud-rain", "rain", "pluie"),
        71..=77 | 85 | 86 => ("cloud-snow", "snow", "neige"),
        95..=99 => ("cloud-lightning", "storm", "orage"),
        _ => ("cloud", "…", "…"),
    }
}
