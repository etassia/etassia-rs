//! The settings readers: `Fill`, `NoteFill`, `PackageColor`, `Caption`,
//! `Paint` (the colour and thickness skinparams) and the `<style>` block
//! reader. Port of `sequence/fill.rb`, `note_fill.rb`, `paint.rb`,
//! `style.rb` and `../package_color.rb`, `../caption.rb`.

use super::model::{Appearance, Fill, HeadStyle};
use crate::render::pie::strip;

/// The names the style reader draws, from `style/colour_names.rb`.
fn colour_name(name: &str) -> Option<&'static str> {
    const TABLE: &[(&str, &str)] = &[
        ("black", "#000000"), ("silver", "#C0C0C0"), ("gray", "#808080"),
        ("grey", "#808080"), ("white", "#FFFFFF"), ("maroon", "#800000"),
        ("red", "#FF0000"), ("fuchsia", "#FF00FF"), ("magenta", "#FF00FF"),
        ("pink", "#FFC0CB"), ("orange", "#FFA500"), ("yellow", "#FFFF00"),
        ("lime", "#00FF00"), ("green", "#008000"), ("olive", "#808000"),
        ("teal", "#008080"), ("cyan", "#00FFFF"), ("aqua", "#00FFFF"),
        ("blue", "#0000FF"), ("navy", "#000080"), ("purple", "#800080"),
        ("brown", "#A52A2A"),
    ];
    TABLE.iter().find(|(key, _)| *key == name).map(|(_, hex)| *hex)
}

fn hex_digits_ok(digits: &[u8]) -> bool {
    digits.iter().all(|b| b.is_ascii_hexdigit())
}

fn none_fill() -> Fill {
    Fill { colour: "none".into(), opacity: None }
}

fn fill_from_hex(digits: &str) -> Fill {
    let full: String = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_string()
    };
    let (rgb, alpha) = if full.len() == 8 {
        let a = i64::from_str_radix(&full[6..], 16).unwrap_or(255);
        (full[..6].to_string(), a)
    } else {
        (full, 255)
    };
    if alpha == 0 {
        return none_fill();
    }
    Fill {
        colour: format!("#{}", rgb.to_uppercase()),
        opacity: if alpha == 255 {
            None
        } else {
            // Ruby: (alpha / 255.0).round(5)
            Some((alpha as f64 / 255.0 * 1e5).round() / 1e5)
        },
    }
}

/// `Fill.read`: `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `#transparent` or a
/// colour name; None when the token is not one of the forms.
pub fn fill_read(token: Option<&str>) -> Option<Fill> {
    let token = token?;
    let digits = token.strip_prefix('#')?;
    if digits.is_empty() {
        return None;
    }
    if digits.eq_ignore_ascii_case("transparent") {
        return Some(none_fill());
    }
    let bytes = digits.as_bytes();
    if bytes.len() == 3 || bytes.len() == 6 || bytes.len() == 8 {
        if hex_digits_ok(bytes) {
            return Some(fill_from_hex(digits));
        }
        return None;
    }
    named_fill(digits)
}

/// `Fill.named`: a bare colour name.
pub fn named_fill(name: &str) -> Option<Fill> {
    colour_name(&name.to_lowercase()).map(|hex| Fill {
        colour: hex.to_string(),
        opacity: None,
    })
}

/// `NoteFill.read`: the names PlantUML draws as these hex values.
pub fn note_fill_read(token: Option<&str>) -> Option<Fill> {
    let token = token?;
    const NAMES: &[(&str, &str)] = &[
        ("red", "#FF0000"), ("blue", "#0000FF"), ("green", "#008000"),
        ("yellow", "#FFFF00"), ("orange", "#FFA500"), ("pink", "#FFC0CB"),
        ("lightblue", "#ADD8E6"), ("lightgreen", "#90EE90"),
        ("lightgray", "#D3D3D3"), ("white", "#FFFFFF"),
        ("black", "#000000"), ("gray", "#808080"), ("cyan", "#00FFFF"),
        ("magenta", "#FF00FF"), ("purple", "#800080"), ("aqua", "#00FFFF"),
    ];
    match NAMES.iter().find(|(name, _)| *name == token.to_lowercase()) {
        Some((_, hex)) => Some(Fill { colour: (*hex).into(), opacity: None }),
        None => fill_read(Some(token)),
    }
}

