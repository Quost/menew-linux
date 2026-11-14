//! Math for the palette (port of `Calculator.swift`): "12*7,5", "20% de 340", "2(3+1)", "sqrt 16".
//! Own evaluator: it never panics on invalid input.

use crate::conversions;
use crate::format::format_number;
use crate::i18n::Language;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcResult {
    /// What is shown and copied.
    pub value: String,
    /// The calculation as understood.
    pub detail: String,
}

/// `None` when the text is not a calculation (a lone number is not one either).
pub fn evaluate(raw: &str, lang: Language) -> Option<CalcResult> {
    let q = raw.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    if let Some(r) = percent_of(&q, lang).or_else(|| conversions::convert(&q, lang)) {
        return Some(r);
    }
    let has_op = q.chars().any(|c| "+-*/^x×÷%()√!".contains(c));
    let has_fn = FUNCTIONS.iter().any(|(n, _)| q.contains(n)) || q.contains("pi") || q.contains('π');
    if !has_op && !has_fn {
        return None;
    }
    let mut p = Parser::new(&q);
    let v = p.parse()?;
    if !v.is_finite() {
        return None;
    }
    Some(CalcResult { value: format_number(v, 10, lang), detail: raw.trim().to_string() })
}

/// "20% de 340"
fn percent_of(q: &str, lang: Language) -> Option<CalcResult> {
    let parts: Vec<&str> = q.split(' ').filter(|s| !s.is_empty()).collect();
    if parts.len() < 3 || !parts[0].ends_with('%') || !["de", "of", "von", "del", "的"].contains(&parts[1]) {
        return None;
    }
    let p = number(&parts[0][..parts[0].len() - 1])?;
    let mut parser = Parser::new(&parts[2..].join(" "));
    let base = parser.parse()?;
    Some(CalcResult { value: format_number(p / 100.0 * base, 10, lang), detail: q.to_string() })
}

/// "1,5" and "1.5" are both 1.5.
pub fn number(s: &str) -> Option<f64> {
    let t = s.replace(',', ".");
    if t.is_empty() || t.starts_with("inf") || t.starts_with("nan") || t.starts_with("+inf") || t.starts_with("-inf") {
        return None;
    }
    // Swift's Double() rejects what Rust's parse accepts only in exotic cases ("1e5" is fine in both)
    t.parse::<f64>().ok()
}

type MathFn = fn(f64) -> f64;

const FUNCTIONS: &[(&str, MathFn)] = &[
    ("sqrt", f64::sqrt),
    ("√", f64::sqrt),
    ("raiz", f64::sqrt),
    ("sin", f64::sin),
    ("sen", f64::sin),
    ("cos", f64::cos),
    ("tan", f64::tan),
    ("log", f64::log10),
    ("ln", f64::ln),
    ("abs", f64::abs),
    ("round", f64::round),
];

fn function(name: &str) -> Option<MathFn> {
    FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}

/// expr = term (+|- term)*  ·  term = power (*|/ power)*  ·  power = unary (^ power)?
struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Self { chars: s.chars().filter(|c| *c != ' ').collect(), i: 0 }
    }

    fn parse(&mut self) -> Option<f64> {
        let v = self.expr()?;
        if self.i == self.chars.len() {
            Some(v)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(c) = self.peek() {
            if c != '+' && c != '-' {
                break;
            }
            self.i += 1;
            let start = self.i;
            let r = self.term()?;
            // "200 + 10%" = 220, like calculators do
            let is_percent = self.i > start && self.chars[self.i - 1] == '%';
            let amount = if is_percent { v * r } else { r };
            v = if c == '+' { v + amount } else { v - amount };
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(c) = self.peek() {
            if c == '*' || c == '×' || c == 'x' {
                self.i += 1;
                v *= self.power()?;
            } else if c == '/' || c == '÷' {
                self.i += 1;
                v /= self.power()?;
            } else if c == '(' || c.is_alphabetic() || c == '√' {
                // "2(3+1)", "2pi"
                v *= self.power()?;
            } else {
                break;
            }
        }
        Some(v)
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.unary()?;
        if self.peek() == Some('^') {
            self.i += 1;
            let exp = self.power()?;
            return Some(base.powf(exp));
        }
        Some(base)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some('-') => {
                self.i += 1;
                self.unary().map(|v| -v)
            }
            Some('+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut v = self.primary()?;
        while let Some(c) = self.peek() {
            if c == '%' {
                self.i += 1;
                v /= 100.0;
            } else if c == '!' {
                self.i += 1;
                if !(0.0..=170.0).contains(&v) || v != v.round() {
                    return None;
                }
                let n = (v as u64).max(1);
                v = (1..=n).fold(1.0, |a, k| a * k as f64);
            } else {
                break;
            }
        }
        Some(v)
    }

    fn primary(&mut self) -> Option<f64> {
        let c = self.peek()?;
        if c == '(' {
            self.i += 1;
            let v = self.expr()?;
            if self.peek() != Some(')') {
                return None;
            }
            self.i += 1;
            return Some(v);
        }
        if c.is_numeric() || c == '.' || c == ',' {
            let start = self.i;
            while let Some(d) = self.peek() {
                if d.is_numeric() || d == '.' || d == ',' {
                    self.i += 1;
                } else {
                    break;
                }
            }
            let s: String = self.chars[start..self.i].iter().collect();
            return number(&s);
        }
        if c == '√' {
            self.i += 1;
            return self.primary().map(f64::sqrt);
        }
        if c.is_alphabetic() || c == 'π' {
            let start = self.i;
            while let Some(d) = self.peek() {
                if d.is_alphabetic() || d == 'π' {
                    self.i += 1;
                } else {
                    break;
                }
            }
            let name: String = self.chars[start..self.i].iter().collect();
            if name == "pi" || name == "π" {
                return Some(std::f64::consts::PI);
            }
            if name == "e" {
                return Some(std::f64::consts::E);
            }
            let f = function(&name)?;
            let arg = self.primary()?;
            return Some(f(arg));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(q: &str) -> Option<String> {
        evaluate(q, Language::En).map(|r| r.value)
    }

    #[test]
    fn plain_expressions() {
        assert_eq!(ev("12*7.5").as_deref(), Some("90"));
        assert_eq!(ev("100 / 4 + 15").as_deref(), Some("40"));
        assert_eq!(ev("2 ^ 8").as_deref(), Some("256"));
        assert_eq!(ev("(2 + 3) * 4").as_deref(), Some("20"));
    }

    #[test]
    fn a_lone_number_or_word_is_not_a_calculation() {
        assert_eq!(ev("42"), None);
        assert_eq!(ev("firefox"), None);
        assert_eq!(ev(""), None);
    }

    #[test]
    fn invalid_input_never_panics() {
        assert_eq!(ev("1/0"), None);
        assert_eq!(ev("(1+"), None);
        assert_eq!(ev("2*"), None);
        assert_eq!(ev("171!"), None);
        assert_eq!(ev("sqrt"), None);
    }
}
