//! Writes each transition of a placed state graph as absolute `:sections`
//! plus the point its label sits at. Port of
//! `lib/etassia/notation/plantuml/state/`'s `Routes`, `RouteGeometry`,
//! `LabelClearance`, `RouteDetour` and `Plane`.
//!
//! A transition between two boxes of one container keeps the route ELK
//! gave it. Every other one (a loop, one that crosses a container's
//! border, one beside a note, one ELK dropped to break a cycle) is a
//! straight line between the two states' borders. Transitions between the
//! same two states are spread apart so none hides another.

use super::hierarchy::{Elk, Level};
use super::ir::{GEdge, GLabel, Section};
use std::collections::HashMap;

use crate::Error;

// ---- plane.rb ----

const STEPS: usize = 60;

type Rect = [f64; 4];

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    [x, y, x + w, y + h]
}

fn around(centre: (f64, f64), half: (f64, f64)) -> Rect {
    [centre.0 - half.0, centre.1 - half.1, centre.0 + half.0, centre.1 + half.1]
}

fn grow(r: Rect, margin: f64) -> Rect {
    [r[0] - margin, r[1] - margin, r[2] + margin, r[3] + margin]
}

fn corners(r: Rect) -> [(f64, f64); 4] {
    [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])]
}

fn centre_rect(r: Rect) -> (f64, f64) {
    ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0)
}

fn meet(one: Rect, other: Rect) -> bool {
    one[0] < other[2] && other[0] < one[2] && one[1] < other[3] && other[1] < one[3]
}

fn inside(r: Rect, spot: (f64, f64)) -> bool {
    spot.0 >= r[0] && spot.0 <= r[2] && spot.1 >= r[1] && spot.1 <= r[3]
}

/// Whether the segment passes through the rect.
fn crosses(r: Rect, from: (f64, f64), to: (f64, f64)) -> bool {
    (0..=STEPS).any(|step| inside(r, along(from, to, step as f64 / STEPS as f64)))
}

fn along(from: (f64, f64), to: (f64, f64), fraction: f64) -> (f64, f64) {
    (
        from.0 + ((to.0 - from.0) * fraction),
        from.1 + ((to.1 - from.1) * fraction),
    )
}

/// The point a fraction of the way along the polyline's length, with the
/// stretch it lies on.
fn at_length(points: &[(f64, f64)], fraction: f64) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let (stretch, left) = stretch_at(points, fraction);
    let len = polyline_length(&stretch);
    let back = if len == 0.0 { 1.0 } else { (1.0 + left / len).clamp(0.0, 1.0) };
    (along(stretch[0], stretch[1], back), stretch[0], stretch[1])
}

/// The stretch a length along the polyline lies on, and how far along it
/// is.
fn stretch_at(points: &[(f64, f64)], fraction: f64) -> ([(f64, f64); 2], f64) {
    let stretches: Vec<[(f64, f64); 2]> = points
        .windows(2)
        .filter(|w| w[0] != w[1])
        .map(|w| [w[0], w[1]])
        .collect();
    let mut left = polyline_length(points) * fraction;
    let mut found = None;
    for stretch in &stretches {
        left -= dist(stretch[0], stretch[1]);
        if left <= 0.0 {
            found = Some(*stretch);
            break;
        }
    }
    (
        found.unwrap_or(*stretches.last().unwrap_or(&[points[0], points[0]])),
        left,
    )
}

fn polyline_length(points: &[(f64, f64)]) -> f64 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

// ---- route_geometry.rb ----

const SPREAD: f64 = 16.0;
const LOOP: f64 = 28.0;
const LOOP_HALF: f64 = 10.0;
const LABEL_GAP: f64 = 6.0;
const BOW: f64 = 1.5;

/// A state's absolute box and the arena slot it came from.
#[derive(Clone, Copy)]
pub struct Boxf {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub slot: Option<usize>,
}

/// A transition's place among those joining the same two states.
#[derive(Clone, Copy)]
struct Place {
    index: usize,
    total: usize,
    offset: f64,
}

impl Place {
    fn lane(&self) -> f64 {
        self.index as f64 - ((self.total - 1) as f64 / 2.0)
    }
}

