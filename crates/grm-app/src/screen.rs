// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The window's own screens, written in GRML (`screens/`), drawn as apps are: the workshop, the inspector's part
//! for the workshop in front. A screen is opened for a workshop (`{ws}` written as its name) and read again as soon
//! as its file changes.
//!
//! What the screens are made of, and what the window draws as a browser draws a button: `rows` (a list the world
//! fills: a workshop's nodes, relations, lines, proposals; the words of the language; the paths that teach it),
//! `editor` (a line of the language being written for a workshop), `network` (a workshop's network as it grows),
//! `airlock` (its way into the main network), and the `menu`s a right button offers. The lines a screen says
//! (`say "..."`) go to the prompt as typed; those that begin with `screen` are the window's own (`screen write WS
//! LINE`, `screen pick WS NAME`, `screen edit WS K`, `screen value WS NAME`, `screen path WS K`, `screen fit WS`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, vec2, Rect, RichText};

use grm_shell::apps::{MenuItem, Widget};
use grm_shell::runtime::AppView;

use crate::app::{App, Ink};
use crate::model::Picture;
use crate::text::{t, Lang};

/// What a workshop's editor holds between frames: the line being written (and the line it changes, when it changes
/// one), the path followed and its step, the line said and what came back (an error, an answer), the node whose
/// value is being changed in place.
#[derive(Debug, Clone, Default)]
pub struct Bench {
    pub text: String,
    pub replacing: Option<usize>,
    pub path: Option<(usize, usize)>,
    pub sent: Option<String>,
    pub error: Option<String>,
    pub answer: Option<String>,
    pub value_of: Option<(String, String)>,
    /// The editor wants the keyboard (a word was put in it).
    pub focus: bool,
}

/// One item of a list the world fills.
#[derive(Debug, Clone, Default)]
pub struct Item {
    /// What `{item}` says (a node's or a relation's name, a word).
    pub key: String,
    pub title: String,
    pub value: String,
    pub detail: String,
    pub sign: String,
    /// What a menu's `when` asks for: `heard`, `used`, `alone`.
    pub flags: Vec<&'static str>,
    /// Its place (from 0: `{item.i}`; from 1: `{item.k}`).
    pub i: usize,
    pub example: String,
    /// Its family (the words): rows of one family are under its title.
    pub family: String,
    /// Code (a line, an equation): shown in the code's letters.
    pub code: bool,
}

// ---------------------------------------------------------------------- the files

/// The screens' folder: beside the memory, or here.
pub fn dir(app: &App) -> PathBuf {
    let near = app.memory.parent().map(|p| p.join("screens"));
    match near.filter(|d| d.exists()) {
        Some(d) => d,
        None => PathBuf::from("screens"),
    }
}

/// A screen opened for a workshop: read (again when its file changed), kept under its key. None: no such file.
pub fn open(app: &mut App, file: &str, ws: &str) -> Option<String> {
    let path = dir(app).join(file);
    let key = format!("{}:{ws}", file.trim_end_matches(".grm"));
    let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    if let Some((s, _)) = app.screens.get(&key) {
        if *s == stamp {
            return Some(key);
        }
    }
    let text = std::fs::read_to_string(&path).ok()?.replace("{ws}", ws);
    let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let p = grm_shell::apps::parse(&text, &base);
    let view = AppView {
        name: key.clone(),
        title: p.title.clone().unwrap_or_else(|| key.clone()),
        icon: p.icon.clone(),
        path,
        text,
        errors: p.errors.clone(),
        widgets: p.widgets.clone(),
        lists: p.lists.clone(),
        prefix: Default::default(),
        apis: vec![],
        can_undo: false,
        can_redo: false,
        looks: p.looks.clone(),
        hands: vec![],
        menus: p.menus.clone(),
    };
    for (line, e) in &view.errors {
        app.note(grm_shell::Kind::Error, format!("{file}, line {line}: {e}"));
    }
    app.screens.insert(key.clone(), (stamp, view));
    Some(key)
}

/// A word of the language, as `screens/words.grm` says it.
#[derive(Debug, Clone, Default)]
pub struct WordDef {
    pub word: String,
    pub family: String,
    pub level: u8,
    pub icon: String,
    pub example: String,
    pub en: String,
    pub fr: String,
}

/// A path that teaches the language, as `screens/paths.grm` says it: its steps, each said and its line.
#[derive(Debug, Clone, Default)]
pub struct PathDef {
    pub name: String,
    pub icon: String,
    pub en: String,
    pub fr: String,
    pub steps: Vec<(String, String, String)>,
}

