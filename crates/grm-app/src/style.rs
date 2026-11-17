// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine's look: a palette (chosen among a few, or each colour by hand in the settings), its widgets drawn in
//! it (not egui's own grey), a typewriter font everywhere if wished. Apps inherit it, and can change it (see
//! `looks.rs`: a look is a small network of its own).

use eframe::egui::{self, Color32, CornerRadius, FontFamily, Stroke};
use serde::{Deserialize, Serialize};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub dark: bool,
    /// The deepest: where the network and the code are drawn.
    pub background: Rgb,
    /// The panels.
    pub panel: Rgb,
    /// What stands on a panel: buttons, fields, windows.
    pub raised: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub faint: Rgb,
    /// What is chosen, what acts.
    pub accent: Rgb,
    /// What strains, what warns (never red).
    pub warn: Rgb,
    /// The colours of the networks in the network (by the order they first came).
    pub sources: Vec<Rgb>,
}

impl Default for Palette {
    fn default() -> Self {
        presets()[0].clone()
    }
}

pub fn presets() -> Vec<Palette> {
    vec![
        Palette {
            name: "night".into(),
            dark: true,
            background: [0x17, 0x18, 0x1B],
            panel: [0x1F, 0x20, 0x24],
            raised: [0x2B, 0x2D, 0x32],
            text: [0xE4, 0xE2, 0xDC],
            muted: [0x9C, 0x9F, 0xA6],
            faint: [0x45, 0x48, 0x50],
            accent: [0xE8, 0xA8, 0x3D],
            warn: [0xF0, 0x8A, 0x4B],
            sources: vec![
                [0xB9, 0xA6, 0xF0],
                [0x7F, 0xB8, 0xF0],
                [0xE8, 0xC4, 0x6A],
                [0x6F, 0xC8, 0xB0],
                [0xE6, 0x9A, 0xC0],
                [0x9F, 0xB0, 0xC6],
                [0xA8, 0xD0, 0x7A],
                [0x6C, 0xC4, 0xE6],
            ],
        },
        Palette {
            name: "day".into(),
            dark: false,
            background: [0xFB, 0xFA, 0xF6],
            panel: [0xF1, 0xEF, 0xEA],
            raised: [0xE4, 0xE1, 0xDA],
            text: [0x22, 0x23, 0x27],
            muted: [0x5E, 0x61, 0x69],
            faint: [0xC5, 0xC2, 0xBA],
            accent: [0xB8, 0x6E, 0x00],
            warn: [0xB3, 0x4E, 0x12],
            sources: vec![
                [0x6A, 0x4C, 0xC4],
                [0x2E, 0x6C, 0xC4],
                [0x9A, 0x74, 0x00],
                [0x12, 0x80, 0x6C],
                [0xB0, 0x44, 0x7C],
                [0x55, 0x66, 0x80],
                [0x45, 0x86, 0x32],
                [0x1E, 0x84, 0xA8],
            ],
        },
        Palette {
            name: "ocean".into(),
            dark: true,
            background: [0x0F, 0x1A, 0x22],
            panel: [0x15, 0x22, 0x2C],
            raised: [0x1F, 0x30, 0x3C],
            text: [0xDD, 0xEA, 0xF0],
            muted: [0x8F, 0xA8, 0xB6],
            faint: [0x34, 0x4A, 0x58],
            accent: [0x5F, 0xD0, 0xC8],
            warn: [0xF0, 0xB0, 0x5A],
            sources: vec![
                [0x8F, 0xC8, 0xFF],
                [0xC8, 0xA8, 0xFF],
                [0x7F, 0xE0, 0xB8],
                [0xFF, 0xC8, 0x8A],
                [0xF0, 0x9A, 0xC8],
                [0xA8, 0xC0, 0xD0],
                [0xB8, 0xE0, 0x7A],
                [0x6C, 0xD8, 0xF0],
            ],
        },
        Palette {
            name: "contrast".into(),
            dark: true,
            background: [0x00, 0x00, 0x00],
            panel: [0x0C, 0x0C, 0x0C],
            raised: [0x22, 0x22, 0x22],
            text: [0xFF, 0xFF, 0xFF],
            muted: [0xD0, 0xD0, 0xD0],
            faint: [0x70, 0x70, 0x70],
            accent: [0xFF, 0xD0, 0x40],
            warn: [0xFF, 0x9A, 0x40],
            sources: vec![
                [0xC8, 0xB4, 0xFF],
                [0x8C, 0xC8, 0xFF],
                [0xFF, 0xE0, 0x8C],
                [0x7C, 0xF0, 0xC8],
                [0xFF, 0xA8, 0xD8],
                [0xD0, 0xD8, 0xE8],
                [0xC0, 0xF0, 0x80],
                [0x80, 0xE8, 0xFF],
            ],
        },
    ]
}

