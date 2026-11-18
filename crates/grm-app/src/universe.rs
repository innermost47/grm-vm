// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The universe of the GRM: a network of grm-core (the constants, the cosmos, the atoms, the planets) living in the
//! machine as an app, seen in three dimensions. Across: each quantity's value in Planck units (in decades: c, G,
//! hbar, kB on 0). Depth and height: the two slowest ways the network vibrates (its shape, decided by its relations).
//! Time: from the Planck time to today, each quantity moving with the cadence H as its relations say. Beside it, the
//! equations it holds without anyone having written them, each with the relations that lead to it. Nothing here is
//! imposed: what is computed is read from the network (grm-engine's `universe`).

use eframe::egui::{self as egui};
use eframe::egui::{vec2, Color32, Pos2, RichText, Sense, Stroke};
use grm_engine::universe::Universe;

use crate::app::{App, Ink};
use crate::text::t;

/// Its app (`apps/universe.grm`, written from the grm-core store the first time).
pub const APP: &str = "universe";
/// The names it is said with.
pub const PREFIX: &str = "u_";
/// Its source in the machine.
const SOURCE: &str = "app:universe";

pub struct View {
    model: Option<Universe>,
    /// How many relations the source had when it was read (read again when it changes).
    read_at: usize,
    seed: String,
    /// The time: log10 of the time since the 0, in Planck times.
    time: f64,
    playing: bool,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    names: bool,
    chosen: Option<usize>,
}

impl Default for View {
    fn default() -> Self {
        View {
            model: None,
            read_at: usize::MAX,
            seed: format!("{PREFIX}Hend"),
            time: 60.9,
            playing: false,
            yaw: -0.35,
            pitch: 0.28,
            zoom: 1.0,
            names: false,
            chosen: Some(0),
        }
    }
}

/// The grm-core store the universe comes from (beside the memory, or the repository's).
pub fn store(memory: &std::path::Path) -> Option<std::path::PathBuf> {
    [memory.parent().map(|p| p.join("data/universe/grm_network.json")), Some("data/universe/grm_network.json".into())]
        .into_iter()
        .flatten()
        .find(|p| p.exists())
}

/// The app's file, from a grm-core store: its header, then the network said in GRML.
pub fn app_text(json: &str) -> Result<String, String> {
    let said = grm_connect::grm_core::to_grml(json, None, PREFIX).map_err(|e| e.to_string())?;
    Ok(format!(
        "app Univers\nicon orbit\n# The universe of the GRM: a network of grm-core ({} quantities, {} relations), each name\n# given the prefix {PREFIX}. Its surface is the window's: the quantities on the scale of Planck, the shape the\n# network decides, time from the Planck time to today, and the equations it holds without anyone writing them.\n{}",
        said.nodes, said.relations, said.grml
    ))
}

fn short(name: &str) -> String {
    name.strip_prefix(PREFIX).unwrap_or(name).to_string()
}

