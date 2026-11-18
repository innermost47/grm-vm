// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The player: the music of a folder (the human's Music folder unless another is chosen), lists of their own, one
//! file after another on a channel of the mixer. She hears it (the room is her ear).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use eframe::egui::{self as egui};
use eframe::egui::{pos2, vec2, RichText, Sense};
use grm_sound::player::{FileSource, Playing};
use serde::{Deserialize, Serialize};

use crate::app::{App, Ink};
use crate::text::t;

/// The player's app (its file is written by the window: its surface is the window's).
pub const APP: &str = "player";
/// Its channel on the mixer.
pub const CHANNEL: &str = "player";

/// What the player keeps beside the memory.
#[derive(Serialize, Deserialize, Clone)]
pub struct Saved {
    pub folder: Option<PathBuf>,
    pub lists: Vec<(String, Vec<PathBuf>)>,
    pub volume: f64,
    pub repeat: bool,
    /// One after another in a mixed order (by a fixed stride, never by chance).
    pub mixed: bool,
}

impl Default for Saved {
    fn default() -> Self {
        Saved { folder: None, lists: vec![], volume: 0.8, repeat: true, mixed: false }
    }
}

#[derive(Default)]
pub struct Player {
    pub saved: Saved,
    /// The folder's music (read beside the window).
    pub files: Arc<Mutex<Option<Vec<PathBuf>>>>,
    reading: bool,
    /// Which list is shown and played: None, the folder.
    pub list: Option<usize>,
    /// Which of it plays (its place in the order played).
    pub at: Option<usize>,
    pub playing: Option<Arc<Playing>>,
    pub filter: String,
    pub new_list: String,
    loaded: bool,
}

/// Where the player keeps its lists.
fn saved_path(memory: &Path) -> PathBuf {
    memory.with_extension("player.json")
}

/// The human's Music folder (Musique, Music), wherever the system keeps it.
pub fn music_folder() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)?;
    ["Music", "Musique", "music"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).or(Some(home))
}

