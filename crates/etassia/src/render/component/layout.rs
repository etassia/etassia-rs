//! Places a [`Graph`]: each container lays out its own children in ranks,
//! outermost container last; links are cut at the box edges; ports sit on
//! their container's frame. Port of `node_graph/layout.rb` with its
//! helpers `detour.rb` and `frame_ports.rb`.

use std::collections::HashMap;

use super::detour::Detour;
use super::frame_ports::{self, FramePorts, REACH};
use super::label_fan::{label_fan, label_waypoint};
use super::metrics::{self, label_reach, lift};
use super::model::{Edge, Graph, NodeKind};
use super::paths::{Packer, Paths, Packed};
use super::ranking::{rank, Item};
use super::scene::{Placed, Route, Scene};

pub const MARGIN: f64 = 16.0;
pub const PAD: f64 = 16.0;
pub const TITLE_HEIGHT: f64 = 30.0;
pub const CLOUD_ROOM: (f64, f64) = (5.0, 0.0);
pub const UNTITLED_CLOUD_ROOM: (f64, f64) = (5.0, 2.15);

pub struct Layout<'a> {
    graph: &'a Graph,
    sizes: HashMap<usize, (f64, f64)>,
    frame_ports: FramePorts,
    ports: HashMap<Option<usize>, Vec<usize>>,
    inner: HashMap<usize, Packed>,
    inset: HashMap<usize, (f64, f64)>,
    paths: Paths<'a>,
}

