//! Port of Ruby `Notation::PlantUML::Layout` (`layout.rb`): positions the
//! class boxes and relations in final canvas space. The IR round trip is
//! bypassed — the Diagram goes straight to the placement the Ruby reader
//! rebuilds.
use super::captions::CaptionLayout;
use super::chrome::{member_icon, member_pitch, member_rule, stereotype_head};
use super::edges::{
    end_texts::{EndTexts, EntryKind},
    label_clearance, label_separator, qualifier_box, relation_path_points,
    styled_relation_curve,
};
use super::frames::PackageFrames;
use super::measures;
use super::model::{Diagram, Direction, Kind, Member, MemberKind, Note, NoteSide, Package, Relation};
use super::notes::NoteLayout;
use super::qualified::{qualifier, QualifiedAssociationLayout};
use super::rank_chain::{self, Engine, Placer};
use super::rank_hierarchy::RankHierarchy;
use super::reader_support::class_name;
use super::scene::{Box, Marker, Relation as SceneRelation, Scene, Segment, Spot, Text};
use crate::js_math;
use std::collections::HashMap;

const MARGIN: f64 = 7.0;
const COLUMN_GAP: f64 = 90.0;
const ROW_GAP: f64 = 90.0;
const ANNOTATED_RELATION_SLOT_WIDTH: f64 = 160.0;
const BOX_PADDING: f64 = 12.0;
const ROW_HEIGHT: f64 = 22.0;
const COMPACT_BOTTOM_PAD: f64 = 6.95;
const COMPACT_HEAD_GAP: f64 = 17.5;
const COMPACT_GROUP_GAP: f64 = 8.0;
const PROPORTIONAL_MEMBER_BASE_HEIGHT: f64 = 40.0;
const COMPACT_RULE_BASE_HEIGHT: f64 = 34.488;
const MARKER_LENGTH: f64 = 12.0;
const THICK_LINE: f64 = 4.0;
const JUNCTION_DOT_RADIUS: f64 = 2.0;
const MARKER_HALF_WIDTH: f64 = 7.0;
const LABEL_OFFSET: f64 = 10.0;
const LABEL_GAP: f64 = 2.0;
const BESIDE_LABEL_SLOPE: f64 = 0.8;
const END_TEXT_ACROSS: f64 = 3.0;
const END_TEXT_ALONG: f64 = 1.0;
const TEXT_DESCENT: f64 = 0.25;
const SMALL_TEXT_SCALE: f64 = 0.9;
const NESTING_RADIUS: f64 = 8.0;
const MEMBER_NOTE_TEXT_SCALE: f64 = 1.15;
const MEMBER_NOTE_ROW_HEIGHT: f64 = 16.48828125;
const MEMBER_NOTE_LINE_HEIGHT: f64 = 15.310546875;
const MEMBER_NOTE_STACK_GAP: f64 = 10.0;
const MEMBER_NOTE_BODY_GAP: f64 = 35.0;
const CLASS_NOTE_GAP: f64 = 35.0;
const MEMBER_NOTE_CLASS_HEIGHT: f64 = 48.0;
const MEMBER_NOTE_SIDE_PADDING: f64 = 6.0;
const STANDARD_MEMBER_SIDE_ALLOWANCE: f64 = 20.0;
const STANDARD_STEREOTYPE_ROW_HEIGHT: f64 = 8.62109375;
const STANDARD_TITLE_SIDE_ALLOWANCE: f64 = 32.0;
const MULTILINE_TITLE_ROW_HEIGHT: f64 = 16.48828125;
const MULTILINE_TITLE_VERTICAL_ALLOWANCE: f64 = 26.0;
const MULTILINE_TITLE_BASELINE: f64 = 18.53515625;
const MULTILINE_TITLE_SIDE_ALLOWANCE: f64 = 36.0;
const SELF_RELATION_START_RATIO: f64 = 0.37;
const SELF_RELATION_END_RATIO: f64 = 0.635;
const SELF_RELATION_CONTROL_OFFSET: f64 = 70.0;
const SELF_RELATION_LABEL_GAP: f64 = 6.0;
const EMPTY_MEMBER_COMPARTMENT: f64 = 14.0;
const EMPTY_COMPARTMENTS_EXTRA: f64 = 2.0;
const AXIS_TOLERANCE: f64 = 0.5;
const EMPTY_CLASS_WIDTH_EXTRA: f64 = 8.0;
const EMPTY_CLASS_MIN_WIDTH: f64 = 41.5;
const HIDE_CIRCLE_BASE_HEIGHT: f64 = 42.48828125;
const HIDE_CIRCLE_TITLE_ALLOWANCE: f64 = 6.0;
const HIDE_CIRCLE_MEMBER_ALLOWANCE: f64 = 12.0;
const ISOLATED_KIND_COLUMN_GAP: f64 = 35.0;
const ISOLATED_KIND_ROW_GAP: f64 = 60.0;
const ISOLATED_KIND_MEMBER_ALLOWANCE: f64 = 26.0;
const HIDDEN_MEMBER_EMPTY_HEIGHT: f64 = 32.0;
const RESHOWN_MEMBER_BASE_HEIGHT: f64 = 40.0;
const QUALIFIED_GENERIC_FONT_SIZE: f64 = 12.0;
const QUALIFIED_GENERIC_GAP: f64 = 7.0;
const QUALIFIED_BOX_HEIGHT: f64 = 48.0;
const QUALIFIED_GENERIC_HEIGHT: f64 = 51.0;
const QUALIFIED_QUALIFIER_ALLOWANCE: f64 = 40.0;
const COMPACT_QUALIFIER_ALLOWANCE: f64 = 20.0;
const QUALIFIED_STEEP_ARROW_OFFSET: f64 = 37.0;
const QUALIFIED_LONG_CURVE_OFFSET: f64 = 210.0;
const QUALIFIED_FLAT_CURVE_OFFSET: f64 = 30.0;
const QUALIFIED_FAN_LANE_ALLOWANCE: f64 = 4.0;
const QUALIFIED_DOUBLE_END_HALF_WIDTH: f64 = 10.0;
const QUALIFIED_INHERITANCE_HALF_WIDTH: f64 = 6.0;
const PACKAGE_CANVAS_MARGIN: f64 = 6.0;
const PACKAGE_OUTER_GAP: f64 = 24.0;
const PACKAGE_CANVAS_EDGE: f64 = 15.0;
const ARROW_LENGTH: f64 = 9.0;
const ARROW_HALF_WIDTH: f64 = 4.0;
const ARROW_NOTCH: f64 = 5.0;
const DIAMOND_LENGTH: f64 = 12.0;
const DIAMOND_HALF_WIDTH: f64 = 4.0;

const FONT_SIZE: f64 = 14.0;

/// The specification row of a box under construction (a Ruby hash here).
#[derive(Clone, Debug)]
struct Spec {
    id: String,
    note: Option<Note>,
    width: f64,
    height: f64,
    title_rows: Vec<String>,
    member_rows: Vec<String>,
    member_rows_count: usize,
    name_rows: usize,
    floating_generic: Option<String>,
    compact_members: bool,
    compact_title_rows: bool,
    stereotype_head: Option<(String, [f64; 2])>,
    member_modifiers: Vec<Vec<&'static str>>,
    package: Option<String>,
    member_kinds: Vec<MemberKind>,
    member_visibilities: Vec<Option<super::model::Visibility>>,
    icons_enabled: bool,
    empty_gaps: bool,
    chain: Vec<String>,
    compact_bottom: f64,
}

impl Spec {
    fn from_class(klass: &super::model::Klass, diagram: &Diagram) -> Spec {
        Spec {
            id: klass.name.clone(),
            note: None,
            width: 0.0,
            height: 0.0,
            title_rows: vec![],
            member_rows: vec![],
            member_rows_count: 0,
            name_rows: 0,
            floating_generic: None,
            compact_members: false,
            compact_title_rows: false,
            stereotype_head: None,
            member_modifiers: vec![],
            package: klass.package.clone(),
            member_kinds: vec![],
            member_visibilities: vec![],
            icons_enabled: false,
            empty_gaps: true,
            chain: diagram.package_chain(&klass.package),
            compact_bottom: 0.0,
        }
    }
}

pub fn scene(diagram: &Diagram) -> Scene {
    let mut layout = Layout {
        kind_in_spot: !circles_hidden(diagram),
        qualified_layout_active: false,
        qualified_route_middle: 0.0,
    };
    let body = if member_note_family(diagram) {
        layout.member_note_scene(diagram)
    } else if class_note_pair_family(diagram) {
        layout.class_note_pair_scene(diagram)
    } else {
        layout.standard_scene(diagram)
    };
    let mut scene = body;
    layout.dress(diagram, &mut scene);
    scene
}

fn circles_hidden(diagram: &Diagram) -> bool {
    static HIDE_CIRCLE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let hide_circle = HIDE_CIRCLE.get_or_init(|| regex::Regex::new(r"(?i)\Ahide[ \t]+circle\z").unwrap());
    diagram.directives.iter().any(|line| hide_circle.is_match(line))
}

struct Layout {
    kind_in_spot: bool,
    qualified_layout_active: bool,
    qualified_route_middle: f64,
}

impl Layout {
    fn font_size(&self) -> f64 {
        FONT_SIZE
    }

    /// Ruby `measure_text(..)[:width]` (the Arial measurement).
    fn measure(&self, text: &str, font_size: f64) -> f64 {
        measures::measured(text, font_size)
    }

    fn measured_width(&self, text: &str) -> f64 {
        measures::measured_monospace(text, FONT_SIZE)
    }

    /// Ruby `standard_text_width`: the DejaVu reference face.
    fn standard_text_width(&self, text: &str, size: f64) -> f64 {
        measures::face_widest_line(text, size, false)
    }

    // ---- family selection ----

    fn standard_scene(&mut self, diagram: &Diagram) -> Scene {
        let specifications = self.box_specifications(diagram);
        if let Some(hidden) = self.hide_members_grid_scene(diagram, &specifications) {
            return hidden;
        }
        if let Some(isolated) = self.isolated_class_kind_scene(diagram, &specifications) {
            return isolated;
        }
        if let Some(qualified) = self.qualified_scene(diagram, &specifications) {
            return qualified;
        }
        self.standard_box_scene(diagram, specifications)
    }

    fn hide_members_grid_family(&self, diagram: &Diagram) -> bool {
        diagram.classes.len() > 1
            && diagram.notes.is_empty()
            && standalone_diagram(diagram)
            && diagram.directives.iter().any(|line| line == "hide members")
            && diagram.classes.iter().any(|klass| !klass.body.is_empty())
    }

    fn hide_members_grid_scene(
        &mut self,
        diagram: &Diagram,
        specifications: &[Spec],
    ) -> Option<Scene> {
        if !self.hide_members_grid_family(diagram) {
            return None;
        }
        let visible: Vec<&Spec> = specifications
            .iter()
            .filter(|item| !item.member_rows.is_empty())
            .collect();
        let hidden: Vec<&Spec> = specifications
            .iter()
            .filter(|item| item.member_rows.is_empty())
            .collect();
        let mut ordered: Vec<&Spec> = visible;
        ordered.extend(hidden);
        let rows: Vec<Vec<&Spec>> = ordered.chunks(2).map(|chunk| chunk.to_vec()).collect();
        let boxes = self.hide_members_grid_boxes(&rows);
        let (width, height) = chain_dimensions(&boxes);
        Some(Scene {
            width,
            height,
            boxes,
            relations: vec![],
            frames: vec![],
            ..Scene::default()
        })
    }

    fn hide_members_grid_boxes(&self, rows: &[Vec<&Spec>]) -> Vec<Box> {
        let row_widths: Vec<f64> = rows.iter().map(|row| hide_members_row_width(row)).collect();
        let widest = row_widths.iter().cloned().fold(0.0_f64, f64::max);
        let tops = isolated_class_kind_row_tops(rows);
        let mut boxes = vec![];
        for (index, row) in rows.iter().enumerate() {
            let left = MARGIN + (widest - row_widths[index]) / 2.0;
            let height = row.iter().map(|item| item.height).fold(0.0_f64, f64::max);
            let mut cursor = left;
            for item in row.iter() {
                let y = tops[index] + (height - item.height) / 2.0;
                boxes.push(self.build_box(item, cursor, y, item.width));
                cursor += item.width + ISOLATED_KIND_COLUMN_GAP;
            }
        }
        boxes
    }

    fn isolated_class_kind_scene(
        &mut self,
        diagram: &Diagram,
        specifications: &[Spec],
    ) -> Option<Scene> {
        if !isolated_class_kind_family(diagram) && !loose_member_class_family(diagram) {
            return None;
        }
        let owned: Vec<Spec> = specifications.to_vec();
        let rows: Vec<Vec<Spec>> = owned.chunks(2).map(|chunk| chunk.to_vec()).collect();
        let boxes = self.isolated_class_kind_boxes(&rows);
        let (width, height) = chain_dimensions(&boxes);
        Some(Scene {
            width,
            height,
            boxes,
            relations: vec![],
            frames: vec![],
            ..Scene::default()
        })
    }

    fn isolated_class_kind_boxes(&self, rows: &[Vec<Spec>]) -> Vec<Box> {
        let widths = isolated_class_kind_column_widths(rows);
        let tops = isolated_class_kind_row_tops_owned(rows);
        let mut boxes = vec![];
        for (row_index, row) in rows.iter().enumerate() {
            let lefts = isolated_class_kind_column_lefts(&widths);
            let height = row.iter().map(|item| item.height).fold(0.0_f64, f64::max);
            for (column, item) in row.iter().enumerate() {
                let left = lefts[column];
                let cell_width = widths[column];
                let origin = [
                    left + (cell_width - item.width) / 2.0,
                    tops[row_index] + (height - item.height) / 2.0,
                ];
                boxes.push(self.build_box(item, origin[0], origin[1], item.width));
            }
        }
        boxes
    }

    fn standard_box_scene(&mut self, diagram: &Diagram, specifications: Vec<Spec>) -> Scene {
        if let Some(placer) = Placer::for_diagram(diagram) {
            return self.chain_scene(diagram, placer, specifications);
        }
        if !diagram.notes.is_empty() {
            return self.noted_scene(diagram, specifications);
        }
        if let Some(hierarchy) = RankHierarchy::for_diagram(diagram) {
            return self.hierarchy_scene(diagram, hierarchy, specifications);
        }
        self.grid_scene(diagram, specifications)
    }

    fn grid_scene(&mut self, diagram: &Diagram, specifications: Vec<Spec>) -> Scene {
        let box_width = widest_box(&specifications);
        let mut boxes = self.position_boxes(
            rows_by_package(&specifications, diagram),
            box_width,
            stable_centers(diagram),
            intrinsic_box_widths(diagram),
            diagram,
        );
        let specs_owned = specifications;
        boxes = self.align_package_relation_targets(diagram, boxes, &specs_owned);
        self.build_scene(diagram, boxes)
    }

    fn box_specifications(&mut self, diagram: &Diagram) -> Vec<Spec> {
        let mut classes: Vec<Spec> = diagram
            .classes
            .iter()
            .map(|klass| self.box_specification(klass, diagram))
            .collect();
        classes.extend(note_specifications(&diagram.notes, &|text| self.measured_width(text)));
        classes
    }

    fn qualified_scene(&mut self, diagram: &Diagram, specifications: &[Spec]) -> Option<Scene> {
        let qualified = QualifiedAssociationLayout::for_diagram(diagram)?;
        let sizes: HashMap<String, [f64; 2]> = specifications
            .iter()
            .map(|item| (item.id.clone(), [item.width, item.height]))
            .collect();
        let constrained = qualified.constrained_sizes(&sizes);
        let mut specs: Vec<Spec> = specifications.to_vec();
        for item in specs.iter_mut() {
            let size = constrained[&item.id];
            item.width = size[0];
            item.height = size[1];
        }
        let corners = qualified.place(&constrained, [MARGIN, MARGIN]);
        let boxes = self.chain_boxes(&specs, &corners, &constrained);
        let (width, height) = chain_dimensions(&boxes);
        self.qualified_layout_active = true;
        self.qualified_route_middle = qualified_route_middle(&boxes);
        let relations = self.scene_relations(diagram, &boxes, &HashMap::new());
        Some(Scene {
            width,
            height,
            boxes,
            frames: vec![],
            relations,
            ..Scene::default()
        })
    }

