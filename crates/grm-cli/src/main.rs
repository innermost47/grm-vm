// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! grm: a prompt to talk to the machine. The machine runs in its own thread (grm-engine's Engine); the prompt
//! sends it what is typed and reads its snapshots, so it never waits for it to compute. The commands themselves
//! live in grm-shell (the same in the window).
//!
//!   grm [FILE.grm]      says the file, then gives the prompt

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grm_engine::engine::Engine;
use grm_engine::Provenance;
use grm_shell::{Kind, Line, Shell};
use grm_sound::mixer::Mixer;

const RED: &str = "\x1b[31m";
const AMBER: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

fn print(line: &Line) {
    let colour = match line.kind {
        Kind::Said => "",
        Kind::Dim => DIM,
        Kind::Warning | Kind::Ask => AMBER,
        Kind::Error => RED,
    };
    // the window's colour marks: the banner, a program, a table
    let (colour, text) = match line.text.chars().next() {
        Some('\u{1}') => (AMBER, &line.text[1..]),
        Some('\u{2}') => ("\x1b[35m", &line.text[1..]),
        Some('\u{3}') => ("\x1b[36m", &line.text[1..]),
        _ => (colour, line.text.as_str()),
    };
    let reset = if colour.is_empty() { "" } else { RESET };
    println!("  {colour}{text}{reset}");
}

fn main() {
    let path = grm_shell::memory();
    // it wakes where it fell asleep (or is born)
    let (world, said) = grm_shell::wake_or_born(&path);
    if let Some(l) = &said {
        print(l);
    }
    let engine = Engine::start(world, Duration::from_millis(8));
    // the sound: a device if there is one (else the mixer is only rendered into files)
    let mixer = Arc::new(Mutex::new(Mixer::new(48000)));
    let _output = match grm_sound::output::open(mixer.clone()) {
        Ok(o) => Some(o),
        Err(e) => {
            print(&Line { kind: Kind::Dim, text: format!("no sound device ({e}): render still writes files") });
            None
        }
    };
    let mut shell = Shell::new(path.clone(), mixer);
    if let Some(file) = std::env::args().nth(1) {
        match engine.say(&format!("use {file}"), Provenance::of(format!("file:{file}"))) {
            Ok(s) => println!("  {s}"),
            Err(e) => println!("  {RED}! {e}{RESET}"),
        }
    }
    println!("grm-vm {} — the GRM machine. help: what it understands.", env!("CARGO_PKG_VERSION"));
    let stdin = io::stdin();
    loop {
        print!("grm> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let (out, quit) = shell.run(&engine, &line);
        out.iter().for_each(print);
        // what the machine feels moves its voice (the window does it a few times a second; here, at each line)
        shell.feel(&engine);
        if quit {
            break;
        }
    }
    // it falls asleep: everything it is, kept for the next time
    let world = engine.stop();
    match world.sleep(&path) {
        Ok(()) => println!("  {DIM}asleep in {}{RESET}", path.display()),
        Err(e) => println!("  {RED}! {e}{RESET}"),
    }
}
