//! The reader: turns the parsed source into the Diagram. Port of
//! `sequence/parser.rb` (its regex readers and the Outline that collects
//! the items). The PARG grammar has already accepted the whole source;
//! this walks the stripped lines the way the Ruby parser does.

use super::model::*;
use super::style::{caption_plain, caption_read, fill_read, note_fill_read, paint_read, style_read};
use crate::Error;
use regex::Regex;
use std::collections::HashMap;

fn re(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("sequence reader pattern {pattern:?}: {error}"),
    }
}

fn cell(init: fn() -> Regex, name: &'static str) -> Regex {
    thread_local! {
        static CELL: std::cell::RefCell<Option<HashMap<&'static str, Regex>>> =
            const { std::cell::RefCell::new(None) };
    }
    CELL.with(|slot| {
        let mut map = slot.borrow_mut();
        let map = map.get_or_insert_with(HashMap::new);
        if let Some(existing) = map.get(name) {
            return existing.clone();
        }
        let built = init();
        map.insert(name, built.clone());
        built
    })
}

macro_rules! pattern {
    ($name:ident, $expr:expr) => {
        fn $name() -> Regex {
            fn init() -> Regex {
                re($expr)
            }
            cell(init, stringify!($name))
        }
    };
}

const KINDS: &str = r"(?i:participant|actor|boundary|control|entity|database|collections|queue)";
const QUOTED: &str = r#""[^"\n]+""#;
const NAME: &str = r"[A-Za-z_][A-Za-z0-9_]*";
const TARGET: &str = "(?:(?:\"[^\"\\n]+\")|(?:[A-Za-z_][A-Za-z0-9_]*))";
/// COLOUR: the token after a message arrow or an activation.
const COLOUR: &str = "(?i:(?:[0-9A-Fa-f]{6})|(?:[0-9A-Fa-f]{3})|red|green|blue|yellow|orange|purple|gray|grey|cyan|magenta|lime|navy|teal|olive|maroon|silver|aqua|fuchsia|pink|brown|black|white)";
const WORD: &str = "[0-9A-Za-z_]";

pattern!(declaration, &format!(
    r"(?i)\A({KINDS})[ \t]+({QUOTED}|{NAME})(?:[ \t]+as[ \t]+({NAME}))?(?:[ \t]+<<[ \t]*([^<>\n]+?)[ \t]*>>)?(?:[ \t]+order[ \t]+(-?\d+))?(?:[ \t]+(\#{WORD}+))?\z"));
pattern!(box_re, &format!(r"(?i)\Abox(?:[ \t]+({QUOTED}))?(?:[ \t]+(\#{WORD}+))?\z"));
pattern!(end_box, r"\A(?:endbox|end[ \t]+box)\z");
pattern!(skinparam_width, r"(?i)\Askinparam[ \t]+[ \t]*MinClassWidth[ \t]+(\d+)\z");
pattern!(skinparam_max_message, r"(?i)\Askinparam[ \t]+[ \t]*maxmessagesize[ \t]+(\d+)\z");
pattern!(skinparam_paint, r"(?i)\Askinparam[ \t]+(\w+)[ \t]+(\S+)\z");
pattern!(skinparam_open, r"(?i)\Askinparam[ \t]*\{\z");
pattern!(skinparam_line, r"(?i)\Amaxmessagesize[ \t]+(\d+)\z");
pattern!(skinparam_paint_line, r"\A(\w+)[ \t]+(\S+)\z");
pattern!(hide_footbox, r"(?i)\Ahide[ \t]+footbox\z");
pattern!(autoactivate, r"(?i)\Aautoactivate[ \t]+(on|off)\z");
pattern!(chrome_open, r"(?i)\A(?:(title|header|footer|caption)|legend(?:[ \t]+(top|bottom))?(?:[ \t]+(left|center|right))?)\z");
pattern!(chrome_close, r"(?i)\Aend[ \t]*(title|header|footer|caption|legend)\z");
pattern!(style_open, r"(?i)\A<style>\z");
pattern!(style_close, r"(?i)\A</style>\z");
pattern!(note, &format!(
    r"(?i)\A(note|hnote|rnote)[ \t]+(left|right|over|across|top|bottom)(?:[ \t]+of)?(?:[ \t]+({TARGET}(?:[ \t]*,[ \t]*{TARGET})*))?(?:[ \t]+(\#{WORD}+))?[ \t]*(?::[ \t]*(.*))?\z"));
pattern!(ref_re, &format!(
    r"(?i)\Aref[ \t]+over[ \t]+({TARGET}(?:[ \t]*,[ \t]*{TARGET})*)(?:[ \t]*:[ \t]*(\S.*))\z"));
pattern!(end_note, r"(?i)\Aend[ \t]*[hr]?note\z");
pattern!(block, &format!(
    r"(?i)\A(alt|opt|loop|par|critical|break|group)(?:[ \t]+(?:\#{WORD}+[ \t]*)?(.*))?\z"));
pattern!(branch, r"(?i)\Aelse(?:[ \t]+(.*))?\z");
pattern!(reply, r"(?i)\Areturn(?:[ \t]+(.*))?\z");
pattern!(divider, r"\A==[ \t]*(.*?)[ \t]*==\z");
pattern!(spacing, r"\A\|\|(?:(\d+)\|\||\|\|?)\z");
pattern!(delay, r"\A\.\.\.(?:[ \t]*(\S.*?)[ \t]*\.\.\.)?\z");
pattern!(destroy, &format!(r"(?i)\Adestroy[ \t]+({TARGET})\z"));
pattern!(create, r"(?i)\Acreate[ \t]+(.+)\z");
pattern!(pragma, r"(?i)\A!pragma[ \t]+(teoz|svginteractive)[ \t]+true\z");
pattern!(activation, &format!(
    r"(?i)\A(activate|deactivate)[ \t]+({QUOTED}|{NAME})(?:[ \t]+(\#({COLOUR}))([^0-9A-Za-z_]|$))?\z"));
