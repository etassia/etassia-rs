//! Port of Ruby `Notation::PlantUML::RankHierarchy` (`rank_hierarchy.rb`):
//! classes joined by relations in a tree or a lattice, ranked the way
//! PlantUML's hierarchical layout ranks them.
use super::model::{Diagram, Direction, Relation};
use std::collections::HashMap;

const RANK_GAP: f64 = 60.0;
const NODE_GAP: f64 = 35.0;
const SAME_RANK_GAP: f64 = 50.0;
const BLOCK_GAP: f64 = 35.0;
const LABEL_ROW: f64 = 15.0;
const LABEL_PAD: f64 = 2.0;
const LOOSE_COLUMNS: usize = 2;

fn level(relation: &Relation) -> bool {
    matches!(relation.direction, Some(Direction::Left) | Some(Direction::Right))
        || (relation.direction.is_none() && relation.length == 1.0)
}

fn pair(relation: &Relation) -> (String, String) {
    if relation.direction == Some(Direction::Up) {
        (relation.right.clone(), relation.left.clone())
    } else {
        (relation.left.clone(), relation.right.clone())
    }
}

fn drawable(relation: &Relation, names: &[String]) -> bool {
    if relation.layout_only {
        return false;
    }
    let ends = [&relation.left, &relation.right];
    if ends[0] == ends[1] {
        return false;
    }
    ends.iter().all(|name| names.iter().any(|own| own == *name))
}

fn fits(diagram: &Diagram) -> bool {
    let names: Vec<String> = diagram.classes.iter().map(|k| k.name.clone()).collect();
    diagram.packages.is_empty()
        && diagram.junctions.is_empty()
        && !diagram.relations.is_empty()
        && diagram.relations.iter().all(|r| drawable(r, &names))
}

/// Ruby `rank_of`: each related class's rank.
fn rank_of(diagram: &Diagram) -> Option<HashMap<String, usize>> {
    let related: Vec<String> = diagram
        .relations
        .iter()
        .flat_map(|r| [r.left.clone(), r.right.clone()])
        .fold(vec![], |mut out, name| {
            if !out.contains(&name) {
                out.push(name);
            }
            out
        });
    let groups = same_rank_groups(diagram, &related);
    let levels = group_levels(diagram, &groups)?;
    let mut ranks = HashMap::new();
    for name in &related {
        let group = &groups[name];
        ranks.insert(name.clone(), levels[group]);
    }
    Some(ranks)
}

/// Union of the classes a one-dash or sideways relation keeps level.
fn same_rank_groups(diagram: &Diagram, names: &[String]) -> HashMap<String, String> {
    let mut groups: HashMap<String, String> =
        names.iter().map(|name| (name.clone(), name.clone())).collect();
    for relation in diagram.relations.iter().filter(|r| level(r)) {
        let keep = groups[&relation.left].clone();
        let drop = groups[&relation.right].clone();
        for (_, group) in groups.iter_mut() {
            if *group == drop {
                *group = keep.clone();
            }
        }
    }
    groups
}

fn group_levels(
    diagram: &Diagram,
    groups: &HashMap<String, String>,
) -> Option<HashMap<String, usize>> {
    let mut unique: Vec<String> = groups.values().cloned().collect();
    unique.sort();
    unique.dedup();
    let deeper: Vec<[String; 2]> = diagram
        .relations
        .iter()
        .filter(|r| !level(r))
        .map(|r| {
            let (top, bottom) = (groups[&r.left].clone(), groups[&r.right].clone());
            if r.direction == Some(Direction::Up) {
                [bottom, top]
            } else {
                [top, bottom]
            }
        })
        .collect();
    longest_paths(unique, deeper)
}

/// Rank by longest path; `None` when the relations loop.
fn longest_paths(
    groups: Vec<String>,
    deeper: Vec<[String; 2]>,
) -> Option<HashMap<String, usize>> {
    let mut levels: HashMap<String, usize> =
        groups.iter().map(|group| (group.clone(), 0)).collect();
    for _ in 0..groups.len() {
        if !lowered(&mut levels, &deeper) {
            return Some(levels);
        }
    }
    None
}

