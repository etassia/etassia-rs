//! Reads PlantUML state-diagram source into the state model. Port of
//! `lib/etassia/notation/plantuml/state/reader.rb`, regex for regex.
//!
//! A state belongs to the scope that first mentions it. `[*]` is the start
//! of the scope when it begins a transition and the end of the scope when
//! it ends one; `[H]` and `[H*]` are the scope's shallow and deep history,
//! or those of the state they follow. `scale` only resizes PlantUML's
//! output. A line outside this subset refuses the native path, the way the
//! Ruby reader raises `UnsupportedConstructError`.

use crate::Error;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The diagram the reader produces (Ruby `Diagram::StateDiagram`).
#[derive(Default)]
pub struct Diagram {
    pub states: Vec<Node>,
    pub transitions: Vec<Transition>,
    pub notes: Vec<Note>,
    pub direction: String,
    /// The source and target ids of each `->`/`-right->` transition, which
    /// PlantUML draws on one rank.
    pub flat: Vec<(String, String)>,
}

/// A state of the diagram. An id of `""` is Ruby's `nil` id — the one a
/// quoted-only declaration carries.
#[derive(Clone, Default)]
pub struct Node {
    pub id: String,
    pub label: Option<String>,
    pub state_type: String,
    pub description: Option<String>,
    pub descriptions: Vec<String>,
    pub children: Vec<Node>,
}

/// A transition of the diagram.
pub struct Transition {
    pub from_id: String,
    pub to_id: String,
    pub trigger: Option<String>,
}

/// A note attached to a state.
pub struct Note {
    pub id: String,
    pub text: String,
    pub target_id: String,
    pub position: String,
}

/// What a `scale` line sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleAxis {
    Width,
    Height,
    Factor,
}

fn unported(text: &str) -> Error {
    Error::Unported(format!("state_diagram: state statement, {text:?}"))
}

// ---- regex table (the reader's constants) ----

fn re(name: &str) -> &'static Regex {
    static CELL: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CELL.get_or_init(|| {
        let name = "[A-Za-z_][A-Za-z0-9_.]*";
        let mark = r"\[(?:\*|H\*?)\]";
        let entries: Vec<(&str, String)> = vec![
            ("state_open", format!(
                r#"(?i)\Astate[ \t]+(?:"([^"]+)"(?:[ \t]+as[ \t]+({name}))?|({name}))(?:[ \t]*<<([^<>]+)>>)?(?:[ \t]*(\$\w+))?(?:[ \t]*(\#\S+))?(?:[ \t]*(\{{))?(?:[ \t]*:[ \t]*(.+))?\z"#
            )),
            ("transition", format!(
                r"\A({name}({mark})?|{mark})[ \t]*(-+(?:[a-z]+-+)?>)[ \t]*({name}({mark})?|{mark})(?:[ \t]*:[ \t]*(.+))?\z"
            )),
            ("reversed", format!(
                r"\A({name}({mark})?|{mark})[ \t]*<-+[ \t]*({name}({mark})?|{mark})(?:[ \t]*:[ \t]*(.+))?\z"
            )),
            ("note", format!(
                r"(?i)\Anote[ \t]+(left|right|top|bottom)[ \t]+of[ \t]+({name})(?:[ \t]*:[ \t]*(.+))?\z"
            )),
            ("note_as", format!(r"(?i)\Anote[ \t]+as[ \t]+({name})\z")),
            ("end_note", r"(?i)\Aend[ \t]*note\z".to_string()),
            ("description", format!(r"\A({name})[ \t]*:[ \t]*(.+)\z")),
            ("style_block", r"(?i)\A<style>\z".to_string()),
            ("end_style", r"(?i)\A</style>\z".to_string()),
            ("skin_block", r"(?i)\Askinparam[ \t]+\S+[ \t]*\{\z".to_string()),
            ("skipped", r"(?i)\A(?:scale\b|title\b|hide\b|remove\b|restore\b|skinparam\b|!pragma\b|left to right direction\z|top to bottom direction\z)".to_string()),
            ("scale", r"(?i)\Ascale[ \t]+(\d+(?:\.\d+)?)(?:[ \t]+(width|height))?\z".to_string()),
            ("mark", format!(r"{mark}")),
            ("flat", r"\A-(?:right-)?>\z".to_string()),
            ("close_skin", r"\A\}\z".to_string()),
        ];
        entries
            .into_iter()
            .map(|(name, pattern)| (name, Regex::new(&pattern).expect("state reader pattern")))
            .collect()
    })
    .get(name)
    .expect("state reader pattern")
}

