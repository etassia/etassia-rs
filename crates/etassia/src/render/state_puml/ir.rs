//! The notation-neutral reading of the state model. Port of
//! `lib/etassia/notation/mermaid/ir_adapters/state_diagram.rb` (the
//! adapter runs for the PlantUML diagram too, through the state layout's
//! `build_graph`) and of `transform_states`/`transform_transitions` from
//! `lib/etassia/layout/state_diagram.rb` with the PlantUML overrides.

use super::reader::Diagram;
use super::Look;
use crate::text::{self, Face};
use std::collections::HashMap;

/// An IR node: the diagram's entities and their semantics.
#[derive(Clone)]
pub struct IrNode {
    pub id: String,
    pub label: Option<String>,
    pub role: String,
    pub parent_id: Option<String>,
}

/// An IR edge.
#[derive(Clone)]
pub struct IrEdge {
    pub id: String,
    pub source_id: Option<String>,
    pub target_id: Option<String>,
    pub parent_id: Option<String>,
    pub label: Option<String>,
}

/// The IR graph.
#[derive(Default)]
pub struct IrGraph {
    pub nodes: Vec<IrNode>,
    pub edges: Vec<IrEdge>,
}

fn state_role(state_type: &str) -> &'static str {
    match state_type {
        "start" => "initial_state",
        "end" => "final_state",
        "choice" => "choice_state",
        "fork" => "fork_state",
        "join" => "join_state",
        _ => "state",
    }
}

/// A fresh id: the preferred one, or `preferred_2`, `preferred_3`, ...
/// (Ruby `reserve_id`).
fn reserve_id(preferred: &str, occupied: &mut Vec<String>) -> String {
    let preferred = if preferred.is_empty() { "item" } else { preferred };
    let mut candidate = preferred.to_string();
    let mut suffix = 2;
    while occupied.iter().any(|id| id == &candidate) {
        candidate = format!("{preferred}_{suffix}");
        suffix += 1;
    }
    occupied.push(candidate.clone());
    candidate
}

