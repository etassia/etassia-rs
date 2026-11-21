//! The PlantUML mindmap layout: the part a mindmap and a work breakdown
//! structure share (a forest of measured boxes and the curves joining
//! them), and the mindmap placement - the root in the middle, its
//! right-hand branches growing right and its left-hand ones growing left,
//! every parent centred on its children. Port of
//! `lib/etassia/notation/plantuml/tree/{layout,mindmap_layout,role}.rb`.

use super::reader::{Entry, MONO_MARK};
use super::rules::Rules;
use crate::render::pie::fsum;
use crate::text;
use crate::Error;
use regex::Regex;
use std::sync::OnceLock;

/// Space between the canvas edge and the boxes.
const MARGIN: f64 = 20.0;
/// Room on either side of a box's text.
const PAD_X: f64 = 10.0;
/// Room above and below a box's text.
const PAD_Y: f64 = 10.0;
const LINE_RATIO: f64 = 1.1777;
const BASELINE_RATIO: f64 = 0.9667;
/// DejaVu Sans Mono, the JAR's `monospace`: line height and ascent.
const MONO_LINE_RATIO: f64 = 1.1641;
const MONO_BASELINE_RATIO: f64 = 0.928;
const FILL: &str = "#F1F1F1";
/// The mindmap placement: sibling and level gaps, the connector's lead
/// into its parent and its bend, the rounded corner, and the text size.
const SIBLING_GAP: f64 = 20.0;
const LEVEL_GAP: f64 = 50.0;
const LEAD: f64 = 10.0;
const BEND: f64 = 25.0;
const CORNER_RADIUS: f64 = 12.5;
const FONT_SIZE: f64 = 14.0;

/// What a tree node carries beyond its text, spelled as the node's role
/// token: the side of its parent it hangs on, whether it draws no box,
/// and the fill a style class gave it. Port of `tree/role.rb`.
pub mod role {
    use regex::Regex;
    use std::sync::OnceLock;

    /// The role token for the parts; a fill loses its `#` and its case.
    pub fn spell(side: &str, boxless: bool, fill: Option<&str>) -> String {
        let mut token = side.to_string();
        if boxless {
            token.push_str("_boxless");
        }
        if let Some(fill) = fill {
            token.push_str("_bg_");
            token.push_str(fill.strip_prefix('#').unwrap_or(fill).to_lowercase().as_str());
        }
        token
    }

    /// `Role.parse`: the side, the boxless mark and the fill a role token
    /// carries. A fill is a CSS colour name or three or six hex digits; a
    /// role token cannot hold the `#`.
    pub fn parse(role: &str) -> (&'static str, bool, Option<String>) {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        let re =
            PATTERN.get_or_init(|| Regex::new(r"\A(left|right)(_boxless)?(?:_bg_([0-9a-z]+))?\z").expect("role pattern"));
        let Some(captures) = re.captures(role) else {
            return ("right", false, None);
        };
        let side = if &captures[1] == "left" { "left" } else { "right" };
        let boxless = captures.get(2).is_some();
        let fill = captures.get(3).map(|token| fill_of(token.as_str()));
        (side, boxless, fill)
    }

    /// `Role.fill_of`: three or six hex digits read back with their `#`.
    fn fill_of(token: &str) -> String {
        if (token.len() == 3 || token.len() == 6) && token.bytes().all(|b| b.is_ascii_hexdigit()) {
            format!("#{token}")
        } else {
            token.to_string()
        }
    }
}

/// A node of the forest with its text measured.
#[allow(dead_code)]
pub struct Item {
    id: String,
    level: usize,
    side: &'static str,
    boxless: bool,
    fill: Option<String>,
    lines: Vec<String>,
    width: f64,
    height: f64,
    children: Vec<Item>,
    mono: Vec<bool>,
}

/// A placed box.
#[derive(Clone, Copy)]
struct Placed<'a> {
    item: &'a Item,
    x: f64,
    y: f64,
}

impl Placed<'_> {
    fn right(&self) -> f64 {
        self.x + self.item.width
    }

    fn mid_y(&self) -> f64 {
        self.y + (self.item.height / 2.0)
    }
}

/// A subtree measured on its own: where each child sits relative to the
/// item's centre, and how far the subtree reaches above and below that
/// centre at each depth.
struct Subtree<'a> {
    item: &'a Item,
    children: Vec<Subtree<'a>>,
    offsets: Vec<f64>,
    contour: Vec<(f64, f64)>,
}

/// The subtrees of one side of a root, their offsets from the fan's
/// centre and the contour about it.
struct Side<'a> {
    subs: Vec<Subtree<'a>>,
    offsets: Vec<f64>,
    contour: Vec<(f64, f64)>,
}

/// One positioned label line.
pub struct LabelLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub mono: bool,
}

