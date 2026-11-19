// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The 3D view, a primitive called from GRML (`widget sky: view3d b in bodies: x {b}_x y {b}_y z {b}_z radius
//! {b}_r hue {b}_hue selected chosen`): an object for each item of a list, its place and its look from the item's
//! nodes. Looking: orbit (drag), pan (right or middle drag, or shift), zoom to the pointer (wheel, pinch), a double
//! click centres on what is under it, F fits everything, the axes' views (top, front, side), perspective or flat.
//! Seeing: a grid on the ground, the three axes, a light, names and values on hover. Handling: a click chooses (its
//! place in the list, from 1, written to the `selected` node; nothing: 0), Escape lets go, the arrows move the one
//! chosen on the ground and Page up / down lifts it (its nodes written back). What moves is the network: the view
//! only draws it.

use eframe::egui::{self, vec2, Color32, Pos2, Rect, Sense, Stroke};
use grm_shell::apps::Widget;

use crate::app::{App, Ink};

/// How a view looks at its world (kept while the window runs).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Turned around the vertical axis, and up or down (radians).
    yaw: f32,
    pitch: f32,
    /// How far the eye is from what it looks at, and what it looks at.
    distance: f32,
    target: [f32; 3],
    /// Flat (no perspective).
    flat: bool,
    /// Moved or zoomed by the human: until then (and again after a fit) it keeps everything in view, as the
    /// objects come and move (their nodes may not be computed yet on the first frames).
    held: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { yaw: -0.6, pitch: 0.45, distance: 10.0, target: [0.0; 3], flat: false, held: false }
    }
}

/// One object of the view: its name, its place, its size, its hue (0 .. 1; none: the style's colours in turn).
struct Object {
    name: String,
    at: [f32; 3],
    radius: f32,
    hue: Option<f32>,
}

impl Camera {
    /// Where a point of the world falls on the view, and how far it is from the eye (for what hides what), and
    /// how many points of the screen one unit of the world is there.
    fn project(&self, p: [f32; 3], rect: Rect) -> Option<(Pos2, f32, f32)> {
        let e = self.seen(p);
        if !self.flat && e[2] < self.near() {
            return None;
        }
        Some(self.on_view(e, rect))
    }

    /// A point of the world as the eye sees it: across, up, and how far in front of the eye.
    fn seen(&self, p: [f32; 3]) -> [f32; 3] {
        let d = [p[0] - self.target[0], p[1] - self.target[1], p[2] - self.target[2]];
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // turned around y, then around x
        let x1 = cy * d[0] - sy * d[2];
        let z1 = sy * d[0] + cy * d[2];
        let y2 = cp * d[1] - sp * z1;
        let z2 = sp * d[1] + cp * z1;
        [x1, y2, self.distance - z2]
    }

    /// The nearest the eye sees (what is closer is not drawn).
    fn near(&self) -> f32 {
        0.05 * self.distance
    }

    /// Where a point seen by the eye falls on the view, its depth, and how many points one unit is there.
    fn on_view(&self, e: [f32; 3], rect: Rect) -> (Pos2, f32, f32) {
        let half = rect.width().min(rect.height()) / 2.0;
        let unit = if self.flat { half / self.distance } else { half * 1.6 / e[2] };
        (rect.center() + vec2(e[0] * unit, -e[1] * unit), e[2], unit)
    }

    /// A line of the world on the view: the part of it behind the eye cut off (a ground that passes under the eye
    /// is still drawn up to it).
    fn segment(&self, a: [f32; 3], b: [f32; 3], rect: Rect) -> Option<[Pos2; 2]> {
        let (mut ea, mut eb) = (self.seen(a), self.seen(b));
        if !self.flat {
            let near = self.near();
            if ea[2] < near && eb[2] < near {
                return None;
            }
            let cut = |from: [f32; 3], to: [f32; 3]| {
                let t = (near - from[2]) / (to[2] - from[2]);
                [from[0] + t * (to[0] - from[0]), from[1] + t * (to[1] - from[1]), near]
            };
            if ea[2] < near {
                ea = cut(ea, eb);
            } else if eb[2] < near {
                eb = cut(eb, ea);
            }
        }
        Some([self.on_view(ea, rect).0, self.on_view(eb, rect).0])
    }

