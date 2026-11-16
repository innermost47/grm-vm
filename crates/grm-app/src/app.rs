// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The window: its parts, where they go, the keys. Every action of a button is a line said to the prompt (it shows
//! in the log, so nothing happens behind the human's back).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Key, Modifiers, RichText};
use indexmap::IndexMap;

use grm_engine::engine::Engine;
use grm_engine::vision::Sight;
use grm_shell::runtime::AppView;
use grm_shell::{Kind as LineKind, Shell};
use grm_sound::mixer::Mixer;

use crate::desk::Mode;
use crate::graph::Graph;
use crate::layout::Layout;
use crate::model::{Detail, Picture, Pull};
use crate::settings::Settings;
use crate::text::{t, Lang};
use crate::worker::{Request, Worker};
use crate::Args;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Inspector,
    Sandboxes,
    Sources,
    Journal,
    Eye,
    Sound,
}

pub const TABS: [(Tab, &str); 6] = [
    (Tab::Inspector, "inspector"),
    (Tab::Sandboxes, "sandboxes"),
    (Tab::Sources, "sources"),
    (Tab::Journal, "journal"),
    (Tab::Eye, "eye"),
    (Tab::Sound, "sound"),
];

pub use crate::style::Ink;

/// A table waiting for its columns to be chosen: (file, names, as written, chosen).
pub type Columns = (PathBuf, Vec<String>, Vec<String>, Vec<bool>);

pub fn dock_key(d: Dock) -> &'static str {
    match d {
        Dock::Right => "d_right",
        Dock::Left => "d_left",
        Dock::Top => "d_top",
        Dock::Bottom => "d_bottom",
    }
}

/// Where the code of the app shown stands beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Dock {
    Right,
    Left,
    Top,
    Bottom,
}

/// A script given on the command line (for tests): its lines are said one after the other, then a picture of the
/// window is taken.
struct Script {
    lines: VecDeque<String>,
    screenshot: Option<PathBuf>,
    wait: Duration,
    done_at: Option<Instant>,
    asked: bool,
}

pub struct Eye {
    pub sight: Sight,
    pub textures: Vec<(String, egui::TextureHandle)>,
}

pub struct App {
    pub engine: Arc<Engine>,
    pub worker: Worker,
    pub mixer: Arc<Mutex<Mixer>>,
    _output: Option<grm_sound::output::Output>,
    _silent: Option<grm_sound::Silent>,
    pub device: Option<String>,
    pub memory: PathBuf,
    fresh: bool,
    pub layout: Layout,
    pub graph: Graph,
    pub picture: Picture,
    pull: Pull<Picture>,
    pub detail: Option<Detail>,
    detail_pull: Pull<Option<Detail>>,
    detail_for: String,
    pub selected: Vec<String>,
    pub view: crate::view::View,
    pub input: String,
    history: Vec<String>,
    history_at: Option<usize>,
    pub log: Vec<(LineKind, String)>,
    pub asking: bool,
    pub tab: Tab,
    pub settings: Settings,
    saved: Settings,
    help_open: bool,
    settings_open: bool,
    focus_prompt: bool,
    pub eyes: IndexMap<String, Eye>,
    pub eye_path: String,
    pub eye_name: String,
    pub play_field: String,
    pub new_sandbox: String,
    pub sandbox_lines: HashMap<String, String>,
    pub filter: String,
    /// The colour index of each source, in the order they first appeared.
    pub colours: IndexMap<String, usize>,
    /// The strain of each relation, smoothed (it changes slowly on screen).
    pub tension: HashMap<String, f32>,
    pub harmony_shown: f32,
    script: Option<Script>,
    pending_select: Vec<String>,
    /// The apps that run, as the prompt last told them; the files that can be opened.
    pub apps: Vec<AppView>,
    pub app_files: Vec<PathBuf>,
    /// The app shown (None: the desk, the whole network).
    pub current: Option<String>,
    pub modes: HashMap<String, Mode>,
    /// The app alone in the window (no panels).
    pub launched: bool,
    pub editing: bool,
    pub xray: bool,
    pub drafts: HashMap<String, crate::code::Draft>,
    pub surface: crate::surface::State,
    /// How many pictures of the world came (a plot hears each).
    pub picture_frame: u64,
    /// A line being written in a small window: (app it goes to, the line it replaces, its text).
    pub line_draft: Option<(Option<String>, Option<usize>, String)>,
    pub new_app: String,
    saved_window: Option<crate::desk::Window>,
    /// Each node's (and relation's) recent values, for its moving curve.
    pub traces: HashMap<String, VecDeque<f32>>,
    /// The nodes the plot view shows.
    pub plotted: Vec<String>,
    pub plot_kind: crate::plot::Kind,
    /// A relation started from the menu: the next node clicked ends it.
    pub relate_from: Option<String>,
    pub node_form: Option<crate::menu::NodeForm>,
    /// The network brought to the front (its source).
    pub focus: Option<String>,
    /// The master equation of everything, and of what is shown.
    pub master: (Option<String>, Option<String>),
    master_pull: Pull<(Option<String>, Option<String>)>,
    /// How the desk is shown (its network, or plots).
    pub desk_mode: Mode,
    /// Files opened to write in, and the one shown.
    pub files: Vec<crate::files::FileTab>,
    pub current_file: Option<usize>,
    /// A table waiting for its columns to be chosen: (file, names, as written, chosen).
    pub columns: Option<Columns>,
    /// The guided visit: its step.
    pub touring: Option<usize>,
    /// Where each part of the window is (for the visit).
    pub areas: HashMap<&'static str, egui::Rect>,
    /// The code of the app shown, beside its network or surface: where (None: not shown).
    pub code_dock: Option<Dock>,
    /// A sandbox shown in the middle, as a network.
    pub sandbox_shown: Option<String>,
    sandbox_picture: Option<Picture>,
    sandbox_pull: Pull<Option<Picture>>,
    /// Each app's looks (a small network each), made again when its look lines or the palette change.
    pub looks: HashMap<String, crate::looks::Looks>,
    /// Whether the apps have been told once (an app that opens after that is shown).
    apps_known: bool,
    wanted_app: Option<String>,
    window_saved_at: Instant,
    world_saved_at: Instant,
    /// Where the human has been, and every change that can be taken back.
    pub journey: crate::journey::Journey,
    /// The window that makes a new app: open, and whether its field still has to take the keys.
    new_app_open: bool,
    new_app_focus: bool,
    /// Files being chosen in the system's window (it answers when the human has chosen).
    picking: Option<(Pick, Chosen)>,
}

/// The files chosen in the system's window, and how long it took to answer (when it comes).
type Chosen = std::sync::mpsc::Receiver<(Vec<PathBuf>, Duration)>;