/// The words of a line, quotes kept together (`word node "a quantity" fr "une quantité"`).
fn tokens(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => {
                if quoted {
                    out.push(std::mem::take(&mut cur));
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// What follows a key among the tokens (`icon box`), and the quoted words that are not a key's.
fn keyed(tokens: &[String], keys: &[&str]) -> (HashMap<String, String>, Vec<String>) {
    let mut map = HashMap::new();
    let mut free = vec![];
    let mut k = 0;
    while k < tokens.len() {
        if keys.contains(&tokens[k].as_str()) && k + 1 < tokens.len() {
            map.insert(tokens[k].clone(), tokens[k + 1].clone());
            k += 2;
        } else {
            free.push(tokens[k].clone());
            k += 1;
        }
    }
    (map, free)
}

/// A family of words: its name, its sign, its words in English and in French.
pub type Family = (String, String, String, String);

/// The families and the words of `screens/words.grm`.
pub fn words(app: &App) -> (Vec<Family>, Vec<WordDef>) {
    let text = std::fs::read_to_string(dir(app).join("words.grm")).unwrap_or_default();
    let mut families = vec![];
    let mut words = vec![];
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let tk = tokens(line);
        let (map, free) = keyed(&tk[1.min(tk.len())..], &["family", "level", "icon", "example", "fr"]);
        match tk.first().map(String::as_str) {
            Some("family") => families.push((
                free.first().cloned().unwrap_or_default(),
                map.get("icon").cloned().unwrap_or_default(),
                free.get(1).cloned().unwrap_or_default(),
                map.get("fr").cloned().unwrap_or_default(),
            )),
            Some("word") => words.push(WordDef {
                word: free.first().cloned().unwrap_or_default(),
                family: map.get("family").cloned().unwrap_or_default(),
                level: map.get("level").and_then(|l| l.parse().ok()).unwrap_or(1),
                icon: map.get("icon").cloned().unwrap_or("circle".into()),
                example: map.get("example").cloned().unwrap_or_default(),
                en: free.get(1).cloned().unwrap_or_default(),
                fr: map.get("fr").cloned().unwrap_or_default(),
            }),
            _ => {}
        }
    }
    (families, words)
}

/// The paths of `screens/paths.grm`.
pub fn paths(app: &App) -> Vec<PathDef> {
    let text = std::fs::read_to_string(dir(app).join("paths.grm")).unwrap_or_default();
    let mut out: Vec<PathDef> = vec![];
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let tk = tokens(line);
        let (map, free) = keyed(&tk[1.min(tk.len())..], &["icon", "fr", "line"]);
        match tk.first().map(String::as_str) {
            Some("path") => out.push(PathDef {
                name: free.first().cloned().unwrap_or_default(),
                icon: map.get("icon").cloned().unwrap_or("footprints".into()),
                en: free.get(1).cloned().unwrap_or_default(),
                fr: map.get("fr").cloned().unwrap_or_default(),
                steps: vec![],
            }),
            Some("step") => {
                if let Some(p) = out.last_mut() {
                    p.steps.push((
                        free.first().cloned().unwrap_or_default(),
                        map.get("fr").cloned().unwrap_or_default(),
                        map.get("line").cloned().unwrap_or_default(),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

fn said_in(lang: Lang, en: &str, fr: &str) -> String {
    if lang == Lang::Fr && !fr.is_empty() {
        fr.to_string()
    } else {
        en.to_string()
    }
}

// ---------------------------------------------------------------------- what the world fills

/// A workshop's picture, pulled now and then (None while it has not come, or when there is no such workshop).
pub fn picture(app: &mut App, ws: &str) -> Option<Arc<Picture>> {
    let name = ws.to_string();
    let entry = app
        .workshop_pulls
        .entry(name.clone())
        .or_insert_with(|| (crate::model::Pull::new(std::time::Duration::from_millis(300)), None));
    let engine = app.engine.clone();
    if let Some(p) = entry.0.poll(&engine, move |w| Picture::of_workshop(w, &name)) {
        entry.1 = p.map(Arc::new);
    }
    entry.1.clone()
}

/// The words of a line that are names.
fn names_in(line: &str) -> Vec<&str> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '~')).filter(|w| !w.is_empty()).collect()
}

/// The line that made a name (its declaration), among a workshop's lines.
pub fn line_of(written: &[String], name: &str) -> Option<usize> {
    written.iter().position(|l| {
        let l = grm_engine::lang::normal(l);
        let mut w = l.split_whitespace();
        let first = w.next().unwrap_or("");
        let second = w.next().unwrap_or("").trim_end_matches(':');
        let second = second.split('=').next().unwrap_or("").trim();
        grm_engine::world::WORDS.contains(&first) && first != "observe" && first != "forget" && second == name
    })
}

/// The items of a source, for a workshop.
pub fn items(app: &mut App, source: &str, ws: &str) -> Vec<Item> {
    let lang = app.settings.lang;
    match source {
        "words" => {
            let (families, words) = words(app);
            let mut out = vec![];
            for (fam, _, en, fr) in &families {
                let mut of: Vec<&WordDef> = words.iter().filter(|w| &w.family == fam).collect();
                of.sort_by_key(|w| w.level);
                for (i, w) in of.into_iter().enumerate() {
                    out.push(Item {
                        key: w.word.clone(),
                        title: w.word.clone(),
                        detail: said_in(lang, &w.en, &w.fr),
                        sign: w.icon.clone(),
                        i,
                        example: w.example.clone(),
                        family: said_in(lang, en, fr),
                        code: true,
                        ..Default::default()
                    });
                }
            }
            out
        }
        "paths" => paths(app)
            .iter()
            .enumerate()
            .map(|(i, p)| Item {
                key: p.name.clone(),
                title: said_in(lang, &p.en, &p.fr),
                detail: format!("{} {}", p.steps.len(), t(lang, "ws_steps")),
                sign: p.icon.clone(),
                i,
                ..Default::default()
            })
            .collect(),
        _ => {
            let Some(p) = picture(app, ws) else { return vec![] };
            match source {
                "nodes" => p
                    .nodes
                    .iter()
                    .filter(|n| !n.name.contains("__"))
                    .map(|n| {
                        let made = line_of(&p.written, &n.name);
                        let used = p.written.iter().enumerate().any(|(k, l)| {
                            Some(k) != made && names_in(l).contains(&n.name.as_str()) && !l.starts_with("observe")
                        }) || p.relations.iter().any(|r| r.nodes.contains(&n.name));
                        let mut flags = vec![if used { "used" } else { "alone" }];
                        if n.output || n.name.ends_with("_s") {
                            flags.push("heard");
                        }
                        let what = if n.input {
                            "input"
                        } else if n.output {
                            "output"
                        } else if n.var {
                            "var"
                        } else if n.learn {
                            "learn"
                        } else {
                            "node"
                        };
                        let detail = match made {
                            Some(k) => p.written[k].clone(),
                            None => t(lang, "ws_made_by_relation").to_string(),
                        };
                        Item {
                            key: n.name.clone(),
                            title: n.name.clone(),
                            value: short_reading(n.value, n.sigma, n.signed),
                            detail,
                            sign: if n.observed { "gauge".into() } else { "circle-dot".into() },
                            flags,
                            i: made.unwrap_or(usize::MAX),
                            family: what.into(),
                            code: true,
                            ..Default::default()
                        }
                    })
                    .collect(),
                "relations" => p
                    .relations
                    .iter()
                    .map(|r| Item {
                        key: r.name.clone(),
                        title: r.name.clone(),
                        detail: r.text.clone(),
                        sign: if r.keep { "fence".into() } else { "spline".into() },
                        i: line_of(&p.written, &r.name).unwrap_or(usize::MAX),
                        flags: vec!["used"],
                        code: true,
                        ..Default::default()
                    })
                    .collect(),
                "lines" => p
                    .written
                    .iter()
                    .enumerate()
                    .map(|(i, l)| Item {
                        key: l.clone(),
                        title: l.clone(),
                        detail: crate::explain::explain(l, lang),
                        sign: "pen-line".into(),
                        i,
                        code: true,
                        ..Default::default()
                    })
                    .collect(),
                "moved" => p
                    .differences
                    .iter()
                    .enumerate()
                    .map(|(i, (n, a, b, s))| {
                        let pc = (b - a) / a.abs().max(1e-12) * 100.0;
                        Item {
                            key: n.clone(),
                            title: n.clone(),
                            value: format!(
                                "{} → {}",
                                grm_engine::lang::g((a * 1000.0).round() / 1000.0),
                                grm_engine::lang::g((b * 1000.0).round() / 1000.0)
                            ),
                            detail: format!(
                                "± {} · {}{:.1} %",
                                grm_engine::lang::g((s * 1000.0).round() / 1000.0),
                                if pc >= 0.0 { "+" } else { "" },
                                pc
                            ),
                            sign: "flask-conical".into(),
                            i,
                            code: true,
                            ..Default::default()
                        }
                    })
                    .collect(),
                "proposals" => p
                    .proposals
                    .iter()
                    .enumerate()
                    .map(|(i, (lines, why))| Item {
                        key: format!("{}", i + 1),
                        title: lines.join("\n"),
                        detail: why.clone(),
                        sign: "sparkles".into(),
                        i,
                        code: true,
                        ..Default::default()
                    })
                    .collect(),
                _ => vec![],
            }
        }
    }
}

/// A line with its item said in it (`{item}`, `{item.i}`, `{item.k}`, `{item.example}`).
pub fn with_item(line: &str, item: &Item) -> String {
    line.replace("{item.example}", &item.example)
        .replace("{item.i}", &item.i.to_string())
        .replace("{item.k}", &(item.i + 1).to_string())
        .replace("{item}", &item.key)
}

// ---------------------------------------------------------------------- what the window does

/// A line of the window's own (`screen ...`): done here, never sent to the prompt. False: not one.
pub fn window_line(app: &mut App, line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("screen ") else { return false };
    let mut w = rest.splitn(3, ' ');
    let (verb, ws, tail) = (w.next().unwrap_or(""), w.next().unwrap_or("").to_string(), w.next().unwrap_or(""));
    let written: Vec<String> = picture(app, &ws).map(|p| p.written.clone()).unwrap_or_default();
    if matches!(verb, "pick" | "value") {
        app.ws_inspector_closed.remove(&ws);
    }
    let bench = app.benches.entry(ws.clone()).or_default();
    match verb {
        "write" => {
            bench.text = tail.to_string();
            bench.replacing = None;
            bench.error = None;
            bench.focus = true;
        }
        "pick" => {
            let name = tail.trim().to_string();
            match line_of(&written, &name) {
                Some(k) => {
                    bench.text = written[k].clone();
                    bench.replacing = Some(k);
                }
                None => {
                    bench.text = format!("node {name} = 1");
                    bench.replacing = None;
                }
            }
            bench.error = None;
            bench.focus = true;
            app.ws_selected = Some((ws, name));
        }
        "edit" => {
            if let Some(k) = tail.trim().parse::<usize>().ok().filter(|k| *k < written.len()) {
                bench.text = written[k].clone();
                bench.replacing = Some(k);
                bench.error = None;
                bench.focus = true;
            }
        }
        "value" => {
            let name = tail.trim().to_string();
            let now =
                picture(app, &ws).and_then(|p| p.node(&name).map(|n| grm_engine::lang::g(n.value))).unwrap_or_default();
            let bench = app.benches.entry(ws.clone()).or_default();
            bench.value_of = Some((name.clone(), now));
            bench.error = None;
            bench.focus = true;
            app.ws_selected = Some((ws, name));
        }
        "path" => {
            if let Ok(k) = tail.trim().parse::<usize>() {
                bench.path = Some((k, 0));
                bench.error = None;
            }
        }
        "fit" => {
            app.ws_views.entry(ws).or_default().fitted = false;
        }
        "export" => {
            // a simulation shared: its lines, and what moved (before -> after, ± its spread now)
            let lang = app.settings.lang;
            let moved = items(app, "moved", &ws);
            let dir = app.memory.with_extension("simulations");
            let path = dir.join(format!("{ws}.grm"));
            let mut text = written.join("\n");
            text.push_str("\n# what moved (before -> after, ± its spread now):\n");
            for m in &moved {
                text.push_str(&format!("# {}: {} ({})\n", m.title, m.value, m.detail));
            }
            match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, text)) {
                Ok(_) => app.toast(format!("{} {}", t(lang, "sim_written"), path.display())),
                Err(e) => app.note(grm_shell::Kind::Warning, format!("! {}: {e}", path.display())),
            }
        }
        _ => {
            app.note(grm_shell::Kind::Warning, format!("? screen write|pick|edit|value|path|fit|export WS ...: {line}"))
        }
    }
    true
}

/// What the prompt answered to a workshop's line: an error said in its editor (so that it is seen where it was
/// written), an answer to a question shown there, the line written let go.
pub fn heard(app: &mut App, typed: &str, lines: &[grm_shell::Line]) {
    let mut w = typed.split_whitespace();
    if w.next() != Some("workshop") {
        return;
    }
    let verb = w.next().unwrap_or("");
    let Some(ws) = w.next() else { return };
    if !matches!(verb, "say" | "edit" | "unsay" | "remove" | "value" | "ask" | "accept" | "refuse" | "propose" | "play")
    {
        return;
    }
    let errors: Vec<String> = lines
        .iter()
        .filter(|l| matches!(l.kind, grm_shell::Kind::Error | grm_shell::Kind::Warning))
        .map(|l| l.text.trim_start_matches("! ").trim_start_matches("? ").to_string())
        .collect();
    let text: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
    let bench = app.benches.entry(ws.to_string()).or_default();
    if !errors.is_empty() {
        bench.error = Some(errors.join("\n"));
        return;
    }
    match verb {
        "ask" | "propose" => bench.answer = Some(text.join("\n")),
        "value" => {
            bench.value_of = None;
            bench.error = None;
        }
        _ => {
            bench.error = None;
            if bench.sent.as_deref() == Some(typed) {
                bench.text.clear();
                bench.replacing = None;
                bench.sent = None;
            }
        }
    }
}

// ---------------------------------------------------------------------- drawing

/// The words of a widget in the window's language.
pub fn label_of(w: &Widget, lang: Lang) -> String {
    let en = if w.text.is_empty() { w.name.clone() } else { w.text.clone() };
    said_in(lang, &en, w.fr.as_deref().unwrap_or(""))
}

/// What a menu offers on an item: each its sign, its words and what it does; greyed when the item lacks what it
/// waits for (`when`). True when one was chosen.
pub fn menu(app: &mut App, ui: &mut egui::Ui, items: &[MenuItem], item: &Item) -> bool {
    let lang = app.settings.lang;
    let mut chosen = None;
    for m in items {
        let enabled = m.when.as_deref().is_none_or(|f| item.flags.contains(&f));
        let words = said_in(lang, &m.words, m.fr.as_deref().unwrap_or(""));
        let what = said_in(lang, &m.what, m.what_fr.as_deref().unwrap_or(""));
        if crate::menu::item_if(ui, enabled, m.icon.as_deref().unwrap_or("circle"), &words, &what) {
            chosen = Some(with_item(&m.say, item));
        }
    }
    match chosen {
        Some(line) => {
            app.say(line);
            ui.close();
            true
        }
        None => false,
    }
}

/// A list the world fills, in its place on a screen.
pub fn rows(app: &mut App, ui: &mut egui::Ui, view: &AppView, w: &Widget, rect: Rect) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let source = w.nodes.first().cloned().unwrap_or_default();
    let ws = w.nodes.get(1).cloned().unwrap_or_default();
    let list = items(app, &source, &ws);
    let menu_items: Vec<MenuItem> = w.menu.as_ref().and_then(|m| view.menus.get(m)).cloned().unwrap_or_default();
    let chosen = app.ws_selected.as_ref().filter(|(x, _)| *x == ws).map(|(_, n)| n.clone());
    // what moved in a simulation: shown only in one
    if list.is_empty() && source == "moved" {
        return;
    }
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    // its title, and how many
    ui.horizontal(|ui| {
        ui.label(RichText::new(label_of(w, lang)).small().strong().color(ink.muted));
        if !list.is_empty() {
            ui.label(RichText::new(list.len().to_string()).small().color(ink.faint));
        }
    });
    if list.is_empty() {
        let key = match source.as_str() {
            "proposals" => "ws_no_proposal",
            "lines" => "ws_no_line",
            _ => "ws_nothing_yet",
        };
        ui.label(RichText::new(t(lang, key)).small().italics().color(ink.muted));
        return;
    }
    let mut said: Option<String> = None;
    let mut family = String::new();
    egui::ScrollArea::vertical().id_salt(("rows", &view.name, &w.name)).auto_shrink([false, false]).show(ui, |ui| {
        for item in &list {
            if source == "words" && item.family != family {
                family = item.family.clone();
                ui.add_space(4.0);
                ui.label(RichText::new(&family).small().color(ink.muted));
            }
            let on = chosen.as_deref() == Some(item.key.as_str()) && matches!(source.as_str(), "nodes" | "relations");
            let frame = egui::Frame::new()
                .fill(if on { ink.raised } else { egui::Color32::TRANSPARENT })
                .corner_radius(crate::style::corner())
                .inner_margin(egui::Margin::symmetric(4, 2));
            let inner = frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                // a proposal: taken or refused (first, always in sight)
                if source == "proposals" {
                    ui.horizontal_wrapped(|ui| {
                        let small = |sign: &str, words: &str, ui: &mut egui::Ui| {
                            ui.add(egui::Button::image_and_text(
                                crate::icons::image(sign, 13.0, ink.text),
                                RichText::new(words).small(),
                            ))
                        };
                        if small("check", t(lang, "ws_take"), ui).clicked() {
                            said = Some(format!("workshop accept {ws} {}", item.i + 1));
                        }
                        if small("x", t(lang, "ws_refuse"), ui).clicked() {
                            said = Some(format!("workshop refuse {ws} {}", item.i + 1));
                        }
                    });
                }
                // what answers a click: its title and its words (never over its buttons, nor its field)
                let mut hit = ui
                    .horizontal(|ui| {
                        crate::icons::show(ui, &item.sign, 14.0, if on { ink.accent } else { ink.muted });
                        let title = if item.code {
                            RichText::new(&item.title).monospace().color(ink.text)
                        } else {
                            RichText::new(&item.title).color(ink.text)
                        };
                        ui.add(egui::Label::new(title.strong()).selectable(false).wrap());
                        if !item.value.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(&item.value).monospace().small().color(ink.muted));
                            });
                        }
                    })
                    .response
                    .rect;
                if source == "words" {
                    let r = ui.add(
                        egui::Label::new(RichText::new(&item.example).monospace().small().color(ink.muted))
                            .selectable(false)
                            .wrap(),
                    );
                    hit = hit.union(r.rect);
                } else if !item.detail.is_empty() {
                    let detail = RichText::new(in_lang(&item.detail, lang)).small().color(ink.muted);
                    let detail = if source == "relations" || source == "nodes" { detail.monospace() } else { detail };
                    hit = hit.union(ui.add(egui::Label::new(detail).selectable(false).wrap()).rect);
                }
                hit
            });
            // the row answers: a click, the right button (its title and its words)
            let r =
                ui.interact(inner.inner.expand(2.0), ui.id().with(("row", &item.key, item.i)), egui::Sense::click());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &item.title));
            let r = if source == "words" { r.on_hover_text(&item.detail) } else { r };
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                if let Some(line) = &w.say {
                    said = Some(with_item(line, item));
                }
                if matches!(source.as_str(), "nodes" | "relations") {
                    app.ws_selected = Some((ws.clone(), item.key.clone()));
                }
            }
            if !menu_items.is_empty() {
                let item = item.clone();
                r.context_menu(|ui| {
                    ui.set_min_width(260.0);
                    ui.label(RichText::new(&item.title).strong().monospace().color(ink.accent));
                    ui.separator();
                    menu(app, ui, &menu_items, &item);
                });
            }
        }
    });
    if let Some(line) = said {
        app.say(line);
    }
}