/// A node's label: the text size and its positioned lines.
pub struct Label {
    pub font_size: f64,
    pub lines: Vec<LabelLine>,
}

/// One laid-out node of a tree, as the renderer draws it.
pub struct Node {
    /// The node's depth in the forest; the Ruby scene carries it, the
    /// SVG the tree renderer draws never spells it.
    #[allow(dead_code)]
    pub level: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub boxed: bool,
    pub fill: String,
    pub corner_radius: f64,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub dasharray: Option<String>,
    pub font_colour: Option<String>,
    pub label: Label,
}

/// A connector, as the renderer draws it.
pub struct Edge {
    pub path: String,
    pub stroke: Option<String>,
    pub stroke_width: f64,
    pub dasharray: Option<String>,
}

/// The finished scene.
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Lays out the mindmap the entries describe.
pub fn scene(entries: &[Entry], rules: &Rules) -> Result<Scene, Error> {
    let forest = build_forest(entries);
    let mut layout = Layout { forest: &forest, boxes: vec![], segments: vec![], depths: vec![], rules };
    layout.run()
}

/// The graph's forest: the nearest earlier entry one level up is a
/// node's parent; a first-level entry is a root of its own.
fn build_forest(entries: &[Entry]) -> Vec<Item> {
    fn make(index: usize, entries: &[Entry], children_of: &std::collections::HashMap<usize, Vec<usize>>, level: usize) -> Item {
        let entry = &entries[index];
        let children = children_of
            .get(&index)
            .map(|kids| kids.iter().map(|&kid| make(kid, entries, children_of, level + 1)).collect())
            .unwrap_or_default();
        // sized_item: the role spelled and parsed back, the text measured.
        let role = role::spell(entry.side, entry.boxless, entry.fill.as_deref());
        let (side, boxless, fill) = role::parse(&role);
        let (lines, mono) = text_lines(&entry.lines.join("\n"));
        let (width, height) = measured(&lines, &mono, boxless);
        Item { id: entry.id.clone(), level, side, boxless, fill, lines, width, height, children, mono }
    }
    let mut children_of: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    let mut roots = vec![];
    for (index, entry) in entries.iter().enumerate() {
        match entry.parent {
            Some(parent) => children_of.entry(parent).or_default().push(index),
            None => roots.push(index),
        }
    }
    roots.into_iter().map(|root| make(root, entries, &children_of, 1)).collect()
}

struct Layout<'a, 'r> {
    forest: &'a [Item],
    boxes: Vec<Placed<'a>>,
    segments: Vec<String>,
    depths: Vec<usize>,
    rules: &'r Rules,
}

impl<'a, 'r> Layout<'a, 'r> {
    fn run(&mut self) -> Result<Scene, Error> {
        if self.forest.is_empty() {
            // The Ruby scene crashes on an empty forest; the engine
            // declines to Ruby, which fails the same way.
            return Err(Error::Parse {
                class: "LayoutError".into(),
                message: "empty mindmap".into(),
                line: 0,
                col: 0,
            });
        }
        let mut top = MARGIN;
        for root in self.forest {
            top = self.place_tree(root, top) + SIBLING_GAP;
        }
        Ok(self.build_scene())
    }

    fn place_tree(&mut self, root: &'a Item, top: f64) -> f64 {
        let (right, left) = root_sides(root);
        let sides = [(right, 1.0), (left, -1.0)];
        let half = root.height / 2.0;
        let (low, high) = extent(&sides, half);
        let centre = top - low;
        let hub = self.box_item(root, 0.0, centre - half);
        for (side, direction) in &sides {
            self.place(&hub, &side.subs, &side.offsets, *direction);
        }
        centre + high
    }

    fn box_item(&mut self, item: &'a Item, left: f64, top: f64) -> Placed<'a> {
        let placed = Placed { item, x: left, y: top };
        self.boxes.push(placed);
        placed
    }

    fn place(&mut self, parent: &Placed<'a>, subs: &[Subtree<'a>], offsets: &[f64], direction: f64) {
        for (sub, offset) in subs.iter().zip(offsets) {
            let centre = parent.mid_y() + offset;
            let left = if direction > 0.0 {
                parent.right() + LEVEL_GAP
            } else {
                parent.x - LEVEL_GAP - sub.item.width
            };
            let child = self.box_item(sub.item, left, centre - (sub.item.height / 2.0));
            self.join(parent, &child, direction);
            self.place(&child, &sub.children, &sub.offsets, direction);
        }
    }

    fn join(&mut self, parent: &Placed<'a>, child: &Placed<'a>, direction: f64) {
        let start = if direction > 0.0 { parent.right() } else { parent.x };
        let stop = if direction > 0.0 { child.x } else { child.right() };
        let path = connector(start, parent.mid_y(), stop, child.mid_y(), direction);
        self.curve(path, parent.item.level - 1);
    }

