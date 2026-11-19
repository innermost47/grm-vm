// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The inspector as a tree, browsed as a file explorer: the networks, then their nodes and relations, then what each
//! one holds. Every line can do two things: show or hide what it is (its detail: for a network, its master equation
//! and its curve), and unfold what is under it (its children). Every line has its sign, its sound (play, stop), and
//! under the right button what can be done with it.

use std::collections::HashSet;

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke};

use crate::app::{App, Ink};
use crate::model::Picture;
use crate::text::t;
use crate::view::reading;

const INDENT: f32 = 16.0;
/// Room kept on the right of a line for its signs.
const ACTIONS: f32 = 54.0;
/// Lines shown in a folder at most (the filter finds the others).
const MOST: usize = 300;

struct RowOut {
    unfold: bool,
    label: egui::Response,
}

/// One line: the chevron (when it has children), its sign, its name, what it reads, its actions on the right.
#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    depth: usize,
    children: bool,
    open: bool,
    detail: bool,
    sign: &str,
    colour: Color32,
    title: RichText,
    value: Option<String>,
    actions: impl FnOnce(&mut egui::Ui),
) -> RowOut {
    let ink = Ink::of(ui.ctx());
    let lang = crate::text::lang_now(ui.ctx());
    let mut unfold = false;
    // the line takes the width there is, never more (the side keeps the width the human gave it)
    let avail = ui.available_width();
    let inner = ui.horizontal(|ui| {
        ui.set_width(avail);
        ui.add_space(depth as f32 * INDENT);
        if children {
            let chevron = if open { "chevron-down" } else { "chevron-right" };
            let what = if open { t(lang, "t_fold") } else { t(lang, "t_unfold") };
            if crate::icons::sign(ui, chevron, what).clicked() {
                unfold = true;
            }
        } else {
            ui.add_space(20.0);
        }
        crate::icons::show(ui, sign, 15.0, colour);
        let title = if detail { title.strong().color(ink.accent) } else { title };
        let left = (ui.available_width() - ACTIONS).max(40.0);
        let label = ui
            .allocate_ui_with_layout(vec2(left, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_max_width(left);
                let label = ui.add(egui::Label::new(title).sense(Sense::click()).truncate());
                if let Some(v) = value {
                    ui.add(egui::Label::new(RichText::new(v).small().color(ink.muted)).truncate());
                }
                label
            })
            .inner;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
        label
    });
    let full = inner.response.rect;
    if inner.inner.hovered() || ui.rect_contains_pointer(full) {
        ui.painter().rect_filled(full.expand(1.0), crate::style::corner(), ink.faint.gamma_multiply(0.18));
    }
    RowOut { unfold, label: inner.inner.on_hover_text(t(lang, "t_row_hint")) }
}

/// A sign for a network, by where it comes from.
fn network_sign(app: &App, source: &str) -> String {
    if let Some(a) = source.strip_prefix("app:") {
        return app
            .apps
            .iter()
            .find(|x| x.name == a)
            .and_then(|x| x.icon.clone())
            .filter(|i| crate::icons::exists(i))
            .unwrap_or("app-window".into());
    }
    // a sandbox let in: rising little by little, then in the main network (its sign says it)
    if let Some(sb) = source.strip_prefix("sandbox:") {
        if !app.picture.sandboxes.iter().any(|x| x.name == sb) {
            let rising = app.picture.integrating.iter().any(|i| i.starts_with(source));
            return if rising { "trending-up" } else { "circle-check" }.to_string();
        }
    }
    match source.split(':').next().unwrap_or("") {
        "typed" => "keyboard",
        "file" => "sheet",
        "sandbox" => "flask-conical",
        "eye" => "eye",
        "found" => "sparkles",
        "base" => "landmark",
        _ => "network",
    }
    .to_string()
}

fn node_sign(n: &crate::model::NodeView) -> &'static str {
    if n.input {
        "log-in"
    } else if n.output {
        "log-out"
    } else if n.learn {
        "brain"
    } else if n.var {
        "variable"
    } else if n.observed {
        "gauge"
    } else {
        "circle-dot"
    }
}

