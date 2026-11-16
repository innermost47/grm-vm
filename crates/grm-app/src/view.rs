// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The network view: nodes (circles) and relations (diamonds) linked as the machine holds them (a factor graph).
//! A colour per source, amber for a relation that strains, faded for what is in quarantine, dashed for what rises
//! little by little or sleeps. Click to inspect, Ctrl+click to add, Shift+drag a box, drag a node to put it
//! somewhere, wheel to zoom, drag the background to move, right click for what can be done.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke};

use crate::app::{App, Ink};
use crate::text::t;

pub struct View {
    pub offset: egui::Vec2,
    pub zoom: f32,
    box_from: Option<Pos2>,
    dragging: Option<String>,
    /// A relation being drawn (Alt+drag from a node): from which node.
    relating: Option<String>,
    menu_at: Option<(Pos2, Option<String>)>,
    pub fitted: bool,
    /// Shown small (on the space): the wheel is the space's, not this view's.
    pub passive: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            offset: vec2(0.0, 0.0),
            zoom: 1.0,
            box_from: None,
            dragging: None,
            relating: None,
            menu_at: None,
            fitted: false,
            passive: false,
        }
    }
}

fn faded(c: Color32, k: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * k) as u8)
}

fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 * (1.0 - k) + y as f32 * k) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The view of the network: all of it (`only` None), or what one source (an app) holds and the nodes it touches.
/// Below this zoom, networks begin to gather into one node each; below the second, they are only that.
const COLLAPSE_FROM: f32 = 0.42;
const COLLAPSE_FULL: f32 = 0.24;

