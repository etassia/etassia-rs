//! The graph model: nodes and links in source order, plus the link's
//! arrow vocabulary. Port of `node_graph/{graph,node,edge,link}.rb`.
//! Identity is the node's index, standing in for Ruby object identity.

use std::collections::HashMap;

use super::archimate_sprites::Sprite;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NodeKind {
    Package,
    Node,
    Database,
    Cloud,
    Rectangle,
    Folder,
    Frame,
    Storage,
    Artifact,
    Card,
    Namespace,
    Component,
    File,
    Interface,
    Actor,
    Usecase,
    Portin,
    Portout,
    Circle,
    Archimate,
    Note,
}

impl NodeKind {
    /// Ruby `Parser::KEYWORDS`, the names a declaration or container opens
    /// with.
    pub fn from_keyword(word: &str) -> Option<NodeKind> {
        Some(match word.to_ascii_lowercase().as_str() {
            "package" => NodeKind::Package,
            "node" => NodeKind::Node,
            "database" => NodeKind::Database,
            "cloud" => NodeKind::Cloud,
            "rectangle" => NodeKind::Rectangle,
            "folder" => NodeKind::Folder,
            "frame" => NodeKind::Frame,
            "storage" => NodeKind::Storage,
            "artifact" => NodeKind::Artifact,
            "card" => NodeKind::Card,
            "namespace" => NodeKind::Namespace,
            "component" => NodeKind::Component,
            "file" => NodeKind::File,
            "interface" => NodeKind::Interface,
            "actor" => NodeKind::Actor,
            "usecase" => NodeKind::Usecase,
            "portin" => NodeKind::Portin,
            "portout" => NodeKind::Portout,
            "circle" => NodeKind::Circle,
            "archimate" => NodeKind::Archimate,
            _ => return None,
        })
    }

    pub fn keyword(self) -> &'static str {
        match self {
            NodeKind::Package => "package",
            NodeKind::Node => "node",
            NodeKind::Database => "database",
            NodeKind::Cloud => "cloud",
            NodeKind::Rectangle => "rectangle",
            NodeKind::Folder => "folder",
            NodeKind::Frame => "frame",
            NodeKind::Storage => "storage",
            NodeKind::Artifact => "artifact",
            NodeKind::Card => "card",
            NodeKind::Namespace => "namespace",
            NodeKind::Component => "component",
            NodeKind::File => "file",
            NodeKind::Interface => "interface",
            NodeKind::Actor => "actor",
            NodeKind::Usecase => "usecase",
            NodeKind::Portin => "portin",
            NodeKind::Portout => "portout",
            NodeKind::Circle => "circle",
            NodeKind::Archimate => "archimate",
            NodeKind::Note => "note",
        }
    }
}

/// One element of a use case, component or deployment diagram.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub lines: Vec<String>,
    pub stereotype: Option<String>,
    pub color: Option<String>,
    pub sprite: Option<&'static Sprite>,
    pub container: bool,
    pub parent: Option<usize>,
}

impl Node {
    pub fn new(id: &str, kind: NodeKind, parent: Option<usize>) -> Node {
        Node {
            id: id.to_string(),
            kind,
            lines: vec![id.to_string()],
            stereotype: None,
            color: None,
            sprite: None,
            container: false,
            parent,
        }
    }

    /// The containers around this node, outermost first.
    pub fn ancestors(graph: &Graph, index: usize) -> Vec<usize> {
        let mut chain = vec![];
        let mut current = graph.nodes[index].parent;
        while let Some(parent) = current {
            chain.push(parent);
            current = graph.nodes[parent].parent;
        }
        chain.reverse();
        chain
    }

    /// `self` plus its ancestors, innermost first (Ruby's
    /// `node.ancestors + [node]`).
    pub fn chain(graph: &Graph, index: usize) -> Vec<usize> {
        let mut chain = Node::ancestors(graph, index);
        chain.push(index);
        chain
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Head {
    Both,
    Target,
    Source,
    Socket,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Solid,
    Dashed,
}

/// A link between two nodes.
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    pub style: Style,
    pub head: Option<Head>,
    pub color: Option<String>,
    pub color2: Option<String>,
    pub width: i64,
    /// How many ranks the layout puts `to` after `from`; `None` reads as
    /// one (Ruby passes nil through `step || 1`).
    pub rank_step: Option<usize>,
    /// Whether `up` or `left` reads the link from the target.
    pub backwards: bool,
    /// The tail of a note written beside its target.
    pub pointer: bool,
}

impl Edge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from: usize,
        to: usize,
        label: Option<String>,
        style: Style,
        head: Option<Head>,
        color: Option<String>,
        color2: Option<String>,
        width: i64,
        rank_step: Option<usize>,
        backwards: bool,
        pointer: bool,
    ) -> Edge {
        Edge { from, to, label, style, head, color, color2, width, rank_step, backwards, pointer }
    }
}

/// The nodes and links of one diagram, in source order.
#[derive(Debug)]
pub struct Graph {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub title: Option<String>,
    by_id: HashMap<String, usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    LeftToRight,
    TopToBottom,
}

