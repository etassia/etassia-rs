//! Port of `activity/renderer.rb` (the drawing half lives in
//! `marks.rs`): each mark becomes one SVG element, in order, with the
//! source's style rules laid over the drawn defaults.

use super::layout::Layout;
use super::marks::Mark;
use super::metrics::num_to_s;
use super::model::Diagram;
use super::style::Sheet;
use crate::svg::{Document, Elem};

const FONT_FAMILY: &str = "sans-serif";
const PAPER: &str = "#F1F1F1";
const INK: &str = "#181818";
const TERMINAL: &str = "#222222";
/// The stroke width of the shapes PlantUML draws heavier than a box.
const WARNING_WIDTH: &str = "3";
const BAR_WIDTH: &str = "1";
/// How far a cubic stops short of the tip of its arrow head.
const HEAD_NOTCH: f64 = 5.0;

/// A written style value the renderer reads.
#[derive(Clone)]
enum Read {
    Text(String),
    Num(f64),
    None,
}

impl Read {
    fn num(&self) -> Option<f64> {
        match self {
            Read::Num(num) => Some(*num),
            _ => None,
        }
    }
}

fn look(sheet: &Sheet, property: &str) -> Read {
    read_value(sheet.explicit("activity", property))
}

/// The rule of the mark's own `<style>` selector (diamond, note,
/// partition, arrow), which wins over the activityDiagram block.
fn own(mark: &Mark, sheet: &Sheet, property: &str) -> Read {
    match mark.role() {
        Some(role) => read_value(sheet.explicit(role, property)),
        None => Read::None,
    }
}

fn read_value(value: Option<super::style::Value>) -> Read {
    match value {
        Some(super::style::Value::Text(text)) => Read::Text(text),
        Some(super::style::Value::Num(num)) => Read::Num(num),
        None => Read::None,
    }
}

fn face(mark: &Mark, sheet: &Sheet, property: &str) -> Read {
    match own(mark, sheet, property) {
        Read::None => look(sheet, property),
        found => found,
    }
}

/// A partition has no paint of its own, so only its own rule fills it; a
/// diamond falls back to the action paint.
fn fill_of(mark: &Mark, sheet: &Sheet) -> Option<String> {
    if let Read::Text(background) = own(mark, sheet, "background") {
        return Some(background);
    }
    match (mark.role(), mark_fill(mark)) {
        (Some("partition"), Some(fill)) => Some(fill),
        (_, fill) => paper(sheet, fill),
    }
}

fn mark_fill(mark: &Mark) -> Option<String> {
    match mark {
        Mark::Rect { fill, .. } | Mark::Polygon { fill, .. } | Mark::Circle { fill, .. } => {
            Some(fill.clone())
        }
        _ => None,
    }
}

/// PlantUML draws a partition's corners at half the RoundCorner.
fn corner(mark: &Mark, sheet: &Sheet) -> Option<f64> {
    if mark.role() != Some("partition") {
        return match look(sheet, "round_corner") {
            found @ Read::Num(_) => found.num(),
            _ => mark_rx(mark),
        };
    }
    match own(mark, sheet, "round_corner") {
        Read::Num(round) => Some(round / 2.0),
        _ => mark_rx(mark),
    }
}

fn mark_rx(mark: &Mark) -> Option<f64> {
    match mark {
        Mark::Rect { rx, .. } => *rx,
        _ => None,
    }
}

fn arrow_colour(sheet: &Sheet) -> String {
    match sheet.explicit("arrow", "border") {
        Some(super::style::Value::Text(colour)) => colour,
        _ => match sheet.from_style("activity", "border") {
            Some(super::style::Value::Text(colour)) => colour,
            _ => INK.to_string(),
        },
    }
}

/// The start disc takes the activityDiagram background.
fn disc(mark: &Mark, sheet: &Sheet) -> String {
    let fill = mark_fill(mark).unwrap_or_default();
    if mark.role() != Some("start") {
        return fill;
    }
    match sheet.from_style("activity", "background") {
        Some(super::style::Value::Text(background)) => background,
        _ => fill,
    }
}

