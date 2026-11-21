//! Port of Ruby `Notation::PlantUML::Renderer` (`renderer.rb`) with
//! `MemberIconElements` and `NoteOutline`: draws the Scene without
//! changing its geometry.
use super::model::Kind;
use super::scene::{Box, Icon, Relation as SceneRelation, Scene};
use super::style::{Prop, Sheet, Value};
use crate::svg::{self, Document, Elem};
use std::sync::OnceLock;

const CREOLE_ROLES: [&str; 3] = ["note", "relation_label", "member_note"];
const BOLD_ROLES: [&str; 1] = ["package_title"];
const RULE_INSET: f64 = 1.0;
const RULE_WIDTH: &str = "0.5";
const ITALIC_SPOTS: [&str; 2] = ["abstract", "interface"];
const FONT_SIZE: f64 = 14.0;

pub fn render(scene: &Scene, sheet: &Sheet) -> Result<String, crate::Error> {
    let renderer = Renderer { sheet, current_box: None, current_spot_kind: None };
    renderer.render(scene)
}

struct Renderer<'a> {
    sheet: &'a Sheet,
    current_box: Option<&'a Box>,
    current_spot_kind: Option<String>,
}

impl<'a> Renderer<'a> {
    fn render(&self, scene: &Scene) -> Result<String, crate::Error> {
        let mut document = Document::new(scene.width, scene.height);
        // render_background
        if let Some(background) = &scene.background {
            let rect = Elem::new("rect")
                .num("x", 0.0)
                .num("y", 0.0)
                .num("width", scene.width)
                .num("height", scene.height)
                .attr("fill", background.clone());
            document.add(rect);
        }
        // The content target: captions shift the whole drawing as one;
        // without captions the content draws straight into the document.
        let with_panels = !scene.panels.is_empty();
        let mut content = Elem::new("g");
        if with_panels {
            content = content.attr(
                "transform",
                format!(
                    "translate({}, {})",
                    svg::ruby_float(scene.content_x),
                    svg::ruby_float(scene.content_y)
                ),
            );
        }
        let mut collected: Vec<Elem> = vec![];
        for frame in &scene.frames {
            let frame_element = self.render_frame(frame);
            if with_panels {
                content = content.child(frame_element);
            } else {
                collected.push(frame_element);
            }
        }
        for relation in thickest_first(&scene.relations) {
            for element in self.render_relation(relation) {
                if with_panels {
                    content = content.child(element);
                } else {
                    collected.push(element);
                }
            }
        }
        for box_ in &scene.boxes {
            let element = self.render_box(box_);
            if with_panels {
                content = content.child(element);
            } else {
                collected.push(element);
            }
        }
        if with_panels {
            document.add(content);
        } else {
            for element in collected {
                document.add(element);
            }
        }
        for panel in &scene.panels {
            document.add(render_panel(panel));
        }
        Ok(document.to_xml())
    }

    fn render_relation(&self, relation: &SceneRelation) -> Vec<Elem> {
        let mut group = Elem::new("g");
        group = group.attr("id", relation.id.clone());
        let mut path = Elem::new("path");
        path = path.attr("d", relation.path.clone());
        path = path.attr("fill", "none");
        let dashing = if relation.dotted {
            Some("2,3")
        } else if relation.dashed {
            Some("6,4")
        } else {
            None
        };
        if let Some(dash) = dashing {
            path = path.attr("stroke-dasharray", dash);
        }
        path = path.attr(
            "stroke",
            relation.stroke.clone().unwrap_or_else(|| self.edge_colour()),
        );
        path = path.attr(
            "stroke-width",
            relation
                .stroke_width
                .map(|width| svg::ruby_float(width))
                .unwrap_or_else(|| self.stroke_width()),
        );
        group = group.child(path);
        for element in self.relation_marker_elements(relation) {
            group = group.child(element);
        }
        for element in self.relation_text_elements(relation) {
            group = group.child(element);
        }
        let mut out = vec![group];
        let mut number = 0usize;
        for marker in &relation.markers {
            if marker.shape != "dot" {
                continue;
            }
            let mut dot_group = Elem::new("g");
            dot_group = dot_group.attr("id", format!("dot-{}-{}", relation.id, number));
            let mut circle = Elem::new("circle");
            circle = circle.num("cx", marker.cx);
            circle = circle.num("cy", marker.cy);
            circle = circle.num("r", marker.r);
            circle = circle.attr("fill", self.edge_colour());
            circle = circle.attr("stroke", self.edge_colour());
            circle = circle.attr("stroke-width", self.stroke_width());
            dot_group = dot_group.child(circle);
            out.push(dot_group);
            number += 1;
        }
        out
    }

