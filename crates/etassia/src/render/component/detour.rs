//! Bends a link around a box that its straight line would cross, as the
//! PlantUML jar routes a spline past what is in the way. Port of
//! `node_graph/detour.rb`.

use super::model::{Graph, NodeKind};
use super::scene::Placed;

pub const CLEARANCE: f64 = 14.0;
pub const MAX_BENDS: usize = 3;
pub const STEP: f64 = 4.0;
pub const EDGE_MARGIN: f64 = 4.0;

pub struct Detour<'a> {
    placed: Vec<Placed>,
    graph: &'a Graph,
}

impl<'a> Detour<'a> {
    /// Every placed node except the ports.
    pub fn new(placed: &[Placed], graph: &'a Graph) -> Detour<'a> {
        Detour {
            placed: placed
                .iter()
                .filter(|boxed| {
                    !matches!(graph.nodes[boxed.node].kind, NodeKind::Portin | NodeKind::Portout)
                })
                .cloned()
                .collect(),
            graph,
        }
    }

    /// `points` with the bends needed to clear the boxes between its ends.
    pub fn bend(&self, edge: &super::model::Edge, points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
        let mut route = points.clone();
        let boxes = self.obstacles(edge);
        for _ in 0..MAX_BENDS {
            let Some((index, boxed)) = blocked(&route, &boxes) else { break };
            let bend = around(&route[index], &route[index + 1], &boxed);
            route.insert(index + 1, bend);
        }
        route
    }

    /// The boxes the link has no business with: not its ends, what holds
    /// them, or what they hold.
    fn obstacles(&self, edge: &super::model::Edge) -> Vec<Placed> {
        let from_chain = super::model::Node::chain(self.graph, edge.from);
        let to_chain = super::model::Node::chain(self.graph, edge.to);
        self.placed
            .iter()
            .filter(|boxed| {
                let box_chain = super::model::Node::chain(self.graph, boxed.node);
                for node in [edge.from, edge.to] {
                    let node_chain = super::model::Node::chain(self.graph, node);
                    if node_chain.contains(&boxed.node) || box_chain.contains(&node) {
                        return false;
                    }
                }
                let _ = (&from_chain, &to_chain);
                true
            })
            .cloned()
            .collect()
    }
}

fn blocked<'a>(route: &[(f64, f64)], boxes: &'a [Placed]) -> Option<(usize, &'a Placed)> {
    for (index, pair) in route.windows(2).enumerate() {
        for boxed in boxes {
            if crosses(pair[0], pair[1], boxed) {
                return Some((index, boxed));
            }
        }
    }
    None
}

fn crosses(from: (f64, f64), to: (f64, f64), boxed: &Placed) -> bool {
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    let steps = (length / STEP).ceil() as i64;
    (1..steps).any(|step| {
        let ratio = step as f64 / steps as f64;
        let point = lerp(from, to, ratio);
        inside(boxed, point)
    })
}

fn lerp(from: (f64, f64), to: (f64, f64), ratio: f64) -> (f64, f64) {
    (
        from.0 + (to.0 - from.0) * ratio,
        from.1 + (to.1 - from.1) * ratio,
    )
}

fn inside(boxed: &Placed, point: (f64, f64)) -> bool {
    point.0 >= boxed.x
        && point.0 <= boxed.x + boxed.width
        && point.1 >= boxed.y
        && point.1 <= boxed.y + boxed.height
}

/// A point beside the box, on the side the line passes nearer.
fn around(from: &(f64, f64), to: &(f64, f64), boxed: &Placed) -> (f64, f64) {
    let middle = boxed.y + boxed.height / 2.0;
    let crossing_x = crossing_x(from, to, middle);
    (nearer_side(boxed, crossing_x), middle)
}

fn crossing_x(from: &(f64, f64), to: &(f64, f64), y: f64) -> f64 {
    let rise = if to.1 - from.1 == 0.0 { 1e-9 } else { to.1 - from.1 };
    lerp(*from, *to, (y - from.1) / rise).0
}

fn nearer_side(boxed: &Placed, x: f64) -> f64 {
    let left = (boxed.x - CLEARANCE).max(EDGE_MARGIN);
    let right = boxed.x + boxed.width + CLEARANCE;
    if (left - x).abs() <= (right - x).abs() {
        left
    } else {
        right
    }
}
