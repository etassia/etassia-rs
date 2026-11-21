//! PlantUML's creole inline markup in a one-line label: toggles, the
//! b/i/u/s/w tags, `<color:..>`, `<size:..>`, `<back:..>` (kept), math
//! shown as monospace source, and undecodable images. Port of
//! `notation/plantuml/creole.rb` (the inline icon/emoji lane is out of
//! scope: component labels carry no pictures).

use crate::svg::Elem;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    wave: bool,
    mono: bool,
    color: Option<String>,
    size: Option<String>,
    font: Option<String>,
    underline_colour: Option<String>,
    strike_colour: Option<String>,
}

impl Style {
    /// `Creole.monospace?`: the `""` toggle or a font the name of which
    /// says mono.
    pub(crate) fn monospace(&self) -> bool {
        self.mono
            || self
                .font
                .as_deref()
                .is_some_and(|font| font.to_lowercase().contains("mono"))
    }
}

#[derive(Clone, Debug)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

const TOGGLES: [&str; 6] = ["**", "//", "__", "--", "~~", "\"\""];

fn toggle_style(marker: &str) -> Option<&'static str> {
    match marker {
        "**" => Some("bold"),
        "//" => Some("italic"),
        "__" => Some("underline"),
        "--" => Some("strike"),
        "~~" => Some("wave"),
        "\"\"" => Some("mono"),
        _ => None,
    }
}

fn tag_style(name: &str) -> Option<&'static str> {
    match name {
        "b" => Some("bold"),
        "i" => Some("italic"),
        "u" => Some("underline"),
        "s" | "strike" | "del" => Some("strike"),
        "w" => Some("wave"),
        _ => None,
    }
}

enum Token {
    Text(String),
    Marker {
        text: String,
        escaped: bool,
        formula_body: Option<String>,
        image: bool,
        close: bool,
        tag: Option<String>,
        arg: Option<String>,
    },
}

/// The line's styled pieces; one unstyled run when there is no markup.
pub fn parse(text: &str) -> Vec<Run> {
    let tokens = split(text);
    let paired = paired_toggles(&tokens);
    let mut state = Style::default();
    let mut runs: Vec<Run> = vec![];
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Text(text) => runs.push(Run {
                text: text.clone(),
                style: state.clone(),
            }),
            Token::Marker {
                text,
                escaped,
                formula_body,
                image,
                close,
                tag,
                arg,
            } => {
                if *escaped {
                    runs.push(Run {
                        text: text.clone(),
                        style: state.clone(),
                    });
                    continue;
                }
                if let Some(body) = formula_body {
                    let mut style = state.clone();
                    style.mono = true;
                    runs.push(Run {
                        text: body.clone(),
                        style,
                    });
                    continue;
                }
                if *image {
                    let mut style = state.clone();
                    style.mono = true;
                    runs.push(Run {
                        text: "(Cannot decode)".into(),
                        style,
                    });
                    continue;
                }
                let name = tag.as_deref().unwrap_or("");
                if TOGGLES.contains(&text.as_str()) {
                    if paired.contains(&index) {
                        // A paired toggle switches the style and its
                        // marker is gone.
                        apply_toggle(&mut state, text);
                    } else {
                        // Ruby keeps an unpaired marker as literal text.
                        runs.push(Run {
                            text: text.clone(),
                            style: state.clone(),
                        });
                    }
                } else if !name.is_empty() {
                    apply_tag(&mut state, name, *close, arg.as_deref());
                } else {
                    runs.push(Run {
                        text: text.clone(),
                        style: state.clone(),
                    });
                }
            }
        }
    }
    let runs: Vec<Run> = runs
        .into_iter()
        .filter(|run| !run.text.is_empty())
        .collect();
    merge(runs)
}

fn merge(runs: Vec<Run>) -> Vec<Run> {
    let mut merged: Vec<Run> = vec![];
    for piece in runs {
        if let Some(last) = merged.last_mut() {
            if last.style == piece.style {
                last.text.push_str(&piece.text);
                continue;
            }
        }
        merged.push(piece);
    }
    merged
}