fn hairline(mark: &Mark, sheet: &Sheet) -> String {
    let thickness = match own(mark, sheet, "line_thickness") {
        Read::Num(num) => Some(num),
        Read::None => look(sheet, "line_thickness").num(),
        _ => None,
    };
    num_to_s(thickness.unwrap_or(0.5))
}

fn frame_width(mark: &Mark, sheet: &Sheet) -> String {
    if mark.role() != Some("partition") {
        return match mark.role() {
            Some("warning") => WARNING_WIDTH.to_string(),
            Some("bar") => BAR_WIDTH.to_string(),
            _ => hairline(mark, sheet),
        };
    }
    match own(mark, sheet, "line_thickness") {
        Read::Num(num) => num_to_s(num),
        _ => "1.5".to_string(),
    }
}

/// The default box paint stands for what the source sets.
fn paper(sheet: &Sheet, fill: Option<String>) -> Option<String> {
    match fill {
        Some(fill) if fill == PAPER => match look(sheet, "background") {
            Read::Text(background) => Some(background),
            _ => Some(fill),
        },
        other => other,
    }
}

fn ink(mark: &Mark, sheet: &Sheet) -> String {
    let stroke = mark_stroke(mark).unwrap_or_default();
    if stroke != INK {
        return stroke;
    }
    match own(mark, sheet, "border") {
        Read::Text(border) => border,
        Read::None => match look(sheet, "border") {
            Read::Text(border) => border,
            _ => stroke,
        },
        _ => stroke,
    }
}

fn mark_stroke(mark: &Mark) -> Option<String> {
    match mark {
        Mark::Rect { stroke, .. } | Mark::Polygon { stroke, .. } | Mark::Path { stroke, .. } => {
            Some(stroke.clone())
        }
        Mark::Circle { stroke, .. } => Some(stroke.to_string()),
        _ => None,
    }
}

fn line_ink(sheet: &Sheet, stroke: &str) -> String {
    match stroke {
        INK => arrow_colour(sheet),
        TERMINAL => match sheet.from_style("activity", "border") {
            Some(super::style::Value::Text(border)) => border,
            _ => stroke.to_string(),
        },
        _ => stroke.to_string(),
    }
}

fn mark_dash(mark: &Mark) -> Option<&'static str> {
    match mark {
        Mark::Path { dash, .. } => *dash,
        _ => None,
    }
}

/// Ruby `Svg::Rect`: a dasharray the source's LineStyle sets.
fn rect_line_style(sheet: &Sheet) -> Option<String> {
    match look(sheet, "line_style") {
        Read::Text(style) => Some(style),
        _ => None,
    }
}

/// Draws a scene: each mark becomes one SVG element, in order.
pub fn render(diagram: &Diagram, sheet: &Sheet) -> Result<String, crate::Error> {
    let (marks, width, height) = Layout::call(diagram, sheet);
    let mut doc = Document::new(width, height);
    for mark in &marks {
        doc.add(draw(mark, sheet));
    }
    Ok(doc.to_xml())
}

