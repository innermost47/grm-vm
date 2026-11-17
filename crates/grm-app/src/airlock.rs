// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The airlock: what a workshop (or a sandbox) would change in the main network, shown before anything changes,
//! and two clear choices. Let in: it comes in little by little (its relations rise step by step, and go to
//! quarantine by themselves if the network loses its harmony or is surprised). Refuse: nothing of it comes in.

use eframe::egui::{self, vec2, RichText};
use grm_engine::world::MergeReport;

use crate::app::{App, Ink};
use crate::text::t;

pub struct Airlock {
    /// The sandbox it goes through.
    pub sandbox: String,
    /// Where it comes from (a workshop), for the words.
    pub from: String,
    pub report: Result<MergeReport, String>,
    /// Lines of the workshop the language did not take.
    pub not_taken: Vec<String>,
}

/// What a workshop made, put in the airlock: a sandbox here with its lines, and a trial of what it would change.
pub fn open_for_workshop(app: &mut App, ws: &str) {
    let w = ws.to_string();
    let got = app.engine.query(move |main| -> Result<(String, MergeReport, Vec<String>), String> {
        let lines = main.workshop_lines(&w).map_err(|e| e.to_string())?;
        let mut sb = w.clone();
        let mut k = 2;
        while main.sandboxes.contains_key(&sb) {
            sb = format!("{w}_{k}");
            k += 1;
        }
        main.open_sandbox(&sb).map_err(|e| e.to_string())?;
        let mut not_taken = vec![];
        for l in &lines {
            if let Err(e) = main.say_in(&sb, l) {
                not_taken.push(format!("{l}: {e}"));
            }
        }
        let report = main.trial_merge(&sb, 10_000).map_err(|e| e.to_string())?;
        Ok((sb, report, not_taken))
    });
    app.airlock = Some(match got {
        Some(Ok((sandbox, report, not_taken))) => {
            Airlock { sandbox, from: ws.to_string(), report: Ok(report), not_taken }
        }
        Some(Err(e)) => Airlock { sandbox: String::new(), from: ws.to_string(), report: Err(e), not_taken: vec![] },
        None => return,
    });
}

/// A sandbox already there, put in the airlock.
pub fn open_for_sandbox(app: &mut App, sb: &str) {
    let s = sb.to_string();
    let report = app.engine.query(move |main| main.trial_merge(&s, 10_000).map_err(|e| e.to_string()));
    if let Some(report) = report {
        app.airlock = Some(Airlock { sandbox: sb.to_string(), from: sb.to_string(), report, not_taken: vec![] });
    }
}

fn harmony_bar(ui: &mut egui::Ui, ink: &Ink, label: &str, h: f64) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).small());
        let (r, _) = ui.allocate_exact_size(vec2(160.0, 8.0), egui::Sense::hover());
        ui.painter().rect_filled(r, 4.0, ink.faint.gamma_multiply(0.5));
        let mut full = r;
        full.set_width(r.width() * h.clamp(0.0, 1.0) as f32);
        ui.painter().rect_filled(full, 4.0, if h < 0.6 { ink.amber } else { ink.accent });
        ui.label(RichText::new(format!("{h:.3}")).small().monospace());
    });
}

/// The airlock's window.
pub fn window(app: &mut App, ctx: &egui::Context) {
    let Some(a) = app.airlock.as_ref() else { return };
    let lang = app.settings.lang;
    let ink = Ink::of(ctx);
    let (sandbox, from) = (a.sandbox.clone(), a.from.clone());
    let report = a.report.clone();
    let not_taken = a.not_taken.clone();
    let mut choice: Option<&str> = None;
    let modal = egui::Modal::new(egui::Id::new("airlock")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.horizontal(|ui| {
            crate::icons::show(ui, "door-open", 20.0, ink.accent);
            ui.label(RichText::new(format!("{} « {from} »", t(lang, "airlock_title"))).strong().size(17.0));
        });
        ui.label(RichText::new(t(lang, "airlock_d")).small().color(ink.muted));
        ui.add_space(6.0);
        match &report {
            Err(e) => {
                ui.label(RichText::new(e).color(ink.amber));
            }
            Ok(r) => {
                // what it brings
                ui.label(RichText::new(t(lang, "airlock_new")).strong());
                let mut lines = vec![];
                if !r.new_nodes.is_empty() {
                    lines.push(format!("node {}", r.new_nodes.join(", ")));
                }
                if !r.new_relations.is_empty() {
                    lines.push(format!("relate {}", r.new_relations.join(", ")));
                }
                if lines.is_empty() {
                    lines.push(format!("# {}", t(lang, "airlock_nothing_new")));
                }
                crate::style::terminal(ui, &lines);
                // what it would change
                ui.add_space(4.0);
                ui.label(RichText::new(t(lang, "airlock_changes")).strong());
                let mut changes = vec![];
                for (n, s) in &r.surprises {
                    changes.push(format!("{n}: {} {s:.1} σ", t(lang, "airlock_moves")));
                }
                for (n, b, after) in &r.tensions {
                    changes.push(format!("{n}: {} {b:.1} σ -> {after:.1} σ", t(lang, "strain")));
                }
                for c in &r.conflicts {
                    changes.push(format!("{}: {c}", t(lang, "airlock_conflict")));
                }
                for c in &r.replaced {
                    changes.push(format!("{}: {c}", t(lang, "airlock_replaced")));
                }
                if changes.is_empty() {
                    changes.push(format!("# {}", t(lang, "airlock_nothing_moves")));
                }
                crate::style::terminal(ui, &changes);
                ui.add_space(4.0);
                harmony_bar(ui, &ink, t(lang, "airlock_before"), r.harmony_before);
                harmony_bar(ui, &ink, t(lang, "airlock_after"), r.harmony_after);
                ui.add_space(4.0);
                let (sign, colour, key) = if r.compatible() {
                    ("circle-check", ink.source(2), "airlock_ok")
                } else {
                    ("triangle-alert", ink.amber, "airlock_careful")
                };
                ui.horizontal(|ui| {
                    crate::icons::show(ui, sign, 16.0, colour);
                    ui.label(RichText::new(t(lang, key)).color(colour));
                });
            }
        }
        if !not_taken.is_empty() {
            ui.label(RichText::new(t(lang, "airlock_not_taken")).small().color(ink.amber));
            crate::style::terminal(ui, &not_taken);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let ok = report.is_ok();
            let b = egui::Button::image_and_text(
                crate::icons::image("door-open", 15.0, ink.on_accent()),
                RichText::new(t(lang, "airlock_let_in")).color(ink.on_accent()),
            )
            .fill(ink.accent);
            if ui.add_enabled(ok, b).on_hover_text(t(lang, "airlock_let_in_d")).clicked() {
                choice = Some("merge");
            }
            if crate::icons::button(ui, "ban", t(lang, "airlock_refuse"))
                .on_hover_text(t(lang, "airlock_refuse_d"))
                .clicked()
            {
                choice = Some("refuse");
            }
            if crate::icons::button(ui, "clock-3", t(lang, "airlock_later"))
                .on_hover_text(t(lang, "airlock_later_d"))
                .clicked()
            {
                choice = Some("later");
            }
        });
    });
    if modal.should_close() && choice.is_none() {
        choice = Some("later");
    }
    match choice {
        Some("merge") if !sandbox.is_empty() => {
            app.say(format!("merge {sandbox}"));
            app.airlock = None;
        }
        Some("refuse") if !sandbox.is_empty() => {
            app.say(format!("refuse {sandbox}"));
            app.airlock = None;
        }
        Some(_) => app.airlock = None,
        None => {}
    }
}