/// The names, the constants and the operators that can be put in a line, as small buttons.
fn chips(ui: &mut egui::Ui, text: &mut String, names: &[String], ink: &Ink, lang: Lang) -> bool {
    let mut put = false;
    let mut add = |text: &mut String, w: &str| {
        if !text.is_empty() && !text.ends_with([' ', '(']) && !w.starts_with(')') {
            text.push(' ');
        }
        text.push_str(w);
        put = true;
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for n in names {
            if ui.small_button(RichText::new(n).monospace()).on_hover_text(t(lang, "ws_put_name")).clicked() {
                add(text, n);
            }
        }
        ui.label(RichText::new("·").color(ink.faint));
        for (c, v) in grm_engine::lang::CONSTANTS.iter().filter(|(c, _)| *c != "inf" && *c != "ln10") {
            let tip = format!("{c} = {}", grm_engine::lang::g(*v));
            if ui.small_button(RichText::new(*c).monospace().color(ink.accent)).on_hover_text(tip).clicked() {
                add(text, c);
            }
        }
        ui.label(RichText::new("·").color(ink.faint));
        for op in ["=", "+", "-", "*", "/", "^", "(", ")", "ln(", "sqrt("] {
            if ui.small_button(RichText::new(op).monospace()).clicked() {
                add(text, op);
            }
        }
    });
    put
}

