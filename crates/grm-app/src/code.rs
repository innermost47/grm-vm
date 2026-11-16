// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! An app's code: its file, as text, coloured by what each word is. A pause in the typing (or Ctrl+Enter) and it
//! is said again into the running app, and written to its file. The lines it could not say are marked in amber,
//! with why.

use std::time::{Duration, Instant};

use eframe::egui::{self, text::LayoutJob, Color32, FontId, RichText, TextFormat};

use crate::app::{App, Ink};
use crate::text::t;

/// How long a pause in the typing is before the code is said.
pub const PAUSE: Duration = Duration::from_millis(700);

pub struct Draft {
    pub text: String,
    /// The text the app last had (to know when it changed from elsewhere).
    pub base: String,
    pub dirty: Option<Instant>,
}

const STATEMENTS: &[&str] = &[
    "node", "relate", "keep", "observe", "forget", "var", "set", "carry", "learn", "input", "output", "fuse",
    "network", "end", "call", "use",
];
const APP_WORDS: &[&str] = &["app", "widget", "api", "read"];

/// The words the prompt knows (completed with Tab, coloured as commands).
pub const PROMPT_WORDS: &[&str] = &[
    "node",
    "relate",
    "keep",
    "observe",
    "forget",
    "var",
    "set",
    "carry",
    "learn",
    "input",
    "output",
    "fuse",
    "network",
    "call",
    "use",
    "tick",
    "show",
    "status",
    "joint",
    "sandbox",
    "in",
    "run",
    "trial",
    "merge",
    "refuse",
    "sources",
    "quarantine",
    "release",
    "recall",
    "remove",
    "journal",
    "track",
    "forecast",
    "why",
    "rewind",
    "embody",
    "insights",
    "discover",
    "connect",
    "take",
    "feed",
    "see",
    "sound",
    "voice",
    "volume",
    "play",
    "pause",
    "stop",
    "mute",
    "unmute",
    "solo",
    "gain",
    "pan",
    "mixer",
    "panic",
    "render",
    "sleep",
    "wake",
    "threads",
    "help",
    "quit",
    "ask",
    "put",
    "inspect",
    "outputs",
    "check",
    "master",
    "focus",
    "history",
    "clear",
    "tour",
    "lang",
    "load",
    "undo",
    "redo",
    "stats",
    "pwd",
    "cd",
    "ls",
    "head",
    "tail",
    "cat",
    "mkdir",
    "touch",
    "cp",
    "mv",
    "rm",
    "new",
    "edit",
    "open",
    "programs",
    "app",
    "apps",
];

/// One line said at the prompt, coloured as code is (its first word: a statement, or a command of the prompt).
pub fn highlight_line(ink: &Ink, text: &str) -> LayoutJob {
    colour_code(ink, text, &[], true)
}

/// A program, coloured: statements, the app's words, commands, numbers, comments; the lines it could not say
/// underlined.
pub fn highlight(ink: &Ink, text: &str, errors: &[usize]) -> LayoutJob {
    colour_code(ink, text, errors, false)
}

fn colour_code(ink: &Ink, text: &str, errors: &[usize], prompt: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(13.0);
    for (k, line) in text.split_inclusive('\n').enumerate() {
        let bad = errors.contains(&(k + 1));
        let (code, comment) = match grm_shell::apps::comment_at(line) {
            Some(i) => (&line[..i], &line[i..]),
            None => (line, ""),
        };
        let mut first = true;
        let mut word = String::new();
        let mut push = |job: &mut LayoutJob, s: &str, colour: Color32, first: bool| {
            let mut f = TextFormat::simple(font.clone(), colour);
            if bad {
                f.underline = egui::Stroke::new(1.0_f32, ink_amber(colour));
            }
            let _ = first;
            job.append(s, 0.0, f);
        };
        let flush = |job: &mut LayoutJob,
                     word: &mut String,
                     first: &mut bool,
                     push: &mut dyn FnMut(&mut LayoutJob, &str, Color32, bool)| {
            if word.is_empty() {
                return;
            }
            let lower = word.to_lowercase();
            let colour = if *first && STATEMENTS.contains(&lower.as_str()) {
                ink.accent
            } else if *first && APP_WORDS.contains(&lower.as_str()) {
                ink.source(1)
            } else if *first
                && (grm_shell::apps::COMMANDS.contains(&lower.as_str())
                    || prompt && PROMPT_WORDS.contains(&lower.as_str()))
            {
                ink.source(2)
            } else if word.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                ink.source(4)
            } else {
                ink.text
            };
            push(job, word, colour, *first);
            *first = false;
            word.clear();
        };
        for c in code.chars() {
            if c.is_alphanumeric() || c == '_' || c == '.' || c == '~' || (c == '-' && word.is_empty()) {
                word.push(c);
            } else {
                flush(&mut job, &mut word, &mut first, &mut push);
                let mut b = [0u8; 4];
                let s = c.encode_utf8(&mut b);
                let colour = if "=:±+-*/^()[]<>".contains(c) { ink.muted } else { ink.text };
                push(&mut job, s, colour, false);
            }
        }
        flush(&mut job, &mut word, &mut first, &mut push);
        if !comment.is_empty() {
            job.append(comment, 0.0, TextFormat::simple(font.clone(), ink.muted));
        }
    }
    job
}

