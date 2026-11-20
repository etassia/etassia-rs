//! The size of each shape, from the text it carries; and the ellipse of a
//! use case. Port of `node_graph/{metrics,usecase_ellipse}.rb`, with
//! `Drawing::Face.width` measured through the shared text module.

use std::collections::HashMap;

use super::model::{Edge, Graph, Node, NodeKind};
use super::ranking::Item;
use crate::text::{self, Face};

pub const FONT_SIZE: f64 = 14.0;
pub const LINE_HEIGHT: f64 = 16.4883;
pub const ASCENT: f64 = 13.5352;
pub const HEADER_PAD: f64 = 10.0;

/// `Drawing::Face.width`: DejaVu Sans advances.
pub fn face_width(text: &str, size: f64) -> f64 {
    text::measure(text, size, Face::DejaVu).width
}

/// The stereotype line, if any, then the label lines (empties dropped).
pub fn text_lines(node: &Node) -> Vec<String> {
    let lines: Vec<String> = node.lines.iter().filter(|l| !l.is_empty()).cloned().collect();
    match &node.stereotype {
        Some(stereotype) => {
            let mut all = vec![format!("«{stereotype}»")];
            all.extend(lines);
            all
        }
        None => lines,
    }
}

pub fn text_width(lines: &[String]) -> f64 {
    lines
        .iter()
        .map(|line| face_width(line, FONT_SIZE))
        .fold(f64::NEG_INFINITY, f64::max)
        .max(0.0)
}

pub fn text_height(lines: &[String]) -> f64 {
    lines.len().max(1) as f64 * LINE_HEIGHT
}

/// Width and height of a leaf node. Entity/data sizes are out of scope:
/// those kinds are never claimed from component sources.
pub fn size(node: &Node) -> (f64, f64) {
    match node.kind {
        NodeKind::Usecase => usecase_size(node),
        _ => text_size(node),
    }
}

fn text_size(node: &Node) -> (f64, f64) {
    let lines = text_lines(node);
    let width = text_width(&lines);
    let height = text_height(&lines) + sprite_room(node);
    shape_size(shape(node.kind), width, height)
}

fn usecase_size(node: &Node) -> (f64, f64) {
    let widths: Vec<f64> = text_lines(node)
        .iter()
        .map(|line| text_width(std::slice::from_ref(line)))
        .collect();
    usecase_ellipse_size(&widths, LINE_HEIGHT)
}

fn sprite_room(node: &Node) -> f64 {
    if node.sprite.is_some() {
        super::archimate::SPRITE_ROOM
    } else {
        0.0
    }
}

fn shape(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Actor => "actor",
        NodeKind::Usecase => "usecase",
        NodeKind::Component => "component",
        NodeKind::Interface => "interface",
        NodeKind::Portin => "portin",
        NodeKind::Portout => "portout",
        NodeKind::Note => "note",
        NodeKind::Database => "database",
        NodeKind::Cloud => "cloud",
        NodeKind::Node => "node",
        NodeKind::Circle => "circle",
        _ => "box",
    }
}

fn shape_size(shape: &str, width: f64, height: f64) -> (f64, f64) {
    match shape {
        "actor" => (width.max(26.0), 59.5 + height),
        "component" => (width + 40.0, height + 30.0),
        "interface" => (width.max(20.0), 28.0 + height),
        "portin" => (width.max(14.0), 24.0 + height),
        "portout" => (width.max(14.0), 12.0 + height),
        "circle" => (width.max(16.0), 24.0 + height),
        "note" => (width + 24.0, height + 16.0),
        "database" => (width + 30.0, height + 40.0),
        "cloud" => (width + 34.0, height + 31.0),
        "node" => (width + 38.0, height + 28.0),
        _ => (width + 26.0, height + 20.0),
    }
}

/// The room a container's label takes at its top.
pub fn header_height(node: &Node) -> f64 {
    let lines: Vec<String> = text_lines(node).into_iter().filter(|l| !l.is_empty()).collect();
    if !lines.is_empty() {
        return text_height(&lines) + HEADER_PAD;
    }
    if node.kind == NodeKind::Cloud { 0.0 } else { HEADER_PAD }
}

pub fn untitled(node: &Node) -> bool {
    text_lines(node).is_empty()
}

pub fn header_width(node: &Node) -> f64 {
    text_width(&text_lines(node)) + 28.0
}

/// The widths of the widest label on a link of each item of `scope`.
pub fn label_reach(graph: &Graph, scope: Option<usize>) -> HashMap<Item, f64> {
    let mut reach: HashMap<Item, f64> = HashMap::new();
    for edge in &graph.edges {
        let width = label_width(edge);
        for node in [edge.from, edge.to] {
            if let Some(item) = lift(graph, node, scope) {
                let entry = reach.entry(Item::Node(item)).or_insert(0.0);
                *entry = entry.max(width);
            }
        }
    }
    reach
}