// ---- label_clearance.rb ----

const CHORD_STEPS: usize = 40;
const CROWD: i64 = 4;
const CROWD_SHIFT: f64 = 0.15;
const TITLE_BAND: f64 = 30.0;
const ALONG: [f64; 8] = [0.4, 0.6, 0.3, 0.7, 0.2, 0.8, 0.1, 0.9];

// ---- route_detour.rb ----

const CLEAR: f64 = 24.0;
const LANE: f64 = 14.0;
const GRAZE: f64 = 7.0;
const PASSES: usize = 6;

/// The writer of absolute sections and label spots.
pub struct Routes<'a> {
    pub slots: &'a [super::hierarchy::Slot],
    pub roots: Vec<usize>,
    pub edges: Vec<GEdge>,
    nodes: HashMap<String, usize>,
    parent: HashMap<String, Option<String>>,
    levels: &'a [(Option<String>, Level)],
    label_reach: &'a super::hierarchy::LabelReach,
    boxes: HashMap<String, Boxf>,
    degree: HashMap<String, i64>,
    placed: Vec<Rect>,
    detoured: Vec<usize>,
    lanes: HashMap<String, usize>,
    slot_map: HashMap<usize, (f64, f64, f64)>,
}

impl<'a> Routes<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slots: &'a [super::hierarchy::Slot],
        roots: Vec<usize>,
        edges: Vec<GEdge>,
        nodes: HashMap<String, usize>,
        parent: HashMap<String, Option<String>>,
        levels: &'a [(Option<String>, Level)],
        label_reach: &'a super::hierarchy::LabelReach,
    ) -> Routes<'a> {
        let degree = Self::degrees(&edges);
        Routes {
            slots,
            roots,
            edges,
            nodes,
            parent,
            levels,
            label_reach,
            boxes: HashMap::new(),
            degree,
            placed: vec![],
            detoured: vec![],
            lanes: HashMap::new(),
            slot_map: HashMap::new(),
        }
    }

    pub fn into_edges(self) -> Vec<GEdge> {
        self.edges
    }

    /// How many transitions meet at each state.
    fn degrees(edges: &[GEdge]) -> HashMap<String, i64> {
        let mut out: HashMap<String, i64> = HashMap::new();
        for edge in edges {
            let mut ends = [edge.sources.first().cloned(), edge.targets.first().cloned()];
            ends.sort();
            if ends[0] != ends[1] {
                if let (Some(a), Some(b)) = (ends[0].clone(), ends[1].clone()) {
                    *out.entry(a).or_default() += 1;
                    *out.entry(b).or_default() += 1;
                }
            }
        }
        out
    }

    pub fn write(&mut self) -> Result<(), Error> {
        let roots = self.roots.clone();
        self.measure(&roots, 0.0, 0.0);
        let slots = self.label_slots();
        self.slot_map = slots;
        let mut routed: Vec<(usize, Vec<(f64, f64)>)> = vec![];
        let edges = self.edges.clone();
        for edge in &edges {
            routed.push((edge.oid, self.polyline(edge)));
        }
        let spread = self.spread(routed);
        for (oid, points, place) in spread {
            self.record(oid, points, place)?;
        }
        Ok(())
    }

    fn record(&mut self, oid: usize, mut points: Vec<(f64, f64)>, place: Place) -> Result<(), Error> {
                let slotted = self.slot_map.get(&oid).copied();
        if let Some((x, y, via)) = slotted {
            points = vec![points[0], (via, y), points[points.len() - 1]];
        }
        let label = self.edges.iter().find(|e| e.oid == oid).and_then(|e| e.labels.first().cloned());
        let route = self.label_route(oid, &points, label.as_ref());
        let spot = label_point(self, &route, label.as_ref(), place);
        let stood = slotted.map(|(x, y, _)| (x, y));
        if let Some((x, y)) = stood {
            if let Some(label) = &label {
                self.placed.push(rect_at((x, y), label));
            }
        }
        let found = match (stood, spot) {
            (Some(at), _) => Some(at),
            (None, Some(spot)) => self.settled(spot, &points, label.as_ref()),
            (None, None) => None,
        };
        let at = self.edges.iter_mut().find(|e| e.oid == oid);
        let Some(edge) = at else { return Ok(()) };
        edge.sections = Some(vec![section_of(&points)]);
        edge.label_at = found;
        Ok(())
    }

    /// The labels that share a rank inside a container stand in the slots
    /// LabelReach gives them, which also size that container.
    fn label_slots(&mut self) -> HashMap<usize, (f64, f64, f64)> {
        let containers: Vec<Boxf> = self.boxes.values().copied().collect();
        let mut out: HashMap<usize, (f64, f64, f64)> = HashMap::new();
        for box_ in &containers {
            let Some(slot) = box_.slot else { continue };
            let members: Vec<usize> = self.slots[slot].children.clone();
            for (oid, at) in self.label_reach.slots(&members, self.slots) {
                out.insert(oid, (box_.x + at.0, box_.y + at.1, box_.x + at.2));
            }
        }
        out
    }

    fn measure(&mut self, children: &[usize], left: f64, top: f64) {
        for &i in children {
            let node = &self.slots[i].node;
            let (x, y, w, h) = (node.x.unwrap_or(0.0), node.y.unwrap_or(0.0), node.width, node.height);
            self.boxes.insert(
                node.id.clone(),
                Boxf { x: left + x, y: top + y, w, h, slot: Some(i) },
            );
            let kids = self.slots[i].children.clone();
            self.measure(&kids, left + x, top + y);
        }
    }

    fn polyline(&mut self, edge: &GEdge) -> Vec<(f64, f64)> {
        let source = &edge.sources[0];
        let target = &edge.targets[0];
        if source == target {
            let box_ = self.boxes.get(source).copied().unwrap_or(Boxf {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
                slot: None,
            });
            return self.loop_points(&box_);
        }
        let route = self.elk_route(edge, source, target).unwrap_or_else(|| self.straight(source, target));
        let detoured = self.around_boxes(edge, &route);
        if detoured != route {
            self.detoured.push(edge.oid);
        }
        detoured
    }

    /// A route bent round a box carries its label along the route itself;
    /// a straight one may slide its label along the chord.
    fn label_route(&self, oid: usize, points: &[(f64, f64)], label: Option<&GLabel>) -> Vec<(f64, f64)> {
        if self.detoured.contains(&oid) {
            return points.to_vec();
        }
        self.clear_of_containers(oid, points, label)
    }

    fn elk_route(&self, edge: &GEdge, source: &str, target: &str) -> Option<Vec<(f64, f64)>> {
        let mut found: Option<(Option<String>, &Level)> = None;
        for (owner, level) in self.levels {
            if placed_edge(level, edge.oid).is_some() {
                found = Some((owner.clone(), level));
                break;
            }
        }
        let (owner, level) = found?;
        let placed = placed_edge(level, edge.oid)?;
        if !(
            placed.sources == vec![source.to_string()]
                && placed.targets == vec![target.to_string()]
                && !self.noted(source)
                && !self.noted(target)
        ) {
            return None;
        }
        let origin = match &owner {
            Some(owner) => self.boxes.get(owner).copied().unwrap_or(Boxf {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
                slot: None,
            }),
            None => Boxf { x: 0.0, y: 0.0, w: 0.0, h: 0.0, slot: None },
        };
        Some(shifted(&placed.sections.as_ref()?[0], &origin, level.shift))
    }

    fn noted(&self, id: &str) -> bool {
        self.boxes.values().any(|box_| match box_.slot {
            Some(slot) => {
                self.slots[slot].node.meta.note_target.as_deref() == Some(id)
            }
            None => false,
        })
    }

    fn straight(&self, source: &str, target: &str) -> Vec<(f64, f64)> {
        let from = self.boxes.get(source).copied().expect("source box");
        let to = self.boxes.get(target).copied().expect("target box");
        let toward = centre_of(&to);
        let back = centre_of(&from);
        vec![self.border(&from, toward), self.border(&to, back)]
    }

    fn centre_of(&self, box_: &Boxf) -> (f64, f64) {
        (box_.x + box_.w / 2.0, box_.y + box_.h / 2.0)
    }

    fn loop_points(&self, box_: &Boxf) -> Vec<(f64, f64)> {
        let right = box_.x + box_.w;
        let (top, bottom) = self.loop_span(box_);
        vec![(right, top), (right + LOOP, top), (right + LOOP, bottom), (right, bottom)]
    }

    /// A composite's loop sits level with the first row of its content.
    fn loop_span(&self, box_: &Boxf) -> (f64, f64) {
        let Some(slot) = box_.slot else {
            return (box_.y + (box_.h * 0.35), box_.y + (box_.h * 0.65));
        };
        let rows: Vec<f64> = self.slots[slot]
            .children
            .iter()
            .map(|&c| {
                self.slots[c].node.y.unwrap_or(0.0) + self.slots[c].node.height / 2.0
            })
            .collect();
        if rows.is_empty() {
            return (box_.y + (box_.h * 0.35), box_.y + (box_.h * 0.65));
        }
        let min = rows.iter().copied().fold(f64::MAX, f64::min);
        (box_.y + (min - LOOP_HALF), box_.y + (min + LOOP_HALF))
    }

    fn shifted(&self, section: &Section, origin: &Boxf, shift: (f64, f64)) -> Vec<(f64, f64)> {
        let (dx, dy) = (origin.x + shift.0, origin.y + shift.1);
        let mut points = vec![section.start_point];
        points.extend(section.bend_points.iter().copied());
        points.push(section.end_point);
        points.into_iter().map(|(x, y)| (x + dx, y + dy)).collect()
    }

    /// Where the line from a box's centre toward a point leaves the box.
    fn border(&self, box_: &Boxf, toward: (f64, f64)) -> (f64, f64) {
        let (cx, cy) = self.centre_of(box_);
        let (dx, dy) = (toward.0 - cx, toward.1 - cy);
        if dx == 0.0 && dy == 0.0 {
            return (cx, cy);
        }
        let scale = self.fit(box_, dx, dy);
        (cx + dx * scale, cy + dy * scale)
    }

    fn fit(&self, box_: &Boxf, run: f64, rise: f64) -> f64 {
        if self.circle(box_) {
            return (box_.w / 2.0) / run.hypot(rise);
        }
        ((box_.w / 2.0) / run.abs()).min((box_.h / 2.0) / rise.abs())
    }

    fn circle(&self, box_: &Boxf) -> bool {
        match box_.slot {
            Some(slot) => {
                let ty = self.slots[slot].node.meta.shape_type.as_str();
                ty == "start" || ty == "end"
            }
            None => false,
        }
    }

    fn sideways(&self, points: &[(f64, f64)], from: (f64, f64), to: (f64, f64), offset: f64) -> Vec<(f64, f64)> {
        let (run, rise) = (to.0 - from.0, to.1 - from.1);
        let scale = offset / run.hypot(rise);
        points.iter().map(|&(x, y)| (x - rise * scale, y + run * scale)).collect()
    }

    fn bowed(&self, points: &[(f64, f64)], from: (f64, f64), to: (f64, f64), offset: f64) -> Vec<(f64, f64)> {
        let middle = midpoint(points[0], points[points.len() - 1]);
        let normal = self.sideways(&[(0.0, 0.0)], from, to, offset * BOW)[0];
        vec![points[0], (middle.0 + normal.0, middle.1 + normal.1), points[points.len() - 1]]
    }

    // -- spreading --

    fn spread(
        &mut self,
        routed: Vec<(usize, Vec<(f64, f64)>)>,
    ) -> Vec<(usize, Vec<(f64, f64)>, Place)> {
        let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
        for (at, (oid, _)) in routed.iter().enumerate() {
            let edge = &self.edges.iter().find(|e| e.oid == *oid).expect("edge");
            let key = pair_key(edge);
            groups.entry(key).or_default().push(at);
        }
        let mut out = vec![];
        for (_, members) in groups {
            let total = members.len();
            for (index, at) in members.into_iter().enumerate() {
                let mut place = Place { index, total, offset: 0.0 };
                place.offset = place.lane() * SPREAD;
                let (oid, points) = &routed[at];
                let key = pair_key(self.edges.iter().find(|e| e.oid == *oid).expect("edge"));
                let moved = self.move_points(points, &key, place.offset);
                out.push((*oid, moved, place));
            }
        }
        out
    }

    fn pair_key(edge: &GEdge) -> (String, String) {
        let mut pair = [edge.sources[0].clone(), edge.targets[0].clone()];
        pair.sort();
        (pair[0].clone(), pair[1].clone())
    }

    /// Parts a route from the others of its pair, across the line between
    /// the two states taken in one fixed order so a pair's two directions
    /// part ways.
    fn move_points(&self, points: &[(f64, f64)], key: &(String, String), offset: f64) -> Vec<(f64, f64)> {
        if offset == 0.0 || key.0 == key.1 {
            return points.to_vec();
        }
        let from = self.centre_of(self.boxes.get(&key.0).expect("pair box"));
        let to = self.centre_of(self.boxes.get(&key.1).expect("pair box"));
        if points.len() == 2 {
            return self.bowed(points, from, to, offset);
        }
        self.sideways(points, from, to, offset)
    }

    // -- label clearance (LabelClearance) --

    /// The label's spot, moved along the route to the nearest one that is
    /// off every container's title, every state and every label already
    /// placed; the first spot when none is.
    fn settled(&mut self, spot: (f64, f64), points: &[(f64, f64)], label: Option<&GLabel>) -> Option<(f64, f64)> {
        let Some(label) = label else { return None };
        let mut found = spot;
        let mut candidates = vec![spot];
        candidates.extend(self.spots_along(points, label));
        for at in candidates {
            let r = rect_at(at, label);
            if self.free(r) {
                found = at;
                break;
            }
        }
        self.placed.push(rect_at(found, label));
        Some(found)
    }

    fn rect_at(&self, spot: (f64, f64), label: &GLabel) -> Rect {
        around(spot, (label.width / 2.0, label.height / 2.0))
    }

    fn free(&self, r: Rect) -> bool {
        let fenced = grow(r, 3.0);
        let mut obstacles = self.obstacles();
        obstacles.extend(self.placed.iter().copied());
        obstacles.iter().all(|other| !meet(fenced, *other))
    }

    /// Each container's title band and each state that holds nothing.
    fn obstacles(&self) -> Vec<Rect> {
        let mut out = vec![];
        for box_ in self.boxes.values() {
            if self.note_box(box_) {
                continue;
            }
            out.push(self.title_band(box_));
        }
        out
    }

    fn title_band(&self, box_: &Boxf) -> Rect {
        let r = rect(box_.x, box_.y, box_.w, box_.h);
        match box_.slot {
            Some(slot) if !self.slots[slot].children.is_empty() => {
                [r[0], r[1], r[2], r[1] + TITLE_BAND]
            }
            _ => r,
        }
    }

    fn spots_along(&self, points: &[(f64, f64)], label: &GLabel) -> Vec<(f64, f64)> {
        let mut out = vec![];
        for fraction in ALONG {
            let (at, from, to) = at_length(points, fraction);
            let (reach, lift) = if steep(from, to) {
                (label.width / 2.0 + LABEL_GAP, 0.0)
            } else {
                (0.0, label.height / 2.0 + LABEL_GAP)
            };
            out.push((at.0 + reach, at.1 - lift));
            out.push((at.0 - reach, at.1 + lift));
        }
        out
    }

    /// The route moved along its chord so its label sits where the chord
    /// is outside the containers of both ends, not over a container's
    /// title, and away from a crowded end.
    fn clear_of_containers(&self, oid: usize, points: &[(f64, f64)], label: Option<&GLabel>) -> Vec<(f64, f64)> {
        let _ = label;
        let edge = match self.edges.iter().find(|e| e.oid == oid) {
            Some(edge) => edge,
            None => return points.to_vec(),
        };
        let slide =
            self.clear_middle(edge, points) + self.crowd_bias(edge) - 0.5;
        if slide == 0.0 {
            return points.to_vec();
        }
        let chord = self.chord(points);
        points
            .iter()
            .map(|&(x, y)| (x + chord.0 * slide, y + chord.1 * slide))
            .collect()
    }

    fn chord(&self, points: &[(f64, f64)]) -> (f64, f64) {
        let last = points[points.len() - 1];
        let first = points[0];
        (last.0 - first.0, last.1 - first.1)
    }

    /// The middle, as a fraction of the chord, of the stretch outside both
    /// ends' containers; 0.5 when there is none.
    fn clear_middle(&self, edge: &GEdge, points: &[(f64, f64)]) -> f64 {
        let boxes = self.foreign_containers(edge);
        let clear: Vec<f64> = (0..=CHORD_STEPS)
            .map(|step| step as f64 / CHORD_STEPS as f64)
            .filter(|&at| self.open(&boxes, points, at))
            .collect();
        if boxes.is_empty() || clear.len() < 2 {
            return 0.5;
        }
        (clear[0] + clear[clear.len() - 1]) / 2.0
    }

    fn open(&self, boxes: &[Boxf], points: &[(f64, f64)], at: f64) -> bool {
        let chord = self.chord(points);
        let spot = (points[0].0 + chord.0 * at, points[0].1 + chord.1 * at);
        boxes.iter().all(|box_| !self.holds(box_, spot))
    }

    /// A label moves toward the far end of a chord that leaves or reaches
    /// a state many transitions meet at.
    fn crowd_bias(&self, edge: &GEdge) -> f64 {
        let out = self.degree.get(&edge.sources[0]).copied().unwrap_or(0);
        let into = self.degree.get(&edge.targets[0]).copied().unwrap_or(0);
        if out.max(into) < CROWD {
            return 0.0;
        }
        if out > into {
            CROWD_SHIFT
        } else {
            -CROWD_SHIFT
        }
    }

    /// The boxes around only one end of the edge.
    fn foreign_containers(&self, edge: &GEdge) -> Vec<Boxf> {
        let source = self.ancestors(&edge.sources[0]);
        let target = self.ancestors(&edge.targets[0]);
        let mut out: Vec<Boxf> =
            source.iter().filter(|s| !target.iter().any(|t| t.x == s.x && t.y == s.y && t.w == s.w && t.h == s.h && t.slot == s.slot)).copied().collect();
        out.extend(
            target
                .iter()
                .filter(|t| !source.iter().any(|s| s.x == t.x && s.y == t.y && s.w == t.w && s.h == t.h && s.slot == t.slot))
                .copied(),
        );
        out
    }

    fn ancestors(&self, id: &str) -> Vec<Boxf> {
        let mut found = vec![];
        let mut id = id.to_string();
        while let Some(owner) = self.parent.get(&id).cloned().flatten() {
            if let Some(box_) = self.boxes.get(&owner) {
                found.push(*box_);
            }
            id = owner;
        }
        found
    }

    fn holds(&self, box_: &Boxf, spot: (f64, f64)) -> bool {
        spot.0 >= box_.x
            && spot.0 <= box_.x + box_.w
            && spot.1 >= box_.y
            && spot.1 <= box_.y + box_.h
    }

    // -- route detour (RouteDetour) --

    /// The route with waypoints beside each box that blocks it.
    fn around_boxes(&mut self, edge: &GEdge, points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let boxes: Vec<Boxf> = self.blockers_of(edge);
        let rects: Vec<Rect> = boxes
            .iter()
            .map(|box_| rect(box_.x, box_.y, box_.w, box_.h))
            .collect();
        if rects.is_empty() {
            return points.to_vec();
        }
        let mut route = points.to_vec();
        for _ in 0..PASSES {
            let detoured = self.detour_once(&route, &rects);
            if detoured == route {
                return detoured;
            }
            route = detoured;
        }
        route
    }

    /// Boxes the edge neither starts in, ends in, nor runs inside of,
    /// outermost only: going round a container clears what it holds.
    fn blockers_of(&self, edge: &GEdge) -> Vec<Boxf> {
        let ends = [edge.sources[0].clone(), edge.targets[0].clone()];
        let mut kin: Vec<String> = vec![];
        for id in &ends {
            kin.push(id.clone());
            kin.extend(self.lineage(id));
        }
        let outside: Vec<(&String, &Boxf)> = self
            .boxes
            .iter()
            .filter(|(id, box_)| {
                let line = self.lineage(id);
                !kin.iter().any(|k| k == *id)
                    && !self.note_box(box_)
                    && !line.iter().any(|up| ends.iter().any(|e| e == up))
            })
            .collect();
        self.outermost(&outside)
    }

    fn outermost(&self, boxes: &[(&String, &Boxf)]) -> Vec<Boxf> {
        boxes
            .iter()
            .filter(|(id, _)| {
                let line = self.lineage(id);
                !line.iter().any(|up| boxes.iter().any(|(other, _)| other.as_str() == up.as_str()))
            })
            .map(|(_, box_)| **box_)
            .collect()
    }

    fn note_box(&self, box_: &Boxf) -> bool {
        match box_.slot {
            Some(slot) => self.slots[slot].node.meta.state_type == "note",
            None => false,
        }
    }

    fn lineage(&self, id: &str) -> Vec<String> {
        // Ruby: `found << id while (id = @parent[id])` — every ancestor,
        // the state itself never among them.
        let mut found = vec![];
        let mut id = id.to_string();
        while let Some(owner) = self.parent.get(&id).cloned().flatten() {
            found.push(owner.clone());
            id = owner;
        }
        found
    }

    fn detour_once(&mut self, route: &[(f64, f64)], rects: &[Rect]) -> Vec<(f64, f64)> {
        for (index, window) in route.windows(2).enumerate() {
            let (from, to) = (window[0], window[1]);
            let Some(rect) = self.nearest_crossed(rects, from, to) else { continue };
            let waypoints = self.waypoints(rect, from, to);
            let mut out: Vec<(f64, f64)> = route[..=index].to_vec();
            out.extend(waypoints);
            out.extend(route[index + 1..].to_vec());
            return out;
        }
        route.to_vec()
    }

    fn nearest_crossed(&self, rects: &[Rect], from: (f64, f64), to: (f64, f64)) -> Option<Rect> {
        let mut best: Option<(Rect, f64)> = None;
        for &r in rects {
            if !crosses(grow(r, GRAZE), from, to) {
                continue;
            }
            let d = dist(centre_rect(r), from);
            if best.is_none() || d < best.unwrap().1 {
                best = Some((r, d));
            }
        }
        best.map(|(r, _)| r)
    }

    /// The way round the rect, a corner, or two corners of one side, that
    /// is the shortest and leaves every stretch clear of it.
    fn waypoints(&mut self, r: Rect, from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
        let reach = self.lane_of(r);
        let ways = self.ways_round(r, reach);
        let mut clear: Vec<Vec<(f64, f64)>> = vec![];
        for way in &ways {
            let mut route = vec![from];
            route.extend(way.iter().copied());
            route.push(to);
            let blocks = route.windows(2).all(|w| !crosses(grow(r, GRAZE), w[0], w[1]));
            if blocks {
                clear.push(way.clone());
            }
        }
        let candidates = if clear.is_empty() { ways } else { clear };
        let mut best: Option<(Vec<(f64, f64)>, f64)> = None;
        for way in candidates {
            let mut route = vec![from];
            route.extend(way.iter().copied());
            route.push(to);
            let len = polyline_length(&route);
            if best.is_none() || len < best.as_ref().unwrap().1 {
                best = Some((way, len));
            }
        }
        best.map(|(way, _)| way).unwrap_or_default()
    }

    fn lane_of(&mut self, r: Rect) -> f64 {
        let key = format!("{},{},{},{}", r[0], r[1], r[2], r[3]);
        let at = self.lanes.entry(key).or_insert(0);
        *at += 1;
        CLEAR + LANE * (*at - 1) as f64
    }

    fn ways_round(&self, r: Rect, reach: f64) -> Vec<Vec<(f64, f64)>> {
        let corners = corners(grow(r, reach));
        let mut ways: Vec<Vec<(f64, f64)>> =
            corners.iter().map(|c| vec![*c]).collect();
        for pair in corners.windows(2) {
            ways.push(vec![pair[0], pair[1]]);
        }
        ways.push(vec![corners[3], corners[0]]);
        ways
    }
}