/// Every music file under a folder (sorted by path; at most twenty thousand, eight folders deep).
pub fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            if p.is_dir() {
                if depth < 8 {
                    stack.push((p, depth + 1));
                }
            } else if grm_sound::player::is_music(&p) {
                out.push(p);
                if out.len() >= 20_000 {
                    out.sort();
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

/// The order a list is played in: as it is, or mixed by a fixed stride (about the golden ratio of its length, made
/// prime with it): every file once, never twice in a row, the same order each time.
pub fn order(n: usize, mixed: bool) -> Vec<usize> {
    if !mixed || n < 3 {
        return (0..n).collect();
    }
    let gcd = |mut a: usize, mut b: usize| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let mut step = ((n as f64) * 0.618_033_988_75).round() as usize;
    step = step.clamp(1, n - 1);
    while gcd(step, n) != 1 {
        step += 1;
    }
    (0..n).map(|k| (k * step) % n).collect()
}

fn title_of(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

impl Player {
    fn load(&mut self, memory: &Path) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Ok(text) = std::fs::read_to_string(saved_path(memory)) {
            if let Ok(s) = serde_json::from_str::<Saved>(&text) {
                self.saved = s;
            }
        }
        self.read_folder();
    }

    fn save(&self, memory: &Path) {
        let _ = std::fs::write(saved_path(memory), serde_json::to_string_pretty(&self.saved).unwrap_or_default());
    }

    /// The folder read again, beside the window.
    pub fn read_folder(&mut self) {
        let Some(dir) = self.saved.folder.clone().or_else(music_folder) else { return };
        let files = self.files.clone();
        if let Ok(mut f) = files.lock() {
            *f = None;
        }
        self.reading = true;
        let _ = std::thread::Builder::new().name("grm-music".into()).spawn(move || {
            let found = scan(&dir);
            if let Ok(mut f) = files.lock() {
                *f = Some(found);
            }
        });
    }

    /// What the list shown holds.
    fn tracks(&self) -> Vec<PathBuf> {
        match self.list {
            Some(i) => self.saved.lists.get(i).map(|l| l.1.clone()).unwrap_or_default(),
            None => self.files.lock().ok().and_then(|f| f.clone()).unwrap_or_default(),
        }
    }

    /// Plays the file at this place of the order (the one before leaves).
    fn play_at(&mut self, app_mixer: &Arc<Mutex<grm_sound::mixer::Mixer>>, k: usize) {
        let tracks = self.tracks();
        let ord = order(tracks.len(), self.saved.mixed);
        let Some(&i) = ord.get(k) else { return };
        let Some(path) = tracks.get(i).cloned() else { return };
        if let Ok(mut m) = app_mixer.lock() {
            let rate = m.rate;
            let src = FileSource::open(&path, rate);
            self.playing = Some(src.playing.clone());
            m.play(CHANNEL, Box::new(src), self.saved.volume);
        }
        self.at = Some(k);
    }

    fn stop(&mut self, mixer: &Arc<Mutex<grm_sound::mixer::Mixer>>) {
        if let Ok(mut m) = mixer.lock() {
            m.stop(CHANNEL);
        }
        self.playing = None;
    }

    /// Goes on by itself: the next when one ends (and from the start again, when it repeats).
    pub fn tick(&mut self, mixer: &Arc<Mutex<grm_sound::mixer::Mixer>>) {
        let Some(p) = &self.playing else { return };
        if !p.ended.load(Ordering::Relaxed) {
            return;
        }
        let n = self.tracks().len();
        let next = self.at.map(|k| k + 1).unwrap_or(0);
        if next < n {
            self.play_at(mixer, next);
        } else if self.saved.repeat && n > 0 {
            self.play_at(mixer, 0);
        } else {
            self.stop(mixer);
        }
    }
}

/// The player's surface.
pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let ink = Ink::of(ui.ctx());
    let lang = app.settings.lang;
    let memory = app.memory.clone();
    let mixer = app.mixer.clone();
    let p = &mut app.player;
    p.load(&memory);
    p.tick(&mixer);
    let rate = mixer.lock().map(|m| m.rate).unwrap_or(48_000);
    let tracks = p.tracks();
    let ord = order(tracks.len(), p.saved.mixed);
    let mut changed = false;

    // what plays: its title, where it is, the controls
    egui::Frame::new().fill(ink.panel).corner_radius(6.0).inner_margin(10.0).show(ui, |ui| {
        let now = p.playing.clone();
        let title = now
            .as_ref()
            .map(|n| n.title.lock().ok().and_then(|t| t.clone()).unwrap_or_else(|| title_of(&n.path)))
            .unwrap_or_else(|| t(lang, "player_nothing").to_string());
        ui.horizontal(|ui| {
            crate::icons::show(ui, "disc-3", 18.0, ink.accent);
            ui.label(RichText::new(title).size(16.0).strong());
        });
        if let Some(n) = &now {
            if let Some(e) = n.error.lock().ok().and_then(|e| e.clone()) {
                ui.label(RichText::new(e).small().color(ink.amber));
            }
            let (at, len) = n.seconds(rate);
            let clock = |s: f64| format!("{}:{:02}", (s / 60.0).floor() as u64, (s % 60.0).floor() as u64);
            ui.horizontal(|ui| {
                ui.label(RichText::new(clock(at)).monospace().small());
                let w = (ui.available_width() - 60.0).max(80.0);
                let (rect, r) = ui.allocate_exact_size(vec2(w, 8.0), Sense::click_and_drag());
                ui.painter().rect_filled(rect, 3.0, ink.surface);
                if len > 0.0 {
                    let f = (at / len).clamp(0.0, 1.0) as f32;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(rect.min, vec2(rect.width() * f, rect.height())),
                        3.0,
                        ink.accent.gamma_multiply(0.8),
                    );
                    if let Some(pos) = r.interact_pointer_pos().filter(|_| r.clicked() || r.drag_stopped()) {
                        let f = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
                        n.seek(f * len, rate);
                    }
                }
                ui.label(RichText::new(if len > 0.0 { clock(len) } else { "–".into() }).monospace().small());
            });
        }
        ui.horizontal(|ui| {
            let at = p.at.unwrap_or(0);
            if crate::icons::button(ui, "skip-back", "").on_hover_text(t(lang, "player_prev")).clicked()
                && !ord.is_empty()
            {
                p.play_at(&mixer, at.saturating_sub(1));
            }
            let paused = now.as_ref().is_none_or(|n| n.paused.load(Ordering::Relaxed));
            let (icon, tip) = if paused { ("play", "player_play") } else { ("pause", "player_pause") };
            if crate::icons::button(ui, icon, "").on_hover_text(t(lang, tip)).clicked() {
                match &now {
                    Some(n) => n.paused.store(!paused, Ordering::Relaxed),
                    None if !ord.is_empty() => p.play_at(&mixer, at),
                    None => {}
                }
            }
            if crate::icons::button(ui, "square", "").on_hover_text(t(lang, "player_stop")).clicked() {
                p.stop(&mixer);
            }
            if crate::icons::button(ui, "skip-forward", "").on_hover_text(t(lang, "player_next")).clicked()
                && !ord.is_empty()
            {
                p.play_at(&mixer, (at + 1) % ord.len());
            }
            ui.add_space(12.0);
            if ui
                .add(egui::Button::new(t(lang, "player_repeat")).selected(p.saved.repeat))
                .on_hover_text(t(lang, "player_repeat_d"))
                .clicked()
            {
                p.saved.repeat = !p.saved.repeat;
                changed = true;
            }
            if ui
                .add(egui::Button::new(t(lang, "player_mixed")).selected(p.saved.mixed))
                .on_hover_text(t(lang, "player_mixed_d"))
                .clicked()
            {
                p.saved.mixed = !p.saved.mixed;
                p.at = None;
                changed = true;
            }
            ui.add_space(12.0);
            crate::icons::show(ui, "volume-2", 14.0, ink.muted);
            let mut v = p.saved.volume;
            if ui.add(egui::Slider::new(&mut v, 0.0..=1.5).show_value(false)).changed() {
                p.saved.volume = v;
                if let Ok(mut m) = mixer.lock() {
                    if let Some(c) = m.channels.get_mut(CHANNEL) {
                        c.gain = v;
                    }
                }
                changed = true;
            }
        });
        // she hears it: a word of how
        let heard = mixer.lock().ok().and_then(|m| m.world_value("elle_hon").map(|_| m.world_value("rv_sglow")));
        if let Some(Some(g)) = heard {
            ui.label(RichText::new(format!("{} ({:.2})", t(lang, "player_she_hears"), g)).small().color(ink.muted));
        }
    });
    ui.add_space(6.0);

    // the lists: the folder, and the human's own
    ui.horizontal_wrapped(|ui| {
        let folder_name = p
            .saved
            .folder
            .clone()
            .or_else(music_folder)
            .and_then(|f| f.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "?".into());
        if ui.add(egui::Button::new(format!("📁 {folder_name}")).selected(p.list.is_none())).clicked() {
            p.list = None;
            p.at = None;
        }
        for i in 0..p.saved.lists.len() {
            let name = p.saved.lists[i].0.clone();
            let r = ui.add(egui::Button::new(&name).selected(p.list == Some(i)));
            if r.clicked() {
                p.list = Some(i);
                p.at = None;
            }
            r.context_menu(|ui| {
                if ui.button(t(lang, "player_delete_list")).clicked() {
                    p.saved.lists.remove(i);
                    p.list = None;
                    changed = true;
                    ui.close();
                }
            });
        }
        ui.add(egui::TextEdit::singleline(&mut p.new_list).hint_text(t(lang, "player_new_list")).desired_width(120.0));
        if crate::icons::button(ui, "list-plus", "").on_hover_text(t(lang, "player_new_list_d")).clicked()
            && !p.new_list.trim().is_empty()
        {
            p.saved.lists.push((p.new_list.trim().to_string(), vec![]));
            p.new_list.clear();
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        if crate::icons::button(ui, "folder-open", t(lang, "player_folder")).clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                p.saved.folder = Some(dir);
                p.list = None;
                p.read_folder();
                changed = true;
            }
        }
        if crate::icons::button(ui, "refresh-cw", t(lang, "player_read_again")).clicked() {
            p.read_folder();
        }
        ui.add(egui::TextEdit::singleline(&mut p.filter).hint_text(t(lang, "player_filter")).desired_width(180.0));
    });
    if p.list.is_none() && p.files.lock().map(|f| f.is_none()).unwrap_or(false) {
        ui.label(RichText::new(t(lang, "player_reading")).small().color(ink.muted));
    }
    let _ = p.reading;
    ui.separator();

    // the list itself, in the order it is played
    let filter = p.filter.to_lowercase();
    let playing_path = p.playing.as_ref().map(|n| n.path.clone());
    let lists: Vec<String> = p.saved.lists.iter().map(|l| l.0.clone()).collect();
    let shown_list = p.list;
    let mut play: Option<usize> = None;
    let mut add_to: Option<(usize, PathBuf)> = None;
    let mut remove: Option<usize> = None;
    let out = egui::ScrollArea::vertical().id_salt("player-list").auto_shrink([false, false]).show(ui, |ui| {
        for (k, &i) in ord.iter().enumerate() {
            let path = &tracks[i];
            let name = title_of(path);
            if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                continue;
            }
            let here = playing_path.as_ref() == Some(path);
            // the whole row answers the mouse: a lighter ground, the hand, a play sign in place of its number;
            // a double click plays it, the right button offers the rest
            let ground = ui.painter().add(egui::Shape::Noop);
            let row = ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                ui.add_space(4.0);
                let number = ui.label(RichText::new(format!("{:>3}", k + 1)).monospace().small().color(ink.muted));
                let text = RichText::new(&name).color(if here { ink.accent } else { ink.text });
                ui.add(egui::Label::new(text).truncate().selectable(false));
                number.rect
            });
            let rect = row.response.rect.expand2(vec2(0.0, 2.0));
            let r = ui.interact(rect, ui.id().with(("player-row", k)), Sense::click());
            if r.hovered() || here {
                let tint = if r.hovered() { 0.16 } else { 0.08 };
                ui.painter().set(ground, egui::Shape::rect_filled(rect, 4.0, ink.accent.gamma_multiply(tint)));
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                let n = row.inner;
                ui.painter().rect_filled(n, 2.0, ink.panel);
                crate::icons::image("play", 12.0, ink.accent)
                    .paint_at(ui, egui::Rect::from_center_size(n.right_center() - vec2(6.0, 0.0), vec2(12.0, 12.0)));
            }
            if r.double_clicked() {
                play = Some(k);
            }
            r.on_hover_text(t(lang, "player_row_d")).context_menu(|ui| {
                if ui.button(t(lang, "player_play")).clicked() {
                    play = Some(k);
                    ui.close();
                }
                for (j, l) in lists.iter().enumerate() {
                    if shown_list != Some(j) && ui.button(format!("{} {l}", t(lang, "player_add_to"))).clicked() {
                        add_to = Some((j, path.clone()));
                        ui.close();
                    }
                }
                if shown_list.is_some() && ui.button(t(lang, "player_remove")).clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
        }
        if ord.is_empty() {
            ui.label(RichText::new(t(lang, "player_empty")).small().color(ink.muted));
        }
    });
    crate::space::wheel_to(ui, &out);
    if let Some(k) = play {
        p.play_at(&mixer, k);
    }
    if let Some((j, path)) = add_to {
        if let Some(l) = p.saved.lists.get_mut(j) {
            l.1.push(path);
            changed = true;
        }
    }
    if let (Some(i), Some(j)) = (remove, shown_list) {
        if let Some(l) = p.saved.lists.get_mut(j) {
            if i < l.1.len() {
                l.1.remove(i);
                changed = true;
            }
        }
    }
    if changed {
        p.save(&memory);
    }
    let _ = pos2(0.0, 0.0);
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_plays_each_file_once_in_the_same_order_each_time() {
        for n in [3usize, 4, 10, 12, 97, 100] {
            let o = order(n, true);
            let mut seen = o.clone();
            seen.sort();
            assert_eq!(seen, (0..n).collect::<Vec<_>>(), "n = {n}");
            assert_eq!(o, order(n, true));
            assert!(o.windows(2).all(|w| w[0] + 1 != w[1]), "not as it was: {o:?}");
        }
        assert_eq!(order(5, false), vec![0, 1, 2, 3, 4]);
    }
}
