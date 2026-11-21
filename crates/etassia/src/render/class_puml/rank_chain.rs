//! Port of Ruby `Notation::PlantUML::RankChain` (`rank_chain.rb`): classes
//! joined by unbranched chains of relations, ranked the way PlantUML's
//! layout engines rank them.
use super::model::{Diagram, Direction, Relation};
use std::collections::HashMap;

const JUNCTION_SIZE: [f64; 2] = [16.0, 24.0];
const VIRTUAL_NODE: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    Smetana,
    Dot,
}

impl Engine {
    fn rank_gap(&self) -> f64 {
        match self {
            Engine::Smetana => 36.0,
            Engine::Dot => 60.0,
        }
    }

    fn node_gap(&self) -> f64 {
        match self {
            Engine::Smetana => 18.0,
            Engine::Dot => 35.0,
        }
    }

    fn label_margin(&self) -> f64 {
        match self {
            Engine::Smetana => 4.0,
            Engine::Dot => 2.0,
        }
    }
}

/// Ruby `RankChain.engine`: the engine PlantUML lays out with.
pub fn engine(diagram: &Diagram) -> Engine {
    static SMETANA: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let smetana = SMETANA.get_or_init(|| {
        regex::Regex::new(r"(?i)\A!pragma[ \t]+layout[ \t]+smetana\z").unwrap()
    });
    if diagram.directives.iter().any(|line| smetana.is_match(line)) {
        Engine::Smetana
    } else {
        Engine::Dot
    }
}

fn left_to_right(diagram: &Diagram) -> bool {
    diagram
        .directives
        .iter()
        .any(|line| line == "left to right direction")
}

/// The layout facts the placement reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Vertical,
    Horizontal,
}

fn relation_axis(relation: &Relation, diagram: &Diagram) -> Axis {
    let mut across =
        matches!(relation.direction, Some(Direction::Left) | Some(Direction::Right));
    if left_to_right(diagram) {
        across = !across;
    }
    if across { Axis::Horizontal } else { Axis::Vertical }
}

fn rank_axis(diagram: &Diagram) -> Axis {
    if left_to_right(diagram) { Axis::Horizontal } else { Axis::Vertical }
}

/// The relations in written order, cut into chains wherever one does not
/// start at the class the last ended on; `None` when a class is on two
/// chains or twice on one.
fn chains_of(diagram: &Diagram) -> Option<Vec<Vec<usize>>> {
    if !diagram.packages.is_empty() || !diagram.notes.is_empty() || diagram.relations.is_empty() {
        return None;
    }
    let mut chains: Vec<Vec<usize>> = vec![];
    let mut current: Vec<usize> = vec![];
    for (index, relation) in diagram.relations.iter().enumerate() {
        if let Some(last) = current.last() {
            if diagram.relations[*last].right != relation.left {
                chains.push(std::mem::take(&mut current));
            }
        }
        current.push(index);
    }
    if !current.is_empty() {
        chains.push(current);
    }
    let names: Vec<String> = chains.iter().flat_map(|chain| names_of(chain, diagram)).collect();
    let mut seen: Vec<String> = vec![];
    for name in &names {
        if seen.contains(name) {
            return None;
        }
        seen.push(name.clone());
    }
    Some(chains)
}

/// Ruby `names_of`: a chain's classes in order.
fn names_of(chain: &[usize], diagram: &Diagram) -> Vec<String> {
    let mut out = vec![diagram.relations[chain[0]].left.clone()];
    for index in chain {
        out.push(diagram.relations[*index].right.clone());
    }
    out
}

fn exact_classes(diagram: &Diagram, names: &[String], owners: &[String]) -> bool {
    let mut declared: Vec<String> =
        diagram.classes.iter().map(|klass| klass.name.clone()).collect();
    let mut expected: Vec<String> = names.to_vec();
    expected.extend(owners.iter().cloned());
    declared.sort();
    expected.sort();
    declared == expected
}

