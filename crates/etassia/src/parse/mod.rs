//! Stage 3: detect the diagram type and parse its body with the PARG artifact
//! (`artifact/mermaid.json`, built by claricle/etassia-mermaid-parg) through the
//! parsanol engine. The tree equals the Ruby native tree
//! (`Etassia::Parser::Native.parse`) for every type.

mod detect;
mod source;

use crate::Error;
use parsanol::PargArtifact;
use std::sync::OnceLock;

/// The parse tree: parsanol-tree/v2 as JSON. Terminals are slice objects
/// `{value, line, column, offset, length}`; offsets are bytes.
pub type Tree = serde_json::Value;

const MERMAID: &str = include_str!("../../artifact/mermaid.json");
const PLANTUML: &str = include_str!("../../artifact/plantuml.json");

fn mermaid() -> &'static PargArtifact {
    static CELL: OnceLock<PargArtifact> = OnceLock::new();
    CELL.get_or_init(|| PargArtifact::from_json(MERMAID).expect("embedded mermaid artifact loads"))
}

fn plantuml() -> &'static PargArtifact {
    static CELL: OnceLock<PargArtifact> = OnceLock::new();
    CELL.get_or_init(|| PargArtifact::from_json(PLANTUML).expect("embedded plantuml artifact loads"))
}

/// The artifact entry for a detected type: the mermaid entries share the
/// type name, the plantuml entries carry the _diagram suffix.
fn entry_for(diagram_type: &str) -> String {
    if diagram_type == "git" {
        "git_diagram".to_string()
    } else {
        diagram_type.to_string()
    }
}

fn artifact_for(entry: &str) -> Option<&'static PargArtifact> {
    if mermaid().entry_names().contains(&entry) {
        Some(mermaid())
    } else if plantuml().entry_names().contains(&entry) {
        Some(plantuml())
    } else {
        None
    }
}

/// Diagram type name for `source` (same names as the corpus directories,
/// e.g. "pie", "state_diagram"), or None when no type matches. The
/// PlantUML wrappers carry their own @start keyword, checked on the raw
/// source before the mermaid body stripping runs.
pub fn detect(source: &str) -> Option<String> {
    if detect::detect_git(source) {
        return Some("git".to_string());
    }
    detect::detect_body(&source::body(source)).map(str::to_string)
}

/// The diagram body of `source`: preamble (frontmatter, directives, comments) removed.
pub fn body(source: &str) -> String {
    source::body(source)
}

/// Parses `source` with the grammar entry for `diagram_type`. The
/// PlantUML entries parse the raw source (their grammar carries the
/// wrapper); the Mermaid entries parse the preamble-stripped body.
pub fn parse(source: &str, diagram_type: &str) -> Result<Tree, Error> {
    let entry = entry_for(diagram_type);
    let Some(artifact) = artifact_for(&entry) else {
        return Err(Error::Unported(diagram_type.to_string()));
    };
    let body = if std::ptr::eq(artifact, plantuml()) {
        source.to_string()
    } else {
        source::body(source)
    };
    artifact.parse_shape(&entry, &body).map_err(|e| match e {
        parsanol::PargError::ParseWire { line, column, .. } => Error::Parse {
            class: "ParseError".into(),
            message: e.to_string(),
            line,
            col: column,
        },
        other => Error::Parse {
            class: "ParseError".into(),
            message: other.to_string(),
            line: 0,
            col: 0,
        },
    })
}
