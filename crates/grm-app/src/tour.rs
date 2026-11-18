// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A guided visit, one part of the window at a time: that part lit, the rest dimmed, a card saying what it is and
//! what to try. It comes the first time, and again whenever asked (`tour`, or the help).

use eframe::egui::{self, pos2, vec2, Color32, Rect, RichText};

use crate::app::{App, Ink};
use crate::text::Lang;

/// (the part lit, title, words) in English and in French.
const STEPS: &[(&str, &str, &str, &str, &str)] = &[
    ("all", "Welcome", "This is the GRM machine: a network of nodes and relations that settles by itself, with how sure it is of everything. Nothing is hidden: what it computes, from where, and why.",
        "Bienvenue", "Voici la machine GRM : un réseau de nœuds et de relations qui se pose tout seul, avec à quel point il est sûr de chaque chose. Rien n'est caché : ce qu'il calcule, d'où ça vient, et pourquoi."),
    ("central", "The network", "Circles are nodes (filled: measured; hollow: computed), diamonds are relations. A colour per network. Rest the pointer on one: its value, its equation, its curve as it moves.",
        "Le réseau", "Les ronds sont des nœuds (pleins : mesurés ; creux : calculés), les losanges des relations. Une couleur par réseau. Poser le pointeur dessus : sa valeur, son équation, sa courbe en mouvement."),
    ("central", "By hand", "Right click: what can be done, with words. Drag with the right button from a node to another: a relation. Drag a node: it stays there. Wheel: zoom.",
        "À la main", "Clic droit : ce qui peut être fait, en mots. Glisser avec le bouton droit d'un nœud à un autre : une relation. Glisser un nœud : il reste là. Molette : zoom."),
    ("prompt", "The prompt", "Talk to it: node mass = 2, relate newton: force = mass * acceleration, ask force, why force. Tab completes, ↑ ↓ the history. ls, cd, load data.csv work as in a shell.",
        "Le prompteur", "Lui parler : node mass = 2, relate newton: force = mass * acceleration, ask force, why force. Tab complète, ↑ ↓ l'historique. ls, cd, load data.csv marchent comme dans un shell."),
    ("side", "The inspector", "A node chosen: its value ± sigma, where it comes from, its relations, its time and why. The other tabs: workshops (networks made apart, let in through an airlock), sources and quarantine, the journal, the sound.",
        "L'inspecteur", "Un nœud choisi : sa valeur ± sigma, d'où il vient, ses relations, son temps et pourquoi. Les autres onglets : ateliers (des réseaux faits à part, qu'un sas laisse entrer), sources et quarantaine, journal, son."),
    ("tabs", "Apps", "+ opens an app or makes one. An app is one file: its network, its surface, its APIs. Code, network, plots, app: four views of one thing, all live.",
        "Les apps", "+ ouvre une app ou en crée une. Une app est un fichier : son réseau, sa surface, ses API. Code, réseau, tracés, app : quatre vues d'une même chose, toutes vivantes."),
    ("tabs", "Edit while it runs", "Ctrl+E: edit mode (widgets moved, added, their lines changed) while the app keeps sounding. Ctrl+R: x-ray (what each widget is bound to). F5: launched alone, full screen.",
        "Éditer pendant qu'elle tourne", "Ctrl+E : mode édition (widgets déplacés, ajoutés, leurs lignes changées) pendant que l'app continue de sonner. Ctrl+R : radiographie (à quoi chaque widget est lié). F5 : lancée seule, plein écran."),
    ("top", "Calm", "Harmony: how well the relations hold. Nothing flashes, everything glides. Panic (Ctrl+.) takes all sound down. The desk comes back as it was left.",
        "Calme", "Harmonie : à quel point les relations tiennent. Rien ne clignote, tout glisse. Panique (Ctrl+.) baisse tout le son. Le bureau revient comme il a été laissé."),
    ("top", "Sleep: a save of its own", "The moon at the top does not turn anything off: the machine writes all it is (its network, its memories, its workshops, its trash) beside its memory file, and wakes from it next time. It is a save, the way a night keeps a day. Ctrl+Q sleeps too, then closes.",
        "Endormir : une sauvegarde particulière", "La lune en haut n'éteint rien : la machine écrit tout ce qu'elle est (son réseau, ses souvenirs, ses ateliers, sa corbeille) à côté de son fichier de mémoire, et s'en réveille la fois suivante. C'est une sauvegarde, comme une nuit garde une journée. Ctrl+Q endort aussi, puis ferme."),
    ("central", "Her consciousness", "The empty triangle at the heart of the network only observes: it changes nothing and keeps nothing. Only what passes now goes through it. The pointer on it, or `witness`: what passes through her, in the third person.",
        "Sa conscience", "Le triangle vide au cœur du réseau ne fait qu'observer : il ne change rien et ne garde rien. Seul ce qui passe maintenant le traverse. Le pointeur dessus, ou `conscience` : ce qui passe en elle, à la troisième personne."),
    ("settings", "The colours", "Settings: a palette, or each colour by hand, the text size, the language. Apps inherit the machine's look; each can have its own.",
        "Les couleurs", "Réglages : une palette, ou chaque couleur à la main, la taille du texte, la langue. Les apps héritent du style de la machine ; chacune peut avoir le sien."),
    ("help", "Again", "F1: the keys and every word. `tour`: this visit again, at any moment.",
        "Encore", "F1 : les touches et tous les mots. `tour` : cette visite à nouveau, à tout moment."),
];