fn sort_pair(a: &str, b: &str) -> [String; 2] {
    let mut pair = [a.to_string(), b.to_string()];
    pair.sort();
    pair
}

fn junction_owners(diagram: &Diagram) -> Option<Vec<(usize, String)>> {
    let relations = &diagram.relations;
    let mut owners = vec![];
    for junction in &diagram.junctions {
        let pair = sort_pair(&junction.from, &junction.to);
        let index = relations
            .iter()
            .position(|r| sort_pair(&r.left, &r.right) == pair)?;
        owners.push((index, junction.owner.clone()));
    }
    let names: Vec<String> = relations
        .iter()
        .flat_map(|r| [r.left.clone(), r.right.clone()])
        .collect();
    let mut seen: Vec<String> = vec![];
    for (_, owner) in &owners {
        if seen.contains(owner) || names.contains(owner) {
            return None;
        }
        seen.push(owner.clone());
    }
    Some(owners)
}

fn shape_fits(
    chains: &[Vec<usize>],
    owners: &[(usize, String)],
    axis: Axis,
    diagram: &Diagram,
) -> bool {
    let simple = chains.len() == 1
        && chains[0]
            .iter()
            .all(|index| diagram.relations[*index].label.is_none());
    simple || (owners.is_empty() && axis == rank_axis(diagram))
}

fn chain_axis(
    diagram: &Diagram,
    relations: &[Relation],
    owners: &[(usize, String)],
) -> Option<Axis> {
    let mut axes: Vec<Axis> = vec![];
    for (index, relation) in relations.iter().enumerate() {
        let reversed =
            matches!(relation.direction, Some(Direction::Up) | Some(Direction::Left));
        if reversed && !owners.iter().any(|(owner, _)| *owner == index) {
            return None;
        }
        axes.push(relation_axis(relation, diagram));
    }
    let first = *axes.first()?;
    axes.iter().all(|axis| *axis == first).then_some(first)
}

pub enum Placer {
    Fork(Fork),
    Chain(RankChain),
}

fn chain_for(diagram: &Diagram) -> Option<RankChain> {
    let chains = chains_of(diagram)?;
    let owners = junction_owners(diagram)?;
    let axis = chain_axis(diagram, &diagram.relations, &owners)?;
    if !shape_fits(&chains, &owners, axis, diagram) {
        return None;
    }
    let owner_names: Vec<String> = owners.iter().map(|(_, owner)| owner.clone()).collect();
    let halved = chains
        .iter()
        .flatten()
        .any(|index| diagram.relations[*index].label.is_some());
    {
        let mut names = vec![];
        for chain in &chains {
            names.extend(names_of(chain, diagram));
        }
        if !exact_classes(diagram, &names, &owner_names) {
            return None;
        }
    }
    Some(RankChain {
        chains,
        owners,
        axis,
        engine: engine(diagram),
        ranked: axis == rank_axis(diagram),
        halved,
    })
}

impl Placer {
    /// Ruby `RankChain.for`: `None` keeps the grid.
    pub fn for_diagram(diagram: &Diagram) -> Option<Placer> {
        let relations = &diagram.relations;
        let forkable = engine(diagram) == Engine::Smetana
            && diagram.packages.is_empty()
            && diagram.notes.is_empty()
            && diagram.junctions.is_empty()
            && relations.len() > 1
            && relations.iter().any(|r| r.layout_only);
        if forkable {
            if let Some((root, leaves)) = fork_ends(relations) {
                let mut names = vec![root.clone()];
                names.extend(leaves.clone());
                if exact_classes(diagram, &names, &[]) {
                    if let Some(axis) = chain_axis(diagram, relations, &[]) {
                        if axis == rank_axis(diagram) {
                            return Some(Placer::Fork(Fork {
                                root,
                                leaves,
                                axis,
                                engine: engine(diagram),
                            }));
                        }
                    }
                }
            }
        }
        chain_for(diagram).map(Placer::Chain)
    }