pattern!(startuml, r"\A@startuml(?:[^0-9A-Za-z_]|$)");
pattern!(title_gate, r"(?i)\A(?:title|header|footer|caption|legend)[ \t]+\S");
pattern!(target_only, &format!(r"\A(?:{QUOTED}|{NAME})\z"));
pattern!(arrow_token, r"\A([oOxX])?(<<|<|//|/|\\\\|\\)?(--?)(>>|>|\\\\|\\|//|/)?([oOxX])?\z");
pattern!(bracket_colour, r"(?i)(\#((?:[0-9A-Fa-f]{6})|(?:[0-9A-Fa-f]{3})|[a-z]+))");
pattern!(bracket_hidden, r"(?i)\bhidden\b");
pattern!(bracket_dashed, r"(?i)\b(?:dashed|dotted)\b");
pattern!(autonumber_line, r#"(?i)\Aautonumber(?:[ \t]+(\d+)(?:[ \t]+(\d+))?)?(?:[ \t]+"([^"]*)")?\z"#);
pattern!(autonumber_pause, r"(?i)\Aautonumber[ \t]+(stop|resume)\z");
pattern!(newpage_line, r"(?i)\Anewpage\z");

/// `ArrowSyntax.read`: the style and whether the arrow is written
/// pointing at the sender's name.
fn arrow_syntax(token: &str) -> Option<(ArrowStyle, bool)> {
    // plain(token): the first [...] run goes.
    let (plain, extras) = match (token.find('['), token.find(']')) {
        (Some(open), Some(close)) if open < close => (
            format!("{}{}", &token[..open], &token[close + 1..]),
            token[open + 1..close].to_string(),
        ),
        _ => (token.to_string(), String::new()),
    };
    let caps = arrow_token().captures(&plain)?;
    let lead = caps.get(1).map(|m| m.as_str().to_lowercase());
    let left = caps.get(2).map(|m| m.as_str());
    let shaft = caps.get(3).map(|m| m.as_str()).unwrap_or("");
    let right = caps.get(4).map(|m| m.as_str());
    let trail = caps.get(5).map(|m| m.as_str().to_lowercase());

    const LEFT_MAP: &[(&str, &str)] = &[
        ("<<", "open"), ("<", "filled"), ("//", "upper_open"), ("/", "upper"),
        ("\\\\", "lower_open"), ("\\", "lower"),
    ];
    const RIGHT_MAP: &[(&str, &str)] = &[
        (">>", "open"), (">", "filled"), ("\\\\", "upper_open"), ("\\", "upper"),
        ("//", "lower_open"), ("/", "lower"),
    ];
    let glyph_of = |raw: Option<&str>, table: &'static [(&'static str, &'static str)]| -> Option<&'static str> {
        raw.and_then(|m| table.iter().find(|(key, _)| *key == m))
            .map(|(_, glyph)| *glyph)
    };
    let left_glyph = glyph_of(left, LEFT_MAP);
    let right_glyph = glyph_of(right, RIGHT_MAP);
    // fits?: at least one head; heads only pair as < >; an x replaces a
    // plain head only.
    let paired = !matches!((left.is_some(), right.is_some()), (true, true))
        || (left == Some("<") && right == Some(">"));
    let crossable = |mark: &Option<String>, raw: Option<&str>, plain_head: &str| {
        mark.as_deref() != Some("x") || raw.is_none() || raw == Some(plain_head)
    };
    if !paired
        || !crossable(&lead, left, "<")
        || !crossable(&trail, right, ">")
    {
        return None;
    }
    // colour?: a colour in the brackets must be one PlantUML draws.
    let written = bracket_colour()
        .captures(&extras)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string());
    if let Some(written) = &written {
        if fill_read(Some(written)).is_none() {
            return None;
        }
    }
    let colour = written.as_deref().and_then(|w| fill_read(Some(w))).map(|fill| fill.colour);
    let hidden = bracket_hidden().is_match(&extras);
    let dashed = shaft.len() == 2 || bracket_dashed().is_match(&extras);

    // ends(): an x lead/trail becomes the cross glyph; o the ring.
    let end_of = |mark: &Option<String>, glyph: Option<&'static str>| ArrowEnd {
        glyph: if mark.as_deref() == Some("x") {
            Some("cross")
        } else {
            glyph
        },
        circle: mark.as_deref() == Some("o"),
    };
    let left_end = end_of(&lead, left_glyph);
    let right_end = end_of(&trail, right_glyph);
    let reversed = left.is_some() && right.is_none();
    let (head, tail) = if reversed {
        (left_end, right_end)
    } else {
        (right_end, left_end)
    };
    Some((
        ArrowStyle { head, tail, dashed, leftward: reversed, hidden, colour },
        reversed,
    ))
}

/// `Edge.read`.
fn edge_read(token: &str, side: Side) -> Option<Edge> {
    match token {
        "[" => Some(Edge { side, local: false }),
        "]" => Some(Edge { side, local: false }),
        "?" => Some(Edge { side, local: true }),
        _ => None,
    }
}

/// The phases a message suffix carries.
fn marks_of_suffix(suffix: &str) -> Vec<Phase> {
    match suffix {
        "++" => vec![Phase::On],
        "--" => vec![Phase::Off],
        "--++" => vec![Phase::Off, Phase::On],
        "++--" => vec![Phase::On, Phase::Off],
        _ => vec![],
    }
}

enum MarkKind {
    Activation,
    Destroy,
}

/// The Outline: the items in source order, with the history queries.
#[derive(Default)]
struct Outline {
    items: Vec<Item>,
    calls: Vec<Message>,
    blocks: Vec<String>,
    depth: HashMap<String, i64>,
    numbering: Option<Numbering>,
    number: i64,
    counting: bool,
}

type Marks = Vec<(Phase, String, Option<String>, MarkKind)>;

impl Outline {
    fn cut_at_page_break(items: &[Item]) -> Vec<Item> {
        let cut = items
            .iter()
            .position(|item| matches!(item, Item::PageBreak(_)));
        match cut {
            Some(cut) => items[..=cut].to_vec(),
            None => items.to_vec(),
        }
    }

    fn message(&mut self, message: Message, marks: Marks) -> bool {
        let Some(message) = self.numbered(message) else {
            return false;
        };
        if !message.dashed() {
            self.calls.push(message.clone());
        }
        self.items.push(Item::Message(message));
        for (phase, id, color, kind) in marks {
            match kind {
                MarkKind::Destroy => self.destroy(id),
                MarkKind::Activation => {
                    self.activation(phase, id, color);
                }
            }
        }
        true
    }

    fn autonumber(&mut self, numbering: Numbering) {
        self.numbering = Some(numbering.clone());
        self.number = numbering.start;
        self.counting = true;
    }

    fn counting(&mut self, on: bool) -> bool {
        if self.numbering.is_none() {
            return false;
        }
        self.counting = on;
        true
    }

    fn activation(&mut self, phase: Phase, id: String, color: Option<String>) -> bool {
        if !self.allowed(phase, &id) {
            return true;
        }
        *self.depth.entry(id.clone()).or_insert(0) += if phase == Phase::On { 1 } else { -1 };
        self.items.push(Item::Activation(Activation { participant: id, phase, color }));
        true
    }

    fn create(&mut self, id: String) -> bool {
        let appears = self.items.iter().any(|item| match item {
            Item::Message(message) => {
                message.from.id() == id || message.to.id() == id
            }
            _ => false,
        });
        if appears {
            return false;
        }
        self.items.push(Item::Create(Create { participant: id }));
        true
    }