/// The line as drawn: the markup is gone, its text stays.
pub fn plain(text: &str) -> String {
    parse(text).into_iter().map(|run| run.text).collect()
}

fn split(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = vec![];
    let mut position = 0;
    let mut plain_start = 0;
    while position < bytes.len() {
        if let Some((token, width)) = marker_at(&text[position..]) {
            if position > plain_start {
                out.push(Token::Text(text[plain_start..position].to_string()));
            }
            out.push(token);
            position += width;
            plain_start = position;
        } else {
            position += text[position..].chars().next().unwrap().len_utf8();
        }
    }
    if plain_start < text.len() {
        out.push(Token::Text(text[plain_start..].to_string()));
    }
    out
}

fn marker_at(text: &str) -> Option<(Token, usize)> {
    // <math>..</math> / <latex>..</latex>
    for name in ["math", "latex"] {
        let open = format!("<{name}>");
        if text.to_lowercase().starts_with(&open) {
            if let Some(close_at) = text.to_lowercase()[open.len()..].find(&format!("</{name}>")) {
                let body = text[open.len()..open.len() + close_at].to_string();
                return Some((
                    Token::Marker {
                        text: String::new(),
                        escaped: false,
                        formula_body: Some(body),
                        image: false,
                        close: false,
                        tag: None,
                        arg: None,
                    },
                    open.len() + close_at + format!("</{name}>").len(),
                ));
            }
        }
    }
    if text.starts_with("<img:") || text.starts_with("<img ") {
        if let Some(close) = text.find('>') {
            return Some((
                Token::Marker {
                    text: String::new(),
                    escaped: false,
                    formula_body: None,
                    image: true,
                    close: false,
                    tag: None,
                    arg: None,
                },
                close + 1,
            ));
        }
    }
    for marker in TOGGLES {
        if text.starts_with(marker) {
            // A tilde before the marker keeps it literal; the escape was
            // consumed as text with `~` dropped by the caller's loop.
            return Some((
                Token::Marker {
                    text: marker.to_string(),
                    escaped: false,
                    formula_body: None,
                    image: false,
                    close: false,
                    tag: None,
                    arg: None,
                },
                marker.len(),
            ));
        }
    }
    // `~` before a toggle marker keeps the marker literal.
    if text.starts_with('~') {
        for marker in TOGGLES {
            if text[1..].starts_with(marker) {
                return Some((
                    Token::Marker {
                        text: marker.to_string(),
                        escaped: true,
                        formula_body: None,
                        image: false,
                        close: false,
                        tag: None,
                        arg: None,
                    },
                    1 + marker.len(),
                ));
            }
        }
    }
    if text.starts_with('<') {
        if let Some(close) = text.find('>') {
            let inner = &text[1..close];
            let (close_tag, rest) = if let Some(rest) = inner.strip_prefix('/') {
                (true, rest)
            } else {
                (false, inner)
            };
            let (name, arg) = match rest.find([':', ' ']) {
                Some(at) => (&rest[..at], Some(rest[at + 1..].trim().to_string())),
                None => (rest, None),
            };
            let known = matches!(
                name.to_lowercase().as_str(),
                "b" | "i" | "u" | "s" | "w" | "strike" | "del" | "color" | "size" | "back" | "font"
            );
            if known {
                return Some((
                    Token::Marker {
                        text: text[..close + 1].to_string(),
                        escaped: false,
                        formula_body: None,
                        image: false,
                        close: close_tag,
                        tag: Some(name.to_string()),
                        arg,
                    },
                    close + 1,
                ));
            }
        }
    }
    None
}

/// Indexes of the toggle markers that have a partner.
fn paired_toggles(tokens: &[Token]) -> Vec<usize> {
    let mut open: Vec<(String, usize)> = vec![];
    let mut found = vec![];
    for (index, token) in tokens.iter().enumerate() {
        if let Token::Marker {
            text, escaped, tag, ..
        } = token
        {
            if *escaped || tag.is_some() || !TOGGLES.contains(&text.as_str()) {
                continue;
            }
            if let Some(at) = open.iter().position(|(marker, _)| marker == text) {
                let (_, partner) = open.remove(at);
                found.push(partner);
                found.push(index);
            } else {
                open.push((text.clone(), index));
            }
        }
    }
    found
}