/// The inspector.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // the workshop in front: its nodes and relations at once (its screen, written in GRML)
    crate::screen::inspector(app, ui);
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let pic = app.picture.clone();
    // a little room on the right: the scroll bar floats there
    ui.set_max_width(ui.available_width() - 10.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.filter)
                .margin(crate::style::field())
                .hint_text(crate::style::hint(t(lang, "filter")))
                .desired_width(ui.available_width() - 36.0),
        );
        if crate::icons::sign(ui, "chevrons-down-up", t(lang, "t_fold_all")).clicked() {
            app.tree_open.clear();
            app.tree_detail.clear();
            app.selected.clear();
        }
    });
    if app.selected.len() > 1 {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", app.selected.len(), t(lang, "selected_many")));
            if ui.button(t(lang, "joint")).clicked() {
                let names = app.selected.join(" ");
                app.say(format!("joint {names}"));
            }
            if ui.button(t(lang, "clear")).clicked() {
                app.selected.clear();
            }
        });
    }
    ui.add_space(4.0);
    // a node chosen elsewhere (in the network): its network and its folder unfold, the list goes to it
    let memory = egui::Id::new("inspector-open");
    let was: Vec<String> = ui.ctx().data(|d| d.get_temp(memory)).unwrap_or_default();
    for s in app.selected.clone() {
        if !was.contains(&s) {
            if let Some(n) = pic.node(&s) {
                let net = format!("n:{}", n.source);
                app.tree_open.insert(net.clone());
                app.tree_open.insert(format!("{net}/nodes"));
                app.tree_scroll = Some(format!("node:{s}"));
            }
        }
    }
    let now = app.selected.clone();
    ui.ctx().data_mut(|d| d.insert_temp(memory, now));
    let playing: HashSet<String> = app.mixer.lock().map(|m| m.playing().into_iter().collect()).unwrap_or_default();
    // the networks, in the order they came
    let mut sources: Vec<String> = vec![];
    for s in pic.nodes.iter().map(|n| &n.source).chain(pic.relations.iter().map(|r| &r.source)) {
        if !sources.contains(s) {
            sources.push(s.clone());
        }
    }
    let filter = app.filter.to_lowercase();
    if sources.is_empty() {
        ui.label(RichText::new(t(lang, "t_empty")).color(ink.muted));
    }
    for source in sources {
        network(app, ui, &pic, &source, &filter, &playing);
    }
}