    fn relation_text_elements(&self, relation: &SceneRelation) -> Vec<Elem> {
        let mut elements: Vec<Elem> = relation
            .texts
            .iter()
            .map(|text| self.text_element(text))
            .collect();
        for qualifier in &relation.qualifiers {
            let mut rect = Elem::new("rect");
            rect = rect.num("x", qualifier.x);
            rect = rect.num("y", qualifier.y);
            rect = rect.num("width", qualifier.width);
            rect = rect.num("height", qualifier.height);
            rect = rect.attr("fill", self.node_fill());
            rect = rect.attr("stroke", self.node_stroke());
            rect = rect.attr("stroke-width", self.node_width());
            elements.push(rect);
            for text in &qualifier.texts {
                elements.push(self.text_element(text));
            }
        }
        elements
    }

    fn relation_marker_elements(&self, relation: &SceneRelation) -> Vec<Elem> {
        let edge = self.edge_colour();
        let node = self.node_fill();
        let mut elements: Vec<(Elem, String)> = vec![];
        for marker in &relation.markers {
            if marker.shape == "dot" {
                continue;
            }
            if marker.shape == "nesting" {
                let mut circle = Elem::new("circle");
                circle = circle.num("cx", marker.cx);
                circle = circle.num("cy", marker.cy);
                circle = circle.num("r", marker.r);
                circle = circle.attr("fill", &node);
                circle = circle.attr("stroke", &edge);
                circle = circle.attr("stroke-width", self.stroke_width());
                elements.push((circle, node.clone()));
                let mut path = Elem::new("path");
                path = path.attr("d", marker.cross.clone());
                path = path.attr("fill", "none");
                path = path.attr("stroke", &edge);
                path = path.attr("stroke-width", self.stroke_width());
                elements.push((path, "none".to_string()));
                continue;
            }
            let fill = if marker.filled { edge.clone() } else { node.clone() };
            let mut polygon = Elem::new("polygon");
            polygon = polygon.attr("points", marker.points.clone());
            polygon = polygon.attr("fill", &fill);
            polygon = polygon.attr("stroke", &edge);
            polygon = polygon.attr("stroke-width", self.stroke_width());
            elements.push((polygon, fill));
        }
        // Ruby `restyle`: a marker takes its relation's own colour and
        // thickness.
        if relation.stroke.is_some() || relation.stroke_width.is_some() {
            let thick = relation.stroke_width.unwrap_or(0.0) > 4.0;
            let edge = self.edge_colour();
            elements = elements
                .into_iter()
                .map(|(mut element, fill)| {
                    element = element.attr(
                        "stroke",
                        relation
                            .stroke
                            .clone()
                            .unwrap_or_else(|| edge.clone()),
                    );
                    element = element.attr(
                        "stroke-width",
                        relation
                            .stroke_width
                            .map(|width| svg::ruby_float(width))
                            .unwrap_or_else(|| self.stroke_width()),
                    );
                    if thick {
                        element = element.attr("stroke-width", "1");
                    }
                    let recoloured =
                        relation.stroke.is_some() && (thick || fill == edge);
                    if recoloured {
                        element =
                            element.attr("fill", relation.stroke.clone().unwrap_or_default());
                    }
                    (element, fill)
                })
                .collect();
        }
        elements.into_iter().map(|(element, _)| element).collect()
    }

