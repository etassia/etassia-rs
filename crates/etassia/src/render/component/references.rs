//! The patterns and hand-rolled matchers that name a node in a link or a
//! declaration. Port of `node_graph/references.rb`. The arrow grammar is
//! matched by hand: its direction words carry a lookahead the regex
//! engine here does not have.

/// `[\p{L}\p{N}_$]+`
pub fn ident_at(text: &str) -> Option<usize> {
    let end = text
        .char_indices()
        .find(|(_, c)| !(c.is_alphabetic() || c.is_numeric() || *c == '_' || *c == '$'))
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    (end > 0).then_some(end)
}

/// `\[[^\]]+\]|\([^()]*\)|:[^:\n]+:`
pub fn bracket_at(text: &str) -> Option<usize> {
    let first = text.chars().next()?;
    match first {
        '[' => {
            let close = text.find(']')?;
            let inner = &text[1..close];
            (!inner.is_empty() && !inner.contains(']')).then_some(close + 1)
        }
        '(' => {
            let close = text.find(')')?;
            let inner = &text[1..close];
            (!inner.contains('(') && !inner.contains(')')).then_some(close + 1)
        }
        ':' => {
            let close = text[1..].find(|c| c == ':' || c == '\n').map(|i| i + 1)?;
            let inner = &text[1..close];
            (!inner.contains('\n') && !inner.is_empty()).then_some(close + 1)
        }
        _ => None,
    }
}

/// `"[^"]+"`
fn quoted_at(text: &str) -> Option<usize> {
    if !text.starts_with('"') {
        return None;
    }
    let close = text[1..].find('"').map(|i| i + 1)?;
    Some(close + 1)
}

/// `#{BRACKET}|"[^"]+"|#{IDENT}` — tried in that order.
pub fn ref_at(text: &str) -> Option<usize> {
    bracket_at(text)
        .or_else(|| quoted_at(text))
        .or_else(|| ident_at(text))
}

/// Whether `text` starts with a bracketed reference.
pub fn is_bracketed(text: &str) -> bool {
    bracket_at(text).is_some()
}

/// One link arrow, matched by hand (Ruby's `ARROW`).
pub struct ArrowMatch {
    pub start: usize,
    pub end: usize,
}

pub fn arrow_at(text: &str) -> Option<ArrowMatch> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut lts = 0;
    while i < bytes.len() && bytes[i] == b'<' && lts < 2 {
        i += 1;
        lts += 1;
    }
    let mut dashes = 0;
    while i < bytes.len() && matches!(bytes[i], b'-' | b'.' | b'=') {
        i += 1;
        dashes += 1;
    }
    if dashes == 0 {
        return None;
    }
    // Optional `[..]` or a direction word not followed by a letter.
    if bytes.get(i) == Some(&b'[') {
        if let Some(close) = text[i..].find(']') {
            i += close + 1;
        }
    } else {
        for word in ["up", "down", "left", "right"] {
            if text[i..].starts_with(word) {
                let after = bytes.get(i + word.len()).copied();
                let boundary = after
                    .map(|b| !char::from(b).is_alphabetic())
                    .unwrap_or(true);
                if boundary {
                    i += word.len();
                    break;
                }
            }
        }
    }
    while i < bytes.len() && matches!(bytes[i], b'-' | b'.' | b'=') {
        i += 1;
    }
    // Optional `(0)`, `(`, `0)` or `)`.
    if text[i..].starts_with("(0)") {
        i += 3;
    } else if text[i..].starts_with("(0") {
        i += 2;
    } else if text[i..].starts_with("0)") {
        i += 2;
    } else if bytes.get(i) == Some(&b')') {
        i += 1;
    }
    while i < bytes.len() && matches!(bytes[i], b'-' | b'.' | b'=') {
        i += 1;
    }
    let mut gts = 0;
    while i < bytes.len() && bytes[i] == b'>' && gts < 2 {
        i += 1;
        gts += 1;
    }
    Some(ArrowMatch { start: 0, end: i })
}

/// An `EDGE` match: the two refs, their optional `::port` names, the
/// arrow and the optional label.
pub struct EdgeMatch {
    pub a: String,
    pub a_port: Option<String>,
    pub arrow: String,
    pub b: String,
    pub b_port: Option<String>,
    pub label: Option<String>,
}

/// Ruby `References::EDGE`, anchored at both ends.
pub fn match_edge(text: &str) -> Option<EdgeMatch> {
    let mut at = 0;
    let a_len = ref_at(&text[at..])?;
    let a = text[at..at + a_len].to_string();
    at += a_len;
    let a_port = port_at(text, &mut at);
    skip_spaces(text, &mut at);
    let arrow = arrow_at(&text[at..])?;
    let arrow_text = text[at..at + arrow.end].to_string();
    at += arrow.end;
    skip_spaces(text, &mut at);
    let b_len = ref_at(&text[at..])?;
    let b = text[at..at + b_len].to_string();
    at += b_len;
    let b_port = port_at(text, &mut at);
    let label = label_at(text, at);
    Some(EdgeMatch { a, a_port, arrow: arrow_text, b, b_port, label })
}

/// `(?:::(?<port>#{IDENT}))?`
fn port_at(text: &str, at: &mut usize) -> Option<String> {
    if !text[*at..].starts_with("::") {
        return None;
    }
    let rest = &text[*at + 2..];
    let len = ident_at(rest)?;
    *at += 2 + len;
    Some(rest[..len].to_string())
}

fn skip_spaces(text: &str, at: &mut usize) {
    while text[*at..].starts_with(|c: char| c == ' ' || c == '\t') {
        *at += 1;
    }
}

/// `(?:\s*:\s*(?<label>.*))?\z`
fn label_at(text: &str, mut at: usize) -> Option<String> {
    let mut spaces = 0;
    while text[at + spaces..].starts_with(|c: char| c == ' ' || c == '\t') {
        spaces += 1;
    }
    if !text[at + spaces..].starts_with(':') {
        return None;
    }
    at += spaces + 1;
    let mut more = 0;
    while text[at + more..].starts_with(|c: char| c == ' ' || c == '\t') {
        more += 1;
    }
    at += more;
    Some(text[at..].to_string())
}

/// Ruby `References::lines_of`: split at `\n` or a literal `\n`, strip
/// each line, and a text with no lines reads as one empty line.
pub fn lines_of(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'n') {
            chars.next();
            lines.push(current.trim().to_string());
            current = String::new();
        } else if c == '\n' {
            lines.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() || lines.is_empty() {
        // Ruby's split drops a trailing empty field; only a text with
        // nothing at all reads as one empty line.
        if !current.is_empty() {
            lines.push(current.trim().to_string());
        } else if lines.is_empty() {
            lines.push(String::new());
        }
    }
    lines
}
