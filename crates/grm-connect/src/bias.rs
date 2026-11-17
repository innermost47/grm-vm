// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What a table leans towards, said before anything is learned from it: groups much less represented than others
//! (representation bias), values missing more for some groups than for others (measurement bias), averages that
//! differ between groups (a gap, said with its numbers: it may have its reasons, the human judges), and a table
//! too small to conclude. It says what it sees, never what to think of it.

use crate::csv::Csv;

/// The groups of a column: (group, its rows).
type Groups = Vec<(String, Vec<usize>)>;

/// What it noticed.
#[derive(Debug, Clone, PartialEq)]
pub enum Lean {
    /// A column whose groups are unevenly represented: (column, sensitive, [(group, rows)]).
    Represented { column: String, sensitive: bool, groups: Vec<(String, usize)> },
    /// A column with many empty cells: (column, share empty).
    Missing { column: String, share: f64 },
    /// Values missing more for one group than another: (column, by, [(group, share empty)]).
    MissingFor { column: String, by: String, groups: Vec<(String, f64)> },
    /// An average that differs between groups of a sensitive column: (column, by, [(group, mean, rows)]).
    Gap { column: String, by: String, groups: Vec<(String, f64, usize)> },
    /// Too few rows to conclude much.
    Few { rows: usize },
}

/// Names of columns that often hold what people must not be judged on (in French and English).
const SENSITIVE: &[&str] = &[
    "sex",
    "sexe",
    "gender",
    "genre",
    "age",
    "âge",
    "race",
    "ethnic",
    "ethnie",
    "origin",
    "origine",
    "nationality",
    "nationalité",
    "nationalite",
    "religion",
    "handicap",
    "disability",
    "pregnan",
    "enceinte",
    "marital",
    "matrimonial",
    "orientation",
    "country",
    "pays",
    "birth",
    "naissance",
    "postcode",
    "code_postal",
    "zip",
];

pub fn sensitive(name: &str) -> bool {
    let n = name.to_lowercase();
    SENSITIVE.iter().any(|s| n.contains(s))
}

fn empty(c: &str) -> bool {
    let c = c.trim();
    c.is_empty() || c.eq_ignore_ascii_case("na") || c.eq_ignore_ascii_case("n/a") || c.eq_ignore_ascii_case("null")
}

/// The groups of a column that reads as categories (2 to 12 of them), with their rows.
fn groups(t: &Csv, k: usize) -> Option<Groups> {
    if t.numeric(k) && !sensitive(&t.names[k]) {
        return None;
    }
    let mut g: Groups = vec![];
    for (i, row) in t.cells.iter().enumerate() {
        let Some(c) = row.get(k).filter(|c| !empty(c)) else { continue };
        let c = c.trim().to_string();
        match g.iter_mut().find(|(n, _)| *n == c) {
            Some((_, rows)) => rows.push(i),
            None => g.push((c, vec![i])),
        }
        if g.len() > 12 {
            return None;
        }
    }
    (g.len() >= 2).then_some(g)
}

/// What the table leans towards.
pub fn leans(t: &Csv) -> Vec<Lean> {
    let n = t.cells.len();
    let mut out = vec![];
    if n > 0 && n < 30 {
        out.push(Lean::Few { rows: n });
    }
    if n == 0 {
        return out;
    }
    let all_groups: Vec<Option<Groups>> = (0..t.names.len()).map(|k| groups(t, k)).collect();
    for (k, name) in t.names.iter().enumerate() {
        let empties = t.cells.iter().filter(|r| r.get(k).is_none_or(|c| empty(c))).count();
        let share = empties as f64 / n as f64;
        if share > 0.10 {
            out.push(Lean::Missing { column: name.clone(), share });
        }
        if let Some(g) = &all_groups[k] {
            let max = g.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
            let min = g.iter().map(|(_, r)| r.len()).min().unwrap_or(0);
            if min == 0 || max as f64 >= 1.5 * min as f64 {
                let mut groups: Vec<(String, usize)> = g.iter().map(|(c, r)| (c.clone(), r.len())).collect();
                groups.sort_by_key(|g| std::cmp::Reverse(g.1));
                out.push(Lean::Represented { column: name.clone(), sensitive: sensitive(name), groups });
            }
        }
    }
    // for each sensitive column: what is missing more, and what differs on average, from one group to another
    for (s, by) in t.names.iter().enumerate() {
        let (true, Some(g)) = (sensitive(by), &all_groups[s]) else { continue };
        for (k, column) in t.names.iter().enumerate() {
            if k == s {
                continue;
            }
            let missing: Vec<(String, f64)> = g
                .iter()
                .map(|(c, rows)| {
                    let e = rows.iter().filter(|&&i| t.cells[i].get(k).is_none_or(|c| empty(c))).count();
                    (c.clone(), e as f64 / rows.len().max(1) as f64)
                })
                .collect();
            let hi = missing.iter().map(|m| m.1).fold(0.0, f64::max);
            let lo = missing.iter().map(|m| m.1).fold(1.0, f64::min);
            if hi - lo > 0.15 {
                out.push(Lean::MissingFor { column: column.clone(), by: by.clone(), groups: missing });
            }
            if !t.numeric(k) {
                continue;
            }
            let means: Vec<(String, f64, usize)> = g
                .iter()
                .filter_map(|(c, rows)| {
                    let v: Vec<f64> = rows.iter().filter_map(|&i| t.rows[i].get(k).copied().flatten()).collect();
                    (v.len() >= 3).then(|| (c.clone(), v.iter().sum::<f64>() / v.len() as f64, v.len()))
                })
                .collect();
            if means.len() < 2 {
                continue;
            }
            let all: Vec<f64> = t.rows.iter().filter_map(|r| r.get(k).copied().flatten()).collect();
            let mean = all.iter().sum::<f64>() / all.len() as f64;
            let sd = (all.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / all.len() as f64).sqrt();
            let hi = means.iter().map(|m| m.1).fold(f64::MIN, f64::max);
            let lo = means.iter().map(|m| m.1).fold(f64::MAX, f64::min);
            if sd > 0.0 && (hi - lo) / sd > 0.5 {
                out.push(Lean::Gap { column: column.clone(), by: by.clone(), groups: means });
            }
        }
    }
    out
}