    fn render_box(&self, box_: &'a Box) -> Elem {
        let renderer = &Renderer {
            sheet: self.sheet,
            current_box: Some(box_),
            current_spot_kind: box_.spot.as_ref().map(|spot| spot.kind.clone()),
        };
        let mut group = Elem::new("g");
        group = group.attr(
            "id",
            if box_.kind.ends_with("note") {
                box_.id.clone()
            } else {
                format!("class-{}", box_.id)
            },
        );
        group = group.child(renderer.box_rectangle(box_));
        for element in renderer.spot_elements(box_) {
            group = group.child(element);
        }
        for line in renderer.rule_lines(box_) {
            group = group.child(line);
        }
        for text in &box_.texts {
            if let Some(icon) = &text.icon {
                group = group.child(member_icon_group(icon));
            }
            group = group.child(renderer.text_element(text));
        }
        group
    }

    fn box_rectangle(&self, box_: &Box) -> Elem {
        if box_.kind == "member_note" {
            return self.member_note_shape(box_);
        }
        if box_.kind == "note" {
            return self.note_shape(box_);
        }
        let mut rect = Elem::new("rect");
        rect = rect.num("x", box_.x);
        rect = rect.num("y", box_.y);
        rect = rect.num("width", box_.width);
        rect = rect.num("height", box_.height);
        let corner = self.box_look_number(Prop::RoundCorner).unwrap_or(2.5);
        rect = rect.num("rx", corner);
        rect = rect.num("ry", corner);
        rect = rect.attr("fill", self.box_look(Prop::Background));
        rect = rect.attr("stroke", self.box_look(Prop::Border));
        rect = rect.attr(
            "stroke-width",
            self.box_look_number(Prop::LineThickness)
                .map(|number| svg::ruby_float(number))
                .unwrap_or_else(|| "0.5".to_string()),
        );
        if let Some(dash) = self.box_look_dash() {
            rect = rect.attr("stroke-dasharray", dash);
        }
        rect
    }

    fn note_shape(&self, box_: &Box) -> Elem {
        let mut path = Elem::new("path");
        path = path.attr("d", note_outline(box_));
        path = path.attr("fill", note_fill(box_));
        path = path.attr("stroke", self.node_stroke());
        path = path.attr("stroke-width", self.node_width());
        path
    }

    fn member_note_shape(&self, box_: &Box) -> Elem {
        let left = box_.x;
        let top = box_.y;
        let middle = top + box_.height / 2.0;
        let pointer = box_.separators.first();
        let mut outline = vec![
            format!("M {} {}", svg::ruby_float(left), svg::ruby_float(top)),
            format!("L {} {}", svg::ruby_float(left), svg::ruby_float(middle - 4.0)),
        ];
        if let Some(pointer) = pointer {
            outline.push(format!("L {} {}", svg::ruby_float(pointer.x1), svg::ruby_float(pointer.y1)));
        } else {
            outline.push(format!("L {} {}", svg::ruby_float(left), svg::ruby_float(middle - 4.0)));
        }
        outline.push(format!("L {} {}", svg::ruby_float(left), svg::ruby_float(middle + 4.0)));
        outline.push(format!("L {} {}", svg::ruby_float(left), svg::ruby_float(top + box_.height)));
        let right = box_.x + box_.width;
        let fold = 10.0;
        outline.push(format!("L {} {}", svg::ruby_float(right), svg::ruby_float(box_.y + box_.height)));
        outline.push(format!("L {} {}", svg::ruby_float(right), svg::ruby_float(box_.y + fold)));
        outline.push(format!("L {} {}", svg::ruby_float(right - fold), svg::ruby_float(box_.y)));
        outline.push("Z".to_string());
        let mut path = Elem::new("path");
        path = path.attr("d", outline.join(" "));
        path = path.attr("fill", note_fill(box_));
        path = path.attr("stroke", self.node_stroke());
        path = path.attr("stroke-width", self.node_width());
        path
    }

