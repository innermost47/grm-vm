// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! Formulas as formulas: what a relation says (or a master equation) read as an expression, drawn as mathematics
//! (fractions, powers, functions) and given as LaTeX to copy. What cannot be read stays the text it was.

use eframe::egui::{self, pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::Ink;

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(String),
    Var(String),
    Neg(Box<Expr>),
    /// Terms, each with its sign (true: minus).
    Sum(Vec<(bool, Expr)>),
    Mul(Vec<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Paren(Box<Expr>),
    /// Sides joined by `=`, `<=`, `>=`...
    Rel(Vec<Expr>, Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Name(String),
    Op(String),
}

fn tokens(text: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = text.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            // an exponent: 3.9e-06
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                let mut j = i + 1;
                if j < c.len() && (c[j] == '-' || c[j] == '+') {
                    j += 1;
                }
                if j < c.len() && c[j].is_ascii_digit() {
                    i = j;
                    while i < c.len() && c[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            out.push(Tok::Num(c[start..i].iter().collect()));
        } else if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '~') {
                i += 1;
            }
            out.push(Tok::Name(c[start..i].iter().collect()));
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if ["<=", ">=", "**"].contains(&two.as_str()) {
                out.push(Tok::Op(if two == "**" { "^".into() } else { two }));
                i += 2;
            } else if "+-*/^()=<>,·×".contains(ch) {
                out.push(Tok::Op(match ch {
                    '·' | '×' => "*".into(),
                    c => c.to_string(),
                }));
                i += 1;
            } else {
                return None;
            }
        }
    }
    Some(out)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek_op(&self) -> Option<&str> {
        match self.t.get(self.i) {
            Some(Tok::Op(o)) => Some(o.as_str()),
            _ => None,
        }
    }

    fn eat(&mut self, op: &str) -> bool {
        if self.peek_op() == Some(op) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn relation(&mut self) -> Option<Expr> {
        let mut sides = vec![self.sum()?];
        let mut ops = vec![];
        while let Some(op) = self.peek_op().filter(|o| ["=", "<=", ">=", "<", ">"].contains(o)).map(String::from) {
            self.i += 1;
            ops.push(op);
            sides.push(self.sum()?);
        }
        Some(if ops.is_empty() { sides.pop()? } else { Expr::Rel(sides, ops) })
    }

    fn sum(&mut self) -> Option<Expr> {
        let mut terms = vec![];
        let mut minus = self.eat("-");
        if !minus {
            self.eat("+");
        }
        loop {
            terms.push((minus, self.product()?));
            if self.eat("+") {
                minus = false;
            } else if self.eat("-") {
                minus = true;
            } else {
                break;
            }
        }
        Some(if terms.len() == 1 && !terms[0].0 { terms.pop()?.1 } else { Expr::Sum(terms) })
    }

    fn product(&mut self) -> Option<Expr> {
        let mut e = self.power()?;
        let mut factors = vec![];
        loop {
            if self.eat("*") {
                factors.push(e);
                e = self.power()?;
            } else if self.eat("/") {
                let d = self.power()?;
                let n = if factors.is_empty() {
                    e
                } else {
                    factors.push(e);
                    Expr::Mul(std::mem::take(&mut factors))
                };
                e = Expr::Div(Box::new(n), Box::new(d));
            } else {
                break;
            }
        }
        if factors.is_empty() {
            Some(e)
        } else {
            factors.push(e);
            Some(Expr::Mul(factors))
        }
    }

    fn power(&mut self) -> Option<Expr> {
        let base = self.atom()?;
        if self.eat("^") {
            // the exponent: a signed atom (x^-1, x^(1/2))
            let minus = self.eat("-");
            let e = self.power()?;
            let e = if minus { Expr::Neg(Box::new(e)) } else { e };
            return Some(Expr::Pow(Box::new(base), Box::new(e)));
        }
        Some(base)
    }

    fn atom(&mut self) -> Option<Expr> {
        match self.t.get(self.i).cloned()? {
            Tok::Num(n) => {
                self.i += 1;
                Some(Expr::Num(n))
            }
            Tok::Name(n) => {
                self.i += 1;
                if self.eat("(") {
                    let mut args = vec![self.sum()?];
                    while self.eat(",") {
                        args.push(self.sum()?);
                    }
                    if !self.eat(")") {
                        return None;
                    }
                    Some(Expr::Call(n, args))
                } else {
                    Some(Expr::Var(n))
                }
            }
            Tok::Op(o) if o == "(" => {
                self.i += 1;
                let e = self.sum()?;
                if !self.eat(")") {
                    return None;
                }
                Some(Expr::Paren(Box::new(e)))
            }
            Tok::Op(o) if o == "-" => {
                self.i += 1;
                Some(Expr::Neg(Box::new(self.power()?)))
            }
            _ => None,
        }
    }
}

/// Reads a formula; None when it is not one (it is then shown as written).
pub fn parse(text: &str) -> Option<Expr> {
    let mut p = Parser { t: tokens(text)?, i: 0 };
    let e = p.relation()?;
    (p.i == p.t.len()).then_some(e)
}

/// A number as it is shown: at most six figures (the LaTeX keeps it whole).
fn short(n: &str) -> String {
    match n.parse::<f64>() {
        Ok(v) if n.len() > 8 => grm_engine::lang::g(v),
        _ => n.to_string(),
    }
}

fn latex_num(n: &str) -> String {
    match n.split_once(['e', 'E']) {
        Some((m, e)) => {
            let e: i64 = e.parse().unwrap_or(0);
            format!("{m} \\times 10^{{{e}}}")
        }
        None => n.to_string(),
    }
}

fn latex_name(n: &str) -> String {
    if n.chars().count() == 1 {
        n.to_string()
    } else {
        format!("\\mathrm{{{}}}", n.replace('_', "\\_"))
    }
}

/// The formula in LaTeX.
pub fn latex(e: &Expr) -> String {
    match e {
        Expr::Num(n) => latex_num(n),
        Expr::Var(v) => latex_name(v),
        Expr::Neg(x) => format!("-{}", latex(x)),
        Expr::Sum(terms) => {
            let mut s = String::new();
            for (k, (minus, t)) in terms.iter().enumerate() {
                if *minus {
                    s.push_str(if k == 0 { "-" } else { " - " });
                } else if k > 0 {
                    s.push_str(" + ");
                }
                s.push_str(&latex(t));
            }
            s
        }
        Expr::Mul(f) => f.iter().map(latex).collect::<Vec<_>>().join(" \\cdot "),
        Expr::Div(a, b) => format!("\\frac{{{}}}{{{}}}", latex(strip(a)), latex(strip(b))),
        Expr::Pow(a, b) => {
            let base = match a.as_ref() {
                Expr::Var(_) | Expr::Num(_) | Expr::Paren(_) | Expr::Call(..) => latex(a),
                _ => format!("\\left({}\\right)", latex(a)),
            };
            format!("{base}^{{{}}}", latex(strip(b)))
        }
        Expr::Call(f, args) => {
            let inner = args.iter().map(latex).collect::<Vec<_>>().join(", ");
            match f.as_str() {
                "ln" | "log" | "exp" | "sin" | "cos" | "tan" => {
                    if f == "exp" {
                        format!("e^{{{inner}}}")
                    } else {
                        format!("\\{f}\\left({inner}\\right)")
                    }
                }
                "sqrt" => format!("\\sqrt{{{inner}}}"),
                _ => format!("{}\\left({inner}\\right)", latex_name(f)),
            }
        }
        Expr::Paren(x) => format!("\\left({}\\right)", latex(x)),
        Expr::Rel(sides, ops) => {
            let mut s = latex(&sides[0]);
            for (op, side) in ops.iter().zip(&sides[1..]) {
                let op = match op.as_str() {
                    "<=" => "\\leq",
                    ">=" => "\\geq",
                    o => o,
                };
                s.push_str(&format!(" {op} {}", latex(side)));
            }
            s
        }
    }
}

/// A parenthesis that a fraction's bar or an exponent's height already says.
fn strip(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(x) => x,
        e => e,
    }
}