    /// Places the classes and junctions: the corners by class name, and
    /// each junction node as `[x, y, width, height]` by relation index.
    pub fn place(
        &self,
        diagram: &Diagram,
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
        label_widths: &HashMap<usize, f64>,
    ) -> (HashMap<String, [f64; 2]>, HashMap<usize, [f64; 4]>) {
        match self {
            Placer::Fork(fork) => (fork.place(sizes, origin), HashMap::new()),
            Placer::Chain(chain) => chain.place(diagram, sizes, origin, label_widths),
        }
    }
}

fn fork_ends(relations: &[Relation]) -> Option<(String, Vec<String>)> {
    let lefts: Vec<&String> = relations.iter().map(|r| &r.left).collect();
    let rights: Vec<&String> = relations.iter().map(|r| &r.right).collect();
    if unique_fork(&lefts, &rights) {
        return Some((lefts[0].clone(), rights.iter().map(|s| (*s).clone()).collect()));
    }
    if unique_fork(&rights, &lefts) {
        return Some((rights[0].clone(), lefts.iter().map(|s| (*s).clone()).collect()));
    }
    None
}

fn unique_fork(roots: &[&String], leaves: &[&String]) -> bool {
    let first = roots[0];
    if roots.iter().any(|root| *root != first) {
        return false;
    }
    let mut seen: Vec<&str> = vec![];
    for leaf in leaves {
        if seen.contains(&leaf.as_str()) {
            return false;
        }
        seen.push(leaf.as_str());
    }
    true
}

pub struct Fork {
    root: String,
    leaves: Vec<String>,
    axis: Axis,
    engine: Engine,
}

impl Fork {
    fn place(&self, sizes: &HashMap<String, [f64; 2]>, origin: [f64; 2]) -> HashMap<String, [f64; 2]> {
        let rank = self.engine.rank_gap();
        let node = self.engine.node_gap();
        let horizontal = self.axis == Axis::Horizontal;
        let across = if horizontal { 1 } else { 0 };
        let along = 1 - across;
        let root_size = sizes[&self.root];
        let spans: Vec<f64> = self.leaves.iter().map(|name| sizes[name][across]).collect();
        let mut parts: Vec<f64> = spans.clone();
        if parts.is_empty() {
            parts.push(0.0);
        }
        for _ in 0..spans.len().saturating_sub(1) {
            parts.push(node);
        }
        let breadth = crate::render::pie::fsum(parts);
        let root_breadth = root_size[across];
        let available = root_breadth.max(breadth) - breadth;
        let mut cursor = origin[across] + available / 2.0;
        let fixed = origin[along] + root_size[along] + rank;
        let mut corners = HashMap::new();
        for (index, name) in self.leaves.iter().enumerate() {
            let corner = if horizontal { [fixed, cursor] } else { [cursor, fixed] };
            corners.insert(name.clone(), corner);
            cursor += spans[index] + node;
        }
        let mut root_corner = origin;
        let extra = (breadth - root_size[across]).max(0.0);
        root_corner[across] += extra / 2.0;
        corners.insert(self.root.clone(), root_corner);
        corners
    }
}

#[derive(Clone, Debug)]
enum Slot {
    Class(String),
    Junction(usize, String),
    Virtual,
    Label(usize),
}

impl Slot {
    fn rendered(&self) -> bool {
        !matches!(self, Slot::Virtual | Slot::Label(_))
    }
}

pub struct RankChain {
    chains: Vec<Vec<usize>>,
    owners: Vec<(usize, String)>,
    axis: Axis,
    engine: Engine,
    ranked: bool,
    halved: bool,
}

struct ChainFrame {
    middle: f64,
    breadth: f64,
    cursor: f64,
    span: f64,
}

impl RankChain {
    fn along(&self) -> usize {
        if self.axis == Axis::Vertical { 1 } else { 0 }
    }

    fn across(&self) -> usize {
        1 - self.along()
    }

    fn owner_by_relation(&self, index: usize) -> Option<&String> {
        self.owners.iter().find(|(owner, _)| *owner == index).map(|(_, name)| name)
    }