    /// The point of the world under a point of the view, at the depth of what is looked at (to zoom toward it).
    fn unproject(&self, s: Pos2, rect: Rect) -> [f32; 3] {
        let half = rect.width().min(rect.height()) / 2.0;
        let unit = if self.flat { half / self.distance } else { half * 1.6 / self.distance };
        let v = (s - rect.center()) / unit;
        let (x1, y2) = (v.x, -v.y);
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // back from around x (z2 = 0), then around y
        let (d1, z1) = (cp * y2, -sp * y2);
        let dx = cy * x1 + sy * z1;
        let dz = -sy * x1 + cy * z1;
        [self.target[0] + dx, self.target[1] + d1, self.target[2] + dz]
    }

    /// Until the human moves it, everything stays in view.
    fn follow(&mut self, objects: &[Object]) {
        if !self.held {
            self.fit(objects);
        }
    }

    /// Everything in view, as close as it fits.
    fn fit(&mut self, objects: &[Object]) {
        if objects.is_empty() {
            return;
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for o in objects {
            for k in 0..3 {
                lo[k] = lo[k].min(o.at[k] - o.radius);
                hi[k] = hi[k].max(o.at[k] + o.radius);
            }
        }
        self.target = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f32, f32::max).max(1e-3);
        self.distance = span * 1.4;
        self.held = false;
    }
}

/// A colour from a hue (0 .. 1), at the lightness and the strength of the style's colours.
fn hue_colour(h: f32, dark: bool) -> Color32 {
    let a = std::f64::consts::TAU * h as f64;
    let l = if dark { 0.74 } else { 0.58 };
    crate::elle::oklab(l, 0.12 * a.cos(), 0.12 * a.sin(), 1.0)
}

/// A round step for the grid: 1, 2 or 5 times a power of ten, about a tenth of what is seen.
fn grid_step(distance: f32) -> f32 {
    let raw = (distance / 6.0).max(1e-6);
    let p = 10f32.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * p).find(|s| *s >= raw).unwrap_or(10.0 * p)
}

