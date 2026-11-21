//! Port of Ruby `Notation::PlantUML::PackageFrames` (`package_frames.rb`):
//! the Scene frame around each package's positioned boxes.
use super::model::{Diagram, PackageShape};
use std::collections::HashMap;
use super::scene::{Frame, Text};

pub const OPEN: f64 = 40.0;
pub const CLOSE: f64 = 16.0;
pub const SIDE: f64 = 16.0;
const NESTING_STEP: f64 = 8.0;
const NESTED_OPEN: f64 = 35.0;
const RELATION_INSET: f64 = 8.0;
const TAB_HEIGHT: f64 = 22.0;
const ICON_WIDTH: f64 = 14.0;

/// Ruby `relation_layout?`: exactly one relation touches a package.
pub fn relation_layout(diagram: &Diagram) -> bool {
    let package_ids: Vec<&String> = diagram.packages.iter().map(|p| &p.id).collect();
    let count = diagram
        .relations
        .iter()
        .filter(|relation| {
            package_ids.contains(&&relation.left) || package_ids.contains(&&relation.right)
        })
        .count();
    count == 1
}

/// Ruby `stack_space`: `depth.times.sum { base + level * NESTING_STEP }`,
/// a compensated sum as Ruby's `sum` is.
pub fn stack_space(depth: usize, base: f64) -> f64 {
    let terms = (0..depth).map(|level| base + level as f64 * NESTING_STEP);
    crate::render::pie::fsum(terms)
}

pub fn stack_open(depth: usize) -> f64 {
    stack_space(depth, NESTED_OPEN)
}

pub fn stack_close(depth: usize) -> f64 {
    stack_space(depth, CLOSE)
}

pub fn stack_side(depth: usize) -> f64 {
    stack_space(depth, SIDE)
}

pub struct PackageFrames<'a> {
    diagram: &'a Diagram,
    measure: &'a dyn Fn(&str) -> f64,
}

