//! The PlantUML activity diagram (`@startuml` with the new or the legacy
//! `(*)` syntax): port of `notation/plantuml/activity/` — its two readers,
//! the ranked column layout, the mark canvas and the SVG renderer over the
//! shared `crate::svg` layer, byte-identical to the Ruby path.

mod layout;
mod marks;
mod metrics;
mod model;
mod parser;
mod renderer;
mod router;
mod style;

use crate::Error;

/// Renders an activity source to SVG bytes.
pub fn render(source: &str) -> Result<String, Error> {
    let diagram = parser::parse(source)?;
    let sheet = style::StyleReader::new().read(source);
    renderer::render(&diagram, &sheet)
}