    fn rule_lines(&self, box_: &Box) -> Vec<Elem> {
        if box_.kind.ends_with("note") {
            return vec![];
        }
        box_
            .separators
            .iter()
            .map(|segment| {
                let mut line = Elem::new("line");
                line = line.num("x1", segment.x1 + RULE_INSET);
                line = line.num("y1", segment.y1);
                line = line.num("x2", segment.x2 - RULE_INSET);
                line = line.num("y2", segment.y2);
                line = line.attr("stroke", self.node_stroke());
                line = line.attr(
                    "stroke-width",
                    segment
                        .width
                        .map(svg::ruby_float)
                        .unwrap_or_else(|| RULE_WIDTH.to_string()),
                );
                if let Some(dash) = segment.dash {
                    line = line.attr("stroke-dasharray", dash);
                }
                line
            })
            .collect()
    }

    fn spot_elements(&self, box_: &Box) -> Vec<Elem> {
        let Some(spot) = &box_.spot else { return vec![] };
        let Some(kind) = kind_of(&spot.kind) else { return vec![] };
        let Some(glyph) = super::class_header::glyph(kind) else {
            return vec![];
        };
        let mut circle = Elem::new("circle");
        circle = circle.num("cx", spot.cx);
        circle = circle.num("cy", spot.cy);
        circle = circle.num("r", super::class_header::RADIUS);
        circle = circle.attr("fill", glyph.fill);
        circle = circle.attr("stroke", self.node_stroke());
        circle = circle.attr("stroke-width", "1");
        let mut path = Elem::new("path");
        path = path.attr("d", translate_outline(glyph.outline, spot.cx, spot.cy));
        path = path.attr("fill", "#000000");
        vec![circle, path]
    }

    fn text_element(&self, scene_text: &super::scene::Text) -> Elem {
        let mut text = Elem::new("text");
        text = text.num("x", scene_text.x);
        text = text.num("y", scene_text.y);
        text = text.content(&scene_text.content);
        text = text.attr("text-anchor", scene_text.anchor.clone());
        text = text.attr("fill", self.box_look(Prop::FontColour));
        text = text.attr("font-family", self.font_family());
        text = text.attr("font-size", self.font_size(scene_text.role));
        if BOLD_ROLES.contains(&scene_text.role) {
            text = text.attr("font-weight", "bold");
        }
        if self.italic(scene_text.role) {
            text = text.attr("font-style", "italic");
        }
        // Ruby `apply_creole` runs for the note roles; the corpus texts
        // carry no markup, so the content stays as written.
        if CREOLE_ROLES.contains(&scene_text.role) {
            let _ = scene_text;
        }
        text
    }

    fn italic(&self, role: &str) -> bool {
        if role == "kind" || role == "member_abstract" {
            return true;
        }
        role == "class_name"
            && self
                .current_spot_kind
                .as_deref()
                .map(|spot| ITALIC_SPOTS.contains(&spot))
                .unwrap_or(false)
    }

    fn font_family(&self) -> String {
        if let Some(written) = self.sheet.explicit("class", Prop::FontName, None, None) {
            return value_to_string(written);
        }
        self.box_look(Prop::FontName)
    }

    fn font_size(&self, role: &str) -> String {
        let normal = FONT_SIZE;
        let value = match role {
            "member_note" => normal * (13.0 / 14.0),
            "kind" => normal * 0.85,
            "multiplicity" | "relation_label" => normal * 0.9,
            _ => normal,
        };
        svg::ruby_float(value)
    }

    fn box_look(&self, property: Prop) -> String {
        let id = self.current_box.map(|box_| box_.id.as_str());
        let stereotype = self.stereotype_of(id);
        value_to_string(self.box_look_resolved(property, id, stereotype.as_deref()))
    }