    fn curve(&mut self, path: String, depth: usize) {
        self.segments.push(path);
        self.depths.push(depth);
    }

    fn build_scene(&mut self) -> Scene {
        let shift = MARGIN - self.boxes.iter().map(|b| b.x).fold(f64::INFINITY, f64::min);
        let width =
            self.boxes.iter().map(|b| b.right()).fold(f64::NEG_INFINITY, f64::max) + shift + MARGIN;
        let height = self
            .boxes
            .iter()
            .map(|b| b.y + b.item.height)
            .fold(f64::NEG_INFINITY, f64::max)
            + MARGIN;
        let nodes = self.boxes.iter().map(|placed| self.node(placed, shift)).collect();
        let edges = self
            .segments
            .iter()
            .zip(&self.depths)
            .map(|(path, depth)| {
                let look = self.rules.arrow(*depth);
                Edge {
                    path: shift_path(path, shift),
                    stroke: look.stroke.clone(),
                    stroke_width: look.stroke_width.unwrap_or_else(stroke_width),
                    dasharray: look.dasharray.clone(),
                }
            })
            .collect();
        Scene { width, height, nodes, edges }
    }

    fn node(&self, placed: &Placed<'a>, shift: f64) -> Node {
        let item = placed.item;
        let left = placed.x + shift;
        let look = self.rules.node(item.level - 1, item.boxless);
        let fill = item.fill.clone().or_else(|| look.fill.clone()).unwrap_or_else(|| FILL.to_string());
        // PlantUML draws a corner of half the RoundCorner it is given.
        let corner_radius = look.round_corner.map(|corner| corner / 2.0).unwrap_or(CORNER_RADIUS);
        let mut line_top = placed.y + PAD_Y;
        let lines = item
            .lines
            .iter()
            .zip(&item.mono)
            .map(|(text, mono)| {
                let ascent = if *mono { MONO_BASELINE_RATIO } else { BASELINE_RATIO };
                let y = round3(line_top + (FONT_SIZE * ascent));
                line_top += pitch(*mono);
                LabelLine { text: text.clone(), x: left + PAD_X, y, mono: *mono }
            })
            .collect();
        Node {
            level: item.level,
            x: left,
            y: placed.y,
            width: item.width,
            height: item.height,
            boxed: !item.boxless,
            fill,
            corner_radius,
            stroke: look.stroke.clone(),
            stroke_width: look.stroke_width,
            dasharray: look.dasharray.clone(),
            font_colour: look.font_colour.clone(),
            label: Label { font_size: FONT_SIZE, lines },
        }
    }
}

/// The connector: a lead into the parent, a bezier with the bend, and a
/// lead out of the child.
fn connector(from_x: f64, from_y: f64, to_x: f64, to_y: f64, direction: f64) -> String {
    let lead = direction * LEAD;
    let bend = direction * BEND;
    format!(
        "M{} L{} C{} {} {} L{}",
        point(from_x, from_y),
        point(from_x + lead, from_y),
        point(from_x + bend, from_y),
        point(to_x - bend, to_y),
        point(to_x - lead, to_y),
        point(to_x, to_y)
    )
}

fn point(x: f64, y: f64) -> String {
    format!("{},{}", round3(x), round3(y))
}

/// Every number pair in a path is an `x,y` point; the x takes the shift.
fn shift_path(path: &str, shift: f64) -> String {
    static PAIR: OnceLock<Regex> = OnceLock::new();
    let re = PAIR
        .get_or_init(|| Regex::new(r"(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)").expect("pair pattern"));
    re.replace_all(path, |captures: &regex::Captures| {
        let x: f64 = captures[1].parse().unwrap_or(0.0) + shift;
        format!("{},{}", round3(x), &captures[2])
    })
    .into_owned()
}

/// The label's lines, and whether each draws in the monospace family.
fn text_lines(label: &str) -> (Vec<String>, Vec<bool>) {
    let mut lines: Vec<String> = label.split('\n').map(|line| line.to_string()).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    let mono = lines.iter().map(|line| line.starts_with(MONO_MARK)).collect();
    let stripped =
        lines.iter().map(|line| line.trim_start_matches(MONO_MARK).to_string()).collect();
    (stripped, mono)
}

/// The box a line's text needs.
fn measured(lines: &[String], mono: &[bool], boxless: bool) -> (f64, f64) {
    let widest = lines
        .iter()
        .zip(mono)
        .map(|(line, mono)| line_width(line, *mono))
        .fold(f64::MIN, f64::max);
    (widest + (2.0 * PAD_X), fsum(mono.iter().map(|mono| pitch(*mono))) + (2.0 * pad_y(boxless)))
}

