//! Draws a sequence Scene without changing its geometry. Port of
//! `sequence/renderer.rb` with the colours of `sequence/skin.rb`.

use super::layout::{Scene, Text};
use crate::svg::{ruby_float, Elem};

const BANNER_FILL: &str = "#FFFFCC";
const BANNER_STROKE: &str = "#FFDD88";
const WARNING_SIZE: &str = "10";
const LEGEND_FILL: &str = "#DDDDDD";

/// `Skin`: the paint of an unstyled diagram (the default theme).
const SKIN_PAINT_HEAD_FILL: &str = "#E2E2F0";
const SKIN_PAINT_LINE: &str = "#181818";
const SKIN_PAINT_NOTE_FILL: &str = "#FEFFDD";
const SKIN_PAINT_BAR_FILL: &str = "#FFFFFF";
const SKIN_PAINT_TAB_FILL: &str = "#EEEEEE";
const SKIN_PAINT_BOX_FILL: &str = "#DDDDDD";
const SKIN_PAINT_FRAME_STROKE: &str = "#000000";
const WIDTH_THIN: &str = "0.5";
const WIDTH_ARROW: &str = "1";
const WIDTH_FRAME: &str = "1.5";
const WIDTH_BAR: &str = "1";
const WIDTH_SEPARATOR: &str = "1";
const HEAD_CORNER: &str = "2.5";
const SEPARATOR_DASH: &str = "2,2";
const FAMILY: &str = "sans-serif";

/// The default theme's colors (theme/builtin/default.yml).
const NODE_FILL: &str = "#ffffff";
const NODE_STROKE: &str = "#000000";
const EDGE_STROKE: &str = "#000000";
const LABEL_TEXT: &str = "#000000";
const THEME_NOTE_FILL: &str = "#fefecd";
const STROKE_WIDTH: &str = "2";

fn num(value: f64) -> String {
    ruby_float(value)
}

struct Painter {
    painted: std::collections::HashMap<String, String>,
    doc: crate::svg::Document,
}

pub fn draw(scene: &Scene) -> String {
    let mut painter = Painter::for_scene(scene);
    for element in painter.scene_elements(scene) {
        painter.doc.add(element);
    }
    painter.doc.to_xml()
}

impl Painter {
    /// A painter seeded with the scene's paint roles.
    fn for_scene(scene: &Scene) -> Painter {
        Painter {
            painted: scene
                .paint
                .iter()
                .filter_map(|pair| {
                    let (role, value) = pair.split_once('=')?;
                    Some((role.to_string(), value.to_string()))
                })
                .collect(),
            doc: crate::svg::Document::new(scene.width, scene.height),
        }
    }
    fn painted(&self, role: &str) -> Option<&String> {
        self.painted.get(role)
    }

    /// The scene's top-level elements, in paint order. Nested pictures reuse
    /// this list directly (Ruby splices the inner document's children).
    fn scene_elements(&mut self, scene: &Scene) -> Vec<Elem> {
        let mut body: Vec<Elem> = vec![];
        if let Some(background) = self.painted("background") {
            let rect = Elem::new("rect")
                .attr("fill", background.clone())
                .num("x", 0.0)
                .num("y", 0.0)
                .num("width", scene.width)
                .num("height", scene.height);
            body.push(rect);
        }
        if let Some(title) = &scene.title {
            body.push(group("title", vec![], self.texts(title.texts.clone())));
        }
        for block in &scene.text_blocks {
            body.push(group(
                &format!("{}", block.id),
                vec![],
                self.texts(block.texts.clone()),
            ));
        }
        if let Some(legend) = &scene.legend {
            let frame = Elem::new("rect")
                .num("x", legend.x)
                .num("y", legend.y)
                .num("width", legend.width)
                .num("height", legend.height)
                .attr("rx", "7.5")
                .attr("ry", "7.5")
                .attr("fill", LEGEND_FILL)
                .attr("stroke", "#000000")
                .attr("stroke-width", "1");
            body.push(group("legend", vec![frame], self.texts(legend.texts.clone())));
        }
        let layers: Vec<Vec<Elem>> = vec![
            scene
                .banners
                .iter()
                .map(|banner| self.banner_group(banner))
                .collect(),
            scene.frames.iter().map(|frame| self.frame_group(frame)).collect(),
            scene.lifelines.iter().map(|line| self.lifeline(line)).collect(),
            scene.bars.iter().map(|bar| self.activation_bar(bar)).collect(),
            scene.crosses.iter().map(|line| self.cross_line(line)).collect(),
            scene
                .page_breaks
                .iter()
                .map(|line| self.page_break_line(line))
                .collect(),
            scene
                .fragments
                .iter()
                .map(|fragment| self.fragment_group(fragment))
                .collect(),
            scene
                .dividers
                .iter()
                .map(|divider| self.divider_group(divider))
                .collect(),
            scene.delays.iter().map(|delay| self.delay_group(delay)).collect(),
            scene.arrows.iter().map(|arrow| self.arrow_group(arrow)).collect(),
            scene.notes.iter().map(|note| self.note_group(note)).collect(),
            scene.heads.iter().map(|head| self.head_group(head)).collect(),
        ];
        for layer in layers {
            for element in layer {
                body.push(element);
            }
        }
        body
    }

