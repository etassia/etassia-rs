//! Port of `activity/{mark,canvas,node_marks,edge_marks,frame_marks,
//! arrow_head}.rb`: the placed diagram becomes marks, back to front, and
//! each mark becomes one SVG element, in order.

use std::collections::HashMap;

use super::layout::{
    Band, Columns, Placement, Ranking, HEADER_HEIGHT, TITLE_HEIGHT, WARNING_PITCH,
};
use super::metrics::{Box, Metrics, DECISION_POINT, FONT_SIZE, NOTE_PAD};
use super::model::{node_lines, Diagram, Edge, EdgeStyle, Kind, Note, Side};
use super::router::Router;
use crate::svg::Elem;
use crate::text::measure_monospace;

/// One thing the renderer draws, in final coordinates (`Mark`).
#[derive(Clone, Debug)]
pub enum Mark {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill: String,
        stroke: String,
        rx: Option<f64>,
        role: Option<&'static str>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: String,
        stroke: &'static str,
        role: Option<&'static str>,
        width: f64,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        fill: String,
        stroke: String,
        role: Option<&'static str>,
    },
    Path {
        points: Vec<(f64, f64)>,
        stroke: String,
        width: f64,
        dash: Option<&'static str>,
        curve: bool,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        anchor: &'static str,
        size: f64,
        family: Option<&'static str>,
        role: Option<&'static str>,
        weight: Option<&'static str>,
    },
}

impl Mark {
    pub fn role(&self) -> Option<&'static str> {
        match self {
            Mark::Rect { role, .. }
            | Mark::Circle { role, .. }
            | Mark::Polygon { role, .. }
            | Mark::Text { role, .. } => *role,
            _ => None,
        }
    }
}

/// Collects marks in drawing order and the extent they reach (`Canvas`).
pub struct Canvas {
    pub marks: Vec<Mark>,
    min_x: f64,
    pub max_x: f64,
    pub max_y: f64,
}

const INK: &str = "#181818";
const MONO_SIZE: f64 = 10.0;

impl Canvas {
    pub fn new() -> Self {
        Canvas {
            marks: vec![],
            min_x: f64::INFINITY,
            max_x: 0.0,
            max_y: 0.0,
        }
    }

    fn see(&mut self, left: f64, right: f64, bottom: f64) {
        self.min_x = self.min_x.min(left);
        self.max_x = self.max_x.max(right);
        self.max_y = self.max_y.max(bottom);
    }

    pub fn rect(
        &mut self,
        node_box: &Box,
        fill: &str,
        radius: Option<f64>,
        role: Option<&'static str>,
        stroke: &str,
    ) {
        self.see(node_box.left(), node_box.right(), node_box.bottom());
        self.marks.push(Mark::Rect {
            x: node_box.left(),
            y: node_box.top(),
            width: node_box.width,
            height: node_box.height,
            fill: fill.to_string(),
            stroke: stroke.to_string(),
            rx: radius,
            role,
        });
    }

    pub fn polygon(
        &mut self,
        points: Vec<(f64, f64)>,
        fill: &str,
        stroke: &str,
        role: Option<&'static str>,
    ) {
        for (x_pos, y_pos) in &points {
            self.see(*x_pos, *x_pos, *y_pos);
        }
        self.marks.push(Mark::Polygon {
            points,
            fill: fill.to_string(),
            stroke: stroke.to_string(),
            role,
        });
    }

    pub fn circle(
        &mut self,
        node_box: &Box,
        radius: f64,
        fill: &str,
        role: Option<&'static str>,
        width: f64,
    ) {
        self.see(node_box.left(), node_box.right(), node_box.bottom());
        self.marks.push(Mark::Circle {
            cx: node_box.cx,
            cy: node_box.cy,
            r: radius,
            fill: fill.to_string(),
            stroke: "#222222",
            role,
            width,
        });
    }

    pub fn path(
        &mut self,
        points: Vec<(f64, f64)>,
        stroke: &str,
        width: f64,
        dash: Option<&'static str>,
        curve: bool,
    ) {
        for (x_pos, y_pos) in &points {
            self.see(*x_pos, *x_pos, *y_pos);
        }
        self.marks.push(Mark::Path {
            points,
            stroke: stroke.to_string(),
            width,
            dash,
            curve,
        });
    }

