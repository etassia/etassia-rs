//! The PlantUML state diagram (`@startuml` with state statements): port of
//! `lib/etassia/notation/plantuml/state/` — the reader, the placement, the
//! routes, the scene and the renderer — plus the `Look` measures, `Curve`
//! and `Extent` the layout draws with, over the shared SVG layer.

pub mod hierarchy;
pub mod ir;
pub mod reader;
mod routes;
pub mod style;

use crate::svg::{ruby_float, Document, Elem, Theme};
use std::collections::HashMap;
use crate::text::{self, Face};
use crate::Error;
use hierarchy::Slot;
use ir::{GEdge, GLabel, GNode, Graph, Section};

// ---- look.rb ----

/// The measures of PlantUML 1.2026.6's state drawing, read from its SVG.
pub struct Look;

impl Look {
    pub const TEXT_FONT: f64 = 14.0;
    pub const TEXT_LINE: f64 = 16.49;
    pub const TEXT_PAD: f64 = 6.0;
    pub const LEAF_BAR: f64 = 26.49;
    pub const BAR: f64 = 22.0;
    pub const EMPTY_BODY: f64 = 23.51;
    pub const NOTE_FONT: f64 = 13.0;
    pub const NOTE_LINE: f64 = 15.31;
    pub const NOTE_PAD_X: f64 = 6.0;
    pub const NOTE_PAD_RIGHT: f64 = 15.0;
    pub const NOTE_PAD_Y: f64 = 5.0;
    pub const CANVAS_MARGIN: f64 = 20.0;
    pub const FOLD: f64 = 10.0;
    pub const TAIL: f64 = 4.0;
    pub const HISTORY_SIZE: (f64, f64) = (22.0, 22.0);
    pub const SMALL_FONT: f64 = 12.0;
    pub const TERMINAL_RADIUS: [(&'static str, f64); 2] = [("start", 10.0), ("end", 11.0)];

    /// A start or end marker's width and height.
    pub fn terminal(kind: &str) -> (f64, f64) {
        let radius = Self::TERMINAL_RADIUS
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| *r)
            .unwrap_or(0.0);
        (radius * 2.0, radius * 2.0)
    }
}

// ---- curve.rb ----

/// The curve a transition is drawn as: PlantUML draws splines where a
/// layout gives a route of bends.
pub struct Curve;

const MIN_GAP: f64 = 12.0;

impl Curve {
    /// SVG path data: a line for two points, a smooth curve through every
    /// point for more.
    pub fn path(points: &[(f64, f64)]) -> String {
        let route = Self::spaced(points);
        let mut out = format!("M {} ", write_point(route[0]));
        if route.len() < 3 {
            out.push_str(&format!("L {}", write_point(route[route.len() - 1])));
        } else {
            out.push_str(&Self::curves(&route));
        }
        out
    }

    fn curves(route: &[(f64, f64)]) -> String {
        let mut out = String::new();
        for (index, to) in route.windows(2).enumerate() {
            let to = to[1];
            out.push_str(&Self::segment(route, index, to));
            out.push(' ');
        }
        out.trim_end().to_string()
    }

    fn spaced(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut kept = Self::thinned(&points[..points.len() - 1]);
        if kept.len() > 1 && !Self::far(kept[kept.len() - 1], points[points.len() - 1]) {
            kept.pop();
        }
        kept.push(points[points.len() - 1]);
        kept
    }

    fn thinned(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut found: Vec<(f64, f64)> = vec![];
        for &point in points {
            if found.is_empty() || Self::far(found[found.len() - 1], point) {
                found.push(point);
            }
        }
        found
    }

    fn far(from: (f64, f64), to: (f64, f64)) -> bool {
        ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt() >= MIN_GAP
    }

    /// Catmull-Rom segment from point `index` to `to`, as a cubic.
    fn segment(route: &[(f64, f64)], index: usize, to: (f64, f64)) -> String {
        let from = route[index];
        let before = route[index.saturating_sub(1)];
        let after = route[(index + 2).min(route.len() - 1)];
        format!(
            "C {} {} {}",
            write_point(Self::control(from, to, before)),
            write_point(Self::control(to, from, after)),
            write_point(to)
        )
    }

