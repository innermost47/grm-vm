// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Files opened in the machine: any text (a program, notes), and tables as tables (cells to change, rows and
//! columns to add), written back as they were separated. And the window that asks which columns of a table to load.

use std::path::PathBuf;

use eframe::egui::{self, RichText};

use crate::app::{App, Ink};
use crate::text::t;

pub struct FileTab {
    pub path: PathBuf,
    pub text: String,
    pub saved: String,
    /// A table: its header as written, its cells, its separator.
    pub table: Option<(Vec<String>, Vec<Vec<String>>, char)>,
    pub table_saved: bool,
}

impl FileTab {
    pub fn open(path: PathBuf) -> FileTab {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let text = grm_connect::csv::text_of(&bytes);
        let table = if path.extension().is_some_and(|x| x == "csv" || x == "tsv") {
            let c = grm_connect::csv::read(&text);
            let mut written = c.written.clone();
            if written.is_empty() {
                written = vec!["a".into(), "b".into()];
            }
            Some((written, c.cells.clone(), if text.trim().is_empty() { ',' } else { c.separator }))
        } else {
            None
        };
        FileTab { path, saved: text.clone(), text, table, table_saved: true }
    }

    pub fn name(&self) -> String {
        self.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
    }

    pub fn dirty(&self) -> bool {
        if self.table.is_some() {
            !self.table_saved
        } else {
            self.text != self.saved
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some((w, cells, sep)) = &self.table {
            let text = grm_connect::csv::write(w, cells, *sep);
            std::fs::write(&self.path, &text)?;
            self.table_saved = true;
            self.saved = text.clone();
            self.text = text;
        } else {
            std::fs::write(&self.path, &self.text)?;
            self.saved = self.text.clone();
        }
        Ok(())
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui, k: usize) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let Some(f) = app.files.get_mut(k) else { return };
    let mut said: Option<String> = None;
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(f.path.display().to_string()).monospace().small().color(ink.muted));
        let save = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
        if ui
            .add_enabled(
                f.dirty(),
                egui::Button::image_and_text(crate::icons::image("save", 15.0, ink.text), t(lang, "save")),
            )
            .on_hover_text("Ctrl+S")
            .clicked()
            || (save && f.dirty())
        {
            if let Err(e) = f.save() {
                said = Some(format!("# {}: {e}", f.path.display()));
            }
        }
        if f.path.extension().is_some_and(|x| x == "grm")
            && crate::icons::button(ui, "play", t(lang, "run")).on_hover_text("F5").clicked()
        {
            let _ = f.save();
            said = Some(format!("use {}", f.path.display()));
        }
        if f.table.is_some() && ui.button(t(lang, "load_it")).clicked() {
            let _ = f.save();
            said = Some(format!("load {}", f.path.display()));
        }
        if ui.button(t(lang, "close")).clicked() {
            close = true;
        }
        if f.dirty() {
            ui.label(RichText::new(t(lang, "not_saved")).small().color(ink.amber));
        }
    });
    ui.separator();
    let path = f.path.clone();
    // what changed, to be taken back: (row, column, what it was) or the whole table
    let mut cell_was: Vec<(usize, usize, String)> = vec![];
    let mut table_was: Option<crate::journey::Table> = None;
    if let Some(table) = &mut f.table {
        let (written, cells, sep) = table;
        let sep = *sep;
        let mut changed = false;
        // one cell, kept as it was while it is being written in
        let mut cell = |ui: &mut egui::Ui, text: &mut String, row: usize, col: usize, changed: &mut bool| {
            let id = egui::Id::new(("cell", k, row, col));
            let before = ui.memory(|m| m.has_focus(id)).then(|| text.clone());
            let r =
                ui.add_sized([120.0, 22.0], egui::TextEdit::singleline(text).id(id).font(egui::TextStyle::Monospace));
            if r.changed() {
                *changed = true;
                if let Some(b) = before {
                    cell_was.push((row, col, b));
                }
            }
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("table", k)).striped(true).spacing([4.0, 2.0]).show(ui, |ui| {
                ui.label("");
                for (c, h) in written.iter_mut().enumerate() {
                    cell(ui, h, usize::MAX, c, &mut changed);
                }
                ui.end_row();
                let cols = written.len();
                for (r, row) in cells.iter_mut().enumerate().take(500) {
                    ui.label(RichText::new(format!("{}", r + 1)).small().color(ink.muted));
                    row.resize(cols, String::new());
                    for (c, text) in row.iter_mut().enumerate() {
                        cell(ui, text, r, c, &mut changed);
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                if crate::icons::button(ui, "between-horizontal-end", t(lang, "add_row")).clicked() {
                    table_was = Some((written.clone(), cells.clone(), sep));
                    cells.push(vec![String::new(); written.len()]);
                    changed = true;
                }
                if crate::icons::button(ui, "between-vertical-end", t(lang, "add_column")).clicked() {
                    table_was = Some((written.clone(), cells.clone(), sep));
                    written.push(format!("column{}", written.len() + 1));
                    for r in cells.iter_mut() {
                        r.push(String::new());
                    }
                    changed = true;
                }
                if cells.len() > 500 {
                    ui.label(
                        RichText::new(format!("{} {}", cells.len() - 500, t(lang, "rows_more")))
                            .small()
                            .color(ink.muted),
                    );
                }
            });
        });
        if changed {
            f.table_saved = false;
        }
    } else {
        let grm = f.path.extension().is_some_and(|x| x == "grm");
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _w: f32| {
                let mut job = if grm {
                    crate::code::highlight(&ink, text.as_str(), &[])
                } else {
                    egui::text::LayoutJob::simple(
                        text.as_str().to_string(),
                        egui::FontId::monospace(13.0),
                        ink.text,
                        f32::INFINITY,
                    )
                };
                job.wrap.max_width = f32::INFINITY;
                ui.fonts_mut(|f| f.layout_job(job))
            };
            ui.add(
                egui::TextEdit::multiline(&mut f.text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .lock_focus(true)
                    .layouter(&mut layouter),
            );
        });
    }
    for (row, col, text) in cell_was {
        let what = format!("{} {}", t(lang, "cell"), path.display());
        app.journey.record(crate::journey::Change::Cell { path: path.clone(), row, col, text }, what);
    }
    if let Some(table) = table_was {
        let what = format!("{} {}", t(lang, "table_shape"), path.display());
        app.journey.record(crate::journey::Change::Table { path: path.clone(), table }, what);
    }
    if close {
        app.files.remove(k);
        app.current_file = None;
    }
    if let Some(l) = said {
        app.say(l);
    }
}