    fn banner_group(&self, banner: &super::layout::Banner) -> Elem {
        let rect = Elem::new("rect")
            .num("x", banner.x)
            .num("y", banner.y)
            .num("width", banner.width)
            .num("height", banner.height)
            .attr("rx", "2.5")
            .attr("ry", "2.5")
            .attr("fill", BANNER_FILL)
            .attr("stroke", BANNER_STROKE)
            .attr("stroke-width", "3");
        group(
            &format!("banner-{}", num(banner.y)),
            vec![rect],
            self.texts(banner.texts.clone()),
        )
    }

    fn frame_rectangle(&self, x: f64, y: f64, width: f64, height: f64) -> Elem {
        Elem::new("rect")
            .num("x", x)
            .num("y", y)
            .num("width", width)
            .num("height", height)
            .attr("fill", "none")
            .attr("stroke", NODE_STROKE)
            .attr("stroke-width", "1")
    }

    fn frame_group(&self, frame: &super::layout::Frame) -> Elem {
        let mut rect = self.frame_rectangle(frame.x, frame.y, frame.width, frame.height);
        if let Some(fill) = frame
            .fill
            .clone()
            .or_else(|| self.painted("box_fill").cloned())
            .or(Some(SKIN_PAINT_BOX_FILL.to_string()))
        {
            rect = rect.attr("fill", fill);
        }
        let stroke = self
            .painted("box_line")
            .cloned()
            .or(Some(SKIN_PAINT_LINE.to_string()))
            .unwrap_or_else(|| rect_stroke_of(&rect, NODE_STROKE));
        rect = rect.attr("stroke", stroke).attr("stroke-width", WIDTH_THIN);
        group(
            &format!("box-{}", num(frame.x)),
            vec![rect],
            self.texts(frame.texts.clone()),
        )
    }

    fn activation_bar(&self, bar: &super::layout::Bar) -> Elem {
        let fill = bar
            .fill
            .clone()
            .or(Some(SKIN_PAINT_BAR_FILL.to_string()))
            .or_else(|| Some(NODE_FILL.to_string()))
            .unwrap();
        let rect = self
            .frame_rectangle(bar.x, bar.y, bar.width, bar.height)
            .attr("fill", fill)
            .attr("stroke", SKIN_PAINT_LINE)
            .attr("stroke-width", WIDTH_BAR);
        rect
    }

    fn cross_line(&self, segment: &super::layout::Segment) -> Elem {
        self.solid(segment).attr("stroke", self.edge_colour())
    }

    fn page_break_line(&self, segment: &super::layout::Segment) -> Elem {
        self.lifeline(segment).attr("stroke-dasharray", "2,2")
    }

    fn lifeline(&self, segment: &super::layout::Segment) -> Elem {
        let mut element = Elem::new("line")
            .num("x1", segment.x1)
            .num("y1", segment.y1)
            .num("x2", segment.x2)
            .num("y2", segment.y2)
            .attr("stroke", stroke_of(self.painted("lifeline"), Some(SKIN_PAINT_LINE), NODE_STROKE))
            .attr("stroke-width", WIDTH_THIN);
        if let Some(dash) = segment.dash {
            element = element.attr("stroke-dasharray", dash);
        } else {
            element = element.attr("stroke-dasharray", "5,5");
        }
        element
    }