    /// The control point a sixth of the way along the chord between the
    /// neighbours of `anchor`, on the side of `ahead`.
    fn control(anchor: (f64, f64), ahead: (f64, f64), behind: (f64, f64)) -> (f64, f64) {
        (
            anchor.0 + ((ahead.0 - behind.0) / 6.0),
            anchor.1 + ((ahead.1 - behind.1) / 6.0),
        )
    }
}

fn write_point(point: (f64, f64)) -> String {
    format!("{} {}", round2(point.0), round2(point.1))
}

fn round2(v: f64) -> String {
    // Ruby `Float#round(2)`: decimal rounding, then `Float#to_s`.
    ruby_float((v * 100.0).round() / 100.0)
}

// ---- extent.rb ----

/// How far a placed state graph reaches right and down.
fn extent_of(graph: &Graph) -> (f64, f64) {
    let mut reach: Vec<(f64, f64)> = graph.children.iter().map(corner).collect();
    for edge in &graph.edges {
        reach.extend(edge_reach(edge));
    }
    let x = reach.iter().map(|r| r.0).fold(f64::MIN, f64::max);
    let y = reach.iter().map(|r| r.1).fold(f64::MIN, f64::max);
    (x, y)
}

fn corner(node: &GNode) -> (f64, f64) {
    (node.x.unwrap_or(0.0) + node.width, node.y.unwrap_or(0.0) + node.height)
}

fn edge_reach(edge: &GEdge) -> Vec<(f64, f64)> {
    let mut reach: Vec<(f64, f64)> = vec![];
    if let Some(sections) = &edge.sections {
        for part in sections {
            reach.push(part.start_point);
            reach.extend(part.bend_points.iter().copied());
            reach.push(part.end_point);
        }
    }
    if let Some((x, y)) = edge.label_at {
        if let Some(label) = edge.labels.first() {
            reach.push((x + label.width / 2.0, y + label.height / 2.0));
        }
    }
    reach
}

// ---- note_shape.rb ----

/// The outline of a note: a sheet with its top-right corner folded down
/// and, on the side that faces the state it annotates, a tail to that
/// state's border.
struct NoteShape<'a> {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    target: Option<&'a SNode>,
}

impl<'a> NoteShape<'a> {
    fn new(note: &'a SNode, target: Option<&'a SNode>) -> NoteShape<'a> {
        NoteShape {
            left: note.x,
            top: note.y,
            right: note.x + note.width,
            bottom: note.y + note.height,
            target,
        }
    }

    /// Path data of the sheet and its tail.
    fn outline(&self) -> String {
        let mut corners: Vec<(f64, f64)> = vec![
            (self.left, self.top),
            (self.left, self.bottom),
            (self.right, self.bottom),
            (self.right, self.top + Look::FOLD),
            (self.right - Look::FOLD, self.top),
        ];
        if let Some((index, tail)) = self.tail_at() {
            for (at, point) in tail.into_iter().enumerate() {
                corners.insert(index + at, point);
            }
        }
        closed(&corners)
    }

    /// Path data of the folded corner.
    fn fold(&self) -> String {
        let edge = self.right - Look::FOLD;
        closed(&[
            (edge, self.top),
            (edge, self.top + Look::FOLD),
            (self.right, self.top + Look::FOLD),
        ])
    }

    fn middle(&self) -> f64 {
        (self.top + self.bottom) / 2.0
    }

    /// Where in the outline the tail goes, and its three points.
    fn tail_at(&self) -> Option<(usize, [(f64, f64); 3])> {
        let target = self.target?;
        if target.x >= self.right {
            Some((3, self.tail(self.right, target.x)))
        } else if target.x + target.width <= self.left {
            let tail = self.tail(self.left, target.x + target.width);
            Some((1, [tail[2], tail[1], tail[0]]))
        } else {
            None
        }
    }

    /// Drawn upward along the right side, downward along the left.
    fn tail(&self, side: f64, tip_x: f64) -> [(f64, f64); 3] {
        let target = self.target.expect("target");
        let tip_y = self.middle().clamp(target.y, target.y + target.height);
        let middle = self.middle();
        [
            (side, middle + Look::TAIL),
            (tip_x, tip_y),
            (side, middle - Look::TAIL),
        ]
    }
}