// ---------------------------------------------------------------------- drawing

#[derive(Clone)]
enum Draw {
    Text(Pos2, String, f32, Color32, bool),
    Line(Pos2, Pos2, Color32),
}

/// A laid out piece: its size, where its baseline is (from its top), what to draw (relative to its top left).
#[derive(Clone)]
struct Piece {
    size: Vec2,
    base: f32,
    draws: Vec<Draw>,
}

impl Piece {
    fn moved(mut self, by: Vec2) -> Vec<Draw> {
        for d in &mut self.draws {
            match d {
                Draw::Text(p, ..) => *p += by,
                Draw::Line(a, b, _) => {
                    *a += by;
                    *b += by;
                }
            }
        }
        self.draws
    }
}

struct Layout<'a> {
    ui: &'a egui::Ui,
    ink: &'a Ink,
}

impl Layout<'_> {
    fn text(&self, s: &str, size: f32, colour: Color32, italic: bool) -> Piece {
        let font = FontId::proportional(size);
        let g = self.ui.fonts_mut(|f| f.layout_no_wrap(s.to_string(), font, colour));
        let size_v = g.size();
        Piece {
            size: size_v,
            base: size_v.y * 0.78,
            draws: vec![Draw::Text(Pos2::ZERO, s.to_string(), size, colour, italic)],
        }
    }

    fn row(&self, pieces: Vec<Piece>, gap: f32) -> Piece {
        let base = pieces.iter().map(|p| p.base).fold(0.0, f32::max);
        let below = pieces.iter().map(|p| p.size.y - p.base).fold(0.0, f32::max);
        let mut x = 0.0;
        let mut draws = vec![];
        let n = pieces.len();
        for (k, p) in pieces.into_iter().enumerate() {
            let w = p.size.x;
            let dy = base - p.base;
            draws.extend(p.moved(vec2(x, dy)));
            x += w + if k + 1 < n { gap } else { 0.0 };
        }
        Piece { size: vec2(x, base + below), base, draws }
    }

    fn op(&self, s: &str, size: f32) -> Piece {
        self.text(s, size, self.ink.muted, false)
    }

    fn piece(&self, e: &Expr, size: f32) -> Piece {
        let ink = self.ink;
        match e {
            Expr::Num(n) => self.text(&short(n), size, ink.source(4), false),
            Expr::Var(v) => self.text(v, size, ink.text, true),
            Expr::Neg(x) => self.row(vec![self.op("−", size), self.piece(x, size)], 1.0),
            Expr::Sum(terms) => {
                let mut parts = vec![];
                for (k, (minus, t)) in terms.iter().enumerate() {
                    if *minus {
                        parts.push(self.op("−", size));
                    } else if k > 0 {
                        parts.push(self.op("+", size));
                    }
                    parts.push(self.piece(t, size));
                }
                self.row(parts, size * 0.3)
            }
            Expr::Mul(f) => {
                let mut parts = vec![];
                for (k, x) in f.iter().enumerate() {
                    if k > 0 {
                        parts.push(self.op("·", size));
                    }
                    parts.push(self.piece(x, size));
                }
                self.row(parts, size * 0.2)
            }
            Expr::Div(a, b) => {
                let small = (size * 0.9).max(10.0);
                let (n, d) = (self.piece(strip(a), small), self.piece(strip(b), small));
                let w = n.size.x.max(d.size.x) + size * 0.4;
                let gap = size * 0.15;
                let bar_y = n.size.y + gap;
                let mut draws = vec![];
                let (nw, dw, dh) = (n.size.x, d.size.x, d.size.y);
                draws.extend(n.moved(vec2((w - nw) / 2.0, 0.0)));
                draws.push(Draw::Line(pos2(0.0, bar_y), pos2(w, bar_y), ink.muted));
                draws.extend(d.moved(vec2((w - dw) / 2.0, bar_y + gap)));
                Piece { size: vec2(w, bar_y + gap + dh), base: bar_y + size * 0.3, draws }
            }
            Expr::Pow(a, b) => {
                let base = match a.as_ref() {
                    Expr::Var(_) | Expr::Num(_) | Expr::Paren(_) | Expr::Call(..) => self.piece(a, size),
                    x => self.paren(self.piece(x, size), size),
                };
                let exp = self.piece(strip(b), (size * 0.68).max(9.0));
                let rise = size * 0.42;
                let top = (exp.size.y - rise).max(0.0);
                let mut draws = base.clone().moved(vec2(0.0, top));
                let (bw, bh, bb) = (base.size.x, base.size.y, base.base);
                draws.extend(exp.clone().moved(vec2(bw + 1.0, 0.0)));
                Piece { size: vec2(bw + 1.0 + exp.size.x, top + bh), base: top + bb, draws }
            }
            Expr::Call(f, args) => {
                let mut inner = vec![];
                for (k, a) in args.iter().enumerate() {
                    if k > 0 {
                        inner.push(self.op(",", size));
                    }
                    inner.push(self.piece(a, size));
                }
                let inside = self.row(inner, size * 0.2);
                if f == "exp" {
                    let e =
                        Expr::Pow(Box::new(Expr::Var("e".into())), Box::new(Expr::Paren(Box::new(args[0].clone()))));
                    return self.piece(&e, size);
                }
                if f == "sqrt" {
                    return self.row(vec![self.op("√", size * 1.1), self.paren(inside, size)], 0.0);
                }
                self.row(vec![self.text(f, size, ink.accent, false), self.paren(inside, size)], 1.0)
            }
            Expr::Paren(x) => self.paren(self.piece(x, size), size),
            Expr::Rel(sides, ops) => {
                let mut parts = vec![self.piece(&sides[0], size)];
                for (op, side) in ops.iter().zip(&sides[1..]) {
                    let op = match op.as_str() {
                        "<=" => "≤",
                        ">=" => "≥",
                        o => o,
                    };
                    parts.push(self.op(op, size));
                    parts.push(self.piece(side, size));
                }
                self.row(parts, size * 0.45)
            }
        }
    }

    /// Parentheses as tall as what they hold.
    fn paren(&self, inside: Piece, size: f32) -> Piece {
        let tall = (inside.size.y / (size * 1.25)).max(1.0);
        let s = size * tall;
        let (l, r) = (self.text("(", s, self.ink.muted, false), self.text(")", s, self.ink.muted, false));
        let mut p = self.row(vec![l, inside.clone(), r], 1.0);
        // the parentheses centred on what they hold
        let _ = inside;
        p.base = p.base.max(0.0);
        p
    }
}

