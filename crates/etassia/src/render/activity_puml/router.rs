//! Port of `activity/{router,route}.rb`: every edge as straight and
//! right-angled segments between the placed nodes. Loops go round the
//! left, forward edges that skip rows round the right.

use std::collections::HashMap;

use super::layout::{top_entry, Ranking, PARTITION_BELOW};
use super::metrics::Box;
use super::model::{Diagram, Kind};

/// The drawn path of one edge and where its label goes (`Route`).
pub struct Route {
    pub points: Vec<(f64, f64)>,
    pub label_x: f64,
    pub label_y: f64,
    pub anchor: &'static str,
}

const DETOUR: f64 = 16.0;
const DETOUR_STEP: f64 = 7.0;
const JOG: f64 = 11.97;
const RUN_LIFT: f64 = 2.32;

/// Draws every edge between the placed nodes (`Router`).
pub struct Router<'a> {
    label_drop: f64,
    boxes: &'a HashMap<usize, Box>,
    ranking: &'a Ranking,
    bottoms: &'a [f64],
    gaps: &'a [f64],
    detours: HashMap<bool, usize>,
    diagram: &'a Diagram,
}

impl<'a> Router<'a> {
    pub fn new(
        diagram: &'a Diagram,
        boxes: &'a HashMap<usize, Box>,
        ranking: &'a Ranking,
        bottoms: &'a [f64],
        gaps: &'a [f64],
        label_drop: f64,
    ) -> Self {
        Router {
            label_drop,
            boxes,
            ranking,
            bottoms,
            gaps,
            detours: Default::default(),
            diagram,
        }
    }

    /// The `detours` counters move per call, so `call` takes `&mut self`.
    pub fn call(&mut self, edge: usize) -> Route {
        let (from, to) = (self.box_of(self.from(edge)), self.box_of(self.to(edge)));
        if self.is_level(edge) {
            return self.level(edge, &from, &to);
        }
        if let Some(side) = self.detour_side(edge, &from, &to) {
            return self.detour(edge, &from, &to, side);
        }
        self.forward(edge, &from, &to)
    }

    fn from(&self, edge: usize) -> usize {
        self.diagram.edges[edge].from
    }

    fn to(&self, edge: usize) -> usize {
        self.diagram.edges[edge].to
    }

    fn box_of(&self, node: usize) -> Box {
        self.boxes[&node].clone()
    }

    fn is_level(&self, edge: usize) -> bool {
        self.diagram.edges[edge].level()
            && !self.ranking.back(edge)
            && self.ranking.rank[&self.from(edge)] == self.ranking.rank[&self.to(edge)]
    }

    /// Along the row: out of the side of `from` that faces `to`. A parked
    /// `to` sits half a row lower, so the arrow runs on to above its
    /// middle and drops into it.
    fn level(&self, edge: usize, from: &Box, to: &Box) -> Route {
        let right = to.cx >= from.cx;
        let ends = if right {
            [from.right(), to.left()]
        } else {
            [from.left(), to.right()]
        };
        if (from.cy - to.cy).abs() > 1.0 {
            // The parked label sits on the run [start_x, to.cx].
            let (label_x, label_y, anchor) = self.level_label(edge, from, [ends[0], to.cx]);
            return Route {
                points: vec![(ends[0], from.cy), (to.cx, from.cy), (to.cx, to.top())],
                label_x,
                label_y,
                anchor,
            };
        }
        let (label_x, label_y, anchor) = self.level_label(edge, from, ends);
        Route {
            points: vec![(ends[0], from.cy), (ends[1], from.cy)],
            label_x,
            label_y,
            anchor,
        }
    }

    /// The label sits at the start of the run, unless a sibling exit
    /// leaves the same side of the decision and prints its own there: then
    /// it sits at the far end.
    fn level_label(&self, edge: usize, from: &Box, ends: [f64; 2]) -> (f64, f64, &'static str) {
        let low = ends[0].min(ends[1]);
        let high = ends[0].max(ends[1]);
        if self.shared_side(edge, from, ends[1]) {
            return (high - 4.0, from.cy - 4.0, "end");
        }
        (low + 5.0, from.cy - 4.0, "start")
    }

