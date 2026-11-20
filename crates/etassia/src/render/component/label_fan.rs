//! The labels of links that would print over one another: each bent
//! through its own place in the rank between the ends. Port of
//! `node_graph/{label_fan,label_waypoint}.rb`.

use super::metrics::face_width;
use super::scene::Route;

pub const LINE: f64 = 14.0;
pub const FONT_SIZE: f64 = 12.0;
/// How far toward the source from its target the label sits.
pub const SHARE: f64 = 0.3;
/// A label this close across to its target stays on the line.
pub const MIN_SPREAD: f64 = 12.0;

/// Bends the crowded labelled links of `routes` through their label
/// points, in place.
pub fn label_fan(routes: &mut [Route], axis: usize) {
    let labelled: Vec<usize> = (0..routes.len()).filter(|at| labelled(&routes[*at])).collect();
    let crowded: Vec<usize> = labelled
        .iter()
        .copied()
        .filter(|at| {
            labelled
                .iter()
                .any(|other| other != at && meet(&routes[*at], &routes[*other], axis))
        })
        .collect();
    for at in crowded {
        bend(&mut routes[at], axis);
    }
}

fn labelled(route: &Route) -> bool {
    let edge = &route.edge;
    let has_label = edge.label.as_deref().map(|l| !l.is_empty()).unwrap_or(false);
    has_label && route.points.len() == 2 && !edge.pointer
}

fn meet(first: &Route, second: &Route, axis: usize) -> bool {
    let across = (middle(first)[1 - axis] - middle(second)[1 - axis]).abs();
    let along = (middle(first)[axis] - middle(second)[axis]).abs();
    along < LINE && across < (width(first) + width(second)) / 2.0
}

fn middle(route: &Route) -> [f64; 2] {
    let mut sum = [0.0, 0.0];
    for point in &route.points {
        sum[0] += point.0;
        sum[1] += point.1;
    }
    [sum[0] / 2.0, sum[1] / 2.0]
}

fn width(route: &Route) -> f64 {
    route
        .edge
        .label
        .as_deref()
        .map(|label| face_width(label, FONT_SIZE))
        .unwrap_or(0.0)
}

fn bend(route: &mut Route, axis: usize) {
    let Some(anchor) = label_waypoint(&route.points.clone(), axis) else { return };
    let first = route.points[0];
    let last = route.points[route.points.len() - 1];
    route.points = vec![first, anchor, last];
    route.label_at = Some(anchor);
}

/// The point a labelled spline passes through so its label sits side by
/// side above its target; `None` when the link has bends of its own or
/// runs straight.
pub fn label_waypoint(points: &[(f64, f64)], axis: usize) -> Option<(f64, f64)> {
    if points.len() != 2 {
        return None;
    }
    let (first, last) = (points[0], points[1]);
    let first = [first.0, first.1];
    let last = [last.0, last.1];
    let spread = first[1 - axis] - last[1 - axis];
    if spread.abs() < MIN_SPREAD {
        return None;
    }
    let share = spread * SHARE;
    let mut point = [0.0, 0.0];
    point[axis] = (first[axis] + last[axis]) / 2.0;
    point[1 - axis] = last[1 - axis] + share;
    Some((point[0], point[1]))
}
