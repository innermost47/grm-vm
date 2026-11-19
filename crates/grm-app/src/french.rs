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
            // the workshop: what is wrong said in French, with what to do
            r(
                r"^'(.*)' is not a word of the language: did you mean '(.*)'\? \(a line begins with (.*)\)$",
                "« $1 » n'est pas un mot du langage : vouliez-vous dire « $2 » ? (une ligne commence par $3)",
            ),
            r(r"^'(.*)' is not a word of the language: a line begins with (.*)$", "« $1 » n'est pas un mot du langage : une ligne commence par $2"),
            r(r"^'(.*)' ends with (\S+): a term is missing after it$", "« $1 » finit par $2 : il manque un terme après"),
            r(r"^'(.*)' begins with (\S+): a term is missing before it$", "« $1 » commence par $2 : il manque un terme avant"),
            r(r"^'(.*)': (\S+) then (\S+), a term is missing between them$", "« $1 » : $2 puis $3, il manque un terme entre les deux"),
            r(
                r"^not a number: '(.*)' \((\S+) is not a constant; the constants are (.*)\)$",
                "pas un nombre : « $1 » ($2 n'est pas une constante ; les constantes sont $3)",
            ),
            r(r"^not a number: '(.*)'$", "pas un nombre : « $1 »"),
            r(r"^(\S+) is a constant \((.*)\): a node is called otherwise$", "$1 est une constante ($2) : un nœud s'appelle autrement"),
            r(
                r"^(\S+) is used by: (.*)\. To remove it with them: (.*)$",
                "$1 est utilisé par : $2. Pour le supprimer avec elles : « supprimer avec ce qui l'utilise » ($3)",
            ),
            r(r"^(\S+) is not a node of the workshop (\S+)$", "$1 n'est pas un nœud de l'atelier $2"),
            r(r"^(\S+) is not in the workshop (\S+) \(nothing written makes it or uses it\)$", "$1 n'est pas dans l'atelier $2 (aucune ligne ne le fait ni ne l'utilise)"),
            r(r"^removed from (\S+): (.*)$", "supprimé de $1 : $2"),
            r(r"^taken back from (\S+): (.*)$", "repris de $1 : $2"),
            r(r"^taken: (.*)$", "pris : $1"),
            r(r"^their values say so now \((.*)\): one moment only, to be checked$", "leurs valeurs le disent maintenant ($1) : un seul instant, à vérifier"),
            r(r"^a law known in (\S+) \((\S+) is missing here, at its value there\): (.*)$", "une loi connue dans $1 ($2 manque ici, à sa valeur de là-bas) : $3"),
            r(r"^a law known in (\S+): (.*)$", "une loi connue dans $1 : $2"),
            r(r"^found in its (\d+) rows: (.*)$", "trouvée dans ses $1 lignes : $2"),
            r(r"^refused: (.*)$", "refusé : $1"),
            r(r"^(\d+) new, (\d+) proposal\(s\) waiting in (\S+): take or refuse each$", "$1 nouvelle(s), $2 proposition(s) en attente dans $3 : à prendre ou à refuser"),
            r(
                r"^nothing to propose in (\S+) for now: .*$",
                "rien à proposer dans $1 pour l'instant : plus de nœuds avec des valeurs, ou un tableau, lui donneraient de quoi chercher",
            ),
            r(r"^(\d+) proposal\(s\) wait in the workshop (\S+): take or refuse each$", "$1 proposition(s) attendent dans l'atelier $2 : à prendre ou à refuser"),
            r(r"^no workshop (\S+) \(workshop new (\S+) makes one\)$", "pas d'atelier $1 (workshop new $2 en fait un)"),
            r(r"^the proposal cannot be written: (.*)$", "la proposition ne peut pas être écrite : $1"),
            r(r"^line (\d+) \((.*)\): (.*) — nothing was changed$", "ligne $1 ($2) : $3 — rien n'a changé"),
            // why: who says what of a node, and how much it weighs
            r(r"^why (\S+) = (.*)$", "pourquoi $1 = $2"),
            r(
                r"^(\S+) put in the trash \(restore (\S+) gives it back\)$",
                "$1 mis à la corbeille (restore $2 le rend)",
            ),
            r(r"^(\S+) given back from the trash$", "$1 rendu depuis la corbeille"),
            r(r"^the trash is empty$", "la corbeille est vide"),
            r(
                r"^! (\S+) came with the machine: it does not go to the trash$",
                "! $1 est venu avec la machine : il ne va pas à la corbeille",
            ),
            r(r"^(\d+) years learned in (\S+)$", "$1 années apprises dans $2"),
            r(
                r"^(\s+)([0-9.]+) %  its measurement \(or what it was given\)$",
                "$1$2 %  sa mesure (ou ce qu'on lui a donné)",
            ),
            r(r"^(\s+)([0-9.]+) %  relation (\S+): (.*)$", "$1$2 %  la relation $3 : $4"),
            r(r"^(\s+)([0-9.]+) %  the limit (\S+): (.*)$", "$1$2 %  la limite $3 : $4"),
            r(
                r"^(\s+)([0-9.]+) %  a belief held with others \((.*)\)$",
                "$1$2 %  une croyance tenue avec d'autres ($3)",
            ),
            r(r"^(\s+)says (.*)$", "${1}dit $2"),
            r(r"^  and (\d+) that say almost nothing of it$", "  et $1 qui n'en disent presque rien"),
            r(r"^(\S+): nothing tells it anything yet$", "$1 : rien ne lui en dit encore quoi que ce soit"),
            r(r"^(\S+) is fixed: (.*)$", "$1 est fixé : $2"),
            r(r"^\? not understood: (.*) \(help\)$", "? pas compris : $1 (help)"),
            r(
                r"^(\d+) links / room (\d+) \(([0-9]+) % taken\): a link costs (\S+) bit a tick$",
                "$1 liens / place $2 ($3 % prise) : un lien coûte $4 bit par tick",
            ),
            r(
                r"^  (\S+): brings (\S+) bits, rent (\S+) \((\d+) links\): pays its rent$",
                "  $1 : apporte $2 bits, loyer $3 ($4 liens) : paie son loyer",
            ),
            r(
                r"^  (\S+): brings (\S+) bits, rent (\S+) \((\d+) links\): does not pay its rent$",
                "  $1 : apporte $2 bits, loyer $3 ($4 liens) : ne paie pas son loyer",
            ),
            r(r"^made smaller to be seen: (\d+)x(\d+) -> (\d+)x(\d+)$", "réduite pour être vue : $1x$2 -> $3x$4"),
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
            r(r"^the sound unplugged and plugged in again$", "le son débranché et rebranché"),
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
    // an answer of several lines: each said again on its own
    if text.contains('\n') {
        return text.split('\n').map(fr).collect::<Vec<_>>().join("\n");
    }
    // what to do, said after what went wrong: `(for example: LINE)`
    if let Some((what, example)) = text.strip_suffix(')').and_then(|t| t.rsplit_once(" (for example: ")) {
        return format!("{} (par exemple : {example})", fr(what));
    }
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
        assert_eq!(
            fr("'m *' ends with *: a term is missing after it (for example: relate newton: force = mass * acceleration)"),
            "« m * » finit par * : il manque un terme après (par exemple : relate newton: force = mass * acceleration)"
        );
        assert!(fr("'nod' is not a word of the language: did you mean 'node'? (a line begins with node, relate)")
            .contains("vouliez-vous dire « node »"));
        let why = "why distance = 18 (± 0.01 dex)\n  50.0 %  relation run: distance = speed * time\n          says 18\n  and 1 that say almost nothing of it";
        assert_eq!(
            fr(why),
            "pourquoi distance = 18 (± 0.01 dex)\n  50.0 %  la relation run : distance = speed * time\n          dit 18\n  et 1 qui n'en disent presque rien"
        );
    }
}
