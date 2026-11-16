// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What a node or a relation is, shown when the pointer rests on it: its value and spread, where it comes from,
//! its equation, and its curve as it moves now (a node that plays: its wave, as it sounds).

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke};

use crate::app::{App, Ink};
use crate::text::t;
use crate::view::reading;

/// How many moments a trace keeps.
pub const TRACE: usize = 240;

/// A small moving curve.
pub fn curve(ui: &mut egui::Ui, values: &[f32], colour: Color32, ink: &Ink, label: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(260.0, 64.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, ink.surface);
    if values.len() < 2 {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(12.0), ink.muted);
        return;
    }
    let (mut lo, mut hi) = values.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
    if !(lo.is_finite() && hi.is_finite()) {
        return;
    }
    if hi - lo < 1e-9 {
        lo -= 1.0;
        hi += 1.0;
    }
    let inner = rect.shrink(6.0);
    let n = values.len() as f32 - 1.0;
    let pts: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(k, v)| {
            pos2(inner.left() + k as f32 / n * inner.width(), inner.bottom() - (v - lo) / (hi - lo) * inner.height())
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5_f32, colour)));
    p.text(
        rect.left_top() + vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(10.0),
        ink.muted,
    );
    p.text(
        rect.right_top() + vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        grm_engine::lang::g(hi as f64),
        egui::FontId::monospace(10.0),
        ink.muted,
    );
    p.text(
        rect.right_bottom() + vec2(-4.0, -2.0),
        egui::Align2::RIGHT_BOTTOM,
        grm_engine::lang::g(lo as f64),
        egui::FontId::monospace(10.0),
        ink.muted,
    );
}

/// Keeps each node's and relation's recent values (a curve to show, moving).
pub fn remember(app: &mut App) {
    for n in &app.picture.nodes {
        let tr = app.traces.entry(n.name.clone()).or_default();
        tr.push_back(n.value as f32);
        while tr.len() > TRACE {
            tr.pop_front();
        }
    }
    for r in &app.picture.relations {
        let tr = app.traces.entry(format!("rel:{}", r.name)).or_default();
        tr.push_back(r.tension as f32);
        while tr.len() > TRACE {
            tr.pop_front();
        }
    }
}

/// What a node is.
pub fn node(app: &App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(n) = app.picture.node(name) else { return };
    let k = app.colours.get(&n.source).copied().unwrap_or(0);
    ui.label(RichText::new(&n.name).strong().size(15.0).color(ink.source(k)));
    ui.label(RichText::new(reading(n.value, n.sigma, n.signed)).monospace().size(14.0));
    let mut what = vec![if n.observed { t(lang, "observed") } else { t(lang, "computed") }];
    for (on, key) in [
        (n.fixed, "fixed"),
        (n.var, "var"),
        (n.learn, "learn"),
        (n.input, "input"),
        (n.output, "output"),
        (n.tracked, "tracked"),
    ] {
        if on {
            what.push(t(lang, key));
        }
    }
    ui.label(
        RichText::new(format!("{} · {} {}", what.join(" · "), t(lang, "source"), n.source)).small().color(ink.muted),
    );
    for r in app.picture.relations.iter().filter(|r| r.nodes.contains(&n.name)) {
        let colour = if r.tension > 1.0 { ink.amber } else { ink.text };
        ui.label(RichText::new(format!("{}: {}", r.name, r.text)).monospace().small().color(colour));
    }
    // its curve: the wave it plays, or how it moved lately
    let scope = app.mixer.lock().ok().and_then(|m| m.scope(name));
    match scope {
        Some(s) => {
            let show: Vec<f32> = s[s.len() - 512..].to_vec();
            curve(ui, &show, ink.accent, &ink, t(lang, "as_it_sounds"));
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
        None => {
            let v: Vec<f32> = app.traces.get(name).map(|t| t.iter().copied().collect()).unwrap_or_default();
            curve(ui, &v, ink.source(k), &ink, t(lang, "lately"));
        }
    }
    if let Some((a, l)) = app.line_in_app(name) {
        ui.label(RichText::new(format!("{a}, {} {l}", t(lang, "line"))).small().color(ink.muted));
    }
}

/// What a relation is.
pub fn relation(app: &App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(r) = app.picture.relations.iter().find(|r| r.name == name) else { return };
    ui.label(RichText::new(&r.name).strong().size(15.0));
    ui.label(RichText::new(&r.text).monospace().size(14.0).color(if r.tension > 1.0 { ink.amber } else { ink.text }));
    let mut state = format!("{} · {} {:.2} σ", r.source, t(lang, "strain"), r.tension);
    if !r.awake {
        state.push_str(&format!(" · {}", t(lang, "asleep")));
    }
    if r.strength < 0.999 {
        state.push_str(&format!(" · {} {:.0} %", t(lang, "integrating"), r.strength * 100.0));
    }
    ui.label(RichText::new(state).small().color(ink.muted));
    // what it is worth: what it brings against the rent of its links
    let pays = r.bits >= r.rent;
    ui.label(
        RichText::new(format!(
            "{} {} bits · {} {} · {}",
            t(lang, "brings"),
            grm_engine::lang::g(r.bits),
            t(lang, "rent"),
            grm_engine::lang::g(r.rent),
            if pays { t(lang, "pays_rent") } else { t(lang, "no_rent") }
        ))
        .small()
        .color(if pays { ink.muted } else { ink.amber }),
    );
    for n in r.nodes.iter().filter(|n| !n.contains("__next")) {
        if let Some(v) = app.picture.node(n) {
            ui.label(RichText::new(format!("{n} = {}", reading(v.value, v.sigma, v.signed))).monospace().small());
        }
    }
    let v: Vec<f32> = app.traces.get(&format!("rel:{name}")).map(|t| t.iter().copied().collect()).unwrap_or_default();
    curve(ui, &v, ink.amber, &ink, t(lang, "its_strain"));
    if let Some((a, l)) = app.line_in_app(name) {
        ui.label(RichText::new(format!("{a}, {} {l}", t(lang, "line"))).small().color(ink.muted));
    }
}