pub fn c(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// The colours drawing needs, from the palette in use (kept in the context, so anything drawing can ask).
#[derive(Clone, Copy, Debug)]
pub struct Ink {
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub surface: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub accent: Color32,
    pub amber: Color32,
    sources: [Color32; 8],
}

impl Ink {
    pub fn from(p: &Palette) -> Ink {
        let mut sources = [Color32::GRAY; 8];
        for (k, s) in p.sources.iter().take(8).enumerate() {
            sources[k] = c(*s);
        }
        Ink {
            text: c(p.text),
            muted: c(p.muted),
            faint: c(p.faint),
            surface: c(p.background),
            panel: c(p.panel),
            raised: c(p.raised),
            accent: c(p.accent),
            amber: c(p.warn),
            sources,
        }
    }

    pub fn of(ctx: &egui::Context) -> Ink {
        ctx.data(|d| d.get_temp::<Ink>(egui::Id::NULL)).unwrap_or_else(|| Ink::from(&Palette::default()))
    }

    /// The colour of a source: fixed by the order sources first appeared (it never changes when others come or
    /// go). The warning colour is kept for strain, so it is not among them.
    pub fn source(&self, k: usize) -> Color32 {
        self.sources[k % 8]
    }

    /// The text that stands on the accent.
    pub fn on_accent(&self) -> Color32 {
        let a = self.accent;
        let l = 0.299 * a.r() as f32 + 0.587 * a.g() as f32 + 0.114 * a.b() as f32;
        if l > 140.0 {
            Color32::from_rgb(0x14, 0x14, 0x16)
        } else {
            Color32::WHITE
        }
    }
}

fn mixed(a: Color32, b: Color32, k: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - k) + y as f32 * k) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn lighter(c: Color32, k: f32) -> Color32 {
    let m = |x: u8| (x as f32 + (255.0 - x as f32) * k) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn darker(c: Color32, k: f32) -> Color32 {
    let m = |x: u8| (x as f32 * (1.0 - k)) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// A colour turned on the wheel (degrees) and made lighter or darker (a share of white or black).
fn shifted(c: Rgb, turn: f32, light: f32) -> Rgb {
    let mut h = egui::ecolor::Hsva::from(Color32::from_rgb(c[0], c[1], c[2]));
    h.h = (h.h + turn / 360.0).rem_euclid(1.0);
    let c = Color32::from(h);
    let c = if light >= 0.0 { lighter(c, light) } else { darker(c, -light) };
    [c.r(), c.g(), c.b()]
}

/// A breath through the colours: slow (periods of half a minute and more), small (a few degrees of hue, one or two
/// hundredths of light), never a flash; the sound lifts the accent a little.
pub fn breathed(p: &Palette, secs: f64, sound: f32) -> Palette {
    use std::f64::consts::TAU;
    let a = (secs * TAU / 47.0).sin() as f32;
    let b = (secs * TAU / 31.0).sin() as f32;
    let c = (secs * TAU / 73.0).sin() as f32;
    let sound = sound.clamp(0.0, 1.0);
    let mut q = p.clone();
    let sign = if p.dark { 1.0 } else { -1.0 };
    q.accent = shifted(p.accent, 4.0 * a, sign * 0.04 * sound);
    q.background = shifted(p.background, 6.0 * c, sign * 0.010 * b);
    q.panel = shifted(p.panel, 6.0 * c, sign * 0.007 * b);
    for (k, s) in q.sources.iter_mut().enumerate() {
        *s = shifted(*s, 5.0 * (a + 0.3 * k as f32).sin(), sign * 0.02 * sound);
    }
    q
}

/// The colours only (not the fonts, not the sizes): what a breath changes.
pub fn colours(ctx: &egui::Context, p: &Palette) {
    let ink = Ink::from(p);
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, ink));
    let v = visuals(p, &ink);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
}

/// The machine's look on the whole window.
pub fn apply(ctx: &egui::Context, p: &Palette, typewriter: bool, zoom: f32) {
    let ink = Ink::from(p);
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, ink));
    let v = visuals(p, &ink);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if p.dark { egui::Theme::Dark } else { egui::Theme::Light });
    rest(ctx, typewriter, zoom);
}