fn network(app: &mut App, ui: &mut egui::Ui, pic: &Picture, source: &str, filter: &str, playing: &HashSet<String>) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let key = format!("n:{source}");
    let matches = |name: &str| filter.is_empty() || name.to_lowercase().contains(filter);
    let nodes: Vec<&crate::model::NodeView> =
        pic.nodes.iter().filter(|n| n.source == source && !n.name.contains("__next") && matches(&n.name)).collect();
    let rels: Vec<&crate::model::RelationView> =
        pic.relations.iter().filter(|r| r.source == source && matches(&r.name)).collect();
    if !filter.is_empty() && nodes.is_empty() && rels.is_empty() {
        return;
    }
    let open = app.tree_open.contains(&key) || !filter.is_empty();
    let detail = app.tree_detail.contains(&key);
    let k = app.colours.get(source).copied().unwrap_or(0);
    let outputs: Vec<String> =
        pic.nodes.iter().filter(|n| n.source == source && n.output).map(|n| n.name.clone()).collect();
    let sounding: Vec<String> = outputs.iter().filter(|o| playing.contains(*o)).cloned().collect();
    let mut act: Option<String> = None;
    let out = row(
        ui,
        0,
        true,
        open,
        detail,
        &network_sign(app, source),
        ink.source(k),
        RichText::new(source).color(ink.source(k)),
        Some({
            let sign = network_sign(app, source);
            let state = match sign.as_str() {
                "circle-check" => format!(" · {}", t(lang, "sb_in_main")),
                "trending-up" => format!(" · {}", t(lang, "sb_rising")),
                _ => String::new(),
            };
            format!("{} · {}{state}", nodes.len(), rels.len())
        }),
        |ui| {
            if !sounding.is_empty() {
                if crate::icons::sign(ui, "square", t(lang, "t_stop_net")).clicked() {
                    act = Some(sounding.iter().map(|o| format!("stop {o}")).collect::<Vec<_>>().join(" ; "));
                }
            } else if !outputs.is_empty() {
                if crate::icons::sign(ui, "play", t(lang, "t_play_net")).clicked() {
                    act = Some(outputs.iter().map(|o| format!("play {o}")).collect::<Vec<_>>().join(" ; "));
                }
            } else {
                crate::icons::sign_enabled(ui, false, "play", t(lang, "t_no_output"));
            }
            // to the front, and back with the others (alone, it has nothing to come in front of)
            let front = app.focus.as_deref() == Some(source);
            let others = app.picture.sources.len() > 1;
            let (sign, what) = if front { ("focus", t(lang, "t_unfocus")) } else { ("focus", t(lang, "t_focus")) };
            if crate::icons::sign_on(ui, others || front, front, sign, what).clicked() {
                act = Some(if front { "focus all".to_string() } else { format!("focus {source}") });
            }
        },
    );
    if let Some(a) = act {
        app.say(a);
    }
    if out.unfold {
        toggle(&mut app.tree_open, &key);
    }
    if out.label.clicked() {
        toggle(&mut app.tree_detail, &key);
    }
    if out.label.double_clicked() {
        app.dive(source);
    }
    out.label.context_menu(|ui| network_menu(app, ui, source, &outputs));
    if detail {
        indented(ui, 1, |ui| network_detail(app, ui, source, pic));
    }
    if !open {
        return;
    }
    // two folders: its nodes, its relations
    for (folder, sign, count, kind) in
        [("nodes", "folder", nodes.len(), "t_nodes"), ("rels", "folder", rels.len(), "t_relations")]
    {
        if count == 0 {
            continue;
        }
        let path = format!("{key}/{folder}");
        let open_f = app.tree_open.contains(&path) || !filter.is_empty();
        let out = row(
            ui,
            1,
            true,
            open_f,
            false,
            if open_f { "folder-open" } else { sign },
            ink.muted,
            RichText::new(t(lang, kind)).color(ink.muted),
            Some(count.to_string()),
            |_| {},
        );
        if out.unfold || out.label.clicked() {
            toggle(&mut app.tree_open, &path);
        }
        if !open_f {
            continue;
        }
        if folder == "nodes" {
            for n in nodes.iter().take(MOST) {
                node_row(app, ui, pic, &n.name, 2, &path, playing);
            }
        } else {
            for r in rels.iter().take(MOST) {
                relation_row(app, ui, pic, &r.name, 2, &path, playing);
            }
        }
        if count > MOST {
            indented(ui, 2, |ui| {
                ui.label(RichText::new(format!("… {} {}", count - MOST, t(lang, "t_more"))).small().color(ink.muted));
            });
        }
    }
}

fn node_row(
    app: &mut App,
    ui: &mut egui::Ui,
    pic: &Picture,
    name: &str,
    depth: usize,
    parent: &str,
    playing: &HashSet<String>,
) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(n) = pic.node(name) else { return };
    let path = format!("{parent}/node:{name}");
    let has = depth < 6 && pic.relations.iter().any(|r| r.nodes.iter().any(|x| x == name));
    let open = app.tree_open.contains(&path);
    let detail = app.selected.iter().any(|s| s == name);
    let k = app.colours.get(&n.source).copied().unwrap_or(0);
    let sounding = playing.contains(name);
    let mut act: Option<String> = None;
    let value = reading(n.value, n.sigma, n.signed);
    let out = row(ui, depth, has, open, detail, node_sign(n), ink.source(k), RichText::new(name), Some(value), |ui| {
        if sounding {
            if crate::icons::sign(ui, "square", t(lang, "t_stop_node")).clicked() {
                act = Some(format!("stop {name}"));
            }
        } else if crate::icons::sign(ui, "play", t(lang, "t_play_node")).clicked() {
            act = Some(format!("play {name}"));
        }
    });
    if app.tree_scroll.as_deref() == Some(&format!("node:{name}")) {
        out.label.scroll_to_me(Some(egui::Align::Center));
        app.tree_scroll = None;
    }
    if let Some(a) = act {
        app.say(a);
    }
    if out.unfold {
        toggle(&mut app.tree_open, &path);
    }
    if out.label.clicked() {
        app.select(name, true);
    }
    out.label.context_menu(|ui| crate::menu::node_menu(app, ui, name));
    if detail {
        let name = name.to_string();
        indented(ui, depth + 1, |ui| crate::panels::sheet(app, ui, &name, pic));
    }
    if open {
        let rels: Vec<String> =
            pic.relations.iter().filter(|r| r.nodes.iter().any(|x| x == name)).map(|r| r.name.clone()).collect();
        for r in rels {
            relation_row(app, ui, pic, &r, depth + 1, &path, playing);
        }
    }
}