fn line_width(line: &str, mono: bool) -> f64 {
    if mono {
        text::measure_mono(line, FONT_SIZE).width
    } else {
        text::measure(line, FONT_SIZE, text::Face::Arial).width
    }
}

fn pitch(mono: bool) -> f64 {
    if mono {
        FONT_SIZE * MONO_LINE_RATIO
    } else {
        FONT_SIZE * LINE_RATIO
    }
}

/// The room above and below a box's text; a boxless node's own layout may
/// keep it tighter.
fn pad_y(boxless: bool) -> f64 {
    let _ = boxless;
    PAD_Y
}

fn stroke_width() -> f64 {
    1.0
}

/// The root's subtrees split by the side of the root they hang on: the
/// right-hand fan draws right, the left-hand one left.
fn root_sides(root: &Item) -> (Side<'_>, Side<'_>) {
    let mut right = vec![];
    let mut left = vec![];
    for child in &root.children {
        if child.side == "right" {
            right.push(child);
        } else {
            left.push(child);
        }
    }
    (fan(&right), fan(&left))
}

fn fan<'a>(items: &[&'a Item]) -> Side<'a> {
    let subs: Vec<Subtree> = items.iter().map(|item| tree(item)).collect();
    if subs.is_empty() {
        return Side { subs, offsets: vec![], contour: vec![] };
    }
    let (offsets, covered) = stack(&subs);
    let (offsets, contour) = recentre(offsets, &covered);
    Side { subs, offsets, contour }
}

fn tree(item: &Item) -> Subtree<'_> {
    let owned: Vec<&Item> = item.children.iter().collect();
    let Side { subs, offsets, contour: deep } = fan(&owned);
    let half = item.height / 2.0;
    let mut contour = vec![(-half, half)];
    contour.extend(deep);
    Subtree { item, children: subs, offsets, contour }
}

/// Subtrees of siblings packed top to bottom, each as close to the one
/// before as their contours allow, and centred on the middle of what they
/// cover.
fn recentre(offsets: Vec<f64>, covered: &[(f64, f64)]) -> (Vec<f64>, Vec<(f64, f64)>) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for (top, bottom) in covered {
        min = min.min(*top).min(*bottom);
        max = max.max(*top).max(*bottom);
    }
    let mid = (min + max) / 2.0;
    (
        offsets.iter().map(|offset| offset - mid).collect(),
        covered.iter().map(|(top, bottom)| (top - mid, bottom - mid)).collect(),
    )
}

fn stack(subs: &[Subtree<'_>]) -> (Vec<f64>, Vec<(f64, f64)>) {
    let mut offsets = vec![];
    let mut covered: Vec<(f64, f64)> = vec![];
    for sub in subs {
        let offset = if covered.is_empty() { 0.0 } else { clearance(&covered, &sub.contour) };
        offsets.push(offset);
        absorb(&mut covered, &sub.contour, offset);
    }
    (offsets, covered)
}

fn clearance(covered: &[(f64, f64)], contour: &[(f64, f64)]) -> f64 {
    let shared = covered.len().min(contour.len());
    (0..shared)
        .map(|depth| covered[depth].1 + SIBLING_GAP - contour[depth].0)
        .fold(f64::MIN, f64::max)
}

fn absorb(covered: &mut Vec<(f64, f64)>, contour: &[(f64, f64)], offset: f64) {
    for (depth, (top, bottom)) in contour.iter().enumerate() {
        let reach = (top + offset, bottom + offset);
        if depth < covered.len() {
            let slot = &mut covered[depth];
            *slot = (slot.0.min(reach.0), slot.1.max(reach.1));
        } else {
            covered.push(reach);
        }
    }
}

fn extent(sides: &[(Side<'_>, f64)], half: f64) -> (f64, f64) {
    let mut low = -half;
    let mut high = half;
    for (side, _) in sides {
        for (top, bottom) in &side.contour {
            low = low.min(*top);
            high = high.max(*bottom);
        }
    }
    (low, high)
}

/// Ruby `Float#round(3)`, the way the path points and label baselines round.
pub fn round3(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let binexp = crate::js_math::binexp(x);
    let limit = if binexp > 0 { binexp / 4 } else { binexp / 3 - 1 };
    if 3 >= 15 - limit {
        return x;
    }
    let under = if binexp > 0 { binexp / 3 + 1 } else { binexp / 4 };
    if 3 < -under {
        return 0.0;
    }
    let s = 1000.0_f64;
    let mut f = (x * s).round();
    if x > 0.0 {
        if (f + 0.5) / s <= x {
            f += 1.0;
        }
    } else if (f - 0.5) / s >= x {
        f -= 1.0;
    }
    f / s
}