    /// Ruby `place`.
    pub fn place(
        &self,
        diagram: &Diagram,
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
        label_widths: &HashMap<usize, f64>,
    ) -> (HashMap<String, [f64; 2]>, HashMap<usize, [f64; 4]>) {
        if !self.ranked {
            return self.place_flat(diagram, sizes, origin, label_widths);
        }
        let items = self.rank_items(diagram);
        let spans = self.rank_spans(&items, sizes, label_widths);
        let frames = self.chain_frames(&items, sizes, origin);
        let tops = self.rank_tops(&spans, origin[self.along()]);
        let mut places: HashMap<String, [f64; 2]> = HashMap::new();
        let mut junctions: HashMap<usize, [f64; 4]> = HashMap::new();
        for (chain, slot, rank) in items {
            let frame = ChainFrame {
                middle: frames[chain].0,
                breadth: frames[chain].1,
                cursor: tops[rank],
                span: spans[rank],
            };
            self.place_slot(&slot, sizes, &frame, &mut places, &mut junctions);
        }
        (places, junctions)
    }

    fn place_flat(
        &self,
        diagram: &Diagram,
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
        label_widths: &HashMap<usize, f64>,
    ) -> (HashMap<String, [f64; 2]>, HashMap<usize, [f64; 4]>) {
        let slots = self.chain_slots(diagram);
        let across = self.across();
        let mut breadth = 0.0_f64;
        for slot in &slots {
            breadth = breadth.max(self.slot_size(slot, sizes)[across]);
        }
        let middle = origin[across] + breadth / 2.0;
        let mut places = HashMap::new();
        let mut junctions = HashMap::new();
        let mut cursor = origin[self.along()];
        for slot in &slots {
            let span = self.along_span(slot, sizes);
            let frame = ChainFrame { middle, breadth, cursor, span };
            self.place_slot(&slot, sizes, &frame, &mut places, &mut junctions);
            cursor += span + self.engine.node_gap();
        }
        let _ = label_widths;
        (places, junctions)
    }

    fn chain_slots(&self, diagram: &Diagram) -> Vec<Slot> {
        let mut out = vec![];
        let names = names_of(&self.chains[0], diagram);
        for (index, name) in names.iter().enumerate() {
            out.push(Slot::Class(name.clone()));
            if let Some(owner) = self.owner_by_relation(index) {
                out.push(Slot::Junction(index, owner.clone()));
            }
        }
        out
    }

    fn rank_items(&self, diagram: &Diagram) -> Vec<(usize, Slot, usize)> {
        let mut index: i64 = -1;
        let mut items = vec![];
        for (number, chain) in self.chains.iter().enumerate() {
            let mut rank = 0usize;
            items.push((
                number,
                Slot::Class(diagram.relations[chain[0]].left.clone()),
                rank,
            ));
            for chain_index in chain {
                index += 1;
                let relation_index = *chain_index;
                let relation = &diagram.relations[relation_index];
                let steps =
                    ((relation.length as i64) - 1) * if self.halved { 2 } else { 1 };
                if let Some(owner) = self.owner_by_relation(relation_index) {
                    // A junction's legs step one rank each; the owner
                    // rides the junction slot.
                    items.push((
                        number,
                        Slot::Junction(relation_index, owner.clone()),
                        rank + 1,
                    ));
                    rank += 2;
                } else {
                    for step in 1..steps {
                        let node = if relation.label.is_some()
                            && step == steps / 2
                        {
                            Slot::Label(relation_index)
                        } else {
                            Slot::Virtual
                        };
                        items.push((number, node, (rank as i64 + step) as usize));
                    }
                    rank += steps as usize;
                }
                items.push((
                    number,
                    Slot::Class(relation.right.clone()),
                    rank,
                ));
            }
        }
        let _ = &mut index;
        items
    }

