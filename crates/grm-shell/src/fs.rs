// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine works in a folder, as a Unix shell does: `pwd`, `cd`, `ls`, `head`, `tail`, `cat`, `mkdir`, `cp`,
//! `mv`, `rm` (asks first), `touch`, `new`; data loaded from any table (`load`, its columns chosen); and the words of
//! the Python machine: `ask`, `put`, `outputs`, `inspect`, `master`, `check`, `why`, `focus`, `programs`, `run`,
//! `history`, `!N`, `!!`, `clear`, `play`/`pause` (the clock), `help WORD`, `tour`, `lang`.

use std::path::{Path, PathBuf};

use grm_engine::engine::Engine;
use grm_engine::lang::g;
use indexmap::IndexMap;

use crate::{Line, Order, Shell};

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| ".".into())
}

/// `~`, `$VARIABLES` and Windows' separators understood.
pub fn expand(p: &str) -> PathBuf {
    let mut s = p.trim().to_string();
    if s == "~" || s.starts_with("~/") || s.starts_with("~\\") {
        s = format!("{}{}", home().display(), &s[1..]);
    }
    while let Some(i) = s.find('$') {
        let rest = s[i + 1..].to_string();
        let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let var = rest[..end].to_string();
        let value = std::env::var(&var).unwrap_or_default();
        s = format!("{}{}{}", &s[..i], value, &rest[end..]);
        if var.is_empty() {
            break;
        }
    }
    PathBuf::from(s)
}

/// Whether a name fits a pattern with `*` and `?`.
pub fn fits(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    go(&pattern.chars().collect::<Vec<_>>(), &name.chars().collect::<Vec<_>>())
}

fn size_text(n: u64) -> String {
    if n < 1024 {
        format!("{n}")
    } else if n < 1024 * 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{:.1}M", n as f64 / 1024.0 / 1024.0)
    }
}

fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

pub const WORDS_HELP: &[(&str, &str, &str)] = &[
    ("node", "node NAME [= value] [fixed] [signed]", "node mass = 2"),
    ("relate", "relate NAME: left = right [± sigma] [when NODE in LO .. HI]", "relate run: speed * time = distance"),
    ("observe", "observe NAME = VALUE [± SIGMA]", "observe distance = 100 ± 1%"),
    ("forget", "forget NAME: its measurement goes", "forget distance"),
    ("var", "var NAME [= value] [positive]: kept until a set writes it", "var n = 0"),
    ("set", "set NAME = EXPRESSION [when ...]: written at each tick", "set n = n + 1 when n in -inf .. 9.5"),
    ("keep", "keep NAME: NODE >= BOUND [when ...]: a contact", "keep floor: y >= 0"),
    ("input", "input NAME [= value] [positive]: measured from outside", "input x = 3"),
    ("output", "output NAME [positive]: read from outside", "output speed"),
    ("learn", "learn NAME [= value]: a weight, what it learns stays", "learn k = 0"),
    ("network", "network NAME(p, q) ... end: a network to call, {p} replaced", "network circle(id, r)"),
    ("call", "call NAME(p=.., q=..)", "call circle(id=small, r=1)"),
    ("use", "use FILE.grm: all its statements", "use programs/fall.grm"),
    ("fuse", "fuse A B [inverse]: B was A under another name", "fuse speed velocity"),
    ("ask", "ask NODE: its value", "ask speed"),
    ("put", "put NAME = VALUE [± SIGMA]: an input measured", "put x = 10"),
    ("load", "load FILE.csv [COLUMN...]: a row a tick; the columns chosen (asked when not said)", "load data.csv"),
    ("why", "why NODE [STEPS]: what makes its value (and, in time, its forecast)", "why speed"),
    ("master", "master [SOURCE]: the master equation", "master"),
    ("check", "check: what does not hold now", "check"),
    ("inspect", "inspect NAME: what a node or a relation holds", "inspect run"),
    ("outputs", "outputs: the outputs and the harmony", "outputs"),
    ("focus", "focus [NAME|all]: a network brought to the front", "focus typed"),
    ("tick", "tick [N]: N beats of the clock", "tick 10"),
    ("play", "play [HZ]: the clock ticks by itself; play NODE: a node heard", "play 4"),
    ("pause", "pause: the clock stops", "pause"),
    ("ls", "ls [-l] [-a] [PATH...]: what is there (patterns: *.csv)", "ls -l"),
    ("cd", "cd [FOLDER | .. | ~ | -]", "cd ~/data"),
    ("new", "new NAME.ext: a new file, opened to write; new NAME: a program", "new prices.csv"),
    ("rm", "rm [-r] PATH...: deleted, once asked", "rm old.csv"),
    ("app", "app new|open|close|undo|redo NAME", "app open oscillator"),
    ("api", "api NAME: get|post|websocket URL ...", "api w: get https://... every 600"),
    (
        "widget",
        "widget NAME: KIND NODE.. [LO .. HI] [at C R] [size W H] [\"words\"] [look L]",
        "widget v: slider volume 0 .. 1",
    ),
    ("undo", "undo / redo: the world as it was before the last change (30 kept)", "undo"),
    ("stats", "stats: the network counted, by source; what the human says most", "stats"),
    ("rent", "rent: what each relation brings, in bits a tick, against the rent of its links (the price grows as room runs out)", "rent"),
    ("workshop", "workshop [list | new NAME | show NAME | say NAME LINE | unsay NAME K | play NAME NODE | merge NAME | close NAME]: networks made apart (load FILE.csv [--into NAME] makes one; merge: through the airlock)", "workshop say fall relate r: h = 4.9 * t^2"),
    ("capacity", "capacity [N | auto]: the links the network may hold (auto: a quarter of the memory available now)", "capacity 20000"),
    ("look", "look NAME: accent = #E8A33D, round = 8, size = 1.2 (a look for widgets)", "look big: size = 1.5"),
];

