//! Final-canvas geometry for a sequence diagram, and the Layout that
//! builds it. Port of `sequence/scene.rb` and the head/centre/frame
//! halves of `sequence/layout.rb`; the item rows live in `walker.rs`.

use super::model::*;
use super::walker;
use crate::Error;
use std::collections::HashMap;

/// `PlantUML::Scene::Text`.
#[derive(Clone, Debug, Default)]
pub struct Text {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub role: &'static str,
    pub anchor: &'static str,
    pub colour: Option<String>,
    pub size: Option<f64>,
    pub family: Option<String>,
    pub font_style: Option<String>,
    pub weight: Option<String>,
}

/// `PlantUML::Scene::Segment`.
#[derive(Clone, Debug, Default)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub dash: Option<&'static str>,
}

/// `Scene::Bar`: one activation bar.
#[derive(Clone, Debug, Default)]
pub struct Bar {
    pub fill: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// `Scene::Head::Rectangle`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
}

/// `Scene::Head::Ellipse`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadEllipse {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

/// `Scene::Head::Path`.
#[derive(Clone, Debug, Default)]
pub struct HeadPath {
    pub data: String,
    pub filled: bool,
}

/// `Scene::Head::Line`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadLine {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// `Scene::Head::Polygon`.
#[derive(Clone, Debug, Default)]
pub struct HeadPolygon {
    pub points: String,
}

/// `Scene::Head`.
#[derive(Clone, Debug, Default)]
pub struct Head {
    pub id: String,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<String>,
    pub fill_opacity: Option<f64>,
    pub stroke: Option<String>,
    pub texts: Vec<Text>,
    pub rectangles: Vec<HeadRectangle>,
    pub ellipses: Vec<HeadEllipse>,
    pub paths: Vec<HeadPath>,
    pub lines: Vec<HeadLine>,
    pub polygons: Vec<HeadPolygon>,
}

/// `Scene::Mark`: one shape at an arrow end.
#[derive(Clone, Debug, Default)]
pub struct Mark {
    pub kind: &'static str,
    pub filled: bool,
    pub heavy: bool,
    pub points: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

/// `Scene::Arrow`.
#[derive(Clone, Debug, Default)]
pub struct Arrow {
    pub id: String,
    pub path: String,
    pub dashed: bool,
    pub colour: Option<String>,
    pub marks: Vec<Mark>,
    pub texts: Vec<Text>,
}

/// `Scene::Frame` (a box round participants).
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<String>,
    pub texts: Vec<Text>,
}

/// `Scene::Fragment` (an alt frame or a ref).
#[derive(Clone, Debug, Default)]
pub struct Fragment {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub tab_path: String,
    pub tab_fill: Option<String>,
    pub separators: Vec<Segment>,
    pub texts: Vec<Text>,
}

/// `Scene::Note`.
#[derive(Clone, Debug, Default)]
pub struct NoteScene {
    pub picture: Option<(f64, f64, f64, Scene)>,
    pub path: String,
    pub fold_path: Option<String>,
    pub fill: Option<String>,
    pub fill_opacity: Option<f64>,
    pub texts: Vec<Text>,
}

/// `Scene::Banner` (a warning above the heads).
#[derive(Clone, Debug, Default)]
pub struct Banner {
    pub y: f64,
    pub x: f64,
    pub width: f64,
    pub height: f64,
    pub texts: Vec<Text>,
}

/// `Scene::TextBlock` (header, caption, footer).
#[derive(Clone, Debug, Default)]
pub struct TextBlock {
    pub id: &'static str,
    pub texts: Vec<Text>,
}

/// `Scene::Legend`.
#[derive(Clone, Debug, Default)]
pub struct Legend {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub texts: Vec<Text>,
}

/// `Scene::Title`.
#[derive(Clone, Debug, Default)]
pub struct Title {
    pub texts: Vec<Text>,
}

/// `Scene::Divider`.
#[derive(Clone, Debug, Default)]
pub struct Divider {
    pub lines: Vec<Segment>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub texts: Vec<Text>,
}

/// `Scene::Delay`.
#[derive(Clone, Debug, Default)]
pub struct Delay {
    pub y1: f64,
    pub y2: f64,
    pub texts: Vec<Text>,
}

/// `Scene`: the laid-out diagram.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub paint: Vec<String>,
    pub banners: Vec<Banner>,
    pub title: Option<Title>,
    pub text_blocks: Vec<TextBlock>,
    pub legend: Option<Legend>,
    pub frames: Vec<Frame>,
    pub fragments: Vec<Fragment>,
    pub notes: Vec<NoteScene>,
    pub dividers: Vec<Divider>,
    pub delays: Vec<Delay>,
    pub bars: Vec<Bar>,
    pub crosses: Vec<Segment>,
    pub page_breaks: Vec<Segment>,
    pub heads: Vec<Head>,
    pub lifelines: Vec<Segment>,
    pub arrows: Vec<Arrow>,
}

// ---- Layout: the head row and the canvas (sequence/layout.rb) ----

const MARGIN: f64 = 20.0;
const DELAY_DASH: &str = "1,4";
const HEAD_PADDING: f64 = 12.0;
const MIN_HEAD_WIDTH: f64 = 80.0;
const INTRINSIC_HEAD_PADDING: f64 = 7.0;
const INTRINSIC_HEAD_HEIGHT: f64 = 30.4883;
const MIN_GAP: f64 = 10.0;
const BOX_PADDING: f64 = 4.0;
const BOX_TITLE_SIZE: f64 = 13.0;
const BOLD_CHROME_SCALE: f64 = 0.9158;
const SELF_MIN_CLEARANCE: f64 = 50.0;
const BOX_TITLE_PADDING: f64 = 3.0;
const BOX_TITLE_HEIGHT: f64 = 26.0;
const MIN_WIDTH_PADDING: f64 = 14.0;
const HEAD_LEADING: f64 = 1.1776;
const EDGE_GAP: f64 = 5.0;
const EDGE_MARGIN: f64 = 10.0;
const PAGE_BREAK_INSET: f64 = 6.0;
const BANNER_LEFT: f64 = 3.0;
const BANNER_TOP: f64 = 8.0;
const BANNER_HEIGHT: f64 = 16.64;
const BANNER_STEP: f64 = 21.64;
const BANNER_PADDING: f64 = 7.0;
const MONOSPACE_ADVANCE: f64 = 6.02;
const GLYPH_HEAD_HEIGHT: f64 = 76.4883;
const GLYPH_BOX_HEIGHT: f64 = 30.4883;
const COMPACT_MARGIN: f64 = 10.0;
const COMPACT_HEAD_PADDING: f64 = 7.0;
const COMPACT_TIMELINE_GAP: f64 = 10.0;
const COMPACT_HEAD_HEIGHT: f64 = 30.4883;
const COMPACT_HEAD_LABEL_INSET: f64 = 9.9531;
const COMPACT_FIRST_ROW_GAP: f64 = 30.1328;
const COMPACT_GROUP_FIRST_ROW_GAP: f64 = 33.0;
const COMPACT_ROW: f64 = 29.1328;
const COMPACT_UNLABELLED_ROW: f64 = 14.0;
const COMPACT_GROUP_ROW: f64 = 13.3105;
const COMPACT_GROUP_CLOSE: f64 = 32.0;
const COMPACT_BRANCH_GAP: f64 = 28.0;
const TEOZ_FRAGMENT_BRANCH_GAP: f64 = 30.9551;
const TEOZ_FIRST_MESSAGE_GAP: f64 = 30.3105;
const TEOZ_FIRST_UNLABELLED_GAP: f64 = 15.0;
const COMPACT_FRAGMENT_CLOSE: f64 = 10.0;
const COMPACT_SELF_ROW: f64 = 42.1328;
const COMPACT_SELF_CLEARANCE: f64 = 53.0;
const CLASSIC_FIRST_ROW_GAP: f64 = 32.3105;
const CLASSIC_UNLABELLED_FIRST_ROW_GAP: f64 = 17.0;
const CLASSIC_FOLLOWING_UNLABELLED_ROW: f64 = 14.0;
const CLASSIC_ROW: f64 = 29.3105;
const CLASSIC_SELF_ROW: f64 = 42.3105;
const CLASSIC_SELF_CLEARANCE: f64 = 50.0;
pub const CLASSIC_LABEL_INSET: f64 = 7.0;
const CLASSIC_FRAGMENT_ROW: f64 = 24.3105;
const CLASSIC_BRANCH_GAP: f64 = 2.0;
const CLASSIC_FRAME_TOP: f64 = 14.3105;
const CLASSIC_FRAME_BOTTOM: f64 = 21.3105;
const CLASSIC_BRANCH_LINE: f64 = 20.3105;
const CLASSIC_FRAGMENT_CLOSE: f64 = 7.0;
const CLASSIC_DIVIDER_ROW: f64 = 42.3105;
const CLASSIC_DEACTIVATION_GAP: f64 = 31.0;
const CLASSIC_BARE_DEACTIVATION_GAP: f64 = 1.0;
const MIN_WIDTH_GLYPH_MARGIN: f64 = 5.0;
const MIN_WIDTH_GLYPH_GAP: f64 = 10.0;
const MIN_WIDTH_GLYPH_FIRST_ROW_GAP: f64 = 31.3105;
const ACTOR_INTRINSIC_WIDTH: f64 = 26.0;
const STEREOTYPE_BOX_HEIGHT: f64 = 46.9766;
const GLYPH_BOX_OFFSET: f64 = 4.0;
const QUEUE_HEIGHT: f64 = 26.4883;
const ICON_LABEL_INSET: f64 = 2.9531;
const TAIL_ICON_LABEL: f64 = 13.5352;
const BOX_LABEL_INSET: f64 = 10.9531;
const TAIL_BOX_LABEL: f64 = 20.5352;
const QUEUE_LABEL_INSET: f64 = 7.9531;
const TAIL_QUEUE_LABEL: f64 = 18.5352;
const ICON_RADIUS: f64 = 12.0;
const CREATE_HEAD_RISE: f64 = 21.31;
const CREATE_LIFELINE_LAG: f64 = 0.25;