    fn destroy(&mut self, id: String) {
        self.items.push(Item::Destroy(Destroy { participant: id }));
    }

    fn active(&self, id: &str) -> bool {
        self.depth.get(id).copied().unwrap_or(0) > 0
    }

    fn allowed(&self, phase: Phase, id: &str) -> bool {
        phase != Phase::Off || self.depth.get(id).copied().unwrap_or(0) > 0
    }

    fn reply(&mut self, label: Option<String>) -> bool {
        let Some(call) = self.calls.pop() else {
            return false;
        };
        if call.edge().is_some() {
            return false;
        }
        let message = Message {
            from: call.to.clone(),
            to: call.from.clone(),
            label: label.unwrap_or_default(),
            label_family: None,
            style: ArrowStyle::plain("filled", true),
            number: None,
            parallel: false,
        };
        self.message(message, vec![])
    }

    fn note(&mut self, note: Note) {
        self.items.push(Item::Note(note));
    }

    fn ref_over(&mut self, ref_over: Ref) {
        self.items.push(Item::Ref(ref_over));
    }

    fn after_message(&self) -> bool {
        self.items
            .iter()
            .rev()
            .find(|item| !matches!(item, Item::Activation(_)))
            .is_some_and(|item| matches!(item, Item::Message(_)))
    }

    fn after_ref(&self) -> bool {
        self.items
            .last()
            .is_some_and(|item| matches!(item, Item::Ref(_)))
    }

    fn after_block(&self) -> bool {
        self.items.last().is_some_and(|item| {
            matches!(item, Item::Fragment(Fragment { phase: FragPhase::Close, .. }))
        })
    }

    fn anchored(&self) -> bool {
        let last = self.items.iter().rev().find(|item| {
            !matches!(item, Item::Activation(_)) && !matches!(item, Item::Destroy(_))
        });
        matches!(
            last,
            Some(Item::Message(_))
                | Some(Item::Note(_))
                | Some(Item::Fragment(Fragment { phase: FragPhase::Close, .. }))
        )
    }

    fn newpage(&mut self) -> bool {
        if !self.blocks.is_empty() {
            return false;
        }
        self.items.push(Item::PageBreak(PageBreak));
        true
    }

    fn divider(&mut self, label: String) {
        self.items.push(Item::Divider(Divider { label }));
    }

    fn spacing(&mut self, height: Option<f64>) {
        self.items.push(Item::Spacing(Spacing { height }));
    }

    fn delay(&mut self, label: String) {
        self.items.push(Item::Delay(Delay { label }));
    }

    fn open_block(&mut self, keyword: &str, label: &str, parallel: bool) {
        self.blocks.push(keyword.to_string());
        self.items.push(Item::Fragment(Fragment {
            phase: FragPhase::Open,
            keyword: keyword.to_string(),
            label: Some(label.to_string()),
            parallel,
        }));
    }

    fn branch(&mut self, label: Option<String>) -> bool {
        let Some(keyword) = self.blocks.last().cloned() else {
            return false;
        };
        self.items.push(Item::Fragment(Fragment {
            phase: FragPhase::Else,
            keyword,
            label,
            parallel: false,
        }));
        true
    }

    fn close_block(&mut self) -> bool {
        let Some(keyword) = self.blocks.pop() else {
            return false;
        };
        self.items.push(Item::Fragment(Fragment {
            phase: FragPhase::Close,
            keyword,
            label: None,
            parallel: false,
        }));
        true
    }

    fn open_block_p(&self) -> bool {
        !self.blocks.is_empty()
    }

    /// Under `autonumber` a message takes the next number; one with no
    /// label or a line break in it is not drawn.
    fn numbered(&mut self, message: Message) -> Option<Message> {
        if !self.counting {
            return Some(message);
        }
        if message.label.trim().is_empty() || message.label.contains("\\n") {
            return None;
        }
        let numbering = self.numbering.as_ref()?;
        let text = numbering.text(self.number);
        let number = Number { text, bold: numbering.bold };
        self.number += numbering.step;
        let mut numbered = message;
        numbered.number = Some(number);
        Some(numbered)
    }
}

struct OpenBox {
    title: String,
    members: Vec<String>,
    fill: Option<Fill>,
    nested: bool,
}

#[derive(Clone)]
struct PendingNote {
    shape: NoteShape,
    side: NoteSide,
    targets: Vec<String>,
    lines: Vec<String>,
    parallel: bool,
    fill: Option<Fill>,
    depth: usize,
}

struct PendingChrome {
    kind: String,
    place: String,
    text: String,
    lines: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Before,
    Statements,
    After,
}

/// The parser state.
struct Parser {
    participants: Vec<(String, Participant)>,
    orders: HashMap<String, i64>,
    outline: Outline,
    boxes: Vec<Box_>,
    open_boxes: Vec<OpenBox>,
    pending_note: Option<PendingNote>,
    pending_style: Option<Vec<String>>,
    skin_open: bool,
    appearance: Appearance,
    teoz: bool,
    parallel: bool,
    footbox: bool,
    autoactivate: bool,
    title: Option<String>,
    chrome: Chrome,
    pending_chrome: Option<PendingChrome>,
    warnings: Vec<String>,
    line: usize,
}

struct DeclarationFields {
    kind: String,
    target: String,
    alias: Option<String>,
    stereotype: Option<String>,
    order: Option<i64>,
    fill: Option<String>,
}

fn declaration_ok(fields: &DeclarationFields) -> bool {
    let participant = fields.kind == "participant";
    (fields.stereotype.is_none() || participant)
        && (fields.fill.is_none()
            || (participant && fields.fill.as_deref().and_then(|f| fill_read(Some(f))).is_some()))
}

fn parse_error(message: &str, line: usize) -> Error {
    Error::Parse {
        class: "ParseError".into(),
        message: message.to_string(),
        line,
        col: 0,
    }
}

fn refusal_error(text: &str, line: usize) -> Error {
    Error::Parse {
        class: "UnsupportedConstructError".into(),
        message: format!("Unsupported construct at line {line}: {text}"),
        line,
        col: 0,
    }
}

fn unquote(name: &str) -> String {
    name.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(name)
        .to_string()
}

/// The length of a leading NAME, 0 when none.
fn name_end(text: &str) -> usize {
    let mut chars = text.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return 0,
    }
    let mut end = text.len();
    for (index, c) in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            end = index;
            break;
        }
    }
    end
}

