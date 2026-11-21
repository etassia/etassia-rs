//! Port of `activity/{node_box,metrics}.rb`: where a node sits (its centre and
//! size) and the sizes of the shapes, from the text they carry and the
//! font size the source gives each role.

use super::model::{node_lines, Kind, Node, Note};
use super::style::Sheet;
use crate::text::{measure, measure_monospace, Face};

/// Ruby `Array#sum` over floats: Kahan-Babuska compensation, elements in
/// order.
pub(crate) fn ruby_sum(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut compensation = 0.0;
    for &value in values {
        let next = total + value;
        if total.abs() >= value.abs() {
            compensation += (total - next) + value;
        } else {
            compensation += (value - next) + total;
        }
        total = next;
    }
    total + compensation
}

/// Ruby's `.max.to_f`: a maximum over nothing is 0.
trait MaxFiniteToF {
    fn max_finite_to_f(self) -> f64;
}

impl MaxFiniteToF for f64 {
    fn max_finite_to_f(self) -> f64 {
        if self.is_finite() {
            self
        } else {
            0.0
        }
    }
}

/// Ruby `Numeric#to_s`: an integral value prints as an integer, any other
/// float as `Float#to_s`.
pub(crate) fn num_to_s(value: f64) -> String {
    if value.trunc() == value && value.abs() < 1e16 {
        format!("{}", value as i64)
    } else {
        crate::svg::ruby_float(value)
    }
}

/// Where a node sits: its centre and size (`Box`).
#[derive(Clone, Debug)]
pub struct Box {
    pub cx: f64,
    pub cy: f64,
    pub width: f64,
    pub height: f64,
    /// Where arrows meet a bar that has one neighbour on a side; None
    /// elsewhere (`cx` then).
    pub anchor: Option<f64>,
    /// The nodes a bar fans out to or gathers.
    pub fan: Vec<usize>,
}

impl Box {
    pub fn new(width: f64, height: f64) -> Self {
        Box {
            cx: 0.0,
            cy: 0.0,
            width,
            height,
            anchor: None,
            fan: vec![],
        }
    }

    /// The node_box with these edges (`Box.spanning`).
    pub fn spanning(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Box {
            cx: (left + right) / 2.0,
            cy: (top + bottom) / 2.0,
            width: right - left,
            height: bottom - top,
            anchor: None,
            fan: vec![],
        }
    }

    /// Whether the vertical at `x` runs through the node_box.
    pub fn spans(&self, x: f64) -> bool {
        self.left() < x && x < self.right()
    }

    pub fn left(&self) -> f64 {
        self.cx - (self.width / 2.0)
    }

    pub fn right(&self) -> f64 {
        self.cx + (self.width / 2.0)
    }

    pub fn top(&self) -> f64 {
        self.cy - (self.height / 2.0)
    }

    pub fn bottom(&self) -> f64 {
        self.cy + (self.height / 2.0)
    }
}

/// The action's size; the other roles' own defaults are `ROLE_SIZES`,
/// which is what PlantUML draws for an unstyled source.
pub const FONT_SIZE: f64 = 12.0;
/// The line pitch and baseline of a font size, as PlantUML's default face
/// lays them out.
const LINE_RATIO: f64 = 1.1777;
const ASCENT_RATIO: f64 = 0.9668;
pub const PAD_X: f64 = 10.0;
pub const PAD_Y: f64 = 10.0;
pub const NOTE_PAD: f64 = 7.0;
/// A decision is a hexagon: the text's own width and height, at least
/// this wide and tall, with a point of this reach each side.
pub const DECISION_POINT: f64 = 12.0;
pub const DECISION_MIN: f64 = 24.0;

/// The fixed sizes, `[width, height]` per kind.
fn fixed_size_of(kind: Kind, legacy: bool) -> (f64, f64) {
    match kind {
        Kind::Start => (20.0, 20.0),
        Kind::Stop | Kind::End | Kind::Kill | Kind::Merge => (24.0, 24.0),
        Kind::Bar => {
            // PlantUML draws a legacy bar at this size however many
            // branches it joins.
            if legacy {
                (80.0, 8.0)
            } else {
                (40.0, 6.0)
            }
        }
        Kind::Action | Kind::Decision => (0.0, 0.0),
    }
}

