//! Port of `activity/{parser,legacy_parser,new_parser,new_parser/blocks,
//! text,arrow_style}.rb`: the two syntaxes read into a [`Diagram`]. The
//! new syntax keeps the open ends of the flow as `(node, label)` pairs;
//! the legacy one keeps the flow's current node and ranks as PlantUML's
//! own reader does.

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::model::{unsupported, Builder, Direction, EdgeStyle, Kind, Note, Side};
use super::style::StyleReader;

/// Ruby `\s`, the whitespace set the readers' patterns share.
const WS: &str = r"[\t\n\x0B\x0C\r\x20]";

/// The legacy arrow shape.
const ARROW: &str = r"-+(?:\[[^\]]*\]|left|right|up|down)?-*>";

/// The readers' regex table, keyed as the Ruby constants.
fn table() -> &'static HashMap<&'static str, Regex> {
    static CELL: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CELL.get_or_init(|| {
        let entries: &[(&str, String)] = &[
            // Text (activity/text.rb)
            ("picture", r"(?i)<(?:img|\$|&)".to_string()),
            ("plain", r"(?i)</?plain>".to_string()),
            // Parser (activity/parser.rb)
            ("legacy_if", r#"(?i)\Aif\s+""#.to_string()),
            // LegacyParser
            ("lp_title", r"(?i)\Atitle\s+(.+)\z".to_string()),
            ("s_title", r"(?i)\Atitle\s+(.+)\z".to_string()),
            (
                "lp_if",
                format!(r#"(?i)\Aif{WS}+(?:"(?P<cond>.*)"|\((?P<paren>.*)\)){WS}+then\z"#),
            ),
            ("lp_else", r"(?i)\Aelse\z".to_string()),
            ("lp_endif", r"(?i)\Aendif\z".to_string()),
            (
                "lp_statement",
                format!(
                    "(?sxm)\\A(?P<source>\\(\\*(?:top)?\\)|\"[^\"]*\"|={{2,}}[^=]+={{2,}}|\\w+)?\n\
                     {WS}*(?P<arrow>{ARROW}){WS}*(?:\\[(?P<label>[^\\]]*)\\])?\n\
                     {WS}*(?P<dest>.+?){WS}*\\z"
                ),
            ),
            ("lp_start", r"\A\(\*(?:top)?\)\z".to_string()),
            ("bar", format!(r"\A={{2,}}{WS}*(.+?){WS}*={{2,}}\z")),
            (
                "activity",
                r#"(?sm)\A"(.*)"(?:\s+as\s+(\w+))?\z"#.to_string(),
            ),
            ("arrow_spec", r"\[([^\]]*)\]".to_string()),
            ("side", r"left|right".to_string()),
            // NewParser
            ("sprite", r"(?i)\Asprite[ \t]+\S+[ \t]*<svg".to_string()),
            ("pragma", r"\A!pragma\b".to_string()),
            ("decoration", r"(?:[ \t]*<<[^<>]*>>)+\z".to_string()),
            ("colour_prefix", r"\A#\w+:[ \t]*(.*)\z".to_string()),
            ("terminator", r";[ \t]*(?:<<[^<>]*>>)*\z".to_string()),
            ("end_note", r"(?i)\Aend\s*note\z".to_string()),
            ("sprite_ref", r"<\$\w+>".to_string()),
            ("stereo", r"<<.*?>>".to_string()),
            ("hash_word", r"#\w+".to_string()),
            // SIMPLE, in the reader's order
            ("s_start", r"(?i)\Astart\z".to_string()),
            ("s_stop", r"(?i)\Astop\z".to_string()),
            ("s_end", r"(?i)\Aend\z".to_string()),
            ("s_end_group", r"(?i)\Aend\s*group\z".to_string()),
            ("s_kill", r"(?i)\Akill\z".to_string()),
            (
                "s_action",
                r"(?sm)\A(?P<colour>#\w+)?:(?P<text>.*)\z".to_string(),
            ),
            (
                "s_arrow",
                r"(?sm)\A-+(?:\[(?P<spec>[^\]]*)\])?-*>\s*(?P<text>.*)\z".to_string(),
            ),
            (
                "s_lane",
                r"\A\|(?:(?P<colour>#\w+)\|)?(?P<name>[^|]+)\|\z".to_string(),
            ),
            (
                "s_note",
                r"(?isx)\A(?:floating[\x20\t]+)?note[\x20\t]+(?:floating[\x20\t]+)?
                   (?P<side>left|right)[\x20\t]*(?:of[\x20\t]+\S+[\x20\t]*)?
                   (?::(?P<text>.*))?\z"
                    .to_string(),
            ),
            (
                "s_partition",
                r"(?i)\A(?:partition|group)\s+(?P<name>.*?)(?:\s*\{)?\z".to_string(),
            ),
            ("s_close", r"\A\}\z".to_string()),
            // Blocks::PATTERNS, in order
            (
                "b_if",
                r"(?isx)\A(?P<kw>if|elseif)\s*\((?P<cond>.*?)\)\s*
                   (?:is\s*\((?P<is>.*?)\)\s*)?
                   then\s*(?:\((?P<then>.*)\))?\z"
                    .to_string(),
            ),
            (
                "b_else",
                r"(?i)\Aelse\s*(?:\((?P<label>.*)\))?\z".to_string(),
            ),
            ("b_endif", r"(?i)\Aend\s*if\z".to_string()),
            (
                "b_while",
                r"(?i)\Awhile\s*\((?P<cond>.*?)\)\s*(?:is\s*\((?P<is>.*?)\))?\z".to_string(),
            ),
            (
                "b_endwhile",
                r"(?i)\Aend\s*while\s*(?:\((?P<label>.*)\))?\z".to_string(),
            ),
            (
                "b_repeat",
                r"(?ism)\Arepeat(?:[ \t]*:(?P<text>.*))?\z".to_string(),
            ),
            (
                "b_repeat_while",
                r"(?isx)\Arepeat\s+while\s*\((?P<cond>.*?)\)\s*
                   (?:is\s*\((?P<is>.*?)\)\s*)?
                   (?:not\s*\((?P<not>.*?)\))?\z"
                    .to_string(),
            ),
            ("b_fork", r"(?i)\Afork\z".to_string()),
            ("b_fork_again", r"(?i)\Afork\s+again\z".to_string()),
            (
                "b_end_fork",
                r"(?i)\Aend\s+(?P<word>fork|merge)(?:\s*\{.*\})?\z".to_string(),
            ),
            ("b_switch", r"(?i)\Aswitch\s*\((?P<cond>.*)\)\z".to_string()),
            ("b_case", r"(?i)\Acase\s*\((?P<label>.*)\)\z".to_string()),
            ("b_endswitch", r"(?i)\Aend\s*switch\z".to_string()),
        ];
        entries
            .iter()
            .map(|(name, pattern)| (*name, Regex::new(pattern).expect("activity reader pattern")))
            .collect()
    })
}

fn re(name: &str) -> &'static Regex {
    table().get(name).expect("activity reader pattern")
}

// ---- Text (activity/text.rb) ----

/// `Text.clean`: the label's lines. Creole styling stays in the line; an
/// embedded image or sprite is refused, never dropped.
pub fn text_clean(raw: &str, line: usize) -> Result<String, crate::Error> {
    if re("picture").is_match(raw) {
        return Err(unsupported("image", line, raw));
    }
    let replaced = raw.replace("\\n", "\n");
    let stripped = re("plain").replace_all(&replaced, "").to_string();
    let lines: Vec<String> = stripped
        .split('\n')
        .map(|line| line.trim().to_string())
        .collect();
    Ok(lines.join("\n").trim().to_string())
}

// ---- ArrowStyle (activity/arrow_style.rb) ----

/// The arrow style words and the style each names.
fn style_word(word: &str) -> Option<Option<EdgeStyle>> {
    match word.to_lowercase().as_str() {
        "dashed" => Some(Some(EdgeStyle::Dashed)),
        "dotted" => Some(Some(EdgeStyle::Dotted)),
        "bold" => Some(Some(EdgeStyle::Bold)),
        "plain" => Some(None),
        _ => None,
    }
}

/// `ArrowStyle.colour`: `#pink` is a colour name, `#ccf` and `#33668E`
/// are hex.
pub fn arrow_colour(word: &str) -> Option<String> {
    let name = word.strip_prefix('#').unwrap_or(word);
    if name.is_empty() {
        return None;
    }
    let hex = (name.len() == 3 || name.len() == 6) && name.chars().all(|c| c.is_ascii_hexdigit());
    Some(if hex {
        format!("#{name}")
    } else {
        name.to_string()
    })
}

/// `ArrowStyle.parse`: the `[#blue,dashed]` part of an arrow, as its
/// colour and style.
fn arrow_style(
    spec: Option<&str>,
    line: usize,
) -> Result<(Option<String>, Option<EdgeStyle>), crate::Error> {
    let mut colour = None;
    let mut style = None;
    // Ruby's `"".split(",")` is an empty list, not one empty word.
    let words = match spec {
        Some(spec) if !spec.is_empty() => spec.split(',').map(str::trim).collect::<Vec<_>>(),
        _ => vec![],
    };
    for word in words {
        if word.starts_with('#') {
            colour = arrow_colour(word);
        } else if let Some(found) = style_word(word) {
            style = found;
        } else {
            return Err(unsupported(
                &format!("arrow style {word:?}"),
                line,
                spec.unwrap_or_default(),
            ));
        }
    }
    Ok((colour, style))
}

// ---- entries (activity/parser.rb) ----

/// `Parser.entries`: each significant line, stripped, with its 1-based
/// number. `skinparam` and `<style>` lines belong to the style reader.
pub fn entries(source: &str) -> Vec<(String, usize)> {
    let texts: Vec<String> = source.lines().map(|line| line.trim().to_string()).collect();
    let styling = StyleReader::styling(&texts);
    texts
        .iter()
        .enumerate()
        .filter(|(index, text)| {
            !text.is_empty()
                && !text.starts_with('\'')
                && !text.starts_with("@start")
                && !text.starts_with("@end")
                && !styling[*index]
        })
        .map(|(index, text)| (text.clone(), index + 1))
        .collect()
}

/// `Parser.legacy?`: whether any line is legacy-only syntax.
pub fn legacy(lines: &[(String, usize)]) -> bool {
    lines
        .iter()
        .any(|(line, _)| line.contains("(*") || re("legacy_if").is_match(line))
}

/// `Parser.parse`: read the source in whichever syntax it is written.
pub fn parse(source: &str) -> Result<super::model::Diagram, crate::Error> {
    let parsed = entries(source);
    if legacy(&parsed) {
        LegacyParser::new().parse(parsed)
    } else {
        NewParser::new().parse(parsed)
    }
}

/// The `SIMPLE` statements, then `Blocks::PATTERNS`, in the reader's
/// order: the first pattern that matches the line names the rule.
fn statement_hit(line: &str) -> Option<&'static str> {
    let simple = [
        "s_start",
        "s_stop",
        "s_end",
        "s_end_group",
        "s_kill",
        "s_title",
        "s_action",
        "s_arrow",
        "s_lane",
        "s_note",
        "s_partition",
        "s_close",
    ];
    for name in simple {
        if re(name).captures(line).is_some() {
            return Some(name);
        }
    }
    blocks_hit(line)
}