fn relation_row(
    app: &mut App,
    ui: &mut egui::Ui,
    pic: &Picture,
    name: &str,
    depth: usize,
    parent: &str,
    playing: &HashSet<String>,
) {
    let ink = Ink::of(ui.ctx());
    let Some(r) = pic.relations.iter().find(|r| r.name == name).cloned() else { return };
    let path = format!("{parent}/rel:{name}");
    let key = format!("rel:{name}");
    let open = app.tree_open.contains(&path);
    let detail = app.tree_detail.contains(&key);
    let colour = if r.tension > 1.0 { ink.amber } else { ink.muted };
    let out = row(
        ui,
        depth,
        depth < 6,
        open,
        detail,
        "spline",
        colour,
        RichText::new(name),
        Some(format!("{:.1} σ", r.tension)),
        |_| {},
    );
    if app.tree_scroll.as_deref() == Some(&key) {
        out.label.scroll_to_me(Some(egui::Align::Center));
        app.tree_scroll = None;
    }
    if out.unfold {
        toggle(&mut app.tree_open, &path);
    }
    if out.label.clicked() {
        toggle(&mut app.tree_detail, &key);
    }
    out.label.context_menu(|ui| relation_menu(app, ui, &r));
    if detail {
        indented(ui, depth + 1, |ui| relation_detail(app, ui, &r));
    }
    if open {
        for n in r.nodes.iter().filter(|n| !n.contains("__next")) {
            node_row(app, ui, pic, n, depth + 1, &path, playing);
        }
    }
}

fn indented(ui: &mut egui::Ui, depth: usize, f: impl FnOnce(&mut egui::Ui)) {
    let ink = Ink::of(ui.ctx());
    let left = depth as f32 * INDENT + 22.0;
    let r = egui::Frame::new()
        .inner_margin(egui::Margin { left: left as i8, right: 4, top: 2, bottom: 8 })
        .show(ui, f)
        .response
        .rect;
    // a thread on the left: what this detail belongs to
    let x = r.left() + left - 10.0;
    ui.painter().line_segment([pos2(x, r.top() + 2.0), pos2(x, r.bottom() - 6.0)], Stroke::new(1.0_f32, ink.faint));
}

fn toggle(set: &mut HashSet<String>, key: &str) {
    if !set.remove(key) {
        set.insert(key.to_string());
    }
}

/// What a network is: its master equation (as mathematics, to copy as LaTeX) and its curve.
fn network_detail(app: &mut App, ui: &mut egui::Ui, source: &str, pic: &Picture) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let rels: Vec<&crate::model::RelationView> = pic.relations.iter().filter(|r| r.source == source).collect();
    let strained = rels.iter().filter(|r| r.tension > 1.0).count();
    let bits: f64 = rels.iter().map(|r| r.bits).sum();
    let rent: f64 = rels.iter().map(|r| r.rent).sum();
    ui.label(
        RichText::new(format!(
            "{} {} · {strained} {} · {} {} bits, {} {}",
            rels.len(),
            t(lang, "t_relations").to_lowercase(),
            t(lang, "t_strained"),
            t(lang, "brings"),
            grm_engine::lang::g(bits),
            t(lang, "rent"),
            grm_engine::lang::g(rent)
        ))
        .small()
        .color(ink.muted),
    );
    match app.masters.get(source).cloned() {
        Some(m) => {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "sigma", 14.0, ink.accent);
                crate::math::show(ui, &m.text(), 15.0, lang);
            });
            ui.label(
                RichText::new(format!(
                    "{} {} · {:.2} {} · {} {:.1e} dex",
                    m.relations,
                    t(lang, "t_gathered"),
                    m.decades,
                    t(lang, "t_decades"),
                    t(lang, "t_residual"),
                    m.residual
                ))
                .small()
                .color(ink.muted),
            );
            master_curve(app, ui, &m);
        }
        None => {
            ui.label(RichText::new(t(lang, "t_no_master")).small().color(ink.muted));
        }
    }
}