pub fn show(app: &mut App, ui: &mut egui::Ui, only: Option<&str>) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, ink.surface);
    let mut pic = app.picture.clone();
    if let Some(src) = only {
        pic.relations.retain(|r| r.source == src);
        let touched: std::collections::HashSet<String> = pic.relations.iter().flat_map(|r| r.nodes.clone()).collect();
        pic.nodes.retain(|n| n.source == src || touched.contains(&n.name));
    }
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            true,
            format!(
                "network: {} nodes, {} relations, harmony {:.2}",
                pic.nodes.len(),
                pic.relations.len(),
                pic.harmony
            ),
        )
    });
    if pic.nodes.is_empty() {
        let mut y = rect.center().y - 40.0;
        painter.text(
            pos2(rect.center().x, y),
            egui::Align2::CENTER_CENTER,
            t(lang, "empty_world"),
            egui::FontId::proportional(15.0),
            ink.muted,
        );
        for ex in [
            "node mass = 2",
            "node force",
            "node acceleration = 9.81",
            "relate newton: force = mass * acceleration",
            "show",
        ] {
            y += 22.0;
            painter.text(
                pos2(rect.center().x, y),
                egui::Align2::CENTER_CENTER,
                ex,
                egui::FontId::monospace(13.0),
                ink.text,
            );
        }
        return;
    }
    // the points and links of the graph
    let quarantined = |s: &str| pic.quarantine.iter().any(|q| q == s);
    // an eye's glance is many inputs: they are drawn as one point (the eye)
    let mut groups: indexmap::IndexMap<String, (usize, String)> = indexmap::IndexMap::new();
    let mut items: Vec<String> = vec![];
    for n in &pic.nodes {
        let key = item_of(&n.name);
        if let Some(eye) = key.strip_prefix("g:") {
            let e = groups.entry(eye.to_string()).or_insert((0, n.source.clone()));
            e.0 += 1;
            if e.0 == 1 {
                items.push(key);
            }
        } else {
            items.push(key);
        }
    }
    let mut links: Vec<(String, String)> = vec![];
    for r in &pic.relations {
        items.push(format!("r:{}", r.name));
        for n in &r.nodes {
            let link = (format!("r:{}", r.name), item_of(n));
            if !links.contains(&link) {
                links.push(link);
            }
        }
    }
    let moving = app.graph.motion > 0.02 || app.graph.pos.len() != items.len() || app.view.dragging.is_some();
    if moving {
        app.graph.step(&items, &links);
    }
    // the first time: everything in view
    // until the human moves the view, it follows the network gently (everything in view)
    if !app.view.fitted && !items.is_empty() {
        glide_to_fit(app, rect);
    }
    let (zoom, centre) = (app.view.zoom, rect.center() + app.view.offset);
    let to_screen = |p: [f32; 2]| pos2(centre.x + p[0] * zoom, centre.y + p[1] * zoom);
    let from_screen = |s: Pos2| [(s.x - centre.x) / zoom, (s.y - centre.y) / zoom];
    let positions = app.graph.pos.clone();
    let at = |key: &str| positions.get(key).map(|p| to_screen(*p));
    let painter = painter.with_clip_rect(rect);
    // links
    for r in &pic.relations {
        let Some(rp) = at(&format!("r:{}", r.name)) else { continue };
        let q = quarantined(&r.source);
        let colour = if q { faded(ink.faint, 0.4) } else { ink.faint };
        let mut drawn: Vec<String> = vec![];
        for n in &r.nodes {
            let key = item_of(n);
            if drawn.contains(&key) {
                continue;
            }
            let Some(np) = at(&key) else { continue };
            drawn.push(key);
            if r.strength < 0.999 || !r.awake {
                painter.extend(Shape::dashed_line(&[rp, np], Stroke::new(1.0_f32, colour), 4.0, 4.0));
            } else {
                painter.line_segment([rp, np], Stroke::new(1.0_f32, colour));
            }
        }
    }
    // relations
    let pointer = resp.hover_pos();
    let mut hovered: Option<String> = None;
    // the node the pointer was pressed on (a drag starts once it has moved away from it)
    let origin = ui.input(|i| i.pointer.press_origin());
    let mut pressed_on: Option<String> = None;
    let mut hover_text: Option<String> = None;
    let mut hover_rel: Option<String> = None;
    for r in &pic.relations {
        let Some(p) = at(&format!("r:{}", r.name)) else { continue };
        let strain = app.tension.get(&r.name).copied().unwrap_or(0.0);
        let q = quarantined(&r.source);
        let mut colour = mix(ink.muted, ink.amber, (strain - 0.5) / 2.0);
        if q {
            colour = faded(colour, 0.35);
        }
        let s = 5.0 * zoom.clamp(0.6, 1.6);
        // a relation is a diamond, a limit (keep) a square
        let diamond = if r.keep {
            let h = s * 0.8;
            vec![pos2(p.x - h, p.y - h), pos2(p.x + h, p.y - h), pos2(p.x + h, p.y + h), pos2(p.x - h, p.y + h)]
        } else {
            vec![pos2(p.x, p.y - s), pos2(p.x + s, p.y), pos2(p.x, p.y + s), pos2(p.x - s, p.y)]
        };
        if r.awake {
            painter.add(Shape::convex_polygon(diamond, colour, Stroke::NONE));
        } else {
            painter.add(Shape::closed_line(diamond, Stroke::new(1.0_f32, colour)));
        }
        if pointer.is_some_and(|m| m.distance(p) < s + 4.0) {
            hover_rel = Some(r.name.clone());
        }
    }
    // nodes
    let label_all = zoom > 0.7 && pic.nodes.len() < 120;
    for (eye, (count, source)) in &groups {
        let Some(p) = at(&format!("g:{eye}")) else { continue };
        let k = app.colours.get(source).copied().unwrap_or(0);
        let r = 11.0 * zoom.clamp(0.6, 1.6);
        painter.circle_filled(p, r, ink.source(k));
        painter.circle_stroke(p, r + 3.0, Stroke::new(1.0_f32, ink.source(k)));
        let label = format!("{eye} · {count}");
        painter.text(
            pos2(p.x, p.y + r + 5.0),
            egui::Align2::CENTER_TOP,
            &label,
            egui::FontId::proportional(12.0),
            ink.muted,
        );
        if pointer.is_some_and(|m| m.distance(p) < r + 4.0) {
            hover_text = Some(format!("{eye}: {count} {}", t(lang, "glance_inputs")));
        }
    }
    for n in pic.nodes.iter().filter(|n| !n.name.contains("_glance_")) {
        let Some(p) = at(&format!("n:{}", n.name)) else { continue };
        let k = app.colours.get(&n.source).copied().unwrap_or(0);
        let mut colour = ink.source(k);
        if quarantined(&n.source) {
            colour = faded(colour, 0.35);
        }
        let r = 7.0 * zoom.clamp(0.6, 1.6);
        let in_focus = app.focus.as_ref().is_none_or(|f| *f == n.source);
        if !in_focus {
            colour = faded(colour, 0.25);
        }
        // a soft halo: the network it belongs to, around it
        painter.circle_filled(p, r * 2.6, colour.gamma_multiply(if in_focus { 0.13 } else { 0.04 }));
        if n.observed || n.fixed {
            painter.circle_filled(p, r, colour);
        } else {
            painter.circle_filled(p, r, ink.surface);
            painter.circle_stroke(p, r - 1.0, Stroke::new(2.0_f32, colour));
        }
        if n.tracked {
            painter.circle_stroke(p, r + 3.0, Stroke::new(1.0_f32, ink.muted));
        }
        let selected = app.selected.contains(&n.name);
        if selected {
            painter.circle_stroke(p, r + 6.0, Stroke::new(2.0_f32, ink.accent));
        }
        let over = pointer.is_some_and(|m| m.distance(p) < r + 4.0);
        if origin.is_some_and(|m| m.distance(p) < r + 4.0) {
            pressed_on = Some(n.name.clone());
        }
        if over {
            hovered = Some(n.name.clone());
            hover_rel = None;
        }
        if label_all || selected || over {
            painter.text(
                pos2(p.x, p.y + r + 3.0),
                egui::Align2::CENTER_TOP,
                &n.name,
                egui::FontId::proportional(12.0),
                ink.muted,
            );
        }
    }
    // far away, each network becomes one node holding its master equation; coming closer, it unfolds again
    // (from the infinitely large to the small: a double click goes into one)
    let mut over_net: Option<String> = None;
    let collapse =
        if only.is_none() { ((COLLAPSE_FROM - zoom) / (COLLAPSE_FROM - COLLAPSE_FULL)).clamp(0.0, 1.0) } else { 0.0 };
    let mut sources: Vec<String> = vec![];
    for n in &pic.nodes {
        if !sources.contains(&n.source) {
            sources.push(n.source.clone());
        }
    }
    if collapse > 0.0 && sources.len() >= 2 {
        painter.rect_filled(rect, 0.0, faded(ink.surface, 0.92 * collapse));
        let mut centres: indexmap::IndexMap<String, (Pos2, f32, usize)> = indexmap::IndexMap::new();
        for s in &sources {
            let pts: Vec<Pos2> =
                pic.nodes.iter().filter(|n| &n.source == s).filter_map(|n| at(&item_of(&n.name))).collect();
            if pts.is_empty() {
                continue;
            }
            let c = pts.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / pts.len() as f32;
            let spread = pts.iter().map(|p| p.distance(c.to_pos2())).fold(0.0, f32::max);
            let radius = (18.0 + (pts.len() as f32).sqrt() * 6.0).max(spread * 0.5).min(90.0);
            centres.insert(s.clone(), (c.to_pos2(), radius, pts.len()));
        }
        // gathered, they must not lie on one another: they push each other apart (a few rounds), keeping room
        // for their names
        {
            let keys: Vec<String> = centres.keys().cloned().collect();
            for _ in 0..60 {
                let mut moved = false;
                for a in 0..keys.len() {
                    for b in a + 1..keys.len() {
                        let (pa, ra, _) = centres[&keys[a]];
                        let (pb, rb, _) = centres[&keys[b]];
                        let need = ra + rb + 36.0;
                        let mut d = pb - pa;
                        if d.length() < 1e-3 {
                            d = vec2(1.0, 0.3 * (b as f32 - a as f32));
                        }
                        let dist = d.length();
                        if dist < need {
                            let push = d / dist * (need - dist) / 2.0;
                            centres.get_mut(&keys[a]).unwrap().0 -= push;
                            centres.get_mut(&keys[b]).unwrap().0 += push;
                            moved = true;
                        }
                    }
                }
                if !moved {
                    break;
                }
            }
        }
        // what joins networks: relations that hold nodes of several, as thick as how many
        let mut joins: indexmap::IndexMap<(String, String), usize> = indexmap::IndexMap::new();
        for r in &pic.relations {
            let mut srcs: Vec<String> = r.nodes.iter().filter_map(|n| pic.node(n).map(|v| v.source.clone())).collect();
            srcs.sort();
            srcs.dedup();
            for a in 0..srcs.len() {
                for b in a + 1..srcs.len() {
                    *joins.entry((srcs[a].clone(), srcs[b].clone())).or_insert(0) += 1;
                }
            }
        }
        for ((a, b), n) in &joins {
            if let (Some(pa), Some(pb)) = (centres.get(a), centres.get(b)) {
                let w = (1.0 + (*n as f32).sqrt() * 1.5).min(8.0);
                painter.line_segment([pa.0, pb.0], Stroke::new(w, faded(ink.muted, collapse)));
            }
        }
        for (s, (c, radius, count)) in &centres {
            let k = app.colours.get(s).copied().unwrap_or(0);
            let colour = ink.source(k);
            let over = pointer.is_some_and(|m| m.distance(*c) < *radius);
            if over {
                over_net = Some(s.clone());
            }
            painter.circle_filled(*c, *radius, faded(colour, (if over { 0.35 } else { 0.22 }) * collapse));
            painter.circle_stroke(
                *c,
                *radius,
                Stroke::new(if over { 2.5_f32 } else { 1.5_f32 }, faded(colour, collapse)),
            );
            painter.text(
                *c + vec2(0.0, *radius + 4.0),
                egui::Align2::CENTER_TOP,
                s,
                egui::FontId::proportional(13.0),
                faded(ink.text, collapse),
            );
            painter.text(
                *c,
                egui::Align2::CENTER_CENTER,
                format!("{count} {}", t(lang, "all_nodes")),
                egui::FontId::proportional(10.0),
                faded(ink.muted, collapse),
            );
        }
        if collapse > 0.5 {
            hovered = None;
            hover_rel = None;
            pressed_on = None;
        }
    }
    if resp.double_clicked() {
        if let Some(net) = over_net.clone() {
            app.dive(&net);
        } else if let Some(n) = hovered.as_ref().and_then(|n| pic.node(n)).filter(|_| only.is_none()) {
            let s = n.source.clone();
            app.dive(&s);
        }
    } else if resp.clicked() {
        if let Some(net) = &over_net {
            let key = format!("n:{net}");
            app.tree_detail.insert(key.clone());
            app.tree_scroll = Some(key);
            app.tab = crate::app::Tab::Inspector;
            app.side_open = true;
        }
    }
    // the legend, quiet, in a corner: the shapes drawn as in the view, then their words
    legend(&painter, rect, &ink, lang);
    let _ = &hover_text;
    let menu_open = egui::Popup::is_any_open(ui.ctx());
    if app.view.dragging.is_none() && app.view.box_from.is_none() && app.view.relating.is_none() && !menu_open {
        let tip = egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("net-tip"),
            egui::PopupAnchor::Pointer,
        );
        if let Some(net) = &over_net {
            let net = net.clone();
            tip.show(|ui| {
                ui.label(RichText::new(&net).strong().size(15.0));
                ui.label(RichText::new(t(lang, "t_row_hint")).small().color(ink.muted));
                if let Some(m) = app.masters.get(&net).cloned() {
                    crate::math::show(ui, &m.text(), 14.0, lang);
                }
            });
        } else if let Some(n) = &hovered {
            let n = n.clone();
            tip.show(|ui| crate::hover::node(app, ui, &n));
        } else if let Some(r) = &hover_rel {
            let r = r.clone();
            tip.show(|ui| crate::hover::relation(app, ui, &r));
        } else if let Some(text) = &hover_text {
            tip.show(|ui| {
                ui.label(text);
            });
        }
    }
    // the master equation, top left, and the one of what is shown
    {
        let mut y = rect.top() + 8.0;
        // drawn as mathematics (right click: LaTeX), what it gathers said beside it
        for (key, m, colour) in
            [("master_now", app.master.0.clone(), ink.text), ("master_here", app.master.1.clone(), ink.muted)]
        {
            let Some((eq, note)) = m else { continue };
            let area = Rect::from_min_size(pos2(rect.left() + 6.0, y), vec2(rect.width() - 12.0, 90.0));
            let mut child = ui
                .new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(egui::Align::Min)));
            let inner = egui::Frame::new()
                .fill(faded(ink.surface, 0.85))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(&mut child, |ui| {
                    ui.horizontal(|ui| {
                        crate::icons::show(ui, "sigma", 14.0, colour);
                        ui.label(egui::RichText::new(t(lang, key)).small().color(colour));
                        crate::math::show(ui, &eq, 14.0, lang);
                        ui.label(egui::RichText::new(note).small().color(ink.muted));
                    });
                });
            y = inner.response.rect.bottom() + 4.0;
        }
        // the networks in the network: a chip each, to bring one to the front
        let mut sources: Vec<String> = pic.nodes.iter().map(|n| n.source.clone()).collect();
        sources.dedup();
        let mut seen = std::collections::HashSet::new();
        sources.retain(|s| seen.insert(s.clone()));
        let mut x = rect.left() + 10.0;
        y += 4.0;
        for s in std::iter::once(String::new()).chain(sources) {
            let label = if s.is_empty() { t(lang, "all").to_string() } else { s.clone() };
            let on = if s.is_empty() { app.focus.is_none() } else { app.focus.as_deref() == Some(s.as_str()) };
            let colour = if s.is_empty() { ink.muted } else { ink.source(app.colours.get(&s).copied().unwrap_or(0)) };
            let g = painter.layout_no_wrap(
                label,
                egui::FontId::proportional(12.0),
                if on { ink.on_accent() } else { colour },
            );
            let chip = Rect::from_min_size(pos2(x, y), g.size() + vec2(14.0, 6.0));
            painter.rect_filled(chip, 9.0, if on { colour } else { faded(colour, 0.14) });
            painter.galley(chip.min + vec2(7.0, 3.0), g, if on { ink.on_accent() } else { colour });
            let r = ui.interact(chip, egui::Id::new(("focus-chip", &s)), Sense::click());
            if r.double_clicked() && !s.is_empty() {
                app.dive(&s);
            } else if r.clicked() {
                app.focus = if s.is_empty() { None } else { Some(s.clone()) };
            }
            // under the right button: what can be done with this network (go into it, play, quarantine, remove...)
            if !s.is_empty() {
                let outputs: Vec<String> =
                    pic.nodes.iter().filter(|n| n.source == s && n.output).map(|n| n.name.clone()).collect();
                r.on_hover_text(t(lang, "t_row_hint")).context_menu(|ui| {
                    ui.set_min_width(280.0);
                    crate::tree::network_menu(app, ui, &s, &outputs)
                });
            }
            x = chip.right() + 6.0;
            if x > rect.right() - 80.0 {
                break;
            }
        }
    }
    // what the pointer does
    let (shift, command, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.command, i.modifiers.alt));
    // a relation is drawn by dragging from a node to another with the right button (or with Alt held)
    let right = resp.drag_started_by(egui::PointerButton::Secondary);
    if resp.drag_started() {
        if (alt || right) && pressed_on.is_some() {
            app.view.relating = pressed_on.clone();
        } else if right {
            // a right drag on nothing does nothing
        } else if shift {
            app.view.box_from = origin.or(resp.interact_pointer_pos());
        } else if let Some(n) = &pressed_on {
            let before = app.layout_change();
            app.journey.record(before, format!("{} {n}", crate::text::t(app.settings.lang, "moved")));
            app.view.dragging = Some(n.clone());
        }
    }
    if let (Some(from), Some(m)) = (app.view.relating.clone(), resp.interact_pointer_pos()) {
        if let Some(p) = at(&format!("n:{from}")) {
            painter.extend(Shape::dashed_line(&[p, m], Stroke::new(2.0_f32, ink.accent), 6.0, 4.0));
        }
        if resp.drag_stopped() {
            if let Some(to) = hovered.clone().filter(|to| *to != from) {
                app.start_relation(&from, &to);
            }
            app.view.relating = None;
        }
    } else if resp.dragged() {
        if let Some(n) = &app.view.dragging {
            if let Some(m) = resp.interact_pointer_pos() {
                let key = format!("n:{n}");
                app.graph.pos.insert(key.clone(), from_screen(m));
                app.graph.pinned.insert(key);
                app.graph.motion = 1.0;
            }
        } else if app.view.box_from.is_none() && resp.dragged_by(egui::PointerButton::Primary) {
            app.view.offset += resp.drag_delta();
            app.view.fitted = true;
        }
    }
    if let (Some(from), Some(to)) = (app.view.box_from, resp.interact_pointer_pos()) {
        let b = Rect::from_two_pos(from, to);
        painter.rect_stroke(b, 2.0, Stroke::new(1.0_f32, ink.accent), egui::StrokeKind::Inside);
        if resp.drag_stopped() {
            let inside: Vec<String> = pic
                .nodes
                .iter()
                .filter(|n| at(&format!("n:{}", n.name)).is_some_and(|p| b.contains(p)))
                .map(|n| n.name.clone())
                .collect();
            if !command {
                app.selected.clear();
            }
            for n in inside {
                if !app.selected.contains(&n) {
                    app.selected.push(n);
                }
            }
            app.view.box_from = None;
        }
    }
    if resp.drag_stopped() {
        app.view.dragging = None;
        app.view.box_from = None;
    }
    if resp.clicked() && app.relate_from.is_some() {
        if let (Some(from), Some(to)) = (app.relate_from.take(), hovered.clone()) {
            if from != to {
                app.start_relation(&from, &to);
            }
        }
    } else if resp.clicked() {
        match &hovered {
            Some(n) => app.select(n, command),
            // a relation clicked opens in the inspector, as a node does
            None if hover_rel.is_some() => app.show_relation(hover_rel.as_deref().unwrap_or_default()),
            None if !command => app.selected.clear(),
            None => {}
        }
    }
    // the wheel, Ctrl and the wheel, two fingers apart on a touchpad: all zoom (even with a card open over it)
    if resp.contains_pointer() && !app.view.passive {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (1.0 + scroll * 0.0015) * pinch;
        if (factor - 1.0).abs() > 1e-4 {
            let old = app.view.zoom;
            let new = (old * factor).clamp(0.1, 6.0);
            // the point under the pointer stays where it is
            if let Some(m) = pointer {
                let c = rect.center() + app.view.offset;
                let rel = m - c;
                app.view.offset += rel - rel * (new / old);
            }
            app.view.zoom = new;
            app.view.fitted = true;
        }
    }
    if resp.secondary_clicked() {
        app.view.menu_at = resp.interact_pointer_pos().map(|p| (p, hovered.clone()));
    }
    let menu_target = app.view.menu_at.as_ref().and_then(|(_, n)| n.clone());
    let menu_at = app.view.menu_at.as_ref().map(|(p, _)| from_screen(*p)).unwrap_or([0.0, 0.0]);
    resp.context_menu(|ui| {
        ui.set_min_width(280.0);
        match &menu_target {
            Some(n) => crate::menu::node_menu(app, ui, n),
            None => crate::menu::background_menu(app, ui, menu_at),
        }
    });
    if let Some(from) = &app.relate_from {
        painter.text(
            rect.center_top() + vec2(0.0, 30.0),
            egui::Align2::CENTER_TOP,
            format!("{from} → {}", t(lang, "click_target")),
            egui::FontId::proportional(13.0),
            ink.accent,
        );
    }
    if app.graph.motion > 0.02 {
        ui.ctx().request_repaint();
    }
}