/// `Drawing::Face.width`.
pub fn face_width(text: &str, size: f64, bold: bool) -> f64 {
    crate::text::measure(
        text,
        size,
        if bold { crate::text::Face::DejaVuBold } else { crate::text::Face::DejaVu },
    )
    .width
}

/// `Drawing::Face.widest_line`.
pub fn face_widest_line(text: &str, size: f64, bold: bool) -> f64 {
    split_lines(text)
        .iter()
        .map(|line| face_width(line, size, bold))
        .fold(0.0f64, f64::max)
}

/// `Layout#measure_text` (the Arial face).
pub fn measure_width(text: &str, size: f64) -> f64 {
    crate::text::measure(text, size, crate::text::Face::Arial).width
}

/// The glyph participants take an icon instead of a box.
fn glyph(kind: &str) -> bool {
    kind != "participant"
}

/// The `Walker::Config` equivalent.
pub struct LayoutConfig {
    pub row: f64,
    pub self_row: f64,
    pub unlabelled_row: Option<f64>,
    pub following_unlabelled_row: Option<f64>,
    pub fragment_row: f64,
    pub group_fragment_row: Option<f64>,
    pub branch_gap: f64,
    pub fragment_close_gap: f64,
    pub divider_row: f64,
    pub group_close_gap: Option<f64>,
    pub activation_edges: bool,
    pub teoz_fragment_cadence: bool,
    pub note_font_size: f64,
    pub deactivation_gap: f64,
    pub bare_deactivation_gap: f64,
    pub frame_top_offset: Option<f64>,
    pub frame_bottom_offset: Option<f64>,
    pub branch_offset: Option<f64>,
    pub bounds: (f64, f64),
    pub head_halves: HashMap<String, f64>,
    pub message_font_size: f64,
    pub label_inset: Option<f64>,
}

/// Lays the diagram out: heads, lifelines, items, canvas.
pub fn scene(diagram: &Diagram) -> Result<Scene, Error> {
    Layout::new(diagram)?.run()
}

struct Layout<'a> {
    diagram: &'a Diagram,
    origin: f64,
    top: f64,
    widths: Vec<f64>,
    centers: Vec<f64>,
    head_height: f64,
    flow: walker::Flow,
    edge_right: Option<f64>,
    creations: HashMap<usize, String>,
    activation_depths: HashMap<usize, HashMap<String, i64>>,
}

impl<'a> Layout<'a> {
    fn new(diagram: &'a Diagram) -> Result<Self, Error> {
        let mut layout = Layout {
            diagram,
            origin: 0.0,
            top: 0.0,
            widths: vec![],
            centers: vec![],
            head_height: 0.0,
            flow: walker::Flow::default(),
            edge_right: None,
            creations: HashMap::new(),
            activation_depths: HashMap::new(),
        };
        layout.prepare()?;
        Ok(layout)
    }

    fn ids(&self) -> Vec<&str> {
        self.diagram.participants.iter().map(|p| p.id.as_str()).collect()
    }

    fn compact(&self) -> bool {
        self.diagram.teoz
    }

    fn font_size(&self) -> f64 {
        14.0
    }

    fn message_font_size(&self) -> f64 {
        self.font_size() - 1.0
    }

    fn note_font_size(&self) -> f64 {
        super::geometry::SKIN_FONT_SIZE
    }

    fn head_style(&self) -> &HeadStyle {
        &self.diagram.appearance.head_style
    }

    fn head_font_size(&self) -> f64 {
        self.head_style().size.unwrap_or_else(|| self.font_size())
    }

    fn head_growth(&self) -> f64 {
        (self.head_font_size() - self.font_size()) * HEAD_LEADING
    }