fn lowered(levels: &mut HashMap<String, usize>, deeper: &[[String; 2]]) -> bool {
    let mut any = false;
    for [upper, lower] in deeper {
        if levels[lower] > levels[upper] {
            continue;
        }
        levels.insert(lower.clone(), levels[upper] + 1);
        any = true;
    }
    any
}

#[derive(Clone, Copy)]
struct Top {
    at: f64,
    height: f64,
}

pub struct RankHierarchy {
    diagram: Diagram,
    ranks: HashMap<String, usize>,
    order: Vec<String>,
}

impl RankHierarchy {
    /// Ruby `RankHierarchy.for`.
    pub fn for_diagram(diagram: &Diagram) -> Option<RankHierarchy> {
        if !fits(diagram) {
            return None;
        }
        let ranks = rank_of(diagram)?;
        let mut order: Vec<String> = ranks.keys().cloned().collect();
        let declared: Vec<String> = diagram.classes.iter().map(|k| k.name.clone()).collect();
        order.sort_by_key(|name| declared.iter().position(|own| own == name).unwrap_or(usize::MAX));
        Some(RankHierarchy { diagram: diagram.clone(), ranks, order })
    }

    /// Ruby `place`: each class's top-left corner.
    pub fn place(&self, sizes: &HashMap<String, [f64; 2]>, origin: [f64; 2]) -> HashMap<String, [f64; 2]> {
        let rows = self.rows_in_order();
        let loose = self.loose_rows();
        let tops = self.rank_tops(&rows, &loose, sizes, origin[1]);
        let mut corners = self.ranked_corners(&rows, sizes, origin[0], &tops);
        corners.extend(self.loose_corners(&loose, sizes, &corners, &tops));
        corners
    }

    fn declared(&self) -> Vec<String> {
        self.diagram.classes.iter().map(|k| k.name.clone()).collect()
    }

    fn neighbours(&self, name: &str, upper: bool) -> Vec<String> {
        self.diagram
            .relations
            .iter()
            .filter(|r| !level(r))
            .filter_map(|relation| {
                let (top, bottom) = pair(relation);
                if upper {
                    (bottom == name).then_some(top)
                } else {
                    (top == name).then_some(bottom)
                }
            })
            .collect()
    }

    /// Ruby `rows_in_order`.
    fn rows_in_order(&self) -> Vec<Vec<String>> {
        let depth = *self.ranks.values().max().unwrap_or(&0);
        let mut rows: Vec<Vec<String>> = vec![vec![]; depth + 1];
        for name in &self.order {
            rows[self.ranks[name]].push(name.clone());
        }
        let mut placed: Vec<Vec<String>> = vec![];
        for (index, row) in rows.into_iter().enumerate() {
            let above = if index == 0 { &[][..] } else { &placed[index - 1][..] };
            placed.push(self.sorted_row(row, above, index));
        }
        placed
    }

    fn sorted_row(&self, row: Vec<String>, above: &[String], index: usize) -> Vec<String> {
        let mut keyed: Vec<(usize, String)> =
            row.into_iter().enumerate().map(|(position, name)| (position, name)).collect();
        keyed.sort_by(|(position_a, name_a), (position_b, name_b)| {
            let mean_a = if index == 0 {
                0.0
            } else {
                self.mean_position(name_a, above)
            };
            let mean_b = if index == 0 { 0.0 } else { self.mean_position(name_b, above) };
            mean_a.partial_cmp(&mean_b).unwrap_or(std::cmp::Ordering::Equal)
                .then(position_a.cmp(position_b))
        });
        let ordered: Vec<String> = keyed.into_iter().map(|(_, name)| name).collect();
        self.honour_level_order(ordered)
    }

    fn mean_position(&self, name: &str, above: &[String]) -> f64 {
        let parents: Vec<Option<usize>> = self
            .neighbours(name, true)
            .into_iter()
            .map(|parent| above.iter().position(|own| own == &parent))
            .collect();
        let found: Vec<usize> = parents.into_iter().flatten().collect();
        if found.is_empty() {
            above.len() as f64
        } else {
            crate::render::pie::fsum(found.iter().map(|p| *p as f64)) / found.len() as f64
        }
    }

