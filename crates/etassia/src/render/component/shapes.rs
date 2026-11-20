//! Draws the laid-out scene: containers, then links, then nodes, then
//! labels, then the title. Port of `node_graph/{renderer,shapes,figures,
//! tab,cloud,note_tail}.rb` and `NodeGraph::Heads`/`Curve`.

use super::heads;
use super::label_placer;
use super::metrics::{self, face_width, ASCENT, FONT_SIZE, LINE_HEIGHT};
use super::model::{Edge, Graph, NodeKind, Style};
use super::scene::{Placed, Route, Scene};
use crate::svg::{ruby_float, Elem};
use crate::text::Face;

const STROKE: &str = "#181818";
const FILL: &str = "#F1F1F1";
const FONT: &str = "sans-serif";
const LEAF_WIDTH: &str = "0.5";
const INK: &str = "#181818";
const DASH: &str = "5,4";
const TITLE_SIZE: f64 = 13.0;
const SECOND_OFFSET: &str = "translate(2,2)";
const TAB_HEIGHT: f64 = 20.0;
const TAB_LEANING_HEIGHT: f64 = 22.49;
const TAB_SLANT: f64 = 7.0;
const TAB_CHAMFER: f64 = 10.0;
const CLOUD_BUMP: f64 = 11.0;
const CLOUD_PUSH: f64 = 8.0;
const CLOUD_RISE: f64 = 4.0;
const CLOUD_CUT: f64 = 9.0;
const NOTE_HALF: f64 = 4.0;
const CURVE_MIN_GAP: f64 = 12.0;
const HEADS_LENGTH: f64 = 9.0;
const HEADS_HALF: f64 = 4.0;
const HEADS_SOCKET: f64 = 5.0;

/// `[[fx, fy, dx, dy], ...]`: `fx` of the width plus `dx`.
type Spec = [f64; 4];

const ORIGIN: Spec = [0.0, 0.0, 0.0, 0.0];
const FULL: Spec = [1.0, 1.0, 0.0, 0.0];