    fn chain_scene(
        &mut self,
        diagram: &Diagram,
        placer: Placer,
        specifications: Vec<Spec>,
    ) -> Scene {
        let sizes = self.chain_sizes(&specifications, diagram);
        let label_widths = self.label_widths(diagram);
        let (corners, nodes) = placer.place(diagram, &sizes, [MARGIN, MARGIN], &label_widths);
        let boxes = self.chain_boxes(&specifications, &corners, &sizes);
        let (width, height) = chain_dimensions(&boxes);
        let relations = self.scene_relations(diagram, &boxes, &nodes);
        Scene {
            width,
            height,
            boxes,
            frames: vec![],
            relations,
            ..Scene::default()
        }
    }

    fn hierarchy_scene(
        &mut self,
        diagram: &Diagram,
        hierarchy: RankHierarchy,
        specifications: Vec<Spec>,
    ) -> Scene {
        let items: Vec<Spec> = specifications.into_iter().filter(|item| item.note.is_none()).collect();
        let sizes: HashMap<String, [f64; 2]> = items
            .iter()
            .map(|item| (item.id.clone(), [item.width, item.height]))
            .collect();
        let corners = hierarchy.place(&sizes, [MARGIN, MARGIN]);
        let boxes = self.chain_boxes(&items, &corners, &sizes);
        let (width, height) = chain_dimensions(&boxes);
        let relations = self.scene_relations(diagram, &boxes, &HashMap::new());
        Scene {
            width,
            height,
            boxes,
            frames: vec![],
            relations,
            ..Scene::default()
        }
    }

    fn noted_scene(&mut self, diagram: &Diagram, specifications: Vec<Spec>) -> Scene {
        let items: Vec<Spec> = specifications.into_iter().filter(|item| item.note.is_none()).collect();
        let (sizes, corners) = self.noted_class_corners(diagram, &items);
        let (width, height, boxes) = self.margined_boxes(diagram, &items, &sizes, &corners);
        let relations = self.scene_relations(diagram, &boxes, &HashMap::new());
        Scene {
            width,
            height,
            boxes,
            frames: vec![],
            relations,
            ..Scene::default()
        }
    }

    fn margined_boxes(
        &mut self,
        diagram: &Diagram,
        items: &[Spec],
        sizes: &HashMap<String, [f64; 2]>,
        corners: &HashMap<String, [f64; 2]>,
    ) -> (f64, f64, Vec<Box>) {
        let boxes = self.chain_boxes(items, corners, sizes);
        let mut noted = self.noted_boxes(diagram, &boxes);
        let mut all = boxes.clone();
        all.append(&mut noted);
        let shift = [
            MARGIN - all.iter().map(|box_| box_.x).fold(f64::INFINITY, f64::min),
            MARGIN - all.iter().map(|box_| box_.y).fold(f64::INFINITY, f64::min),
        ];
        let shifted: HashMap<String, [f64; 2]> = corners
            .iter()
            .map(|(name, corner)| (name.clone(), [corner[0] + shift[0], corner[1] + shift[1]]))
            .collect();
        let boxes = self.chain_boxes(items, &shifted, sizes);
        let mut boxes = boxes;
        boxes.extend(self.noted_boxes(diagram, &boxes));
        let (width, height) = chain_dimensions(&boxes);
        (width, height, boxes)
    }

    fn noted_class_corners(
        &self,
        diagram: &Diagram,
        items: &[Spec],
    ) -> (HashMap<String, [f64; 2]>, HashMap<String, [f64; 2]>) {
        if let Some(hierarchy) = RankHierarchy::for_diagram(diagram) {
            let sizes: HashMap<String, [f64; 2]> = items
                .iter()
                .map(|item| (item.id.clone(), [item.width, item.height]))
                .collect();
            let corners = hierarchy.place(&sizes, [MARGIN, MARGIN]);
            return (sizes, corners);
        }
        let rows = rows_by_package(items, diagram);
        let placed = self.position_boxes(
            rows,
            widest_box(items),
            stable_centers(diagram),
            intrinsic_box_widths(diagram),
            diagram,
        );
        let sizes: HashMap<String, [f64; 2]> = placed
            .iter()
            .map(|box_| (box_.id.clone(), [box_.width, box_.height]))
            .collect();
        let corners: HashMap<String, [f64; 2]> = placed
            .iter()
            .map(|box_| (box_.id.clone(), [box_.x, box_.y]))
            .collect();
        (sizes, corners)
    }

    fn noted_boxes(&self, diagram: &Diagram, boxes: &[Box]) -> Vec<Box> {
        let note_line_width =
            |text: &str| self.measure(text, FONT_SIZE * (13.0 / 14.0));
        let member_text_width = |text: &str| self.standard_text_width(text, FONT_SIZE);
        NoteLayout::new(diagram, &note_line_width, &member_text_width).place(boxes)
    }

    fn chain_sizes(&self, specifications: &[Spec], diagram: &Diagram) -> HashMap<String, [f64; 2]> {
        let widest = widest_box(specifications);
        let intrinsic = intrinsic_box_widths(diagram);
        specifications
            .iter()
            .map(|item| {
                let width = if compact_empty_item(item, diagram) || intrinsic {
                    item.width
                } else {
                    widest
                };
                (item.id.clone(), [width, item.height])
            })
            .collect()
    }

    fn chain_boxes(
        &self,
        specifications: &[Spec],
        corners: &HashMap<String, [f64; 2]>,
        sizes: &HashMap<String, [f64; 2]>,
    ) -> Vec<Box> {
        specifications
            .iter()
            .map(|item| {
                let Some(corner) = corners.get(&item.id) else {
                    eprintln!("MISSING corner for {} in {:?} sizes {:?}", item.id, corners.keys().collect::<Vec<_>>(), sizes.keys().collect::<Vec<_>>());
                    panic!("missing corner");
                };
                self.build_box(item, corner[0], corner[1], sizes[&item.id][0])
            })
            .collect()
    }

    fn label_widths(&self, diagram: &Diagram) -> HashMap<usize, f64> {
        let mut out = HashMap::new();
        for (index, relation) in diagram.relations.iter().enumerate() {
            let Some(label) = visible_relation_label(relation.label.as_deref()) else {
                continue;
            };
            out.insert(index, self.end_text_width(&label));
        }
        out
    }

    fn end_text_width(&self, content: &str) -> f64 {
        self.measure(content, FONT_SIZE * SMALL_TEXT_SCALE)
    }

    // ---- member-note and class-note-pair families ----

    fn member_note_scene(&mut self, diagram: &Diagram) -> Scene {
        let klass = &diagram.classes[0];
        let class_box = self.member_note_class_box(klass);
        let notes = self.member_note_boxes(diagram, klass, &class_box);
        let mut boxes = vec![class_box];
        boxes.extend(notes);
        boxes_only_scene(boxes)
    }

    fn member_note_class_box(&self, klass: &super::model::Klass) -> Box {
        let width = self.member_note_class_width(klass);
        let height = MEMBER_NOTE_CLASS_HEIGHT + klass.body.len() as f64 * MEMBER_NOTE_ROW_HEIGHT;
        let texts = self.member_note_class_texts(klass, width);
        let separator_y = MARGIN + MEMBER_NOTE_CLASS_HEIGHT - MEMBER_NOTE_ROW_HEIGHT;
        Box {
            id: klass.name.clone(),
            x: MARGIN,
            y: MARGIN,
            width,
            height,
            texts,
            separators: vec![Segment::new(MARGIN, separator_y, MARGIN + width, separator_y)],
            kind: "class",
            fill: None,
            spot: None,
        }
    }

    fn member_note_class_width(&self, klass: &super::model::Klass) -> f64 {
        let name = self.plantuml_text_width(&klass.name) + 32.0;
        let mut widths = klass
            .body
            .iter()
            .map(|member| {
                self.plantuml_text_width(&member_text(member))
                    + MEMBER_NOTE_SIDE_PADDING * 2.0
            })
            .collect::<Vec<_>>();
        widths.push(name);
        widths.iter().cloned().fold(0.0_f64, f64::max)
    }

    fn member_note_class_texts(&self, klass: &super::model::Klass, width: f64) -> Vec<Text> {
        let title = Text::new(&klass.name, MARGIN + width / 2.0, MARGIN + 21.291, "class_name", "middle");
        let members: Vec<Text> = klass
            .body
            .iter()
            .enumerate()
            .map(|(index, member)| self.member_note_row_text(member, index))
            .collect();
        let mut texts = vec![title];
        texts.extend(members);
        texts
    }

    fn member_note_row_text(&self, member: &Member, index: usize) -> Text {
        let vertical = MARGIN + 49.535 + index as f64 * MEMBER_NOTE_ROW_HEIGHT;
        let role = if member.type_position == super::model::TypePosition::Prefix {
            "member_prefix"
        } else {
            "member"
        };
        Text::new(
            &member_text(member),
            MARGIN + MEMBER_NOTE_SIDE_PADDING,
            vertical,
            role,
            "start",
        )
    }

    fn member_note_boxes(&self, diagram: &Diagram, klass: &super::model::Klass, class_box: &Box) -> Vec<Box> {
        let heights: Vec<f64> = diagram.notes.iter().map(member_note_height).collect();
        let total = crate::render::pie::fsum(heights.clone())
            + MEMBER_NOTE_STACK_GAP * (heights.len().saturating_sub(1)) as f64;
        let first = class_box.y + (class_box.height - total) / 2.0 - 5.0;
        let mut tops = vec![first];
        for height in &heights {
            let next = *tops.last().unwrap() + height + MEMBER_NOTE_STACK_GAP;
            tops.push(next);
        }
        let fill = self.shared_member_note_fill(diagram);
        let mut boxes = vec![];
        for (index, note) in diagram.notes.iter().enumerate() {
            let mut box_ = self.member_note_box(note, index, klass, class_box, tops[index]);
            box_.fill = fill.clone();
            boxes.push(box_);
        }
        boxes
    }

    fn member_note_box(
        &self,
        note: &Note,
        index: usize,
        klass: &super::model::Klass,
        class_box: &Box,
        top: f64,
    ) -> Box {
        let height = member_note_height(note);
        let left = class_box.x + class_box.width + MEMBER_NOTE_BODY_GAP;
        let anchor = [left, top + height / 2.0];
        let target = self.member_note_target(note, klass, class_box);
        let pointer = Segment::new(target[0], target[1], anchor[0], anchor[1]);
        let width = self.member_note_width(note);
        Box {
            id: format!("note-{index}"),
            x: left,
            y: top,
            width,
            height,
            texts: self.member_note_texts(note, left, top),
            separators: vec![pointer],
            kind: "member_note",
            fill: None,
            spot: None,
        }
    }

    fn member_note_width(&self, note: &Note) -> f64 {
        let widest = note
            .lines
            .iter()
            .map(|line| self.plantuml_note_width(line))
            .fold(0.0_f64, f64::max);
        widest + 21.0
    }

    fn member_note_target(&self, note: &Note, klass: &super::model::Klass, class_box: &Box) -> [f64; 2] {
        let index = klass
            .body
            .iter()
            .position(|member| member.name == note.member.clone().unwrap_or_default())
            .unwrap_or(0);
        let member = &klass.body[index];
        [
            class_box.x + MEMBER_NOTE_SIDE_PADDING + self.plantuml_text_width(&member_text(member)),
            class_box.y + 44.244 + index as f64 * MEMBER_NOTE_ROW_HEIGHT,
        ]
    }

