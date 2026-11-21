//! Port of Ruby `Notation::PlantUML::{StyleReader, StyleSheet}`: the lines
//! between `<style>` and `</style>`, read one at a time. Only
//! `document { <kind> { ... } }` with BackGroundColor, FontColor and
//! FontSize is read.
use super::model::{CaptionKind, RuleProps, StyleSheet};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Ruby `StyleReader::ELEMENTS`: element rules belong to Style::Reader;
/// this reader steps over them.
const ELEMENTS: [&str; 9] = [
    "classdiagram",
    "statediagram",
    "activitydiagram",
    "class",
    "state",
    "activity",
    "note",
    "arrow",
    "root",
];

const CAPTION_KINDS: [(&str, CaptionKind); 5] = [
    ("title", CaptionKind::Title),
    ("header", CaptionKind::Header),
    ("footer", CaptionKind::Footer),
    ("caption", CaptionKind::Caption),
    ("legend", CaptionKind::Legend),
];

/// The sheet a `<style>` block built, carried into the Diagram.
#[derive(Clone, Debug, Default)]
pub struct SheetFromBlock {
    pub background: Option<String>,
    pub rules: HashMap<CaptionKind, RuleProps>,
}

impl From<SheetFromBlock> for StyleSheet {
    fn from(sheet: SheetFromBlock) -> StyleSheet {
        StyleSheet { background: sheet.background, rules: sheet.rules }
    }
}

pub struct BlockReader {
    path: Vec<String>,
    skipped: usize,
    background: Option<String>,
    rules: HashMap<CaptionKind, RuleProps>,
}

impl BlockReader {
    pub fn new() -> BlockReader {
        BlockReader {
            path: vec![],
            skipped: 0,
            background: None,
            rules: HashMap::new(),
        }
    }

    /// Ruby `feed`: `Ok(())` keeps the block open, `Err(())` closed it.
    pub fn feed(&mut self, text: &str) -> Result<(), ()> {
        if is_closing_tag(text) {
            if !self.path.is_empty() || self.skipped != 0 {
                return Err(()); // refused: a selector still open
            }
            return Err(());
        }
        self.read_rule(text);
        Ok(())
    }

    pub fn sheet(self) -> SheetFromBlock {
        SheetFromBlock { background: self.background, rules: self.rules }
    }

    fn read_rule(&mut self, text: &str) {
        if self.skipping(text) {
            self.skip_element_rule(text);
            return;
        }
        static OPENING: OnceLock<regex::Regex> = OnceLock::new();
        let opening = OPENING
            .get_or_init(|| regex::Regex::new(r"\A(\.?\w+)[ \t]*\{\z").unwrap());
        if let Some(m) = opening.captures(text) {
            let name = m.get(1).map(|g| g.as_str()).unwrap_or("");
            self.open_selector(name, text);
        } else if text == "}" && !self.path.is_empty() {
            self.path.pop();
        } else {
            self.read_property(text);
        }
    }

    fn read_property(&mut self, text: &str) {
        static PROPERTY: OnceLock<regex::Regex> = OnceLock::new();
        let property = PROPERTY
            .get_or_init(|| regex::Regex::new(r"\A(\w+)[ \t]+(\S+)\z").unwrap());
        let Some(m) = property.captures(text) else {
            // refused: style rule form
            return;
        };
        let name = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
        let value = m.get(2).map(|g| g.as_str()).unwrap_or("");
        self.set(&name, value);
    }

    fn skipping(&self, text: &str) -> bool {
        self.skipped > 0 || self.element_rule(text)
    }

    fn element_rule(&self, text: &str) -> bool {
        static OPENING: OnceLock<regex::Regex> = OnceLock::new();
        let opening = OPENING
            .get_or_init(|| regex::Regex::new(r"\A(\.?\w+)[ \t]*\{\z").unwrap());
        let Some(m) = opening.captures(text) else { return false };
        if !self.path.is_empty() {
            return false;
        }
        let name = m.get(1).map(|g| g.as_str()).unwrap_or("");
        name.starts_with('.') || ELEMENTS.contains(&name.to_lowercase().as_str())
    }

    fn skip_element_rule(&mut self, text: &str) {
        static OPENING: OnceLock<regex::Regex> = OnceLock::new();
        let opening = OPENING
            .get_or_init(|| regex::Regex::new(r"\A(\.?\w+)[ \t]*\{\z").unwrap());
        if opening.is_match(text) || text.ends_with('{') {
            self.skipped += 1;
        } else if text == "}" {
            self.skipped -= 1;
        }
    }

    fn open_selector(&mut self, name: &str, _text: &str) {
        let word = name.to_lowercase();
        let allowed: Vec<&str> = if self.path.is_empty() {
            vec!["document"]
        } else if self.path == ["document"] {
            CAPTION_KINDS.iter().map(|(name, _)| *name).collect()
        } else {
            vec![]
        };
        if !allowed.contains(&word.as_str()) {
            return; // refused: style selector
        }
        self.path.push(word);
    }

    fn set(&mut self, property: &str, value: &str) {
        let name = match property {
            "backgroundcolor" => Some("background"),
            "fontcolor" => Some("colour"),
            "fontsize" => Some("size"),
            _ => None,
        };
        // A document sets only its background; a caption kind sets any.
        let scope = self.path.len();
        let settable = match name {
            Some(_) => scope == 2 || (scope == 1 && property == "backgroundcolor"),
            None => false,
        };
        if !settable {
            return; // refused
        }
        let Some(name) = name else { return };
        if name == "size" {
            let Some(parsed) = self.parse(name, value) else { return };
            let Some(parsed) = parsed.parse::<f64>().ok() else { return };
            let Some(kind) = caption_kind(self.path.last().map(String::as_str)) else { return };
            self.rules.entry(kind).or_default().size = Some(parsed);
            return;
        }
        let Some(parsed) = self.parse(name, value) else { return };
        if scope == 1 {
            self.background = Some(parsed);
        } else {
            let Some(kind) = caption_kind(self.path.last().map(String::as_str)) else { return };
            let rule = self.rules.entry(kind).or_default();
            match name {
                "background" => rule.background = Some(parsed),
                "colour" => rule.colour = Some(parsed),
                _ => {}
            }
        }
    }

    fn parse(&self, name: &str, value: &str) -> Option<String> {
        if name == "size" {
            // Ruby: value.to_i when /\A[1-9]\d{0,2}\z/, else refused.
            let ok = (1..=3).contains(&value.len())
                && value.starts_with(|c: char| ('1'..='9').contains(&c))
                && value.chars().all(|c| c.is_ascii_digit());
            if ok {
                return Some(value.to_string());
            }
            return None;
        }
        super::reader_support::package_color(value)
    }
}

fn caption_kind(word: Option<&str>) -> Option<CaptionKind> {
    let word = word?;
    CAPTION_KINDS.iter().find(|(name, _)| *name == word).map(|(_, kind)| *kind)
}

fn is_closing_tag(text: &str) -> bool {
    static CLOSE: OnceLock<regex::Regex> = OnceLock::new();
    CLOSE.get_or_init(|| regex::Regex::new(r"(?i)\A</style>\z").unwrap())
        .is_match(text)
}