/// The stripped, non-empty, non-comment lines of the first diagram, with
/// their numbers (Ruby `Reader.lines_of`).
pub fn lines_of(source: &str) -> Vec<(String, usize)> {
    let numbered: Vec<(&str, usize)> =
        source.lines().enumerate().map(|(i, l)| (l.trim(), i + 1)).collect();
    let start = numbered.iter().position(|(line, _)| line.starts_with("@start"));
    let Some(start) = start else { return vec![] };
    numbered[start + 1..]
        .iter()
        .take_while(|(line, _)| !line.starts_with("@end"))
        .filter(|(line, _)| !line.is_empty() && !line.starts_with('\''))
        .map(|(line, n)| (line.to_string(), *n))
        .collect()
}

/// The number of a `scale` line and what it sizes (Ruby `Reader.scale_of`).
pub fn scale_of(source: &str) -> Option<(f64, ScaleAxis)> {
    lines_of(source)
        .iter()
        .filter_map(|(line, _)| re("scale").captures(line))
        .next()
        .map(|m| {
            let value: f64 = m[1].parse().unwrap_or(0.0);
            let axis = m.get(2).map(|g| g.as_str().to_lowercase()).unwrap_or_else(|| "factor".to_string());
            let axis = match axis.as_str() {
                "width" => ScaleAxis::Width,
                "height" => ScaleAxis::Height,
                _ => ScaleAxis::Factor,
            };
            (value, axis)
        })
}

/// Reads the source into the diagram and its `scale` line.
pub fn read(source: &str) -> Result<(Diagram, Option<(f64, ScaleAxis)>), Error> {
    let scale = scale_of(source);
    let mut reader = Reader {
        diagram: Diagram { direction: "TB".to_string(), ..Default::default() },
        closing: None,
        scopes: vec![None],
        note: None,
        owners: HashMap::new(),
        last_state: None,
    };
    for (text, number) in lines_of(source) {
        reader.read_line(&text, number)?;
    }
    if reader.scopes.len() != 1 || reader.note.is_some() {
        return Err(Error::Parse {
            class: "ParseError".into(),
            message: "Parse error: a state body is never closed with }".into(),
            line: 0,
            col: 0,
        });
    }
    Ok((reader.diagram, scale))
}

struct Reader {
    diagram: Diagram,
    /// What a skipped block closes with (Ruby `@closing`).
    closing: Option<Closing>,
    /// The ids of the open composites, innermost last (Ruby `@scopes`).
    scopes: Vec<Option<String>>,
    /// The open note block: target id, side, and its lines.
    note: Option<(String, String, Vec<String>)>,
    /// The container of each state (Ruby `@owners`).
    owners: HashMap<String, Option<String>>,
    /// The last declared state (Ruby `@last_state`).
    last_state: Option<String>,
}

impl Reader {
    fn read_line(&mut self, text: &str, number: usize) -> Result<(), Error> {
        let _ = number;
        if self.note.is_some() {
            self.note_line(text);
            return Ok(());
        }
        if self.skipped(text) {
            return Ok(());
        }
        if text == "}" {
            self.close_scope();
            return Ok(());
        }
        if self.statement(text)? {
            return Ok(());
        }
        Err(unported(text))
    }

    // -- skipped lines and blocks --

    fn skipped(&mut self, text: &str) -> bool {
        if self.closing.is_some() {
            self.skip_block(text);
            return true;
        }
        if self.skipped_block(text) {
            self.open_skipped(text);
            return true;
        }
        re("skipped").is_match(text) || matches!(text, "--" | "||")
    }

    fn skipped_block(&self, text: &str) -> bool {
        re("style_block").is_match(text) || re("skin_block").is_match(text)
    }

    /// The `skinparam x {` and `<style>` bodies are read by the style
    /// sheet; this reader only steps over them.
    fn open_skipped(&mut self, text: &str) {
        self.closing = if re("style_block").is_match(text) {
            Some(Closing::Style)
        } else {
            Some(Closing::Brace)
        };
    }

    fn skip_block(&mut self, text: &str) {
        let done = match self.closing {
            Some(Closing::Style) => re("end_style").is_match(text),
            Some(Closing::Brace) => re("close_skin").is_match(text),
            None => false,
        };
        if done {
            self.closing = None;
        }
    }

    fn close_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    // -- statements --

    fn statement(&mut self, text: &str) -> Result<bool, Error> {
        Ok(self.declaration(text)
            || self.transition(text)
            || self.note(text)?
            || self.description(text))
    }

