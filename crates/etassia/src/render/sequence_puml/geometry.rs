//! Measured shapes: note geometry, the note/ref/fragment outlines, the
//! arrow-end marks, the message wrapper and the numbered label. Port of
//! `sequence/{note_geometry,note_shape,fragment_shape,ref_shape,
//! arrow_marks,message_wrap,tab_size,numbered_label}.rb`.

use super::layout::{
    face_width, Fragment, HeadRectangle, Segment, Text,
};
use super::model::{split_lines, Item, Message, Note, Number, Numbering};
use crate::render::component::creole;
use crate::text;

pub const PAD: f64 = 8.0;
pub const SKIN_FONT_SIZE: f64 = 13.0;
pub const OFFSET: f64 = 6.0;
pub const OVER_MARGIN: f64 = 10.0;
pub const BLOCK_GAP: f64 = 5.0;
pub const PICTURE_CHROME: f64 = 15.0;
pub const LINE_LEADING: f64 = 1.1776;
const LINE_PITCH: f64 = 15.0;
const CHROME: f64 = 10.0;
pub const FIRST_BASELINE: f64 = 17.568;
pub const EDGE_RUN_PADDING: f64 = 24.0;
pub const EDGE_RING_INSET: f64 = 8.0;
pub const ARROW_MARKS_LENGTH: f64 = 10.0;
const HALF_WIDTH: f64 = 4.0;
const CROSS_CENTRE: f64 = 11.5;
const CROSS_HALF: f64 = 5.0;
const RING_RADIUS: f64 = 4.0;
const RING_SHIFT: f64 = 5.5;
const RING_LIFT: f64 = 0.75;
const NOTE_FOLD: f64 = 10.0;
const REF_SIDE_PAD: f64 = 14.0;
const REF_TEXT_PAD: f64 = 9.0;
const REF_LABEL_SIZE: f64 = 12.0;
const REF_MIN_WIDTH: f64 = 63.916;
const REF_TAB_WIDTH: f64 = 63.0;
const REF_HEIGHT: f64 = 39.4434;
pub const REF_TAIL: f64 = 5.0;
const REF_LABEL_BASELINE: f64 = 32.6;
const REF_TAB_BASELINE: f64 = 14.6;
pub const TAB_PAD: f64 = 8.0;
pub const TAB_HEIGHT: f64 = 20.0;
pub const BASE_PAD: f64 = 10.0;
const NEST_PAD: f64 = 8.0;
const BRANCH_BASELINE: f64 = 10.7;
pub const NUMBER_GAP: f64 = 4.0;
const HEIGHT_PER_PX: f64 = 1.18;
const BASELINE_PER_PX: f64 = 0.97;

fn measure_mono(text: &str, size: f64) -> f64 {
    text::measure_mono(text, size).width
}

fn run_width(text: &str, size: f64, bold: bool, mono: bool, run_size: Option<&str>) -> f64 {
    let size = run_size
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|size| *size > 0.0)
        .unwrap_or(size);
    if mono {
        return measure_mono(text, size);
    }
    face_width(text, size, bold)
}

/// `NoteGeometry.line_width`: the creole runs of one line, measured.
pub fn line_width(line: &str, font_size: f64, bold: bool) -> f64 {
    creole::parse(line)
        .iter()
        .map(|run| {
            run_width(
                &run.text,
                font_size,
                bold || run.style.is_bold(),
                run.style.is_mono(),
                run.style.size(),
            )
        })
        .sum()
}

/// `NoteGeometry.widest_line`.
pub fn widest_line(text: &str, font_size: f64, bold: bool) -> f64 {
    split_lines(text)
        .iter()
        .map(|line| line_width(line, font_size, bold))
        .fold(0.0f64, f64::max)
}

pub fn line_height(font_size: f64) -> f64 {
    font_size * LINE_LEADING
}