/// `PackageColor.hex`: the fill written after a box title.
fn package_hex(text: &str) -> Option<String> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    const NAMED: &[(&str, &str)] = &[
        ("blue", "#0000FF"), ("gray", "#808080"), ("green", "#008000"),
        ("lightgray", "#D3D3D3"), ("orange", "#FFA500"), ("pink", "#FFC0CB"),
        ("purple", "#800080"), ("red", "#FF0000"), ("white", "#FFFFFF"),
        ("yellow", "#FFFF00"),
    ];
    if digits.len() != 3 && digits.len() != 6 {
        return NAMED.iter().find(|(name, _)| *name == digits.to_lowercase())
            .map(|(_, hex)| hex.to_string());
    }
    if !hex_digits_ok(digits.as_bytes()) {
        return None;
    }
    let full: String = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_string()
    };
    Some(format!("#{}", full.to_uppercase()))
}

/// Style.read's `colour`: hex or one of the two name tables.
fn style_colour(value: &str) -> Option<String> {
    if value.starts_with('#') {
        return package_hex(value);
    }
    if value.eq_ignore_ascii_case("lightyellow") {
        return Some("#FFFFE0".into());
    }
    named_fill(value).map(|fill| fill.colour)
}

/// `Caption.read`: whether the line is one of the five chrome keywords
/// plus one plain body, and the pair when it is.
pub fn caption_read(text: &str) -> Option<(String, String)> {
    const KINDS: [&str; 5] = ["title", "header", "footer", "caption", "legend"];
    let (kind, body) = KINDS.iter().find_map(|kind| {
        let rest = text.strip_prefix(*kind)?;
        rest.strip_prefix([' ', '\t']).map(strip).map(|body| (*kind, body))
    })?;
    if !caption_plain(body) {
        return None;
    }
    Some((kind.to_string(), body.to_string()))
}

/// `Caption.plain?`: one plain line. Ruby `/\A[[:alnum:]][[:alnum:]
/// .,:;!?()'-]*\z/` with no `--`, `..`, `->` or `<-` run in it.
pub fn caption_plain(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_alphanumeric() => {}
        _ => return false,
    }
    if text.chars().any(|c| !(c.is_alphanumeric() || " .,:;!?()'-".contains(c))) {
        return false;
    }
    let head_end = text.find(char::is_whitespace).unwrap_or(text.len());
    let head = &text[..head_end];
    let bytes = head.as_bytes();
    for i in 0..bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with(b"--")
            || rest.starts_with(b"..")
            || rest.starts_with(b"->")
            || rest.starts_with(b"<-")
        {
            return false;
        }
    }
    true
}