fn closed(points: &[(f64, f64)]) -> String {
    let parts: Vec<String> = points
        .iter()
        .map(|p| format!("{} {}", ruby_float(p.0), ruby_float(p.1)))
        .collect();
    format!("M {} Z", parts.join(" L "))
}

// ---- the typed scene (layout/state_diagram.rb scene_from_graph) ----

/// A label of the typed scene.
#[derive(Clone, Debug)]
pub struct SLabel {
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub font_size: f64,
}

/// A node of the typed scene.
#[derive(Clone, Debug)]
pub struct SNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub labels: Vec<SLabel>,
    pub state_type: String,
    pub shape_type: String,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub inner_radius: f64,
    pub shape_points: Option<String>,
    pub shape_y: f64,
    pub shape_height: f64,
    pub children: Vec<SNode>,
}

/// An edge of the typed scene.
pub struct SEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub path: String,
    pub labels: Vec<SLabel>,
}

/// The scene the renderer draws.
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub view_box: String,
    pub children: Vec<SNode>,
    pub edges: Vec<SEdge>,
}

// ---- renderer state ----

struct Renderer {
    style: style::Sheet,
    scale: Option<(f64, reader::ScaleAxis)>,
    owner: Option<String>,
    states: HashMap<String, SNode>,
}

pub fn render(source: &str) -> Result<String, Error> {
    let (diagram, scale) = reader::read(source)?;
    let sheet = style::read(source);
    let mut graph = ir::build_graph(&diagram);
    let options = graph_options();
    let (mut slots, roots) = hierarchy::arena_of(&graph);
    let mut placer = hierarchy::Hierarchy::new(slots, roots, std::mem::take(&mut graph.edges), diagram.flat.clone(), options);
    placer.place()?;
    graph.children = hierarchy::graph_of(&placer.slots, &placer.roots).children;
    graph.edges = placer.edges;
    let scene = scene_from_graph(&graph);
    let mut renderer = Renderer { style: sheet, scale, owner: None, states: HashMap::new() };
    Ok(renderer.render(&scene))
}

/// The state layout's ELK options (layout/state_diagram.rb
/// `layout_options`): layered, downward, PlantUML's spacings.
pub fn graph_options_pub() -> serde_json::Map<String, serde_json::Value> {
    graph_options()
}

fn graph_options() -> serde_json::Map<String, serde_json::Value> {
    let mut options = serde_json::Map::new();
    options.insert("elk.algorithm".to_string(), serde_json::json!("layered"));
    options.insert("elk.direction".to_string(), serde_json::json!("DOWN"));
    options.insert("elk.spacing.nodeNode".to_string(), serde_json::json!(60));
    options.insert("elk.layered.spacing.nodeNodeBetweenLayers".to_string(), serde_json::json!(60));
    options.insert("elk.spacing.edgeNode".to_string(), serde_json::json!(40));
    options.insert("elk.spacing.edgeEdge".to_string(), serde_json::json!(30));
    options.insert("elk.layered.nodePlacement.strategy".to_string(), serde_json::json!("SIMPLE"));
    options
}

fn scene_from_graph(graph: &Graph) -> Scene {
    let children = typed_children(&graph.children, 0.0, 0.0);
    let raw_children = positioned_children(&graph.children, 0.0, 0.0);
    // The canvas ends a margin past the last box, route or label (the
    // PlantUML layout's `scene_geometry`).
    let (far_x, far_y) = extent_of(graph);
    let width = far_x + Look::CANVAS_MARGIN;
    let height = far_y + Look::CANVAS_MARGIN;
    let edges = typed_edges(&graph.edges, &children, &raw_children);
    Scene {
        width,
        height,
        view_box: format!("0 0 {} {}", ruby_float(width), ruby_float(height)),
        children,
        edges,
    }
}

