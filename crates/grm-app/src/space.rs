// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The space: no desk, a table. An infinite surface (drag the background to move, Ctrl and the wheel or two fingers
//! to zoom) where the human puts what they want, in their own order or their own mess: apps (small, an app is a
//! widget; bigger, a window), networks, files, plots, what the machine feels, a clock. A button tidies it.
//!
//! The space is itself a file (`space.grm` beside the memory), one line per thing on the table:
//! `place app weather at 40 60 size 420 300`. It can be written by hand: what is written is what is shown.

use std::path::{Path, PathBuf};

use eframe::egui::{self, pos2, vec2, Color32, Rect, RichText, Sense, Stroke};

use crate::app::{App, Ink};
use crate::text::t;

/// What can be put on the table.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    App(String),
    /// A network (a source), or "all".
    Network(String),
    File(PathBuf),
    Feelings,
    Plots,
    Clock,
    /// A network made apart, where the language is learned.
    Workshop(String),
    Inspector,
}

impl Kind {
    pub fn word(&self) -> String {
        match self {
            Kind::App(a) => format!("app {a}"),
            Kind::Network(n) => format!("network {n}"),
            Kind::File(p) => format!("file {}", grm_shell::quoted(&p.display().to_string())),
            Kind::Feelings => "feelings".into(),
            Kind::Plots => "plots".into(),
            Kind::Clock => "clock".into(),
            Kind::Workshop(w) => format!("workshop {w}"),
            Kind::Inspector => "inspector".into(),
        }
    }

    pub fn sign(&self, app: &App) -> String {
        match self {
            Kind::App(a) => app
                .apps
                .iter()
                .find(|x| &x.name == a)
                .and_then(|x| x.icon.clone())
                .filter(|i| crate::icons::exists(i))
                .unwrap_or("app-window".into()),
            Kind::Network(_) => "network".into(),
            Kind::File(p) if p.extension().is_some_and(|x| x == "csv" || x == "tsv") => "sheet".into(),
            Kind::File(_) => "file-text".into(),
            Kind::Feelings => "heart-pulse".into(),
            Kind::Plots => "chart-line".into(),
            Kind::Clock => "clock-3".into(),
            Kind::Workshop(_) => "hammer".into(),
            Kind::Inspector => "search".into(),
        }
    }

    pub fn title(&self, app: &App) -> String {
        let lang = app.settings.lang;
        match self {
            Kind::App(a) => app.apps.iter().find(|x| &x.name == a).map(|x| x.title.clone()).unwrap_or(a.clone()),
            Kind::Network(n) if n == "all" => t(lang, "world_all").to_string(),
            Kind::Network(n) => n.clone(),
            Kind::File(p) => p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
            Kind::Feelings => t(lang, "feel").to_string(),
            Kind::Plots => t(lang, "m_plots").to_string(),
            Kind::Clock => t(lang, "clock").to_string(),
            Kind::Workshop(w) => format!("{} {w}", t(lang, "workshop")),
            Kind::Inspector => t(lang, "inspector").to_string(),
        }
    }
}

/// One thing on the table: what, where, how big; a network keeps its own way of being looked at.
pub struct Item {
    pub kind: Kind,
    pub rect: Rect,
    view: crate::view::View,
}

pub struct Space {
    pub items: Vec<Item>,
    /// What part of the table is in view.
    pub seen: Rect,
    /// The file it is written in, and when it was last read or written (an edit by hand is read again).
    path: Option<PathBuf>,
    stamp: Option<std::time::SystemTime>,
    pub dirty: bool,
}

impl Default for Space {
    fn default() -> Self {
        Space { items: vec![], seen: Rect::NOTHING, path: None, stamp: None, dirty: false }
    }
}

/// Where the space is written: beside the memory.
pub fn path_of(memory: &Path) -> PathBuf {
    memory.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.join("space.grm")).unwrap_or("space.grm".into())
}