/// `Paint.read`: the colour and thickness skinparams, with the role each
/// sets. None for a key or value not read.
pub fn paint_read(key: &str, value: &str) -> Option<Appearance> {
    let lowered = key.to_lowercase();
    let name = lowered
        .strip_prefix("sequence")
        .filter(|rest| rest.starts_with("participant") || rest.starts_with("arrow"))
        .map(str::to_string)
        .unwrap_or_else(|| lowered.clone());
    if name.ends_with("arrowthickness") {
        // /\A(?:\d+|\d*\.\d+)\z/
        let ok = !value.is_empty()
            && value.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            && (value.contains('.') || value.bytes().all(|b| b.is_ascii_digit()));
        return if ok {
            Some(Appearance {
                arrow_width: Some(value.to_string()),
                ..Appearance::default()
            })
        } else {
            None
        };
    }
    let colour = fill_read(Some(if value.starts_with('#') {
        value
    } else {
        Box::leak(format!("#{value}").into_boxed_str())
    }))?;
    const PAINTED: &[(&str, &str)] = &[
        ("participantbackgroundcolor", "head_fill"),
        ("participantbordercolor", "head_line"),
        ("arrowcolor", "arrow"),
        ("sequencelifelinebordercolor", "lifeline"),
        ("sequenceboxbackgroundcolor", "box_fill"),
        ("sequenceboxbordercolor", "box_line"),
        ("sequencedividerbackgroundcolor", "divider_fill"),
        ("sequencedividerbordercolor", "divider_line"),
        ("sequencegroupbordercolor", "group_line"),
        ("notebackgroundcolor", "note_fill"),
        ("notebordercolor", "note_line"),
        ("backgroundcolor", "background"),
        ("arrowfontcolor", "text_message_label"),
        ("notefontcolor", "text_note"),
        ("sequenceboxfontcolor", "text_box"),
        ("sequencedividerfontcolor", "text_divider"),
    ];
    let role = |name: &str| -> Option<&'static str> {
        PAINTED.iter().find(|(key, _)| *key == name).map(|(_, role)| *role)
    };
    if let Some(role) = role(&name) {
        let mut appearance = Appearance::default();
        appearance.paint.insert(role, colour.colour.clone());
        return Some(appearance);
    }
    match lowered.as_str() {
        "sequencegroupbackgroundcolor" => {
            Some(Appearance { tab_fill: Some(colour.colour), ..Default::default() })
        }
        "sequencegroupheaderfontcolor" => {
            Some(Appearance { tab_colour: Some(colour.colour), ..Default::default() })
        }
        "participantfontcolor" => Some(Appearance {
            head_style: HeadStyle { colour: Some(colour.colour), ..Default::default() },
            ..Default::default()
        }),
        _ => None,
    }
}

/// `Style.read`: one `<style>` block. None when it sets anything the
/// reader does not list.
pub fn style_read(text: &str) -> Option<Appearance> {
    // Ruby `gsub(COMMENT, " ")`: each comment becomes one space.
    let mut plain = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        plain.push_str(&rest[..start]);
        plain.push(' ');
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    plain.push_str(rest);
    // Ruby `gsub(/(\w):(?=\s)/, "\\1")`: a word's trailing colon goes.
    let chars: Vec<char> = plain.chars().collect();
    let mut trimmed = String::with_capacity(plain.len());
    for (index, ch) in chars.iter().enumerate() {
        if *ch == ':'
            && index > 0
            && chars[index - 1].is_alphanumeric()
            && chars.get(index + 1).is_some_and(|c| c.is_whitespace())
        {
            continue;
        }
        trimmed.push(*ch);
    }
    // Ruby `gsub(/;(?=\s|\z)/, "")`.
    let chars: Vec<char> = trimmed.chars().collect();
    let mut clean = String::with_capacity(trimmed.len());
    for (index, ch) in chars.iter().enumerate() {
        if *ch == ';' && chars.get(index + 1).is_none_or(|c| c.is_whitespace()) {
            continue;
        }
        clean.push(*ch);
    }

    // TOKEN: `{`, `}` or a run of non-space non-brace characters.
    let mut tokens: Vec<String> = vec![];
    let mut current = String::new();
    for ch in clean.chars() {
        match ch {
            '{' | '}' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // The token walk; a refused block is None.
    let mut path: Vec<String> = vec![];
    let mut values: Vec<(String, String)> = vec![];
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].clone();
        if token == "}" {
            if path.pop().is_none() {
                return None;
            }
        } else if tokens.get(index + 1).map(String::as_str) == Some("{") {
            path.push(token.to_lowercase());
            index += 1;
            if !known_path(&path) {
                return None;
            }
        } else {
            let value = tokens.get(index + 1)?;
            assign(&mut values, &path, &token, value)?;
            index += 1;
        }
        index += 1;
    }
    if !path.is_empty() {
        return None;
    }

    // appearance()
    let mut appearance = Appearance::default();
    for (key, value) in &values {
        match key.as_str() {
            "min_width" => appearance.min_width = value.parse().ok(),
            "alignment" => {
                appearance.alignment = match value.as_str() {
                    "left" => Some("left"),
                    "center" => Some("center"),
                    "right" => Some("right"),
                    _ => return None,
                };
            }
            "tab_fill" => appearance.tab_fill = Some(value.clone()),
            "tab_colour" => appearance.tab_colour = Some(value.clone()),
            "tab_size" => appearance.tab_size = value.parse().ok(),
            "head_colour" => appearance.head_style.colour = Some(value.clone()),
            "head_size" => appearance.head_style.size = value.parse().ok(),
            "head_style" => appearance.head_style.style = Some(value.clone()),
            "head_family" => appearance.head_style.family = Some(value.clone()),
            "head_weight" => appearance.head_style.weight = Some(value.clone()),
            "head_line" => appearance.head_style.line = Some(value.clone()),
            _ => return None,
        }
    }
    Some(appearance)
}

