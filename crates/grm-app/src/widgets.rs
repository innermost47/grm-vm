// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The input widgets, the same everywhere (the window's panels, the workshop's forms, the apps' surfaces): a field
//! that says what it waits for and what is missing, a choice that always shows what is chosen, a toggle readable
//! when on. Each takes its look from the style.

use eframe::egui::{self, RichText};

use crate::app::Ink;
use crate::text::{t, Lang};

/// What a field waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// A name: letters, digits and `_`, beginning with a letter.
    Name,
    /// A number (a comma or a point before the decimals, as one writes it).
    Number,
    /// An equation: `NAME = expression`.
    Equation,
    /// An expression: `x + speed`.
    Expression,
}

/// A number as it is written or pasted: `9,81`, `9.81`, `-2,5e3`, `1 000,5`, `9,81 m/s²` (what follows the number
/// is its unit, left aside). None: there is no number in it.
pub fn number(s: &str) -> Option<f64> {
    let s = s.trim();
    let mut kept = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (k, &c) in chars.iter().enumerate() {
        let digit_at = |at: usize| chars.get(at).is_some_and(|x| x.is_ascii_digit());
        let digit_before = k > 0 && digit_at(k - 1);
        match c {
            '0'..='9' | '.' | ',' => kept.push(c),
            '+' | '-' if kept.is_empty() || kept.ends_with(['e', 'E']) => kept.push(c),
            // an exponent: after a digit, before a digit (or its sign and a digit)
            'e' | 'E'
                if kept.ends_with(|x: char| x.is_ascii_digit())
                    && (digit_at(k + 1) || (matches!(chars.get(k + 1), Some('-' | '+')) && digit_at(k + 2))) =>
            {
                kept.push(c)
            }
            // a space between digits groups thousands (a narrow one too)
            ' ' | '\u{a0}' | '\u{202f}' if digit_before && digit_at(k + 1) => {}
            _ => break,
        }
    }
    let kept = match (kept.contains('.'), kept.contains(',')) {
        // 1,000.5: the commas group thousands
        (true, true) => kept.replace(',', ""),
        // 9,81: the comma is the decimal point
        (false, true) if kept.matches(',').count() == 1 => kept.replace(',', "."),
        (false, true) => kept.replace(',', ""),
        _ => kept,
    };
    kept.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// What is missing for a field to say what it waits for, in words with an example (None: it says it).
pub fn missing(need: Need, s: &str, lang: Lang) -> Option<String> {
    let s = s.trim();
    let fine = match need {
        Need::Name => grm_engine::lang::is_name(s),
        Need::Number => number(s).is_some(),
        Need::Equation => s.contains('=') && crate::math::parse(s).is_some(),
        Need::Expression => crate::math::parse(s).is_some(),
    };
    if fine {
        return None;
    }
    let key = match need {
        Need::Name => "need_name",
        Need::Number => "need_number",
        Need::Equation => "need_equation",
        Need::Expression => "need_expression",
    };
    Some(t(lang, key).to_string())
}

/// What a field holds, written as the language wants it (a number with a point).
pub fn written(need: Need, s: &str) -> String {
    match need {
        Need::Number => number(s).map(grm_engine::lang::g).unwrap_or_else(|| s.trim().to_string()),
        _ => s.trim().to_string(),
    }
}

/// A field: what it waits for shown in it while empty (an example), and under it, once something is written,
/// what is missing (quietly, in words). Pasting works as typing does. Whether it says what it waits for, and the
/// field itself (its focus, Enter).
pub fn field(
    ui: &mut egui::Ui,
    value: &mut String,
    need: Need,
    example: &str,
    width: f32,
    lang: Lang,
) -> (bool, egui::Response) {
    let ink = Ink::of(ui.ctx());
    let code = matches!(need, Need::Equation | Need::Expression);
    let wrong = missing(need, value, lang);
    ui.vertical(|ui| {
        let mut edit = egui::TextEdit::singleline(value)
            .margin(crate::style::field())
            .desired_width(width)
            .hint_text(crate::style::hint(example));
        if code {
            edit = edit.font(egui::TextStyle::Monospace);
        }
        let r = ui.add(edit);
        if let Some(w) = wrong.as_ref().filter(|_| !value.trim().is_empty()) {
            ui.label(RichText::new(w).small().color(ink.amber));
        }
        (wrong.is_none(), r)
    })
    .inner
}

/// A choice among a few: it always shows the one chosen (its words), and the list opens under it. True when
/// another was chosen.
pub fn choice(ui: &mut egui::Ui, id: impl std::hash::Hash, chosen: &mut usize, options: &[String], tip: &str) -> bool {
    let before = *chosen;
    let shown = options.get(*chosen).cloned().unwrap_or_default();
    let r = egui::ComboBox::from_id_salt(id).selected_text(shown).show_ui(ui, |ui| {
        for (k, o) in options.iter().enumerate() {
            ui.selectable_value(chosen, k, o);
        }
    });
    if !tip.is_empty() {
        r.response.on_hover_text(tip);
    }
    *chosen != before
}

/// A button that stays pressed while what it does is on: its sign and its words, readable on (the words take the
/// colour that stands on the accent) and off; what it does said when the pointer rests on it.
pub fn toggle(ui: &mut egui::Ui, sign: &str, words: &str, on: bool, tip: &str) -> egui::Response {
    let ink = Ink::of(ui.ctx());
    let colour = if on { ink.on_accent() } else { ink.text };
    let mut b =
        egui::Button::image_and_text(crate::icons::image(sign, 15.0, colour), RichText::new(words).color(colour));
    if on {
        b = b.fill(ink.accent);
    }
    let r = ui.add(b);
    if tip.is_empty() {
        r
    } else {
        r.on_hover_text(tip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_read_as_it_is_written_or_pasted() {
        for (s, v) in [
            ("9,81", 9.81),
            ("9.81", 9.81),
            (" -2,5e3 ", -2500.0),
            ("1 000,5", 1000.5),
            ("1,000.5", 1000.5),
            ("9,81 m/s²", 9.81),
            ("+4", 4.0),
            ("3e-2", 0.03),
        ] {
            assert_eq!(number(s), Some(v), "{s}");
        }
        for s in ["", "m/s", "-", ",", "abc 3"] {
            assert_eq!(number(s), None, "{s}");
        }
        assert_eq!(written(Need::Number, "9,81"), "9.81");
    }

    /// Played again: « le symbole # ne marche pas dans la liste des tonalités » (the fonts have no ♯ nor ♭): her
    /// keys are written with # and b.
    #[test]
    fn the_keys_are_named_with_signs_the_fonts_have() {
        let her = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../widgets/her.grm")).unwrap();
        assert!(her.contains("key: C#=4") && her.contains("key: Bb=1"));
        assert!(!her.contains(['♯', '♭']));
    }

    #[test]
    fn what_is_missing_is_said_with_an_example() {
        let fr = Lang::Fr;
        assert!(missing(Need::Number, "9,81", fr).is_none());
        assert!(missing(Need::Number, "vite", fr).unwrap().contains("9,81"));
        assert!(missing(Need::Name, "2x", fr).is_some() && missing(Need::Name, "masse_1", fr).is_none());
        assert!(missing(Need::Equation, "force = masse * a", fr).is_none());
        assert!(missing(Need::Equation, "masse *", fr).is_some());
    }
}
