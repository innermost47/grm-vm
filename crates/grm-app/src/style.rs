// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine's look, read from its style file (`style/style.grm`, GRML): the fonts (Courier for the code only),
//! a few sizes of text used everywhere, the corners, the room inside things, the colours; its variants. The
//! widgets are drawn in it (not egui's own grey). Apps inherit it, and can change it (see `looks.rs`: a look is a
//! small network of its own).

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

/// The style as it comes (the factory's): `style/style.grm`, inside the machine.
pub const FACTORY: &str = include_str!("../../../style/style.grm");

/// What a style says besides its colours: its fonts, its few sizes, its corners, the room inside things.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// The fonts of the titles, of the text, of the code.
    pub title_font: String,
    pub text_font: String,
    pub code_font: String,
    /// title, subtitle, text, small, tiny (see `Size`), and the code's.
    pub sizes: [f32; 5],
    pub code_size: f32,
    pub corner: f32,
    pub button: [f32; 2],
    pub field: [f32; 2],
    pub spacing: [f32; 2],
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            title_font: "IBM Plex Sans SemiBold".into(),
            text_font: "IBM Plex Sans".into(),
            code_font: "Courier Prime".into(),
            sizes: [18.0, 15.0, 13.0, 11.0, 9.0],
            code_size: 13.0,
            corner: 2.0,
            button: [10.0, 4.0],
            field: [8.0, 5.0],
            spacing: [8.0, 6.0],
        }
    }
}

/// The few sizes of text, used everywhere (never a size of its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Title,
    Subtitle,
    Text,
    Small,
    /// Inside a small widget (a knob's name, a step's number).
    Tiny,
}

/// The fonts the machine carries (free: SIL Open Font License, in `assets/fonts`).
pub const FONTS: [(&str, &[u8]); 3] = [
    ("IBM Plex Sans", include_bytes!("../../../assets/fonts/IBMPlexSans-Regular.ttf")),
    ("IBM Plex Sans SemiBold", include_bytes!("../../../assets/fonts/IBMPlexSans-SemiBold.ttf")),
    ("Courier Prime", include_bytes!("../../../assets/fonts/CourierPrime-Regular.ttf")),
];

/// One style read: its name, its palette, its shape.
#[derive(Debug, Clone)]
pub struct Style {
    pub palette: Palette,
    pub shape: Shape,
}