    fn rank_spans(
        &self,
        items: &[(usize, Slot, usize)],
        sizes: &HashMap<String, [f64; 2]>,
        label_widths: &HashMap<usize, f64>,
    ) -> Vec<f64> {
        let mut spans: Vec<f64> = vec![];
        for (_, slot, rank) in items {
            let span = match slot {
                Slot::Virtual => VIRTUAL_NODE,
                Slot::Label(index) => self.label_span(label_widths[index]),
                _ => self.along_span(slot, sizes),
            };
            while spans.len() <= *rank {
                spans.push(0.0);
            }
            spans[*rank] = spans[*rank].max(span);
        }
        spans
    }

    fn label_span(&self, width: f64) -> f64 {
        (if self.axis == Axis::Vertical { 15.0 } else { width })
            + self.engine.label_margin()
    }

    fn rank_tops(&self, spans: &[f64], start: f64) -> Vec<f64> {
        let separation = self.engine.rank_gap() / if self.halved { 2.0 } else { 1.0 };
        let mut tops = vec![start];
        for span in spans {
            let next = *tops.last().unwrap() + span + separation;
            tops.push(next);
        }
        tops
    }

    fn chain_frames(
        &self,
        items: &[(usize, Slot, usize)],
        sizes: &HashMap<String, [f64; 2]>,
        origin: [f64; 2],
    ) -> Vec<(f64, f64)> {
        let across = self.across();
        let mut breadths = vec![0.0_f64; self.chains.len()];
        for (chain, slot, _) in items {
            if !slot.rendered() {
                continue;
            }
            breadths[*chain] = breadths[*chain].max(self.slot_size(slot, sizes)[across]);
        }
        let mut frames = vec![];
        let mut edge = origin[across];
        for breadth in &breadths {
            frames.push((edge + breadth / 2.0, *breadth));
            edge += breadth + self.engine.node_gap();
        }
        frames
    }

    fn slot_size(&self, slot: &Slot, sizes: &HashMap<String, [f64; 2]>) -> [f64; 2] {
        match slot {
            Slot::Class(name) => sizes[name],
            _ => JUNCTION_SIZE,
        }
    }

    fn along_span(&self, slot: &Slot, sizes: &HashMap<String, [f64; 2]>) -> f64 {
        let along = self.along();
        let span = self.slot_size(slot, sizes)[along];
        if self.ranked {
            if let Slot::Junction(_, owner) = slot {
                return span.max(sizes[owner][along]);
            }
        }
        span
    }

    fn place_slot(
        &self,
        slot: &Slot,
        sizes: &HashMap<String, [f64; 2]>,
        frame: &ChainFrame,
        places: &mut HashMap<String, [f64; 2]>,
        junctions: &mut HashMap<usize, [f64; 4]>,
    ) {
        if !slot.rendered() {
            return;
        }
        let size = self.slot_size(slot, sizes);
        let along = self.along();
        let across = self.across();
        let along_position = frame.cursor + (frame.span - size[along]) / 2.0;
        let across_position = frame.middle - size[across] / 2.0;
        let corner = if self.axis == Axis::Vertical {
            [across_position, along_position]
        } else {
            [along_position, across_position]
        };
        match slot {
            Slot::Class(name) => {
                places.insert(name.clone(), corner);
            }
            Slot::Junction(index, owner) => {
                junctions.insert(*index, [corner[0], corner[1], size[0], size[1]]);
                let centre = corner[along] + JUNCTION_SIZE[along] / 2.0;
                let offset = if self.ranked {
                    corner[across] + JUNCTION_SIZE[across] + self.engine.node_gap()
                } else {
                    frame.middle + frame.breadth / 2.0 + self.engine.rank_gap()
                };
                let owner_size = sizes[owner];
                let owner_along = centre - owner_size[along] / 2.0;
                let owner_corner = if self.axis == Axis::Vertical {
                    [offset, owner_along]
                } else {
                    [owner_along, offset]
                };
                places.insert(owner.clone(), owner_corner);
            }
            _ => {}
        }
    }
}