    /// Ruby `honour_level_order`: a one-dash relation puts its
    /// written-first class on the left.
    fn honour_level_order(&self, row: Vec<String>) -> Vec<String> {
        let constraints: Vec<&Relation> = self
            .diagram
            .relations
            .iter()
            .filter(|r| level(r) && row.contains(&r.left))
            .collect();
        let mut ordered = row;
        for relation in constraints {
            let ends = if relation.direction == Some(Direction::Left) {
                [&relation.right, &relation.left]
            } else {
                [&relation.left, &relation.right]
            };
            let left = ends[0];
            let right = ends[1];
            let left_at = ordered.iter().position(|name| name == left);
            let right_at = ordered.iter().position(|name| name == right);
            if let (Some(left_at), Some(right_at)) = (left_at, right_at) {
                if left_at > right_at {
                    let left = ordered.remove(left_at);
                    let right_at = ordered.iter().position(|name| name == right).unwrap_or(0);
                    ordered.insert(right_at, left);
                }
            }
        }
        ordered
    }

    /// Ruby `centred_lefts`.
    fn centred_lefts(&self, rows: &[Vec<String>], sizes: &HashMap<String, [f64; 2]>) -> HashMap<String, f64> {
        let mut lefts = HashMap::new();
        for row in rows {
            self.pack(row, sizes, &mut lefts);
        }
        for row in rows.iter().skip(1) {
            self.pull(row, sizes, &mut lefts, true);
        }
        for row in rows.iter().rev() {
            self.pull(row, sizes, &mut lefts, false);
        }
        lefts
    }

    fn gap_after(&self, name: &str, row: &[String]) -> f64 {
        let level = self.diagram.relations.iter().any(|relation| {
            level(relation) && relation.left == name && row.contains(&relation.right)
        });
        if level { SAME_RANK_GAP } else { NODE_GAP }
    }

    fn pack(&self, row: &[String], sizes: &HashMap<String, [f64; 2]>, lefts: &mut HashMap<String, f64>) {
        let mut cursor = 0.0;
        for name in row {
            lefts.insert(name.clone(), cursor);
            cursor += sizes[name][0] + self.gap_after(name, row);
        }
    }

    fn pull(&self, row: &[String], sizes: &HashMap<String, [f64; 2]>, lefts: &mut HashMap<String, f64>, upper: bool) {
        for (index, name) in row.iter().enumerate() {
            let Some(wanted) = self.wanted_centre(name, sizes, lefts, upper) else { continue };
            let floor = self.floor_for(row, index, sizes, lefts);
            let pulled = (wanted - sizes[name][0] / 2.0).max(floor);
            lefts.insert(name.clone(), pulled);
        }
    }

    fn floor_for(&self, row: &[String], index: usize, sizes: &HashMap<String, [f64; 2]>, lefts: &HashMap<String, f64>) -> f64 {
        if index == 0 {
            return f64::NEG_INFINITY;
        }
        let before = &row[index - 1];
        lefts[before] + sizes[before][0] + self.gap_after(before, row)
    }

    fn wanted_centre(&self, name: &str, sizes: &HashMap<String, [f64; 2]>, lefts: &HashMap<String, f64>, upper: bool) -> Option<f64> {
        let others = self.neighbours(name, upper);
        if others.is_empty() {
            return None;
        }
        let total = crate::render::pie::fsum(
            others
                .iter()
                .map(|other| lefts[other] + sizes[other][0] / 2.0),
        );
        Some(total / others.len() as f64)
    }

    fn ranked_corners(
        &self,
        rows: &[Vec<String>],
        sizes: &HashMap<String, [f64; 2]>,
        left: f64,
        tops: &[Top],
    ) -> HashMap<String, [f64; 2]> {
        let lefts = self.centred_lefts(rows, sizes);
        let shift = left - lefts.values().cloned().fold(f64::INFINITY, f64::min);
        let mut corners = HashMap::new();
        for (rank, row) in rows.iter().enumerate() {
            for name in row {
                corners.insert(
                    name.clone(),
                    [
                        lefts[name] + shift,
                        self.centred_top(tops[rank], sizes[name][1]),
                    ],
                );
            }
        }
        corners
    }