fn hex(w: &str) -> Option<Rgb> {
    let h = w.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let b = |k: usize| u8::from_str_radix(&h[k..k + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

/// The styles of a file (each variant made from the one it comes from, then what it says), and what was not
/// understood (line, why).
pub fn read(text: &str) -> (Vec<Style>, Vec<(usize, String)>) {
    let mut out: Vec<Style> = vec![];
    let mut wrong = vec![];
    for (k, raw) in text.lines().enumerate() {
        // a comment: a line that begins with `#`, or what follows `# ` (a colour is `#RRGGBB`, without a space)
        let t = raw.trim();
        let line = if t.starts_with('#') { "" } else { t.find("# ").map(|k| &t[..k]).unwrap_or(t).trim() };
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let no = k + 1;
        match words.as_slice() {
            ["style", name] => out.push(Style {
                palette: Palette { name: name.to_string(), ..Palette::bare() },
                shape: Shape::default(),
            }),
            ["variant", name, "from", base] => match out.iter().find(|s| s.palette.name == *base).cloned() {
                Some(mut s) => {
                    s.palette.name = name.to_string();
                    out.push(s);
                }
                None => wrong.push((no, format!("no style {base} before it"))),
            },
            _ => {
                let Some(st) = out.last_mut() else {
                    wrong.push((no, "a style begins with `style NAME`".into()));
                    continue;
                };
                if let Err(e) = said(st, &words, line) {
                    wrong.push((no, e));
                }
            }
        }
    }
    (out, wrong)
}

fn said(st: &mut Style, words: &[&str], line: &str) -> Result<(), String> {
    let num =
        |w: &str| w.parse::<f32>().ok().filter(|x| x.is_finite() && *x >= 0.0).ok_or(format!("not a number: {w}"));
    let quoted = || line.split('"').nth(1).map(str::to_string).ok_or("a font is named between quotes".to_string());
    let sh = &mut st.shape;
    match words {
        ["dark"] => st.palette.dark = true,
        ["light"] => st.palette.dark = false,
        ["font", which, "=", ..] => {
            let f = quoted()?;
            if !FONTS.iter().any(|(n, _)| *n == f) {
                return Err(format!("no font {f} (known: {})", FONTS.map(|x| x.0).join(", ")));
            }
            match *which {
                "title" => sh.title_font = f,
                "text" => sh.text_font = f,
                "code" => sh.code_font = f,
                _ => return Err(format!("font {which}: title, text or code")),
            }
        }
        ["size", which, "=", v] => {
            let v = num(v)?.clamp(6.0, 48.0);
            match *which {
                "title" => sh.sizes[0] = v,
                "subtitle" => sh.sizes[1] = v,
                "text" => sh.sizes[2] = v,
                "small" => sh.sizes[3] = v,
                "tiny" => sh.sizes[4] = v,
                "code" => sh.code_size = v,
                _ => return Err(format!("size {which}: title, subtitle, text, small, tiny or code")),
            }
        }
        ["corner", "=", v] => sh.corner = num(v)?.min(12.0),
        ["padding", which, "=", x, y] => {
            let p = [num(x)?, num(y)?];
            match *which {
                "button" => sh.button = p,
                "field" => sh.field = p,
                _ => return Err(format!("padding {which}: button or field")),
            }
        }
        ["spacing", "=", x, y] => sh.spacing = [num(x)?, num(y)?],
        ["colour" | "color", which, "=", v] => {
            let c = hex(v).ok_or(format!("not a colour: {v} (#RRGGBB)"))?;
            let p = &mut st.palette;
            match *which {
                "background" => p.background = c,
                "panel" => p.panel = c,
                "raised" => p.raised = c,
                "text" => p.text = c,
                "muted" => p.muted = c,
                "faint" => p.faint = c,
                "accent" => p.accent = c,
                "warn" => p.warn = c,
                _ => {
                    return Err(format!(
                        "colour {which}: background, panel, raised, text, muted, faint, accent or warn"
                    ))
                }
            }
        }
        ["sources", "=", rest @ ..] => {
            let cs: Option<Vec<Rgb>> = rest.iter().map(|w| hex(w)).collect();
            st.palette.sources = cs.filter(|c| !c.is_empty()).ok_or("sources = #RRGGBB #RRGGBB ...".to_string())?;
        }
        _ => return Err(format!("not understood: {line}")),
    }
    Ok(())
}

/// The styles there are: the factory's (the human's own file comes in 2b, beside the memory).
pub fn styles() -> Vec<Style> {
    read(FACTORY).0
}

/// The palettes of the styles (each variant one).
pub fn presets() -> Vec<Palette> {
    styles().into_iter().map(|s| s.palette).collect()
}

/// The shape of the style a palette comes from (the first one's, if it is a palette of one's own).
pub fn shape_of(palette: &Palette) -> Shape {
    let all = styles();
    all.iter().find(|s| s.palette.name == palette.name).or(all.first()).map(|s| s.shape.clone()).unwrap_or_default()
}

/// The shape in use (one window, one style at a time).
static SHAPE: std::sync::RwLock<Option<Shape>> = std::sync::RwLock::new(None);

fn shape() -> Shape {
    SHAPE.read().ok().and_then(|s| s.clone()).unwrap_or_default()
}

/// A size of text of the style.
pub fn size(s: Size) -> f32 {
    let k = s as usize;
    shape().sizes[k]
}

/// The text's font at one of the style's sizes (the titles' font for titles and subtitles).
pub fn font(s: Size) -> egui::FontId {
    let family = if matches!(s, Size::Title | Size::Subtitle) { title_family() } else { FontFamily::Proportional };
    egui::FontId::new(size(s), family)
}

/// The code's font (only the code: what is written in GRML, the prompt, the journal's lines).
pub fn code(s: Size) -> egui::FontId {
    let sz = if s == Size::Text { shape().code_size } else { size(s) };
    egui::FontId::new(sz, FontFamily::Monospace)
}

/// The corner of everything drawn (few are round).
pub fn corner() -> f32 {
    shape().corner
}

/// The room inside a field where words are written.
pub fn field() -> egui::Margin {
    let f = shape().field;
    egui::Margin::symmetric(f[0] as i8, f[1] as i8)
}

fn title_family() -> FontFamily {
    FontFamily::Name("title".into())
}

impl Palette {
    /// A palette with nothing said yet (a style's colours are said in its file).
    fn bare() -> Palette {
        Palette {
            name: String::new(),
            dark: true,
            background: [0x17, 0x18, 0x1B],
            panel: [0x1F, 0x20, 0x24],
            raised: [0x2B, 0x2D, 0x32],
            text: [0xE4, 0xE2, 0xDC],
            muted: [0x9C, 0x9F, 0xA6],
            faint: [0x45, 0x48, 0x50],
            accent: [0xE8, 0xA8, 0x3D],
            warn: [0xF0, 0x8A, 0x4B],
            sources: vec![[0xB9, 0xA6, 0xF0]],
        }
    }
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
    /// What is chosen, filled: the accent made a ground for words (`under_words`).
    pub chosen: Color32,
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
            chosen: under_words(c(p.accent)),
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

    /// The words that stand on what is filled (a chosen tab, a pressed button): always the dark ink, on a fill made
    /// light enough for it (`under_words`).
    pub fn on_chosen(&self) -> Color32 {
        ON_FILL
    }
}

/// How much two colours stand apart for the eye (WCAG: 1 .. 21; 4.5 reads as text).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let f = |x: u8| {
            let v = x as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The ink of the words on anything filled: dark, always (never white on a colour).
pub const ON_FILL: Color32 = Color32::from_rgb(0x14, 0x14, 0x16);

/// How far apart the words and what they stand on must be, at least (WCAG's 7: read without effort).
pub const WORDS_APART: f32 = 7.0;

/// A colour made a fill that words can stand on: the same colour, lightened toward white until the dark ink reads
/// on it without effort. Never two colours close to each other, whatever the style or a look gives.
pub fn under_words(c: Color32) -> Color32 {
    let mut k = 0.0;
    loop {
        let f = mixed(c, Color32::WHITE, k);
        if contrast(ON_FILL, f) >= WORDS_APART || k >= 1.0 {
            return f;
        }
        k += 0.05;
    }
}

/// What is pressed: the raised ground tinted by the accent, as much as the text's own colour still reads on it
/// without effort.
pub fn pressed(ink: &Ink) -> Color32 {
    let mut k = 0.55;
    loop {
        let f = mixed(ink.raised, ink.accent, k);
        if contrast(ink.text, f) >= WORDS_APART || k <= 0.0 {
            return f;
        }
        k -= 0.05;
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
pub fn apply(ctx: &egui::Context, p: &Palette, zoom: f32) {
    let ink = Ink::from(p);
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, ink));
    let v = visuals(p, &ink);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if p.dark { egui::Theme::Dark } else { egui::Theme::Light });
    rest(ctx, shape_of(p), zoom);
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
    v.selection.bg_fill = ink.chosen;
    v.selection.stroke = Stroke::new(1.0_f32, ink.on_chosen());
    // what can be clicked shows a hand (a text cursor only where text is written)
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(3);
    v.window_stroke = Stroke::new(1.0_f32, ink.faint);
    v.window_shadow =
        egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) };
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(80) };
    let r = CornerRadius::same(2);
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
    w.active.bg_fill = pressed(&ink);
    w.active.weak_bg_fill = pressed(&ink);
    w.active.bg_stroke = Stroke::new(1.0_f32, ink.accent);
    w.active.fg_stroke = Stroke::new(1.5_f32, if p.dark { lighter(ink.text, 0.3) } else { darker(ink.text, 0.3) });
    w.active.corner_radius = r;
    w.open.bg_fill = lift(ink.raised, 0.05);
    w.open.weak_bg_fill = lift(ink.raised, 0.05);
    w.open.corner_radius = r;
    v
}

