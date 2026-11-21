//! The sequence model: what the reader builds and the layout places.
//! Port of the small classes in `sequence/*.rb` (message, edge, arrow
//! style, fragment, note, participant, box, activation, and the rest).

use std::collections::HashMap;

/// `Fill`: the colour written after a participant or note.
#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub colour: String,
    pub opacity: Option<f64>,
}

/// `Participant`: one lifeline.
#[derive(Clone, Debug)]
pub struct Participant {
    pub id: String,
    pub label: String,
    pub kind: &'static str,
    pub stereotype: Option<String>,
    pub fill: Option<Fill>,
}

/// Which end of a message a missing participant sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// `Edge`: the missing end of a message.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub side: Side,
    pub local: bool,
}

impl Edge {
    pub fn global(&self) -> bool {
        !self.local
    }

    pub fn left(&self) -> bool {
        self.side == Side::Left
    }
}

/// One end of a message: a participant id, or the diagram edge.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Id(String),
    Edge(Edge),
}

impl Place {
    pub fn id(&self) -> &str {
        match self {
            Place::Id(id) => id,
            Place::Edge(_) => "",
        }
    }
}

/// `ArrowEnd`: the glyph drawn at one end of an arrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ArrowEnd {
    pub glyph: Option<&'static str>,
    pub circle: bool,
}

/// `ArrowStyle`: how a message is drawn.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArrowStyle {
    pub head: ArrowEnd,
    pub tail: ArrowEnd,
    pub dashed: bool,
    pub leftward: bool,
    pub hidden: bool,
    pub colour: Option<String>,
}

impl ArrowStyle {
    /// `ArrowStyle.plain`: a filled head, dashed for `return`.
    pub fn plain(glyph: &'static str, dashed: bool) -> Self {
        ArrowStyle {
            head: ArrowEnd { glyph: Some(glyph), circle: false },
            tail: ArrowEnd::default(),
            dashed,
            ..ArrowStyle::default()
        }
    }
}

/// `Message::Number`: the text `autonumber` draws before a label.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    pub text: String,
    pub bold: bool,
}

/// `Message`: one arrow, `from` always the sender.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub from: Place,
    pub to: Place,
    pub label: String,
    pub label_family: Option<&'static str>,
    pub style: ArrowStyle,
    pub number: Option<Number>,
    /// `ParallelMessage`: written after `&` under teoz.
    pub parallel: bool,
}

impl Message {
    pub fn self_message(&self) -> bool {
        self.from == self.to
    }

    pub fn dashed(&self) -> bool {
        self.style.dashed
    }

    pub fn leftward(&self) -> bool {
        self.style.leftward
    }

    /// The end with no participant.
    pub fn edge(&self) -> Option<&Edge> {
        match (&self.from, &self.to) {
            (Place::Edge(e), _) | (_, Place::Edge(e)) => Some(e),
            _ => None,
        }
    }

    /// The participants the message touches.
    pub fn participants(&self) -> Vec<&str> {
        let mut out = vec![];
        if let Place::Id(id) = &self.from {
            out.push(id.as_str());
        }
        if let Place::Id(id) = &self.to {
            out.push(id.as_str());
        }
        out
    }