/// Finds the arrow of a message line and the two endpoint tokens either
/// side of it: the hand parse of the Ruby MESSAGE regex, whose head and
/// trail look-arounds the `regex` crate cannot carry. Returns
/// (from, arrow token, to, marks, colour token, label).
#[allow(clippy::type_complexity)]
fn message_match(
    text: &str,
) -> Option<(String, String, String, Option<String>, Option<String>, Option<String>)> {
    let bytes = text.as_bytes();

    // FROM: QUOTED | NAME | a [ / ? glued to an arrow.
    let mut pos = 0;
    let from;
    if bytes.first() == Some(&b'"') {
        let inner = text[1..].find('"')?;
        if inner == 0 {
            return None;
        }
        let close = inner + 1;
        from = text[..=close].to_string();
        pos = close + 1;
    } else {
        let end = name_end(text);
        if end > 0 {
            from = text[..end].to_string();
            pos = end;
        } else if matches!(bytes.first(), Some(b'[') | Some(b'?'))
            && bytes.get(1).is_some_and(|b| {
                matches!(
                    b,
                    b'-' | b'<' | b'\\' | b'/' | b'o' | b'x' | b'O' | b'X'
                )
            }) {
            from = text[..1].to_string();
            pos = 1;
        } else {
            return None;
        }
    }

    let mut rest = &text[pos..];
    let blanks = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    rest = &rest[blanks..];

    // The arrow: an optional o/x lead, an optional left head, one dash,
    // an optional [...] style, an optional second dash, an optional right
    // head, an optional o/x trail before a blank, `]`, `?` or the end.
    // The Ruby regex backtracks, so the arrow ends are tried in its
    // depth-first order - the lead outermost, then the left head, then
    // the right head, then the trail - and the first end the rest of the
    // line parses with wins.
    let b = rest.as_bytes();
    let trail_ok = |at: usize| -> bool {
        matches!(
            b.get(at),
            Some(b'o') | Some(b'x') | Some(b'O') | Some(b'X')
        ) && matches!(
            b.get(at + 1),
            None | Some(b' ') | Some(b'\t') | Some(b']') | Some(b'?')
        )
    };
    let lead_ok = |at: usize| -> bool {
        matches!(
            b.get(at),
            Some(b'o') | Some(b'x') | Some(b'O') | Some(b'X')
        )
    };
    let mut ends: Vec<usize> = vec![];
    for lead in [true, false] {
        let lead_len = if lead && lead_ok(0) { 1 } else { 0 };
        if lead && lead_len == 0 {
            continue;
        }
        let heads_l: Vec<&str> = ["<<", "<", "//", "/", "\\\\", "\\"]
            .into_iter()
            .filter(|token| rest[lead_len..].starts_with(token))
            .chain(std::iter::once(""))
            .collect();
        for left_head in &heads_l {
            let mut p = lead_len + left_head.len();
            if b.get(p) != Some(&b'-') {
                continue;
            }
            p += 1;
            if b.get(p) == Some(&b'[') {
                let Some(close) = rest[p..].find(']') else { continue };
                p += close + 1;
            }
            if b.get(p) == Some(&b'-') {
                p += 1;
            }
            let heads_r: Vec<&str> = [">>", ">", "\\\\", "\\", "//", "/"]
                .into_iter()
                .filter(|token| rest[p..].starts_with(token))
                .chain(std::iter::once(""))
                .collect();
            for right_head in &heads_r {
                let base = p + right_head.len();
                for trail in [true, false] {
                    if trail && !trail_ok(base) {
                        continue;
                    }
                    ends.push(base + usize::from(trail));
                }
            }
        }
    }

    // TO: QUOTED | NAME | a glued ] / ?; then marks, colour, label.
    let mut ends = ends.into_iter();
    loop {
        let Some(arrow_len) = ends.next() else {
            return None;
        };
        let mut after = &rest[arrow_len..];
        let lead_blanks = after.len() - after.trim_start_matches([' ', '\t']).len();
        after = &after[lead_blanks..];
        let to;
        if after.starts_with('"') {
            let inner = after[1..].find('"')?;
            if inner == 0 {
                continue;
            }
            let close = inner + 1;
            to = after[..=close].to_string();
            after = &after[close + 1..];
        } else {
            let end = name_end(after);
            if end > 0 {
                to = after[..end].to_string();
                after = &after[end..];
            } else if lead_blanks == 0 && after.starts_with([']', '?']) {
                to = after[..1].to_string();
                after = &after[1..];
            } else {
                continue;
            }
        }

        // [ \t]* marks? [ \t]* colour? [ \t]* (: label)?
        let mut tail = after.trim_start_matches([' ', '\t']);
        let mut marks = None;
        for candidate in ["--++", "++--", "++", "--", "!!"] {
            if let Some(rest) = tail.strip_prefix(candidate) {
                marks = Some(candidate.to_string());
                tail = rest;
                break;
            }
        }
        tail = tail.trim_start_matches([' ', '\t']);
        let mut colour = None;
        if tail.starts_with('#') {
            let end = tail[1..]
                .find(|c: char| !(c.is_ascii_hexdigit() || c.is_ascii_alphabetic()))
                .map(|stop| stop + 1)
                .unwrap_or(tail.len());
            let body = &tail[1..end];
            let boundary = tail[end..]
                .chars()
                .next()
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
            let hex = (body.len() == 6 || body.len() == 3)
                && body.bytes().all(|b| b.is_ascii_hexdigit());
            let named = boundary
                && !body.is_empty()
                && body.bytes().all(|b| b.is_ascii_alphabetic())
                && COLOUR_NAMES.contains(&body.to_lowercase().as_str());
            if hex || named {
                colour = Some(tail[..end].to_string());
                tail = &tail[end..];
            }
        }
        let tail = tail.trim_start_matches([' ', '\t']);
        let mut label = None;
        if let Some(rest) = tail.strip_prefix(':') {
            label = Some(rest.trim_start_matches([' ', '\t']).to_string());
        } else if !tail.is_empty() {
            continue;
        }
        break Some((
            from,
            rest[..arrow_len].to_string(),
            to,
            marks,
            colour,
            label,
        ));
    }
}

const COLOUR_NAMES: &[&str] = &[
    "red", "green", "blue", "yellow", "orange", "purple", "gray", "grey",
    "cyan", "magenta", "lime", "navy", "teal", "olive", "maroon", "silver",
    "aqua", "fuchsia", "pink", "brown", "black", "white",
];

fn colour_value(token: &str) -> String {
    if !token.is_empty() && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("#{token}")
    } else {
        token.to_lowercase()
    }
}

fn message_label(label: &str) -> String {
    let doubled = label.starts_with("\"\"") && label.ends_with("\"\"") && label.len() >= 4;
    if !doubled || label.contains("\\n") {
        return label.to_string();
    }
    label[2..label.len() - 2].trim().replace("\\\\", "\\")
}