    fn shared_side(&self, edge: usize, from: &Box, end_x: f64) -> bool {
        if self.diagram.nodes[self.from(edge)].kind != Kind::Decision {
            return false;
        }
        self.ranking
            .forward(self.from(edge))
            .iter()
            .any(|other| self.sibling_beside(*other, edge, from, end_x))
    }

    fn sibling_beside(&self, other: usize, edge: usize, from: &Box, end_x: f64) -> bool {
        if other == edge || self.diagram.edges[other].level() {
            return false;
        }
        let offset = self.boxes[&self.diagram.edges[other].to].cx - from.cx;
        offset.abs() >= from.width / 2.0
            && (offset.partial_cmp(&0.0)
                == Some(
                    (end_x - from.cx)
                        .partial_cmp(&0.0)
                        .unwrap_or(std::cmp::Ordering::Equal),
                ))
    }

    fn detour_side(&mut self, edge: usize, from: &Box, to: &Box) -> Option<bool> {
        if self.ranking.back(edge) {
            return Some(true);
        }
        if !self.skips(edge) {
            return None;
        }
        if self.same_column(from, to) {
            // A drop down one column goes round the right only when a node
            // stands in it; a clear column is drawn straight.
            return if self.blockers(edge, from, to).is_empty() {
                None
            } else {
                Some(false)
            };
        }
        self.blocked_side(edge, from, to)
    }

    fn skips(&self, edge: usize) -> bool {
        self.ranking.rank[&self.to(edge)] - self.ranking.rank[&self.from(edge)] > 1
    }

    /// Round the left when everything in the way lies to the right. The
    /// side comes back as `true` for the left.
    fn blocked_side(&self, edge: usize, from: &Box, to: &Box) -> Option<bool> {
        let boxes = self.blockers(edge, from, to);
        if boxes.is_empty() {
            return None;
        }
        Some(boxes.iter().all(|node| self.boxes[node].cx > from.cx))
    }

    /// Boxes on the rows between that stand where the edge drops.
    fn blockers(&self, edge: usize, from: &Box, to: &Box) -> Vec<usize> {
        let rows = (self.ranking.rank[&self.from(edge)] + 1)..self.ranking.rank[&self.to(edge)];
        let drops = [from.cx, to.cx];
        let mut blockers: Vec<usize> = self
            .boxes
            .keys()
            .copied()
            .filter(|node| {
                self.diagram.nodes[*node].kind != Kind::Bar
                    && rows.contains(&self.ranking.rank[node])
                    && drops.iter().any(|x_pos| {
                        let node_box = &self.boxes[node];
                        *x_pos >= node_box.left() && *x_pos <= node_box.right()
                    })
            })
            .collect();
        // Ruby's `filter_map` over the hash keeps insertion order; the
        // placement fills the map in node order.
        blockers.sort_unstable();
        blockers
    }

    fn forward(&mut self, edge: usize, from: &Box, to: &Box) -> Route {
        let start_x = self.anchor_x(edge, self.from(edge), from, self.to(edge), to);
        let end_x = self.anchor_x(edge, self.to(edge), to, self.from(edge), from);
        match self.shape(edge, from, to) {
            Shape::SideExit => self.decision_exit(edge, from, to, end_x),
            Shape::SideEntry => {
                let route = self.side_entry(from, to, start_x);
                self.below_frame(edge, route)
            }
            Shape::Straight => {
                let route = self.straight(from, to, start_x);
                self.below_frame(edge, route)
            }
            Shape::Elbow => {
                let route = self.elbow(edge, from, to, start_x, end_x);
                self.below_frame(edge, route)
            }
        }
    }