/// What files chosen in the system's window are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Opened, each as what it is.
    Open,
    /// A picture for the eye.
    Eye,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args) -> App {
        let memory = grm_shell::memory();
        let (world, said) =
            if args.fresh { (grm_engine::world::World::new(), None) } else { grm_shell::wake_or_born(&memory) };
        let engine = Arc::new(Engine::start(world, Duration::from_millis(8)));
        let mut settings = Settings::load();
        if let Some(l) = &args.lang {
            settings.lang = if l == "fr" { Lang::Fr } else { Lang::En };
        }
        if let Some(th) = &args.theme {
            let name = match th.as_str() {
                "dark" => "night",
                "light" => "day",
                other => other,
            };
            if let Some(p) = crate::style::presets().into_iter().find(|p| p.name == name) {
                settings.palette = p;
            }
        }
        let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
        let (output, device) = if args.screenshot.is_some() {
            (None, None)
        } else {
            match grm_sound::output::open(mixer.clone()) {
                Ok(o) => {
                    let d = o.device.clone();
                    (Some(o), Some(d))
                }
                Err(_) => (None, None),
            }
        };
        if let Ok(mut m) = mixer.lock() {
            m.master = settings.volume;
            m.ambient_gain = settings.voice_level;
            let rate = m.rate;
            let kind = voice_kind(&settings.voice);
            m.ambient = grm_sound::ambient::Ambient::new(kind, 0.8, rate);
            if settings.voice_on {
                m.ambient_on = true;
                m.ambient.fade_in();
            }
        }
        let silent = if output.is_none() { Some(grm_sound::silent(mixer.clone())) } else { None };
        let mut shell = Shell::new(memory.clone(), mixer.clone());
        shell.keep_desk = !(args.fresh || args.screenshot.is_some());
        shell.window = true;
        let mut worker = Worker::start(engine.clone(), shell);
        let mut log: Vec<(LineKind, String)> =
            crate::style::BANNER.lines().map(|l| (LineKind::Said, format!("\u{1}{l}"))).collect();
        if let Some(l) = said {
            log.push((l.kind, l.text));
        }
        if let Some(file) = &args.file {
            worker.send(format!("use {file}"));
        }
        let script = args.script.as_ref().map(|path| {
            let lines = std::fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            Script {
                lines,
                screenshot: args.screenshot.as_ref().map(PathBuf::from),
                wait: Duration::from_secs_f64(args.wait),
                done_at: None,
                asked: false,
            }
        });
        let script = script.or_else(|| {
            args.screenshot.as_ref().map(|s| Script {
                lines: VecDeque::new(),
                screenshot: Some(PathBuf::from(s)),
                wait: Duration::from_secs_f64(args.wait),
                done_at: None,
                asked: false,
            })
        });
        let tab = match args.tab.as_deref() {
            Some("sandboxes") => Tab::Sandboxes,
            Some("sources") => Tab::Sources,
            Some("journal") => Tab::Journal,
            Some("eye") => Tab::Eye,
            Some("sound") => Tab::Sound,
            _ => Tab::Inspector,
        };
        apply_look(&cc.egui_ctx, &settings);
        App {
            engine,
            worker,
            mixer,
            _output: output,
            _silent: silent,
            device,
            memory,
            fresh: args.fresh,
            layout: Layout::new(),
            graph: Graph::default(),
            picture: Picture::default(),
            pull: Pull::new(Duration::from_millis(200)),
            detail: None,
            detail_pull: Pull::new(Duration::from_millis(1000)),
            detail_for: String::new(),
            selected: vec![],
            view: crate::view::View::default(),
            input: String::new(),
            history: vec![],
            history_at: None,
            log,
            asking: false,
            tab,
            saved: settings.clone(),
            settings,
            help_open: false,
            settings_open: false,
            focus_prompt: true,
            eyes: IndexMap::new(),
            eye_path: String::new(),
            eye_name: "eye".into(),
            play_field: String::new(),
            new_sandbox: String::new(),
            sandbox_lines: HashMap::new(),
            filter: String::new(),
            colours: IndexMap::new(),
            tension: HashMap::new(),
            harmony_shown: 1.0,
            script,
            pending_select: args.select,
            apps: vec![],
            app_files: vec![],
            current: None,
            modes: match (&args.app, args.mode.as_deref()) {
                (Some(a), Some(m)) => [(
                    a.clone(),
                    match m {
                        "code" => Mode::Code,
                        "network" => Mode::Network,
                        _ => Mode::App,
                    },
                )]
                .into_iter()
                .collect(),
                _ => HashMap::new(),
            },
            launched: args.launch,
            editing: args.edit,
            xray: args.xray,
            drafts: HashMap::new(),
            surface: Default::default(),
            picture_frame: 0,
            line_draft: None,
            new_app: String::new(),
            saved_window: None,
            traces: HashMap::new(),
            plotted: vec![],
            plot_kind: crate::plot::Kind::Lines,
            relate_from: None,
            node_form: None,
            focus: None,
            master: (None, None),
            master_pull: Pull::new(Duration::from_millis(1000)),
            desk_mode: Mode::Network,
            files: vec![],
            current_file: None,
            columns: None,
            touring: None,
            areas: HashMap::new(),
            code_dock: None,
            sandbox_shown: None,
            sandbox_picture: None,
            sandbox_pull: Pull::new(Duration::from_millis(300)),
            looks: HashMap::new(),
            apps_known: false,
            wanted_app: args.app.clone(),
            window_saved_at: Instant::now(),
            world_saved_at: Instant::now(),
            journey: Default::default(),
            new_app_open: false,
            new_app_focus: false,
            picking: None,
        }
        .restored(args.fresh || args.screenshot.is_some())
        .first_visit(args.screenshot.is_none())
    }

    /// The visit comes by itself the first time.
    fn first_visit(mut self, live: bool) -> App {
        if live && !self.settings.toured {
            self.touring = Some(0);
        }
        self
    }

    /// The window as it was left (not in a test run).
    fn restored(mut self, fresh: bool) -> App {
        if fresh {
            return self;
        }
        let Some(w) = crate::desk::Window::load(&self.memory) else { return self };
        self.current = w.current.clone();
        self.modes = w.modes.clone();
        self.launched = w.launched;
        self.editing = w.editing;
        self.xray = w.xray;
        self.tab = TABS.iter().find(|(_, k)| *k == w.side_tab).map(|(t, _)| *t).unwrap_or(Tab::Inspector);
        self.pending_select = w.selected.clone();
        self.graph.pos = w.positions.clone();
        self.graph.pinned = w.pinned.iter().cloned().collect();
        if w.zoom > 0.0 {
            self.view.zoom = w.zoom;
            self.view.offset = egui::vec2(w.offset[0], w.offset[1]);
            self.view.fitted = w.fitted;
        }
        if let Some(x) = w.side_wish {
            self.layout.wish_side(x);
        }
        if let Some(x) = w.bottom_wish {
            self.layout.wish_bottom(x);
        }
        self.eye_path = w.eye_path.clone();
        self.history = w.history.clone();
        let kind = |c: char| match c {
            'd' => LineKind::Dim,
            'w' => LineKind::Warning,
            'e' => LineKind::Error,
            'a' => LineKind::Ask,
            _ => LineKind::Said,
        };
        let mut log: Vec<(LineKind, String)> = w.log.iter().map(|(c, t)| (kind(*c), t.clone())).collect();
        log.append(&mut self.log);
        self.log = log;
        self.saved_window = Some(w);
        self
    }

    fn window_now(&self, ctx: &egui::Context) -> crate::desk::Window {
        let code = |k: &LineKind| match k {
            LineKind::Dim => 'd',
            LineKind::Warning => 'w',
            LineKind::Error => 'e',
            LineKind::Ask => 'a',
            LineKind::Said => 's',
        };
        let start = self.log.len().saturating_sub(300);
        crate::desk::Window {
            current: self.current.clone(),
            modes: self.modes.clone(),
            launched: self.launched,
            editing: self.editing,
            xray: self.xray,
            side_tab: TABS.iter().find(|(t, _)| *t == self.tab).map(|(_, k)| k.to_string()).unwrap_or_default(),
            selected: self.selected.clone(),
            positions: self.graph.pos.clone(),
            pinned: self.graph.pinned.iter().cloned().collect(),
            zoom: self.view.zoom,
            offset: [self.view.offset.x, self.view.offset.y],
            fitted: self.view.fitted,
            side_wish: self.layout.side_wish,
            bottom_wish: self.layout.bottom_wish,
            eye_path: self.eye_path.clone(),
            history: self.history.iter().rev().take(500).rev().cloned().collect(),
            // the banner is said again at each start: it is not kept
            log: self.log[start..]
                .iter()
                .filter(|(_, t)| !t.starts_with('\u{1}'))
                .map(|(k, t)| (code(k), t.clone()))
                .collect(),
            size: Some({
                let r = ctx.content_rect();
                [r.width() * ctx.zoom_factor(), r.height() * ctx.zoom_factor()]
            }),
        }
    }

    /// Keeps the window as it is (when it changed), and the world now and then.
    fn keep(&mut self, ctx: &egui::Context) {
        if self.fresh {
            return;
        }
        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing {
            // it falls asleep now, whatever happens to the window after
            let path = self.memory.clone();
            let _ = self.engine.ask(move |w| w.sleep(&path).map(|_| String::new()));
            self.world_saved_at = Instant::now();
        }
        if closing || self.window_saved_at.elapsed() > Duration::from_secs(3) {
            self.window_saved_at = Instant::now();
            let w = self.window_now(ctx);
            if self.saved_window.as_ref() != Some(&w) {
                w.save(&self.memory);
                self.saved_window = Some(w);
            }
        }
        if self.world_saved_at.elapsed() > Duration::from_secs(60) {
            self.world_saved_at = Instant::now();
            let path = self.memory.clone();
            self.engine.send(move |w| w.sleep(&path).map(|_| String::new()));
        }
    }

    pub fn mode_of(&self, app: &str) -> Mode {
        self.modes.get(app).copied().unwrap_or(Mode::App)
    }

    /// An app's looks, settled (made again when what makes them changed; the hour read each minute).
    pub fn looks_of(&mut self, view: &AppView) -> &crate::looks::Looks {
        let key = format!("{:?}{:?}{}", self.settings.palette, view.looks, self.settings.follow_hour);
        let fresh = self.looks.get(&view.name).is_none_or(|l| l.key != key);
        if fresh {
            let l = crate::looks::Looks::build(&self.settings.palette, &view.looks, self.settings.follow_hour);
            self.looks.insert(view.name.clone(), l);
        }
        let l = self.looks.get_mut(&view.name).unwrap();
        l.settle(false);
        l
    }

    /// The plots, where they are shown.
    pub fn show_plot(&mut self) {
        match self.current.clone() {
            Some(c) => {
                self.modes.insert(c, Mode::Plot);
            }
            None => self.desk_mode = Mode::Plot,
        }
    }

    /// A line made by the mouse: into the app shown (its code shows it), or said to the desk.
    pub fn write_line(&mut self, line: &str) {
        match self.current.clone() {
            Some(a) => {
                if let Some(v) = self.apps.iter().find(|x| x.name == a) {
                    let text = grm_shell::apps::with_added(&v.text, line);
                    self.edit_app_text(&a, text);
                }
            }
            None => self.say(line.to_string()),
        }
    }

    /// A new text for a running app: it is said again, and written to its file.
    pub fn edit_app_text(&mut self, name: &str, text: String) {
        if let Some(v) = self.apps.iter_mut().find(|a| a.name == name) {
            if v.text == text {
                return;
            }
            v.text = text.clone();
        }
        self.journey.record(crate::journey::Change::App(name.to_string()), format!("app {name}"));
        self.worker.request(Request::Edit(name.to_string(), text));
    }

    /// A relation drawn from one node to another: its line, to write before it is said.
    pub fn start_relation(&mut self, from: &str, to: &str) {
        let signed = |n: &str| self.picture.node(n).is_some_and(|x| x.signed);
        let mut name = format!("{from}_{to}");
        let mut k = 2;
        while self.picture.relations.iter().any(|r| r.name == name) {
            name = format!("{from}_{to}_{k}");
            k += 1;
        }
        let text = if signed(from) || signed(to) {
            format!("relate {name}: {to} = 1 * {from} + 0")
        } else {
            format!("relate {name}: {to} = 1 * {from}")
        };
        self.line_draft = Some((self.current.clone(), None, text));
    }

    /// The app that writes a relation or a node itself, and its line there.
    pub fn line_in_app(&self, name: &str) -> Option<(String, usize)> {
        for a in &self.apps {
            let original = a.prefix.iter().find(|(_, v)| *v == name).map(|(k, _)| k.as_str()).unwrap_or(name);
            if let Some(l) = grm_shell::apps::line_of(&a.text, original) {
                return Some((a.name.clone(), l));
            }
        }
        None
    }

    pub fn tr(&self, key: &str) -> &'static str {
        t(self.settings.lang, key)
    }

    /// Says a line to the prompt (as if typed).
    pub fn say(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.log.push((LineKind::Dim, format!("› {line}")));
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.worker.send(line);
    }

    pub fn colour_of(&mut self, source: &str) -> usize {
        let n = self.colours.len();
        *self.colours.entry(source.to_string()).or_insert(n)
    }

    pub fn select(&mut self, node: &str, add: bool) {
        if add {
            if let Some(k) = self.selected.iter().position(|s| s == node) {
                self.selected.remove(k);
            } else {
                self.selected.push(node.to_string());
            }
        } else {
            self.selected = vec![node.to_string()];
        }
        self.detail_pull.soon();
        if self.selected.len() == 1 {
            self.tab = Tab::Inspector;
        }
    }

    fn pull(&mut self, ctx: &egui::Context) {
        // a surface, and the curves under the pointer, move with the world: it is asked often
        self.pull.every = Duration::from_millis(if self.current.is_some() { 50 } else { 100 });
        let here = self.current.as_ref().map(|c| grm_shell::runtime::source_of(c)).or(self.focus.clone());
        if let Some(m) = self.master_pull.poll(&self.engine, move |w| {
            let text = |m: Option<grm_engine::master::Master>| {
                m.map(|m| format!("{}   ({:.2} decades, residual {:.1e} dex)", m.text(), m.decades, m.residual))
            };
            (text(w.master(None)), here.as_deref().and_then(|h| text(w.master(Some(h)))))
        }) {
            self.master = m;
        }
        if let Some(p) = self.pull.poll(&self.engine, |w| Picture::of(w)) {
            for n in &p.nodes {
                let s = n.source.clone();
                self.colour_of(&s);
            }
            for r in &p.relations {
                let s = r.source.clone();
                self.colour_of(&s);
            }
            self.picture = p;
            self.picture_frame += 1;
            crate::hover::remember(self);
            self.selected.retain(|s| self.picture.node(s).is_some());
            if !self.pending_select.is_empty() {
                let wanted = std::mem::take(&mut self.pending_select);
                for w in wanted {
                    if self.picture.node(&w).is_some() {
                        self.selected.push(w);
                    } else {
                        self.pending_select.push(w);
                    }
                }
            }
            ctx.request_repaint();
        }
        let one = if self.selected.len() == 1 { self.selected[0].clone() } else { String::new() };
        if one != self.detail_for {
            self.detail_for = one.clone();
            self.detail = None;
            self.detail_pull.soon();
        }
        if !one.is_empty() && self.picture.node(&one).is_some_and(|n| n.tracked) {
            let name = one.clone();
            if let Some(d) = self.detail_pull.poll(&self.engine, move |w| Detail::of(w, &name)) {
                if d.as_ref().is_none_or(|d| d.node == self.detail_for) {
                    self.detail = d;
                }
            }
        }
        // what is shown changes slowly: strain and harmony glide
        let k = 0.08f32;
        for r in &self.picture.relations {
            let v = self.tension.entry(r.name.clone()).or_insert(0.0);
            *v += ((r.tension as f32).min(4.0) - *v) * k;
        }
        self.harmony_shown += (self.picture.harmony as f32 - self.harmony_shown) * k;
    }

    fn answers(&mut self, ctx: &egui::Context) {
        for a in self.worker.answers() {
            if a.kept {
                self.journey.record(crate::journey::Change::World, a.typed.clone());
            }
            for l in a.lines {
                self.log.push((l.kind, l.text));
            }
            self.asking = a.asking;
            if let Some(apps) = a.apps {
                if self.apps_known {
                    for v in &apps {
                        if !self.apps.iter().any(|x| x.name == v.name) {
                            // an app that just opened is shown (and can be closed again by taking it back)
                            self.current = Some(v.name.clone());
                            if !self.journey.quiet.remove(&v.name) {
                                let change =
                                    crate::journey::Change::Opened { name: v.name.clone(), path: v.path.clone() };
                                self.journey.record(change, format!("app open {}", v.name));
                            }
                        }
                    }
                    for x in &self.apps {
                        if !apps.iter().any(|v| v.name == x.name) && !self.journey.quiet.remove(&x.name) {
                            let change = crate::journey::Change::Closed { name: x.name.clone(), path: x.path.clone() };
                            self.journey.record(change, format!("app close {}", x.name));
                        }
                    }
                }
                let closed: Vec<String> = self
                    .apps
                    .iter()
                    .filter(|x| !apps.iter().any(|v| v.name == x.name))
                    .map(|x| x.name.clone())
                    .collect();
                self.apps = apps;
                self.apps_known = true;
                if let Some(c) = &self.current {
                    if closed.contains(c) || !self.apps.iter().any(|a| &a.name == c) {
                        self.current = None;
                        self.launched = false;
                    }
                }
                if let Some(w) = &self.wanted_app {
                    if self.apps.iter().any(|a| &a.name == w) {
                        self.current = Some(w.clone());
                    }
                }
            }
            if let Some(files) = a.files {
                self.app_files = files;
            }
            for o in a.orders {
                match o {
                    grm_shell::Order::OpenFile(p) => {
                        let k = match self.files.iter().position(|f| f.path == p) {
                            Some(k) => k,
                            None => {
                                self.files.push(crate::files::FileTab::open(p));
                                self.files.len() - 1
                            }
                        };
                        self.current_file = Some(k);
                    }
                    grm_shell::Order::Columns { path, names, written, numeric } => {
                        self.columns = Some((path, names, written, numeric));
                    }
                    grm_shell::Order::Focus(f) => self.focus = f,
                    grm_shell::Order::Tour => self.touring = Some(0),
                    grm_shell::Order::Lang(l) => {
                        self.settings.lang = if l.starts_with("fr") { Lang::Fr } else { Lang::En };
                    }
                    grm_shell::Order::Clear => self.log.clear(),
                    grm_shell::Order::Folder => {}
                }
            }
            if let Some((name, sight)) = a.sight {
                let textures = textures(ctx, &name, &sight);
                self.eyes.insert(name, Eye { sight, textures });
                if self.script.is_none() {
                    self.tab = Tab::Eye;
                }
            }
            if a.quit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            self.pull.soon();
            self.detail_pull.soon();
        }
        if self.log.len() > 2000 {
            self.log.drain(0..self.log.len() - 2000);
        }
    }

    fn script(&mut self, ctx: &egui::Context) {
        let Some(s) = &mut self.script else { return };
        if self.worker.waiting == 0 {
            if let Some(line) = s.lines.pop_front() {
                let line = line.clone();
                self.say(line);
                return;
            }
            let s = self.script.as_mut().unwrap();
            if s.done_at.is_none() {
                s.done_at = Some(Instant::now());
            }
        }
        let s = self.script.as_mut().unwrap();
        if let (Some(at), Some(_)) = (s.done_at, &s.screenshot) {
            if at.elapsed() >= s.wait && !s.asked {
                s.asked = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        ctx.request_repaint_after(Duration::from_millis(50));
        let shot: Option<Arc<egui::ColorImage>> = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = shot {
            if let Some(path) = &s.screenshot {
                let [w, h] = image.size;
                let mut data = Vec::with_capacity(w * h * 3);
                for c in &image.pixels {
                    let [r, g, b, _] = c.to_srgba_unmultiplied();
                    data.extend([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]);
                }
                let p = grm_engine::vision::Picture { width: w, height: h, channels: 3, data };
                if let Err(e) = grm_connect::write_picture(path, &p) {
                    eprintln!("{}: {e}", path.display());
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (help, esc, prompt, panic) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F1),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::COMMAND, Key::L),
                i.consume_key(Modifiers::COMMAND, Key::Period),
            )
        });
        if help {
            self.help_open = !self.help_open;
        }
        // back and forward, as in a browser
        let (back, forward) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::ALT, Key::ArrowLeft) || i.pointer.button_pressed(egui::PointerButton::Extra1),
                i.consume_key(Modifiers::ALT, Key::ArrowRight) || i.pointer.button_pressed(egui::PointerButton::Extra2),
            )
        });
        if back {
            self.go_back();
        }
        if forward {
            self.go_forward();
        }
        // take back, give again: unless words are being written (they have their own)
        let typing =
            ctx.memory(|m| m.focused()).is_some_and(|id| id != egui::Id::new("prompt-line") || !self.input.is_empty());
        if !typing {
            let (undo, redo) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::COMMAND, Key::Z),
                    i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                        || i.consume_key(Modifiers::COMMAND, Key::Y),
                )
            });
            if redo {
                self.take_back(true);
            } else if undo {
                self.take_back(false);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.pick(Pick::Open);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let (edit, xray, launch) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::E),
                i.consume_key(Modifiers::COMMAND, Key::R),
                i.consume_key(Modifiers::NONE, Key::F5),
            )
        });
        if edit && self.current.is_some() {
            self.editing = !self.editing;
            if let Some(c) = self.current.clone() {
                self.modes.insert(c, Mode::App);
            }
        }
        if xray {
            self.xray = !self.xray;
        }
        if launch && self.current.is_some() {
            self.launched = !self.launched;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.launched));
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) && self.current.is_some() {
            self.code_dock = if self.code_dock.is_some() { None } else { Some(self.settings.dock) };
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
        if esc && self.touring.is_some() {
            self.touring = None;
            self.settings.toured = true;
        } else if esc {
            if self.launched {
                self.launched = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            } else if self.editing {
                self.editing = false;
            } else {
                self.selected.clear();
                self.help_open = false;
                self.settings_open = false;
                self.line_draft = None;
                self.relate_from = None;
                self.node_form = None;
                self.new_app_open = false;
            }
        }
        if prompt {
            self.focus_prompt = true;
        }
        if panic {
            self.say("panic");
        }
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6];
        for (k, key) in keys.iter().enumerate() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, *key)) {
                self.tab = TABS[k].0;
            }
        }
    }

    fn top(&mut self, ui: &mut egui::Ui) {
        let ink = Ink::of(ui.ctx());
        {
            ui.horizontal(|ui| {
                let lang = self.settings.lang;
                if crate::icons::sign_enabled(ui, self.journey.can_back(), "arrow-left", t(lang, "go_back")).clicked() {
                    self.go_back();
                }
                if crate::icons::sign_enabled(ui, self.journey.can_forward(), "arrow-right", t(lang, "go_forward"))
                    .clicked()
                {
                    self.go_forward();
                }
                let last = |e: Option<&crate::journey::Entry>| e.map(|e| e.what.clone()).unwrap_or_default();
                let undo_what = format!("{} (Ctrl+Z)  {}", t(lang, "undo"), last(self.journey.undo.last()));
                if crate::icons::sign_enabled(ui, !self.journey.undo.is_empty(), "undo-2", &undo_what).clicked() {
                    self.take_back(false);
                }
                let redo_what = format!("{} (Ctrl+Y)  {}", t(lang, "redo"), last(self.journey.redo.last()));
                if crate::icons::sign_enabled(ui, !self.journey.redo.is_empty(), "redo-2", &redo_what).clicked() {
                    self.take_back(true);
                }
                if crate::icons::sign(ui, "folder-open", &format!("{} (Ctrl+O)", t(lang, "open_files"))).clicked() {
                    self.pick(Pick::Open);
                }
                ui.separator();
                ui.label(RichText::new("grm-vm").strong());
                ui.separator();
                let h = self.harmony_shown;
                let colour = if h < 0.6 { ink.amber } else { ink.text };
                ui.label(RichText::new(format!("{} {:.2}", self.tr("harmony"), h)).color(colour))
                    .on_hover_text("how well the relations hold together (1: all of them)");
                let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 6.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, ink.faint);
                let mut full = rect;
                full.set_width(rect.width() * h.clamp(0.0, 1.0));
                ui.painter().rect_filled(full, 3.0, colour);
                ui.label(RichText::new(format!("{} {}", self.picture.ticks, self.tr("ticks"))).color(ink.muted));
                ui.label(
                    RichText::new(if self.picture.settled { self.tr("settled") } else { self.tr("computing") })
                        .color(ink.muted),
                );
                if self.worker.waiting > 0 {
                    ui.label(RichText::new(self.tr("busy")).color(ink.muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let help = crate::icons::button(ui, "circle-help", self.tr("help")).on_hover_text("F1");
                    self.areas.insert("help", help.rect);
                    if help.clicked() {
                        self.help_open = !self.help_open;
                    }
                    let settings = crate::icons::button(ui, "settings", self.tr("settings"));
                    self.areas.insert("settings", settings.rect);
                    if settings.clicked() {
                        self.settings_open = !self.settings_open;
                    }
                    let sounding = self.mixer.lock().map(|m| m.ambient_on || !m.channels.is_empty()).unwrap_or(false);
                    if sounding
                        && ui.button(RichText::new(self.tr("panic")).color(ink.amber)).on_hover_text("Ctrl+.").clicked()
                    {
                        self.say("panic");
                    }
                    if crate::icons::button(ui, "moon", self.tr("sleep"))
                        .on_hover_text(self.memory.display().to_string())
                        .clicked()
                    {
                        self.say("sleep");
                    }
                    if ui.button(self.tr("tick")).clicked() {
                        self.say("tick");
                    }
                });
            });
        }
    }

    fn prompt(&mut self, ui: &mut egui::Ui) {
        let ink = Ink::of(ui.ctx());
        {
            let id = egui::Id::new("prompt-line");
            let line_h = ui.spacing().interact_size.y + 10.0;
            egui::ScrollArea::vertical()
                .max_height((ui.available_height() - line_h).max(20.0))
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for (kind, text) in &self.log {
                        let colour = match kind {
                            LineKind::Said => ink.text,
                            LineKind::Dim => ink.muted,
                            LineKind::Warning | LineKind::Ask => ink.amber,
                            LineKind::Error => ink.amber,
                        };
                        if let Some(banner) = text.strip_prefix('\u{1}') {
                            ui.label(RichText::new(banner).monospace().color(ink.accent));
                            continue;
                        }
                        // what was said: coloured as code is
                        if let Some(said) = text.strip_prefix("› ") {
                            let mut job = crate::code::highlight_line(&ink, said);
                            job.sections.iter_mut().for_each(|s| s.format.color = s.format.color.gamma_multiply(0.75));
                            job.sections.insert(
                                0,
                                egui::text::LayoutSection {
                                    leading_space: 0.0,
                                    byte_range: 0..0,
                                    format: Default::default(),
                                },
                            );
                            let mut full = egui::text::LayoutJob::default();
                            full.append("› ", 0.0, egui::TextFormat::simple(egui::FontId::monospace(13.0), ink.muted));
                            for sec in job.sections.iter().skip(1) {
                                full.append(&job.text[sec.byte_range.clone()], 0.0, sec.format.clone());
                            }
                            ui.label(full);
                            continue;
                        }
                        let shown = if self.settings.lang == Lang::Fr && !text.starts_with("› ") {
                            crate::french::fr(text)
                        } else {
                            text.clone()
                        };
                        let rich = RichText::new(shown).monospace().color(colour);
                        let rich = if *kind == LineKind::Error { rich.strong() } else { rich };
                        ui.label(rich);
                    }
                });
            ui.horizontal(|ui| {
                let here = std::env::current_dir()
                    .map(|d| {
                        let d = d.display().to_string();
                        let home = std::env::var("HOME").unwrap_or_default();
                        if !home.is_empty() && d.starts_with(&home) {
                            format!("~{}", &d[home.len()..])
                        } else {
                            d
                        }
                    })
                    .unwrap_or_default();
                ui.label(RichText::new(format!("grm {here}›")).monospace().color(ink.accent));
                let has_focus = ui.memory(|m| m.has_focus(id));
                if has_focus {
                    let (up, down, tab) = ui.input_mut(|i| {
                        (
                            i.consume_key(Modifiers::NONE, Key::ArrowUp),
                            i.consume_key(Modifiers::NONE, Key::ArrowDown),
                            i.consume_key(Modifiers::NONE, Key::Tab),
                        )
                    });
                    let mut moved = false;
                    if up && !self.history.is_empty() {
                        let k = self.history_at.map(|k| k.saturating_sub(1)).unwrap_or(self.history.len() - 1);
                        self.history_at = Some(k);
                        self.input = self.history[k].clone();
                        moved = true;
                    }
                    if down {
                        if let Some(k) = self.history_at {
                            if k + 1 < self.history.len() {
                                self.history_at = Some(k + 1);
                                self.input = self.history[k + 1].clone();
                            } else {
                                self.history_at = None;
                                self.input.clear();
                            }
                            moved = true;
                        }
                    }
                    if tab {
                        self.input = self.complete(&self.input);
                        moved = true;
                    }
                    if moved {
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                            let end = egui::text::CCursor::new(self.input.chars().count());
                            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                            state.store(ui.ctx(), id);
                        }
                    }
                }
                let asking = self.asking;
                let width = ui.available_width() - if asking { 140.0 } else { 0.0 };
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _w: f32| {
                    let job = crate::code::highlight_line(&ink, text.as_str());
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let edit = egui::TextEdit::singleline(&mut self.input)
                    .layouter(&mut layouter)
                    .id(id)
                    .lock_focus(true)
                    .font(egui::TextStyle::Monospace)
                    .hint_text(t(self.settings.lang, "prompt_hint"))
                    .desired_width(width.max(80.0));
                let r = ui.add(edit);
                if self.focus_prompt {
                    r.request_focus();
                    self.focus_prompt = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    let line = std::mem::take(&mut self.input);
                    self.history_at = None;
                    if !line.trim().is_empty() {
                        self.say(line);
                    }
                    r.request_focus();
                }
                if asking {
                    if ui.button(self.tr("yes")).clicked() {
                        self.say("y");
                    }
                    if ui.button(self.tr("no")).clicked() {
                        self.say("n");
                    }
                }
            });
        }
    }

    /// Completes the last word: a command, then a node.
    fn complete(&self, input: &str) -> String {
        const WORDS: &[&str] = crate::code::PROMPT_WORDS;
        let start = input.rfind(|c: char| c.is_whitespace() || "=+-*/(),:".contains(c)).map(|k| k + 1).unwrap_or(0);
        let (head, word) = input.split_at(start);
        if word.is_empty() {
            return input.to_string();
        }
        let mut found: Vec<String> = if head.trim().is_empty() {
            WORDS.iter().filter(|w| w.starts_with(word)).map(|w| w.to_string()).collect()
        } else {
            vec![]
        };
        found.extend(self.picture.nodes.iter().filter(|n| n.name.starts_with(word)).map(|n| n.name.clone()));
        found.extend(self.picture.sources.iter().filter(|(s, _)| s.starts_with(word)).map(|(s, _)| s.clone()));
        found.dedup();
        match found.len() {
            0 => input.to_string(),
            1 => format!("{head}{} ", found[0]),
            _ => {
                // what they all begin with
                let first = &found[0];
                let common = (word.len()..=first.len())
                    .rev()
                    .find(|&k| found.iter().all(|f| f.get(..k) == first.get(..k)))
                    .unwrap_or(word.len());
                format!("{head}{}", &first[..common])
            }
        }
    }

    fn windows(&mut self, ctx: &egui::Context) {
        self.line_window(ctx);
        self.new_app_window(ctx);
        crate::menu::node_form(self, ctx);
        let lang = self.settings.lang;
        let mut open = self.help_open;
        egui::Window::new(t(lang, "help")).open(&mut open).default_width(640.0).vscroll(true).show(ctx, |ui| {
            if crate::style::chosen(ui, true, t(lang, "tour_again")).clicked() {
                self.touring = Some(0);
            }
            ui.heading(t(lang, "shortcuts"));
            ui.label(RichText::new(t(lang, "keys_text")).monospace());
            ui.label(format!("{} {} (Lucide, ISC)", crate::icons::names().count(), t(lang, "icons_note")));
            ui.separator();
            ui.label(t(lang, "engine_english"));
            ui.label(RichText::new(grm_shell::HELP).monospace().small());
        });
        self.help_open = open;
        let mut open = self.settings_open;
        let mut reset = false;
        egui::Window::new(t(lang, "settings")).open(&mut open).default_width(420.0).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(t(lang, "language"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings.lang, Lang::Fr, "Français");
                    ui.selectable_value(&mut self.settings.lang, Lang::En, "English");
                });
                ui.end_row();
                ui.label(t(lang, "theme"));
                ui.horizontal_wrapped(|ui| {
                    for p in crate::style::presets() {
                        if crate::style::chosen(ui, self.settings.palette.name == p.name, &p.name).clicked() {
                            self.settings.palette = p;
                        }
                    }
                });
                ui.end_row();
                ui.label(t(lang, "colours"));
                ui.horizontal_wrapped(|ui| {
                    let pal = &mut self.settings.palette;
                    let mut edited = false;
                    for (key, rgb) in [
                        ("col_background", &mut pal.background),
                        ("col_panel", &mut pal.panel),
                        ("col_raised", &mut pal.raised),
                        ("col_text", &mut pal.text),
                        ("col_muted", &mut pal.muted),
                        ("col_accent", &mut pal.accent),
                        ("col_warn", &mut pal.warn),
                    ] {
                        ui.vertical(|ui| {
                            edited |= ui.color_edit_button_srgb(rgb).changed();
                            ui.label(RichText::new(t(lang, key)).small());
                        });
                    }
                    if edited && !pal.name.ends_with('*') {
                        pal.name.push('*');
                    }
                });
                ui.end_row();
                ui.label(t(lang, "typewriter"));
                ui.checkbox(&mut self.settings.typewriter, "");
                ui.end_row();
                ui.label(t(lang, "follow_hour"));
                ui.checkbox(&mut self.settings.follow_hour, t(lang, "follow_hour_d"));
                ui.end_row();
                ui.label(t(lang, "text_size"));
                ui.add(egui::Slider::new(&mut self.settings.text, 0.8..=2.0).step_by(0.1));
                ui.end_row();
                ui.label(t(lang, "memory"));
                ui.label(RichText::new(self.memory.display().to_string()).monospace());
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new(t(lang, "layout")).strong());
            ui.label(t(lang, "layout_note"));
            ui.label(format!("{} {:.2}", t(lang, "harmony"), self.layout.sizes.harmony));
            for s in self.layout.statements() {
                ui.label(RichText::new(s).monospace().small());
            }
            if ui.button(t(lang, "reset_layout")).clicked() {
                reset = true;
            }
        });
        if reset {
            self.layout.reset();
        }
        self.settings_open = open;
        if self.settings != self.saved {
            self.journey.record(crate::journey::Change::Settings(Box::new(self.saved.clone())), t(lang, "settings"));
            apply_look(ctx, &self.settings);
            self.settings.save();
            self.saved = self.settings.clone();
        }
    }
}

