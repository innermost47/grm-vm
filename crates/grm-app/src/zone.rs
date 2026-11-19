// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A zone: anything the window shows (an app, a network, a workshop, a file, plots, what it feels, a clock, the
//! inspector). Every zone can be four things: small on the space (a frame among others), in place of the space,
//! the whole screen, or a window of its own (a real window of the system, put on another screen if wished).

use std::collections::HashMap;

use eframe::egui::{self, RichText};

use crate::app::{App, Ink};
use crate::space::Kind;
use crate::text::t;

/// What can be done with a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Back on the space, small.
    Space,
    /// In place of the space.
    Center,
    /// The whole screen.
    Screen,
    /// A window of its own.
    Detach,
    /// Off the space.
    Remove,
}

/// The signs of a zone, on its title: what it can become (not what it is).
pub fn signs(ui: &mut egui::Ui, lang: crate::text::Lang, now: Act) -> Option<Act> {
    let mut act = None;
    if now == Act::Space && crate::icons::sign(ui, "x", t(lang, "space_take_off")).clicked() {
        act = Some(Act::Remove);
    }
    if now != Act::Detach && crate::icons::sign(ui, "picture-in-picture-2", t(lang, "zone_detach")).clicked() {
        act = Some(Act::Detach);
    }
    if now != Act::Screen && crate::icons::sign(ui, "maximize", t(lang, "zone_screen")).clicked() {
        act = Some(Act::Screen);
    }
    if now != Act::Center && crate::icons::sign(ui, "maximize-2", t(lang, "space_full")).clicked() {
        act = Some(Act::Center);
    }
    if now != Act::Space && crate::icons::sign(ui, "minimize-2", t(lang, "zone_back")).clicked() {
        act = Some(Act::Space);
    }
    act
}

