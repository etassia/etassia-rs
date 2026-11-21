//! Port of Ruby `Notation::PlantUML::NoteLayout` (`note_layout.rb`): notes
//! placed once the classes are.
use super::model::{Diagram, MemberKind, Note, NoteSide};
use super::scene::{Box, Segment, Text};
use std::collections::HashMap;

const GAP: f64 = 30.0;
const STACK_GAP: f64 = 10.0;
const FREE_GAP: f64 = 20.0;
const LINE_HEIGHT: f64 = 15.310546875;
const PADDING: f64 = 10.0;
const SIDE: f64 = 21.0;
const TEXT_INDENT: f64 = 6.0;
const FIRST_BASELINE: f64 = 17.568;
const ROW_CENTRE: f64 = 5.2911;

pub struct NoteLayout<'a> {
    diagram: &'a Diagram,
    measure: &'a dyn Fn(&str) -> f64,
    member_measure: &'a dyn Fn(&str) -> f64,
}

impl<'a> NoteLayout<'a> {
    pub fn new(
        diagram: &'a Diagram,
        measure: &'a dyn Fn(&str) -> f64,
        member_measure: &'a dyn Fn(&str) -> f64,
    ) -> NoteLayout<'a> {
        NoteLayout { diagram, measure, member_measure }
    }

    /// Ruby `place`: the note boxes, in source order.
    pub fn place(&self, boxes: &[Box]) -> Vec<Box> {
        let by_name: HashMap<&str, &Box> =
            boxes.iter().map(|box_| (box_.id.as_str(), box_)).collect();
        let free_left =
            boxes.iter().map(|box_| box_.x + box_.width).fold(f64::NEG_INFINITY, f64::max) + FREE_GAP;
        let mut free_top = boxes.iter().map(|box_| box_.y).fold(f64::INFINITY, f64::min);
        let mut taken: HashMap<(String, u8), f64> = HashMap::new();
        let mut others: Vec<Rect> = boxes.iter().map(Rect::of).collect();
        let mut placed = vec![];
        for (index, note) in self.diagram.notes.iter().enumerate() {
            let target = note
                .target
                .as_deref()
                .and_then(|name| by_name.get(name).copied());
            let box_ = match target {
                Some(target) => {
                    let member = self.member_row(note, target);
                    let frame = Rect::of(target);
                    let anchored = self.anchored(note, &frame, member.as_ref());
                    let side = side_byte(note.side);
                    let mut rect = anchored;
                    if matches!(note.side, NoteSide::Left | NoteSide::Right) {
                        let key = (target.id.clone(), side);
                        let floor = taken.get(&key).copied().unwrap_or(0.0);
                        rect.y = rect.y.max(floor);
                    }
                    rect = self.step_out(note.side, rect, &others);
                    let key = (target.id.clone(), side);
                    taken.insert(key, rect.bottom() + STACK_GAP);
                    let pointer =
                        self.pointer_for(note.side, &rect, &frame, member.as_ref());
                    others.push(rect);
                    self.build(note, index, &rect, Some(pointer))
                }
                None => {
                    let size = self.size_of(note);
                    let rect = Rect {
                        x: free_left,
                        y: free_top,
                        width: size.0,
                        height: size.1,
                    };
                    free_top += rect.height + STACK_GAP;
                    others.push(rect);
                    self.build(note, index, &rect, None)
                }
            };
            placed.push(box_);
        }
        placed
    }

    fn step_out(&self, side: NoteSide, rect: Rect, others: &[Rect]) -> Rect {
        let (across, down) = match side {
            NoteSide::Left => (-GAP, 0.0),
            NoteSide::Right => (GAP, 0.0),
            NoteSide::Top => (0.0, -GAP),
            NoteSide::Bottom | NoteSide::Free => (0.0, GAP),
        };
        let mut rect = rect;
        while others.iter().any(|other| rect.overlaps(other)) {
            rect = rect.moved(across, down);
        }
        rect
    }

    fn member_row(&self, note: &Note, target: &Box) -> Option<MemberRow> {
        let index = self.member_index(note)?;
        let row = self.member_text(target, index)?;
        Some(MemberRow {
            x: row.x,
            y: row.y - ROW_CENTRE,
            content: row.content.clone(),
        })
    }

    fn member_text<'b>(&self, target: &'b Box, index: usize) -> Option<&'b Text> {
        target
            .texts
            .iter()
            .filter(|text| text.role.starts_with("member"))
            .nth(index)
    }

    fn member_index(&self, note: &Note) -> Option<usize> {
        let member = note.member.as_deref()?;
        let klass = self
            .diagram
            .classes
            .iter()
            .find(|item| Some(&item.name) == note.target.as_ref())?;
        let wanted = member.trim_matches('"');
        klass.body.iter().position(|item| signature(item) == wanted)
    }

    fn anchored(&self, note: &Note, target: &Rect, member: Option<&MemberRow>) -> Rect {
        let (width, height) = self.size_of(note);
        let mid = match member {
            Some(member) => member.y,
            None => target.y + target.height / 2.0,
        };
        let (x, y) = match note.side {
            NoteSide::Left => (target.x - GAP - width, mid - height / 2.0),
            NoteSide::Right => (target.right() + GAP, mid - height / 2.0),
            NoteSide::Top => (target.mid_x() - width / 2.0, target.y - GAP - height),
            NoteSide::Bottom | NoteSide::Free => {
                (target.mid_x() - width / 2.0, target.bottom() + GAP)
            }
        };
        Rect { x, y, width, height }
    }

    fn size_of(&self, note: &Note) -> (f64, f64) {
        let lines = note.lines.len().max(1) as f64;
        let widest = note
            .lines
            .iter()
            .map(|line| (self.measure)(line))
            .fold(f64::NEG_INFINITY, f64::max);
        (widest + SIDE, PADDING + lines * LINE_HEIGHT)
    }

    fn pointer_for(&self, side: NoteSide, rect: &Rect, target: &Rect, member: Option<&MemberRow>) -> Segment {
        match side {
            NoteSide::Left | NoteSide::Right => {
                let mid = rect.mid_y().clamp(rect.y + 6.0, rect.bottom() - 6.0);
                let row = member.map(|member| member.y).unwrap_or(rect.mid_y());
                if side == NoteSide::Right {
                    let tip_x = match member {
                        Some(member) => member.x + (self.member_measure)(&member.content),
                        None => target.right(),
                    };
                    Segment::new(tip_x, row, rect.x, mid)
                } else {
                    let tip_x = member.map(|member| member.x).unwrap_or(target.x);
                    Segment::new(tip_x, row, rect.right(), mid)
                }
            }
            NoteSide::Top => {
                Segment::new(rect.mid_x(), target.y, rect.mid_x(), rect.bottom())
            }
            NoteSide::Bottom | NoteSide::Free => {
                Segment::new(rect.mid_x(), target.bottom(), rect.mid_x(), rect.y)
            }
        }
    }

    fn build(&self, note: &Note, index: usize, rect: &Rect, pointer: Option<Segment>) -> Box {
        Box {
            id: format!("note-{index}"),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            kind: "note",
            fill: note.color.clone(),
            texts: note
                .lines
                .iter()
                .enumerate()
                .map(|(line_index, line)| {
                    Text::new(
                        line,
                        rect.x + TEXT_INDENT,
                        rect.y + FIRST_BASELINE + line_index as f64 * LINE_HEIGHT,
                        "member_note",
                        "start",
                    )
                })
                .collect(),
            separators: pointer.into_iter().collect(),
            spot: None,
        }
    }
}