/// The point a node is drawn as: itself, or its eye when it is one of the eye's glance inputs.
fn item_of(node: &str) -> String {
    match node.split_once("_glance_") {
        Some((eye, _)) => format!("g:{eye}"),
        None => format!("n:{node}"),
    }
}

fn legend(painter: &egui::Painter, rect: Rect, ink: &Ink, lang: crate::text::Lang) {
    let font = egui::FontId::proportional(11.0);
    let y = rect.bottom() - 12.0;
    let back = Rect::from_min_max(pos2(rect.left(), y - 10.0), pos2(rect.right(), rect.bottom()));
    painter.rect_filled(back, 0.0, faded(ink.surface, 0.85));
    let mut x = rect.left() + 12.0;
    let word = |x: &mut f32, key: &str| {
        let r = painter.text(pos2(*x, y), egui::Align2::LEFT_CENTER, t(lang, key), font.clone(), ink.muted);
        *x = r.right() + 14.0;
    };
    painter.circle_filled(pos2(x, y), 4.0, ink.muted);
    x += 8.0;
    word(&mut x, "legend_observed");
    painter.circle_stroke(pos2(x, y), 3.5, Stroke::new(1.5_f32, ink.muted));
    x += 8.0;
    word(&mut x, "legend_computed");
    let d = 4.0;
    painter.add(Shape::convex_polygon(
        vec![pos2(x, y - d), pos2(x + d, y), pos2(x, y + d), pos2(x - d, y)],
        ink.muted,
        Stroke::NONE,
    ));
    x += 8.0;
    word(&mut x, "legend_relation");
    painter.rect_filled(Rect::from_center_size(pos2(x, y), vec2(6.5, 6.5)), 0.0, ink.muted);
    x += 8.0;
    word(&mut x, "legend_keep");
    painter.add(Shape::convex_polygon(
        vec![pos2(x, y - d), pos2(x + d, y), pos2(x, y + d), pos2(x - d, y)],
        ink.amber,
        Stroke::NONE,
    ));
    x += 8.0;
    word(&mut x, "legend_strain");
    painter.extend(Shape::dashed_line(
        &[pos2(x - 4.0, y), pos2(x + 10.0, y)],
        Stroke::new(1.0_f32, ink.muted),
        3.0,
        3.0,
    ));
    x += 14.0;
    word(&mut x, "legend_dashed");
    painter.circle_filled(pos2(x, y), 4.0, faded(ink.muted, 0.35));
    x += 8.0;
    word(&mut x, "legend_faded");
}