/// Draws the visit over everything.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(k) = app.touring else { return };
    let Some(step) = STEPS.get(k) else {
        app.touring = None;
        return;
    };
    let ink = Ink::of(ctx);
    let fr = app.settings.lang == Lang::Fr;
    let (title, words) = if fr { (step.3, step.4) } else { (step.1, step.2) };
    let screen = ctx.content_rect();
    let target = if step.0 == "all" { None } else { app.areas.get(step.0).copied() };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("tour-dim")));
    let dim = Color32::from_black_alpha(150);
    match target {
        Some(t) => {
            let t = t.expand(4.0);
            painter.rect_filled(Rect::from_min_max(screen.min, pos2(screen.max.x, t.min.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(screen.min.x, t.max.y), screen.max), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(screen.min.x, t.min.y), pos2(t.min.x, t.max.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(t.max.x, t.min.y), pos2(screen.max.x, t.max.y)), 0.0, dim);
            painter.rect_stroke(
                t,
                crate::style::corner(),
                egui::Stroke::new(2.0_f32, ink.accent),
                egui::StrokeKind::Outside,
            );
        }
        None => {
            painter.rect_filled(screen, 0.0, dim);
        }
    }
    // the card: beside what is lit, where there is room
    let size = vec2(380.0, 190.0);
    let at = match target {
        Some(t) if t.width() < screen.width() * 0.6 && t.left() > size.x + 20.0 => {
            pos2(t.left() - size.x - 16.0, t.top().max(20.0))
        }
        Some(t) if t.width() < screen.width() * 0.6 => {
            pos2((t.right() + 16.0).min(screen.right() - size.x - 10.0), t.top().max(20.0))
        }
        Some(t) if t.center().y > screen.center().y => {
            pos2(screen.center().x - size.x / 2.0, (t.top() - size.y - 16.0).max(10.0))
        }
        Some(t) => pos2(screen.center().x - size.x / 2.0, (t.bottom() + 16.0).min(screen.bottom() - size.y - 10.0)),
        None => screen.center() - size / 2.0,
    };
    let mut next = None;
    egui::Area::new(egui::Id::new("tour-card")).order(egui::Order::Tooltip).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::popup(ui.style())
            .fill(ink.panel)
            .stroke(egui::Stroke::new(1.5_f32, ink.accent))
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(size.x - 28.0);
                ui.label(
                    RichText::new(format!("{}  ·  {}/{}", title, k + 1, STEPS.len()))
                        .strong()
                        .size(crate::style::size(crate::style::Size::Subtitle))
                        .color(ink.accent),
                );
                ui.add_space(4.0);
                ui.label(words);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if k > 0 && ui.button(if fr { "← Retour" } else { "← Back" }).clicked() {
                        next = Some(Some(k - 1));
                    }
                    let last = k + 1 == STEPS.len();
                    let label = if last {
                        if fr {
                            "Terminer"
                        } else {
                            "Done"
                        }
                    } else if fr {
                        "Suivant →"
                    } else {
                        "Next →"
                    };
                    if crate::style::chosen(ui, true, label).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::ArrowRight))
                    {
                        next = Some(if last { None } else { Some(k + 1) });
                    }
                    if ui.button(if fr { "Fermer (Échap)" } else { "Close (Esc)" }).clicked() {
                        next = Some(None);
                    }
                });
            });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && k > 0 {
        next = Some(Some(k - 1));
    }
    if let Some(n) = next {
        app.touring = n;
        if n.is_none() {
            app.settings.toured = true;
        }
    }
}
