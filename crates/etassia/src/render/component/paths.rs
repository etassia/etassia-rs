//! Stacks the items of each rank along the cross axis and the ranks along
//! the main axis; and the points links across ranks pass through. Port of
//! `node_graph/{packer,paths}.rb`.

use std::collections::HashMap;

use super::model::{Edge, Graph, Node};
use super::ranking::Item;

pub const MAIN_GAP: f64 = 70.0;
pub const CROSS_GAP: f64 = 28.0;
pub const WAYPOINT_GAP: f64 = 17.0;
pub const REACH_SHARE: f64 = 1.3;

/// What one scope's layout came to: where each item sits inside the scope
/// and how big the scope is.
#[derive(Clone)]
pub struct Packed {
    pub offsets: Vec<(Item, (f64, f64))>,
    pub width: f64,
    pub height: f64,
    /// For a link across ranks, the points it passes through in the ranks
    /// it skips: keyed by the node pair, in chain order.
    pub paths: Vec<((usize, usize), Vec<(f64, f64)>)>,
}

pub struct Packer {
    /// width/height of each node of the scope.
    sizes: HashMap<usize, (f64, f64)>,
    left_to_right: bool,
    /// The widest label on a link of each item.
    reach: HashMap<Item, f64>,
    main: usize,
    cross: usize,
}

impl Packer {
    pub fn new(
        sizes: HashMap<usize, (f64, f64)>,
        left_to_right: bool,
        reach: HashMap<Item, f64>,
    ) -> Packer {
        let main = if left_to_right { 0 } else { 1 };
        let cross = 1 - main;
        let reach = if left_to_right { HashMap::new() } else { reach };
        Packer { sizes, left_to_right, reach, main, cross }
    }

    pub fn pack(&self, layers: &[Vec<Item>]) -> Packed {
        let extents: Vec<f64> = layers.iter().map(|layer| self.cross_extent(layer)).collect();
        let widest = extents.iter().cloned().fold(0.0, f64::max);
        let mut offsets: Vec<(Item, (f64, f64))> = vec![];
        let margins: Vec<f64> = extents.iter().map(|extent| (widest - extent) / 2.0).collect();
        let main = self.lay_out(layers, &margins, &mut offsets);
        let mut size = [0.0, 0.0];
        size[self.main] = main;
        size[self.cross] = widest;
        Packed {
            offsets,
            width: size[0],
            height: size[1],
            paths: vec![],
        }
    }

    /// `packed` with the point of each chain, and without the waypoints
    /// among its offsets.
    pub fn route(&self, mut packed: Packed, chains: &[((usize, usize), Vec<Item>)]) -> Packed {
        packed.paths = chains
            .iter()
            .map(|(ends, waypoints)| {
                let points = waypoints
                    .iter()
                    .map(|waypoint| {
                        packed
                            .offsets
                            .iter()
                            .find(|(item, _)| item == waypoint)
                            .map(|(_, point)| *point)
                            .unwrap_or((0.0, 0.0))
                    })
                    .collect();
                (*ends, points)
            })
            .collect();
        packed.offsets.retain(|(item, _)| matches!(item, Item::Node(_)));
        packed
    }

    fn lay_out(&self, layers: &[Vec<Item>], margins: &[f64], offsets: &mut Vec<(Item, (f64, f64))>) -> f64 {
        let mut at = 0.0;
        for (index, layer) in layers.iter().enumerate() {
            self.stack(layer, at, margins[index], offsets);
            at += self.main_extent(layer) + MAIN_GAP;
        }
        if layers.is_empty() { 0.0 } else { at - MAIN_GAP }
    }

    fn main_extent(&self, layer: &[Item]) -> f64 {
        layer
            .iter()
            .map(|item| axis(self.size(item), self.main))
            .fold(0.0, f64::max)
    }

    fn cross_extent(&self, layer: &[Item]) -> f64 {
        if layer.is_empty() {
            return 0.0;
        }
        let sum: f64 = layer.iter().map(|item| axis(self.size(item), self.cross)).sum();
        sum + self.gaps(layer).iter().sum::<f64>()
    }