/// The `Blocks::PATTERNS` statements, in order.
fn blocks_hit(line: &str) -> Option<&'static str> {
    const BLOCKS: [&str; 13] = [
        "b_if",
        "b_else",
        "b_endif",
        "b_while",
        "b_endwhile",
        "b_repeat",
        "b_repeat_while",
        "b_fork",
        "b_fork_again",
        "b_end_fork",
        "b_switch",
        "b_case",
        "b_endswitch",
    ];
    for name in BLOCKS {
        if re(name).captures(line).is_some() {
            return Some(name);
        }
    }
    None
}

// ---- LegacyParser (activity/legacy_parser.rb) ----

struct LegacyParser {
    builder: Builder,
    current: Option<usize>,
    stop: Option<usize>,
    frames: Vec<usize>,
    names: HashMap<String, usize>,
    labels: HashMap<String, usize>,
    bars: HashMap<String, usize>,
    number: usize,
    line: String,
}

impl LegacyParser {
    fn new() -> Self {
        LegacyParser {
            builder: Builder::new(),
            current: None,
            stop: None,
            frames: vec![],
            names: Default::default(),
            labels: Default::default(),
            bars: Default::default(),
            number: 0,
            line: String::new(),
        }
    }

    /// `LegacyParser#parse`.
    fn parse(
        mut self,
        entries: Vec<(String, usize)>,
    ) -> Result<super::model::Diagram, crate::Error> {
        for (line, number) in join_quoted(entries) {
            self.statement(&line, number)?;
        }
        if !self.frames.is_empty() {
            return Err(unsupported("unclosed if", self.number, ""));
        }
        let mut diagram = self.builder.into_diagram();
        diagram.legacy = true;
        Ok(diagram)
    }

