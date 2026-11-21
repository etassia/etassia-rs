//! PlantUML class renderer: port of Ruby `Notation::PlantUML::{Parser,
//! DiagramBuilder, Layout, Renderer}` (`etassia/etassia` lib/etassia/
//! notation/plantuml). The pipeline runs the grammar's reader-faithful
//! gate, the line reader, the placement and the SVG drawing; anything
//! not reproduced declines so the caller falls back to Ruby.
mod arrow;
mod captions;
mod class_header;
mod chrome;
mod draw;
mod edges;
mod frames;
mod layout;
mod measures;
mod model;
mod notes;
mod portion;
mod qualified;
mod rank_chain;
mod rank_hierarchy;
mod reader;
mod reader_support;
mod scene;
mod sheet;
mod style;

use crate::Error;

pub(crate) fn render(source: &str) -> Result<String, Error> {
    // The grammar gate: the artifact's reader-faithful language. The
    // class reader below is the source of the diagram; the gate refuses
    // what the Ruby reader refuses (etassia-grammar class_diagram).
    crate::parse::parse(source, "class")?;
    let diagram = reader::read(source)?;
    let sheet = style::Sheet::read(source);
    let scene = layout::scene(&diagram);
    draw::render(&scene, &sheet)
}

/// Scratch hook.
#[doc(hidden)]
pub fn format_g_pub(value: f64) -> String {
    layout::format_g(value)
}
