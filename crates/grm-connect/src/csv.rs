// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! A table written by any program: UTF-8 with or without its mark, or Windows' own encoding; separated by `,`,
//! `;`, tabs or `|`; decimal points or commas (`3,14`), thousands set apart (`1 234,5`, `1,234.5`), percentages
//! (`12%`). Its header is the first line of distinct names (a title above it, a line of formats under it, are not
//! taken for it); a header that is not a name of the language becomes one (`Température (°C)` → `Temperature_C`).
//! An empty cell is not a measurement.

/// A table read.
#[derive(Debug, Clone, PartialEq)]
pub struct Csv {
    /// The columns' names as the language needs them, and as they were written.
    pub names: Vec<String>,
    pub written: Vec<String>,
    /// Each row: a value per column (None: empty, or not a number).
    pub rows: Vec<Vec<Option<f64>>>,
    /// The cells as they were (to edit the table).
    pub cells: Vec<Vec<String>>,
    pub separator: char,
}

impl Csv {
    /// Whether a column holds numbers (most of its filled cells).
    pub fn numeric(&self, k: usize) -> bool {
        let filled = self.cells.iter().filter(|r| r.get(k).is_some_and(|c| !c.trim().is_empty())).count();
        let numbers = self.rows.iter().filter(|r| r.get(k).copied().flatten().is_some()).count();
        filled > 0 && numbers * 10 >= filled * 8
    }
}

/// Text from bytes: UTF-8 (its mark taken away), or Windows-1252.
pub fn text_of(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80 => '€',
                0x92 => '\'',
                0x96 | 0x97 => '-',
                b if b < 0x80 => b as char,
                b => char::from_u32(b as u32).unwrap_or('?'),
            })
            .collect(),
    }
}

fn split(line: &str, sep: char) -> Vec<String> {
    let mut out = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if quoted && chars.peek() == Some(&'"') {
                cell.push('"');
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if c == sep && !quoted {
            out.push(cell.trim().to_string());
            cell.clear();
        } else {
            cell.push(c);
        }
    }
    out.push(cell.trim().to_string());
    out
}

/// A cell as a number, if it is one.
pub fn number(cell: &str, sep: char) -> Option<f64> {
    let mut s: String = cell.trim().chars().filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\'')).collect();
    if s.is_empty() {
        return None;
    }
    let percent = s.ends_with('%');
    if percent {
        s.pop();
    }
    let commas = s.matches(',').count();
    let dots = s.matches('.').count();
    if commas > 0 && dots > 0 {
        // the last one is the decimal point
        if s.rfind(',') > s.rfind('.') {
            s = s.replace('.', "").replace(',', ".");
        } else {
            s = s.replace(',', "");
        }
    } else if commas == 1 && sep != ',' {
        s = s.replace(',', ".");
    } else if commas > 1 {
        s = s.replace(',', "");
    }
    let v: f64 = s.parse().ok()?;
    Some(if percent { v / 100.0 } else { v })
}

/// A header made a name of the language.
pub fn name_of(written: &str) -> String {
    let plain: String = written
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'À' | 'Á' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'Î' | 'Ï' => 'I',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'Ô' | 'Ö' => 'O',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            _ => '_',
        })
        .collect();
    let mut out = String::new();
    for c in plain.chars() {
        if c == '_' && (out.is_empty() || out.ends_with('_')) {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        "column".into()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

pub fn read(text: &str) -> Csv {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    // the separator: the one that cuts the first lines most evenly
    let sep = [',', ';', '\t', '|']
        .into_iter()
        .max_by_key(|s| {
            // how many of the first lines it cuts into the same number of cells (more than one)
            let counts: Vec<usize> = lines.iter().take(6).map(|l| l.matches(*s).count()).filter(|c| *c > 0).collect();
            let mode = counts.iter().copied().max_by_key(|c| counts.iter().filter(|x| *x == c).count()).unwrap_or(0);
            counts.iter().filter(|c| **c == mode).count() * 100 + mode
        })
        .unwrap_or(',');
    // the header: the first line of distinct names that are not all numbers
    let mut header_at = 0;
    for (k, l) in lines.iter().enumerate().take(5) {
        let cells = split(l, sep);
        let distinct = cells.iter().collect::<std::collections::HashSet<_>>().len() == cells.len();
        let names = cells.iter().filter(|c| !c.is_empty() && number(c, sep).is_none()).count();
        if cells.len() > 1 && distinct && names * 2 >= cells.len() {
            header_at = k;
            break;
        }
    }
    let written: Vec<String> = lines.get(header_at).map(|l| split(l, sep)).unwrap_or_default();
    let mut names: Vec<String> = vec![];
    for w in &written {
        let mut n = name_of(w);
        let mut k = 2;
        while names.contains(&n) {
            n = format!("{}_{k}", name_of(w));
            k += 1;
        }
        names.push(n);
    }
    let mut cells: Vec<Vec<String>> = vec![];
    for l in lines.iter().skip(header_at + 1) {
        let row = split(l, sep);
        // a line of formats under the header (no number in it) is not a row
        if cells.is_empty() && row.iter().all(|c| number(c, sep).is_none()) && row.iter().any(|c| !c.is_empty()) {
            continue;
        }
        cells.push(row);
    }
    let rows =
        cells.iter().map(|r| (0..names.len()).map(|k| r.get(k).and_then(|c| number(c, sep))).collect()).collect();
    Csv { names, written, rows, cells, separator: sep }
}

/// The table written back (as it was separated).
pub fn write(written: &[String], cells: &[Vec<String>], sep: char) -> String {
    let quote = |c: &str| {
        if c.contains(sep) || c.contains('"') {
            format!("\"{}\"", c.replace('"', "\"\""))
        } else {
            c.to_string()
        }
    };
    let mut out = written.iter().map(|c| quote(c)).collect::<Vec<_>>().join(&sep.to_string());
    out.push('\n');
    for r in cells {
        out.push_str(&r.iter().map(|c| quote(c)).collect::<Vec<_>>().join(&sep.to_string()));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_from_anywhere_are_read() {
        let t = read("Mesures de la station\nTempérature (°C);CO2 (ppm);2020\n12,5;410;1 234,5\n13;411,2;12%\n;412;\n");
        assert_eq!(t.separator, ';');
        assert_eq!(t.names, vec!["Temperature_C", "CO2_ppm", "_2020"]);
        assert_eq!(t.rows[0], vec![Some(12.5), Some(410.0), Some(1234.5)]);
        assert_eq!(t.rows[1][2], Some(0.12));
        assert_eq!(t.rows[2][0], None);
        assert!(t.numeric(1));
        let t = read("\u{feff}x,y\n1,234.5,2\n");
        assert_eq!(t.names, vec!["x", "y"]);
        let t = read("a\tb\n\"1\"\t2\n");
        assert_eq!(t.separator, '\t');
        assert_eq!(t.rows[0], vec![Some(1.0), Some(2.0)]);
        assert_eq!(write(&t.written, &t.cells, '\t'), "a\tb\n1\t2\n");
    }
}