fn rest(ctx: &egui::Context, sh: Shape, zoom: f32) {
    let sizes = sh.sizes;
    ctx.all_styles_mut(|st| {
        st.spacing.item_spacing = egui::vec2(sh.spacing[0], sh.spacing[1]);
        st.spacing.button_padding = egui::vec2(sh.button[0], sh.button[1]);
        st.spacing.interact_size.y = (sizes[2] * 1.85).max(22.0);
        // words shown are not taken for words to select (no text cursor over a label)
        st.interaction.selectable_labels = false;
        // check boxes and radio buttons with a little room
        st.spacing.icon_width = 17.0;
        st.spacing.icon_width_inner = 9.0;
        st.spacing.icon_spacing = 8.0;
        st.spacing.window_margin = egui::Margin::same(12);
        st.animation_time = 0.12;
        use egui::TextStyle as T;
        st.text_styles = [
            (T::Heading, egui::FontId::new(sizes[0], title_family())),
            (T::Name("subtitle".into()), egui::FontId::new(sizes[1], title_family())),
            (T::Body, egui::FontId::new(sizes[2], FontFamily::Proportional)),
            (T::Button, egui::FontId::new(sizes[2], FontFamily::Proportional)),
            (T::Small, egui::FontId::new(sizes[3], FontFamily::Proportional)),
            (T::Monospace, egui::FontId::new(sh.code_size, FontFamily::Monospace)),
        ]
        .into();
    });
    // the style's fonts (egui's own after them, for the signs they lack)
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in FONTS {
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    // after the style's font, egui's own (the signs it lacks: the banner's blocks, arrows, emoji)
    let mut after: Vec<String> = vec![];
    for f in [FontFamily::Monospace, FontFamily::Proportional] {
        for n in fonts.families.get(&f).cloned().unwrap_or_default() {
            if !after.contains(&n) {
                after.push(n);
            }
        }
    }
    let mut put = |family: FontFamily, first: &str| {
        let mut list = vec![first.to_string()];
        list.extend(after.iter().cloned());
        fonts.families.insert(family, list);
    };
    put(title_family(), &sh.title_font);
    put(FontFamily::Monospace, &sh.code_font);
    put(FontFamily::Proportional, &sh.text_font);
    ctx.set_fonts(fonts);
    ctx.set_zoom_factor(zoom.clamp(0.8, 2.0));
    if let Ok(mut s) = SHAPE.write() {
        *s = Some(sh);
    }
}

/// What a field waits for: in italics and quiet, so that it is not taken for something written.
pub fn hint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).italics()
}

