//! Port of Ruby `Notation::PlantUML::QualifiedAssociationLayout`
//! (`qualified_association_layout.rb`): places the small graph family whose
//! bracketed relation qualifiers make PlantUML honour written direction
//! words and one-dash same-rank links.
use super::model::{Diagram, Direction, Relation};
use std::collections::HashMap;

const RADIAL_HORIZONTAL_GAP: f64 = 80.0;
const FAN_HORIZONTAL_GAP: f64 = 35.0;
const MIXED_HORIZONTAL_GAP: f64 = 70.0;
const COMPACT_HORIZONTAL_GAP: f64 = 130.0;
const STANDARD_VERTICAL_GAP: f64 = 100.0;
const MIXED_NEAR_VERTICAL_GAP: f64 = 60.0;
const MIXED_LONG_VERTICAL_GAP: f64 = 80.0;
const COMPACT_VERTICAL_GAP: f64 = 120.0;
const COMPONENT_GAP: f64 = 25.0;
const FAN_PARENT_ALLOWANCE: f64 = 35.0;

pub fn qualifier(relation: &Relation) -> bool {
    [relation.left_multiplicity.as_ref(), relation.right_multiplicity.as_ref()]
        .iter()
        .flatten()
        .any(|text| text.starts_with('['))
}

fn isolated_class_graph(diagram: &Diagram) -> bool {
    diagram.packages.is_empty() && diagram.notes.is_empty() && diagram.junctions.is_empty()
}

fn qualified_relations(relations: &[Relation]) -> bool {
    relations.iter().any(qualifier)
        && relations.iter().all(|relation| {
            relation.left_role.is_none() && relation.right_role.is_none()
        })
}

pub struct QualifiedAssociationLayout {
    names: Vec<String>,
    classes: super::model::Diagram,
    relations: Vec<Relation>,
    directives: Vec<String>,
}

impl QualifiedAssociationLayout {
    /// Ruby `QualifiedAssociationLayout.for`.
    pub fn for_diagram(diagram: &Diagram) -> Option<QualifiedAssociationLayout> {
        if !isolated_class_graph(diagram) || !qualified_relations(&diagram.relations) {
            return None;
        }
        Some(QualifiedAssociationLayout {
            names: diagram.classes.iter().map(|klass| klass.name.clone()).collect(),
            classes: diagram.clone(),
            relations: diagram.relations.clone(),
            directives: diagram.directives.clone(),
        })
    }

    /// Ruby `fan_relations`: parents whose every outgoing relation is
    /// unlabelled and longer than one dash.
    fn fan_relations(&self) -> Vec<(String, Vec<usize>)> {
        let mut by_left: Vec<(String, Vec<usize>)> = vec![];
        for (index, relation) in self.relations.iter().enumerate() {
            let Some(slot) = by_left.iter_mut().find(|(name, _)| *name == relation.left)
            else {
                by_left.push((relation.left.clone(), vec![index]));
                continue;
            };
            slot.1.push(index);
        }
        by_left
            .into_iter()
            .filter(|(_, outgoing)| {
                outgoing.len() > 2
                    && outgoing.iter().all(|index| {
                        let relation = &self.relations[*index];
                        relation.direction.is_none() && relation.length > 1.0
                    })
            })
            .collect()
    }

    /// Ruby `constrained_sizes`: fan-out parents span the compact row
    /// occupied by their targets.
    pub fn constrained_sizes(
        &self,
        sizes: &HashMap<String, [f64; 2]>,
    ) -> HashMap<String, [f64; 2]> {
        let mut constrained = sizes.clone();
        for (parent, outgoing) in self.fan_relations() {
            let child_width: f64 = crate::render::pie::fsum(
                outgoing.iter().map(|index| constrained[&self.relations[*index].right][0]),
            );
            let row_width =
                child_width + FAN_HORIZONTAL_GAP * (outgoing.len().saturating_sub(1)) as f64;
            let slot = constrained.get_mut(&parent).unwrap();
            slot[0] = slot[0].max(row_width + FAN_PARENT_ALLOWANCE);
        }
        constrained
    }

    /// Ruby `place`: top-left corners.
    pub fn place(
        &self,
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
    ) -> HashMap<String, [f64; 2]> {
        let mut edge = origin[0];
        let mut corners = HashMap::new();
        for component in self.components() {
            let (logical, order) = self.logical_positions(&component);
            let logical = self.spread_collisions(logical, &order);
            let (placed, width) = self.place_component(&component, &logical, sizes, [edge, origin[1]]);
            corners.extend(placed);
            edge += width + COMPONENT_GAP;
        }
        corners
    }