    /// What is drawn at the edge end.
    pub fn edge_end(&self) -> &ArrowEnd {
        if matches!(self.from, Place::Edge(_)) {
            &self.style.tail
        } else {
            &self.style.head
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    On,
    Off,
}

/// `Activation`: a bar opens or closes on a lifeline.
#[derive(Clone, Debug)]
pub struct Activation {
    pub participant: String,
    pub phase: Phase,
    pub color: Option<String>,
}

impl Activation {
    pub fn on(&self) -> bool {
        self.phase == Phase::On
    }
}

/// `Destroy`: a cross on a lifeline.
#[derive(Clone, Debug)]
pub struct Destroy {
    pub participant: String,
}

/// `Create`: the next message to the participant makes its head appear.
#[derive(Clone, Debug)]
pub struct Create {
    pub participant: String,
}

/// `Divider`: a `== text ==` line.
#[derive(Clone, Debug)]
pub struct Divider {
    pub label: String,
}

/// `Delay`: a `...` line.
#[derive(Clone, Debug)]
pub struct Delay {
    pub label: String,
}

/// `Spacing`: a `|||` or `||N||` line.
#[derive(Clone, Debug)]
pub struct Spacing {
    pub height: Option<f64>,
}

impl Spacing {
    pub fn room(&self) -> f64 {
        self.height.unwrap_or(25.0)
    }
}

/// `PageBreak`: nothing after the first one is drawn.
#[derive(Clone, Debug)]
pub struct PageBreak;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragPhase {
    Open,
    Else,
    Close,
}

/// `Fragment`: one edge of an alt/loop/group block.
#[derive(Clone, Debug)]
pub struct Fragment {
    pub phase: FragPhase,
    pub keyword: String,
    pub label: Option<String>,
    pub parallel: bool,
}

/// `Ref`: a frame over the participants it names.
#[derive(Clone, Debug)]
pub struct Ref {
    pub targets: Vec<String>,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteShape {
    Fold,
    Hex,
    Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteSide {
    Left,
    Right,
    Over,
    Across,
    Top,
    Bottom,
}

/// The laid-out embedded diagram of a note.
#[derive(Clone, Debug)]
pub struct Picture {
    pub scene: std::sync::Arc<SceneOwned>,
    pub scale: f64,
}

/// The final canvas scene lives in `layout`; this alias keeps the model's
/// `Picture` self-referential without a cycle.
pub type SceneOwned = super::layout::Scene;

impl Picture {
    pub fn width(&self) -> f64 {
        self.scene.width * self.scale
    }

    pub fn height(&self) -> f64 {
        self.scene.height * self.scale
    }
}

/// `Note`: a `note`, `hnote` or `rnote`.
#[derive(Clone, Debug)]
pub struct Note {
    pub shape: NoteShape,
    pub side: NoteSide,
    pub targets: Vec<String>,
    pub text: String,
    pub parallel: bool,
    pub fill: Option<Fill>,
    pub picture: Option<Picture>,
}

impl Note {
    /// PlantUML reads the two characters `\n` as a line break too.
    pub fn lines(&self) -> Vec<&str> {
        split_lines(&self.text)
    }

    pub fn attached(&self) -> bool {
        self.targets.is_empty()
            && matches!(self.side, NoteSide::Left | NoteSide::Right)
    }
}

/// Ruby `text.split("\n")`: a trailing empty line is dropped.
pub fn ruby_split(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|last| last.is_empty()) {
        lines.pop();
    }
    lines
}

/// Ruby `text.split(/\\n|\n/, -1)`.
pub fn split_lines(text: &str) -> Vec<&str> {
    let mut out = vec![];
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'n') {
            out.push(&text[start..i]);
            i += 2;
            start = i;
        } else if bytes[i] == b'\n' {
            out.push(&text[start..i]);
            i += 1;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(&text[start..]);
    out
}

/// `Box`: a grouping of neighbouring participants.
#[derive(Clone, Debug)]
pub struct Box_ {
    pub title: String,
    pub members: Vec<String>,
    pub fill: Option<Fill>,
}

/// `Numbering`: what an `autonumber` line sets.
#[derive(Clone, Debug)]
pub struct Numbering {
    pub start: i64,
    pub step: i64,
    pub bold: bool,
    pub before: String,
    pub zeros: usize,
    pub after: String,
    pub literal: bool,
}

impl Numbering {
    /// `Numbering.read`; None when the format is not one read.
    pub fn read(start: i64, step: i64, format: Option<&str>) -> Option<Self> {
        let Some(format) = format else {
            return Some(Numbering {
                start,
                step,
                bold: true,
                before: String::new(),
                zeros: 0,
                after: String::new(),
                literal: true,
            });
        };
        // FORMAT: an optional leading <b>, text with no 0 < > # . then one
        // run of zeros, then text with none of them.
        let (format, bold) = match format.strip_prefix("<b>") {
            Some(rest) => (rest, true),
            None => (format, false),
        };
        let bytes = format.as_bytes();
        let mut i = 0;
        let before_start = i;
        while i < bytes.len() && bytes[i] != b'0' {
            if matches!(bytes[i], b'<' | b'>' | b'#' | b'.') {
                return None;
            }
            i += 1;
        }
        let before = format[before_start..i].to_string();
        let zeros_start = i;
        while i < bytes.len() && bytes[i] == b'0' {
            i += 1;
        }
        if i == zeros_start {
            return None;
        }
        let zeros = i - zeros_start;
        let after_start = i;
        while i < bytes.len() {
            if matches!(bytes[i], b'0' | b'<' | b'>' | b'#' | b'.') {
                return None;
            }
            i += 1;
        }
        let after = format[after_start..].to_string();
        Some(Numbering {
            start,
            step,
            bold,
            before,
            zeros,
            after,
            literal: false,
        })
    }

    /// The text drawn for the `count`th number.
    pub fn text(&self, count: i64) -> String {
        if self.literal {
            return count.to_string();
        }
        let digits = format!("{count:0>width$}", width = self.zeros);
        format!("{}{}{}", self.before, digits, self.after)
    }
}

/// One item of the diagram, in source order.
#[derive(Clone, Debug)]
pub enum Item {
    Message(Message),
    Note(Note),
    Ref(Ref),
    Fragment(Fragment),
    Divider(Divider),
    Delay(Delay),
    Spacing(Spacing),
    Activation(Activation),
    Destroy(Destroy),
    Create(Create),
    PageBreak(PageBreak),
}

impl Item {
    pub fn as_message(&self) -> Option<&Message> {
        match self {
            Item::Message(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_note(&self) -> Option<&Note> {
        match self {
            Item::Note(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_fragment(&self) -> Option<&Fragment> {
        match self {
            Item::Fragment(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_activation(&self) -> Option<&Activation> {
        match self {
            Item::Activation(a) => Some(a),
            _ => None,
        }
    }
}

/// `Chrome`: the header, footer, caption and legend.
#[derive(Clone, Debug)]
pub struct Chrome {
    pub header: Option<String>,
    pub footer: Option<String>,
    pub caption: Option<String>,
    pub legend: Option<String>,
    /// "<top|bottom> <left|center|right>"
    pub legend_place: String,
}

impl Default for Chrome {
    fn default() -> Self {
        Chrome {
            header: None,
            footer: None,
            caption: None,
            legend: None,
            legend_place: "bottom center".into(),
        }
    }
}

impl Chrome {
    pub fn with(mut self, kind: &str, text: String, place: Option<String>) -> Self {
        match kind {
            "header" => self.header = Some(text),
            "footer" => self.footer = Some(text),
            "caption" => self.caption = Some(text),
            "legend" => {
                self.legend = Some(text);
                if let Some(place) = place {
                    self.legend_place = place;
                }
            }
            _ => {}
        }
        self
    }

    pub fn lines(&self, kind: &str) -> Vec<&str> {
        let text = match kind {
            "header" => &self.header,
            "footer" => &self.footer,
            "caption" => &self.caption,
            "legend" => &self.legend,
            _ => return vec![],
        };
        ruby_split(text.as_deref().unwrap_or(""))
    }
}

/// `HeadStyle`: what a `participant { ... }` style block sets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeadStyle {
    pub colour: Option<String>,
    pub size: Option<f64>,
    pub style: Option<String>,
    pub family: Option<String>,
    pub weight: Option<String>,
    pub line: Option<String>,
}

impl HeadStyle {
    pub fn merge(&self, other: &HeadStyle) -> HeadStyle {
        HeadStyle {
            colour: other.colour.clone().or_else(|| self.colour.clone()),
            size: other.size.or(self.size),
            style: other.style.clone().or_else(|| self.style.clone()),
            family: other.family.clone().or_else(|| self.family.clone()),
            weight: other.weight.clone().or_else(|| self.weight.clone()),
            line: other.line.clone().or_else(|| self.line.clone()),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == HeadStyle::default()
    }
}

/// `Appearance`: what skinparam and `<style>` set.
#[derive(Clone, Debug, Default)]
pub struct Appearance {
    pub min_width: Option<i64>,
    pub tab_fill: Option<String>,
    pub tab_colour: Option<String>,
    pub tab_size: Option<f64>,
    pub max_message: Option<i64>,
    pub arrow_width: Option<String>,
    pub alignment: Option<&'static str>,
    pub head_style: HeadStyle,
    pub paint: HashMap<&'static str, String>,
}

impl Appearance {
    pub fn alignment_or_center(&self) -> &'static str {
        self.alignment.unwrap_or("center")
    }

    pub fn merge(&self, other: &Appearance) -> Appearance {
        let mut paint = self.paint.clone();
        for (role, colour) in &other.paint {
            paint.insert(role, colour.clone());
        }
        Appearance {
            min_width: other.min_width.or(self.min_width),
            tab_fill: other.tab_fill.clone().or_else(|| self.tab_fill.clone()),
            tab_colour: other
                .tab_colour
                .clone()
                .or_else(|| self.tab_colour.clone()),
            tab_size: other.tab_size.or(self.tab_size),
            max_message: other.max_message.or(self.max_message),
            arrow_width: other
                .arrow_width
                .clone()
                .or_else(|| self.arrow_width.clone()),
            alignment: other.alignment.or(self.alignment),
            head_style: self.head_style.merge(&other.head_style),
            paint,
        }
    }
}

/// `Diagram`: what the parser returns.
#[derive(Clone, Debug)]
pub struct Diagram {
    pub participants: Vec<Participant>,
    pub items: Vec<Item>,
    pub boxes: Vec<Box_>,
    pub appearance: Appearance,
    pub footbox: bool,
    pub warnings: Vec<String>,
    pub title: Option<String>,
    pub chrome: Chrome,
    pub teoz: bool,
}

impl Diagram {
    pub fn min_head_width(&self) -> Option<i64> {
        self.appearance.min_width
    }

    pub fn messages(&self) -> Vec<&Message> {
        self.items.iter().filter_map(Item::as_message).collect()
    }
}