fn known_path(path: &[String]) -> bool {
    let flat: Vec<&str> = path.iter().map(String::as_str).collect();
    matches!(
        flat.as_slice(),
        ["sequencediagram"]
            | ["sequencediagram", "participant"]
            | ["sequencediagram", "groupheader"]
            | ["sequencediagram", "group"]
            | ["sequencediagram", "group", "header"]
            | ["sequencediagram", "reference"]
            | ["sequencediagram", "reference", "header"]
    )
}

/// The PROPERTIES / NESTED tables: (path, name downcased) -> key.
fn assign(
    values: &mut Vec<(String, String)>,
    path: &[String],
    name: &str,
    value: &str,
) -> Option<()> {
    let flat: Vec<&str> = path.iter().map(String::as_str).collect();
    let key: Option<String> = match flat.as_slice() {
        ["sequencediagram", "participant"] => match name.to_lowercase().as_str() {
            "minimumwidth" => Some("min_width".into()),
            "horizontalalignment" => Some("alignment".into()),
            "fontcolor" => Some("head_colour".into()),
            "fontsize" => Some("head_size".into()),
            "fontstyle" => Some("head_style".into()),
            "fontname" => Some("head_family".into()),
            "fontweight" => Some("head_weight".into()),
            "linecolor" => Some("head_line".into()),
            _ => return None,
        },
        ["sequencediagram", "groupheader"] => match name.to_lowercase().as_str() {
            "fontcolor" => Some("tab_colour".into()),
            "backgroundcolor" => Some("tab_fill".into()),
            "fontsize" => Some("tab_size".into()),
            _ => return None,
        },
        // The nested forms are read and dropped.
        ["sequencediagram", "group", "header"]
        | ["sequencediagram", "reference", "header"] => {
            return if matches!(
                name.to_lowercase().as_str(),
                "fontcolor" | "backgroundcolor"
            ) {
                Some(())
            } else {
                None
            };
        }
        _ => return None,
    };
    let key = key?;
    let converted = convert(&key, value)?;
    if let Some(slot) = values.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = converted;
    } else {
        values.push((key, converted));
    }
    Some(())
}

fn convert(key: &str, value: &str) -> Option<String> {
    if matches!(key, "min_width" | "tab_size" | "head_size") {
        // Integer(value, 10) and positive.
        let n: i64 = value.parse().ok()?;
        if n <= 0 {
            return None;
        }
        return Some(n.to_string());
    }
    if key == "alignment" {
        let lowered = value.to_lowercase();
        return if matches!(lowered.as_str(), "left" | "center" | "right") {
            Some(lowered)
        } else {
            None
        };
    }
    match key {
        "head_style" => {
            if value.eq_ignore_ascii_case("italic") {
                Some("italic".into())
            } else {
                None
            }
        }
        "head_family" => {
            let ok = !value.is_empty()
                && value
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
            if ok { Some(value.to_string()) } else { None }
        }
        "head_weight" => {
            let lowered = value.to_lowercase();
            let ok = matches!(lowered.as_str(), "bold" | "normal")
                || (lowered.len() == 3
                    && lowered.ends_with("00")
                    && lowered.starts_with(|c: char| ('1'..='9').contains(&c)));
            if ok { Some(lowered) } else { None }
        }
        _ => style_colour(value),
    }
}
