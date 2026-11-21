//! The PlantUML sequence diagram: participants, messages, notes, blocks,
//! dividers, spacing and delays. Port of `notation/plantuml/sequence/`
//! (parser, layout, walker, renderer).

mod draw;
mod geometry;
mod layout;
mod model;
mod read;
mod style;
mod walker;

use crate::Error;

pub fn render(source: &str) -> Result<String, Error> {
    let tree = crate::parse::parse(source, "sequence")?;
    let diagram = read::diagram(&tree, source)?;
    let scene = layout::scene(&diagram)?;
    Ok(draw::draw(&scene))
}