/// The window asking which columns of a table to load.
pub fn columns(app: &mut App, ctx: &egui::Context) {
    let Some((path, names, written, mut checks)) = app.columns.clone() else { return };
    let lang = app.settings.lang;
    let mut open = true;
    let mut go = false;
    egui::Window::new(t(lang, "which_columns")).open(&mut open).collapsible(false).default_width(420.0).show(
        ctx,
        |ui| {
            ui.label(RichText::new(path.display().to_string()).monospace().small());
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (k, (n, w)) in names.iter().zip(&written).enumerate() {
                    let label = if n == w { n.clone() } else { format!("{w}  →  {n}") };
                    ui.checkbox(&mut checks[k], label);
                }
            });
            ui.horizontal(|ui| {
                if ui.button(t(lang, "all")).clicked() {
                    checks.iter_mut().for_each(|c| *c = true);
                }
                if ui.button(t(lang, "none")).clicked() {
                    checks.iter_mut().for_each(|c| *c = false);
                }
                if ui.add_enabled(checks.iter().any(|c| *c), egui::Button::new(t(lang, "load_it"))).clicked() {
                    go = true;
                }
            });
        },
    );
    if !open {
        app.columns = None;
    } else if go {
        let cols: Vec<String> = names.iter().zip(&checks).filter(|(_, c)| **c).map(|(n, _)| n.clone()).collect();
        app.columns = None;
        app.say(format!("load {} {}", path.display(), cols.join(" ")));
    } else {
        app.columns = Some((path, names, written, checks));
    }
}
