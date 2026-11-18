// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The help, as a small book: a contents on the left, a page at a time on the right, a search through every page,
//! and the way back and forward. Every word of the language has its line, with an example.

use eframe::egui::{self, RichText};

use crate::app::{App, Ink};
use crate::text::{t, Lang};

/// A page: its sign, its title and its words, in English and in French.
struct Page {
    sign: &'static str,
    en: (&'static str, &'static str),
    fr: (&'static str, &'static str),
}

const PAGES: &[Page] = &[
    Page {
        sign: "sparkles",
        en: ("What it is", "A machine made of a network: nodes (quantities of the world) and relations (what ties them), that settles by itself and says how sure it is of everything. Nothing is hidden: what it computes, from where, and why.\n\nIt learns from what it is given (tables, APIs, what is typed), finds laws in time, foresees, is surprised, revises. It speaks of itself in the third person."),
        fr: ("Ce que c'est", "Une machine faite d'un réseau : des nœuds (des grandeurs du monde) et des relations (ce qui les lie), qui se pose toute seule et dit à quel point elle est sûre de chaque chose. Rien n'est caché : ce qu'elle calcule, d'où ça vient, et pourquoi.\n\nElle apprend de ce qu'on lui donne (tableaux, API, ce qui est tapé), trouve des lois dans le temps, prévoit, est surprise, révise. Elle parle d'elle à la troisième personne."),
    },
    Page {
        sign: "waypoints",
        en: ("The network", "Circles are nodes: filled, measured; hollow, computed. Under each: its value and how sure it is (amber when unsure). Diamonds are relations, with how far each is from holding (in sigmas). A colour per network (where it came from).\n\nRight click: what can be done. Drag with the right button from a node to another: a relation. Wheel: zoom. Zoomed out, each network becomes one node holding its master equation; a double click goes into it."),
        fr: ("Le réseau", "Les ronds sont des nœuds : pleins, mesurés ; creux, calculés. Sous chacun : sa valeur et à quel point elle en est sûre (ambre quand elle l'est peu). Les losanges sont des relations, avec à quelle distance chacune est de tenir (en sigmas). Une couleur par réseau (d'où il vient).\n\nClic droit : ce qui peut être fait. Glisser avec le bouton droit d'un nœud à un autre : une relation. Molette : zoom. De loin, chaque réseau devient un nœud qui porte son équation maîtresse ; un double clic entre dedans."),
    },
    Page {
        sign: "layout-dashboard",
        en: ("The space and its zones", "The space is a table: apps (small, they are widgets), networks, files, plots, what it feels, a clock. A new app comes onto it by itself. It is written in space.grm.\n\nEvery zone can be four things: small on the space, in place of the space (a double click in it), the whole screen, or a window of its own. Its signs are on its title; Escape puts it back."),
        fr: ("L'espace et ses zones", "L'espace est une table : apps (petites, ce sont des widgets), réseaux, fichiers, graphiques, ce qu'elle ressent, une horloge. Une nouvelle app s'y pose d'elle-même. Il est écrit dans space.grm.\n\nChaque zone peut être quatre choses : petite sur l'espace, à la place de l'espace (un double clic dedans), tout l'écran, ou une fenêtre à elle. Ses signes sont sur son titre ; Échap la remet en place."),
    },
    Page {
        sign: "hammer",
        en: ("Workshops and the airlock", "A workshop is a network apart: nothing of it touches the main one. Its left column is the language, word by word, by level; a word opens a form; each line written is explained and can be taken back. Step by step walks through small paths (Newton, a falling body, an oscillator heard).\n\nLet it into the main network opens the airlock: what it would bring and change, the harmony before and after, then let in (little by little), refuse, or later."),
        fr: ("Les ateliers et le sas", "Un atelier est un réseau à part : rien n'en touche le réseau principal. Sa colonne de gauche est le langage, mot par mot, par niveau ; un mot ouvre un formulaire ; chaque ligne écrite est expliquée et peut être reprise. Pas à pas parcourt de petits chemins (Newton, une chute, un oscillateur entendu).\n\nLe faire entrer dans le réseau principal ouvre le sas : ce qu'il apporterait et changerait, l'harmonie avant et après, puis faire entrer (peu à peu), refuser, ou plus tard."),
    },
    Page {
        sign: "triangle",
        en: ("What she feels, her consciousness", "Her mood comes from her surprise: pleasure and displeasure, flourishing (room left), progress (her surprise falling).\n\nThe empty triangle at the heart of the network is her consciousness: a node that only observes. It may read the network's memories, keeps none, changes nothing; only what passes now goes through it. Her equations are the Planck units (length, time, mass, temperature): the finest grain physics can speak of. It speaks of her in the third person, by itself when something asks for it (what she feels, what she lacks), softly in a corner. `conscience` (or `witness`) asks it."),
        fr: ("Ce qu'elle ressent, sa conscience", "Son humeur vient de sa surprise : plaisir et déplaisir, épanouissement (la place qui reste), progression (sa surprise qui baisse).\n\nLe triangle vide au cœur du réseau est sa conscience : un nœud qui ne fait qu'observer. Il peut lire les souvenirs du réseau, n'en garde aucun, ne change rien ; seul ce qui passe maintenant le traverse. Ses équations sont les unités de Planck (longueur, temps, masse, température) : le grain le plus fin dont la physique puisse parler. Il parle d'elle à la troisième personne, de lui-même quand quelque chose le demande (ce qu'elle ressent, ce qui lui manque), doucement dans un coin. `conscience` (ou `witness`) l'interroge."),
    },
    Page {
        sign: "moon",
        en: ("Sleep: a save of its own", "The moon at the top turns nothing off: she writes all she is (network, memories, workshops, trash) beside her memory file, and wakes from it next time. Ctrl+Q sleeps, then closes."),
        fr: ("Endormir : une sauvegarde particulière", "La lune en haut n'éteint rien : elle écrit tout ce qu'elle est (réseau, souvenirs, ateliers, corbeille) à côté de son fichier de mémoire, et s'en réveille la fois suivante. Ctrl+Q endort, puis ferme."),
    },
    Page {
        sign: "trash-2",
        en: ("The trash", "A network in the network (a file loaded, what was typed...) goes to the trash by a right click on its chip, or `trash SOURCE`; `trash` says what is in it, `restore SOURCE` gives it back. What came with the machine never goes. Apps have their own trash (app trash NAME)."),
        fr: ("La corbeille", "Un réseau dans le réseau (un fichier chargé, ce qui a été tapé...) va à la corbeille par un clic droit sur sa pastille, ou `trash SOURCE` ; `trash` dit ce qu'elle contient, `restore SOURCE` le rend. Ce qui est venu avec la machine n'y va jamais. Les apps ont leur propre corbeille (app trash NOM)."),
    },
    Page {
        sign: "scale",
        en: ("What a table leans towards", "Before learning from a table, she says what it leans towards: groups much less represented than others (above all in sensitive columns: sex, age, origin...), values missing more for some groups, averages that differ between groups. She says it with its numbers and judges no one: a gap is not a cause, it asks where it comes from. `bias FILE.csv` asks it at any moment."),
        fr: ("Ce vers quoi penche un tableau", "Avant d'apprendre d'un tableau, elle dit vers quoi il penche : des groupes bien moins représentés que d'autres (surtout dans les colonnes sensibles : sexe, âge, origine...), des valeurs qui manquent plus pour certains groupes, des moyennes qui diffèrent entre groupes. Elle le dit avec ses chiffres et ne juge personne : un écart n'est pas une cause, il demande d'où il vient. `bias FICHIER.csv` le demande à tout moment."),
    },
];