fn semantic_nodes(
    parent_id: &str,
    values: Vec<(&'static str, Option<String>)>,
    occupied: &mut Vec<String>,
) -> Vec<IrNode> {
    values
        .into_iter()
        .filter_map(|(role, value)| {
            value.map(|value| IrNode {
                id: reserve_id(&format!("{parent_id}_{role}"), occupied),
                label: Some(value),
                role: role.to_string(),
                parent_id: Some(parent_id.to_string()),
            })
        })
        .collect()
}

/// One state of the model: its values and its children's positions in the
/// entries list, preorder.
struct Entry {
    id: String,
    label: Option<String>,
    state_type: String,
    description: Option<String>,
    descriptions: Vec<String>,
    children: Vec<usize>,
    parent_entry: Option<usize>,
}

fn collect_entries(diagram: &Diagram) -> Vec<Entry> {
    fn walk(
        states: &[super::reader::Node],
        parent: Option<usize>,
        out: &mut Vec<Entry>,
    ) {
        for state in states {
            let entry = out.len();
            out.push(Entry {
                id: state.id.clone(),
                label: state.label.clone(),
                state_type: state.state_type.clone(),
                description: state.description.clone(),
                descriptions: state.descriptions.clone(),
                children: vec![],
                parent_entry: parent,
            });
            if let Some(p) = parent {
                out[p].children.push(entry);
            }
            walk(&state.children, Some(entry), out);
        }
    }
    let mut out = vec![];
    walk(&diagram.states, None, &mut out);
    out
}

/// The IR graph of a state diagram (Ruby `IRAdapters::StateDiagram.call`).
pub fn ir_graph(diagram: &Diagram) -> IrGraph {
    let mut occupied: Vec<String> = vec![];
    let entries = collect_entries(diagram);

    // The IR id of each state entry, in entry order.
    let ir_ids: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let preferred = if entry.id.is_empty() {
                format!("state_{index}")
            } else {
                entry.id.clone()
            };
            reserve_id(&preferred, &mut occupied)
        })
        .collect();
    // The written id -> IR id of the first entry that carries it.
    let mut endpoints: HashMap<String, String> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        endpoints
            .entry(entry.id.clone())
            .or_insert_with(|| ir_ids[index].clone());
    }

    let mut nodes: Vec<IrNode> = vec![];
    for (index, entry) in entries.iter().enumerate() {
        let id = &ir_ids[index];
        nodes.push(IrNode {
            id: id.clone(),
            label: entry.label.clone(),
            role: state_role(&entry.state_type).to_string(),
            parent_id: entry.parent_entry.map(|p| ir_ids[p].clone()),
        });
        let mut values: Vec<(&'static str, Option<String>)> = vec![
            ("original_identifier", if entry.id.is_empty() { None } else { Some(entry.id.clone()) }),
            ("declared_state_type", Some(entry.state_type.clone())),
            ("latest_description", entry.description.clone()),
            ("sequence_index", Some(index.to_string())),
        ];
        for d in &entry.descriptions {
            values.push(("display_text", Some(d.clone())));
        }
        nodes.extend(semantic_nodes(id, values, &mut occupied));
    }
    for (index, note) in diagram.notes.iter().enumerate() {
        let id = reserve_id(&note.id, &mut occupied);
        nodes.push(IrNode {
            id: id.clone(),
            label: Some(note.text.clone()),
            role: "annotation".to_string(),
            parent_id: endpoints.get(&note.target_id).cloned(),
        });
        let values: Vec<(&'static str, Option<String>)> = vec![
            ("original_identifier", Some(note.id.clone())),
            ("placement_side", Some(note.position.clone())),
        ];
        nodes.extend(semantic_nodes(&id, values, &mut occupied));
    }
    let mut edges: Vec<IrEdge> = vec![];
    for (index, transition) in diagram.transitions.iter().enumerate() {
        let details_id = reserve_id(&format!("transition_{index}_details"), &mut occupied);
        nodes.push(IrNode {
            id: details_id.clone(),
            label: None,
            role: "transition_details".to_string(),
            parent_id: None,
        });
        let trigger = transition.trigger.clone().filter(|t| !t.is_empty());
        let values: Vec<(&'static str, Option<String>)> = vec![
            ("trigger", trigger.clone()),
            ("guard_condition", None),
            (
                "transition_identifier",
                Some(format!("{}_to_{}", transition.from_id, transition.to_id)),
            ),
            ("sequence_index", Some(index.to_string())),
        ];
        nodes.extend(semantic_nodes(&details_id, values, &mut occupied));
        let id = reserve_id(&format!("transition_{index}"), &mut occupied);
        edges.push(IrEdge {
            id,
            source_id: endpoints.get(&transition.from_id).cloned(),
            target_id: endpoints.get(&transition.to_id).cloned(),
            parent_id: Some(details_id),
            label: trigger,
        });
    }
    let settings_id = reserve_id("diagram_settings", &mut occupied);
    nodes.push(IrNode {
        id: settings_id.clone(),
        label: None,
        role: "diagram_settings".to_string(),
        parent_id: None,
    });
    let values: Vec<(&'static str, Option<String>)> =
        vec![("layout_direction", Some(diagram.direction.clone()))];
    nodes.extend(semantic_nodes(&settings_id, values, &mut occupied));

    IrGraph { nodes, edges }
}

// ---- the ELK-shaped graph (layout/state_diagram.rb build_graph) ----

/// A label of a graph node or edge: the text with its measured box.
#[derive(Clone, Debug)]
pub struct GLabel {
    pub text: String,
    pub width: f64,
    pub height: f64,
}

/// The metadata bag of a graph node.
#[derive(Clone, Debug, Default)]
pub struct GMeta {
    pub state_type: String,
    pub shape_type: String,
    pub description: Option<String>,
    pub cluster: bool,
    pub note_target: Option<String>,
    pub note_position: Option<String>,
    pub shape_radius: Option<f64>,
    pub trigger: Option<String>,
    pub guard_condition: Option<String>,
}

/// A node of the graph hash.
#[derive(Clone, Debug, Default)]
pub struct GNode {
    pub id: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub labels: Vec<GLabel>,
    pub meta: GMeta,
    pub children: Vec<GNode>,
}

