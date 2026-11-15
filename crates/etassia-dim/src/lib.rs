//! Diagram Interchange Model (DIM) 0.1 support.
//!
//! DIM is the boundary between model-holding tools and the engine
//! (specification: `docs/dim/SPEC.md`). A DIM document is an ELK JSON
//! graph extended with node `content` blocks and edge semantic `style`;
//! the pipeline is parse -> layout (elkr) -> render (generic SVG). The
//! output is deterministic: the same document always renders to the same
//! bytes, which is the property the version triple in the specification
//! pins down.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const DIM_VERSION: &str = "0.1";

#[derive(Debug)]
pub enum DimError {
    Json(String),
    Unsupported(String),
    Layout(String),
}

impl fmt::Display for DimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DimError::Json(m) => write!(f, "invalid DIM document: {m}"),
            DimError::Unsupported(m) => write!(f, "unsupported DIM feature: {m}"),
            DimError::Layout(m) => write!(f, "layout failed: {m}"),
        }
    }
}

impl DimError {
    /// The C ABI error envelope: `{class,message,line,col}`.
    pub fn to_json(&self) -> String {
        let class = match self {
            DimError::Json(_) => "DimError",
            DimError::Unsupported(_) => "Unsupported",
            DimError::Layout(_) => "LayoutError",
        };
        let message = self.to_string();
        serde_json::json!({ "class": class, "message": message, "line": 0, "col": 0 }).to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DimDocument {
    pub dim: String,
    pub notation: String,
    pub graph: DimGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DimGraph {
    pub id: String,
    #[serde(default)]
    pub children: Vec<DimNode>,
    #[serde(default)]
    pub edges: Vec<DimEdge>,
    #[serde(default)]
    pub layout_options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DimNode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DimNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContentBlock {
    pub kind: String,
    pub text: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "member".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DimEdge {
    pub id: String,
    pub sources: Vec<String>,
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<EdgeLabel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EdgeLabel {
    pub text: String,
}

pub fn parse(input: &str) -> Result<DimDocument, DimError> {
    let doc: DimDocument =
        serde_json::from_str(input).map_err(|e| DimError::Json(e.to_string()))?;
    if doc.dim != DIM_VERSION {
        return Err(DimError::Unsupported(format!(
            "DIM version {} is not supported; this engine speaks {DIM_VERSION}",
            doc.dim
        )));
    }
    for n in &doc.graph.children {
        if !n.children.is_empty() {
            return Err(DimError::Unsupported(format!(
                "node {} is hierarchical; this release lays out flat documents",
                n.id
            )));
        }
    }
    Ok(doc)
}

/// The node geometry a renderer can assume when the document does not
/// declare it: a deterministic estimate from the content blocks (7.2
/// units per character of the widest line, 18 per line, 24 padding).
pub fn estimated_size(node: &DimNode) -> (f64, f64) {
    if let (Some(w), Some(h)) = (node.width, node.height) {
        return (w, h);
    }
    let lines = node.content.len().max(1);
    let widest = node
        .content
        .iter()
        .map(|b| b.text.chars().count())
        .max()
        .unwrap_or(8) as f64;
    let w = node
        .width
        .unwrap_or_else(|| (widest * 7.2 + 24.0).max(60.0));
    let h = node.height.unwrap_or(lines as f64 * 18.0 + 16.0);
    (w, h)
}

fn dim_node_to_elk(node: &DimNode) -> elkr::ElkNode {
    let (w, h) = estimated_size(node);
    elkr::ElkNode {
        id: node.id.clone(),
        x: None,
        y: None,
        width: Some(w),
        height: Some(h),
        children: Vec::new(),
        labels: Vec::new(),
    }
}

fn to_elk_graph(doc: &DimDocument) -> elkr::ElkGraph {
    elkr::ElkGraph {
        id: doc.graph.id.clone(),
        children: doc.graph.children.iter().map(dim_node_to_elk).collect(),
        edges: doc
            .graph
            .edges
            .iter()
            .map(|e| elkr::ElkEdge {
                id: e.id.clone(),
                sources: e.sources.clone(),
                targets: e.targets.clone(),
                sections: Vec::new(),
            })
            .collect(),
        layout_options: elkr::LayoutOptions(doc.graph.layout_options.clone()),
    }
}

/// Lay the document out and render it to SVG. The returned bytes are a
/// pure function of the document and the (elkr version, renderer)
/// pair.
pub fn render(input: &str) -> Result<String, DimError> {
    let doc = parse(input)?;
    let mut laid = to_elk_graph(&doc);
    elkr::apply(&mut laid).map_err(|e| DimError::Layout(e.to_string()))?;
    render_svg(&doc, &laid)
}

// ---- SVG emission ----

/// Ruby-style float text: one to three decimals, no bare integers.
fn f(v: f64) -> String {
    let s = format!("{v:.3}");
    let t = s.trim_end_matches('0');
    if t.ends_with('.') {
        format!("{t}0")
    } else {
        t.to_string()
    }
}

const STYLES: &[(&str, &str)] = &[
    ("association", "arr-filled"),
    ("reference", "arr-filled"),
    ("generalization", "arr-open-triangle"),
    ("composition", "diamond"),
    ("dependency", "arr-open"),
];

fn marker_ref(style: &str) -> &'static str {
    STYLES
        .iter()
        .find(|(name, _)| *name == style)
        .map(|(_, id)| id)
        .unwrap_or(&"arr-filled")
}

fn render_svg(doc: &DimDocument, laid: &elkr::ElkGraph) -> Result<String, DimError> {
    let find = |id: &str| {
        laid.find_node(id)
            .ok_or_else(|| DimError::Layout(format!("node {id} missing after layout")))
    };

    // bounds from laid-out nodes plus a margin for strokes
    let mut max_x = 0.0f64;
    let mut max_y = 0.0f64;
    for n in &laid.children {
        let (w, h) = (n.width.unwrap_or(40.0), n.height.unwrap_or(40.0));
        max_x = max_x.max(n.x.unwrap_or(0.0) + w);
        max_y = max_y.max(n.y.unwrap_or(0.0) + h);
    }
    let width = max_x + 1.0;
    let height = max_y + 1.0;

    let mut s = String::new();
    s.push_str("<svg\n width=\"");
    s.push_str(&f(width));
    s.push_str("\"\n height=\"");
    s.push_str(&f(height));
    s.push_str("\"\n viewBox=\"0 0 ");
    s.push_str(&f(width));
    s.push(' ');
    s.push_str(&f(height));
    s.push_str("\"\n version=\"1.2\"\n baseProfile=\"tiny\"\n xmlns=\"http://www.w3.org/2000/svg\"\n overflow=\"hidden\"\n>\n");

    // marker vocabulary: one marker per edge style that appears
    let used_styles: BTreeMap<&str, &str> = doc
        .graph
        .edges
        .iter()
        .filter_map(|e| e.style.as_deref().map(|st| (st, marker_ref(st))))
        .collect();
    if !used_styles.is_empty() {
        s.push_str("<defs>\n");
        for (style, id) in &used_styles {
            match *id {
                "arr-open-triangle" => {
                    s.push_str(&format!(
                        "<marker id=\"{id}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"10\" markerHeight=\"10\" orient=\"auto\"><path fill=\"none\" stroke=\"#000000\" stroke-width=\"1.5\" d=\"M 1 1 L 9 5 L 1 9\"/></marker>\n"
                    ));
                }
                "arr-open" => {
                    s.push_str(&format!(
                        "<marker id=\"{id}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"10\" markerHeight=\"10\" orient=\"auto\"><path fill=\"none\" stroke=\"#000000\" stroke-width=\"1.5\" d=\"M 1 1 L 9 5 L 1 9\"/></marker>\n"
                    ));
                }
                "diamond" => {
                    s.push_str(&format!(
                        "<marker id=\"{id}\" viewBox=\"0 0 12 8\" refX=\"10\" refY=\"4\" markerWidth=\"12\" markerHeight=\"8\" orient=\"auto\"><path fill=\"#000000\" stroke=\"#000000\" d=\"M 0 4 L 4 0 L 8 4 L 4 8 Z\"/></marker>\n"
                    ));
                }
                _ => {
                    let _ = style;
                    s.push_str(
                        "<marker id=\"arr-filled\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"10\" markerHeight=\"10\" orient=\"auto\"><path fill=\"#000000\" stroke=\"#000000\" d=\"M 1 1 L 9 5 L 1 9 Z\"/></marker>\n",
                    );
                }
            }
        }
        s.push_str("</defs>\n");
    }

    // edges first, nodes on top
    for e in &doc.graph.edges {
        let section = laid
            .edges
            .iter()
            .find(|l| l.id == e.id)
            .and_then(|l| l.sections.first())
            .ok_or_else(|| DimError::Layout(format!("edge {} was not routed", e.id)))?;
        let style = e.style.as_deref().unwrap_or("reference");
        s.push_str(&format!(
            "<g id=\"{}\">\n  <line fill=\"none\" stroke=\"#000000\" stroke-width=\"2.0\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" marker-end=\"url(#{})\"/>\n",
            e.id,
            f(section.start_point.x),
            f(section.start_point.y),
            f(section.end_point.x),
            f(section.end_point.y),
            marker_ref(style),
        ));
        if let Some(label) = &e.label {
            let mx = (section.start_point.x + section.end_point.x) / 2.0;
            let my = (section.start_point.y + section.end_point.y) / 2.0;
            let tw = label.text.chars().count() as f64 * 7.2 + 8.0;
            s.push_str(&format!(
                "  <rect fill=\"#ffffff\" stroke=\"none\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"20.0\"/>\n",
                f(mx - tw / 2.0),
                f(my - 10.0),
                f(tw),
            ));
            s.push_str(&format!(
                "  <text fill=\"#000000\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"Arial, Helvetica, sans-serif\" font-size=\"14.0\">{}</text>\n",
                f(mx),
                f(my + 5.0),
                xml_escape(&label.text),
            ));
        }
        s.push_str("</g>\n");
    }

    for n in &doc.graph.children {
        let laid_n = find(&n.id)?;
        let (w, h) = estimated_size(n);
        let x = laid_n.x.unwrap_or(0.0);
        let y = laid_n.y.unwrap_or(0.0);
        s.push_str(&format!(
            "<g id=\"{}\">\n  <rect fill=\"#ffffff\" stroke=\"#000000\" stroke-width=\"1.0\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>\n",
            n.id,
            f(x),
            f(y),
            f(w),
            f(h),
        ));
        let lines = n.content.len().max(1) as f64;
        let line_h = if lines > 1.0 { 18.0 } else { 0.0 };
        let start_y = y + h / 2.0 - (lines - 1.0) * line_h / 2.0;
        for (i, block) in n.content.iter().enumerate() {
            let ty = start_y + i as f64 * line_h + 5.0;
            let weight = if block.role == "title" {
                " font-weight=\"bold\""
            } else {
                ""
            };
            s.push_str(&format!(
                "  <text fill=\"#000000\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"Arial, Helvetica, sans-serif\" font-size=\"14.0\"{weight}>{}</text>\n",
                f(x + w / 2.0),
                f(ty),
                xml_escape(&block.text),
            ));
        }
        s.push_str("</g>\n");
    }

    s.push_str("</svg>");
    Ok(s)
}

fn xml_escape(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