impl<'a> Layout<'a> {
    pub fn new(graph: &'a Graph) -> Layout<'a> {
        let axis = if graph.direction == super::model::Direction::LeftToRight { 0 } else { 1 };
        Layout {
            graph,
            sizes: HashMap::new(),
            frame_ports: FramePorts::new(),
            ports: HashMap::new(),
            inner: HashMap::new(),
            inset: HashMap::new(),
            paths: Paths::new(axis, graph),
        }
    }

    pub fn run(mut self) -> Scene {
        let top = self.arrange(None);
        let mut placed = vec![];
        self.place(&top, (MARGIN, MARGIN + self.title_room()), 0, &mut placed);
        let routes = self.routes(&placed);
        Scene {
            width: (top.width + 2.0 * MARGIN).max(self.title_width()),
            height: top.height + 2.0 * MARGIN + self.title_room(),
            placed,
            routes,
            title: self.graph.title.clone(),
        }
    }

    fn left_to_right(&self) -> bool {
        self.graph.direction == super::model::Direction::LeftToRight
    }

    fn title_room(&self) -> f64 {
        if self.graph.title.is_some() { TITLE_HEIGHT } else { 0.0 }
    }

    fn title_width(&self) -> f64 {
        match &self.graph.title {
            Some(title) => metrics::face_width(title, 13.0) + 2.0 * MARGIN,
            None => 0.0,
        }
    }

    /// Lays out the children of `scope` and records their sizes.
    fn arrange(&mut self, scope: Option<usize>) -> Packed {
        let (ports, children): (Vec<usize>, Vec<usize>) = self
            .graph
            .children_of(scope)
            .into_iter()
            .partition(|node| frame_ports::on_frame(self.graph, *node));
        self.ports.insert(scope, ports.clone());
        for child in ports.iter().chain(children.iter()) {
            self.measure(*child);
        }
        let pairs = self.pairs(scope);
        let mut ranking = rank(
            &children.iter().map(|n| Item::Node(*n)).collect::<Vec<_>>(),
            &pairs,
        );
        let mut packed = {
            let packer = Packer::new(self.sizes.clone(), self.left_to_right(), label_reach(self.graph, scope));
            let packed = packer.pack(&ranking.layers);
            packer.route(packed, &ranking.chains)
        };
        ranking.links.clear();
        if scope.is_some() {
            let scope_index = scope.unwrap();
            let ports = self.ports.get(&Some(scope_index)).cloned().unwrap_or_default();
            packed.width = packed.width.max(self.frame_ports.width(self.graph, &ports, &self.sizes));
            packed.width += 2.0 * PAD;
            packed.height += self.top_inset(scope_index) + self.bottom_inset(scope_index);
        }
        packed
    }

    /// The children of `scope` on its frame, then the rest; link ends
    /// lifted into pairs.
    fn pairs(&self, scope: Option<usize>) -> Vec<(usize, usize, Option<usize>)> {
        self.graph
            .edges
            .iter()
            .filter_map(|edge| {
                let from = lift(self.graph, edge.from, scope);
                let to = lift(self.graph, edge.to, scope);
                if !self.inner(from, to) {
                    return None;
                }
                let (from, to) = if edge.backwards { (to, from) } else { (from, to) };
                Some((from?, to?, edge.rank_step))
            })
            .collect()
    }

    /// Whether a link is between two items that take part in ranking.
    fn inner(&self, from: Option<usize>, to: Option<usize>) -> bool {
        match (from, to) {
            (Some(from), Some(to)) => {
                from != to
                    && ![from, to]
                        .iter()
                        .any(|node| frame_ports::on_frame(self.graph, *node))
            }
            _ => false,
        }
    }

    fn measure(&mut self, node: usize) {
        if !self.graph.nodes[node].container {
            let size = metrics::size(&self.graph.nodes[node]);
            self.sizes.insert(node, size);
            return;
        }
        let inner = self.arrange(Some(node));
        let header = metrics::header_width(&self.graph.nodes[node]);
        let mut size = (inner.width.max(header), inner.height);
        if self.graph.nodes[node].kind == NodeKind::Cloud {
            let inset = self.cloud_room(node);
            self.inset.insert(node, inset);
            size = (size.0 + 2.0 * inset.0, size.1 + 2.0 * inset.1);
        }
        self.inner.insert(node, inner);
        self.sizes.insert(node, size);
    }

    fn cloud_room(&self, node: usize) -> (f64, f64) {
        if metrics::untitled(&self.graph.nodes[node]) {
            UNTITLED_CLOUD_ROOM
        } else {
            CLOUD_ROOM
        }
    }

    fn top_inset(&self, node: usize) -> f64 {
        if self.ports_on(node, NodeKind::Portin) {
            return REACH;
        }
        PAD + metrics::header_height(&self.graph.nodes[node])
    }

    fn bottom_inset(&self, node: usize) -> f64 {
        if self.ports_on(node, NodeKind::Portout) {
            REACH
        } else {
            PAD
        }
    }

    fn ports_on(&self, node: usize, kind: NodeKind) -> bool {
        self.ports
            .get(&Some(node))
            .map(|ports| ports.iter().any(|port| self.graph.nodes[*port].kind == kind))
            .unwrap_or(false)
    }

    fn place(&mut self, packed: &Packed, origin: (f64, f64), depth: usize, placed: &mut Vec<Placed>) {
        self.paths.record(&packed.paths, origin);
        for (item, offset) in &packed.offsets {
            let Item::Node(node) = item else { continue };
            let (width, height) = self.sizes[node];
            let box_placed = Placed {
                node: *node,
                x: origin.0 + offset.0,
                y: origin.1 + offset.1,
                width,
                height,
                depth,
            };
            let is_container = self.graph.nodes[*node].container;
            placed.push(box_placed);
            if is_container {
                self.place_inside(placed, depth);
            }
        }
    }

    fn place_inside(&mut self, placed: &mut Vec<Placed>, depth: usize) {
        let last = placed.len() - 1;
        let node = placed[last].node;
        let inner = self.inner[&node].clone();
        let origin = self.inside_origin(&placed[last]);
        self.place(&inner, origin, depth + 1, placed);
        let ports = self.ports.get(&Some(node)).cloned().unwrap_or_default();
        let frame_ports = self.frame_ports.place(self.graph, &placed[last], &ports, &self.sizes, depth + 1);
        placed.extend(frame_ports);
    }

    fn inside_origin(&self, boxed: &Placed) -> (f64, f64) {
        let room = self.inset.get(&boxed.node).copied().unwrap_or((0.0, 0.0));
        (
            boxed.x + PAD + room.0,
            boxed.y + room.1 + self.top_inset(boxed.node),
        )
    }

    fn routes(&self, placed: &[Placed]) -> Vec<Route> {
        let mut boxes: HashMap<usize, Placed> = placed
            .iter()
            .map(|box_placed| {
                (
                    box_placed.node,
                    if frame_ports::on_frame(self.graph, box_placed.node) {
                        self.frame_ports.terminal(self.graph, box_placed)
                    } else {
                        box_placed.clone()
                    },
                )
            })
            .collect();
        let detour = Detour::new(placed, self.graph);
        let mut routes = vec![];
        for edge in &self.graph.edges {
            let (Some(from), Some(to)) = (boxes.get(&edge.from), boxes.get(&edge.to)) else {
                continue;
            };
            if from.node == to.node {
                continue;
            }
            let between = self.paths.between(edge, center(from));
            let points = segment(from, to, edge, between);
            let points = if edge.pointer {
                points
            } else {
                detour.bend(edge, points)
            };
            routes.push(Route { edge: edge.clone(), points, label_at: None });
        }
        label_fan(&mut routes, self.axis());
        routes
    }

    fn axis(&self) -> usize {
        if self.left_to_right() { 0 } else { 1 }
    }
}

fn center(boxed: &Placed) -> (f64, f64) {
    (boxed.x + boxed.width / 2.0, boxed.y + boxed.height / 2.0)
}

/// `between` is where a link across ranks passes the ranks it skips.
fn segment(from: &Placed, to: &Placed, edge: &Edge, between: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let a = center(from);
    let b = center(to);
    let toward_end = *between.last().unwrap_or(&a);
    let mut points = vec![end_at(from, a, *between.first().unwrap_or(&b))];
    points.extend(between);
    points.push(end_at(to, b, toward_end));
    let _ = edge;
    points
}

fn end_at(boxed: &Placed, inner: (f64, f64), toward: (f64, f64)) -> (f64, f64) {
    clip(boxed, inner, toward)
}

/// The point where the line from `inner` toward `outer` leaves `box`.
fn clip(boxed: &Placed, inner: (f64, f64), outer: (f64, f64)) -> (f64, f64) {
    let delta = (outer.0 - inner.0, outer.1 - inner.1);
    if delta.0 == 0.0 && delta.1 == 0.0 {
        return inner;
    }
    let scale = exit_scale(boxed, delta);
    (inner.0 + delta.0 * scale, inner.1 + delta.1 * scale)
}

fn exit_scale(boxed: &Placed, delta: (f64, f64)) -> f64 {
    let x = boxed.width / 2.0 / delta.0.abs().max(1e-9);
    let y = boxed.height / 2.0 / delta.1.abs().max(1e-9);
    x.min(y)
}

pub fn label_waypoint_port(points: &[(f64, f64)], axis: usize) -> Option<(f64, f64)> {
    label_waypoint(points, axis)
}