fn glide_to_fit(app: &mut App, rect: Rect) {
    let (zoom, offset) = (app.view.zoom, app.view.offset);
    fit(app, rect);
    let k = 0.15;
    app.view.zoom = zoom + (app.view.zoom - zoom) * k;
    app.view.offset = offset + (app.view.offset - offset) * k;
    if (app.view.zoom - zoom).abs() > 1e-3 || (app.view.offset - offset).length() > 0.3 {
        app.graph.motion = app.graph.motion.max(0.03);
    }
}

fn fit(app: &mut App, rect: Rect) {
    if let Some(b) = app.graph.bounds() {
        let w = (b[2] - b[0]).max(1.0) + 80.0;
        let h = (b[3] - b[1]).max(1.0) + 80.0;
        let zoom = (rect.width() / w).min(rect.height() / h).clamp(0.2, 2.0);
        app.view.zoom = zoom;
        let c = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
        app.view.offset = vec2(-c[0] * zoom, -c[1] * zoom);
    }
}

/// value ± sigma, as a human reads it: a positive node's spread is a factor (shown as a percentage).
pub fn reading(value: f64, sigma: f64, signed: bool) -> String {
    let v = grm_engine::lang::g(value);
    if !sigma.is_finite() || (!signed && sigma > 6.0) || (signed && sigma > 1e4 * value.abs().max(1.0)) {
        // a value it starts from, not one it knows: said as such
        if value.is_finite() {
            format!("≈ {v} (a guess: nothing measured it yet)")
        } else {
            "? (nothing measured it yet)".to_string()
        }
    } else if signed {
        format!("{v} ± {}", grm_engine::lang::g(sigma))
    } else if sigma < 0.3 {
        format!("{v} ± {:.2} %", (10f64.powf(sigma) - 1.0) * 100.0)
    } else {
        format!("{v} (×/÷ {})", grm_engine::lang::g(10f64.powf(sigma)))
    }
}
