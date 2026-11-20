// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The network view: nodes (circles) and relations (diamonds) linked as the machine holds them (a factor graph).
//! A colour per source, amber for a relation that strains, faded for what is in quarantine, dashed for what rises
//! little by little or sleeps. Click to inspect, Ctrl+click to add, Shift+drag a box, drag a node to put it
//! somewhere, wheel to zoom, drag the background to move, right click for what can be done.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke};

use crate::app::{App, Ink};
use crate::text::t;

#[derive(Clone)]
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
    /// How far down the master equations reach (from the top of the view): the triangle stays below them.
    eq_below: f32,
    /// How tall one of them is (measured as drawn).
    eq_row: f32,
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
            eq_below: 0.0,
            eq_row: 30.0,
        }
    }
}

fn faded(c: Color32, k: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * k) as u8)
}

pub(crate) fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 * (1.0 - k) + y as f32 * k) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The view of the network: all of it (`only` None), or what one source (an app) holds and the nodes it touches.
/// Below this zoom, networks begin to gather into one node each; below the second, they are only that.
const COLLAPSE_FROM: f32 = 0.42;
const COLLAPSE_FULL: f32 = 0.24;
/// Below this zoom, the nodes a `repeat` wrote are seen as their packet (one point each).
const PACKETS_FROM: f32 = 0.7;

/// What a view shows of the picture: its nodes and relations (borrowed, not copied).
struct Shown<'a> {
    nodes: Vec<&'a crate::model::NodeView>,
    relations: Vec<&'a crate::model::RelationView>,
    #[allow(dead_code)]
    full: &'a crate::model::Picture,
}

/// The labels already placed, by squares of the screen: a new one is looked at only against those near it.
#[derive(Default)]
struct Placed {
    cells: std::collections::HashMap<(i32, i32), Vec<Rect>>,
}