fn number(s: Option<&str>) -> Option<f32> {
    s.and_then(|x| x.parse::<f32>().ok()).filter(|x| x.is_finite())
}

/// A kind from its word (as `Kind::word` writes it).
pub fn kind_of(word: &str) -> Option<Kind> {
    parse(&format!("place {word} at 0 0 size 200 200")).into_iter().next().map(|(k, _)| k)
}

/// Reads the lines of a space (what is not understood is left out).
pub fn parse(text: &str) -> Vec<(Kind, Rect)> {
    let mut out = vec![];
    for raw in text.lines() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix("place ") else { continue };
        let words = grm_shell::path_words(rest);
        let at = words.iter().position(|w| w == "at");
        let size = words.iter().position(|w| w == "size");
        let (Some(at), Some(size)) = (at, size) else { continue };
        let w = |k: usize| words.get(k).map(String::as_str);
        let (Some(x), Some(y), Some(wd), Some(ht)) =
            (number(w(at + 1)), number(w(at + 2)), number(w(size + 1)), number(w(size + 2)))
        else {
            continue;
        };
        let kind = match (w(0), w(1)) {
            (Some("app"), Some(a)) => Kind::App(a.to_string()),
            (Some("network"), Some(n)) => Kind::Network(n.to_string()),
            (Some("file"), Some(p)) => Kind::File(PathBuf::from(p)),
            (Some("feelings"), _) => Kind::Feelings,
            (Some("plots"), _) => Kind::Plots,
            (Some("clock"), _) => Kind::Clock,
            (Some("workshop"), Some(w)) => Kind::Workshop(w.to_string()),
            (Some("inspector"), _) => Kind::Inspector,
            _ => continue,
        };
        out.push((kind, Rect::from_min_size(pos2(x, y), vec2(wd.max(120.0), ht.max(80.0)))));
    }
    out
}

impl Space {
    fn text(&self) -> String {
        let mut s = String::from(
            "# The space: what is on the table, and where (written by the window; it can be written by hand).\n\
             # place app NAME | network SOURCE | network all | file PATH | feelings | plots | clock  at X Y  size W H\n",
        );
        for it in &self.items {
            s.push_str(&format!(
                "place {} at {:.0} {:.0} size {:.0} {:.0}\n",
                it.kind.word(),
                it.rect.left(),
                it.rect.top(),
                it.rect.width(),
                it.rect.height()
            ));
        }
        s
    }

    /// Read from its file (None: there is none yet).
    pub fn load(memory: &Path) -> Option<Space> {
        let path = path_of(memory);
        let text = std::fs::read_to_string(&path).ok()?;
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let items =
            parse(&text).into_iter().map(|(kind, rect)| Item { kind, rect, view: Default::default() }).collect();
        Some(Space { items, seen: Rect::NOTHING, path: Some(path), stamp, dirty: false })
    }

    pub fn save(&mut self, memory: &Path) {
        let path = path_of(memory);
        if std::fs::write(&path, self.text()).is_ok() {
            self.stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            self.path = Some(path);
            self.dirty = false;
        }
    }

    /// Written by hand meanwhile: read again (where each thing was looked at is kept).
    fn reread(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp.is_none() || stamp == self.stamp {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut old = std::mem::take(&mut self.items);
            for (kind, rect) in parse(&text) {
                let view = old.iter().position(|i| i.kind == kind).map(|k| old.remove(k).view).unwrap_or_default();
                self.items.push(Item { kind, rect, view });
            }
        }
        self.stamp = stamp;
    }

