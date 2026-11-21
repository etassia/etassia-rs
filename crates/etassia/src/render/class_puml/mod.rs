//! PlantUML class renderer: port of Ruby `Notation::PlantUML::{Parser,
//! DiagramBuilder, Layout, Renderer}` (`etassia/etassia` lib/etassia/
//! notation/plantuml). Every path declines until it is byte-identical to
//! the Ruby output, so the caller falls back to Ruby.
mod arrow;
mod model;
mod portion;
mod reader;
mod reader_support;
mod sheet;
mod style;

pub(crate) fn render(source: &str) -> Result<String, Error> {
    // The grammar gate: the artifact's reader-faithful language. The
    // class reader below is the source of the diagram; the gate refuses
    // what the Ruby reader refuses (etassia-grammar class_diagram).
    crate::parse::parse(source, "class")?;
    let diagram = reader::read(source)?;
    let _ = diagram;
    Err(Error::Unported("class".into()))
}

use crate::Error;

/// Scratch hook for the reader parity check (tests/scratch_class.rs).
#[doc(hidden)]
pub fn class_puml_reader_read(source: &str) -> Result<crate::render::class_puml::model::Diagram, crate::Error> {
    reader::read(source)
}

/// Scratch probes.
#[doc(hidden)]
pub mod probe {
    pub fn directive_matches_probe(text: &str) -> bool {
        crate::render::class_puml::reader_support::directive_matches(text)
    }
    pub fn relation_matches_probe(text: &str) -> bool {
        crate::render::class_puml::reader::relation_matches(text)
    }
}