#[derive(Clone, Debug)]
struct MemberRow {
    x: f64,
    y: f64,
    content: String,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn of(box_: &Box) -> Rect {
        Rect { x: box_.x, y: box_.y, width: box_.width, height: box_.height }
    }

    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }

    fn mid_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    fn mid_y(&self) -> f64 {
        self.y + self.height / 2.0
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && other.x < self.right()
            && self.y < other.y + other.height
            && other.y < self.bottom()
    }

    fn moved(&self, across: f64, down: f64) -> Rect {
        Rect { x: self.x + across, y: self.y + down, width: self.width, height: self.height }
    }
}

fn side_byte(side: NoteSide) -> u8 {
    match side {
        NoteSide::Left => 0,
        NoteSide::Right => 1,
        NoteSide::Top => 2,
        NoteSide::Bottom => 3,
        NoteSide::Free => 4,
    }
}

/// Ruby `signature`: how a note names a member.
fn signature(item: &super::model::Member) -> String {
    match &item.parameters {
        Some(parameters) => format!("{}({})", item.name, parameters),
        None => item.name.clone(),
    }
}

/// Ruby `MemberKind` re-export for the signature check above.
fn _kind_is_separator(item: &super::model::Member) -> bool {
    item.kind == MemberKind::Separator
}