/// Draws a formula (or its text when it cannot be read as one). Right click: copy it as LaTeX. Returns its
/// response.
pub fn show(ui: &mut egui::Ui, text: &str, size: f32, lang: crate::text::Lang) -> egui::Response {
    let ink = Ink::of(ui.ctx());
    let Some(e) = parse(text) else {
        return ui.label(egui::RichText::new(text).monospace().size(size * 0.9));
    };
    let piece = Layout { ui, ink: &ink }.piece(&e, size);
    let (rect, resp) = ui.allocate_exact_size(piece.size + vec2(4.0, 4.0), Sense::click());
    paint(ui, rect, piece);
    let tex = latex(&e);
    let resp = resp.on_hover_text(crate::text::t(lang, "latex_hint"));
    resp.context_menu(|ui| {
        if crate::menu::item(ui, "copy", crate::text::t(lang, "copy_latex"), &tex) {
            ui.ctx().copy_text(tex.clone());
            ui.close();
        }
        if crate::menu::item(ui, "type", crate::text::t(lang, "copy_text"), text) {
            ui.ctx().copy_text(text.to_string());
            ui.close();
        }
    });
    if resp.double_clicked() {
        ui.ctx().copy_text(tex);
    }
    resp
}

fn paint(ui: &egui::Ui, rect: Rect, piece: Piece) {
    let origin = rect.min + vec2(2.0, 2.0);
    let painter = ui.painter();
    for d in piece.draws {
        match d {
            Draw::Text(p, s, size, colour, italic) => {
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    &s,
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(size),
                        color: colour,
                        italics: italic,
                        ..Default::default()
                    },
                );
                let g = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(origin + p.to_vec2(), g, colour);
            }
            Draw::Line(a, b, colour) => {
                painter.line_segment([origin + a.to_vec2(), origin + b.to_vec2()], Stroke::new(1.2_f32, colour));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_are_read_and_written_in_latex() {
        let e = parse("found_1_ln = 0.460504915068 * DM - 11.5124825682").unwrap();
        assert_eq!(latex(&e), "\\mathrm{found\\_1\\_ln} = 0.460504915068 \\cdot \\mathrm{DM} - 11.5124825682");
        let e = parse("found_1_ln = ln(distance_Mpc)").unwrap();
        assert_eq!(latex(&e), "\\mathrm{found\\_1\\_ln} = \\ln\\left(\\mathrm{distance\\_Mpc}\\right)");
        let e = parse("force · mass^-1 = 2").unwrap();
        assert_eq!(latex(&e), "\\mathrm{force} \\cdot \\mathrm{mass}^{-1} = 2");
        assert_eq!(latex(&parse("y = a / (b + c)").unwrap()), "y = \\frac{a}{b + c}");
        assert_eq!(latex(&parse("x >= 3.9e-06").unwrap()), "x \\geq 3.9 \\times 10^{-6}");
        assert_eq!(latex(&parse("h = K * t^2").unwrap()), "h = K \\cdot t^{2}");
        assert_eq!(latex(&parse("v = sqrt(2 * g * h)").unwrap()), "v = \\sqrt{2 \\cdot g \\cdot h}");
        assert!(parse("x in 1 .. 2").is_none());
    }
}
