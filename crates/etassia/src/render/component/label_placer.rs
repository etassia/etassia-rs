//! Where a label can sit without a line running through it, and the
//! segment/box overlap tests beneath it. Port of
//! `plantuml/label_clearance.rb`, `plantuml/segment_box.rb` and
//! `node_graph/label_placer.rb`.

use super::metrics::face_width;
use super::model::Edge;
use super::scene::{Placed, Route};

const FONT_SIZE: f64 = 12.0;
const LINE_STEP: f64 = 14.0;
const RISE: f64 = 4.0;
const MARGIN: f64 = 2.0;
const MAX_STEPS: i64 = 24;
const BESIDE_GAP: f64 = 4.0;
const NEIGHBOUR_GAP: f64 = 6.0;
const REACH: f64 = 20.0;
const FRACTIONS: [f64; 5] = [0.5, 0.65, 0.35, 0.8, 0.2];

pub struct Placement {
    pub route_index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

impl Placement {
    pub fn right(&self) -> f64 {
        self.x + self.width / 2.0 + MARGIN
    }

    pub fn bottom(&self) -> f64 {
        self.y + FONT_SIZE / 4.0
    }
}

/// One label spot per labelled route, none covering a box or another
/// label.
pub fn call(routes: &[Route], boxes: &[Placed]) -> Vec<Placement> {
    let taken = boxes_area(boxes);
    let mut placements = vec![];
    let mut taken = taken;
    for (index, route) in routes.iter().enumerate() {
        let Some(text) = route.edge.label.as_deref() else { continue };
        if text.is_empty() {
            continue;
        }
        let width = face_width(text, FONT_SIZE);
        let others: Vec<usize> =
            (0..routes.len()).filter(|other| *other != index).collect();
        if let Some(placement) = place(route, index, width, &mut taken, routes, &others) {
            placements.push(placement);
        }
    }
    placements
}

fn place(
    route: &Route,
    index: usize,
    width: f64,
    taken: &mut Vec<[f64; 4]>,
    routes: &[Route],
    others: &[usize],
) -> Option<Placement> {
    match spot_for(route, index, width, taken, routes, others) {
        Some(spot) => {
            taken.push(bounds(spot.0, spot.1, width));
            Some(Placement { route_index: index, x: spot.0, y: spot.1, width })
        }
        None => {
            let (x_pos, y_pos) = along(&route.points, FRACTIONS[0]);
            taken.push(bounds(x_pos, y_pos - RISE, width));
            Some(Placement { route_index: index, x: x_pos, y: y_pos - RISE, width })
        }
    }
}

type Spot = (f64, f64);

fn spot_for(
    route: &Route,
    index: usize,
    width: f64,
    taken: &[[f64; 4]],
    routes: &[Route],
    others: &[usize],
) -> Option<Spot> {
    let mut fewest: Option<(Spot, usize)> = None;
    for spot in candidates(route, width) {
        let Some(count) = clashes(spot, route, index, width, taken, routes, others) else {
            continue;
        };
        if count == 0 {
            return Some(spot);
        }
        let better = match fewest {
            None => true,
            Some((_, best)) => count < best,
        };
        if better {
            fewest = Some((spot, count));
        }
    }
    fewest.map(|(spot, _)| spot)
}

/// How many other edges lie across the candidate, or `None` when it
/// cannot be used.
#[allow(clippy::too_many_arguments)]
fn clashes(
    spot: Spot,
    route: &Route,
    _index: usize,
    width: f64,
    taken: &[[f64; 4]],
    routes: &[Route],
    others: &[usize],
) -> Option<usize> {
    let boxed = bounds(spot.0, spot.1, width);
    if taken.iter().any(|other| overlap(&boxed, other)) {
        return None;
    }
    if covers(&boxed, route, 0.0) || !reaches(&boxed, route) {
        return None;
    }
    Some(
        others
            .iter()
            .filter(|other| covers(&boxed, &routes[**other], NEIGHBOUR_GAP))
            .count(),
    )
}

/// Every candidate, nearest the edge first.
fn candidates(route: &Route, width: f64) -> Vec<Spot> {
    let mut spots = vec![];
    spots.extend(beside(route.label_at, width));
    spots.extend(aside(route, width));
    spots.extend(clear_of(route, width));
    spots.extend(spread(route, width));
    spots.extend(along_edge(&route.points));
    spots
}

/// Right of a sloping edge, in the corner the edge leaves free.
fn aside(route: &Route, width: f64) -> Vec<Spot> {
    let first = route.points[0];
    let last = route.points[route.points.len() - 1];
    let slope = (last.0 - first.0) * (last.1 - first.1);
    if slope == 0.0 {
        return vec![];
    }
    anchors(route)
        .into_iter()
        .filter_map(|anchor| anchor.map(|a| corner(a, route, width, slope)))
        .collect()
}

fn corner(anchor: Spot, route: &Route, width: f64, slope: f64) -> Spot {
    let half = stroke(&route.edge);
    let y = if slope > 0.0 {
        anchor.1 - half - BESIDE_GAP
    } else {
        anchor.1 + half + 1.0 + FONT_SIZE
    };
    (anchor.0 + half + BESIDE_GAP + width / 2.0, y)
}

fn anchors(route: &Route) -> Vec<Option<Spot>> {
    let mut list = vec![route.label_at];
    for fraction in FRACTIONS {
        list.push(Some(along(&route.points, fraction)));
    }
    list
}

/// The spots beside the edge itself, one per place along it.
fn clear_of(route: &Route, width: f64) -> Vec<Spot> {
    FRACTIONS
        .iter()
        .map(|fraction| {
            let (from, to) = leg(&route.points, *fraction);
            let spot = along(&route.points, *fraction);
            let half = stroke(&route.edge);
            if steep(&from, &to) {
                right_of(&from, &to, &spot, width, half)
            } else {
                above(&from, &to, &spot, width, half)
            }
        })
        .collect()
}

/// Both sides of the edge at finer places along it, nearest the middle
/// first (Ruby's SPREAD order).
fn spread(route: &Route, width: f64) -> Vec<Spot> {
    let mut shares: Vec<f64> = (1..20).map(|step| step as f64 * 0.05).collect();
    shares.sort_by(|a, b| {
        (a - 0.5).abs().partial_cmp(&(b - 0.5).abs()).expect("finite shares")
    });
    let mut spots = vec![];
    for fraction in shares {
        let (from, to) = leg(&route.points, fraction);
        let spot = along(&route.points, fraction);
        let half = stroke(&route.edge);
        let places: [&str; 2] = if steep(&from, &to) { ["right_of", "left_of"] } else { ["above", "below"] };
        for place in places {
            spots.push(match place {
                "right_of" => right_of(&from, &to, &spot, width, half),
                "left_of" => left_of(&from, &to, &spot, width, half),
                "above" => above(&from, &to, &spot, width, half),
                _ => below(&from, &to, &spot, width, half),
            });
        }
    }
    spots
}

fn steep(from: &(f64, f64), to: &(f64, f64)) -> bool {
    (to.1 - from.1).abs() > (to.0 - from.0).abs()
}

fn stroke(edge: &Edge) -> f64 {
    edge.width as f64 / 2.0
}

fn above(from: &Spot, to: &Spot, spot: &Spot, width: f64, half: f64) -> Spot {
    let reach = width / 2.0 + MARGIN;
    let top = top_between(*from, *to, spot.0 - reach, spot.0 + reach);
    (spot.0, top - half - 1.0 - FONT_SIZE / 4.0)
}

fn below(from: &Spot, to: &Spot, spot: &Spot, width: f64, half: f64) -> Spot {
    let reach = width / 2.0 + MARGIN;
    let bottom = bottom_between(*from, *to, spot.0 - reach, spot.0 + reach);
    (spot.0, bottom + half + 1.0 + FONT_SIZE)
}

fn left_of(from: &Spot, to: &Spot, spot: &Spot, width: f64, half: f64) -> Spot {
    let edge = left_between(*from, *to, spot.1 - FONT_SIZE, spot.1 + FONT_SIZE / 4.0);
    (edge - half - BESIDE_GAP - width / 2.0, spot.1 + FONT_SIZE / 3.0)
}

fn right_of(from: &Spot, to: &Spot, spot: &Spot, width: f64, half: f64) -> Spot {
    let edge = right_between(*from, *to, spot.1 - FONT_SIZE, spot.1 + FONT_SIZE / 4.0);
    (edge + half + BESIDE_GAP + width / 2.0, spot.1 + FONT_SIZE / 3.0)
}

/// Whether the box lies within reach of the route's line.
fn reaches(boxed: &[f64; 4], route: &Route) -> bool {
    let reach = stroke(&route.edge) + REACH;
    route
        .points
        .windows(2)
        .any(|pair| segment_box_within(boxed, pair[0], pair[1], reach))
}

/// Whether the box lies across any part of the route's line, or within
/// `gap` of it.
fn covers(boxed: &[f64; 4], route: &Route, gap: f64) -> bool {
    let pad = stroke(&route.edge) + gap;
    let padded = [boxed[0] - pad, boxed[1] - pad, boxed[2] + pad, boxed[3] + pad];
    route
        .points
        .windows(2)
        .any(|pair| segment_box_crosses(&padded, pair[0], pair[1]))
}

/// The jar prints a label on the right of the point its line passes
/// through.
fn beside(anchor: Option<Spot>, width: f64) -> Vec<Spot> {
    match anchor {
        Some((x_pos, y_pos)) => vec![(x_pos + width / 2.0 + BESIDE_GAP, y_pos + FONT_SIZE / 3.0)],
        None => vec![],
    }
}

fn along_edge(points: &[(f64, f64)]) -> Vec<Spot> {
    let mut offsets = vec![0.0];
    for step in 1..=MAX_STEPS {
        offsets.push(-(step as f64) * LINE_STEP);
        offsets.push(step as f64 * LINE_STEP);
    }
    let mut spots = vec![];
    for shift in offsets {
        for fraction in FRACTIONS {
            let (x_pos, y_pos) = along(points, fraction);
            spots.push((x_pos, y_pos - RISE + shift));
        }
    }
    spots
}

fn bounds(x_pos: f64, y_pos: f64, width: f64) -> [f64; 4] {
    [
        x_pos - width / 2.0 - MARGIN,
        y_pos - FONT_SIZE,
        x_pos + width / 2.0 + MARGIN,
        y_pos + FONT_SIZE / 4.0,
    ]
}

fn overlap(first: &[f64; 4], second: &[f64; 4]) -> bool {
    first[0] < second[2] && second[0] < first[2] && first[1] < second[3] && second[1] < first[3]
}

fn boxes_area(boxes: &[Placed]) -> Vec<[f64; 4]> {
    boxes
        .iter()
        .map(|boxed| [boxed.x, boxed.y, boxed.x + boxed.width, boxed.y + boxed.height])
        .collect()
}

/// The point `fraction` of the way along the polyline.
pub fn along(points: &[(f64, f64)], fraction: f64) -> Spot {
    let (start, finish, share) = locate(points, fraction);
    lerp(start, finish, share)
}

/// The segment of the polyline `fraction` of the way along it.
fn leg(points: &[(f64, f64)], fraction: f64) -> ((f64, f64), (f64, f64)) {
    let (start, finish, _) = locate(points, fraction);
    (start, finish)
}

fn locate(points: &[(f64, f64)], fraction: f64) -> ((f64, f64), (f64, f64), f64) {
    let segments: Vec<((f64, f64), (f64, f64), f64)> = points
        .windows(2)
        .map(|pair| (pair[0], pair[1], (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1)))
        .collect();
    let total: f64 = segments.iter().map(|(_, _, length)| *length).sum();
    let mut remaining = total * fraction;
    for (index, (start, finish, length)) in segments.iter().enumerate() {
        let last = index == segments.len() - 1;
        if remaining <= *length || last {
            let ratio = if *length == 0.0 { 0.0 } else { remaining / *length };
            return (*start, *finish, ratio);
        }
        remaining -= length;
    }
    (
        segments[segments.len() - 1].0,
        segments[segments.len() - 1].1,
        0.0,
    )
}

fn lerp(start: (f64, f64), finish: (f64, f64), ratio: f64) -> Spot {
    (
        start.0 + (finish.0 - start.0) * ratio,
        start.1 + (finish.1 - start.1) * ratio,
    )
}

/// Whether a line segment passes through a box (Liang-Barsky).
pub fn segment_box_crosses(rect: &[f64; 4], from: Spot, to: Spot) -> bool {
    let mut span = [0.0, 1.0];
    [0usize, 1].iter().all(|&axis| {
        let (start, finish) = if axis == 0 { (from.0, to.0) } else { (from.1, to.1) };
        inside(&mut span, start, finish, rect[axis], rect[axis + 2])
    })
}

/// Whether the segment lies within `gap` of the box.
pub fn segment_box_within(rect: &[f64; 4], from: Spot, to: Spot, gap: f64) -> bool {
    segment_box_distance(rect, from, to) <= gap
}

fn segment_box_distance(rect: &[f64; 4], from: Spot, to: Spot) -> f64 {
    if segment_box_crosses(rect, from, to) {
        return 0.0;
    }
    let corners = [
        [rect[0], rect[1]],
        [rect[2], rect[1]],
        [rect[2], rect[3]],
        [rect[0], rect[3]],
    ];
    let mut best = f64::INFINITY;
    for corner in &corners {
        best = best.min(to_segment((corner[0], corner[1]), from, to));
    }
    for point in [from, to] {
        best = best.min(to_box(point, rect));
    }
    best
}

fn to_box(point: Spot, rect: &[f64; 4]) -> f64 {
    let x = (rect[0] - point.0).max(0.0).max(point.0 - rect[2]);
    let y = (rect[1] - point.1).max(0.0).max(point.1 - rect[3]);
    x.hypot(y)
}

fn to_segment(point: Spot, from: Spot, to: Spot) -> f64 {
    let near = nearest_on(point, from, to);
    (point.0 - near.0).hypot(point.1 - near.1)
}

fn nearest_on(point: Spot, from: Spot, to: Spot) -> Spot {
    let run = (to.0 - from.0, to.1 - from.1);
    let gone = (point.0 - from.0, point.1 - from.1);
    let length = run.0 * run.0 + run.1 * run.1;
    let share = if length == 0.0 {
        0.0
    } else {
        ((gone.0 * run.0 + gone.1 * run.1) / length).clamp(0.0, 1.0)
    };
    (from.0 + share * run.0, from.1 + share * run.1)
}

fn inside(span: &mut [f64; 2], start: f64, finish: f64, low: f64, high: f64) -> bool {
    let delta = finish - start;
    if delta == 0.0 {
        return start >= low && start <= high;
    }
    let mut bounds_pair = [(low - start) / delta, (high - start) / delta];
    bounds_pair.sort_by(|a, b| a.partial_cmp(b).unwrap());
    span[0] = span[0].max(bounds_pair[0]);
    span[1] = span[1].min(bounds_pair[1]);
    span[0] <= span[1]
}

/// Ruby `LabelClearance`: where the line's furthest point under a label
/// sits, per axis.
pub fn top_between(from: Spot, to: Spot, left: f64, right: f64) -> f64 {
    let lowest = from.1.min(to.1);
    [left, right]
        .iter()
        .map(|x| across(from, to, 0, *x, lowest))
        .fold(f64::INFINITY, f64::min)
}

pub fn right_between(from: Spot, to: Spot, top: f64, bottom: f64) -> f64 {
    let widest = from.0.max(to.0);
    [top, bottom]
        .iter()
        .map(|y| across(from, to, 1, *y, widest))
        .fold(f64::NEG_INFINITY, f64::max)
}

pub fn bottom_between(from: Spot, to: Spot, left: f64, right: f64) -> f64 {
    let highest = from.1.max(to.1);
    [left, right]
        .iter()
        .map(|x| across(from, to, 0, *x, highest))
        .fold(f64::NEG_INFINITY, f64::max)
}

pub fn left_between(from: Spot, to: Spot, top: f64, bottom: f64) -> f64 {
    let narrowest = from.0.min(to.0);
    [top, bottom]
        .iter()
        .map(|y| across(from, to, 1, *y, narrowest))
        .fold(f64::INFINITY, f64::min)
}

fn across(from: Spot, to: Spot, axis: usize, value: f64, flat: f64) -> f64 {
    let (start, stop) = if axis == 0 { (from.0, to.0) } else { (from.1, to.1) };
    let span = stop - start;
    let Some(share) = (if span == 0.0 { None } else { Some(((value - start) / span).clamp(0.0, 1.0)) })
    else {
        return flat;
    };
    let (other_from, other_to) = if axis == 0 { (from.1, to.1) } else { (from.0, to.0) };
    other_from + (other_to - other_from) * share
}