/// What is wrong in a line, before it is said (the word, an equation that ends with an operator, a value that is
/// not a number); None: nothing seen. The machine says the rest when the line is written.
pub fn check(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let word = line.split_whitespace().next().unwrap_or("").to_lowercase();
    if !grm_engine::world::WORDS.contains(&word.as_str()) && !matches!(word.as_str(), "ask" | "play") {
        return Some(grm_engine::world::unknown_word(&word));
    }
    if word == "relate" || word == "keep" {
        let Some((_, eq)) = line.split_once(':') else {
            return Some(t(Lang::En, "ws_relate_needs_colon").to_string());
        };
        let eq = eq.split(" when ").next().unwrap_or(eq).split('±').next().unwrap_or(eq);
        if word == "relate" && !eq.contains('=') {
            return Some("an equation has its = (relate r: y = 2 * x)".into());
        }
        let trimmed = eq.trim_end();
        if trimmed.ends_with(['*', '/', '+', '-', '^', '=']) {
            return Some(format!(
                "'{}' ends with {}: a term is missing after it",
                trimmed.trim(),
                &trimmed[trimmed.len() - 1..]
            ));
        }
    }
    if word == "node" || word == "input" || word == "var" || word == "learn" {
        if let Some((_, v)) = line.split_once('=') {
            let v = v
                .split_whitespace()
                .filter(|w| !matches!(*w, "signed" | "fixed" | "positive"))
                .collect::<Vec<_>>()
                .join(" ");
            if let Err(e) = grm_engine::lang::number(&v) {
                return Some(e.to_string());
            }
        }
    }
    None
}