fn placed_edge(level: &Level, oid: usize) -> Option<&super::hierarchy::LevelEdge> {
    level
        .elk
        .as_ref()?
        .edges
        .iter()
        .find(|edge| edge.id == oid.to_string() && edge.sections.is_some())
}

fn rect_at(spot: (f64, f64), label: &GLabel) -> Rect {
    around(spot, (label.width / 2.0, label.height / 2.0))
}

fn centre_of(box_: &Boxf) -> (f64, f64) {
    (box_.x + box_.w / 2.0, box_.y + box_.h / 2.0)
}

fn pair_key(edge: &GEdge) -> (String, String) {
    let mut pair = [edge.sources[0].clone(), edge.targets[0].clone()];
    pair.sort();
    (pair[0].clone(), pair[1].clone())
}

/// Beside the longest stretch of the route; parallel labels are stacked so
/// none is written over another.
fn label_point(
    routes: &Routes,
    points: &[(f64, f64)],
    label: Option<&GLabel>,
    place: Place,
) -> Option<(f64, f64)> {
    let Some(label) = label else { return None };
    if place.total > 1 && points.len() == 3 {
        return Some(bowed_label(routes, points, label));
    }
    let (from, to) = longest_stretch(points)?;
    let middle = midpoint(from, to);
    if steep(from, to) {
        return Some(beside(label, place, middle));
    }
    Some((middle.0, middle.1 - LABEL_GAP - label.height / 2.0))
}