    /// Something put on the table, beside what is there (it is not put twice).
    pub fn put(&mut self, kind: Kind) {
        if self.items.iter().any(|i| i.kind == kind) {
            return;
        }
        let size = match &kind {
            Kind::Clock => vec2(220.0, 240.0),
            Kind::Feelings => vec2(340.0, 300.0),
            Kind::Network(_) => vec2(460.0, 360.0),
            Kind::Workshop(_) => vec2(900.0, 640.0),
            _ => vec2(480.0, 360.0),
        };
        let right = self.items.iter().map(|i| i.rect.right()).fold(0.0, f32::max);
        let top = self.items.iter().map(|i| i.rect.top()).fold(f32::INFINITY, f32::min);
        let at = if self.items.is_empty() {
            pos2(0.0, 0.0)
        } else {
            pos2(right + 24.0, if top.is_finite() { top } else { 0.0 })
        };
        self.items.push(Item { kind, rect: Rect::from_min_size(at, size), view: Default::default() });
        self.dirty = true;
    }

    /// Tidied: in rows, each as big as it is, in the order they came.
    pub fn tidy(&mut self) {
        let width = 1500.0;
        let (mut x, mut y, mut row) = (0.0f32, 0.0f32, 0.0f32);
        for it in &mut self.items {
            let s = it.rect.size();
            if x > 0.0 && x + s.x > width {
                x = 0.0;
                y += row + 24.0;
                row = 0.0;
            }
            it.rect = Rect::from_min_size(pos2(x, y), s);
            x += s.x + 24.0;
            row = row.max(s.y);
        }
        self.dirty = true;
        self.seen = Rect::NOTHING;
    }

    /// Everything in view.
    pub fn bounds(&self) -> Rect {
        self.items.iter().fold(Rect::NOTHING, |r, i| r.union(i.rect)).expand(40.0)
    }
}

/// What the space holds the first time: a clock, what it feels, the whole network, and each app open.
pub fn first(app: &App) -> Space {
    let mut s = Space::default();
    s.put(Kind::Clock);
    s.put(Kind::Network("all".into()));
    s.put(Kind::Feelings);
    for a in &app.apps {
        s.put(Kind::App(a.name.clone()));
    }
    s.tidy();
    s.dirty = true;
    s
}

const WHEEL_INSIDE: &str = "space-wheel-inside";

/// A list that scrolls, on the space: when the pointer is over it and it has more than it shows, the wheel is its
/// own (the table does not move under it).
pub fn wheel_to<R>(ui: &egui::Ui, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let more = out.content_size.y > out.inner_rect.height() + 1.0 || out.content_size.x > out.inner_rect.width() + 1.0;
    if more && ui.rect_contains_pointer(out.inner_rect) {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(WHEEL_INSIDE), true));
    }
}