    /// Waypoints have no size.
    fn size(&self, item: &Item) -> (f64, f64) {
        match item {
            Item::Waypoint(_) => (0.0, 0.0),
            Item::Node(index) => self.sizes[index],
        }
    }

    fn gaps(&self, layer: &[Item]) -> Vec<f64> {
        layer.windows(2).map(|pair| self.gap(&pair[0], &pair[1])).collect()
    }

    fn gap(&self, before: &Item, after: &Item) -> f64 {
        if matches!(before, Item::Waypoint(_)) || matches!(after, Item::Waypoint(_)) {
            return WAYPOINT_GAP;
        }
        // Centres of two neighbours sit half of both labels' reach apart.
        let reach = (self.reach.get(before).copied().unwrap_or(0.0)
            + self.reach.get(after).copied().unwrap_or(0.0))
            / 2.0;
        let half = (axis(self.size(before), self.cross) + axis(self.size(after), self.cross)) / 2.0;
        CROSS_GAP.max(REACH_SHARE * reach - half)
    }

    /// A waypoint's offset is the point itself, in the middle of the rank.
    fn stack(&self, layer: &[Item], main: f64, margin: f64, offsets: &mut Vec<(Item, (f64, f64))>) {
        let middle = main + self.main_extent(layer) / 2.0;
        let gaps = self.gaps(layer);
        let mut cross = margin;
        for (index, item) in layer.iter().enumerate() {
            if index > 0 {
                cross += gaps[index - 1];
            }
            let anchor = match item {
                Item::Waypoint(_) => middle,
                Item::Node(_) => main,
            };
            offsets.push((*item, self.offset(anchor, cross)));
            cross += axis(self.size(item), self.cross);
        }
    }

    fn offset(&self, main: f64, cross: f64) -> (f64, f64) {
        let mut point = [0.0, 0.0];
        point[self.main] = main;
        point[self.cross] = cross;
        (point[0], point[1])
    }
}

/// The points the links across ranks pass through, in final coordinates.
pub struct Paths<'a> {
    axis: usize,
    graph: &'a Graph,
    through: Vec<((usize, usize), Vec<(f64, f64)>)>,
}

fn axis(size: (f64, f64), which: usize) -> f64 {
    if which == 0 { size.0 } else { size.1 }
}

impl<'a> Paths<'a> {
    pub fn new(axis: usize, graph: &'a Graph) -> Paths<'a> {
        Paths { axis, graph, through: vec![] }
    }

    /// Notes the points of a scope laid out at `origin`.
    pub fn record(&mut self, paths: &[((usize, usize), Vec<(f64, f64)>)], origin: (f64, f64)) {
        for (ends, points) in paths {
            let moved = points
                .iter()
                .map(|point| (point.0 + origin.0, point.1 + origin.1))
                .collect();
            match self.through.iter_mut().find(|(known, _)| known == ends) {
                Some((_, existing)) => *existing = moved,
                None => self.through.push((*ends, moved)),
            }
        }
    }

    /// Its points, nearest `start` first (Ruby sorts by |axis - start|).
    pub fn between(&self, edge: &Edge, start: (f64, f64)) -> Vec<(f64, f64)> {
        let points = self
            .through
            .iter()
            .find(|(ends, _)| self.spans(ends, edge))
            .map(|(_, points)| points.clone())
            .unwrap_or_default();
        let mut points = points;
        points.sort_by(|a, b| {
            let da = (axis(*a, self.axis) - axis(start, self.axis)).abs();
            let db = (axis(*b, self.axis) - axis(start, self.axis)).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        points
    }

    /// Whether the link's ends span this recorded chain: either direction,
    /// through any ancestors.
    fn spans(&self, ends: &(usize, usize), edge: &Edge) -> bool {
        let pairs = [(edge.from, edge.to), (edge.to, edge.from)];
        pairs.iter().any(|(node, scope)| {
            node == scope || Node::chain(self.graph, *node).contains(scope)
        })
    }
}
