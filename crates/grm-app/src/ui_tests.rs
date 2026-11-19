// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The window's tests: the whole window, run in a home of its own (its world, its settings, its apps, her mind,
//! copied from the repository), with no sound card, clicked and typed into as a human would, and looked at
//! through what it says of itself (each widget's label). Each bug fixed is played again here.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use super::App;
use crate::Args;

/// A home of its own, emptied when the test ends. One window at a time: each runs its own engine and its own
/// sound, and several at once on a busy machine would make the waits fail, not the window.
pub struct Home {
    pub dir: PathBuf,
    _alone: std::sync::MutexGuard<'static, ()>,
}

static ONE_WINDOW: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn copy_dir(from: &Path, to: &Path) {
    let _ = std::fs::create_dir_all(to);
    let Ok(d) = std::fs::read_dir(from) else { return };
    for e in d.flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            let _ = std::fs::copy(&p, &q);
        }
    }
}

impl Home {
    pub fn new(name: &str) -> Home {
        let alone = ONE_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("grm-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for d in ["apps", "mind", "data", "widgets"] {
            copy_dir(&root.join(d), &dir.join(d));
        }
        // the visit is not made again: it was made
        std::fs::write(dir.join("window.json"), r#"{"toured": true}"#).unwrap();
        Home { dir, _alone: alone }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The window, opened in this home (in French, as Anthony uses it).
pub fn open(home: &Home) -> Harness<'static, App> {
    let args =
        Args { home: Some(home.dir.clone()), headless: true, lang: Some("fr".into()), wait: 3.0, ..Default::default() };
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        App::new(cc, args)
    });
    frames_until(&mut h, "the world's first picture", |a| a.picture_frame > 0 && a.apps_known && a.space_ready);
    h
}

/// Frames, in real time (the engine and the worker answer between them), until `done` holds.
pub fn frames_until(h: &mut Harness<'static, App>, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while !done(h.state()) {
        assert!(start.elapsed() < Duration::from_secs(30), "waited 30 s for {what}");
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    h.step();
}