    fn member_note_texts(&self, note: &Note, left: f64, top: f64) -> Vec<Text> {
        note.lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                Text::new(
                    line,
                    left + 6.0,
                    top + 17.568 + index as f64 * MEMBER_NOTE_LINE_HEIGHT,
                    "member_note",
                    "start",
                )
            })
            .collect()
    }

    fn shared_member_note_fill(&self, diagram: &Diagram) -> Option<String> {
        let note = diagram.notes.last()?;
        note.color.clone().or_else(|| stereotype_note_fill(&note.stereotype, &diagram.directives))
    }

    fn plantuml_text_width(&self, text: &str) -> f64 {
        self.measure(text, FONT_SIZE) * MEMBER_NOTE_TEXT_SCALE
    }

    fn plantuml_note_width(&self, text: &str) -> f64 {
        self.measure(text, FONT_SIZE * (13.0 / 14.0)) * MEMBER_NOTE_TEXT_SCALE
    }

    fn class_note_pair_scene(&mut self, diagram: &Diagram) -> Scene {
        let specifications: HashMap<String, Spec> = diagram
            .classes
            .iter()
            .map(|klass| (klass.name.clone(), self.box_specification(klass, diagram)))
            .collect();
        let notes: HashMap<String, (Note, usize)> = diagram
            .notes
            .iter()
            .enumerate()
            .map(|(index, note)| (note.target.clone().unwrap_or_default(), (note.clone(), index)))
            .collect();
        let boxes = self.place_class_note_pairs(diagram, &specifications, &notes);
        boxes_only_scene(boxes)
    }

    fn place_class_note_pairs(
        &mut self,
        diagram: &Diagram,
        specifications: &HashMap<String, Spec>,
        notes: &HashMap<String, (Note, usize)>,
    ) -> Vec<Box> {
        let mut cursor = MARGIN;
        let mut boxes = vec![];
        for klass in &diagram.classes {
            let item = &specifications[&klass.name];
            let class_box = self.build_box(item, cursor, MARGIN, item.width);
            let (note, index) = &notes[&klass.name];
            let note_box = self.class_level_note_box(note, *index, &class_box);
            cursor = note_box.x + note_box.width + CLASS_NOTE_GAP;
            boxes.push(class_box);
            boxes.push(note_box);
        }
        boxes
    }

    fn class_level_note_box(&self, note: &Note, index: usize, class_box: &Box) -> Box {
        let height = member_note_height(note);
        let x = class_box.x + class_box.width + CLASS_NOTE_GAP;
        let y = class_box.y + (class_box.height - height) / 2.0;
        let width = self.class_level_note_width(note);
        let pointer = Segment::new(
            class_box.x + class_box.width,
            class_box.y + class_box.height / 2.0,
            x,
            y + height / 2.0,
        );
        Box {
            id: format!("note-{index}"),
            x,
            y,
            width,
            height,
            texts: self.member_note_texts(note, x, y),
            separators: vec![pointer],
            kind: "member_note",
            fill: note.color.clone(),
            spot: None,
        }
    }

    fn class_level_note_width(&self, note: &Note) -> f64 {
        let size = FONT_SIZE * (13.0 / 14.0);
        let widest = note
            .lines
            .iter()
            .map(|line| measures::face_widest_line(line, size, false))
            .fold(0.0_f64, f64::max);
        widest + 21.0
    }

    // ---- box specifications ----

    fn box_specification(&mut self, klass: &super::model::Klass, diagram: &Diagram) -> Spec {
        let member_rows: Vec<String> = klass.body.iter().map(member_text).collect();
        let (names, generic) = self.qualified_box_names(klass, diagram);
        let header = header_rows(klass, self.kind_in_spot);
        let mut title_rows = header.clone();
        title_rows.extend(names.clone());
        let record = self.box_record(&klass.name, &names, &member_rows, title_rows);
        let mut spec = record;
        spec.member_rows_count = member_rows.len();
        spec.member_kinds = klass.body.iter().map(|member| member.kind).collect();
        spec.member_visibilities = klass.body.iter().map(|member| member.visibility).collect();
        spec.icons_enabled = member_icon::enabled(&diagram.directives);
        spec.empty_gaps = empty_gaps(diagram);
        spec.member_modifiers = klass
            .body
            .iter()
            .map(|member| rule_modifiers(member))
            .collect();
        spec.package = klass.package.clone();
        spec.chain = diagram.package_chain(&klass.package);
        spec.name_rows = names.len();
        spec.floating_generic = generic.clone();
        let complete = self.complete_box_specification(spec, klass, diagram, &names, &member_rows, generic);
        complete
    }

    fn qualified_box_names(
        &self,
        klass: &super::model::Klass,
        diagram: &Diagram,
    ) -> (Vec<String>, Option<String>) {
        let qualified = QualifiedAssociationLayout::for_diagram(diagram).is_some();
        let names = name_rows(klass, qualified);
        let generic = if qualified { klass.generics.clone() } else { None };
        let names = match &generic {
            Some(_) => names[..names.len() - 1].to_vec(),
            None => names,
        };
        (names, generic)
    }

    fn box_record(&self, name: &str, names: &[String], member_rows: &[String], title_rows: Vec<String>) -> Spec {
        let mut rows: Vec<&str> = title_rows.iter().map(String::as_str).collect();
        rows.extend(member_rows.iter().map(String::as_str));
        let widest = rows
            .iter()
            .map(|row| self.measured_width(row))
            .fold(0.0_f64, f64::max);
        let mut height = BOX_PADDING * 2.0 + rows.len() as f64 * ROW_HEIGHT;
        if !member_rows.is_empty() {
            height += 6.0;
        }
        Spec {
            id: name.to_string(),
            note: None,
            width: widest + BOX_PADDING * 2.0,
            height,
            title_rows,
            member_rows: member_rows.to_vec(),
            member_rows_count: member_rows.len(),
            name_rows: 0,
            floating_generic: None,
            compact_members: false,
            compact_title_rows: false,
            stereotype_head: None,
            member_modifiers: vec![],
            package: None,
            member_kinds: vec![],
            member_visibilities: vec![],
            icons_enabled: false,
            empty_gaps: true,
            chain: vec![],
            compact_bottom: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_box_specification(
        &mut self,
        mut record: Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
        generic: Option<String>,
    ) -> Spec {
        self.proportional_member_box(&mut record, klass, diagram, names, member_rows);
        self.compartmented_empty_box(&mut record, member_rows, diagram);
        self.compact_empty_box(&mut record, member_rows, diagram);
        self.smetana_hide_circle_box(&mut record, klass, diagram, names, member_rows);
        self.adjust_plain_empty_width(&mut record, klass, diagram, names);
        self.package_relation_box(&mut record, klass, diagram, names);
        self.multiline_title_box(&mut record, klass, member_rows);
        self.isolated_box_sizes(&mut record, klass, diagram, names, member_rows, generic);
        record
    }

    fn proportional_member_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        rows: &[String],
    ) {
        if rows.is_empty() || record.title_rows.len() > 1 {
            return;
        }
        let mut widths: Vec<f64> = self.standard_title_widths(names);
        widths.extend(self.member_row_widths(klass, rows, diagram));
        record.width = widths.iter().cloned().fold(0.0_f64, f64::max);
        record.height = self.pitched_member_height(klass, diagram, rows);
        record.compact_members = true;
    }

    fn member_row_widths(&self, klass: &super::model::Klass, rows: &[String], diagram: &Diagram) -> Vec<f64> {
        let insets = member_icon::insets(
            &klass.body.iter().map(|m| m.kind).collect::<Vec<_>>(),
            &klass.body.iter().map(|m| m.visibility).collect::<Vec<_>>(),
            member_icon::enabled(&diagram.directives),
        );
        let mut out = vec![];
        for index in 0..klass.body.len() {
            let Some(inset) = insets[index] else { continue };
            let member = &klass.body[index];
            let text = member_icon::visible_text(
                &rows[index],
                member.visibility,
                member_icon::enabled(&diagram.directives),
            );
            out.push(inset + self.standard_text_width(&text, FONT_SIZE) + member_icon::TEXT_PLAIN);
        }
        out
    }

    fn compartmented_empty_box(&self, record: &mut Spec, member_rows: &[String], diagram: &Diagram) {
        if !member_rows.is_empty() || !empty_gaps(diagram) {
            return;
        }
        if record.title_rows.len() != 1 {
            return;
        }
        record.height += EMPTY_COMPARTMENTS_EXTRA;
    }

    fn compact_empty_box(&self, record: &mut Spec, member_rows: &[String], diagram: &Diagram) {
        if !member_rows.is_empty() || !compact_empty_members(diagram) {
            return;
        }
        record.height -= EMPTY_MEMBER_COMPARTMENT;
        record.width += EMPTY_CLASS_WIDTH_EXTRA;
    }

    fn smetana_hide_circle_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
    ) {
        if rank_chain::engine(diagram) != Engine::Smetana
            || !plain_class(klass)
            || klass.package.is_some()
            || !diagram.directives.iter().any(|line| hide_circle_exact(line))
        {
            return;
        }
        let widths_vec = names.to_vec();
        let mut widths = self.standard_row_widths(&widths_vec, FONT_SIZE, HIDE_CIRCLE_TITLE_ALLOWANCE);
        let member_widths: Vec<f64> = member_rows
            .iter()
            .map(|row| self.standard_text_width(row, FONT_SIZE) + HIDE_CIRCLE_MEMBER_ALLOWANCE)
            .collect();
        widths.extend(member_widths);
        record.width = widths.iter().cloned().fold(0.0_f64, f64::max);
        record.compact_members = true;
        record.height =
            HIDE_CIRCLE_BASE_HEIGHT + member_rows.len() as f64 * MEMBER_NOTE_ROW_HEIGHT;
    }

    fn adjust_plain_empty_width(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
    ) {
        if plain_empty_class(klass, diagram, names) {
            record.width = self.plain_empty_class_width(&names[0]);
        } else if heterogeneous_smetana_empty_class(klass, diagram, names) {
            record.width = self
                .standard_title_widths(names)
                .into_iter()
                .fold(0.0_f64, f64::max);
            record.height = MEMBER_NOTE_CLASS_HEIGHT;
        }
    }

    fn plain_empty_class_width(&self, name: &str) -> f64 {
        let title_width = measures::face_widest_line(name, FONT_SIZE, false);
        let rounded = (title_width * 2.0).round() / 2.0 + 32.0;
        rounded.max(EMPTY_CLASS_MIN_WIDTH)
    }

    fn package_relation_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
    ) {
        if !super::frames::relation_layout(diagram) {
            return;
        }
        if !plain_class(klass) || !klass.body.is_empty() || names.len() != 1 {
            return;
        }
        record.width = self
            .standard_title_widths(names)
            .into_iter()
            .fold(0.0_f64, f64::max);
        record.height = MEMBER_NOTE_CLASS_HEIGHT;
    }

    fn multiline_title_box(&mut self, record: &mut Spec, klass: &super::model::Klass, member_rows: &[String]) {
        let header_empty = header_rows(klass, self.kind_in_spot).is_empty();
        if !member_rows.is_empty() || record.title_rows.len() <= 1 || !header_empty {
            return;
        }
        let rows = record.title_rows.len() as f64;
        record.height = MULTILINE_TITLE_VERTICAL_ALLOWANCE + rows * MULTILINE_TITLE_ROW_HEIGHT;
        let width = record
            .title_rows
            .iter()
            .map(|row| self.measure(row, FONT_SIZE))
            .fold(0.0_f64, f64::max);
        record.width = width + MULTILINE_TITLE_SIDE_ALLOWANCE;
        record.compact_title_rows = true;
    }

    fn isolated_box_sizes(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
        generic: Option<String>,
    ) {
        self.isolated_standard_box(record, klass, diagram, names, member_rows);
        self.isolated_class_kind_box(record, klass, diagram, names, member_rows);
        self.hide_members_grid_box(record, diagram, names, member_rows);
        self.qualified_association_box(record, klass, diagram, names, member_rows, generic);
    }

    fn isolated_standard_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
    ) {
        if !self.isolated_standard_box_p(klass, diagram, member_rows) {
            return;
        }
        let mut widths = self.standard_title_widths(names);
        widths.extend(self.standard_header_widths(klass));
        widths.extend(self.standard_member_widths(member_rows));
        record.width = widths.into_iter().fold(0.0_f64, f64::max);
        record.height = self.isolated_standard_height(klass, diagram, member_rows);
        record.compact_members = header_rows(klass, self.kind_in_spot).is_empty();
        // stereotype_headed
        if stereotype_head::applies(klass, self.kind_in_spot) {
            let row = format!("\u{AB}{}\u{BB}", klass.stereotypes[0]);
            let sizes = [
                self.standard_text_width(&row, FONT_SIZE * 0.85),
                self.standard_text_width(&names[0], FONT_SIZE),
            ];
            let head_width = stereotype_head::width(sizes[0], sizes[1]);
            let member_widths = self.standard_member_widths(member_rows);
            record.width = head_width
                .max(member_widths.into_iter().fold(0.0_f64, f64::max));
            record.compact_members = true;
            record.stereotype_head = Some((row, sizes));
        }
    }

    fn isolated_standard_box_p(
        &self,
        klass: &super::model::Klass,
        diagram: &Diagram,
        member_rows: &[String],
    ) -> bool {
        lone_standard_class(klass, diagram)
            && (!member_rows.is_empty() || !klass.stereotypes.is_empty())
    }

    fn isolated_standard_height(
        &self,
        klass: &super::model::Klass,
        diagram: &Diagram,
        member_rows: &[String],
    ) -> f64 {
        let headers = header_rows(klass, self.kind_in_spot);
        if !headers.is_empty() {
            return MEMBER_NOTE_CLASS_HEIGHT
                + member_rows.len() as f64 * MEMBER_NOTE_ROW_HEIGHT
                + headers.len() as f64 * STANDARD_STEREOTYPE_ROW_HEIGHT;
        }
        self.pitched_member_height(klass, diagram, member_rows)
    }

    fn pitched_member_height(
        &self,
        klass: &super::model::Klass,
        diagram: &Diagram,
        member_rows: &[String],
    ) -> f64 {
        let kinds: Vec<MemberKind> = klass.body.iter().map(|m| m.kind).collect();
        let titles: Vec<&str> = member_rows.iter().map(String::as_str).collect();
        let (_, extent) = member_pitch::layout(&kinds, &titles, empty_gaps(diagram));
        PROPORTIONAL_MEMBER_BASE_HEIGHT + extent
    }

    fn standard_title_widths(&self, rows: &[String]) -> Vec<f64> {
        self.standard_row_widths(rows, FONT_SIZE, STANDARD_TITLE_SIDE_ALLOWANCE)
    }

    fn standard_header_widths(&self, klass: &super::model::Klass) -> Vec<f64> {
        let rows: Vec<String> = header_rows(klass, self.kind_in_spot)
            .into_iter()
            .map(|row| replace_stereo(&row))
            .collect();
        self.standard_row_widths(&rows, FONT_SIZE * (12.0 / 14.0), STANDARD_TITLE_SIDE_ALLOWANCE)
    }

    fn standard_member_widths(&self, rows: &[String]) -> Vec<f64> {
        self.standard_row_widths(rows, FONT_SIZE, STANDARD_MEMBER_SIDE_ALLOWANCE)
    }

    fn standard_row_widths(&self, rows: &[String], size: f64, allowance: f64) -> Vec<f64> {
        rows.iter().map(|row| self.standard_text_width(row, size) + allowance).collect()
    }

    fn isolated_class_kind_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
    ) {
        if !isolated_class_kind_family(diagram) {
            return;
        }
        let mut widths = self.standard_title_widths(names);
        let member_widths: Vec<f64> = member_rows
            .iter()
            .map(|row| {
                let visible = row.strip_prefix(|c: char| matches!(c, '+' | '-' | '#' | '~')).unwrap_or(row);
                self.standard_text_width(visible, FONT_SIZE) + ISOLATED_KIND_MEMBER_ALLOWANCE
            })
            .collect();
        widths.extend(member_widths);
        record.width = widths.into_iter().fold(0.0_f64, f64::max);
        record.title_rows = names.to_vec();
        record.compact_members = true;
        record.height =
            MEMBER_NOTE_CLASS_HEIGHT + member_rows.len() as f64 * MEMBER_NOTE_ROW_HEIGHT;
    }

    fn hide_members_grid_box(
        &mut self,
        record: &mut Spec,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
    ) {
        if !self.hide_members_grid_family(diagram) {
            return;
        }
        let mut widths = self.standard_title_widths(names);
        let member_widths: Vec<f64> = member_rows
            .iter()
            .map(|row| {
                let visible = row.strip_prefix(|c: char| matches!(c, '+' | '-' | '#' | '~')).unwrap_or(row);
                self.standard_text_width(visible, FONT_SIZE) + ISOLATED_KIND_MEMBER_ALLOWANCE
            })
            .collect();
        widths.extend(member_widths);
        record.width = widths.into_iter().fold(0.0_f64, f64::max);
        record.compact_members = true;
        record.height = if member_rows.is_empty() {
            HIDDEN_MEMBER_EMPTY_HEIGHT
        } else {
            RESHOWN_MEMBER_BASE_HEIGHT + member_rows.len() as f64 * MEMBER_NOTE_ROW_HEIGHT
        };
    }

    fn qualified_association_box(
        &mut self,
        record: &mut Spec,
        klass: &super::model::Klass,
        diagram: &Diagram,
        names: &[String],
        member_rows: &[String],
        generic: Option<String>,
    ) {
        let Some(_qualified) = QualifiedAssociationLayout::for_diagram(diagram) else {
            return;
        };
        record.title_rows = names.to_vec();
        let mut widths = self.standard_title_widths(names);
        if let Some(generic) = &generic {
            let title_width = widths.iter().cloned().fold(0.0_f64, f64::max);
            widths.push(
                title_width
                    + self.standard_text_width(generic, QUALIFIED_GENERIC_FONT_SIZE)
                    + QUALIFIED_GENERIC_GAP,
            );
        }
        let mut incident = self.incident_qualifiers(klass, diagram);
        incident.sort_by_key(|text| std::cmp::Reverse(text.len()));
        if let Some(text) = incident.first() {
            widths.push(self.standard_text_width(text, FONT_SIZE) + self.qualifier_allowance(diagram));
        }
        widths.extend(member_rows.iter().map(|row| self.measured_width(row) + BOX_PADDING * 2.0));
        record.width = widths.into_iter().fold(0.0_f64, f64::max);
        record.height = self.qualified_height_with_members(klass, diagram, member_rows);
        record.compact_members = !member_rows.is_empty();
    }

    fn qualified_height_with_members(
        &self,
        klass: &super::model::Klass,
        diagram: &Diagram,
        member_rows: &[String],
    ) -> f64 {
        let height = if qualified_compact(diagram) {
            HIDDEN_MEMBER_EMPTY_HEIGHT
        } else if klass.generics.is_some() {
            QUALIFIED_GENERIC_HEIGHT
        } else {
            QUALIFIED_BOX_HEIGHT
        };
        let rows = member_rows.len();
        if rows == 0 {
            return height;
        }
        let base = if qualified_compact(diagram) { COMPACT_RULE_BASE_HEIGHT } else { height };
        let gaps = member_group_boundaries(&klass.body.iter().map(|m| m.kind).collect::<Vec<_>>()).len();
        base + rows as f64 * MEMBER_NOTE_ROW_HEIGHT + gaps as f64 * COMPACT_GROUP_GAP
    }

    fn qualifier_allowance(&self, diagram: &Diagram) -> f64 {
        if qualified_compact(diagram) {
            COMPACT_QUALIFIER_ALLOWANCE
        } else {
            QUALIFIED_QUALIFIER_ALLOWANCE
        }
    }

    fn incident_qualifiers(&self, klass: &super::model::Klass, diagram: &Diagram) -> Vec<String> {
        diagram
            .relations
            .iter()
            .filter(|relation| relation.direction.is_none())
            .filter(|relation| [relation.left.as_str(), relation.right.as_str()].contains(&klass.name.as_str()))
            .filter_map(|relation| self.qualified_text_at(relation, &klass.name))
            .collect()
    }

    fn qualified_text_at(&self, relation: &Relation, name: &str) -> Option<String> {
        let content = if relation.left == name {
            relation.left_multiplicity.as_ref()
        } else {
            relation.right_multiplicity.as_ref()
        }?;
        if !content.starts_with('[') {
            return None;
        }
        let inner = content.trim_start_matches('[').trim_end_matches(']');
        inner
            .split("\\n")
            .max_by_key(|line| line.len())
            .map(str::to_string)
    }

    // ---- the grid ----

    fn position_boxes(
        &self,
        rows: Vec<Vec<Option<Spec>>>,
        box_width: f64,
        stable_centers: bool,
        intrinsic_widths: bool,
        diagram: &Diagram,
    ) -> Vec<Box> {
        let tops = self.row_tops(&rows, diagram);
        let left = self.left_margin(&rows, diagram);
        let footprint = if stable_centers {
            ANNOTATED_RELATION_SLOT_WIDTH.max(box_width)
        } else {
            box_width
        };
        let left_align = intrinsic_widths && super::frames::relation_layout(diagram);
        let mut boxes = vec![];
        for (row_index, row) in rows.iter().enumerate() {
            for (column, item) in row.iter().enumerate() {
                let Some(item) = item else { continue };
                let slot = left + column as f64 * (footprint + COLUMN_GAP);
                let width = if intrinsic_widths { item.width } else { box_width };
                let x = if left_align {
                    slot
                } else {
                    slot + (footprint - width) / 2.0
                };
                boxes.push(self.build_box(item, x, tops[row_index], width));
            }
        }
        boxes
    }

    fn row_tops(&self, rows: &[Vec<Option<Spec>>], diagram: &Diagram) -> Vec<f64> {
        let chains: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .flatten()
                    .next()
                    .map(|item| item.chain.clone())
                    .unwrap_or_default()
            })
            .collect();
        let mut tops = vec![self.top_margin(diagram) + self.frame_space(&[], &chains[0], diagram)];
        for index in 0..rows.len() {
            let below = chains.get(index + 1).cloned().unwrap_or_default();
            let height = row_height(&rows[index]);
            let step = height
                + self.row_gap(diagram, &chains[index], &below)
                + self.frame_space(&chains[index], &below, diagram);
            let next = *tops.last().unwrap() + step;
            tops.push(next);
        }
        tops
    }

    fn frame_space(&self, above: &[String], below: &[String], diagram: &Diagram) -> f64 {
        let shared = above
            .iter()
            .zip(below.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let closing = above.len() - shared;
        let opening = below.len() - shared;
        if super::frames::relation_layout(diagram) {
            return super::frames::stack_close(closing) + super::frames::stack_open(opening);
        }
        closing as f64 * super::frames::CLOSE + opening as f64 * super::frames::OPEN
    }

    fn top_margin(&self, diagram: &Diagram) -> f64 {
        if super::frames::relation_layout(diagram) {
            PACKAGE_CANVAS_MARGIN
        } else {
            MARGIN
        }
    }

    fn row_gap(&self, diagram: &Diagram, above: &[String], below: &[String]) -> f64 {
        let crossing = above.is_empty() != below.is_empty();
        if super::frames::relation_layout(diagram) && crossing {
            PACKAGE_OUTER_GAP
        } else {
            ROW_GAP
        }
    }

    fn left_margin(&self, rows: &[Vec<Option<Spec>>], diagram: &Diagram) -> f64 {
        let mut depth = 0usize;
        for row in rows {
            for item in row.iter().flatten() {
                depth = depth.max(item.chain.len());
            }
        }
        if super::frames::relation_layout(diagram) {
            return PACKAGE_CANVAS_MARGIN + super::frames::stack_side(depth);
        }
        if depth == 0 {
            return MARGIN;
        }
        MARGIN.max(depth as f64 * super::frames::SIDE + 8.0)
    }

    fn align_package_relation_targets(
        &mut self,
        diagram: &Diagram,
        boxes: Vec<Box>,
        specifications: &[Spec],
    ) -> Vec<Box> {
        let mut by_name: HashMap<String, Box> =
            boxes.into_iter().map(|box_| (box_.id.clone(), box_)).collect();
        for relation in &diagram.relations {
            let Some(replacement) =
                self.aligned_package_target(diagram, &by_name, specifications, relation)
            else {
                continue;
            };
            by_name.insert(replacement.id.clone(), replacement);
        }
        specifications
            .iter()
            .filter(|item| item.note.is_none())
            .map(|item| by_name.remove(&item.id).expect("box present"))
            .collect()
    }

    fn aligned_package_target(
        &self,
        diagram: &Diagram,
        by_name: &HashMap<String, Box>,
        specifications: &[Spec],
        relation: &Relation,
    ) -> Option<Box> {
        if !diagram.packages.iter().any(|item| item.id == relation.left) {
            return None;
        }
        let target = by_name.get(&relation.right)?;
        let mut descendants = vec![];
        for klass in &diagram.classes {
            let chain = diagram.package_chain(&klass.package);
            if chain.contains(&relation.left) {
                if let Some(box_) = by_name.get(&klass.name) {
                    descendants.push(box_);
                }
            }
        }
        if descendants.is_empty() {
            return None;
        }
        let edge = descendants
            .iter()
            .map(|box_| box_.x + box_.width)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut depth = 0usize;
        for klass in &diagram.classes {
            let chain = diagram.package_chain(&klass.package);
            if let Some(index) = chain.iter().position(|id| *id == relation.left) {
                depth = depth.max(chain.len() - index);
            }
        }
        let left = edge + super::frames::stack_side(depth) - target.width / 2.0;
        let spec = specifications.iter().find(|item| item.id == target.id)?;
        Some(self.build_box(spec, left, target.y, target.width))
    }

    fn build_box(&self, item: &Spec, left: f64, top: f64, width: f64) -> Box {
        let (texts, separators) = self.box_contents(item, left, top, width);
        Box {
            id: item.id.clone(),
            x: left,
            y: top,
            width,
            height: item.height,
            texts,
            separators,
            kind: if item.note.is_some() { "note" } else { "class" },
            fill: item.note.as_ref().and_then(|note| note.color.clone()),
            spot: self.stereotype_spot(item, left, top),
        }
    }

    fn stereotype_spot(&self, item: &Spec, left: f64, top: f64) -> Option<Spot> {
        let [across, down] = stereotype_head::circle_centre();
        item.stereotype_head
            .as_ref()
            .map(|_| Spot { kind: "class".to_string(), cx: left + across, cy: top + down })
    }

    fn box_contents(&self, item: &Spec, horizontal: f64, vertical: f64, width: f64) -> (Vec<Text>, Vec<Segment>) {
        if item.note.is_some() {
            return (self.note_texts(item, horizontal, vertical), vec![]);
        }
        let (mut texts, cursor) = self.title_texts(item, horizontal, vertical, width);
        let (members, separators) = self.member_contents(item, horizontal, cursor, width, vertical + item.height);
        texts.extend(members);
        (texts, separators)
    }

    fn note_texts(&self, item: &Spec, horizontal: f64, vertical: f64) -> Vec<Text> {
        let Some(note) = &item.note else { return vec![] };
        note.lines
            .iter()
            .enumerate()
            .map(|(index, content)| {
                let baseline = vertical + BOX_PADDING + FONT_SIZE + index as f64 * ROW_HEIGHT;
                Text::new(content, horizontal + BOX_PADDING, baseline, "note", "start")
            })
            .collect()
    }

    fn title_texts(&self, item: &Spec, horizontal: f64, vertical: f64, width: f64) -> (Vec<Text>, f64) {
        if let Some((row, sizes)) = &item.stereotype_head {
            let centre = horizontal + stereotype_head::centre(sizes[0], sizes[1]);
            let baselines = stereotype_head::baselines();
            return (
                vec![
                    Text::new(row, centre, vertical + baselines[0], "kind", "middle"),
                    Text::new(
                        &item.title_rows.last().cloned().unwrap_or_default(),
                        centre,
                        vertical + baselines[1],
                        "class_name",
                        "middle",
                    ),
                ],
                vertical + baselines[1],
            );
        }
        let mut cursor = self.title_text_cursor(item, vertical);
        let mut texts = vec![];
        for (index, content) in item.title_rows.iter().enumerate() {
            let role = self.title_role(item, index);
            texts.push(Text::new(content, horizontal + width / 2.0, cursor, role, "middle"));
            cursor += if item.compact_title_rows {
                MULTILINE_TITLE_ROW_HEIGHT
            } else {
                ROW_HEIGHT
            };
        }
        if let Some(generic) = &item.floating_generic {
            let scaled_width = self.measured_width(generic) * 0.85;
            texts.push(Text::new(
                generic,
                horizontal + width - scaled_width + 4.0,
                vertical + FONT_SIZE * 0.9,
                "kind",
                "start",
            ));
        }
        (texts, cursor)
    }

    fn title_text_cursor(&self, item: &Spec, vertical: f64) -> f64 {
        if item.compact_title_rows {
            return vertical + MULTILINE_TITLE_BASELINE;
        }
        let baseline = vertical + BOX_PADDING + FONT_SIZE;
        if self.compact_single_title(item) {
            let (offsets, _) =
                member_pitch::layout(&item.member_kinds, &item.member_rows_titles(), item.empty_gaps);
            let empty_gap_tail = self.empty_gap(item, "tail");
            let last = vertical + item.height - COMPACT_BOTTOM_PAD - empty_gap_tail;
            let final_offset = *offsets.last().unwrap_or(&0.0);
            let baselines: Vec<f64> =
                offsets.iter().map(|offset| last - final_offset + *offset).collect();
            let head = baselines.first().copied().unwrap_or(0.0)
                - COMPACT_HEAD_GAP
                - offsets.first().copied().unwrap_or(0.0);
            return baseline.min(head - 10.709);
        }
        baseline
    }

    fn compact_single_title(&self, item: &Spec) -> bool {
        item.compact_members && item.title_rows.len() == 1 && !item.member_rows.is_empty()
    }

    /// Ruby `empty_gap`: the gap an empty field or method group keeps.
    fn empty_gap(&self, item: &Spec, side: &str) -> f64 {
        if !item.empty_gaps {
            return 0.0;
        }
        if side == "tail" {
            member_pitch::tail(&item.member_kinds)
        } else {
            member_pitch::lead(&item.member_kinds)
        }
    }

    fn title_role(&self, item: &Spec, index: usize) -> &'static str {
        let header = item.title_rows.len() as i64 - item.name_rows as i64;
        if (index as i64) < header {
            "kind"
        } else {
            "class_name"
        }
    }

    fn member_contents(
        &self,
        item: &Spec,
        horizontal: f64,
        cursor: f64,
        width: f64,
        compact_bottom: f64,
    ) -> (Vec<Text>, Vec<Segment>) {
        if item.member_rows.is_empty() {
            return (vec![], vec![]);
        }
        let rows = self.member_row_geometry(item, cursor, compact_bottom);
        let texts = self.member_texts(item, horizontal, width, &rows);
        let lines = self.member_lines(item, horizontal, width, &rows);
        (texts, lines)
    }

    fn member_row_geometry(&self, item: &Spec, cursor: f64, compact_bottom: f64) -> RowGeometry {
        if item.compact_members {
            let (offsets, _) =
                member_pitch::layout(&item.member_kinds, &item.member_rows_titles(), item.empty_gaps);
            let empty_gap_tail = self.empty_gap(item, "tail");
            let last = compact_bottom - COMPACT_BOTTOM_PAD - empty_gap_tail;
            let final_offset = *offsets.last().unwrap_or(&0.0);
            let baselines: Vec<f64> =
                offsets.iter().map(|offset| last - final_offset + *offset).collect();
            let head = baselines.first().copied().unwrap_or(0.0)
                - COMPACT_HEAD_GAP
                - offsets.first().copied().unwrap_or(0.0);
            return RowGeometry { head, baselines };
        }
        let head = cursor - ROW_HEIGHT / 2.0;
        let baselines: Vec<f64> = (0..item.member_rows.len())
            .map(|index| cursor + 6.0 + index as f64 * ROW_HEIGHT)
            .collect();
        RowGeometry { head, baselines }
    }

    fn member_lines(&self, item: &Spec, horizontal: f64, width: f64, rows: &RowGeometry) -> Vec<Segment> {
        let mut out = vec![Segment::new(horizontal, rows.head, horizontal + width, rows.head)];
        // group rules
        if item.compact_members {
            for at in self.group_rule_heights(item, &rows.baselines) {
                out.push(Segment::new(horizontal, at, horizontal + width, at));
            }
        }
        // underlines
        for (index, content) in item.member_rows.iter().enumerate() {
            let modifiers = item.member_modifiers.get(index).cloned().unwrap_or_default();
            if !modifiers.contains(&"static") {
                continue;
            }
            let vertical = rows.baselines[index] + 2.0;
            let insets = member_icon::insets(&item.member_kinds, &item.member_visibilities, item.icons_enabled);
            let left = horizontal + insets[index].unwrap_or(member_icon::TEXT_PLAIN);
            let text = self.visible_member(item, content, index);
            let w = self.standard_text_width(&text, FONT_SIZE);
            out.push(Segment::new(left, vertical, left + w, vertical));
        }
        // separator rules
        for (index, content) in item.member_rows.iter().enumerate() {
            let modifiers = item.member_modifiers.get(index).cloned().unwrap_or_default();
            if !modifiers.contains(&"rule") {
                continue;
            }
            let character = modifiers.get(1).copied().unwrap_or("-");
            let vertical = rows.baselines[index] - 4.0;
            let segments = self.rule_segments(content, horizontal, vertical, width);
            out.extend(member_rule::apply(character, &segments));
        }
        out
    }

    fn group_rule_heights(&self, item: &Spec, baselines: &[f64]) -> Vec<f64> {
        let mut out: Vec<f64> = member_group_boundaries(&item.member_kinds)
            .into_iter()
            .map(|index| (baselines[index - 1] + baselines[index]) / 2.0 - 5.2911)
            .collect();
        let lead = if item.empty_gaps { member_pitch::lead(&item.member_kinds) } else { 0.0 };
        let tail = if item.empty_gaps { member_pitch::tail(&item.member_kinds) } else { 0.0 };
        if lead > 0.0 {
            out.push(baselines[0] - COMPACT_HEAD_GAP);
        }
        if tail > 0.0 {
            out.push(*baselines.last().unwrap() + COMPACT_BOTTOM_PAD);
        }
        out
    }

    fn rule_segments(&self, content: &str, horizontal: f64, vertical: f64, width: f64) -> Vec<Segment> {
        let right = horizontal + width;
        if content.is_empty() {
            return vec![Segment::new(horizontal, vertical, right, vertical)];
        }
        let gap = self.standard_text_width(content, FONT_SIZE) / 2.0;
        let middle = horizontal + width / 2.0;
        vec![
            Segment::new(horizontal, vertical, middle - gap, vertical),
            Segment::new(middle + gap, vertical, right, vertical),
        ]
    }

    fn member_texts(&self, item: &Spec, horizontal: f64, width: f64, rows: &RowGeometry) -> Vec<Text> {
        let insets = member_icon::insets(&item.member_kinds, &item.member_visibilities, item.icons_enabled);
        let mut texts = vec![];
        for (index, content) in item.member_rows.iter().enumerate() {
            if content.is_empty() {
                continue;
            }
            let visible = self.visible_member(item, content, index);
            let modifiers = item.member_modifiers.get(index).cloned().unwrap_or_default();
            let text = if modifiers.contains(&"rule") {
                Text::new(&visible, horizontal + width / 2.0, rows.baselines[index], "member", "middle")
            } else if modifiers.contains(&"abstract") {
                Text::new(&visible, horizontal + insets[index].unwrap_or(0.0), rows.baselines[index], "member_abstract", "start")
            } else {
                Text::new(&visible, horizontal + insets[index].unwrap_or(0.0), rows.baselines[index], "member", "start")
            };
            let mut text = text;
            text.icon = member_icon::at(
                item.member_visibilities[index],
                item.member_kinds[index],
                horizontal,
                rows.baselines[index],
            )
            .filter(|_| item.icons_enabled);
            let _ = width;
            texts.push(text);
        }
        texts
    }

    fn visible_member(&self, item: &Spec, content: &str, index: usize) -> String {
        member_icon::visible_text(content, item.member_visibilities[index], item.icons_enabled)
    }

    // ---- relations ----

    fn scene_relations(
        &self,
        diagram: &Diagram,
        boxes: &[Box],
        nodes: &HashMap<usize, [f64; 4]>,
    ) -> Vec<SceneRelation> {
        self.build_relations(diagram, boxes, nodes)
    }

    fn build_relations(
        &self,
        diagram: &Diagram,
        boxes: &[Box],
        nodes: &HashMap<usize, [f64; 4]>,
    ) -> Vec<SceneRelation> {
        let mut by_name: HashMap<String, &Box> = HashMap::new();
        for box_ in boxes {
            let name = box_.id.strip_prefix("package-").unwrap_or(&box_.id);
            by_name.insert(name.to_string(), box_);
        }
        let owners = self.junction_owners(diagram);
        let engine = rank_chain::engine(diagram);
        let mut scenes = vec![];
        for (index, relation) in diagram.relations.iter().enumerate() {
            if relation.layout_only {
                continue;
            }
            let end_texts = EndTexts::new(relation, engine);
            let junctions = owners.get(&index).cloned().unwrap_or_default();
            if junctions.is_empty() {
                let ends = self.relation_ends(relation, &by_name);
                let scene = self.relation_scene(
                    relation,
                    &format!("relation-{index}"),
                    &ends,
                    &end_texts,
                    engine,
                    &[true, false],
                    &None,
                );
                scenes.push(scene);
                continue;
            }
            let ends = self.relation_ends(relation, &by_name);
            let junction = self.junction_box(nodes.get(&index).copied(), &ends, engine);
            for (number, owner) in &junctions {
                let swap = matches!(relation.direction, Some(Direction::Up) | Some(Direction::Left));
                let mut legs = vec![];
                let sides = [true, false];
                let tails = [&by_name[&relation.left], &junction];
                let heads = [&junction, &by_name[&relation.right]];
                for (side_index, side) in sides.iter().enumerate() {
                    let markers = self.junction_markers(&relation.markers, swap, *side);
                    let ends = self.relation_ends_boxes(tails[side_index], heads[side_index]);
                    let mut scene = self.relation_scene(
                        relation,
                        &format!("relation-{index}-{}", if *side { "from" } else { "to" }),
                        &ends,
                        &end_texts,
                        engine,
                        &[*side],
                        &Some(markers),
                    );
                    if side_index == 0 {
                        scene.markers.push(self.junction_dot(&junction));
                    }
                    legs.push(scene);
                }
                scenes.extend(legs);
                let edge = self.relation_ends_boxes(&junction, &by_name[owner]);
                scenes.push(SceneRelation {
                    id: format!("junction-{number}"),
                    path: self.relation_path(&edge),
                    dashed: true,
                    dotted: false,
                    stroke: None,
                    stroke_width: None,
                    markers: vec![],
                    texts: vec![],
                    qualifiers: vec![],
                });
            }
        }
        self.separate_relation_labels(&mut scenes, boxes);
        scenes
    }

    fn separate_relation_labels(&self, scenes: &mut [SceneRelation], boxes: &[Box]) {
        let size = FONT_SIZE * SMALL_TEXT_SCALE;
        let width_of = |content: &str| self.end_text_width(content);
        let drawn_width_of = |content: &str| measures::face_width(content, FONT_SIZE * SMALL_TEXT_SCALE, false);
        label_separator::Separator {
            size,
            boxes,
            width_of: &width_of,
            drawn_width_of: &drawn_width_of,
        }
        .call(scenes);
    }

    fn junction_owners(&self, diagram: &Diagram) -> HashMap<usize, Vec<(usize, String)>> {
        let mut owners: HashMap<usize, Vec<(usize, String)>> = HashMap::new();
        for (number, junction) in diagram.junctions.iter().enumerate() {
            let mut pair = [junction.from.clone(), junction.to.clone()];
            pair.sort();
            let index = diagram.relations.iter().position(|relation| {
                let mut pair = [relation.left.clone(), relation.right.clone()];
                pair.sort();
                pair == pair_of(&junction.from, &junction.to)
            });
            if let Some(index) = index {
                owners.entry(index).or_default().push((number, junction.owner.clone()));
            }
        }
        owners
    }

        fn junction_markers(
        &self,
        markers: &super::model::Markers,
        swap: bool,
        side: bool,
    ) -> super::model::Markers {
        let opposite = if side { markers.right } else { markers.left };
        let own = if swap { opposite } else { markers.get(side) };
        let mut out = super::model::Markers::default();
        if side {
            out.left = own;
        } else {
            out.right = own;
        }
        out
    }

    fn junction_dot(&self, junction: &Box) -> Marker {
        Marker {
            shape: "dot",
            points: String::new(),
            filled: true,
            cx: junction.x + junction.width / 2.0,
            cy: junction.y + junction.height / 2.0,
            r: JUNCTION_DOT_RADIUS,
            cross: String::new(),
        }
    }

    /// The junction node, or a zero-size one at the middle of the relation.
    fn junction_box(
        &self,
        node: Option<[f64; 4]>,
        ends: &HashMap<&str, [f64; 2]>,
        engine: Engine,
    ) -> Box {
        let node = node.unwrap_or_else(|| {
            let left = ends[&"left"];
            let right = ends[&"right"];
            let mid = [
                (left[0] + right[0]) / 2.0,
                (left[1] + right[1]) / 2.0,
            ];
            [mid[0], mid[1], 0.0, 0.0]
        });
        let (x, y, width, height) = (node[0], node[1], node[2], node[3]);
        if engine != Engine::Dot {
            return Box {
                id: "junction".into(),
                x,
                y,
                width,
                height,
                texts: vec![],
                separators: vec![],
                kind: "junction",
                fill: None,
                spot: None,
            };
        }
        Box {
            id: "junction".into(),
            x: x + width / 2.0,
            y: y + height / 2.0,
            width: 0.0,
            height: 0.0,
            texts: vec![],
            separators: vec![],
            kind: "junction",
            fill: None,
            spot: None,
        }
    }

    fn relation_ends<'a>(
        &self,
        relation: &Relation,
        by_name: &HashMap<String, &'a Box>,
    ) -> HashMap<&'static str, [f64; 2]> {
        let left = by_name[&relation.left];
        let right = by_name[&relation.right];
        self.relation_ends_boxes(left, right)
    }

    fn relation_ends_boxes(&self, left: &Box, right: &Box) -> HashMap<&'static str, [f64; 2]> {
        if std::ptr::eq(left, right) {
            return self.self_relation_endpoints(left);
        }
        if let Some(aligned) = self.aligned_package_endpoints(left, right) {
            return aligned;
        }
        HashMap::from([
            ("left", self.boundary_point(left, right)),
            ("right", self.boundary_point(right, left)),
        ])
    }

    fn aligned_package_endpoints(
        &self,
        left: &Box,
        right: &Box,
    ) -> Option<HashMap<&'static str, [f64; 2]>> {
        if package_frame(left) {
            return self.vertical_package_endpoints(left, right, true);
        }
        if package_frame(right) {
            return self.vertical_package_endpoints(right, left, false);
        }
        None
    }

    fn vertical_package_endpoints(
        &self,
        frame: &Box,
        box_: &Box,
        frame_is_left: bool,
    ) -> Option<HashMap<&'static str, [f64; 2]>> {
        let centre = box_centre(box_);
        if !(frame.x..=frame.x + frame.width).contains(&centre[0]) {
            return None;
        }
        let bottom_of = |b: &Box| b.y + b.height;
        let points: [[f64; 2]; 2] = if box_.y >= bottom_of(frame) {
            [[centre[0], bottom_of(frame)], [centre[0], box_.y]]
        } else if frame.y >= bottom_of(box_) {
            [[centre[0], frame.y], [centre[0], bottom_of(box_)]]
        } else {
            return None;
        };
        let (frame_point, box_point) = (points[0], points[1]);
        if frame_is_left {
            Some(HashMap::from([("left", frame_point), ("right", box_point)]))
        } else {
            Some(HashMap::from([("left", box_point), ("right", frame_point)]))
        }
    }

    fn self_relation_endpoints(&self, box_: &Box) -> HashMap<&'static str, [f64; 2]> {
        let edge_point = |ratio: f64, offset: f64| -> [f64; 2] {
            [box_.x + box_.width + offset, box_.y + box_.height * ratio]
        };
        HashMap::from([
            ("left", edge_point(SELF_RELATION_START_RATIO, 0.0)),
            ("right", edge_point(SELF_RELATION_END_RATIO, 0.0)),
            ("control", edge_point(0.5, SELF_RELATION_CONTROL_OFFSET)),
        ])
    }

    fn boundary_point(&self, box_: &Box, target: &Box) -> [f64; 2] {
        let centre = box_centre(box_);
        let destination = box_centre(target);
        let delta = [destination[0] - centre[0], destination[1] - centre[1]];
        let horizontal = axis_scale(box_.width, delta[0]);
        let vertical = axis_scale(box_.height, delta[1]);
        let scale = match (horizontal, vertical) {
            (Some(h), Some(v)) => h.min(v),
            (Some(h), None) => h,
            (None, Some(v)) => v,
            (None, None) => return centre,
        };
        [
            centre[0] + delta[0] * scale,
            centre[1] + delta[1] * scale,
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn relation_scene(
        &self,
        relation: &Relation,
        id: &str,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        end_texts: &EndTexts,
        engine: Engine,
        sides: &[bool],
        markers_override: &Option<super::model::Markers>,
    ) -> SceneRelation {
        let (boxes, endpoints) = self.qualifier_boxes(relation, end_texts, endpoints, sides, engine);
        let texts = self.relation_texts(relation, &endpoints, end_texts, sides, boxes.len(), engine);
        let markers = match markers_override {
            Some(markers) => markers.clone(),
            None => relation.markers.clone(),
        };
        let drawn = self.relation_markers(relation, &markers, sides, &endpoints);
        SceneRelation {
            id: id.to_string(),
            path: self.relation_path_for(relation, &endpoints),
            dashed: relation.dashed,
            dotted: relation.dotted,
            stroke: svg_colour(relation.color.as_deref()),
            stroke_width: relation.width,
            markers: drawn,
            texts,
            qualifiers: boxes,
        }
    }

    fn qualifier_boxes(
        &self,
        relation: &Relation,
        end_texts: &EndTexts,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        sides: &[bool],
        self_engine: Engine,
    ) -> (Vec<Box>, HashMap<&'static str, [f64; 2]>) {
        let first = end_texts.sides[0];
        let second = end_texts.sides[1];
        let direction = unit_vector(
            endpoints[if first { "left" } else { "right" }],
            endpoints[if second { "left" } else { "right" }],
        );
        let entries = end_texts.entries(relation, self_engine);
        let mut placed: Vec<(bool, qualifier_box::Placed)> = vec![];
        if self.boxable_link(endpoints, relation) {
            for entry in &entries {
                if entry.kind != EntryKind::Qualifier || entry.pieces.len() != 1 {
                    continue;
                }
                if !sides.contains(&entry.side) {
                    continue;
                }
                let content = &entry.pieces[0];
                let outward = if entry.side == first {
                    direction
                } else {
                    [-direction[0], -direction[1]]
                };
                let width = self.measure(content, FONT_SIZE);
                let placement = qualifier_box::place(
                    content,
                    endpoints[if entry.side { "left" } else { "right" }],
                    outward,
                    width,
                    FONT_SIZE,
                );
                placed.push((entry.side, placement));
            }
        }
        let mut boxes = vec![];
        let mut endpoints = endpoints.clone();
        for (side, placement) in &placed {
            boxes.push(placement.box_of("qualifier"));
            let key = if *side { "left" } else { "right" };
            endpoints.insert(key, placement.clear(endpoints[key]));
        }
        (boxes, endpoints)
    }

    fn boxable_link(&self, endpoints: &HashMap<&'static str, [f64; 2]>, relation: &Relation) -> bool {
        let left = endpoints["left"];
        let right = endpoints["right"];
        !endpoints.contains_key("control")
            && relation.label.is_none()
            && (left[0] - right[0]).abs() < AXIS_TOLERANCE
    }

    fn relation_markers(
        &self,
        relation: &Relation,
        markers: &super::model::Markers,
        sides: &[bool],
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> Vec<Marker> {
        let half_width = self.qualified_marker_half_width(relation, endpoints);
        let mut drawn = self.marker_scenes(markers.slice(sides), endpoints, half_width);
        if let Some(scene) = self.label_arrow_scene(relation, endpoints) {
            drawn.push(scene);
        }
        self.scale_thick_markers(drawn, relation, endpoints)
    }

    fn qualified_marker_half_width(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> Option<f64> {
        let qualifiers = [relation.left_multiplicity.as_ref(), relation.right_multiplicity.as_ref()];
        let all_qualified = qualifiers
            .iter()
            .all(|text| text.map(|t| t.starts_with('[')).unwrap_or(false));
        if all_qualified {
            return Some(QUALIFIED_DOUBLE_END_HALF_WIDTH);
        }
        if self.qualified_layout_active
            && qualifiers.iter().all(|text| text.is_none())
            && relation.markers.has_value(super::model::ArrowKind::Implementation)
            && steep(endpoints["left"], endpoints["right"])
        {
            return Some(QUALIFIED_INHERITANCE_HALF_WIDTH);
        }
        None
    }

    fn marker_scenes(
        &self,
        markers: Vec<(bool, super::model::ArrowKind)>,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        half_width: Option<f64>,
    ) -> Vec<Marker> {
        markers
            .into_iter()
            .map(|(side, kind)| {
                let tip = endpoints[if side { "left" } else { "right" }];
                let opposite = endpoints[if side { "right" } else { "left" }];
                let other = endpoints.get("control").copied().unwrap_or(opposite);
                self.marker_scene(kind, tip, other, half_width)
            })
            .collect()
    }

    fn marker_scene(
        &self,
        kind: super::model::ArrowKind,
        tip: [f64; 2],
        other: [f64; 2],
        half_width: Option<f64>,
    ) -> Marker {
        if kind == super::model::ArrowKind::Nesting {
            let centre = shift(tip, unit_vector(tip, other), NESTING_RADIUS);
            let (x, y) = (centre[0], centre[1]);
            let reach = NESTING_RADIUS;
            return Marker {
                shape: "nesting",
                points: String::new(),
                filled: false,
                cx: x,
                cy: y,
                r: NESTING_RADIUS,
                cross: format!(
                    "M {} {} L {} {} M {} {} L {} {}",
                    crate::svg::ruby_float(x - reach), crate::svg::ruby_float(y),
                    crate::svg::ruby_float(x + reach), crate::svg::ruby_float(y),
                    crate::svg::ruby_float(x), crate::svg::ruby_float(y - reach),
                    crate::svg::ruby_float(x), crate::svg::ruby_float(y + reach),
                ),
            };
        }
        let direction = unit_vector(other, tip);
        let shape = marker_shape(kind);
        let mut points: Vec<[f64; 2]> = match shape {
            MarkerShape::Arrow => {
                let base = shift(tip, direction, -ARROW_LENGTH);
                let half = half_width.unwrap_or(ARROW_HALF_WIDTH);
                vec![
                    tip,
                    marker_side(base, direction, half),
                    shift(tip, direction, -ARROW_NOTCH),
                    marker_side(base, direction, -half),
                ]
            }
            MarkerShape::Diamond => {
                let middle = shift(tip, direction, -(DIAMOND_LENGTH / 2.0));
                let half = half_width.unwrap_or(DIAMOND_HALF_WIDTH);
                vec![
                    tip,
                    marker_side(middle, direction, half),
                    shift(tip, direction, -DIAMOND_LENGTH),
                    marker_side(middle, direction, -half),
                ]
            }
            MarkerShape::Triangle => {
                let base = shift(tip, direction, -MARKER_LENGTH);
                let half = half_width.unwrap_or(MARKER_HALF_WIDTH);
                vec![
                    tip,
                    marker_side(base, direction, half),
                    marker_side(base, direction, -half),
                ]
            }
        };
        Marker {
            shape: "polygon",
            points: formatted_points(&points),
            filled: filled_marker(kind),
            cx: 0.0,
            cy: 0.0,
            r: 0.0,
            cross: String::new(),
        }
    }

    fn scale_thick_markers(
        &self,
        mut markers: Vec<Marker>,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> Vec<Marker> {
        let factor = relation.width.unwrap_or(0.0) / THICK_LINE;
        if factor <= 1.0 {
            return markers;
        }
        for marker in markers.iter_mut() {
            if marker.shape != "polygon" {
                continue;
            }
            let pairs = point_pairs(&marker.points);
            let Some(first) = pairs.first() else { continue };
            let tip = endpoints
                .values()
                .copied()
                .min_by(|a, b| {
                    let da = js_math::hypot2(a[0] - first[0], a[1] - first[1]);
                    let db = js_math::hypot2(b[0] - first[0], b[1] - first[1]);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            let Some(tip) = tip else { continue };
            let scaled: Vec<[f64; 2]> = pairs
                .iter()
                .map(|point| {
                    [
                        tip[0] + (point[0] - tip[0]) * factor,
                        tip[1] + (point[1] - tip[1]) * factor,
                    ]
                })
                .collect();
            marker.points = formatted_points(&scaled);
        }
        markers
    }

    fn relation_path_for(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> String {
        let ordered = if reversed_qualified(relation) {
            HashMap::from([
                ("left", endpoints["right"]),
                ("right", endpoints["left"]),
                ("control", endpoints.get("control").copied().unwrap_or([0.0, 0.0])),
            ])
        } else {
            endpoints.clone()
        };
        let has_control = endpoints.contains_key("control");
        let mut ordered = ordered;
        if !has_control {
            ordered.remove("control");
        }
        if let Some(path) = self.qualified_special_path(relation, &ordered) {
            return path;
        }
        self.controlled_relation_path(relation, &ordered)
    }

    fn qualified_special_path(
        &self,
        relation: &Relation,
        ordered: &HashMap<&'static str, [f64; 2]>,
    ) -> Option<String> {
        if self.fan_qualifier_relation(relation) {
            return Some(self.fan_qualifier_path(relation, ordered));
        }
        let left = ordered["left"];
        let right = ordered["right"];
        if qualifier(relation) && relation.length > 2.0 && steep(left, right) {
            let vertical = (right[1] - left[1]) / 3.0;
            let outgoing = [left[0] - QUALIFIED_LONG_CURVE_OFFSET, left[1] + vertical];
            let incoming = [right[0] + QUALIFIED_LONG_CURVE_OFFSET, right[1] - vertical];
            return Some(format!(
                "M {} C {} {} {}",
                point(left), point(outgoing), point(incoming), point(right)
            ));
        }
        None
    }

    fn fan_qualifier_relation(&self, relation: &Relation) -> bool {
        let multiplicities: Vec<&String> = [
            relation.left_multiplicity.as_ref(),
            relation.right_multiplicity.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        relation.direction.is_none()
            && relation.length == 2.0
            && multiplicities.len() == 1
            && multiplicities[0].starts_with('[')
    }

    fn fan_qualifier_path(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> String {
        let first = endpoints["left"];
        let last = endpoints["right"];
        let text = [relation.left_multiplicity.as_ref(), relation.right_multiplicity.as_ref()]
            .into_iter()
            .flatten()
            .next()
            .cloned()
            .unwrap_or_default();
        let text = text.trim_start_matches('[').trim_end_matches(']');
        let width = self.standard_text_width(text, FONT_SIZE) + QUALIFIED_FAN_LANE_ALLOWANCE;
        let centre = (first[0] + last[0]) / 2.0;
        let rise = (last[1] - first[1]) / 3.0;
        let left = [centre - width / 2.0, first[1] + rise];
        let right = [centre + width / 2.0, last[1] - rise];
        format!(
            "M {} L {} L {} L {}",
            point(first), point(left), point(right), point(last)
        )
    }

    fn controlled_relation_path(
        &self,
        relation: &Relation,
        ordered: &HashMap<&'static str, [f64; 2]>,
    ) -> String {
        let control = self.qualified_label_control(relation, ordered)
            .or_else(|| self.qualified_lane_control(relation, ordered));
        if let Some(control) = control {
            return format!(
                "M {} Q {} {}",
                point(ordered["left"]),
                point(control),
                point(ordered["right"])
            );
        }
        self.styled_curve_path(relation, ordered)
    }

    fn styled_curve_path(
        &self,
        relation: &Relation,
        ordered: &HashMap<&'static str, [f64; 2]>,
    ) -> String {
        let first = ordered["left"];
        let last = ordered["right"];
        if let Some(controls) = styled_relation_curve::controls(relation, first, last) {
            return format!(
                "M {} C {} {} {}",
                point(first),
                point(controls[0]),
                point(controls[1]),
                point(last)
            );
        }
        self.relation_path(ordered)
    }

    fn relation_path(&self, endpoints: &HashMap<&'static str, [f64; 2]>) -> String {
        let left = endpoints["left"];
        let right = endpoints["right"];
        if let Some(control) = endpoints.get("control") {
            return format!("M {} Q {} {}", point(left), point(*control), point(right));
        }
        format!("M {} L {}", point(left), point(right))
    }

    fn qualified_label_control(
        &self,
        relation: &Relation,
        ordered: &HashMap<&'static str, [f64; 2]>,
    ) -> Option<[f64; 2]> {
        if !qualifier(relation) || !label_arrow(relation) {
            return None;
        }
        let left = ordered["left"];
        let right = ordered["right"];
        if steep(left, right) {
            return None;
        }
        let middle = [(left[0] + right[0]) / 2.0, (left[1] + right[1]) / 2.0];
        let outward = if middle[1] < self.qualified_route_middle { -30.0 } else { 30.0 };
        Some([middle[0], middle[1] + outward])
    }

    fn qualified_lane_control(
        &self,
        relation: &Relation,
        ordered: &HashMap<&'static str, [f64; 2]>,
    ) -> Option<[f64; 2]> {
        let multiplicities: Vec<&String> = [
            relation.left_multiplicity.as_ref(),
            relation.right_multiplicity.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let qualified_count = multiplicities.iter().filter(|t| t.starts_with('[')).count();
        if !(qualified_count == 1 && multiplicities.len() > 1) {
            return None;
        }
        let left = ordered["left"];
        let right = ordered["right"];
        if steep(left, right) {
            return None;
        }
        let middle = [(left[0] + right[0]) / 2.0, (left[1] + right[1]) / 2.0];
        let outward = if middle[1] < self.qualified_route_middle {
            -QUALIFIED_FLAT_CURVE_OFFSET
        } else {
            QUALIFIED_FLAT_CURVE_OFFSET
        };
        Some([middle[0], middle[1] + outward])
    }

    fn label_arrow_scene(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
    ) -> Option<Marker> {
        if !qualifier(relation) || !label_arrow(relation) {
            return None;
        }
        let left = endpoints["left"];
        let right = endpoints["right"];
        let mut direction = unit_vector(left, right);
        if relation.label.as_deref().unwrap_or("").ends_with('<') {
            direction = [-direction[0], -direction[1]];
        }
        let mut centre = self
            .qualified_label_control(relation, endpoints)
            .unwrap_or_else(|| [(left[0] + right[0]) / 2.0, (left[1] + right[1]) / 2.0]);
        if steep(left, right) {
            centre = shift(centre, [1.0, 0.0], QUALIFIED_STEEP_ARROW_OFFSET);
        }
        let tip = shift(centre, direction, 5.0);
        let base = shift(centre, direction, -4.0);
        let points = [tip, marker_side(base, direction, 3.0), marker_side(base, direction, -3.0)];
        Some(Marker {
            shape: "polygon",
            points: formatted_points(&points),
            filled: true,
            cx: 0.0,
            cy: 0.0,
            r: 0.0,
            cross: String::new(),
        })
    }

    fn relation_texts(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        end_texts: &EndTexts,
        sides: &[bool],
        boxed: usize,
        engine: Engine,
    ) -> Vec<Text> {
        let first = end_texts.sides[0];
        let second = end_texts.sides[1];
        let direction = unit_vector(
            endpoints[if first { "left" } else { "right" }],
            endpoints[if second { "left" } else { "right" }],
        );
        let label = self.relation_text_label(relation, endpoints, sides);
        let entries = end_texts.entries(relation, engine);
        let entries: Vec<_> = if boxed == 0 {
            entries
        } else {
            entries
                .into_iter()
                .filter(|entry| !(entry.kind == EntryKind::Qualifier && entry.pieces.len() == 1))
                .collect()
        };
        let mut texts = vec![];
        if let Some(label) = label {
            texts.push(label);
        }
        for entry in &entries {
            if !sides.contains(&entry.side) {
                continue;
            }
            let inward = if entry.side == first { 1.0 } else { -1.0 };
            let endpoint = endpoints[if entry.side { "left" } else { "right" }];
            texts.extend(self.end_text(entry, endpoint, direction, inward));
        }
        texts
    }

    fn relation_text_label(
        &self,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        sides: &[bool],
    ) -> Option<Text> {
        if !sides.contains(&true) {
            return None;
        }
        let content = visible_relation_label(relation.label.as_deref())?;
        let clearance = ((relation.width.unwrap_or(0.0) - 1.0).max(0.0)) / 2.0;
        self.curved_relation_label(&content, relation, endpoints, clearance)
            .or_else(|| self.relation_label(&content, endpoints, clearance))
    }

    fn curved_relation_label(
        &self,
        content: &str,
        relation: &Relation,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        clearance: f64,
    ) -> Option<Text> {
        let left = endpoints["left"];
        let right = endpoints["right"];
        let at = styled_relation_curve::label_point(relation, left, right);
        if content.is_empty() {
            return None;
        }
        let point = at?;
        if endpoints.contains_key("control") {
            return None;
        }
        let descending = (left[0] * left[1] - right[0] * right[1]) > 0.0;
        let descending = descending_product(left, right);
        Some(if descending {
            Text::new(content, point[0] + LABEL_GAP, point[1] - LABEL_OFFSET - clearance, "relation_label", "start")
        } else {
            Text::new(content, point[0] - LABEL_GAP, point[1] - LABEL_OFFSET - clearance, "relation_label", "end")
        })
    }

    fn relation_label(
        &self,
        content: &str,
        endpoints: &HashMap<&'static str, [f64; 2]>,
        clearance: f64,
    ) -> Option<Text> {
        if content.is_empty() {
            return None;
        }
        if let Some(control) = endpoints.get("control") {
            let left = endpoints["left"];
            let right = endpoints["right"];
            let curve_right = (left[0] + 2.0 * control[0] + right[0]) / 4.0;
            let horizontal = curve_right + SELF_RELATION_LABEL_GAP;
            let vertical = (left[1] + right[1]) / 2.0;
            let size = FONT_SIZE * SMALL_TEXT_SCALE;
            let baseline = vertical + size * (1.0 - TEXT_DESCENT) / 2.0;
            return Some(Text::new(content, horizontal, baseline, "relation_label", "start"));
        }
        let left = endpoints["left"];
        let right = endpoints["right"];
        if beside_label(left, right) {
            let horizontal = (left[0] + right[0]) / 2.0;
            let middle = (left[1] + right[1]) / 2.0;
            let size = FONT_SIZE * SMALL_TEXT_SCALE;
            let baseline = middle + size * (1.0 - TEXT_DESCENT) / 2.0;
            let reach = label_clearance::right_between(left, right, baseline - size, baseline);
            return Some(Text::new(
                content,
                horizontal.max(reach) + clearance + LABEL_GAP,
                baseline,
                "relation_label",
                "start",
            ));
        }
        let horizontal = (left[0] + right[0]) / 2.0;
        let middle = (left[1] + right[1]) / 2.0;
        let size = FONT_SIZE * SMALL_TEXT_SCALE;
        let half = self.standard_text_width(content, size) / 2.0;
        let top = label_clearance::top_between(left, right, horizontal - half, horizontal + half);
        let baseline = (middle - LABEL_OFFSET).min(top - size * TEXT_DESCENT) - clearance;
        Some(Text::new(content, horizontal, baseline, "relation_label", "middle"))
    }

    fn end_text(
        &self,
        entry: &super::edges::end_texts::Entry,
        endpoint: [f64; 2],
        direction: [f64; 2],
        inward: f64,
    ) -> Vec<Text> {
        let side = if entry.kind == EntryKind::Role { -1.0 } else { 1.0 };
        let normal = [-direction[1] * side, direction[0] * side];
        let mut inward = inward;
        if entry.kind == EntryKind::Qualifier {
            inward = -inward;
        }
        let along = [direction[0] * inward, direction[1] * inward];
        let point = shift(
            shift(endpoint, along, END_TEXT_ALONG),
            normal,
            END_TEXT_ACROSS,
        );
        let (sideways, upright) = if direction[1].abs() >= direction[0].abs() {
            (normal, along)
        } else {
            (along, normal)
        };
        let anchor = if sideways[0] > 0.0 { "start" } else { "end" };
        let size = FONT_SIZE * SMALL_TEXT_SCALE;
        let drop = if upright[1] > 0.0 { size } else { -(size * TEXT_DESCENT) };
        let point = [point[0], point[1] + drop];
        if entry.multiline {
            let line_height = FONT_SIZE * SMALL_TEXT_SCALE * 1.215;
            let top = point[1] - line_height * (entry.pieces.len() as f64 - 1.0);
            return entry
                .pieces
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    Text::new(line, point[0], top + index as f64 * line_height, "multiplicity", anchor)
                })
                .collect();
        }
        if entry.pieces.len() == 1 {
            return vec![Text::new(&entry.pieces[0], point[0], point[1], "multiplicity", anchor)];
        }
        let joined = entry.pieces.join(" ");
        let width = self.end_text_width(&joined);
        let start = point[0]
            - match anchor {
                "end" => width,
                "middle" => width / 2.0,
                _ => 0.0,
            };
        let offset = self.end_text_width(&format!("{} ", entry.pieces[0]));
        vec![
            Text::new(&entry.pieces[0], start, point[1], "multiplicity", "start"),
            Text::new(&entry.pieces.last().unwrap(), start + offset, point[1], "multiplicity", "start"),
        ]
    }

    // ---- the dress pass ----

    fn dress(&mut self, diagram: &Diagram, scene: &mut Scene) {
        let layout = &*self;
        let measure = move |text: &str| layout.measure(text, FONT_SIZE);
        super::class_header::apply(diagram, scene, &measure, self.kind_in_spot);
        let size_measure = move |text: &str, size: f64| layout.measure(text, size);
        let captions = CaptionLayout::new(diagram, FONT_SIZE, &size_measure);
        captions.dress(scene);
    }

    // ---- scene assembly ----

    fn build_scene(&mut self, diagram: &Diagram, boxes: Vec<Box>) -> Scene {
        let measured = |text: &str| self.measured_width(text);
        let frames = PackageFrames::new(diagram, &measured).call(&boxes);
        let (width, height) = self.scene_dimensions(diagram, &boxes, &frames);
        let mut all = boxes.clone();
        for frame in &frames {
            all.push(Box {
                id: frame.id.clone(),
                x: frame.x,
                y: frame.y,
                width: frame.width,
                height: frame.height,
                texts: vec![],
                separators: vec![],
                kind: "frame",
                fill: None,
                spot: None,
            });
        }
        let relations = self.scene_relations(diagram, &all, &HashMap::new());
        Scene {
            width,
            height,
            boxes,
            frames,
            relations,
            ..Scene::default()
        }
    }

    fn scene_dimensions(&self, diagram: &Diagram, boxes: &[Box], frames: &[super::scene::Frame]) -> (f64, f64) {
        if !super::frames::relation_layout(diagram) {
            let width = self.canvas_width(diagram, boxes);
            let height = self.scene_height(diagram, boxes);
            return (width, height);
        }
        let mut figures: Vec<(f64, f64, f64, f64)> = boxes
            .iter()
            .map(|box_| (box_.x, box_.y, box_.width, box_.height))
            .collect();
        for frame in frames {
            figures.push((frame.x, frame.y, frame.width, frame.height));
        }
        let width = figures
            .iter()
            .map(|(x, _, w, _)| x + w)
            .fold(f64::NEG_INFINITY, f64::max)
            + PACKAGE_CANVAS_EDGE;
        let height = figures
            .iter()
            .map(|(_, y, _, h)| y + h)
            .fold(f64::NEG_INFINITY, f64::max)
            + PACKAGE_CANVAS_EDGE;
        (width, height)
    }

    fn scene_height(&self, diagram: &Diagram, boxes: &[Box]) -> f64 {
        let mut rows: Vec<(f64, Vec<&Box>)> = vec![];
        for box_ in boxes {
            match rows.iter_mut().find(|(y, _)| *y == box_.y) {
                Some((_, row)) => row.push(box_),
                None => rows.push((box_.y, vec![box_])),
            }
        }
        let room = super::frames::OPEN + super::frames::CLOSE;
        let heights: Vec<f64> = rows
            .iter()
            .map(|(_, row)| row.iter().map(|box_| box_.height).fold(0.0_f64, f64::max))
            .collect();
        let total = crate::render::pie::fsum(heights)
            + ((rows.len() as f64 - 1.0) * ROW_GAP)
            + MARGIN * 2.0;
        total + diagram.packages.len() as f64 * room
            + if diagram.packages.is_empty() { MARGIN } else { 0.0 }
    }

    fn canvas_width(&self, diagram: &Diagram, boxes: &[Box]) -> f64 {
        let right = boxes
            .iter()
            .map(|box_| box_.x + box_.width)
            .fold(f64::NEG_INFINITY, f64::max);
        let left = boxes.iter().map(|box_| box_.x).fold(f64::INFINITY, f64::min);
        right + left + if diagram.packages.is_empty() { MARGIN } else { 0.0 }
    }
}

// ---- free functions and small helpers ----

#[derive(Clone, Debug)]
struct RowGeometry {
    head: f64,
    baselines: Vec<f64>,
}

impl Spec {
    fn member_rows_titles(&self) -> Vec<&str> {
        self.member_rows.iter().map(String::as_str).collect()
    }
}

/// Ruby `note_specifications`.
fn note_specifications(notes: &[Note], measure: &dyn Fn(&str) -> f64) -> Vec<Spec> {
    notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let rows = note.lines.clone();
            let widest = rows
                .iter()
                .map(|row| measure(row))
                .fold(0.0_f64, f64::max);
            Spec {
                id: format!("note-{index}"),
                note: Some(note.clone()),
                width: widest + BOX_PADDING * 2.0,
                height: BOX_PADDING * 2.0 + rows.len().max(1) as f64 * ROW_HEIGHT,
                title_rows: vec![],
                member_rows: vec![],
                member_rows_count: 0,
                name_rows: 0,
                floating_generic: None,
                compact_members: false,
                compact_title_rows: false,
                stereotype_head: None,
                member_modifiers: vec![],
                package: None,
                member_kinds: vec![],
                member_visibilities: vec![],
                icons_enabled: false,
                empty_gaps: true,
                chain: vec![],
                compact_bottom: 0.0,
            }
        })
        .collect()
}

fn boxes_only_scene(boxes: Vec<Box>) -> Scene {
    let width = boxes
        .iter()
        .map(|box_| box_.x + box_.width)
        .fold(f64::NEG_INFINITY, f64::max)
        + MARGIN * 2.0;
    let height = boxes
        .iter()
        .map(|box_| box_.y + box_.height)
        .fold(f64::NEG_INFINITY, f64::max)
        + MARGIN * 2.0;
    Scene {
        width,
        height,
        boxes,
        ..Scene::default()
    }
}

fn member_note_height(note: &Note) -> f64 {
    10.0 + note.lines.len().max(1) as f64 * MEMBER_NOTE_LINE_HEIGHT
}

fn stereotype_note_fill(stereotype: &Option<String>, directives: &[String]) -> Option<String> {
    let stereotype = stereotype.as_deref()?;
    // The reader stores the stereotype's text; either form here names it.
    let name = stereotype
        .strip_prefix("<<")
        .and_then(|s| s.strip_suffix(">>"))
        .unwrap_or(stereotype);
    let prefix = format!("skinparam\\s+noteBackgroundColor<<{}>>", regex::escape(name));
    let pattern = regex::Regex::new(&format!(r"(?i)\A{}\s+(\S+)\z", prefix)).unwrap();
    directives
        .iter()
        .rev()
        .find(|line| pattern.is_match(line))
        .and_then(|line| pattern.captures(line))
        .map(|m| m[1].to_string())
}

fn standalone_diagram(diagram: &Diagram) -> bool {
    diagram.relations.is_empty()
        && diagram.junctions.is_empty()
        && diagram.packages.is_empty()
}

fn plain_class(klass: &super::model::Klass) -> bool {
    klass.kind == Kind::Class && klass.stereotypes.is_empty() && klass.generics.is_none()
}

fn member_note_family(diagram: &Diagram) -> bool {
    if diagram.classes.len() != 1 || diagram.notes.is_empty() || !standalone_diagram(diagram) {
        return false;
    }
    let klass = &diagram.classes[0];
    if !plain_class(klass) {
        return false;
    }
    let members: Vec<&String> = klass.body.iter().map(|m| &m.name).collect();
    diagram.notes.iter().all(|note| {
        note.member.is_some()
            && members.contains(&note.member.as_ref().unwrap())
            && note.target.as_deref() == Some(klass.name.as_str())
            && note.side == NoteSide::Right
    })
}

fn class_note_pair_family(diagram: &Diagram) -> bool {
    let classes = &diagram.classes;
    let notes = &diagram.notes;
    if !(classes.len() > 1 && notes.len() == classes.len()) {
        return false;
    }
    if !standalone_diagram(diagram) {
        return false;
    }
    if !classes.iter().all(|klass| plain_class(klass) && klass.body.is_empty()) {
        return false;
    }
    if !notes
        .iter()
        .all(|note| note.member.is_none() && note.side == NoteSide::Right)
    {
        return false;
    }
    let mut targets: Vec<String> = notes.iter().map(|n| n.target.clone().unwrap_or_default()).collect();
    targets.sort();
    let mut names: Vec<String> = classes.iter().map(|k| k.name.clone()).collect();
    names.sort();
    targets == names
}

fn loose_member_class_family(diagram: &Diagram) -> bool {
    diagram.classes.len() > 1
        && diagram.notes.is_empty()
        && standalone_diagram(diagram)
        && diagram.classes.iter().any(|klass| !klass.body.is_empty())
}

fn isolated_class_kind_family(diagram: &Diagram) -> bool {
    diagram.classes.len() > 1
        && diagram.notes.is_empty()
        && standalone_diagram(diagram)
        && diagram.classes.iter().any(|klass| klass.kind != Kind::Class)
}

fn lone_standard_class(klass: &super::model::Klass, diagram: &Diagram) -> bool {
    diagram.classes.len() == 1
        && diagram.notes.is_empty()
        && standalone_diagram(diagram)
        && klass.kind == Kind::Class
        && klass.generics.is_none()
}

fn hide_circle_exact(line: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\Ahide[ \t]+circle\z").unwrap());
    re.is_match(line)
}

fn hide_or_show(line: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)\A(?:hide|show)\b").unwrap());
    re.is_match(line)
}

static MEMBER_PORTIONS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

fn member_portions(line: &str) -> bool {
    let re = MEMBER_PORTIONS.get_or_init(|| {
        regex::Regex::new(r"(?ix)\A(?:hide|show)\b.*\b(?:members|attributes|methods|fields)\z").unwrap()
    });
    re.is_match(line)
}

fn empty_gaps(diagram: &Diagram) -> bool {
    !diagram.directives.iter().any(|line| member_portions(line))
}

fn qualified_compact(diagram: &Diagram) -> bool {
    diagram.directives.iter().any(|line| line == "hide empty members")
}

fn compact_empty_members(diagram: &Diagram) -> bool {
    rank_chain::engine(diagram) == Engine::Smetana
        && diagram.directives.iter().any(|line| line == "hide empty members")
}

fn compact_empty_item(item: &Spec, diagram: &Diagram) -> bool {
    compact_empty_members(diagram) && item.member_rows.is_empty()
}

fn plain_empty_class(klass: &super::model::Klass, diagram: &Diagram, names: &[String]) -> bool {
    if !simple_empty_class(klass, names) || !uniform_empty_class_lengths(diagram) {
        return false;
    }
    !diagram.directives.iter().any(|line| {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = RE.get_or_init(|| {
            regex::Regex::new(r"(?i)\Ahide[ \t]+(?:circle|empty[ \t]+members)\z").unwrap()
        });
        re.is_match(line)
    })
}

fn simple_empty_class(klass: &super::model::Klass, names: &[String]) -> bool {
    plain_class(klass) && klass.body.is_empty() && klass.package.is_none() && names.len() == 1
}

fn uniform_empty_class_lengths(diagram: &Diagram) -> bool {
    let mut lengths: Vec<usize> = vec![];
    for klass in &diagram.classes {
        let rows = name_rows(klass, false);
        if !simple_empty_class(klass, &rows) {
            return false;
        }
        lengths.push(rows[0].chars().count());
    }
    let mut unique = lengths.clone();
    unique.sort();
    unique.dedup();
    unique.len() == 1
}

fn heterogeneous_smetana_empty_class(
    klass: &super::model::Klass,
    diagram: &Diagram,
    names: &[String],
) -> bool {
    simple_empty_class(klass, names) && heterogeneous_smetana_empty_classes(diagram)
}

fn heterogeneous_smetana_empty_classes(diagram: &Diagram) -> bool {
    if !plain_smetana_diagram(diagram) {
        return false;
    }
    let mut names: Vec<String> = vec![];
    for klass in &diagram.classes {
        let rows = name_rows(klass, false);
        if !simple_empty_class(klass, &rows) {
            return false;
        }
        names.push(rows[0].clone());
    }
    let mut unique = names.iter().map(|n| n.chars().count()).collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    unique.len() > 1
}

fn plain_smetana_diagram(diagram: &Diagram) -> bool {
    rank_chain::engine(diagram) == Engine::Smetana
        && !compact_empty_members(diagram)
        && !diagram.directives.iter().any(|line| hide_or_show(line))
}

/// Ruby `header_rows`.
fn header_rows(klass: &super::model::Klass, kind_in_spot: bool) -> Vec<String> {
    let mut rows = vec![];
    let spot_names_kind = klass.kind == Kind::Class
        || (kind_in_spot
            && glyph_exists(klass.kind)
            && class_name::lines(&klass.name).len() == 1);
    if !spot_names_kind {
        rows.push(format!("<<{}>>", klass.kind.as_str()));
    }
    if !klass.stereotypes.is_empty() {
        rows.push(klass
            .stereotypes
            .iter()
            .map(|tag| format!("<<{tag}>>"))
            .collect::<Vec<_>>()
            .join(" "));
    }
    rows
}

fn glyph_exists(_kind: Kind) -> bool {
    // Ruby ClassHeader::GLYPHS covers every kind.
    true
}

fn replace_stereo(row: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"<<([^<>]+)>>").unwrap());
    re.replace_all(row, "\u{AB}$1\u{BB}").to_string()
}

/// Ruby `name_rows`.
pub fn name_rows(klass: &super::model::Klass, separate_generics: bool) -> Vec<String> {
    let lines = class_name::lines(&klass.name);
    let last = lines.last().cloned().unwrap_or_default();
    let first: Vec<String> = lines[..lines.len().saturating_sub(1)].to_vec();
    let mut rows = first;
    match (separate_generics, &klass.generics) {
        (true, Some(generics)) => {
            rows.push(last);
            rows.push(generics.clone());
        }
        _ => {
            rows.push(match &klass.generics {
                Some(generics) => format!("{last}<{generics}>"),
                None => last,
            });
        }
    }
    rows
}

/// Ruby `member_text`.
pub fn member_text(member: &Member) -> String {
    if member.kind == MemberKind::Separator {
        return member.name.clone();
    }
    let mut name = member.name.clone();
    if let Some(parameters) = &member.parameters {
        name.push_str(&format!("({parameters})"));
    }
    let content = match (&member.kind_type, member.type_position) {
        (Some(kind_type), super::model::TypePosition::Prefix) => {
            format!("{kind_type} {name}")
        }
        (Some(kind_type), _) => {
            format!("{name}{}{kind_type}", member.type_separator)
        }
        (None, _) => name,
    };
    format!("{}{content}", visibility_mark(member.visibility))
}

fn visibility_mark(visibility: Option<super::model::Visibility>) -> String {
    visibility.map(|v| v.mark().to_string()).unwrap_or_default()
}

fn rule_modifiers(member: &Member) -> Vec<&'static str> {
    if member.kind == MemberKind::Separator {
        let character = match member.kind_type.as_deref() {
            Some(".") => ".",
            Some("=") => "=",
            Some("_") => "_",
            _ => "-",
        };
        return vec!["rule", character];
    }
    member.modifiers.clone()
}

fn member_group_boundaries(kinds: &[MemberKind]) -> Vec<usize> {
    kinds
        .windows(2)
        .enumerate()
        .skip(1)
        .filter_map(|(index, pair)| {
            (pair[0] == MemberKind::Field && pair[1] == MemberKind::Method).then_some(index)
        })
        .collect()
}

/// Ruby `rows_by_package`.
fn rows_by_package(specifications: &[Spec], diagram: &Diagram) -> Vec<Vec<Option<Spec>>> {
    let columns = column_count(specifications);
    let mut rows = vec![];
    for group in package_groups(specifications, diagram) {
        for chunk in group.chunks(columns) {
            let mut row: Vec<Option<Spec>> = chunk.iter().map(|item| Some(item.clone())).collect();
            while row.len() < columns {
                row.push(None);
            }
            rows.push(row);
        }
    }
    rows
}

fn package_groups(specifications: &[Spec], diagram: &Diagram) -> Vec<Vec<Spec>> {
    let ids = package_group_ids(diagram);
    let mut groups = vec![];
    for id in ids {
        let group: Vec<Spec> = specifications
            .iter()
            .filter(|item| item.package == id)
            .cloned()
            .collect();
        if !group.is_empty() {
            groups.push(group);
        }
    }
    groups
}

fn package_group_ids(diagram: &Diagram) -> Vec<Option<String>> {
    let package_ids: Vec<String> = diagram.packages.iter().map(|p| p.id.clone()).collect();
    let source_package = diagram.relations.iter().any(|relation| {
        package_ids.contains(&relation.left) && !package_ids.contains(&relation.right)
    });
    if source_package {
        let mut ids: Vec<Option<String>> =
            package_ids.into_iter().map(Some).collect();
        ids.push(None);
        ids
    } else {
        let mut ids = vec![None];
        ids.extend(package_ids.into_iter().map(Some));
        ids
    }
}

fn column_count(specifications: &[Spec]) -> usize {
    if specifications.len() > 1 { 2 } else { 1 }
}

fn stable_centers(diagram: &Diagram) -> bool {
    let horizontal = diagram.directives.iter().any(|line| line == "left to right direction");
    diagram.relations.iter().any(|relation| {
        let qualifiers = [&relation.left_multiplicity, &relation.right_multiplicity];
        let qualified = qualifiers
            .iter()
            .any(|text| text.as_deref().map(|t| t.starts_with('[')).unwrap_or(false));
        qualified
            || (horizontal
                && relation.left_role.is_some()
                && relation.right_role.is_some())
    })
}

fn intrinsic_box_widths(diagram: &Diagram) -> bool {
    super::frames::relation_layout(diagram) || intrinsic_content(diagram)
}

fn intrinsic_content(diagram: &Diagram) -> bool {
    let body = diagram.classes.iter().any(|klass| !klass.body.is_empty());
    !diagram.notes.is_empty()
        || body
        || heterogeneous_smetana_empty_classes(diagram)
        || diagram.directives.iter().any(|line| hide_or_show(line))
}

fn widest_box(specifications: &[Spec]) -> f64 {
    specifications
        .iter()
        .map(|item| item.width)
        .fold(0.0_f64, f64::max)
}

fn row_height(row: &[Option<Spec>]) -> f64 {
    row.iter()
        .flatten()
        .map(|item| item.height)
        .fold(0.0_f64, f64::max)
}

fn hide_members_row_width(row: &[&Spec]) -> f64 {
    let widths: Vec<f64> = row.iter().map(|item| item.width).collect();
    crate::render::pie::fsum(widths.into_iter().chain(
        std::iter::repeat(ISOLATED_KIND_COLUMN_GAP).take(row.len().saturating_sub(1)),
    ))
}

fn hide_members_grid_row(row: &[&Spec], left: f64, top: f64) -> Vec<Box> {
    // The caller re-builds through the layout; this free form is unused.
    let _ = (row, left, top);
    vec![]
}

fn isolated_class_kind_row_tops<'a>(rows: &[Vec<&'a Spec>]) -> Vec<f64> {
    let heights: Vec<f64> = rows
        .iter()
        .map(|row| row.iter().map(|item| item.height).fold(0.0_f64, f64::max))
        .collect();
    let mut tops = vec![MARGIN];
    for height in &heights {
        let next = *tops.last().unwrap() + height + ISOLATED_KIND_ROW_GAP;
        tops.push(next);
    }
    tops
}

