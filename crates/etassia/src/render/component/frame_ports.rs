//! Ports on a container's frame: `portin` along the top edge, `portout`
//! along the bottom, each square straddling the edge, as the PlantUML jar
//! draws them. Port of `node_graph/frame_ports.rb`.

use std::collections::HashMap;

use super::model::{Graph, NodeKind};
use super::scene::Placed;

pub const SIDE: f64 = 12.0;
pub const GAP: f64 = 12.0;
/// From a container's border to the children inside it, when ports sit on
/// that side: the jar ranks the children a full rank gap below the port
/// row.
pub const REACH: f64 = 66.0;

/// Whether `node` sits on the frame of its container.
pub fn on_frame(graph: &Graph, node: usize) -> bool {
    let node = &graph.nodes[node];
    node.parent.is_some() && matches!(node.kind, NodeKind::Portin | NodeKind::Portout)
}

#[derive(Default)]
pub struct FramePorts;

impl FramePorts {
    pub fn new() -> FramePorts {
        FramePorts
    }

    /// The frame width the busier side needs.
    pub fn width(&self, graph: &Graph, ports: &[usize], sizes: &HashMap<usize, (f64, f64)>) -> f64 {
        let mut by_kind: HashMap<NodeKind, f64> = HashMap::new();
        for port in ports {
            let kind = graph.nodes[*port].kind;
            let total = sizes.get(port).map(|s| s.0).unwrap_or(0.0) + GAP;
            *by_kind.entry(kind).or_insert(0.0) += total;
        }
        by_kind.values().copied().fold(0.0f64, f64::max)
    }

    /// The ports, spread along their edge.
    pub fn place(
        &self,
        graph: &Graph,
        frame: &Placed,
        ports: &[usize],
        sizes: &HashMap<usize, (f64, f64)>,
        depth: usize,
    ) -> Vec<Placed> {
        let mut placed = vec![];
        for kind in [NodeKind::Portin, NodeKind::Portout] {
            let side: Vec<usize> = ports
                .iter()
                .copied()
                .filter(|port| graph.nodes[*port].kind == kind)
                .collect();
            let count = side.len();
            if count == 0 {
                continue;
            }
            for (index, port) in side.iter().enumerate() {
                let at = frame.x + frame.width * (index as f64 + 0.5) / count as f64;
                let (width, height) = sizes.get(port).copied().unwrap_or((0.0, 0.0));
                let edge_y = if kind == NodeKind::Portin { frame.y } else { frame.y + frame.height };
                let reach = if kind == NodeKind::Portin {
                    height - SIDE / 2.0
                } else {
                    SIDE / 2.0
                };
                placed.push(Placed {
                    node: *port,
                    x: at - width / 2.0,
                    y: edge_y - reach,
                    width,
                    height,
                    depth,
                });
            }
        }
        placed
    }

    /// The square a link to `box` ends at.
    pub fn terminal(&self, graph: &Graph, boxed: &Placed) -> Placed {
        let top = if graph.nodes[boxed.node].kind == NodeKind::Portin {
            boxed.height - SIDE
        } else {
            0.0
        };
        Placed {
            node: boxed.node,
            x: boxed.x + (boxed.width - SIDE) / 2.0,
            y: boxed.y + top,
            width: SIDE,
            height: SIDE,
            depth: boxed.depth,
        }
    }
}