    fn statement(&mut self, line: &str, number: usize) -> Result<(), crate::Error> {
        self.number = number;
        self.line = line.to_string();
        if let Some(found) = re("lp_title").captures(line) {
            self.builder.diagram.title = Some(text_clean(&found[1], number)?);
            return Ok(());
        }
        if let Some(found) = re("lp_if").captures(line) {
            return self.open_if(&found);
        }
        if re("lp_else").is_match(line) {
            return self.open_else();
        }
        if re("lp_endif").is_match(line) {
            return self.close_if();
        }
        if let Some(found) = re("lp_statement").captures(line) {
            return self.flow(&found);
        }
        Err(unsupported("statement", number, line))
    }

    fn open_if(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let written = found
            .name("cond")
            .or_else(|| found.name("paren"))
            .map(|m| m.as_str())
            .unwrap_or_default();
        let text = text_clean(written, self.number)?;
        let decision = self.builder.node(Kind::Decision, Some(text), None, true);
        self.builder.diagram.nodes[decision].legacy = true;
        if let Some(current) = self.current {
            self.builder.connect(current, decision, None, None, None);
        }
        self.frames.push(decision);
        self.current = Some(decision);
        Ok(())
    }

    fn open_else(&mut self) -> Result<(), crate::Error> {
        self.current = Some(self.top()?);
        Ok(())
    }

