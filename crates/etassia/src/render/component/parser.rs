//! Reads use case, component and deployment source into a [`Graph`].
//! Port of `node_graph/parser.rb` (the entity and JSON-data paths are out
//! of scope: a source carrying them is never claimed by the component
//! notation).

use regex::Regex;
use std::sync::OnceLock;

use super::archimate;
use super::model::{Direction, Edge, Graph, Head, Link, NodeKind, Style};
use super::references::{self, lines_of};
use super::reader;

pub struct Entry {
    pub text: String,
    pub number: usize,
}

fn re(pattern: &'static str) -> &'static Regex {
    static CELL: OnceLock<std::collections::HashMap<&'static str, Regex>> = OnceLock::new();
    let map = CELL.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        let mut add = |key: &'static str, pattern: &str| {
            map.insert(key, Regex::new(pattern).expect("parser pattern"));
        };
        add("container", r"(?i)\A(?P<kw>package|node|database|cloud|rectangle|folder|frame|storage|artifact|card|namespace|component|file|interface|actor|usecase|portin|portout|circle|archimate)\s+(?P<rest>.*?)\s*\{\z");
        add("leaf", r"(?is)\A(?P<kw>package|node|database|cloud|rectangle|folder|frame|storage|artifact|card|namespace|component|file|interface|actor|usecase|portin|portout|circle|archimate)\s+(?P<rest>.+)\z");
        add("short", r#"\A(?P<ref>\[[^\]]+\]|\([^()]+\)|:[^:\n]+:)(?:\s+as\s+(?P<alias>"[^"]+"|\(?[^\s()]+\)?))?(?P<tail>(?:\s*<<.*?>>)?(?:\s+\#\w+)?)\z"#);
        add("bare", r"\A(?P<id>[\p{L}\p{N}_$]+)\s*(?P<tail><<.*>>)\z");
        add("lollipop", r"\A\(\)\s+(?P<rest>.+)\z");
        add("skipped", r"(?i)\A(?:skinparam|hide|show|allow_?mixing|scale|!pragma)\b");
        add("shapes", r"(?i)\A(?:queue|stack|agent|boundary|control|collections|hexagon|label|person)\s");
        add("keywords_refused", r"\A(?:legend|header|footer|sprite|mainframe|together)\b");
        add("style_block", r"(?i)\A<style>");
        add("floating_note", r"(?i)\Afloating\s+note");
        add("note", r#"(?is)\Anote\s+(?:(?P<side>left|right|top|bottom)(?:\s+of\s+(?P<ref>\[[^\]]+\]|\([^()]*\)|:[^:\n]+:|"[^"]+"|[\p{L}\p{N}_$]+))?|(?:(?:"(?P<quoted>.*)"\s+)?as\s+(?P<name>\S+)))\s*(?::\s*(?P<text>.*))?\z"#);
        add("title", r"(?i)\Atitle\s+(.+)\z");
        add("left_to_right", r"(?i)\Aleft to right direction\z");
        add("top_to_bottom", r"(?i)\Atop to bottom direction\z");
        add("sprite", r"(?i)\Asprite[ \t]+\S+[ \t]*<svg");
        add("end_note", r"(?i)\Aend\s*note\z");
        add("stereotype", r"<<\s*(?P<st>.*?)\s*>>");
        add("leading_color", r"\A\#(?P<color>[0-9A-Fa-f]{3,8}|[A-Za-z]+)\s+");
        add("color", r"\s+\#(?P<color>[0-9A-Fa-f]{3,8}|[A-Za-z]+)\z");
        add("tail_color", r"\#(\w+)\s*\z");
        add("names_quoted_as", r#"(?s)\A"(.*)"\s+as\s+(\S+)\z"#);
        add("names_as_quoted", r#"(?s)\A(\S+)\s+as\s+"(.*)"\z"#);
        add("names_bracket", r"(?s)\A(\S+)\s+\[(.*)\]\z");
        add("names_as", r"\A(\S+)\s+as\s+(\S+)\z");
        map
    });
    map.get(pattern).expect("known parser pattern")
}

pub enum DirectionInput {
    LeftToRight,
    TopToBottom,
}

pub struct Parser {
    graph: Graph,
    default_kind: NodeKind,
    stack: Vec<usize>,
    counter: usize,
    entries: Vec<Entry>,
    index: usize,
    number: usize,
    text: String,
}

impl Parser {
    pub fn new(direction: DirectionInput, default_kind: NodeKind) -> Parser {
        let direction = match direction {
            DirectionInput::LeftToRight => Direction::LeftToRight,
            DirectionInput::TopToBottom => Direction::TopToBottom,
        };
        Parser {
            graph: Graph::new(direction),
            default_kind,
            stack: vec![],
            counter: 0,
            entries: vec![],
            index: 0,
            number: 0,
            text: String::new(),
        }
    }

    /// The body entries the PARG tree carried, as `(text, number)`.
    pub fn parse_entries(mut self, entries: Vec<Entry>) -> Result<Graph, crate::Error> {
        self.entries = entries;
        self.index = 0;
        while self.index < self.entries.len() {
            let (text, number) = {
                let entry = &self.entries[self.index];
                (entry.text.clone(), entry.number)
            };
            self.index += 1;
            self.number = number;
            self.text = text.clone();
            self.statement(&text)?;
        }
        Ok(self.graph)
    }

    fn statement(&mut self, text: &str) -> Result<(), crate::Error> {
        if re("sprite").is_match(text) {
            self.skip_sprite();
            return Ok(());
        }
        if self.skip(text)? {
            return Ok(());
        }
        let handlers: [fn(&mut Parser, &str) -> bool; 7] = [
            |p, t| p.take_close(t),
            |p, t| p.take_container(t),
            |p, t| p.take_note(t),
            |p, t| p.take_short(t),
            |p, t| p.take_lollipop(t),
            |p, t| p.take_leaf(t),
            |p, t| p.take_bare(t),
        ];
        for handler in handlers {
            if handler(self, text) {
                return Ok(());
            }
        }
        if self.take_edge(text) {
            return Ok(());
        }
        Err(self.unsupported("statement"))
    }

    fn unsupported(&self, construct: &str) -> crate::Error {
        crate::Error::Parse {
            class: "UnsupportedConstructError".into(),
            message: format!("unsupported construct: {} at line {}: {}", construct, self.number, self.text),
            line: self.number,
            col: 0,
        }
    }

    fn skip_sprite(&mut self) {
        while self.index < self.entries.len()
            && !self.entries[self.index - 1].text.contains("</svg>")
        {
            self.index += 1;
        }
    }

    fn skip(&mut self, text: &str) -> Result<bool, crate::Error> {
        self.refuse_known(text)?;
        if self.title(text) {
            return Ok(true);
        }
        if self.direction(text) {
            return Ok(true);
        }
        self.skip_setting(text)
    }

    fn refuse_known(&mut self, text: &str) -> Result<(), crate::Error> {
        if re("style_block").is_match(text) {
            return Err(self.unsupported("style block"));
        }
        if text.starts_with('!') {
            let after = &text[1..];
            let pragma = after.starts_with("pragma")
                && after
                    .chars()
                    .nth(6)
                    .map(|c| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(true);
            if !pragma {
                return Err(self.unsupported("preprocessor"));
            }
        }
        if re("keywords_refused").is_match(text) {
            return Err(self.unsupported("keyword"));
        }
        if re("floating_note").is_match(text) {
            return Err(self.unsupported("floating note"));
        }
        if re("shapes").is_match(text) {
            return Err(self.unsupported("shape"));
        }
        Ok(())
    }

    fn skip_setting(&mut self, text: &str) -> Result<bool, crate::Error> {
        if !re("skipped").is_match(text) {
            return Ok(false);
        }
        if text.ends_with('{') {
            self.skip_block();
        }
        Ok(true)
    }

    fn skip_block(&mut self) {
        while self.index < self.entries.len() && self.entries[self.index].text != "}" {
            self.index += 1;
        }
        self.index += 1;
    }

    fn title(&mut self, text: &str) -> bool {
        match re("title").captures(text) {
            Some(captures) => {
                self.graph.title = Some(captures[1].to_string());
                true
            }
            None => false,
        }
    }

    fn direction(&mut self, text: &str) -> bool {
        if re("left_to_right").is_match(text) {
            self.graph.direction = Direction::LeftToRight;
            return true;
        }
        if re("top_to_bottom").is_match(text) {
            self.graph.direction = Direction::TopToBottom;
            return true;
        }
        false
    }

    fn take_close(&mut self, text: &str) -> bool {
        if text == "}" {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    fn take_container(&mut self, text: &str) -> bool {
        let Some(captures) = re("container").captures(text) else { return false; };
        let kw = captures["kw"].to_lowercase();
        let rest = captures["rest"].to_string();
        let kind = NodeKind::from_keyword(&kw).expect("container keyword");
        let node = self.declare(kind, &rest);
        self.graph.nodes[node].container = true;
        self.stack.push(node);
        true
    }

    fn take_leaf(&mut self, text: &str) -> bool {
        let Some(captures) = re("leaf").captures(text) else { return false; };
        let kw = captures["kw"].to_lowercase();
        let rest = captures["rest"].to_string();
        let kind = NodeKind::from_keyword(&kw).expect("leaf keyword");
        self.declare(kind, &rest);
        true
    }

    fn take_lollipop(&mut self, text: &str) -> bool {
        let Some(captures) = re("lollipop").captures(text) else { return false; };
        self.declare(NodeKind::Interface, &captures["rest"]);
        true
    }

    fn take_short(&mut self, text: &str) -> bool {
        let Some(captures) = re("short").captures(text) else { return false; };
        let reference = captures["ref"].to_string();
        let (kind, name) = bracketed(&reference);
        let id = captures
            .name("alias")
            .map(|alias| unbracket(&strip_quotes(alias.as_str())));
        let node = self.place(&name, kind, id.as_deref());
        let tail = captures.name("tail").map(|t| t.as_str());
        self.style(node, tail);
        true
    }

    fn take_bare(&mut self, text: &str) -> bool {
        let Some(captures) = re("bare").captures(text) else { return false; };
        let id = captures["id"].to_string();
        let node = self.place(&id, self.default_kind, Some(&id));
        self.style(node, captures.name("tail").map(|t| t.as_str()));
        true
    }

    fn take_note(&mut self, text: &str) -> bool {
        let Some(captures) = re("note").captures(text) else { return false; };
        let body = captures
            .name("text")
            .map(|m| m.as_str().to_string())
            .or_else(|| captures.name("quoted").map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| self.note_body());
        let id = match captures.name("name") {
            Some(name) => name.as_str().to_string(),
            None => {
                self.counter += 1;
                format!("note{}", self.counter)
            }
        };
        let parent = self.stack.last().copied();
        let note = self.graph.node(&id, NodeKind::Note, parent);
        self.graph.nodes[note].lines = lines_of(&body);
        let reference = captures.name("ref").map(|m| m.as_str().to_string());
        let side = captures.name("side").map(|m| m.as_str().to_string());
        self.link_note(note, reference.as_deref(), side.as_deref());
        true
    }

    fn note_body(&mut self) -> String {
        let mut lines = vec![];
        while self.index < self.entries.len() {
            let text = self.entries[self.index].text.clone();
            self.index += 1;
            if re("end_note").is_match(&text) {
                break;
            }
            lines.push(text);
        }
        lines.join("\n")
    }

    fn link_note(&mut self, note: usize, reference: Option<&str>, side: Option<&str>) {
        let Some(reference) = reference else { return; };
        let beside = matches!(side, Some("left") | Some("right"));
        let to = self.resolve(reference);
        let backwards = matches!(side, Some("right") | Some("bottom"));
        self.graph.edges.push(Edge::new(
            note,
            to,
            None,
            Style::Dashed,
            None,
            None,
            None,
            1,
            Some(if beside { 0 } else { 1 }),
            backwards,
            true,
        ));
    }

    fn take_edge(&mut self, text: &str) -> bool {
        let Some(matched) = references::match_edge(text) else { return false; };
        let link = Link::new(&matched.arrow);
        if link.hidden() {
            return true;
        }
        let from = self.resolve(&matched.a);
        let to = self.resolve(&matched.b);
        self.graph.edges.push(Edge::new(
            from,
            to,
            matched.label.as_deref().map(|l| l.trim().to_string()),
            link.style(),
            link.head(),
            link.color.clone(),
            link.color2.clone(),
            link.width,
            Some(link.rank_step()),
            link.backwards(),
            false,
        ));
        true
    }

    fn declare(&mut self, kind: NodeKind, rest: &str) -> usize {
        let stereo = re("stereotype").captures(rest).and_then(|c| c.name("st")).map(|m| m.as_str().to_string());
        let color = re("color").captures(rest).and_then(|c| c.name("color")).map(|m| m.as_str().to_string())
            .or_else(|| re("leading_color").captures(rest).and_then(|c| c.name("color")).map(|m| m.as_str().to_string()));
        let bare = bare_name(rest);
        let (id, label) = names(&bare);
        let node = self.place(&label, kind, Some(&id));
        if kind == NodeKind::Archimate {
            self.graph.nodes[node].color = archimate::color(color.as_deref());
            self.graph.nodes[node].sprite = archimate::sprite(stereo.as_deref());
        } else {
            self.graph.nodes[node].stereotype = stereo;
            self.graph.nodes[node].color = color;
        }
        node
    }

    /// Creates or updates the node `id` and sets its label.
    fn place(&mut self, label: &str, kind: NodeKind, id: Option<&str>) -> usize {
        let id = id.unwrap_or(label);
        let parent = self.stack.last().copied();
        let node = self.graph.node(id, kind, parent);
        self.graph.nodes[node].kind = kind;
        self.graph.nodes[node].lines = lines_of(label);
        node
    }

    fn style(&mut self, node: usize, tail: Option<&str>) {
        let Some(tail) = tail else { return };
        let trimmed = tail.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(captures) = re("stereotype").captures(trimmed) {
            if let Some(st) = captures.name("st") {
                self.graph.nodes[node].stereotype = Some(st.as_str().to_string());
            }
        }
        if let Some(captures) = re("tail_color").captures(trimmed) {
            self.graph.nodes[node].color = Some(captures[1].to_string());
        }
    }

    /// The node a link end names, made if it is new.
    fn resolve(&mut self, reference: &str) -> usize {
        let (kind, name) = if references::is_bracketed(reference) {
            let (kind, name) = bracketed(reference);
            (kind, name)
        } else {
            (self.default_kind, strip_quotes(reference))
        };
        match self.graph.find(&name) {
            Some(node) => node,
            None => {
                let parent = self.stack.last().copied();
                self.graph.node(&name, kind, parent)
            }
        }
    }
}

/// `rest` without its stereotype and colour trappings.
fn bare_name(rest: &str) -> String {
    let without_stereo = re("stereotype").replace(rest, "").to_string();
    let without_color = re("color").replace(&without_stereo, "").to_string();
    let without_leading = re("leading_color").replace(&without_color, "").to_string();
    without_leading.trim().to_string()
}

/// A declaration's `(id, label)`, per Ruby's `names` case order.
fn names(rest: &str) -> (String, String) {
    if let Some(captures) = re("names_quoted_as").captures(rest) {
        return (unbracket(&captures[2]), captures[1].to_string());
    }
    if let Some(captures) = re("names_as_quoted").captures(rest) {
        return (unbracket(&captures[1]), captures[2].to_string());
    }
    if let Some(captures) = re("names_bracket").captures(rest) {
        return (captures[1].to_string(), captures[2].to_string());
    }
    if let Some(captures) = re("names_as").captures(rest) {
        return (unbracket(&captures[2]), unbracket(&captures[1]));
    }
    let stripped = rest.strip_prefix('"').unwrap_or(rest);
    let stripped = stripped.strip_suffix('"').unwrap_or(stripped);
    let name = unbracket(stripped);
    (name.clone(), name)
}

fn unbracket(name: &str) -> String {
    let stripped = name
        .strip_prefix(['(', '[', ':'])
        .unwrap_or(name);
    let stripped = stripped.strip_suffix([')', ']', ':']).unwrap_or(stripped);
    stripped.to_string()
}

fn strip_quotes(name: &str) -> String {
    let stripped = name.strip_prefix('"').unwrap_or(name);
    let stripped = stripped.strip_suffix('"').unwrap_or(stripped);
    stripped.to_string()
}

/// `[name]`/`(name)`/`:name:` reads as its kind and inner text.
fn bracketed(reference: &str) -> (NodeKind, String) {
    let first = reference.chars().next().unwrap_or(' ');
    let inner = reference[1..reference.len().saturating_sub(1)].to_string();
    let kind = match first {
        '[' => NodeKind::Component,
        '(' => NodeKind::Usecase,
        ':' => NodeKind::Actor,
        _ => NodeKind::Component,
    };
    (kind, inner)
}

/// The body entries of a PARG `component_diagram` tree.
pub fn entry_entries(tree: &serde_json::Value) -> Vec<Entry> {
    let mut result = vec![];
    if let serde_json::Value::Array(items) = tree {
        for item in items {
            if let Some(object) = item.as_object() {
                if let Some(v) = object.get("line") {
                    let text = crate::render::pie::text_of(v).trim().to_string();
                    if !text.is_empty() {
                        let number = object
                            .get("line_number")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(result.len() as u64 + 1) as usize;
                        result.push(Entry { text, number });
                    }
                }
            }
        }
    }
    result
}