/// One section of an edge route.
#[derive(Clone, Debug)]
pub struct Section {
    pub start_point: (f64, f64),
    pub end_point: (f64, f64),
    pub bend_points: Vec<(f64, f64)>,
}

/// An edge of the graph hash.
#[derive(Clone, Debug)]
pub struct GEdge {
    /// The transition identifier the scene shows (`from_to`).
    pub id: String,
    /// The internal identity the level graphs and routes key on (Ruby's
    /// `object_id`).
    pub oid: usize,
    pub sources: Vec<String>,
    pub targets: Vec<String>,
    pub labels: Vec<GLabel>,
    pub sections: Option<Vec<Section>>,
    /// The point the label sits at (Ruby `edge[:label_at]`).
    pub label_at: Option<(f64, f64)>,
    pub trigger: Option<String>,
    pub guard_condition: Option<String>,
}

/// The graph the placer places and the scene reads.
#[derive(Default)]
pub struct Graph {
    pub id: String,
    pub children: Vec<GNode>,
    pub edges: Vec<GEdge>,
}

const STATE_TYPES: &[(&str, &str)] = &[
    ("state", "normal"),
    ("initial_state", "start"),
    ("final_state", "end"),
    ("choice_state", "choice"),
    ("fork_state", "fork"),
    ("join_state", "join"),
];

fn state_type_of(role: &str) -> Option<&'static str> {
    STATE_TYPES.iter().find(|(r, _)| *r == role).map(|(_, t)| *t)
}

/// The semantics of one IR parent: its child nodes.
fn semantics_of(nodes: &[IrNode]) -> HashMap<Option<String>, Vec<usize>> {
    let mut out: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        out.entry(node.parent_id.clone()).or_default().push(i);
    }
    out
}

fn semantic_value(
    semantics: &HashMap<Option<String>, Vec<usize>>,
    nodes: &[IrNode],
    parent_id: &str,
    role: &str,
) -> Option<String> {
    semantic_values(semantics, nodes, parent_id, role).into_iter().next()
}