fn apply_toggle(state: &mut Style, marker: &str) {
    match toggle_style(marker).expect("known toggle") {
        "bold" => state.bold = !state.bold,
        "italic" => state.italic = !state.italic,
        "underline" => state.underline = !state.underline,
        "strike" => state.strike = !state.strike,
        "wave" => state.wave = !state.wave,
        "mono" => state.mono = !state.mono,
        _ => {}
    }
}

fn apply_tag(state: &mut Style, name: &str, close: bool, arg: Option<&str>) {
    let name = name.to_lowercase();
    if name == "back" {
        return;
    }
    if name == "color" {
        if close {
            state.color = None;
        } else {
            state.color = arg.map(str::to_string);
        }
        return;
    }
    if name == "size" {
        if close {
            state.size = None;
        } else {
            state.size = arg.map(str::to_string);
        }
        return;
    }
    if name == "font" {
        if close {
            state.font = None;
        } else {
            state.font = arg.map(str::to_string);
        }
        return;
    }
    let Some(style) = tag_style(&name) else {
        return;
    };
    let mut apply = |state: &mut Style| match style {
        "bold" => state.bold = !close,
        "italic" => state.italic = !close,
        "underline" => state.underline = !close,
        "strike" => state.strike = !close,
        "wave" => state.wave = !close,
        _ => {}
    };
    if close {
        // Ruby `closer_known?`: a closing tag only lands when the style is
        // set (ignored names excepted).
        let set = match style {
            "bold" => state.bold,
            "italic" => state.italic,
            "underline" => state.underline,
            "strike" => state.strike,
            "wave" => state.wave,
            _ => false,
        };
        if set {
            apply(state);
            match style {
                "underline" => state.underline_colour = None,
                "strike" => state.strike_colour = None,
                _ => {}
            }
        }
        return;
    }
    apply(state);
    let colour = arg.map(str::trim).unwrap_or("");
    if !colour.is_empty() {
        match style {
            "underline" => state.underline_colour = Some(colour.to_string()),
            "strike" => state.strike_colour = Some(colour.to_string()),
            _ => {}
        }
    }
}

/// Gives `element` the line's content: plain text when there is no
/// markup, one tspan per styled piece otherwise.
pub fn assign(element: Elem, text: &str) -> Elem {
    let runs = parse(text);
    if runs.iter().all(|run| run.style == Style::default()) {
        let joined: String = runs.into_iter().map(|run| run.text).collect();
        return element.content(&joined);
    }
    let mut element = element.attr("xml:space", "preserve");
    for run in runs {
        let mut tspan = Elem::new("tspan").content(&run.text);
        if run.style.bold {
            tspan = tspan.attr("font-weight", "bold");
        }
        if run.style.italic {
            tspan = tspan.attr("font-style", "italic");
        }
        if run.style.mono {
            tspan = tspan.attr("font-family", "monospace");
        }
        let mut words = vec![];
        if run.style.underline {
            words.push("underline");
        }
        if run.style.wave {
            words.push("wavy underline");
        }
        if run.style.strike {
            words.push("line-through");
        }
        if !words.is_empty() {
            tspan = tspan.attr("text-decoration", &words.join(" "));
        }
        if let Some(color) = &run.style.color {
            tspan = tspan.attr("fill", &colour(color));
        }
        if let Some(size) = &run.style.size {
            if size.chars().all(|c| c.is_ascii_digit()) {
                tspan = tspan.attr("font-size", size);
            }
        }
        element = element.child(tspan);
    }
    element
}

fn colour(value: &str) -> String {
    let hex = value.strip_prefix('#').unwrap_or(value);
    let valid = hex.len() == 3 || hex.len() == 6;
    let hex_digits = hex.chars().all(|c| c.is_ascii_hexdigit());
    if valid && hex_digits {
        format!("#{hex}")
    } else {
        value.to_string()
    }
}