    fn components(&self) -> Vec<Vec<String>> {
        let mut unseen: Vec<String> = self.names.clone();
        let mut found = vec![];
        for name in &self.names {
            if !unseen.contains(name) {
                continue;
            }
            let component = self.reachable(name);
            found.push(component.clone());
            for member in &component {
                if let Some(position) = unseen.iter().position(|own| own == member) {
                    unseen.remove(position);
                }
            }
        }
        found
    }

    fn reachable(&self, root: &str) -> Vec<String> {
        let mut seen = vec![];
        let mut queue = vec![root.to_string()];
        while let Some(name) = queue.first().cloned() {
            queue.remove(0);
            if seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            queue.extend(self.neighbours(&name));
        }
        seen
    }

    fn neighbours(&self, name: &str) -> Vec<String> {
        let mut out = vec![];
        for relation in &self.relations {
            if relation.left == name {
                out.push(relation.right.clone());
            }
        }
        for relation in &self.relations {
            if relation.right == name {
                out.push(relation.left.clone());
            }
        }
        out
    }

    /// Ruby's Hash preserves insertion order; Rust's HashMap does not, so
    /// the visit order travels alongside the map for the collision spread.
    fn logical_positions(&self, component: &[String]) -> (HashMap<String, [f64; 2]>, Vec<String>) {
        let mut positions = HashMap::new();
        let mut order = vec![component[0].clone()];
        positions.insert(component[0].clone(), [0.0, 0.0]);
        let mut queue = vec![component[0].clone()];
        while let Some(name) = queue.first().cloned() {
            queue.remove(0);
            for (other, delta) in self.connected(&name) {
                if positions.contains_key(&other) {
                    continue;
                }
                let here = positions[&name];
                positions.insert(other.clone(), [here[0] + delta[0], here[1] + delta[1]]);
                order.push(other.clone());
                queue.push(other);
            }
        }
        (positions, order)
    }

    fn connected(&self, name: &str) -> Vec<(String, [f64; 2])> {
        let mut out = vec![];
        for relation in &self.relations {
            let delta = self.relation_delta(relation);
            if relation.left == name {
                out.push((relation.right.clone(), delta));
            } else if relation.right == name {
                out.push((relation.left.clone(), [-delta[0], -delta[1]]));
            }
        }
        out
    }

    fn relation_delta(&self, relation: &Relation) -> [f64; 2] {
        match relation.direction {
            Some(Direction::Up) => [0.0, -1.0],
            Some(Direction::Down) => [0.0, 1.0],
            Some(Direction::Left) => [-1.0, 0.0],
            Some(Direction::Right) => [1.0, 0.0],
            None => {
                if relation.length == 1.0 {
                    return [1.0, 0.0];
                }
                [0.0, (relation.length - 1.0).max(1.0)]
            }
        }
    }

    fn spread_collisions(
        &self,
        mut positions: HashMap<String, [f64; 2]>,
        order: &[String],
    ) -> HashMap<String, [f64; 2]> {
        // Ruby groups by the point arrays themselves, in insertion order.
        let mut groups: Vec<Vec<String>> = vec![];
        for key in order {
            let point = positions[key];
            let mut found = false;
            for group in groups.iter_mut() {
                if positions[&group[0]] == point {
                    group.push(key.clone());
                    found = true;
                    break;
                }
            }
            if !found {
                groups.push(vec![key.clone()]);
            }
        }
        for group in groups.into_iter().filter(|group| group.len() > 1) {
            let middle = (group.len() - 1) as f64 / 2.0;
            for (index, name) in group.iter().enumerate() {
                let point = positions[name];
                positions.insert(name.clone(), [point[0] + index as f64 - middle, point[1]]);
            }
        }
        positions
    }

    fn place_component(
        &self,
        component: &[String],
        logical: &HashMap<String, [f64; 2]>,
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
    ) -> (HashMap<String, [f64; 2]>, f64) {
        let columns = self.axis_layout(component, logical, sizes, 0, origin[0]);
        let rows = self.axis_layout(component, logical, sizes, 1, origin[1]);
        let mut corners = HashMap::new();
        for name in component {
            corners.insert(
                name.clone(),
                [
                    self.centred(name, logical, sizes, &columns, 0),
                    self.centred(name, logical, sizes, &rows, 1),
                ],
            );
        }
        let mut extent = self.axis_extent(&columns, origin[0]);
        if self.fan_component(component) {
            let inset = FAN_PARENT_ALLOWANCE / 2.0;
            for corner in corners.values_mut() {
                corner[0] += inset;
            }
            extent += FAN_PARENT_ALLOWANCE;
        }
        (corners, extent)
    }