    fn box_look_resolved(
        &self,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Value {
        self.sheet
            .resolve("class", property, id, stereotype)
            .or_else(|| super::style::from_defaults("class", property))
            .expect("class defaults carry every looked-up property")
    }

    fn box_look_number(&self, property: Prop) -> Option<f64> {
        match self.box_look_value(property) {
            Some(Value::Number(number)) => Some(number),
            _ => None,
        }
    }

    fn box_look_value(&self, property: Prop) -> Option<Value> {
        let id = self.current_box.map(|box_| box_.id.as_str());
        let stereotype = self.stereotype_of(id);
        self.sheet.resolve("class", property, id, stereotype.as_deref())
    }

    fn box_look_dash(&self) -> Option<String> {
        match self.box_look_value(Prop::LineStyle) {
            Some(Value::Style(dash)) => Some(dash.to_string()),
            _ => None,
        }
    }

    fn stereotype_of(&self, id: Option<&str>) -> Option<String> {
        let id = id?;
        self.sheet.element(id).get("stereotype").map(|value| match value {
            Value::Name(name) => name.clone(),
            Value::Colour(colour) => colour.clone(),
            other => value_to_string(other.clone()),
        })
    }

    fn node_fill(&self) -> String {
        value_to_string(
            self.sheet
                .resolve("class", Prop::Background, None, None)
                .or_else(|| super::style::from_defaults("class", Prop::Background))
                .expect("class background default"),
        )
    }

    fn node_stroke(&self) -> String {
        value_to_string(
            self.sheet
                .resolve("class", Prop::Border, None, None)
                .or_else(|| super::style::from_defaults("class", Prop::Border))
                .expect("class border default"),
        )
    }

    fn edge_colour(&self) -> String {
        value_to_string(
            self.sheet
                .resolve("arrow", Prop::Border, None, None)
                .or_else(|| super::style::from_defaults("arrow", Prop::Border))
                .expect("arrow border default"),
        )
    }

    fn node_width(&self) -> String {
        self.sheet
            .resolve("class", Prop::LineThickness, None, None)
            .map(|value| value_to_string(value))
            .unwrap_or_else(|| "0.5".to_string())
    }

    fn stroke_width(&self) -> String {
        self.sheet
            .resolve("arrow", Prop::LineThickness, None, None)
            .map(|value| value_to_string(value))
            .unwrap_or_else(|| "1".to_string())
    }

    fn render_frame(&self, frame: &super::scene::Frame) -> Elem {
        let mut group = Elem::new("g");
        group = group.attr("id", frame.id.clone());
        group = group.child(self.frame_rectangle(frame, frame.width, frame.height));
        if let Some(tab_width) = frame.tab_width {
            group = group.child(self.frame_rectangle(frame, tab_width, 22.0));
        }
        if frame.icon {
            let mut circle = Elem::new("circle");
            circle = circle.num("cx", frame.x + 12.0);
            circle = circle.num("cy", frame.y + 11.0);
            circle = circle.num("r", 3.0);
            circle = circle.attr("fill", "#84be84");
            circle = circle.attr("stroke", "#038048");
            group = group.child(circle);
        }
        for text in &frame.texts {
            group = group.child(self.text_element(text));
        }
        group
    }

    fn frame_rectangle(&self, frame: &super::scene::Frame, width: f64, height: f64) -> Elem {
        let mut rect = Elem::new("rect");
        rect = rect.num("x", frame.x);
        rect = rect.num("y", frame.y);
        rect = rect.num("width", width);
        rect = rect.num("height", height);
        rect = rect.attr("fill", frame.fill.clone().unwrap_or_else(|| "none".into()));
        rect = rect.attr("stroke", self.node_stroke());
        rect = rect.attr("stroke-width", self.node_width());
        rect
    }
}

fn kind_of(text: &str) -> Option<Kind> {
    match text {
        "class" => Some(Kind::Class),
        "abstract" => Some(Kind::Abstract),
        "interface" => Some(Kind::Interface),
        "enum" => Some(Kind::Enum),
        "annotation" => Some(Kind::Annotation),
        _ => None,
    }
}

fn value_to_string(value: Value) -> String {
    match value {
        Value::Colour(colour) => colour,
        Value::Name(name) => name,
        Value::Style(style) => style.to_string(),
        Value::Number(number) => svg::ruby_float(number),
        Value::Int(number) => number.to_string(),
    }
}

fn thickest_first(relations: &[SceneRelation]) -> Vec<&SceneRelation> {
    let mut indexed: Vec<(usize, &SceneRelation)> = relations.iter().enumerate().collect();
    indexed.sort_by(|(index_a, a), (index_b, b)| {
        let width_a = a.stroke_width.unwrap_or(0.0);
        let width_b = b.stroke_width.unwrap_or(0.0);
        width_b
            .partial_cmp(&width_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(index_a.cmp(index_b))
    });
    indexed.into_iter().map(|(_, relation)| relation).collect()
}

fn render_panel(panel: &super::scene::Panel) -> Elem {
    let mut group = Elem::new("g");
    group = group.attr("id", panel.id.clone());
    if panel.fill.is_some() || panel.bordered {
        let mut rect = Elem::new("rect");
        rect = rect.num("x", panel.x);
        rect = rect.num("y", panel.y);
        rect = rect.num("width", panel.width);
        rect = rect.num("height", panel.height);
        rect = rect.attr("fill", panel.fill.clone().unwrap_or_else(|| "none".into()));
        if panel.bordered {
            rect = rect.num("rx", 7.5);
            rect = rect.num("ry", 7.5);
            rect = rect.attr("stroke", "#000000");
            rect = rect.attr("stroke-width", "1");
        }
        group = group.child(rect);
    }
    let mut text = Elem::new("text");
    text = text.num("x", panel.x + panel.width / 2.0);
    text = text.num("y", panel.y + 5.0 + panel.font_size * 0.95);
    text = text.content(&panel.content);
    text = text.attr("text-anchor", "middle");
    text = text.attr(
        "fill",
        panel.colour.clone().unwrap_or_else(|| "#000000".to_string()),
    );
    text = text.attr("font-family", "sans-serif");
    text = text.attr("font-size", svg::ruby_float(panel.font_size));
    if panel.bold {
        text = text.attr("font-weight", "bold");
    }
    group = group.child(text);
    group
}

/// Ruby `MemberIconElements.group`.
fn member_icon_group(icon: &Icon) -> Elem {
    let modifier = format!(
        "{}_{}",
        match icon.visibility.as_str() {
            "public" => "PUBLIC",
            "private" => "PRIVATE",
            "protected" => "PROTECTED",
            _ => "PACKAGE_PRIVATE",
        },
        if icon.filled { "METHOD" } else { "FIELD" }
    );
    let mut group = Elem::new("g");
    group = group.attr("data-visibility-modifier", modifier);
    group = group.child(member_icon_element(icon));
    group
}

fn member_icon_element(icon: &Icon) -> Elem {
    let (stroke, fill) = match icon.visibility.as_str() {
        "public" => ("#038048", "#84BE84"),
        "private" => ("#C82930", "#F24D5C"),
        "protected" => ("#B38D22", "#FFFF44"),
        _ => ("#1963A0", "#4177AF"),
    };
    let fill = if icon.filled { fill } else { "none" };
    if icon.shape == "circle" {
        let mut circle = Elem::new("circle");
        circle = circle.num("cx", icon.cx);
        circle = circle.num("cy", icon.cy);
        circle = circle.num("r", 3.0);
        circle = circle.attr("fill", fill);
        circle = circle.attr("stroke", stroke);
        circle = circle.attr("stroke-width", "1");
        return circle;
    }
    if icon.shape == "square" {
        let mut rect = Elem::new("rect");
        rect = rect.num("x", icon.cx - 3.0);
        rect = rect.num("y", icon.cy - 3.0);
        rect = rect.num("width", 6.0);
        rect = rect.num("height", 6.0);
        rect = rect.attr("fill", fill);
        rect = rect.attr("stroke", stroke);
        rect = rect.attr("stroke-width", "1");
        return rect;
    }
    let half: f64 = if icon.shape == "diamond" { 4.0 } else { 4.0 };
    let points: Vec<[f64; 2]> = if icon.shape == "diamond" {
        vec![
            [icon.cx, icon.cy - half],
            [icon.cx + half, icon.cy],
            [icon.cx, icon.cy + half],
            [icon.cx - half, icon.cy],
        ]
    } else {
        vec![
            [icon.cx, icon.cy - 3.0],
            [icon.cx - half, icon.cy + 3.0],
            [icon.cx + half, icon.cy + 3.0],
        ]
    };
    let mut polygon = Elem::new("polygon");
    polygon = polygon.attr(
        "points",
        points
            .iter()
            .map(|p| format!("{},{}", svg::ruby_float(p[0]), svg::ruby_float(p[1])))
            .collect::<Vec<_>>()
            .join(" "),
    );
    polygon = polygon.attr("fill", fill);
    polygon = polygon.attr("stroke", stroke);
    polygon = polygon.attr("stroke-width", "1");
    polygon
}

/// Ruby `NoteOutline.call`.
fn note_outline(box_: &Box) -> String {
    let left = box_.x;
    let top = box_.y;
    let right = left + box_.width;
    let bottom = top + box_.height;
    let fold = 10.0;
    let pointer = box_.separators.first();
    let mut points: Vec<(f64, f64)> = vec![(left, top)];
    // A tail on the edge the pointer lands on, in drawing order.
    if let Some(pointer) = pointer {
        if on_edge(pointer.y2, top) {
            let half = half_for(top_edge_direction(pointer));
            let along = pointer.x2;
            points.push((along - half, top));
            points.push((pointer.x1, pointer.y1));
            points.push((along + half, top));
        }
    }
    points.push((right - fold, top));
    points.push((right, top + fold));
    if let Some(pointer) = pointer {
        if on_edge(pointer.x2, right) {
            let half = if bottom > top + fold { 4.0 } else { -4.0 };
            let along = pointer.y2;
            points.push((right, along - half));
            points.push((pointer.x1, pointer.y1));
            points.push((right, along + half));
        }
    }
    points.push((right, bottom));
    if let Some(pointer) = pointer {
        if on_edge(pointer.x2, left) {
            let half = if top > bottom { 4.0 } else { -4.0 };
            let along = pointer.y2;
            points.push((left, along - half));
            points.push((pointer.x1, pointer.y1));
            points.push((left, along + half));
        }
    }
    points.push((left, bottom));
    if let Some(pointer) = pointer {
        if on_edge(pointer.y2, bottom) {
            let half = if left > right { 4.0 } else { -4.0 };
            let along = pointer.x2;
            points.push((along - half, bottom));
            points.push((pointer.x1, pointer.y1));
            points.push((along + half, bottom));
        }
    }
    let commands: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            format!("{} {} {}", if index == 0 { "M" } else { "L" }, g(*x), g(*y))
        })
        .collect();
    format!("{} Z", commands.join(" "))
}

