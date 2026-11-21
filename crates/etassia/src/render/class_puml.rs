//! PlantUML class renderer: port of Ruby `Notation::PlantUML::{Parser,
//! DiagramBuilder, Layout, Renderer}` (etassia/etassia lib/etassia/
//! notation/plantuml). Port in progress; every path declines until it is
//! byte-identical, so the caller falls back to Ruby.
use crate::Error;

pub(crate) fn render(_source: &str) -> Result<String, Error> {
    Err(Error::Unported("class".into()))
}
