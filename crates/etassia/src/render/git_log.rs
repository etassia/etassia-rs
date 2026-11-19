//! Git log renderer: port of Ruby `Diagram::GitLog`,
//! `PlantUML::GitLogLayout` and `PlantUML::GitLogRenderer`.
use crate::render::pie::text_of;
use crate::svg::{ruby_float, Document, Elem};
use crate::Error;
use serde_json::Value;

const COLUMN_WIDTH: f64 = 44.0;
const BOX_WIDTH: f64 = 26.0;
const BOX_HEIGHT: f64 = 24.0;
const ROW_HEIGHT: f64 = 49.0;
const MARGIN: f64 = 10.0;
const INK: &str = "#181818";

/// One source line's drawing record (`GitLog.read_line`): the raw text,
/// its line number, the commit column and the rail columns.
struct LineRec {
    raw: String,
    number: usize,
    star: Option<usize>,
    rails: Vec<usize>,
}

struct Commit {
    name: String,
    rail: usize,
    line: usize,
}

/// A laid-out commit box (`GitLogLayout.box_for`).
struct CommitBox {
    name: String,
    x: f64,
    y: f64,
}

pub fn render(source: &str) -> Result<String, Error> {
    let tree = crate::parse::parse(source, "git")?;
    let lines: Vec<LineRec> = body_lines(&tree)
        .iter()
        .enumerate()
        .map(|(number, raw)| read_line(raw, number))
        .collect();
    let commits: Vec<Commit> = lines
        .iter()
        .filter_map(|l| l.star.map(|rail| Commit { name: commit_name(&l.raw), rail, line: l.number }))
        .collect();
    let base = commits.iter().map(|c| c.line).min().unwrap_or(0);
    let width = MARGIN * 2.0 + rails_count(&lines) as f64 * COLUMN_WIDTH;
    let height = MARGIN * 2.0 + lines.len() as f64 * ROW_HEIGHT;
    let boxes: Vec<CommitBox> = commits
        .iter()
        .map(|c| CommitBox {
            name: c.name.clone(),
            x: rail_x(c.rail, &lines) - BOX_WIDTH / 2.0,
            y: MARGIN + (c.line - base) as f64 * ROW_HEIGHT,
        })
        .collect();

    let mut doc = Document::new(width, height);
    for (index, b) in boxes.iter().enumerate() {
        draw_curves_from(&boxes, index, &mut doc);
        draw_box(b, &mut doc);
    }
    Ok(doc.to_xml())
}

/// The `as line` captures of the artifact tree, in source order.
fn body_lines(tree: &Value) -> Vec<String> {
    match tree {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_object().and_then(|o| o.get("line")))
            .map(text_of)
            .collect(),
        _ => vec![],
    }
}

fn read_line(raw: &str, number: usize) -> LineRec {
    let mut rails: Vec<usize> = raw
        .chars()
        .enumerate()
        .filter(|(_, c)| matches!(c, '|' | '*' | '\\' | '/'))
        .map(|(i, _)| i)
        .collect();
    rails.sort_unstable();
    rails.dedup();
    LineRec { raw: raw.to_string(), number, star: raw.chars().position(|c| c == '*'), rails }
}

/// The commit name: everything after the star with the drawing characters
/// stripped, up to the first space (`GitLog.commit_name`).
fn commit_name(raw: &str) -> String {
    let Some(pos) = raw.chars().position(|c| c == '*') else { return String::new() };
    let cleaned: String = raw
        .chars()
        .skip(pos)
        .filter(|c| !matches!(c, '-' | '.' | '*' | '|' | '/' | '\\'))
        .collect();
    let trimmed = trimmed(cleaned);
    trimmed.split(' ').next().unwrap_or("").to_string()
}

fn trimmed(s: String) -> String {
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0'))
        .to_string()
}

fn rails_count(lines: &[LineRec]) -> usize {
    let mut rails: Vec<usize> = lines.iter().flat_map(|l| l.rails.iter().copied()).collect();
    rails.sort_unstable();
    rails.dedup();
    rails.len().max(1)
}

fn rail_x(star_column: usize, lines: &[LineRec]) -> f64 {
    let mut star_columns: Vec<usize> = lines.iter().flat_map(|l| l.rails.iter().copied()).collect();
    star_columns.sort_unstable();
    star_columns.dedup();
    let index = star_columns.iter().position(|c| *c == star_column).unwrap_or(0);
    MARGIN + index as f64 * COLUMN_WIDTH + COLUMN_WIDTH / 2.0
}

fn draw_box(b: &CommitBox, doc: &mut Document) {
    doc.add(
        Elem::new("rect")
            .attr("fill", "none")
            .attr("stroke", INK)
            .attr("stroke-width", "1.5")
            .num("x", b.x)
            .num("y", b.y)
            .num("width", BOX_WIDTH)
            .num("height", BOX_HEIGHT)
            .num("rx", 4.0)
            .num("ry", 4.0),
    );
    doc.add(
        Elem::new("text")
            .attr("fill", "#000000")
            .num("x", b.x + 4.0)
            .num("y", b.y + 17.2)
            .content(&b.name)
            .attr("font-family", "monospace")
            .attr("font-size", "15")
            .attr("font-weight", "700"),
    );
}

/// The curve to the next commit on the same rail, if any
/// (`GitLogRenderer.draw_curves_from`).
fn draw_curves_from(boxes: &[CommitBox], index: usize, doc: &mut Document) {
    let Some(first) = boxes.get(index) else { return };
    for other in boxes.iter().skip(index + 1) {
        if (first.x - other.x).abs() < 1.0 {
            draw_curve(first, other, doc);
            break;
        }
    }
}

fn draw_curve(first: &CommitBox, other: &CommitBox, doc: &mut Document) {
    let top = first.y + BOX_HEIGHT;
    let bottom = other.y;
    let mid1 = top + (bottom - top) * 0.22;
    let mid2 = bottom - (bottom - top) * 0.22;
    let x = first.x + BOX_WIDTH / 2.0;
    let d = format!(
        "M{},{} C{},{} {},{} {},{}",
        ruby_float(x),
        ruby_float(top),
        ruby_float(x),
        ruby_float(mid1),
        ruby_float(x),
        ruby_float(mid2),
        ruby_float(x),
        ruby_float(bottom)
    );
    doc.add(
        Elem::new("path")
            .attr("fill", "none")
            .attr("stroke", INK)
            .attr("stroke-width", "1")
            .attr("d", d),
    );
}