/// The master equation as a curve: the first of its nodes along, the second as it follows (the others held where
/// they are), in log on both sides (a power law is a straight line); where the network is now, a point.
fn master_curve(app: &App, ui: &mut egui::Ui, m: &grm_engine::master::Master) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    if m.powers.len() < 2 {
        return;
    }
    let value = |n: &str| app.picture.node(n).map(|v| v.value).filter(|v| *v > 0.0 && v.is_finite());
    let (x, px) = (&m.powers[0].0, m.powers[0].1);
    let (y, py) = (&m.powers[1].0, m.powers[1].1);
    let (Some(x0), Some(_)) = (value(x), value(y)) else { return };
    if py.abs() < 1e-12 {
        return;
    }
    // log10 y = (constant - px log x - sum of the others) / py
    let rest: f64 = m.powers[2..].iter().map(|(n, p)| p * value(n).unwrap_or(1.0).log10()).sum();
    let ly = |lx: f64| (m.constant - px * lx - rest) / py;
    let (lo, hi) = (x0.log10() - 1.0, x0.log10() + 1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(300.0), 110.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, crate::style::corner(), ink.surface);
    let inner = rect.shrink2(vec2(30.0, 16.0));
    let ys: Vec<f64> = (0..=40).map(|k| ly(lo + (hi - lo) * k as f64 / 40.0)).collect();
    let (ylo, yhi) = ys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
    let (ylo, yhi) = if yhi - ylo < 1e-9 { (ylo - 0.5, yhi + 0.5) } else { (ylo, yhi) };
    let at = |lx: f64, lyv: f64| {
        pos2(
            inner.left() + ((lx - lo) / (hi - lo)) as f32 * inner.width(),
            inner.bottom() - ((lyv - ylo) / (yhi - ylo)) as f32 * inner.height(),
        )
    };
    let pts: Vec<egui::Pos2> = ys.iter().enumerate().map(|(k, v)| at(lo + (hi - lo) * k as f64 / 40.0, *v)).collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.8_f32, ink.accent)));
    p.circle_filled(at(x0.log10(), ly(x0.log10())), 4.0, ink.text);
    let small = crate::style::font(crate::style::Size::Small);
    p.text(
        inner.left_bottom() + vec2(0.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("{x} (log)"),
        small.clone(),
        ink.muted,
    );
    p.text(
        inner.left_top() + vec2(-26.0, -14.0),
        egui::Align2::LEFT_TOP,
        format!("{y} (log)"),
        small.clone(),
        ink.muted,
    );
    p.text(rect.right_top() + vec2(-6.0, 3.0), egui::Align2::RIGHT_TOP, t(lang, "t_now_point"), small, ink.muted);
}

/// What a relation is: its formula (as mathematics), how far it is from holding, what it is worth, its strain lately.
fn relation_detail(app: &mut App, ui: &mut egui::Ui, r: &crate::model::RelationView) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    crate::math::show(ui, &r.text, 16.0, lang);
    let mut note = format!("{} · {} {:.2} σ", r.source, t(lang, "strain"), r.tension);
    if !r.awake {
        note.push_str(&format!(" · {}", t(lang, "asleep")));
    }
    if r.strength < 0.999 {
        note.push_str(&format!(" · {} ({:.0} %)", t(lang, "integrating"), r.strength * 100.0));
    }
    ui.label(RichText::new(note).small().color(ink.muted));
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
    let v: Vec<f32> =
        app.traces.get(&format!("rel:{}", r.name)).map(|t| t.iter().copied().collect()).unwrap_or_default();
    crate::hover::curve(ui, &v, ink.amber, &ink, t(lang, "its_strain"));
    if let Some((a, l)) = app.line_in_app(&r.name) {
        if crate::icons::button(ui, "pencil", format!("{a}, {} {l}", t(lang, "line")))
            .on_hover_text(t(lang, "edit_line"))
            .clicked()
        {
            let text = app
                .apps
                .iter()
                .find(|x| x.name == a)
                .and_then(|x| x.text.lines().nth(l - 1))
                .unwrap_or("")
                .trim()
                .to_string();
            app.line_draft = Some((Some(a), Some(l), text));
        }
    }
}

