//! Reads a PlantUML mindmap's significant lines into the hierarchy the
//! layout places. Port of `lib/etassia/notation/plantuml/tree/reader.rb`
//! (the mindmap half; wbs shares the reader and carries no lane yet).

use super::rules;
use crate::Error;
use crate::render::pie::{strip, text_of};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// A line of the source, as read: the node id the reader gave it, its
/// depth, the side of its parent it hangs on, whether it draws no box,
/// the fill a `[#colour]` or a style class gave it, its text lines, and
/// its parent's index.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub depth: usize,
    pub side: &'static str,
    pub boxless: bool,
    pub fill: Option<String>,
    pub lines: Vec<String>,
    pub parent: Option<usize>,
}

/// What the reader produces: the entries in source order and what the
/// source's `<style>` blocks set.
pub struct Read {
    pub entries: Vec<Entry>,
    pub rules: rules::Rules,
}

/// A front mark carried in front of a `<code>` block's lines, which the
/// layout reads back as "draw in the monospace family".
pub const MONO_MARK: char = '\u{E000}';

/// The body lines of a PARG `mindmap_diagram` tree, in source order,
/// trimmed the way the Ruby reader's source trims them.
pub fn entry_lines(tree: &Value) -> Vec<String> {
    let mut out = vec![];
    walk(tree, &mut out);
    out
}

fn walk(tree: &Value, out: &mut Vec<String>) {
    match tree {
        Value::Array(items) => {
            for item in items {
                walk(item, out);
            }
        }
        Value::Object(object) => {
            // A tagged capture is an object with a `line` key holding a
            // slice (or a string); a slice itself carries a `line` field
            // of its own, which is a line NUMBER, not a capture.
            let capture = object
                .get("line")
                .map(|line| line.is_string() || crate::render::pie::is_slice(line))
                .unwrap_or(false);
            if capture {
                let text = strip(&text_of(&object["line"])).to_string();
                if !text.is_empty() {
                    out.push(text);
                }
            } else {
                for value in object.values() {
                    walk(value, out);
                }
            }
        }
        _ => {}
    }
}

/// Reads the trimmed lines into the hierarchy. A line outside the reader's
/// subset raises, the way the Ruby reader's UnsupportedConstructError does.
pub fn read(lines: Vec<String>) -> Result<Read, Error> {
    let mut reader = Reader { entries: vec![], rules: rules::Rules::default(), classes: Default::default(), open: None, code: false, style: None };
    for text in lines {
        reader.read(&text)?;
    }
    Ok(Read { entries: reader.entries, rules: reader.rules })
}

struct Reader {
    entries: Vec<Entry>,
    rules: rules::Rules,
    classes: std::collections::HashMap<String, String>,
    open: Option<usize>,
    code: bool,
    style: Option<Vec<String>>,
}

fn item_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"\A([*+-]+)([<>])?(_)?(?:\[(\#?(?-u:\w+))\])?(:|[ \t])[ \t]*(.*)\z")
            .expect("item pattern")
    })
}

fn skipped_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(
            r"(?i)\A(?:title|header|footer|caption|legend|skinparam|scale|!theme|hide|show|left to right direction|top to bottom direction)(?-u:\b)",
        )
        .expect("skipped pattern")
    })
}

fn style_open_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)\A<style>").expect("style open pattern"))
}

fn style_close_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)</style>\z").expect("style close pattern"))
}

fn class_style_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"(?s)\.((?-u:\w+))[ \t]*\{([^{}]*)\}").expect("class style pattern")
    })
}

fn background_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"(?i)BackgroundColor[ \t]+(\#?(?-u:\w+))").expect("background pattern")
    })
}

fn trailing_class_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"[ \t]*<<((?-u:\w+))>>\z").expect("trailing class pattern"))
}

fn code_tag_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)\A</?code>\z").expect("code tag pattern"))
}

impl Reader {
    fn read(&mut self, text: &str) -> Result<(), Error> {
        if self.open.is_some() {
            return Ok(self.continue_text(text));
        }
        if self.style.is_some() {
            return Ok(self.style_line(text, false));
        }
        if skipped_re().is_match(text) {
            return Ok(());
        }
        if style_open_re().is_match(text) {
            let rest = style_open_re().replace(text, "").into_owned();
            return Ok(self.style_line(&rest, true));
        }
        match item_re().captures(text) {
            Some(captures) => Ok(self.start_item(&captures)),
            None => Err(Error::Parse {
                class: "UnsupportedConstructError".into(),
                message: format!("PlantUML mindmap statement is not yet supported: {text:?}"),
                line: 0,
                col: 0,
            }),
        }
    }

