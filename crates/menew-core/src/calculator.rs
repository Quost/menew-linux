//! Simple, safe inline math evaluator for the Menew palette

pub fn evaluate_expression(input: &str) -> Option<f64> {
    let clean = input.trim();
    if clean.is_empty() {
        return None;
    }

    // Support "X% of Y" pattern
    if let Some(res) = try_eval_percent_of(clean) {
        return Some(res);
    }

    // Check if contains valid math characters
    let valid_chars = clean
        .chars()
        .all(|c| c.is_ascii_digit() || "+-*/%^.() ".contains(c));
    if !valid_chars {
        return None;
    }

    // Must contain at least one operator
    let has_operator = clean.chars().any(|c| "+-*/^".contains(c));
    if !has_operator {
        return None;
    }

    let tokens = tokenize(clean)?;
    let mut parser = Parser::new(tokens);
    parser.parse_expression()
}

fn try_eval_percent_of(input: &str) -> Option<f64> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() == 3 && parts[1].eq_ignore_ascii_case("of") {
        let percent_str = parts[0].trim_end_matches('%');
        let percent: f64 = percent_str.parse().ok()?;
        let total: f64 = parts[2].parse().ok()?;
        return Some((percent / 100.0) * total);
    }
    None
}

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => {
                chars.next();
            }
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Token::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Token::Star);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Slash);
                chars.next();
            }
            '^' => {
                tokens.push(Token::Caret);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num_str = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() || nc == '.' {
                        num_str.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let val: f64 = num_str.parse().ok()?;
                tokens.push(Token::Number(val));
            }
            _ => return None,
        }
    }

    Some(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Option<f64> {
        let mut left = self.parse_term()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.next_token();
                    let right = self.parse_term()?;
                    left += right;
                }
                Token::Minus => {
                    self.next_token();
                    let right = self.parse_term()?;
                    left -= right;
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut left = self.parse_factor()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => {
                    self.next_token();
                    let right = self.parse_factor()?;
                    left *= right;
                }
                Token::Slash => {
                    self.next_token();
                    let right = self.parse_factor()?;
                    if right == 0.0 {
                        return None; // Division by zero
                    }
                    left /= right;
                }
                Token::Caret => {
                    self.next_token();
                    let right = self.parse_factor()?;
                    left = left.powf(right);
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_factor(&mut self) -> Option<f64> {
        match self.next_token()? {
            Token::Number(n) => Some(n),
            Token::Minus => {
                let val = self.parse_factor()?;
                Some(-val)
            }
            Token::Plus => self.parse_factor(),
            Token::LParen => {
                let val = self.parse_expression()?;
                if self.next_token()? != Token::RParen {
                    return None;
                }
                Some(val)
            }
            _ => None,
        }
    }
}

pub fn format_result(val: f64) -> String {
    if val.fract() == 0.0 && val.abs() < 1e15 {
        format!("{:.0}", val)
    } else {
        format!("{:.4}", val).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_expressions() {
        assert_eq!(evaluate_expression("12 * 7.5"), Some(90.0));
        assert_eq!(evaluate_expression("100 / 4 + 15"), Some(40.0));
        assert_eq!(evaluate_expression("2 ^ 8"), Some(256.0));
        assert_eq!(evaluate_expression("(2 + 3) * 4"), Some(20.0));
        assert_eq!(evaluate_expression("20% of 340"), Some(68.0));
    }

    #[test]
    fn test_format() {
        assert_eq!(format_result(90.0), "90");
        assert_eq!(format_result(1.23456), "1.2346");
    }
}