    fn bold_heads(&self) -> bool {
        let style = self.head_style();
        let weight = style.weight.clone().unwrap_or_default();
        weight == "bold"
            || weight.parse::<i64>().map(|w| w >= 700).unwrap_or(false)
            || style.style.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("bold"))
    }

    fn glyph_participants(&self) -> bool {
        self.diagram.participants.iter().any(|p| glyph(p.kind))
    }

    fn plain_participants(&self) -> bool {
        self.diagram
            .participants
            .iter()
            .all(|p| p.kind == "participant" && p.stereotype.is_none())
    }

    fn group_fragment(&self) -> bool {
        self.diagram.items.iter().any(|item| match item {
            Item::Fragment(f) => f.phase == FragPhase::Open && f.keyword == "group",
            _ => false,
        })
    }


    fn boxed_plain_heads(&self) -> bool {
        !self.compact()
            && self.diagram.min_head_width().is_none()
            && !self.diagram.boxes.is_empty()
            && self.plain_participants()
    }

    fn ordinary_plain_heads(&self) -> bool {
        !self.compact()
            && self.diagram.min_head_width().is_none()
            && self.diagram.boxes.is_empty()
            && self.plain_participants()
            && !self.group_fragment()
    }

    fn intrinsic_group_heads(&self) -> bool {
        !self.compact()
            && self.diagram.min_head_width().is_none()
            && self.diagram.boxes.is_empty()
            && self.plain_participants()
            && self.group_fragment()
    }

    fn intrinsic_head_width(&self) -> bool {
        !self.compact() && self.diagram.min_head_width().is_none()
    }

    fn head_padding(&self) -> f64 {
        if self.compact() { COMPACT_HEAD_PADDING } else { HEAD_PADDING }
    }

    fn minimum_head_width(&self) -> f64 {
        if self.compact() { 0.0 } else { MIN_HEAD_WIDTH }
    }

    fn minimum_width_mixed_glyphs(&self) -> bool {
        if self.compact() || self.diagram.min_head_width().is_none() {
            return false;
        }
        self.diagram.participants.iter().any(|p| glyph(p.kind))
            && self.diagram.participants.iter().any(|p| !glyph(p.kind))
    }

    fn margin(&self) -> f64 {
        if self.minimum_width_mixed_glyphs() {
            MIN_WIDTH_GLYPH_MARGIN
        } else if self.compact() {
            COMPACT_MARGIN
        } else {
            MARGIN
        }
    }

    fn plain_head_height(&self) -> f64 {
        if self.compact() { COMPACT_HEAD_HEIGHT } else { 36.0 }
    }

    fn head_height_value(&self) -> f64 {
        if self.glyph_participants() {
            return GLYPH_HEAD_HEIGHT + self.head_growth();
        }
        let height = if self.intrinsic_group_heads()
            || self.boxed_plain_heads()
            || self.ordinary_plain_heads()
        {
            INTRINSIC_HEAD_HEIGHT
        } else if self.plain_participants() {
            self.plain_head_height()
        } else {
            56.0
        };
        height + self.head_growth()
    }

    fn head_width(&self, participant: &Participant) -> f64 {
        if self.minimum_width_mixed_glyphs() && participant.kind == "actor" {
            return ACTOR_INTRINSIC_WIDTH;
        }
        let (minimum, padding) = if self.intrinsic_head_width() {
            (0.0, INTRINSIC_HEAD_PADDING)
        } else {
            let padding = if self.compact() { self.head_padding() } else { INTRINSIC_HEAD_PADDING };
            (self.minimum_head_width(), padding)
        };
        let mut widths = vec![
            minimum,
            self.participant_label_width(participant) + 2.0 * padding,
            self.requested_width(),
        ];
        if !self.boxed_plain_heads() {
            widths.push(self.boxed_title_width(participant));
        }
        widths.into_iter().fold(f64::MIN, f64::max)
    }

    fn participant_label_width(&self, participant: &Participant) -> f64 {
        super::geometry::widest_line(
            &participant.label,
            self.head_font_size(),
            self.bold_heads(),
        )
    }

    fn requested_width(&self) -> f64 {
        match self.diagram.min_head_width() {
            None => 0.0,
            Some(min) => min as f64 + MIN_WIDTH_PADDING,
        }
    }

    fn boxed_title_width(&self, participant: &Participant) -> f64 {
        match self
            .diagram
            .boxes
            .iter()
            .find(|b| b.members.len() == 1 && b.members[0] == participant.id)
        {
            Some(box_) => self.title_width(box_),
            None => 0.0,
        }
    }

    fn title_width(&self, box_: &Box_) -> f64 {
        if self.boxed_plain_heads() {
            return face_widest_line(&box_.title, BOX_TITLE_SIZE, true);
        }
        measure_width(&box_.title, self.font_size())
    }

    fn required_gaps(&self, widths: &[f64]) -> Vec<f64> {
        let minimum_gap = if self.minimum_width_mixed_glyphs() {
            MIN_WIDTH_GLYPH_GAP
        } else if self.compact() {
            COMPACT_TIMELINE_GAP
        } else {
            MIN_GAP
        };
        let mut gaps: Vec<f64> = widths
            .windows(2)
            .enumerate()
            .map(|(index, pair)| (pair[0] + pair[1]) / 2.0 + minimum_gap)
            .collect();
        self.widen_for_items(&mut gaps);
        if self.boxed_plain_heads() {
            self.clear_box_titles(widths, &mut gaps);
        }
        gaps
    }

    fn clear_box_titles(&self, widths: &[f64], gaps: &mut [f64]) {
        for box_ in &self.diagram.boxes {
            let places = self.member_indexes(box_);
            let (Some(&first), Some(&last)) = (
                places.iter().min(),
                places.iter().max(),
            ) else {
                continue;
            };
            let extra = self.box_overhang(box_, widths, gaps);
            for index in [first.wrapping_sub(1), last] {
                if index < gaps.len() {
                    gaps[index] += extra;
                }
            }
        }
    }

    fn box_overhang(&self, box_: &Box_, widths: &[f64], gaps: &[f64]) -> f64 {
        let places = self.member_indexes(box_);
        let (Some(&first), Some(&last)) = (places.iter().min(), places.iter().max()) else {
            return 0.0;
        };
        let inner: f64 = gaps[first..last].iter().sum::<f64>()
            + (widths[first] + widths[last]) / 2.0;
        let need = self.title_width(box_) + 2.0 * BOX_TITLE_PADDING;
        ((need - inner - 2.0 * BOX_PADDING) / 2.0).max(0.0)
    }

    fn member_indexes(&self, box_: &Box_) -> Vec<usize> {
        let ids = self.ids();
        box_
            .members
            .iter()
            .map(|id| ids.iter().position(|known| *known == id.as_str()).unwrap_or(0))
            .collect()
    }

    fn widen_for_items(&self, gaps: &mut [f64]) {
        for message in self.diagram.messages() {
            self.widen(gaps, message);
        }
        for box_ in &self.diagram.boxes {
            self.widen_box(gaps, box_);
        }
        self.each_note(|note, span| self.widen_note(gaps, note, span));
        for item in &self.diagram.items {
            if let Item::Ref(ref_) = item {
                self.widen_ref(gaps, ref_);
            }
        }
    }

    fn each_note(&self, mut yield_: impl FnMut(&Note, (usize, usize))) {
        let ids = self.ids();
        let mut previous: Option<&Item> = None;
        for item in &self.diagram.items {
            if let Item::Note(note) = item {
                let free = if !note.attached() {
                    true
                } else {
                    !self.compact()
                        && !previous.is_some_and(|previous| {
                            matches!(previous, Item::Fragment(_))
                                || matches!(previous, Item::Message(m) if m.self_message())
                        })
                };
                if free {
                    let span = super::geometry::span(note, previous, &ids);
                    yield_(note, span);
                }
            }
            if !matches!(item, Item::Activation(_)) {
                previous = Some(item);
            }
        }
    }

    fn widen_note(&self, gaps: &mut [f64], note: &Note, span: (usize, usize)) {
        if !self.compact() {
            return;
        }
        if matches!(note.side, NoteSide::Left | NoteSide::Right) {
            return;
        }
        let need = super::geometry::natural_width(note, self.note_font_size());
        let (low, high) = span;
        match note.side {
            NoteSide::Left => raise_gap(gaps, low.wrapping_sub(1), need + 12.0),
            NoteSide::Right => raise_gap(gaps, high, need + 12.0),
            _ => {
                if low != high {
                    let between: Vec<usize> = (low..high).collect();
                    self.widen_over(gaps, &between, low, need);
                }
            }
        }
    }

    fn widen_ref(&self, gaps: &mut [f64], ref_: &Ref) {
        let ids = self.ids();
        let places: Vec<usize> = ref_
            .targets
            .iter()
            .map(|id| ids.iter().position(|known| *known == id.as_str()).unwrap_or(0))
            .collect();
        let Some(low) = places.iter().min().copied() else {
            return;
        };
        let Some(high) = places.iter().max().copied() else {
            return;
        };
        let between: Vec<usize> = (low..high).collect();
        let need = super::geometry::ref_width_for(&ref_.label);
        if between.is_empty() {
            self.widen_over(gaps, &between, low, need);
            return;
        }
        let share = (need - 2.0 * super::geometry::ref_side_pad()) / between.len() as f64;
        for index in between {
            raise_gap(gaps, index, share);
        }
    }

    fn widen_over(&self, gaps: &mut [f64], between: &[usize], index: usize, need: f64) {
        if between.is_empty() {
            let share = need / 2.0 + 12.0;
            raise_gap(gaps, index.wrapping_sub(1), share);
            raise_gap(gaps, index, share);
        } else {
            let share = (need - 20.0) / between.len() as f64;
            for index in between {
                raise_gap(gaps, *index, share);
            }
        }
    }

    fn widen_box(&self, gaps: &mut [f64], box_: &Box_) {
        if self.boxed_plain_heads() {
            return;
        }
        let places = self.member_indexes(box_);
        let (Some(&first), Some(&last)) = (places.iter().min(), places.iter().max()) else {
            return;
        };
        let span: Vec<usize> = (first..last).collect();
        let need = self.title_width(box_) / span.len() as f64;
        for index in span {
            raise_gap(gaps, index, need);
        }
    }

    fn widen(&self, gaps: &mut [f64], message: &Message) {
        if message.edge().is_some() {
            return;
        }
        let ids = self.ids();
        let low = ids.iter().position(|id| *id == message.from.id()).unwrap_or(0);
        let high = ids.iter().position(|id| *id == message.to.id()).unwrap_or(0);
        let (low, high) = (low.min(high), low.max(high));
        let span: Vec<usize> = if message.self_message() && message.leftward() {
            vec![low.wrapping_sub(1)]
        } else {
            (low..high.max(low + 1))
                .filter(|index| *index < gaps.len())
                .collect()
        };
        let need = self.message_room(message, low == high) + self.created_room(message);
        if span.is_empty() {
            return;
        }
        let share = need / span.len() as f64;
        for index in span {
            raise_gap(gaps, index, share);
        }
    }

    fn created_room(&self, message: &Message) -> f64 {
        let created = self
            .creations
            .values()
            .any(|id| id == &message.to.id().to_string());
        if !created {
            return 0.0;
        }
        self.half_width_of(&message.to.id())
    }

    fn half_width_of(&self, id: &str) -> f64 {
        let ids = self.ids();
        let index = ids.iter().position(|known| *known == id).unwrap_or(0);
        self.widths.get(index).copied().unwrap_or(0.0) / 2.0
    }

    fn message_room(&self, message: &Message, self_message: bool) -> f64 {
        let room = self.label_width(message) + 2.0 * self.head_padding();
        if !self.compact() {
            if !self_message {
                return room;
            }
            let classic = (self.label_width(message) + 2.0 * INTRINSIC_HEAD_PADDING)
                .max(SELF_MIN_CLEARANCE);
            if !self.nested_in_fragment(message) {
                return classic;
            }
            let depth = self
                .activation_depths
                .get(&self.item_key(message))
                .and_then(|depths| depths.get(message.from.id()).copied())
                .unwrap_or(0);
            let extra =
                if depth > 0 { (depth + 1) as f64 * walker::BAR_WIDTH } else { 0.0 };
            return classic.max(CLASSIC_SELF_CLEARANCE + extra);
        }
        if !self_message {
            let clearance = self.activation_bar_clearance(message);
            return room + super::geometry::ARROW_MARKS_LENGTH + clearance;
        }
        let label_room =
            walker::SELF_WIDTH.max(7.0 + self.sans_label_width(message));
        COMPACT_SELF_CLEARANCE.max(label_room + self.head_padding())
    }

    fn activation_bar_clearance(&self, message: &Message) -> f64 {
        let depths = self.contact_activation_depths(message);
        let ids = self.ids();
        let from = ids.iter().position(|id| *id == message.from.id()).unwrap_or(0);
        let to = ids.iter().position(|id| *id == message.to.id()).unwrap_or(0);
        let target = depths.get(message.to.id()).copied().unwrap_or(0);
        if from <= to {
            return active_bar_half(target);
        }
        let from_depth = depths.get(message.from.id()).copied().unwrap_or(0);
        (target as f64 * walker::BAR_WIDTH / 2.0) + active_bar_half(from_depth)
    }

    fn contact_activation_depths(&self, message: &Message) -> HashMap<String, i64> {
        let mut depths = self
            .activation_depths
            .get(&self.item_key(message))
            .cloned()
            .unwrap_or_default();
        let index = self.item_key(message);
        let following: Vec<Item> = self.diagram.items[index + 1..].to_vec();
        for item in walker::following_activations(&following, true) {
            if let Item::Activation(activation) = item {
                if activation.on() {
                    *depths.entry(activation.participant.clone()).or_insert(0) += 1;
                }
            }
        }
        depths
    }

    fn item_key(&self, message: &Message) -> usize {
        self.diagram
            .items
            .iter()
            .position(|item| {
                matches!(item, Item::Message(m) if std::ptr::eq(m, message))
                    || (item.as_message().is_some()
                        && item.as_message().unwrap().from == message.from
                        && item.as_message().unwrap().to == message.to
                        && item.as_message().unwrap().label == message.label)
            })
            .unwrap_or(0)
    }

    fn nested_in_fragment(&self, item: &Message) -> bool {
        let mut depth = 0;
        for candidate in &self.diagram.items {
            if let Item::Message(m) = candidate {
                if std::ptr::eq(m, item) {
                    return depth > 0;
                }
            }
            if let Item::Fragment(fragment) = candidate {
                match fragment.phase {
                    FragPhase::Open => depth += 1,
                    FragPhase::Close => depth -= 1,
                    FragPhase::Else => {}
                }
            }
        }
        false
    }

    fn sans_label_width(&self, message: &Message) -> f64 {
        let size = self.message_font_size();
        let measure = move |text: &str| super::geometry::widest_line(text, size, false);
        let wrap = super::geometry::MessageWrap::new(self.diagram.appearance.max_message, measure);
        wrap.width(&message.label) + self.numbering_extra(message)
    }

    fn label_width(&self, message: &Message) -> f64 {
        let measure = self.label_measure(message);
        let wrap =
            super::geometry::MessageWrap::new(self.diagram.appearance.max_message, &measure);
        wrap.width(&message.label) + self.numbering_extra(message)
    }

    fn label_measure(&self, message: &Message) -> Box<dyn Fn(&str) -> f64 + '_> {
        if message.label_family == Some("monospace") || !self.compact() {
            Box::new(move |text| self.label_text_width(text))
        } else {
            Box::new(move |text| {
                super::geometry::widest_line(text, self.message_font_size(), false)
            })
        }
    }

    fn label_text_width(&self, text: &str) -> f64 {
        if self.compact() {
            crate::text::measure_monospace(text, self.message_font_size()).width
        } else {
            super::geometry::widest_line(text, self.message_font_size(), false)
        }
    }

    fn numbering_extra(&self, message: &Message) -> f64 {
        match &message.number {
            None => 0.0,
            Some(number) => {
                super::geometry::numbered_extra(number, &|text| self.label_text_width(text))
            }
        }
    }

    fn edge_messages(&self, side: Side) -> Vec<&Message> {
        self.diagram
            .messages()
            .into_iter()
            .filter(|m| {
                matches!(m.edge(), Some(edge) if edge.global() && edge.side == side)
            })
            .collect()
    }

    fn edge_room(&self, message: &Message) -> f64 {
        let ring = if message.edge_end().circle {
            super::geometry::EDGE_RING_INSET
        } else {
            0.0
        };
        self.label_width(message) + super::geometry::EDGE_RUN_PADDING + ring
    }

    fn right_edge(&self) -> Option<f64> {
        let ids = self.ids();
        let ends: Vec<f64> = self
            .edge_messages(Side::Right)
            .into_iter()
            .map(|m| {
                let index = ids.iter().position(|id| *id == m.from.id()).unwrap_or(0);
                self.centers[index] + self.edge_room(m)
            })
            .collect();
        if ends.is_empty() {
            return None;
        }
        let bounds = self.head_bounds();
        Some(
            [bounds.1 + EDGE_GAP]
                .iter()
                .copied()
                .chain(ends)
                .fold(f64::MIN, f64::max),
        )
    }

    fn head_bounds(&self) -> (f64, f64) {
        (
            self.centers[0] - self.widths[0] / 2.0,
            self.centers[self.centers.len() - 1]
                + self.widths[self.widths.len() - 1] / 2.0,
        )
    }

    fn compact_timeline(&self) -> bool {
        self.compact()
            && self.group_fragment()
            && self
                .diagram
                .messages()
                .iter()
                .all(|m| m.label.is_empty())
    }

    fn teoz_fragment_cadence(&self) -> bool {
        self.compact()
            && self.diagram.items.iter().any(|item| matches!(item, Item::Fragment(_)))
            && !self.compact_timeline()
    }

    fn two_party_activation_timeline(&self) -> bool {
        self.compact()
            && self.diagram.participants.len() == 2
            && !self.diagram.items.iter().any(|item| matches!(item, Item::Fragment(_)))
            && self.diagram.items.iter().any(|item| matches!(item, Item::Activation(_)))
    }

    fn walker_config(&self) -> LayoutConfig {
        let row = if self.teoz_fragment_cadence() {
            CLASSIC_ROW
        } else if self.compact() {
            COMPACT_ROW
        } else {
            CLASSIC_ROW
        };
        let self_row = CLASSIC_SELF_ROW;
        let branch_gap = if self.teoz_fragment_cadence() {
            TEOZ_FRAGMENT_BRANCH_GAP
        } else if self.compact() {
            COMPACT_BRANCH_GAP
        } else {
            CLASSIC_BRANCH_GAP
        };
        let fragment_close_gap = if self.compact() {
            COMPACT_FRAGMENT_CLOSE
        } else {
            CLASSIC_FRAGMENT_CLOSE
        };
        let group_close_gap = if self.compact() {
            Some(COMPACT_GROUP_CLOSE)
        } else {
            Some(CLASSIC_FRAGMENT_CLOSE)
        };
        let divider_row = if self.compact() {
            walker::ROW
        } else {
            CLASSIC_DIVIDER_ROW
        };
        let deactivation_gap = if self.compact() { 0.0 } else { CLASSIC_DEACTIVATION_GAP };
        let bare_deactivation_gap =
            if self.compact() { 0.0 } else { CLASSIC_BARE_DEACTIVATION_GAP };
        let (frame_top_offset, frame_bottom_offset, branch_offset) = if self.compact() {
            (None, None, None)
        } else {
            (
                Some(CLASSIC_FRAME_TOP),
                Some(CLASSIC_FRAME_BOTTOM),
                Some(CLASSIC_BRANCH_LINE),
            )
        };
        LayoutConfig {
            row,
            self_row,
            unlabelled_row: self.compact_timeline().then_some(COMPACT_UNLABELLED_ROW),
            following_unlabelled_row: (!self.compact())
                .then_some(CLASSIC_FOLLOWING_UNLABELLED_ROW),
            fragment_row: if self.compact() {
                walker::TAB_HEIGHT
            } else {
                CLASSIC_FRAGMENT_ROW
            },
            group_fragment_row: self.compact_timeline().then_some(COMPACT_GROUP_ROW),
            branch_gap,
            fragment_close_gap,
            divider_row,
            group_close_gap,
            activation_edges: self.compact_timeline() || self.two_party_activation_timeline(),
            teoz_fragment_cadence: self.teoz_fragment_cadence(),
            note_font_size: self.note_font_size(),
            deactivation_gap,
            bare_deactivation_gap,
            frame_top_offset,
            frame_bottom_offset,
            branch_offset,
            bounds: self.head_bounds(),
            head_halves: self
                .ids()
                .iter()
                .enumerate()
                .map(|(index, id)| (id.to_string(), self.widths[index] / 2.0))
                .collect(),
            message_font_size: self.message_font_size(),
            label_inset: self.compact().then_some(7.0),
        }
    }

    fn first_row_gap(&self) -> f64 {
        if self.minimum_width_mixed_glyphs() {
            return MIN_WIDTH_GLYPH_FIRST_ROW_GAP;
        }
        if self.compact_timeline() {
            return COMPACT_GROUP_FIRST_ROW_GAP;
        }
        if self.teoz_fragment_cadence() {
            let first = self.diagram.items.iter().find(|item| !matches!(item, Item::Activation(_)));
            return match first {
                Some(Item::Fragment(_)) => COMPACT_GROUP_FIRST_ROW_GAP,
                Some(Item::Message(message)) if message.label.is_empty() => {
                    TEOZ_FIRST_UNLABELLED_GAP
                }
                _ => TEOZ_FIRST_MESSAGE_GAP,
            };
        }
        if self.classic_unlabelled_first_message() {
            return CLASSIC_UNLABELLED_FIRST_ROW_GAP;
        }
        if self.compact() {
            if self
                .diagram
                .messages()
                .first()
                .is_some_and(|m| m.label_family == Some("monospace"))
            {
                return COMPACT_FIRST_ROW_GAP;
            }
            return TEOZ_FIRST_MESSAGE_GAP;
        }
        CLASSIC_FIRST_ROW_GAP
    }

    fn classic_unlabelled_first_message(&self) -> bool {
        let first = self.diagram.items.first();
        if !matches!(first, Some(Item::Message(m)) if m.label.is_empty()) || self.compact() {
            return false;
        }
        !self.first_message_carries_picture()
    }

    fn first_message_carries_picture(&self) -> bool {
        let following = self
            .diagram
            .items
            .iter()
            .skip(1)
            .find(|item| !matches!(item, Item::Activation(_)));
        matches!(following, Some(Item::Note(note)) if note.attached() && note.picture.is_some())
    }

    fn prepare(&mut self) -> Result<(), Error> {
        // pictured(): notes with embedded diagrams are laid out first.
        let diagram = self.diagram.clone();
        let diagram = pictured(diagram)?;
        self.creations = walker::creation_messages(&diagram.items);
        self.activation_depths = activation_depths(&diagram.items);
        self.widths = diagram.participants.iter().map(|p| self.head_width(p)).collect();
        let gaps = self.required_gaps(&self.widths);
        let mut x = self.margin() + self.widths[0] / 2.0 + self.left_edge_room(&self.widths, &gaps);
        let mut centers = vec![];
        for index in 0..self.widths.len() {
            if index > 0 {
                x += gaps[index - 1];
            }
            centers.push(x);
        }
        self.centers = centers;
        self.head_height = self.head_height_value();
        self.origin = self.margin() + self.banner_room() + self.chrome_rows_top_room();
        self.top = self.origin
            + if diagram.boxes.is_empty() { 0.0 } else { BOX_TITLE_HEIGHT };
        self.edge_right = self.right_edge();
        let start_y = self.top + self.head_height + self.first_row_gap();
        let lifelines: HashMap<String, f64> = self
            .ids()
            .iter()
            .enumerate()
            .map(|(index, id)| (id.to_string(), self.centers[index]))
            .collect();
        self.flow = walker::walk(
            &diagram.items,
            lifelines,
            self.edge_right,
            &diagram.appearance,
            self.font_size(),
            &self.walker_config(),
            start_y,
            &|text| measure_width(text, self.font_size()),
            &|text| self.label_text_width(text),
        );
        Ok(())
    }
}

