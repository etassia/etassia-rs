//! Reader support: ports of the Ruby reader's small modules —
//! `PackageColor`, `ClassName`, `Caption.read`, `Directives.match?`,
//! `UnsupportedConstructs.member_construct`, `GLOBAL_COMMAND` and the
//! `plain_note_text` cleanup.
use super::model::{Caption, CaptionKind};

pub(crate) fn refused() -> crate::Error {
    crate::Error::Unported("class".into())
}

/// Ruby `PackageColor.hex`.
pub(crate) fn package_color(written: &str) -> Option<String> {
    let digits = written.strip_prefix('#').unwrap_or(written);
    let is_hex = !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_hexdigit());
    if !is_hex {
        return named_color(digits);
    }
    if digits.len() != 3 && digits.len() != 6 {
        return None;
    }
    let expanded = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        digits.to_string()
    };
    Some(format!("#{}", expanded.to_uppercase()))
}

fn named_color(word: &str) -> Option<String> {
    let hex = match word.to_lowercase().as_str() {
        "blue" => "#0000FF",
        "gray" => "#808080",
        "green" => "#008000",
        "lightgray" => "#D3D3D3",
        "orange" => "#FFA500",
        "pink" => "#FFC0CB",
        "purple" => "#800080",
        "red" => "#FF0000",
        "white" => "#FFFFFF",
        "yellow" => "#FFFF00",
        _ => return None,
    };
    Some(hex.to_string())
}

/// Ruby `ClassName`.
pub(crate) mod class_name {
    /// `\u` escape in a name.
    pub(crate) fn escapes(name: &str) -> bool {
        name.contains("\\u")
    }

    /// `split(".", -1)`: the trailing empty part is kept.
    fn split_keep(name: &str) -> Vec<&str> {
        let mut out: Vec<&str> = name.split('.').collect();
        if name.ends_with('.') {
            out.push("");
        }
        out
    }

    /// The namespaces, outermost first.
    pub(crate) fn namespaces(name: &str) -> Vec<String> {
        let parts = split_keep(name);
        parts[..parts.len() - 1].iter().map(|p| p.to_string()).collect()
    }

    /// The drawn lines of the class itself; a blank line is a
    /// non-breaking space so it keeps its height.
    pub(crate) fn lines(name: &str) -> Vec<String> {
        let parts = split_keep(name);
        let text = unescape_all(parts[parts.len() - 1]);
        text.split('\n')
            .map(|line| {
                let kept = line.trim_matches(|c: char| {
                    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0')
                });
                if kept.is_empty() {
                    "\u{A0}".to_string()
                } else {
                    kept.to_string()
                }
            })
            .collect()
    }

    /// Ruby `gsub(/\\(.)/m) { unescape($1) }`.
    fn unescape_all(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') | Some('r') | Some('l') => out.push('\n'),
                Some('t') => out.push_str("\u{A0}\u{A0}\u{A0}\u{A0}"),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        }
        out
    }
}

/// Ruby `Caption.read`.
pub(crate) fn caption_read(text: &str) -> Option<Caption> {
    static LINE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let line = LINE.get_or_init(|| {
        regex::Regex::new(r"(?i)\A(title|header|footer|caption|legend)[ \t]+(.+)\z")
            .unwrap()
    });
    let m = line.captures(text)?;
    let word = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
    let kind = match word.as_str() {
        "title" => CaptionKind::Title,
        "header" => CaptionKind::Header,
        "footer" => CaptionKind::Footer,
        "caption" => CaptionKind::Caption,
        _ => CaptionKind::Legend,
    };
    let body = m.get(2).map(|g| g.as_str()).unwrap_or("").trim();
    if !caption_plain(body) || arrow_start(body) {
        return None;
    }
    if kind == CaptionKind::Legend && alignment_word(body) {
        return None;
    }
    Some(Caption { kind, text: body.to_string() })
}

fn alignment_word(body: &str) -> bool {
    matches!(
        body.to_lowercase().as_str(),
        "left" | "right" | "center" | "top" | "bottom"
    )
}

