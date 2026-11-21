//! Port of Ruby `Notation::PlantUML::Arrow` (`arrow.rb`): reads a relation
//! glyph such as `<|--`, `o-down-`, `*.r.>` or `..>`. The one lookahead in
//! the Ruby patterns (a bare `o`/`*`/`+` right marker only ends the glyph
//! before a blank or a quote) is a hand check here, and the relation scan
//! replays the regex's own backtracking order.
use super::model::{Arrow, ArrowKind, Direction, Markers};
use std::sync::OnceLock;

/// Ruby `Arrow.parse(glyph)`: the parsed arrow, `None` for `<->` (which a
/// sequence diagram also has and which is not drawn here).
pub fn parse(glyph_text: &str) -> Option<Arrow> {
    let glyph = scan(glyph_text, 0, true, true)?;
    let left = glyph.left.as_deref();
    let right = glyph.right.as_deref();
    if plain_heads(left, right) {
        return None;
    }
    Some(details(glyph_text, left, &glyph.body, right))
}

/// Ruby `plain_heads?`: both ends carry `<` or `>` markers.
fn plain_heads(left: Option<&str>, right: Option<&str>) -> bool {
    let heads = ["<", ">"];
    [left, right]
        .iter()
        .all(|end| end.map(|e| heads.contains(&e)).unwrap_or(false))
}

fn details(glyph_text: &str, left: Option<&str>, body: &str, right: Option<&str>) -> Arrow {
    let style = bracket_style(body);
    let bare = match &style {
        Some(content) => body.replace(&format!("[{content}]"), ""),
        None => body.to_string(),
    };
    let style_text = style.unwrap_or_default();
    let dashed = bare.contains('.') || has_dashing_word(&style_text);
    let markers = markers_of(left, right, dashed);
    let (color, width, dotted) = stroke_of(&style_text);
    Arrow {
        markers,
        dashed,
        plain: matches!(glyph_text, "-->" | "<--" | "->" | "<-"),
        direction: direction_of(&bare),
        length: bare.chars().filter(|c| *c == '-' || *c == '.').count() as f64,
        color,
        width,
        dotted,
    }
}

/// The first `[...]` bracket content in the body.
fn bracket_style(body: &str) -> Option<String> {
    let open = body.find('[')?;
    let close = body[open..].find(']')? + open;
    Some(body[open + 1..close].to_string())
}

/// Ruby `DASHING = /\b(?:dashed|dotted)\b/i`.
fn has_dashing_word(style: &str) -> bool {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\b(?:dashed|dotted)\b").unwrap())
        .is_match(style)
}

fn markers_of(left: Option<&str>, right: Option<&str>, dashed: bool) -> Markers {
    Markers {
        left: left.map(|marker| kind_of(marker, dashed)),
        right: right.map(|marker| kind_of(marker, dashed)),
    }
}

fn kind_of(marker: &str, dashed: bool) -> ArrowKind {
    let shape: String =
        marker.chars().filter(|c| *c != '<' && *c != '>').collect();
    if shape.is_empty() {
        return line_kind(dashed);
    }
    if shape == "|" {
        return if dashed {
            ArrowKind::Implementation
        } else {
            ArrowKind::Extension
        };
    }
    match shape.as_str() {
        "o" => ArrowKind::Aggregation,
        "*" => ArrowKind::Composition,
        "+" => ArrowKind::Nesting,
        _ => line_kind(dashed),
    }
}

fn line_kind(dashed: bool) -> ArrowKind {
    if dashed {
        ArrowKind::Dependency
    } else {
        ArrowKind::Association
    }
}

/// Ruby `stroke_of`: of comma-separated colours the last wins, of a
/// `#red;#green` pair the first.
fn stroke_of(style: &str) -> (Option<String>, Option<f64>, bool) {
    let words: Vec<&str> =
        style.split(|c| c == ',' || c == ';').map(str::trim).collect();
    let color = color_of(style);
    let width = words.iter().find_map(|word| {
        word.strip_prefix("thickness=").and_then(|digits| {
            let digits: String =
                digits.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                return None;
            }
            digits.parse::<f64>().ok().filter(|w| *w > 0.0)
        })
    });
    let dotted = words.iter().any(|word| word.eq_ignore_ascii_case("dotted"));
    (color, width, dotted)
}

fn color_of(style: &str) -> Option<String> {
    let mut found = None;
    for group in style.split(',') {
        let first = group.split(';').next().unwrap_or("").trim();
        if let Some(rest) = first.strip_prefix('#') {
            let word: String =
                rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !word.is_empty() {
                found = Some(word);
            }
        }
    }
    found
}

