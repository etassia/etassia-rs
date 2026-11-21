//! Stage 4 dispatch. Owner: integration. Each type module is owned by its builder.
pub mod activity_puml;
pub mod architecture;
pub mod block;
pub mod c4;
pub mod class_diagram;
pub mod component;
pub mod ditaa;
pub mod er_diagram;
pub mod flowchart;
pub mod gantt;
pub mod gantt_puml;
pub mod git_log;
pub mod gitgraph;
pub mod info;
pub mod kanban;
pub mod mindmap;
pub mod mindmap_puml;
pub mod packet;
pub mod pie;
pub mod quadrant;
pub mod radar;
pub mod state_puml;
pub mod requirement;
pub mod sankey;
pub mod sequence;
pub mod sequence_puml;
pub mod state_diagram;
pub mod state_puml;
pub mod class_puml;
pub mod timeline;
pub mod timing_puml;
pub mod treemap;
pub mod user_journey;
pub mod xychart;
use crate::Error;
/// Render `source` to SVG, or Err(Unported) when the type has no Rust renderer.
///
/// Ruby's `Source.normalize` turns every `\r\n` and lone `\r` into `\n` before any
/// type sees the source, so the renderers below only ever read LF input.
pub fn render(source: &str) -> Result<String, Error> {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let source = normalized.as_str();
    let ty = crate::detect(source).ok_or_else(|| Error::Unported("unknown".into()))?;
    match ty.as_str() {
        "component" => component::render(source, component::Family::Component),
        "usecase" => component::render(source, component::Family::Usecase),
        "timing" => timing_puml::render(source),
        "ditaa" => ditaa::render(source),
        "git" => git_log::render(source),
        "info" => info::render(source),
        "pie" => pie::render(source),
        "packet" => packet::render(source),
        "sankey" => sankey::render(source),
        "timeline" => timeline::render(source),
        "treemap" => treemap::render(source),
        "quadrant" => quadrant::render(source),
        "radar" => radar::render(source),
        "xychart" => xychart::render(source),
        "user_journey" => user_journey::render(source),
        "kanban" => kanban::render(source),
        "mindmap" => {
            // The Mermaid mindmap and PlantUML's Tree mindmap share the
            // type name; the wrapper picks the PlantUML lane.
            if crate::parse::detect_mindmap(source) {
                mindmap_puml::render(source)
            } else {
                mindmap::render(source)
            }
        }
        // The gantt type names both mermaid's gantt DSL and the PlantUML
        // @startgantt wrapper; the wrapper picks the PlantUML lane.
        "activity" => activity_puml::render(source),
        "gantt" => {
            if crate::parse::is_puml_gantt(source) {
                gantt_puml::render(source)
            } else {
                gantt::render(source)
            }
        }
        "gitgraph" => gitgraph::render(source),
        "er_diagram" => er_diagram::render(source),
        "requirement" => requirement::render(source),
        "c4" => c4::render(source),
        "block" => block::render(source),
        "architecture" => architecture::render(source),
        // The sequence type names both mermaid's sequenceDiagram and the
        // PlantUML family; a source the family claims picks the PlantUML
        // lane.
        "sequence" => {
            if crate::parse::detect_sequence(source) {
                sequence_puml::render(source)
            } else {
                sequence::render(source)
            }
        }
        // PlantUML's own state diagram (@startuml with state statements).
        "state" => state_puml::render(source),
        "state_diagram" => state_diagram::render(source),
        "class_diagram" => class_diagram::render(source),
        // The PlantUML class lane: any PlantUML opener no earlier family
        // claimed (Ruby reads the class reader as its fallback).
        "class" => class_puml::render(source),
        "flowchart" => flowchart::render(source),
        other => Err(Error::Unported(other.to_string())),
    }
}