fn isolated_class_kind_row_tops_owned(rows: &[Vec<Spec>]) -> Vec<f64> {
    let heights: Vec<f64> = rows
        .iter()
        .map(|row| row.iter().map(|item| item.height).fold(0.0_f64, f64::max))
        .collect();
    let mut tops = vec![MARGIN];
    for height in &heights {
        let next = *tops.last().unwrap() + height + ISOLATED_KIND_ROW_GAP;
        tops.push(next);
    }
    tops
}

fn isolated_class_kind_column_widths(rows: &[Vec<Spec>]) -> Vec<f64> {
    let columns = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column).map(|item| item.width))
                .fold(0.0_f64, f64::max)
        })
        .collect()
}

fn isolated_class_kind_column_lefts(widths: &[f64]) -> Vec<f64> {
    let mut lefts = vec![MARGIN];
    for width in widths {
        let next = *lefts.last().unwrap() + width + ISOLATED_KIND_COLUMN_GAP;
        lefts.push(next);
    }
    lefts
}

fn chain_dimensions(boxes: &[Box]) -> (f64, f64) {
    let width = boxes
        .iter()
        .map(|box_| box_.x + box_.width)
        .fold(f64::NEG_INFINITY, f64::max)
        + MARGIN * 2.0;
    let height = boxes
        .iter()
        .map(|box_| box_.y + box_.height)
        .fold(f64::NEG_INFINITY, f64::max)
        + MARGIN * 2.0;
    (width, height)
}