pub fn network_menu(app: &mut App, ui: &mut egui::Ui, source: &str, outputs: &[String]) {
    let lang = app.settings.lang;
    let ink = Ink::of(ui.ctx());
    ui.label(RichText::new(source).strong().color(ink.accent));
    ui.separator();
    if crate::menu::item(ui, "zoom-in", t(lang, "t_dive"), t(lang, "t_dive_d")) {
        app.dive(source);
        ui.close();
    }
    if crate::menu::item(ui, "focus", t(lang, "t_focus"), t(lang, "t_focus_d")) {
        app.say(format!("focus {source}"));
        ui.close();
    }
    if !outputs.is_empty() && crate::menu::item(ui, "play", t(lang, "t_play_net"), &outputs.join(", ")) {
        let line = outputs.iter().map(|o| format!("play {o}")).collect::<Vec<_>>().join(" ; ");
        app.say(line);
        ui.close();
    }
    if crate::menu::item(ui, "chart-line", t(lang, "m_plot"), t(lang, "t_plot_net_d")) {
        app.plotted = app.picture.nodes.iter().filter(|n| n.source == source).take(6).map(|n| n.name.clone()).collect();
        app.show_plot();
        ui.close();
    }
    if app.picture.quarantine.iter().any(|q| q == source) {
        if crate::menu::item(ui, "shield-check", t(lang, "t_release"), t(lang, "t_release_d")) {
            app.say(format!("release {source}"));
            ui.close();
        }
    } else if crate::menu::item(ui, "shield-alert", t(lang, "quarantine_source"), t(lang, "m_quarantine_d")) {
        app.say(format!("quarantine {source}"));
        ui.close();
    }
    let always = source.strip_prefix("app:").is_some_and(crate::elle::always);
    if !grm_engine::world::factory(source)
        && !always
        && crate::menu::item(ui, "trash-2", t(lang, "t_trash_net"), t(lang, "t_trash_net_d"))
    {
        app.ask_trash(source, false);
        ui.close();
    }
    if let Some(a) = source.strip_prefix("app:").filter(|_| !always) {
        let a = a.to_string();
        if crate::menu::item(ui, "x", t(lang, "close_app"), t(lang, "m_close_app_d")) {
            app.say(format!("app close {a}"));
            ui.close();
        }
        if crate::menu::item(ui, "trash-2", t(lang, "m_trash_app"), t(lang, "m_trash_app_d")) {
            app.ask_trash(&a, true);
            ui.close();
        }
    }
}

fn relation_menu(app: &mut App, ui: &mut egui::Ui, r: &crate::model::RelationView) {
    let lang = app.settings.lang;
    let ink = Ink::of(ui.ctx());
    ui.label(RichText::new(&r.name).strong().color(ink.accent));
    ui.separator();
    if let Some((a, l)) = app.line_in_app(&r.name) {
        if crate::menu::item(ui, "pencil", t(lang, "edit_line"), &format!("{a}, {} {l}", t(lang, "line"))) {
            let text = app
                .apps
                .iter()
                .find(|x| x.name == a)
                .and_then(|x| x.text.lines().nth(l - 1))
                .unwrap_or("")
                .trim()
                .to_string();
            app.line_draft = Some((Some(a), Some(l), text));
            ui.close();
        }
    }
    if crate::menu::item(ui, "copy", t(lang, "copy_latex"), t(lang, "t_copy_latex_d")) {
        if let Some(e) = crate::math::parse(&r.text) {
            ui.ctx().copy_text(crate::math::latex(&e));
        }
        ui.close();
    }
    if crate::menu::item(ui, "chart-line", t(lang, "m_plot"), t(lang, "t_plot_rel_d")) {
        app.plotted = r.nodes.iter().filter(|n| !n.contains("__next")).cloned().collect();
        app.show_plot();
        ui.close();
    }
    if !app.picture.quarantine.contains(&r.source)
        && crate::menu::item(
            ui,
            "shield-alert",
            t(lang, "quarantine_source"),
            &format!("{}: {}", r.source, t(lang, "m_quarantine_d")),
        )
    {
        app.say(format!("quarantine {}", r.source));
        ui.close();
    }
    if crate::menu::item(ui, "trash-2", t(lang, "remove"), t(lang, "m_remove_d")) {
        app.ask_remove(&r.name);
        ui.close();
    }
}