impl Placed {
    const CELL: f32 = 80.0;
    fn cells_of(r: Rect) -> impl Iterator<Item = (i32, i32)> {
        let (x0, x1) = ((r.left() / Self::CELL).floor() as i32, (r.right() / Self::CELL).floor() as i32);
        let (y0, y1) = ((r.top() / Self::CELL).floor() as i32, (r.bottom() / Self::CELL).floor() as i32);
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }
    fn free(&self, r: Rect) -> bool {
        let r = r.expand(2.0);
        !Self::cells_of(r).any(|c| self.cells.get(&c).is_some_and(|v| v.iter().any(|o| o.intersects(r))))
    }
    fn put(&mut self, r: Rect) {
        for c in Self::cells_of(r) {
            self.cells.entry(c).or_default().push(r);
        }
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui, only: Option<&str>) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, ink.surface);
    // what this view needs of the picture, made once for each new picture (not at every frame)
    let prep = app.prepared(only);
    let full = app.picture.clone();
    let pic = Shown {
        nodes: prep.nodes.iter().map(|&k| &full.nodes[k]).collect(),
        relations: prep.rels.iter().map(|&i| &full.relations[i]).collect(),
        full: &full,
    };
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            true,
            format!(
                "network: {} nodes, {} relations, harmony {:.2}",
                pic.nodes.len(),
                pic.relations.len(),
                full.harmony
            ),
        )
    });
    if pic.nodes.is_empty() {
        let mut y = rect.center().y - 40.0;
        painter.text(
            pos2(rect.center().x, y),
            egui::Align2::CENTER_CENTER,
            t(lang, "empty_world"),
            crate::style::font(crate::style::Size::Subtitle),
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
                crate::style::code(crate::style::Size::Text),
                ink.text,
            );
        }
        return;
    }
    // the points and links of the graph
    let quarantined = |s: &str| full.quarantine.iter().any(|q| q == s);
    // an eye's glance is many inputs: they are drawn as one point (the eye)
    let groups = &prep.groups;
    let items = &prep.items;
    // the layout is the world's: what is gone from the world (an app closed) leaves no place behind, and it takes
    // one step a frame, whatever the views that show it
    let pass = ui.ctx().cumulative_pass_nr();
    let world = app.prepared(None);
    if app.layout_pass != pass && !app.flow.on {
        app.layout_pass = pass;
        let missing = world.items.iter().any(|k| !app.graph.pos.contains_key(k));
        if missing || app.graph.pos.len() > world.items.len() + app.graph.pinned.len() {
            let here: std::collections::HashSet<&String> = world.items.iter().collect();
            let pinned = app.graph.pinned.clone();
            app.graph.pos.retain(|k, _| here.contains(k) || pinned.contains(k));
        }
        let moving = app.graph.motion > 0.02 || missing || app.view.dragging.is_some();
        if moving {
            app.graph.step(&world.items, &world.links);
        }
    }
    // what is near the pointer stays still (to take it, or stop it, without a fight)
    app.graph.held.clear();
    if let Some(m) = resp.hover_pos() {
        let (zoom, centre) = (app.view.zoom, rect.center() + app.view.offset);
        let reach = 90.0 / zoom;
        let here = [(m.x - centre.x) / zoom, (m.y - centre.y) / zoom];
        for (k, p) in &app.graph.pos {
            if (p[0] - here[0]).hypot(p[1] - here[1]) < reach {
                app.graph.held.insert(k.clone());
            }
        }
    }
    // laid out as a flow: each point glides to its place (inputs left, what is heard right); what passes through
    // each node now is read from the sound itself
    let played: std::collections::HashSet<String> =
        app.mixer.lock().map(|m| m.playing().into_iter().collect()).unwrap_or_default();
    if app.flow.on {
        let shown = crate::model::Picture {
            nodes: pic.nodes.iter().map(|n| (*n).clone()).collect(),
            relations: pic.relations.iter().map(|r| (*r).clone()).collect(),
            ..Default::default()
        };
        let targets = crate::flow::layout(&shown, &played);
        let mut moved = 0.0f32;
        for (k, t) in &targets {
            if app.graph.held.contains(k) {
                continue;
            }
            let p = app.graph.pos.entry(k.clone()).or_insert(*t);
            let d = [t[0] - p[0], t[1] - p[1]];
            // slowly: a place that changes is reached in a few seconds, never in a jump
            p[0] += 0.04 * d[0];
            p[1] += 0.04 * d[1];
            moved = moved.max(d[0].abs() + d[1].abs());
        }
        app.graph.motion = if moved > 0.5 { 0.03 } else { 0.0 };
        if moved > 0.5 {
            ui.ctx().request_repaint();
        }
        let values: Vec<(String, f64)> = match (app.mixer.lock(), played.iter().next()) {
            (Ok(m), Some(ch)) => {
                pic.nodes.iter().filter_map(|n| m.value(ch, &n.name).map(|v| (n.name.clone(), v))).collect()
            }
            _ => vec![],
        };
        if !values.is_empty() {
            app.flow.listen(&values);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
    // the first time: everything in view
    // until the human moves the view, it follows the network gently (everything in view)
    if !app.view.fitted && !items.is_empty() {
        glide_to_fit(app, rect, items);
    }
    let (zoom, centre) = (app.view.zoom, rect.center() + app.view.offset);
    let to_screen = |p: [f32; 2]| pos2(centre.x + p[0] * zoom, centre.y + p[1] * zoom);
    let from_screen = |s: Pos2| [(s.x - centre.x) / zoom, (s.y - centre.y) / zoom];
    // where each of its points is on the screen, read once (the world's places are not copied at every frame)
    let screen: std::collections::HashMap<&str, Pos2> =
        items.iter().filter_map(|k| app.graph.pos.get(k).map(|p| (k.as_str(), to_screen(*p)))).collect();
    let at = |key: &str| screen.get(key).copied();
    // what is drawn: what is on the screen (and a little around it); far away, the packets; farther, the networks
    let vis = rect.expand(40.0);
    let collapse = if only.is_none() && !app.flow.on {
        ((COLLAPSE_FROM - zoom) / (COLLAPSE_FROM - COLLAPSE_FULL)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let detail = collapse < 0.999 || prep.counts.len() < 2;
    let far = zoom < PACKETS_FROM && !app.flow.on && !prep.packets.is_empty();
    let packet_at: Vec<Option<Pos2>> = prep
        .packets
        .iter()
        .map(|pk| {
            let pts: Vec<Pos2> = pk.members.iter().filter_map(|&m| at(&prep.node_key[m])).collect();
            (!pts.is_empty())
                .then(|| (pts.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / pts.len() as f32).to_pos2())
        })
        .collect();
    let end_at = |e: &crate::prepared::End| match e {
        crate::prepared::End::Item(k) => at(k),
        crate::prepared::End::Packet(p) => packet_at.get(*p).copied().flatten(),
    };
    let painter = painter.with_clip_rect(rect);
    // the triangle: the container in which the content is reflected, and the content reflecting the container (two,
    // and their relation: three). It is no centre of the network: it stays above it, at the top and in the middle of
    // the view, and the light goes both ways through it, coming back changed
    let below = (app.view.eq_below + 30.0).max(48.0);
    let witness_at = (!screen.is_empty()).then(|| rect.center_top() + vec2(0.0, below));
    if let Some(c) = witness_at {
        let g = &full.glance;
        let passing: Vec<String> = g
            .surprised
            .iter()
            .map(|(r, _)| format!("r:{r}"))
            .chain(g.moved.iter().map(|(n, _, _)| item_of(n)))
            .collect();
        // a faint thread from each node; on it a grain that goes in, and later another that comes back out, of
        // another colour and slower (what was received, given back changed); none of them change anything
        let time = ui.input(|i| i.time);
        let faint = ink.accent.gamma_multiply(0.06);
        let inwards = ink.accent.gamma_multiply(0.32);
        let back = mix(ink.text, ink.accent, 0.25).gamma_multiply(0.22);
        for (k, p) in prep.node_key.iter().filter(|k| k.starts_with("n:")).filter_map(|k| at(k)).take(240).enumerate() {
            painter.line_segment([p, c], Stroke::new(0.5_f32, faint));
            let f = ((time / 7.0 + k as f64 * 0.137) % 1.0) as f32;
            painter.circle_filled(p + (c - p) * f, 1.3, inwards.gamma_multiply(0.4 + 0.6 * f));
            let o = ((time / 11.0 + k as f64 * 0.291) % 1.0) as f32;
            painter.circle_filled(c + (p - c) * o, 1.6, back.gamma_multiply(1.0 - 0.6 * o));
            // the node, touched by what came back: a ring, barely
            if o > 0.9 {
                painter.circle_stroke(
                    p,
                    6.0 + 8.0 * (o - 0.9),
                    Stroke::new(0.6_f32, back.gamma_multiply(1.0 - (o - 0.9) * 10.0)),
                );
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        let thread = ink.accent.gamma_multiply(0.45);
        for key in &passing {
            if let Some(p) = at(key) {
                // the thread goes through the eye of the needle, and on beyond it
                let beyond = c + (c - p).normalized() * 22.0;
                painter.line_segment([p, beyond], Stroke::new(1.0_f32, thread));
            }
        }
    }
    // links: each relation to its nodes; from afar, what joins the packets (once each, as thick as how many)
    if detail && far {
        for (a, b, n) in &prep.far_links {
            let (Some(pa), Some(pb)) = (end_at(a), end_at(b)) else { continue };
            if !vis.contains(pa) && !vis.contains(pb) {
                continue;
            }
            let w = (1.0 + (*n as f32).ln() * 0.5).min(3.5);
            painter.line_segment([pa, pb], Stroke::new(w, ink.faint));
        }
    } else if detail {
        for (i, r) in pic.relations.iter().enumerate() {
            let Some(rp) = at(&prep.rel_key[i]) else { continue };
            let q = quarantined(&r.source);
            let colour = if q { faded(ink.faint, 0.4) } else { ink.faint };
            for key in &prep.rel_ends[i] {
                let Some(np) = at(key) else { continue };
                if !vis.contains(rp) && !vis.contains(np) {
                    continue;
                }
                // in a flow, what passes through it now: a link brighter and thicker with it
                let n = key.strip_prefix("n:").unwrap_or(key);
                let live = if app.flow.on { app.flow.live.get(n).copied().unwrap_or(0.0) } else { 0.0 };
                if r.strength < 0.999 || !r.awake {
                    painter.extend(Shape::dashed_line(&[rp, np], Stroke::new(1.0_f32, colour), 4.0, 4.0));
                } else if live > 0.02 {
                    painter.line_segment([rp, np], Stroke::new(1.0 + 2.5 * live, mix(colour, ink.accent, live)));
                } else {
                    painter.line_segment([rp, np], Stroke::new(1.0_f32, colour));
                }
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
    for (i, r) in pic.relations.iter().enumerate() {
        if !detail || (far && prep.rel_hidden[i]) {
            continue;
        }
        let Some(p) = at(&prep.rel_key[i]) else { continue };
        if !vis.contains(p) {
            continue;
        }
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
    if let Some(c) = witness_at {
        // the triangle, over the threads and the links, under the nodes and the networks gathered: empty (nothing is kept in it), a second fainter one around it, the reflection
        let r = 17.0;
        let tri = |r: f32, turn: f32| -> Vec<Pos2> {
            (0..3)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + turn + k as f32 * std::f32::consts::TAU / 3.0;
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect()
        };
        painter.add(Shape::convex_polygon(
            tri(r, 0.0),
            ink.surface,
            Stroke::new(1.6_f32, ink.accent.gamma_multiply(0.85)),
        ));
        painter.add(Shape::closed_line(
            tri(r + 7.0, std::f32::consts::PI),
            Stroke::new(0.7_f32, ink.accent.gamma_multiply(0.22)),
        ));
        if pointer_over(&resp, c, r + 6.0) {
            resp.clone().on_hover_ui(|ui| {
                let lang = app.settings.lang;
                let ink = crate::app::Ink::of(ui.ctx());
                ui.horizontal(|ui| {
                    crate::icons::show(ui, "triangle", 15.0, ink.accent);
                    ui.label(egui::RichText::new(crate::text::t(lang, "witness")).strong());
                });
                ui.label(egui::RichText::new(crate::text::t(lang, "witness_d")).small().color(ink.muted));
                ui.separator();
                crate::panels::witness_lines(app, ui);
                ui.separator();
                crate::panels::witness_equations(app, ui);
            });
        }
    }
    // nodes
    // every label where there is room (the ones that would fall on others wait for the pointer)
    let label_all = zoom > 0.45 && pic.nodes.len() < 3000;
    // the labels already placed: a label that would fall on another goes above its node, or waits for the pointer
    let mut placed = Placed::default();
    for (eye, (count, source)) in groups.iter() {
        let Some(p) = at(&format!("g:{eye}")) else { continue };
        let k = app.colours.get(source).copied().unwrap_or(0);
        let r = 11.0 * zoom.clamp(0.6, 1.6);
        let colour =
            if app.focus.as_ref().is_none_or(|f| f == source) { ink.source(k) } else { faded(ink.source(k), 0.25) };
        painter.circle_filled(p, r, colour);
        painter.circle_stroke(p, r + 3.0, Stroke::new(1.0_f32, colour));
        let label = format!("{eye} · {count}");
        painter.text(
            pos2(p.x, p.y + r + 5.0),
            egui::Align2::CENTER_TOP,
            &label,
            crate::style::font(crate::style::Size::Text),
            ink.muted,
        );
        if pointer.is_some_and(|m| m.distance(p) < r + 4.0) {
            hover_text = Some(format!("{eye}: {count} {}", t(lang, "glance_inputs")));
        }
    }
    // the waves of what plays (a little of each: what is drawn around the node), a picture every frame
    let scopes: std::collections::HashMap<String, Vec<f32>> = app
        .mixer
        .lock()
        .map(|m| {
            m.playing()
                .into_iter()
                .filter_map(|name| {
                    m.scope(&name).map(|s| {
                        let tail = &s[s.len().saturating_sub(480)..];
                        let w: Vec<f32> = tail.iter().step_by(4).copied().collect();
                        (name, w)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if !scopes.is_empty() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    for (k, n) in pic.nodes.iter().enumerate() {
        if !detail || n.name.contains("_glance_") || (far && prep.packet_of[k].is_some()) {
            continue;
        }
        let Some(p) = at(&prep.node_key[k]) else { continue };
        if !vis.contains(p) {
            continue;
        }
        let k = app.colours.get(&n.source).copied().unwrap_or(0);
        let mut colour = ink.source(k);
        if quarantined(&n.source) {
            colour = faded(colour, 0.35);
        }
        // in a flow, small points: the flow is what is seen
        let r = if app.flow.on { 3.5 * zoom.clamp(0.6, 1.6) } else { 7.0 * zoom.clamp(0.6, 1.6) };
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
        // in a flow, what is heard is an oscilloscope beside its node: its wave as it sounds (flat when silent)
        if app.flow.on && (played.contains(&n.name) || n.output) {
            let bx = Rect::from_min_size(p + vec2(r + 8.0, -26.0), vec2(130.0, 52.0));
            painter.rect_filled(bx, crate::style::corner(), ink.surface);
            painter.rect_stroke(bx, crate::style::corner(), Stroke::new(1.0_f32, colour), egui::StrokeKind::Inside);
            painter.line_segment(
                [pos2(bx.left() + 4.0, bx.center().y), pos2(bx.right() - 4.0, bx.center().y)],
                Stroke::new(0.5_f32, ink.faint),
            );
            if let Some(wave) = scopes.get(&n.name) {
                let m = wave.len().max(2);
                let pts: Vec<Pos2> = wave
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        pos2(
                            bx.left() + 4.0 + (bx.width() - 8.0) * i as f32 / (m - 1) as f32,
                            bx.center().y - v.clamp(-1.0, 1.0) * (bx.height() / 2.0 - 4.0),
                        )
                    })
                    .collect();
                painter.add(Shape::line(pts, Stroke::new(1.3_f32, ink.accent)));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
            }
        } else if let Some(wave) = scopes.get(&n.name) {
            // a node that plays is seen too: its wave around it, as it sounds, and what it touches lit by it
            let m = wave.len().max(2);
            let pts: Vec<Pos2> = wave
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let a = i as f32 / (m - 1) as f32 * std::f32::consts::TAU;
                    let rad = r * 2.2 + v.clamp(-1.0, 1.0) * r * 1.2;
                    p + vec2(a.cos(), a.sin()) * rad
                })
                .collect();
            painter.add(Shape::closed_line(pts, Stroke::new(1.5_f32, colour)));
            let loud = wave.iter().map(|v| v.abs()).fold(0.0, f32::max).min(1.0);
            for (i, _) in pic.relations.iter().enumerate().filter(|(_, rel)| rel.nodes.contains(&n.name)) {
                if let Some(rp) = at(&prep.rel_key[i]) {
                    painter
                        .line_segment([p, rp], Stroke::new(1.0 + 2.5 * loud, colour.gamma_multiply(0.3 + 0.5 * loud)));
                }
            }
        }
        let selected = match &app.write_into {
            Some(ws) => app.ws_selected.as_ref().is_some_and(|(w, x)| w == ws && *x == n.name),
            None => app.selected.contains(&n.name),
        };
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
        if (label_all && in_focus) || selected || over {
            // its name, and under it how it is: its value and how sure (amber when it is unsure)
            let (health, unsure) = health(n);
            let name = painter.layout_no_wrap(n.name.clone(), crate::style::font(crate::style::Size::Text), ink.text);
            let how = painter.layout_no_wrap(
                health,
                crate::style::font(crate::style::Size::Small),
                if unsure { ink.amber } else { ink.muted },
            );
            let w = name.size().x.max(how.size().x);
            let h = name.size().y + how.size().y;
            let below = Rect::from_min_size(pos2(p.x - w / 2.0, p.y + r + 3.0), vec2(w, h));
            let above = Rect::from_min_size(pos2(p.x - w / 2.0, p.y - r - 3.0 - h), vec2(w, h));
            let spot = [below, above].into_iter().find(|b| placed.free(*b));
            let spot = if over || selected { spot.or(Some(below)) } else { spot };
            if let Some(b) = spot {
                placed.put(b);
                let ny = name.size().y;
                painter.galley(pos2(p.x - name.size().x / 2.0, b.top()), name, ink.text);
                painter.galley(pos2(p.x - how.size().x / 2.0, b.top() + ny), how, ink.muted);
            }
        }
    }
    // from afar, the packets: one point each for the nodes a `repeat` wrote, as big as how many, its name beside it
    let mut over_packet: Option<usize> = None;
    if detail && far {
        for (i, pk) in prep.packets.iter().enumerate() {
            let Some(p) = packet_at[i].filter(|p| vis.contains(*p)) else { continue };
            let k = app.colours.get(&pk.source).copied().unwrap_or(0);
            let mut colour = ink.source(k);
            let in_focus = app.focus.as_ref().is_none_or(|f| *f == pk.source);
            if !in_focus {
                colour = faded(colour, 0.25);
            }
            let r = (6.0 + 3.0 * (pk.members.len() as f32).ln()) * zoom.clamp(0.6, 1.6);
            let over = pointer.is_some_and(|m| m.distance(p) < r + 4.0);
            painter.circle_filled(p, r, colour.gamma_multiply(if over { 0.45 } else { 0.28 }));
            painter.circle_stroke(p, r, Stroke::new(if over { 2.0_f32 } else { 1.2_f32 }, colour));
            // the packet's points inside it, quietly: a few dots in a ring
            for j in 0..pk.members.len().min(12) {
                let a = j as f32 / pk.members.len().min(12) as f32 * std::f32::consts::TAU;
                painter.circle_filled(p + vec2(a.cos(), a.sin()) * r * 0.55, 1.4, colour);
            }
            if over {
                over_packet = Some(i);
                hovered = None;
                hover_rel = None;
            }
            let g = painter.layout_no_wrap(
                format!("{} ×{}", pk.name, pk.members.len()),
                crate::style::font(crate::style::Size::Small),
                ink.muted,
            );
            let b = Rect::from_min_size(pos2(p.x - g.size().x / 2.0, p.y + r + 3.0), g.size());
            if over || (in_focus && placed.free(b)) {
                placed.put(b);
                painter.galley(b.min, g, ink.muted);
            }
        }
    }
    // the relations' names too, and how well each holds (where there is room: the nodes' labels come first)
    if label_all && detail && !far {
        for (i, r) in pic.relations.iter().enumerate() {
            let Some(p) = at(&prep.rel_key[i]).filter(|p| vis.contains(*p)) else { continue };
            let held = format!("{:.1} σ", r.tension);
            let g = painter.layout_no_wrap(
                format!("{} · {held}", r.name),
                crate::style::font(crate::style::Size::Small),
                if r.tension > 1.0 { ink.amber } else { ink.faint },
            );
            let b = Rect::from_min_size(pos2(p.x + 8.0, p.y - g.size().y / 2.0), g.size());
            if placed.free(b) {
                placed.put(b);
                painter.galley(b.min, g, ink.faint);
            }
        }
    }
    // far away, each network becomes one node holding its master equation; coming closer, it unfolds again
    // (from the infinitely large to the small: a double click goes into one)
    let mut over_net: Option<String> = None;
    let sources: Vec<String> = prep.counts.iter().map(|c| c.0.clone()).collect();
    if collapse > 0.0 && sources.len() >= 2 {
        painter.rect_filled(rect, 0.0, faded(ink.surface, 0.985 * collapse));
        // the triangle stays, behind the networks gathered
        if let Some(c) = witness_at {
            let tri: Vec<Pos2> = (0..3)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 3.0;
                    c + vec2(a.cos(), a.sin()) * 17.0
                })
                .collect();
            painter.add(Shape::convex_polygon(tri, ink.surface, Stroke::new(1.6_f32, ink.accent.gamma_multiply(0.85))));
        }
        let mut centres: indexmap::IndexMap<String, (Pos2, f32, usize)> = indexmap::IndexMap::new();
        for s in &sources {
            let pts: Vec<Pos2> = pic
                .nodes
                .iter()
                .zip(&prep.node_key)
                .filter(|(n, _)| &n.source == s)
                .filter_map(|(_, k)| at(k))
                .collect();
            if pts.is_empty() {
                continue;
            }
            let c = pts.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / pts.len() as f32;
            let spread = pts.iter().map(|p| p.distance(c.to_pos2())).fold(0.0, f32::max);
            // its size says how many it holds (gently: a few hundred is not a hundred times a few), not how its
            // nodes are spread
            let _ = spread;
            let radius = (16.0_f32 + 9.0 * (pts.len() as f32).ln_1p()).min(84.0);
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
        for (a, b, n) in &prep.joins {
            if let (Some(pa), Some(pb)) = (centres.get(a), centres.get(b)) {
                let w = (1.0 + (*n as f32).sqrt() * 1.5).min(8.0);
                painter.line_segment([pa.0, pb.0], Stroke::new(w, faded(ink.muted, collapse)));
            }
        }
        // laid out among themselves: each keeps the side where its nodes are, none on another
        let keys: Vec<String> = centres.keys().cloned().collect();
        for _ in 0..60 {
            for a in 0..keys.len() {
                for b in (a + 1)..keys.len() {
                    let (ca, ra, _) = centres[&keys[a]];
                    let (cb, rb, _) = centres[&keys[b]];
                    let d = cb - ca;
                    let room = ra + rb + 18.0;
                    let dist = d.length().max(0.01);
                    if dist < room {
                        let dir = if dist < 0.1 { vec2(1.0, 0.3).normalized() } else { d / dist };
                        let push = dir * (room - dist) * 0.5;
                        centres.get_mut(&keys[a]).unwrap().0 -= push;
                        centres.get_mut(&keys[b]).unwrap().0 += push;
                    }
                }
            }
        }
        for (s, (c, radius, count)) in &centres {
            let k = app.colours.get(s).copied().unwrap_or(0);
            let colour = ink.source(k);
            let over = pointer.is_some_and(|m| m.distance(*c) < *radius);
            if over {
                over_net = Some(s.clone());
            }
            // a ground of its own under its colour: a network gathered stands in front of the triangle, never seen
            // through to it
            painter.circle_filled(*c, *radius, faded(ink.surface, collapse));
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
                crate::style::font(crate::style::Size::Text),
                faded(ink.text, collapse),
            );
            painter.text(
                *c,
                egui::Align2::CENTER_CENTER,
                format!("{count} {}", t(lang, "all_nodes")),
                crate::style::font(crate::style::Size::Small),
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
            // a network gathered into one node: the view comes close to it, until it unfolds (its nodes fill it)
            zoom_to_net(app, rect, &full, &net);
        } else if let Some(n) = hovered.as_ref().and_then(|n| full.node(n)).filter(|_| only.is_none()) {
            let s = n.source.clone();
            app.dive(&s);
        }
    } else if resp.clicked() && over_packet.is_some() {
        let pk = &prep.packets[over_packet.unwrap_or(0)];
        let pts: Vec<[f32; 2]> =
            pk.members.iter().filter_map(|&m| app.graph.pos.get(&prep.node_key[m]).copied()).collect();
        if !pts.is_empty() {
            let c = pts.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]);
            let c = [c[0] / pts.len() as f32, c[1] / pts.len() as f32];
            let zoom = (PACKETS_FROM + 0.3).max(app.view.zoom);
            app.view.zoom = zoom;
            app.view.offset = vec2(-c[0] * zoom, -c[1] * zoom);
            app.view.fitted = true;
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
    // a flow or the usual layout: inputs left, what is heard right, what passes lit while it plays
    if !app.view.passive {
        let at = Rect::from_min_size(pos2(rect.right() - 110.0, rect.top() + 8.0), vec2(100.0, 26.0));
        let b = egui::Button::image_and_text(
            crate::icons::image(
                "workflow",
                14.0,
                if app.flow.on { ui.visuals().selection.stroke.color } else { ink.text },
            ),
            RichText::new(t(lang, "flow")).color(if app.flow.on {
                ui.visuals().selection.stroke.color
            } else {
                ink.text
            }),
        )
        .selected(app.flow.on);
        if ui.put(at, b).on_hover_text(t(lang, "flow_d")).clicked() {
            app.flow.on = !app.flow.on;
            app.graph.motion = 0.05;
            // back to the free layout: a few seconds of quicker steps, then calm again
            if !app.flow.on {
                app.graph.hurry = 240;
                app.graph.wake();
            }
            app.view.fitted = false;
        }
    }
    let _ = &hover_text;
    let menu_open = egui::Popup::is_any_open(ui.ctx());
    if app.view.dragging.is_none() && app.view.box_from.is_none() && app.view.relating.is_none() && !menu_open {
        let tip = egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("net-tip"),
            egui::PopupAnchor::Pointer,
        );
        if let Some(i) = over_packet {
            let pk = prep.packets[i].clone();
            tip.show(|ui| {
                ui.label(RichText::new(&pk.name).strong().size(crate::style::size(crate::style::Size::Subtitle)));
                ui.label(
                    RichText::new(format!(
                        "{} · {} {} · {} {}",
                        pk.source,
                        pk.members.len(),
                        t(lang, "all_nodes"),
                        pk.relations,
                        t(lang, "packet_relations")
                    ))
                    .small()
                    .color(ink.muted),
                );
                if !pk.form.is_empty() {
                    ui.label(RichText::new(t(lang, "packet_form")).small().color(ink.muted));
                    ui.label(RichText::new(&pk.form).monospace().size(crate::style::size(crate::style::Size::Small)));
                }
                ui.label(RichText::new(t(lang, "packet_open")).small().color(ink.muted));
            });
        } else if let Some(net) = &over_net {
            let net = net.clone();
            tip.show(|ui| {
                ui.label(RichText::new(&net).strong().size(crate::style::size(crate::style::Size::Subtitle)));
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
    {
        let mut y = rect.top() + 8.0;
        y = y.max(rect.top() + 8.0 + 2.0 * app.view.eq_row);
        app.view.eq_below = y - rect.top();

        if !app.view.passive && app.write_into.is_none() {
            let counts = &prep.counts;
            let height = ui.available_height();
            let list = Rect::from_min_size(
                pos2(rect.left() + 6.0, rect.top() + 8.0), 
                vec2(200.0, height)
            );
            let mut child =
                ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::Frame::new()
                .fill(faded(ink.surface, 0.9))
                .stroke(Stroke::new(1.0_f32, faded(ink.faint, 0.6)))
                .corner_radius(crate::style::corner())
                .inner_margin(egui::Margin::same(6))
                .show(&mut child, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        crate::icons::show(ui, "search", 13.0, ink.muted);
                        ui.add(
                            egui::TextEdit::singleline(&mut app.net_search)
                                .hint_text(crate::style::hint(t(lang, "net_search")))
                                .desired_width(ui.available_width()),
                        );
                    });
                    let q = app.net_search.trim().to_lowercase();
                    let all_on = app.focus.is_none();
                    if crate::icons::toggle(ui, "layers", format!("{} · {}", t(lang, "all"), counts.len()), all_on)
                        .clicked()
                    {
                        app.focus = None;
                    }
                    let scroll_output = egui::ScrollArea::vertical()
                        .id_salt("net-list")
                        .auto_shrink([false, false])
                        .max_height(list.height() - 70.0)
                        .show(ui, |ui| {
                            for (s, k) in counts.iter().filter(|(s, _)| q.is_empty() || s.to_lowercase().contains(&q)) {
                                let colour = ink.source(app.colours.get(s).copied().unwrap_or(0));
                                let on = app.focus.as_deref() == Some(s.as_str());
                                let r = ui
                                    .horizontal(|ui| {
                                        let (dot, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
                                        ui.painter().circle_filled(dot.center(), 4.0, colour);
                                        let text =
                                            egui::RichText::new(s).small().color(if on { ink.on_chosen() } else { colour });
                                        let b = egui::Button::new(text)
                                            .fill(if on {
                                                crate::style::under_words(colour)
                                            } else {
                                                egui::Color32::TRANSPARENT
                                            })
                                            .frame(on);
                                        let r = ui.add(b);
                                        ui.label(egui::RichText::new(format!("{k}")).small().color(ink.muted));
                                        r
                                    })
                                    .inner;
                                if r.double_clicked() {
                                    zoom_to_net(app, rect, &full, s);
                                } else if r.clicked() {
                                    app.focus = if on { None } else { Some(s.clone()) };
                                }
                                let outputs: Vec<String> = pic
                                    .nodes
                                    .iter()
                                    .filter(|n| &n.source == s && n.output)
                                    .map(|n| n.name.clone())
                                    .collect();
                                r.on_hover_text(t(lang, "net_list_hint")).context_menu(|ui| {
                                    ui.set_min_width(280.0);
                                    crate::tree::network_menu(app, ui, s, &outputs)
                                });
                            }
                        });

                    if scroll_output.inner_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                    || ui.rect_contains_pointer(list)
                    {
                        ui.input_mut(|i| {
                            i.smooth_scroll_delta = egui::Vec2::ZERO;
                            i.raw_scroll_delta = egui::Vec2::ZERO;
                        });
                    }
                });
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
                app.graph.wake();
            }
        } else if app.view.box_from.is_none() && resp.dragged_by(egui::PointerButton::Primary) {
            app.view.offset += resp.drag_delta();
            app.view.fitted = true;
        }
    }
    if let (Some(from), Some(to)) = (app.view.box_from, resp.interact_pointer_pos()) {
        let b = Rect::from_two_pos(from, to);
        painter.rect_stroke(b, crate::style::corner(), Stroke::new(1.0_f32, ink.accent), egui::StrokeKind::Inside);
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
            // in a workshop: the node is taken in hand there (the inspector shows the main network)
            Some(n) if app.write_into.is_some() => {
                let ws = app.write_into.clone().unwrap_or_default();
                app.ws_selected = if app.ws_selected.as_ref() == Some(&(ws.clone(), n.clone())) {
                    None
                } else {
                    Some((ws, n.clone()))
                };
            }
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
            crate::style::font(crate::style::Size::Text),
            ink.accent,
        );
    }
    if app.graph.motion > 0.02 {
        ui.ctx().request_repaint();
    }
}

/// The point a node is drawn as: itself, or its eye when it is one of the eye's glance inputs.
/// How a node is, in a few signs: its value and how sure it is ("9.81 ± 2 %"); true when it is unsure.
fn health(n: &crate::model::NodeView) -> (String, bool) {
    let v = grm_engine::lang::g((n.value * 1000.0).round() / 1000.0 + 0.0);
    if n.signed {
        let rel = n.sigma / n.value.abs().max(1e-3);
        (format!("{v} ± {}", grm_engine::lang::g((n.sigma * 1000.0).round() / 1000.0)), rel > 0.25)
    } else {
        let pc = (10f64.powf(n.sigma) - 1.0) * 100.0;
        if !pc.is_finite() || pc > 999.0 {
            // nothing tells it yet: a guess
            return (format!("≈ {v} ?"), true);
        }
        let pc = if pc < 1.0 { format!("{:.1}", pc) } else { format!("{:.0}", pc) };
        let unsure = pc.parse::<f64>().unwrap_or(0.0) > 25.0;
        (format!("{v} ± {pc} %"), unsure)
    }
}

/// The view comes close to a network, until its nodes fill it, and the others are dimmed.
fn zoom_to_net(app: &mut App, rect: Rect, pic: &crate::model::Picture, net: &str) {
    let pts: Vec<[f32; 2]> = pic
        .nodes
        .iter()
        .filter(|n| n.source == net)
        .filter_map(|n| app.graph.pos.get(&item_of(&n.name)).copied())
        .collect();
    if pts.is_empty() {
        return;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let zoom =
        (rect.width() / (x1 - x0 + 120.0)).min(rect.height() / (y1 - y0 + 120.0)).clamp(COLLAPSE_FROM + 0.1, 2.0);
    app.view.zoom = zoom;
    // a little to the right: the list of networks is on the left
    app.view.offset = vec2(-(x0 + x1) / 2.0 * zoom + 100.0, -(y0 + y1) / 2.0 * zoom);
    app.view.fitted = true;
    app.focus = Some(net.to_string());
}

fn pointer_over(resp: &egui::Response, c: Pos2, r: f32) -> bool {
    resp.hover_pos().is_some_and(|m| m.distance(c) < r)
}

fn item_of(node: &str) -> String {
    match node.split_once("_glance_") {
        Some((eye, _)) => format!("g:{eye}"),
        None => format!("n:{node}"),
    }
}

fn legend(painter: &egui::Painter, rect: Rect, ink: &Ink, lang: crate::text::Lang) {
    let font = crate::style::font(crate::style::Size::Small);
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

fn glide_to_fit(app: &mut App, rect: Rect, items: &[String]) {
    let (zoom, offset) = (app.view.zoom, app.view.offset);
    fit(app, rect, items);
    // the view follows slowly (a window resized does not shake the network)
    let k = 0.04;
    app.view.zoom = zoom + (app.view.zoom - zoom) * k;
    app.view.offset = offset + (app.view.offset - offset) * k;
    if (app.view.zoom - zoom).abs() > 1e-3 || (app.view.offset - offset).length() > 0.3 {
        app.graph.motion = app.graph.motion.max(0.03);
    }
}

/// Everything this view shows in view (its points only: the world's others are not its own).
fn fit(app: &mut App, rect: Rect, items: &[String]) {
    let mut it = items.iter().filter_map(|k| app.graph.pos.get(k));
    let b = it.next().map(|f| {
        it.fold([f[0], f[1], f[0], f[1]], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
    });
    if let Some(b) = b {
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
    // what is nothing but rounding is nothing (a signed value; a positive one is never nothing: Planck's constant
    // is 6.6e-34, not 0)
    let value = if signed && value.abs() < 1e-12 { 0.0 } else { value };
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