/// The space, in the middle.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if crate::zone::center(app, ui) {
        return;
    }
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    app.space.reread();
    // its bar: put something on it, tidy it, everything in view, its code
    ui.horizontal(|ui| {
        egui::containers::menu::MenuButton::from_button(egui::Button::image_and_text(
            crate::icons::image("plus", 15.0, ui.visuals().text_color()),
            t(lang, "space_put"),
        ))
        .ui(ui, |ui| {
            ui.set_min_width(260.0);
            put_menu(app, ui);
        });
        if crate::icons::button(ui, "layout-grid", t(lang, "space_tidy"))
            .on_hover_text(t(lang, "space_tidy_d"))
            .clicked()
        {
            app.space.tidy();
        }
        if crate::icons::button(ui, "scan", t(lang, "fit")).clicked() {
            app.space.seen = Rect::NOTHING;
        }
        if crate::icons::button(ui, "file-code", t(lang, "space_code")).on_hover_text(t(lang, "space_code_d")).clicked()
        {
            app.space.save(&app.memory);
            let p = path_of(&app.memory);
            app.say(format!("edit {}", grm_shell::quoted(&p.display().to_string())));
        }
        ui.label(RichText::new(t(lang, "space_hint")).small().color(ink.muted));
    });
    ui.separator();
    if app.space.seen == Rect::NOTHING || !app.space.seen.is_finite() || app.space.seen.width() < 1.0 {
        app.space.seen = app.space.bounds();
    }
    let mut seen = app.space.seen;
    // only what is seen of the middle (the side may stand over the end of it)
    let back = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    ui.painter().rect_filled(back, 0.0, ink.surface);
    let mut table =
        ui.new_child(egui::UiBuilder::new().max_rect(back).layout(egui::Layout::top_down(egui::Align::Min)));
    table.set_clip_rect(back);
    let ui = &mut table;
    // a quiet dotted ground: where the table is
    let scene = egui::Scene::new().zoom_range(0.1..=2.0);
    let mut order: Option<usize> = None;
    let mut acts: Vec<(Kind, crate::zone::Act)> = vec![];
    // the wheel over a list that scrolls is the list's (it said so the frame before): the table does not move then
    let inside = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new(WHEEL_INSIDE))).unwrap_or(false);
    let held = if inside { ui.ctx().input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta)) } else { vec2(0.0, 0.0) };
    scene.show(ui, &mut seen, |ui| {
        if inside {
            ui.ctx().input_mut(|i| i.smooth_scroll_delta = held);
        }
        // nothing of the table is drawn outside its place on the screen (not over the side, not over the prompt)
        if let Some(to_screen) = ui.ctx().layer_transform_to_global(ui.layer_id()) {
            ui.set_clip_rect(to_screen.inverse() * back);
        }
        let n = app.space.items.len();
        for k in 0..n {
            match frame(app, ui, k) {
                Some(Act::Front) => order = Some(k),
                Some(Act::Zone(a)) => acts.push((app.space.items[k].kind.clone(), a)),
                None => {}
            }
        }
    });
    app.space.seen = seen;
    if let Some(k) = order {
        // the one touched comes on top
        let it = app.space.items.remove(k);
        app.space.items.push(it);
    }
    let ctx = ui.ctx().clone();
    for (kind, a) in acts {
        crate::zone::apply(app, &ctx, &kind, a);
    }
    if app.space.dirty && !ui.input(|i| i.pointer.any_down()) {
        app.space.save(&app.memory);
    }
}

enum Act {
    Front,
    Zone(crate::zone::Act),
}

fn put_menu(app: &mut App, ui: &mut egui::Ui) {
    // a long list scrolls (it never runs off the screen)
    let high = (ui.ctx().content_rect().height() * 0.7).max(200.0);
    egui::ScrollArea::vertical().max_height(high).auto_shrink([false, true]).show(ui, |ui| put_list(app, ui));
}

fn put_list(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.settings.lang;
    let mut put: Option<Kind> = None;
    ui.label(RichText::new(t(lang, "space_apps")).strong());
    let mut names: Vec<String> = app.apps.iter().map(|a| a.name.clone()).collect();
    // the apps that are not open yet: opened, then put
    if let Ok(d) = std::fs::read_dir(grm_shell::runtime::apps_dir(&app.memory)) {
        for e in d.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "grm") {
                let n = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
    }
    for n in names {
        // an app by its title (hers is her name, not the file it is written in)
        let title = app.apps.iter().find(|a| a.name == n).map(|a| a.title.clone()).unwrap_or_else(|| n.clone());
        if crate::icons::button(ui, "app-window", &title).clicked() {
            if !app.apps.iter().any(|a| a.name == n) {
                app.say(format!("app open {n}"));
            }
            put = Some(Kind::App(n));
        }
    }
    ui.separator();
    ui.label(RichText::new(t(lang, "space_networks")).strong());
    let mut sources: Vec<String> = vec!["all".into()];
    for n in &app.picture.nodes {
        if !sources.contains(&n.source) {
            sources.push(n.source.clone());
        }
    }
    for s in sources {
        let label = if s == "all" { t(lang, "world_all").to_string() } else { s.clone() };
        if crate::icons::button(ui, "network", label).clicked() {
            put = Some(Kind::Network(s));
        }
    }
    ui.separator();
    ui.label(RichText::new(t(lang, "workshops")).strong());
    for (w, _, _) in app.picture.workshops.clone() {
        if crate::icons::button(ui, "hammer", &w).clicked() {
            put = Some(Kind::Workshop(w));
        }
    }
    if crate::icons::button(ui, "circle-plus", t(lang, "ws_new")).on_hover_text(t(lang, "workshop_d")).clicked() {
        app.new_workshop_open = true;
        ui.close();
    }
    ui.separator();
    for (sign, key, kind) in [
        ("search", "inspector", Kind::Inspector),
        ("heart-pulse", "feel", Kind::Feelings),
        ("chart-line", "m_plots", Kind::Plots),
        ("clock-3", "clock", Kind::Clock),
    ] {
        if crate::icons::button(ui, sign, t(lang, key)).clicked() {
            put = Some(kind);
        }
    }
    for f in app.files.iter() {
        if crate::icons::button(ui, "file-text", f.name()).clicked() {
            put = Some(Kind::File(f.path.clone()));
        }
    }
    if let Some(k) = put {
        app.space.put(k);
        app.space.seen = Rect::NOTHING;
        ui.close();
    }
}