    /// The flow carries on from the last node the closing branch drew.
    fn close_if(&mut self) -> Result<(), crate::Error> {
        self.top()?;
        self.frames.pop();
        Ok(())
    }

    fn flow(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let from = self.source_of(found.name("source").map(|m| m.as_str()))?;
        let spec = re("arrow_spec")
            .captures(&found["arrow"])
            .and_then(|m| m.get(1))
            .map(|m| m.as_str());
        let (colour, style) = arrow_style(spec, self.number)?;
        let dest = self.destination(found.name("dest").map(|m| m.as_str()).unwrap_or(""))?;
        let label = found
            .name("label")
            .map(|m| text_clean(m.as_str(), self.number))
            .transpose()?;
        let edge = self.builder.connect(from, dest, label, colour, style);
        self.builder.diagram.edges[edge].direction = direction_of(&found["arrow"]);
        self.current = Some(dest);
        Ok(())
    }

    /// The node an arrow leaves: the one written, else wherever the flow
    /// is.
    fn source_of(&mut self, written: Option<&str>) -> Result<usize, crate::Error> {
        let Some(written) = written else {
            return self
                .current
                .ok_or_else(|| unsupported("arrow without a source", self.number, &self.line));
        };
        self.resolve(written)
    }

    fn resolve(&mut self, source: &str) -> Result<usize, crate::Error> {
        if re("lp_start").is_match(source) {
            return Ok(self.builder.node(Kind::Start, None, None, false));
        }
        if source.starts_with('"') {
            return self.activity(source);
        }
        if let Some(found) = re("bar").captures(source) {
            return Ok(self.bar(&found[1]));
        }
        match self.names.get(source).copied() {
            Some(node) => Ok(node),
            None => self.implicit_activity(source),
        }
    }

    fn destination(&mut self, text: &str) -> Result<usize, crate::Error> {
        if re("lp_start").is_match(text) {
            return Ok(self.stop.unwrap_or_else(|| {
                let node = self.builder.node(Kind::Stop, None, None, false);
                self.stop = Some(node);
                node
            }));
        }
        if let Some(found) = re("bar").captures(text) {
            return Ok(self.bar(&found[1]));
        }
        if text.starts_with('"') {
            return self.activity(text);
        }
        match self.names.get(text).copied() {
            Some(node) => Ok(node),
            None => self.implicit_activity(text),
        }
    }

    /// A name a transition mentions but no line declared is an activity
    /// with that name, as PlantUML draws it.
    fn implicit_activity(&mut self, name: &str) -> Result<usize, crate::Error> {
        let node = self.action(name)?;
        self.names.insert(name.to_string(), node);
        Ok(node)
    }

    fn activity(&mut self, text: &str) -> Result<usize, crate::Error> {
        let Some(found) = re("activity").captures(text) else {
            return Err(unsupported("activity", self.number, text));
        };
        match found.get(2) {
            None => {
                let key = found[1].to_string();
                match self.labels.get(&key).copied() {
                    Some(node) => Ok(node),
                    None => {
                        let node = self.action(&found[1])?;
                        self.labels.insert(key, node);
                        Ok(node)
                    }
                }
            }
            Some(alias) => {
                let node = self.action(&found[1])?;
                self.names.insert(alias.as_str().to_string(), node);
                Ok(node)
            }
        }
    }

    fn action(&mut self, text: &str) -> Result<usize, crate::Error> {
        let cleaned = text_clean(text, self.number)?;
        Ok(self.builder.node(Kind::Action, Some(cleaned), None, false))
    }

    fn bar(&mut self, name: &str) -> usize {
        match self.bars.get(name).copied() {
            Some(node) => node,
            None => {
                let node = self
                    .builder
                    .node(Kind::Bar, Some(name.to_string()), None, false);
                self.builder.diagram.nodes[node].legacy = true;
                self.bars.insert(name.to_string(), node);
                node
            }
        }
    }

    fn top(&self) -> Result<usize, crate::Error> {
        self.frames.last().copied().ok_or_else(|| {
            unsupported(
                &format!(
                    "{} outside if",
                    self.line.split_whitespace().next().unwrap_or_default()
                ),
                self.number,
                &self.line,
            )
        })
    }
}

/// `LegacyParser.join_quoted`: a quoted activity may run over several
/// lines.
fn join_quoted(entries: Vec<(String, usize)>) -> Vec<(String, usize)> {
    let mut joined: Vec<(String, usize)> = vec![];
    for (line, number) in entries {
        match joined.last_mut() {
            Some(open) if open.0.matches('"').count() % 2 == 1 => {
                open.0.push_str(&format!("\n{line}"));
            }
            _ => joined.push((line, number)),
        }
    }
    joined
}