    /// An edge that leaves a partition from the node's bottom crosses the
    /// frame's bottom edge; its label hangs below that edge.
    fn below_frame(&self, edge: usize, route: Route) -> Route {
        let Some(leaving) = &self.diagram.nodes[self.from(edge)].partition else {
            return route;
        };
        if Some(leaving) == self.diagram.nodes[self.to(edge)].partition.as_ref() {
            return route;
        }
        Route {
            points: route.points,
            label_x: route.label_x,
            label_y: route.label_y + PARTITION_BELOW,
            anchor: route.anchor,
        }
    }

    fn shape(&self, edge: usize, from: &Box, to: &Box) -> Shape {
        if self.same_column(from, to) {
            return Shape::Straight;
        }
        if self.diagram.nodes[self.from(edge)].kind == Kind::Decision {
            return Shape::SideExit;
        }
        if self.side_entry_shape(edge, from, to) {
            Shape::SideEntry
        } else {
            Shape::Elbow
        }
    }

    fn side_entry_shape(&self, edge: usize, from: &Box, to: &Box) -> bool {
        self.diagram.nodes[self.to(edge)].kind == Kind::Merge
            && !top_entry(from, to, self.diagram, edge)
    }

    fn straight(&self, from: &Box, to: &Box, x_pos: f64) -> Route {
        Route {
            points: vec![(x_pos, from.bottom()), (x_pos, to.top())],
            label_x: x_pos + 5.0,
            label_y: from.bottom() + self.label_drop,
            anchor: "start",
        }
    }

    fn elbow(&self, edge: usize, from: &Box, to: &Box, start_x: f64, end_x: f64) -> Route {
        if start_x == end_x {
            return self.straight(from, to, start_x);
        }
        let mid = self.run_height(edge);
        Route {
            points: vec![
                (start_x, from.bottom()),
                (start_x, mid),
                (end_x, mid),
                (end_x, to.top()),
            ],
            label_x: start_x + 5.0,
            label_y: from.bottom() + self.label_drop,
            anchor: "start",
        }
    }

    /// Halfway down the space below the edge's row.
    fn run_height(&self, edge: usize) -> f64 {
        let row = self.ranking.rank[&self.from(edge)] as usize;
        self.bottoms[row] + self.gaps.get(row + 1).copied().unwrap_or(0.0) / 2.0
    }

    /// A branch that lands under the decision's own tip needs no side run:
    /// it drops straight, as the first branch of a switch does.
    fn decision_exit(&self, edge: usize, from: &Box, to: &Box, end_x: f64) -> Route {
        if (end_x - from.cx).abs() < 1.0 {
            return self.straight(from, to, end_x);
        }
        if (end_x - from.cx).abs() < from.width / 2.0 {
            return self.jog_exit(from, to, end_x);
        }
        self.side_exit(edge, from, to, end_x)
    }

    /// Down from the decision's foot, across, and down into the step.
    fn jog_exit(&self, from: &Box, to: &Box, end_x: f64) -> Route {
        let level = from.bottom() + JOG;
        Route {
            points: vec![
                (from.cx, from.bottom()),
                (from.cx, level),
                (end_x, level),
                (end_x, to.top()),
            ],
            label_x: end_x + 5.0,
            label_y: level + self.label_drop,
            anchor: "start",
        }
    }

    fn side_exit(&self, edge: usize, from: &Box, to: &Box, end_x: f64) -> Route {
        let sign = if end_x > from.cx { 1.0 } else { -1.0 };
        let edge_x = if sign > 0.0 {
            from.right()
        } else {
            from.left()
        };
        let (label_x, label_y, anchor) = self.exit_label(edge, [edge_x, end_x], from, sign);
        Route {
            points: vec![(edge_x, from.cy), (end_x, from.cy), (end_x, to.top())],
            label_x,
            label_y,
            anchor,
        }
    }