impl Parser {
    fn new() -> Self {
        Parser {
            participants: vec![],
            orders: HashMap::new(),
            outline: Outline::default(),
            boxes: vec![],
            open_boxes: vec![],
            pending_note: None,
            pending_style: None,
            skin_open: false,
            appearance: Appearance::default(),
            teoz: false,
            parallel: false,
            footbox: true,
            autoactivate: false,
            title: None,
            chrome: Chrome::default(),
            pending_chrome: None,
            warnings: vec![],
            line: 0,
        }
    }

    /// The id a mention names, creating a bare participant on first use.
    fn mention(&mut self, name: &str) -> String {
        let id = unquote(name);
        if !self.participants.iter().any(|(known, _)| *known == id) {
            self.participants.push((
                id.clone(),
                Participant {
                    id: id.clone(),
                    label: id.clone(),
                    kind: "participant",
                    stereotype: None,
                    fill: None,
                },
            ));
        }
        id
    }

    fn set(&mut self, appearance: Appearance) {
        self.appearance = self.appearance.merge(&appearance);
    }

    fn declare(&mut self, fields: DeclarationFields) {
        let display = unquote(&fields.target);
        let id = fields.alias.clone().unwrap_or_else(|| display.clone());
        if !self.participants.iter().any(|(known, _)| *known == id) {
            let kind: &'static str = match fields.kind.as_str() {
                "participant" => "participant",
                "actor" => "actor",
                "boundary" => "boundary",
                "control" => "control",
                "entity" => "entity",
                "database" => "database",
                "collections" => "collections",
                _ => "queue",
            };
            self.participants.push((
                id.clone(),
                Participant {
                    id: id.clone(),
                    label: display,
                    kind,
                    stereotype: fields.stereotype,
                    fill: fields.fill.as_deref().and_then(|f| fill_read(Some(f))),
                },
            ));
        }
        self.orders.entry(id.clone()).or_insert(fields.order.unwrap_or(0));
        if let Some(open) = self.open_boxes.last_mut() {
            open.members.push(id);
        }
    }

    fn diagram(mut self) -> Result<Diagram, Error> {
        // ordered_participants: `order N` seats left to right by N; equal
        // numbers keep the order they were met in (a stable sort).
        let mut ranked: Vec<(String, Participant)> = self.participants.clone();
        ranked.sort_by_key(|(id, _)| self.orders.get(id).copied().unwrap_or(0));
        let participants = ranked;

        // check_boxes: box members must be neighbours.
        let ids: Vec<&str> = participants.iter().map(|(id, _)| id.as_str()).collect();
        for box_ in &self.boxes {
            let mut places: Vec<usize> = box_
                .members
                .iter()
                .map(|id| ids.iter().position(|known| known == id).unwrap_or(0))
                .collect();
            places.sort_unstable();
            if places.windows(2).any(|pair| pair[1] != pair[0] + 1) {
                return Err(parse_error(
                    &format!(
                        "Parse error: participants of box {} are not neighbours, which is not supported",
                        box_.title
                    ),
                    0,
                ));
            }
        }
        // check_numbering.
        if self.appearance.max_message.is_some()
            && self.outline.items.iter().any(|item| {
                matches!(item, Item::Message(m) if m.number.is_some())
            })
        {
            return Err(parse_error(
                "Parse error: autonumber with Maxmessagesize is not supported",
                0,
            ));
        }
        let items = Outline::cut_at_page_break(&self.outline.items);
        Ok(Diagram {
            participants: participants.into_iter().map(|(_, p)| p).collect(),
            items,
            boxes: self.boxes,
            appearance: self.appearance,
            footbox: self.footbox,
            warnings: self.warnings,
            title: self.title,
            chrome: self.chrome,
            teoz: self.teoz,
        })
    }

    fn skippable(&self, text: &str) -> bool {
        if self.pending_note.is_some() || self.pending_chrome.is_some() {
            return false;
        }
        text.is_empty() || text.starts_with('\'')
    }

    fn step(&mut self, stage: Stage, text: &str) -> Result<Stage, Error> {
        if self.skippable(text) {
            return Ok(stage);
        }
        match stage {
            Stage::Before => {
                if startuml().is_match(text) {
                    Ok(Stage::Statements)
                } else {
                    Err(parse_error(
                        "Unable to detect diagram type from source. Source must start with one of: @startuml",
                        self.line,
                    ))
                }
            }
            Stage::Statements => {
                self.statement(text)?;
                Ok(Stage::Statements)
            }
            Stage::After => Err(refusal_error(text, self.line)),
        }
    }

    fn collecting(&self) -> bool {
        self.pending_note.is_some()
            || self.pending_chrome.is_some()
            || self.pending_style.is_some()
    }

    fn statement(&mut self, text: &str) -> Result<(), Error> {
        if self.collecting() {
            return self.collect_pending(text);
        }
        if self.skin_open {
            return self.collect_skinparam(text);
        }
        if text == "@enduml" {
            return self.end_of_diagram(text);
        }
        if let Some(caps) = pragma().captures(text) {
            if caps.get(1).unwrap().as_str().eq_ignore_ascii_case("teoz") {
                self.teoz = true;
            }
            return Ok(());
        }
        self.read(text)
    }

    fn collect_pending(&mut self, text: &str) -> Result<(), Error> {
        if self.pending_note.is_some() {
            return self.collect_note(text);
        }
        if self.pending_chrome.is_some() {
            return self.collect_chrome(text);
        }
        self.collect_style(text)
    }

    fn read(&mut self, text: &str) -> Result<(), Error> {
        if let Some(caps) = declaration().captures(text) {
            let fields = self.declaration_fields(caps);
            if declaration_ok(&fields) {
                self.declare(fields);
                return Ok(());
            }
        }
        if box_re().is_match(text) {
            self.open_box(text);
            return Ok(());
        }
        self.read_setting(text)
    }

    fn declaration_fields(&self, caps: regex::Captures) -> DeclarationFields {
        DeclarationFields {
            kind: caps.get(1).unwrap().as_str().to_lowercase(),
            target: caps.get(2).unwrap().as_str().to_string(),
            alias: caps.get(3).map(|m| m.as_str().to_string()),
            stereotype: caps.get(4).map(|m| m.as_str().to_string()),
            order: caps.get(5).and_then(|m| m.as_str().parse().ok()),
            fill: caps.get(6).map(|m| m.as_str().to_string()),
        }
    }

    fn read_setting(&mut self, text: &str) -> Result<(), Error> {
        if let Some(caps) = skinparam_width().captures(text) {
            let mut appearance = Appearance::default();
            appearance.min_width = caps.get(1).and_then(|m| m.as_str().parse().ok());
            self.set(appearance);
            return Ok(());
        }
        if let Some(caps) = skinparam_max_message().captures(text) {
            let mut appearance = Appearance::default();
            appearance.max_message = caps.get(1).and_then(|m| m.as_str().parse().ok());
            self.set(appearance);
            return Ok(());
        }
        if let Some(caps) = skinparam_paint().captures(text) {
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();
            return match super::style::paint_read(key, value) {
                Some(appearance) => {
                    self.set(appearance);
                    Ok(())
                }
                None => Err(refusal_error(text, self.line)),
            };
        }
        if skinparam_open().is_match(text) {
            self.skin_open = true;
            return Ok(());
        }
        if hide_footbox().is_match(text) {
            self.footbox = false;
            return Ok(());
        }
        if let Some(caps) = autoactivate().captures(text) {
            self.autoactivate = caps.get(1).unwrap().as_str().trim().eq_ignore_ascii_case("on");
            return Ok(());
        }
        if let Some(caps) = chrome_open().captures(text) {
            let kind = caps
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_else(|| "legend".into());
            let place = format!(
                "{} {}",
                caps.get(2).map(|m| m.as_str()).unwrap_or("bottom"),
                caps.get(3).map(|m| m.as_str()).unwrap_or("center")
            )
            .to_lowercase();
            self.pending_chrome = Some(PendingChrome {
                kind,
                place,
                text: text.to_string(),
                lines: vec![],
            });
            return Ok(());
        }
        if title_gate().is_match(text) {
            let Some((kind, body)) = caption_read(text) else {
                return Err(refusal_error(text, self.line));
            };
            self.keep(&kind, body, None);
            return Ok(());
        }
        if style_open().is_match(text) {
            self.pending_style = Some(vec![]);
            return Ok(());
        }
        self.read_structure(text)
    }

    fn keep(&mut self, kind: &str, text: String, place: Option<String>) {
        if kind == "title" {
            self.title = Some(text);
        } else {
            self.chrome = std::mem::take(&mut self.chrome).with(kind, text, place);
        }
    }

    fn collect_chrome(&mut self, text: &str) -> Result<(), Error> {
        let kind = self.pending_chrome.as_ref().unwrap().kind.clone();
        let close = chrome_close().captures(text);
        let kind_matches = close
            .as_ref()
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().eq_ignore_ascii_case(&kind))
            .unwrap_or(false);
        if kind_matches {
            let pending = self.pending_chrome.take().unwrap();
            if pending.lines.is_empty() {
                return Err(refusal_error(&pending.text, self.line));
            }
            let place = if pending.kind == "legend" {
                Some(pending.place.clone())
            } else {
                None
            };
            self.keep(&pending.kind, pending.lines.join("\n"), place);
        } else {
            if !caption_plain(text) {
                return Err(Error::Parse {
                    class: "UnsupportedConstructError".into(),
                    message: kind,
                    line: self.line,
                    col: 0,
                });
            }
            self.pending_chrome.as_mut().unwrap().lines.push(text.to_string());
        }
        Ok(())
    }

    fn collect_style(&mut self, text: &str) -> Result<(), Error> {
        if style_close().is_match(text) {
            let lines = self.pending_style.take().unwrap_or_default();
            let line = self.line;
            match style_read(&lines.join("\n")) {
                Some(read) => {
                    self.set(read);
                    Ok(())
                }
                None => Err(refusal_error("<style>", line)),
            }
        } else {
            if let Some(style) = self.pending_style.as_mut() {
                style.push(text.to_string());
                Ok(())
            } else {
                Ok(())
            }
        }
    }

    fn collect_skinparam(&mut self, text: &str) -> Result<(), Error> {
        if text == "}" {
            self.skin_open = false;
        } else if let Some(caps) = skinparam_line().captures(text) {
            let mut appearance = Appearance::default();
            appearance.max_message = caps.get(1).and_then(|m| m.as_str().parse().ok());
            self.set(appearance);
        } else {
            let caps = skinparam_paint_line()
                .captures(text)
                .ok_or_else(|| refusal_error(text, self.line))?;
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();
            match super::style::paint_read(key, value) {
                Some(appearance) => self.set(appearance),
                None => return Err(refusal_error(text, self.line)),
            }
        }
        Ok(())
    }

    fn open_box(&mut self, text: &str) {
        let caps = box_re().captures(text).unwrap();
        if let Some(open) = self.open_boxes.last_mut() {
            open.nested = true;
        }
        self.open_boxes.push(OpenBox {
            title: caps.get(1).map(|m| unquote(m.as_str())).unwrap_or_default(),
            members: vec![],
            fill: caps.get(2).and_then(|m| fill_read(Some(m.as_str()))),
            nested: false,
        });
    }

    fn close_box(&mut self) {
        let open = self.open_boxes.pop().unwrap();
        if open.nested && open.members.is_empty() {
            return;
        }
        self.boxes.push(Box_ {
            title: open.title,
            members: open.members,
            fill: open.fill,
        });
    }

    fn end_of_diagram(&mut self, text: &str) -> Result<(), Error> {
        if self.participants.is_empty() {
            return Err(refusal_error(text, self.line));
        }
        if let Some(open) = self.open_boxes.last() {
            return Err(parse_error(
                &format!("Parse error: box {:?} is never closed with endbox", open.title),
                self.line,
            ));
        }
        if self.outline.open_block_p() {
            return Err(parse_error(
                "Parse error: a block is never closed with end",
                self.line,
            ));
        }
        Ok(())
    }

    fn read_structure(&mut self, text: &str) -> Result<(), Error> {
        if end_box().is_match(text) && !self.open_boxes.is_empty() {
            self.close_box();
            return Ok(());
        }
        if !self.open_boxes.is_empty() || !self.timeline(text) {
            return Err(refusal_error(text, self.line));
        }
        Ok(())
    }

    fn timeline(&mut self, text: &str) -> bool {
        if let Some(rest) = text.strip_prefix('&') {
            let rest = rest.trim_start_matches([' ', '\t']);
            return self.parallel_line(rest);
        }
        self.read_timeline(text)
    }

    fn parallel_line(&mut self, text: &str) -> bool {
        // Only teoz draws `&` on the row above.
        if !(self.teoz && self.outline.anchored()) {
            return false;
        }
        if !(message_match(text).is_some()
            || note().is_match(text)
            || block().is_match(text))
        {
            return false;
        }
        self.parallel = true;
        let out = self.read_timeline(text);
        self.parallel = false;
        out
    }

    fn read_timeline(&mut self, text: &str) -> bool {
        if let Some(captures) = message_match(text) {
            return self.message(captures);
        }
        if let Some(caps) = note().captures(text) {
            return self.note(caps);
        }
        if let Some(caps) = block().captures(text) {
            let keyword = caps.get(1).unwrap().as_str().to_lowercase();
            let label = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            self.outline.open_block(&keyword, label, self.parallel);
            return true;
        }
        if let Some(caps) = ref_re().captures(text) {
            return self.ref_line(caps);
        }
        if let Some(caps) = branch().captures(text) {
            return self.outline.branch(caps.get(1).map(|m| m.as_str().to_string()));
        }
        if let Some(caps) = activation().captures(text) {
            return self.activation(caps);
        }
        if let Some(caps) = reply().captures(text) {
            return self.outline.reply(caps.get(1).map(|m| m.as_str().to_string()));
        }
        if let Some(caps) = divider().captures(text) {
            let label = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            self.outline.divider(label);
            return true;
        }
        if let Some(caps) = delay().captures(text) {
            let label = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            self.outline.delay(label);
            return true;
        }
        if let Some(caps) = spacing().captures(text) {
            let height = caps.get(1).and_then(|m| m.as_str().parse().ok());
            self.outline.spacing(height);
            return true;
        }
        if let Some(caps) = destroy().captures(text) {
            let id = self.mention(caps.get(1).unwrap().as_str());
            self.outline.destroy(id);
            return true;
        }
        if let Some(caps) = create().captures(text) {
            return self.create(caps.get(1).unwrap().as_str());
        }
        if newpage_line().is_match(text) {
            return self.outline.newpage();
        }
        if let Some(caps) = autonumber_line().captures(text) {
            let start = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let step = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let format = caps.get(3).map(|m| m.as_str().to_string());
            return match Numbering::read(start, step, format.as_deref()) {
                Some(numbering) => {
                    self.outline.autonumber(numbering);
                    true
                }
                None => false,
            };
        }
        if let Some(caps) = autonumber_pause().captures(text) {
            return self
                .outline
                .counting(caps.get(1).unwrap().as_str().eq_ignore_ascii_case("resume"));
        }
        text.eq_ignore_ascii_case("end") && self.outline.close_block()
    }

    #[allow(clippy::type_complexity)]
    fn message(
        &mut self,
        (from, arrow, to, marks, colour, label): (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ) -> bool {
        let Some((style, reversed)) = arrow_syntax(&arrow) else {
            return false;
        };
        let left = self.endpoint(&from, Side::Left);
        let right = self.endpoint(&to, Side::Right);
        let ends = [left, right];
        let edges = ends
            .iter()
            .filter(|place| matches!(place, Place::Edge(_)))
            .count();
        if edges >= 2
            || ends
                .iter()
                .any(|place| matches!(place, Place::Edge(e) if reversed && e.global()))
        {
            return false;
        }
        let (from_place, to_place) = if reversed {
            (ends[1].clone(), ends[0].clone())
        } else {
            (ends[0].clone(), ends[1].clone())
        };
        let family = if label.as_deref().is_some_and(|l| l.starts_with("\"\"")) {
            Some("monospace")
        } else {
            None
        };
        let message = Message {
            from: from_place,
            to: to_place,
            label: message_label(label.as_deref().unwrap_or("")),
            label_family: family,
            style,
            number: None,
            parallel: self.parallel,
        };

        // unplaceable?: a `?` end has no ring, and a missing end cannot be
        // activated or destroyed.
        if let Some(edge) = message.edge() {
            if self.autoactivate
                || marks.is_some()
                || (edge.local && message.edge_end().circle)
            {
                return false;
            }
        }

        // Ruby match[6] is the token after the `#`.
        let colour_value = colour
            .as_deref()
            .map(|token| token.strip_prefix('#').unwrap_or(token))
            .map(colour_value);
        let resolved: Marks = if self.autoactivate && marks.is_none() && colour.is_none() {
            // automatic_marks: a call activates its receiver, a dashed
            // answer deactivates its sender when active; a lost message
            // does neither.
            if message.style.head.glyph == Some("cross") {
                vec![]
            } else if !message.dashed() {
                vec![(
                    Phase::On,
                    message.to.id().to_string(),
                    None,
                    MarkKind::Activation,
                )]
            } else if self.outline.active(&message.from.id()) {
                vec![(
                    Phase::Off,
                    message.from.id().to_string(),
                    None,
                    MarkKind::Activation,
                )]
            } else {
                vec![]
            }
        } else if marks.as_deref() == Some("!!") {
            vec![(
                Phase::Off,
                message.to.id().to_string(),
                None,
                MarkKind::Destroy,
            )]
        } else {
            marks
                .as_deref()
                .map(marks_of_suffix)
                .unwrap_or_default()
                .into_iter()
                .map(|phase| {
                    let id = if phase == Phase::Off {
                        message.from.id().to_string()
                    } else {
                        message.to.id().to_string()
                    };
                    let color = if phase == Phase::On {
                        colour_value.clone()
                    } else {
                        None
                    };
                    (phase, id, color, MarkKind::Activation)
                })
                .collect()
        };
        if colour.is_some() && !resolved.iter().any(|(phase, _, _, _)| *phase == Phase::On) {
            return false;
        }
        self.outline.message(message, resolved)
    }

    fn endpoint(&mut self, token: &str, side: Side) -> Place {
        match edge_read(token, side) {
            Some(edge) => Place::Edge(edge),
            None => Place::Id(self.mention(token)),
        }
    }

    fn create(&mut self, body: &str) -> bool {
        if let Some(caps) = declaration().captures(body) {
            let fields = self.declaration_fields(caps);
            if declaration_ok(&fields) {
                self.declare(fields);
                let caps = declaration().captures(body).unwrap();
                let id = caps
                    .get(3)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| unquote(caps.get(2).unwrap().as_str()));
                return self.outline.create(id);
            }
        }
        if target_only().is_match(body) {
            let id = self.mention(body);
            return self.outline.create(id);
        }
        false
    }

    fn activation(&mut self, caps: regex::Captures) -> bool {
        let phase = if caps.get(1).unwrap().as_str().eq_ignore_ascii_case("activate") {
            Phase::On
        } else {
            Phase::Off
        };
        // match[3] is the whole `#colour`, match[4] the inner token.
        let token = caps.get(4).map(|m| m.as_str().to_string());
        if token.is_some() && phase == Phase::Off {
            return false;
        }
        let id = self.mention(caps.get(2).unwrap().as_str());
        let color = token.as_deref().map(colour_value);
        self.outline.activation(phase, id, color)
    }

    fn ref_line(&mut self, caps: regex::Captures) -> bool {
        let targets_text = caps.get(1).unwrap().as_str();
        let label = caps
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
            .trim()
            .to_string();
        if label.contains("\\n") {
            return false;
        }
        let targets = self.targets_of(Some(targets_text));
        self.outline.ref_over(Ref { targets, label });
        true
    }

    fn targets_of(&mut self, list: Option<&str>) -> Vec<String> {
        // Ruby `list.to_s.split(",")`: an absent list splits to nothing.
        let Some(list) = list else {
            return vec![];
        };
        if list.is_empty() {
            return vec![];
        }
        list.split(',').map(|target| self.mention(target.trim())).collect()
    }

    fn note(&mut self, caps: regex::Captures) -> bool {
        let shape = match caps.get(1).unwrap().as_str().to_lowercase().as_str() {
            "hnote" => NoteShape::Hex,
            "rnote" => NoteShape::Rect,
            _ => NoteShape::Fold,
        };
        let side = match caps.get(2).unwrap().as_str().to_lowercase().as_str() {
            "left" => NoteSide::Left,
            "right" => NoteSide::Right,
            "across" => NoteSide::Across,
            "over" => NoteSide::Over,
            "top" => NoteSide::Top,
            _ => NoteSide::Bottom,
        };
        let fill_token = caps.get(4).map(|m| m.as_str().to_string());
        let fill = match &fill_token {
            Some(token) => match note_fill_read(Some(token)) {
                Some(fill) => Some(fill),
                None => return false,
            },
            None => None,
        };
        let targets = self.targets_of(caps.get(3).map(|m| m.as_str()));
        let text = caps.get(5).map(|m| m.as_str().to_string());
        let pending = PendingNote {
            shape,
            side,
            targets,
            lines: vec![],
            parallel: self.parallel,
            fill,
            depth: 0,
        };
        // PlantUML has no top or bottom note.
        if matches!(pending.side, NoteSide::Top | NoteSide::Bottom) {
            return self.positioned_note(pending, text.as_deref());
        }
        let hanging = pending.targets.is_empty()
            && matches!(pending.side, NoteSide::Left | NoteSide::Right);
        if hanging && !(self.outline.after_message() || self.outline.after_block()) {
            return false;
        }
        self.pending_note = Some(pending);
        match text {
            Some(text) => {
                self.finish_note(text.trim());
                true
            }
            None => true,
        }
    }

    fn positioned_note(&mut self, pending: PendingNote, text: Option<&str>) -> bool {
        let Some(text) = text else {
            return false;
        };
        if !pending.targets.is_empty() {
            return false;
        }
        if self.parallel {
            return false;
        }
        if self.outline.after_message() {
            let mut pending = pending;
            pending.side = NoteSide::Left;
            self.pending_note = Some(pending);
            self.finish_note(text.trim());
            true
        } else if self.outline.after_ref() {
            let name = match pending.side {
                NoteSide::Top => "TOP",
                _ => "BOTTOM",
            };
            self.warnings.push(format!("This position is ignored: {name}"));
            true
        } else {
            false
        }
    }

    fn collect_note(&mut self, text: &str) -> Result<(), Error> {
        let closes =
            end_note().is_match(text) && self.pending_note.as_ref().unwrap().depth == 0;
        if closes {
            let lines = self.pending_note.as_ref().unwrap().lines.join("\n");
            self.finish_note(&lines);
        } else {
            let pending = self.pending_note.as_mut().unwrap();
            if embedded_opens(text) {
                pending.depth += 1;
            }
            if embedded_closes(text) && pending.depth > 0 {
                pending.depth -= 1;
            }
            pending.lines.push(text.to_string());
        }
        Ok(())
    }

    fn finish_note(&mut self, text: &str) -> bool {
        let Some(pending) = self.pending_note.take() else {
            return false;
        };
        self.outline.note(Note {
            shape: pending.shape,
            side: pending.side,
            targets: pending.targets,
            text: text.to_string(),
            parallel: pending.parallel,
            fill: pending.fill,
            picture: None,
        });
        true
    }
}