/// Ruby `PLAIN = /\A[[:alnum:]][[:alnum:] .,:;!?()'-]*\z/`.
fn caption_plain(body: &str) -> bool {
    let mut chars = body.chars();
    let Some(first) = chars.next() else { return false };
    if !first.is_alphanumeric() {
        return false;
    }
    chars.all(|c| {
        c.is_alphanumeric()
            || matches!(c, ' ' | '.' | ',' | ':' | ';' | '!' | '?' | '(' | ')' | '\'' | '-')
    })
}

/// Ruby `ARROW_START = /\A\S*(?:--|\.\.|->|<-)/`.
fn arrow_start(body: &str) -> bool {
    let start = body.len() - body.trim_start().len();
    let run = &body[start..];
    let end = run.find(char::is_whitespace).unwrap_or(run.len());
    let head = &run[..end];
    head.contains("--") || head.contains("..") || head.contains("->") || head.contains("<-")
}

/// Ruby `Directives.match?`.
pub(crate) fn directive_matches(text: &str) -> bool {
    static PRAGMA: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pragma = PRAGMA.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\A!pragma[ \t]+(?:layout[ \t]+(?:smetana|dot)|svginteractive[ \t]+(?:true|false)|svgparser[ \t]+(?:sax|stax))\z",
        )
        .unwrap()
    });
    static HIDE_SHOW: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let hide_show = HIDE_SHOW.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\A(?:hide|show)[ \t]+(?:empty[ \t]+)?(?:[A-Za-z_][A-Za-z0-9_]*[ \t]+|<<[^<>\s]+>>[ \t]+)?(?:members|attributes|methods|fields|circle|stereotype)\z",
        )
        .unwrap()
    });
    static SKINPARAM: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let skinparam = SKINPARAM.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\Askinparam[ \t]+[A-Za-z0-9_.]+(?:<<[^<>\s]+>>)?[ \t]+[^{}\s][^{}]*\z",
        )
        .unwrap()
    });
    static DIRECTION: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let direction = DIRECTION.get_or_init(|| {
        regex::Regex::new(r"(?i)\A(?:left to right|top to bottom) direction\z").unwrap()
    });
    pragma.is_match(text)
        || hide_show.is_match(text)
        || skinparam.is_match(text)
        || direction.is_match(text)
}

/// Ruby `GLOBAL_COMMAND`: a sequence-diagram word, not a participant.
pub(crate) fn global_command(text: &str) -> bool {
    static COMMAND: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let command = COMMAND.get_or_init(|| {
        regex::Regex::new(r"(?i)\A(?:caption|footer|header|legend|mainframe|title)[ \t]").unwrap()
    });
    command.is_match(text)
}

/// Ruby `UnsupportedConstructs.member_construct`: the line holds a member
/// construct the subset does not read.
pub(crate) fn member_construct(text: &str) -> bool {
    if text.starts_with('!') {
        return true;
    }
    static MODIFIER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let modifier = MODIFIER.get_or_init(|| {
        regex::Regex::new(r"(?i)\{classifier\}|\{(?:static|abstract|field|method)\}\z").unwrap()
    });
    if modifier.is_match(text) {
        return true;
    }
    if text.starts_with("--")
        || text.starts_with("==")
        || text.starts_with("..")
        || text.starts_with("__")
    {
        return true;
    }
    text.contains("<<")
}

/// Ruby `plain_note_text`: an undecodable `<img:>` becomes its printed
/// text and runs of spaces fold to one.
pub(crate) fn plain_note_text(text: &str) -> String {
    static MARKUP: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let markup = MARKUP.get_or_init(|| {
        regex::Regex::new(r"(?i)<img:[^>]*>").unwrap()
    });
    let replaced = markup.replace_all(text, "\"\"(Cannot decode)\"\"").to_string();
    squeeze_spaces(&replaced)
}

/// Ruby `String#squeeze(" ")`: runs of the same character fold to one.
pub(crate) fn squeeze_spaces(text: &str) -> String {
    let mut out = String::new();
    let mut previous = '\0';
    for c in text.chars() {
        if c == ' ' && previous == ' ' {
            continue;
        }
        out.push(c);
        previous = c;
    }
    out
}

/// Ruby `STARTUML` with its lookahead, by hand.
pub(crate) fn is_startuml(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("@startuml") else {
        return false;
    };
    !rest
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}
