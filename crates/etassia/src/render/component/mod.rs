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

/// Renders a component-family source to SVG bytes.
pub fn render(source: &str) -> Result<String, Error> {
    let tree = crate::parse::parse(source, "component")?;
    let entries = parser::entry_entries(&tree);
    let graph = parser::Parser::new(parser::DirectionInput::TopToBottom, NodeKind::Interface)
        .parse_entries(entries)?;
    let scene = layout::Layout::new(&graph).run();
    Ok(shapes::render_document(&graph, &scene))
}