/// Lines shown as a terminal shows them: a dark pane, a fixed font, coloured as code is, room around.
pub fn terminal(ui: &mut egui::Ui, lines: &[String]) {
    let ink = Ink::of(ui.ctx());
    let dark = Color32::from_rgb(0x16, 0x17, 0x1b);
    egui::Frame::new().fill(dark).corner_radius(corner()).inner_margin(egui::Margin::symmetric(12, 10)).show(
        ui,
        |ui| {
            ui.set_min_width(ui.available_width());
            // the code's colours, on a dark ground whatever the palette
            let term =
                Ink { text: Color32::from_rgb(0xd8, 0xd8, 0xd0), muted: Color32::from_rgb(0x8a, 0x8c, 0x92), ..ink };
            for l in lines {
                let job = crate::code::highlight_line(&term, l);
                ui.label(job);
            }
        },
    );
}

/// A selected tab or button: the accent behind, the text that stands on it.
pub fn chosen(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let ink = Ink::of(ui.ctx());
    let rich = egui::RichText::new(text).color(if on { ink.on_chosen() } else { ink.text });
    let b = egui::Button::new(rich).fill(if on { ink.chosen } else { ink.raised });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Played again: « attention aux boutons écrits en orange sur fond jaune », then « le blanc sur les boutons
    /// jaunes ça dégage » (never two colours so close): the words on what is chosen are dark, on a fill light
    /// enough, in every variant of the style and on any colour a look gives.
    #[test]
    fn the_words_on_what_is_filled_read_in_every_variant_and_on_any_colour() {
        let (styles, _) = read(FACTORY);
        for st in &styles {
            let ink = Ink::from(&st.palette);
            let c = contrast(ink.on_chosen(), ink.chosen);
            assert!(c >= WORDS_APART, "{}: {c:.2}", st.palette.name);
            let c = contrast(ink.text, pressed(&ink));
            assert!(c >= WORDS_APART, "{} pressed: {c:.2}", st.palette.name);
        }
        for colour in [
            Color32::from_rgb(0xB8, 0x6E, 0x00),
            Color32::from_rgb(0x00, 0x00, 0xFF),
            Color32::from_rgb(0x80, 0x00, 0x00),
            Color32::from_rgb(0xFF, 0xD0, 0x40),
            Color32::BLACK,
            Color32::WHITE,
        ] {
            assert!(contrast(ON_FILL, under_words(colour)) >= WORDS_APART, "{colour:?}");
        }
        // a light colour is kept as it is
        assert_eq!(under_words(Color32::from_rgb(0xFF, 0xD0, 0x40)), Color32::from_rgb(0xFF, 0xD0, 0x40));
    }

    /// Nothing filled with the accent itself: what is chosen is filled with `chosen`, its words in `on_chosen`.
    #[test]
    fn nothing_is_filled_with_a_colour_words_cannot_stand_on() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let s = std::fs::read_to_string(&p).unwrap();
            // written in two halves, so that this test does not find itself
            for bad in [
                concat!(".fill(ink.", "accent)"),
                concat!("bg_fill = ink.", "accent"),
                concat!("on_", "accent"),
                concat!("Color32::", "WHITE }"),
                concat!("{ ink.", "surface } else"),
            ] {
                assert!(!s.contains(bad), "{}: {bad}", p.display());
            }
        }
    }

    #[test]
    fn the_factory_style_reads_whole_and_its_variants_change_only_what_they_say() {
        let (styles, wrong) = read(FACTORY);
        assert!(wrong.is_empty(), "{wrong:?}");
        let names: Vec<&str> = styles.iter().map(|s| s.palette.name.as_str()).collect();
        assert_eq!(names, ["night", "day", "ocean", "contrast"]);
        let (night, day) = (&styles[0], &styles[1]);
        assert!(night.palette.dark && !day.palette.dark);
        assert_eq!(night.shape, day.shape, "a variant keeps the shape it does not change");
        assert_ne!(night.palette.background, day.palette.background);
        assert_eq!(night.shape.code_font, "Courier Prime");
        assert_eq!(night.shape.text_font, "IBM Plex Sans");
        assert_eq!(night.palette.sources.len(), 8);
    }

    #[test]
    fn what_a_style_file_says_wrong_is_said_with_its_line() {
        let (styles, wrong) = read(
            "corner = 3\nstyle mine\n  font text = \"Comic\"\n  size huge = 40\n  colour accent = red\n  corner = 4 # round enough\nvariant other from nobody\n",
        );
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].shape.corner, 4.0);
        let lines: Vec<usize> = wrong.iter().map(|w| w.0).collect();
        assert_eq!(lines, [1, 3, 4, 5, 7], "{wrong:?}");
    }

    /// Few sizes, few corners: no widget has a size or a corner of its own, all come from the style.
    #[test]
    fn no_size_of_text_and_no_corner_is_written_outside_the_style() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let literal = |s: &str, at: &str| {
            s.match_indices(at)
                .any(|(k, _)| s[k + at.len()..].chars().next().is_some_and(|c| c.is_ascii_digit() && c != '0'))
        };
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name == "style.rs" {
                continue;
            }
            let text = std::fs::read_to_string(e.path()).unwrap();
            for (k, line) in text.lines().enumerate() {
                for at in [
                    "FontId::proportional(",
                    "FontId::monospace(",
                    "RichText::new(",
                    ".size(",
                    "corner_radius(",
                    "CornerRadius::same(",
                ] {
                    let bad = if at == "RichText::new(" { false } else { literal(line, at) };
                    assert!(!bad, "{name}:{}: a size or a corner of its own: {}", k + 1, line.trim());
                }
            }
        }
    }
}