/// The font size each role carries before the source says anything.
fn role_default(role: Option<&str>) -> f64 {
    match role {
        Some("diamond") | Some("arrow") => 11.0,
        Some("note") => 13.0,
        Some("partition") => 14.0,
        _ => FONT_SIZE,
    }
}

/// The font-size value the sheet writes for an element.
fn written(sheet: &Sheet, element: &str, property: &str) -> Option<f64> {
    match sheet.explicit(element, property) {
        Some(super::style::Value::Num(size)) => Some(size),
        _ => None,
    }
}

/// The `<style>`-only value the sheet writes for an element.
fn written_style(sheet: &Sheet, element: &str, property: &str) -> Option<f64> {
    match sheet.from_style(element, property) {
        Some(super::style::Value::Num(size)) => Some(size),
        _ => None,
    }
}

/// Skinparam names PlantUML reads for a role, besides the role.
fn skin_aliases(role: &str) -> Vec<&str> {
    match role {
        "diamond" => vec!["diamond", "activitydiamond"],
        "arrow" => vec!["arrow", "activityarrow"],
        other => vec![other],
    }
}

/// The sizes of the shapes (`Metrics`).
pub struct Metrics<'a> {
    sheet: &'a Sheet,
}

impl<'a> Metrics<'a> {
    pub fn new(sheet: &'a Sheet) -> Self {
        Metrics { sheet }
    }

    /// The font size of the role's text; None is an action. PlantUML
    /// cascades `activityDiagram` down to every role from `<style>` only;
    /// skinparam `activityFontSize` is the action's and the decision's.
    pub fn size(&self, role: Option<&str>) -> f64 {
        let written_size = match role {
            Some(role) => self.role_size(role),
            None => written(self.sheet, "activity", "font_size"),
        };
        let Some(written_size) = written_size else {
            return role_default(role);
        };
        if written_size.trunc() == written_size {
            written_size.trunc()
        } else {
            written_size
        }
    }

    /// The distance between the baselines of two lines.
    pub fn line_height(&self, role: Option<&str>) -> f64 {
        LINE_RATIO * self.size(role)
    }

    /// The baseline of a line below the top of its row.
    pub fn ascent(&self, role: Option<&str>) -> f64 {
        ASCENT_RATIO * self.size(role)
    }

    /// `Metrics#role_size`.
    fn role_size(&self, role: &str) -> Option<f64> {
        written_style(self.sheet, role, "font_size")
            .or_else(|| written_style(self.sheet, "activity", "font_size"))
            .or_else(|| self.skin_size(role))
    }

    /// `Metrics#skin_size`: the role's own skinparam, then its alias.
    fn skin_size(&self, role: &str) -> Option<f64> {
        skin_aliases(role)
            .iter()
            .find_map(|name| written(self.sheet, name, "font_size"))
            .or_else(|| {
                if role == "diamond" {
                    written(self.sheet, "activity", "font_size")
                } else {
                    None
                }
            })
    }

    /// The width of the widest of the lines.
    pub fn text_width(&self, lines: &[String], role: Option<&str>) -> f64 {
        let size = self.size(role);
        self.text_width_sized(lines, role, size)
    }

    /// The same, at an explicit size (`Metrics#text_width`'s `size:`).
    /// Ruby's `.max.to_f`: no lines measure as 0.
    pub fn text_width_sized(&self, lines: &[String], role: Option<&str>, size: f64) -> f64 {
        lines
            .iter()
            .map(|line| self.line_width(line, role, size))
            .fold(f64::NEG_INFINITY, f64::max)
            .max_finite_to_f()
    }

