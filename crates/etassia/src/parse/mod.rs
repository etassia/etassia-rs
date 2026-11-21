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
    CELL.get_or_init(|| {
        PargArtifact::from_json(PLANTUML).expect("embedded plantuml artifact loads")
    })
}

/// The artifact entry for a detected type: the mermaid entries share the
/// type name, the plantuml entries carry the _diagram suffix.
fn entry_for(diagram_type: &str) -> String {
    if diagram_type == "git" {
        "git_diagram".to_string()
    } else if diagram_type == "ditaa" {
        "ditaa_diagram".to_string()
    } else if diagram_type == "activity" {
        "activity_diagram".to_string()
    } else if diagram_type == "component" || diagram_type == "usecase" || diagram_type == "timing" {
        // The NodeGraph family shares one grammar entry: any @start opens
        // the first diagram, any @end closes it.
        "component_diagram".to_string()
    } else if diagram_type == "state" {
        "state_diagram".to_string()
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
    if detect::detect_ditaa(source) {
        return Some("ditaa".to_string());
    }
    if detect::detect_git(source) {
        return Some("git".to_string());
    }
    // The mermaid body patterns claim their own types first (a mermaid
    // gantt is a mermaid kind); the PlantUML @startgantt and @startmindmap
    // wrappers run next, before the component family lanes, whose markers
    // would otherwise steal their sources - a Mermaid source claims itself
    // first, then the tagged PlantUML kinds before the families.
    detect::detect_body(&source::body(source))
        .map(str::to_string)
        .or_else(|| detect::detect_gantt(source).then(|| "gantt".to_string()))
        .or_else(|| detect::detect_mindmap(source).then(|| "mindmap".to_string()))
        .or_else(|| detect::detect_timing(source).then(|| "timing".to_string()))
        // The state lane sits between the Further group and Usecase: the
        // Ruby families order (Further, State, Usecase, Component, then
        // Activity).
        .or_else(|| detect::detect_state(source).then(|| "state".to_string()))
        .or_else(|| detect::detect_usecase(source).then(|| "usecase".to_string()))
        .or_else(|| detect::detect_component(source).then(|| "component".to_string()))
        // The activity lane runs after the component family, the Ruby
        // families order (Further, State, Usecase, Component, Activity):
        // the markers the earlier families claim never reach it.
        .or_else(|| detect::detect_activity(source).then(|| "activity".to_string()))
        // The sequence family runs after component in the Ruby families
        // order (Further, State, Usecase, Component, Activity, Entity,
        // Sequence, Objects); the kinds ahead of it are not ported yet.
        .or_else(|| detect::detect_sequence(source).then(|| "sequence".to_string()))
        // The class lane is what is left: any PlantUML opener no earlier
        // family claimed (Ruby runs the class reader as its fallback).
        .or_else(|| detect::detect_class(source).then(|| "class".to_string()))
}

/// Whether the raw source opens with the PlantUML gantt wrapper: the
/// gantt type names both mermaid's gantt DSL and this wrapper, and the
/// artifact, entry and renderer pick on it.
pub fn is_puml_gantt(source: &str) -> bool {
    detect::detect_gantt(source)
}

/// The diagram body of `source`: preamble (frontmatter, directives, comments) removed.
pub fn body(source: &str) -> String {
    source::body(source)
}

/// Whether the raw source opens with the PlantUML mindmap wrapper; the
/// render dispatch uses it to split the shared mindmap type name.
pub fn detect_mindmap(source: &str) -> bool {
    detect::detect_mindmap(source)
}

/// Whether the source reads as the PlantUML sequence family; the render
/// dispatch uses it to split the shared sequence type name.
pub fn detect_sequence(source: &str) -> bool {
    detect::detect_sequence(source)
}

/// Parses `source` with the grammar entry for `diagram_type`. The
/// PlantUML entries parse the raw source (their grammar carries the
/// wrapper); the Mermaid entries parse the preamble-stripped body.
pub fn parse(source: &str, diagram_type: &str) -> Result<Tree, Error> {
    // The gantt type names both mermaid's gantt DSL and the PlantUML
    // @startgantt wrapper; the wrapper claims the plantuml artifact's
    // gantt_diagram entry over the raw source.
    if diagram_type == "gantt" && detect::detect_gantt(source) {
        return parse_with(plantuml(), "gantt_diagram", source);
    }
    // The PlantUML mindmap shares the Mermaid type name; the wrapper
    // claims the plantuml artifact's mindmap_diagram entry the same way.
    if diagram_type == "mindmap" && detect::detect_mindmap(source) {
        return parse_with(plantuml(), "mindmap_diagram", source);
    }
    // The state type is PlantUML's own (@startuml with state statements);
    // the shared state_diagram name also belongs to Mermaid, so the raw
    // source claims the plantuml artifact's state_diagram entry directly.
    if diagram_type == "state" {
        return parse_with(plantuml(), "state_diagram", source);
    }
    // The sequence type names Mermaid's sequenceDiagram and the PlantUML
    // family; a source the family claims parses with the plantuml
    // artifact's sequence_diagram entry over the raw source.
    if diagram_type == "sequence" && detect::detect_sequence(source) {
        return parse_with(plantuml(), "sequence_diagram", source);
    }
    // The class lane's own type name: the plantuml artifact carries the
    // class_diagram entry over the raw source. The mermaid classDiagram
    // keeps its own "class_diagram" type, so the two never meet.
    if diagram_type == "class" {
        return parse_with(plantuml(), "class_diagram", source);
    }
    let entry = entry_for(diagram_type);
    let Some(artifact) = artifact_for(&entry) else {
        return Err(Error::Unported(diagram_type.to_string()));
    };
    let body = if std::ptr::eq(artifact, plantuml()) {
        source.to_string()
    } else {
        source::body(source)
    };
    parse_with(artifact, &entry, &body)
}

fn parse_with(artifact: &'static PargArtifact, entry: &str, body: &str) -> Result<Tree, Error> {
    artifact.parse_shape(entry, body).map_err(|e| match e {
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
