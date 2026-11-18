// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Her home's surface: her reverie (three veils of colour, read from her network: `mind/reverie.grm`), and a few
//! words of what she hears and sees. The window computes nothing of it: it only paints her nodes.

use eframe::egui::{self as egui};
use eframe::egui::{pos2, vec2, Color32, RichText, Sense};

use crate::app::{App, Ink};
use crate::text::t;

/// Her app (`mind/home.grm`).
pub const APP: &str = "home";

/// A colour of OKLab (lightness, green to red, blue to yellow) in sRGB.
pub fn oklab(l: f64, a: f64, b: f64, alpha: f32) -> Color32 {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3;
    let gamma = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        let y = if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        (y * 255.0).round() as u8
    };
    Color32::from_rgba_unmultiplied(gamma(r), gamma(g), gamma(bl), (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Whether this app is hers (always there: it neither closes nor goes to the trash).
pub fn always(app: &str) -> bool {
    app == APP
}

/// Her reverie, painted in a rectangle: a dark ground of her mood, three soft veils where her state is.
pub fn reverie(app: &App, ui: &mut egui::Ui, size: egui::Vec2) -> bool {
    // all she shows, read in one look (the sound is not held up a score of times each frame)
    const NAMES: [&str; 21] = [
        "rv_l2",
        "rv_a2",
        "rv_b2",
        "rv_x1",
        "rv_y1",
        "rv_r1",
        "rv_l1",
        "rv_a1",
        "rv_b1",
        "rv_x2",
        "rv_y2",
        "rv_r2",
        "rv_x3",
        "rv_y3",
        "rv_r3",
        "rv_l3",
        "rv_a3",
        "rv_b3",
        "rv_sglow",
        "elle_pleasure",
        "elle_seen_n",
    ];
    let seen = app.mixer.lock().map(|m| m.world_values(&NAMES)).unwrap_or_default();
    let read = |n: &str| NAMES.iter().position(|x| *x == n).and_then(|k| seen.get(k).copied().flatten());
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let Some(l2) = read("rv_l2") else {
        ui.painter().rect_filled(rect, 8.0, Color32::from_gray(12));
        return false;
    };
    let v = |n: &str| read(n).unwrap_or(0.0);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, oklab(0.12 + 0.3 * l2 * 0.4, v("rv_a2") * 0.4, v("rv_b2") * 0.4, 1.0));
    let span = rect.width().min(rect.height());
    // the dream under, then her mood, then now on top: each a soft round veil (rings fading outwards)
    for k in [3, 2, 1] {
        let (x, y, r) = (v(&format!("rv_x{k}")), v(&format!("rv_y{k}")), v(&format!("rv_r{k}")));
        let (l, a, b) = (v(&format!("rv_l{k}")), v(&format!("rv_a{k}")), v(&format!("rv_b{k}")));
        let centre = pos2(
            rect.left() + rect.width() * x.clamp(0.05, 0.95) as f32,
            rect.top() + rect.height() * y.clamp(0.05, 0.95) as f32,
        );
        let radius = span * r.clamp(0.08, 0.6) as f32;
        const RINGS: usize = 28;
        for i in 0..RINGS {
            let f = 1.0 - i as f32 / RINGS as f32;
            let alpha = 0.055 * (1.0 - f).powf(0.6) + 0.01;
            painter.circle_filled(centre, radius * f, oklab(l, a, b, alpha));
        }
    }
    true
}

/// Her home's surface.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let mixer = app.mixer.clone();
    let read = |n: &str| mixer.lock().ok().and_then(|m| m.world_value(n));
    ui.horizontal(|ui| {
        crate::icons::show(ui, "sparkles", 16.0, ink.accent);
        ui.label(RichText::new(t(lang, "elle_home")).strong());
        ui.label(RichText::new(t(lang, "elle_home_d")).small().color(ink.muted));
    });
    let w = ui.available_width().max(200.0);
    let h = (ui.available_height() - 60.0).clamp(200.0, w * 0.75);
    let awake = reverie(app, ui, vec2(w, h));
    if !awake {
        ui.label(RichText::new(t(lang, "elle_asleep")).small().color(ink.muted));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        let glow = read("rv_sglow").unwrap_or(0.0);
        let pleasure = read("elle_pleasure").unwrap_or(0.0);
        let seen = read("elle_seen_n").unwrap_or(0.0);
        ui.label(RichText::new(format!("{} {:.2}", t(lang, "elle_hears"), glow)).small().color(ink.muted));
        ui.label(RichText::new(format!("{} {:+.2}", t(lang, "elle_pleasure"), pleasure)).small().color(ink.muted));
        ui.label(RichText::new(format!("{} {seen:.0}", t(lang, "elle_saw"))).small().color(ink.muted));
        if crate::icons::button(ui, "eye", t(lang, "elle_show_her")).on_hover_text(t(lang, "elle_show_her_d")).clicked()
        {
            app.pick(crate::app::Pick::Eye);
        }
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_goes_there_and_back() {
        let white = oklab(1.0, 0.0, 0.0, 1.0);
        assert!(white.r() >= 254 && white.g() >= 254 && white.b() >= 254, "{white:?}");
        let p = grm_engine::vision::Picture { width: 1, height: 1, channels: 3, data: vec![1.0, 0.0, 0.0] };
        let (l, a, b) = p.mean_oklab();
        assert!((l - 0.628).abs() < 0.01 && (a - 0.225).abs() < 0.01 && (b - 0.126).abs() < 0.01, "{l} {a} {b}");
        let red = oklab(l, a, b, 1.0);
        assert!(red.r() >= 253 && red.g() <= 3 && red.b() <= 3, "{red:?}");
    }
}