/// One thing on the table: its frame (moved by its title, made bigger by its corner), and what it shows.
fn frame(app: &mut App, ui: &mut egui::Ui, k: usize) -> Option<Act> {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let kind = app.space.items[k].kind.clone();
    let rect = app.space.items[k].rect;
    let mut act = None;
    let painter = ui.painter().clone();
    painter.rect_filled(rect.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(50));
    painter.rect_filled(rect, 4.0, ink.panel);
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ink.faint), egui::StrokeKind::Inside);
    // the title: it moves the frame; a double click opens it whole
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0));
    let id = egui::Id::new(("space-item", kind.word()));
    let r = ui.interact(bar, id.with("bar"), Sense::click_and_drag());
    if r.dragged() {
        let d = r.drag_delta();
        app.space.items[k].rect = rect.translate(d);
        app.space.dirty = true;
    }
    if r.drag_started() || r.clicked() {
        act = Some(Act::Front);
    }
    if r.double_clicked() {
        act = Some(Act::Zone(crate::zone::Act::Center));
    }
    let sign = kind.sign(app);
    let mut title_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar.shrink2(vec2(10.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    title_ui.set_clip_rect(bar.intersect(ui.clip_rect()));
    crate::icons::show(&mut title_ui, &sign, 15.0, ink.accent);
    title_ui.label(RichText::new(kind.title(app)).strong());
    title_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if let Some(a) = crate::zone::signs(ui, lang, crate::zone::Act::Space) {
            act = Some(Act::Zone(a));
        }
    });
    painter.line_segment(
        [pos2(rect.left() + 8.0, bar.bottom()), pos2(rect.right() - 8.0, bar.bottom())],
        Stroke::new(1.0_f32, ink.faint),
    );
    // the corner: bigger, smaller
    let corner = Rect::from_min_size(rect.max - vec2(18.0, 18.0), vec2(18.0, 18.0));
    let c = ui.interact(corner, id.with("corner"), Sense::drag());
    if c.hovered() || c.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if c.dragged() {
        let size = (rect.size() + c.drag_delta()).max(vec2(160.0, 110.0));
        app.space.items[k].rect = Rect::from_min_size(rect.min, size);
        app.space.dirty = true;
    }
    for i in 0..3 {
        let o = 4.0 + i as f32 * 4.0;
        painter.line_segment([rect.max - vec2(o, 3.0), rect.max - vec2(3.0, o)], Stroke::new(1.0_f32, ink.faint));
    }
    // what it shows
    let inner = Rect::from_min_max(pos2(rect.left() + 8.0, bar.bottom() + 6.0), rect.max - vec2(8.0, 10.0));
    let mut body = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .id_salt(("space-body", kind.word()))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    body.set_clip_rect(inner.intersect(ui.clip_rect()));
    // the whole frame, under what it shows: a double click where nothing else takes it makes it bigger
    let back = ui.interact(inner, id.with("back"), Sense::click());
    if back.double_clicked() {
        act = Some(Act::Zone(crate::zone::Act::Center));
    }
    let mut view = std::mem::take(&mut app.space.items[k].view);
    crate::zone::body(app, &mut body, &kind, &mut view, true);
    if let Some(it) = app.space.items.get_mut(k) {
        it.view = view;
    }
    act
}

