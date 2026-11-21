//! Port of `activity/{node,edge,note,diagram,builder}.rb`: the nodes and
//! edges both syntaxes parse into, in source order. Nodes are arena
//! indices; every identity-keyed Ruby hash becomes an index lookup.

/// One box, diamond, bar or terminal in an activity diagram (`Node`).
#[derive(Clone, Debug)]
pub struct Node {
    /// Stable id, `n<index>` in source order; carried, never drawn.
    #[allow(dead_code)]
    pub id: String,
    pub kind: Kind,
    /// Label, lines separated by newlines.
    pub text: Option<String>,
    /// Fill colour from `#colour:` or a lane.
    pub fill: Option<String>,
    /// Swimlane name.
    pub lane: Option<String>,
    /// Enclosing partition name.
    pub partition: Option<String>,
    /// Whether the node came from the legacy `(*)` syntax.
    pub legacy: bool,
    pub notes: Vec<Note>,
    /// A decision drawn as a bare diamond, its question beside the arrow
    /// that enters it (legacy `if`).
    pub bare: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Start,
    Stop,
    End,
    Kill,
    Action,
    Decision,
    Merge,
    Bar,
}

/// Ruby's `String#split("\n")`: an empty string splits to nothing.
pub fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        vec![]
    } else {
        text.split('\n').map(str::to_string).collect()
    }
}

/// The label's lines (`Node#lines`).
pub fn node_lines(text: Option<&str>) -> Vec<String> {
    text.map(split_lines).unwrap_or_default()
}

/// The lines of an edge label (`edge.label.to_s.split("\n")`).
pub fn edge_label_lines(label: Option<&String>) -> Vec<String> {
    label.map(|label| split_lines(label)).unwrap_or_default()
}

/// A note drawn beside the node it follows (`Note`).
#[derive(Clone, Debug)]
pub struct Note {
    pub side: Side,
    /// The note text, lines separated by newlines.
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Note {
    pub fn lines(&self) -> Vec<String> {
        split_lines(&self.text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeStyle {
    Dashed,
    Dotted,
    Bold,
}

/// A flow arrow between two nodes (`Edge`).
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    /// An SVG colour.
    pub colour: Option<String>,
    pub style: Option<EdgeStyle>,
    /// Left or right when the arrow lies along its row (legacy syntax).
    pub direction: Option<Direction>,
}

impl Edge {
    /// Whether the arrow lies along its row (`Edge#level?`).
    pub fn level(&self) -> bool {
        self.direction.is_some()
    }
}

/// A parsed activity diagram (`Diagram`): nodes and edges in source
/// order, the swimlanes in order of first use, an optional title and the
/// notices PlantUML prints above the drawing.
#[derive(Clone, Debug, Default)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Name and colour, in order of first use.
    pub lanes: Vec<(String, Option<String>)>,
    pub title: Option<String>,
    pub warnings: Vec<String>,
    pub legacy: bool,
}

/// Accumulates the nodes and edges both syntaxes parse into, and the
/// swimlane and partition a new node belongs to (`Builder`).
#[derive(Default)]
pub struct Builder {
    pub diagram: Diagram,
    lane: Option<String>,
    partition: Option<String>,
}

impl Builder {
    pub fn new() -> Self {
        Builder::default()
    }

    /// Records a notice PlantUML prints above the diagram.
    pub fn warn(&mut self, message: &str) {
        self.diagram.warnings.push(message.to_string());
    }

    pub fn node(
        &mut self,
        kind: Kind,
        text: Option<String>,
        fill: Option<String>,
        bare: bool,
    ) -> usize {
        let id = format!("n{}", self.diagram.nodes.len());
        let node = Node {
            id,
            kind,
            text,
            fill,
            lane: self.lane.clone(),
            partition: self.partition.clone(),
            legacy: false,
            notes: vec![],
            bare,
        };
        let index = self.diagram.nodes.len();
        self.diagram.nodes.push(node);
        index
    }

    pub fn connect(
        &mut self,
        from: usize,
        to: usize,
        label: Option<String>,
        colour: Option<String>,
        style: Option<EdgeStyle>,
    ) -> usize {
        let edge = Edge {
            from,
            to,
            label,
            colour,
            style,
            direction: None,
        };
        let index = self.diagram.edges.len();
        self.diagram.edges.push(edge);
        index
    }

    pub fn lane(&mut self, name: &str, colour: Option<String>) {
        if !self.diagram.lanes.iter().any(|(lane, _)| lane == name) {
            self.diagram.lanes.push((name.to_string(), colour));
        }
        self.lane = Some(name.to_string());
    }

    /// `Builder#partition`: nil leaves the partition.
    pub fn partition(
        &mut self,
        name: Option<&str>,
        line: usize,
        text: &str,
    ) -> Result<(), crate::Error> {
        if name.is_some() && self.partition.is_some() {
            return Err(unsupported("nested partition", line, text));
        }
        self.partition = name.map(str::to_string);
        Ok(())
    }

    pub fn into_diagram(self) -> Diagram {
        self.diagram
    }
}

/// The `UnsupportedConstructError` the readers raise: the Ruby message
/// quotes the offending line with `String#inspect`.
pub fn unsupported(construct: &str, line: usize, text: &str) -> crate::Error {
    let excerpt = if text.chars().count() <= 60 {
        text.to_string()
    } else {
        let head: String = text.chars().take(57).collect();
        format!("{head}...")
    };
    crate::Error::Parse {
        class: "UnsupportedConstructError".into(),
        message: format!(
            "PlantUML {construct} is not yet supported (line {line}): {}",
            ruby_inspect(&excerpt)
        ),
        line,
        col: 0,
    }
}

/// Ruby `String#inspect`, as far as the excerpts reach: quotes, `\n`,
/// `\t`, `\\` and the non-printable escapes.
pub fn ruby_inspect(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\x0C' => out.push_str("\\f"),
            '\x0B' => out.push_str("\\v"),
            '\x08' => out.push_str("\\b"),
            '\x07' => out.push_str("\\a"),
            '\x1B' => out.push_str("\\e"),
            '#' => out.push_str("\\#"),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                out.push_str(&format!("\\x{:0>2X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_carry_their_lane_and_partition() {
        let mut builder = Builder::new();
        builder.lane("Actor 1", None);
        builder.partition(Some("P"), 0, "partition P").unwrap();
        let index = builder.node(Kind::Action, Some("A".into()), None, false);
        let diagram = builder.into_diagram();
        assert_eq!(diagram.nodes[index].lane.as_deref(), Some("Actor 1"));
        assert_eq!(diagram.nodes[index].partition.as_deref(), Some("P"));
        assert_eq!(diagram.nodes[index].id, "n0");
    }
}