/// Frames for a while (what comes by itself has come).
pub fn frames_for(h: &mut Harness<'static, App>, ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A line said as typed at the prompt, and frames until the worker has done it.
pub fn say(h: &mut Harness<'static, App>, line: &str) {
    h.state_mut().say(line);
    frames_for(h, 300);
}

/// A widget clicked by its words, once what was just opened has settled (a window takes a few frames to find its
/// size and place).
pub fn click(h: &mut Harness<'static, App>, label: &str) {
    frames_for(h, 250);
    h.get_by_label(label).click();
    frames_for(h, 100);
}

/// The window closed as by its cross: everything kept, the world asleep.
pub fn close(mut h: Harness<'static, App>) {
    let ctx = h.ctx.clone();
    h.state_mut().keep_all(&ctx);
    eframe::App::on_exit(h.state_mut(), None);
}

fn workshop_with_three_nodes(h: &mut Harness<'static, App>, name: &str) {
    say(h, &format!("workshop new {name}"));
    for line in ["node a = 1", "node b = 2", "node c = 3"] {
        say(h, &format!("workshop say {name} {line}"));
    }
    frames_until(h, "the workshop's three nodes", |a| a.picture.workshops.iter().any(|w| w.0 == name && w.1 == 3));
}

/// Played again: « Erreur après avoir supprimé un atelier … view.rs:106: index out of bounds: the len is 2 but
/// the index is 2 ». A workshop shown in the middle of the space (its network drawn), closed from its menu, then
/// the window opened again: nothing of it is left (the space, its zones, the world), and nothing breaks.
#[test]
fn a_workshop_closed_while_shown_leaves_nothing_behind_even_after_a_restart() {
    let home = Home::new("ws-close");
    let mut h = open(&home);
    workshop_with_three_nodes(&mut h, "fall");
    let kind = crate::space::Kind::Workshop("fall".into());
    assert_eq!(h.state().zone_center.as_ref(), Some(&kind), "a workshop made is shown at once");
    assert!(h.state().space.items.iter().any(|i| i.kind == kind));
    frames_for(&mut h, 500);
    // the main network drawn, then the workshop's, then the main one again: each its own view of its own picture
    say(&mut h, "node x = 1");
    say(&mut h, "node y = 2");
    frames_for(&mut h, 500);
    say(&mut h, "workshop close fall");
    frames_until(&mut h, "the workshop gone from the picture", |a| a.picture.workshops.is_empty());
    frames_for(&mut h, 500);
    let a = h.state();
    assert!(!a.space.items.iter().any(|i| i.kind == kind), "the space still has the workshop");
    assert_ne!(a.zone_center.as_ref(), Some(&kind));
    assert!(a.workshop_pulls.is_empty() && a.workshop_state.is_empty());
    close(h);
    let written = std::fs::read_to_string(home.dir.join("space.grm")).unwrap_or_default();
    assert!(!written.contains("workshop fall"), "space.grm still places it:\n{written}");
    // opened again: no workshop, nothing breaks
    let mut h = open(&home);
    frames_for(&mut h, 1500);
    let a = h.state();
    assert!(a.picture.workshops.is_empty(), "the world woke with the workshop closed before");
    assert!(!a.space.items.iter().any(|i| i.kind == kind));
    close(h);
}

/// Each picture is drawn from what was made for it, never for another one that stood in the same place before
/// (a workshop's picture is made again and again; the world's comes every 200 ms).
#[test]
fn what_a_view_needs_is_made_for_its_own_picture() {
    let home = Home::new("prepared");
    let mut h = open(&home);
    let picture = |names: &[&str]| {
        let nodes: Vec<crate::model::NodeView> =
            names.iter().map(|n| crate::model::NodeView { name: n.to_string(), ..Default::default() }).collect();
        let by_name = nodes.iter().enumerate().map(|(k, n)| (n.name.clone(), k)).collect();
        std::sync::Arc::new(crate::model::Picture { nodes, by_name, ..Default::default() })
    };
    for round in 0..200 {
        let names: Vec<String> = (0..(round % 5)).map(|k| format!("n{k}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let p = picture(&names);
        let len = p.nodes.len();
        let prep = h.state_mut().showing(p, |app| app.prepared(None));
        assert!(prep.nodes.iter().all(|&k| k < len), "round {round}: a view made for another picture");
        assert_eq!(prep.nodes.len(), len);
    }
    close(h);
}

/// Played again: « dans atelier, le status entré dans le sas ne persiste pas entre deux sessions » and « si
/// l'atelier est dans le SAS, ce bouton doit changer de texte, sauf si l'user édite cet atelier : alors le check
/// doit être refait ». The workshop's door, clicked; refused in the airlock; the window opened again: still
/// refused, said by the door; a line written in it: through the airlock again.
#[test]
fn where_a_workshop_is_on_its_way_in_is_kept_and_a_change_sends_it_through_the_airlock_again() {
    let home = Home::new("ws-airlock");
    let mut h = open(&home);
    workshop_with_three_nodes(&mut h, "fall");
    frames_for(&mut h, 500);
    click(&mut h, "Le faire entrer dans le réseau principal");
    frames_until(&mut h, "the airlock", |a| a.airlock.is_some());
    click(&mut h, "Refuser");
    frames_until(&mut h, "refused", |a| a.picture.passages.iter().any(|p| p.0 == "fall" && p.1 == "refused"));
    assert_eq!(crate::airlock::passage(h.state(), "fall"), Some(crate::airlock::Passage::Refused));
    assert!(h.query_by_label("Le faire entrer dans le réseau principal").is_none());
    assert!(h.query_by_label("Refusé : repasser par le sas").is_some());
    close(h);
    let mut h = open(&home);
    frames_until(&mut h, "the workshop again", |a| !a.picture.workshops.is_empty());
    assert_eq!(crate::airlock::passage(h.state(), "fall"), Some(crate::airlock::Passage::Refused), "forgotten");
    frames_for(&mut h, 300);
    assert!(h.query_by_label("Refusé : repasser par le sas").is_some());
    say(&mut h, "workshop say fall node d = 4");
    frames_until(&mut h, "changed", |a| a.picture.passages.is_empty());
    assert_eq!(crate::airlock::passage(h.state(), "fall"), None);
    frames_for(&mut h, 300);
    assert!(h.query_by_label("Le faire entrer dans le réseau principal").is_some());
    close(h);
}

/// Played again: « les boutons dans l'inspecteur, ceux à côté des boutons de sons, ne marchent pas ». A node
/// chosen, each of its buttons pressed: each does what it says.
#[test]
fn each_button_of_the_inspector_does_what_it_says() {
    let home = Home::new("inspector");
    let mut h = open(&home);
    say(&mut h, "node level = 1");
    say(&mut h, "node rate = 2");
    say(&mut h, "relate grow: level = rate * 3");
    frames_until(&mut h, "the nodes", |a| a.picture.node("level").is_some());
    h.state_mut().selected = vec!["level".into()];
    h.state_mut().tab = super::Tab::Inspector;
    h.state_mut().side_open = true;
    click(&mut h, "Suivre dans le temps");
    frames_until(&mut h, "tracked", |a| a.picture.node("level").is_some_and(|n| n.tracked));
    for _ in 0..30 {
        say(&mut h, "tick 5");
    }
    click(&mut h, "Prévoir");
    frames_until(&mut h, "a forecast", |a| a.detail.as_ref().is_some_and(|d| !d.forecast.is_empty()));
    click(&mut h, "Incarner");
    frames_until(&mut h, "embodied", |a| a.log.iter().any(|l| l.1.contains("embod")));
    click(&mut h, "Jouer");
    frames_until(&mut h, "heard", |a| {
        a.mixer.lock().map(|m| m.playing().contains(&"level".to_string())).unwrap_or(false)
    });
    click(&mut h, "Arrêter");
    frames_until(&mut h, "silent", |a| {
        a.mixer.lock().map(|m| !m.playing().contains(&"level".to_string())).unwrap_or(false)
    });
    close(h);
}

/// The signs beside a network's sound in the inspector's tree: to the front, and back again; each node's own
/// sound, played and stopped.
#[test]
fn the_signs_beside_the_sounds_of_the_tree_do_what_they_say() {
    let home = Home::new("tree-signs");
    let mut h = open(&home);
    say(&mut h, "node level = 1");
    say(&mut h, "node rate = 2");
    say(&mut h, "relate grow: level = rate * 3");
    h.state_mut().tab = super::Tab::Inspector;
    h.state_mut().side_open = true;
    // her network is always there: with the one typed, two
    frames_until(&mut h, "two networks", |a| a.picture.sources.len() == 2);
    frames_for(&mut h, 300);
    h.get_all_by_label("Le mettre au premier plan").next().unwrap().click();
    frames_until(&mut h, "one in front", |a| a.focus.is_some());
    click(&mut h, "Le remettre avec les autres");
    frames_until(&mut h, "all together again", |a| a.focus.is_none());
    // a node chosen: its row is shown, with its sound
    h.state_mut().selected = vec!["level".into()];
    frames_for(&mut h, 300);
    h.get_all_by_label("Jouer : son réseau tourné à la vitesse du son, entendu").next().unwrap().click();
    frames_until(&mut h, "a node heard", |a| a.mixer.lock().map(|m| !m.playing().is_empty()).unwrap_or(false));
    click(&mut h, "Arrêter : on ne l'entend plus");
    frames_until(&mut h, "silence", |a| a.mixer.lock().map(|m| m.playing().is_empty()).unwrap_or(false));
    close(h);
}

/// Played again: « il était sur Espace, il a cliqué sur "son code" et quand il a fermé, ça l'a ramené sur
/// Réseau » and « s'il retourne sur Espace puis reclique sur space.grm, le bouton étant encore sélectionné, ça ne
/// le ramène pas dessus ». One logic for every tab: lit when in front, in front when pressed, and what is closed
/// gives the place back to what was in front before it.
#[test]
fn every_tab_follows_one_logic_and_closing_goes_back_where_one_was() {
    use crate::front::Front;
    let home = Home::new("tabs");
    let mut h = open(&home);
    click(&mut h, "Espace");
    assert_eq!(h.state().front(), Front::Space);
    click(&mut h, "Son code");
    frames_until(&mut h, "space.grm in front", |a| matches!(a.front(), Front::File(ref p) if p.ends_with("space.grm")));
    // back to the space: the file's tab is no longer lit; pressed, it is in front again
    click(&mut h, "Espace");
    assert_eq!(h.state().front(), Front::Space);
    click(&mut h, "space.grm");
    assert!(matches!(h.state().front(), Front::File(_)), "the file's tab did not bring it back");
    // closed (its menu): the space again, not the network
    frames_for(&mut h, 200);
    h.get_by_label("space.grm").click_secondary();
    frames_for(&mut h, 250);
    // the menu's (the last drawn: it stands above the rest)
    h.get_all_by_label("Fermer").last().unwrap().click();
    frames_for(&mut h, 100);
    assert_eq!(h.state().front(), Front::Space, "closing the file did not give the space back");
    close(h);
}

/// Played again: « lors d'un changement de fenêtre, garder le réseau qui était sélectionné, garder la vue telle
/// qu'elle était ». The network looked at closely, a node chosen; the space, then the network again: as it was.
#[test]
fn a_tab_left_and_come_back_to_is_as_it_was_left() {
    use crate::front::Front;
    let home = Home::new("tab-view");
    let mut h = open(&home);
    say(&mut h, "node level = 1");
    say(&mut h, "node rate = 2");
    click(&mut h, "Réseau");
    frames_until(&mut h, "the nodes", |a| a.picture.node("level").is_some());
    frames_for(&mut h, 500);
    h.state_mut().selected = vec!["level".into()];
    h.state_mut().view.zoom = 2.5;
    h.state_mut().view.offset = egui::vec2(40.0, -30.0);
    h.state_mut().view.fitted = true;
    click(&mut h, "Espace");
    assert_eq!(h.state().front(), Front::Space);
    click(&mut h, "Réseau");
    let a = h.state();
    assert_eq!(a.selected, vec!["level".to_string()]);
    assert_eq!(a.view.zoom, 2.5);
    assert_eq!(a.view.offset, egui::vec2(40.0, -30.0));
    close(h);
}

/// Her name is the one the human gives her (Her, until they choose another): her tab says it.
#[test]
fn her_tab_says_the_name_she_was_given() {
    let home = Home::new("her-name");
    let mut h = open(&home);
    frames_until(&mut h, "her app", |a| a.apps.iter().any(|x| x.name == crate::elle::APP));
    frames_for(&mut h, 200);
    // her tab (and her zone on the space)
    assert!(h.query_all_by_label("Her").count() >= 1, "her tab is not called Her");
    h.state_mut().settings.her = "Lou".into();
    h.state_mut().name_her();
    frames_for(&mut h, 200);
    assert!(h.query_all_by_label("Lou").count() >= 1 && h.query_all_by_label("Her").count() == 0);
    close(h);
}

/// Widgets that hold widgets: tabs show one page at a time (chosen by its tab), a scroll shows what fits whole
/// and the rest further down, a kind of one's own is made once and used twice.
#[test]
fn widgets_hold_widgets_in_an_app() {
    let home = Home::new("boxes");
    let file = home.dir.join("apps").join("boxes.grm");
    let mut text = String::from(
        "app Boxes\n\
         kind page(n): column \"Page {n}\"\n\
           widget words: text \"page {n} here\"\n\
         end\n\
         widget main: column at 0 0 size 12 12\n\
           widget pages: tabs size 12 3\n\
             widget p1: page(n=1)\n\
             widget p2: page(n=2)\n\
           end\n\
           widget list: scroll size 12 4\n",
    );
    for k in 0..12 {
        text.push_str(&format!("    widget line{k}: text \"line {k}\" tip \"the line {k}\"\n"));
    }
    text.push_str("  end\nend\n");
    std::fs::write(&file, text).unwrap();
    fn h_open(home: &Home, file: &std::path::Path) -> Harness<'static, App> {
        let mut h = open(home);
        say(&mut h, &format!("app open {}", file.display()));
        frames_until(&mut h, "the app", |a| a.apps.iter().any(|x| x.name == "boxes"));
        h.state_mut().modes.insert("boxes".into(), super::Mode::App);
        h.state_mut().go(crate::front::Front::App("boxes".into()));
        frames_for(&mut h, 300);
        h
    }
    let mut h = h_open(&home, &file);
    let errors = h.state().apps.iter().find(|x| x.name == "boxes").map(|x| x.errors.clone()).unwrap_or_default();
    assert!(errors.is_empty(), "{errors:?}");
    assert!(h.query_by_label("page 1 here").is_some() && h.query_by_label("page 2 here").is_none());
    click(&mut h, "Page 2");
    assert!(h.query_by_label("page 2 here").is_some() && h.query_by_label("page 1 here").is_none());
    assert!(h.query_by_label("line 0").is_some() && h.query_by_label("line 11").is_none(), "all of the scroll shown");
    h.state_mut().surface.scrolls.insert("boxes/list".into(), 10_000.0);
    frames_for(&mut h, 200);
    assert!(h.query_by_label("line 11").is_some() && h.query_by_label("line 0").is_none(), "scrolled to its end");
    close(h);
}

/// Played again: « le copié-collé ne marche pas dans le formulaire, c'est à cause de la virgule » and « il manque
/// quoi ? ». A value written with a comma (pasted with its unit) is taken; what is missing is said.
#[test]
fn the_workshop_form_takes_a_comma_and_says_what_is_missing() {
    let home = Home::new("ws-form");
    let mut h = open(&home);
    say(&mut h, "workshop new fall");
    frames_until(&mut h, "the workshop", |a| a.picture.workshops.iter().any(|w| w.0 == "fall"));
    let form = |name: &str, value: &str| {
        let v: std::collections::HashMap<String, String> =
            [("name".to_string(), name.to_string()), ("value".to_string(), value.to_string())].into_iter().collect();
        crate::workshop::State { level: 1, form: Some(("node", v)), ..Default::default() }
    };
    h.state_mut().workshop_state.insert("fall".into(), form("2g", ""));
    frames_for(&mut h, 300);
    assert!(h.query_by_label_contains("il manque").is_some(), "what is missing is not said");
    h.state_mut().workshop_state.insert("fall".into(), form("g", "9,81 m/s²"));
    click(&mut h, "L'écrire");
    frames_until(&mut h, "the node written", |a| a.picture.workshops.iter().any(|w| w.0 == "fall" && w.1 == 1));
    let written = h.state().engine.query(|w| w.workshops["fall"].written.clone()).unwrap();
    assert_eq!(written, ["node g = 9.81"]);
    close(h);
}

/// A choice shows the one chosen; a field takes a number written with a comma when Enter is pressed.
#[test]
fn a_choice_and_a_field_in_an_app() {
    let home = Home::new("inputs");
    let file = home.dir.join("apps").join("inputs.grm");
    std::fs::write(&file, "app Inputs\ninput level = 1\nwidget c: choice level \"low|middle|high\" at 0 0 size 4 1\nwidget f: field level at 0 1 size 4 1\n").unwrap();
    let mut h = open(&home);
    say(&mut h, &format!("app open {}", file.display()));
    frames_until(&mut h, "the app", |a| a.apps.iter().any(|x| x.name == "inputs"));
    h.state_mut().modes.insert("inputs".into(), super::Mode::App);
    h.state_mut().go(crate::front::Front::App("inputs".into()));
    frames_for(&mut h, 400);
    let shown = |h: &Harness<'static, App>| {
        h.query_all_by_role(egui::accesskit::Role::ComboBox).filter_map(|n| n.value()).collect::<Vec<String>>()
    };
    assert_eq!(shown(&h), ["middle"], "the choice does not show the one chosen");
    let field = h.get_all_by_role(egui::accesskit::Role::TextInput).last().unwrap();
    field.focus();
    frames_for(&mut h, 100);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    frames_for(&mut h, 100);
    let field = h.get_all_by_role(egui::accesskit::Role::TextInput).last().unwrap();
    field.type_text("2,5");
    frames_for(&mut h, 100);
    h.key_press(egui::Key::Enter);
    frames_until(&mut h, "level 2.5", |a| a.picture.node("level").is_some_and(|n| (n.value - 2.5).abs() < 1e-6));
    frames_for(&mut h, 300);
    assert_eq!(shown(&h), ["high"], "the choice follows its node (2,5 is the third)");
    close(h);
}

/// A list in an app's file grows from a button: the line it says changes the list in the file, and what the list
/// makes follows at once (a node and a widget for the new one).
#[test]
fn a_button_adds_to_a_list_and_what_the_list_makes_follows() {
    let home = Home::new("lists");
    let file = home.dir.join("apps").join("rows.grm");
    std::fs::write(
        &file,
        "app Rows\nlist rows = r1 r2\nrepeat r in rows: input {r}_v = {r.k}\n\
         widget col: column at 0 0 size 6 6\n  repeat r in rows: widget {r}w: value {r}_v \"{r}\"\nend\n\
         widget add: button \"one more\" icon plus say \"list {app} rows add next\" at 6 0 size 3 1\n",
    )
    .unwrap();
    let mut h = open(&home);
    say(&mut h, &format!("app open {}", file.display()));
    frames_until(&mut h, "the app", |a| a.apps.iter().any(|x| x.name == "rows"));
    h.state_mut().modes.insert("rows".into(), super::Mode::App);
    h.state_mut().go(crate::front::Front::App("rows".into()));
    frames_until(&mut h, "r2_v", |a| a.picture.node("r2_v").is_some());
    click(&mut h, "one more");
    frames_until(&mut h, "r3_v", |a| a.picture.node("r3_v").is_some_and(|n| (n.value - 3.0).abs() < 1e-9));
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("list rows = r1 r2 r3"), "{written}");
    frames_for(&mut h, 300);
    let col = h.state().apps.iter().find(|x| x.name == "rows").map(|x| x.widgets[0].children.len());
    assert_eq!(col, Some(3), "the new one's widget is not there");
    close(h);
}