fn label_width(edge: &Edge) -> f64 {
    let Some(text) = &edge.label else { return 0.0 };
    if text.is_empty() {
        return 0.0;
    }
    face_width(text, 12.0)
}

/// The child of `scope` the node counts as, or the outermost ancestor.
pub fn lift(graph: &Graph, node: usize, scope: Option<usize>) -> Option<usize> {
    let chain = Node::chain(graph, node);
    match scope {
        None => chain.first().copied(),
        Some(scope) => chain
            .iter()
            .position(|entry| *entry == scope)
            .and_then(|at| chain.get(at + 1).copied()),
    }
}

/// The ellipse of a use case: the smallest fixed-proportion ellipse
/// holding every line's box, grown all round.
pub fn usecase_ellipse_size(widths: &[f64], line_height: f64) -> (f64, f64) {
    let widest = widths.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let proportion = proportion(widest, widths.len() as f64 * line_height);
    let points = corners(widths, line_height, proportion);
    let radius = enclosing_radius(&points);
    (
        2.0 * (radius + 3.0),
        2.0 * ((proportion * radius) + 3.0),
    )
}

fn proportion(width: f64, height: f64) -> f64 {
    let raw = height / width;
    raw.clamp(0.2, 0.8)
}

fn corners(widths: &[f64], line_height: f64, proportion: f64) -> Vec<(f64, f64)> {
    let mut points = vec![];
    for (index, width) in widths.iter().enumerate() {
        for row in [index, index + 1] {
            for x in [-width / 2.0, width / 2.0] {
                points.push((x, row as f64 * line_height / proportion));
            }
        }
    }
    points
}

/// The radius of the smallest circle holding every point (Welzl-style
/// incremental algorithm, exactly Ruby's).
fn enclosing_radius(points: &[(f64, f64)]) -> f64 {
    let mut circle: Option<((f64, f64), f64)> = None;
    for (index, point) in points.iter().enumerate() {
        if let Some(current) = circle {
            if holds(current, *point) {
                continue;
            }
        }
        circle = Some(around(&points[..index], *point));
    }
    circle.map(|(_, radius)| radius).unwrap_or(0.0)
}

fn around(earlier: &[(f64, f64)], point: (f64, f64)) -> ((f64, f64), f64) {
    let mut circle = (point, 0.0);
    for (index, other) in earlier.iter().enumerate() {
        if holds(circle, *other) {
            continue;
        }
        circle = across(&earlier[..index], point, *other);
    }
    circle
}

fn across(earlier: &[(f64, f64)], point: (f64, f64), other: (f64, f64)) -> ((f64, f64), f64) {
    let mut circle = diameter(point, other);
    for third in earlier {
        if !holds(circle, *third) {
            circle = through(point, other, *third);
        }
    }
    circle
}

fn holds(circle: ((f64, f64), f64), point: (f64, f64)) -> bool {
    distance(point, circle.0) <= circle.1 + 1e-9
}

fn diameter(one: (f64, f64), other: (f64, f64)) -> ((f64, f64), f64) {
    let center = ((one.0 + other.0) / 2.0, (one.1 + other.1) / 2.0);
    (center, distance(one, center))
}

fn distance(one: (f64, f64), other: (f64, f64)) -> f64 {
    ((one.0 - other.0).hypot(one.1 - other.1))
}

/// The circle through three points, found about the first.
fn through(one: (f64, f64), two: (f64, f64), three: (f64, f64)) -> ((f64, f64), f64) {
    let (u_re, u_im) = centre_from(two.0 - one.0, two.1 - one.1, three.0 - one.0, three.1 - one.1);
    ((one.0 + u_re, one.1 + u_im), u_re.hypot(u_im))
}

/// Ruby `centre_from`: `Complex(0, -1) * ((second * first.abs2) -
/// (first * second.abs2)) / d` with `d = 2.0 * (first.conj * second).imag`.
fn centre_from(a_re: f64, a_im: f64, b_re: f64, b_im: f64) -> (f64, f64) {
    // d = 2 * imag(conj(first) * second) = 2 * (a.re*b.im - a.im*b.re)
    let d = 2.0 * (a_re * b_im - a_im * b_re);
    // second * first.abs2 - first * second.abs2
    let a2 = a_re * a_re + a_im * a_im;
    let b2 = b_re * b_re + b_im * b_im;
    let x_re = b_re * a2 - a_re * b2;
    let x_im = b_im * a2 - a_im * b2;
    // Complex(0, -1) * x = (x.im, -x.re)
    let y_re = x_im;
    let y_im = -x_re;
    // ... / d
    (y_re / d, y_im / d)
}
