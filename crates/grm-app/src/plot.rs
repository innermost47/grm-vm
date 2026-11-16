// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A network as plots: its nodes through time (one track each: they are not on the same scale, so they are not
//! drawn on one axis), one against another (a scatter, or a path through time: a phase), or their values now with
//! their spread (bars). The curves move with the world.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke};

use crate::app::{App, Ink};
use crate::text::t;
use crate::view::reading;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lines,
    Scatter,
    Phase,
    Bars,
}

const KINDS: [(Kind, &str); 4] =
    [(Kind::Lines, "p_lines"), (Kind::Scatter, "p_scatter"), (Kind::Phase, "p_phase"), (Kind::Bars, "p_bars")];

fn span(v: &[f32]) -> (f32, f32) {
    let (mut lo, mut hi) =
        v.iter().filter(|x| x.is_finite()).fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
    if !(lo.is_finite() && hi.is_finite()) {
        return (0.0, 1.0);
    }
    if hi - lo < 1e-9 {
        lo -= 0.5;
        hi += 0.5;
    }
    (lo, hi)
}

pub fn show(app: &mut App, ui: &mut egui::Ui, only: Option<&str>) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    // what can be plotted: the nodes shown here
    let names: Vec<String> = app
        .picture
        .nodes
        .iter()
        .filter(|n| only.is_none_or(|s| n.source == s) && !n.name.contains("__next"))
        .map(|n| n.name.clone())
        .collect();
    app.plotted.retain(|p| app.picture.node(p).is_some());
    if app.plotted.is_empty() {
        let first: Vec<String> =
            if !app.selected.is_empty() { app.selected.clone() } else { names.iter().take(4).cloned().collect() };
        app.plotted = first;
    }
    ui.horizontal_wrapped(|ui| {
        for (k, key) in KINDS {
            if crate::style::chosen(ui, app.plot_kind == k, t(lang, key)).clicked() {
                app.plot_kind = k;
            }
        }
        ui.separator();
        for p in app.plotted.clone() {
            let k = app.picture.node(&p).map(|n| app.colours.get(&n.source).copied().unwrap_or(0)).unwrap_or(0);
            if ui
                .button(RichText::new(format!("{p} ×")).color(ink.source(k)))
                .on_hover_text(t(lang, "p_remove"))
                .clicked()
            {
                app.plotted.retain(|x| *x != p);
            }
        }
        egui::ComboBox::from_id_salt("plot-add").selected_text(t(lang, "p_add")).show_ui(ui, |ui| {
            for n in &names {
                if !app.plotted.contains(n) && ui.selectable_label(false, n).clicked() {
                    app.plotted.push(n.clone());
                }
            }
        });
        if !app.selected.is_empty() && ui.button(t(lang, "p_selected")).clicked() {
            app.plotted = app.selected.clone();
        }
    });
    let rect = ui.available_rect_before_wrap();
    let (resp, painter) = ui.allocate_painter(rect.size(), Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, ink.surface);
    let series: Vec<(String, Vec<f32>, Color32)> = app
        .plotted
        .iter()
        .map(|p| {
            let v: Vec<f32> = app.traces.get(p).map(|t| t.iter().copied().collect()).unwrap_or_default();
            let k = app.picture.node(p).map(|n| app.colours.get(&n.source).copied().unwrap_or(0)).unwrap_or(0);
            (p.clone(), v, ink.source(k))
        })
        .collect();
    if series.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            t(lang, "p_nothing"),
            egui::FontId::proportional(14.0),
            ink.muted,
        );
        return;
    }
    let font = egui::FontId::proportional(11.0);
    match app.plot_kind {
        Kind::Lines => {
            // small multiples: one track each, its own scale
            let h = rect.height() / series.len() as f32;
            for (k, (name, v, colour)) in series.iter().enumerate() {
                let band = Rect::from_min_size(pos2(rect.left(), rect.top() + k as f32 * h), vec2(rect.width(), h));
                let inner = Rect::from_min_max(
                    pos2(band.left() + 120.0, band.top() + 14.0),
                    pos2(band.right() - 80.0, band.bottom() - 14.0),
                );
                painter.line_segment(
                    [pos2(band.left(), band.bottom()), pos2(band.right(), band.bottom())],
                    Stroke::new(0.5_f32, ink.faint),
                );
                painter.text(
                    band.left_top() + vec2(8.0, 6.0),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::proportional(13.0),
                    *colour,
                );
                if let Some(n) = app.picture.node(name) {
                    painter.text(
                        band.left_top() + vec2(8.0, 24.0),
                        egui::Align2::LEFT_TOP,
                        reading(n.value, n.sigma, n.signed),
                        font.clone(),
                        ink.muted,
                    );
                }
                if v.len() < 2 {
                    continue;
                }
                let (lo, hi) = span(v);
                let x = |i: usize| inner.left() + i as f32 / (v.len() - 1) as f32 * inner.width();
                let y = |val: f32| inner.bottom() - (val - lo) / (hi - lo) * inner.height();
                let pts: Vec<Pos2> = v.iter().enumerate().map(|(i, val)| pos2(x(i), y(*val))).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, *colour)));
                painter.text(
                    pos2(inner.right() + 6.0, inner.top()),
                    egui::Align2::LEFT_TOP,
                    grm_engine::lang::g(hi as f64),
                    font.clone(),
                    ink.muted,
                );
                painter.text(
                    pos2(inner.right() + 6.0, inner.bottom()),
                    egui::Align2::LEFT_BOTTOM,
                    grm_engine::lang::g(lo as f64),
                    font.clone(),
                    ink.muted,
                );
            }
        }
        Kind::Scatter | Kind::Phase => {
            if series.len() < 2 {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    t(lang, "p_two"),
                    egui::FontId::proportional(14.0),
                    ink.muted,
                );
                return;
            }
            let (xs, ys) = (&series[0].1, &series[1].1);
            let n = xs.len().min(ys.len());
            let inner = rect.shrink2(vec2(60.0, 40.0));
            let (xl, xh) = span(&xs[xs.len() - n..]);
            let (yl, yh) = span(&ys[ys.len() - n..]);
            let at = |k: usize| {
                let (xv, yv) = (xs[xs.len() - n + k], ys[ys.len() - n + k]);
                pos2(
                    inner.left() + (xv - xl) / (xh - xl) * inner.width(),
                    inner.bottom() - (yv - yl) / (yh - yl) * inner.height(),
                )
            };
            painter.rect_stroke(inner, 0.0, Stroke::new(0.5_f32, ink.faint), egui::StrokeKind::Inside);
            painter.text(
                pos2(inner.center().x, inner.bottom() + 8.0),
                egui::Align2::CENTER_TOP,
                &series[0].0,
                egui::FontId::proportional(13.0),
                series[0].2,
            );
            painter.text(
                pos2(inner.left() - 8.0, inner.center().y),
                egui::Align2::RIGHT_CENTER,
                &series[1].0,
                egui::FontId::proportional(13.0),
                series[1].2,
            );
            painter.text(
                inner.left_bottom() + vec2(0.0, 4.0),
                egui::Align2::LEFT_TOP,
                grm_engine::lang::g(xl as f64),
                font.clone(),
                ink.muted,
            );
            painter.text(
                inner.right_bottom() + vec2(0.0, 4.0),
                egui::Align2::RIGHT_TOP,
                grm_engine::lang::g(xh as f64),
                font.clone(),
                ink.muted,
            );
            painter.text(
                inner.left_top() + vec2(-6.0, 0.0),
                egui::Align2::RIGHT_TOP,
                grm_engine::lang::g(yh as f64),
                font.clone(),
                ink.muted,
            );
            painter.text(
                inner.left_bottom() + vec2(-6.0, 0.0),
                egui::Align2::RIGHT_BOTTOM,
                grm_engine::lang::g(yl as f64),
                font.clone(),
                ink.muted,
            );
            if app.plot_kind == Kind::Phase && n > 1 {
                let pts: Vec<Pos2> = (0..n).map(at).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, ink.accent.gamma_multiply(0.7))));
            }
            for k in 0..n {
                // the older, the fainter
                let a = 0.2 + 0.8 * k as f32 / n.max(1) as f32;
                painter.circle_filled(at(k), 3.0, ink.accent.gamma_multiply(a));
            }
        }
        Kind::Bars => {
            let h = (rect.height() / series.len() as f32).min(48.0);
            let left = rect.left() + 150.0;
            let width = rect.width() - 220.0;
            let values: Vec<(f64, f64)> = series
                .iter()
                .map(|(n, _, _)| {
                    app.picture
                        .node(n)
                        .map(|v| {
                            (v.value, if v.signed { v.sigma } else { v.value * (10f64.powf(v.sigma.min(3.0)) - 1.0) })
                        })
                        .unwrap_or((0.0, 0.0))
                })
                .collect();
            let top = values.iter().map(|(v, _)| v.abs()).fold(0.0f64, f64::max).max(1e-12);
            let zero = if values.iter().any(|(v, _)| *v < 0.0) { left + width / 2.0 } else { left };
            let scale = if zero > left { width as f64 / 2.0 / top } else { width as f64 / top };
            for (k, ((name, _, colour), (v, s))) in series.iter().zip(values).enumerate() {
                let y = rect.top() + 12.0 + k as f32 * h;
                painter.text(
                    pos2(rect.left() + 10.0, y + h / 2.0),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(13.0),
                    *colour,
                );
                let end = zero + (v * scale) as f32;
                let bar = Rect::from_two_pos(pos2(zero, y + 6.0), pos2(end, y + h - 8.0));
                painter.rect_filled(bar, 4.0, colour.gamma_multiply(0.8));
                // its spread
                if s.is_finite() && s * scale < width as f64 {
                    let (a, b) = (zero + ((v - s) * scale) as f32, zero + ((v + s) * scale) as f32);
                    painter.line_segment([pos2(a, y + h / 2.0), pos2(b, y + h / 2.0)], Stroke::new(1.5_f32, ink.text));
                }
                painter.text(
                    pos2(left + width + 10.0, y + h / 2.0),
                    egui::Align2::LEFT_CENTER,
                    grm_engine::lang::g(v),
                    font.clone(),
                    ink.muted,
                );
            }
            painter.line_segment([pos2(zero, rect.top()), pos2(zero, rect.bottom())], Stroke::new(1.0_f32, ink.faint));
        }
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
}