fn ink_amber(_c: Color32) -> Color32 {
    Color32::from_rgb(0xE8, 0xB0, 0x4A)
}

pub fn show(app: &mut App, ui: &mut egui::Ui, name: &str) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(view) = app.apps.iter().find(|a| a.name == name).cloned() else { return };
    let draft = app.drafts.entry(name.to_string()).or_insert_with(|| Draft {
        text: view.text.clone(),
        base: view.text.clone(),
        dirty: None,
    });
    // changed from elsewhere (its surface, its network, undo): the code follows, unless it is being typed
    if draft.base != view.text && draft.dirty.is_none() {
        draft.text = view.text.clone();
        draft.base = view.text.clone();
    }
    let error_lines: Vec<usize> = view.errors.iter().map(|e| e.0).collect();
    ui.horizontal(|ui| {
        ui.label(RichText::new(view.path.display().to_string()).monospace().small().color(ink.muted));
        if draft.dirty.is_some() {
            ui.label(RichText::new(t(lang, "typing")).small().color(ink.muted));
        } else {
            ui.label(RichText::new(t(lang, "saved")).small().color(ink.muted));
        }
    });
    let errors_h = (view.errors.len().min(6) as f32) * 18.0 + if view.errors.is_empty() { 0.0 } else { 8.0 };
    let mut changed = false;
    egui::ScrollArea::both().max_height((ui.available_height() - errors_h).max(80.0)).id_salt(("code", name)).show(
        ui,
        |ui| {
            ui.horizontal_top(|ui| {
                // line numbers, the lines it could not say in amber
                let n = draft.text.lines().count().max(1);
                let mut numbers = LayoutJob::default();
                for k in 1..=n {
                    let colour = if error_lines.contains(&k) { ink.amber } else { ink.faint };
                    numbers.append(&format!("{k:>3}\n"), 0.0, TextFormat::simple(FontId::monospace(13.0), colour));
                }
                ui.label(numbers);
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                    // no wrapping: a line of code stays one line, beside its number
                    let mut job = highlight(&ink, text.as_str(), &error_lines);
                    job.wrap.max_width = f32::INFINITY;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let r = ui.add(
                    egui::TextEdit::multiline(&mut draft.text)
                        .id(egui::Id::new(("code-edit", name)))
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(24)
                        .lock_focus(true)
                        .layouter(&mut layouter),
                );
                if r.changed() {
                    changed = true;
                }
                if r.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
                    draft.dirty = Some(Instant::now() - PAUSE);
                }
            });
        },
    );
    if changed {
        draft.dirty = Some(Instant::now());
    }
    let look_errors = app.looks.get(name).map(|l| l.errors.clone()).unwrap_or_default();
    for (l, e) in view.errors.iter().chain(look_errors.iter()) {
        ui.label(RichText::new(format!("{} {l}: {e}", t(lang, "line"))).small().color(ink.amber));
    }
    // a pause: it is said
    if let Some(at) = draft.dirty {
        if at.elapsed() >= PAUSE {
            draft.dirty = None;
            draft.base = draft.text.clone();
            let text = draft.text.clone();
            app.edit_app_text(name, text);
        } else {
            ui.ctx().request_repaint_after(PAUSE);
        }
    }
}