fn top_edge_direction(_pointer: &super::scene::Segment) -> (f64, f64) {
    (0.0, 1.0)
}

fn half_for(_direction: (f64, f64)) -> f64 {
    4.0
}

fn on_edge(value: f64, edge: f64) -> bool {
    (value - edge).abs() < 0.001
}

/// Ruby `note_fill(box)`.
fn note_fill(box_: &Box) -> String {
    let Some(colour) = &box_.fill else {
        return "#FEFFDD".to_string();
    };
    let colour = colour.strip_prefix('#').unwrap_or(colour);
    if (colour.len() == 3 || colour.len() == 6)
        && colour.chars().all(|c| c.is_ascii_hexdigit())
    {
        return format!("#{colour}");
    }
    colour.to_string()
}

/// Ruby `translate_outline`: the glyph outline moved to the spot, each
/// coordinate rounded to four decimals and printed like `%g`.
fn translate_outline(outline: &str, cx: f64, cy: f64) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(-?[\d.]+),(-?[\d.]+)").unwrap());
    re.replace_all(outline, |captures: &regex::Captures| {
        let x: f64 = captures[1].parse().unwrap_or(0.0);
        let y: f64 = captures[2].parse().unwrap_or(0.0);
        format!(
            "{},{}",
            g(round4(x + cx)),
            g(round4(y + cy))
        )
    })
    .to_string()
}

/// Ruby `Float#round(4)`.
fn round4(value: f64) -> f64 {
    super::layout::round4_half(value)
}

/// Ruby `format('%g', x)`.
fn g(value: f64) -> String {
    super::layout::format_g(value)
}