    /// One line's width: the sum of its creole runs' widths.
    fn line_width(&self, line: &str, role: Option<&str>, size: f64) -> f64 {
        let face = self.face(role);
        let runs = crate::render::component::creole::parse(line);
        let widths: Vec<f64> = runs
            .iter()
            .map(|run| {
                if run.style.monospace() {
                    measure_monospace(&run.text, size).width
                } else {
                    measure(&run.text, size, face).width
                }
            })
            .collect();
        ruby_sum(&widths)
    }

    pub fn text_height(&self, lines: &[String], role: Option<&str>) -> f64 {
        lines.len().max(1) as f64 * self.line_height(role)
    }

    /// A node_box of the node's size, not yet placed.
    pub fn box_for(&self, node: &Node) -> Box {
        match node.kind {
            Kind::Start | Kind::Stop | Kind::End | Kind::Kill | Kind::Merge | Kind::Bar => {
                let (width, height) = fixed_size_of(node.kind, node.legacy);
                Box::new(width, height)
            }
            _ if node.bare => Box::new(24.0, 24.0),
            Kind::Decision => {
                self.decision_box(&node_lines(Some(&node.text.clone().unwrap_or_default())))
            }
            _ => self.action_box(&node_lines(Some(&node.text.clone().unwrap_or_default()))),
        }
    }

    fn action_box(&self, lines: &[String]) -> Box {
        Box::new(
            self.text_width(lines, None) + (2.0 * PAD_X),
            self.text_height(lines, None) + (2.0 * PAD_Y),
        )
    }

    fn decision_box(&self, lines: &[String]) -> Box {
        Box::new(
            self.text_width(lines, Some("diamond")).max(DECISION_MIN) + (2.0 * DECISION_POINT),
            self.text_height(lines, Some("diamond")).max(DECISION_MIN),
        )
    }

    /// A note's node_box.
    pub fn note_box(&self, note: &Note) -> Box {
        let lines = note.lines();
        Box::new(
            self.text_width(&lines, Some("note")) + (2.0 * NOTE_PAD) + 6.0,
            self.text_height(&lines, Some("note")) + (2.0 * NOTE_PAD),
        )
    }

    /// The face the role's text is measured in: the source's FontName
    /// when etassia has the table for it, else the default face.
    pub fn face(&self, role: Option<&str>) -> Face {
        let name = self.written_face(role).unwrap_or_default().to_lowercase();
        let faces: [(&str, Face); 4] = [
            ("arial", Face::Arial),
            ("trebuchet", Face::Trebuchet),
            ("dejavu", Face::DejaVu),
            ("dejavu_bold", Face::DejaVuBold),
        ];
        faces
            .iter()
            .find(|(face, _)| name.contains(face))
            .map(|(_, face)| *face)
            .unwrap_or(Face::DejaVu)
    }

    fn written_face(&self, role: Option<&str>) -> Option<String> {
        let own = role.and_then(|role| match self.sheet.explicit(role, "font_name") {
            Some(super::style::Value::Text(name)) => Some(name),
            _ => None,
        });
        own.or_else(|| match self.sheet.explicit("activity", "font_name") {
            Some(super::style::Value::Text(name)) => Some(name),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_box_fits_two_lines() {
        let sheet = Sheet::default();
        let metrics = Metrics::new(&sheet);
        let lines = vec!["foo1".to_string(), "foo2".to_string()];
        let one = metrics.action_box(&lines[..1]);
        let two = metrics.action_box(&lines);
        assert!(two.height > one.height);
        assert!((one.height - (12.0 * 1.1777 + 20.0)).abs() < 1e-12);
    }

    #[test]
    fn defaults_carry_the_role_sizes() {
        let sheet = Sheet::default();
        let metrics = Metrics::new(&sheet);
        assert_eq!(metrics.size(None), 12.0);
        assert_eq!(metrics.size(Some("diamond")), 11.0);
        assert_eq!(metrics.size(Some("note")), 13.0);
        assert_eq!(metrics.size(Some("partition")), 14.0);
    }
}