fn qualified_route_middle(boxes: &[Box]) -> f64 {
    let count = boxes.len() as f64;
    crate::render::pie::fsum(boxes.iter().map(|box_| box_.y + box_.height / 2.0)) / count
}

fn package_frame(box_: &Box) -> bool {
    box_.id.starts_with("package-")
}

fn box_centre(box_: &Box) -> [f64; 2] {
    [box_.x + box_.width / 2.0, box_.y + box_.height / 2.0]
}

fn axis_scale(size: f64, delta: f64) -> Option<f64> {
    if delta == 0.0 {
        return None;
    }
    Some((size / 2.0) / delta.abs())
}

fn pair_of(a: &str, b: &str) -> [String; 2] {
    let mut pair = [a.to_string(), b.to_string()];
    pair.sort();
    pair
}

fn unit_vector(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let length = js_math::hypot2(dx, dy);
    if length == 0.0 {
        return [1.0, 0.0];
    }
    [dx / length, dy / length]
}

fn shift(origin: [f64; 2], direction: [f64; 2], distance: f64) -> [f64; 2] {
    [
        origin[0] + direction[0] * distance,
        origin[1] + direction[1] * distance,
    ]
}

fn marker_side(base: [f64; 2], direction: [f64; 2], distance: f64) -> [f64; 2] {
    shift(base, [-direction[1], direction[0]], distance)
}