    fn axis_layout(
        &self,
        component: &[String],
        logical: &HashMap<String, [f64; 2]>,
        sizes: &HashMap<String, [f64; 2]>,
        axis: usize,
        origin: f64,
    ) -> AxisLayoutInfo {
        let mut coordinates: Vec<f64> =
            component.iter().map(|name| logical[name][axis]).collect();
        coordinates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        coordinates.dedup();
        let mut layout = AxisLayoutInfo { placements: vec![], cursor: origin };
        for coordinate in &coordinates {
            let gap = self.axis_gap(component, axis, *coordinate, &coordinates, logical);
            let span = self.axis_span(component, logical, sizes, axis, *coordinate);
            layout.placements.push((*coordinate, layout.cursor, span));
            layout.cursor += span + gap;
        }
        layout
    }

    fn axis_gap(
        &self,
        component: &[String],
        axis: usize,
        coordinate: f64,
        coordinates: &[f64],
        logical: &HashMap<String, [f64; 2]>,
    ) -> f64 {
        if coordinate == *coordinates.last().unwrap() {
            return 0.0;
        }
        if axis == 0 {
            return self.horizontal_gap(component);
        }
        self.vertical_gap(component, coordinate, coordinates, logical)
    }

    fn horizontal_gap(&self, component: &[String]) -> f64 {
        if self.compact() {
            return COMPACT_HORIZONTAL_GAP;
        }
        if self.mixed_kind() {
            return MIXED_HORIZONTAL_GAP;
        }
        if self.fan_component(component) {
            return FAN_HORIZONTAL_GAP;
        }
        RADIAL_HORIZONTAL_GAP
    }

    fn vertical_gap(
        &self,
        component: &[String],
        coordinate: f64,
        coordinates: &[f64],
        logical: &HashMap<String, [f64; 2]>,
    ) -> f64 {
        if self.compact() {
            return COMPACT_VERTICAL_GAP;
        }
        if !self.mixed_kind() {
            return STANDARD_VERTICAL_GAP;
        }
        let first = coordinates[0];
        if coordinate == first && self.long_qualified_source(component, logical) {
            return MIXED_LONG_VERTICAL_GAP;
        }
        MIXED_NEAR_VERTICAL_GAP
    }

    fn fan_component(&self, component: &[String]) -> bool {
        self.fan_relations()
            .iter()
            .any(|(parent, _)| component.contains(parent))
    }

    fn compact(&self) -> bool {
        self.directives.iter().any(|line| line == "hide empty members")
    }

    fn mixed_kind(&self) -> bool {
        self.classes.classes.iter().any(|klass| klass.kind != super::model::Kind::Class)
    }

    fn long_qualified_source(
        &self,
        component: &[String],
        logical: &HashMap<String, [f64; 2]>,
    ) -> bool {
        let top = component
            .iter()
            .min_by(|a, b| {
                logical[a.as_str()][1]
                    .partial_cmp(&logical[b.as_str()][1])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();
        let Some(top) = top else { return false };
        let top = top;
        self.relations.iter().any(|relation| {
            relation.left == top && relation.length > 2.0 && qualifier(relation)
        })
    }

    fn axis_span(
        &self,
        component: &[String],
        logical: &HashMap<String, [f64; 2]>,
        sizes: &HashMap<String, [f64; 2]>,
        axis: usize,
        coordinate: f64,
    ) -> f64 {
        let mut names: Vec<&String> = component
            .iter()
            .filter(|name| logical[name.as_str()][axis] == coordinate)
            .collect();
        if axis == 0 && names.len() > 1 {
            if let Some(parent) = self.fan_parent(component) {
                names.retain(|name| *name != &parent);
            }
        }
        names
            .iter()
            .filter(|name| logical[name.as_str()][axis] == coordinate)
            .map(|name| sizes[*name][axis])
            .fold(0.0_f64, f64::max)
    }

    fn fan_parent(&self, component: &[String]) -> Option<String> {
        self.fan_relations()
            .iter()
            .find(|(parent, _)| component.contains(parent))
            .map(|(parent, _)| parent.clone())
    }

    fn centred(
        &self,
        name: &str,
        logical: &HashMap<String, [f64; 2]>,
        sizes: &HashMap<String, [f64; 2]>,
        layout: &AxisLayoutInfo,
        axis: usize,
    ) -> f64 {
        let coordinate = logical[name][axis];
        let Some((_, left, span)) =
            layout.placements.iter().find(|(at, _, _)| *at == coordinate)
        else {
            return 0.0;
        };
        left + (span - sizes[name][axis]) / 2.0
    }
}

impl QualifiedAssociationLayout {
    fn axis_extent(&self, layout: &AxisLayoutInfo, origin: f64) -> f64 {
        layout.cursor - origin
    }
}

struct AxisLayoutInfo {
    placements: Vec<(f64, f64, f64)>,
    cursor: f64,
}