/// The studio is its file: « + synth » adds a synthesiser to its list, and its network, its panel, its strip and
/// its place in the mix follow (no song of the window, no file written by it).
#[test]
fn the_studio_grows_a_synthesiser_from_its_own_file() {
    let home = Home::new("studio");
    let mut h = open(&home);
    let file = home.dir.join("apps").join("studio.grm");
    say(&mut h, &format!("app open {}", file.display()));
    frames_until(&mut h, "the studio", |a| a.picture.node("t3_out").is_some());
    h.state_mut().modes.insert("studio".into(), super::Mode::App);
    h.state_mut().go(crate::front::Front::App("studio".into()));
    frames_for(&mut h, 500);
    click(&mut h, "synth");
    frames_until(&mut h, "t4", |a| a.picture.node("t4_out").is_some() && a.picture.node("t4_l").is_some());
    frames_for(&mut h, 500);
    let at = h.state().apps.iter().find(|x| x.name == "studio").map(|x| x.path.clone());
    assert_eq!(at.as_ref(), Some(&file));
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.contains("list my_synths = t1 t4"),
        "{:?}",
        written.lines().filter(|l| l.starts_with("list")).collect::<Vec<_>>()
    );
    let studio = h.state().apps.iter().find(|x| x.name == "studio").cloned().unwrap();
    let names: Vec<String> = grm_shell::apps::all_widgets(&studio.widgets).iter().map(|w| w.name.clone()).collect();
    assert!(names.iter().any(|n| n == "t4sec") && names.iter().any(|n| n == "t4mix_level"), "{names:?}");
    assert!(h.state().synth_names().iter().any(|s| s == "studio|t4_"));
    close(h);
}