fn visuals(p: &Palette, ink: &Ink) -> egui::Visuals {
    let ink = *ink;
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let lift = |c: Color32, k: f32| if p.dark { lighter(c, k) } else { darker(c, k) };
    // no override: the text on the accent (a chosen tab, a selected button) takes the colour that stands on it
    v.override_text_color = None;
    v.weak_text_color = Some(mixed(ink.panel, ink.muted, 0.62));
    v.panel_fill = ink.panel;
    v.window_fill = ink.panel;
    v.extreme_bg_color = ink.surface;
    v.faint_bg_color = lift(ink.panel, 0.03);
    v.code_bg_color = ink.surface;
    v.hyperlink_color = ink.accent;
    v.warn_fg_color = ink.amber;
    v.error_fg_color = ink.amber;
    v.selection.bg_fill = ink.accent;
    v.selection.stroke = Stroke::new(1.0_f32, ink.on_accent());
    // what can be clicked shows a hand (a text cursor only where text is written)
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, ink.faint);
    v.window_shadow =
        egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) };
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(80) };
    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = ink.panel;
    w.noninteractive.weak_bg_fill = ink.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, ink.faint);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, ink.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = ink.raised;
    w.inactive.weak_bg_fill = ink.raised;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0_f32, ink.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = lift(ink.raised, 0.08);
    w.hovered.weak_bg_fill = lift(ink.raised, 0.08);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, ink.accent.gamma_multiply(0.7));
    w.hovered.fg_stroke = Stroke::new(1.5_f32, ink.text);
    w.hovered.corner_radius = r;
    // pressed: the accent, softened (strong text keeps the text's own colour)
    w.active.bg_fill = mixed(ink.raised, ink.accent, 0.55);
    w.active.weak_bg_fill = mixed(ink.raised, ink.accent, 0.55);
    w.active.bg_stroke = Stroke::new(1.0_f32, ink.accent);
    w.active.fg_stroke = Stroke::new(1.5_f32, if p.dark { lighter(ink.text, 0.3) } else { darker(ink.text, 0.3) });
    w.active.corner_radius = r;
    w.open.bg_fill = lift(ink.raised, 0.05);
    w.open.weak_bg_fill = lift(ink.raised, 0.05);
    w.open.corner_radius = r;
    v
}

fn rest(ctx: &egui::Context, typewriter: bool, zoom: f32) {
    ctx.all_styles_mut(|st| {
        st.spacing.item_spacing = egui::vec2(8.0, 6.0);
        st.spacing.button_padding = egui::vec2(10.0, 4.0);
        st.spacing.interact_size.y = 24.0;
        // words shown are not taken for words to select (no text cursor over a label)
        st.interaction.selectable_labels = false;
        // check boxes and radio buttons with a little room
        st.spacing.icon_width = 17.0;
        st.spacing.icon_width_inner = 9.0;
        st.spacing.icon_spacing = 8.0;
        st.spacing.window_margin = egui::Margin::same(12);
        st.animation_time = 0.12;
    });
    // a typewriter everywhere, as the machine first spoke
    let mut fonts = egui::FontDefinitions::default();
    if typewriter {
        let mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
        fonts.families.insert(FontFamily::Proportional, mono);
    }
    ctx.set_fonts(fonts);
    ctx.set_zoom_factor(zoom.clamp(0.8, 2.0));
}

/// The room inside a field where words are written.
pub const FIELD: egui::Margin = egui::Margin { left: 8, right: 8, top: 5, bottom: 5 };

/// What a field waits for: in italics and quiet, so that it is not taken for something written.
pub fn hint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).italics()
}

/// Lines shown as a terminal shows them: a dark pane, a fixed font, coloured as code is, room around.
pub fn terminal(ui: &mut egui::Ui, lines: &[String]) {
    let ink = Ink::of(ui.ctx());
    let dark = Color32::from_rgb(0x16, 0x17, 0x1b);
    egui::Frame::new().fill(dark).corner_radius(6.0).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        // the code's colours, on a dark ground whatever the palette
        let term = Ink { text: Color32::from_rgb(0xd8, 0xd8, 0xd0), muted: Color32::from_rgb(0x8a, 0x8c, 0x92), ..ink };
        for l in lines {
            let job = crate::code::highlight_line(&term, l);
            ui.label(job);
        }
    });
}

/// A selected tab or button: the accent behind, the text that stands on it.
pub fn chosen(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let ink = Ink::of(ui.ctx());
    let rich = egui::RichText::new(text).color(if on { ink.on_accent() } else { ink.text });
    let b = egui::Button::new(rich).fill(if on { ink.accent } else { ink.raised });
    ui.add(b)
}

/// The machine's name, in block letters, with a small network beside it.
pub const BANNER: &str = concat!(
    " ██████  ██████  ███    ███     o───o\n",
    "██       ██   ██ ████  ████    / \\ / \\\n",
    "██   ███ ██████  ██ ████ ██   o───o───o\n",
    "██    ██ ██   ██ ██  ██  ██    \\ / \\ /\n",
    " ██████  ██   ██ ██      ██     o───o\n",
    "    the Global Relational Model machine",
);