fn typed_children(children: &[GNode], parent_x: f64, parent_y: f64) -> Vec<SNode> {
    children
        .iter()
        .map(|state| typed_node(state, parent_x, parent_y))
        .collect()
}

fn typed_node(state: &GNode, parent_x: f64, parent_y: f64) -> SNode {
    let x = state.x.unwrap_or(0.0) + parent_x;
    let y = state.y.unwrap_or(0.0) + parent_y;
    let state_type = state.meta.state_type.clone();
    let shape_type = if state.meta.shape_type.is_empty() {
        state_type.clone()
    } else {
        state.meta.shape_type.clone()
    };
    let mut node = SNode {
        id: state.id.clone(),
        x,
        y,
        width: state.width,
        height: state.height,
        labels: typed_labels(state, x, y),
        state_type,
        shape_type: shape_type.clone(),
        center_x: x + state.width / 2.0,
        center_y: y + state.height / 2.0,
        radius: state
            .meta
            .shape_radius
            .unwrap_or(state.width.min(state.height) / 2.0),
        inner_radius: 0.0,
        shape_points: None,
        shape_y: y + state.height / 2.0 - 5.0,
        shape_height: 10.0,
        children: typed_children(&state.children, x, y),
    };
    node.inner_radius = node.radius - 5.0;
    if shape_type == "choice" {
        node.shape_points = Some(format!(
            "{},{} {},{} {},{} {},{}",
            node.center_x, y,
            x + state.width, node.center_y,
            node.center_x, y + state.height,
            x, node.center_y,
        ));
    }
    node
}

/// The labels a state draws (the PlantUML layout's `typed_labels`).
fn typed_labels(state: &GNode, x: f64, y: f64) -> Vec<SLabel> {
    match state.meta.shape_type.as_str() {
        "start" | "end" => vec![],
        "note" => state
            .labels
            .iter()
            .enumerate()
            .map(|(index, line)| text_label(
                line,
                x + Look::NOTE_PAD_X,
                y + Look::NOTE_PAD_Y + (Look::NOTE_LINE / 2.0)
                    + (index as f64 * Look::NOTE_LINE),
                Look::NOTE_FONT,
            ))
            .collect(),
        _ => titled_labels(state, x, y),
    }
}

fn titled_labels(state: &GNode, x: f64, y: f64) -> Vec<SLabel> {
    let mut labels = state.labels.iter();
    let Some(name) = labels.next() else { return vec![] };
    let bar = if state.children.is_empty() { Look::LEAF_BAR } else { Look::BAR };
    let mut out = vec![text_label(name, x + state.width / 2.0, y + bar / 2.0, Look::TEXT_FONT)];
    for (index, line) in labels.enumerate() {
        out.push(text_label(
            line,
            x + Look::TEXT_PAD,
            y + bar + 13.0 + (index as f64 * Look::TEXT_LINE),
            Look::TEXT_FONT,
        ));
    }
    out
}

fn text_label(line: &GLabel, x: f64, y: f64, size: f64) -> SLabel {
    SLabel {
        text: line.text.clone(),
        width: line.width,
        height: line.height,
        x,
        y,
        font_size: size,
    }
}

fn positioned_children(children: &[GNode], parent_x: f64, parent_y: f64) -> Vec<GNode> {
    children
        .iter()
        .flat_map(|state| {
            let mut positioned = state.clone();
            positioned.x = Some(state.x.unwrap_or(0.0) + parent_x);
            positioned.y = Some(state.y.unwrap_or(0.0) + parent_y);
            let px = positioned.x.unwrap_or(0.0);
            let py = positioned.y.unwrap_or(0.0);
            let mut out = vec![positioned];
            out.extend(positioned_children(&state.children, px, py));
            out
        })
        .collect()
}