/// `Embedded.opens?` / `Embedded.closes?`.
fn embedded_opens(line: &str) -> bool {
    line.trim() == "{{"
}

fn embedded_closes(line: &str) -> bool {
    line.trim() == "}}"
}

/// Reads the stripped lines into the Diagram. The grammar has accepted
/// the whole source; the script ends at its `@enduml` line.
pub fn diagram(tree: &serde_json::Value, source: &str) -> Result<Diagram, Error> {
    let _ = tree;
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    let lines: Vec<&str> = source.split('\n').collect();
    let last = lines
        .iter()
        .rposition(|line| line.trim() == "@enduml")
        .ok_or_else(|| {
            parse_error("Parse error: missing @enduml before the end of the source", 0)
        })?;
    read_lines(&lines[..=last])
}

/// The embedded path: the same reader over a document of its own.
pub fn read_document(source: &str) -> Result<Diagram, Error> {
    let lines: Vec<&str> = source.split('\n').collect();
    let last = lines
        .iter()
        .rposition(|line| line.trim() == "@enduml")
        .ok_or_else(|| {
            parse_error("Parse error: missing @enduml before the end of the source", 0)
        })?;
    read_lines(&lines[..=last])
}

fn read_lines(lines: &[&str]) -> Result<Diagram, Error> {
    let mut parser = Parser::new();
    let mut stage = Stage::Before;
    for (index, raw) in lines.iter().enumerate() {
        let text = raw.trim_matches(|c: char| {
            matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0')
        });
        parser.line = index + 1;
        stage = parser.step(stage, text)?;
    }
    if stage == Stage::Before {
        return Err(parse_error(
            "Unable to detect diagram type from source. Source must start with one of: @startuml",
            0,
        ));
    }
    parser.diagram()
}