/// The view drawn in its rect, handled by the pointer and the keys.
pub fn show(app: &mut App, ui: &mut egui::Ui, rect: Rect, w: &Widget, app_name: &str, lists: &grm_shell::apps::Lists) {
    let ink = Ink::of(ui.ctx());
    let key = format!("{app_name}/{}", w.name);
    let id = ui.id().with(("view3d", &key));
    let Some((var, list)) = w.over.clone() else { return };
    let field = |f: &str| w.fields.iter().find(|(k, _)| k == f).map(|(_, p)| p.clone());
    let items: Vec<String> = lists.get(&list).map(|l| l.0.clone()).unwrap_or_default();
    let read = |pattern: &Option<String>, item: &str| -> Option<f64> {
        let name = pattern.as_ref()?.replace(&format!("{{{var}}}"), item);
        app.picture.node(&name).map(|n| n.value)
    };
    let (fx, fy, fz, fr, fh) = (field("x"), field("y"), field("z"), field("radius"), field("hue"));
    let objects: Vec<Object> = items
        .iter()
        .map(|it| Object {
            name: it.clone(),
            at: [
                read(&fx, it).unwrap_or(0.0) as f32,
                read(&fy, it).unwrap_or(0.0) as f32,
                read(&fz, it).unwrap_or(0.0) as f32,
            ],
            radius: read(&fr, it).map(|r| r.abs() as f32).unwrap_or(0.3).max(1e-4),
            hue: read(&fh, it).map(|h| h.rem_euclid(1.0) as f32),
        })
        .collect();
    let selected_node = field("selected");
    let chosen = selected_node.as_ref().and_then(|n| app.picture.node(n)).map(|n| n.value.round() as i64).unwrap_or(0);
    let mut cam = app.surface.cameras.get(&key).copied().unwrap_or_default();
    cam.follow(&objects);

    // the background: what the pointer does where there is no object
    let bg = ui.interact(rect, id, Sense::click_and_drag());
    bg.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "3D view"));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, crate::style::corner(), ink.panel);

    // the grid on the ground and the three axes
    let step = grid_step(cam.distance);
    let n = ((cam.distance * 1.5 / step).ceil() as i32).clamp(4, 40);
    let (gx, gz) = ((cam.target[0] / step).round() * step, (cam.target[2] / step).round() * step);
    for k in -n..=n {
        let o = k as f32 * step;
        for (a, b) in [
            ([gx + o, 0.0, gz - n as f32 * step], [gx + o, 0.0, gz + n as f32 * step]),
            ([gx - n as f32 * step, 0.0, gz + o], [gx + n as f32 * step, 0.0, gz + o]),
        ] {
            if let Some(line) = cam.segment(a, b, rect) {
                painter.line_segment(line, Stroke::new(1.0_f32, ink.faint.gamma_multiply(0.5)));
            }
        }
    }
    let reach = step * n as f32;
    for (axis, colour, word) in
        [([reach, 0.0, 0.0], ink.amber, "x"), ([0.0, reach, 0.0], ink.accent, "y"), ([0.0, 0.0, reach], ink.muted, "z")]
    {
        if let Some(line) = cam.segment([0.0; 3], axis, rect) {
            painter.line_segment(line, Stroke::new(1.5_f32, colour));
            if let Some((end, _, _)) = cam.project(axis, rect).filter(|e| rect.contains(e.0)) {
                painter.text(
                    end,
                    egui::Align2::CENTER_CENTER,
                    word,
                    crate::style::font(crate::style::Size::Small),
                    colour,
                );
            }
        }
    }

    // the objects, the farthest first; each a sphere lit from above, at the left (a light that follows the eye)
    let mut drawn: Vec<(usize, Pos2, f32, f32)> = objects
        .iter()
        .enumerate()
        .filter_map(|(k, o)| cam.project(o.at, rect).map(|(p, depth, unit)| (k, p, depth, o.radius * unit)))
        .collect();
    drawn.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut hovered: Option<usize> = None;
    let mut clicked: Option<usize> = None;
    for &(k, p, _, r) in &drawn {
        let r = r.clamp(2.0, rect.width());
        let o = &objects[k];
        let base = match o.hue {
            Some(h) => hue_colour(h, ui.visuals().dark_mode),
            None => ink.source(k),
        };
        painter.circle_filled(p, r, base.gamma_multiply(0.75));
        painter.circle_filled(p - vec2(r * 0.25, r * 0.25), r * 0.7, base);
        painter.circle_filled(p - vec2(r * 0.4, r * 0.4), r * 0.25, base.lerp_to_gamma(Color32::WHITE, 0.45));
        if chosen == k as i64 + 1 {
            painter.circle_stroke(p, r + 3.0, Stroke::new(2.0_f32, ink.accent));
        }
        // each object answers the pointer itself (and says its name to whoever cannot see it)
        let hit = Rect::from_center_size(p, vec2(2.0 * r.max(6.0), 2.0 * r.max(6.0))).intersect(rect);
        let resp = ui.interact(hit, id.with(("object", k)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &o.name));
        if resp.hovered() {
            hovered = Some(k);
        }
        if resp.clicked() {
            clicked = Some(k);
        }
        if resp.double_clicked() {
            cam.target = o.at;
            cam.held = true;
        }
    }
    if let Some(k) = hovered {
        let o = &objects[k];
        let words = format!("{}\nx {:.3}  y {:.3}  z {:.3}", o.name, o.at[0], o.at[1], o.at[2]);
        if let Some(&(_, p, _, r)) = drawn.iter().find(|d| d.0 == k) {
            painter.text(
                p + vec2(r + 6.0, -r),
                egui::Align2::LEFT_BOTTOM,
                &o.name,
                crate::style::font(crate::style::Size::Small),
                ink.text,
            );
        }
        let r = ui.interact(rect, id.with("hover"), Sense::hover());
        r.show_tooltip_text(words);
    }

    // choosing, letting go
    let write_chosen = |app: &App, v: f64| {
        if let Some(n) = &selected_node {
            crate::surface::observe(app, n, v);
        }
    };
    if let Some(k) = clicked {
        write_chosen(app, k as f64 + 1.0);
    } else if bg.clicked() {
        write_chosen(app, 0.0);
    }

    // looking: orbit, pan, zoom to the pointer, pinch, double click on nothing: fit
    let shift = ui.input(|i| i.modifiers.shift);
    if bg.dragged_by(egui::PointerButton::Primary) && !shift {
        let d = bg.drag_delta();
        cam.yaw += d.x * 0.008;
        cam.pitch = (cam.pitch + d.y * 0.008).clamp(-1.55, 1.55);
    }
    if bg.dragged_by(egui::PointerButton::Secondary)
        || bg.dragged_by(egui::PointerButton::Middle)
        || (bg.dragged_by(egui::PointerButton::Primary) && shift)
    {
        let d = bg.drag_delta();
        let a = cam.unproject(rect.center(), rect);
        let b = cam.unproject(rect.center() - d, rect);
        for k in 0..3 {
            cam.target[k] += b[k] - a[k];
        }
        cam.held = true;
    }
    if bg.double_clicked() && hovered.is_none() {
        cam.fit(&objects);
    }
    if ui.rect_contains_pointer(rect) {
        let (wheel, pinch, at) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
        let factor = (-wheel * 0.002).exp() / pinch;
        if (factor - 1.0).abs() > 1e-4 {
            cam.held = true;
            // toward what is under the pointer: it stays under it
            if let Some(at) = at {
                let before = cam.unproject(at, rect);
                cam.distance = (cam.distance * factor).clamp(1e-6, 1e9);
                let after = cam.unproject(at, rect);
                for k in 0..3 {
                    cam.target[k] += before[k] - after[k];
                }
            } else {
                cam.distance = (cam.distance * factor).clamp(1e-6, 1e9);
            }
        }
    }

    // Escape lets go of the one chosen: the window takes the key for everything that is chosen, and says so
    if app.escaped && chosen != 0 {
        write_chosen(app, 0.0);
    }
    // the keys, while the pointer is over it and nothing is typed: F fits, the arrows move the one chosen (on the
    // ground: x and z as seen; Page up / down: y), a step of the grid at a time
    if ui.rect_contains_pointer(rect) && !ui.ctx().wants_keyboard_input() {
        let pressed = |k: egui::Key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
        if pressed(egui::Key::F) {
            cam.fit(&objects);
        }
        if chosen >= 1 && (chosen as usize) <= objects.len() {
            let it = &objects[chosen as usize - 1].name;
            let (sy, cy) = cam.yaw.sin_cos();
            let right = [cy * step, 0.0, -sy * step];
            let ahead = [-sy * step, 0.0, -cy * step];
            let mut moved = [0.0f32; 3];
            for (k, dir, sign) in [
                (egui::Key::ArrowRight, right, 1.0),
                (egui::Key::ArrowLeft, right, -1.0),
                (egui::Key::ArrowUp, ahead, 1.0),
                (egui::Key::ArrowDown, ahead, -1.0),
                (egui::Key::PageUp, [0.0, step, 0.0], 1.0),
                (egui::Key::PageDown, [0.0, step, 0.0], -1.0),
            ] {
                if pressed(k) {
                    for a in 0..3 {
                        moved[a] += sign * dir[a];
                    }
                }
            }
            if moved != [0.0; 3] {
                for (a, pattern) in [(0, &fx), (1, &fy), (2, &fz)] {
                    if moved[a] != 0.0 {
                        if let Some(p) = pattern {
                            let name = p.replace(&format!("{{{var}}}"), it);
                            let now = app.picture.node(&name).map(|n| n.value).unwrap_or(0.0);
                            crate::surface::observe(app, &name, now + moved[a] as f64);
                        }
                    }
                }
            }
        }
    }

    // the views along the axes, flat or not, fit: signs in a corner (each says its words)
    let lang = app.settings.lang;
    let corner = Rect::from_min_size(rect.right_top() + vec2(-190.0, 6.0), vec2(184.0, 22.0));
    let mut bar =
        ui.new_child(egui::UiBuilder::new().max_rect(corner).layout(egui::Layout::right_to_left(egui::Align::Center)));
    if crate::icons::sign(&mut bar, "scan", crate::text::t(lang, "v3_fit")).clicked() {
        cam.fit(&objects);
    }
    let (sign, words) = if cam.flat { ("box", "v3_perspective") } else { ("square-dashed", "v3_flat") };
    if crate::icons::sign(&mut bar, sign, crate::text::t(lang, words)).clicked() {
        cam.flat = !cam.flat;
    }
    for (word, words, yaw, pitch) in [
        ("v3_side_word", "v3_side", -std::f32::consts::FRAC_PI_2, 0.0),
        ("v3_front_word", "v3_front", 0.0, 0.0),
        ("v3_top_word", "v3_top", 0.0, 1.55),
    ] {
        let b = egui::Button::new(egui::RichText::new(crate::text::t(lang, word)).small()).frame(false);
        if bar.add(b).on_hover_text(crate::text::t(lang, words)).clicked() {
            cam.yaw = yaw;
            cam.pitch = pitch;
        }
    }
    app.surface.cameras.insert(key, cam);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn what_is_looked_at_stands_in_the_middle_and_the_pointer_finds_it_again() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        for flat in [false, true] {
            let cam = Camera { yaw: 0.7, pitch: 0.3, distance: 12.0, target: [1.0, 2.0, 3.0], flat, held: false };
            let (p, _, _) = cam.project([1.0, 2.0, 3.0], rect).unwrap();
            assert!((p - rect.center()).length() < 1e-3);
            // a point of the plane facing the eye through the target: back where it was
            let s = pos2(260.0, 90.0);
            let w = cam.unproject(s, rect);
            let (q, _, _) = cam.project(w, rect).unwrap();
            assert!((q - s).length() < 0.05, "{q:?} {s:?}");
        }
        let mut cam = Camera::default();
        let objects = [
            Object { name: "a".into(), at: [-5.0, 0.0, 0.0], radius: 1.0, hue: None },
            Object { name: "b".into(), at: [5.0, 2.0, 0.0], radius: 1.0, hue: None },
        ];
        cam.fit(&objects);
        assert_eq!(cam.target, [0.0, 1.0, 0.0]);
        for o in &objects {
            let (p, _, _) = cam.project(o.at, rect).unwrap();
            assert!(rect.contains(p), "{} out of view", o.name);
        }
        assert_eq!(grid_step(10.0), 2.0);
    }

    /// Seen first: the view opened before its nodes were computed (all at 0) and stayed fitted to nothing, the sun
    /// filling it. Until the human moves it, it follows its objects; once moved, it stays where it was put.
    #[test]
    fn a_view_follows_its_objects_until_the_human_moves_it() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let at = |x: f32| Object { name: "o".into(), at: [x, 0.0, 0.0], radius: 0.3, hue: None };
        let mut cam = Camera::default();
        cam.follow(&[at(0.0), at(0.0)]);
        cam.follow(&[at(-20.0), at(20.0)]);
        for x in [-20.0, 20.0] {
            assert!(rect.contains(cam.project([x, 0.0, 0.0], rect).unwrap().0), "{x} out of view");
        }
        cam.held = true;
        let before = cam.distance;
        cam.follow(&[at(-90.0), at(90.0)]);
        assert_eq!(cam.distance, before);
    }

    /// Seen first: the near half of the ground was missing (a line with an end behind the eye was not drawn).
    #[test]
    fn a_line_that_passes_behind_the_eye_is_drawn_up_to_it() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let cam = Camera { yaw: 0.0, pitch: 0.3, distance: 10.0, target: [0.0; 3], flat: false, held: false };
        // from far in front of the eye to far behind it, on the ground
        let (a, b) = ([0.0, 0.0, -40.0], [0.0, 0.0, 40.0]);
        assert!(cam.project(a, rect).is_none() || cam.project(b, rect).is_none());
        let [p, q] = cam.segment(a, b, rect).expect("drawn");
        assert!(p.y.is_finite() && q.y.is_finite() && (p - q).length() > 100.0);
        // wholly behind: nothing
        assert!(cam.segment([0.0, 0.0, 30.0], [1.0, 0.0, 40.0], rect).is_none());
    }
}