    /// Two branches carry their label along the run, by the decision's
    /// tip. Where there are more, the label rides the branch's own drop,
    /// so branches that share one run do not print their labels on top of
    /// each other.
    fn exit_label(
        &self,
        edge: usize,
        ends: [f64; 2],
        from: &Box,
        sign: f64,
    ) -> (f64, f64, &'static str) {
        if self.ranking.forward(self.from(edge)).len() > 2 {
            return (ends[1] + 5.0, from.cy + self.label_drop, "start");
        }
        (
            ends[0],
            from.cy - RUN_LIFT,
            if sign > 0.0 { "start" } else { "end" },
        )
    }

    fn side_entry(&self, from: &Box, to: &Box, start_x: f64) -> Route {
        let edge_x = if start_x > to.cx {
            to.right()
        } else {
            to.left()
        };
        Route {
            points: vec![(start_x, from.bottom()), (start_x, to.cy), (edge_x, to.cy)],
            label_x: start_x + 5.0,
            label_y: from.bottom() + self.label_drop,
            anchor: "start",
        }
    }

    /// Out of one side of `from`, along a lane beside every node in the
    /// rows between, and into the same side of `to`. `side`: the left.
    fn detour(&mut self, edge: usize, from: &Box, to: &Box, side: bool) -> Route {
        let lane = self.lane_x(edge, side);
        let ends = if side {
            [from.left(), to.left()]
        } else {
            [from.right(), to.right()]
        };
        Route {
            points: vec![
                (ends[0], from.cy),
                (lane, from.cy),
                (lane, to.cy),
                (ends[1], to.cy),
            ],
            label_x: ends[0] + if side { -4.0 } else { 4.0 },
            label_y: from.cy - 4.0,
            anchor: if side { "end" } else { "start" },
        }
    }

    fn lane_x(&mut self, edge: usize, side: bool) -> f64 {
        let count = self.detours.entry(side).or_insert(0);
        *count += 1;
        let offset = DETOUR + (DETOUR_STEP * (*count as f64));
        let boxes = self.boxes_between(edge);
        if side {
            boxes
                .iter()
                .map(|node| self.boxes[node].left())
                .fold(f64::INFINITY, f64::min)
                - offset
        } else {
            boxes
                .iter()
                .map(|node| self.boxes[node].right())
                .fold(f64::NEG_INFINITY, f64::max)
                + offset
        }
    }

    fn same_column(&self, from: &Box, to: &Box) -> bool {
        (from.cx - to.cx).abs() < 1.0
    }

    fn boxes_between(&self, edge: usize) -> Vec<usize> {
        let (first, last) = (
            self.ranking.rank[&self.from(edge)],
            self.ranking.rank[&self.to(edge)],
        );
        let (first, last) = (first.min(last), first.max(last));
        let mut between: Vec<usize> = self
            .boxes
            .keys()
            .copied()
            .filter(|node| (first..=last).contains(&self.ranking.rank[node]))
            .collect();
        between.sort_unstable();
        between
    }

    /// A bar is crossed wherever its neighbour stands. A decision reaches
    /// for the middle of the branches the bar fans out to, and a bar that
    /// closes onto the merge diamond leaves from that middle.
    fn anchor_x(
        &self,
        edge: usize,
        node: usize,
        node_box: &Box,
        other: usize,
        other_box: &Box,
    ) -> f64 {
        if self.diagram.nodes[node].kind != Kind::Bar {
            return node_box.cx;
        }
        if self.middle(edge, node, node_box, other) {
            return node_box.anchor.unwrap_or(node_box.cx);
        }
        other_box.cx.clamp(node_box.left(), node_box.right())
    }

    fn middle(&self, edge: usize, node: usize, node_box: &Box, other: usize) -> bool {
        if self.diagram.nodes[self.to(edge)].kind == Kind::Merge && node == self.from(edge) {
            return true;
        }
        self.diagram.nodes[self.from(edge)].kind == Kind::Decision && !node_box.fan.contains(&other)
    }
}

/// The route's shape, in the order `Router#hanging` reads them.
enum Shape {
    SideEntry,
    SideExit,
    Straight,
    Elbow,
}
