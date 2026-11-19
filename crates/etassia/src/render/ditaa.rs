//! Ditaa renderer: port of Ruby `Diagram::Ditaa` (TextGrid -> CellSet ->
//! Builder) and `PlantUML::DitaaRenderer`. The ASCII-art interpretation is
//! algorithmic, so the grammar carries the wrapper and the verbatim body
//! and this module performs the shape pipeline the Ruby side performs.
//!
//! Ported in stages: the flags and the grid first, then the shape
//! pipeline. Until the pipeline lands the renderer declines, and the
//! engine falls back to Ruby exactly as it does for every unported
//! construct.
use crate::Error;

pub fn render(source: &str) -> Result<String, Error> {
    crate::parse::parse(source, "ditaa")?;
    Err(Error::Unported("ditaa".into()))
}