/// `NoteGeometry.natural_width`: the width the text needs.
pub fn natural_width(note: &Note, font_size: f64) -> f64 {
    if let Some(picture) = &note.picture {
        return picture.width() + 2.0 * PAD;
    }
    let widest = note
        .lines()
        .iter()
        .map(|line| line_width(line, font_size, false))
        .fold(0.0f64, f64::max);
    widest + 2.0 * PAD
}

/// `NoteGeometry.height`.
pub fn height(note: &Note) -> f64 {
    if let Some(picture) = &note.picture {
        return picture.height() + 14.0;
    }
    (note.lines().len() as f64 * LINE_PITCH) + CHROME
}

/// `NoteGeometry.picture_inset`.
pub fn picture_inset(picture: &super::model::Picture) -> f64 {
    let nested = picture.scene.notes.iter().any(|note| note.picture.is_some());
    if nested { PICTURE_CHROME * picture.scale } else { 0.0 }
}

/// `NoteGeometry.span`: the first and last participant index.
pub fn span(note: &Note, previous: Option<&Item>, ids: &[&str]) -> (usize, usize) {
    if note.side == super::model::NoteSide::Across {
        return (0, ids.len().saturating_sub(1));
    }
    if note.attached() {
        let indexes: Vec<usize> = previous
            .and_then(Item::as_message)
            .map(|message| {
                message
                    .participants()
                    .iter()
                    .filter_map(|id| ids.iter().position(|known| *known == *id))
                    .collect()
            })
            .unwrap_or_default();
        return minmax(indexes, ids);
    }
    let indexes: Vec<usize> = note
        .targets
        .iter()
        .filter_map(|id| ids.iter().position(|known| *known == id.as_str()))
        .collect();
    minmax(indexes, ids)
}

fn minmax(indexes: Vec<usize>, ids: &[&str]) -> (usize, usize) {
    match (indexes.iter().min(), indexes.iter().max()) {
        (Some(&low), Some(&high)) => (low, high),
        _ => (0, ids.len().saturating_sub(1)),
    }
}

/// `NoteGeometry.beside`: where a note on a closed block goes.
pub fn beside(block_x: f64, block_width: f64, natural: f64) -> (f64, f64) {
    (block_x + block_width + BLOCK_GAP, natural)
}

/// `NoteGeometry.box`.
pub fn note_box(
    note: &Note,
    centers: &[f64],
    natural: f64,
) -> (f64, f64) {
    let low = centers[0];
    let high = centers[centers.len() - 1];
    use super::model::NoteSide::*;
    match note.side {
        Left => (low - OFFSET - natural, natural),
        Right => (high + OFFSET, natural),
        _ => {
            let width = natural.max(high - low + 2.0 * OVER_MARGIN);
            ((low + high) / 2.0 - width / 2.0, width)
        }
    }
}

/// `NoteShape.outline`.
pub fn note_outline(
    shape: super::model::NoteShape,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) -> String {
    use super::model::NoteShape::*;
    let right = left + width;
    let bottom = top + height;
    let points: Vec<[f64; 2]> = match shape {
        Hex => {
            let middle = (top + bottom) / 2.0;
            vec![
                [left + NOTE_FOLD, top],
                [right - NOTE_FOLD, top],
                [right, middle],
                [right - NOTE_FOLD, bottom],
                [left + NOTE_FOLD, bottom],
                [left, middle],
            ]
        }
        Rect => vec![[left, top], [right, top], [right, bottom], [left, bottom]],
        Fold => vec![
            [left, top],
            [right - NOTE_FOLD, top],
            [right, top + NOTE_FOLD],
            [right, bottom],
            [left, bottom],
        ],
    };
    let path = points
        .iter()
        .map(|point| format!("{} {}", crate::svg::ruby_float(point[0]), crate::svg::ruby_float(point[1])))
        .collect::<Vec<_>>()
        .join(" L ");
    format!("M {path} Z")
}