#[derive(Clone, Copy)]
enum MarkerShape {
    Arrow,
    Diamond,
    Triangle,
}

fn marker_shape(kind: super::model::ArrowKind) -> MarkerShape {
    use super::model::ArrowKind::*;
    match kind {
        Aggregation | Composition => MarkerShape::Diamond,
        Association | Dependency => MarkerShape::Arrow,
        _ => MarkerShape::Triangle,
    }
}

fn filled_marker(kind: super::model::ArrowKind) -> bool {
    use super::model::ArrowKind::*;
    matches!(kind, Association | Dependency | Composition)
}

fn formatted_points(points: &[[f64; 2]]) -> String {
    points
        .iter()
        .map(|p| format!("{},{}", ruby_round3(p[0]), ruby_round3(p[1])))
        .collect::<Vec<_>>()
        .join(" ")
}

fn point_pairs(points: &str) -> Vec<[f64; 2]> {
    points
        .split_whitespace()
        .map(|pair| {
            let mut parts = pair.split(',');
            let x = parts.next().and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
            let y = parts.next().and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
            [x, y]
        })
        .collect()
}

fn steep(left: [f64; 2], right: [f64; 2]) -> bool {
    let dx = right[0] - left[0];
    let dy = right[1] - left[1];
    dy.abs() > dx.abs()
}

