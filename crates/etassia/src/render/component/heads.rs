//! The ends of a link: arrowheads and the circle of a socket. Port of
//! `node_graph/heads.rb` (the curve and note tail live beside the scene
//! painter).

use super::model::{Edge, Head};
use crate::svg::Elem;

pub const LENGTH: f64 = 9.0;
pub const HALF: f64 = 4.0;
pub const SOCKET: f64 = 5.0;

/// What the edge draws at its ends. `inner` are the points next to `from`
/// and next to `to` on the line.
pub fn heads(edge: &Edge, from: (f64, f64), to: (f64, f64), color: &str, inner: [(f64, f64); 2]) -> Vec<Elem> {
    match edge.head {
        Some(Head::Target) => vec![arrow(to, inner[1], color)],
        Some(Head::Source) => vec![arrow(from, inner[0], color)],
        Some(Head::Both) => vec![arrow(to, inner[1], color), arrow(from, inner[0], color)],
        Some(Head::Socket) => vec![socket(to, inner[1])],
        None => vec![],
    }
}

/// Where the head points, plus the other end of the line.
fn arrow(tip: (f64, f64), tail: (f64, f64), color: &str) -> Elem {
    let vec = unit(tip, tail);
    let base = along(tip, vec, LENGTH);
    let wings = [
        along(base, (-vec.1, vec.0), HALF),
        along(base, (vec.1, -vec.0), HALF),
    ];
    polygon(&[tip, wings[0], wings[1]], color)
}

fn polygon(corners: &[(f64, f64)], color: &str) -> Elem {
    let points = corners
        .iter()
        .map(|point| format!("{},{}", rounded(point.0), rounded(point.1)))
        .collect::<Vec<_>>()
        .join(" ");
    Elem::new("polygon")
        .attr("points", points)
        .attr("fill", color)
        .attr("stroke", color)
        .attr("stroke-width", "1")
}

fn socket(tip: (f64, f64), tail: (f64, f64)) -> Elem {
    let (cx, cy) = along(tip, unit(tip, tail), SOCKET);
    Elem::new("circle")
        .num("cx", cx)
        .num("cy", cy)
        .num("r", SOCKET)
        .attr("fill", "#FFFFFF")
        .attr("stroke", "#181818")
        .attr("stroke-width", "1")
}

fn along(from: (f64, f64), vec: (f64, f64), distance: f64) -> (f64, f64) {
    (from.0 + vec.0 * distance, from.1 + vec.1 * distance)
}

/// The unit vector from `from` to `to`.
pub fn unit(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let delta = (to.0 - from.0, to.1 - from.1);
    let length = delta.0.hypot(delta.1);
    if length == 0.0 {
        (0.0, 1.0)
    } else {
        (delta.0 / length, delta.1 / length)
    }
}

/// Ruby `Float#round(2).to_s` inside a path string.
fn rounded(value: f64) -> String {
    crate::svg::ruby_float(crate::render::ditaa::ruby_round(value, 2))
}

pub fn note_pointer(edge: &Edge) -> bool {
    edge.pointer
}
