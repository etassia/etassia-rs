//! The typed, final-canvas Scene: port of Ruby `Notation::PlantUML::Scene`
//! (`scene.rb`). Coordinates and drawable roles only; the renderer picks
//! colours, fonts and strokes from the theme and the style sheet.
use super::model::{Note, NoteSide};

#[derive(Clone, Debug)]
pub struct Icon {
    pub shape: String,
    pub visibility: String,
    pub filled: bool,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Clone, Debug)]
pub struct Text {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub role: &'static str,
    pub anchor: String,
    pub icon: Option<Icon>,
}

impl Text {
    pub fn new(content: &str, x: f64, y: f64, role: &'static str, anchor: &str) -> Text {
        Text {
            content: content.to_string(),
            x,
            y,
            role,
            anchor: anchor.to_string(),
            icon: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub width: Option<f64>,
    pub dash: Option<&'static str>,
}

impl Segment {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        Segment { x1, y1, x2, y2, width: None, dash: None }
    }
}

/// The circled kind letter of a class header.
#[derive(Clone, Debug)]
pub struct Spot {
    pub kind: String,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Clone, Debug)]
pub struct Box {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub texts: Vec<Text>,
    pub separators: Vec<Segment>,
    pub kind: &'static str,
    pub fill: Option<String>,
    pub spot: Option<Spot>,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub tab_width: Option<f64>,
    pub fill: Option<String>,
    pub icon: bool,
    pub texts: Vec<Text>,
}

/// An arrowhead or diamond (`points`), or the circled plus of a nested
/// class (`shape` "nesting", its plus sign the `cross` path).
#[derive(Clone, Debug)]
pub struct Marker {
    pub shape: &'static str,
    pub points: String,
    pub filled: bool,
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub cross: String,
}

#[derive(Clone, Debug)]
pub struct Relation {
    pub id: String,
    pub path: String,
    pub dashed: bool,
    pub dotted: bool,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub markers: Vec<Marker>,
    pub texts: Vec<Text>,
    pub qualifiers: Vec<Box>,
}

/// A title, header, footer, caption or legend panel.
#[derive(Clone, Debug)]
pub struct Panel {
    pub id: String,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<String>,
    pub bordered: bool,
    pub colour: Option<String>,
    pub font_size: f64,
    pub bold: bool,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub panels: Vec<Panel>,
    pub background: Option<String>,
    pub content_x: f64,
    pub content_y: f64,
    pub frames: Vec<Frame>,
    pub boxes: Vec<Box>,
    pub relations: Vec<Relation>,
}

/// Ruby `Scene::Box` built for a class or note; the note keeps its source
/// `Note` for the note-outline drawing.
#[derive(Clone, Debug)]
pub struct PlacedNote {
    pub note: Note,
}

/// Ruby `LabelClearance` input rect.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn mid_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    pub fn mid_y(&self) -> f64 {
        self.y + self.height / 2.0
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && other.x < self.right()
            && self.y < other.y + other.height
            && other.y < self.bottom()
    }

    pub fn moved(&self, across: f64, down: f64) -> Rect {
        Rect { x: self.x + across, y: self.y + down, width: self.width, height: self.height }
    }
}

/// Ruby `NoteSide` as the layout asks it.
pub fn side_is_horizontal(side: NoteSide) -> bool {
    matches!(side, NoteSide::Left | NoteSide::Right)
}