/// `NoteShape.fold`: the crease of a folded corner.
pub fn note_fold(shape: super::model::NoteShape, left: f64, top: f64, width: f64) -> Option<String> {
    if shape != super::model::NoteShape::Fold {
        return None;
    }
    let right = left + width;
    Some(format!(
        "M {} {} L {} {} L {} {}",
        crate::svg::ruby_float(right - NOTE_FOLD),
        crate::svg::ruby_float(top),
        crate::svg::ruby_float(right - NOTE_FOLD),
        crate::svg::ruby_float(top + NOTE_FOLD),
        crate::svg::ruby_float(right),
        crate::svg::ruby_float(top + NOTE_FOLD),
    ))
}

/// `TabSize`: how far a fragment's keyword tab grows.
pub struct TabSize {
    pub size: f64,
    base: f64,
}

impl Clone for TabSize {
    fn clone(&self) -> Self {
        TabSize { size: self.size, base: self.base }
    }
}

impl TabSize {
    pub fn new(size: Option<f64>, base: f64) -> Self {
        TabSize { size: size.unwrap_or(base), base }
    }

    pub fn base_of(&self) -> f64 {
        self.base
    }

    pub fn custom(&self) -> bool {
        self.size != self.base
    }

    pub fn scale(&self) -> f64 {
        self.size / self.base
    }

    pub fn height_growth(&self) -> f64 {
        round2((self.size - self.base) * HEIGHT_PER_PX)
    }