/// `Embedded.read`: the note body as one embedded diagram, with its scale.
pub struct Embedded {
    pub document: String,
    pub scale: f64,
}

pub fn embedded_read(text: &str) -> Option<Embedded> {
    let lines: Vec<&str> = split_lines(text).iter().map(|l| l.trim()).collect();
    // whole?: opens on the first line, closes on the last, no early close.
    if lines.len() < 2 || lines.first() != Some(&"{{") || lines.last() != Some(&"}}") {
        return None;
    }
    let mut depth = 0;
    for line in &lines[..lines.len() - 1] {
        if embedded_opens(line) {
            depth += 1;
        }
        if embedded_closes(line) {
            depth -= 1;
        }
        if depth == 0 {
            return None;
        }
    }
    let mut body = lines[1..lines.len() - 1].to_vec();
    let scale_re = cell(
        || re(r"\Ascale[ \t]+(\d+(?:\.\d+)?)\z"),
        "embedded_scale",
    );
    let mut scale = 1.0;
    if let Some(first) = body.first() {
        if let Some(caps) = scale_re.captures(first) {
            scale = caps.get(1).unwrap().as_str().parse().unwrap_or(1.0);
            body.remove(0);
        }
    }
    Some(Embedded {
        document: format!("@startuml\n{}\n@enduml\n", body.join("\n")),
        scale,
    })
}

/// Whether any line of the body opens an embedded block.
pub fn embeds_any(text: &str) -> bool {
    text.split('\n').any(embedded_opens)
}