/// The studio's transport and her band are GRML (widgets/transport.grm, widgets/her.grm): the space bar plays and
/// stops; a key given to her is written to her node, and shown as given.
#[test]
fn the_studio_plays_by_the_space_bar_and_her_key_is_given_by_its_choice() {
    let home = Home::new("transport");
    let mut h = open(&home);
    say(&mut h, &format!("app open {}", home.dir.join("apps").join("studio.grm").display()));
    frames_until(&mut h, "the studio", |a| a.picture.node("elle_key_pick").is_some());
    h.state_mut().modes.insert("studio".into(), super::Mode::App);
    h.state_mut().go(crate::front::Front::App("studio".into()));
    frames_for(&mut h, 500);
    let playing = |a: &App| a.mixer.lock().map(|m| m.playing().iter().any(|p| p == "out")).unwrap_or(false);
    assert!(!playing(h.state()), "nothing plays before play: she does not either");
    // nothing being typed (the prompt let go, as a click elsewhere does)
    h.ctx.memory_mut(|m| m.stop_text_input());
    frames_for(&mut h, 100);
    h.key_press(egui::Key::Space);
    frames_until(&mut h, "playing", playing);
    h.key_press(egui::Key::Space);
    frames_until(&mut h, "stopped", |a| !playing(a));
    frames_for(&mut h, 250);
    h.query_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|n| n.value().as_deref() == Some("key: hers"))
        .expect("her key's choice")
        .click();
    // the first ones of the list are in view (the list scrolls)
    frames_for(&mut h, 250);
    h.get_all_by_label("key: C").next().unwrap().click();
    frames_until(&mut h, "her key", |a| a.picture.node("elle_key_pick").is_some_and(|n| (n.value - 3.0).abs() < 1e-9));
    let shown: Vec<String> = h.query_all_by_role(egui::accesskit::Role::ComboBox).filter_map(|n| n.value()).collect();
    assert!(shown.iter().any(|s| s == "key: C"), "{shown:?}");
    close(h);
}

