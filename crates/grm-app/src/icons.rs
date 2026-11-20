// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Signs drawn, never typed: the Lucide icons (ISC licence, assets/icons/LICENSE-lucide), every one of them embedded,
//! drawn white and tinted with the palette (so they follow the machine's colours and an app's looks).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use eframe::egui::{ self, vec2, Color32, Response };

static TABLE: &str = include_str!("../../../assets/icons/lucide.tsv");

/// Each icon by its name, as a whole SVG.
fn all() -> &'static BTreeMap<&'static str, String> {
    static ALL: OnceLock<BTreeMap<&'static str, String>> = OnceLock::new();
    ALL.get_or_init(|| {
        TABLE.lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(name, inner)| {
                (
                    name,
                    format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" \
                         fill=\"none\" stroke=\"#fff\" stroke-width=\"2\" stroke-linecap=\"round\" \
                         stroke-linejoin=\"round\">{inner}</svg>"
                    ),
                )
            })
            .collect()
    })
}

/// Whether an icon of this name exists.
pub fn exists(name: &str) -> bool {
    all().contains_key(name)
}

/// Every name, in order (for a chooser).
pub fn names() -> impl Iterator<Item = &'static str> {
    all().keys().copied()
}

/// An icon as an image: its size in points, its colour. A name that does not exist shows a question mark.
pub fn image(name: &str, size: f32, colour: Color32) -> egui::Image<'static> {
    let (name, svg) = match all().get_key_value(name) {
        Some((k, v)) => (*k, v),
        None =>
            all()
                .get_key_value("circle-help")
                .map(|(k, v)| (*k, v))
                .expect("the question mark"),
    };
    egui::Image
        ::from_bytes(format!("bytes://icon/{name}.svg"), svg.as_bytes())
        .fit_to_exact_size(vec2(size, size))
        .tint(colour)
}

/// An icon drawn where the layout puts it.
pub fn show(ui: &mut egui::Ui, name: &str, size: f32, colour: Color32) -> Response {
    ui.add(image(name, size, colour))
}

/// A button with its sign and its words.
pub fn button(ui: &mut egui::Ui, name: &str, text: impl Into<egui::WidgetText>) -> Response {
    let colour = ui.visuals().text_color();
    ui.add(egui::Button::image_and_text(image(name, 15.0, colour), text))
}

/// A button with its sign and its words, shown as chosen or not.
pub fn toggle(
    ui: &mut egui::Ui,
    name: &str,
    text: impl Into<egui::WidgetText>,
    on: bool
) -> Response {
    let colour = if on { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    let widget_text = text.into();

    if widget_text.is_empty() {
        let b = egui::Button::image(image(name, 15.0, colour)).selected(on);
        ui.add(b)
    } else {
        let b = egui::Button::image_and_text(image(name, 15.0, colour), widget_text).selected(on);
        ui.add(b)
    }
}

/// A button that is only its sign (what it does is said when the pointer rests on it).
pub fn sign(ui: &mut egui::Ui, name: &str, what: &str) -> Response {
    sign_enabled(ui, true, name, what)
}

/// A sign that can be pressed or not (greyed when it cannot).
pub fn sign_enabled(ui: &mut egui::Ui, enabled: bool, name: &str, what: &str) -> Response {
    sign_on(ui, enabled, false, name, what)
}

/// A sign that stays lit while what it does is on (pressed again, it is undone).
pub fn sign_on(ui: &mut egui::Ui, enabled: bool, on: bool, name: &str, what: &str) -> Response {
    let colour = if !enabled {
        ui.visuals().weak_text_color()
    } else if on {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    let r = ui.add_enabled(enabled, egui::Button::image(image(name, 16.0, colour)).frame(false));
    // said by its words to whoever cannot see it (a screen reader, the window's tests)
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, what));
    r.on_hover_text(what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_used_by_the_window_exists() {
        assert!(names().count() > 2000);
        for n in [
            "search",
            "timer",
            "trending-up",
            "square",
            "play",
            "chart-line",
            "link",
            "pencil",
            "pin-off",
            "shield-alert",
            "trash-2",
            "circle-plus",
            "scan-search",
            "file-text",
            "table",
            "app-window",
            "triangle-alert",
            "folder-open",
            "arrow-left",
            "arrow-right",
            "undo-2",
            "redo-2",
            "settings",
            "circle-help",
            "chevron-right",
            "chevron-down",
            "maximize",
            "code",
            "plus",
            "moon",
            "image",
            "octagon-x",
            "panel-right-close",
            "panel-right-open",
            "panel-bottom-close",
            "panel-bottom-open",
            "flask-conical",
            "waypoints",
            "scroll-text",
            "eye",
            "audio-lines",
            "x",
            "sheet",
            "file-code",
            "layout-grid",
            "save",
            "between-horizontal-end",
            "between-vertical-end",
        ] {
            assert!(exists(n), "{n}");
        }
        assert!(
            all()["play"].contains("stroke=\"#fff\"") && !all()["play"].contains("currentColor")
        );
    }
}