fn active_bar_half(depth: i64) -> f64 {
    if depth > 0 { walker::BAR_WIDTH / 2.0 } else { 0.0 }
}

fn raise_gap(gaps: &mut [f64], index: usize, need: f64) {
    if index < gaps.len() {
        gaps[index] = gaps[index].max(need);
    }
}

fn activation_depths(items: &[Item]) -> HashMap<usize, HashMap<String, i64>> {
    let mut active: HashMap<String, i64> = HashMap::new();
    let mut depths: HashMap<usize, HashMap<String, i64>> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            Item::Message(message) => {
                depths.insert(index, active.clone());
            }
            Item::Activation(activation) => {
                let delta = if activation.on() { 1 } else { -1 };
                let depth = active.get(&activation.participant).copied().unwrap_or(0) + delta;
                active.insert(activation.participant.clone(), depth.max(0));
            }
            Item::Destroy(destroy) => {
                active.insert(destroy.participant.clone(), 0);
            }
            _ => {}
        }
    }
    depths
}

/// Lays out each note's embedded diagram, so the notes know their size.
fn pictured(diagram: Diagram) -> Result<Diagram, Error> {
    let embeds = diagram.items.iter().any(|item| {
        matches!(item, Item::Note(note) if super::read::embeds_any(&note.text))
    });
    if !embeds {
        return Ok(diagram);
    }
    let mut items = diagram.items.clone();
    for item in &mut items {
        let Item::Note(note) = item else { continue };
        let Some(embedded) = super::read::embedded_read(&note.text) else {
            continue;
        };
        let inner = super::read::read_document(&embedded.document)?;
        let inner_scene = scene(&inner)?;
        note.picture = Some(Picture {
            scene: std::sync::Arc::new(inner_scene),
            scale: embedded.scale,
        });
    }
    Ok(Diagram { items, ..diagram })
}

