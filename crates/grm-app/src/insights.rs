// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Taking a node in hand, simulating, and what comes out of it.
//!
//! - A node made by the human (typed, written in a workshop) is moved by hand: a slider and a field, and the
//!   network settles at once (what follows from it moves, its outputs too).
//! - A node that comes from outside (a table, Our World in Data, an API, a peer) is locked: reality is not
//!   changed by hand. It can be simulated: a copy of its network, apart, where it takes the value asked; what would
//!   move is shown before -> after, and the simulation can be written to a file to be shared.
//! - Insights: reality (what came, from where), the reflection on it (strains, levers, laws, simulations), and
//!   what is shared, each with how sure it is.
//!
//! A relation of the GRM has no direction: a simulation shows what would go with the value asked (its causes as
//! well as its consequences), not yet what it would cause. Links learned in time (before -> after) are directed;
//! the words say which is shown.

use eframe::egui::{self, RichText, Stroke};

use crate::app::{App, Ink};
use crate::model::{NodeView, Picture};
use crate::text::t;

/// Whether a source is the world outside (its values are not changed by hand).
pub fn external(source: &str) -> bool {
    ["owid:", "file:", "api:", "peer:", "meteo", "sandbox:"].iter().any(|p| source.starts_with(p))
}

/// Whether a node of this network may be moved by hand (a workshop's inputs are its table's; a simulation's are
/// free).
fn locked(n: &NodeView, workshop: Option<&str>) -> bool {
    match workshop {
        Some(ws) if ws.starts_with("sim_") => false,
        Some(ws) => n.input || ws.starts_with("world_"),
        None => external(&n.source),
    }
}

/// A value said by hand: in the main network, or in a workshop (built anew there, as it is small).
fn send(app: &App, node: &str, v: f64, workshop: Option<&str>) {
    let line = format!("observe {node} = {} ± 0.000001", grm_engine::lang::g(v));
    match workshop {
        None => app.engine.send(move |w| w.say(&line, &grm_engine::Provenance::of("hand")).map(|_| String::new())),
        Some(ws) => {
            let ws = ws.to_string();
            app.engine.send(move |w| {
                let Some(x) = w.workshops.get_mut(&ws) else { return Ok(String::new()) };
                x.say(&line, &grm_engine::Provenance::of("hand"))?;
                if let Ok(m) = grm_engine::machine::Machine::new(x.machine.net.clone()) {
                    x.machine = m;
                }
                x.machine.run(1500);
                Ok(String::new())
            })
        }
    }
}

/// A node in hand: a slider and a field when it is the human's, a lock and a simulation when it is the world's.
pub fn control(app: &mut App, ui: &mut egui::Ui, n: &NodeView, workshop: Option<&str>) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let key = format!("{}/{}", workshop.unwrap_or(""), n.name);
    // the range is chosen once (it stays while the value moves)
    let (lo, hi) = *app.hand_range.entry(key.clone()).or_insert_with(|| {
        let v = n.value;
        if v > 0.0 && !n.signed {
            (0.0, (2.0 * v).max(1.0))
        } else if v != 0.0 {
            (-2.0 * v.abs(), 2.0 * v.abs())
        } else {
            (-1.0, 1.0)
        }
    });
    let mut v = app.hand_value.get(&key).copied().unwrap_or(n.value);
    // it never makes the side wider than it is
    let width = ui.available_width() - 18.0;
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, ink.faint))
        .corner_radius(crate::style::corner())
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_max_width(width);
            if locked(n, workshop) {
                ui.horizontal_wrapped(|ui| {
                    crate::icons::show(ui, "lock", 14.0, ink.muted);
                    ui.label(RichText::new(t(lang, "hand_locked")).small().color(ink.muted));
                });
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut v).speed((hi - lo).abs() / 200.0).max_decimals(4));
                    app.hand_value.insert(key.clone(), v);
                    if crate::icons::button(ui, "flask-conical", t(lang, "hand_simulate"))
                        .on_hover_text(t(lang, "hand_simulate_d"))
                        .clicked()
                    {
                        let (name, from) = (n.name.clone(), workshop.map(String::from));
                        let made = app.engine.query(move |w| w.simulate(from.as_deref(), &name, v));
                        match made {
                            Some(Ok(sim)) => {
                                let kind = crate::space::Kind::Workshop(sim);
                                app.space.put(kind.clone());
                                app.zone_center = Some(kind);
                                app.go(crate::front::Front::Space);
                            }
                            Some(Err(e)) => app.note(grm_shell::Kind::Warning, format!("! {e}")),
                            None => {}
                        }
                    }
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    crate::icons::show(ui, "hand", 14.0, ink.accent);
                    ui.label(RichText::new(t(lang, "hand_move")).small());
                });
                ui.horizontal(|ui| {
                    let s = ui.add(
                        egui::Slider::new(&mut v, lo..=hi).clamping(egui::SliderClamping::Never).show_value(false),
                    );
                    let f = ui.add(egui::DragValue::new(&mut v).speed((hi - lo).abs() / 200.0).max_decimals(4));
                    if s.changed() || f.changed() {
                        app.hand_value.insert(key.clone(), v);
                        send(app, &n.name, v, workshop);
                    }
                    if !(s.dragged() || f.dragged() || f.has_focus()) {
                        app.hand_value.remove(&key);
                    }
                });
            }
        });
}