/// Where a line of the language is written for a workshop.
pub fn editor(app: &mut App, ui: &mut egui::Ui, w: &Widget, rect: Rect) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let ws = w.nodes.first().cloned().unwrap_or_default();
    let pic = picture(app, &ws);
    let names: Vec<String> = pic
        .as_ref()
        .map(|p| p.nodes.iter().filter(|n| !n.name.contains("__")).map(|n| n.name.clone()).collect())
        .unwrap_or_default();
    let all_paths = paths(app);
    let mut bench = app.benches.remove(&ws).unwrap_or_default();
    let mut to_say: Vec<String> = vec![];
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    egui::ScrollArea::vertical().id_salt(("editor", &ws)).auto_shrink([false, false]).show(ui, |ui| {
        let card = egui::Frame::new()
            .fill(ink.raised)
            .corner_radius(crate::style::corner())
            .inner_margin(egui::Margin::symmetric(10, 6));
        // a path: its step said, its line put in the editor
        if let Some((p, k)) = bench.path {
            if let Some(path) = all_paths.get(p) {
                let k = k.min(path.steps.len().saturating_sub(1));
                let (en, fr, line) = path.steps.get(k).cloned().unwrap_or_default();
                card.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        crate::icons::show(ui, &path.icon, 15.0, ink.accent);
                        ui.label(RichText::new(said_in(lang, &path.en, &path.fr)).strong());
                        ui.label(RichText::new(format!("{} / {}", k + 1, path.steps.len())).small().color(ink.muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                                bench.path = None;
                            }
                        });
                    });
                    ui.label(RichText::new(said_in(lang, &en, &fr)));
                    ui.horizontal(|ui| {
                        if k > 0 && crate::icons::button(ui, "arrow-left", t(lang, "ws_prev")).clicked() {
                            bench.path = Some((p, k - 1));
                        }
                        let last = k + 1 >= path.steps.len();
                        if line.is_empty() {
                            if crate::icons::button(
                                ui,
                                "arrow-right",
                                t(lang, if last { "ws_done" } else { "ws_next" }),
                            )
                            .clicked()
                            {
                                bench.path = if last { None } else { Some((p, k + 1)) };
                            }
                        } else if crate::icons::button(ui, "pen-line", t(lang, "ws_put_step")).clicked() {
                            bench.text = line.clone();
                            bench.replacing = None;
                            bench.focus = true;
                        }
                    });
                    // the step done when its line is written
                    if !line.is_empty() {
                        let done = pic.as_ref().is_some_and(|p| p.written.iter().any(|l| l.trim() == line.trim()))
                            || (line.starts_with("ask ") || line.starts_with("play "))
                                && bench.sent.as_deref().is_some_and(|s| s.ends_with(&line));
                        if done
                            && crate::icons::button(
                                ui,
                                "arrow-right",
                                t(lang, if last_step(&path.steps, k) { "ws_done" } else { "ws_next" }),
                            )
                            .clicked()
                        {
                            bench.path = if last_step(&path.steps, k) { None } else { Some((p, k + 1)) };
                        }
                    }
                });
                ui.add_space(4.0);
            }
        }
        // the answer to a question (or what was proposed)
        if let Some(a) = bench.answer.clone() {
            card.show(ui, |ui| {
                ui.set_width(ui.available_width());
                let a = in_lang(&a, lang);
                ui.horizontal(|ui| {
                    crate::icons::show(ui, "circle-help", 15.0, ink.accent);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                            bench.answer = None;
                        }
                    });
                });
                ui.add(egui::Label::new(RichText::new(a.lines().next().unwrap_or("")).strong()).wrap());
                let rest: Vec<String> = a.lines().skip(1).map(String::from).collect();
                if !rest.is_empty() {
                    crate::style::terminal(ui, &rest);
                }
            });
            ui.add_space(4.0);
        }
        // a node's value, changed here (a number, a constant, a little arithmetic): asked by « its value »
        if let Some((name, mut text)) = bench.value_of.clone() {
            card.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t(lang, "ws_its_value")).small().color(ink.muted));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{name} =")).monospace().strong());
                    let room = (ui.available_width() - 150.0).max(80.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .id_salt(("ws-value", &ws))
                            .font(egui::TextStyle::Monospace)
                            .desired_width(room)
                            .hint_text(crate::style::hint("2*pi")),
                    );
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "ws-value"));
                    if bench.focus {
                        r.request_focus();
                        bench.focus = false;
                    }
                    let fine = grm_engine::lang::number(&pointed(text.trim())).is_ok();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let b = egui::Button::image_and_text(
                        crate::icons::image("check", 15.0, ink.on_chosen()),
                        RichText::new(t(lang, "ws_write")).color(ink.on_chosen()),
                    )
                    .fill(ink.chosen);
                    if (ui.add_enabled(fine, b).clicked() || (enter && fine)) && !text.trim().is_empty() {
                        to_say.push(format!("workshop value {ws} {name} {}", pointed(text.trim())));
                    }
                    if crate::icons::sign(ui, "x", t(lang, "cancel")).clicked() {
                        bench.value_of = None;
                    } else if let Some(v) = bench.value_of.as_mut() {
                        v.1 = text.clone();
                    }
                });
                if !text.trim().is_empty() {
                    if let Err(e) = grm_engine::lang::number(&pointed(text.trim())) {
                        ui.horizontal_top(|ui| {
                            crate::icons::show(ui, "triangle-alert", 14.0, ink.amber);
                            ui.add(egui::Label::new(RichText::new(in_lang(&e.to_string(), lang)).small()).wrap());
                        });
                    }
                }
            });
            ui.add_space(4.0);
        }
        // the line, coloured as it is written
        let replacing = bench.replacing;
        ui.horizontal(|ui| {
            let what = match replacing {
                Some(k) => format!("{} {}", t(lang, "ws_changing_line"), k + 1),
                None => t(lang, "ws_write_line").to_string(),
            };
            ui.label(RichText::new(what).small().color(ink.muted));
        });
        let ink2 = ink;
        let mut layouter = move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = crate::code::highlight_line(&ink2, text.as_str());
            job.wrap.max_width = wrap;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let seen = check(&bench.text);
        let ready = !bench.text.trim().is_empty() && seen.is_none();
        // the line and what writes it, side by side: always in sight
        ui.horizontal(|ui| {
            let room = (ui.available_width() - 140.0).max(120.0);
            let r = ui.add(
                egui::TextEdit::singleline(&mut bench.text)
                    .id_salt(("ws-line", &ws))
                    .font(egui::TextStyle::Monospace)
                    .desired_width(room)
                    .margin(crate::style::field())
                    .hint_text(crate::style::hint("node mass = 70"))
                    .layouter(&mut layouter),
            );
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "ws-line"));
            if bench.focus {
                r.request_focus();
                bench.focus = false;
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // a sent line changed: what was wrong with it is no longer said
            if r.changed() {
                bench.error = None;
            }
            let key = if replacing.is_some() { "ws_change" } else { "ws_write" };
            let b = egui::Button::image_and_text(
                crate::icons::image("pen-line", 15.0, ink.on_chosen()),
                RichText::new(t(lang, key)).color(ink.on_chosen()),
            )
            .fill(ink.chosen);
            if ui.add_enabled(ready, b).clicked() || (enter && ready) {
                let line = pointed(bench.text.trim());
                let typed = match (replacing, line.split_once(' ')) {
                    (_, Some(("ask", n))) => format!("workshop ask {ws} {}", n.trim()),
                    (_, Some(("play", n))) => format!("workshop play {ws} {}", n.trim()),
                    (Some(k), _) => format!("workshop edit {ws} {k} {line}"),
                    (None, _) => format!("workshop say {ws} {line}"),
                };
                bench.sent = Some(typed.clone());
                if line.starts_with("ask ") || line.starts_with("play ") {
                    bench.text.clear();
                }
                to_say.push(typed);
            }
            if (replacing.is_some() || !bench.text.is_empty())
                && crate::icons::sign(ui, "x", t(lang, "cancel")).clicked()
            {
                bench.text.clear();
                bench.replacing = None;
                bench.error = None;
            }
        });
        // in words, as it is written
        let words = crate::explain::explain(&bench.text, lang);
        if !words.is_empty() && seen.is_none() {
            ui.add(egui::Label::new(RichText::new(words).italics().small().color(ink.muted)).wrap());
        }
        // what is wrong: what is seen now, or what the machine said
        if let Some(e) = seen.clone().or(bench.error.clone()) {
            ui.horizontal_top(|ui| {
                crate::icons::show(ui, "triangle-alert", 14.0, ink.amber);
                ui.add(egui::Label::new(RichText::new(in_lang(&e, lang)).small().color(ink.text)).wrap());
            });
        }
        chips(ui, &mut bench.text, &names, &ink, lang);
    });
    app.benches.insert(ws.clone(), bench);
    for l in to_say {
        app.say(l);
    }
}

