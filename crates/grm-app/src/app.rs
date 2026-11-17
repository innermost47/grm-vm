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
    Insights,
}

pub const TABS: [(Tab, &str); 7] = [
    (Tab::Inspector, "inspector"),
    (Tab::Sandboxes, "sandboxes"),
    (Tab::Sources, "sources"),
    (Tab::Journal, "journal"),
    (Tab::Eye, "eye"),
    (Tab::Sound, "sound"),
    (Tab::Insights, "insights"),
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

#[derive(Clone)]
pub struct Eye {
    pub sight: Sight,
    pub textures: Vec<(String, egui::TextureHandle)>,
}

/// What is being recorded: when it began, and every node written (when, which, its value).
pub type Tape = (f64, Vec<(f64, String, f64)>);

/// A take played again: when it began, its notes (a take of before) and the next to play, the gates to close
/// (when, which), what was turned and played (seconds from the start, node, value) and the next.
pub struct Replay {
    pub start: Instant,
    pub notes: Vec<crate::notes::Note>,
    pub next: usize,
    pub offs: Vec<(f64, String)>,
    pub events: Vec<(f64, String, f64)>,
    pub next_event: usize,
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
    pub help_open: bool,
    pub help_page: usize,
    /// The world asked of Our World in Data: a place, and the categories left out.
    pub world_place: String,
    pub world_off: Vec<bool>,
    /// The node chosen in a workshop: (workshop, node).
    pub ws_selected: Option<(String, String)>,
    /// What is looked for in the list of networks.
    pub net_search: String,
    /// What was played lately (the roll, the keys), and the take being recorded.
    pub played: crate::notes::Played,
    /// The network laid out as a flow (inputs left, outputs right), and what passes through it now.
    pub flow: crate::flow::Flow,
    music_saved_at: Instant,
    /// What is recorded (when it began, and every node written by hand or by her, in time), while recording.
    pub tape: std::sync::Mutex<Option<Tape>>,
    /// Where the beat is: (when it was read, the phase in the beat 0 .. 1, seconds a beat, the beat in the bar 0 .. 3).
    pub beat_clock: Option<(f64, f64, f64, f64)>,
    /// The synthesiser whose voices the human's hands hold.
    hands_view: Option<String>,
    /// The gates kept open in the memory were let go, at the start.
    gates_closed: bool,
    /// Which voice the human holds, and since when: (the key, when).
    human_hands: [Option<(i32, f64)>; 4],
    /// How many times the sound ran away, as last said.
    fused_seen: u64,
    /// The studio's song (its tracks), and each strip's meter.
    pub song: crate::studio::Song,
    pub studio_vu: HashMap<String, f32>,
    /// What her network learned (its `memory` nodes), as last taken from the sound: given back when she wakes.
    pub elle_memory: indexmap::IndexMap<String, f64>,
    /// When her own network was last written (what she feels, in it).
    elle_at: f64,
    /// Her note sounding now (when it began, which), drawn on the roll when it ends.
    elle_note: Option<(f64, i32)>,
    /// A take played again: (when it began, its notes, the next to play, the notes to let go: when, which voice).
    pub replay: Option<Replay>,
    started: Instant,
    /// A node in hand: the range of its slider (chosen once) and the value being moved.
    pub hand_range: std::collections::HashMap<String, (f64, f64)>,
    pub hand_value: std::collections::HashMap<String, f64>,
    /// The MIDI controllers listened to, what is being learned, the keys held (by gate).
    pub midi: crate::midi::Midi,
    pub midi_learn: Option<crate::midi::Target>,
    midi_held: std::collections::HashMap<String, Vec<u8>>,
    /// The last values sent to each node by MIDI (what changed goes once a frame).
    midi_sent: std::collections::HashMap<String, f64>,
    pub help_search: String,
    pub settings_open: bool,
    focus_prompt: bool,
    pub eyes: IndexMap<String, Eye>,
    pub eye_path: String,
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
    /// The network the view went into (double click): only it is shown.
    pub dived: Option<String>,
    /// Zones: one in place of the space, one on the whole screen, some in windows of their own (each with its own
    /// way of being looked at).
    pub zone_center: Option<crate::space::Kind>,
    pub zone_screen: Option<crate::space::Kind>,
    pub detached: Vec<crate::space::Kind>,
    pub zone_views: HashMap<String, crate::view::View>,
    /// The workshops' windows: what each keeps, and its picture.
    pub workshop_state: HashMap<String, crate::workshop::State>,
    pub workshop_pulls: HashMap<String, (Pull<Option<Picture>>, Option<Picture>)>,
    /// A workshop being drawn: what the mouse makes is written there.
    pub write_into: Option<String>,
    /// A workshop a table is being chosen for.
    pub workshop_loading: Option<String>,
    /// The airlock, open.
    pub airlock: Option<crate::airlock::Airlock>,
    /// The pictures given to the eye and not seen yet (the lines said).
    pub seeing: Vec<String>,
    /// A picture of an eye shown big: (eye, which), the arrows go through the others.
    pub lightbox: Option<(String, usize)>,
    pub new_workshop_open: bool,
    new_workshop: String,
    /// The space (a table where the human puts what they want), and whether it is shown.
    pub space: crate::space::Space,
    pub space_on: bool,
    space_ready: bool,
    /// The master equation of everything, and of what is shown.
    pub master: (Option<Said>, Option<Said>),
    master_pull: Pull<Masters>,
    /// Each network's master equation (the inspector draws it, and its curve).
    pub masters: indexmap::IndexMap<String, grm_engine::master::Master>,
    /// The inspector's tree: what is unfolded (by its path), what shows its detail (by what it is), and where
    /// the list must go.
    pub tree_open: std::collections::HashSet<String>,
    pub tree_detail: std::collections::HashSet<String>,
    pub tree_scroll: Option<String>,
    /// How the desk is shown (its network, or plots).
    pub desk_mode: Mode,
    /// Files opened to write in, and the one shown.
    pub files: Vec<crate::files::FileTab>,
    pub current_file: Option<usize>,
    /// A table waiting for its columns to be chosen: (file, names, as written, chosen).
    pub columns: Option<Columns>,
    /// A table whose columns would land on nodes holding other data: the names proposed instead.
    pub collide: Option<crate::files::Collide>,
    /// The guided visit: its step.
    pub touring: Option<usize>,
    /// Where each part of the window is (for the visit).
    pub areas: HashMap<&'static str, egui::Rect>,
    /// The code of the app shown, beside its network or surface: where (None: not shown).
    pub code_dock: Option<Dock>,
    /// A sandbox shown in the middle, as a network.
    pub sandbox_shown: Option<String>,
    sandbox_picture: Option<Picture>,
    /// A workshop shown in the middle (a network of data made apart).
    pub workshop_shown: Option<String>,
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
    /// The side (the inspector...) unfolded, or folded into a rail of signs; the prompt's lines shown, or only
    /// its line.
    pub side_open: bool,
    pub prompt_open: bool,
    /// What went wrong, said softly in a corner for a few seconds (and kept in the prompt's lines).
    toasts: Vec<(LineKind, String, Instant)>,
    /// The window was full screen when it was left.
    fullscreen_back: bool,
    /// The window was maximised: it is again, at the first frame.
    maximized_back: bool,
    /// A removal asked for by a click: the window asks (yes or no), not the prompt.
    pub confirm: Option<Confirm>,
    /// The line whose question was already answered yes in such a window.
    quiet_yes: Option<String>,
    /// When the machine was last put to sleep (its sign shows it was done, for a while).
    slept: Option<Instant>,
    /// What her consciousness said last, and when (the window remembers it, not the witness: it keeps nothing).
    spoken: Option<(String, Instant)>,
    /// The colours' breath: when it last moved, how loud the sound was lately, since when the window lives.
    breath_at: Instant,
    breath_sound: f32,
    breathing: bool,
    /// The window that makes a new app: open, and whether its field still has to take the keys.
    new_app_open: bool,
    new_app_focus: bool,
    /// Files being chosen in the system's window (it answers when the human has chosen).
    picking: Option<(Pick, Chosen)>,
}

/// The files chosen in the system's window, and how long it took to answer (when it comes).
type Chosen = std::sync::mpsc::Receiver<(Vec<PathBuf>, Duration)>;

/// A master equation as it is said: the equation, and what it gathers.
pub type Said = (String, String);

/// The master equation of everything, of what is shown, and of each network.
type Masters = (Option<Said>, Option<Said>, indexmap::IndexMap<String, grm_engine::master::Master>);

/// A question asked in a window before something is taken away: what would go, and the line that does it.
#[derive(Debug, Clone)]
pub struct Confirm {
    pub title: String,
    pub detail: Vec<String>,
    pub line: String,
    /// To the trash (it can be given back): softer words, and a toast once it is done.
    pub trash: bool,
}

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
        // the studio's file, written from its song before anything opens it (the desk opens it again at once: it
        // must find this version, not the one of before)
        {
            let mut song = crate::studio::Song::load(&memory);
            let human = song.human(&settings.human);
            song.save(&memory);
            let dir = grm_shell::runtime::apps_dir(&memory);
            let path = dir.join(format!("{}.grm", crate::studio::APP));
            let text = song.grm(human);
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, text));
            }
        }
        let mut shell = Shell::new(memory.clone(), mixer.clone());
        shell.keep_desk = !(args.fresh || args.screenshot.is_some());
        shell.human = Some(settings.human.clone()).filter(|h| !h.trim().is_empty());
        shell.french = settings.lang == Lang::Fr;
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
            Some("insights") => Tab::Insights,
            _ => Tab::Inspector,
        };
        apply_look(&cc.egui_ctx, &settings);
        // her music, as she left it (her ear, her tastes, her lessons, her mood)
        let kept = if args.fresh { None } else { crate::notes::Kept::load(&memory) }.unwrap_or_default();
        let memory_for_song = memory.clone();
        let kept_elle = kept.elle.clone();
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
            help_page: 0,
            world_place: String::new(),
            world_off: vec![],
            ws_selected: None,
            net_search: String::new(),
            played: Default::default(),
            flow: Default::default(),
            music_saved_at: Instant::now(),
            tape: std::sync::Mutex::new(None),
            human_hands: [None; 4],
            hands_view: None,
            beat_clock: None,
            gates_closed: false,
            fused_seen: 0,
            song: crate::studio::Song::load(&memory_for_song),
            studio_vu: HashMap::new(),
            elle_memory: kept_elle,
            elle_at: 0.0,
            elle_note: None,
            replay: None,
            started: Instant::now(),
            hand_range: Default::default(),
            hand_value: Default::default(),
            midi: crate::midi::Midi::start(),
            midi_learn: None,
            midi_held: Default::default(),
            midi_sent: Default::default(),
            help_search: String::new(),
            settings_open: false,
            focus_prompt: true,
            eyes: IndexMap::new(),
            eye_path: String::new(),
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
            workshop_shown: None,
            sandbox_pull: Pull::new(Duration::from_millis(300)),
            looks: HashMap::new(),
            apps_known: false,
            wanted_app: args.app.clone(),
            window_saved_at: Instant::now(),
            world_saved_at: Instant::now(),
            journey: Default::default(),
            dived: None,
            zone_center: None,
            zone_screen: None,
            detached: vec![],
            zone_views: HashMap::new(),
            workshop_state: HashMap::new(),
            workshop_pulls: HashMap::new(),
            write_into: None,
            workshop_loading: None,
            airlock: None,
            lightbox: None,
            seeing: vec![],
            new_workshop_open: false,
            new_workshop: String::new(),
            space: Default::default(),
            space_on: true,
            space_ready: false,
            masters: Default::default(),
            tree_open: Default::default(),
            tree_detail: Default::default(),
            tree_scroll: None,
            collide: None,
            side_open: true,
            prompt_open: true,
            toasts: vec![],
            fullscreen_back: false,
            maximized_back: false,
            confirm: None,
            quiet_yes: None,
            slept: None,
            spoken: None,
            breath_at: Instant::now(),
            breath_sound: 0.0,
            breathing: false,
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
        self.side_open = w.side_open;
        self.space_on = w.space_on;
        self.zone_center = w.zone_center.as_deref().and_then(crate::space::kind_of);
        self.detached = w.detached.iter().filter_map(|d| crate::space::kind_of(d)).collect();
        self.prompt_open = w.prompt_open;
        self.fullscreen_back = w.fullscreen || w.launched;
        self.maximized_back = w.maximized;
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
            side_open: self.side_open,
            space_on: self.space_on,
            zone_center: self.zone_center.as_ref().map(|k| k.word()),
            detached: self.detached.iter().map(|k| k.word()).collect(),
            prompt_open: self.prompt_open,
            fullscreen: ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)),
            maximized: ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
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
        if closing || self.music_saved_at.elapsed() > Duration::from_secs(30) {
            self.music_saved_at = Instant::now();
            self.keep_music();
        }
        if self.world_saved_at.elapsed() > Duration::from_secs(60) {
            self.world_saved_at = Instant::now();
            let path = self.memory.clone();
            self.engine.send(move |w| w.sleep(&path).map(|_| String::new()));
        }
    }

    /// Where an app first puts a node (`input NAME = value` in its file), for a knob double-clicked.
    pub fn default_of(&self, node: &str) -> Option<f64> {
        for view in &self.apps {
            let original =
                view.prefix.iter().find(|(_, v)| v.as_str() == node).map(|(k, _)| k.as_str()).unwrap_or(node);
            for line in view.text.lines() {
                let mut words = line.split_whitespace();
                let (Some(kind), Some(name), Some("="), Some(value)) =
                    (words.next(), words.next(), words.next(), words.next())
                else {
                    continue;
                };
                if ["input", "node", "var", "output"].contains(&kind) && name == original {
                    if let Ok(v) = value.parse::<f64>() {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    /// Every voice nobody holds is let go, on every synthesiser (a gate left open in the memory, or by a note that
    /// was never let go, would sound for ever).
    pub fn close_gates(&mut self) {
        for view in self.synth_names() {
            for (k, (_, gate)) in self.voices_in(&view).iter().enumerate() {
                let held = self.hands_view.as_deref() == Some(view.as_str()) && self.human_hands[k].is_some();
                if !held && self.picture.node(gate).is_some_and(|n| n.value != 0.0) {
                    self.by_ear(gate, 0.0);
                }
            }
        }
    }

    pub fn reset_sound(&mut self) {
        self.close_gates();
        if let Ok(mut m) = self.mixer.lock() {
            m.reset();
        }
        self.toast(self.tr("reset_sound").to_string());
    }

    /// A node written by the window (a control turned), as the surface does it.
    pub fn write(&self, node: &str, v: f64) {
        crate::surface::observe(self, node, v);
    }

    /// The studio's file written again from its song, and opened again (its sound goes on).
    pub fn studio_rewrite(&mut self) {
        let human = self.song.human(&self.settings.human.clone());
        self.song.save(&self.memory);
        let dir = grm_shell::runtime::apps_dir(&self.memory);
        let path = dir.join(format!("{}.grm", crate::studio::APP));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, self.song.grm(human))) {
            self.toast(format!("{}: {e}", path.display()));
            return;
        }
        self.say(format!("app open {}", path.display()));
    }

    /// A sound that ran away and started again by itself is said (once each time).
    fn fuses(&mut self) {
        let fused = self.mixer.lock().map(|m| m.fused()).unwrap_or(0);
        if fused < self.fused_seen {
            // a channel went: its count with it
            self.fused_seen = fused;
        }
        if fused > self.fused_seen {
            self.fused_seen = fused;
            self.toast(self.tr("sound_ran_away").to_string());
        }
    }

    /// Her music, kept beside the memory (what she learned stays between sessions).
    pub fn keep_music(&self) {
        let kept = crate::notes::Kept { elle: self.elle_memory.clone() };
        if let Err(e) = kept.save(&self.memory) {
            eprintln!("{e}");
        }
    }

    /// Where her takes are kept (MIDI files, and their notes to play them again).
    pub fn takes_dir(&self) -> std::path::PathBuf {
        self.memory.with_extension("takes")
    }

    /// The takes recorded, newest first.
    pub fn takes(&self) -> Vec<std::path::PathBuf> {
        let mut v: Vec<_> = std::fs::read_dir(self.takes_dir())
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
            .unwrap_or_default();
        v.sort();
        v.reverse();
        v
    }

    /// Recording starts, or stops and is written: a MIDI file of the notes, and the take itself (every note and
    /// every knob turned, by hand or by her, in time) to play again.
    pub fn record(&mut self) {
        match self.played.recording.take() {
            None => {
                self.played.recording = Some(vec![]);
                if let Ok(mut t) = self.tape.lock() {
                    *t = Some((self.ear_now(), vec![]));
                }
            }
            Some(notes) => {
                let events = self
                    .tape
                    .lock()
                    .ok()
                    .and_then(|mut t| t.take())
                    .map(|(t0, e)| e.into_iter().map(|(t, n, v)| (t - t0, n, v)).collect::<Vec<_>>());
                let events = events.unwrap_or_default();
                if notes.is_empty() && events.is_empty() {
                    return;
                }
                let dir = self.takes_dir();
                let _ = std::fs::create_dir_all(&dir);
                let n = self.takes().len() + 1;
                let bpm = 60.0 / self.beat().max(1e-3);
                let base = dir.join(format!("take-{n:03}"));
                let mid = crate::notes::midi(&notes, bpm);
                let take = crate::notes::Take { notes, events };
                let ok = std::fs::write(base.with_extension("mid"), mid).is_ok()
                    && std::fs::write(base.with_extension("json"), serde_json::to_string(&take).unwrap_or_default())
                        .is_ok();
                if ok {
                    self.toast(format!("{}: {}", self.tr("take_saved"), base.with_extension("mid").display()));
                }
            }
        }
    }

    /// For how long it has been recording (seconds), when it is.
    pub fn recording_for(&self) -> Option<f64> {
        let t0 = self.tape.lock().ok()?.as_ref()?.0;
        Some(self.ear_now() - t0)
    }

    /// A node written (a key, a note, a knob) goes straight to the sound too: heard at the next block, not when the
    /// machine next tells it (a short note, an arpeggio, would be over before).
    pub fn sound_now(&self, node: &str, v: f64) {
        if let Ok(mut m) = self.mixer.lock() {
            m.set_now(node, v);
        }
    }

    /// A node written (by hand or by her), kept on the tape while recording.
    pub fn taped(&self, node: &str, v: f64) {
        if let Ok(mut t) = self.tape.lock() {
            if let Some((_, events)) = t.as_mut() {
                events.push((self.started.elapsed().as_secs_f64(), node.to_string(), v));
            }
        }
    }

    /// A take played again: what was turned and played, in time (a take of before, its notes alone).
    pub fn play_take(&mut self, path: &std::path::Path) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let take: crate::notes::Take = serde_json::from_str(&text)
            .or_else(|_| {
                serde_json::from_str::<Vec<crate::notes::Note>>(&text)
                    .map(|notes| crate::notes::Take { notes, events: vec![] })
            })
            .unwrap_or_default();
        let notes = if take.events.is_empty() {
            let t0 = take.notes.iter().map(|n| n.0).fold(f64::MAX, f64::min);
            take.notes.into_iter().map(|(a, l, n, v)| (a - t0, l, n, v)).collect()
        } else {
            vec![]
        };
        self.replay =
            Some(Replay { start: Instant::now(), notes, next: 0, offs: vec![], events: take.events, next_event: 0 });
    }

    /// Seconds a beat, as the instrument's tempo says.
    fn beat(&self) -> f64 {
        let bpm = self.picture.node(&self.global("bpm")).map(|n| n.value);
        60.0 / bpm.or_else(|| self.inst_value("bpm")).unwrap_or(104.0).max(10.0)
    }

    /// A node's value on the instrument she plays, by the name its file gives it.
    pub fn inst_value(&self, short: &str) -> Option<f64> {
        self.picture.node(&self.named(short)).map(|n| n.value)
    }

    /// The synthesisers, each by a key `app|prefix`: an app with a `by ear` switch is one (`analog|`); in the
    /// studio, each synthesiser track is one (`studio|t2_`).
    pub fn synth_names(&self) -> Vec<String> {
        let mut out = vec![];
        for a in &self.apps {
            if a.name == crate::studio::APP {
                for t in self.song.tracks.iter().filter(|t| t.kind == crate::studio::Kind::Synth) {
                    out.push(format!("{}|t{}_", a.name, t.id));
                }
            } else if a.widgets.iter().any(|w| w.nodes.iter().any(|n| n.ends_with("auto_on"))) {
                out.push(format!("{}|", a.name));
            }
        }
        out
    }

    /// An instrument's key, cut in two: its app, and the prefix of its nodes there.
    pub fn split_key(key: &str) -> (&str, &str) {
        key.split_once('|').unwrap_or((key, ""))
    }

    fn view_named(&self, key: &str) -> Option<&AppView> {
        let (name, _) = Self::split_key(key);
        self.apps.iter().find(|a| a.name == name)
    }

    /// A node of an instrument (`app|prefix`), by the name its file gives it.
    pub fn named_in(&self, key: &str, short: &str) -> String {
        let (_, pre) = Self::split_key(key);
        let full = format!("{pre}{short}");
        match self.view_named(key) {
            Some(a) => a.prefix.get(&full).cloned().unwrap_or(full),
            None => full,
        }
    }

    /// A node of the app the instrument is in, not its own (the studio's tempo, its sound).
    pub fn global_in(&self, key: &str, short: &str) -> String {
        let (name, _) = Self::split_key(key);
        self.named_in(&format!("{name}|"), short)
    }

    pub fn value_in(&self, key: &str, short: &str) -> Option<f64> {
        self.picture.node(&self.named_in(key, short)).map(|n| n.value)
    }

    /// Her own synthesiser (the one whose `hers` node is 1), when there is one.
    pub fn hers(&self) -> Option<String> {
        // its own `hers` node (a name not renamed may belong to another app: its source says whose it is)
        self.synth_names().into_iter().find(|k| {
            let src = format!("app:{}", Self::split_key(k).0);
            self.picture.node(&self.named_in(k, "hers")).is_some_and(|n| n.value > 0.5 && n.source == src)
        })
    }

    fn shown(&self, key: &str) -> bool {
        self.current.as_deref() == Some(Self::split_key(key).0)
    }

    /// The synthesiser she plays: hers when she has one; or the one shown; or the first there is.
    pub fn her_synth(&self) -> Option<String> {
        let all = self.synth_names();
        self.hers().or_else(|| all.iter().find(|a| self.shown(a)).cloned()).or_else(|| all.first().cloned())
    }

    /// The human's synthesiser: one that is not hers (the one shown first); with a single synthesiser, that one.
    pub fn human_synth(&self) -> Option<String> {
        let hers = self.hers();
        let all: Vec<String> = self.synth_names().into_iter().filter(|v| Some(v) != hers.as_ref()).collect();
        all.iter().find(|a| self.shown(a)).cloned().or_else(|| all.first().cloned())
    }

    /// The parts of every app she holds now: the nodes of each `hand` whose switch is on.
    pub fn her_hands(&self) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        for a in &self.apps {
            for h in &a.hands {
                if self.picture.node(&h.switch).is_some_and(|n| n.value > 0.5) {
                    out.extend(h.nodes.iter().cloned());
                }
            }
        }
        out
    }

    /// A node the human may not write: one of a part she holds (they see it move); on her own synthesiser of
    /// before (an app of its own), all but its loudness and its switch to play by ear.
    pub fn locked(&self, node: &str) -> bool {
        if self.her_hands().contains(node) {
            return true;
        }
        let Some(hers) = self.hers() else { return false };
        let (app, pre) = Self::split_key(&hers);
        if !pre.is_empty() {
            return false;
        }
        let src = format!("app:{app}");
        let mine = self.picture.node(node).is_some_and(|n| n.source == src);
        mine && !["volume", "auto_on"].iter().any(|s| self.named_in(&hers, s) == node)
    }

    /// The voices of a polyphonic synthesiser (pitch, gate): the first the keys' (or the sequencer's), the second
    /// the melody's, the third and fourth the chords'.
    fn voices_in(&self, view: &str) -> [(String, String); 4] {
        let n = |s: &str| self.named_in(view, s);
        [
            (n("pitch"), n("hold")),
            (n("lead_pitch"), n("lead_gate")),
            (n("c1_pitch"), n("c1_gate")),
            (n("c2_pitch"), n("c2_gate")),
        ]
    }

    /// The instrument she plays.
    fn instrument_view(&self) -> Option<&AppView> {
        self.her_synth().and_then(|n| self.view_named(&n))
    }

    fn instrument(&self) -> Option<(String, Option<(String, String)>)> {
        let key = self.her_synth()?;
        let (_, pre) = Self::split_key(&key);
        // her voices: the keys' nodes of her instrument (pitch, hold), whether a keyboard shows them or not
        let keys = Some((self.named_in(&key, "pitch"), self.named_in(&key, "hold")));
        Some((pre.to_string(), keys))
    }

    /// A node of the instrument she plays, by the name its file gives it (the window may have renamed it, when
    /// another app holds a node of that name: `pitch` becomes `analog__pitch`).
    pub fn named(&self, short: &str) -> String {
        match self.her_synth() {
            Some(k) => self.named_in(&k, short),
            None => short.to_string(),
        }
    }

    /// A node of the app her instrument is in (the studio's tempo, its sound, its transport).
    pub fn global(&self, short: &str) -> String {
        match self.her_synth() {
            Some(k) => self.global_in(&k, short),
            None => short.to_string(),
        }
    }

    /// Whether the keys playing this pitch node are a polyphonic instrument's (it has a `poly` node).
    pub fn is_poly(&self, pitch: &str) -> bool {
        self.apps.iter().any(|a| {
            let map = |n: &str| a.prefix.get(n).cloned().unwrap_or_else(|| n.to_string());
            a.widgets.iter().any(|w| {
                w.kind == grm_shell::apps::WidgetKind::Keys && w.nodes.first().is_some_and(|n| map(n) == pitch)
            }) && self.picture.node(&map("poly")).is_some()
        }) || pitch
            .strip_suffix("pitch")
            .is_some_and(|pre| !pre.is_empty() && self.picture.node(&format!("{pre}poly")).is_some())
    }

    /// The human's hands on a polyphonic instrument: each key held takes a voice, a free one first (one she is not
    /// playing: her chords before her melody, her melody before her bass), and gives it back when let go; while the
    /// human holds a voice, what she would play on it waits.
    fn hands(&mut self) {
        // once, when the instruments are known: what the memory kept open is let go (no note survives a session)
        if !self.gates_closed && self.instrument_view().is_some() && self.picture.node(&self.named("hold")).is_some() {
            self.gates_closed = true;
            self.close_gates();
        }
        let Some(pitch) = self.surface.poly.clone() else { return };
        // the synthesiser whose keys these are (never hers: only she plays it)
        let Some(view) = self.synth_names().into_iter().find(|v| self.named_in(v, "pitch") == pitch) else { return };
        if self.hers().as_deref() == Some(view.as_str()) {
            return;
        }
        if self.hands_view.as_deref() != Some(view.as_str()) {
            self.human_hands = [None; 4];
            self.hands_view = Some(view.clone());
        }
        let voices = self.voices_in(&view);
        // the arpeggiator on, the keys held are its
        let arp = self.value_in(&view, "arp_on").is_some_and(|v| v > 0.5);
        let held = if arp { Default::default() } else { self.surface.keys_held.clone() };
        let now = self.ear_now();
        // let go
        for (k, voice) in voices.iter().enumerate() {
            if let Some((m, at)) = self.human_hands[k] {
                if !held.contains(&m) {
                    self.by_ear(&voice.1, 0.0);
                    self.human_hands[k] = None;
                    self.played.played(at, (now - at).max(0.05), m - 33, 2);
                }
            }
        }
        // taken: on a single synthesiser shared with her, a free voice first, hers when none is
        let her = self.her_synth().as_deref() == Some(view.as_str())
            && self.value_in(&view, "auto_on").is_some_and(|v| v > 0.5);
        // the sequencer on, the first voice is its: a key held there would not be heard
        let seq = self.value_in(&view, "seq_on").is_some_and(|v| v > 0.5);
        let order: Vec<usize> = match (her, seq) {
            (true, true) => vec![2, 3, 1],
            (true, false) => vec![2, 3, 1, 0],
            (false, true) => vec![1, 2, 3],
            (false, false) => vec![0, 1, 2, 3],
        };
        for m in held {
            if self.human_hands.iter().any(|h| h.is_some_and(|(x, _)| x == m)) {
                continue;
            }
            let Some(k) = order.iter().copied().find(|k| self.human_hands[*k].is_none()) else { break };
            self.human_hands[k] = Some((m, now));
            let hz = 440.0 * 2f64.powf((m as f64 - 69.0) / 12.0);
            self.by_ear(&voices[k].0, (hz * 100.0).round() / 100.0);
            self.by_ear(&voices[k].1, 1.0);
        }
    }

    /// Where the beat is, read from the sound (the metronome's phase and its beat in the bar), for the roll's grid.
    fn clock(&mut self) {
        // one clock for both: the human's synthesiser's (hers follows its tempo)
        let Some(view) = self.human_synth().or_else(|| self.her_synth()) else { return };
        // the studio: its transport is the clock (a beat is four sixteenths)
        if !Self::split_key(&view).1.is_empty() {
            let (i, f) = match self.mixer.lock() {
                Ok(m) => (m.value("out", "tr_i"), m.value("out", "tr_f")),
                Err(_) => (None, None),
            };
            let beat = 60.0 / self.value_in(&view, "bpm").unwrap_or(120.0).max(10.0);
            let now = self.ear_now();
            let read = match (i, f) {
                (Some(i), Some(f)) => {
                    let i = i.round().clamp(0.0, 15.0);
                    Some((now, ((i % 4.0) + f.clamp(0.0, 1.0)) / 4.0, beat, (i / 4.0).floor()))
                }
                _ => None,
            };
            // the sound is read by blocks: what is read jumps a little ahead, a little behind. The clock goes on by
            // itself, steadily, and takes a new reading only when it has drifted (the grid does not tremble)
            self.beat_clock = match (self.beat_clock, read) {
                (Some((at, p, b, bar)), Some((_, p2, b2, _))) if (b - b2).abs() < 1e-6 => {
                    let predicted = (p + (now - at) / b.max(1e-3)).fract();
                    let gap = (p2 - predicted + 1.5).fract() - 0.5;
                    if gap.abs() < 0.25 {
                        // a small gap: corrected a little at a time (a twentieth each frame), never by a jump
                        let gone = p + (now - at) / b.max(1e-3);
                        let bar = (bar + gone.floor()).rem_euclid(4.0);
                        Some((now, (predicted + 0.05 * gap + 1.0).fract(), b, bar))
                    } else {
                        read
                    }
                }
                _ => read,
            };
            return;
        }
        // no transport: no beat
        self.beat_clock = None;
    }

    /// What her network plays, drawn on the piano roll (read from the sound: the window only shows it).
    fn elle_roll(&mut self) {
        let (gate, semi) = match self.mixer.lock() {
            Ok(m) => (m.value("out", "elle_gate"), m.value("out", "elle_semi")),
            Err(_) => return,
        };
        let (Some(gate), Some(semi)) = (gate, semi) else { return };
        let now = self.ear_now();
        let note = semi.round() as i32 + 12;
        match self.elle_note {
            Some((t0, n)) if gate < 0.5 || n != note => {
                self.played.roll.push_back((t0, now - t0, n, 4));
                while self.played.roll.front().is_some_and(|r| r.0 + r.1 < now - 20.0) {
                    self.played.roll.pop_front();
                }
                self.elle_note = (gate >= 0.5).then_some((now, note));
            }
            None if gate >= 0.5 => self.elle_note = Some((now, note)),
            _ => {}
        }
    }

    /// Her own network (ELLE) is GRML (`mind/elle.grm`, called by the apps); the window only carries what it
    /// learned from one session to the next: taken from the sound now and then, given back when she wakes (her
    /// `born` memory still 0). The first time, what she starts knowing is said to the machine, each file with its
    /// own provenance.
    fn elle(&mut self) {
        let now = self.ear_now();
        if now - self.elle_at < 1.0 {
            return;
        }
        let first = self.elle_at == 0.0;
        self.elle_at = now;
        if first {
            let dirs = [self.memory.parent().map(|p| p.join("mind")), Some(std::path::PathBuf::from("mind"))];
            let Some(dir) = dirs.into_iter().flatten().find(|d| d.join("knowledge").exists()) else { return };
            let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
            self.engine.send(move |w| {
                // her first network was a few nodes of mood written by the window: she is a network of her own now
                if w.sources().contains_key("elle") {
                    let _ = w.trash_source("elle");
                }
                for k in ["physics", "acoustics", "music", "maths", "grml"] {
                    let origin = format!("knowledge:{k}");
                    let f = dir.join("knowledge").join(format!("{k}.grm"));
                    // said line by line (its measurements too: `use` inside the machine leaves them to the prompt), at
                    // each launch: said again it stays the same, and a memory made before (when they were missing)
                    // gets them
                    if let Ok(text) = std::fs::read_to_string(&f) {
                        let origin = grm_engine::Provenance::of(origin);
                        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                            let _ = w.say(line, &origin);
                        }
                    }
                }
                Ok(String::new())
            });
            return;
        }
        let Ok(mut m) = self.mixer.lock() else { return };
        match m.value("out", "elle_born") {
            Some(b) if b < 0.5 => {
                // she wakes: what she learned before is given back to her network
                let mut back = self.elle_memory.clone();
                back.insert("elle_born".into(), 1.0);
                m.restore("out", &back);
            }
            Some(_) => {
                let learned = m.memories("out");
                if !learned.is_empty() {
                    self.elle_memory = learned;
                }
                drop(m);
                if (now as u64).is_multiple_of(30) {
                    self.keep_music();
                }
            }
            None => {}
        }
    }

    /// A take being played again: what was turned and played, when its time comes (a take of before: its notes, let
    /// go when they end).
    fn replaying(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.replay.take() else { return };
        let t = r.start.elapsed().as_secs_f64();
        while r.next_event < r.events.len() && r.events[r.next_event].0 <= t {
            let (_, node, v) = r.events[r.next_event].clone();
            self.by_ear(&node, v);
            r.next_event += 1;
        }
        if let Some((_, keys)) = self.instrument() {
            let now = self.ear_now();
            while r.next < r.notes.len() && r.notes[r.next].0 <= t {
                let (_, len, note, voice) = r.notes[r.next];
                let hz = 55.0 * 2f64.powf(note as f64 / 12.0);
                let (pitch, gate) = match (voice, &keys) {
                    (1, _) => (self.named("lead_pitch"), self.named("lead_gate")),
                    (_, Some(k)) => k.clone(),
                    _ => (self.named("lead_pitch"), self.named("lead_gate")),
                };
                self.by_ear(&pitch, (hz * 100.0).round() / 100.0);
                self.by_ear(&gate, 1.0);
                r.offs.push((r.notes[r.next].0 + len, gate));
                self.played.roll.push_back((now, len, note, voice));
                r.next += 1;
            }
        }
        let mut offs = std::mem::take(&mut r.offs);
        offs.retain(|(at, gate)| {
            if *at <= t {
                self.by_ear(gate, 0.0);
                false
            } else {
                true
            }
        });
        r.offs = offs;
        ctx.request_repaint_after(Duration::from_millis(20));
        if r.next < r.notes.len() || !r.offs.is_empty() || r.next_event < r.events.len() {
            self.replay = Some(r);
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
        if let Some(ws) = self.write_into.clone() {
            // in a workshop: its relation form, filled with what the drag said
            let mut v = std::collections::HashMap::new();
            v.insert("name".to_string(), name);
            v.insert("equation".to_string(), text.split_once(':').map(|x| x.1.trim().to_string()).unwrap_or_default());
            self.workshop_state.entry(ws).or_default().form = Some(("relate", v));
            return;
        }
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
        if line.trim() == "sleep" {
            self.slept = Some(Instant::now());
        }
        if line.trim() == "panic" {
            self.close_gates();
        }
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
                m.map(|m| (m.text(), format!("{:.2} decades, residual {:.1e} dex", m.decades, m.residual)))
            };
            w.mood.host = grm_shell::host_capacity();
            let all: indexmap::IndexMap<String, grm_engine::master::Master> =
                w.sources().into_iter().filter_map(|(s, _)| w.master(Some(&s)).map(|m| (s, m))).collect();
            (text(w.master(None)), here.as_deref().and_then(|h| text(w.master(Some(h)))), all)
        }) {
            self.master = (m.0, m.1);
            self.masters = m.2;
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
            if let Some(k) = self.seeing.iter().position(|l| *l == a.typed) {
                self.seeing.remove(k);
            }
            // a question already answered in a window (a removal asked for by a click): not asked again here
            // only the answer to that very line (never another question of the prompt)
            let answered = a.asking && self.quiet_yes.as_deref() == Some(a.typed.as_str());
            if self.quiet_yes.as_deref() == Some(a.typed.as_str()) {
                self.quiet_yes = None;
            }
            for l in a.lines {
                if answered && l.kind == LineKind::Ask {
                    continue;
                }
                self.note(l.kind, l.text);
            }
            self.asking = a.asking;
            if answered {
                self.asking = false;
                self.worker.send("y".to_string());
            }
            if let Some(apps) = a.apps {
                if self.apps_known {
                    for v in &apps {
                        if !self.apps.iter().any(|x| x.name == v.name) {
                            // an app that just opened is put on the space, and the space is shown (it can be closed
                            // again by taking it back)
                            self.space.put(crate::space::Kind::App(v.name.clone()));
                            self.space_on = true;
                            self.zone_center = None;
                            self.current = None;
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
                // an app closed: the network it leaves is shown whole again
                if !closed.is_empty() {
                    self.view.fitted = false;
                }
                // an app the human just opened is what they see (not the space)
                let opened: Vec<String> = apps
                    .iter()
                    .filter(|v| !self.apps.iter().any(|x| x.name == v.name))
                    .map(|v| v.name.clone())
                    .collect();
                if self.apps_known {
                    if let Some(n) = opened.last() {
                        self.wanted_app = Some(n.clone());
                        self.space_on = false;
                        self.current_file = None;
                        self.sandbox_shown = None;
                        self.workshop_shown = None;
                    }
                }
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
                        self.space_on = false;
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
                    grm_shell::Order::Workshop(n) => {
                        // a workshop opened (or a table just learned): in place of the space, at once
                        let kind = crate::space::Kind::Workshop(n);
                        self.space.put(kind.clone());
                        self.zone_center = Some(kind);
                        self.space_on = true;
                        self.sandbox_shown = None;
                        self.current = None;
                        self.current_file = None;
                        self.launched = false;
                        self.sandbox_picture = None;
                        self.view.fitted = false;
                    }
                    grm_shell::Order::Collide { path, stem, columns, into, taken } => {
                        let rows = columns
                            .iter()
                            .map(|c| {
                                let node = into.get(c).cloned().unwrap_or(c.clone());
                                let clash = taken.contains(&node);
                                (c.clone(), if clash { format!("{stem}_{node}") } else { node }, clash)
                            })
                            .collect();
                        self.collide = Some(crate::files::Collide { path, rows });
                    }
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
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::M)) {
            self.toggle_voice();
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
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7];
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
                let sounding = self.mixer.lock().map(|m| m.ambient_on || !m.channels.is_empty()).unwrap_or(false);
                let sign = crate::icons::image("octagon-x", 15.0, if sounding { ink.amber } else { ink.muted });
                let words = RichText::new(self.tr("panic")).color(if sounding { ink.amber } else { ink.muted });
                if ui.add(egui::Button::image_and_text(sign, words)).on_hover_text("Ctrl+.").clicked() {
                    self.say("panic");
                }
                // the machine's own voice (the wind, the pad): on or off from anywhere
                let voice = self.mixer.lock().map(|m| m.ambient_on).unwrap_or(false);
                let (sign, what) = if voice { ("volume-2", "voice_off_d") } else { ("volume-x", "voice_on_d") };
                if crate::icons::sign(ui, sign, &format!("{} (Ctrl+M)", self.tr(what))).clicked() {
                    self.toggle_voice();
                }
                ui.label(RichText::new(format!("{} {}", self.picture.ticks, self.tr("ticks"))).color(ink.muted));
                ui.label(
                    RichText::new(if self.picture.settled { self.tr("settled") } else { self.tr("computing") })
                        .color(ink.muted),
                );
                if self.worker.waiting > 0 {
                    ui.label(RichText::new(self.tr("busy")).color(ink.muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let help = crate::icons::sign(ui, "circle-help", &format!("{} (F1)", self.tr("help")));
                    self.areas.insert("help", help.rect);
                    if help.clicked() {
                        self.help_open = !self.help_open;
                    }
                    let settings = crate::icons::sign(ui, "settings", self.tr("settings"));
                    self.areas.insert("settings", settings.rect);
                    if settings.clicked() {
                        self.settings_open = !self.settings_open;
                    }
                    let sleep_what = format!("{}: {}", self.tr("sleep"), self.memory.display());
                    // once asked, the sign changes for a while: the memory was written, it can be seen
                    let just = self.slept.filter(|at| at.elapsed().as_secs_f32() < 4.0);
                    if just.is_some() {
                        let ink = Ink::of(ui.ctx());
                        let done = format!("{}: {}", self.tr("slept"), self.memory.display());
                        let b = egui::Button::image_and_text(
                            crate::icons::image("moon-star", 16.0, ink.accent),
                            RichText::new(self.tr("slept")).small().color(ink.accent),
                        )
                        .frame(false);
                        if ui.add(b).on_hover_text(done).clicked() {
                            self.say("sleep");
                        }
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
                    } else if crate::icons::sign(ui, "moon", &sleep_what).clicked() {
                        self.say("sleep");
                    }
                    if crate::icons::sign(ui, "step-forward", self.tr("tick_d")).clicked() {
                        self.say("tick");
                    }
                });
            });
        }
    }

    fn prompt(&mut self, ui: &mut egui::Ui) {
        let ink = Ink::of(ui.ctx());
        // it fills the height the human gave it (a panel keeps the height of what it holds)
        ui.set_min_height(ui.available_height());
        {
            let id = egui::Id::new("prompt-line");
            let line_h = ui.spacing().interact_size.y + 10.0;
            if self.prompt_open {
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
                                job.sections
                                    .iter_mut()
                                    .for_each(|s| s.format.color = s.format.color.gamma_multiply(0.75));
                                job.sections.insert(
                                    0,
                                    egui::text::LayoutSection {
                                        leading_space: 0.0,
                                        byte_range: 0..0,
                                        format: Default::default(),
                                    },
                                );
                                let mut full = egui::text::LayoutJob::default();
                                full.append(
                                    "› ",
                                    0.0,
                                    egui::TextFormat::simple(egui::FontId::monospace(13.0), ink.muted),
                                );
                                for sec in job.sections.iter().skip(1) {
                                    full.append(&job.text[sec.byte_range.clone()], 0.0, sec.format.clone());
                                }
                                ui.add(egui::Label::new(full).selectable(true));
                                continue;
                            }
                            let shown = if self.settings.lang == Lang::Fr && !text.starts_with("› ") {
                                crate::french::fr(text)
                            } else {
                                text.clone()
                            };
                            let rich = RichText::new(shown).monospace().color(colour);
                            let rich = if *kind == LineKind::Error { rich.strong() } else { rich };
                            // what the prompt said can be selected and copied
                            ui.add(egui::Label::new(rich).selectable(true));
                        }
                    });
            }
            ui.horizontal(|ui| {
                let (sign, what) = if self.prompt_open {
                    ("panel-bottom-close", t(self.settings.lang, "fold_prompt"))
                } else {
                    ("panel-bottom-open", t(self.settings.lang, "unfold_prompt"))
                };
                if crate::icons::sign(ui, sign, what).clicked() {
                    self.prompt_open = !self.prompt_open;
                }
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
                    .margin(crate::style::FIELD)
                    .layouter(&mut layouter)
                    .id(id)
                    .lock_focus(true)
                    .font(egui::TextStyle::Monospace)
                    .hint_text(crate::style::hint(t(self.settings.lang, "prompt_hint")))
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
        crate::airlock::window(self, ctx);
        self.lightbox_window(ctx);
        self.new_workshop_window(ctx);
        self.confirm_window(ctx);
        self.new_app_window(ctx);
        crate::menu::node_form(self, ctx);
        let lang = self.settings.lang;
        crate::help::window(self, ctx);
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
                ui.label(t(lang, "breathe"));
                ui.checkbox(&mut self.settings.breathe, t(lang, "breathe_d"));
                ui.end_row();
                ui.label(t(lang, "human"));
                let host = grm_engine::witness::human(None, lang == Lang::Fr);
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.settings.human)
                        .hint_text(crate::style::hint(&host))
                        .desired_width(180.0),
                );
                if field.lost_focus() {
                    let name = self.settings.human.trim().to_string();
                    self.worker.send(format!("human {}", if name.is_empty() { host.clone() } else { name }));
                }
                field.on_hover_text(t(lang, "human_d"));
                ui.end_row();
                ui.label(t(lang, "text_size"));
                ui.add(egui::Slider::new(&mut self.settings.text, 0.8..=2.0).step_by(0.1));
                ui.end_row();
                ui.label(t(lang, "memory"));
                ui.label(RichText::new(self.memory.display().to_string()).monospace());
                ui.end_row();
            });
            ui.separator();
            crate::midi::settings(self, ui);
            ui.separator();
            ui.label(RichText::new(t(lang, "layout")).strong());
            ui.label(t(lang, "layout_note"));
            let mut lines = vec![format!("# {} {:.2}", t(lang, "harmony"), self.layout.sizes.harmony)];
            lines.extend(self.layout.statements());
            crate::style::terminal(ui, &lines);
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
        if std::mem::take(&mut self.fullscreen_back) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        if std::mem::take(&mut self.maximized_back) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("grm-lang"), self.settings.lang));
        self.breath(ctx);
        self.answers(ctx);
        if !self.space_ready && self.apps_known {
            self.space = crate::space::Space::load(&self.memory).unwrap_or_else(|| crate::space::first(self));
            self.space_ready = true;
        }
        self.follow();
        self.files_dropped(ctx);
        self.picked();
        self.pull(ctx);
        self.script(ctx);
        self.keys(ctx);
        self.keep(ctx);
        if crate::zone::screen(self, ctx) {
            self.windows(ctx);
            crate::zone::detached(self, ctx);
            return;
        }
        if self.launched {
            if let Some(name) = self.current.clone() {
                self.launched_view(ctx, &name);
                self.windows(ctx);
                // what lives goes on in the app alone too (her, the keys, the clock, the toasts)
                self.toasts(ctx);
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
        }
        let size = ctx.content_rect().size();
        let sizes = self.layout.window(size.x, size.y);
        let r = egui::TopBottomPanel::top("top").show(ctx, |ui| self.top(ui));
        self.areas.insert("top", r.response.rect);
        // the side first: it goes down to the bottom (the prompt is under the middle only); its width is the
        // human's, told to the layout network as a wish
        if !sizes.stacked {
            if self.side_open {
                let r = egui::SidePanel::right("side")
                    .resizable(true)
                    .default_width(sizes.side)
                    .width_range(260.0..=(size.x * 0.6).max(280.0))
                    .show(ctx, |ui| crate::panels::show(self, ui));
                self.areas.insert("side", r.response.rect);
                let w = r.response.rect.width();
                if (w - self.layout.sizes.side).abs() > 2.0 && r.response.ctx.input(|i| i.pointer.any_down()) {
                    self.layout.wish_side(w);
                }
            } else {
                let r = egui::SidePanel::right("side-rail")
                    .exact_width(44.0)
                    .resizable(false)
                    .show(ctx, |ui| crate::panels::rail(self, ui));
                self.areas.insert("side", r.response.rect);
            }
        }
        let prompt = egui::TopBottomPanel::bottom("prompt");
        let prompt = if self.prompt_open {
            prompt.resizable(true).default_height(sizes.bottom.min(size.y * 0.5)).height_range(90.0..=size.y * 0.7)
        } else {
            prompt.exact_height(40.0).resizable(false)
        };
        let r = prompt.show(ctx, |ui| self.prompt(ui));
        self.areas.insert("prompt", r.response.rect);
        // the height the human gave it: kept (as a wish of the layout network), given back at the next start
        let h = r.response.rect.height();
        if self.prompt_open && (h - self.layout.sizes.bottom).abs() > 2.0 && ctx.input(|i| i.pointer.any_down()) {
            self.layout.wish_bottom(h);
        }
        if sizes.stacked && self.side_open {
            let r = egui::TopBottomPanel::bottom("side")
                .exact_height(sizes.band.min(size.y * 0.45))
                .show(ctx, |ui| crate::panels::show(self, ui));
            self.areas.insert("side", r.response.rect);
        }
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
        });
        self.windows(ctx);
        crate::zone::detached(self, ctx);
        self.toasts(ctx);
        crate::files::columns(self, ctx);
        crate::files::collide(self, ctx);
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
        // the tabs: the space, the whole network, each app that runs, and what can be opened
        let before = (self.current.clone(), self.current_file, self.sandbox_shown.clone(), self.workshop_shown.clone());
        let mut to_space = false;
        ui.horizontal_wrapped(|ui| {
            if crate::icons::toggle(ui, "layout-dashboard", t(lang, "space"), self.space_on)
                .on_hover_text(t(lang, "space_d"))
                .clicked()
            {
                to_space = true;
            }
            let desk_on = !self.space_on
                && self.current.is_none()
                && self.current_file.is_none()
                && self.sandbox_shown.is_none()
                && self.workshop_shown.is_none();
            if crate::icons::toggle(ui, "network", t(lang, "desk"), desk_on).clicked() {
                self.space_on = false;
                self.workshop_shown = None;
                self.current = None;
                self.current_file = None;
                self.sandbox_shown = None;
                self.editing = false;
            }
            for a in self.apps.clone() {
                let on = !self.space_on
                    && self.current.as_deref() == Some(a.name.as_str())
                    && self.current_file.is_none()
                    && self.sandbox_shown.is_none();
                let sign = a.icon.clone().filter(|i| crate::icons::exists(i)).unwrap_or("app-window".into());
                let colour = if !a.errors.is_empty() {
                    ink.amber
                } else if on {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().text_color()
                };
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
                    self.workshop_shown = None;
                    self.space_on = false;
                }
                r.context_menu(|ui| {
                    if crate::menu::item(ui, "maximize", t(lang, "launch"), t(lang, "m_launch_d")) {
                        self.current = Some(a.name.clone());
                        self.launched = true;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                        ui.close();
                    }
                    if crate::menu::item(ui, "x", t(lang, "close_app"), t(lang, "m_close_app_d")) {
                        self.say(format!("app close {}", a.name));
                        ui.close();
                    }
                    if crate::menu::item(ui, "layout-dashboard", t(lang, "space_put_it"), t(lang, "space_put_it_d")) {
                        self.space.put(crate::space::Kind::App(a.name.clone()));
                        self.space_on = true;
                        self.space.seen = egui::Rect::NOTHING;
                        ui.close();
                    }
                    if crate::menu::item(ui, "trash-2", t(lang, "m_trash_app"), t(lang, "m_trash_app_d")) {
                        self.ask_trash(&a.name.clone(), true);
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
        if to_space {
            self.space_on = true;
        } else if before
            != (self.current.clone(), self.current_file, self.sandbox_shown.clone(), self.workshop_shown.clone())
        {
            self.space_on = false;
        }
        if self.space_on {
            ui.separator();
            crate::space::show(self, ui);
            return;
        }
        if let Some(k) = self.current_file {
            if k < self.files.len() {
                ui.separator();
                crate::files::show(self, ui, k);
                return;
            }
            self.current_file = None;
        }
        if let Some(ws) = self.workshop_shown.clone() {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "hammer", 16.0, ink.accent);
                ui.label(RichText::new(format!("{} {ws}", t(lang, "workshop"))).strong().color(ink.accent))
                    .on_hover_text(t(lang, "workshop_d"));
                if crate::icons::button(ui, "merge", t(lang, "workshop_merge"))
                    .on_hover_text(t(lang, "workshop_merge_d"))
                    .clicked()
                {
                    self.say(format!("workshop merge {ws}"));
                    self.tab = Tab::Sandboxes;
                    self.side_open = true;
                }
                if crate::icons::button(ui, "chart-line", t(lang, "m_plots")).clicked() {
                    self.plotted = self
                        .sandbox_picture
                        .as_ref()
                        .map(|p| p.nodes.iter().take(4).map(|n| n.name.clone()).collect())
                        .unwrap_or_default();
                    self.desk_mode = Mode::Plot;
                }
                if crate::icons::button(ui, "x", t(lang, "workshop_close")).clicked() {
                    self.say(format!("workshop close {ws}"));
                    self.workshop_shown = None;
                }
                if crate::icons::button(ui, "arrow-left", t(lang, "back")).clicked() {
                    self.workshop_shown = None;
                }
            });
            ui.separator();
            let name = ws.clone();
            if let Some(p) = self.sandbox_pull.poll(&self.engine, move |w| Picture::of_workshop(w, &name)) {
                self.sandbox_picture = p;
            }
            match self.sandbox_picture.clone() {
                Some(p) => {
                    let saved = std::mem::replace(&mut self.picture, p);
                    if self.desk_mode == Mode::Plot {
                        crate::plot::show(self, ui, None);
                    } else {
                        crate::view::show(self, ui, None);
                    }
                    self.picture = saved;
                }
                None => {
                    ui.label(t(lang, "no_workshop"));
                }
            }
            return;
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
                ui.separator();
                // where the view is: the whole world, or inside one network (each step goes back up)
                if crate::icons::toggle(ui, "globe", t(lang, "world_all"), self.dived.is_none())
                    .on_hover_text(t(lang, "world_all_d"))
                    .clicked()
                {
                    self.dived = None;
                    self.view.fitted = false;
                }
                if let Some(d) = self.dived.clone() {
                    crate::icons::show(ui, "chevron-right", 14.0, Ink::of(ui.ctx()).muted);
                    let _ = crate::icons::toggle(ui, "network", d.as_str(), true);
                }
            });
            ui.separator();
            let only = self.dived.clone();
            match self.desk_mode {
                Mode::Plot => crate::plot::show(self, ui, only.as_deref()),
                _ => crate::view::show(self, ui, only.as_deref()),
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
            // her own instrument: only she plays it
            if self.hers().as_deref() == Some(name.as_str()) {
                crate::icons::show(ui, "lock", 12.0, ink.accent);
                ui.label(RichText::new(t(lang, "hers_locked")).small().color(ink.muted));
            } else if !view.prefix.is_empty() {
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
                if name == crate::studio::APP {
                    crate::studio::panel(self, ui);
                } else {
                    egui::ScrollArea::vertical().id_salt(("surface", &name)).auto_shrink([false, false]).show(
                        ui,
                        |ui| {
                            crate::surface::show(self, ui, &name);
                        },
                    );
                }
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
            if name == crate::studio::APP {
                crate::studio::panel(self, ui);
                return;
            }
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
                    egui::TextEdit::singleline(&mut text)
                        .margin(crate::style::FIELD)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
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
                        .margin(crate::style::FIELD)
                        .hint_text(crate::style::hint(t(lang, "new_app")))
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
                Pick::Eye => dialog.add_filter("PNG", &["png"]).pick_files(),
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
                    self.note(LineKind::Warning, self.tr("no_file_window").to_string());
                }
                match what {
                    Pick::Eye => {
                        for f in files {
                            self.see(&f);
                        }
                    }
                    Pick::Open => {
                        match self.workshop_loading.clone() {
                            // tables for a workshop: learned in it
                            Some(ws) => {
                                for f in files {
                                    let p = grm_shell::quoted(&f.display().to_string());
                                    self.say(format!("load {p} --into {ws}"));
                                }
                            }
                            None => {
                                for f in files {
                                    self.open_path(f);
                                }
                            }
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
        // a path with spaces is said between quotes
        let shown = grm_shell::quoted(&p.display().to_string());
        if p.is_dir() {
            self.say(format!("cd {shown}"));
            return;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "png" => {
                self.tab = Tab::Eye;
                self.see(&p);
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

/// A toast says little: its first lines, cut short with an ellipsis (the whole stays in the prompt's lines).
fn brief(text: &str) -> String {
    const LINES: usize = 3;
    const CHARS: usize = 220;
    let mut out = String::new();
    let mut cut = false;
    for (k, line) in text.lines().enumerate() {
        if k >= LINES {
            cut = true;
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    if out.chars().count() > CHARS {
        out = out.chars().take(CHARS).collect();
        cut = true;
    }
    if cut {
        out.push_str(" …");
    }
    out
}

impl App {
    /// A line for the human: in the prompt's lines, and (what went wrong) said softly in a corner too.
    pub fn note(&mut self, kind: LineKind, text: String) {
        if matches!(kind, LineKind::Warning | LineKind::Error) {
            self.toasts.push((kind, brief(&text), Instant::now()));
            if self.toasts.len() > 3 {
                self.toasts.remove(0);
            }
        }
        self.log.push((kind, text));
    }

    /// Something done that the human asked for, said softly in a corner (and in the prompt's lines).
    pub fn toast(&mut self, text: String) {
        self.toasts.push((LineKind::Said, brief(&text), Instant::now()));
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
        self.log.push((LineKind::Said, text));
    }

    /// What went wrong, in a corner: it blocks nothing, comes and goes slowly, can be closed.
    /// Her consciousness speaks by itself when something in her asks for it (what she feels, what she lacks):
    /// softly, in a corner, not more than once in a while, and never twice the same words in a row.
    fn witness_speaks(&mut self) {
        let fr = self.settings.lang == Lang::Fr;
        let human = grm_engine::witness::human(Some(self.settings.human.as_str()), fr);
        let Some(words) = grm_engine::witness::speak(&self.picture.glance, &human, fr) else { return };
        let fresh = match &self.spoken {
            Some((last, at)) => *last != words && at.elapsed().as_secs_f32() > 45.0,
            None => true,
        };
        if fresh {
            self.toasts.push((LineKind::Said, brief(&words), Instant::now()));
            if self.toasts.len() > 3 {
                self.toasts.remove(0);
            }
            self.log.push((LineKind::Said, format!("◌ {words}")));
            self.spoken = Some((words, Instant::now()));
        }
    }

    /// What the controllers sent since the last frame: learned, or turned into observations and lines.
    fn midi_in(&mut self, ctx: &egui::Context) {
        let came = self.midi.drain();
        if came.is_empty() {
            if self.midi_learn.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
            return;
        }
        let mut nodes: indexmap::IndexMap<String, f64> = indexmap::IndexMap::new();
        let mut lines = vec![];
        for (port, s) in came {
            if let Some(target) = self.midi_learn.clone() {
                if let Some(b) = crate::midi::learn(&port, s, &target) {
                    let words = b.words();
                    self.settings.midi.retain(|x| x.control != b.control || x.port != b.port);
                    self.settings.midi.push(b);
                    self.midi_learn = None;
                    self.toasts.push((LineKind::Said, format!("MIDI · {words}"), Instant::now()));
                    continue;
                }
            }
            let (n, l) = crate::midi::apply(&self.settings.midi, &port, s, &mut self.midi_held);
            for (k, v) in n {
                nodes.insert(k, v);
            }
            lines.extend(l);
        }
        for (n, v) in nodes {
            if self.midi_sent.get(&n) != Some(&v) {
                self.worker.send(format!("observe {n} = {}", grm_engine::lang::g(v)));
                self.midi_sent.insert(n, v);
            }
        }
        for l in lines {
            self.say(l);
        }
        ctx.request_repaint();
    }

    /// Seconds since the window opened (the clock she plays by).
    pub fn ear_now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// What she writes by ear, straight into the world (not through the prompt: no line to keep, nothing to undo).
    fn by_ear(&self, node: &str, v: f64) {
        self.taped(node, v);
        self.sound_now(node, v);
        let line = format!("observe {node} = {} ± 0.000001", grm_engine::lang::g(v));
        self.engine.send(move |w| w.say(&line, &grm_engine::Provenance::of("ear")).map(|_| String::new()));
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.midi_in(ctx);
        self.replaying(ctx);
        self.hands();
        self.clock();
        self.fuses();
        self.elle();
        self.elle_roll();
        self.witness_speaks();
        const STAY: f32 = 8.0;
        self.toasts.retain(|(_, _, at)| at.elapsed().as_secs_f32() < STAY);
        if self.toasts.is_empty() {
            return;
        }
        let ink = Ink::of(ctx);
        let lang = self.settings.lang;
        let bottom = self.areas.get("prompt").map(|r| r.top()).unwrap_or(ctx.content_rect().bottom());
        let right = self.areas.get("side").map(|r| r.left()).unwrap_or(ctx.content_rect().right());
        let mut closed = None;
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(egui::pos2(right - 16.0, bottom - 12.0))
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                for (k, (kind, text, at)) in self.toasts.iter().enumerate() {
                    let age = at.elapsed().as_secs_f32();
                    // in over half a second, out over the last two: nothing jumps
                    let a = (age / 0.5).min(1.0) * ((STAY - age) / 2.0).clamp(0.0, 1.0);
                    let colour = match kind {
                        LineKind::Error => ink.amber,
                        LineKind::Warning => ink.amber.gamma_multiply(0.85),
                        _ => ink.accent,
                    };
                    let text = text.trim_start_matches("! ").trim_start_matches("? ");
                    let shown = if lang == Lang::Fr { crate::french::fr(text) } else { text.to_string() };
                    egui::Frame::new()
                        .fill(ink.raised)
                        .stroke(egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.6)))
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .multiply_with_opacity(a)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let sign = if matches!(kind, LineKind::Error | LineKind::Warning) {
                                    "triangle-alert"
                                } else {
                                    "triangle"
                                };
                                crate::icons::show(ui, sign, 16.0, colour.gamma_multiply(a));
                                ui.add(egui::Label::new(RichText::new(shown).color(ink.text.gamma_multiply(a))).wrap());
                                if crate::icons::sign(ui, "x", t(lang, "close")).clicked() {
                                    closed = Some(k);
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        if let Some(k) = closed {
            self.toasts.remove(k);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

impl App {
    /// A relation chosen (in the network): it opens in the inspector, its network unfolded.
    pub fn show_relation(&mut self, name: &str) {
        let Some(r) = self.picture.relations.iter().find(|r| r.name == name) else { return };
        let net = format!("n:{}", r.source);
        let key = format!("rel:{name}");
        self.tree_open.insert(net.clone());
        self.tree_open.insert(format!("{net}/rels"));
        self.tree_detail.insert(key.clone());
        self.tree_scroll = Some(key);
        self.tab = Tab::Inspector;
        self.side_open = true;
    }
}

impl App {
    /// Into a network: only it, filling the view (the way back is the way back: Alt+Left).
    pub fn dive(&mut self, source: &str) {
        if !self.picture.nodes.iter().any(|n| n.source == source) {
            return;
        }
        self.dived = Some(source.to_string());
        self.current = None;
        self.current_file = None;
        self.sandbox_shown = None;
        self.launched = false;
        self.desk_mode = Mode::Network;
        self.view.fitted = false;
    }
}

impl App {
    /// Living colours: every tenth of a second, the palette a little turned (slowly), the sound lifting it.
    fn breath(&mut self, ctx: &egui::Context) {
        if !self.settings.breathe || self.settings_open {
            if std::mem::take(&mut self.breathing) {
                apply_look(ctx, &self.settings);
            }
            return;
        }
        if self.breath_at.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.breath_at = Instant::now();
        self.breathing = true;
        let peak = self.mixer.lock().map(|m| m.peak).unwrap_or(0.0);
        // the sound follows slowly too
        self.breath_sound += (peak - self.breath_sound) * 0.15;
        let secs = self.started.elapsed().as_secs_f64();
        crate::style::colours(ctx, &crate::style::breathed(&self.settings.palette, secs, self.breath_sound));
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl App {
    /// A node or a relation to take away, asked for by a click: a window says what would go and asks.
    pub fn ask_remove(&mut self, name: &str) {
        let lang = self.settings.lang;
        let n = name.to_string();
        let preview = self
            .engine
            .query(move |w| w.removal(&[n.as_str()]).map(|p| (p.kind.to_string(), p.nodes, p.relations)))
            .and_then(|r| r.ok());
        let mut detail = vec![];
        if let Some((app, l)) = self.line_in_app(name) {
            let text = self.apps.iter().find(|a| a.name == app).and_then(|a| a.text.lines().nth(l - 1)).unwrap_or("");
            detail.push(format!("# {app}, {} {l}", t(lang, "line")));
            detail.push(text.trim().to_string());
        }
        if let Some((kind, nodes, rels)) = preview {
            detail.push(format!("# {kind} {name}"));
            if !nodes.is_empty() {
                detail.push(format!("node {}", nodes.join(", ")));
            }
            if !rels.is_empty() {
                detail.push(format!("relate {}", rels.join(", ")));
            }
        }
        self.confirm = Some(Confirm {
            title: format!("{} {name} ?", t(lang, "remove_q")),
            detail,
            line: format!("remove {name}"),
            trash: false,
        });
    }

    /// A network or an app to the trash: asked softly first (it can be given back), a toast once it is done.
    pub fn ask_trash(&mut self, what: &str, app: bool) {
        let lang = self.settings.lang;
        let mut detail = vec![];
        if app {
            detail.push(format!("# app {what}"));
        } else {
            let nodes: Vec<String> =
                self.picture.nodes.iter().filter(|n| n.source == what).map(|n| n.name.clone()).collect();
            let rels: Vec<String> =
                self.picture.relations.iter().filter(|r| r.source == what).map(|r| r.name.clone()).collect();
            detail.push(format!("# {what}"));
            if !nodes.is_empty() {
                detail.push(format!("node {}", nodes.join(", ")));
            }
            if !rels.is_empty() {
                detail.push(format!("relate {}", rels.join(", ")));
            }
        }
        self.confirm = Some(Confirm {
            title: format!("{} {what} ?", t(lang, "trash_q")),
            detail,
            line: if app { format!("app trash {what}") } else { format!("trash {what}") },
            trash: true,
        });
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(c) = self.confirm.clone() else { return };
        let lang = self.settings.lang;
        let ink = Ink::of(ctx);
        let mut answer: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.horizontal(|ui| {
                crate::icons::show(ui, "trash-2", 18.0, ink.amber);
                ui.label(RichText::new(&c.title).strong().size(16.0));
            });
            ui.add_space(6.0);
            ui.label(RichText::new(t(lang, "remove_what")).small().color(ink.muted));
            crate::style::terminal(ui, &c.detail);
            ui.label(
                RichText::new(t(lang, if c.trash { "trash_undo" } else { "remove_undo" })).small().color(ink.muted),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let yes = egui::Button::image_and_text(
                    crate::icons::image("trash-2", 15.0, ink.on_accent()),
                    RichText::new(t(lang, if c.trash { "trash_yes" } else { "remove_yes" })).color(ink.on_accent()),
                )
                .fill(if c.trash { ink.accent } else { ink.amber });
                if ui.add(yes).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    answer = Some(true);
                }
                if crate::icons::button(ui, "x", t(lang, "cancel")).clicked() {
                    answer = Some(false);
                }
            });
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(false);
        }
        match answer {
            Some(true) => {
                self.confirm = None;
                self.quiet_yes = Some(c.line.clone());
                if c.trash {
                    let what = c.line.rsplit(' ').next().unwrap_or("").to_string();
                    self.toast(format!("{} {what}", t(lang, "trash_done")));
                }
                self.say(c.line);
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

impl App {
    /// The machine's voice on, or off (it fades either way).
    pub fn toggle_voice(&mut self) {
        let on = !self.mixer.lock().map(|m| m.ambient_on).unwrap_or(false);
        self.say(if on { "sound on" } else { "sound off" });
        self.settings.voice_on = on;
    }
}

impl App {
    /// The window that opens a new workshop: its name, and Enter.
    fn new_workshop_window(&mut self, ctx: &egui::Context) {
        if !self.new_workshop_open {
            return;
        }
        let lang = self.settings.lang;
        let mut open = true;
        let mut made = false;
        egui::Window::new(t(lang, "ws_new"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.label(t(lang, "workshop_d"));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.new_workshop)
                        .margin(crate::style::FIELD)
                        .hint_text(crate::style::hint(t(lang, "ws_name")))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(260.0),
                );
                r.request_focus();
                let fine = grm_engine::lang::is_name(self.new_workshop.trim());
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if crate::icons::button(ui, "hammer", t(lang, "ws_open")).clicked() || enter {
                    made = fine;
                }
            });
        if made {
            let n = std::mem::take(&mut self.new_workshop);
            self.say(format!("workshop new {}", n.trim()));
            self.new_workshop_open = false;
        } else if !open {
            self.new_workshop_open = false;
        }
    }
}

impl App {
    /// A picture of an eye, big; the arrows (or the keys) go through the others; Esc closes.
    fn lightbox_window(&mut self, ctx: &egui::Context) {
        let Some((name, k)) = self.lightbox.clone() else { return };
        // every picture of every eye, one after another
        let all: Vec<(String, usize)> =
            self.eyes.iter().flat_map(|(n, e)| (0..e.textures.len()).map(move |i| (n.clone(), i))).collect();
        let Some(at) = all.iter().position(|(n, i)| *n == name && *i == k) else {
            self.lightbox = None;
            return;
        };
        let n = all.len();
        let k = at;
        let (tag, tex) = self.eyes[&name].textures[self.lightbox.as_ref().map(|l| l.1).unwrap_or(0)].clone();
        let lang = self.settings.lang;
        let ink = Ink::of(ctx);
        let mut go: i32 = 0;
        let screen = ctx.content_rect();
        let modal = egui::Modal::new(egui::Id::new("lightbox")).show(ctx, |ui| {
            ui.horizontal(|ui| {
                crate::icons::show(ui, "eye", 16.0, ink.accent);
                ui.label(RichText::new(format!("{name} · {}", t(lang, &tag))).strong());
                ui.label(RichText::new(format!("{} / {n}", k + 1)).small().color(ink.muted));
            });
            let size = tex.size_vec2();
            let room = egui::vec2(screen.width() * 0.75, screen.height() * 0.7);
            let scale = (room.x / size.x).min(room.y / size.y).max(0.1);
            ui.horizontal(|ui| {
                if crate::icons::sign_enabled(ui, k > 0, "chevron-left", t(lang, "ws_prev")).clicked() {
                    go = -1;
                }
                // drawn as pixels, not blurred: what the eye saw, as it saw it
                ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), size * scale)));
                if crate::icons::sign_enabled(ui, k + 1 < n, "chevron-right", t(lang, "ws_next")).clicked() {
                    go = 1;
                }
            });
            ui.label(RichText::new(t(lang, "eye_keys")).small().color(ink.muted));
        });
        let (left, right) = ctx.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight)));
        if left {
            go = -1;
        }
        if right {
            go = 1;
        }
        if modal.should_close() {
            self.lightbox = None;
            return;
        }
        let next = (k as i32 + go).clamp(0, n as i32 - 1) as usize;
        self.lightbox = Some(all[next].clone());
    }
}

impl App {
    /// A picture given to the eye: each picture its own eye, named after its file (several can be given at once:
    /// they are seen one after another, and the window says how many wait).
    pub fn see(&mut self, path: &std::path::Path) {
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("eye".into());
        let name = grm_connect::csv::name_of(&stem);
        let line = format!("see {} {name}", grm_shell::quoted(&path.display().to_string()));
        self.seeing.push(line.clone());
        self.say(line);
    }
}