pub fn voice_kind(name: &str) -> grm_sound::ambient::Kind {
    match name {
        "engine" => grm_sound::ambient::Kind::Engine,
        "wind" => grm_sound::ambient::Kind::Wind,
        _ => grm_sound::ambient::Kind::Pad,
    }
}

fn apply_look(ctx: &egui::Context, s: &Settings) {
    crate::style::apply(ctx, &s.palette, s.typewriter, s.text);
}

/// The pictures an eye made, as textures.
fn textures(ctx: &egui::Context, name: &str, sight: &Sight) -> Vec<(String, egui::TextureHandle)> {
    let mut out = vec![];
    let mut add = |tag: &str, p: &grm_engine::vision::Picture| {
        let mut pixels = Vec::with_capacity(p.width * p.height);
        for y in 0..p.height {
            for x in 0..p.width {
                let c = |ch: usize| {
                    let v = p.at(x, y, ch.min(p.channels - 1));
                    if v.is_finite() {
                        (v.clamp(0.0, 1.0) * 255.0) as u8
                    } else {
                        0
                    }
                };
                pixels.push(Color32::from_rgb(c(0), c(1), c(2)));
            }
        }
        let image = egui::ColorImage {
            size: [p.width, p.height],
            pixels,
            source_size: egui::vec2(p.width as f32, p.height as f32),
        };
        out.push((tag.to_string(), ctx.load_texture(format!("{name}-{tag}"), image, egui::TextureOptions::NEAREST)));
    };
    add("clean", &sight.clean);
    add("surprise", &grm_connect::sigma_picture(&sight.surprise));
    if let Some(c) = &sight.change {
        add("change", &grm_connect::sigma_picture(c));
    }
    out
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.answers(ctx);
        self.follow();
        self.files_dropped(ctx);
        self.picked();
        self.pull(ctx);
        self.script(ctx);
        self.keys(ctx);
        self.keep(ctx);
        if self.launched {
            if let Some(name) = self.current.clone() {
                self.launched_view(ctx, &name);
                self.windows(ctx);
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
        }
        let size = ctx.content_rect().size();
        let sizes = self.layout.window(size.x, size.y);
        let r = egui::TopBottomPanel::top("top").show(ctx, |ui| self.top(ui));
        self.areas.insert("top", r.response.rect);
        let r = egui::TopBottomPanel::bottom("prompt")
            .exact_height(sizes.bottom.min(size.y * 0.5))
            .show(ctx, |ui| self.prompt(ui));
        self.areas.insert("prompt", r.response.rect);
        let r = if sizes.stacked {
            egui::TopBottomPanel::bottom("side")
                .exact_height(sizes.band.min(size.y * 0.45))
                .show(ctx, |ui| crate::panels::show(self, ui))
        } else {
            egui::SidePanel::right("side")
                .exact_width(sizes.side)
                .resizable(false)
                .show(ctx, |ui| crate::panels::show(self, ui))
        };
        self.areas.insert("side", r.response.rect);
        // the code of the app shown, beside what it makes
        if let (Some(dock), Some(name)) = (self.code_dock, self.current.clone()) {
            if self.mode_of(&name) != Mode::Code && self.current_file.is_none() {
                let show = |app: &mut App, ui: &mut egui::Ui| crate::code::show(app, ui, &name);
                match dock {
                    Dock::Right => {
                        egui::SidePanel::right("code-dock")
                            .resizable(true)
                            .default_width(size.x * 0.32)
                            .show(ctx, |ui| show(self, ui));
                    }
                    Dock::Left => {
                        egui::SidePanel::left("code-dock")
                            .resizable(true)
                            .default_width(size.x * 0.32)
                            .show(ctx, |ui| show(self, ui));
                    }
                    Dock::Top => {
                        egui::TopBottomPanel::top("code-dock")
                            .resizable(true)
                            .default_height(size.y * 0.3)
                            .show(ctx, |ui| show(self, ui));
                    }
                    Dock::Bottom => {
                        egui::TopBottomPanel::bottom("code-dock")
                            .resizable(true)
                            .default_height(size.y * 0.3)
                            .show(ctx, |ui| show(self, ui));
                    }
                }
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.areas.insert("central", ui.max_rect());
            self.central(ui);
            // the edges: dragging one is a wish the layout network weighs
            let r = ui.max_rect();
            if !sizes.stacked {
                let edge = egui::Rect::from_x_y_ranges(r.right() - 3.0..=r.right() + 6.0, r.y_range());
                let e = ui.interact(edge, egui::Id::new("side-edge"), egui::Sense::drag());
                if e.hovered() || e.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if e.dragged() {
                    let dx = e.drag_delta().x;
                    self.layout.wish_side(self.layout.sizes.side - dx);
                }
            }
            let edge = egui::Rect::from_x_y_ranges(r.x_range(), r.bottom() - 3.0..=r.bottom() + 6.0);
            let e = ui.interact(edge, egui::Id::new("bottom-edge"), egui::Sense::drag());
            if e.hovered() || e.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if e.dragged() && !sizes.stacked {
                let dy = e.drag_delta().y;
                self.layout.wish_bottom(self.layout.sizes.bottom - dy);
            }
        });
        self.windows(ctx);
        crate::files::columns(self, ctx);
        crate::tour::show(self, ctx);
        // repaint: often while something moves, rarely when all is at rest (no busy loop, no flicker)
        let moving = self.graph.motion > 0.05 || self.worker.waiting > 0;
        ctx.request_repaint_after(Duration::from_millis(if moving { 16 } else { 200 }));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.worker.stop();
        if let Ok(mut m) = self.mixer.lock() {
            m.panic();
        }
        if self.fresh {
            return;
        }
        // it falls asleep where it woke (the engine gives its world back once nothing else holds it)
        let engine = std::mem::replace(
            &mut self.engine,
            Arc::new(Engine::start(grm_engine::world::World::new(), Duration::from_millis(50))),
        );
        if let Ok(engine) = Arc::try_unwrap(engine) {
            let world = engine.stop();
            if let Err(e) = world.sleep(&self.memory) {
                eprintln!("{e}");
            }
        }
    }
}

