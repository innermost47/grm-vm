// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! grm-app: the GRM machine in a window. Its network, a prompt, an inspector (value ± sigma, where it comes from,
//! its relations, its time and why), the sandboxes, the sources and the quarantine, the journal, the
//! sound. The panels are placed by a small GRM network of their own.
//!
//!   grm-app [FILE.grm]
//!   grm-app --fresh --script LINES.txt --select a,b --tab sound --size 1280x800 --theme light --lang fr
//!           --app NAME --mode code|network|app --edit --xray --launch
//!           --screenshot OUT.png [--wait SECONDS]       (a picture of the window, then it closes: for tests)

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod airlock;
mod app;
mod code;
mod desk;
mod elle;
mod explain;
mod files;
mod flow;
mod french;
mod front;
mod graph;
mod help;
mod hover;
mod icons;
mod insights;
mod journey;
mod layout;
mod looks;
mod math;
mod menu;
mod midi;
mod model;
mod notes;
mod panels;
mod player;
mod plot;
mod prepared;
mod screen;
mod settings;
mod space;
mod studio;
mod style;
mod surface;
mod text;
mod tour;
mod tree;
mod turns;
mod universe;
mod view;
mod view3d;
mod widgets;
mod worker;
mod zone;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub file: Option<String>,
    pub fresh: bool,
    pub script: Option<String>,
    pub select: Vec<String>,
    pub tab: Option<String>,
    pub size: Option<(f32, f32)>,
    pub theme: Option<String>,
    pub lang: Option<String>,
    pub screenshot: Option<String>,
    pub wait: f64,
    /// The app shown, how (code, network, app), edited, x-rayed, launched.
    pub app: Option<String>,
    pub mode: Option<String>,
    pub edit: bool,
    pub xray: bool,
    pub launch: bool,
    /// A folder that holds all it keeps (its world, its settings, its apps beside): a home of its own (`--home DIR`,
    /// and the window's tests, each in its own).
    pub home: Option<std::path::PathBuf>,
    /// No sound card (the tests): the sound is computed all the same, heard by nobody.
    pub headless: bool,
}

fn args() -> Args {
    let mut a = Args { wait: 3.0, ..Default::default() };
    let mut it = std::env::args().skip(1);
    while let Some(w) = it.next() {
        match w.as_str() {
            "--fresh" => a.fresh = true,
            "--script" => a.script = it.next(),
            "--select" => {
                a.select = it.next().map(|s| s.split(',').map(|x| x.to_string()).collect()).unwrap_or_default()
            }
            "--tab" => a.tab = it.next(),
            "--size" => {
                a.size = it.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
            }
            "--theme" => a.theme = it.next(),
            "--lang" => a.lang = it.next(),
            "--screenshot" => a.screenshot = it.next(),
            "--app" => a.app = it.next(),
            "--mode" => a.mode = it.next(),
            "--edit" => a.edit = true,
            "--xray" => a.xray = true,
            "--launch" => a.launch = true,
            "--home" => a.home = it.next().map(std::path::PathBuf::from),
            "--headless" => a.headless = true,
            "--wait" => a.wait = it.next().and_then(|s| s.parse().ok()).unwrap_or(3.0),
            _ => a.file = Some(w),
        }
    }
    a
}

/// Load the application window icon from the assets folder.
fn load_icon() -> Option<eframe::egui::IconData> {
    let bytes = include_bytes!("../../../assets/icon.png");
    eframe::icon_data::from_png_bytes(bytes).ok()
}

fn main() -> eframe::Result<()> {
    let args = args();
    let (w, h) = args.size.unwrap_or((1280.0, 800.0));
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([w, h])
        .with_min_inner_size([360.0, 420.0])
        .with_title("grm-vm")
        .with_app_id("grm-vm");
    if let Some(app_icon) = load_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(app_icon));
    }
  let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "grm-vm",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc, args)))
        }),
    )
}
