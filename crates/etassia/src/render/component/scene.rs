//! What one scope's layout came to, and the geometry records the
//! renderer consumes. Port of `node_graph/scene.rb`.

use super::model::Edge;

/// A node placed at final coordinates. Only nodes are placed; waypoints
/// exist only inside the ranking.
#[derive(Clone, Debug)]
pub struct Placed {
    pub node: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub depth: usize,
}

/// A link routed between two placed nodes, as a list of points.
/// `label_at` is the point the line passes through at its label.
#[derive(Clone, Debug)]
pub struct Route {
    pub edge: Edge,
    pub points: Vec<(f64, f64)>,
    pub label_at: Option<(f64, f64)>,
}

/// A laid-out diagram.
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub placed: Vec<Placed>,
    pub routes: Vec<Route>,
    pub title: Option<String>,
}