fn descending_product(left: [f64; 2], right: [f64; 2]) -> bool {
    (right[0] - left[0]) * (right[1] - left[1]) > 0.0
}

fn beside_label(left: [f64; 2], right: [f64; 2]) -> bool {
    let dx = right[0] - left[0];
    let dy = right[1] - left[1];
    dy.abs() > dx.abs() * BESIDE_LABEL_SLOPE
}

fn reversed_qualified(relation: &Relation) -> bool {
    let qualifiers = [&relation.left_multiplicity, &relation.right_multiplicity];
    let any = qualifiers
        .iter()
        .any(|text| text.as_deref().map(|t| t.starts_with('[')).unwrap_or(false));
    any && matches!(relation.direction, Some(Direction::Up) | Some(Direction::Left))
}

fn label_arrow(relation: &Relation) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"[ \t]+[<>]\z").unwrap());
    relation
        .label
        .as_deref()
        .map(|label| re.is_match(label))
        .unwrap_or(false)
}

fn visible_relation_label(label: Option<&str>) -> Option<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"[ \t]+[<>]\z").unwrap());
    label.map(|label| {
        if re.is_match(label) {
            re.replace(label, "").to_string()
        } else {
            label.to_string()
        }
    })
}

fn svg_colour(color: Option<&str>) -> Option<String> {
    let color = color?;
    if color.chars().count() == 3
        || color.chars().count() == 6
        || color.chars().count() == 8
    {
        let hex = color.chars().all(|c| c.is_ascii_hexdigit());
        if hex {
            return Some(format!("#{color}"));
        }
    }
    Some(color.to_string())
}