impl App {
    /// The middle of the window: the desk or an app, as code, network or surface.
    fn central(&mut self, ui: &mut egui::Ui) {
        let ink = Ink::of(ui.ctx());
        let lang = self.settings.lang;
        // the tabs: the desk, each app that runs, and what can be opened
        ui.horizontal_wrapped(|ui| {
            let desk_on = self.current.is_none() && self.current_file.is_none() && self.sandbox_shown.is_none();
            if crate::icons::toggle(ui, "layout-grid", t(lang, "desk"), desk_on).clicked() {
                self.current = None;
                self.current_file = None;
                self.sandbox_shown = None;
                self.editing = false;
            }
            for a in self.apps.clone() {
                let on = self.current.as_deref() == Some(a.name.as_str())
                    && self.current_file.is_none()
                    && self.sandbox_shown.is_none();
                let sign = a.icon.clone().filter(|i| crate::icons::exists(i)).unwrap_or("app-window".into());
                let colour = if a.errors.is_empty() { ui.visuals().text_color() } else { ink.amber };
                let r = ui
                    .add(
                        egui::Button::image_and_text(
                            crate::icons::image(&sign, 15.0, colour),
                            RichText::new(&a.title).color(colour),
                        )
                        .selected(on),
                    )
                    .on_hover_text(if a.errors.is_empty() {
                        a.path.display().to_string()
                    } else {
                        format!("{}\n{} {}", a.path.display(), a.errors.len(), t(lang, "errors_in_app"))
                    });
                if r.clicked() {
                    self.current = Some(a.name.clone());
                    self.current_file = None;
                    self.sandbox_shown = None;
                }
                r.context_menu(|ui| {
                    if ui.button(t(lang, "close_app")).clicked() {
                        self.say(format!("app close {}", a.name));
                        ui.close();
                    }
                });
            }
            for k in 0..self.files.len() {
                let on = self.current_file == Some(k);
                let mut title = self.files[k].name();
                if self.files[k].dirty() {
                    title.push_str(" •");
                }
                let sign = if self.files[k].table.is_some() {
                    "sheet"
                } else if self.files[k].path.extension().is_some_and(|x| x == "grm") {
                    "file-code"
                } else {
                    "file-text"
                };
                if crate::icons::toggle(ui, sign, title, on)
                    .on_hover_text(self.files[k].path.display().to_string())
                    .clicked()
                {
                    self.current_file = Some(k);
                    self.sandbox_shown = None;
                }
            }
            let plus = egui::Button::image(crate::icons::image("plus", 16.0, ui.visuals().text_color()));
            egui::containers::menu::MenuButton::from_button(plus).ui(ui, |ui| {
                if crate::menu::item(ui, "circle-plus", t(lang, "new_app_title"), t(lang, "new_app_d")) {
                    self.new_app_open = true;
                    self.new_app_focus = true;
                    ui.close();
                }
                if crate::menu::item(ui, "folder-open", t(lang, "open_files"), t(lang, "open_files_d")) {
                    self.pick(Pick::Open);
                    ui.close();
                }
                ui.separator();
                ui.label(RichText::new(t(lang, "open_app")).strong());
                for f in self.app_files.clone() {
                    let n = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if self.apps.iter().any(|a| a.name == n) {
                        continue;
                    }
                    if crate::icons::button(ui, "app-window", &n).on_hover_text(f.display().to_string()).clicked() {
                        self.say(format!("app open {}", f.display()));
                        ui.close();
                    }
                }
            });
        });
        self.areas.insert("tabs", ui.min_rect());
        if let Some(k) = self.current_file {
            if k < self.files.len() {
                ui.separator();
                crate::files::show(self, ui, k);
                return;
            }
            self.current_file = None;
        }
        if let Some(sb) = self.sandbox_shown.clone() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} {sb}", t(lang, "sandbox_shown"))).strong().color(ink.accent));
                for (key, word) in [("run", "run"), ("trial", "trial"), ("merge", "merge"), ("refuse", "refuse")] {
                    if ui.button(t(lang, key)).clicked() {
                        self.say(format!("{word} {sb}"));
                    }
                }
                if ui.button(t(lang, "back")).clicked() {
                    self.sandbox_shown = None;
                }
            });
            ui.separator();
            let name = sb.clone();
            if let Some(p) = self.sandbox_pull.poll(&self.engine, move |w| Picture::of_sandbox(w, &name)) {
                self.sandbox_picture = p;
            }
            match self.sandbox_picture.clone() {
                Some(p) => {
                    let saved = std::mem::replace(&mut self.picture, p);
                    crate::view::show(self, ui, None);
                    self.picture = saved;
                }
                None => {
                    ui.label(t(lang, "no_sandbox"));
                }
            }
            return;
        }
        let Some(name) = self.current.clone() else {
            ui.horizontal(|ui| {
                for (m, key) in [(Mode::Network, "network"), (Mode::Plot, "plots")] {
                    if crate::style::chosen(ui, self.desk_mode == m, t(lang, key)).clicked() {
                        self.desk_mode = m;
                    }
                }
            });
            ui.separator();
            match self.desk_mode {
                Mode::Plot => crate::plot::show(self, ui, None),
                _ => crate::view::show(self, ui, None),
            }
            return;
        };
        let Some(view) = self.apps.iter().find(|a| a.name == name).cloned() else { return };
        // the app's bar: its three views, launch, edit, x-ray, undo
        ui.horizontal_wrapped(|ui| {
            let mut mode = self.mode_of(&name);
            for (m, key, keys) in
                [(Mode::Code, "code", ""), (Mode::Network, "network", ""), (Mode::App, "app_view", "")]
            {
                let _ = keys;
                ui.selectable_value(&mut mode, m, t(lang, key));
            }
            self.modes.insert(name.clone(), mode);
            ui.separator();
            if crate::icons::button(ui, "maximize", t(lang, "launch")).on_hover_text("F5").clicked() {
                self.launched = true;
                self.modes.insert(name.clone(), Mode::App);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            }
            let docked = self.code_dock.is_some();
            if crate::icons::toggle(ui, "code", t(lang, "code_beside"), docked).on_hover_text("Ctrl+K").clicked() {
                self.code_dock = if docked { None } else { Some(self.settings.dock) };
            }
            if docked {
                egui::ComboBox::from_id_salt("dock-where")
                    .selected_text(t(lang, dock_key(self.settings.dock)))
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for d in [Dock::Right, Dock::Left, Dock::Top, Dock::Bottom] {
                            if ui.selectable_label(self.settings.dock == d, t(lang, dock_key(d))).clicked() {
                                self.settings.dock = d;
                                self.code_dock = Some(d);
                            }
                        }
                    });
            }
            let edit = ui
                .add(
                    egui::Button::new(RichText::new(t(lang, "edit")).color(if self.editing {
                        ink.amber
                    } else {
                        ink.text
                    }))
                    .selected(self.editing),
                )
                .on_hover_text("Ctrl+E");
            if edit.clicked() {
                self.editing = !self.editing;
                self.modes.insert(name.clone(), Mode::App);
            }
            if ui.add(egui::Button::new(t(lang, "xray")).selected(self.xray)).on_hover_text("Ctrl+R").clicked() {
                self.xray = !self.xray;
            }
            if !view.prefix.is_empty() {
                ui.label(RichText::new(t(lang, "own_copy")).small().color(ink.amber)).on_hover_text(
                    view.prefix.iter().map(|(k, v)| format!("{k} → {v}")).collect::<Vec<_>>().join("\n"),
                );
            }
            if !view.apis.is_empty() {
                ui.label(RichText::new(format!("api: {}", view.apis.join(", "))).small().color(ink.muted));
            }
        });
        ui.separator();
        match self.mode_of(&name) {
            Mode::Code => crate::code::show(self, ui, &name),
            Mode::Network => crate::view::show(self, ui, Some(&grm_shell::runtime::source_of(&name))),
            Mode::Plot => crate::plot::show(self, ui, Some(&grm_shell::runtime::source_of(&name))),
            Mode::App => {
                if self.editing {
                    egui::Frame::new().fill(ink.amber.gamma_multiply(0.12)).inner_margin(6.0).show(ui, |ui| {
                        ui.label(RichText::new(t(lang, "editing_note")).color(ink.amber));
                        crate::surface::edit_panel(self, ui, &name);
                    });
                }
                egui::ScrollArea::vertical().id_salt(("surface", &name)).auto_shrink([false, false]).show(ui, |ui| {
                    crate::surface::show(self, ui, &name);
                });
            }
        }
    }

    /// The app alone in the window: its surface (Esc or F5: back to the desk).
    fn launched_view(&mut self, ctx: &egui::Context, name: &str) {
        let ink = Ink::of(ctx);
        let lang = self.settings.lang;
        let title = self.apps.iter().find(|a| a.name == name).map(|a| a.title.clone()).unwrap_or_default();
        egui::TopBottomPanel::top("launched").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&title).strong());
                ui.label(RichText::new(t(lang, "launched_note")).small().color(ink.muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::button(ui, "arrow-left", t(lang, "back")).clicked() {
                        self.launched = false;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    }
                    if ui.add(egui::Button::new(t(lang, "xray")).selected(self.xray)).clicked() {
                        self.xray = !self.xray;
                    }
                    let edit = ui.add(
                        egui::Button::new(RichText::new(t(lang, "edit")).color(if self.editing {
                            ink.amber
                        } else {
                            ink.text
                        }))
                        .selected(self.editing),
                    );
                    if edit.clicked() {
                        self.editing = !self.editing;
                    }
                    let sounding = self.mixer.lock().map(|m| m.ambient_on || !m.playing().is_empty()).unwrap_or(false);
                    if sounding && ui.button(RichText::new(t(lang, "panic")).color(ink.amber)).clicked() {
                        self.say("panic");
                    }
                });
            });
            if self.editing {
                crate::surface::edit_panel(self, ui, name);
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt(("launched", name)).auto_shrink([false, false]).show(ui, |ui| {
                crate::surface::show(self, ui, name);
            });
        });
    }

    /// The small window where a line is written before it is said (a relation drawn, a relation edited).
    fn line_window(&mut self, ctx: &egui::Context) {
        let Some((app, line, mut text)) = self.line_draft.clone() else { return };
        let lang = self.settings.lang;
        let mut open = true;
        let mut done = false;
        egui::Window::new(t(lang, "line_window")).open(&mut open).collapsible(false).default_width(480.0).show(
            ctx,
            |ui| {
                ui.label(match &app {
                    Some(a) => format!("{} {a}", t(lang, "goes_to_app")),
                    None => t(lang, "goes_to_desk").to_string(),
                });
                let r = ui.add(
                    egui::TextEdit::singleline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY),
                );
                r.request_focus();
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(t(lang, "apply")).clicked() || enter {
                        done = true;
                    }
                    if ui.button(t(lang, "cancel")).clicked() {
                        self.line_draft = None;
                    }
                });
            },
        );
        if !open {
            self.line_draft = None;
            return;
        }
        if self.line_draft.is_none() {
            return;
        }
        if done {
            self.line_draft = None;
            match app {
                Some(a) => {
                    let Some(v) = self.apps.iter().find(|x| x.name == a) else { return };
                    let new = match line {
                        Some(l) => grm_shell::apps::with_line(&v.text, l, Some(text.trim())),
                        None => grm_shell::apps::with_added(&v.text, text.trim()),
                    };
                    self.edit_app_text(&a, new);
                }
                None => self.say(text.trim().to_string()),
            }
        } else {
            self.line_draft = Some((app, line, text));
        }
    }
}