/// In a simulation: what moved from its reality, before -> after, and the file to share it.
pub fn differences(app: &mut App, ui: &mut egui::Ui, ws: &str, pic: &Picture) {
    if pic.differences.is_empty() {
        return;
    }
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    ui.horizontal(|ui| {
        crate::icons::show(ui, "flask-conical", 15.0, ink.accent);
        ui.label(RichText::new(t(lang, "sim_moved")).strong());
    });
    ui.label(RichText::new(t(lang, "sim_note")).small().color(ink.muted));
    let lines = lines_of(pic);
    crate::style::terminal(ui, &lines);
    if crate::icons::button(ui, "share-2", t(lang, "sim_export")).on_hover_text(t(lang, "sim_export_d")).clicked() {
        let dir = app.memory.with_extension("simulations");
        let path = dir.join(format!("{ws}.grm"));
        let mut text = pic.written.join("\n");
        text.push_str("\n# what moved (before -> after, ± its spread now):\n");
        for l in &lines {
            text.push_str(&format!("# {l}\n"));
        }
        let done = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, text));
        match done {
            Ok(_) => app.toast(format!("{} {}", t(lang, "sim_written"), path.display())),
            Err(e) => app.note(grm_shell::Kind::Warning, format!("! {}: {e}", path.display())),
        }
    }
}

fn lines_of(pic: &Picture) -> Vec<String> {
    pic.differences
        .iter()
        .take(14)
        .map(|(n, a, b, s)| {
            let pc = (b - a) / a.abs().max(1e-12) * 100.0;
            format!(
                "{n}: {} -> {} (± {}) {}{:.1} %",
                grm_engine::lang::g((a * 1000.0).round() / 1000.0),
                grm_engine::lang::g((b * 1000.0).round() / 1000.0),
                grm_engine::lang::g((s * 1000.0).round() / 1000.0),
                if pc >= 0.0 { "+" } else { "" },
                pc
            )
        })
        .collect()
}

