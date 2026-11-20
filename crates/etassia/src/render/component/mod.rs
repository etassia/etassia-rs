//! The node-and-link engine behind the use case, component and deployment
//! notations: a reader/parser, a ranked layout and a renderer. Port of
//! `lib/etassia/notation/plantuml/node_graph*` plus its shared helpers
//! (`label_clearance`, `segment_box`, `creole`).

mod archimate;
mod archimate_sprites;
mod creole;
mod detour;
mod frame_ports;
mod heads;
mod label_fan;
mod label_placer;
mod layout;
mod metrics;
mod model;
mod parser;
mod paths;
mod ranking;
mod reader;
mod references;
mod scene;
mod shapes;

use crate::Error;
use model::NodeKind;

/// Which NodeGraph notation reads the source: the component family
/// defaults bare link ends to interfaces, the use case family to actors.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Component,
    Usecase,
}

impl Family {
    fn default_kind(self) -> NodeKind {
        match self {
            Family::Component => NodeKind::Interface,
            Family::Usecase => NodeKind::Actor,
        }
    }

    fn detect_type(self) -> &'static str {
        match self {
            Family::Component => "component",
            Family::Usecase => "usecase",
        }
    }
}

/// Renders a component-family source to SVG bytes.
pub fn render(source: &str, family: Family) -> Result<String, Error> {
    let tree = crate::parse::parse(source, family.detect_type())?;
    let entries = parser::entry_entries(&tree);
    let graph = parser::Parser::new(parser::DirectionInput::TopToBottom, family.default_kind())
        .parse_entries(entries)?;
    let scene = layout::Layout::new(&graph).run();
    Ok(shapes::render_document(&graph, &scene))
}

#[test]
fn probe_dump_usecase_graph() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/usecase/resources.vega.tikz.tikz-scale-down--f8416a40f77a.mmd")).unwrap();
    let tree = crate::parse::parse(&src, "usecase").unwrap();
    let entries = parser::entry_entries(&tree);
    for entry in &entries {
        println!("entry {}", entry.text.escape_debug());
    }
    let graph = parser::Parser::new(parser::DirectionInput::TopToBottom, NodeKind::Actor)
        .parse_entries(entries)
        .unwrap();
    let scene = layout::Layout::new(&graph).run();
    for boxed in &scene.placed {
        println!("placed {} x={} y={} w={} h={} d={}", graph.nodes[boxed.node].id, boxed.x, boxed.y, boxed.width, boxed.height, boxed.depth);
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        println!("node {} {:?} lines={:?} parent={:?}", node.id, node.kind, node.lines, node.parent.map(|p| graph.nodes[p].id.clone()));
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        println!("edge{} {} -> {} label={:?} step={:?} back={} pointer={}", index, graph.nodes[edge.from].id, graph.nodes[edge.to].id, edge.label, edge.rank_step, edge.backwards, edge.pointer);
    }
}

#[test]
fn probe_reach() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/usecase/resources.vega.tikz.tikz-scale-down--f8416a40f77a.mmd")).unwrap();
    let tree = crate::parse::parse(&src, "usecase").unwrap();
    let graph = parser::Parser::new(parser::DirectionInput::TopToBottom, NodeKind::Actor)
        .parse_entries(parser::entry_entries(&tree))
        .unwrap();
    let reach = metrics::label_reach(&graph, None);
    for (item, width) in &reach {
        println!("reach {:?} = {}", item, width);
    }
}