/// Ruby `direction_of`: the first lowercase run in the body names the
/// direction through its first letter; a one-character body lays out
/// rightwards.
fn direction_of(body: &str) -> Option<Direction> {
    let chars: Vec<char> = body.chars().collect();
    let mut index = 0;
    while index < chars.len() && !chars[index].is_ascii_lowercase() {
        index += 1;
    }
    if index < chars.len() {
        return Some(match chars[index] {
            'u' => Direction::Up,
            'd' => Direction::Down,
            'l' => Direction::Left,
            _ => Direction::Right,
        });
    }
    if chars.len() == 1 {
        return Some(Direction::Right);
    }
    None
}

#[derive(Clone, Debug)]
pub(crate) struct Glyph {
    pub text: String,
    pub left: Option<String>,
    pub body: String,
    pub right: Option<String>,
    pub end: usize,
}

/// Ruby `PATTERN` at `at`, replaying the regex's backtracking order: the
/// optional direction group is inside the body and the right marker sits
/// after it, so the greedy choices fall away right to left.
pub(crate) fn scan(
    text: &str,
    at: usize,
    with_direction: bool,
    with_right: bool,
) -> Option<Glyph> {
    let attempts = [
        (true, true),
        (true, false),
        (false, true),
        (false, false),
    ];
    for (direction, right) in attempts {
        if direction && !with_direction {
            continue;
        }
        if right && !with_right {
            continue;
        }
        if let Some(glyph) = scan_once(text, at, direction, right) {
            return Some(glyph);
        }
    }
    None
}

fn scan_once(
    text: &str,
    at: usize,
    with_direction: bool,
    with_right: bool,
) -> Option<Glyph> {
    let bytes = text.as_bytes();
    let mut pos = at;
    // LEFT_MARKER = (?:<\||<|o|\*|\+)
    let mut left = None;
    if bytes[pos..].starts_with(b"<|") {
        left = Some("<|".to_string());
        pos += 2;
    } else if bytes.get(pos) == Some(&b'<') {
        left = Some("<".to_string());
        pos += 1;
    } else if matches!(bytes.get(pos), Some(b'o' | b'*' | b'+')) {
        left = Some(((bytes[pos]) as char).to_string());
        pos += 1;
    }
    // BODY = [-.]+(dir[-.]+)? | -[...]-*
    let (body, body_end) = scan_body(text, pos, with_direction)?;
    pos = body_end;
    // RIGHT_IN_LINE = \|> | > | (o|*|+)(?=[ \t"])
    let mut right = None;
    if with_right {
        if bytes[pos..].starts_with(b"|>") {
            right = Some("|>".to_string());
            pos += 2;
        } else if bytes.get(pos) == Some(&b'>') {
            right = Some(">".to_string());
            pos += 1;
        } else if matches!(bytes.get(pos), Some(b'o' | b'*' | b'+')) {
            if matches!(bytes.get(pos + 1), Some(b' ') | Some(b'\t') | Some(b'"')) {
                right = Some(((bytes[pos]) as char).to_string());
                pos += 1;
            }
        }
    }
    Some(Glyph {
        text: text[at..pos].to_string(),
        left,
        body,
        right,
        end: pos,
    })
}

/// The body at `at`; `None` when no body starts there.
fn scan_body(
    text: &str,
    at: usize,
    with_direction: bool,
) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    // BRACKET = -\[[^\]]*\]-*
    if bytes.get(at) == Some(&b'-') && bytes.get(at + 1) == Some(&b'[') {
        let mut pos = at + 2;
        while pos < bytes.len() && bytes[pos] != b']' {
            pos += 1;
        }
        if pos >= bytes.len() {
            return None;
        }
        pos += 1; // the ]
        while bytes.get(pos) == Some(&b'-') {
            pos += 1;
        }
        return Some((text[at..pos].to_string(), pos));
    }
    let run = dash_run(bytes, at)?;
    let run_end = at + run;
    if with_direction {
        static WORDS: [&str; 8] =
            ["up", "down", "left", "right", "u", "d", "l", "r"];
        for word in WORDS {
            if text[run_end..].starts_with(word) {
                let after = run_end + word.len();
                let second = dash_run(bytes, after).unwrap_or(0);
                if second > 0 {
                    return Some((text[at..after + second].to_string(), after + second));
                }
            }
        }
    }
    Some((text[at..run_end].to_string(), run_end))
}

fn dash_run(bytes: &[u8], at: usize) -> Option<usize> {
    let mut pos = at;
    while matches!(bytes.get(pos), Some(b'-') | Some(b'.')) {
        pos += 1;
    }
    if pos == at { None } else { Some(pos - at) }
}