    fn declaration(&mut self, text: &str) -> bool {
        let Some(m) = re("state_open").captures(text) else { return false };
        let id = m
            .get(2)
            .or_else(|| m.get(3))
            .map(|g| g.as_str().to_string())
            .unwrap_or_default();
        let state = self.ensure_state(id);
        self.adopt(&state);
        self.last_state = Some(state.clone());
        if let Some(label) = m.get(1).map(|g| g.as_str().to_string()) {
            self.set_label(&state, label);
        }
        if let Some(d) = m.get(8).map(|g| g.as_str().to_string()) {
            self.describe(&state, d);
        }
        if m.get(7).is_some() {
            self.scopes.push(Some(state));
        }
        true
    }

    fn transition(&mut self, text: &str) -> bool {
        let (from, to, label, flat) = if let Some(m) = re("transition").captures(text) {
            (
                m[1].to_string(),
                m[4].to_string(),
                m.get(6).map(|g| g.as_str().to_string()),
                re("flat").is_match(&m[3]),
            )
        } else if let Some(m) = re("reversed").captures(text) {
            (m[3].to_string(), m[1].to_string(), m.get(5).map(|g| g.as_str().to_string()), false)
        } else {
            return false;
        };
        let source = self.endpoint(&from, true);
        let target = self.endpoint(&to, false);
        self.add_transition(&source, &target, label);
        if flat {
            self.diagram.flat.push((source.clone(), target.clone()));
        }
        true
    }

    /// The state a written endpoint names: a marker is the scope's start
    /// or end, or the owner's (deep) history (Ruby `endpoint`).
    fn endpoint(&mut self, written: &str, source: bool) -> String {
        let marker = re("mark").find(written).map(|m| m.as_str().to_string());
        let Some(marker) = marker else {
            return self.ensure_state(written.to_string());
        };
        let owner = written.strip_suffix(&marker).unwrap_or(written).to_string();
        let owner = if owner.is_empty() { None } else { Some(self.ensure_state(owner)) };
        self.marker_state(&marker, source, owner)
    }

    fn marker_state(&mut self, marker: &str, source: bool, owner: Option<String>) -> String {
        if marker == "[H]" || marker == "[H*]" {
            let (kind, label) = if marker == "[H]" { ("history", "H") } else { ("deep_history", "H*") };
            let parent = owner.or_else(|| self.scopes.last().cloned().flatten());
            let id = format!("{}_{}", parent.as_deref().unwrap_or("root"), kind);
            if self.find(&id).is_none() {
                self.build_state(id.clone(), Some(label.to_string()), "normal", parent);
            }
            return id;
        }
        let kind = if source { "start" } else { "end" };
        let scope = self.scopes.last().cloned().flatten().unwrap_or_else(|| "root".to_string());
        let id = format!("{scope}_{kind}");
        if self.find(&id).is_none() {
            let parent = self.scopes.last().cloned().flatten();
            self.build_state(id.clone(), Some("[*]".to_string()), kind, parent);
        }
        id
    }

    fn add_transition(&mut self, from: &str, to: &str, trigger: Option<String>) {
        self.diagram.transitions.push(Transition {
            from_id: from.to_string(),
            to_id: to.to_string(),
            trigger,
        });
    }

    // -- notes --

    fn note(&mut self, text: &str) -> Result<bool, Error> {
        if re("note_as").is_match(text) {
            // `note as NAME` floats in PlantUML, but the model requires a
            // target, so it attaches to the last declared state.
            let Some(last) = self.last_state.clone() else {
                return Err(Error::Unported(
                    "state_diagram: note as without a declared state".into(),
                ));
            };
            self.note = Some((last, "bottom".to_string(), vec![]));
            return Ok(true);
        }
        let Some(m) = re("note").captures(text) else { return Ok(false) };
        let side = m[1].to_lowercase();
        let target = self.ensure_state(m[2].to_string());
        match m.get(3).map(|g| g.as_str().to_string()) {
            Some(body) => self.add_note(&target, &side, body),
            None => self.note = Some((target, side, vec![])),
        }
        Ok(true)
    }

    fn note_line(&mut self, text: &str) {
        if re("end_note").is_match(text) {
            let Some((target, side, lines)) = self.note.take() else { return };
            self.add_note(&target, &side, lines.join("<br>"));
            return;
        }
        if let Some((_, _, lines)) = self.note.as_mut() {
            lines.push(text.to_string());
        }
    }