impl<'a> Layout<'a> {
    fn left_edge_room(&self, widths: &[f64], gaps: &[f64]) -> f64 {
        let start = self.margin() + widths[0] / 2.0;
        let ids = self.ids();
        let mut short: Vec<f64> = self
            .edge_messages(Side::Left)
            .into_iter()
            .map(|m| {
                let to = ids.iter().position(|id| *id == m.to.id()).unwrap_or(0);
                let sum: f64 = gaps[..to.min(gaps.len())].iter().sum();
                self.edge_room(m) - start - sum
            })
            .collect();
        if self.boxed_plain_heads() {
            for box_ in &self.diagram.boxes {
                let places = self.member_indexes(box_);
                if places.iter().min().copied() == Some(0) {
                    short.push(self.box_overhang(box_, widths, gaps));
                }
            }
        }
        short.push(0.0);
        short.into_iter().fold(f64::MIN, f64::max)
    }

    fn banner_room(&self) -> f64 {
        self.diagram.warnings.len() as f64 * BANNER_STEP
    }

    fn chrome_rows_top_room(&self) -> f64 {
        let rows = ChromeRows::new(self.diagram);
        rows.top_room()
    }

    fn run(mut self) -> Result<Scene, Error> {
        let mut flow = std::mem::take(&mut self.flow);
        if flow.left < self.margin() {
            let shift = self.margin() - flow.left;
            for centre in &mut self.centers {
                *centre += shift;
            }
            let lifelines: HashMap<String, f64> = self
                .ids()
                .iter()
                .enumerate()
                .map(|(index, id)| (id.to_string(), self.centers[index]))
                .collect();
            flow = walker::walk(
                &self.diagram.items,
                lifelines,
                self.edge_right,
                &self.diagram.appearance,
                self.font_size(),
                &self.walker_config(),
                self.top + self.head_height + self.first_row_gap(),
                &|text| measure_width(text, self.font_size()),
                &|text| self.label_text_width(text),
            );
        }
        self.flow = flow;
        Ok(self.build())
    }

    fn foot_height(&self) -> f64 {
        if self.diagram.footbox { self.head_height } else { 0.0 }
    }

    fn canvas_width(&self) -> f64 {
        let ids = self.ids();
        let last = self.centers.last().copied().unwrap_or(0.0);
        let right = last + self.widths.last().copied().unwrap_or(0.0) / 2.0;
        let self_room =
            self.self_reach(self.diagram.participants.len().saturating_sub(1), &ids);
        let padded = (right.max(last + self_room)).max(self.flow.right) + self.margin();
        let edge_extent = self
            .edge_right
            .map(|right| right + EDGE_MARGIN)
            .unwrap_or(0.0);
        let banner_extent = if self.diagram.warnings.is_empty() {
            0.0
        } else {
            let widest = self
                .diagram
                .warnings
                .iter()
                .map(|line| self.banner_width(line))
                .fold(f64::MIN, f64::max);
            BANNER_LEFT + widest + BANNER_LEFT
        };
        let rows = ChromeRows::new(self.diagram);
        padded
            .max(edge_extent)
            .max(banner_extent)
            .max(rows.width(self.font_size()))
    }

    fn banner_width(&self, line: &str) -> f64 {
        let text = line.chars().count() as f64 * MONOSPACE_ADVANCE;
        round2(text + 2.0 * BANNER_PADDING)
    }

    fn self_reach(&self, index: usize, ids: &[&str]) -> f64 {
        let labels: Vec<f64> = self
            .diagram
            .messages()
            .into_iter()
            .filter(|m| {
                m.self_message()
                    && !m.leftward()
                    && ids.iter().position(|id| *id == m.from.id()) == Some(index)
            })
            .map(|m| self.label_width(m))
            .collect();
        if labels.is_empty() {
            return 0.0;
        }
        let widest = labels.into_iter().fold(f64::MIN, f64::max);
        if self.compact() {
            walker::SELF_WIDTH + 6.0 + widest
        } else {
            walker::SELF_WIDTH.max(CLASSIC_LABEL_INSET + widest)
        }
    }

    fn build(&self) -> Scene {
        let rows = ChromeRows::new(self.diagram);
        let width = self.canvas_width();
        let height = self.flow.y + self.foot_height() + self.margin() + rows.bottom_room();
        let chrome_top = self.margin() + self.banner_room();
        Scene {
            width,
            height,
            paint: self.scene_paint(),
            banners: self.banners(),
            title: rows.title(chrome_top, width, self.font_size()),
            text_blocks: rows.blocks(
                chrome_top,
                self.flow.y + self.foot_height(),
                width,
                self.font_size(),
            ),
            legend: rows.legend(
                chrome_top,
                self.flow.y + self.foot_height(),
                width,
                self.font_size(),
            ),
            frames: self.frames(),
            fragments: self.flow.fragments.clone(),
            notes: self.flow.notes.clone(),
            dividers: self.flow.dividers.clone(),
            delays: self.flow.delays.clone(),
            bars: self.flow.bars.clone(),
            crosses: self.flow.crosses.clone(),
            page_breaks: self
                .flow
                .page_breaks
                .iter()
                .map(|y| Segment { x1: 0.0, y1: *y, x2: width - PAGE_BREAK_INSET, y2: *y, dash: None })
                .collect(),
            heads: self.heads(),
            lifelines: self.lifelines(),
            arrows: self.flow.arrows.clone(),
        }
    }