    /// `solid`: a lifeline drawn with no dash attribute at all.
    fn solid(&self, segment: &super::layout::Segment) -> Elem {
        Elem::new("line")
            .num("x1", segment.x1)
            .num("y1", segment.y1)
            .num("x2", segment.x2)
            .num("y2", segment.y2)
            .attr("stroke", SKIN_PAINT_FRAME_STROKE)
            .attr("stroke-width", WIDTH_SEPARATOR)
    }

    fn dashed(&self, segment: &super::layout::Segment) -> Elem {
        Elem::new("line")
            .num("x1", segment.x1)
            .num("y1", segment.y1)
            .num("x2", segment.x2)
            .num("y2", segment.y2)
            .attr("stroke", stroke_of(self.painted("group_line"), Some(SKIN_PAINT_FRAME_STROKE), NODE_STROKE))
            .attr("stroke-width", WIDTH_SEPARATOR)
            .attr("stroke-dasharray", "2,2")
    }

    fn arrow_group(&self, arrow: &super::layout::Arrow) -> Elem {
        let colour = arrow
            .colour
            .clone()
            .unwrap_or_else(|| self.edge_colour());
        let mut children = vec![self.arrow_path(arrow, &colour)];
        for mark in &arrow.marks {
            children.push(self.arrow_mark(mark, &colour));
        }
        group(&arrow.id, children, self.texts(arrow.texts.clone()))
    }

    fn arrow_path(&self, arrow: &super::layout::Arrow, colour: &str) -> Elem {
        let mut element = Elem::new("path")
            .attr("d", arrow.path.clone())
            .attr("fill", "none")
            .attr("stroke", colour.to_string())
            .attr("stroke-width", self.stroke_width());
        if arrow.dashed {
            element = element.attr("stroke-dasharray", "2,2");
        }
        element
    }

    fn arrow_mark(&self, mark: &super::layout::Mark, colour: &str) -> Elem {
        match mark.kind {
            "polygon" => {
                let fill = if mark.filled {
                    colour.to_string()
                } else {
                    SKIN_PAINT_BAR_FILL.to_string()
                };
                Elem::new("polygon")
                    .attr("points", mark.points.clone())
                    .attr("fill", fill)
                    .attr("stroke", colour.to_string())
                    .attr("stroke-width", self.stroke_width())
            }
            "circle" => Elem::new("circle")
                .num("cx", mark.cx)
                .num("cy", mark.cy)
                .num("r", mark.r)
                .attr("fill", SKIN_PAINT_BAR_FILL)
                .attr("stroke", colour.to_string())
                .attr("stroke-width", self.stroke_width()),
            _ => {
                let width = if mark.heavy {
                    ruby_float(stroke_width_value() * 2.0)
                } else {
                    self.stroke_width()
                };
                Elem::new("line")
                    .num("x1", mark.x1)
                    .num("y1", mark.y1)
                    .num("x2", mark.x2)
                    .num("y2", mark.y2)
                    .attr("stroke", colour.to_string())
                    .attr("stroke-width", width)
            }
        }
    }

    fn fragment_group(&self, fragment: &super::layout::Fragment) -> Elem {
        let mut children = vec![
            self.frame_rectangle(fragment.x, fragment.y, fragment.width, fragment.height)
                .attr(
                    "stroke",
                    stroke_of(self.painted("group_line"), Some(SKIN_PAINT_FRAME_STROKE), NODE_STROKE),
                )
                .attr("stroke-width", WIDTH_FRAME),
            self.tab_path(fragment),
        ];
        for line in &fragment.separators {
            children.push(self.dashed(line));
        }
        group(
            &format!("fragment-{}-{}", num(fragment.x), num(fragment.y)),
            children,
            self.texts(fragment.texts.clone()),
        )
    }