enum FigureCommand {
    Rect(Spec, Spec, Option<f64>),
    Circle(Spec, f64),
    Line(Spec, Spec),
    Ellipse,
    Polygon(Vec<Spec>, &'static str),
    Path(Vec<PathPart>, &'static str),
    Cloud,
}

enum PathPart {
    M(Spec),
    L(Spec),
    A(Spec, f64, Spec),
    Z,
}

/// `Figures::FILE`.
fn file_figure() -> Vec<FigureCommand> {
    vec![
        FigureCommand::Rect(ORIGIN, FULL, None),
        FigureCommand::Polygon(
            vec![[1.0, 0.0, -8.0, 0.0], [1.0, 0.0, 0.0, 8.0], [1.0, 0.0, -8.0, 8.0]],
            "#FFFFFF",
        ),
    ]
}

/// `Figures::DATABASE`.
fn database_figure() -> Vec<FigureCommand> {
    vec![
        FigureCommand::Path(
            vec![
                PathPart::M([0.0, 0.0, 0.0, 8.0]),
                PathPart::A([0.5, 0.0, 0.0, 8.0], 1.0, [1.0, 0.0, 0.0, 8.0]),
                PathPart::L([1.0, 1.0, 0.0, -8.0]),
                PathPart::A([0.5, 0.0, 0.0, 8.0], 1.0, [0.0, 1.0, 0.0, -8.0]),
                PathPart::Z,
            ],
            "",
        ),
        FigureCommand::Path(
            vec![
                PathPart::M([0.0, 0.0, 0.0, 8.0]),
                PathPart::A([0.5, 0.0, 0.0, 8.0], 0.0, [1.0, 0.0, 0.0, 8.0]),
            ],
            "none",
        ),
    ]
}

/// `Figures::NODE`.
fn node_figure() -> Vec<FigureCommand> {
    vec![
        FigureCommand::Polygon(
            vec![[0.0, 0.0, 0.0, 8.0], [0.0, 0.0, 8.0, 0.0], [1.0, 0.0, 0.0, 0.0], [1.0, 0.0, -8.0, 8.0]],
            "",
        ),
        FigureCommand::Polygon(
            vec![[1.0, 0.0, -8.0, 8.0], [1.0, 0.0, 0.0, 0.0], [1.0, 1.0, 0.0, -8.0], [1.0, 1.0, -8.0, 0.0]],
            "",
        ),
        FigureCommand::Rect([0.0, 0.0, 0.0, 8.0], [1.0, 1.0, -8.0, -8.0], None),
    ]
}

/// `Figures::COMPONENT`: the rounded box and its small icon.
fn component_figure() -> Vec<FigureCommand> {
    vec![
        FigureCommand::Rect(ORIGIN, FULL, Some(2.5)),
        FigureCommand::Rect([1.0, 0.0, -20.0, 5.0], [0.0, 0.0, 15.0, 10.0], None),
        FigureCommand::Rect([1.0, 0.0, -22.0, 7.0], [0.0, 0.0, 4.0, 2.0], None),
        FigureCommand::Rect([1.0, 0.0, -22.0, 11.0], [0.0, 0.0, 4.0, 2.0], None),
    ]
}

/// What a container is drawn as; any other kind is a plain frame.
fn container_figure(kind: NodeKind) -> Option<Vec<FigureCommand>> {
    match kind {
        NodeKind::Node => Some(node_figure()),
        NodeKind::Database => Some(database_figure()),
        NodeKind::Cloud => Some(vec![FigureCommand::Cloud]),
        _ => None,
    }
}

fn figure(kind: NodeKind) -> Vec<FigureCommand> {
    match kind {
        NodeKind::Actor => vec![
            FigureCommand::Circle([0.5, 0.0, 0.0, 8.0], 8.0),
            FigureCommand::Line([0.5, 0.0, 0.0, 16.0], [0.5, 0.0, 0.0, 43.0]),
            FigureCommand::Line([0.5, 0.0, -13.0, 24.0], [0.5, 0.0, 13.0, 24.0]),
            FigureCommand::Line([0.5, 0.0, 0.0, 43.0], [0.5, 0.0, -13.0, 58.0]),
            FigureCommand::Line([0.5, 0.0, 0.0, 43.0], [0.5, 0.0, 13.0, 58.0]),
        ],
        NodeKind::Usecase => vec![FigureCommand::Ellipse],
        NodeKind::Component => component_figure(),
        NodeKind::Interface => vec![FigureCommand::Circle([0.5, 0.0, 0.0, 10.0], 8.0)],
        NodeKind::Portin => {
            vec![FigureCommand::Rect([0.5, 1.0, -6.0, -12.0], [0.0, 0.0, 12.0, 12.0], None)]
        }
        NodeKind::Portout => {
            vec![FigureCommand::Rect([0.5, 0.0, -6.0, 0.0], [0.0, 0.0, 12.0, 12.0], None)]
        }
        NodeKind::Note => vec![FigureCommand::Polygon(
            vec![
                [0.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, -10.0, 0.0],
                [1.0, 0.0, 0.0, 10.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
            ],
            "#FEFFDD",
        )],
        NodeKind::File => file_figure(),
        NodeKind::Circle => vec![FigureCommand::Circle([0.5, 0.0, 0.0, 8.0], 8.0)],
        NodeKind::Database => database_figure(),
        NodeKind::Node => node_figure(),
        NodeKind::Cloud => vec![FigureCommand::Cloud],
        _ => vec![FigureCommand::Rect(ORIGIN, FULL, Some(2.5))],
    }
}

/// Where the label goes, by kind.
#[derive(Default, Clone, Copy)]
struct LabelSpec {
    top: Option<f64>,
    dy: Option<f64>,
    shift: Option<f64>,
    shrink: Option<f64>,
    left: Option<f64>,
}

fn label_spec(kind: NodeKind) -> LabelSpec {
    match kind {
        NodeKind::Actor => LabelSpec { top: Some(59.5), ..LabelSpec::default() },
        NodeKind::Interface => LabelSpec { top: Some(22.0), ..LabelSpec::default() },
        NodeKind::Circle => LabelSpec { top: Some(20.0), ..LabelSpec::default() },
        NodeKind::Portin => LabelSpec { top: Some(0.0), ..LabelSpec::default() },
        NodeKind::Portout => LabelSpec { top: Some(12.0), ..LabelSpec::default() },
        NodeKind::Note => LabelSpec { top: Some(4.0), left: Some(8.0), ..LabelSpec::default() },
        NodeKind::Database => LabelSpec { dy: Some(6.0), ..LabelSpec::default() },
        NodeKind::Component => LabelSpec { dy: Some(4.0), ..LabelSpec::default() },
        NodeKind::Node => LabelSpec { dy: Some(4.0), shrink: Some(8.0), ..LabelSpec::default() },
        _ => LabelSpec::default(),
    }
}

fn tabbed(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Package | NodeKind::Folder | NodeKind::Frame)
}

fn header_drop(kind: NodeKind) -> f64 {
    match kind {
        NodeKind::Database => 10.0,
        NodeKind::Cloud => 12.0,
        NodeKind::Node => 8.0,
        _ => 0.0,
    }
}

fn header_left(kind: NodeKind) -> f64 {
    if kind == NodeKind::Cloud { 24.0 } else { 0.0 }
}

/// The tab over the top left of a package, folder or frame. Ruby `Tab`.
fn tab_points(left: f64, top: f64, width: f64, kind: NodeKind) -> String {
    let pads = tab_pads(kind);
    let right = left + (pads.0 + pads.1) + width;
    let outline = if kind == NodeKind::Frame {
        vec![
            [right, top + TAB_HEIGHT - TAB_CHAMFER],
            [right - TAB_CHAMFER, top + TAB_HEIGHT],
        ]
    } else {
        vec![[right + TAB_SLANT, top + TAB_LEANING_HEIGHT]]
    };
    let mut points = vec![[left, top], [right, top]];
    points.extend(outline);
    points.push([left, top + tab_height(kind)]);
    points
        .iter()
        .map(|point| format!("{},{}", ruby_float(point[0]), ruby_float(point[1])))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tab_pads(kind: NodeKind) -> (f64, f64) {
    if kind == NodeKind::Frame {
        (3.0, 7.0)
    } else {
        (4.0, 2.0)
    }
}

fn tab_height(kind: NodeKind) -> f64 {
    if kind == NodeKind::Frame { TAB_HEIGHT } else { TAB_LEANING_HEIGHT }
}

fn tab_baseline(kind: NodeKind) -> f64 {
    if kind == NodeKind::Frame { 14.5 } else { 15.5 }
}

fn tab_indent(kind: NodeKind) -> f64 {
    tab_pads(kind).0
}

/// The outline of a cloud: the box's rectangle with its corners cut, every
/// side a run of small outward bumps. Ruby `Cloud`.
fn cloud_to_d(boxed: &Placed) -> String {
    let left = boxed.x + CLOUD_RISE;
    let top = boxed.y + CLOUD_RISE;
    let right = boxed.x + boxed.width - CLOUD_RISE;
    let bottom = boxed.y + boxed.height - CLOUD_RISE;
    let corners = [
        [left + CLOUD_CUT, top],
        [right - CLOUD_CUT, top],
        [right, top + CLOUD_CUT],
        [right, bottom - CLOUD_CUT],
        [right - CLOUD_CUT, bottom],
        [left + CLOUD_CUT, bottom],
        [left, bottom - CLOUD_CUT],
        [left, top + CLOUD_CUT],
    ];
    let mut sides = vec![];
    for index in 0..corners.len() {
        let from = corners[index];
        let to = corners[(index + 1) % corners.len()];
        sides.push(cloud_side(from, to));
    }
    format!(
        "M{},{} {} Z",
        ruby_float(corners[0][0]),
        ruby_float(corners[0][1]),
        sides.join(" ")
    )
}

fn cloud_side(from: [f64; 2], to: [f64; 2]) -> String {
    let length = (to[0] - from[0]).hypot(to[1] - from[1]);
    let count = ((length / CLOUD_BUMP).round() as i64).max(1);
    let mut bumps = vec![];
    for index in 0..count {
        let along = (index as f64 + 0.5) / count as f64;
        let control = cloud_point(from, to, along, CLOUD_PUSH);
        let stop = cloud_point(from, to, (index as f64 + 1.0) / count as f64, 0.0);
        bumps.push(format!(
            "Q{},{} {},{}",
            ruby_float(control[0]),
            ruby_float(control[1]),
            ruby_float(stop[0]),
            ruby_float(stop[1])
        ));
    }
    bumps.join(" ")
}

fn cloud_point(from: [f64; 2], to: [f64; 2], along: f64, push: f64) -> [f64; 2] {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let unit = push / dx.hypot(dy);
    [
        from[0] + dx * along + dy * unit,
        from[1] + dy * along - dx * unit,
    ]
}

fn shape_along(fraction: f64, offset: f64, origin: f64, extent: f64) -> f64 {
    origin + fraction * extent + offset
}

fn hex_color(color: &str) -> bool {
    matches!(color.len(), 3..=8) && color.chars().all(|c| c.is_ascii_hexdigit())
}

/// Draws one placed node as SVG elements. Ruby `Shapes`.
struct Shapes<'a> {
    graph: &'a Graph,
    p: Placed,
    out: Vec<Elem>,
    icon: bool,
}

impl<'a> Shapes<'a> {
    fn draw(graph: &'a Graph, placed: &Placed) -> Vec<Elem> {
        let mut shapes = Shapes { graph, p: placed.clone(), out: vec![], icon: false };
        if graph.nodes[placed.node].container {
            shapes.container();
        } else {
            shapes.figure();
        }
        shapes.out
    }

    fn node(&self) -> &super::model::Node {
        &self.graph.nodes[self.p.node]
    }

    fn line_attrs(&self) -> Vec<(&'static str, &'static str)> {
        if self.heavy() {
            return vec![("stroke-width", "1.5"), ("stroke", "#000000")];
        }
        vec![("stroke", STROKE), ("stroke-width", self.stroke_width())]
    }

    fn heavy(&self) -> bool {
        self.node().container && matches!(self.node().kind, NodeKind::Package | NodeKind::Folder)
    }

    fn stroke_width(&self) -> &'static str {
        if self.icon {
            return "0.5";
        }
        if self.node().container {
            return "1";
        }
        match self.node().kind {
            NodeKind::Portin | NodeKind::Portout => "1.5",
            _ => LEAF_WIDTH,
        }
    }

    fn apply_line(&self, element: Elem) -> Elem {
        let mut element = element;
        for (name, value) in self.line_attrs() {
            element = element.attr(name, value);
        }
        element
    }

    fn fill(&self) -> String {
        match &self.node().color {
            Some(color) => {
                if hex_color(color) {
                    format!("#{color}")
                } else {
                    color.clone()
                }
            }
            None => {
                if self.node().container {
                    "none".to_string()
                } else {
                    FILL.to_string()
                }
            }
        }
    }

    fn at(&self, spec: Spec, y_origin: f64) -> (f64, f64) {
        (
            shape_along(spec[0], spec[2], self.p.x, self.p.width),
            shape_along(spec[1], spec[3], y_origin, self.p.height),
        )
    }

    fn figure_drop(&self) -> f64 {
        if self.above_figure() { LINE_HEIGHT } else { 0.0 }
    }

    fn above_figure(&self) -> bool {
        self.node().kind == NodeKind::Actor && self.node().stereotype.is_some()
    }

    fn size(&self, spec: Spec) -> (f64, f64) {
        (spec[0] * self.p.width + spec[2], spec[1] * self.p.height + spec[3])
    }

    fn figure(&mut self) {
        let kind = self.node().kind;
        let sprite = self.node().sprite.is_some();
        let mut spec = label_spec(kind);
        if sprite {
            spec.dy = Some(10.0);
        }
        for command in figure(kind) {
            self.execute(command);
        }
        self.label(spec);
        if sprite {
            self.draw_sprite();
        }
    }

    /// The icon of an archimate element, re-anchored the way the jar
    /// places it.
    fn draw_sprite(&mut self) {
        let sprite = self.node().sprite.expect("sprite checked");
        let left = (self.p.x + self.p.width / 2.0 + sprite.dx) - sprite.cx;
        let top = self.p.y + sprite.top - sprite.cy_top;
        let place = format!("translate({},{})", ruby_float(left), ruby_float(top));
        self.out.push(
            Elem::new("path")
                .attr("d", sprite.d)
                .attr("fill", "#000000")
                .attr("transform", place),
        );
    }

    fn execute(&mut self, command: FigureCommand) {
        let y_origin = self.p.y + self.figure_drop();
        let fill = self.fill();
        match command {
            FigureCommand::Rect(origin, extent, radius) => {
                let (x, y) = self.at(origin, y_origin);
                let (width, height) = self.size(extent);
                let mut element = Elem::new("rect")
                    .num("x", x)
                    .num("y", y)
                    .num("width", width)
                    .num("height", height);
                if let Some(radius) = radius {
                    element = element.num("rx", radius).num("ry", radius);
                }
                element = element.attr("fill", &fill);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Circle(center, radius) => {
                let (x, y) = self.at(center, y_origin);
                let element = Elem::new("circle")
                    .num("cx", x)
                    .num("cy", y)
                    .num("r", radius)
                    .attr("fill", &fill);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Line(from, to) => {
                let (x1, y1) = self.at(from, y_origin);
                let (x2, y2) = self.at(to, y_origin);
                let element = Elem::new("line")
                    .num("x1", x1)
                    .num("y1", y1)
                    .num("x2", x2)
                    .num("y2", y2);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Ellipse => {
                let element = Elem::new("ellipse")
                    .num("cx", self.p.x + self.p.width / 2.0)
                    .num("cy", self.p.y + self.p.height / 2.0)
                    .num("rx", self.p.width / 2.0)
                    .num("ry", self.p.height / 2.0)
                    .attr("fill", &fill);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Polygon(points, color) => {
                let color = if color.is_empty() { fill.clone() } else { color.to_string() };
                let text = points
                    .iter()
                    .map(|point| {
                        let (x, y) = self.at(*point, y_origin);
                        format!("{},{}", ruby_float(x), ruby_float(y))
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let element = Elem::new("polygon").attr("points", text).attr("fill", &color);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Path(commands, color) => {
                let color = if color.is_empty() { fill.clone() } else { color.to_string() };
                let d = commands
                    .iter()
                    .map(|part| self.path_command(part, y_origin))
                    .collect::<Vec<_>>()
                    .join(" ");
                let element = Elem::new("path").attr("d", d).attr("fill", &color);
                self.out.push(self.apply_line(element));
            }
            FigureCommand::Cloud => {
                let element = Elem::new("path")
                    .attr("d", cloud_to_d(&self.p))
                    .attr("fill", &fill);
                self.out.push(self.apply_line(element));
            }
        }
    }

    fn path_command(&self, part: &PathPart, y_origin: f64) -> String {
        match part {
            PathPart::Z => "Z".to_string(),
            PathPart::M(spec) | PathPart::L(spec) => {
                let verb = match part {
                    PathPart::M(_) => "M",
                    _ => "L",
                };
                let (x, y) = self.at(*spec, y_origin);
                format!("{verb}{},{}", ruby_float(x), ruby_float(y))
            }
            PathPart::A(radii, sweep, target) => {
                let (rx, ry) = self.size(*radii);
                let (x, y) = self.at(*target, y_origin);
                format!(
                    "A{},{} 0 0 {} {},{}",
                    ruby_float(rx),
                    ruby_float(ry),
                    ruby_float(*sweep),
                    ruby_float(x),
                    ruby_float(y)
                )
            }
        }
    }

    fn lines(&self) -> Vec<String> {
        metrics::text_lines(self.node())
    }

    fn label(&mut self, options: LabelSpec) {
        let top = match options.top {
            Some(top) => self.p.y + top,
            None => self.middle(&options),
        };
        let (anchor, x_pos) = self.label_anchor(&options);
        let lines = self.lines();
        for (index, line) in lines.iter().enumerate() {
            let y = top + (index as f64 * LINE_HEIGHT) + ASCENT;
            self.caption(index == 0 && self.node().stereotype.is_some(), (x_pos, y), line, anchor);
        }
    }

    fn caption(&mut self, stereotype: bool, point: (f64, f64), line: &str, anchor: &str) {
        if !stereotype {
            self.text(point.0, point.1, line, anchor, None);
            return;
        }
        let y = if self.above_figure() {
            self.p.y + ASCENT
        } else {
            point.1
        };
        self.text(point.0, y, line, anchor, None);
        self.set_last_italic();
    }

    fn set_last_italic(&mut self) {
        if let Some(last) = self.out.last_mut() {
            last.set("font-style".into(), "italic".into());
        }
    }

    fn label_anchor(&self, options: &LabelSpec) -> (&'static str, f64) {
        if let Some(left) = options.left {
            return ("start", self.p.x + left);
        }
        let width = self.p.width - options.shrink.unwrap_or(0.0);
        ("middle", self.p.x + options.shift.unwrap_or(0.0) + width / 2.0)
    }

    fn middle(&self, options: &LabelSpec) -> f64 {
        self.p.y
            + (self.p.height - metrics::text_height(&self.lines())) / 2.0
            + options.dy.unwrap_or(0.0)
    }

    fn container(&mut self) {
        let kind = self.node().kind;
        let outline = container_figure(kind)
            .unwrap_or_else(|| vec![FigureCommand::Rect(ORIGIN, FULL, Some(2.5))]);
        for command in outline {
            self.execute(command);
        }
        self.decorate();
        let lines = self.lines();
        for (index, line) in lines.iter().enumerate() {
            self.header(line, index);
        }
    }

    fn decorate(&mut self) {
        let kind = self.node().kind;
        if tabbed(kind) {
            self.draw_tab();
        }
        if kind == NodeKind::Component {
            self.draw_icon();
        }
    }

    /// A component container carries the glyph of a component too.
    fn draw_icon(&mut self) {
        self.icon = true;
        for command in component_figure().into_iter().skip(1) {
            self.execute(command);
        }
    }

    fn draw_tab(&mut self) {
        let width = self.tab_text_width();
        let points = tab_points(self.p.x, self.p.y, width, self.node().kind);
        let mut element = Elem::new("polygon").attr("points", points).attr("fill", FILL);
        element = self.apply_line(element);
        self.out.push(element);
    }

    fn tab_text_width(&self) -> f64 {
        let first = self.lines().first().cloned().unwrap_or_default();
        crate::text::measure(&first, FONT_SIZE, Face::DejaVuBold).width
    }

    fn header(&mut self, line: &str, index: usize) {
        if tabbed(self.node().kind) && index == 0 {
            self.tab_text(line);
            return;
        }
        let weight = if index == 0 { Some("bold") } else { None };
        if self.node().kind == NodeKind::Component {
            self.centred_title(line, index, weight);
            return;
        }
        self.text(
            self.p.x + 12.0 + header_left(self.node().kind),
            self.header_top(index),
            line,
            "start",
            weight,
        );
    }

    fn centred_title(&mut self, line: &str, index: usize, weight: Option<&str>) {
        self.text(self.p.x + self.p.width / 2.0, self.header_top(index), line, "middle", weight);
    }

    fn header_top(&self, index: usize) -> f64 {
        self.p.y + 13.0 + ASCENT + header_drop(self.node().kind) + (index as f64 * LINE_HEIGHT)
    }

    fn tab_text(&mut self, line: &str) {
        let top = self.p.y + tab_baseline(self.node().kind);
        self.text(self.p.x + tab_indent(self.node().kind), top, line, "start", Some("bold"));
    }

    fn text(&mut self, x: f64, y: f64, content: &str, anchor: &str, weight: Option<&str>) {
        let mut element = Elem::new("text")
            .num("x", x)
            .num("y", y)
            .attr("text-anchor", anchor)
            .attr("font-family", FONT)
            .attr("font-size", "14")
            .attr("fill", "#000000");
        if let Some(weight) = weight {
            element = element.attr("font-weight", weight);
        }
        element = super::creole::assign(element, content);
        self.out.push(element);
    }
}

fn ink(color: Option<&str>) -> String {
    let Some(color) = color else { return INK.to_string() };
    if color.chars().all(|c| c.is_ascii_alphabetic()) && !color.is_empty() {
        color.to_string()
    } else {
        format!("#{color}")
    }
}

/// `Renderer::Base`'s link stroke: colour, width and the dash of a dashed
/// line.
fn apply_edge_stroke(element: Elem, edge: &Edge, color: &str) -> Elem {
    let mut element = element.attr("stroke", color).attr("stroke-width", &edge.width.to_string());
    if edge.style == Style::Dashed {
        element = element.attr("stroke-dasharray", DASH);
    }
    element
}

/// Ruby `Heads.for`: the inner points give each end its direction.
fn heads_for_edge(edge: &Edge, points: &[(f64, f64)], color: &str) -> Vec<Elem> {
    let from = points[0];
    let to = points[points.len() - 1];
    let inner = [
        points.get(1).copied().unwrap_or(to),
        points[points.len().saturating_sub(2)],
    ];
    heads::heads(edge, from, to, color, inner)
}

/// `Curve.path`: a straight line between two points, a smooth curve
/// through every point when the route bends.
fn curve_path(points: &[(f64, f64)]) -> String {
    let route = spaced(points);
    if route.len() < 3 {
        return format!(
            "M {} L {}",
            write_point(route[0]),
            write_point(route[route.len() - 1])
        );
    }
    let mut segments = vec![];
    for (index, pair) in route.windows(2).enumerate() {
        let before = route[index.saturating_sub(1)];
        let after = route[(index + 2).min(route.len() - 1)];
        segments.push(curve_segment(pair[0], pair[1], before, after));
    }
    format!("M {} {}", write_point(route[0]), segments.join(" "))
}

fn curve_segment(from: (f64, f64), to: (f64, f64), before: (f64, f64), after: (f64, f64)) -> String {
    format!(
        "C {} {} {}",
        write_point(curve_control(from, to, before)),
        write_point(curve_control(to, from, after)),
        write_point(to)
    )
}

fn spaced(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut kept: Vec<(f64, f64)> = vec![];
    for point in &points[..points.len() - 1] {
        if kept.is_empty() || curve_far(kept[kept.len() - 1], *point) {
            kept.push(*point);
        }
    }
    if kept.len() > 1 && !curve_far(kept[kept.len() - 1], points[points.len() - 1]) {
        kept.pop();
    }
    kept.push(points[points.len() - 1]);
    kept
}

fn curve_far(from: (f64, f64), to: (f64, f64)) -> bool {
    (to.0 - from.0).hypot(to.1 - from.1) >= CURVE_MIN_GAP
}

fn curve_control(anchor: (f64, f64), ahead: (f64, f64), behind: (f64, f64)) -> (f64, f64) {
    (
        anchor.0 + (ahead.0 - behind.0) / 6.0,
        anchor.1 + (ahead.1 - behind.1) / 6.0,
    )
}

fn write_point(point: (f64, f64)) -> String {
    format!(
        "{} {}",
        ruby_float(crate::render::ditaa::ruby_round(point.0, 2)),
        ruby_float(crate::render::ditaa::ruby_round(point.1, 2))
    )
}

/// The pointer of a note: a pale triangle with its base on the note and
/// its tip on the note's target. Ruby `NoteTail`.
fn note_tail(points: &[(f64, f64)]) -> Elem {
    let base = points[0];
    let tip = points[points.len() - 1];
    let (dx, dy) = (tip.0 - base.0, tip.1 - base.1);
    let scale = NOTE_HALF / dx.hypot(dy).max(1.0);
    let across = (-dy * scale, dx * scale);
    let outline = [
        (base.0 + across.0, base.1 + across.1),
        tip,
        (base.0 - across.0, base.1 - across.1),
    ];
    let text = outline
        .iter()
        .map(|point| {
            format!(
                "{},{}",
                ruby_float(crate::render::ditaa::ruby_round(point.0, 2)),
                ruby_float(crate::render::ditaa::ruby_round(point.1, 2))
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    Elem::new("polygon")
        .attr("points", text)
        .attr("fill", "#FEFFDD")
        .attr("stroke", "#181818")
        .attr("stroke-width", "0.5")
}

/// A link drawn: a note's pointer, or line + heads + the second stroke of
/// a two-colour link.
fn link_elements(route: &Route) -> Vec<Elem> {
    let edge = &route.edge;
    if edge.pointer {
        return vec![note_tail(&route.points)];
    }
    let points = &route.points;
    let color = ink(edge.color.as_deref());
    let mut elements = vec![];
    elements.push(line_element(points, edge, &color));
    elements.extend(heads_for_edge(edge, points, &color));
    if let Some(color2) = &edge.color2 {
        let mut copy = line_element(points, edge, &ink(Some(color2)));
        copy.set("transform".into(), SECOND_OFFSET.into());
        elements.push(copy);
    }
    elements
}

fn line_element(points: &[(f64, f64)], edge: &Edge, color: &str) -> Elem {
    if points.len() > 2 {
        let element = Elem::new("path")
            .attr("d", curve_path(points))
            .attr("fill", "none");
        return apply_edge_stroke(element, edge, color);
    }
    let element = Elem::new("line")
        .num("x1", points[0].0)
        .num("y1", points[0].1)
        .num("x2", points[points.len() - 1].0)
        .num("y2", points[points.len() - 1].1);
    apply_edge_stroke(element, edge, color)
}

fn edge_label_text(x: f64, y: f64, content: &str, weight: Option<&str>) -> Elem {
    let mut element = Elem::new("text")
        .num("x", x)
        .num("y", y)
        .attr("text-anchor", "middle")
        .attr("font-family", "sans-serif")
        .attr("font-size", "12")
        .attr("fill", "#000000");
    if let Some(weight) = weight {
        element = element.attr("font-weight", weight);
    }
    super::creole::assign(element, content)
}

/// Ruby `NodeGraph::Renderer#render`: label placements, the document
/// sized to cover them, then containers, links, leaves, labels and title.
pub fn render_document(graph: &Graph, scene: &Scene) -> String {
    let title_area = title_area(scene);
    let mut boxes: Vec<Placed> = scene.placed.clone();
    boxes.extend(title_area);
    let placements = label_placer::call(&scene.routes, &boxes);
    let width = placements
        .iter()
        .map(|placement| placement.right())
        .chain(std::iter::once(scene.width))
        .fold(f64::NEG_INFINITY, f64::max);
    let height = placements
        .iter()
        .map(|placement| placement.bottom())
        .chain(std::iter::once(scene.height))
        .fold(f64::NEG_INFINITY, f64::max);
    let mut document = crate::svg::Document::new(width, height);
    document.view_box = Some(format!("0 0 {} {}", ruby_float(width), ruby_float(height)));
    for element in elements(graph, scene, &placements) {
        document.add(element);
    }
    document.to_xml()
}

fn elements(graph: &Graph, scene: &Scene, placements: &[label_placer::Placement]) -> Vec<Elem> {
    let mut containers: Vec<&Placed> =
        scene.placed.iter().filter(|boxed| graph.nodes[boxed.node].container).collect();
    containers.sort_by_key(|boxed| boxed.depth);
    let mut out = vec![];
    for boxed in &containers {
        out.extend(Shapes::draw(graph, boxed));
    }
    for route in &scene.routes {
        out.extend(link_elements(route));
    }
    for boxed in scene.placed.iter().filter(|boxed| !graph.nodes[boxed.node].container) {
        out.extend(Shapes::draw(graph, boxed));
    }
    for placement in placements {
        let label = scene.routes[placement.route_index]
            .edge
            .label
            .clone()
            .unwrap_or_default();
        out.push(edge_label_text(placement.x, placement.y, &label, None));
    }
    if let Some(title) = &scene.title {
        out.push(edge_label_text(scene.width / 2.0, 22.0, title, Some("bold")));
    }
    out
}

/// The room the title takes, which no label covers.
fn title_area(scene: &Scene) -> Vec<Placed> {
    let Some(title) = &scene.title else { return vec![] };
    let width = face_width(title, TITLE_SIZE);
    vec![Placed {
        node: usize::MAX,
        x: (scene.width - width) / 2.0,
        y: 8.0,
        width,
        height: 18.0,
        depth: 0,
    }]
}