    fn scene_paint(&self) -> Vec<String> {
        let appearance = &self.diagram.appearance;
        let mut pairs: Vec<String> = appearance
            .paint
            .iter()
            .map(|(role, colour)| format!("{role}={colour}"))
            .collect();
        if let Some(width) = &appearance.arrow_width {
            pairs.push(format!("arrow_width={width}"));
        }
        pairs
    }

    fn banners(&self) -> Vec<Banner> {
        self.diagram
            .warnings
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let top = BANNER_TOP + index as f64 * BANNER_STEP;
                let width = self.banner_width(line);
                Banner {
                    x: BANNER_LEFT,
                    y: top,
                    width,
                    height: BANNER_HEIGHT,
                    texts: vec![Text {
                        content: line.clone(),
                        x: BANNER_LEFT + BANNER_PADDING,
                        y: top + BANNER_HEIGHT - 6.0,
                        role: "warning",
                        anchor: "start",
                        ..Text::default()
                    }],
                }
            })
            .collect()
    }

    fn frames(&self) -> Vec<Frame> {
        self.diagram
            .boxes
            .iter()
            .map(|box_| {
                let places = self.member_indexes(box_);
                let (Some(&first), Some(&last)) = (places.iter().min(), places.iter().max())
                else {
                    return Frame::default();
                };
                let mut left = self.centers[first] - self.widths[first] / 2.0 - BOX_PADDING;
                let mut right =
                    self.centers[last] + self.widths[last] / 2.0 + BOX_PADDING;
                if self.boxed_plain_heads() {
                    let shortfall = self.title_width(box_) + 2.0 * BOX_TITLE_PADDING
                        - (right - left);
                    if shortfall > 0.0 {
                        left -= shortfall / 2.0;
                        right += shortfall / 2.0;
                    }
                }
                Frame {
                    x: left,
                    y: self.origin,
                    width: right - left,
                    height: self.flow.y + self.foot_height() + BOX_PADDING - self.origin,
                    fill: box_.fill.as_ref().map(|fill| fill.colour.clone()),
                    texts: vec![Text {
                        content: box_.title.clone(),
                        x: (left + right) / 2.0,
                        y: self.origin + 17.0,
                        role: "box",
                        anchor: "middle",
                        ..Text::default()
                    }],
                }
            })
            .collect()
    }

    fn lifelines(&self) -> Vec<Segment> {
        let ids = self.ids();
        let mut out = vec![];
        for (index, id) in ids.iter().enumerate() {
            let centre = self.centers[index];
            let top = self.lifeline_top(id);
            let bottom = self.flow.y;
            let mut at = top;
            for gap in &self.flow.delays {
                if gap.y2 <= gap.y1 || gap.y1 < at || gap.y2 > bottom {
                    continue;
                }
                out.push(self.lifeline_segment(centre, at, gap.y1, None));
                out.push(self.lifeline_segment(centre, gap.y1, gap.y2, Some(DELAY_DASH)));
                at = gap.y2;
            }
            out.push(self.lifeline_segment(centre, at, bottom, None));
        }
        out
    }

    fn lifeline_segment(&self, centre: f64, from: f64, to: f64, dash: Option<&'static str>) -> Segment {
        Segment { x1: centre, y1: from, x2: centre, y2: to, dash }
    }

    fn lifeline_top(&self, id: &str) -> f64 {
        match self.flow.created.get(id) {
            None => self.top + self.head_height,
            Some(row) => row - CREATE_HEAD_RISE + self.head_height + CREATE_LIFELINE_LAG,
        }
    }

    fn heads(&self) -> Vec<Head> {
        let mut out = self.place_heads(self.top, false);
        if self.diagram.footbox {
            out.extend(self.place_heads(self.flow.y, true));
        }
        out
    }

    fn place_heads(&self, top: f64, tail: bool) -> Vec<Head> {
        self.diagram
            .participants
            .iter()
            .enumerate()
            .map(|(index, participant)| {
                let at = if tail {
                    top
                } else {
                    match self.flow.created.get(&participant.id) {
                        Some(row) => row - CREATE_HEAD_RISE,
                        None => top,
                    }
                };
                self.head(participant, self.centers[index], self.widths[index], at, tail)
            })
            .collect()
    }

    fn head(&self, participant: &Participant, centre: f64, width: f64, top: f64, tail: bool) -> Head {
        let mut head = Head {
            id: participant.id.clone(),
            kind: participant.kind,
            x: centre - width / 2.0,
            y: top,
            width,
            height: self.head_height,
            fill: participant.fill.as_ref().map(|fill| fill.colour.clone()),
            fill_opacity: participant.fill.as_ref().and_then(|fill| fill.opacity),
            stroke: self.head_style().line.clone(),
            texts: self.head_texts(participant, centre, top, tail),
            ..Head::default()
        };
        let geometry = self.head_geometry(participant, centre, width, top, tail);
        head.rectangles = geometry.0;
        head.ellipses = geometry.1;
        head.paths = geometry.2;
        head.lines = geometry.3;
        head.polygons = geometry.4;
        head
    }

    #[allow(clippy::type_complexity)]
    fn head_geometry(
        &self,
        participant: &Participant,
        centre: f64,
        width: f64,
        top: f64,
        tail: bool,
    ) -> (
        Vec<HeadRectangle>,
        Vec<HeadEllipse>,
        Vec<HeadPath>,
        Vec<HeadLine>,
        Vec<HeadPolygon>,
    ) {
        match participant.kind {
            "actor" => self.actor_geometry(centre, top, tail),
            "boundary" => self.boundary_geometry(centre, top, tail),
            "control" => self.control_geometry(centre, top, tail),
            "entity" => self.entity_geometry(centre, top, tail),
            "database" => self.database_geometry(centre, top, tail),
            "queue" => self.queue_geometry(centre, width, top, tail),
            "collections" => self.collections_geometry(centre, width, top, tail),
            _ => self.rectangle_geometry(participant, centre, width, top, tail),
        }
    }

    fn rectangle_geometry(
        &self,
        participant: &Participant,
        centre: f64,
        width: f64,
        top: f64,
        tail: bool,
    ) -> LayoutGeometry {
        let left = centre - width / 2.0;
        let height = if self.glyph_participants() {
            self.participant_box_height(participant)
        } else {
            self.head_height
        };
        let y = if !self.glyph_participants() || tail {
            top
        } else {
            top + self.head_height - height - 1.0
        };
        let radius = if self.glyph_participants() { 2.5 } else { 3.0 };
        (
            vec![HeadRectangle {
                x: left,
                y,
                width,
                height,
                rx: Some(radius),
                ry: Some(radius),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )
    }

    fn collections_geometry(&self, centre: f64, width: f64, top: f64, tail: bool) -> LayoutGeometry {
        let left = centre - width / 2.0;
        let y = if tail { top } else { top + self.head_height - GLYPH_BOX_HEIGHT - 1.0 };
        let glyph_width = width - GLYPH_BOX_OFFSET;
        let radius = if self.compact() { 3.0 } else { 3.0 };
        (
            vec![
                HeadRectangle {
                    x: left + GLYPH_BOX_OFFSET,
                    y: y - GLYPH_BOX_OFFSET,
                    width: glyph_width,
                    height: GLYPH_BOX_HEIGHT,
                    rx: None,
                    ry: None,
                },
                HeadRectangle {
                    x: left,
                    y,
                    width: glyph_width,
                    height: GLYPH_BOX_HEIGHT,
                    rx: None,
                    ry: None,
                },
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        )
    }

    fn actor_geometry(&self, centre: f64, top: f64, tail: bool) -> LayoutGeometry {
        let centre_y = if tail { top + self.head_height - 51.5 } else { top + 8.5 };
        let body_top = centre_y + 8.0;
        let body_bottom = centre_y + 35.0;
        let foot = centre_y + 50.0;
        let data = format!(
            "M {centre} {body_top} L {centre} {body_bottom} M {} {} L {} {} M {centre} {body_bottom} L {} {foot} M {centre} {body_bottom} L {} {foot}",
            fmt(centre - 13.0),
            fmt(body_top + 8.0),
            fmt(centre + 13.0),
            fmt(body_top + 8.0),
            fmt(centre - 13.0),
            fmt(centre + 13.0),
        );
        (
            vec![],
            vec![HeadEllipse { cx: centre, cy: centre_y, rx: 8.0, ry: 8.0 }],
            vec![HeadPath { data, filled: false }],
            vec![],
            vec![],
        )
    }

    fn boundary_geometry(&self, centre: f64, top: f64, tail: bool) -> LayoutGeometry {
        let centre_y = self.icon_centre_y(top, tail);
        let stem_x = centre - 29.0;
        let data = format!(
            "M {} {} L {} {} M {} {} L {} {}",
            fmt(stem_x),
            fmt(centre_y - 12.0),
            fmt(stem_x),
            fmt(centre_y + 12.0),
            fmt(stem_x),
            fmt(centre_y),
            fmt(centre - 12.0),
            fmt(centre_y),
        );
        (
            vec![],
            vec![self.icon_ellipse(centre, centre_y)],
            vec![HeadPath { data, filled: false }],
            vec![],
            vec![],
        )
    }

    fn control_geometry(&self, centre: f64, top: f64, tail: bool) -> LayoutGeometry {
        let centre_y = self.icon_centre_y(top, tail);
        let points = [
            [centre - 4.0, centre_y - 12.0],
            [centre + 2.0, centre_y - 17.0],
            [centre, centre_y - 12.0],
            [centre + 2.0, centre_y - 7.0],
        ];
        let mut all = points.to_vec();
        all.push(points[0]);
        let polygon = polygon_points(&all);
        (
            vec![],
            vec![self.icon_ellipse(centre, centre_y)],
            vec![],
            vec![],
            vec![HeadPolygon { points: polygon }],
        )
    }

    fn entity_geometry(&self, centre: f64, top: f64, tail: bool) -> LayoutGeometry {
        let centre_y = self.icon_centre_y(top, tail);
        (
            vec![],
            vec![self.icon_ellipse(centre, centre_y)],
            vec![],
            vec![HeadLine {
                x1: centre - ICON_RADIUS,
                y1: centre_y + 14.0,
                x2: centre + ICON_RADIUS,
                y2: centre_y + 14.0,
            }],
            vec![],
        )
    }

    fn icon_ellipse(&self, centre: f64, centre_y: f64) -> HeadEllipse {
        HeadEllipse { cx: centre, cy: centre_y, rx: ICON_RADIUS, ry: ICON_RADIUS }
    }

    fn icon_centre_y(&self, top: f64, tail: bool) -> f64 {
        if tail { top + self.head_height - 44.0 } else { top + 44.0 }
    }

    fn database_geometry(&self, centre: f64, top: f64, tail: bool) -> LayoutGeometry {
        let y = if tail { top + self.head_height - 60.0 } else { top + 14.0 };
        let left = centre - 18.0;
        let right = centre + 18.0;
        let outline = format!(
            "M {} {} C {} {} {} {} {} {} L {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} L {} {}",
            fmt(left), fmt(y + 10.0),
            fmt(left), fmt(y), fmt(centre), fmt(y), fmt(centre), fmt(y),
            fmt(right), fmt(y + 36.0),
            fmt(right), fmt(y + 46.0), fmt(centre), fmt(y + 46.0), fmt(centre), fmt(y + 46.0),
            fmt(centre), fmt(y + 46.0), fmt(left), fmt(y + 46.0), fmt(left), fmt(y + 36.0),
            fmt(left), fmt(y + 10.0),
        );
        let rim = format!(
            "M {} {} C {} {} {} {} {} {} C {} {} {} {} {} {}",
            fmt(left), fmt(y + 10.0),
            fmt(left), fmt(y + 20.0), fmt(centre), fmt(y + 20.0), fmt(centre), fmt(y + 20.0),
            fmt(centre), fmt(y + 20.0), fmt(right), fmt(y + 20.0), fmt(right), fmt(y + 10.0),
        );
        (
            vec![],
            vec![],
            vec![
                HeadPath { data: outline, filled: true },
                HeadPath { data: rim, filled: false },
            ],
            vec![],
            vec![],
        )
    }

    fn queue_geometry(&self, centre: f64, width: f64, top: f64, tail: bool) -> LayoutGeometry {
        let y = if tail { top } else { top + self.head_height - QUEUE_HEIGHT };
        let left = centre - width / 2.0;
        let right = left + width;
        let middle = y + QUEUE_HEIGHT / 2.0;
        let outline = format!(
            "M {} {} L {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} L {} {} C {} {} {} {} {} {} C {} {} {} {} {} {}",
            fmt(left + 5.0), fmt(y),
            fmt(right - 5.0), fmt(y),
            fmt(right), fmt(y), fmt(right), fmt(middle), fmt(right), fmt(middle),
            fmt(right), fmt(middle), fmt(right), fmt(y + QUEUE_HEIGHT), fmt(right - 5.0), fmt(y + QUEUE_HEIGHT),
            fmt(left + 5.0), fmt(y + QUEUE_HEIGHT),
            fmt(left), fmt(y + QUEUE_HEIGHT), fmt(left), fmt(middle), fmt(left), fmt(middle),
            fmt(left), fmt(middle), fmt(left), fmt(y), fmt(left + 5.0), fmt(y),
        );
        let rim = format!(
            "M {} {} C {} {} {} {} {} {} C {} {} {} {} {} {}",
            fmt(right - 5.0), fmt(y),
            fmt(right - 10.0), fmt(y), fmt(right - 10.0), fmt(middle), fmt(right - 10.0), fmt(middle),
            fmt(right - 10.0), fmt(y + QUEUE_HEIGHT), fmt(right - 5.0), fmt(y + QUEUE_HEIGHT), fmt(right - 5.0), fmt(y + QUEUE_HEIGHT),
        );
        (
            vec![],
            vec![],
            vec![
                HeadPath { data: outline, filled: true },
                HeadPath { data: rim, filled: false },
            ],
            vec![],
            vec![],
        )
    }

    fn participant_box_height(&self, participant: &Participant) -> f64 {
        if self.minimum_width_mixed_glyphs() {
            return if participant.stereotype.is_some() {
                STEREOTYPE_BOX_HEIGHT
            } else {
                GLYPH_BOX_HEIGHT
            };
        }
        if self.diagram.participants.iter().any(|p| p.stereotype.is_some()) {
            56.0
        } else {
            GLYPH_BOX_HEIGHT
        }
    }

    fn head_texts(&self, participant: &Participant, centre: f64, top: f64, tail: bool) -> Vec<Text> {
        if !glyph(participant.kind) {
            return self.participant_texts(participant, centre, top);
        }
        self.glyph_texts(participant, centre, top, tail)
    }

    fn participant_texts(&self, participant: &Participant, centre: f64, top: f64) -> Vec<Text> {
        let baseline = top + self.head_height - self.head_label_inset();
        let mut label = Text {
            content: participant.label.clone(),
            x: centre,
            y: baseline,
            role: "participant",
            anchor: "middle",
            ..Text::default()
        };
        self.restyle(&mut label);
        let Some(stereotype) = &participant.stereotype else {
            return vec![label];
        };
        let name = format!("«{stereotype}»");
        let mut lines = vec![
            Text {
                content: name,
                x: centre,
                y: top + 14.0,
                role: "kind",
                anchor: "middle",
                ..Text::default()
            },
            label,
        ];
        if participant.stereotype.is_some() {
            self.align(&mut lines, centre);
        }
        lines
    }

    fn glyph_texts(&self, participant: &Participant, centre: f64, top: f64, tail: bool) -> Vec<Text> {
        let baseline = top + self.glyph_label_offset(participant.kind, tail);
        let mut label = Text {
            content: participant.label.clone(),
            x: centre,
            y: baseline,
            role: "participant",
            anchor: "middle",
            ..Text::default()
        };
        self.restyle(&mut label);
        let Some(stereotype) = &participant.stereotype else {
            return vec![label];
        };
        let mut stereotype_text = Text {
            content: format!("«{stereotype}»"),
            x: centre,
            y: baseline - 18.0,
            role: "kind",
            anchor: "middle",
            ..Text::default()
        };
        let _ = &mut stereotype_text;
        let mut lines = vec![stereotype_text, label];
        self.align(&mut lines, centre);
        lines
    }

    fn glyph_label_offset(&self, kind: &str, tail: bool) -> f64 {
        let (tail_label, inset) = match kind {
            "collections" | "participant" => (TAIL_BOX_LABEL, BOX_LABEL_INSET),
            "queue" => (TAIL_QUEUE_LABEL, QUEUE_LABEL_INSET),
            _ => (TAIL_ICON_LABEL, ICON_LABEL_INSET),
        };
        if tail {
            tail_label
        } else {
            self.head_height - inset
        }
    }

    fn head_label_inset(&self) -> f64 {
        if self.compact() { COMPACT_HEAD_LABEL_INSET } else { 12.0 }
    }

    fn restyle(&self, text: &mut Text) {
        let style = self.head_style();
        text.colour = style.colour.clone();
        text.size = style.size;
        text.family = style.family.clone();
        text.font_style = style.style.clone();
        text.weight = style.weight.clone();
    }

    fn align(&self, lines: &mut [Text], centre: f64) {
        let alignment = self.diagram.appearance.alignment_or_center();
        if alignment == "center" {
            return;
        }
        let half = lines
            .iter()
            .map(|line| self.line_width(line))
            .fold(f64::MIN, f64::max) / 2.0;
        for line in lines.iter_mut() {
            if alignment == "left" {
                line.anchor = "start";
                line.x = centre - half;
            } else {
                line.anchor = "end";
                line.x = centre + half;
            }
        }
    }

    fn line_width(&self, line: &Text) -> f64 {
        let scale = if line.role == "kind" { 0.85 } else { 1.0 };
        measure_width(&line.content, self.font_size() * scale)
    }
}

type LayoutGeometry = (
    Vec<HeadRectangle>,
    Vec<HeadEllipse>,
    Vec<HeadPath>,
    Vec<HeadLine>,
    Vec<HeadPolygon>,
);

fn fmt(value: f64) -> String {
    crate::svg::ruby_float(value)
}

fn polygon_points(points: &[[f64; 2]]) -> String {
    points
        .iter()
        .map(|point| format!("{},{}", fmt(point[0]), fmt(point[1])))
        .collect::<Vec<_>>()
        .join(" ")
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// `ChromeRows`: sizes and places the chrome texts.
struct ChromeRows<'a> {
    diagram: &'a Diagram,
}

const CHROME_LINE: f64 = 16.49;

struct ChromeStyle {
    step: f64,
    lead: f64,
    base: f64,
    anchor: &'static str,
    size: Option<f64>,
    colour: Option<&'static str>,
    pad: f64,
    bold: bool,
}

fn chrome_style(kind: &str) -> ChromeStyle {
    match kind {
        "header" => ChromeStyle {
            step: 11.78, lead: 1.0, base: 9.67, anchor: "end",
            size: Some(10.0), colour: Some("#888888"), pad: 0.0, bold: false,
        },
        "title" => ChromeStyle {
            step: CHROME_LINE, lead: 21.0, base: 23.54, anchor: "middle",
            size: None, colour: None, pad: 27.0, bold: true,
        },
        "caption" => ChromeStyle {
            step: CHROME_LINE, lead: 3.0, base: 16.54, anchor: "middle",
            size: None, colour: None, pad: 2.0, bold: false,
        },
        _ => ChromeStyle {
            step: 11.78, lead: 1.0, base: 11.67, anchor: "middle",
            size: Some(10.0), colour: Some("#888888"), pad: 0.0, bold: false,
        },
    }
}

const LEGEND_BEFORE_TOP: f64 = 12.0;
const LEGEND_BEFORE_BOTTOM: f64 = 14.0;
const LEGEND_AFTER_TOP: f64 = 13.0;
const LEGEND_AFTER_BOTTOM: f64 = 11.0;
const LEGEND_BOX: f64 = 10.0;
const LEGEND_SIDE: f64 = 12.0;
const LEGEND_PAD: f64 = 5.0;
const LEGEND_BASE: f64 = 18.54;

impl<'a> ChromeRows<'a> {
    fn new(diagram: &'a Diagram) -> Self {
        ChromeRows { diagram }
    }

    fn lines(&self, kind: &str) -> Vec<String> {
        // Ruby String#split("\n"): a trailing empty line is dropped.
        let text = if kind == "title" {
            self.diagram.title.clone().unwrap_or_default()
        } else {
            self.diagram.chrome.lines(kind).join("\n")
        };
        ruby_split_lines(&text)
    }

    fn room(&self, kind: &str) -> f64 {
        let count = self.lines(kind).len();
        if count == 0 {
            return 0.0;
        }
        let style = chrome_style(kind);
        style.lead + style.step * count as f64
    }

    fn legend_room(&self, edge: &str) -> f64 {
        if self.lines("legend").is_empty()
            || !self.diagram.chrome.legend_place.starts_with(edge)
        {
            return 0.0;
        }
        let (before, after) = if edge == "top" {
            (LEGEND_BEFORE_TOP, LEGEND_AFTER_TOP)
        } else {
            (LEGEND_BEFORE_BOTTOM, LEGEND_AFTER_BOTTOM)
        };
        before + self.legend_height() + after
    }

    fn legend_height(&self) -> f64 {
        LEGEND_BOX + CHROME_LINE * self.lines("legend").len() as f64
    }

    fn top_room(&self) -> f64 {
        self.room("header") + self.room("title") + self.legend_room("top")
    }

    fn bottom_room(&self) -> f64 {
        self.legend_room("bottom") + self.room("caption") + self.room("footer")
    }

    fn chrome_width(&self, text: &str, size: Option<f64>, bold: bool, font_size: f64) -> f64 {
        let size = size.unwrap_or(font_size);
        if bold {
            return face_width(text, size, true) * BOLD_CHROME_SCALE;
        }
        measure_width(text, size)
    }

    fn width(&self, font_size: f64) -> f64 {
        let mut widths: Vec<f64> = ["header", "title", "caption", "footer"]
            .iter()
            .map(|kind| {
                let style = chrome_style(kind);
                let widest = self
                    .lines(kind)
                    .iter()
                    .map(|line| {
                        self.chrome_width(line, style.size, style.bold, font_size)
                    })
                    .fold(f64::MIN, f64::max);
                if self.lines(kind).is_empty() {
                    0.0
                } else {
                    widest + style.pad
                }
            })
            .collect();
        widths.push(self.legend_width() + 2.0 * LEGEND_SIDE);
        widths.into_iter().fold(f64::MIN, f64::max)
    }

    fn legend_width(&self) -> f64 {
        let widest = self
            .lines("legend")
            .iter()
            .map(|line| measure_width(line, 14.0))
            .fold(f64::MIN, f64::max);
        if self.lines("legend").is_empty() {
            0.0
        } else {
            widest + 2.0 * LEGEND_PAD
        }
    }

    fn texts(&self, kind: &str, top: f64, canvas: f64, font_size: f64) -> Vec<Text> {
        let style = chrome_style(kind);
        self.lines(kind)
            .iter()
            .enumerate()
            .map(|(index, line)| Text {
                content: line.clone(),
                role: box_role(kind),
                x: if style.anchor == "end" { canvas } else { canvas / 2.0 },
                y: top + style.base + style.step * index as f64,
                anchor: style.anchor,
                colour: style.colour.map(str::to_string),
                size: style.size,
                ..Text::default()
            })
            .collect()
    }

    fn title(&self, top: f64, canvas: f64, font_size: f64) -> Option<Title> {
        if self.lines("title").is_empty() {
            return None;
        }
        Some(Title {
            texts: self.texts("title", top + self.room("header"), canvas, font_size),
        })
    }

    fn blocks(&self, top: f64, bottom: f64, canvas: f64, font_size: f64) -> Vec<TextBlock> {
        let caption = bottom + self.legend_room("bottom");
        let tops = [
            ("header", top),
            ("caption", caption),
            ("footer", caption + self.room("caption")),
        ];
        tops.iter()
            .filter(|(kind, _)| !self.lines(kind).is_empty())
            .map(|(kind, top)| TextBlock {
                id: box_role(kind),
                texts: self.texts(kind, *top, canvas, font_size),
            })
            .collect()
    }

    fn legend(&self, top: f64, bottom: f64, canvas: f64, font_size: f64) -> Option<Legend> {
        if self.lines("legend").is_empty() {
            return None;
        }
        let edge = self
            .diagram
            .chrome
            .legend_place
            .split(' ')
            .next()
            .unwrap_or("bottom")
            .to_string();
        let above = top + self.room("header") + self.room("title");
        let start = if edge == "top" { above } else { bottom };
        let width = self.legend_width();
        let height = self.legend_height();
        let top = start
            + if edge == "top" { LEGEND_BEFORE_TOP } else { LEGEND_BEFORE_BOTTOM };
        let left = match self
            .diagram
            .chrome
            .legend_place
            .split(' ')
            .last()
            .unwrap_or("center")
        {
            "left" => LEGEND_SIDE,
            "right" => canvas - LEGEND_SIDE - width,
            _ => (canvas - width) / 2.0,
        };
        Some(Legend {
            x: left,
            y: top,
            width,
            height,
            texts: self
                .lines("legend")
                .iter()
                .enumerate()
                .map(|(index, line)| Text {
                    content: line.clone(),
                    role: "legend",
                    x: left + LEGEND_PAD,
                    y: top + LEGEND_BASE + CHROME_LINE * index as f64,
                    anchor: "start",
                    ..Text::default()
                })
                .collect(),
        })
    }
}

/// Ruby `text.split("\n")`: a trailing empty line is dropped.
fn ruby_split_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> =
        text.split('\n').map(str::to_string).collect();
    while lines.last().is_some_and(|last| last.is_empty()) {
        lines.pop();
    }
    lines
}

fn box_role(kind: &str) -> &'static str {
    match kind {
        "header" => "header",
        "title" => "title",
        "caption" => "caption",
        "footer" => "footer",
        _ => "legend",
    }
}