    fn tab_path(&self, fragment: &super::layout::Fragment) -> Elem {
        let fill = fragment
            .tab_fill
            .clone()
            .or(Some(SKIN_PAINT_TAB_FILL.to_string()))
            .or_else(|| Some(NODE_FILL.to_string()))
            .unwrap();
        Elem::new("path")
            .attr("d", fragment.tab_path.clone())
            .attr("fill", fill)
            .attr("stroke", stroke_of(self.painted("group_line"), Some(SKIN_PAINT_FRAME_STROKE), NODE_STROKE))
            .attr("stroke-width", WIDTH_FRAME)
    }

    fn note_group(&self, note: &super::layout::NoteScene) -> Elem {
        let id = format!("note-{}", crc32(&note.path));
        let mut children = vec![self.note_path(Some(&note.path), note)];
        if let Some(fold) = &note.fold_path {
            children.push(self.note_path(Some(fold), note));
        }
        if let Some((x, y, scale, inner)) = &note.picture {
            let inner = draw_picture(inner);
            let mut group = Elem::new("g")
                .attr("transform", &format!("translate({}, {}) scale({})", num(*x), num(*y), num(*scale)));
            for child in inner {
                group = group.child(child);
            }
            children.push(group);
        }
        group(&id, children, self.texts(note.texts.clone()))
    }

    fn note_path(&self, data: Option<&str>, note: &super::layout::NoteScene) -> Elem {
        let Some(data) = data else {
            return Elem::new("path");
        };
        let mut element = Elem::new("path")
            .attr("d", data.to_string())
            .attr(
                "fill",
                note.fill.clone().unwrap_or_else(|| {
                    self.painted("note_fill")
                        .cloned()
                        .unwrap_or_else(|| SKIN_PAINT_NOTE_FILL.to_string())
                }),
            )
            .attr("stroke", stroke_of(self.painted("note_line"), Some(SKIN_PAINT_LINE), NODE_STROKE))
            .attr("stroke-width", WIDTH_THIN);
        if let Some(opacity) = note.fill_opacity {
            element = element.attr("fill-opacity", num(opacity));
        }
        element
    }

    fn divider_group(&self, divider: &super::layout::Divider) -> Elem {
        let mut children: Vec<Elem> = divider
            .lines
            .iter()
            .map(|line| {
                self.solid(line)
                    .attr("stroke", self.painted("divider_line").cloned().unwrap_or_else(|| NODE_STROKE.to_string()))
            })
            .collect();
        if !divider.texts.is_empty() {
            children.push(
                Elem::new("rect")
                    .num("x", divider.x)
                    .num("y", divider.y)
                    .num("width", divider.width)
                    .num("height", divider.height)
                    .attr(
                        "fill",
                        self.painted("divider_fill").cloned().unwrap_or_else(|| NODE_FILL.to_string()),
                    )
                    .attr(
                        "stroke",
                        self.painted("divider_line").cloned().unwrap_or_else(|| NODE_STROKE.to_string()),
                    )
                    .attr("stroke-width", "1"),
            );
        }
        group(
            &format!("divider-{}", num(divider.y)),
            children,
            self.texts(divider.texts.clone()),
        )
    }

    fn delay_group(&self, delay: &super::layout::Delay) -> Elem {
        let key = delay
            .texts
            .first()
            .map(|text| num(text.y))
            .unwrap_or_else(|| num(delay.y1));
        group(&format!("delay-{key}"), vec![], self.texts(delay.texts.clone()))
    }

    fn head_group(&self, head: &super::layout::Head) -> Elem {
        let shapes = self.head_shapes(head);
        group(
            &format!("participant-{}-{}", head.id, num(head.y)),
            shapes,
            self.texts(head.texts.clone()),
        )
    }

    fn head_shapes(&self, head: &super::layout::Head) -> Vec<Elem> {
        let mut shapes: Vec<Elem> = vec![];
        for rectangle in &head.rectangles {
            shapes.push(self.head_rectangle(rectangle, head));
        }
        for ellipse in &head.ellipses {
            shapes.push(self.head_ellipse(ellipse, head));
        }
        for path in &head.paths {
            shapes.push(self.head_path(path, head));
        }
        for line in &head.lines {
            shapes.push(self.head_line(line, head));
        }
        for polygon in &head.polygons {
            shapes.push(self.head_polygon(polygon, head));
        }
        if shapes.is_empty() {
            let corner = "3";
            shapes.push(self.legacy_head_rectangle(head, corner));
        }
        shapes
    }