fn pc(v: f64) -> String {
    format!("{:.0} %", v * 100.0)
}

/// What it says of a lean, in words that judge no one.
pub fn words(l: &Lean, french: bool) -> String {
    match l {
        Lean::Few { rows } => {
            if french {
                format!("{rows} ligne(s) seulement : ce qui en sera appris restera peu sûr.")
            } else {
                format!("only {rows} row(s): what is learned from them will stay unsure.")
            }
        }
        Lean::Missing { column, share } => {
            if french {
                format!("{column} : {} des cases sont vides.", pc(*share))
            } else {
                format!("{column}: {} of the cells are empty.", pc(*share))
            }
        }
        Lean::Represented { column, sensitive, groups } => {
            let n: usize = groups.iter().map(|g| g.1).sum();
            let list = groups
                .iter()
                .map(|(g, c)| format!("{g} {}", pc(*c as f64 / n.max(1) as f64)))
                .collect::<Vec<_>>()
                .join(", ");
            match (french, sensitive) {
                (true, true) => format!("{column} (une donnée sensible) n'est pas équilibré : {list}. Ce qui sera appris vaudra surtout pour les groupes les plus présents."),
                (true, false) => format!("{column} n'est pas équilibré : {list}."),
                (false, true) => format!("{column} (a sensitive column) is not balanced: {list}. What is learned will hold mostly for the groups most present."),
                (false, false) => format!("{column} is not balanced: {list}."),
            }
        }
        Lean::MissingFor { column, by, groups } => {
            let list = groups.iter().map(|(g, s)| format!("{g} {}", pc(*s))).collect::<Vec<_>>().join(", ");
            if french {
                format!("{column} manque plus souvent selon {by} : {list} de cases vides.")
            } else {
                format!("{column} is missing more often depending on {by}: {list} empty.")
            }
        }
        Lean::Gap { column, by, groups } => {
            let list = groups
                .iter()
                .map(|(g, m, c)| format!("{g} {} ({c})", grm_engine::lang::g((*m * 100.0).round() / 100.0)))
                .collect::<Vec<_>>()
                .join(", ");
            if french {
                format!("{column} diffère en moyenne selon {by} : {list}. Un écart n'est pas une cause : il demande d'où il vient.")
            } else {
                format!("{column} differs on average depending on {by}: {list}. A gap is not a cause: it asks where it comes from.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_of_cvs_says_what_it_leans_towards() {
        let mut text = String::from("sexe,annees,embauche\n");
        for i in 0..40 {
            text.push_str(&format!("H,{},{}\n", 5 + i % 7, i % 2));
        }
        for i in 0..10 {
            let years = if i % 3 == 0 { String::new() } else { format!("{}", 2 + i % 4) };
            text.push_str(&format!("F,{years},{}\n", i % 5 / 4));
        }
        let t = crate::csv::read(&text);
        let l = leans(&t);
        assert!(
            l.iter().any(|x| matches!(x, Lean::Represented { column, sensitive: true, .. } if column == "sexe")),
            "{l:?}"
        );
        assert!(l.iter().any(|x| matches!(x, Lean::MissingFor { column, .. } if column == "annees")), "{l:?}");
        assert!(l.iter().any(|x| matches!(x, Lean::Gap { column, .. } if column == "annees")), "{l:?}");
        assert!(words(&l[0], true).len() > 10);
    }
}