impl Shell {
    /// The words of this file; None: not one of them.
    pub(crate) fn more(&mut self, engine: &Engine, word: &str, rest: &str) -> Option<Vec<Line>> {
        let words = crate::path_words(rest);
        let args: Vec<&str> = words.iter().map(String::as_str).collect();
        let flags: Vec<&str> = args.iter().copied().filter(|a| a.starts_with('-') && a.len() > 1).collect();
        let paths: Vec<&str> = args
            .iter()
            .copied()
            .filter(|a| !(a.starts_with('-') && a.len() > 1) || *a == "--mix" || *a == "--here" || *a == "--into")
            .collect();
        let err = |e: std::io::Error, p: &dyn std::fmt::Display| vec![Line::error(format!("! {p}: {e}"))];
        Some(match word {
            "pwd" => vec![Line::said(std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default())],
            "cd" => {
                let to = match rest.trim() {
                    "" => home(),
                    "-" => match self.before_cd.clone() {
                        Some(p) => p,
                        None => return Some(vec![Line::warn("? no folder before")]),
                    },
                    p => expand(p),
                };
                let here = std::env::current_dir().ok();
                match std::env::set_current_dir(&to) {
                    Ok(()) => {
                        self.before_cd = here;
                        self.orders.push(Order::Folder);
                        vec![Line::dim(std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default())]
                    }
                    Err(e) => err(e, &to.display()),
                }
            }
            "ls" => {
                let long = flags.iter().any(|f| f.contains('l'));
                let all = flags.iter().any(|f| f.contains('a'));
                let targets: Vec<String> =
                    if paths.is_empty() { vec![".".into()] } else { paths.iter().map(|s| s.to_string()).collect() };
                let mut out = vec![];
                for t in targets {
                    let p = expand(&t);
                    let (dir, pattern) = if t.contains('*') || t.contains('?') {
                        let parent = p
                            .parent()
                            .filter(|x| !x.as_os_str().is_empty())
                            .map(|x| x.to_path_buf())
                            .unwrap_or_else(|| ".".into());
                        (parent, p.file_name().map(|f| f.to_string_lossy().to_string()))
                    } else {
                        (p.clone(), None)
                    };
                    if dir.is_file() {
                        out.push(Line::said(t.clone()));
                        continue;
                    }
                    let Ok(entries) = std::fs::read_dir(&dir) else {
                        out.push(Line::error(format!("! {t}: no such folder")));
                        continue;
                    };
                    let mut list: Vec<(String, std::fs::Metadata)> = entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.metadata().ok().map(|m| (e.file_name().to_string_lossy().to_string(), m)))
                        .filter(|(n, _)| all || !n.starts_with('.'))
                        .filter(|(n, _)| pattern.as_ref().is_none_or(|pt| fits(pt, n)))
                        .collect();
                    list.sort_by_key(|a| (!a.1.is_dir(), a.0.to_lowercase()));
                    for (n, m) in list {
                        let shown = if m.is_dir() { format!("{n}/") } else { n.clone() };
                        let text = if long {
                            let when = m
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| {
                                    let days = d.as_secs() / 86400;
                                    format!(
                                        "{} days ago",
                                        (std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .map(|n| n.as_secs() / 86400)
                                            .unwrap_or(days))
                                        .saturating_sub(days)
                                    )
                                })
                                .unwrap_or_default();
                            format!(
                                "{} {:>8}  {:<14} {shown}",
                                if m.permissions().readonly() { "r-" } else { "rw" },
                                size_text(m.len()),
                                when
                            )
                        } else {
                            shown
                        };
                        // folders, programs and data each in their colour
                        out.push(if m.is_dir() {
                            Line::said(text)
                        } else if n.ends_with(".grm") {
                            Line { kind: crate::Kind::Said, text: format!("\u{2}{text}") }
                        } else if n.ends_with(".csv") || n.ends_with(".json") {
                            Line { kind: crate::Kind::Said, text: format!("\u{3}{text}") }
                        } else {
                            Line::dim(text)
                        });
                    }
                }
                out
            }
            "head" | "tail" | "cat" => {
                let mut n = if word == "cat" { 2000 } else { 10 };
                let mut file = None;
                let mut k = 0;
                while k < args.len() {
                    let a = args[k];
                    if a == "-n" {
                        n = args.get(k + 1).and_then(|x| x.parse().ok()).unwrap_or(n);
                        k += 2;
                        continue;
                    }
                    if let Some(x) = a.strip_prefix('-').and_then(|x| x.parse().ok()) {
                        n = x;
                    } else {
                        file = Some(a);
                    }
                    k += 1;
                }
                let Some(file) = file else { return Some(vec![Line::warn(format!("? {word} [-n N] FILE"))]) };
                match std::fs::read(expand(file)) {
                    Ok(b) => {
                        let text = grm_connect::csv::text_of(&b);
                        let lines: Vec<&str> = text.lines().collect();
                        let pick: Vec<&str> = if word == "tail" {
                            lines[lines.len().saturating_sub(n)..].to_vec()
                        } else {
                            lines.iter().take(n).copied().collect()
                        };
                        pick.into_iter().map(Line::said).collect()
                    }
                    Err(e) => err(e, &file),
                }
            }
            "mkdir" => {
                let deep = flags.contains(&"-p");
                let mut out = vec![];
                for p in &paths {
                    let r = if deep { std::fs::create_dir_all(expand(p)) } else { std::fs::create_dir(expand(p)) };
                    out.extend(match r {
                        Ok(()) => vec![Line::dim(format!("{p}/"))],
                        Err(e) => err(e, p),
                    });
                }
                out
            }
            "touch" => paths
                .iter()
                .flat_map(|p| {
                    let path = expand(p);
                    let r = if path.exists() { Ok(()) } else { std::fs::write(&path, "") };
                    match r {
                        Ok(()) => vec![Line::dim(p.to_string())],
                        Err(e) => err(e, p),
                    }
                })
                .collect(),
            "cp" | "mv" => {
                if paths.len() < 2 {
                    return Some(vec![Line::warn(format!("? {word} FROM... TO"))]);
                }
                let to = expand(paths[paths.len() - 1]);
                let mut out = vec![];
                for from in &paths[..paths.len() - 1] {
                    let src = expand(from);
                    let dest = if to.is_dir() { to.join(src.file_name().unwrap_or_default()) } else { to.clone() };
                    let r = if word == "mv" {
                        std::fs::rename(&src, &dest)
                    } else if src.is_dir() && !flags.contains(&"-r") {
                        Err(std::io::Error::other("a folder: cp -r"))
                    } else {
                        copy_all(&src, &dest)
                    };
                    out.extend(match r {
                        Ok(()) => vec![Line::dim(format!("{from} → {}", dest.display()))],
                        Err(e) => err(e, from),
                    });
                }
                out
            }
            "rm" => {
                if paths.is_empty() {
                    return Some(vec![Line::warn("? rm [-r] PATH...")]);
                }
                let deep = flags.iter().any(|f| f.contains('r'));
                if let Some(d) = paths.iter().find(|p| expand(p).is_dir()) {
                    if !deep {
                        return Some(vec![Line::warn(format!("? {d} is a folder: rm -r"))]);
                    }
                }
                self.asking = Some(("rm".into(), paths.join("\n")));
                vec![Line::ask(format!("! delete {}? (y/n)", paths.join(", ")))]
            }
            "new" if !rest.trim().is_empty() => {
                let name = rest.trim();
                let path = if name.contains('.') { expand(name) } else { expand(&format!("{name}.grm")) };
                if !path.exists() {
                    let text = if name.contains('.') {
                        String::new()
                    } else {
                        format!(
                            "#! name: {name}\n#! does: (what it does)\n# its nodes and relations\nnode {name}_a = 1\n"
                        )
                    };
                    if let Err(e) = std::fs::write(&path, text) {
                        return Some(err(e, &path.display()));
                    }
                }
                self.orders.push(Order::OpenFile(path.clone()));
                vec![Line::said(format!("{} opened to write", path.display()))]
            }
            "edit" | "open" if !rest.trim().is_empty() => {
                let path = expand(rest.trim());
                if !path.exists() {
                    return Some(vec![Line::warn(format!("? no file {} (new {} makes it)", rest.trim(), rest.trim()))]);
                }
                self.orders.push(Order::OpenFile(path));
                vec![]
            }
            "programs" | "progs" => {
                let mut out = vec![];
                for dir in [PathBuf::from("."), self.apps_dir()] {
                    if let Ok(d) = std::fs::read_dir(&dir) {
                        let mut files: Vec<PathBuf> = d
                            .filter_map(|e| e.ok())
                            .map(|e| e.path())
                            .filter(|p| p.extension().is_some_and(|x| x == "grm"))
                            .collect();
                        files.sort();
                        for f in files {
                            let text = std::fs::read_to_string(&f).unwrap_or_default();
                            let does = text
                                .lines()
                                .find_map(|l| l.strip_prefix("#! does:").or_else(|| l.strip_prefix("#! about:")))
                                .or_else(|| text.lines().find_map(|l| l.strip_prefix("app ")))
                                .or_else(|| text.lines().find_map(|l| l.strip_prefix('#')))
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            out.push(Line { kind: crate::Kind::Said, text: format!("\u{2}{}  {does}", f.display()) });
                        }
                    }
                }
                if out.is_empty() {
                    out.push(Line::dim("no program here (new NAME makes one)"));
                }
                out
            }
            "run" if !rest.trim().is_empty() => {
                let name = rest.trim();
                let path = if name.ends_with(".grm") { expand(name) } else { expand(&format!("{name}.grm")) };
                let file = path.display().to_string();
                crate::said(engine.say(&format!("use {file}"), grm_engine::Provenance::of(format!("file:{file}"))))
            }
            "history" => {
                let n: usize = rest.trim().parse().unwrap_or(20);
                let start = self.history.len().saturating_sub(n);
                self.history[start..]
                    .iter()
                    .enumerate()
                    .map(|(k, h)| Line::dim(format!("{:>4}  {h}", start + k + 1)))
                    .collect()
            }
            "clear" => {
                self.orders.push(Order::Clear);
                vec![]
            }
            "tour" => {
                self.orders.push(Order::Tour);
                vec![]
            }
            "lang" => {
                self.orders.push(Order::Lang(rest.trim().to_string()));
                vec![]
            }
            "focus" | "fc" => {
                let f = rest.trim();
                self.orders.push(Order::Focus(if f.is_empty() || f == "all" { None } else { Some(f.to_string()) }));
                crate::said(engine.ask(|w| {
                    Ok(w.sources().iter().map(|(s, n)| format!("{s}: {n} relation(s)")).collect::<Vec<_>>().join("\n"))
                }))
            }
            "ask" => {
                let name = rest.trim().to_string();
                crate::said(engine.ask(move |w| {
                    if !w.machine.net.nodes.contains_key(&name) {
                        grm_engine::bail!("no node {name}");
                    }
                    let (v, s) = w.machine.reading(&name);
                    let signed = w.machine.net.nodes[&name].signed;
                    Ok(format!("{name} = {}  (± {}{})", g(v), g(s), if signed { "" } else { " dex" }))
                }))
            }
            "put" => {
                let line = format!("observe {}", rest.trim());
                crate::said(engine.say(&line, grm_engine::Provenance::of("typed")))
            }
            "outputs" | "outs" => crate::said(engine.ask(|w| {
                let net = &w.machine.net;
                let mut out: Vec<String> = net
                    .nodes
                    .iter()
                    .filter(|(_, s)| s.output)
                    .map(|(n, _)| {
                        let (v, s) = w.machine.reading(n);
                        format!("{n} = {} ± {}", g(v), g(s))
                    })
                    .collect();
                out.push(format!("harmony {:.3}", w.machine.harmony()));
                Ok(out.join("\n"))
            })),
            "inspect" | "ins" => {
                let name = rest.trim().to_string();
                crate::said(engine.ask(move |w| {
                    let net = &w.machine.net;
                    if let Some(s) = net.nodes.get(&name) {
                        let (v, sg) = w.machine.reading(&name);
                        let rels: Vec<String> = net
                            .relations
                            .iter()
                            .filter(|r| r.nodes().contains(&name))
                            .map(|r| format!("  {}: {}", r.name, r.text))
                            .collect();
                        return Ok(format!(
                            "node {name} = {} ± {}\n  {}{}{}{}{}, from {}\n{}",
                            g(v),
                            g(sg),
                            if s.signed { "signed" } else { "positive" },
                            if s.fixed { ", fixed" } else { "" },
                            if s.var { ", a variable" } else { "" },
                            if s.input { ", an input" } else { "" },
                            if s.output { ", an output" } else { "" },
                            s.origin.source,
                            rels.join("\n")
                        ));
                    }
                    if let Some(r) = net.relation(&name) {
                        return Ok(format!(
                            "relation {}: {}\n  from {}, holding at {:.0} %, nodes {}",
                            r.name,
                            r.text,
                            r.origin.source,
                            r.strength * 100.0,
                            r.nodes().join(", ")
                        ));
                    }
                    grm_engine::bail!("nothing called {name}")
                }))
            }
            "master" | "ms" => {
                let src = rest.trim().to_string();
                crate::said(engine.ask(move |w| {
                    Ok(match w.master(if src.is_empty() { None } else { Some(&src) }) {
                        Some(m) => format!(
                            "master equation: {}\n  {} relations, {:.2} decades, residual {:.1e} dex",
                            m.text(),
                            m.relations,
                            m.decades,
                            m.residual
                        ),
                        None => "no master equation: fewer than two relations of products gather".into(),
                    })
                }))
            }
            "check" | "chk" => crate::said(engine.ask(|w| {
                let m = &w.machine;
                let mut out: Vec<String> = m
                    .factors
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        matches!(f.kind, grm_engine::machine::FKind::Relation | grm_engine::machine::FKind::Nonlinear)
                            && f.w > 0.0
                    })
                    .filter_map(|(k, f)| {
                        let r = m.residual(k);
                        (r.abs() > 1.0).then(|| format!("{} does not hold: {:.1} sigma ({})", f.name, r.abs(), f.text))
                    })
                    .collect();
                for q in &w.quarantine {
                    out.push(format!("{q} is in quarantine"));
                }
                if out.is_empty() {
                    out.push(format!("everything holds (harmony {:.3})", m.harmony()));
                }
                Ok(out.join("\n"))
            })),
            "why" => {
                let mut words = rest.split_whitespace();
                let node = words.next().unwrap_or("").to_string();
                let steps: Option<usize> = words.next().and_then(|s| s.parse().ok());
                crate::said(engine.ask(move |w| {
                    let mut out = vec![];
                    if steps.is_none() {
                        out.push(w.why_value(&node)?);
                    }
                    if w.time.nodes.contains_key(&node) {
                        out.push(w.time.why(&node, steps.unwrap_or(1), 300)?.text(&w.node_sources()));
                    }
                    Ok(out.join("\n"))
                }))
            }
            "play" if rest.trim().is_empty() || rest.trim().parse::<f64>().is_ok() => {
                let hz = rest.trim().parse::<f64>().unwrap_or(self.rt.desk_hz.unwrap_or(10.0)).clamp(0.01, 200.0);
                self.rt.desk_hz = Some(hz);
                vec![Line::said(format!("the clock ticks {hz} times a second (pause: it stops)"))]
            }
            "pause" => {
                self.rt.desk_hz = None;
                vec![Line::said("the clock stops")]
            }
            "load" | "ld" => self.load(engine, &paths, rest),
            "undo" | "redo" if rest.trim().is_empty() => {
                let (from, to) = if word == "undo" {
                    (&mut self.undo_world, &mut self.redo_world)
                } else {
                    (&mut self.redo_world, &mut self.undo_world)
                };
                match from.pop() {
                    Some(w) => {
                        if let Some(now) = engine.query(move |world| std::mem::replace(world, w)) {
                            to.push(now);
                        }
                        vec![Line::said(if word == "undo" {
                            "the world as it was before the last change"
                        } else {
                            "the change given again"
                        })]
                    }
                    None => vec![Line::dim("nothing to take back")],
                }
            }
            "workshop" | "ws" => self.workshop(engine, rest),
            "capacity" => match args.first().copied() {
                None => {
                    let host = crate::host_capacity();
                    let Some((held, room, by_hand)) = engine.query(move |w| {
                        (
                            grm_engine::mind::links(&w.machine.net),
                            grm_engine::mind::capacity(&w.machine.net, host),
                            w.machine.net.capacity.is_some(),
                        )
                    }) else {
                        return Some(vec![Line::error("! the engine has stopped")]);
                    };
                    vec![Line::said(format!(
                        "{held} links held, room for {room} ({}); the host's memory allows {host}",
                        if by_hand { "set by hand" } else { "the host's" }
                    ))]
                }
                Some(a) => {
                    let n = if a == "auto" { None } else { a.parse::<usize>().ok() };
                    if a != "auto" && n.is_none() {
                        return Some(vec![Line::warn("? capacity N | auto")]);
                    }
                    let _ = engine
                        .query(move |w| w.machine.net.capacity = n.map(|n| n.max(grm_engine::mind::MIN_CAPACITY)));
                    vec![Line::dim(match n {
                        Some(n) => format!("room for {} links", n.max(grm_engine::mind::MIN_CAPACITY)),
                        None => format!("room: the host's ({} links)", crate::host_capacity()),
                    })]
                }
            },
            "rent" => {
                let host = crate::host_capacity();
                let Some((held, room, price, mut acc)) =
                    engine.query(move |w| grm_engine::mind::accounts(&w.machine, host))
                else {
                    return Some(vec![Line::error("! the engine has stopped")]);
                };
                acc.sort_by(|a, b| a.worth().partial_cmp(&b.worth()).unwrap_or(std::cmp::Ordering::Equal));
                let mut out = vec![Line::said(format!(
                    "{held} links / room {room} ({:.0} % taken): a link costs {} bit a tick",
                    100.0 * held as f64 / room as f64,
                    grm_engine::lang::g(price)
                ))];
                for a in acc {
                    let pays = if a.worth() >= 0.0 { "pays its rent" } else { "does not pay its rent" };
                    let line = format!(
                        "  {}: brings {} bits, rent {} ({} links): {pays}",
                        a.name,
                        grm_engine::lang::g(a.bits),
                        grm_engine::lang::g(a.rent),
                        a.links
                    );
                    out.push(if a.worth() >= 0.0 { Line::dim(line) } else { Line::warn(line) });
                }
                out
            }
            "stats" => {
                let mut said_words: IndexMap<String, usize> = IndexMap::new();
                for h in &self.history {
                    *said_words.entry(h.split_whitespace().next().unwrap_or("").to_string()).or_insert(0) += 1;
                }
                said_words.sort_by(|_, a, _, b| b.cmp(a));
                let most: Vec<String> = said_words.iter().take(6).map(|(w, n)| format!("{w} {n}")).collect();
                let apps = self.rt.apps.len();
                let mut out = crate::said(engine.ask(move |w| {
                    let net = &w.machine.net;
                    let mut by: IndexMap<String, (usize, usize)> = IndexMap::new();
                    for s in net.nodes.values() {
                        by.entry(s.origin.source.clone()).or_default().0 += 1;
                    }
                    for r in &net.relations {
                        by.entry(r.origin.source.clone()).or_default().1 += 1;
                    }
                    let mut text = format!(
                        "{} nodes, {} relations, {} in time, {} apps running, harmony {:.3}, {} ticks, {} rounds, {} messages",
                        net.nodes.len(),
                        net.relations.len(),
                        w.time.nodes.len(),
                        apps,
                        w.machine.harmony(),
                        net.ticks,
                        w.machine.rounds,
                        w.machine.messages
                    );
                    for (s, (n, r)) in by {
                        text.push_str(&format!("\n  {s}: {n} node(s), {r} relation(s)"));
                    }
                    Ok(text)
                }));
                out.push(Line::dim(format!("what the human says most: {}", most.join(", "))));
                out
            }
            "help" | "h" if !rest.trim().is_empty() => {
                let w = rest.trim();
                match WORDS_HELP.iter().find(|(k, _, _)| *k == w) {
                    Some((_, form, ex)) => {
                        vec![Line::said(form.to_string()), Line::dim(format!("  for example: {ex}"))]
                    }
                    None => vec![Line::warn(format!("? {w}: not a word it knows (help: all of them)"))],
                }
            }
            _ => return None,
        })
    }

    /// `load FILE [COLUMN...]`: its rows, one a tick, into the chosen columns' nodes; then the laws it finds.
    fn load(&mut self, engine: &Engine, paths: &[&str], rest: &str) -> Vec<Line> {
        let Some(file) = paths.first() else { return vec![Line::warn("? load FILE.csv [COLUMN...]")] };
        let path = expand(file);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => return vec![Line::error(format!("! {file}: {e}"))],
        };
        let table = grm_connect::csv::read(&grm_connect::csv::text_of(&bytes));
        let numeric: Vec<bool> = (0..table.names.len()).map(|k| table.numeric(k)).collect();
        let mut chosen: Vec<String> =
            paths[1..].iter().filter(|s| **s != "--mix" && **s != "--here").map(|s| s.to_string()).collect();
        // the name after `--into` is not a column
        if let Some(k) = rest.split_whitespace().position(|w| w == "--into") {
            if let Some(n) = rest.split_whitespace().nth(k + 1) {
                chosen.retain(|c| c != n);
            }
        }
        if chosen.is_empty() {
            if self.window {
                // the window asks which columns
                self.orders.push(Order::Columns {
                    path: path.clone(),
                    names: table.names.clone(),
                    written: table.written.clone(),
                    numeric,
                });
                return vec![Line::dim(format!(
                    "{file}: {} column(s), {} row(s): which ones?",
                    table.names.len(),
                    table.rows.len()
                ))];
            }
            chosen = table.names.iter().zip(&numeric).filter(|(_, n)| **n).map(|(c, _)| c.clone()).collect();
        }
        let _ = rest;
        // COLUMN=NODE: a column into a node of another name
        let mut into: IndexMap<String, String> = IndexMap::new();
        for c in &mut chosen {
            if let Some((col, node)) = c.split_once('=') {
                into.insert(col.to_string(), node.to_string());
                *c = col.to_string();
            }
        }
        if let Some(bad) = into.values().find(|n| !grm_engine::lang::is_name(n)) {
            return vec![Line::warn(format!("? {bad}: a node's name (a letter first, then letters, digits or _)"))];
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let source = format!("file:{stem}");
        // by default a table is learned in a workshop of its own (a new network, apart): only `--here` puts it in
        // the global network at once
        if !rest.split_whitespace().any(|w| w == "--here") {
            // `--into NAME`: into a workshop already there (several tables in one)
            let target = rest
                .split_whitespace()
                .skip_while(|w| *w != "--into")
                .nth(1)
                .map(String::from)
                .unwrap_or_else(|| grm_connect::csv::name_of(&stem));
            return self.load_in_workshop(engine, &table, &chosen, &into, &target, &source);
        }
        // a node of the same name, from somewhere else: its data would be mixed with this table's
        let wanted: Vec<String> = chosen
            .iter()
            .filter(|c| table.names.contains(c))
            .map(|c| into.get(c).cloned().unwrap_or(c.clone()))
            .collect();
        let probe = (wanted.clone(), source.clone());
        let taken: Vec<(String, String)> = engine
            .query(move |w| {
                probe
                    .0
                    .iter()
                    .filter_map(|n| {
                        w.machine
                            .net
                            .nodes
                            .get(n)
                            .filter(|s| s.origin.source != probe.1)
                            .map(|s| (n.clone(), s.origin.source.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !taken.is_empty() && !paths.contains(&"--mix") {
            let list = taken.iter().map(|(n, s)| format!("{n} ({s})")).collect::<Vec<_>>().join(", ");
            if self.window {
                let columns: Vec<String> = chosen.iter().filter(|c| table.names.contains(c)).cloned().collect();
                self.orders.push(Order::Collide {
                    path: path.clone(),
                    stem: grm_connect::csv::name_of(&stem),
                    columns,
                    into: into.clone(),
                    taken: taken.iter().map(|(n, _)| n.clone()).collect(),
                });
            }
            return vec![Line::warn(format!(
                "? these nodes already hold other data: {list}. Load into other names (`load {file} COLUMN=NAME`) \
                 or mix them (`--mix`)"
            ))];
        }
        let mut out = vec![];
        let renamed: Vec<String> =
            table.written.iter().zip(&table.names).filter(|(w, n)| w != n).map(|(w, n)| format!("{w} → {n}")).collect();
        if !renamed.is_empty() {
            out.push(Line::dim(format!("names of the language: {}", renamed.join(", "))));
        }
        let cols: Vec<usize> = chosen.iter().filter_map(|c| table.names.iter().position(|n| n == c)).collect();
        if cols.is_empty() {
            return vec![Line::warn(format!("? none of these columns: {}", table.names.join(", ")))];
        }
        let node_of = |k: usize| into.get(&table.names[k]).cloned().unwrap_or(table.names[k].clone());
        let names: Vec<String> = cols.iter().map(|k| node_of(*k)).collect();
        let rows: Vec<(f64, IndexMap<String, f64>)> = table
            .rows
            .iter()
            .enumerate()
            .map(|(t, r)| (t as f64, cols.iter().filter_map(|k| r[*k].map(|v| (node_of(*k), v))).collect()))
            .collect();
        let origin = grm_engine::Provenance::of(source);
        let n = rows.len();
        // a column that is never below zero is a quantity (positive: its laws are products and powers)
        let positive: Vec<bool> =
            cols.iter().map(|k| table.rows.iter().filter_map(|r| r[*k]).all(|v| v > 0.0)).collect();
        out.extend(crate::said(engine.ask(move |w| {
            for (name, pos) in names.iter().zip(&positive) {
                if !w.machine.net.nodes.contains_key(name) {
                    w.say(&format!("input {name}{}", if *pos { " positive" } else { "" }), &origin)?;
                }
            }
            let fed = w.feed_rows(&rows)?;
            let found = w.discover();
            let mut text = format!("{fed} rows learned, into {}", names.join(", "));
            for f in found {
                text.push_str(&format!("\nfound by itself: {f}"));
            }
            Ok(text)
        })));
        let _ = n;
        out
    }

    /// A table learned in a workshop: a world of its own, named after the file (the same file again: its rows go
    /// on in the same workshop). The window shows it.
    fn load_in_workshop(
        &mut self,
        engine: &Engine,
        table: &grm_connect::csv::Csv,
        chosen: &[String],
        into: &IndexMap<String, String>,
        name: &str,
        source: &str,
    ) -> Vec<Line> {
        let cols: Vec<usize> = chosen.iter().filter_map(|c| table.names.iter().position(|n| n == c)).collect();
        if cols.is_empty() {
            return vec![Line::warn(format!("? none of these columns: {}", table.names.join(", ")))];
        }
        let node_of = |k: usize| into.get(&table.names[k]).cloned().unwrap_or(table.names[k].clone());
        let names: Vec<String> = cols.iter().map(|k| node_of(*k)).collect();
        let positive: Vec<bool> =
            cols.iter().map(|k| table.rows.iter().filter_map(|r| r[*k]).all(|v| v > 0.0)).collect();
        let rows: Vec<(f64, IndexMap<String, f64>)> = table
            .rows
            .iter()
            .enumerate()
            .map(|(t, r)| (t as f64, cols.iter().filter_map(|k| r[*k].map(|v| (node_of(*k), v))).collect()))
            .collect();
        let origin = grm_engine::Provenance::of(source);
        let ws_name = name.to_string();
        let mut out = crate::said(engine.ask(move |w| {
            let ws = w.workshops.entry(ws_name.clone()).or_insert_with(|| Box::new(grm_engine::world::World::new()));
            for (n, pos) in names.iter().zip(&positive) {
                if !ws.machine.net.nodes.contains_key(n) {
                    ws.say(&format!("input {n}{}", if *pos { " positive" } else { "" }), &origin)?;
                }
            }
            let fed = ws.feed_rows(&rows)?;
            let found = ws.discover();
            let mut text = format!(
                "{fed} rows learned in the workshop {ws_name} (a network of its own, apart): {}",
                names.join(", ")
            );
            for f in found {
                text.push_str(&format!("\nfound by itself: {f}"));
            }
            text.push_str(&format!("\nwhen it is ready: workshop merge {ws_name}"));
            let tick = w.machine.net.ticks;
            w.mood.reward(0.1, tick, format!("a new workshop of data: {ws_name}"));
            Ok(text)
        }));
        if self.window {
            self.orders.push(Order::Workshop(name.to_string()));
        }
        out.retain(|l| !l.text.is_empty());
        out
    }

    /// `workshop`: the workshops; `workshop merge NAME`: what it made goes into a sandbox here (to be tried, merged
    /// or refused); `workshop close NAME`; `workshop show NAME`.
    pub(crate) fn workshop(&mut self, engine: &Engine, rest: &str) -> Vec<Line> {
        let mut words = rest.split_whitespace();
        let what = words.next().unwrap_or("");
        let name = words.next().unwrap_or("").to_string();
        match what {
            "" | "list" => {
                let list = engine
                    .query(|w| {
                        w.workshops
                            .iter()
                            .map(|(n, ws)| (n.clone(), ws.machine.net.nodes.len(), ws.machine.net.relations.len()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if list.is_empty() {
                    return vec![Line::dim("no workshop: load FILE.csv makes one")];
                }
                list.into_iter().map(|(n, a, b)| Line::said(format!("{n}: {a} node(s), {b} relation(s)"))).collect()
            }
            "show" | "open" => {
                if self.window {
                    self.orders.push(Order::Workshop(name.clone()));
                }
                vec![Line::dim(format!("workshop {name}"))]
            }
            "merge" => {
                let n = name.clone();
                crate::said(engine.ask(move |w| {
                    let lines = w.workshop_lines(&n)?;
                    let sb = if w.sandboxes.contains_key(&n) { format!("{n}_ws") } else { n.clone() };
                    w.open_sandbox(&sb)?;
                    let mut errors = vec![];
                    for l in &lines {
                        if let Err(e) = w.say_in(&sb, l) {
                            errors.push(format!("{l}: {e}"));
                        }
                    }
                    let mut text = format!(
                        "what the workshop {n} made is in the sandbox {sb} ({} lines): trial {sb}, then merge {sb} or refuse {sb}",
                        lines.len() - errors.len()
                    );
                    for e in errors {
                        text.push_str(&format!("\n  not taken: {e}"));
                    }
                    Ok(text)
                }))
            }
            "new" => {
                if !grm_engine::lang::is_name(&name) {
                    return vec![Line::warn("? workshop new NAME (a name: a letter first, then letters, digits or _)")];
                }
                let n = name.clone();
                let out = crate::said(engine.ask(move |w| {
                    if w.workshops.contains_key(&n) {
                        return Err(grm_engine::GrmError::new(format!("there is already a workshop {n}")));
                    }
                    w.workshops.insert(n.clone(), Box::new(grm_engine::world::World::new()));
                    Ok(format!("workshop {n} opened: a network of its own, empty"))
                }));
                if self.window {
                    self.orders.push(Order::Workshop(name.clone()));
                }
                out
            }
            "say" => {
                // the rest of the line, as written, said in the workshop only
                let line = rest.trim_start().strip_prefix("say").unwrap_or("").trim_start();
                let line = line.strip_prefix(name.as_str()).unwrap_or("").trim().to_string();
                let n = name.clone();
                crate::said(engine.ask(move |w| {
                    let Some(ws) = w.workshops.get_mut(&n) else {
                        return Err(grm_engine::GrmError::new(format!("no workshop {n}")));
                    };
                    let origin = grm_engine::Provenance::of(format!("workshop:{n}"));
                    let said = ws.say(&line, &origin)?;
                    // a small network: built anew, so that everything it holds is heard at once
                    if let Ok(m) = grm_engine::machine::Machine::new(ws.machine.net.clone()) {
                        ws.machine = m;
                    }
                    ws.machine.run(4000);
                    ws.written.push(line.clone());
                    Ok(said.unwrap_or_default())
                }))
            }
            "unsay" => {
                // a line taken back: the workshop is said again without it
                let k: usize = words.next().and_then(|x| x.parse().ok()).unwrap_or(usize::MAX);
                let n = name.clone();
                crate::said(engine.ask(move |w| {
                    let Some(ws) = w.workshops.get_mut(&n) else {
                        return Err(grm_engine::GrmError::new(format!("no workshop {n}")));
                    };
                    if k >= ws.written.len() {
                        return Err(grm_engine::GrmError::new(format!("no line {k} in the workshop {n}")));
                    }
                    let mut lines = ws.written.clone();
                    let gone = lines.remove(k);
                    let mut fresh = grm_engine::world::World::new();
                    let origin = grm_engine::Provenance::of(format!("workshop:{n}"));
                    for l in &lines {
                        let _ = fresh.say(l, &origin);
                    }
                    if let Ok(m) = grm_engine::machine::Machine::new(fresh.machine.net.clone()) {
                        fresh.machine = m;
                    }
                    fresh.machine.run(4000);
                    fresh.written = lines;
                    **ws = fresh;
                    Ok(format!("taken back from {n}: {gone}"))
                }))
            }
            "play" => {
                let node = words.next().unwrap_or("").to_string();
                let rate = self.mixer.lock().map(|m| m.rate).unwrap_or(48_000);
                match Self::reflex_in(engine, Some(&name), &node, rate) {
                    Ok(r) => match grm_sound::mixer::ReflexSource::new(r, &node) {
                        Some(source) => {
                            if let Ok(mut m) = self.mixer.lock() {
                                m.play(&node, Box::new(source), 0.5);
                            }
                            vec![Line::said(format!("{node} plays, from the workshop {name}"))]
                        }
                        None => vec![Line::error(format!("! {node} is not in its network"))],
                    },
                    Err(e) => vec![Line::error(format!("! {e}"))],
                }
            }
            "close" => {
                let n = name.clone();
                crate::said(engine.ask(move |w| match w.workshops.shift_remove(&n) {
                    Some(_) => Ok(format!("workshop {n} closed")),
                    None => Err(grm_engine::GrmError::new(format!("no workshop {n}"))),
                }))
            }
            _ => vec![Line::warn(
                "? workshop [list | new NAME | show NAME | say NAME LINE | unsay NAME K | play NAME NODE | merge NAME | close NAME]",
            )],
        }
    }

    /// `rm` answered yes.
    pub(crate) fn remove_files(&mut self, list: &str) -> Vec<Line> {
        list.lines()
            .map(|p| {
                let path = expand(p);
                let r = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
                match r {
                    Ok(()) => Line::dim(format!("{p} deleted")),
                    Err(e) => Line::error(format!("! {p}: {e}")),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_and_paths() {
        assert!(fits("*.csv", "a.csv") && !fits("*.csv", "a.grm") && fits("d?ta*", "data.csv"));
        std::env::set_var("GRM_TEST_DIR", "/tmp");
        assert_eq!(expand("$GRM_TEST_DIR/x"), PathBuf::from("/tmp/x"));
    }
}