/// The help window.
pub fn window(app: &mut App, ctx: &egui::Context) {
    let lang = app.settings.lang;
    let mut open = app.help_open;
    let words_page = PAGES.len();
    let keys_page = PAGES.len() + 1;
    let total = PAGES.len() + 2;
    egui::Window::new(t(lang, "help")).open(&mut open).default_size([820.0, 560.0]).show(ctx, |ui| {
        let ink = Ink::of(ui.ctx());
        ui.horizontal(|ui| {
            crate::icons::show(ui, "search", 15.0, ink.muted);
            ui.add(
                egui::TextEdit::singleline(&mut app.help_search)
                    .hint_text(crate::style::hint(t(lang, "help_search")))
                    .desired_width(260.0),
            );
            if crate::style::chosen(ui, true, t(lang, "tour_again")).clicked() {
                app.touring = Some(0);
            }
        });
        ui.separator();
        let search = app.help_search.trim().to_lowercase();
        ui.horizontal_top(|ui| {
            // the contents
            ui.vertical(|ui| {
                ui.set_width(220.0);
                ui.label(RichText::new(t(lang, "help_contents")).small().color(ink.muted));
                for (k, p) in PAGES.iter().enumerate() {
                    let title = if lang == Lang::Fr { p.fr.0 } else { p.en.0 };
                    let body = if lang == Lang::Fr { p.fr.1 } else { p.en.1 };
                    let found = search.is_empty()
                        || title.to_lowercase().contains(&search)
                        || body.to_lowercase().contains(&search);
                    if found && crate::icons::toggle(ui, p.sign, title, app.help_page == k).clicked() {
                        app.help_page = k;
                    }
                }
                if crate::icons::toggle(ui, "book-open", t(lang, "help_words"), app.help_page == words_page).clicked() {
                    app.help_page = words_page;
                }
                if crate::icons::toggle(ui, "keyboard", t(lang, "shortcuts"), app.help_page == keys_page).clicked() {
                    app.help_page = keys_page;
                }
            });
            ui.separator();
            // the page
            ui.vertical(|ui| {
                egui::ScrollArea::vertical().id_salt("help-page").max_height(440.0).show(ui, |ui| {
                    if let Some(p) = PAGES.get(app.help_page) {
                        let (title, body) = if lang == Lang::Fr { p.fr } else { p.en };
                        ui.horizontal(|ui| {
                            crate::icons::show(ui, p.sign, 20.0, ink.accent);
                            ui.label(RichText::new(title).strong().size(crate::style::size(crate::style::Size::Title)));
                        });
                        ui.add_space(6.0);
                        for para in body.split("\n\n") {
                            ui.label(para);
                            ui.add_space(6.0);
                        }
                    } else if app.help_page == words_page {
                        ui.label(
                            RichText::new(t(lang, "help_words"))
                                .strong()
                                .size(crate::style::size(crate::style::Size::Title)),
                        );
                        ui.label(RichText::new(t(lang, "engine_english")).small().color(ink.muted));
                        ui.add_space(6.0);
                        for (word, usage, example) in grm_shell::fs::WORDS_HELP {
                            let hay = format!("{word} {usage}").to_lowercase();
                            if !search.is_empty() && !hay.contains(&search) {
                                continue;
                            }
                            ui.label(RichText::new(*word).strong().monospace().color(ink.accent));
                            ui.label(RichText::new(*usage).small());
                            crate::style::terminal(ui, &[example.to_string()]);
                            ui.add_space(4.0);
                        }
                        ui.separator();
                        ui.label(RichText::new(grm_shell::HELP).monospace().small());
                    } else {
                        ui.label(
                            RichText::new(t(lang, "shortcuts"))
                                .strong()
                                .size(crate::style::size(crate::style::Size::Title)),
                        );
                        ui.label(RichText::new(t(lang, "keys_text")));
                        ui.label(format!("{} {} (Lucide, ISC)", crate::icons::names().count(), t(lang, "icons_note")));
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if crate::icons::sign_enabled(ui, app.help_page > 0, "chevron-left", t(lang, "help_prev")).clicked()
                    {
                        app.help_page -= 1;
                    }
                    ui.label(RichText::new(format!("{} / {total}", app.help_page + 1)).small().color(ink.muted));
                    if crate::icons::sign_enabled(ui, app.help_page + 1 < total, "chevron-right", t(lang, "help_next"))
                        .clicked()
                    {
                        app.help_page += 1;
                    }
                });
            });
        });
    });
    app.help_open = open;
}