    fn rank_tops(&self, rows: &[Vec<String>], loose: &[Vec<String>], sizes: &HashMap<String, [f64; 2]>, start: f64) -> Vec<Top> {
        let mut cursor = start;
        let count = rows.len().max(loose.len());
        let mut tops = vec![];
        for rank in 0..count {
            let height_rows = self.row_height(rows.get(rank).map(|r| &r[..]).unwrap_or(&[]), sizes);
            let height_loose = self.row_height(loose.get(rank).map(|r| &r[..]).unwrap_or(&[]), sizes);
            let height = height_rows.max(height_loose);
            tops.push(Top { at: cursor, height });
            cursor += height + RANK_GAP + self.label_room(rows, rank);
        }
        tops
    }

    fn centred_top(&self, rank: Top, height: f64) -> f64 {
        rank.at + (rank.height - height) / 2.0
    }

    fn label_room(&self, rows: &[Vec<String>], rank: usize) -> f64 {
        let Some(below) = rows.get(rank + 1) else {
            return 0.0;
        };
        let lines = self
            .diagram
            .relations
            .iter()
            .filter_map(|relation| self.label_lines(relation, &rows[rank], below))
            .max();
        lines.map(|lines| LABEL_PAD + lines as f64 * LABEL_ROW).unwrap_or(0.0)
    }

    fn label_lines(&self, relation: &Relation, row: &[String], below: &[String]) -> Option<usize> {
        if level(relation) || relation.label.as_deref().unwrap_or("").is_empty() {
            return None;
        }
        let (top, bottom) = pair(relation);
        if !row.contains(&top) || !below.contains(&bottom) {
            return None;
        }
        Some(relation.label.as_deref().unwrap_or("").split("\\n").count())
    }

    fn row_height(&self, row: &[String], sizes: &HashMap<String, [f64; 2]>) -> f64 {
        row.iter().map(|name| sizes[name][1]).fold(0.0_f64, f64::max)
    }

    fn loose_rows(&self) -> Vec<Vec<String>> {
        let declared = self.declared();
        let loose: Vec<String> = declared
            .into_iter()
            .filter(|name| !self.ranks.contains_key(name))
            .collect();
        loose.chunks(LOOSE_COLUMNS).map(|chunk| chunk.to_vec()).collect()
    }

    fn loose_corners(
        &self,
        loose: &[Vec<String>],
        sizes: &HashMap<String, [f64; 2]>,
        corners: &HashMap<String, [f64; 2]>,
        tops: &[Top],
    ) -> HashMap<String, [f64; 2]> {
        if loose.is_empty() {
            return HashMap::new();
        }
        let left = self.loose_left(loose, sizes, corners);
        let widths: Vec<f64> = loose.iter().map(|row| sizes[&row[0]][0]).collect();
        let widest = widths.iter().cloned().fold(0.0_f64, f64::max);
        let starts: Vec<f64> = widths.iter().map(|width| left + (widest - width) / 2.0).collect();
        let mut out = HashMap::new();
        for (rank, row) in loose.iter().enumerate() {
            let mut x = starts[rank];
            for name in row {
                let (width, height) = (sizes[name][0], sizes[name][1]);
                out.insert(name.clone(), [x, self.centred_top(tops[rank], height)]);
                x += width + NODE_GAP;
            }
        }
        out
    }

    fn loose_left(&self, loose: &[Vec<String>], sizes: &HashMap<String, [f64; 2]>, corners: &HashMap<String, [f64; 2]>) -> f64 {
        let mut right = 0.0_f64;
        for row in loose {
            for name in row {
                if self.ranks.get(name).copied().unwrap_or(usize::MAX) < loose.len() {
                    if let Some(corner) = corners.get(name) {
                        right = right.max(corner[0] + sizes[name][0]);
                    }
                }
            }
        }
        right + BLOCK_GAP
    }
}
