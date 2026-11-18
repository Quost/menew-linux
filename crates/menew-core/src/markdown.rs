//! Markdown of the notes (port of `Markdown.swift`): the blocks (headings, lists, checklists, quotes, code,
//! rules, paragraphs) are parsed by the same rules as the Mac, line by line, and the inline marks
//! (bold, italic, `code`, strikethrough, links and `#tags`) become Pango markup for a `GtkLabel`.
//! Pure: no GTK. `docs/notes-spec.md` has the vectors.

/// One block of a note. `line` is the 0-based line of the note text, so that a click on a checklist can
/// change that line (`toggle_task`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading(usize, String),
    Task { done: bool, text: String, line: usize },
    Bullet { text: String, indent: usize },
    Numbered(String, String),
    Quote(String),
    Code(String),
    Rule,
    Paragraph(String),
    Space,
}

/// `trimmingCharacters(in: .whitespaces)`: spaces and tabs (and the other horizontal blanks), not newlines.
fn is_blank(c: char) -> bool {
    c == '\t' || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

pub fn trim_blanks(s: &str) -> &str {
    s.trim_matches(is_blank)
}

/// `^(#{1,3})\s+(.+)$`
fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    let after = rest.trim_start_matches(char::is_whitespace);
    if after.len() == rest.len() || after.is_empty() {
        return None;
    }
    Some((hashes, after))
}

/// `^[-*+]\s+` and what follows it.
fn after_bullet_marker(line: &str) -> Option<&str> {
    let mut chars = line.chars();
    let first = chars.next()?;
    if !matches!(first, '-' | '*' | '+') {
        return None;
    }
    let rest = &line[1..];
    let after = rest.trim_start_matches(char::is_whitespace);
    (after.len() != rest.len()).then_some(after)
}

/// `^\[([ xX])\]\s*(.*)$`: (done, text).
fn task_box(s: &str) -> Option<(bool, &str)> {
    let mut it = s.chars();
    if it.next()? != '[' {
        return None;
    }
    let mark = it.next()?;
    if !matches!(mark, ' ' | 'x' | 'X') || it.next()? != ']' {
        return None;
    }
    let rest = &s[3..];
    Some((mark != ' ', rest.trim_start_matches(char::is_whitespace)))
}

/// `^(\d+)[.)]\s+(.+)$`
fn numbered(line: &str) -> Option<(&str, &str)> {
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &line[digits..];
    let mut it = rest.chars();
    if !matches!(it.next()?, '.' | ')') {
        return None;
    }
    let tail = &rest[1..];
    let after = tail.trim_start_matches(char::is_whitespace);
    (after.len() != tail.len() && !after.is_empty()).then_some((&line[..digits], after))
}

/// The blocks of a note, in the order and with the spacing rules of `MarkdownView.blocks`.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = vec![];
    let mut code: Option<Vec<&str>> = None;
    for (i, raw) in text.split('\n').enumerate() {
        let line = trim_blanks(raw);
        if line.starts_with("```") {
            match code.take() {
                Some(c) => out.push(Block::Code(c.join("\n"))),
                None => code = Some(vec![]),
            }
            continue;
        }
        if let Some(c) = code.as_mut() {
            c.push(raw);
            continue;
        }
        if line.is_empty() {
            out.push(Block::Space);
            continue;
        }
        if line == "---" || line == "***" {
            out.push(Block::Rule);
            continue;
        }
        if let Some((level, t)) = heading(line) {
            out.push(Block::Heading(level, t.to_string()));
            continue;
        }
        if let Some(after) = after_bullet_marker(line) {
            if let Some((done, t)) = task_box(after) {
                out.push(Block::Task { done, text: t.to_string(), line: i });
                continue;
            }
            if !after.is_empty() {
                let indent = raw.chars().take_while(|c| *c == ' ' || *c == '\t').count() / 2;
                out.push(Block::Bullet { text: after.to_string(), indent });
                continue;
            }
        }
        if let Some((n, t)) = numbered(line) {
            out.push(Block::Numbered(n.to_string(), t.to_string()));
            continue;
        }
        if let Some(rest) = line.strip_prefix('>') {
            let rest = rest.strip_prefix(char::is_whitespace).unwrap_or(rest);
            out.push(Block::Quote(rest.to_string()));
            continue;
        }
        out.push(Block::Paragraph(line.to_string()));
    }
    if let Some(c) = code {
        out.push(Block::Code(c.join("\n")));
    }
    // no empty space at the start or end, and no two in a row (the check looks at the list before filtering)
    let last = out.len().wrapping_sub(1);
    let keep: Vec<bool> = out
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if *b != Block::Space {
                return true;
            }
            !(i == 0 || i == last || out[i - 1] == Block::Space)
        })
        .collect();
    out.into_iter().zip(keep).filter(|(_, k)| *k).map(|(b, _)| b).collect()
}

