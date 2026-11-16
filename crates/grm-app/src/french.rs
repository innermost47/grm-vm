// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What the machine answers, said in French in the window (the engine speaks English: its sentences are
//! recognised here and said again). A sentence it does not know stays as it is, in English.

use std::sync::OnceLock;

use regex::Regex;

type Rule = (Regex, &'static str);

fn rules() -> &'static Vec<Rule> {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let r = |p: &str, to: &'static str| (Regex::new(p).expect("a rule"), to);
        vec![
            r(r"^node (\S+) = (.*)$", "nœud $1 = $2"),
            r(r"^relation (\S+): (.*)$", "relation $1 : $2"),
            r(r"^observed (\S+) = (.*) dex$", "observé $1 = $2 dex"),
            r(r"^observed (\S+) = (.*)$", "observé $1 = $2"),
            r(r"^a variable: (.*)$", "une variable : $1"),
            r(r"^a weight it learns: (.*)$", "un poids qu'elle apprend : $1"),
            r(r"^an input: (.*)$", "une entrée : $1"),
            r(r"^an output: (.*)$", "une sortie : $1"),
            r(r"^at each tick (\S+) <- (.*)$", "à chaque tick $1 <- $2"),
            r(r"^forgot (\S+)$", "oublié $1"),
            r(
                r"^harmony ([0-9.]+), (\d+) rounds, (\d+) messages, (\d+) ticks, frame (\d+) \((.*)\), settled$",
                "harmonie $1, $2 tours, $3 messages, $4 ticks, image $5 ($6), posé",
            ),
            r(
                r"^harmony ([0-9.]+), (\d+) rounds, (\d+) messages, (\d+) ticks, frame (\d+) \((.*)\), computing$",
                "harmonie $1, $2 tours, $3 messages, $4 ticks, image $5 ($6), calcule",
            ),
            r(r"^sandbox (\S+) opened$", "bac à sable $1 ouvert"),
            r(r"^(\d+) rounds$", "$1 tours"),
            r(r"^merge of (\S+): (.*)$", "fusion de $1 : $2"),
            r(r"^nothing taken away$", "rien n'est retiré"),
            r(r"^removed (.*)$", "retiré : $1"),
            r(r"^\? not understood: (\S+) \(help\)$", "? pas compris : $1 (help)"),
            r(r"^! (\S+) is not measured$", "! $1 n'est pas mesuré"),
            r(r"^! no node (\S+)$", "! pas de nœud $1"),
            r(r"^! remove (.*)\? \(y/n\)(.*)$", "! retirer $1 ? (o/n)$2"),
            r(
                r"^(\S+) lives in time: it remembers, learns how it moves \((\d+) lag\(s\)\), and can be foreseen$",
                "$1 vit dans le temps : il se souvient, apprend comment il bouge ($2 retard(s)), et peut être prévu",
            ),
            r(
                r"^woke from (\S+): (\d+) node\(s\), (\d+) relation\(s\), (\d+) in time$",
                "réveillé de $1 : $2 nœud(s), $3 relation(s), $4 dans le temps",
            ),
            r(r"^asleep in (\S+) \(it goes on\)$", "endormi dans $1 (il continue)"),
            r(r"^asleep in (\S+)$", "endormi dans $1"),
            r(r"^a new world$", "un monde neuf"),
            r(
                r"^(\S+) plays, (\d+) times a second \(it rises from silence\)$",
                "$1 joue, $2 fois par seconde (il monte depuis le silence)",
            ),
            r(r"^(\S+) fades out$", "$1 s'éteint doucement"),
            r(r"^its voice rises, slowly$", "sa voix monte, lentement"),
            r(r"^its voice goes quiet$", "sa voix se tait"),
            r(r"^its voice: (\S+)$", "sa voix : $1"),
            r(r"^everything down, in a breath$", "tout baisse, en un souffle"),
            r(r"^volume ([0-9.]+)$", "volume $1"),
            r(r"^(\S+) opens \((.*)\)$", "$1 s'ouvre ($2)"),
            r(
                r"^(\S+) closes \(its file stays, what it was living through is kept\)$",
                "$1 se ferme (son fichier reste, ce qu'il vivait est gardé)",
            ),
            r(r"^(\S+) runs already$", "$1 tourne déjà"),
            r(r"^(\S+), line (\d+): (.*)$", "$1, ligne $2 : $3"),
            r(
                r"^(\S+) runs its own copy of what another running app holds: (.*)$",
                "$1 tourne avec sa propre copie de ce qu'une autre app lancée tient : $2",
            ),
            r(r"^api (\S+) starts$", "l'api $1 démarre"),
            r(r"^(\d+) api\(s\) of the desk$", "$1 api du bureau"),
            r(r"^line (\d+) taken from (\S+)$", "ligne $1 retirée de $2"),
            r(
                r"^! (\S+) is written by the app (\S+), line (\d+): «(.*)»\. Take this line from it\? \(y/n\)$",
                "! $1 est écrit par l'app $2, ligne $3 : «$4». Retirer cette ligne ? (o/n)",
            ),
            r(r"^the desk as it was left: (.*)$", "le bureau tel qu'il a été laissé : $1"),
            r(
                r"^(\S+) no longer plays \(it is no longer in the network\)$",
                "$1 ne joue plus (il n'est plus dans le réseau)",
            ),
            r(
                r"^no sound device \((.*)\): render still writes files$",
                "pas de sortie son ($1) : render écrit encore des fichiers",
            ),
            r(r"^(\S+) saw (\S+) in (\d+) rounds \((.*)\)$", "$1 a vu $2 en $3 tours ($4)"),
            r(r"^cleaned: (.*)$", "nettoyée : $1"),
            r(r"^surprise: (.*)$", "surprise : $1"),
            r(r"^what changed: (.*)$", "ce qui a changé : $1"),
            r(r"^most surprised at (.*)$", "le plus surpris en $1"),
            r(
                r"^its glance \((\S+)\) is heard by the network as (\S+)$",
                "son coup d'œil ($1) est entendu par le réseau comme $2",
            ),
            r(r"^(\S+): cannot reach the server \((\S+)\)$", "$1 : impossible de joindre le serveur ($2)"),
            r(r"^no new law holds on the rows it has seen$", "aucune nouvelle loi ne tient sur les lignes vues"),
            r(r"^(\S+) <- (.*)$", "$1 <- $2"),
            r(r"^threads: (\S+)$", "threads : $1"),
        ]
    })
}

pub fn fr(text: &str) -> String {
    for (re, to) in rules() {
        if re.is_match(text) {
            return re.replace(text, *to).to_string();
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sentences_are_said_in_french_the_others_stay() {
        assert_eq!(fr("node force = 1"), "nœud force = 1");
        assert_eq!(fr("observed mass = 2 ± 0.01 dex"), "observé mass = 2 ± 0.01 dex");
        assert_eq!(fr("synth, line 4: no node x"), "synth, ligne 4 : no node x");
        assert_eq!(fr("something new"), "something new");
    }
}