fn typed_edges(edges: &[GEdge], children: &[SNode], raw_children: &[GNode]) -> Vec<SEdge> {
    let nodes: HashMap<&str, &SNode> = flatten(children)
        .into_iter()
        .map(|n| (n.id.as_str(), n))
        .collect();
    let raw_nodes: HashMap<&str, &GNode> =
        raw_children.iter().map(|n| (n.id.as_str(), n)).collect();
    edges
        .iter()
        .filter_map(|edge| {
            let source = nodes.get(edge.sources[0].as_str())?;
            let target = nodes.get(edge.targets[0].as_str())?;
            let raw_source = raw_nodes.get(source.id.as_str()).copied();
            let raw_target = raw_nodes.get(target.id.as_str()).copied();
            let sections = typed_sections(edge, raw_source, raw_target);
            // The PlantUML edge labels sit at their routed spot.
            let labels = match edge.label_at {
                Some((x, y)) => match edge.labels.first() {
                    Some(label) => vec![SLabel {
                        text: label.text.clone(),
                        width: label.width,
                        height: label.height,
                        x,
                        y,
                        font_size: Look::SMALL_FONT,
                    }],
                    None => vec![],
                },
                None => vec![],
            };
            Some(SEdge {
                id: edge.id.clone(),
                source: source.id.clone(),
                target: target.id.clone(),
                path: sections
                    .iter()
                    .map(|s| {
                        let mut points = vec![s.start_point];
                        points.extend(s.bend_points.iter().copied());
                        points.push(s.end_point);
                        Curve::path(&points)
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                labels,
            })
        })
        .collect()
}

fn flatten(children: &[SNode]) -> Vec<&SNode> {
    children
        .iter()
        .flat_map(|n| std::iter::once(n).chain(flatten(&n.children)))
        .collect()
}

fn typed_sections(edge: &GEdge, source: Option<&GNode>, target: Option<&GNode>) -> Vec<Section> {
    let center = |n: &GNode| {
        (n.x.unwrap_or(0.0) + n.width / 2.0, n.y.unwrap_or(0.0) + n.height / 2.0)
    };
    let source_point = source.map(center);
    let target_point = target.map(center);
    match &edge.sections {
        Some(sections) if !sections.is_empty() => sections
            .iter()
            .map(|section| Section {
                start_point: section.start_point,
                end_point: section.end_point,
                bend_points: section.bend_points.clone(),
            })
            .collect(),
        _ => match (source_point, target_point) {
            (Some(from), Some(to)) => vec![Section {
                start_point: from,
                end_point: to,
                bend_points: vec![],
            }],
            _ => vec![],
        },
    }
}

// ---- the renderer ----

fn fmt_size(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        ruby_float(v)
    }
}

impl Renderer {
    fn look(&self, property: style::Prop, id: Option<&str>) -> Option<style::Value> {
        let stereotype = match id {
            Some(id) => self.style.element(id).stereotype.clone(),
            None => None,
        };
        self.style
            .resolve("state", property, id, stereotype.as_deref())
    }

    fn string_look(&self, property: style::Prop, id: Option<&str>) -> String {
        match self.look(property, id) {
            Some(style::Value::Str(s)) => s,
            Some(style::Value::Num(n)) | Some(style::Value::Int(n)) => ruby_float(n),
            None => String::new(),
        }
    }

    fn num_look(&self, property: style::Prop, id: Option<&str>) -> f64 {
        match self.look(property, id) {
            Some(style::Value::Num(n)) | Some(style::Value::Int(n)) => n,
            Some(style::Value::Str(s)) => s.parse().unwrap_or(0.0),
            None => 0.0,
        }
    }

    fn label_look(&self, property: style::Prop) -> String {
        match &self.owner {
            Some(owner) => self.string_look(property, Some(owner)),
            None => self.string_prop(property, "arrow"),
        }
    }

    fn render(&mut self, scene: &Scene) -> String {
        let theme = Theme::default_theme();
        self.states = index_states(&scene.children);
        let mut doc = Document::new(scene.width, scene.height);
        doc.view_box = Some(scene.view_box.clone());
        // `scale` sizes the picture, not what is drawn: the viewBox keeps
        // the drawing's own units.
        if let Some((value, axis)) = self.scale {
            let factor = match axis {
                reader::ScaleAxis::Width => value / scene.width,
                reader::ScaleAxis::Height => value / scene.height,
                reader::ScaleAxis::Factor => value,
            };
            doc.width = Some(doc.width.unwrap_or(scene.width) * factor);
            doc.height = Some(doc.height.unwrap_or(scene.height) * factor);
        }
        // The canvas (Ruby `paint_canvas`: the document's background).
        let colour = match self
            .style
            .resolve("document", style::Prop::Background, None, None)
        {
            Some(style::Value::Str(s)) => s,
            _ => String::new(),
        };
        if colour != "none" && !colour.is_empty() {
            doc.add(
                Elem::new("rect")
                    .num("x", 0.0)
                    .num("y", 0.0)
                    .num("width", scene.width)
                    .num("height", scene.height)
                    .attr("fill", colour),
            );
        }
        let _ = theme;
        // PlantUML draws its transitions above the states, so a transition
        // inside a composite is not painted over by the composite's fill.
        for state in &scene.children {
            let group = self.state_group(state);
            doc.add(group);
        }
        self.owner = None;
        for edge in &scene.edges {
            doc.add(self.transition_group(edge));
        }
        doc.to_xml()
    }

    /// The document element is styled by `resolve(:document, :background)`
    /// only; a sheet that sets nothing draws no canvas (the defaults carry
    /// `#FFFFFF` for the document).
    fn has_document_background(&self) -> bool {
        true
    }

    fn transition_group(&mut self, edge: &SEdge) -> Elem {
        let mut group = Elem::new("g").attr("id", format!("transition-{}", edge.id));
        let stroke = self.string_prop(style::Prop::Border, "arrow");
        let stroke_width = self.thickness_prop("arrow");
        let path = Elem::new("path")
            .attr("d", edge.path.clone())
            .attr("fill", "none")
            .attr("stroke", stroke.clone())
            .attr("stroke-width", stroke_width.clone());
        group.push(path);
        // SVG Tiny 1.2 cannot reference a <marker>; the head is drawn here
        // instead (Ruby `Svg::Arrowhead`).
        for head in crate::svg::Arrowhead::for_path(&crate::svg::ArrowPath {
            d: &edge.path,
            stroke: Some(&stroke),
            stroke_width: Some(&stroke_width),
            marker_end: Some("url(#arrowhead)"),
            ..Default::default()
        }) {
            group.push(head);
        }
        if let Some(label) = edge.labels.first() {
            group.push(self.label_text(label, None));
        }
        group
    }

    fn string_prop(&self, property: style::Prop, element: &str) -> String {
        match self.style.resolve(element, property, None, None) {
            Some(style::Value::Str(s)) => s,
            Some(style::Value::Num(n)) | Some(style::Value::Int(n)) => ruby_float(n),
            None => String::new(),
        }
    }

    fn thickness_prop(&self, element: &str) -> String {
        match self.style.resolve(element, style::Prop::LineThickness, None, None) {
            Some(style::Value::Int(n)) => format!("{}", n as i64),
            Some(style::Value::Num(n)) => ruby_float(n),
            Some(style::Value::Str(s)) => s,
            None => String::new(),
        }
    }

    fn label_text(&mut self, label: &SLabel, state: Option<&str>) -> Elem {
        let mut text = Elem::new("text")
            .content(&label.text)
            .attr("fill", self.label_look(style::Prop::FontColour))
            .attr("font-family", self.label_look(style::Prop::FontName))
            .attr("font-size", fmt_size(label.font_size))
            .attr("text-anchor", "middle");
        if state.is_none() {
            // The transition labels carry no dominant baseline.
        } else {
            text = text.attr("dominant-baseline", "middle");
        }
        text = text.attr("x", ruby_float(label.x));
        text = text.attr("y", ruby_float(label.y));
        if state.is_some() {
            let id = state.unwrap().to_string();
            let left_aligned = {
                let node = self.states.get(&id);
                match node {
                    Some(n) => {
                        n.shape_type == "note"
                            || (n.labels.len() > 1
                                && n.labels.iter().position(|l| l.text == label.text).unwrap_or(0) > 0
                                && matches!(n.shape_type.as_str(), "normal" | ""))
                    }
                    None => false,
                }
            };
            if left_aligned {
                text = text.attr("text-anchor", "start".to_string());
            }
        }
        text
    }

    fn state_group(&mut self, state: &SNode) -> Elem {
        let mut group = Elem::new("g").attr("id", format!("state-{}", state.id));
        self.owner = Some(state.id.clone());
        group.push(self.state_shape(state));
        // The band goes just beneath the outline (Ruby `underlay_band`:
        // `children.insert(-2, band)` — first when the outline is alone).
        if self.banded(state) {
            let band = self.band(state);
            let at = group.children_len().saturating_sub(1);
            group.insert_child(at, band);
        }
        // The divider under the name of every plain state and composite.
        if self.divided(state) {
            let bar = if state.children.is_empty() { Look::LEAF_BAR } else { Look::BAR };
            let mut line = Elem::new("line")
                .num("x1", state.x)
                .num("y1", state.y + bar)
                .num("x2", state.x + state.width)
                .num("y2", state.y + bar);
            self.stroke(&mut line, Some(&state.id));
            group.push(line);
        }
        for label in &state.labels {
            let text = self.label_text_owned(label, &state.id, state);
            group.push(text);
        }
        for child in &state.children {
            let inner = self.state_group(child);
            group.push(inner);
        }
        self.owner = None;
        group
    }

    /// A composite left at the default background draws a band; a colour
    /// the diagram set fills the whole composite.
    fn banded(&self, state: &SNode) -> bool {
        !state.children.is_empty()
            && self.string_look(style::Prop::Background, Some(&state.id))
                == style::defaults_box_background()
    }

    /// A divider runs under the name of a plain state or composite.
    fn divided(&self, state: &SNode) -> bool {
        state.shape_type.is_empty() || state.shape_type == "normal"
    }

    fn label_text_owned(&mut self, label: &SLabel, id: &str, state: &SNode) -> Elem {
        let mut text = Elem::new("text")
            .content(&label.text)
            .attr("fill", self.label_look(style::Prop::FontColour))
            .attr("font-family", self.label_look(style::Prop::FontName))
            .attr("font-size", fmt_size(label.font_size))
            .attr("text-anchor", "middle")
            .attr("dominant-baseline", "middle");
        text = text.attr("x", ruby_float(label.x));
        text = text.attr("y", ruby_float(label.y));
        let left_aligned = state.shape_type == "note"
            || (state.labels.iter().position(|l| l.text == label.text).unwrap_or(0) > 0
                && matches!(state.shape_type.as_str(), "normal" | ""));
        if left_aligned {
            text = text.attr("text-anchor", "start".to_string());
        }
        let _ = id;
        text
    }

    fn stroke(&mut self, element: &mut Elem, id: Option<&str>) {
        element.set("stroke", self.string_look(style::Prop::Border, id));
        element.set(
            "stroke-width",
            match self.look(style::Prop::LineThickness, id) {
                Some(style::Value::Int(n)) => format!("{}", n as i64),
                Some(style::Value::Num(n)) => ruby_float(n),
                _ => String::new(),
            },
        );
        let dash = match self.look(style::Prop::LineStyle, id) {
            Some(style::Value::Str(s)) => Some(s),
            _ => None,
        };
        if let Some(dash) = dash {
            element.set("stroke-dasharray", dash);
        }
    }

    fn state_shape(&mut self, state: &SNode) -> Elem {
        match state.shape_type.as_str() {
            "start" => Elem::new("circle")
                .num("cx", state.center_x)
                .num("cy", state.center_y)
                .num("r", state.radius)
                .attr("stroke", "none")
                .attr(
                    "fill",
                    self.string_prop(style::Prop::Background, "start"),
                ),
            "end" => {
                let mut group = Elem::new("g");
                let mut ring = Elem::new("circle")
                    .num("cx", state.center_x)
                    .num("cy", state.center_y)
                    .num("r", state.radius)
                    .attr("fill", "none")
                    .attr("stroke-width", "2");
                ring = ring.attr("stroke", self.string_prop(style::Prop::Border, "start"),);
                let mut dot = Elem::new("circle")
                    .num("cx", state.center_x)
                    .num("cy", state.center_y)
                    .num("r", state.inner_radius)
                    .attr("stroke", "none");
                dot = dot.attr("fill", self.string_prop(style::Prop::Background, "start"),);
                group.push(ring);
                group.push(dot);
                group
            }
            "history" => {
                let mut circle = Elem::new("circle")
                    .num("cx", state.center_x)
                    .num("cy", state.center_y)
                    .num("r", state.width / 2.0)
                    .attr(
                        "fill",
                        self.string_look(style::Prop::Background, Some(&state.id)),
                    );
                self.stroke(&mut circle, Some(&state.id));
                circle
            }
            "note" => self.note_shape(state),
            _ => self.outline(state),
        }
    }

    /// PlantUML fills only the name band of a composite left at the
    /// default background and leaves its body clear; a colour the diagram
    /// sets fills the whole composite. The frame stays the group's first
    /// child either way, so its box is the state's box.
    fn outline(&mut self, state: &SNode) -> Elem {
        let background = self.string_look(style::Prop::Background, Some(&state.id));
        let fill = if self.banded(state) { "none".to_string() } else { background };
        let mut rect = Elem::new("rect")
            .num("x", state.x)
            .num("y", state.y)
            .num("width", state.width)
            .num("height", state.height)
            .attr("fill", fill);
        self.stroke(&mut rect, Some(&state.id));
        let radius = self.num_look(style::Prop::RoundCorner, Some(&state.id));
        rect = rect.attr("rx", ruby_float(radius));
        rect = rect.attr("ry", ruby_float(radius));
        rect
    }

    /// The name band: rounded above, square below, as far down as the
    /// divider.
    fn band(&mut self, state: &SNode) -> Elem {
        let left = state.x;
        let right = left + state.width;
        let top = state.y;
        let radius = self.num_look(style::Prop::RoundCorner, Some(&state.id));
        let bottom = top + Look::BAR;
        let d = format!(
            "M{},{} L{},{} A{},{} 0 0 1 {},{} L{},{} L{},{} L{},{} A{},{} 0 0 1 {},{}",
            ruby_num(left + radius), ruby_num(top),
            ruby_num(right - radius), ruby_num(top),
            ruby_num(radius), ruby_num(radius),
            ruby_num(right), ruby_num(top + radius),
            ruby_num(right), ruby_num(bottom),
            ruby_num(left), ruby_num(bottom),
            ruby_num(left), ruby_num(top + radius),
            ruby_num(radius), ruby_num(radius),
            ruby_num(left + radius), ruby_num(top),
        );
        let mut path = Elem::new("path").attr("d", d);
        path = path.attr("fill", self.string_look(style::Prop::Background, Some(&state.id)),);
        path
    }

    /// A pale sheet with a folded corner and a tail to its state.
    fn note_shape(&mut self, state: &SNode) -> Elem {
        let target_id = state
            .id
            .split("----note")
            .next()
            .unwrap_or(&state.id)
            .to_string();
        let target = self.states.get(&target_id);
        let sheet = NoteShape::new(state, target);
        let mut group = Elem::new("g");
        for data in [sheet.outline(), sheet.fold()] {
            let mut path = Elem::new("path").attr("d", data);
            path = path.attr("fill", self.string_prop(style::Prop::Background, "note"),);
            path = path.attr("stroke", self.string_prop(style::Prop::Border, "note"),);
            path = path.attr("stroke-width", self.thickness_prop("note"),);
            group.push(path);
        }
        group
    }
}

/// Ruby `Float#to_s` in string interpolation.
fn ruby_num(v: f64) -> String {
    ruby_float(v)
}

fn index_states(children: &[SNode]) -> HashMap<String, SNode> {
    let mut out = HashMap::new();
    fn walk(nodes: &[SNode], out: &mut HashMap<String, SNode>) {
        for n in nodes {
            out.insert(n.id.clone(), n.clone());
            walk(&n.children, out);
        }
    }
    walk(children, &mut out);
    out
}