    fn head_paint(&self, head: &super::layout::Head) -> Vec<(&'static str, String)> {
        vec![
            (
                "fill",
                head.fill.clone().unwrap_or_else(|| {
                    self.painted("head_fill").cloned().unwrap_or_else(|| SKIN_PAINT_HEAD_FILL.to_string())
                }),
            ),
            (
                "stroke",
                head.stroke.clone().unwrap_or_else(|| {
                    stroke_of(self.painted("head_line"), Some(SKIN_PAINT_LINE), NODE_STROKE)
                }),
            ),
            ("stroke-width", WIDTH_THIN.to_string()),
        ]
    }

    fn head_rectangle(&self, shape: &super::layout::HeadRectangle, head: &super::layout::Head) -> Elem {
        let mut element = Elem::new("rect")
            .num("x", shape.x)
            .num("y", shape.y)
            .num("width", shape.width)
            .num("height", shape.height);
        for (name, value) in self.head_paint(head) {
            element = element.attr(name, value);
        }
        let (rx, ry) = match (shape.rx, shape.ry) {
            (Some(rx), Some(ry)) => (Some(rx), Some(ry)),
            _ => (None, None),
        };
        if rx.is_some() || matches!(shape.rx, None) {
            element = element.attr("rx", HEAD_CORNER);
        }
        if ry.is_some() || matches!(shape.ry, None) {
            element = element.attr("ry", HEAD_CORNER);
        }
        if let Some(opacity) = head.fill_opacity {
            element = element.attr("fill-opacity", num(opacity));
        }
        element
    }

    fn legacy_head_rectangle(&self, head: &super::layout::Head, corner: &str) -> Elem {
        let mut element = Elem::new("rect")
            .num("x", head.x)
            .num("y", head.y)
            .num("width", head.width)
            .num("height", head.height)
            .attr("rx", corner)
            .attr("ry", corner);
        for (name, value) in self.head_paint(head) {
            element = element.attr(name, value);
        }
        if let Some(opacity) = head.fill_opacity {
            element = element.attr("fill-opacity", num(opacity));
        }
        element
    }

    fn head_ellipse(&self, shape: &super::layout::HeadEllipse, head: &super::layout::Head) -> Elem {
        let mut element = Elem::new("ellipse")
            .num("cx", shape.cx)
            .num("cy", shape.cy)
            .num("rx", shape.rx)
            .num("ry", shape.ry);
        for (name, value) in self.head_paint(head) {
            element = element.attr(name, value);
        }
        if let Some(opacity) = head.fill_opacity {
            element = element.attr("fill-opacity", num(opacity));
        }
        element
    }

    fn head_path(&self, shape: &super::layout::HeadPath, head: &super::layout::Head) -> Elem {
        let mut element = Elem::new("path").attr("d", shape.data.clone());
        if shape.filled {
            element = element.attr(
                "fill",
                head.fill.clone().unwrap_or_else(|| {
                    self.painted("head_fill").cloned().unwrap_or_else(|| SKIN_PAINT_HEAD_FILL.to_string())
                }),
            );
            if let Some(opacity) = head.fill_opacity {
                element = element.attr("fill-opacity", num(opacity));
            }
        } else {
            element = element.attr("fill", "none");
        }
        element
            .attr(
                "stroke",
                head.stroke.clone().unwrap_or_else(|| {
                    stroke_of(self.painted("head_line"), Some(SKIN_PAINT_LINE), NODE_STROKE)
                }),
            )
            .attr("stroke-width", WIDTH_THIN)
    }

    fn head_line(&self, shape: &super::layout::HeadLine, head: &super::layout::Head) -> Elem {
        Elem::new("line")
            .num("x1", shape.x1)
            .num("y1", shape.y1)
            .num("x2", shape.x2)
            .num("y2", shape.y2)
            .attr(
                "stroke",
                head.stroke.clone().unwrap_or_else(|| {
                    stroke_of(self.painted("head_line"), Some(SKIN_PAINT_LINE), NODE_STROKE)
                }),
            )
            .attr("stroke-width", WIDTH_THIN)
    }