/// The 3D view (a primitive called from GRML): an object chosen by a click is written to its node, moved by the
/// arrows (its nodes written back), let go by Escape; F fits everything in view.
#[test]
fn the_3d_view_chooses_moves_and_lets_go() {
    let home = Home::new("view3d");
    let mut h = open(&home);
    say(&mut h, &format!("app open {}", home.dir.join("apps").join("gallery.grm").display()));
    frames_until(&mut h, "the gallery", |a| a.picture.node("earth_x").is_some());
    h.state_mut().modes.insert("gallery".into(), super::Mode::App);
    h.state_mut().go(crate::front::Front::App("gallery".into()));
    h.state_mut().surface.tabs.insert("gallery/pages".into(), 5);
    h.ctx.memory_mut(|m| m.stop_text_input());
    frames_for(&mut h, 500);
    click(&mut h, "earth");
    frames_until(&mut h, "earth chosen", |a| a.picture.node("g_chosen").is_some_and(|n| (n.value - 2.0).abs() < 1e-9));
    let x0 = h.state().picture.node("earth_x").unwrap().value;
    // the pointer over the view, the arrow pressed: the earth moves on the ground
    let at = h.get_by_label("3D view").rect().center();
    h.hover_at(at);
    frames_for(&mut h, 100);
    h.key_press(egui::Key::ArrowRight);
    frames_until(&mut h, "earth moved", |a| a.picture.node("earth_x").is_some_and(|n| (n.value - x0).abs() > 1e-6));
    h.key_press(egui::Key::Escape);
    frames_until(&mut h, "let go", |a| a.picture.node("g_chosen").is_some_and(|n| n.value.abs() < 1e-9));
    close(h);
}