/// The surface.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    // read again when the network changed (or the seed)
    let rels = app.picture.relations.iter().filter(|r| r.source == SOURCE).count();
    if app.universe.model.is_none() || rels != app.universe.read_at {
        let seed = app.universe.seed.clone();
        let got = app.engine.query(move |w| w.universe(SOURCE, Some(&seed)));
        app.universe.model = got;
        app.universe.read_at = rels;
    }
    let Some(model) = app.universe.model.clone() else {
        ui.label(RichText::new(t(lang, "uni_none")).color(ink.muted));
        return;
    };
    if model.names.is_empty() {
        ui.label(RichText::new(t(lang, "uni_none")).color(ink.muted));
        import_button(app, ui);
        return;
    }
    let at = |n: &str| model.names.iter().position(|x| short(x) == n);
    // today: the age of the universe the network holds (t0), in Planck times
    let today = at("t0").map(|i| (model.value[i] / grm_engine::universe::PLANCK[2]).log10()).unwrap_or(60.9);
    let v = &mut app.universe;
    if v.playing {
        v.time += ui.input(|i| i.stable_dt).min(0.1) as f64 * 3.0;
        if v.time >= today {
            v.time = today;
            v.playing = false;
        }
        ui.ctx().request_repaint();
    }
    let shift = today - v.time;
    egui::SidePanel::left("universe-side").resizable(true).default_width(360.0).show_inside(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("universe-side-scroll").show(ui, |ui| {
            ui.label(RichText::new(t(lang, "uni_title")).size(crate::style::size(crate::style::Size::Title)).strong());
            ui.label(RichText::new(t(lang, "uni_about")).small().color(ink.muted));
            ui.add_space(6.0);
            // the seed: the one degree of freedom the family tree grows from, and the cadence quantities follow
            let mut seed = app.universe.seed.clone();
            egui::ComboBox::from_label(t(lang, "uni_seed"))
                .selected_text(short(&seed))
                .show_ui(ui, |ui| {
                    for (i, n) in model.names.iter().enumerate() {
                        if !model.fixed[i] {
                            ui.selectable_value(&mut seed, n.clone(), short(n));
                        }
                    }
                })
                .response
                .on_hover_text(t(lang, "uni_seed_d"));
            if seed != app.universe.seed {
                app.universe.seed = seed;
                app.universe.model = None;
            }
            ui.horizontal_wrapped(|ui| {
                for (c, key) in [
                    (ink.amber, "uni_follows"),
                    (Color32::from_rgb(110, 160, 255), "uni_against"),
                    (ink.muted, "uni_still"),
                    (Color32::WHITE, "uni_constants"),
                ] {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.5, c);
                    ui.label(RichText::new(t(lang, key)).small());
                }
            });
            ui.label(RichText::new(t(lang, "uni_sizes")).small().color(ink.faint));
            ui.separator();
            ui.label(RichText::new(t(lang, "uni_found")).strong());
            ui.label(RichText::new(t(lang, "uni_found_d")).small().color(ink.muted));
            let name = |i: usize| short(&model.names[i]);
            for (k, f) in model.found.iter().enumerate().take(24) {
                let chosen = app.universe.chosen == Some(k);
                let text = f.text(&name);
                // checked at the values the machine holds now
                let check: f64 =
                    f.terms.iter().map(|(i, p)| p * model.value[*i].log10()).sum::<f64>() - f.number.log10();
                let words = RichText::new(&text).color(if chosen { ink.accent } else { ink.text });
                let r = ui.add(egui::Button::new(words).frame(false));
                if r.clicked() {
                    app.universe.chosen = if chosen { None } else { Some(k) };
                }
                if chosen {
                    egui::Frame::new().inner_margin(egui::Margin { left: 14, ..Default::default() }).show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} {} · {} {} · {} {:.1e} {}",
                                f.path.len(),
                                t(lang, if f.written { "uni_written" } else { "uni_steps" }),
                                t(lang, "uni_cancelled"),
                                f.cancelled.iter().map(|i| short(&model.names[*i])).collect::<Vec<_>>().join(", "),
                                t(lang, "uni_check"),
                                check,
                                t(lang, "uni_decades")
                            ))
                            .small()
                            .color(ink.muted),
                        );
                        for (w, rname, rtext) in &f.path {
                            ui.label(
                                RichText::new(format!("{w:+.3} × {}  ({})", short(rname), rtext.replace(PREFIX, "")))
                                    .small()
                                    .monospace(),
                            );
                        }
                    });
                }
            }
            ui.separator();
            import_button(app, ui);
        });
    });
    // ---- the three dimensions
    let v = &mut app.universe;
    let (rect, resp) = ui.allocate_exact_size(ui.available_size() - vec2(0.0, 64.0), Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        v.yaw += d.x * 0.006;
        v.pitch = (v.pitch + d.y * 0.006).clamp(-1.4, 1.4);
    }
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        v.zoom = (v.zoom * (1.0 + s * 0.002)).clamp(0.3, 8.0);
    }
    if resp.double_clicked() {
        (v.yaw, v.pitch, v.zoom) = (-0.35, 0.28, 1.0);
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, crate::style::corner(), Color32::from_rgb(6, 8, 14));
    let (cy, sy, cp, sp) = (v.yaw.cos(), v.yaw.sin(), v.pitch.cos(), v.pitch.sin());
    let scale = rect.width().min(rect.height() * 1.6) / 300.0 * v.zoom;
    let centre = rect.center();
    // a point of the universe (decades across, its modes in depth and height) on the screen, with its depth
    let project = |x: f64, y: f64, z: f64| -> (Pos2, f32) {
        let (x, y, z) = (x as f32, y as f32, z as f32);
        let (x1, z1) = (x * cy - z * sy, x * sy + z * cy);
        let (y2, z2) = (y * cp - z1 * sp, y * sp + z1 * cp);
        let f = 420.0 / (420.0 + z2);
        (centre + vec2(x1 * f * scale, -y2 * f * scale), z2)
    };
    let x_of = |i: usize| -> f64 {
        let e = model.exponent[i].unwrap_or(0.0);
        let x = model.planck[i] + e * shift;
        if x.is_finite() {
            x
        } else {
            0.0
        }
    };
    let pt = |i: usize| project(x_of(i), model.modes[i].0 * 55.0, model.modes[i].1 * 55.0);
    // the scale of Planck: a line across, its decades
    let (a, _) = project(-130.0, -62.0, 0.0);
    let (b, _) = project(130.0, -62.0, 0.0);
    painter.line_segment([a, b], Stroke::new(1.0_f32, ink.faint));
    for d in (-120..=120).step_by(40) {
        let (p, _) = project(d as f64, -62.0, 0.0);
        let label = if d == 0 { t(lang, "uni_zero").to_string() } else { format!("10^{d}") };
        painter.text(
            p + vec2(0.0, 6.0),
            egui::Align2::CENTER_TOP,
            label,
            crate::style::font(crate::style::Size::Small),
            ink.muted,
        );
    }
    // its relations, faint
    for &(i, j) in &model.ties {
        let (p, _) = pt(i);
        let (q, _) = pt(j);
        painter.line_segment([p, q], Stroke::new(0.5_f32, Color32::from_rgba_unmultiplied(120, 140, 180, 26)));
    }
    // its quantities, the farthest first
    let chosen: Vec<usize> = v
        .chosen
        .and_then(|k| model.found.get(k))
        .map(|f| f.terms.iter().map(|(i, _)| *i).collect())
        .unwrap_or_default();
    let mut order: Vec<(usize, Pos2, f32)> = (0..model.names.len())
        .map(|i| {
            let (p, z) = pt(i);
            (i, p, z)
        })
        .collect();
    order.sort_by(|a, b| b.2.total_cmp(&a.2));
    let hover = resp.hover_pos();
    let mut nearest: Option<(usize, f32)> = None;
    for &(i, p, z) in &order {
        let colour = if model.fixed[i] {
            Color32::WHITE
        } else {
            match model.exponent[i] {
                Some(e) if e > 1e-9 => ink.amber,
                Some(e) if e < -1e-9 => Color32::from_rgb(110, 160, 255),
                _ => Color32::from_rgb(140, 145, 160),
            }
        };
        let depth = (420.0 / (420.0 + z)).clamp(0.5, 1.6);
        let r = if model.measured[i] || model.fixed[i] { 4.5 } else { 2.6 } * depth;
        let lit = chosen.contains(&i);
        if lit {
            painter.circle_stroke(p, r + 4.0, Stroke::new(1.5_f32, ink.accent));
        }
        painter.circle_filled(p, r, if lit || chosen.is_empty() { colour } else { colour.gamma_multiply(0.55) });
        let named = v.names
            || lit
            || Some(i) == model.seed
            || (model.fixed[i] && ["c", "G", "hbar", "kB"].contains(&short(&model.names[i]).as_str()));
        if named {
            painter.text(
                p + vec2(r + 4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                short(&model.names[i]),
                crate::style::font(crate::style::Size::Small),
                if lit { ink.text } else { ink.muted },
            );
        }
        if let Some(h) = hover {
            let d = h.distance(p);
            if d < 10.0 && nearest.is_none_or(|(_, b)| d < b) {
                nearest = Some((i, d));
            }
        }
    }
    if let Some((i, _)) = nearest {
        let born = model.born[i].as_ref().map(|(g, r, ps)| {
            if *g == 0 {
                t(lang, "uni_seedling").to_string()
            } else {
                format!(
                    "{} {g} ({} ← {})",
                    t(lang, "uni_generation"),
                    short(r),
                    ps.iter().map(|p| short(&model.names[*p])).collect::<Vec<_>>().join(", ")
                )
            }
        });
        let text = format!(
            "{}\n{} ± {:.2} dex\n{} 10^{:.2}\n{}{}",
            short(&model.names[i]),
            grm_engine::lang::g(model.value[i]),
            model.sigma[i],
            t(lang, "uni_in_planck"),
            x_of(i),
            model.exponent[i].map(|e| format!("{} {e:+.3}\n", t(lang, "uni_exponent"))).unwrap_or_default(),
            born.unwrap_or_else(|| t(lang, "uni_not_born").to_string())
        );
        resp.clone().on_hover_text(text);
    }
    // ---- time
    ui.horizontal(|ui| {
        let v = &mut app.universe;
        let (icon, key) = if v.playing { ("pause", "uni_pause") } else { ("play", "uni_play") };
        if crate::icons::button(ui, icon, t(lang, key)).clicked() {
            if !v.playing && v.time >= today - 1e-6 {
                v.time = 0.0;
            }
            v.playing = !v.playing;
        }
        if crate::icons::button(ui, "sun", t(lang, "uni_today")).clicked() {
            v.time = today;
            v.playing = false;
        }
        ui.add(egui::Button::new(t(lang, "uni_names")).selected(v.names)).clicked().then(|| v.names = !v.names);
        ui.spacing_mut().slider_width = (ui.available_width() - 20.0).max(100.0);
        ui.add(egui::Slider::new(&mut v.time, 0.0..=today).show_value(false));
    });
    let seconds = 10f64.powf(app.universe.time) * grm_engine::universe::PLANCK[2];
    ui.label(
        RichText::new(format!(
            "{} 10^{:.1} t_P ({:.3e} s) · {} {:.3e} s⁻¹ · {} 10^{:.1} t_P",
            t(lang, "uni_since"),
            app.universe.time,
            seconds,
            t(lang, "uni_cadence"),
            1.0 / seconds,
            t(lang, "uni_now"),
            today
        ))
        .small()
        .color(ink.muted),
    );
}

fn import_button(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.settings.lang;
    if crate::icons::button(ui, "file-input", t(lang, "uni_import")).on_hover_text(t(lang, "uni_import_d")).clicked() {
        if let Some(path) = rfd::FileDialog::new().add_filter("grm-core", &["json"]).pick_file() {
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|j| app_text(&j)) {
                Ok(text) => {
                    let file = grm_shell::runtime::apps_dir(&app.memory).join(format!("{APP}.grm"));
                    match std::fs::write(&file, text) {
                        Ok(()) => {
                            app.universe.model = None;
                            app.say(format!("app open {}", file.display()));
                        }
                        Err(e) => app.toast(format!("{}: {e}", file.display())),
                    }
                }
                Err(e) => app.toast(e),
            }
        }
    }
}