    fn head_polygon(&self, shape: &super::layout::HeadPolygon, head: &super::layout::Head) -> Elem {
        let colour = head.stroke.clone().unwrap_or_else(|| {
            stroke_of(self.painted("head_line"), Some(SKIN_PAINT_LINE), NODE_STROKE)
        });
        Elem::new("polygon")
            .attr("points", shape.points.clone())
            .attr("fill", colour.clone())
            .attr("stroke", colour)
            .attr("stroke-width", WIDTH_THIN)
    }

    fn text_element(&self, text: &Text) -> Elem {
        let fill = text
            .colour
            .clone()
            .or_else(|| self.painted_role_text(text.role))
            .unwrap_or_else(|| LABEL_TEXT.to_string());
        let family = text.family.clone().unwrap_or_else(|| self.font_family(text.role));
        let size = text.size.map(num).unwrap_or_else(|| self.font_size_of(text.role));
        let weight = text
            .weight
            .clone()
            .or_else(|| text_weight(text.role));
        let style = text
            .font_style
            .clone()
            .or_else(|| (text.role == "kind").then(|| "italic".to_string()));
        let mut element = Elem::new("text")
            .attr("fill", fill)
            .num("x", text.x)
            .num("y", text.y)
            .attr("text-anchor", text.anchor)
            .attr("font-family", family)
            .attr("font-size", size);
        if let Some(weight) = weight {
            element = element.attr("font-weight", weight);
        }
        if let Some(style) = style {
            element = element.attr("font-style", style);
        }
        crate::render::component::creole::assign(element, &text.content)
    }

    fn painted_role_text(&self, role: &str) -> Option<String> {
        self.painted(&format!("text_{role}")).cloned()
    }

    fn font_family(&self, role: &str) -> String {
        if role == "warning" {
            return "monospace".to_string();
        }
        FAMILY.to_string()
    }

    fn font_size_of(&self, role: &str) -> String {
        if role == "warning" {
            return WARNING_SIZE.to_string();
        }
        match role {
            "message_label" | "note" | "box" | "fragment_tab" | "divider" => "13".to_string(),
            "fragment_guard" | "delay" => "11".to_string(),
            "kind" => "11.9".to_string(),
            _ => "14.0".to_string(),
        }
    }

    fn edge_colour(&self) -> String {
        self.painted("arrow")
            .cloned()
            .or(Some(SKIN_PAINT_LINE.to_string()))
            .or_else(|| Some(EDGE_STROKE.to_string()))
            .unwrap()
    }

    fn stroke_width(&self) -> String {
        self.painted("arrow_width")
            .cloned()
            .unwrap_or_else(|| WIDTH_ARROW.to_string())
    }
}

fn stroke_width_value() -> f64 {
    1.0
}

fn stroke_of(painted: Option<&String>, skin: Option<&str>, default: &str) -> String {
    painted
        .cloned()
        .or_else(|| skin.map(str::to_string))
        .unwrap_or_else(|| default.to_string())
}

fn rect_stroke_of(_rect: &Elem, default: &str) -> String {
    default.to_string()
}

fn text_weight(role: &str) -> Option<String> {
    match role {
        "fragment_tab" | "box" | "title" => Some("bold".to_string()),
        "fragment_guard" => Some("bold".to_string()),
        _ => None,
    }
}

impl Painter {
    fn texts(&self, items: Vec<Text>) -> Vec<Elem> {
        items.iter().map(|text| self.text_element(text)).collect()
    }
}

fn group(id: &str, children: Vec<Elem>, texts: Vec<Elem>) -> Elem {
    let mut element = Elem::new("g").attr("id", id);
    for child in children {
        element = element.child(child);
    }
    for text in texts {
        element = element.child(text);
    }
    element
}

/// Draws an embedded picture scene into its own element list (the inner
/// document's children, spliced under a transformed group).
fn draw_picture(scene: &Scene) -> Vec<Elem> {
    let mut painter = Painter::for_scene(scene);
    painter.scene_elements(scene)
}

/// Ruby `Zlib.crc32`.
fn crc32(data: &str) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in data.bytes() {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