/// A line with its decimal commas written as the language writes them (`9,81` → `9.81`; `f(a, b)` stays).
pub fn pointed(line: &str) -> String {
    let c: Vec<char> = line.chars().collect();
    c.iter()
        .enumerate()
        .map(|(k, &x)| {
            let digit = |j: Option<&char>| j.is_some_and(|d| d.is_ascii_digit());
            if x == ',' && k > 0 && digit(c.get(k - 1)) && digit(c.get(k + 1)) {
                '.'
            } else {
                x
            }
        })
        .collect()
}

/// A value as a list shows it: the number, and how sure (a `?` when nothing holds it yet).
fn short_reading(v: f64, s: f64, signed: bool) -> String {
    let g = grm_engine::lang::fmt_g(v, 4);
    if signed {
        if s > v.abs().max(1.0) * 10.0 {
            format!("{g} ?")
        } else if s < 1e-9 {
            g
        } else {
            format!("{g} ± {}", grm_engine::lang::fmt_g(s, 2))
        }
    } else if s > 2.0 {
        format!("{g} ?")
    } else if s < 5e-4 {
        g
    } else {
        format!("{g} ± {}%", grm_engine::lang::fmt_g((10f64.powf(s) - 1.0) * 100.0, 2))
    }
}

/// What the machine said, in the window's language.
fn in_lang(text: &str, lang: Lang) -> String {
    if lang == Lang::Fr {
        crate::french::fr(text)
    } else {
        text.to_string()
    }
}