/// Ruby `point`: coordinates rounded to three decimals, joined.
fn point(coordinates: [f64; 2]) -> String {
    format!("{} {}", ruby_round3(coordinates[0]), ruby_round3(coordinates[1]))
}

fn ruby_round3(value: f64) -> String {
    crate::svg::ruby_float((value * 1000.0).round() / 1000.0)
}

/// Ruby `format('%g', x.round(4))`. printf formats the double's shortest
/// decimal representation, rounding it half-even at six significant
/// digits.
pub fn format_g(value: f64) -> String {
    let value = round4_half(value);
    let mut text = if value == 0.0 {
        "0".to_string()
    } else {
        digits_of(value.abs())
    };
    if value < 0.0 {
        text.insert(0, '-');
    }
    text
}

/// Ruby `format('%g', v)`: the shortest decimal digits rounded half-even
/// at six significant digits, trailing zeros stripped, the exponent form
/// outside the fixed window.
fn digits_of(value: f64) -> String {
    let negative = value < 0.0;
    let shortest = format!("{}", value.abs());
    let (mantissa, exponent) = match shortest.split_once('e') {
        Some((m, e)) => (m.to_string(), e.parse::<i32>().unwrap_or(0)),
        None => (shortest.clone(), 0),
    };
    let (whole, fraction) = mantissa_parts(&mantissa);
    // The digits and the position of the decimal point before rounding.
    let mut digits: String = format!("{whole}{fraction}")
        .trim_start_matches('0')
        .to_string();
    let mut scientific = if whole.is_empty() || whole == "0" {
        // 0.0ddd: the exponent is one past the leading zero run.
        let lead = fraction
            .find(|c: char| c != '0')
            .unwrap_or(fraction.len());
        -(lead as i32 + 1)
    } else {
        whole.len() as i32 - 1 + exponent
    };
    if digits.is_empty() {
        digits = "0".to_string();
        scientific = 0;
    }
    // Round to six significant digits. A dropped digit below 5 rounds
    // down and strips the kept zeros; a dropped digit above 5 rounds up;
    // an exact decimal tie (a lone 5 with zeros after) splits by the
    // parity of the last kept digit — odd rounds up, even rounds down,
    // and where the double itself sits above the down decimal the zeros
    // stay (Ruby's dtoa window holds them).
    const SIG: usize = 6;
    let mut keep_zeros = false;
    if digits.len() > SIG {
        let keep = &digits[..SIG];
        let rest = &digits[SIG..];
        let mut up = match rest.as_bytes()[0].cmp(&b'5') {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => {
                if rest[1..].bytes().any(|b| b != b'0') {
                    true
                } else {
                    let last = keep.as_bytes()[SIG - 1] - b'0';
                    if last % 2 == 1 {
                        true
                    } else {
                        // The double's own decimal side decides: a value
                        // at or above the midpoint decimal keeps the
                        // window's zeros; below it, they strip.
                        let point = (scientific + 1).max(1) as usize;
                        let above = value_at_or_above(value, &keep, scientific, point);
                        keep_zeros = above;
                        false
                    }
                }
            }
        };
        if !up && !keep_zeros && rest.as_bytes()[0] != b'5' {
            // a strict down: the strip happens below
        }
        let _ = &mut up;
        if up {
            let mut carried: Vec<u8> = keep.bytes().collect();
            let mut index = SIG;
            loop {
                if index == 0 {
                    carried.insert(0, b'1');
                    scientific += 1;
                    break;
                }
                index -= 1;
                if carried[index] == b'9' {
                    carried[index] = b'0';
                } else {
                    carried[index] += 1;
                    break;
                }
            }
            digits = String::from_utf8(carried).unwrap();
        } else {
            digits = keep.to_string();
        }
    }
    if !keep_zeros {
        digits = digits.trim_end_matches('0').to_string();
    }
    if digits.is_empty() {
        digits = "0".to_string();
    }
    let sign = if negative { "-" } else { "" };
    if scientific < -4 || scientific >= 6 {
        let mantissa = if digits.len() == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let mantissa = if keep_zeros {
            mantissa
        } else {
            trim_zeros(&mantissa)
        };
        if scientific < 0 {
            return format!("{sign}{mantissa}e-{:02}", -scientific);
        }
        return format!("{sign}{mantissa}e+{:02}", scientific);
    }
    if scientific < 0 {
        let zeros = "0".repeat((-scientific - 1) as usize);
        let body = format!("0.{zeros}{digits}");
        return format!("{sign}{}", trim_zeros(&body));
    }
    let point = (scientific + 1) as usize;
    if digits.len() <= point {
        let zeros = "0".repeat(point - digits.len());
        return format!("{sign}{digits}{zeros}");
    }
    let body = format!("{}.{}", &digits[..point], &digits[point..]);
    format!("{sign}{}", if keep_zeros { body } else { trim_zeros(&body) })
}

/// Whether `value` sits at or above the exact decimal
/// `<keep-digits>5` at `scientific` — the rounding tie's midpoint —
/// by comparing the double's exact decimal expansion digit by digit.
fn value_at_or_above(
    value: f64,
    keep: &str,
    scientific: i32,
    point: usize,
) -> bool {
    let exact = format!("{:.70}", value);
    let negative = exact.starts_with('-');
    let exact = exact.trim_start_matches('-');
    let (whole, fraction) = exact.split_once('.').unwrap_or((exact, ""));
    // The midpoint: keep's digits with a 5 appended at the 7th place.
    let mut mid_fraction = String::new();
    if point > keep.len() {
        mid_fraction.push_str(&"0".repeat(point - keep.len()));
        mid_fraction.push_str(keep);
    } else {
        mid_fraction.push_str(&keep[point.min(keep.len())..]);
    }
    mid_fraction.push('5');
    // Align: the midpoint's integer part is keep[..point].
    let mid_whole = &keep[..point.min(keep.len())];
    let _ = negative;
    let value_side = whole_cmp(whole, fraction, mid_whole, &mid_fraction);
    if negative {
        // A negative below the midpoint strips: reverse the order.
        !value_side
    } else {
        value_side
    }
}

/// Lexicographic comparison of two same-sign decimal expansions:
/// `true` when the first is at or above the second.
fn whole_cmp(whole_a: &str, frac_a: &str, whole_b: &str, frac_b: &str) -> bool {
    let width = whole_a.len().max(whole_b.len());
    let a_int = format!("{}{}", "0".repeat(width - whole_a.len()), whole_a);
    let b_int = format!("{}{}", "0".repeat(width - whole_b.len()), whole_b);
    if a_int != b_int {
        return a_int >= b_int;
    }
    let width = frac_a.len().max(frac_b.len());
    let a = format!("{frac_a}{}", "0".repeat(width - frac_a.len()));
    let b = format!("{frac_b}{}", "0".repeat(width - frac_b.len()));
    a >= b
}

fn mantissa_parts(text: &str) -> (String, String) {
    match text.split_once('.') {
        Some((whole, fraction)) => (whole.to_string(), fraction.to_string()),
        None => (text.to_string(), String::new()),
    }
}

pub(super) fn round4_half(value: f64) -> f64 {
    // Ruby Float#round(4): half-up over the shortest decimal
    // representation, not over the binary value.
    let negative = value < 0.0;
    let shortest = format!("{}", value.abs());
    let (integer, fraction) = mantissa_parts(&shortest);
    if fraction.len() <= 4 {
        return value;
    }
    let mut digits: Vec<u8> = format!("{}{}", integer, fraction)
        .bytes()
        .take(integer.len() + 4)
        .collect();
    if fraction.as_bytes()[4] >= b'5' {
        let mut index = digits.len();
        loop {
            if index == 0 {
                digits.insert(0, b'1');
                break;
            }
            index -= 1;
            if digits[index] == b'9' {
                digits[index] = b'0';
            } else {
                digits[index] += 1;
                break;
            }
        }
    }
    let text: String = digits.iter().map(|d| *d as char).collect();
    let point = text.len() as i32 - 4;
    let magnitude: f64 = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), text)
            .parse()
            .unwrap_or(0.0)
    } else if point >= text.len() as i32 {
        format!("{}{}", text, "0".repeat((point - text.len() as i32) as usize))
            .parse()
            .unwrap_or(0.0)
    } else {
        format!("{}.{}", &text[..point as usize], &text[point as usize..])
            .parse()
            .unwrap_or(0.0)
    };
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

/// The shortest decimal form as sign, digits without the point, and the
/// decimal exponent of the last digit (so "132.8955" is ("", "1328955", -4)
/// and "1.23457e+06" keeps its own shape handled by the caller's parse).
fn split_decimal(text: &str) -> (String, String, i32) {
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-".to_string(), rest),
        None => (String::new(), text),
    };
    let Some((mantissa, tail)) = rest.split_once('e') else {
        let (whole, fraction) = mantissa_parts(rest);
        let digits: String = format!("{whole}{fraction}");
        let digits = digits.trim_start_matches('0').to_string();
        // The exponent of the last kept digit.
        let exponent = -(fraction.len() as i32);
        return (sign, digits, exponent);
    };
    // Exponent form: the exponent of the last digit.
    let exponent: i32 = tail.parse().unwrap_or(0);
    let (whole, fraction) = mantissa_parts(mantissa);
    let digits: String = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0').to_string();
    (sign, digits, exponent - fraction.len() as i32)
}


fn trim_zeros(text: &str) -> String {
    let mut out = text.trim_end_matches('0').to_string();
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" {
        return "0".to_string();
    }
    out
}