    fn style_line(&mut self, text: &str, opening: bool) {
        if opening {
            self.style = Some(vec![]);
        }
        let closed = style_close_re().is_match(text);
        let stripped = style_close_re().replace(text, "").into_owned();
        if let Some(style) = self.style.as_mut() {
            style.push(stripped);
        }
        if closed {
            self.close_style();
        }
    }

    fn close_style(&mut self) {
        let style = self.style.take().unwrap_or_default();
        let text = style.join("\n");
        self.rules.add(&text);
        let sheet = strip_comments(&text);
        for captures in class_style_re().captures_iter(&sheet) {
            let name = captures[1].to_string();
            if let Some(colour) = background_re().captures(&captures[2]) {
                self.classes.insert(name, colour[1].to_string());
            }
        }
    }

    fn start_item(&mut self, captures: &regex::Captures) {
        let marks = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let hang = captures.get(2).map(|m| m.as_str().to_string());
        let boxless = captures.get(3).is_some();
        let fill = captures.get(4).map(|m| m.as_str().to_string());
        let joint = captures.get(5).map(|m| m.as_str().to_string()).unwrap_or_default();
        let rest = captures.get(6).map(|m| m.as_str().to_string()).unwrap_or_default();
        let depth = marks.chars().count();
        let id = format!("n{}", self.entries.len() + 1);
        let entry = Entry { id, depth, side: "right", boxless, fill, lines: vec![], parent: None };
        self.attach(entry, marks.chars().next(), hang);
        let index = self.entries.len() - 1;
        self.settle(index, &joint, &rest);
    }

    fn settle(&mut self, index: usize, joint: &str, rest: &str) {
        if joint == ":" && !rest.ends_with(';') {
            if let Some(entry) = self.entries.get_mut(index) {
                entry.lines.push(rest.to_string());
            }
            self.open = Some(index);
            return;
        }
        let last = if joint == ":" { rest.strip_suffix(';').unwrap_or(rest) } else { rest };
        self.finish(index, last, false);
    }

    fn continue_text(&mut self, text: &str) {
        if code_tag_re().is_match(text) {
            self.code = !text.starts_with("</");
            return;
        }
        let index = match self.open {
            Some(index) => index,
            None => return,
        };
        if !text.ends_with(';') {
            let marked = self.marked(text);
            if let Some(entry) = self.entries.get_mut(index) {
                entry.lines.push(marked);
            }
            return;
        }
        let last = if text == ";" { String::new() } else { self.marked(text.strip_suffix(';').unwrap_or(text)) };
        self.code = false;
        self.finish(index, &last, true);
    }

    fn marked(&self, text: &str) -> String {
        if self.code {
            format!("{MONO_MARK}{text}")
        } else {
            text.to_string()
        }
    }

    fn finish(&mut self, index: usize, last: &str, continued: bool) {
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        if continued && entry.lines.last().is_some_and(|line| line.is_empty()) {
            entry.lines.pop();
        }
        entry.lines.push(last.to_string());
        if let Some(last_line) = entry.lines.last_mut() {
            let text = std::mem::take(last_line);
            *last_line = classed_line(&self.classes, &mut entry.fill, &text);
        }
        self.open = None;
    }

    fn attach(&mut self, mut entry: Entry, mark: Option<char>, hang: Option<String>) {
        entry.parent = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, other)| other.depth < entry.depth)
            .map(|(index, _)| index);
        let parent_side = entry.parent.and_then(|index| self.entries.get(index)).map(|other| other.side);
        entry.side = side_of(parent_side, mark, hang.as_deref());
        self.entries.push(entry);
    }
}

/// `classed`: a trailing `<<name>>` fills the box from the style class and
/// leaves the text.
fn classed_line(classes: &std::collections::HashMap<String, String>, fill: &mut Option<String>, text: &str) -> String {
    let Some(captures) = trailing_class_re().captures(text) else {
        return text.to_string();
    };
    if fill.is_none() {
        if let Some(colour) = classes.get(&captures[1]) {
            *fill = Some(colour.clone());
        }
    }
    trailing_class_re().replace(text, "").into_owned()
}

fn side_of(parent_side: Option<&'static str>, mark: Option<char>, hang: Option<&str>) -> &'static str {
    if mark == Some('-') || hang == Some("<") {
        return "left";
    }
    if mark == Some('+') || hang == Some(">") {
        return "right";
    }
    parent_side.unwrap_or("right")
}

fn strip_comments(text: &str) -> String {
    static COMMENT: OnceLock<Regex> = OnceLock::new();
    let re = COMMENT.get_or_init(|| Regex::new(r"//[^\n]*").expect("comment pattern"));
    re.replace_all(text, "").into_owned()
}