fn semantic_values(
    semantics: &HashMap<Option<String>, Vec<usize>>,
    nodes: &[IrNode],
    parent_id: &str,
    role: &str,
) -> Vec<String> {
    semantics
        .get(&Some(parent_id.to_string()))
        .map(|children| {
            children
                .iter()
                .filter(|&&i| nodes[i].role == role)
                .filter_map(|&i| nodes[i].label.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Ruby `TextMeasurement.measure` (Arial).
fn measure_text(text: &str, size: f64) -> (f64, f64) {
    let m = text::measure(text, size, Face::Arial);
    (m.width, m.height)
}

/// The texts a state draws (layout/state_diagram.rb `state_texts`).
fn state_texts(
    descriptions: &[String],
    label: &Option<String>,
    description: &Option<String>,
) -> Vec<String> {
    let kept: Vec<String> =
        descriptions.iter().filter(|d| !d.is_empty()).cloned().collect();
    if !kept.is_empty() {
        return kept;
    }
    [label, description]
        .into_iter()
        .flatten()
        .filter(|t| !t.is_empty())
        .cloned()
        .collect()
}

/// `build_graph` (layout/state_diagram.rb): the IR graph folded to an
/// ELK-shaped hash, each state measured and sized, the notes beside their
/// targets, the transitions with their labels.
pub fn build_graph(diagram: &Diagram) -> Graph {
    let ir = ir_graph(diagram);
    let semantics = semantics_of(&ir.nodes);
    let states_by_id: HashMap<&str, usize> =
        ir.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();

    let mut graph = Graph { id: "state_diagram".to_string(), ..Default::default() };

    // The written id of an IR state.
    let written =
        |i: usize| -> String {
            semantic_value(&semantics, &ir.nodes, &ir.nodes[i].id, "original_identifier")
                .unwrap_or_else(|| ir.nodes[i].id.clone())
        };

    // Top-level states, then top-level notes.
    for (i, node) in ir.nodes.iter().enumerate() {
        if node.parent_id.is_some() {
            continue;
        }
        if let Some(base) = state_type_of(&node.role) {
            graph.children.push(state_from(i, base, &ir.nodes, &semantics, &written));
        }
    }
    for node in &ir.nodes {
        if node.role != "annotation" {
            continue;
        }
        let written_note = semantic_value(&semantics, &ir.nodes, &node.id, "original_identifier")
            .unwrap_or_else(|| node.id.clone());
        let target = node.parent_id.as_ref().map(|parent| {
            let i = states_by_id.get(parent.as_str()).copied().unwrap_or(usize::MAX);
            if i == usize::MAX {
                parent.clone()
            } else {
                written(i)
            }
        });
        let position = semantic_value(&semantics, &ir.nodes, &node.id, "placement_side");
        let lines = note_lines(node.label.as_deref().unwrap_or(""));
        let labels: Vec<GLabel> = lines
            .iter()
            .map(|text| {
                let (width, height) = measure_text(text, Look::NOTE_FONT);
                GLabel { text: text.clone(), width, height }
            })
            .collect();
        let (width, height) = note_dimensions(&labels);
        graph.children.push(GNode {
            id: written_note,
            width,
            height,
            labels,
            meta: GMeta {
                state_type: "note".to_string(),
                shape_type: "note".to_string(),
                note_target: target,
                note_position: position,
                ..Default::default()
            },
            ..Default::default()
        });
    }
    // Transitions.
    for (oid, edge) in ir.edges.iter().enumerate() {
        let Some(parent) = edge.parent_id.clone() else { continue };
        let trigger = semantic_value(&semantics, &ir.nodes, &parent, "trigger");
        let guard = semantic_value(&semantics, &ir.nodes, &parent, "guard_condition");
        let identifier =
            semantic_value(&semantics, &ir.nodes, &parent, "transition_identifier")
                .unwrap_or_else(|| edge.id.clone());
        let (Some(source), Some(target)) = (
            edge.source_id.as_deref().and_then(|id| states_by_id.get(id).copied()),
            edge.target_id.as_deref().and_then(|id| states_by_id.get(id).copied()),
        ) else {
            continue;
        };
        graph.edges.push(GEdge {
            id: identifier,
            oid,
            sources: vec![written(source)],
            targets: vec![written(target)],
            labels: transition_labels(&edge.label),
            sections: None,
            label_at: None,
            trigger,
            guard_condition: guard,
        });
    }
    graph
}

fn state_from(
    i: usize,
    base: &str,
    nodes: &[IrNode],
    semantics: &HashMap<Option<String>, Vec<usize>>,
    written: &dyn Fn(usize) -> String,
) -> GNode {
    let node = &nodes[i];
    let id = written(i);
    let state_type = semantic_value(semantics, nodes, &node.id, "declared_state_type")
        .unwrap_or_else(|| base.to_string());
    let description = semantic_value(semantics, nodes, &node.id, "latest_description");
    let descriptions = semantic_values(semantics, nodes, &node.id, "display_text");
    let children: Vec<GNode> = semantics
        .get(&Some(node.id.clone()))
        .map(|children| {
            children
                .iter()
                .filter_map(|&c| {
                    state_type_of(&nodes[c].role)
                        .map(|base| state_from(c, base, nodes, semantics, written))
                })
                .collect()
        })
        .unwrap_or_default();

    let texts = state_texts(&descriptions, &node.label, &description);
    let (label_width, label_height) = measure_text(
        texts.first().unwrap_or(&node.id),
        Look::TEXT_FONT,
    );
    let meta = GMeta {
        state_type: state_type.clone(),
        shape_type: shape_type(&state_type, &descriptions, &description, &node.label, &id),
        description,
        cluster: !children.is_empty(),
        note_target: None,
        note_position: None,
        shape_radius: None,
        trigger: None,
        guard_condition: None,
    };
    let shape = meta.shape_type.clone();
    let mut meta = meta;
    if shape == "start" || shape == "end" {
        meta.shape_radius =
            Some(Look::TERMINAL_RADIUS.iter().find(|(k, _)| *k == shape).map(|(_, r)| *r).unwrap_or(0.0));
    }
    let labels: Vec<GLabel> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let size = if index == 0 { Look::TEXT_FONT } else { Look::SMALL_FONT };
            let (width, height) = measure_text(text, size);
            GLabel { text: text.clone(), width, height }
        })
        .collect();
    let (width, height) = dimensions_for_shape(
        &shape,
        &children,
        (label_width, label_height),
        &texts[texts.len().min(1)..],
        &labels,
    );
    GNode {
        id,
        width,
        height,
        labels,
        meta,
        children,
        ..Default::default()
    }
}

/// The dimensions of one state (PlantUML layout `dimensions_for_shape`).
fn dimensions_for_shape(
    shape: &str,
    children: &[GNode],
    label_dims: (f64, f64),
    descriptions: &[String],
    _labels: &[GLabel],
) -> (f64, f64) {
    match shape {
        "history" => (Look::HISTORY_SIZE.0, Look::HISTORY_SIZE.1),
        "start" | "end" => Look::terminal(shape),
        "note" => unreachable!("notes are sized when read"),
        _ => leaf_dimensions(children, label_dims, descriptions),
    }
}

/// A plain state is a name above a divider; a composite is sized from its
/// children later (PlantUML layout `leaf_dimensions`).
fn leaf_dimensions(
    children: &[GNode],
    label_dims: (f64, f64),
    descriptions: &[String],
) -> (f64, f64) {
    if !children.is_empty() {
        return normal_state_dimensions(label_dims, descriptions);
    }
    let lines: Vec<f64> = descriptions
        .iter()
        .map(|text| measure_text(text, Look::TEXT_FONT).0)
        .collect();
    let width = lines.iter().copied().fold(label_dims.0, f64::max) + 20.0;
    (width, leaf_height(descriptions.len()))
}

fn leaf_height(lines: usize) -> f64 {
    let mut body = (lines as f64 * Look::TEXT_LINE) + 10.0;
    if lines == 0 {
        body = Look::EMPTY_BODY;
    }
    Look::LEAF_BAR + body
}

/// The base layout's `calculate_normal_state_dimensions` — in the
/// PlantUML path always over a composite, so the simple branch never
/// runs: the width is the widest text plus 40, clamped at 100x50.
fn normal_state_dimensions(
    label_dims: (f64, f64),
    descriptions: &[String],
) -> (f64, f64) {
    let dimensions: Vec<(f64, f64)> = descriptions
        .iter()
        .map(|text| measure_text(text, Look::SMALL_FONT))
        .collect();
    let width = dimensions.iter().map(|d| d.0).fold(label_dims.0, f64::max) + 40.0;
    let height =
        label_dims.1 + 30.0 + dimensions.iter().map(|d| d.1 + 10.0).sum::<f64>();
    (width.max(100.0), height.max(50.0))
}

fn note_dimensions(labels: &[GLabel]) -> (f64, f64) {
    let widest = labels.iter().map(|l| l.width).fold(f64::MIN, f64::max);
    (
        widest + Look::NOTE_PAD_X + Look::NOTE_PAD_RIGHT,
        (labels.len() as f64 * Look::NOTE_LINE) + (2.0 * Look::NOTE_PAD_Y),
    )
}

fn note_lines(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?i)\s*<br\s*/?\s*>\s*").unwrap();
    re.split(text)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// The shape a state draws (the PlantUML layout's `state_shape_type`).
fn shape_type(
    state_type: &str,
    descriptions: &[String],
    description: &Option<String>,
    label: &Option<String>,
    id: &str,
) -> String {
    if state_type == "note" {
        return "note".to_string();
    }
    if matches!(label.as_deref(), Some("H") | Some("H*"))
        && (id.ends_with("_history") || id.ends_with("_deep_history"))
    {
        return "history".to_string();
    }
    if descriptions.iter().any(|d| !d.is_empty()) {
        return "normal".to_string();
    }
    if description.as_ref().is_some_and(|d| !d.is_empty()) {
        return "normal".to_string();
    }
    state_type.to_string()
}

fn transition_labels(label: &Option<String>) -> Vec<GLabel> {
    match label {
        Some(text) if !text.is_empty() => {
            let (width, height) = measure_text(text, Look::SMALL_FONT);
            vec![GLabel { text: text.clone(), width, height }]
        }
        _ => vec![],
    }
}