/// `LegacyParser#direction`: `->` lies along the row to the right; a
/// `-left->` or `-right->` names its own side.
fn direction_of(arrow: &str) -> Option<Direction> {
    if arrow == "->" {
        return Some(Direction::Right);
    }
    re("side")
        .find(arrow)
        .and_then(|found| match found.as_str() {
            "left" => Some(Direction::Left),
            "right" => Some(Direction::Right),
            _ => None,
        })
}

// ---- NewParser (activity/new_parser.rb) ----

/// One frame of an open branching construct.
#[derive(Default)]
struct Frame {
    kind: &'static str,
    branches: Vec<(usize, Option<String>)>,
    decision: usize,
    else_seen: bool,
    start: usize,
    head: usize,
}

pub struct NewParser {
    builder: Builder,
    ends: Vec<(usize, Option<String>)>,
    frames: Vec<Frame>,
    /// The pending arrow style: colour, label and style.
    arrow: (Option<String>, Option<String>, Option<EdgeStyle>),
    entries: std::vec::IntoIter<(String, usize)>,
    number: usize,
    line: String,
}

impl NewParser {
    pub fn new() -> Self {
        NewParser {
            builder: Builder::new(),
            ends: vec![],
            frames: vec![],
            arrow: (None, None, None),
            entries: vec![].into_iter(),
            number: 0,
            line: String::new(),
        }
    }

    /// `NewParser#parse`.
    pub fn parse(
        mut self,
        entries: Vec<(String, usize)>,
    ) -> Result<super::model::Diagram, crate::Error> {
        self.entries = entries.into_iter();
        while let Some((line, number)) = self.entries.next() {
            self.statement(&line, number)?;
        }
        if let Some(frame) = self.frames.last() {
            return Err(unsupported(
                &format!("unclosed {}", frame.kind),
                self.number,
                &self.line,
            ));
        }
        Ok(self.builder.into_diagram())
    }

    fn statement(&mut self, line: &str, number: usize) -> Result<(), crate::Error> {
        self.line = line.to_string();
        self.number = number;
        // The deprecated `#colour:` prefix on a construct line: the
        // structure is what matters, so a coloured construct gets its
        // block handler with the colour dropped.
        if let Some(rest) = re("colour_prefix").captures(line) {
            let rest = rest
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            if let Some(name) = blocks_hit(&rest) {
                let found = re(name).captures(&rest).expect("just matched");
                return self.dispatch(name, &found);
            }
        }
        if re("sprite").is_match(line) {
            return self.skip_sprite(line);
        }
        if re("pragma").is_match(line) {
            return Ok(());
        }
        if self.match_statement(line)? {
            return Ok(());
        }
        // A trailing `<<colour>>` on a branching line colours the arm's
        // arrow; it carries no structure, so decorated lines get a second
        // match without it.
        let undecorated = re("decoration").replace_all(line, "").to_string();
        if self.match_statement(&undecorated)? {
            return Ok(());
        }
        Err(unsupported("statement", number, line))
    }

    /// Icons are dropped, the rest renders.
    fn skip_sprite(&mut self, line: &str) -> Result<(), crate::Error> {
        let mut line = line.to_string();
        while !line.contains("</svg>") {
            line = self.next_line()?.0;
        }
        Ok(())
    }

