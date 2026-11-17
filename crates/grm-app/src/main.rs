// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! grm-app: the GRM machine in a window. Its network, a prompt, an inspector (value ± sigma, where it comes from,
//! its relations, its time and why), the sandboxes, the sources and the quarantine, the journal, the eye, the
//! sound. The panels are placed by a small GRM network of their own.
//!
//!   grm-app [FILE.grm]
//!   grm-app --fresh --script LINES.txt --select a,b --tab sound --size 1280x800 --theme light --lang fr
//!           --app NAME --mode code|network|app --edit --xray --launch
//!           --screenshot OUT.png [--wait SECONDS]       (a picture of the window, then it closes: for tests)

mod airlock;
mod app;
mod code;
mod desk;
mod files;
mod flow;
mod french;
mod graph;
mod help;
mod hover;
mod icons;
mod improvise;
mod insights;
mod journey;
mod layout;
mod looks;
mod math;
mod menu;
mod midi;
mod model;
mod music;
mod panels;
mod plot;
mod settings;
mod space;
mod style;
mod surface;
mod text;
mod tour;
mod tree;
mod view;
mod worker;
mod workshop;
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
            "--wait" => a.wait = it.next().and_then(|s| s.parse().ok()).unwrap_or(3.0),
            _ => a.file = Some(w),
        }
    }
    a
}

/// The machine's sign for the task bar: a small network of five nodes, amber on night.
fn icon() -> eframe::egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let nodes =
        [(20.0f32, 14.0f32), (44.0, 14.0), (10.0, 34.0), (32.0, 34.0), (54.0, 34.0), (20.0, 52.0), (44.0, 52.0)];
    let links = [(0, 1), (0, 2), (0, 3), (1, 3), (1, 4), (2, 3), (3, 4), (2, 5), (3, 5), (3, 6), (4, 6), (5, 6)];
    let seg = |px: f32, py: f32, a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
    };
    for y in 0..n {
        for x in 0..n {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // a rounded square of night
            let (cx, cy) = ((px - 32.0).abs() - 22.0, (py - 32.0).abs() - 22.0);
            let corner = (cx.max(0.0).powi(2) + cy.max(0.0).powi(2)).sqrt() + cx.max(cy).min(0.0) - 9.0;
            let mut c = [0u8, 0, 0, 0];
            if corner < 0.0 {
                c = [0x1F, 0x20, 0x24, 255];
            }
            let line = links.iter().map(|&(a, b)| seg(px, py, nodes[a], nodes[b])).fold(f32::INFINITY, f32::min);
            if line < 1.3 && corner < 0.0 {
                c = [0x9C, 0x9F, 0xA6, 255];
            }
            let dot = nodes
                .iter()
                .map(|&(nx, ny)| ((px - nx).powi(2) + (py - ny).powi(2)).sqrt())
                .fold(f32::INFINITY, f32::min);
            if dot < 5.0 {
                c = [0xE8, 0xA8, 0x3D, 255];
            }
            rgba[(y * n + x) * 4..(y * n + x) * 4 + 4].copy_from_slice(&c);
        }
    }
    eframe::egui::IconData { rgba, width: n as u32, height: n as u32 }
}

fn main() -> eframe::Result<()> {
    let args = args();
    let (w, h) = args.size.unwrap_or((1280.0, 800.0));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([w, h])
            .with_min_inner_size([360.0, 420.0])
            .with_title("grm-vm")
            .with_app_id("grm-vm")
            .with_icon(std::sync::Arc::new(icon())),
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