/// A clock: a face with its hands (the seconds move slowly, without jumps), the time and the day in words.
pub fn clock(ui: &mut egui::Ui, ink: &Ink, lang: crate::text::Lang) {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let size = ui.available_size();
    let side = size.x.min(size.y - 44.0).max(60.0);
    let (rect, _) = ui.allocate_exact_size(vec2(size.x, side), Sense::hover());
    let p = ui.painter_at(rect);
    let c = rect.center();
    let r = side / 2.0 - 6.0;
    p.circle_filled(c, r, ink.surface);
    p.circle_stroke(c, r, Stroke::new(1.5_f32, ink.faint));
    for h in 0..12 {
        let a = h as f32 / 12.0 * std::f32::consts::TAU;
        let (s, cs) = a.sin_cos();
        let inner = if h % 3 == 0 { r * 0.80 } else { r * 0.87 };
        p.line_segment(
            [c + vec2(s, -cs) * inner, c + vec2(s, -cs) * (r * 0.95)],
            Stroke::new(if h % 3 == 0 { 2.0_f32 } else { 1.0_f32 }, ink.muted),
        );
    }
    let secs = now.second() as f32 + now.nanosecond() as f32 / 1e9;
    let mins = now.minute() as f32 + secs / 60.0;
    let hours = (now.hour() % 12) as f32 + mins / 60.0;
    let hand = |turn: f32, len: f32, width: f32, colour: Color32| {
        let a = turn * std::f32::consts::TAU;
        p.line_segment([c, c + vec2(a.sin(), -a.cos()) * len], Stroke::new(width, colour));
    };
    hand(hours / 12.0, r * 0.5, 3.5, ink.text);
    hand(mins / 60.0, r * 0.75, 2.5, ink.text);
    hand(secs / 60.0, r * 0.85, 1.0, ink.accent);
    p.circle_filled(c, 3.5, ink.accent);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(now.format("%H:%M").to_string()).size(22.0).strong());
        let days_fr = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
        let months_fr = [
            "janvier",
            "février",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "août",
            "septembre",
            "octobre",
            "novembre",
            "décembre",
        ];
        let day = if lang == crate::text::Lang::Fr {
            format!(
                "{} {} {} {}",
                days_fr[now.weekday().num_days_from_monday() as usize],
                now.day(),
                months_fr[now.month0() as usize],
                now.year()
            )
        } else {
            now.format("%A %-d %B %Y").to_string()
        };
        ui.label(RichText::new(day).small().color(ink.muted));
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_is_written_and_read_back_and_tidied() {
        let mut s = Space::default();
        s.put(Kind::Clock);
        s.put(Kind::App("weather".into()));
        s.put(Kind::Network("all".into()));
        s.put(Kind::File(PathBuf::from("/tmp/a b.csv")));
        s.put(Kind::Clock);
        assert_eq!(s.items.len(), 4);
        let back = parse(&s.text());
        assert_eq!(back.len(), 4);
        assert_eq!(back[1].0, Kind::App("weather".into()));
        assert_eq!(back[3].0, Kind::File(PathBuf::from("/tmp/a b.csv")));
        assert_eq!(back[0].1, s.items[0].rect);
        s.tidy();
        assert!(s.items.iter().all(|i| i.rect.left() >= 0.0 && i.rect.right() <= 1500.0 + 500.0));
        // what is not understood is left out
        assert_eq!(parse("place nothing at 1 2 size 3 4\nplace clock at x 2 size 3 4\n# a word").len(), 0);
    }
}