    fn add_note(&mut self, target: &str, side: &str, body: String) {
        let notes = &mut self.diagram.notes;
        let id = format!("{}----note{}", target, notes.len());
        notes.push(Note {
            id,
            text: body.replace("\\n", "<br>").trim().to_string(),
            target_id: target.to_string(),
            position: side.to_string(),
        });
    }

    // -- descriptions --

    fn description(&mut self, text: &str) -> bool {
        let Some(m) = re("description").captures(text) else { return false };
        let state = self.ensure_state(m[1].to_string());
        self.describe(&state, m[2].to_string());
        true
    }

    /// A described state keeps its name as the first line, as PlantUML
    /// draws it (Ruby `describe`).
    fn describe(&mut self, id: &str, text: String) {
        let Some(path) = self.find(id) else { return };
        let node = Self::at_mut(&mut self.diagram.states, &path);
        let titled = if node.descriptions.is_empty() {
            node.label.clone().into_iter().collect::<Vec<_>>()
        } else {
            node.descriptions.clone()
        };
        node.description = Some(text.clone());
        let mut descriptions = titled;
        descriptions.push(text);
        node.descriptions = descriptions;
    }

    // -- the state tree --

    fn find(&self, id: &str) -> Option<Vec<usize>> {
        let mut prefix = vec![];
        self.dfs(&self.diagram.states, id, &mut prefix)
    }

    fn dfs(&self, states: &[Node], id: &str, prefix: &mut Vec<usize>) -> Option<Vec<usize>> {
        for (i, node) in states.iter().enumerate() {
            prefix.push(i);
            if node.id == id {
                return Some(prefix.clone());
            }
            if let Some(p) = self.dfs(&node.children, id, prefix) {
                return Some(p);
            }
            prefix.pop();
        }
        None
    }

    fn at_mut<'a>(states: &'a mut Vec<Node>, path: &[usize]) -> &'a mut Node {
        let (first, rest) = path.split_first().expect("non-empty path");
        let node = &mut states[*first];
        if rest.is_empty() {
            node
        } else {
            Self::at_mut(&mut node.children, rest)
        }
    }

    /// The state named `id`, built in the current scope when new
    /// (Ruby `ensure_state`; `""` is Ruby's nil id).
    fn ensure_state(&mut self, id: String) -> String {
        if self.find(&id).is_none() {
            let owner = self.scopes.last().cloned().flatten();
            self.build_state(id.clone(), Some(id.clone()), "normal", owner);
        }
        id
    }

    fn build_state(&mut self, id: String, label: Option<String>, ty: &str, owner: Option<String>) {
        let node = Node {
            id: id.clone(),
            label,
            state_type: ty.to_string(),
            ..Default::default()
        };
        match owner.as_deref() {
            None => self.diagram.states.push(node),
            Some(owner) => {
                if let Some(path) = self.find(owner) {
                    Self::at_mut(&mut self.diagram.states, &path).children.push(node);
                } else {
                    self.diagram.states.push(node);
                }
            }
        }
        self.owners.insert(id, owner);
    }

    /// `state X` names where X lives: a state first met as a transition
    /// target inside another body is moved to the scope that declares it
    /// (Ruby `adopt`).
    fn adopt(&mut self, id: &str) {
        let parent = self.scopes.last().cloned().flatten();
        if self.owners.get(id).and_then(|o| o.clone()) == parent {
            return;
        }
        // Cut the state out of its old container.
        let node = match self.find(id) {
            Some(path) => {
                if path.len() == 1 {
                    self.diagram.states.remove(path[0])
                } else {
                    let at = *path.last().expect("path");
                    let owner_path = path[..path.len() - 1].to_vec();
                    Self::at_mut(&mut self.diagram.states, &owner_path).children.remove(at)
                }
            }
            None => Node { id: id.to_string(), ..Default::default() },
        };
        // Hand it to the new container.
        match parent.as_deref() {
            None => self.diagram.states.push(node),
            Some(owner) => {
                if let Some(path) = self.find(owner) {
                    Self::at_mut(&mut self.diagram.states, &path).children.push(node);
                } else {
                    self.diagram.states.push(node);
                }
            }
        }
        self.owners.insert(id.to_string(), parent);
    }

    fn set_label(&mut self, id: &str, label: String) {
        if let Some(path) = self.find(id) {
            Self::at_mut(&mut self.diagram.states, &path).label = Some(label);
        }
    }
}

/// What a skipped block closes with (Ruby `@closing`).
enum Closing {
    Style,
    Brace,
}