fn last_step(steps: &[(String, String, String)], k: usize) -> bool {
    k + 1 >= steps.len()
}

/// A workshop's network as it grows, in its place on a screen: its nodes chosen by a click, the screen's menus
/// by the right button.
pub fn network(app: &mut App, ui: &mut egui::Ui, view: &AppView, w: &Widget, rect: Rect, small: bool) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let ws = w.nodes.first().cloned().unwrap_or_default();
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    match picture(app, &ws) {
        Some(p) if !p.nodes.is_empty() => {
            let master = (p.master.clone(), None);
            let saved_master = std::mem::replace(&mut app.master, master);
            let mut v = app.ws_views.remove(&ws).unwrap_or_default();
            std::mem::swap(&mut app.view, &mut v);
            app.view.passive = small;
            app.write_into = Some(ws.clone());
            app.ws_menus = Some((view.name.clone(), w.menu.clone().unwrap_or_default()));
            app.showing(p, |app| crate::view::show(app, ui, None));
            app.ws_menus = None;
            app.write_into = None;
            app.view.passive = false;
            std::mem::swap(&mut app.view, &mut v);
            app.ws_views.insert(ws, v);
            app.master = saved_master;
        }
        _ => {
            ui.add_space(rect.height() * 0.3);
            ui.vertical_centered(|ui| {
                crate::icons::show(ui, "hammer", 28.0, ink.faint);
                ui.label(RichText::new(t(lang, "ws_empty")).color(ink.muted));
            });
        }
    }
}

/// The menus a workshop's network offers (right button), from its screen: on a node, or on nothing.
pub fn network_menu(app: &mut App, ui: &mut egui::Ui, node: Option<&str>) -> bool {
    let Some((key, menu_name)) = app.ws_menus.clone() else { return false };
    let Some(view) = app.screens.get(&key).map(|s| s.1.clone()) else { return false };
    let Some(ws) = app.write_into.clone() else { return false };
    match node {
        Some(n) => {
            let Some(items_of) = view.menus.get(&menu_name).cloned() else { return false };
            let all = items(app, "nodes", &ws);
            let item =
                all.into_iter().find(|i| i.key == n).unwrap_or(Item { key: n.to_string(), ..Default::default() });
            menu(app, ui, &items_of, &item);
        }
        None => {
            let Some(items_of) = view.menus.get("background_menu").cloned() else { return false };
            menu(app, ui, &items_of, &Item::default());
        }
    }
    true
}

/// A workshop's way into the main network, in its place on a screen.
pub fn airlock(app: &mut App, ui: &mut egui::Ui, w: &Widget, rect: Rect) {
    let ws = w.nodes.first().cloned().unwrap_or_default();
    let ready = picture(app, &ws).is_some_and(|p| !p.written.is_empty());
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    crate::airlock::door(app, &mut child, &ws, ready, true);
}

