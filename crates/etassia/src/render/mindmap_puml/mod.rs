//! The PlantUML mindmap lane: the Tree notation's `@startmindmap` sources,
//! read into a hierarchy, laid out as a tree and drawn as grey boxes with
//! dark outlines, text set from the left of each box, and thin dark
//! connectors under them. Port of `lib/etassia/notation/plantuml/tree`
//! (reader, rules, layout, renderer). The Mermaid mindmap, a force layout
//! of coloured shapes, is `render::mindmap`.

mod layout;
mod reader;
mod rules;

use crate::svg::{Document, Elem};
use crate::Error;

/// The grey a box draws when nothing else fills it, and the ink of the
/// outlines, connectors and text.
const INK: &str = "#181818";
const INK_TEXT: &str = "#000000";
const FONT: &str = "sans-serif";
const MONO_FONT: &str = "monospace";
/// The outline width a box draws when no rule sets one.
const BOX_STROKE_WIDTH: f64 = 1.5;

/// Renders a PlantUML mindmap source to SVG bytes.
pub fn render(source: &str) -> Result<String, Error> {
    let tree = crate::parse::parse(source, "mindmap")?;
    let read = reader::read(reader::entry_lines(&tree))?;
    let scene = layout::scene(&read.entries, &read.rules)?;
    Ok(render_scene(&scene))
}

/// Draws the scene: connectors first, then the node groups.
fn render_scene(scene: &layout::Scene) -> String {
    let mut document = Document::new(scene.width, scene.height);
    document.view_box = Some(format!("0 0 {} {}", dimension(scene.width), dimension(scene.height)));
    for edge in &scene.edges {
        let mut path = Elem::new("path")
            .attr("fill", "none")
            .attr("stroke", edge.stroke.clone().unwrap_or_else(|| INK.to_string()))
            .attr("stroke-width", ruby_stroke(edge.stroke_width))
            .attr("d", &edge.path);
        if let Some(dasharray) = &edge.dasharray {
            path = path.attr("stroke-dasharray", dasharray);
        }
        document.add(path);
    }
    for node in &scene.nodes {
        let mut group = Elem::new("g");
        if node.boxed {
            let mut rect = Elem::new("rect")
                .attr("fill", &node.fill)
                .attr("stroke", node.stroke.clone().unwrap_or_else(|| INK.to_string()))
                .attr("stroke-width", ruby_stroke(node.stroke_width.unwrap_or(BOX_STROKE_WIDTH)))
                .num("x", node.x)
                .num("y", node.y)
                .num("width", node.width)
                .num("height", node.height)
                .num("rx", node.corner_radius)
                .num("ry", node.corner_radius);
            if let Some(dasharray) = &node.dasharray {
                rect = rect.attr("stroke-dasharray", dasharray);
            }
            group.push(rect);
        }
        for line in &node.label.lines {
            let text = Elem::new("text")
                .attr("fill", node.font_colour.clone().unwrap_or_else(|| INK_TEXT.to_string()))
                .num("x", line.x)
                .num("y", line.y)
                .content(&line.text)
                .attr("font-family", if line.mono { MONO_FONT } else { FONT })
                .attr("font-size", ruby_stroke(node.label.font_size));
            group.push(text);
        }
        document.add(group);
    }
    document.to_xml()
}

/// `Base#create_document`'s dimension: an integral size reads as an
/// integer in the view box.
fn dimension(value: f64) -> String {
    if value.trunc() == value && value.abs() < 9.0e15 {
        format!("{}", value as i64)
    } else {
        crate::svg::ruby_float(value)
    }
}

/// A stroke width as the attribute text Ruby writes: `Float#to_s`.
fn ruby_stroke(value: f64) -> String {
    crate::svg::ruby_float(value)
}