/// `NotesView.toggleTask`: tick or untick the checklist on that line of the note.
pub fn toggle_task(text: &str, line: usize) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let Some(l) = lines.get(line).cloned() else { return text.to_string() };
    if let Some(p) = l.find("[ ]") {
        lines[line] = format!("{}[x]{}", &l[..p], &l[p + 3..]);
    } else if let Some(p) = l.find("[x]").or_else(|| l.find("[X]")) {
        lines[line] = format!("{}[ ]{}", &l[..p], &l[p + 3..]);
    }
    lines.join("\n")
}

// ---- inline ----

fn is_tag_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || c == '_' || c == '-'
}

/// The `#tags` of a text: after the start or a whitespace, letters, digits, `_` and `-`, lowercased,
/// each once, in order of appearance (`Note.tags(in:)`).
pub fn tags_in(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut seen: Vec<String> = vec![];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            let mut j = i + 1;
            while j < chars.len() && is_tag_char(chars[j]) {
                j += 1;
            }
            if j > i + 1 {
                let t: String = chars[i + 1..j].iter().collect::<String>().to_lowercase();
                if !seen.contains(&t) {
                    seen.push(t);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    seen
}

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Links that a click may open. `menew-tag://` is the tag chip of the Mac.
fn safe_url(u: &str) -> bool {
    let l = u.to_lowercase();
    l.starts_with("https://") || l.starts_with("http://") || l.starts_with("mailto:")
}

fn find_close(chars: &[char], from: usize, hi: usize, pat: &[char]) -> Option<usize> {
    let mut i = from;
    while i + pat.len() <= hi {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i..i + pat.len()] == *pat {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn render(chars: &[char], lo: usize, hi: usize, out: &mut String) {
    let mut i = lo;
    'outer: while i < hi {
        let c = chars[i];
        let next = if i + 1 < hi { Some(chars[i + 1]) } else { None };
        // backslash escapes
        if c == '\\' {
            if let Some(n) = next {
                if "\\`*_{}[]()#+-.!~>".contains(n) {
                    out.push_str(&escape(&n.to_string()));
                    i += 2;
                    continue;
                }
            }
        }
        if c == '`' {
            if let Some(e) = find_close(chars, i + 1, hi, &['`']).filter(|e| *e > i + 1) {
                out.push_str("<tt>");
                out.push_str(&escape(&chars[i + 1..e].iter().collect::<String>()));
                out.push_str("</tt>");
                i = e + 1;
                continue;
            }
        }
        for (mark, open, close) in [("**", "<b>", "</b>"), ("__", "<b>", "</b>"), ("~~", "<s>", "</s>")] {
            let m: Vec<char> = mark.chars().collect();
            if i + 2 <= hi && chars[i..i + 2] == m[..] {
                if let Some(e) = find_close(chars, i + 2, hi, &m).filter(|e| *e > i + 2 && !chars[i + 2].is_whitespace()) {
                    out.push_str(open);
                    render(chars, i + 2, e, out);
                    out.push_str(close);
                    i = e + 2;
                    continue 'outer;
                }
            }
        }
        if (c == '*' || c == '_') && next.map(|n| !n.is_whitespace() && n != c).unwrap_or(false) {
            // `snake_case` is not italic: an underscore inside a word does not open
            let word_before = i > lo && chars[i - 1].is_alphanumeric();
            if !(c == '_' && word_before) {
                if let Some(e) = find_close(chars, i + 1, hi, &[c]).filter(|e| *e > i + 1 && !chars[*e - 1].is_whitespace()) {
                    let word_after = c == '_' && e + 1 < hi && chars[e + 1].is_alphanumeric();
                    if !word_after {
                        out.push_str("<i>");
                        render(chars, i + 1, e, out);
                        out.push_str("</i>");
                        i = e + 1;
                        continue;
                    }
                }
            }
        }
        if c == '[' {
            if let Some(mid) = find_close(chars, i + 1, hi, &[']', '(']) {
                if let Some(end) = find_close(chars, mid + 2, hi, &[')']) {
                    let label = &chars[i + 1..mid];
                    let url: String = chars[mid + 2..end].iter().collect();
                    if !label.is_empty() && safe_url(url.trim()) {
                        out.push_str(&format!("<a href=\"{}\">", escape(url.trim())));
                        render(chars, i + 1, mid, out);
                        out.push_str("</a>");
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        if c == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            let mut j = i + 1;
            while j < hi && is_tag_char(chars[j]) {
                j += 1;
            }
            if j > i + 1 {
                let name: String = chars[i + 1..j].iter().collect();
                out.push_str(&format!("<a href=\"menew-tag://{}\">#{}</a>", escape(&name.to_lowercase()), escape(&name)));
                i = j;
                continue;
            }
        }
        out.push_str(&escape(&c.to_string()));
        i += 1;
    }
}

/// The inline Markdown of one block as Pango markup (`MarkdownView.inline`).
pub fn inline_pango(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 16);
    render(&chars, 0, chars.len(), &mut out);
    out
}

/// The tag in a `menew-tag://name` link.
pub fn tag_of_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("menew-tag://").filter(|t| !t.is_empty())
}
