// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What can be done, under the right button: each with its sign and a line saying what it does. And the small
//! form that makes a node without writing it (what it makes is written in the code all the same).

use eframe::egui::{self, RichText};

use crate::app::{App, Ink};
use crate::text::t;

/// One thing to do: its sign, its name, what it does. True when chosen.
pub fn item(ui: &mut egui::Ui, sign: &str, title: &str, what: &str) -> bool {
    let ink = Ink::of(ui.ctx());
    let r = ui.add(
        egui::Button::image_and_text(crate::icons::image(sign, 16.0, ink.accent), RichText::new(title).strong())
            .frame(false)
            .min_size(egui::vec2(260.0, 0.0)),
    );
    egui::Frame::new().inner_margin(egui::Margin { left: 24, ..Default::default() }).show(ui, |ui| {
        ui.label(RichText::new(what).small().color(ink.muted));
    });
    ui.add_space(2.0);
    r.clicked()
}

/// The menu of a node.
pub fn node_menu(app: &mut App, ui: &mut egui::Ui, n: &str) {
    let lang = app.settings.lang;
    let ink = Ink::of(ui.ctx());
    ui.label(RichText::new(n).strong().size(15.0).color(ink.accent));
    ui.separator();
    let node = app.picture.node(n).cloned();
    let tracked = node.as_ref().is_some_and(|x| x.tracked);
    if item(ui, "search", t(lang, "m_inspect"), t(lang, "m_inspect_d")) {
        app.select(n, false);
        ui.close();
    }
    if !tracked && item(ui, "timer", t(lang, "track"), t(lang, "m_track_d")) {
        app.say(format!("track {n}"));
        ui.close();
    }
    if tracked && item(ui, "trending-up", t(lang, "forecast"), t(lang, "m_forecast_d")) {
        app.say(format!("forecast {n} 12"));
        ui.close();
    }
    let playing = app.mixer.lock().map(|m| m.playing().contains(&n.to_string())).unwrap_or(false);
    if playing {
        if item(ui, "square", t(lang, "stop"), t(lang, "m_stop_d")) {
            app.say(format!("stop {n}"));
            ui.close();
        }
    } else if item(ui, "play", t(lang, "play"), t(lang, "m_play_d")) {
        app.say(format!("play {n}"));
        ui.close();
    }
    if item(ui, "chart-line", t(lang, "m_plot"), t(lang, "m_plot_d")) {
        if !app.plotted.contains(&n.to_string()) {
            app.plotted.push(n.to_string());
        }
        app.show_plot();
        ui.close();
    }
    if item(ui, "link", t(lang, "m_relate"), t(lang, "m_relate_d")) {
        app.relate_from = Some(n.to_string());
        ui.close();
    }
    if let Some((a, l)) = app.line_in_app(n) {
        if item(ui, "pencil", t(lang, "edit_line"), &format!("{a}, {} {l}", t(lang, "line"))) {
            let text =
                app.apps.iter().find(|x| x.name == a).and_then(|x| x.text.lines().nth(l - 1)).unwrap_or("").to_string();
            app.line_draft = Some((Some(a), Some(l), text.trim().to_string()));
            ui.close();
        }
    }
    if app.graph.pinned.contains(&format!("n:{n}")) && item(ui, "pin-off", t(lang, "unpin"), t(lang, "m_unpin_d")) {
        app.graph.pinned.remove(&format!("n:{n}"));
        app.graph.motion = 1.0;
        ui.close();
    }
    if let Some(src) = node.as_ref().map(|x| x.source.clone()) {
        if !app.picture.quarantine.contains(&src)
            && item(ui, "shield-alert", t(lang, "quarantine_source"), &format!("{src}: {}", t(lang, "m_quarantine_d")))
        {
            app.say(format!("quarantine {src}"));
            ui.close();
        }
    }
    if item(ui, "trash-2", t(lang, "remove"), t(lang, "m_remove_d")) {
        app.say(format!("remove {n}"));
        ui.close();
    }
}

/// The menu of the background.
pub fn background_menu(app: &mut App, ui: &mut egui::Ui, at: [f32; 2]) {
    let lang = app.settings.lang;
    if item(ui, "circle-plus", t(lang, "new_node"), t(lang, "m_new_node_d")) {
        app.node_form = Some(NodeForm { at, ..Default::default() });
        ui.close();
    }
    if item(ui, "link", t(lang, "m_new_relation"), t(lang, "m_new_relation_d")) {
        app.line_draft = Some((app.current.clone(), None, "relate r: a = 2 * b".into()));
        ui.close();
    }
    if item(ui, "scan-search", t(lang, "fit"), t(lang, "m_fit_d")) {
        app.view.fitted = false;
        ui.close();
    }
    if item(ui, "chart-line", t(lang, "m_plots"), t(lang, "m_plots_d")) {
        app.show_plot();
        ui.close();
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeForm {
    pub name: String,
    /// 0 positive, 1 signed, 2 input, 3 var, 4 output, 5 learn
    pub kind: usize,
    pub value: String,
    pub at: [f32; 2],
}

const KINDS: [(&str, &str); 6] = [
    ("k_positive", "node {n} = {v}"),
    ("k_signed", "node {n} = {v} signed"),
    ("k_input", "input {n} = {v}"),
    ("k_var", "var {n} = {v}"),
    ("k_output", "output {n}"),
    ("k_learn", "learn {n} = {v}"),
];

/// The small form that makes a node.
pub fn node_form(app: &mut App, ctx: &egui::Context) {
    let Some(mut f) = app.node_form.clone() else { return };
    let lang = app.settings.lang;
    let mut open = true;
    let mut made: Option<String> = None;
    egui::Window::new(t(lang, "new_node")).open(&mut open).collapsible(false).default_width(360.0).show(ctx, |ui| {
        egui::Grid::new("node-form").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label(t(lang, "f_name"));
            ui.text_edit_singleline(&mut f.name).request_focus();
            ui.end_row();
            ui.label(t(lang, "f_kind"));
            ui.vertical(|ui| {
                for (k, (key, _)) in KINDS.iter().enumerate() {
                    ui.radio_value(&mut f.kind, k, t(lang, key));
                }
            });
            ui.end_row();
            if f.kind != 4 {
                ui.label(t(lang, "value"));
                ui.text_edit_singleline(&mut f.value);
                ui.end_row();
            }
        });
        let name_ok = grm_engine::lang::is_name(f.name.trim());
        let value = if f.value.trim().is_empty() {
            if f.kind == 0 { "1" } else { "0" }.to_string()
        } else {
            f.value.trim().to_string()
        };
        let line = KINDS[f.kind].1.replace("{n}", f.name.trim()).replace("{v}", &value);
        ui.label(RichText::new(&line).monospace().color(Ink::of(ui.ctx()).muted));
        ui.horizontal(|ui| {
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add_enabled(name_ok, egui::Button::new(t(lang, "apply"))).clicked() || (enter && name_ok) {
                made = Some(line.clone());
            }
            if ui.button(t(lang, "cancel")).clicked() {
                app.node_form = None;
            }
        });
    });
    if !open {
        app.node_form = None;
        return;
    }
    if app.node_form.is_none() {
        return;
    }
    if let Some(line) = made {
        // where it was asked for: it is put there
        let key = format!("n:{}", f.name.trim());
        app.graph.pos.insert(key.clone(), f.at);
        app.graph.pinned.insert(key);
        app.node_form = None;
        app.write_line(&line);
    } else {
        app.node_form = Some(f);
    }
}
