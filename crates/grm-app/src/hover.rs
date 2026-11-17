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

/// A line with its sign.
fn row(ui: &mut egui::Ui, sign: &str, text: RichText, colour: Color32) {
    ui.horizontal(|ui| {
        crate::icons::show(ui, sign, 13.0, colour);
        ui.label(text);
    });
}

/// A small heading inside the card.
fn heading(ui: &mut egui::Ui, sign: &str, text: &str, ink: &Ink) {
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        crate::icons::show(ui, sign, 12.0, ink.muted);
        ui.label(RichText::new(text.to_uppercase()).small().strong().color(ink.muted));
    });
}

/// What a node is.
pub fn node(app: &App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(n) = app.picture.node(name) else { return };
    let k = app.colours.get(&n.source).copied().unwrap_or(0);
    ui.set_max_width(320.0);
    ui.horizontal(|ui| {
        crate::icons::show(ui, if n.observed { "circle-dot" } else { "circle" }, 16.0, ink.source(k));
        ui.label(RichText::new(&n.name).strong().size(15.0).color(ink.source(k)));
    });
    ui.label(RichText::new(reading(n.value, n.sigma, n.signed)).monospace().size(15.0));
    ui.horizontal_wrapped(|ui| {
        let mut what =
            vec![if n.observed { ("eye", t(lang, "observed")) } else { ("calculator", t(lang, "computed")) }];
        for (on, sign, key) in [
            (n.fixed, "pin", "fixed"),
            (n.var, "variable", "var"),
            (n.learn, "graduation-cap", "learn"),
            (n.input, "log-in", "input"),
            (n.output, "log-out", "output"),
            (n.tracked, "timer", "tracked"),
        ] {
            if on {
                what.push((sign, t(lang, key)));
            }
        }
        for (sign, w) in what {
            crate::icons::show(ui, sign, 12.0, ink.muted);
            ui.label(RichText::new(w).small().color(ink.muted));
        }
    });
    row(ui, "waypoints", RichText::new(&n.source).small().color(ink.source(k)), ink.source(k));
    let rels: Vec<&crate::model::RelationView> =
        app.picture.relations.iter().filter(|r| r.nodes.contains(&n.name)).collect();
    if !rels.is_empty() {
        heading(ui, "link", t(lang, "card_relations"), &ink);
        for r in rels.iter().take(6) {
            let colour = if r.tension > 1.0 { ink.amber } else { ink.text };
            ui.horizontal_wrapped(|ui| {
                crate::icons::show(ui, "diamond", 11.0, colour);
                ui.label(RichText::new(&r.name).small().strong().color(colour));
                crate::math::show(ui, &r.text, 12.0, lang);
            });
        }
    }
    // its curve: the wave it plays, or how it moved lately
    heading(ui, "activity", t(lang, "card_moves"), &ink);
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
        row(
            ui,
            "file-code",
            RichText::new(format!("{a}, {} {l}", t(lang, "line"))).small().color(ink.muted),
            ink.muted,
        );
    }
}

/// What a relation is.
pub fn relation(app: &App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(r) = app.picture.relations.iter().find(|r| r.name == name) else { return };
    ui.set_max_width(340.0);
    let strained = r.tension > 1.0;
    ui.horizontal(|ui| {
        crate::icons::show(ui, "diamond", 16.0, if strained { ink.amber } else { ink.accent });
        ui.label(RichText::new(&r.name).strong().size(15.0));
    });
    // what it says, as mathematics
    crate::math::show(ui, &r.text, 15.0, lang);
    // how it is: where from, how far from holding, awake or not, how far in
    ui.horizontal_wrapped(|ui| {
        let k = app.colours.get(&r.source).copied().unwrap_or(0);
        crate::icons::show(ui, "waypoints", 12.0, ink.source(k));
        ui.label(RichText::new(&r.source).small().color(ink.source(k)));
        crate::icons::show(ui, "activity", 12.0, if strained { ink.amber } else { ink.muted });
        ui.label(RichText::new(format!("{} {:.2} σ", t(lang, "strain"), r.tension)).small().color(if strained {
            ink.amber
        } else {
            ink.muted
        }));
        if !r.awake {
            crate::icons::show(ui, "moon", 12.0, ink.muted);
            ui.label(RichText::new(t(lang, "asleep")).small().color(ink.muted));
        }
        if r.strength < 0.999 {
            crate::icons::show(ui, "trending-up", 12.0, ink.muted);
            ui.label(
                RichText::new(format!("{} {:.0} %", t(lang, "integrating"), r.strength * 100.0))
                    .small()
                    .color(ink.muted),
            );
        }
    });
    // what it is worth: what it brings against the rent of its links
    let pays = r.bits >= r.rent;
    heading(ui, "coins", t(lang, "card_worth"), &ink);
    ui.horizontal_wrapped(|ui| {
        crate::icons::show(
            ui,
            if pays { "circle-check" } else { "circle-alert" },
            12.0,
            if pays { ink.source(2) } else { ink.amber },
        );
        ui.label(
            RichText::new(format!(
                "{} {} bits · {} {} · {}",
                t(lang, "brings"),
                grm_engine::lang::g((r.bits * 100.0).round() / 100.0),
                t(lang, "rent"),
                grm_engine::lang::g(r.rent),
                if pays { t(lang, "pays_rent") } else { t(lang, "no_rent") }
            ))
            .small()
            .color(if pays { ink.muted } else { ink.amber }),
        );
    });
    // the nodes it holds, once each
    heading(ui, "circle-dot", t(lang, "card_nodes"), &ink);
    let mut seen: Vec<&String> = vec![];
    for n in r.nodes.iter().filter(|n| !n.contains("__next")) {
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        if let Some(v) = app.picture.node(n) {
            let k = app.colours.get(&v.source).copied().unwrap_or(0);
            ui.horizontal(|ui| {
                crate::icons::show(ui, if v.observed { "circle-dot" } else { "circle" }, 11.0, ink.source(k));
                ui.label(RichText::new(n).small().monospace().color(ink.source(k)));
                ui.label(RichText::new(reading(v.value, v.sigma, v.signed)).small().monospace());
            });
        }
    }
    heading(ui, "activity", t(lang, "card_strain_lately"), &ink);
    let v: Vec<f32> = app.traces.get(&format!("rel:{name}")).map(|t| t.iter().copied().collect()).unwrap_or_default();
    curve(ui, &v, ink.amber, &ink, t(lang, "its_strain"));
    if let Some((a, l)) = app.line_in_app(name) {
        row(
            ui,
            "file-code",
            RichText::new(format!("{a}, {} {l}", t(lang, "line"))).small().color(ink.muted),
            ink.muted,
        );
    }
}