    pub fn baseline_growth(&self) -> f64 {
        round2((self.size - self.base) * BASELINE_PER_PX)
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn rf(value: f64) -> String {
    crate::svg::ruby_float(value)
}

/// `FragmentShape.tab_outline`: the keyword tab in a frame corner.
pub fn tab_outline(left: f64, top: f64, width: f64, height: f64) -> String {
    let right = left + width;
    let bottom = top + height;
    format!(
        "M {} {} L {} {} L {} {} L {} {} L {} {} Z",
        rf(left),
        rf(top),
        rf(right),
        rf(top),
        rf(right),
        rf(bottom - 6.0),
        rf(right - 6.0),
        rf(bottom),
        rf(left),
        rf(bottom),
    )
}

/// The Walker's record of one open block, fed to `FragmentShape`.
pub struct BlockRecord {
    pub keyword: String,
    pub label: String,
    pub top: f64,
    pub low: f64,
    pub high: f64,
    pub branches: Vec<(f64, String)>,
    pub depth: usize,
    pub tab: TabSize,
}

/// `FragmentShape`: the frame of an alt/loop/group block.
pub struct FragmentShape {
    pub x: f64,
    pub width: f64,
    record: BlockRecord,
    bottom: f64,
    centers: Vec<f64>,
    appearance_tab_fill: Option<String>,
    appearance_tab_colour: Option<String>,
}

impl FragmentShape {
    pub fn new(
        record: BlockRecord,
        bottom: f64,
        centers: Vec<f64>,
        measure: impl Fn(&str) -> f64,
        appearance_tab_fill: Option<String>,
        appearance_tab_colour: Option<String>,
    ) -> Self {
        let (low, high) = if record.low > record.high {
            (
                centers.first().copied().unwrap_or(0.0),
                centers.last().copied().unwrap_or(0.0),
            )
        } else {
            (record.low, record.high)
        };
        let pad = BASE_PAD + NEST_PAD * record.depth as f64;
        let x = low - pad;
        let width = (high - low + 2.0 * pad).max(minimum_width(&record, measure));
        FragmentShape {
            x,
            width,
            record,
            bottom,
            centers,
            appearance_tab_fill,
            appearance_tab_colour,
        }
    }

    pub fn scene(&self) -> Fragment {
        let top = self.record.top;
        let top_y = top + 14.0 + self.record.tab.baseline_growth();
        let mut texts = vec![Text {
            content: tab_text(&self.record),
            x: self.x + TAB_PAD,
            y: top_y,
            role: "fragment_tab",
            anchor: "start",
            colour: self.appearance_tab_colour.clone(),
            size: self.record.tab.custom().then_some(self.record.tab.size),
            ..Text::default()
        }];
        let guard = self.bracket();
        if let Some(guard) = guard {
            texts.push(Text {
                content: guard,
                x: self.x + self.tab_width() + TAB_PAD,
                y: top_y,
                role: "fragment_guard",
                anchor: "start",
                ..Text::default()
            });
        }
        for (y, label) in &self.record.branches {
            if label.trim().is_empty() {
                continue;
            }
            texts.push(Text {
                content: format!("[{label}]"),
                x: self.x + TAB_PAD,
                y: y + BRANCH_BASELINE,
                role: "fragment_guard",
                anchor: "start",
                ..Text::default()
            });
        }
        Fragment {
            x: self.x,
            y: top,
            width: self.width,
            height: self.bottom - top,
            tab_path: tab_outline(self.x, top, self.tab_width(), TAB_HEIGHT + self.record.tab.height_growth()),
            tab_fill: self.appearance_tab_fill.clone(),
            separators: self
                .record
                .branches
                .iter()
                .map(|(y, _)| Segment { x1: self.x, y1: *y, x2: self.x + self.width, y2: *y, dash: None })
                .collect(),
            texts,
        }
    }

    fn group(&self) -> bool {
        self.record.keyword == "group"
    }

    fn guard(&self) -> String {
        self.record.label.clone()
    }

    fn bracket(&self) -> Option<String> {
        let label = self.guard();
        if label.is_empty() || self.group() {
            None
        } else {
            Some(format!("[{label}]"))
        }
    }

    fn tab_width(&self) -> f64 {
        let text = tab_text(&self.record);
        (face_width(&text, 14.0, false) * self.record.tab.scale()) + 2.0 * TAB_PAD
    }
}

fn tab_text(record: &BlockRecord) -> String {
    if record.keyword == "group" && !record.label.is_empty() {
        record.label.clone()
    } else {
        record.keyword.clone()
    }
}

fn minimum_width(record: &BlockRecord, measure: impl Fn(&str) -> f64) -> f64 {
    let label = record.label.clone();
    let guard_width = if label.is_empty() || record.keyword == "group" {
        0.0
    } else {
        measure(&format!("[{label}]"))
    };
    (measure(&tab_text(record)) * record.tab.scale() + 2.0 * TAB_PAD)
        + guard_width
        + 3.0 * TAB_PAD
}

/// `RefShape.width_for`.
pub fn ref_width_for(label: &str) -> f64 {
    let label_width = face_widest_line(label, REF_LABEL_SIZE, false);
    (label_width + 2.0 * REF_TEXT_PAD).max(REF_MIN_WIDTH)
}

pub fn ref_side_pad() -> f64 {
    REF_SIDE_PAD
}

fn face_widest_line(text: &str, size: f64, bold: bool) -> f64 {
    split_lines(text)
        .iter()
        .map(|line| face_width(line, size, bold))
        .fold(0.0f64, f64::max)
}

/// `RefShape`: the frame of a `ref over` line.
pub struct RefShape {
    pub x: f64,
    pub width: f64,
    label: String,
    top: f64,
}

impl RefShape {
    pub fn new(label: String, top: f64, span: (f64, f64)) -> Self {
        let width =
            (span.1 - span.0 + 2.0 * REF_SIDE_PAD).max(ref_width_for(&label));
        let x = (span.0 + span.1) / 2.0 - width / 2.0;
        RefShape { x, width, label, top }
    }

    pub fn scene(&self) -> Fragment {
        Fragment {
            x: self.x,
            y: self.top,
            width: self.width,
            height: REF_HEIGHT,
            tab_path: tab_outline(self.x, self.top, REF_TAB_WIDTH, TAB_HEIGHT),
            tab_fill: None,
            separators: vec![],
            texts: vec![
                Text {
                    content: "ref".into(),
                    x: self.x + TAB_PAD,
                    y: self.top + REF_TAB_BASELINE,
                    role: "fragment_tab",
                    anchor: "start",
                    ..Text::default()
                },
                Text {
                    content: self.label.clone(),
                    x: self.x + self.width / 2.0,
                    y: self.top + REF_LABEL_BASELINE,
                    role: "fragment_guard",
                    anchor: "middle",
                    ..Text::default()
                },
            ],
        }
    }
}

/// `ArrowMarks.for`: the shapes at one arrow end, and where the shaft stops.
pub fn arrow_marks(
    glyph: Option<&str>,
    circle: bool,
    x: f64,
    y: f64,
    inward: f64,
) -> (Vec<super::layout::Mark>, f64) {
    let mut marks: Vec<super::layout::Mark> = vec![];
    let mut shaft = x;
    if let Some(glyph) = glyph {
        if glyph == "cross" {
            let centre = x + inward * CROSS_CENTRE;
            for slant in [-CROSS_HALF, CROSS_HALF] {
                marks.push(stroke(
                    [centre - CROSS_HALF, y - slant],
                    [centre + CROSS_HALF, y + slant],
                    true,
                ));
            }
            shaft = centre;
        } else {
            let tip = if circle { x + inward * RING_SHIFT } else { x };
            let back = tip + inward * ARROW_MARKS_LENGTH;
            match glyph {
                "filled" => {
                    let notch = tip + inward * 6.0;
                    marks.push(polygon_mark(
                        &[
                            [back, y - HALF_WIDTH],
                            [tip, y],
                            [back, y + HALF_WIDTH],
                            [notch, y],
                        ],
                        true,
                    ));
                }
                "open" => {
                    marks.push(stroke([tip, y], [back, y - HALF_WIDTH], false));
                    marks.push(stroke([tip, y], [back, y + HALF_WIDTH], false));
                }
                "upper" => {
                    marks.push(polygon_mark(&[[tip, y], [back, y - HALF_WIDTH], [back, y]], true));
                }
                "lower" => {
                    marks.push(polygon_mark(&[[tip, y], [back, y + HALF_WIDTH], [back, y]], true));
                }
                "upper_open" => {
                    marks.push(stroke([tip, y], [back, y - HALF_WIDTH], false));
                }
                _ => {
                    marks.push(stroke([tip, y], [back, y + HALF_WIDTH], false));
                }
            }
            shaft = tip;
        }
    }
    if circle {
        marks.push(super::layout::Mark {
            kind: "circle",
            cx: x,
            cy: y - RING_LIFT,
            r: RING_RADIUS,
            ..Default::default()
        });
        if glyph.is_none() {
            shaft = x + inward * RING_RADIUS;
        }
    }
    (marks, shaft)
}

fn stroke(from: [f64; 2], to: [f64; 2], heavy: bool) -> super::layout::Mark {
    super::layout::Mark {
        kind: "line",
        x1: from[0],
        y1: from[1],
        x2: to[0],
        y2: to[1],
        heavy,
        ..Default::default()
    }
}

fn polygon_mark(points: &[[f64; 2]], filled: bool) -> super::layout::Mark {
    let list = points
        .iter()
        .map(|point| format!("{},{}", rf(point[0]), rf(point[1])))
        .collect::<Vec<_>>()
        .join(" ");
    super::layout::Mark {
        kind: "polygon",
        filled,
        points: list,
        ..Default::default()
    }
}

/// `MessageWrap`: explicit lines, wrapped to `Maxmessagesize`.
pub struct MessageWrap<M: Fn(&str) -> f64> {
    limit: Option<i64>,
    measure: M,
}

impl<M: Fn(&str) -> f64> MessageWrap<M> {
    pub fn new(limit: Option<i64>, measure: M) -> Self {
        MessageWrap { limit, measure }
    }

    /// The lines of `label`.
    pub fn lines(&self, label: &str) -> Vec<String> {
        self.explicit_lines(label)
            .into_iter()
            .flat_map(|line| self.wrapped_line(&line))
            .collect()
    }

    /// The width of the widest line.
    pub fn width(&self, label: &str) -> f64 {
        self.lines(label)
            .iter()
            .map(|line| (self.measure)(line))
            .fold(0.0f64, f64::max)
    }

    fn explicit_lines(&self, label: &str) -> Vec<String> {
        let decoded = decode_line_breaks(label);
        let decoded = if starts_ends_doubled(label) {
            decode_quote_escapes(&decoded)
        } else {
            decoded
        };
        let decoded = if !decoded.contains('\n') && starts_ends_doubled(&decoded) {
            decoded[2..decoded.len() - 2].trim().to_string()
        } else {
            decoded
        };
        if decoded.is_empty() {
            vec![String::new()]
        } else {
            decoded.split(['\r', '\n']).map(str::to_string).collect()
        }
    }

    fn wrapped_line(&self, line: &str) -> Vec<String> {
        let Some(limit) = self.limit else {
            return vec![line.to_string()];
        };
        let words: Vec<&str> = line.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            return vec![String::new()];
        }
        let mut lines: Vec<String> = vec![];
        for word in words {
            if lines.is_empty() {
                lines.push(word.to_string());
                continue;
            }
            let joined = format!("{} {word}", lines.last().unwrap());
            if (self.measure)(&joined) > limit as f64 {
                lines.push(word.to_string());
            } else {
                *lines.last_mut().unwrap() = joined;
            }
        }
        lines
    }
}

fn starts_ends_doubled(label: &str) -> bool {
    label.starts_with("\"\"") && label.ends_with("\"\"") && label.len() >= 4
}

/// Ruby `gsub(/(\\+)n/)`: each run of backslashes before an `n` halves,
/// an odd one leaves a newline.
fn decode_line_breaks(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let bytes = label.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let slashes = i - start;
            if i < bytes.len() && bytes[i] == b'n' {
                let pairs = slashes / 2;
                out.push_str(&"\\".repeat(pairs));
                if slashes % 2 == 1 {
                    out.push('\n');
                } else {
                    out.push('n');
                }
                i += 1;
            } else {
                out.push_str(&"\\".repeat(slashes));
            }
        } else {
            let ch = label[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Ruby `gsub(/\\+(?![\\n])/)`: a run of backslashes not before a
/// backslash or `n` rounds up to half its length.
fn decode_quote_escapes(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let bytes = label.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let slashes = i - start;
            if i < bytes.len() && (bytes[i] == b'\\' || bytes[i] == b'n') {
                out.push_str(&"\\".repeat(slashes));
            } else {
                out.push_str(&"\\".repeat((slashes + 1) / 2));
            }
        } else {
            let ch = label[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `NumberedLabel.extra`: the width the number adds.
pub fn numbered_extra(number: &Number, measure: &dyn Fn(&str) -> f64) -> f64 {
    measure(&number.text) + NUMBER_GAP
}

/// `NumberedLabel.texts`: the number, then the label, four pixels on.
pub fn numbered_texts(
    number: &Number,
    label: &str,
    at_x: f64,
    at_y: f64,
    anchor: &str,
    weight: Option<&str>,
    size: Option<f64>,
    family: Option<String>,
    measure: &dyn Fn(&str) -> f64,
) -> Vec<Text> {
    let extra = measure(&number.text) + NUMBER_GAP;
    let total = extra + measure(label);
    let left = match anchor {
        "middle" => at_x - total / 2.0,
        "end" => at_x - total,
        _ => at_x,
    };
    vec![
        Text {
            content: number.text.clone(),
            x: left,
            y: at_y,
            role: "message_number",
            anchor: "start",
            weight: weight.map(str::to_string),
            size,
            family: family.clone(),
            ..Text::default()
        },
        Text {
            content: label.to_string(),
            x: left + extra,
            y: at_y,
            role: "message_label",
            anchor: "start",
            size,
            family,
            ..Text::default()
        },
    ]
}

pub use super::layout::HeadRectangle as _HeadRectangleUnused;
type _Unused = (Fragment, HeadRectangle);