/// Past the apex of a bowed route, on the side it bows to.
fn bowed_label(_routes: &Routes, points: &[(f64, f64)], label: &GLabel) -> (f64, f64) {
    let apex = points[1];
    let unit = unit_between(midpoint(points[0], points[points.len() - 1]), apex);
    let reach = outward_reach(unit, label) + LABEL_GAP;
    (apex.0 + unit.0 * reach, apex.1 + unit.1 * reach)
}

fn unit_between(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let (run, rise) = (to.0 - from.0, to.1 - from.1);
    let length = run.hypot(rise);
    (run / length, rise / length)
}

fn outward_reach(unit: (f64, f64), label: &GLabel) -> f64 {
    ((unit.0.abs() * label.width) + (unit.1.abs() * label.height)) / 2.0
}

fn midpoint(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0)
}

fn steep(from: (f64, f64), to: (f64, f64)) -> bool {
    (to.0 - from.0).abs() < (to.1 - from.1).abs()
}

fn longest_stretch(points: &[(f64, f64)]) -> Option<((f64, f64), (f64, f64))> {
    points
        .windows(2)
        .map(|w| (w[0], w[1]))
        .max_by(|a, b| {
            let al = dist(a.0, a.1);
            let bl = dist(b.0, b.1);
            al.partial_cmp(&bl).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Right of the group's rightmost line, one row per transition.
fn beside(label: &GLabel, place: Place, middle: (f64, f64)) -> (f64, f64) {
    let clear = if place.total > 2 && place.offset == 0.0 {
        label.height + 2.0
    } else {
        0.0
    };
    (
        rightmost(place, middle.0) + label.width / 2.0 + LABEL_GAP,
        middle.1 + (place.lane() * (label.height + 4.0)) + clear,
    )
}

fn rightmost(place: Place, line_x: f64) -> f64 {
    line_x - place.offset + (SPREAD * (place.total - 1) as f64 / 2.0)
}

fn section_of(points: &[(f64, f64)]) -> Section {
    Section {
        start_point: points[0],
        end_point: points[points.len() - 1],
        bend_points: points[1..points.len() - 1].to_vec(),
    }
}

fn shifted(section: &Section, origin: &Boxf, shift: (f64, f64)) -> Vec<(f64, f64)> {
    let (dx, dy) = (origin.x + shift.0, origin.y + shift.1);
    let mut points = vec![section.start_point];
    points.extend(section.bend_points.iter().copied());
    points.push(section.end_point);
    points.into_iter().map(|(x, y)| (x + dx, y + dy)).collect()
}