impl Graph {
    pub fn new(direction: Direction) -> Graph {
        Graph { direction, nodes: vec![], edges: vec![], title: None, by_id: HashMap::new() }
    }

    /// The node for `id`, made with `kind` if new.
    pub fn node(&mut self, id: &str, kind: NodeKind, parent: Option<usize>) -> usize {
        if let Some(&index) = self.by_id.get(id) {
            return index;
        }
        let index = self.nodes.len();
        self.nodes.push(Node::new(id, kind, parent));
        self.by_id.insert(id.to_string(), index);
        index
    }

    /// `graph.find(name)`: by id, else the node whose label is exactly the
    /// name.
    pub fn find(&self, name: &str) -> Option<usize> {
        if let Some(&index) = self.by_id.get(name) {
            return Some(index);
        }
        self.nodes.iter().position(|node| node.lines == [name])
    }

    /// The nodes directly inside `parent`, in source order.
    pub fn children_of(&self, parent: Option<usize>) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.parent == parent)
            .map(|(index, _)| index)
            .collect()
    }
}

/// What the arrow of a link says. Port of `node_graph/link.rb`.
pub struct Link {
    pub color: Option<String>,
    pub color2: Option<String>,
    pub width: i64,
    options: Vec<String>,
    bare: String,
}

impl Link {
    pub fn new(arrow: &str) -> Link {
        let options: Vec<String> = bracketed_options(arrow);
        let mut colors = vec![];
        for option in &options {
            if let Some(hex) = option.strip_prefix('#') {
                if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_alphanumeric()) {
                    colors.push(hex.to_string());
                }
            }
        }
        let mut width: i64 = options
            .iter()
            .find_map(|o| o.strip_prefix("thickness=").and_then(|rest| {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.is_empty() { None } else { digits.parse().ok() }
            }))
            .unwrap_or(0);
        if width < 1 {
            width = 1;
        }
        let bare = strip_brackets(arrow);
        let color = colors.first().cloned();
        let color2 = colors.get(1).cloned();
        Link { color, color2, width, options, bare }
    }

    pub fn hidden(&self) -> bool {
        self.options.iter().any(|o| o == "hidden")
    }

    /// `:dashed` or `:solid`.
    pub fn style(&self) -> Style {
        let dashed = self.bare.contains('.')
            || self.options.iter().any(|o| o == "dashed" || o == "dotted");
        if dashed { Style::Dashed } else { Style::Solid }
    }

    /// How many ranks the target sits after the source.
    pub fn rank_step(&self) -> usize {
        match direction_word(&self.bare) {
            Some(word) => {
                if word == "up" || word == "down" { 1 } else { 0 }
            }
            None => (self.bare.chars().filter(|c| *c == '-' || *c == '.' || *c == '=').count() as i64 - 1).max(0)
                as usize,
        }
    }

    /// Whether `up` or `left` reads the link from the target.
    pub fn backwards(&self) -> bool {
        for word in ["up", "left"] {
            let mut from = 0;
            while let Some(at) = self.bare[from..].find(word) {
                let at = from + at;
                let after = self.bare[at + word.len()..].chars().next();
                let boundary = after.map(|c| !c.is_alphabetic()).unwrap_or(true);
                if boundary {
                    return true;
                }
                from = at + 1;
            }
        }
        false
    }

    pub fn head(&self) -> Option<Head> {
        let starts_lt = self.bare.starts_with('<');
        let ends_gt = self.bare.ends_with('>');
        if starts_lt && ends_gt {
            return Some(Head::Both);
        }
        if ends_gt {
            return Some(Head::Target);
        }
        if starts_lt {
            return Some(Head::Source);
        }
        if self.bare.contains("0)") || self.bare.contains("(0") || self.bare.contains(')') {
            Some(Head::Socket)
        } else {
            None
        }
    }
}

/// `arrow[/\[(.*)\]/, 1].to_s.split(/[,;]/).map(&:strip)` — the greedy
/// capture takes from the first `[` to the last `]`.
fn bracketed_options(arrow: &str) -> Vec<String> {
    let open = arrow.find('[');
    let close = arrow.rfind(']');
    match (open, close) {
        (Some(open), Some(close)) if close > open => arrow[open + 1..close]
            .split(|c| c == ',' || c == ';')
            .map(|part| part.trim().to_string())
            .collect(),
        _ => vec![],
    }
}

fn strip_brackets(arrow: &str) -> String {
    match (arrow.find('['), arrow.rfind(']')) {
        (Some(open), Some(close)) if close > open => {
            format!("{}{}", &arrow[..open], &arrow[close + 1..])
        }
        _ => arrow.to_string(),
    }
}

/// The first `up|down|left|right` in `text` not followed by a letter, in
/// Ruby's alternation order at each position.
pub fn direction_word(text: &str) -> Option<&'static str> {
    let bytes = text.as_bytes();
    for at in 0..bytes.len() {
        for word in ["up", "down", "left", "right"] {
            if text[at..].starts_with(word) {
                let after = bytes.get(at + word.len()).copied();
                let boundary = after
                    .map(|b| !char::from(b).is_alphabetic())
                    .unwrap_or(true);
                if boundary {
                    return Some(word);
                }
            }
        }
    }
    None
}