/// A zone becomes what was asked.
pub fn apply(app: &mut App, ctx: &egui::Context, kind: &Kind, act: Act) {
    // wherever it was, it is there no more
    if app.zone_center.as_ref() == Some(kind) {
        app.zone_center = None;
    }
    if app.zone_screen.as_ref() == Some(kind) {
        app.zone_screen = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
    }
    app.detached.retain(|k| k != kind);
    match act {
        Act::Space => {
            app.space.put(kind.clone());
            app.go(crate::front::Front::Space);
        }
        Act::Center => {
            app.space.put(kind.clone());
            app.zone_center = Some(kind.clone());
            app.go(crate::front::Front::Space);
        }
        Act::Screen => {
            app.zone_screen = Some(kind.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        Act::Detach => app.detached.push(kind.clone()),
        Act::Remove => {
            if let Some(k) = app.space.items.iter().position(|i| &i.kind == kind) {
                app.space.items.remove(k);
                app.space.dirty = true;
            }
        }
    }
}

/// What a zone shows (in whatever place it is). `small`: on the space (the wheel is the space's, but over a list
/// that scrolls: the list's).
pub fn body(app: &mut App, ui: &mut egui::Ui, kind: &Kind, view: &mut crate::view::View, small: bool) {
    let lang = app.settings.lang;
    match kind {
        Kind::App(a) => {
            if app.apps.iter().any(|x| &x.name == a) {
                let editing = if small { std::mem::replace(&mut app.editing, false) } else { app.editing };
                if a == crate::elle::APP {
                    crate::elle::panel(app, ui);
                } else if a == crate::player::APP {
                    crate::player::panel(app, ui);
                } else if a == crate::universe::APP {
                    crate::universe::panel(app, ui);
                } else {
                    let out = egui::ScrollArea::vertical()
                        .id_salt(("zone-app", a, small))
                        .show(ui, |ui| crate::surface::show(app, ui, a));
                    crate::space::wheel_to(ui, &out);
                }
                app.editing = editing;
            } else {
                // a closed app: said so, quietly, in the middle, with what opens it again (or takes it off the space)
                let ink = crate::app::Ink::of(ui.ctx());
                ui.add_space((ui.available_height() / 2.0 - 50.0).max(8.0));
                ui.vertical_centered(|ui| {
                    crate::icons::show(ui, "app-window", 26.0, ink.faint);
                    ui.label(egui::RichText::new(a).strong().color(ink.muted));
                    ui.label(egui::RichText::new(t(lang, "space_app_closed")).small().color(ink.faint));
                    ui.add_space(6.0);
                    if crate::icons::button(ui, "play", t(lang, "space_open_app")).clicked() {
                        app.say(format!("app open {a}"));
                    }
                });
            }
        }
        Kind::Network(n) => {
            std::mem::swap(&mut app.view, view);
            app.view.passive = small;
            let only = if n == "all" { None } else { Some(n.as_str()) };
            crate::view::show(app, ui, only);
            app.view.passive = false;
            std::mem::swap(&mut app.view, view);
        }
        Kind::Workshop(w) => crate::screen::workshop(app, ui, w, view, small),
        Kind::File(p) => {
            let k = match app.files.iter().position(|f| &f.path == p) {
                Some(k) => k,
                None => {
                    app.files.push(crate::files::FileTab::open(p.clone()));
                    app.files.len() - 1
                }
            };
            crate::files::show(app, ui, k);
        }
        Kind::Feelings => {
            let out = egui::ScrollArea::vertical().id_salt(("zone-feel", small)).show(ui, |ui| {
                crate::panels::feelings(app, ui);
                crate::panels::witness(app, ui);
            });
            crate::space::wheel_to(ui, &out);
        }
        Kind::Plots => crate::plot::show(app, ui, None),
        Kind::Clock => crate::space::clock(ui, &Ink::of(ui.ctx()), lang),
        Kind::Inspector => {
            let out = egui::ScrollArea::vertical()
                .id_salt(("zone-inspector", small))
                .show(ui, |ui| crate::tree::show(app, ui));
            crate::space::wheel_to(ui, &out);
        }
    }
}

/// Each zone shown outside the space keeps its own way of being looked at.
fn view_of(views: &mut HashMap<String, crate::view::View>, kind: &Kind) -> crate::view::View {
    views.remove(&kind.word()).unwrap_or_default()
}

/// The title of a zone: its sign, its name, its signs on the right.
fn title(app: &App, ui: &mut egui::Ui, kind: &Kind, now: Act) -> Option<Act> {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let mut act = None;
    ui.horizontal(|ui| {
        crate::icons::show(ui, &kind.sign(app), 16.0, ink.accent);
        ui.label(RichText::new(kind.title(app)).strong().size(crate::style::size(crate::style::Size::Subtitle)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| act = signs(ui, lang, now));
    });
    ui.separator();
    act
}

/// A zone in place of the space (true when there is one).
pub fn center(app: &mut App, ui: &mut egui::Ui) -> bool {
    let Some(kind) = app.zone_center.clone() else { return false };
    let act = title(app, ui, &kind, Act::Center);
    let mut view = view_of(&mut app.zone_views, &kind);
    body(app, ui, &kind, &mut view, false);
    app.zone_views.insert(kind.word(), view);
    if let Some(a) = act {
        let ctx = ui.ctx().clone();
        apply(app, &ctx, &kind, a);
    }
    true
}

/// A zone on the whole screen (true when there is one: nothing else is drawn).
pub fn screen(app: &mut App, ctx: &egui::Context) -> bool {
    let Some(kind) = app.zone_screen.clone() else { return false };
    let mut act = None;
    egui::CentralPanel::default().show(ctx, |ui| {
        act = title(app, ui, &kind, Act::Screen);
        let mut view = view_of(&mut app.zone_views, &kind);
        body(app, ui, &kind, &mut view, false);
        app.zone_views.insert(kind.word(), view);
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        act = Some(Act::Space);
    }
    if let Some(a) = act {
        apply(app, ctx, &kind, a);
    }
    true
}

/// The zones in windows of their own (each a real window; closing it puts the zone back on the space).
pub fn detached(app: &mut App, ctx: &egui::Context) {
    let mut acts: Vec<(Kind, Act)> = vec![];
    for kind in app.detached.clone() {
        let id = egui::ViewportId::from_hash_of(("zone", kind.word()));
        let builder = egui::ViewportBuilder::default()
            .with_title(format!("grm-vm · {}", kind.title(app)))
            .with_inner_size([760.0, 560.0])
            .with_app_id("grm-vm");
        ctx.show_viewport_immediate(id, builder, |ctx, _class| {
            let mut act = None;
            egui::CentralPanel::default().show(ctx, |ui| {
                act = title(app, ui, &kind, Act::Detach);
                let mut view = view_of(&mut app.zone_views, &kind);
                body(app, ui, &kind, &mut view, false);
                app.zone_views.insert(kind.word(), view);
            });
            if ctx.input(|i| i.viewport().close_requested()) {
                act = Some(Act::Space);
            }
            if let Some(a) = act {
                acts.push((kind.clone(), a));
            }
        });
    }
    for (kind, a) in acts {
        apply(app, ctx, &kind, a);
    }
}