/// The Insights tab: reality, the reflection on it, what is shared.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let pic = app.picture.clone();
    let head = |ui: &mut egui::Ui, sign: &str, key: &str, d: &str| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            crate::icons::show(ui, sign, 15.0, ink.accent);
            ui.label(RichText::new(t(lang, key)).strong());
        });
        ui.label(RichText::new(t(lang, d)).small().color(ink.muted));
    };
    // reality
    head(ui, "database", "ins_reality", "ins_reality_d");
    let mut sources: Vec<(String, usize)> = vec![];
    for n in &pic.nodes {
        match sources.iter_mut().find(|s| s.0 == n.source) {
            Some(s) => s.1 += 1,
            None => sources.push((n.source.clone(), 1)),
        }
    }
    for (s, k) in &sources {
        ui.horizontal(|ui| {
            crate::icons::show(ui, if external(s) { "lock" } else { "hand" }, 12.0, ink.muted);
            ui.label(RichText::new(format!("{s} · {k} {}", t(lang, "ins_nodes"))).small());
        });
    }
    for (name, nodes, _) in pic.workshops.iter().filter(|w| w.0.starts_with("world_")) {
        ui.horizontal(|ui| {
            crate::icons::show(ui, "globe", 12.0, ink.muted);
            ui.label(RichText::new(format!("{name} · {nodes} {}", t(lang, "ins_nodes"))).small());
        });
    }
    ui.separator();
    // the reflection: strains, levers, laws, simulations
    head(ui, "brain", "ins_reflection", "ins_reflection_d");
    let mut strained: Vec<&crate::model::RelationView> = pic.relations.iter().filter(|r| r.tension > 0.5).collect();
    strained.sort_by(|a, b| b.tension.partial_cmp(&a.tension).unwrap_or(std::cmp::Ordering::Equal));
    if strained.is_empty() {
        ui.label(RichText::new(t(lang, "ins_no_strain")).small().color(ink.muted));
    } else {
        ui.label(RichText::new(t(lang, "ins_strains")).small().strong());
        let lines: Vec<String> =
            strained.iter().take(6).map(|r| format!("{} · {:.1} σ · {}", r.name, r.tension, r.text)).collect();
        crate::style::terminal(ui, &lines);
        // the levers: the nodes that hold the most of the strain (moving them would ease the most)
        let total: f64 = strained.iter().map(|r| r.tension * r.tension).sum();
        let mut levers: Vec<(String, f64)> = vec![];
        for r in &strained {
            for n in &r.nodes {
                let share = r.tension * r.tension / total / r.nodes.len().max(1) as f64;
                match levers.iter_mut().find(|l| &l.0 == n) {
                    Some(l) => l.1 += share,
                    None => levers.push((n.clone(), share)),
                }
            }
        }
        levers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ui.label(RichText::new(t(lang, "ins_levers")).small().strong()).on_hover_text(t(lang, "ins_levers_d"));
        for (n, share) in levers.iter().take(5) {
            let sure =
                pic.node(n)
                    .map(|x| if x.signed { x.sigma / x.value.abs().max(1e-9) } else { 10f64.powf(x.sigma) - 1.0 });
            let sure = sure.map(|d| format!("{:.0} %", 100.0 / (1.0 + d))).unwrap_or_else(|| "?".into());
            let locked = pic.node(n).is_some_and(|x| external(&x.source));
            ui.horizontal(|ui| {
                crate::icons::show(ui, if locked { "lock" } else { "hand" }, 12.0, ink.muted);
                if ui.link(RichText::new(n)).clicked() {
                    app.select(n, false);
                    app.tab = crate::app::Tab::Inspector;
                }
                ui.label(
                    RichText::new(format!(
                        "{:.0} % {} · {} {sure}",
                        share * 100.0,
                        t(lang, "ins_of_strain"),
                        t(lang, "ins_sure")
                    ))
                    .small(),
                );
            });
        }
    }
    let found: Vec<&(u64, String, String)> =
        pic.journal.iter().rev().filter(|e| e.1 == "found" || e.1 == "insight").take(5).collect();
    if !found.is_empty() {
        ui.label(RichText::new(t(lang, "ins_found")).small().strong());
        for (tick, _, text) in found {
            ui.label(RichText::new(format!("[{tick}] {text}")).small());
        }
    }
    let sims: Vec<String> = pic.workshops.iter().filter(|w| w.0.starts_with("sim_")).map(|w| w.0.clone()).collect();
    if !sims.is_empty() {
        ui.label(RichText::new(t(lang, "ins_sims")).small().strong());
        for s in sims {
            if crate::icons::button(ui, "flask-conical", &s).clicked() {
                let kind = crate::space::Kind::Workshop(s);
                app.space.put(kind.clone());
                app.zone_center = Some(kind);
                app.go(crate::front::Front::Space);
            }
        }
    }
    ui.separator();
    // shared
    head(ui, "share-2", "ins_shared", "ins_shared_d");
    let dir = app.memory.with_extension("simulations");
    let files: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    if files.is_empty() {
        ui.label(RichText::new(t(lang, "ins_nothing_shared")).small().color(ink.muted));
    }
    for f in files {
        ui.label(RichText::new(format!("{} · {f}", dir.display())).small());
    }
}