impl<'a> PackageFrames<'a> {
    pub fn new(diagram: &'a Diagram, measure: &'a dyn Fn(&str) -> f64) -> PackageFrames<'a> {
        PackageFrames { diagram, measure }
    }

    /// Ruby `call`: outer packages before the ones inside them.
    pub fn call<'b>(&self, boxes: &'b [super::scene::Box]) -> Vec<Frame> {
        let nested = relation_layout(self.diagram);
        let mut cache: HashMap<String, [f64; 4]> = HashMap::new();
        let mut frames = vec![];
        for package in &self.diagram.packages {
            let own: Vec<[f64; 4]> = {
                let names: Vec<&String> = self
                    .diagram
                    .classes
                    .iter()
                    .filter(|klass| klass.package.as_ref() == Some(&package.id))
                    .map(|klass| &klass.name)
                    .collect();
                boxes
                    .iter()
                    .filter(|box_| names.iter().any(|name| *name == &box_.id))
                    .map(|box_| [box_.x, box_.y, box_.x + box_.width, box_.y + box_.height])
                    .collect()
            };
            let inner: Vec<[f64; 4]> = self
                .diagram
                .packages
                .iter()
                .filter(|p| p.parent.as_ref() == Some(&package.id))
                .map(|p| self.bounds_of(p, boxes, &mut cache))
                .collect();
            let mut rectangles = own;
            rectangles.extend(inner);
            let (side, above, below) = if nested {
                let increment = self.descendant_depth(&package.id) as f64 * NESTING_STEP;
                (SIDE + increment, NESTED_OPEN + increment, CLOSE + increment)
            } else {
                (SIDE, OPEN - 6.0, CLOSE - 4.0)
            };
            let mut left = f64::INFINITY;
            let mut top = f64::INFINITY;
            let mut right = f64::NEG_INFINITY;
            let mut bottom = f64::NEG_INFINITY;
            for rect in &rectangles {
                left = left.min(rect[0]);
                top = top.min(rect[1]);
                right = right.max(rect[2]);
                bottom = bottom.max(rect[3]);
            }
            let mut left = left - side;
            let mut right = right + side;
            let top = top - above;
            let bottom = bottom + below;
            // relation_edges: room on each side for an external target's
            // centre.
            let mut targets = vec![];
            for relation in &self.diagram.relations {
                let name = if relation.left == package.id {
                    Some(&relation.right)
                } else if relation.right == package.id {
                    Some(&relation.left)
                } else {
                    None
                };
                if let Some(name) = name {
                    if let Some(box_) = boxes.iter().find(|box_| &box_.id == name) {
                        targets.push(box_);
                    }
                }
            }
            for target in &targets {
                let centre = target.x + target.width / 2.0;
                left = left.min(centre - RELATION_INSET);
                right = right.max(centre + RELATION_INSET);
            }
            let width = right - left;
            let height = bottom - top;
            let title = Text::new(
                &package.title,
                left + 10.0 + if package.icon { ICON_WIDTH } else { 0.0 },
                top + 16.0,
                "package_title",
                "start",
            );
            let folder = package.shape == PackageShape::Folder;
            frames.push(Frame {
                id: format!("package-{}", package.id),
                x: left,
                y: top,
                width,
                height,
                icon: package.icon,
                texts: vec![title],
                tab_width: if folder {
                    Some((self.measure)(&package.title) + 20.0 + if package.icon { ICON_WIDTH } else { 0.0 })
                } else {
                    None
                },
                fill: package.color.clone(),
            });
            cache.insert(package.id.clone(), [left, top, right, bottom]);
        }
        frames
    }

    /// Ruby `bounds_of`: memoized, recursing into the inner packages.
    fn bounds_of(
        &self,
        package: &super::model::Package,
        boxes: &[super::scene::Box],
        cache: &mut HashMap<String, [f64; 4]>,
    ) -> [f64; 4] {
        if let Some(found) = cache.get(&package.id) {
            return *found;
        }
        let own: Vec<[f64; 4]> = {
            let names: Vec<&String> = self
                .diagram
                .classes
                .iter()
                .filter(|klass| klass.package.as_ref() == Some(&package.id))
                .map(|klass| &klass.name)
                .collect();
            boxes
                .iter()
                .filter(|box_| names.iter().any(|name| *name == &box_.id))
                .map(|box_| [box_.x, box_.y, box_.x + box_.width, box_.y + box_.height])
                .collect()
        };
        let inner: Vec<[f64; 4]> = self
            .diagram
            .packages
            .iter()
            .filter(|p| p.parent.as_ref() == Some(&package.id))
            .map(|p| self.bounds_of(p, boxes, cache))
            .collect();
        let mut rectangles = own;
        rectangles.extend(inner);
        let (side, above, below) = if relation_layout(self.diagram) {
            let increment = self.descendant_depth(&package.id) as f64 * NESTING_STEP;
            (SIDE + increment, NESTED_OPEN + increment, CLOSE + increment)
        } else {
            (SIDE, OPEN - 6.0, CLOSE - 4.0)
        };
        let mut left = f64::INFINITY;
        let mut top = f64::INFINITY;
        let mut right = f64::NEG_INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for rect in &rectangles {
            left = left.min(rect[0]);
            top = top.min(rect[1]);
            right = right.max(rect[2]);
            bottom = bottom.max(rect[3]);
        }
        let mut left = left - side;
        let mut right = right + side;
        let top = top - above;
        let bottom = bottom + below;
        let mut targets = vec![];
        for relation in &self.diagram.relations {
            let name = if relation.left == package.id {
                Some(&relation.right)
            } else if relation.right == package.id {
                Some(&relation.left)
            } else {
                None
            };
            if let Some(name) = name {
                if let Some(box_) = boxes.iter().find(|box_| &box_.id == name) {
                    targets.push(box_);
                }
            }
        }
        for target in &targets {
            let centre = target.x + target.width / 2.0;
            left = left.min(centre - RELATION_INSET);
            right = right.max(centre + RELATION_INSET);
        }
        let bounds = [left, top, right, bottom];
        cache.insert(package.id.clone(), bounds);
        bounds
    }

    fn descendant_depth(&self, package_id: &str) -> usize {
        let children: Vec<&super::model::Package> = self
            .diagram
            .packages
            .iter()
            .filter(|item| item.parent.as_deref() == Some(package_id))
            .collect();
        if children.is_empty() {
            return 0;
        }
        1 + children
            .iter()
            .map(|child| self.descendant_depth(&child.id))
            .max()
            .unwrap_or(0)
    }
}