    /// One text mark per line, the first baseline at `point`.
    pub fn lines(
        &mut self,
        rows: &[String],
        point: (f64, f64),
        metrics: &Metrics<'_>,
        anchor: &'static str,
        role: Option<&'static str>,
    ) {
        let size = metrics.size(role);
        self.lines_sized(rows, point, metrics, anchor, role, size, None);
    }

    /// The same, at an explicit size and weight (`Canvas#lines`'s face
    /// merge).
    #[allow(clippy::too_many_arguments)]
    pub fn lines_sized(
        &mut self,
        rows: &[String],
        point: (f64, f64),
        metrics: &Metrics<'_>,
        anchor: &'static str,
        role: Option<&'static str>,
        size: f64,
        weight: Option<&'static str>,
    ) {
        for (index, content) in rows.iter().enumerate() {
            let y_pos = point.1 + (index as f64 * metrics.line_height(role));
            self.text(
                (point.0, y_pos),
                content,
                metrics,
                anchor,
                role,
                size,
                None,
                weight,
            );
        }
    }

    /// The width of the widest of the monospace rows.
    pub fn mono_width(&self, rows: &[String]) -> f64 {
        rows.iter()
            .map(|row| measure_monospace(row, MONO_SIZE).width)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Monospace notice lines, `pitch` apart.
    pub fn mono_lines(&mut self, rows: &[String], point: (f64, f64), pitch: f64) {
        for (index, content) in rows.iter().enumerate() {
            self.marks.push(Mark::Text {
                x: point.0,
                y: point.1 + (index as f64 * pitch),
                content: content.clone(),
                anchor: "start",
                size: MONO_SIZE,
                family: Some("monospace"),
                role: None,
                weight: None,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        point: (f64, f64),
        content: &str,
        metrics: &Metrics<'_>,
        anchor: &'static str,
        role: Option<&'static str>,
        size: f64,
        family: Option<&'static str>,
        weight: Option<&'static str>,
    ) {
        let width = metrics.text_width_sized(&[content.to_string()], role, size);
        let factor = match anchor {
            "start" => 0.0,
            "middle" => 0.5,
            _ => 1.0,
        };
        let left = point.0 - (width * factor);
        self.see(left, left + width, point.1 + 3.0);
        self.marks.push(Mark::Text {
            x: point.0,
            y: point.1,
            content: content.to_string(),
            anchor,
            size,
            family,
            role,
            weight,
        });
    }

    /// The leftmost x any mark reaches.
    pub fn min_x(&self) -> f64 {
        self.min_x
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

// ---- ArrowHead (activity/arrow_head.rb) ----

const HEAD_LENGTH: f64 = 9.0;
const HEAD_HALF_WIDTH: f64 = 3.5;

/// The triangle at the end of an edge: tip, then the two corners.
pub fn arrow_head_points(segment: [(f64, f64); 2]) -> Vec<(f64, f64)> {
    let tip = segment[1];
    let run = segment[1].0 - segment[0].0;
    let rise = segment[1].1 - segment[0].1;
    let length = run.hypot(rise);
    let unit = (run / length, rise / length);
    let base = (
        tip.0 - (HEAD_LENGTH * unit.0),
        tip.1 - (HEAD_LENGTH * unit.1),
    );
    let side = (-unit.1, unit.0);
    vec![
        tip,
        (
            base.0 + (HEAD_HALF_WIDTH * side.0),
            base.1 + (HEAD_HALF_WIDTH * side.1),
        ),
        (
            base.0 - (HEAD_HALF_WIDTH * side.0),
            base.1 - (HEAD_HALF_WIDTH * side.1),
        ),
    ]
}

// ---- NodeMarks (activity/node_marks.rb) ----

const PAPER: &str = "#F1F1F1";
const NOTE_PAPER: &str = "#FEFFDD";
const BAR_GREY: &str = "#555555";
const FOLD: f64 = 6.0;
const TAIL_HALF: f64 = 4.0;

/// Draws a node, and the notes beside it (`NodeMarks`).
pub struct NodeMarks<'a> {
    boxes: &'a HashMap<usize, Box>,
    metrics: &'a Metrics<'a>,
}

impl<'a> NodeMarks<'a> {
    pub fn new(boxes: &'a HashMap<usize, Box>, metrics: &'a Metrics<'a>) -> Self {
        NodeMarks { boxes, metrics }
    }

    pub fn draw(&self, canvas: &mut Canvas, diagram: &Diagram, node: usize) {
        let node_box = &self.boxes[&node];
        match diagram.nodes[node].kind {
            Kind::Start => {
                canvas.circle(node_box, 10.0, "#222222", Some("start"), 1.0);
            }
            Kind::Stop => {
                canvas.circle(node_box, 11.0, "none", None, 1.0);
                canvas.circle(node_box, 6.0, "#222222", None, 1.0);
            }
            Kind::Kill => self.cross(canvas, node_box, 9.0, 2.0),
            Kind::End => {
                canvas.circle(node_box, 10.0, "none", None, 1.5);
                self.cross(canvas, node_box, 6.2, 2.5);
            }
            Kind::Merge => self.diamond(canvas, node_box, diagram.nodes[node].fill.as_deref()),
            Kind::Bar => self.bar(canvas, diagram, node, node_box),
            Kind::Decision => {
                if diagram.nodes[node].bare {
                    return self.diamond(canvas, node_box, diagram.nodes[node].fill.as_deref());
                }
                let fill = diagram.nodes[node]
                    .fill
                    .clone()
                    .unwrap_or_else(|| PAPER.to_string());
                canvas.polygon(hexagon(node_box), &fill, INK, Some("diamond"));
                self.centred(canvas, diagram, node, node_box, Some("diamond"));
            }
            Kind::Action => {
                let fill = diagram.nodes[node]
                    .fill
                    .clone()
                    .unwrap_or_else(|| PAPER.to_string());
                canvas.rect(node_box, &fill, Some(12.5), None, INK);
                self.centred(canvas, diagram, node, node_box, None);
            }
        }
    }

    /// The note the node carries, where the placement put it.
    pub fn note(&self, canvas: &mut Canvas, note: &Note, note_box: &Box, anchor: &Box) {
        canvas.polygon(
            outline(note_box, anchor, note.side),
            NOTE_PAPER,
            INK,
            Some("note"),
        );
        let pad = NOTE_PAD;
        let lines = note.lines();
        let point = (
            note_box.left() + pad,
            note_box.top() + pad + self.metrics.ascent(Some("note")),
        );
        canvas.lines(&lines, point, self.metrics, "start", Some("note"));
    }

    fn cross(&self, canvas: &mut Canvas, node_box: &Box, reach: f64, width: f64) {
        for (ax, ay, bx, by) in [(-1.0, -1.0, 1.0, 1.0), (-1.0, 1.0, 1.0, -1.0)] {
            canvas.path(
                vec![
                    (node_box.cx + (ax * reach), node_box.cy + (ay * reach)),
                    (node_box.cx + (bx * reach), node_box.cy + (by * reach)),
                ],
                "#222222",
                width,
                None,
                false,
            );
        }
    }

    /// PlantUML draws a legacy bar as a grey slab and a fork bar as a grey
    /// rounded one.
    fn bar(&self, canvas: &mut Canvas, diagram: &Diagram, node: usize, node_box: &Box) {
        if diagram.nodes[node].legacy {
            canvas.rect(node_box, BAR_GREY, None, None, INK);
        } else {
            canvas.rect(node_box, BAR_GREY, Some(2.5), Some("bar"), BAR_GREY);
        }
        if diagram.nodes[node].legacy || diagram.nodes[node].text.is_none() {
            return;
        }
        let text = vec![diagram.nodes[node].text.clone().unwrap_or_default()];
        let point = (node_box.right() + 6.0, node_box.cy + 4.0);
        canvas.lines_sized(&text, point, self.metrics, "start", None, FONT_SIZE, None);
    }

    fn diamond(&self, canvas: &mut Canvas, node_box: &Box, fill: Option<&str>) {
        let fill = fill.unwrap_or(PAPER);
        canvas.polygon(
            vec![
                (node_box.cx, node_box.top()),
                (node_box.right(), node_box.cy),
                (node_box.cx, node_box.bottom()),
                (node_box.left(), node_box.cy),
            ],
            fill,
            INK,
            Some("diamond"),
        );
    }

    fn centred(
        &self,
        canvas: &mut Canvas,
        diagram: &Diagram,
        node: usize,
        node_box: &Box,
        role: Option<&'static str>,
    ) {
        let rows = node_lines(diagram.nodes[node].text.as_deref());
        let height = self.metrics.text_height(&rows, role);
        let first = node_box.cy - (height / 2.0) + self.metrics.ascent(role);
        canvas.lines(&rows, (node_box.cx, first), self.metrics, "middle", role);
    }
}

/// A rectangle with its top right corner folded, and the note's tail: a
/// wedge from the middle of the edge facing the node to the node's
/// border.
fn outline(node_box: &Box, anchor: &Box, side: Side) -> Vec<(f64, f64)> {
    let mut points = folded(node_box);
    if side == Side::Right {
        points.extend(floor_of(node_box));
        points.extend(wedge(node_box.left(), anchor.right(), node_box.cy, -1.0));
    } else {
        points.extend(wedge(node_box.right(), anchor.left(), node_box.cy, 1.0));
        points.extend(floor_of(node_box));
    }
    points
}

fn folded(node_box: &Box) -> Vec<(f64, f64)> {
    vec![
        (node_box.left(), node_box.top()),
        (node_box.right() - FOLD, node_box.top()),
        (node_box.right(), node_box.top() + FOLD),
    ]
}

fn floor_of(node_box: &Box) -> Vec<(f64, f64)> {
    vec![
        (node_box.right(), node_box.bottom()),
        (node_box.left(), node_box.bottom()),
    ]
}

/// `edge`: x of the note's edge; `tip`: x of the node's border; `sign`: 1
/// walking down the edge, -1 walking up.
fn wedge(edge: f64, tip: f64, mid: f64, sign: f64) -> Vec<(f64, f64)> {
    vec![
        (edge, mid - (sign * TAIL_HALF)),
        (tip, mid),
        (edge, mid + (sign * TAIL_HALF)),
    ]
}

fn hexagon(node_box: &Box) -> Vec<(f64, f64)> {
    let left = node_box.left() + DECISION_POINT;
    let right = node_box.right() - DECISION_POINT;
    vec![
        (left, node_box.top()),
        (right, node_box.top()),
        (node_box.right(), node_box.cy),
        (right, node_box.bottom()),
        (left, node_box.bottom()),
        (node_box.left(), node_box.cy),
    ]
}

// ---- EdgeMarks (activity/edge_marks.rb) ----

const DASHED: &str = "7,5";
const DOTTED: &str = "2,4";
const QUESTION_GAP: f64 = 13.0;

/// Draws an edge: its path, its arrow head and its label (`EdgeMarks`).
pub struct EdgeMarks {
    curved: bool,
}

impl EdgeMarks {
    pub fn new(curved: bool) -> Self {
        EdgeMarks { curved }
    }

    pub fn draw(
        &self,
        canvas: &mut Canvas,
        diagram: &Diagram,
        router: &mut Router<'_>,
        metrics: &Metrics<'_>,
        edge: usize,
    ) {
        let edge_ref = &diagram.edges[edge];
        if edge_ref.from == edge_ref.to {
            return;
        }
        let route = router.call(edge);
        self.paint(canvas, edge_ref, &route.points);
        self.label(canvas, edge_ref, &route, metrics);
        if diagram.nodes[edge_ref.to].bare {
            let segment: [(f64, f64); 2] = [
                route.points[route.points.len() - 2],
                route.points[route.points.len() - 1],
            ];
            self.question(canvas, diagram, edge_ref.to, &segment, metrics);
        }
    }

    fn paint(&self, canvas: &mut Canvas, edge: &Edge, points: &[(f64, f64)]) {
        let colour = edge.colour.clone().unwrap_or_else(|| INK.to_string());
        let dash = match edge.style {
            Some(EdgeStyle::Dashed) => Some(DASHED),
            Some(EdgeStyle::Dotted) => Some(DOTTED),
            _ => None,
        };
        let width = if edge.style == Some(EdgeStyle::Bold) {
            2.5
        } else {
            1.0
        };
        canvas.path(points.to_vec(), &colour, width, dash, self.curved);
        let segment = [points[points.len() - 2], points[points.len() - 1]];
        canvas.polygon(arrow_head_points(segment), &colour, &colour, None);
    }

    /// A bare decision's question stands left of the arrow into it, level
    /// with the middle of the arrow.
    fn question(
        &self,
        canvas: &mut Canvas,
        diagram: &Diagram,
        node: usize,
        segment: &[(f64, f64); 2],
        metrics: &Metrics<'_>,
    ) {
        let (tail, tip) = (segment[0], segment[1]);
        let lines = node_lines(diagram.nodes[node].text.as_deref());
        let point = (tip.0 - QUESTION_GAP, ((tail.1 + tip.1) / 2.0) + 10.0);
        canvas.lines(&lines, point, metrics, "end", Some("arrow"));
    }

    fn label(
        &self,
        canvas: &mut Canvas,
        edge: &Edge,
        route: &super::router::Route,
        metrics: &Metrics<'_>,
    ) {
        let Some(label) = &edge.label else {
            return;
        };
        let lines = super::model::split_lines(label);
        canvas.lines(
            &lines,
            (route.label_x, route.label_y),
            metrics,
            route.anchor,
            Some("arrow"),
        );
    }
}

// ---- FrameMarks (activity/frame_marks.rb) ----

const PARTITION_PAD: f64 = 10.0;
const LANE_FLOOR: f64 = 20.0;
const LANE_RULE: &str = "#000000";
const WARNING_PAPER: &str = "#FFFFCC";
const WARNING_INK: &str = "#FFDD88";

/// Draws what frames the flow: the title, swimlane bands and partition
/// frames (`FrameMarks`).
pub struct FrameMarks<'a> {
    diagram: &'a Diagram,
    placement: &'a Placement,
    metrics: &'a Metrics<'a>,
}

impl<'a> FrameMarks<'a> {
    pub fn new(diagram: &'a Diagram, placement: &'a Placement, metrics: &'a Metrics<'a>) -> Self {
        FrameMarks {
            diagram,
            placement,
            metrics,
        }
    }

    pub fn title(&self, canvas: &mut Canvas) {
        let Some(title) = &self.diagram.title else {
            return;
        };
        let rows: Vec<String> = title.split('\n').map(str::to_string).collect();
        canvas.lines_sized(
            &rows,
            (super::layout::MARGIN, super::layout::MARGIN + 14.0),
            self.metrics,
            "start",
            None,
            14.0,
            Some("bold"),
        );
    }

    /// PlantUML's notices, in a yellow box under the title: one monospace
    /// line each.
    pub fn warnings(&self, canvas: &mut Canvas) {
        let rows = &self.diagram.warnings;
        if rows.is_empty() {
            return;
        }
        let left = super::layout::MARGIN;
        let top = left
            + if self.diagram.title.is_some() {
                TITLE_HEIGHT
            } else {
                0.0
            };
        let notice = Box::spanning(
            left,
            top,
            left + canvas.mono_width(rows) + 10.0,
            top + (WARNING_PITCH * rows.len() as f64) - 5.0,
        );
        canvas.rect(
            &notice,
            WARNING_PAPER,
            Some(2.5),
            Some("warning"),
            WARNING_INK,
        );
        canvas.mono_lines(rows, (left + 7.0, top + 10.6406), WARNING_PITCH);
    }

    pub fn lanes(&self, canvas: &mut Canvas) {
        let floor = self.placement.bottoms.last().copied().unwrap_or(0.0) + LANE_FLOOR;
        let ceiling = self.placement.top - HEADER_HEIGHT;
        for band in &self.placement.bands {
            self.lane(canvas, band, ceiling, floor);
        }
    }

    pub fn partitions(&self, canvas: &mut Canvas) {
        let mut names: Vec<String> = vec![];
        for node in &self.diagram.nodes {
            if let Some(name) = &node.partition {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        for name in names {
            let boxes: Vec<&Box> = self
                .diagram
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.partition.as_deref() == Some(name.as_str()))
                .map(|(index, _)| &self.placement.boxes[&index])
                .collect();
            self.partition(canvas, &name, &boxes);
        }
    }

    fn lane(&self, canvas: &mut Canvas, band: &Band, ceiling: f64, floor: f64) {
        let (left, right) = (band.left, band.right);
        canvas.rect(
            &Box::spanning(left, ceiling, right, floor),
            band.colour.as_deref().unwrap_or("none"),
            None,
            None,
            INK,
        );
        for x_pos in [left, right] {
            canvas.path(
                vec![(x_pos, ceiling), (x_pos, floor)],
                LANE_RULE,
                1.5,
                None,
                false,
            );
        }
        canvas.lines_sized(
            &[band.name.clone()],
            ((left + right) / 2.0, ceiling + 20.0),
            self.metrics,
            "middle",
            None,
            super::layout::LANE_TITLE,
            None,
        );
    }

    fn partition(&self, canvas: &mut Canvas, name: &str, member_boxes: &[&Box]) {
        let frame = self.frame_around(member_boxes, name);
        canvas.rect(&frame, "none", None, Some("partition"), INK);
        let point = (
            frame.left() + 8.0,
            frame.top() + self.metrics.size(Some("partition")) + 3.0,
        );
        canvas.lines(
            &[name.to_string()],
            point,
            self.metrics,
            "start",
            Some("partition"),
        );
    }

    /// The frame is as tall above the nodes as the title is, and wide
    /// enough that the title ends before the arrow that enters it.
    fn frame_around(&self, member_boxes: &[&Box], name: &str) -> Box {
        let top = member_boxes
            .iter()
            .copied()
            .min_by(|a, b| {
                a.top()
                    .partial_cmp(&b.top())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("a partition has members");
        let left = (member_boxes
            .iter()
            .map(|member| member.left())
            .fold(f64::INFINITY, f64::min)
            - PARTITION_PAD)
            .min(top.cx - self.title_width(name));
        let right = member_boxes
            .iter()
            .map(|member| member.right())
            .fold(f64::NEG_INFINITY, f64::max)
            + PARTITION_PAD;
        let bottom = member_boxes
            .iter()
            .map(|member| member.bottom())
            .fold(f64::NEG_INFINITY, f64::max)
            + PARTITION_PAD;
        Box::spanning(
            left,
            top.top() - self.placement.head(self.metrics),
            right,
            bottom,
        )
    }

    fn title_width(&self, name: &str) -> f64 {
        (1.12
            * self
                .metrics
                .text_width(&[name.to_string()], Some("partition")))
            + 12.0
    }
}

// ---- Marks (activity/marks.rb) ----

/// Turns a placed diagram into marks, back to front: lanes, partitions,
/// edges, nodes, notes, title.
pub struct Marks<'a> {
    diagram: &'a Diagram,
    placement: &'a Placement,
    canvas: Canvas,
    metrics: &'a Metrics<'a>,
    curved: bool,
}

impl<'a> Marks<'a> {
    pub fn new(diagram: &'a Diagram, placement: &'a Placement, metrics: &'a Metrics<'a>) -> Self {
        Marks {
            diagram,
            placement,
            canvas: Canvas::new(),
            metrics,
            curved: diagram.legacy,
        }
    }

    /// The leftmost x any mark reaches.
    pub fn min_x(&self) -> f64 {
        self.canvas.min_x()
    }

    pub fn scene(&self) -> (Vec<Mark>, f64, f64) {
        (
            self.canvas.marks.clone(),
            self.canvas.max_x + super::layout::MARGIN,
            self.canvas.max_y + super::layout::MARGIN,
        )
    }

    /// Fills the canvas.
    pub fn build(&mut self, ranking: &Ranking, columns: &Columns) {
        let frame = FrameMarks::new(self.diagram, self.placement, self.metrics);
        frame.title(&mut self.canvas);
        frame.warnings(&mut self.canvas);
        if !self.placement.bands.is_empty() {
            frame.lanes(&mut self.canvas);
        }
        frame.partitions(&mut self.canvas);
        let mut router = Router::new(
            self.diagram,
            &self.placement.boxes,
            ranking,
            &self.placement.bottoms,
            &self.placement.gaps,
            self.metrics.size(Some("arrow")) + 1.0,
        );
        let edges = EdgeMarks::new(self.curved);
        for edge in 0..self.diagram.edges.len() {
            edges.draw(
                &mut self.canvas,
                self.diagram,
                &mut router,
                self.metrics,
                edge,
            );
        }
        let nodes = NodeMarks::new(&self.placement.boxes, self.metrics);
        for node in 0..self.diagram.nodes.len() {
            nodes.draw(&mut self.canvas, self.diagram, node);
        }
        self.notes(&nodes);
        let _ = columns;
    }

    fn notes(&mut self, nodes: &NodeMarks<'_>) {
        for (node, pairs) in &self.placement.notes {
            for (note, note_box) in pairs {
                nodes.note(
                    &mut self.canvas,
                    note,
                    note_box,
                    &self.placement.boxes[node],
                );
            }
        }
    }
}

/// Gives `element` the line's content: plain text when there is no
/// markup, one tspan per styled piece otherwise (`Creole.assign`).
pub fn assign_creole(element: Elem, text: &str) -> Elem {
    crate::render::component::creole::assign(element, text)
}