fn draw(mark: &Mark, sheet: &Sheet) -> Elem {
    match mark {
        Mark::Rect {
            x,
            y,
            width,
            height,
            ..
        } => {
            let mut element = Elem::new("rect")
                .attr("fill", &fill_of(mark, sheet).unwrap_or_default())
                .attr("stroke", &ink(mark, sheet))
                .attr("stroke-width", &frame_width(mark, sheet));
            if let Some(style) = rect_line_style(sheet) {
                element = element.attr("stroke-dasharray", &style);
            }
            element = element
                .num("x", *x)
                .num("y", *y)
                .num("width", *width)
                .num("height", *height);
            if let Some(rx) = corner(mark, sheet) {
                element = element.num("rx", rx);
            }
            element
        }
        Mark::Circle {
            cx,
            cy,
            r,
            stroke,
            width,
            ..
        } => Elem::new("circle")
            .attr("fill", &disc(mark, sheet))
            .attr("stroke", &line_ink(sheet, stroke))
            .attr("stroke-width", &num_to_s(*width))
            .num("cx", *cx)
            .num("cy", *cy)
            .num("r", *r),
        Mark::Polygon { points, .. } => {
            let head = mark_fill(mark).as_deref() == Some(INK);
            let (fill, stroke, width) = if head {
                let arrow = arrow_colour(sheet);
                (arrow.clone(), arrow, "1".to_string())
            } else {
                (
                    fill_of(mark, sheet).unwrap_or_default(),
                    ink(mark, sheet),
                    hairline(mark, sheet),
                )
            };
            Elem::new("polygon")
                .attr("points", &format_points(points))
                .attr("fill", &fill)
                .attr("stroke", &stroke)
                .attr("stroke-width", &width)
        }
        Mark::Path {
            points,
            stroke,
            width,
            ..
        } => {
            let curve = match mark {
                Mark::Path { curve, .. } => *curve,
                _ => false,
            };
            Elem::new("path")
                .attr("d", &path_data(points, curve))
                .attr("fill", "none")
                .attr("stroke", &line_ink(sheet, stroke))
                // The path width is a Float in Ruby (`1.0`), the circle's
                // an Integer (`1`).
                .attr("stroke-width", &crate::svg::ruby_float(*width))
                .set_or_skip("stroke-dasharray", mark_dash(mark).map(str::to_string))
        }
        Mark::Text {
            x,
            y,
            content,
            anchor,
            size,
            family,
            weight,
            ..
        } => {
            let face_name = match face(mark, sheet, "font_name") {
                Read::Text(name) => Some(name),
                _ => None,
            };
            let family = family
                .map(str::to_string)
                .or(face_name)
                .unwrap_or_else(|| FONT_FAMILY.to_string());
            let fill = match face(mark, sheet, "font_colour") {
                Read::Text(colour) => colour,
                _ => "#000000".to_string(),
            };
            let mut element = Elem::new("text")
                .attr("fill", &fill)
                .num("x", *x)
                .num("y", *y)
                .attr("text-anchor", *anchor)
                .attr("font-family", &family)
                .attr("font-size", &num_to_s(*size));
            if let Some(weight) = weight {
                element = element.attr("font-weight", *weight);
            }
            super::marks::assign_creole(element, content)
        }
    }
}

trait SetOrSkip {
    fn set_or_skip(self, name: &str, value: Option<String>) -> Elem;
}

impl SetOrSkip for Elem {
    fn set_or_skip(self, name: &str, value: Option<String>) -> Elem {
        match value {
            Some(value) => self.attr(name, value),
            None => self,
        }
    }
}

fn format_points(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{},{}", number(*x), number(*y)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// PlantUML's cubic for a straight edge: control points a third of the
/// way along, stopping short of the arrow head's notch.
fn cubic(points: &[(f64, f64)]) -> String {
    let (from, to) = (points[0], points[1]);
    let to = shortened(from, to);
    let ctrl1 = lerp(from, to, 1.0 / 3.0);
    let ctrl2 = lerp(from, to, 2.0 / 3.0);
    let curve = [ctrl1, ctrl2, to]
        .iter()
        .map(|point| format!("{},{}", number(point.0), number(point.1)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("M{},{} C{}", number(from.0), number(from.1), curve)
}

fn lerp(from: (f64, f64), to: (f64, f64), fraction: f64) -> (f64, f64) {
    (
        from.0 + ((to.0 - from.0) * fraction),
        from.1 + ((to.1 - from.1) * fraction),
    )
}

fn shortened(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    lerp(from, to, ((length - HEAD_NOTCH) / length).max(0.0))
}

fn path_data(points: &[(f64, f64)], curve: bool) -> String {
    if curve && points.len() == 2 {
        return cubic(points);
    }
    points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            format!(
                "{}{},{}",
                if index == 0 { "M" } else { "L" },
                number(*x),
                number(*y)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ruby `value.round(2).to_s.delete_suffix(".0")`.
fn number(value: f64) -> String {
    let rounded = crate::render::user_journey::ruby_round(value, 2);
    let text = num_to_s(rounded);
    text.strip_suffix(".0").map(str::to_string).unwrap_or(text)
}