impl App {
    /// The window that makes a new app: its name, and Enter.
    fn new_app_window(&mut self, ctx: &egui::Context) {
        if !self.new_app_open {
            return;
        }
        let lang = self.settings.lang;
        let mut open = true;
        let mut made = false;
        egui::Window::new(t(lang, "new_app_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.label(t(lang, "new_app_d"));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.new_app)
                        .hint_text(t(lang, "new_app"))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(260.0),
                );
                if self.new_app_focus {
                    r.request_focus();
                    self.new_app_focus = false;
                }
                let name = self.new_app.trim().to_string();
                let fine = grm_engine::lang::is_name(&name);
                if !name.is_empty() && !fine {
                    ui.label(RichText::new(t(lang, "name_rule")).small().color(Ink::of(ui.ctx()).amber));
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    if crate::icons::button(ui, "circle-plus", t(lang, "create")).clicked() || enter {
                        made = fine;
                    }
                });
            });
        if made {
            let n = std::mem::take(&mut self.new_app);
            self.say(format!("app new {}", n.trim()));
            self.new_app_open = false;
        } else if !open {
            self.new_app_open = false;
        }
    }

    /// Asks the system's own window for files (it answers later: the window goes on meanwhile).
    pub fn pick(&mut self, what: Pick) {
        if self.picking.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let here = std::env::current_dir().unwrap_or_default();
        let title = t(self.settings.lang, "open_files").to_string();
        let _ = std::thread::Builder::new().name("grm-files".into()).spawn(move || {
            let start = Instant::now();
            let dialog = rfd::FileDialog::new().set_directory(here).set_title(title);
            let chosen = match what {
                Pick::Eye => dialog.add_filter("PNG", &["png"]).pick_file().map(|f| vec![f]),
                Pick::Open => dialog.pick_files(),
            };
            let _ = tx.send((chosen.unwrap_or_default(), start.elapsed()));
        });
        self.picking = Some((what, rx));
    }

    fn picked(&mut self) {
        let Some((what, rx)) = &self.picking else { return };
        let what = *what;
        match rx.try_recv() {
            Ok((files, took)) => {
                self.picking = None;
                // nothing, at once: the system has no window for it (no file portal on this desktop)
                if files.is_empty() && took < Duration::from_millis(400) {
                    self.log.push((LineKind::Warning, self.tr("no_file_window").to_string()));
                }
                match what {
                    Pick::Eye => {
                        if let Some(f) = files.first() {
                            self.eye_path = f.display().to_string();
                            let line = format!("see {} {}", self.eye_path, self.eye_name.trim());
                            self.say(line);
                        }
                    }
                    Pick::Open => {
                        for f in files {
                            self.open_path(f);
                        }
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.picking = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Files dropped on the window: each opened as what it is. While they are held above it, it says so (quietly).
    fn files_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            self.open_path(p);
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let ink = Ink::of(ctx);
            let rect = ctx.content_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dropping")));
            painter.rect_filled(rect, 0.0, ink.surface.gamma_multiply(0.85));
            painter.rect_stroke(
                rect.shrink(12.0),
                12.0,
                egui::Stroke::new(2.0_f32, ink.accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                t(self.settings.lang, "drop_here"),
                egui::FontId::proportional(22.0),
                ink.text,
            );
        }
    }

    /// A file given to the machine (dropped, or chosen): opened as what it is.
    pub fn open_path(&mut self, p: PathBuf) {
        let shown = p.display().to_string();
        if shown.contains(char::is_whitespace) {
            self.log.push((LineKind::Warning, format!("{shown}: {}", self.tr("no_spaces"))));
            return;
        }
        if p.is_dir() {
            self.say(format!("cd {shown}"));
            return;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "png" => {
                self.eye_path = shown.clone();
                self.tab = Tab::Eye;
                let line = format!("see {shown} {}", self.eye_name.trim());
                self.say(line);
            }
            "grm" => {
                let app = std::fs::read_to_string(&p).is_ok_and(|text| {
                    text.lines().any(|l| {
                        let w = l.split_whitespace().next().unwrap_or("");
                        w == "app" || w == "widget"
                    })
                });
                if app {
                    self.say(format!("app open {shown}"));
                } else {
                    self.say(format!("edit {shown}"));
                }
            }
            "wav" | "mp3" | "flac" | "ogg" => {
                self.log.push((LineKind::Dim, format!("{shown}: {}", self.tr("sound_files_later"))));
            }
            _ => self.say(format!("edit {shown}")),
        }
    }
}