    fn match_statement(&mut self, line: &str) -> Result<bool, crate::Error> {
        match statement_hit(line) {
            Some(name) => {
                let found = re(name).captures(line).expect("just matched");
                self.dispatch(name, &found)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn dispatch(
        &mut self,
        name: &'static str,
        found: &regex::Captures<'_>,
    ) -> Result<(), crate::Error> {
        match name {
            "s_start" => self.on_start(),
            "s_stop" => self.on_stop(),
            "s_end" => self.on_end(),
            "s_end_group" | "s_close" => self.on_close_partition(),
            "s_kill" => self.on_kill(),
            "s_title" | "lp_title" => self.on_title(found),
            "s_action" => self.on_action(found),
            "s_arrow" => self.on_arrow(found),
            "s_lane" => self.on_lane(found),
            "s_note" => self.on_note(found),
            "s_partition" => self.on_partition(found),
            "b_if" => self.on_if(found),
            "b_else" => self.on_else(found),
            "b_endif" => self.on_endif(),
            "b_while" => self.on_while(found),
            "b_endwhile" => self.on_endwhile(found),
            "b_repeat" => self.on_repeat(found),
            "b_repeat_while" => self.on_repeat_while(found),
            "b_fork" => self.on_fork(),
            "b_fork_again" => self.on_fork_again(),
            "b_end_fork" => self.on_end_fork(found),
            "b_switch" => self.on_switch(found),
            "b_case" => self.on_case(found),
            "b_endswitch" => self.on_endswitch(),
            other => panic!("no such statement {other}"),
        }
    }

    fn on_start(&mut self) -> Result<(), crate::Error> {
        let node = self.builder.node(Kind::Start, None, None, false);
        self.add(node);
        Ok(())
    }

    fn on_stop(&mut self) -> Result<(), crate::Error> {
        let node = self.builder.node(Kind::Stop, None, None, false);
        self.add(node);
        self.ends = vec![];
        Ok(())
    }

    /// `end` ends the flow at a crossed circle.
    fn on_end(&mut self) -> Result<(), crate::Error> {
        let node = self.builder.node(Kind::End, None, None, false);
        self.add(node);
        self.ends = vec![];
        Ok(())
    }

    /// `kill` ends the flow at a cross, as `stop` ends it at a circle.
    fn on_kill(&mut self) -> Result<(), crate::Error> {
        let node = self.builder.node(Kind::Kill, None, None, false);
        self.add(node);
        self.ends = vec![];
        Ok(())
    }

    fn on_title(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let title = self.clean(&found[1])?;
        self.builder.diagram.title = Some(title);
        Ok(())
    }

    fn on_action(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let collected = self.collect(found.name("text").map(|m| m.as_str()).unwrap_or_default())?;
        let text = self.clean(&collected)?;
        if let Some(colour) = found.name("colour") {
            // PlantUML draws no colour from the old `#colour:` prefix; it
            // prints a notice that names the suffix to use instead.
            self.builder.warn(&format!(
                "This syntax is deprecated, you must add <<{}>> at the end of the line, after the ';'",
                colour.as_str()
            ));
        }
        let node = self.builder.node(Kind::Action, Some(text), None, false);
        self.add(node);
        Ok(())
    }

    fn on_arrow(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let spec = found.name("spec").map(|m| m.as_str());
        let (colour, style) = arrow_style(spec, self.number)?;
        let collected = self.collect(found.name("text").map(|m| m.as_str()).unwrap_or_default())?;
        let label = if collected.is_empty() {
            None
        } else {
            Some(text_clean(&collected, self.number)?)
        };
        self.arrow = (colour, label, style);
        Ok(())
    }

    fn on_lane(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let colour = found.name("colour").and_then(|m| arrow_colour(m.as_str()));
        let name = found.name("name").map(|m| m.as_str()).unwrap_or_default();
        self.builder.lane(name.trim(), colour);
        Ok(())
    }

    fn on_note(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let text = match found.name("text") {
            Some(text) => text.as_str().trim().to_string(),
            None => self.collect_note()?,
        };
        let side = match found
            .name("side")
            .map(|m| m.as_str().to_lowercase())
            .as_deref()
        {
            Some("left") => Side::Left,
            _ => Side::Right,
        };
        let Some(node) = self.builder.diagram.nodes.len().checked_sub(1) else {
            return Err(unsupported(
                "note without a preceding activity",
                self.number,
                &self.line,
            ));
        };
        let cleaned = self.clean(&text)?;
        self.builder.diagram.nodes[node].notes.push(Note {
            side,
            text: cleaned,
        });
        Ok(())
    }

    fn on_partition(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let name = found.name("name").map(|m| m.as_str()).unwrap_or_default();
        let title = re("hash_word").replace_all(name, "");
        let title = re("stereo").replace_all(&title, "");
        let title = title.replace('"', "").trim().to_string();
        self.builder
            .partition(Some(&title), self.number, &self.line)
    }

    fn on_close_partition(&mut self) -> Result<(), crate::Error> {
        self.builder.partition(None, 0, "")
    }

    /// Text that runs to the next line ending in `;`. A deprecated
    /// `<<colour>>` decoration may follow the semicolon.
    fn collect(&mut self, first: &str) -> Result<String, crate::Error> {
        let mut text = first.to_string();
        while !re("terminator").is_match(&text) {
            let (line, _) = self.next_line()?;
            text.push('\n');
            text.push_str(&line);
        }
        let start = re("terminator")
            .find(&text)
            .map(|m| m.start())
            .unwrap_or(text.len());
        Ok(text[..start].to_string())
    }

    fn collect_note(&mut self) -> Result<String, crate::Error> {
        let mut lines: Vec<String> = vec![];
        loop {
            let Some((line, _)) = self.entries.next() else {
                return Err(unsupported(
                    "unterminated activity (missing `;`)",
                    self.number,
                    &self.line,
                ));
            };
            if re("end_note").is_match(&line) {
                break;
            }
            lines.push(line);
        }
        Ok(lines.join("\n"))
    }

    fn next_line(&mut self) -> Result<(String, usize), crate::Error> {
        match self.entries.next() {
            Some(entry) => Ok(entry),
            None => Err(unsupported(
                "unterminated activity (missing `;`)",
                self.number,
                &self.line,
            )),
        }
    }

    /// Joins every open end to the new node, then makes it the only open
    /// end.
    fn add(&mut self, node: usize) {
        let ends = std::mem::take(&mut self.ends);
        let (colour, label, style) = std::mem::take(&mut self.arrow);
        for (from, end_label) in ends {
            let label = label.clone().or(end_label);
            self.builder
                .connect(from, node, label, colour.clone(), style);
        }
        self.ends = vec![(node, None)];
    }

    fn clean(&mut self, text: &str) -> Result<String, crate::Error> {
        let stripped = re("sprite_ref").replace_all(text, "").to_string();
        if stripped != text {
            self.builder.warn("sprite dropped from the label");
        }
        text_clean(&stripped, self.number)
    }

    fn decision_node(&mut self, found: &regex::Captures<'_>) -> Result<usize, crate::Error> {
        let cond = found.name("cond").map(|m| m.as_str()).unwrap_or_default();
        let text = self.clean(cond)?;
        Ok(self.builder.node(Kind::Decision, Some(text), None, false))
    }

    // ---- Blocks (activity/new_parser/blocks.rb) ----

    fn on_if(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let written = found
            .name("then")
            .or_else(|| found.name("is"))
            .map(|m| m.as_str());
        let label = self.label_text(written)?;
        if !found["kw"].eq_ignore_ascii_case("if") {
            return self.elseif(label, found);
        }
        let decision = self.decision_node(found)?;
        self.add(decision);
        self.frames.push(Frame {
            kind: "if",
            decision,
            ..Default::default()
        });
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn elseif(
        &mut self,
        label: Option<String>,
        found: &regex::Captures<'_>,
    ) -> Result<(), crate::Error> {
        let frame = self.top("if")?;
        let previous = self.frames[frame].decision;
        self.frames[frame].branches.extend(self.ends.drain(..));
        let decision = self.decision_node(found)?;
        self.builder.connect(previous, decision, None, None, None);
        self.frames[frame].decision = decision;
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_else(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let frame = self.top("if")?;
        self.frames[frame].branches.extend(self.ends.drain(..));
        self.frames[frame].else_seen = true;
        let label = self.label_text(found.name("label").map(|m| m.as_str()))?;
        let decision = self.frames[frame].decision;
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_endif(&mut self) -> Result<(), crate::Error> {
        let frame = self.top("if")?;
        let (branches, decision, else_seen, lane) = {
            let frame = &self.frames[frame];
            (
                frame.branches.clone(),
                frame.decision,
                frame.else_seen,
                self.builder.diagram.nodes[frame.decision].lane.clone(),
            )
        };
        self.frames.pop();
        let mut branches = branches;
        branches.extend(self.ends.drain(..));
        if !else_seen {
            branches.push((decision, None));
        }
        self.join(branches, Kind::Merge, lane);
        Ok(())
    }

    fn on_while(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let decision = self.decision_node(found)?;
        self.add(decision);
        let label = self.label_text(found.name("is").map(|m| m.as_str()))?;
        self.frames.push(Frame {
            kind: "while",
            decision,
            ..Default::default()
        });
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_endwhile(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let frame = self.top("while")?;
        let decision = self.frames[frame].decision;
        self.frames.pop();
        for (from, label) in self.ends.drain(..) {
            self.builder.connect(from, decision, label, None, None);
        }
        let label = self.label_text(found.name("label").map(|m| m.as_str()))?;
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_repeat(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let merge = self.builder.node(Kind::Merge, None, None, false);
        self.add(merge);
        self.frames.push(Frame {
            kind: "repeat",
            start: merge,
            ..Default::default()
        });
        let Some(text) = found.name("text") else {
            return Ok(());
        };
        // `repeat :action;` puts the action on the loop's backward path;
        // the flow enters it right after the loop's head.
        let collected = self.collect(text.as_str())?;
        let text = self.clean(&collected)?;
        let node = self.builder.node(Kind::Action, Some(text), None, false);
        self.add(node);
        Ok(())
    }

    fn on_repeat_while(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let frame = self.top("repeat")?;
        let start = self.frames[frame].start;
        self.frames.pop();
        let decision = self.decision_node(found)?;
        self.add(decision);
        let label = self.label_text(found.name("is").map(|m| m.as_str()))?;
        self.builder.connect(decision, start, label, None, None);
        let label = self.label_text(found.name("not").map(|m| m.as_str()))?;
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_fork(&mut self) -> Result<(), crate::Error> {
        let head = self.builder.node(Kind::Bar, None, None, false);
        self.add(head);
        self.frames.push(Frame {
            kind: "fork",
            head,
            ..Default::default()
        });
        Ok(())
    }

    fn on_fork_again(&mut self) -> Result<(), crate::Error> {
        let frame = self.top("fork")?;
        self.frames[frame].branches.extend(self.ends.drain(..));
        let head = self.frames[frame].head;
        self.ends = vec![(head, None)];
        Ok(())
    }

    fn on_end_fork(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let frame = self.top("fork")?;
        let branches = self.frames[frame].branches.clone();
        self.frames.pop();
        let mut branches = branches;
        branches.extend(self.ends.drain(..));
        let kind = match found
            .name("word")
            .map(|m| m.as_str().to_lowercase())
            .as_deref()
        {
            Some("merge") => Kind::Merge,
            _ => Kind::Bar,
        };
        self.join(branches, kind, None);
        Ok(())
    }

    fn on_switch(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let decision = self.decision_node(found)?;
        self.add(decision);
        self.frames.push(Frame {
            kind: "switch",
            decision,
            ..Default::default()
        });
        self.ends = vec![];
        Ok(())
    }

    fn on_case(&mut self, found: &regex::Captures<'_>) -> Result<(), crate::Error> {
        let frame = self.top("switch")?;
        self.frames[frame].branches.extend(self.ends.drain(..));
        let label = self.label_text(found.name("label").map(|m| m.as_str()))?;
        let decision = self.frames[frame].decision;
        self.ends = vec![(decision, label)];
        Ok(())
    }

    fn on_endswitch(&mut self) -> Result<(), crate::Error> {
        let frame = self.top("switch")?;
        let branches = self.frames[frame].branches.clone();
        let lane = self.builder.diagram.nodes[self.frames[frame].decision]
            .lane
            .clone();
        self.frames.pop();
        let mut branches = branches;
        branches.extend(self.ends.drain(..));
        self.join(branches, Kind::Merge, lane);
        Ok(())
    }

    /// Closes a branching construct: many open ends become one merge
    /// node, one stays as it is, none means the flow ended. The diamond
    /// stays in the swimlane of the decision it closes.
    fn join(&mut self, branches: Vec<(usize, Option<String>)>, kind: Kind, lane: Option<String>) {
        if branches.len() < 2 {
            self.ends = branches;
            return;
        }
        let merge = self.builder.node(kind, None, None, false);
        if lane.is_some() {
            self.builder.diagram.nodes[merge].lane = lane;
        }
        for (from, label) in branches {
            self.builder.connect(from, merge, label, None, None);
        }
        self.ends = vec![(merge, None)];
    }

    fn label_text(&mut self, text: Option<&str>) -> Result<Option<String>, crate::Error> {
        match text {
            Some(text) if !text.trim().is_empty() => self.clean(text).map(Some),
            _ => Ok(None),
        }
    }

    fn top(&self, kind: &str) -> Result<usize, crate::Error> {
        match self.frames.last() {
            Some(frame) if frame.kind == kind => Ok(self.frames.len() - 1),
            _ => Err(unsupported(
                &format!(
                    "{} outside {kind}",
                    self.line.split_whitespace().next().unwrap_or_default()
                ),
                self.number,
                &self.line,
            )),
        }
    }
}

impl Default for NewParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_action_flow() {
        let diagram = parse("@startuml\nstart\n:foo1;\nstop\n@enduml\n").unwrap();
        assert_eq!(diagram.nodes.len(), 3);
        assert_eq!(diagram.edges.len(), 2);
        assert!(!diagram.legacy);
    }

    #[test]
    fn legacy_syntax_is_detected() {
        let diagram = parse("@startuml\n(*) --> \"Do it\"\n(*) --> \"Other\"\n@enduml\n").unwrap();
        assert!(diagram.legacy);
        // Each `(*)` source is a fresh start node; PlantUML ranks them so.
        assert_eq!(diagram.nodes.len(), 4);
    }

    #[test]
    fn branch_construct_joins() {
        let diagram = parse(
            "@startuml\nstart\nif (c?) then (yes)\n  :A;\nelse (no)\n  :B;\nendif\nstop\n@enduml\n",
        )
        .unwrap();
        // start, decision, A, B, merge, stop
        assert_eq!(diagram.nodes.len(), 6);
    }

    #[test]
    fn deprecated_colour_prefix_warns_on_actions_only() {
        let diagram = parse("@startuml\n#F2F2F2:X;\n@enduml\n").unwrap();
        assert_eq!(diagram.warnings.len(), 1);
        let diagram =
            parse("@startuml\n#red:repeat\n:A;\nrepeat while (more?)\n@enduml\n").unwrap();
        assert!(diagram.warnings.is_empty());
    }
}