/// A workshop, as the space or the middle shows it: its screen (written in GRML), or, small, its network only.
pub fn workshop(app: &mut App, ui: &mut egui::Ui, ws: &str, view: &mut crate::view::View, small: bool) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    if !app.picture.workshops.iter().any(|w| w.0 == ws) {
        ui.label(RichText::new(t(lang, "no_workshop")).color(ink.muted));
        if crate::icons::button(ui, "hammer", t(lang, "ws_reopen")).clicked() {
            app.say(format!("workshop new {ws}"));
        }
        return;
    }
    // the zone's own way of looking at it
    std::mem::swap(view, app.ws_views.entry(ws.to_string()).or_default());
    if small {
        let rect = ui.available_rect_before_wrap();
        let w = Widget { nodes: vec![ws.to_string()], ..small_network() };
        let v = AppView { name: format!("small:{ws}"), ..empty_view() };
        network(app, ui, &v, &w, rect, true);
        ui.allocate_rect(rect, egui::Sense::hover());
    } else {
        app.ws_front = Some(ws.to_string());
        match open(app, "workshop.grm", ws) {
            Some(key) => {
                // tall enough: the screen fits its room, nothing scrolls; too low, it scrolls
                let rows = app
                    .screens
                    .get(&key)
                    .map(|s| s.1.widgets.iter().map(|w| w.at.1 + w.size.1).max().unwrap_or(1))
                    .unwrap_or(1);
                if ui.available_height() >= rows as f32 * crate::surface::ROW * crate::surface::FIT_LEAST + 8.0 {
                    app.surface.fit = true;
                    crate::surface::show(app, ui, &key);
                } else {
                    let out = egui::ScrollArea::vertical()
                        .id_salt(("ws-screen", ws))
                        .show(ui, |ui| crate::surface::show(app, ui, &key));
                    crate::space::wheel_to(ui, &out);
                }
            }
            None => {
                ui.label(RichText::new(format!("{}: screens/workshop.grm", t(lang, "ws_no_screen"))).color(ink.amber));
            }
        }
    }
    std::mem::swap(view, app.ws_views.entry(ws.to_string()).or_default());
}

/// The inspector's part for the workshop in front (its nodes and relations at once), written in GRML.
pub fn inspector(app: &mut App, ui: &mut egui::Ui) {
    let Some(ws) = app.ws_front.clone() else { return };
    if !app.picture.workshops.iter().any(|w| w.0 == ws) {
        app.ws_front = None;
        return;
    }
    // closed by its cross: it stays closed until the workshop is asked for again (opened, a node changed)
    if app.ws_inspector_closed.contains(&ws) {
        return;
    }
    if let Some(key) = open(app, "workshop_inspector.grm", &ws) {
        let ink = Ink::of(ui.ctx());
        // never wider than the room it is given (a panel would widen itself frame after frame)
        let room = (ui.available_width() - 4.0).max(100.0);
        egui::Frame::new().stroke(egui::Stroke::new(1.0_f32, ink.faint)).corner_radius(crate::style::corner()).show(
            ui,
            |ui| {
                ui.set_max_width(room);
                ui.set_width(room);
                ui.horizontal(|ui| {
                    crate::icons::show(ui, "hammer", 14.0, ink.accent);
                    ui.label(RichText::new(format!("{} {ws}", t(app.settings.lang, "workshop"))).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::icons::sign(ui, "x", t(app.settings.lang, "close")).clicked() {
                            app.ws_inspector_closed.insert(ws.clone());
                        }
                    });
                });
                ui.allocate_ui(vec2(room, 8.0 * crate::surface::ROW), |ui| {
                    ui.set_max_width(room);
                    crate::surface::show(app, ui, &key);
                });
            },
        );
        ui.add_space(6.0);
    }
}

fn small_network() -> Widget {
    grm_shell::apps::parse_widget("net: network ws", 0).expect("a network")
}

fn empty_view() -> AppView {
    AppView {
        name: String::new(),
        title: String::new(),
        icon: None,
        path: PathBuf::new(),
        text: String::new(),
        errors: vec![],
        widgets: vec![],
        lists: Default::default(),
        prefix: Default::default(),
        apis: vec![],
        can_undo: false,
        can_redo: false,
        looks: vec![],
        hands: vec![],
        menus: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_says_what_is_wrong_before_it_is_written() {
        assert!(check("node mass = 70").is_none());
        assert!(check("node m = 2*pi").is_none());
        assert!(check("nod x = 1").unwrap().contains("node"));
        assert!(check("relate r: f = m *").unwrap().contains("ends with *"));
        assert!(check("node m = 2*pie").unwrap().contains("pie"));
        assert!(check("ask force").is_none());
        assert!(check("node g = 9,81").is_none());
        assert_eq!(pointed("node g = 9,81"), "node g = 9.81");
        assert_eq!(pointed("carry a, b = c, d"), "carry a, b = c, d");
    }

    #[test]
    fn the_words_and_the_paths_are_read_from_their_grml_files() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../screens");
        let text = std::fs::read_to_string(format!("{root}/words.grm")).unwrap();
        let tk = tokens(r#"word node family there level 1 example "node mass = 70" "a quantity" fr "une quantité""#);
        assert_eq!(tk[0], "word");
        assert!(tk.contains(&"node mass = 70".to_string()));
        // every example of every word is a line the machine takes (or a question, or a sound)
        let mut w = grm_engine::world::World::workshop();
        let o = grm_engine::Provenance::of("test");
        for line in text.lines().filter(|l| l.starts_with("word ")) {
            let tk = tokens(line);
            let (map, _) = keyed(&tk[1..], &["family", "level", "icon", "example", "fr"]);
            let ex = map.get("example").cloned().unwrap_or_default();
            assert!(!ex.is_empty(), "{line}");
            assert!(check(&ex).is_none(), "{ex}: {:?}", check(&ex));
            let word = ex.split_whitespace().next().unwrap_or("");
            if matches!(word, "ask" | "play" | "network" | "call" | "fuse" | "delay" | "carry" | "rail" | "repeat") {
                continue;
            }
            let fresh = |w: &mut grm_engine::world::World, n: &str| {
                if !w.machine.net.nodes.contains_key(n) {
                    let _ = w.say(&format!("node {n} = 2 signed"), &o);
                }
            };
            for n in names_in(&ex).into_iter().skip(1) {
                if grm_engine::lang::constant(n).is_none() && n.parse::<f64>().is_err() && grm_engine::lang::is_name(n)
                {
                    fresh(&mut w, n);
                }
            }
            assert!(w.say(&ex, &o).is_ok(), "{ex}: {:?}", w.say(&ex, &o));
        }
        let paths = std::fs::read_to_string(format!("{root}/paths.grm")).unwrap();
        assert_eq!(paths.lines().filter(|l| l.starts_with("path ")).count(), 4);
        assert_eq!(paths.lines().filter(|l| l.starts_with("step ")).count(), 25);
    }
}
