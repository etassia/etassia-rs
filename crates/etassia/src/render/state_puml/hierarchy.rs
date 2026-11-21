//! Places the states of a placed state graph one container at a time,
//! innermost first. Port of `lib/etassia/notation/plantuml/state/`'s
//! `Hierarchy`, `Ranking`, `Rows`, `Row`, `LevelEdges`, `LabelReach`,
//! `ClusterPadding`, `DotAxis`, `AxisSolver` and `Stretch`, over the
//! shared `crate::elk` layered engine (Ruby `ElkPlacement`).

use super::ir::{GEdge, GLabel, GNode, Graph, Section};
use crate::elk;
use crate::Error;
use std::collections::HashMap;

/// The arena the placement mutates: each state with its tree links.
pub struct Slot {
    pub node: GNode,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

/// Splits a graph into the arena the placement mutates.
pub fn arena_of(graph: &Graph) -> (Vec<Slot>, Vec<usize>) {
    let mut slots: Vec<Slot> = vec![];
    let mut roots: Vec<usize> = vec![];
    fn walk(nodes: &[GNode], parent: Option<usize>, slots: &mut Vec<Slot>) -> Vec<usize> {
        nodes
            .iter()
            .map(|node| {
                let at = slots.len();
                slots.push(Slot {
                    node: GNode { children: vec![], ..node.clone() },
                    children: vec![],
                    parent,
                });
                let children = walk(&node.children, Some(at), slots);
                slots[at].children = children;
                at
            })
            .collect()
    }
    roots = walk(&graph.children, None, &mut slots);
    (slots, roots)
}

/// Folds the arena back into the graph tree.
pub fn graph_of(slots: &[Slot], roots: &[usize]) -> Graph {
    fn build(slots: &[Slot], at: usize) -> GNode {
        let mut node = slots[at].node.clone();
        node.children = slots[at].children.iter().map(|&c| build(slots, c)).collect();
        node
    }
    Graph {
        id: String::new(),
        children: roots.iter().map(|&r| build(slots, r)).collect(),
        edges: vec![],
    }
}

// ---- ranking.rb ----

/// A rank key: a state's id, or a composite's top or bottom.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Key {
    Id(String),
    Top(String),
    Bottom(String),
}

/// The ranks PlantUML's layout gives the states, read across every
/// container at once (Ruby `Ranking`).
pub struct Ranking {
    arcs: HashMap<Key, Vec<Key>>,
    order: Vec<Key>,
    ends: HashMap<String, (Key, Key)>,
    starts: Vec<String>,
    entered: Vec<String>,
    rank: HashMap<Key, usize>,
}

impl Ranking {
    pub fn new(slots: &[Slot], roots: &[usize], edges: &[GEdge]) -> Ranking {
        let mut ranking = Ranking {
            arcs: HashMap::new(),
            order: vec![],
            ends: HashMap::new(),
            starts: vec![],
            entered: vec![],
            rank: HashMap::new(),
        };
        ranking.structure(slots, roots, None);
        for edge in edges {
            ranking.transition(edge);
        }
        ranking.entered = ranking.entered_by_start(edges);
        ranking.build_rank();
        ranking
    }

    /// The ranks a composite spans.
    pub fn span(&self, id: &str) -> Option<(usize, usize)> {
        let (top, bottom) = self.ends.get(id)?;
        Some((*self.rank.get(top)?, *self.rank.get(bottom)?))
    }

    /// Whether a state ranks below a composite's bottom.
    pub fn below(&self, id: &str, composite: &str) -> bool {
        let known = self.order.iter().find(|k| matches!(k, Key::Id(s) if s == id));
        let Some(known) = known else { return false };
        let Some((_, bottom)) = self.ends.get(composite) else { return false };
        self.rank.get(known).copied().unwrap_or(0) > self.rank.get(bottom).copied().unwrap_or(0)
    }

    /// Whether two composites share a rank.
    pub fn overlap(&self, one: &str, other: &str) -> bool {
        let (Some(first), Some(second)) = (self.span(one), self.span(other)) else {
            return false;
        };
        first.0 <= second.0 && second.0 <= first.1
            || second.0 <= first.0 && first.0 <= second.1
    }

    fn structure(&mut self, slots: &[Slot], at: &[usize], owner: Option<(Key, Key)>) {
        for &i in at {
            if slots[i].node.meta.state_type == "note" {
                continue;
            }
            self.enter(slots, i, owner.clone());
        }
    }

    fn enter(&mut self, slots: &[Slot], i: usize, owner: Option<(Key, Key)>) {
        let composite = !slots[i].children.is_empty();
        let (top, bottom) = if composite {
            let key = (Key::Top(slots[i].node.id.clone()), Key::Bottom(slots[i].node.id.clone()));
            self.ends.insert(slots[i].node.id.clone(), key.clone());
            key
        } else {
            let key = Key::Id(slots[i].node.id.clone());
            (key.clone(), key)
        };
        self.order.push(top.clone());
        if slots[i].node.meta.state_type == "start" {
            self.starts.push(slots[i].node.id.clone());
        }
        if let Some((owner_top, owner_bottom)) = &owner {
            self.arcs.entry(owner_top.clone()).or_default().push(top.clone());
            self.arcs.entry(bottom.clone()).or_default().push(owner_bottom.clone());
        }
        if composite {
            let children = slots[i].children.clone();
            self.structure(slots, &children, Some((top, bottom)));
        }
    }

    fn transition(&mut self, edge: &GEdge) {
        let from = self.bottom_of(edge.sources.first());
        let to = self.top_of(edge.targets.first());
        if let (Some(from), Some(to)) = (from, to) {
            if from != to {
                self.arcs.entry(from).or_default().push(to);
            }
        }
    }

    fn bottom_of(&self, id: Option<&String>) -> Option<Key> {
        let id = id?;
        Some(match self.ends.get(id) {
            Some((_, bottom)) => bottom.clone(),
            None => Key::Id(id.clone()),
        })
    }

    fn top_of(&self, id: Option<&String>) -> Option<Key> {
        let id = id?;
        Some(match self.ends.get(id) {
            Some((top, _)) => top.clone(),
            None => Key::Id(id.clone()),
        })
    }

    fn entered_by_start(&self, edges: &[GEdge]) -> Vec<String> {
        edges
            .iter()
            .filter(|edge| self.starts.iter().any(|s| Some(s) == edge.sources.first()))
            .filter_map(|edge| edge.targets.first().cloned())
            .collect()
    }

    /// Longest path from the sources, over the arcs that do not close a
    /// cycle. Composites are searched first.
    fn build_rank(&mut self) {
        let mut acyclic: HashMap<Key, Vec<Key>> = HashMap::new();
        self.cut_cycles(&mut acyclic);
        let mut keys: Vec<Key> = acyclic.keys().cloned().collect();
        for targets in acyclic.values() {
            keys.extend(targets.iter().cloned());
        }
        keys.extend(self.order.iter().cloned());
        let mut depth: HashMap<Key, usize> = HashMap::new();
        for key in self.topological(keys, &acyclic) {
            if let Some(targets) = acyclic.get(&key) {
                for target in targets {
                    let next = depth.get(&key).copied().unwrap_or(0) + 1;
                    let slot = depth.entry(target.clone()).or_insert(0);
                    *slot = (*slot).max(next);
                }
            }
        }
        self.rank = depth;
    }

    fn cut_cycles(&self, acyclic: &mut HashMap<Key, Vec<Key>>) {
        let mut state: HashMap<Key, u8> = HashMap::new();
        let composites: Vec<&Key> =
            self.order.iter().filter(|k| matches!(k, Key::Top(_))).collect();
        let leaves: Vec<&Key> =
            self.order.iter().filter(|k| matches!(k, Key::Id(_))).collect();
        let mut free: Vec<&Key> = vec![];
        let mut entered: Vec<&Key> = vec![];
        for key in &composites {
            let id = match key {
                Key::Top(id) => id,
                _ => unreachable!(),
            };
            if self.entered.iter().any(|e| e == id) {
                entered.push(key);
            } else {
                free.push(key);
            }
        }
        let roots: Vec<&Key> = free.into_iter().chain(entered).chain(leaves).collect();
        for key in roots {
            let key = key.clone();
            self.visit(&key, &mut state, acyclic);
        }
    }

    fn visit(&self, key: &Key, state: &mut HashMap<Key, u8>, acyclic: &mut HashMap<Key, Vec<Key>>) {
        if state.get(key).copied().unwrap_or(0) == 2 {
            return;
        }
        state.insert(key.clone(), 1);
        let arcs = self.arcs.get(key).cloned().unwrap_or_default();
        let mut kept: Vec<Key> = vec![];
        for target in &arcs {
            if state.get(target).copied() == Some(1) {
                continue;
            }
            kept.push(target.clone());
            self.visit(target, state, acyclic);
        }
        acyclic.insert(key.clone(), kept);
        state.insert(key.clone(), 2);
    }

    fn topological(&self, keys: Vec<Key>, acyclic: &HashMap<Key, Vec<Key>>) -> Vec<Key> {
        let mut seen: HashMap<Key, bool> = HashMap::new();
        let mut sorted: Vec<Key> = vec![];
        for key in &keys {
            self.sort_from(key, acyclic, &mut seen, &mut sorted);
        }
        sorted.reverse();
        sorted
    }

    fn sort_from(
        &self,
        key: &Key,
        acyclic: &HashMap<Key, Vec<Key>>,
        seen: &mut HashMap<Key, bool>,
        sorted: &mut Vec<Key>,
    ) {
        if seen.get(key).copied().unwrap_or(false) {
            return;
        }
        seen.insert(key.clone(), true);
        if let Some(targets) = acyclic.get(key) {
            for target in targets {
                self.sort_from(target, acyclic, seen, sorted);
            }
        }
        sorted.push(key.clone());
    }

    fn rank_of(&mut self, key: &Key) -> usize {
        if self.rank.is_empty() {
            self.build_rank();
        }
        *self.rank.get(key).unwrap_or(&0)
    }
}

// ---- rows.rb / row.rb ----

/// The note of a state, by its id: the note's arena slot.
pub type Notes = HashMap<String, usize>;

/// Chains the states of a container that PlantUML draws on one rank
/// (Ruby `Rows`).
struct Rows;

impl Rows {
    fn chains(flat: &[(String, String)], ids: &[String]) -> Vec<Vec<String>> {
        let mut follower: HashMap<String, String> = HashMap::new();
        for (from, to) in flat {
            if from != to && ids.iter().any(|i| i == from) && ids.iter().any(|i| i == to)
                && !follower.contains_key(from)
                && !follower.values().any(|v| v == to)
            {
                follower.insert(from.clone(), to.clone());
            }
        }
        let mut chained: Vec<Vec<String>> = vec![];
        let mut covered: Vec<String> = vec![];
        for id in ids {
            if follower.values().any(|v| v == id) {
                continue;
            }
            let mut row = vec![id.clone()];
            loop {
                let Some(onward) = follower.get(row.last().expect("row")) else { break };
                if row.iter().any(|r| r == onward) {
                    break;
                }
                row.push(onward.clone());
            }
            covered.extend(row.iter().cloned());
            chained.push(row);
        }
        for id in ids {
            if !covered.iter().any(|c| c == id) {
                chained.push(vec![id.clone()]);
            }
        }
        chained
    }
}

/// States of one container placed on one rank (Ruby `Row`).
pub struct Row {
    pub id: String,
    pub nodes: Vec<usize>,
}

const ROW_GAP: f64 = 50.0;
const NOTE_GAP: f64 = 30.0;

impl Row {
    fn width(&self, slots: &[Slot], notes: &Notes) -> f64 {
        let slots_w: f64 = self
            .nodes
            .iter()
            .map(|&n| slot_width(&slots[n], slots, notes))
            .sum();
        slots_w + ROW_GAP * (self.nodes.len().saturating_sub(1)) as f64
    }

    fn height(&self, slots: &[Slot], notes: &Notes) -> f64 {
        self.nodes
            .iter()
            .map(|&n| slot_height(&slots[n], slots, notes))
            .fold(f64::MIN, f64::max)
    }

    /// The left of the row's box.
    fn left(&self, slots: &[Slot], notes: &Notes) -> f64 {
        let first = &slots[self.nodes[0]];
        match notes.get(&first.node.id) {
            Some(&note) => first.node.x.unwrap_or(0.0) - slots[note].node.width - NOTE_GAP,
            None => first.node.x.unwrap_or(0.0),
        }
    }

    /// Moves each state and note `shift` to the right.
    fn slide(&self, slots: &mut [Slot], notes: &Notes, shift: f64) {
        for &n in &self.nodes {
            slots[n].node.x = Some(slots[n].node.x.unwrap_or(0.0) + shift);
            if let Some(&note) = notes.get(&slots[n].node.id) {
                slots[note].node.x = Some(slots[note].node.x.unwrap_or(0.0) + shift);
            }
        }
    }

    /// Writes the position of each state and note, the row's box at
    /// `left`, `top`.
    fn place(&self, slots: &mut [Slot], notes: &Notes, left: f64, top: f64) {
        let height = self.height(slots, notes);
        let mut left = left;
        for &n in &self.nodes {
            let slot_height = slot_height(&slots[n], slots, notes);
            let slot_width = slot_width(&slots[n], slots, notes);
            let note = notes.get(&slots[n].node.id).copied();
            slots[n].node.y = Some(top + ((height - slot_height) / 2.0));
            slots[n].node.x = Some(match note {
                Some(_) => left + slots[note.expect("note")].node.width + NOTE_GAP,
                None => left,
            });
            if let Some(note) = note {
                let beyond = slots[n].node.x.unwrap_or(0.0) + slots[n].node.width + NOTE_GAP;
                let leftward = slots[note].node.meta.note_position.as_deref() == Some("left");
                slots[note].node.x = Some(if leftward { left } else { beyond });
                slots[note].node.y = Some(top + ((height - slots[note].node.height) / 2.0));
            }
            left += slot_width + ROW_GAP;
        }
    }
}

fn slot_width(slot: &Slot, slots: &[Slot], notes: &Notes) -> f64 {
    match notes.get(&slot.node.id) {
        Some(&note) => slot.node.width + 2.0 * (slots[note].node.width + NOTE_GAP),
        None => slot.node.width,
    }
}

fn slot_height(slot: &Slot, slots: &[Slot], notes: &Notes) -> f64 {
    match notes.get(&slot.node.id) {
        Some(&note) => slot.node.height.max(slots[note].node.height),
        None => slot.node.height,
    }
}

// ---- cluster_padding.rb ----

/// The room PlantUML keeps between a composite's border and what it holds
/// (Ruby `ClusterPadding`).
pub struct ClusterPadding {
    edges: Vec<(String, String)>,
    parent: HashMap<String, Option<String>>,
}

#[derive(Clone, Copy)]
pub struct Room {
    pub left: f64,
    pub right: f64,
    pub bottom: f64,
    pub title: f64,
}

impl ClusterPadding {
    pub fn new(edges: &[GEdge], parent: &HashMap<String, Option<String>>) -> ClusterPadding {
        ClusterPadding {
            edges: edges
                .iter()
                .map(|edge| {
                    (
                        edge.sources.first().cloned().unwrap_or_default(),
                        edge.targets.first().cloned().unwrap_or_default(),
                    )
                })
                .collect(),
            parent: parent.clone(),
        }
    }

    pub fn room(&self, id: &str) -> Room {
        const BARE: Room = Room { left: 24.0, right: 24.0, bottom: 13.0, title: 20.0 };
        const CROSSED: Room = Room { left: 16.0, right: 16.0, bottom: 16.0, title: 15.5 };
        const BOTH: Room = Room { left: 24.0, right: 24.0, bottom: 24.0, title: 15.5 };
        if !self.crossing(id) {
            return BARE;
        }
        if self.own(id) {
            BOTH
        } else {
            CROSSED
        }
    }

    fn own(&self, id: &str) -> bool {
        self.edges.iter().any(|(from, to)| from == id || to == id)
    }

    fn crossing(&self, id: &str) -> bool {
        self.edges
            .iter()
            .any(|(from, to)| self.inside(from, id) != self.inside(to, id) && from != id && to != id)
    }

    fn inside(&self, state: &str, id: &str) -> bool {
        let mut state = state.to_string();
        loop {
            match self.parent.get(&state) {
                Some(None) => return false,
                Some(owner) if owner.as_deref() == Some(id) => return true,
                Some(owner) => state = owner.clone().expect("owner"),
                None => return false,
            }
        }
    }
}

// ---- level_edges.rb ----

/// The transitions one container's layout is given, as joins between its
/// rows (Ruby `LevelEdges`).
pub struct LevelEdges {
    pub edges: Vec<GEdge>,
    pub parent: HashMap<String, Option<String>>,
    pub ranking: Ranking,
}

impl LevelEdges {

    /// ELK edges between row ids.
    pub fn of(&self, rows: &[Row], slots: &[Slot]) -> Vec<LevelEdge> {
        let unit = unit_ids(rows, slots);
        let joins = self.joins(&unit);
        let (kept, peers): (Vec<_>, Vec<_>) = joins
            .into_iter()
            .partition(|(_, pair)| !self.ranking.overlap(&pair.0, &pair.1));
        let mut elk = self.elk_edges(majority(kept), &unit);
        let peer_pairs: Vec<(String, String)> =
            peers.into_iter().map(|(_, pair)| pair).collect();
        let entries = self.entries(&elk, &peer_pairs, &unit);
        elk.extend(entries);
        if peer_pairs.is_empty() {
            elk
        } else {
            by_target(elk, rows)
        }
    }

    fn elk_edges(&self, joined: Vec<(usize, (String, String))>, unit: &HashMap<String, String>) -> Vec<LevelEdge> {
        joined
            .into_iter()
            .filter_map(|(oid, pair)| {
                let from = unit.get(&pair.0)?;
                let to = unit.get(&pair.1)?;
                if from == to {
                    return None;
                }
                Some(LevelEdge {
                    id: oid.to_string(),
                    sources: vec![from.clone()],
                    targets: vec![to.clone()],
                    sections: None,
                })
            })
            .collect()
    }

    /// The ids of rows PlantUML draws beside a row one of its transitions
    /// joins, in the order given.
    pub fn peer_ids(&self, rows: &[Row], slots: &[Slot]) -> Vec<String> {
        let unit = unit_ids(rows, slots);
        let linked: Vec<(String, String)> = self
            .joins(&unit)
            .into_iter()
            .map(|(_, pair)| pair)
            .filter(|pair| self.ranking.overlap(&pair.0, &pair.1))
            .collect();
        let linked: Vec<String> = linked
            .iter()
            .flat_map(|pair| [pair.0.clone(), pair.1.clone()])
            .collect();
        let flat: Vec<String> = linked
            .into_iter()
            .filter_map(|id| unit.get(&id).cloned())
            .collect();
        rows.iter()
            .map(|r| r.id.clone())
            .filter(|id| flat.iter().any(|f| f == id))
            .collect()
    }

    fn joins(&self, unit: &HashMap<String, String>) -> Vec<(usize, (String, String))> {
        let keys: Vec<String> = unit.keys().cloned().collect();
        self.edges
            .iter()
            .enumerate()
            .filter_map(|(oid, edge)| {
                RoutesHelper::level_pair(edge, &self.parent, &keys).map(|pair| (oid, pair))
            })
            .collect()
    }

    /// A composite with nothing leading to it enters at the rank of the
    /// peer it stands beside.
    fn entries(
        &self,
        elk: &[LevelEdge],
        peers: &[(String, String)],
        unit: &HashMap<String, String>,
    ) -> Vec<LevelEdge> {
        let mut out = vec![];
        for pair in peers {
            let mine = unit.get(&pair.0).cloned().unwrap_or_default();
            let other = unit.get(&pair.1).cloned().unwrap_or_default();
            for (mine, other) in [(mine.clone(), other.clone()), (other, mine)] {
                if elk.iter().any(|found| found.targets.first().map(|t| t == &mine).unwrap_or(false)) {
                    continue;
                }
                for found in elk {
                    if found.targets.first().map(|t| t == &other).unwrap_or(false) {
                        out.push(LevelEdge {
                            id: format!("{}>{}", found.id, mine),
                            sources: found.sources.clone(),
                            targets: vec![mine.clone()],
                            sections: None,
                        });
                    }
                }
            }
        }
        out
    }
}

/// Of two states joined both ways, graphviz keeps the way most transitions
/// run (Ruby `majority`).
fn majority(joined: Vec<(usize, (String, String))>) -> Vec<(usize, (String, String))> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for (_, pair) in &joined {
        *counts.entry(pair.clone()).or_default() += 1;
    }
    joined
        .into_iter()
        .filter(|(_, pair)| {
            let reverse = (pair.1.clone(), pair.0.clone());
            let mine = counts.get(pair).copied().unwrap_or(0);
            let theirs = counts.get(&reverse).copied().unwrap_or(0);
            mine >= theirs
        })
        .collect()
}

fn unit_ids(rows: &[Row], slots: &[Slot]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for row in rows {
        for node in &row.nodes {
            out.insert(slots[*node].node.id.clone(), row.id.clone());
        }
    }
    out
}

/// Edges into the rows listed first come first, so ELK places those rows
/// first (Ruby `by_target`).
fn by_target(edges: Vec<LevelEdge>, rows: &[Row]) -> Vec<LevelEdge> {
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let mut indexed: Vec<(LevelEdge, usize)> = edges.into_iter().enumerate().map(|(i, e)| (e, i)).collect();
    indexed.sort_by(|(a, ai), (b, bi)| {
        let at = ids.iter().position(|id| Some(*id) == a.targets.first().map(String::as_str));
        let bt = ids.iter().position(|id| Some(*id) == b.targets.first().map(String::as_str));
        (at, *ai).cmp(&(bt, *bi))
    });
    indexed.into_iter().map(|(e, _)| e).collect()
}

/// An edge of a level's ELK graph.
#[derive(Clone, Debug)]
pub struct LevelEdge {
    pub id: String,
    pub sources: Vec<String>,
    pub targets: Vec<String>,
    pub sections: Option<Vec<Section>>,
}

// ---- label_reach.rb ----

/// How far right a container must reach to hold the labels of the
/// transitions between its members (Ruby `LabelReach`).
pub struct LabelReach {
    edges: Vec<GEdge>,
    parent: HashMap<String, Option<String>>,
}

/// One label of a rank (Ruby `LabelReach::Label`).
#[derive(Clone)]
struct ReachLabel {
    edge: usize,
    ideal: f64,
    width: f64,
    row: f64,
    down: bool,
    direct: bool,
}

impl LabelReach {
    pub fn new(edges: &[GEdge], parent: &HashMap<String, Option<String>>) -> LabelReach {
        LabelReach {
            edges: edges.iter().filter(|e| !e.labels.is_empty()).cloned().collect(),
            parent: parent.clone(),
        }
    }

    /// The width the container needs for its labels.
    pub fn width(&self, members: &[usize], slots: &[Slot]) -> f64 {
        const MARGIN: f64 = 12.5;
        let ranks = self.by_rank(members, slots);
        if ranks.is_empty() {
            return 0.0;
        }
        ranks
            .iter()
            .map(|(_, labels)| placed(labels).into_iter().next_back())
            .filter_map(|last| last)
            .map(|(label, left)| left + label.width)
            .fold(f64::MIN, f64::max)
            + MARGIN
    }

    /// Where the labels of a rank shared by two or more transitions stand.
    pub fn slots(
        &self,
        members: &[usize],
        slots: &[Slot],
    ) -> Vec<(usize, (f64, f64, f64))> {
        let owned_ranks = self.by_rank(members, slots);
        let ranks: Vec<Vec<&ReachLabel>> = owned_ranks
            .iter()
            .map(|(_, all)| all.iter().filter(|l| l.direct).collect::<Vec<_>>())
            .filter(|labels: &Vec<&ReachLabel>| labels.len() > 1)
            .collect();
        let mut out = vec![];
        for labels in ranks {
            let owned: Vec<ReachLabel> = labels.iter().map(|l| (*l).clone()).collect();
            let row = placed(&owned);
            let first = row.first().map(|(_, left)| *left).unwrap_or(0.0);
            let span = row
                .last()
                .map(|(label, left)| left + label.width - first)
                .unwrap_or(0.0);
            let ideal: f64 = labels.iter().map(|l| l.ideal).sum::<f64>() / labels.len() as f64;
            let slide = (ideal - span / 2.0).max(0.0) - first;
            for (label, left) in row {
                out.push((
                    label.edge,
                    (left + slide + label.width / 2.0, label.row, left + slide),
                ));
            }
        }
        out
    }

    fn by_rank(&self, members: &[usize], slots: &[Slot]) -> Vec<(f64, Vec<ReachLabel>)> {
        let by_id: HashMap<&str, usize> =
            members.iter().map(|&m| (slots[m].node.id.as_str(), m)).collect();
        let keys: Vec<String> = by_id.keys().map(|k| k.to_string()).collect();
        let mut out: Vec<(f64, Vec<ReachLabel>)> = vec![];
        for edge in &self.edges {
            let Some(pair) =
                RoutesHelper::level_pair(edge, &self.parent, &keys)
            else {
                continue;
            };
            let (Some(&from), Some(&to)) = (
                by_id.get(pair.0.as_str()),
                by_id.get(pair.1.as_str()),
            ) else {
                continue;
            };
            let (upper, lower) = if slots[from].node.y.unwrap_or(0.0)
                <= slots[to].node.y.unwrap_or(0.0)
            {
                (from, to)
            } else {
                (to, from)
            };
            let upper = &slots[upper];
            let lower = &slots[lower];
            let key = upper.node.y.unwrap_or(0.0) + upper.node.height;
            let label = ReachLabel {
                edge: edge.oid,
                ideal: middle(upper, lower) + 1.0,
                width: edge.labels[0].width,
                row: rank_middle(upper, lower),
                down: upper.node.id == slots[from].node.id,
                direct: edge.sources.first() == Some(&slots[from].node.id)
                    && edge.targets.first() == Some(&slots[to].node.id),
            };
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some((_, labels)) => labels.push(label),
                None => out.push((key, vec![label])),
            }
        }
        out
    }
}

fn middle(upper: &Slot, lower: &Slot) -> f64 {
    let one = upper.node.x.unwrap_or(0.0) + upper.node.width / 2.0;
    let two = lower.node.x.unwrap_or(0.0) + lower.node.width / 2.0;
    (one + two) / 2.0
}

fn rank_middle(upper: &Slot, lower: &Slot) -> f64 {
    let bottom = upper.node.y.unwrap_or(0.0) + upper.node.height;
    bottom + ((lower.node.y.unwrap_or(0.0) - bottom) / 2.0)
}

/// Each label of a rank with its left edge: side by side from the ideal
/// spot, those of edges that run down before those that run up.
fn placed(labels: &[ReachLabel]) -> Vec<(&ReachLabel, f64)> {
    let mut ordered: Vec<(usize, &ReachLabel)> = labels.iter().enumerate().collect();
    ordered.sort_by(|(ai, a), (bi, b)| {
        let base = a.ideal.partial_cmp(&b.ideal).unwrap_or(std::cmp::Ordering::Equal);
        let base = base.then((if a.down { 0u8 } else { 1u8 }).cmp(&(if b.down { 0u8 } else { 1u8 })));
        base.then(ai.cmp(bi))
    });
    let mut right = 0.0;
    ordered
        .into_iter()
        .map(|(_, label)| {
            let left = label.ideal.max(right + 18.0);
            right = left + label.width;
            (label, left)
        })
        .collect()
}

// ---- routes helper shared ahead of routes.rs ----

pub(crate) struct RoutesHelper;

impl RoutesHelper {
    /// The two members of a container the edge joins.
    pub fn level_pair(
        edge: &GEdge,
        parent: &HashMap<String, Option<String>>,
        ids: &[String],
    ) -> Option<(String, String)> {
        let source = member_of(edge.sources.first()?, parent, ids)?;
        let target = member_of(edge.targets.first()?, parent, ids)?;
        if source != target {
            Some((source, target))
        } else {
            None
        }
    }
}

pub(crate) fn member_of(
    id: &str,
    parent: &HashMap<String, Option<String>>,
    ids: &[String],
) -> Option<String> {
    let mut id = id.to_string();
    loop {
        if ids.iter().any(|i| i == &id) {
            return Some(id);
        }
        match parent.get(&id) {
            Some(owner) => id = owner.clone()?,
            None => return None,
        }
    }
}

// ---- label width helper for the container sizing ----

pub fn label_width(labels: &[GLabel]) -> f64 {
    labels.first().map(|l| l.width).unwrap_or(0.0)
}

// ---- ElkPlacement (layout/elk_placement.rb) ----

/// Positions an ELK-shaped level graph with the layered engine.
pub fn elk_apply(
    children: &[(String, f64, f64)],
    edges: &[LevelEdge],
    options: &serde_json::Map<String, serde_json::Value>,
) -> Result<Elk, Error> {
    let mut opts = serde_json::Map::new();
    opts.insert(
        "elk.layered.considerModelOrder.strategy".to_string(),
        serde_json::json!("NODES_AND_EDGES"),
    );
    opts.insert("elk.direction".to_string(), serde_json::json!("DOWN"));
    opts.insert("elk.alignment".to_string(), serde_json::json!("TOP"));
    opts.insert("elk.separateConnectedComponents".to_string(), serde_json::json!(false));
    opts.insert(
        "elk.layered.crossingMinimization.strategy".to_string(),
        serde_json::json!("NONE"),
    );
    for key in ["elk.direction", "elk.spacing.nodeNode", "elk.layered.spacing.nodeNodeBetweenLayers"] {
        if let Some(v) = options.get(key) {
            opts.insert(key.to_string(), v.clone());
        }
    }
    // Edges that close a cycle are dropped from the input (Ruby
    // `copy_input_edges`).
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    let mut kept: Vec<&LevelEdge> = vec![];
    for edge in edges {
        let (Some(source), Some(target)) = (edge.sources.first(), edge.targets.first()) else {
            continue;
        };
        if cycle_between(source, target, &links) {
            continue;
        }
        links.entry(source.clone()).or_default().push(target.clone());
        kept.push(edge);
    }
    let input = serde_json::json!({
        "id": "level",
        "layoutOptions": opts,
        "children": children
            .iter()
            .map(|(id, w, h)| {
                let mut child = serde_json::json!({"id": id, "width": w, "height": h});
                child["layoutOptions"] =
                    serde_json::json!({"elk.alignment": opts["elk.alignment"]});
                child
            })
            .collect::<Vec<_>>(),
        "edges": kept
            .iter()
            .map(|edge| serde_json::json!({
                "id": edge.id, "sources": edge.sources, "targets": edge.targets,
            }))
            .collect::<Vec<_>>(),
    });
    let placed = elk::layout(&input)?;
    let spots: HashMap<String, (f64, f64, f64, f64)> = placed["children"]
        .as_array()
        .map(|children| {
            children
                .iter()
                .filter_map(|child| {
                    let id = child["id"].as_str()?.to_string();
                    Some((
                        id,
                        (
                            child["x"].as_f64()?,
                            child["y"].as_f64()?,
                            child["width"].as_f64()?,
                            child["height"].as_f64()?,
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let sections: HashMap<String, Vec<Section>> = placed["edges"]
        .as_array()
        .map(|edges| {
            edges
                .iter()
                .filter_map(|edge| {
                    let id = edge["id"].as_str()?.to_string();
                    let list: Vec<Section> = edge["sections"]
                        .as_array()?
                        .iter()
                        .filter_map(|s| {
                            Some(Section {
                                start_point: point_of(&s["startPoint"])?,
                                end_point: point_of(&s["endPoint"])?,
                                bend_points: s["bendPoints"]
                                    .as_array()
                                    .map(|bends| {
                                        bends.iter().filter_map(point_of).collect()
                                    })
                                    .unwrap_or_default(),
                            })
                        })
                        .collect();
                    if list.is_empty() {
                        None
                    } else {
                        Some((id, list))
                    }
                })
                .fold(HashMap::new(), |mut all, (id, list)| {
                    all.entry(id).or_insert_with(|| list.clone());
                    all
                })
        })
        .unwrap_or_default();
    Ok(Elk {
        spots,
        edges: kept
            .iter()
            .map(|edge| LevelEdge {
                id: edge.id.clone(),
                sources: edge.sources.clone(),
                targets: edge.targets.clone(),
                sections: sections.get(&edge.id).cloned(),
            })
            .collect(),
    })
}

fn point_of(v: &serde_json::Value) -> Option<(f64, f64)> {
    Some((v["x"].as_f64()?, v["y"].as_f64()?))
}

fn cycle_between(
    source: &str,
    target: &str,
    links: &HashMap<String, Vec<String>>,
) -> bool {
    if source == target {
        return true;
    }
    let mut pending = vec![target.to_string()];
    let mut visited: HashMap<String, bool> = HashMap::new();
    while let Some(node) = pending.pop() {
        if node == source {
            return true;
        }
        if visited.get(&node).copied().unwrap_or(false) {
            continue;
        }
        visited.insert(node.clone(), true);
        pending.extend(links.get(&node).cloned().unwrap_or_default());
    }
    false
}

/// What the ELK run left: row placements and routed level edges.
#[derive(Default)]
pub struct Elk {
    pub spots: HashMap<String, (f64, f64, f64, f64)>,
    pub edges: Vec<LevelEdge>,
}


// ---- hierarchy.rb ----

const MARGIN: f64 = 20.0;
const HEADER: f64 = 36.0;
/// The step of one rank between the tops of two composites.
const RANK_DROP: f64 = 68.0;
/// The distance from a composite to the end marker beside it.
const END_GAP: f64 = 130.0;
/// The room between two composites that stand side by side.
const PEER_GAP: f64 = 24.0;
const PEER_SPACING: &str = "elk.spacing.nodeNode";

/// A placed level: the ELK run of its rows and the shift its container's
/// padding asked for.
#[derive(Default)]
pub struct Level {
    pub elk: Option<Elk>,
    pub shift: (f64, f64),
}

/// Places the graph's states and edges in place (Ruby `Hierarchy#place`).
pub struct Hierarchy {
    pub slots: Vec<Slot>,
    pub roots: Vec<usize>,
    pub edges: Vec<GEdge>,
    flat: Vec<(String, String)>,
    nodes: HashMap<String, usize>,
    parent: HashMap<String, Option<String>>,
    levels: Vec<(Option<String>, Level)>,
    notes: Notes,
    label_reach: LabelReach,
    padding: ClusterPadding,
    options: serde_json::Map<String, serde_json::Value>,
}

impl Hierarchy {
    pub fn new(
        slots: Vec<Slot>,
        roots: Vec<usize>,
        edges: Vec<GEdge>,
        flat: Vec<(String, String)>,
        options: serde_json::Map<String, serde_json::Value>,
    ) -> Hierarchy {
        let mut nodes: HashMap<String, usize> = HashMap::new();
        let mut parent: HashMap<String, Option<String>> = HashMap::new();
        Self::index(&slots, &roots, None, &mut nodes, &mut parent);
        let label_reach = LabelReach::new(&edges, &parent);
        let padding = ClusterPadding::new(&edges, &parent);
        Hierarchy {
            slots,
            roots,
            edges,
            flat,
            nodes,
            parent,
            levels: vec![],
            notes: HashMap::new(),
            label_reach,
            padding,
            options,
        }
    }

    /// The joins of one level, read fresh (the Ruby reader holds one
    /// instance; the data it reads does not change under it).
    fn level_edges(&self) -> LevelEdges {
        LevelEdges {
            edges: self.edges.clone(),
            parent: self.parent.clone(),
            ranking: Ranking::new(&self.slots, &self.roots, &self.edges),
        }
    }

    pub fn place(&mut self) -> Result<(), Error> {
        self.adopt_notes();
        let level_edges = self.level_edges();
        let roots = self.roots.clone();
        self.place_level(None, &roots, &level_edges)?;
        self.sink_followers()?;
        let edges = std::mem::take(&mut self.edges);
        let mut routes = super::routes::Routes::new(
            self.slots.as_slice(),
            self.roots.clone(),
            edges,
            self.nodes.clone(),
            self.parent.clone(),
            &self.levels,
            &self.label_reach,
        );
        routes.write()?;
        self.edges = routes.into_edges();
        Ok(())
    }

    fn index(
        slots: &[Slot],
        at: &[usize],
        parent: Option<&str>,
        nodes: &mut HashMap<String, usize>,
        parents: &mut HashMap<String, Option<String>>,
    ) {
        for &i in at {
            let id = slots[i].node.id.clone();
            nodes.insert(id.clone(), i);
            parents.insert(id, parent.map(str::to_string));
            Self::index(slots, &slots[i].children, Some(&slots[i].node.id), nodes, parents);
        }
    }

    /// A note moves into the container of its target.
    fn adopt_notes(&mut self) {
        let note_slots: Vec<usize> = (0..self.slots.len())
            .filter(|&i| {
                self.slots[i].node.meta.state_type == "note"
                    && self.slots[i].parent.is_none()
            })
            .collect();
        for i in note_slots {
            let Some(target) = self.slots[i].node.meta.note_target.clone() else { continue };
            let Some(&target_slot) = self.nodes.get(&target) else { continue };
            let holder = self.slots[target_slot].parent;
            let Some(at) = self.roots.iter().position(|&r| r == i) else { continue };
            self.roots.remove(at);
            let id = self.slots[i].node.id.clone();
            self.parent.insert(id, holder.map(|h| self.slots[h].node.id.clone()));
            match holder {
                Some(h) => self.slots[h].children.push(i),
                None => self.roots.push(i),
            }
            self.notes.insert(target, i);
        }
    }

    fn members_of(&self, children: &[usize]) -> Vec<usize> {
        children
            .iter()
            .copied()
            .filter(|&c| !self.notes.values().any(|&n| n == c))
            .collect()
    }

    fn place_level(
        &mut self,
        owner: Option<String>,
        children: &[usize],
        level_edges: &LevelEdges,
    ) -> Result<(), Error> {
        for &child in children {
            if !self.slots[child].children.is_empty() {
                let id = self.slots[child].node.id.clone();
                let grand = self.slots[child].children.clone();
                let inner = self.level_edges();
                self.place_level(Some(id), &grand, &inner)?;
            }
        }
        let members = self.members_of(children);
        let elk = self.position(&members, level_edges)?;
        let shift = self.shift_for(owner.as_deref(), children);
        self.finish(owner.as_deref(), children, shift);
        self.levels.push((owner, Level { elk, shift }));
        Ok(())
    }

    /// The placed ELK graph of the members.
    fn position(
        &mut self,
        members: &[usize],
        level_edges: &LevelEdges,
    ) -> Result<Option<Elk>, Error> {
        if members.is_empty() {
            return Ok(None);
        }
        let mut rows = self.rows_of(members, level_edges);
        let mut elk = self.lay_out(&rows, level_edges)?;
        self.settle(&mut rows, &mut elk, level_edges);
        Ok(Some(elk))
    }

    fn settle(&mut self, rows: &mut [Row], elk: &mut Elk, level_edges: &LevelEdges) {
        let spots = elk.spots.clone();
        let drop = self.entered_peers(rows, level_edges);
        for row in rows.iter_mut() {
            let Some(&(x, y, _, _)) = spots.get(&row.id) else { continue };
            let dropped = drop.iter().any(|d| d == &row.id);
            let mut slots = std::mem::take(&mut self.slots);
            row.place(&mut slots, &self.notes, x, y + if dropped { RANK_DROP } else { 0.0 });
            self.slots = slots;
        }
        self.follow(elk, &drop);
        self.above_entered(rows, elk, &drop);
        self.beside_composites(rows, elk);
        self.slide_to_dot(rows, elk);
    }

    /// The rows of the diagram's top level that graphviz puts elsewhere
    /// along the x axis move there, and the routes ELK drew to them are
    /// dropped.
    fn slide_to_dot(&mut self, rows: &mut [Row], elk: &mut Elk) {
        let Some(&first) = rows.first().and_then(|r| r.nodes.first()) else { return };
        if self.slots[first].parent.is_some() {
            return;
        }
        let mut axis = DotAxis::new(&self.edges, &self.nodes, &self.parent, self.notes.clone());
        let moved = axis.apply(rows, &mut self.slots);
        let moved_ids: Vec<String> = moved.iter().map(|&r| rows[r].id.clone()).collect();
        for edge in &mut elk.edges {
            let touches = edge
                .sources
                .iter()
                .chain(edge.targets.iter())
                .any(|id| moved_ids.iter().any(|m| m == id));
            if touches {
                edge.sections = None;
            }
        }
    }

    /// An end marker a composite leads to stands beside the composite,
    /// on the rank after the one its own start marker sits on.
    fn beside_composites(&mut self, rows: &[Row], elk: &mut Elk) {
        let mut markers: Vec<usize> = vec![];
        for row in rows {
            for &n in &row.nodes {
                if self.slots[n].node.meta.state_type == "end" {
                    markers.push(n);
                }
            }
        }
        for marker in markers {
            let Some(composite) = self.leaving_composite(marker) else { continue };
            let Some(after) = self.after_start(composite) else { continue };
            let m = self.slots[marker].node.height;
            let cx = self.slots[composite].node.x.unwrap_or(0.0)
                + self.slots[composite].node.width
                + END_GAP;
            let cy = self.slots[composite].node.y.unwrap_or(0.0)
                + self.slots[after].node.y.unwrap_or(0.0)
                + ((self.slots[after].node.height - m) / 2.0);
            self.slots[marker].node.x = Some(cx);
            self.slots[marker].node.y = Some(cy);
            let marker_id = self.slots[marker].node.id.clone();
            for edge in &mut elk.edges {
                if edge.targets.clone() == vec![marker_id.clone()] {
                    edge.sections = None;
                }
            }
        }
    }

    fn leaving_composite(&self, marker: usize) -> Option<usize> {
        let id = self.slots[marker].node.id.clone();
        for edge in &self.edges {
            if edge.targets.len() == 1 && edge.targets[0] == id {
                let Some(source) = self.nodes.get(&edge.sources[0]) else { continue };
                if !self.slots[*source].children.is_empty() {
                    return Some(*source);
                }
            }
        }
        None
    }

    fn after_start(&self, composite: usize) -> Option<usize> {
        let start = self.slots[composite]
            .children
            .iter()
            .copied()
            .find(|&c| self.slots[c].node.meta.state_type == "start")?;
        let start_id = self.slots[start].node.id.clone();
        let edge = self
            .edges
            .iter()
            .find(|edge| edge.sources.len() == 1 && edge.sources[0] == start_id)?;
        self.nodes.get(&edge.targets[0]).copied()
    }

    /// A start marker whose only way on is a composite that stands a rank
    /// lower hangs straight above it; its route is then the straight line
    /// between them.
    fn above_entered(&mut self, rows: &[Row], elk: &mut Elk, dropped: &[String]) {
        let mut starts: Vec<usize> = vec![];
        for row in rows {
            for &n in &row.nodes {
                if self.slots[n].node.meta.state_type == "start" {
                    starts.push(n);
                }
            }
        }
        for id in dropped {
            let Some(&start) = starts.iter().find(|&&s| self.starts_into(s, id)) else {
                continue;
            };
            let Some(&target) = self.nodes.get(id) else { continue };
            let x = self.slots[target].node.x.unwrap_or(0.0)
                + ((self.slots[target].node.width - self.slots[start].node.width) / 2.0);
            self.slots[start].node.x = Some(x);
            let start_id = self.slots[start].node.id.clone();
            for edge in &mut elk.edges {
                if edge.sources.clone() == vec![start_id.clone()] {
                    edge.sections = None;
                }
            }
        }
    }

    fn starts_into(&self, start: usize, id: &str) -> bool {
        let start_id = self.slots[start].node.id.clone();
        self.edges.iter().any(|edge| {
            edge.sources == vec![start_id.clone()]
                && edge.targets.first().map(String::as_str) == Some(id)
        })
    }

    /// A route ELK drew to a row that has moved follows it down: its end
    /// and last bend travel with the row.
    fn follow(&self, elk: &mut Elk, moved: &[String]) {
        for edge in &mut elk.edges {
            let touches = edge.targets.iter().any(|t| moved.iter().any(|m| m == t));
            if !touches {
                continue;
            }
            if let Some(sections) = &mut edge.sections {
                for section in sections {
                    section.end_point.1 += RANK_DROP;
                    if let Some(last) = section.bend_points.last_mut() {
                        last.1 += RANK_DROP;
                    }
                }
            }
        }
    }

    /// Composites side by side, the one a start marker enters stands a
    /// rank below the others.
    fn entered_peers(&self, rows: &[Row], level_edges: &LevelEdges) -> Vec<String> {
        let peers = level_edges.peer_ids(rows, &self.slots);
        let entered: Vec<String> = peers
            .iter()
            .filter(|id| self.entered_by_start(id))
            .cloned()
            .collect();
        if entered.len() == peers.len() {
            vec![]
        } else {
            entered
        }
    }

    fn entered_by_start(&self, id: &str) -> bool {
        self.edges.iter().any(|edge| {
            edge.targets.first().map(String::as_str) == Some(id)
                && self
                    .nodes
                    .get(&edge.sources[0])
                    .map(|&s| self.slots[s].node.meta.state_type.as_str() == "start")
                    .unwrap_or(false)
        })
    }

    /// The members of a level, those joined by a flat transition gathered
    /// into one row in the order they chain.
    fn rows_of(&self, members: &[usize], level_edges: &LevelEdges) -> Vec<Row> {
        let ids: Vec<String> = members.iter().map(|&m| self.slots[m].node.id.clone()).collect();
        let chains = Rows::chains(&self.flat, &ids);
        let by_id: HashMap<String, usize> =
            members.iter().map(|&m| (self.slots[m].node.id.clone(), m)).collect();
        let mut rows: Vec<Row> = chains
            .into_iter()
            .map(|row_ids| Row {
                id: if row_ids.len() > 1 {
                    format!("row:{}", row_ids[0])
                } else {
                    row_ids[0].clone()
                },
                nodes: row_ids
                    .iter()
                    .filter_map(|id| by_id.get(id).copied())
                    .collect(),
            })
            .collect();
        // Composites side by side that a transition joins stand in the
        // reverse of the order they are listed.
        let peers = level_edges.peer_ids(&rows, &self.slots);
        let spots: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| peers.iter().any(|p| p == &r.id))
            .map(|(at, _)| at)
            .collect();
        let mut order: HashMap<usize, usize> = HashMap::new();
        for (a, b) in spots.iter().zip(spots.iter().rev()) {
            order.insert(*a, *b);
        }
        (0..rows.len())
            .map(|at| {
                let from = order.get(&at).copied().unwrap_or(at);
                Row { id: rows[from].id.clone(), nodes: rows[from].nodes.clone() }
            })
            .collect()
    }

    fn lay_out(&self, rows: &[Row], level_edges: &LevelEdges) -> Result<Elk, Error> {
        let mut options = self.options.clone();
        let peers = level_edges.peer_ids(rows, &self.slots);
        if !peers.is_empty() {
            options.insert(PEER_SPACING.to_string(), serde_json::json!(PEER_GAP));
        }
        let children: Vec<(String, f64, f64)> = rows
            .iter()
            .map(|row| {
                (
                    row.id.clone(),
                    row.width(&self.slots, &self.notes),
                    row.height(&self.slots, &self.notes),
                )
            })
            .collect();
        let edges = level_edges.of(rows, &self.slots);
        elk_apply(&children, &edges, &options)
    }

    fn shift_for(&self, owner: Option<&str>, children: &[usize]) -> (f64, f64) {
        let Some(owner) = owner else { return (MARGIN, MARGIN) };
        let left = children
            .iter()
            .filter_map(|&c| self.slots[c].node.x)
            .fold(f64::MAX, f64::min);
        let top = children
            .iter()
            .filter_map(|&c| self.slots[c].node.y)
            .fold(f64::MAX, f64::min);
        let left = if left.is_finite() { left } else { 0.0 };
        let top = if top.is_finite() { top } else { 0.0 };
        let room = self.padding.room(owner);
        (room.left - left, HEADER - top)
    }

    fn finish(&mut self, owner: Option<&str>, children: &[usize], shift: (f64, f64)) {
        for &c in children {
            self.slots[c].node.x = Some(self.slots[c].node.x.unwrap_or(0.0) + shift.0);
            self.slots[c].node.y = Some(self.slots[c].node.y.unwrap_or(0.0) + shift.1);
        }
        if let Some(owner) = owner {
            let Some(&at) = self.nodes.get(owner) else { return };
            let room = self.padding.room(owner);
            let extent_x = self.extent(children, true);
            let extent_y = self.extent(children, false);
            let title = self.slots[at].node.labels.first().map(|l| l.width).unwrap_or(0.0);
            let members = self.members_of(children);
            let reach = self.label_reach.width(&members, &self.slots);
            let width = (extent_x + room.right).max(title + room.title).max(reach);
            let height = extent_y + room.bottom;
            self.slots[at].node.width = width;
            self.slots[at].node.height = height;
        }
    }

    /// The farthest x+width (horizontal) or y+height over the children.
    fn extent(&self, children: &[usize], horizontal: bool) -> f64 {
        children
            .iter()
            .map(|&c| {
                let node = &self.slots[c].node;
                if horizontal {
                    node.x.unwrap_or(0.0) + node.width
                } else {
                    node.y.unwrap_or(0.0) + node.height
                }
            })
            .fold(0.0f64, f64::max)
    }

    /// Lowers a state inside a top-level composite to the rank below
    /// another composite it follows, and stretches its composite to hold
    /// it (Ruby `Stretch`).
    fn sink_followers(&mut self) -> Result<(), Error> {
        let mut ranking = Ranking::new(&self.slots, &self.roots, &self.edges);
        let composites: Vec<String> = self
            .roots
            .iter()
            .filter(|&&r| !self.slots[r].children.is_empty())
            .map(|&r| self.slots[r].node.id.clone())
            .collect();
        let mut moved: Vec<usize> = vec![];
        let edges = self.edges.clone();
        for edge in &edges {
            let Some(&source) = self.nodes.get(&edge.sources[0]) else { continue };
            let Some(&target) = self.nodes.get(&edge.targets[0]) else { continue };
            let Some(holder_id) =
                self.parent.get(&self.slots[target].node.id).cloned().flatten()
            else {
                continue;
            };
            let Some(&holder) = self.nodes.get(&holder_id) else { continue };
            let source_is_composite =
                composites.iter().any(|c| c == &self.slots[source].node.id);
            let holder_is_composite =
                composites.iter().any(|c| c == &self.slots[holder].node.id);
            let follows = holder != source
                && source_is_composite
                && holder_is_composite
                && ranking.below(&self.slots[target].node.id, &self.slots[source].node.id);
            if !follows {
                continue;
            }
            let floor = self.slots[source].node.y.unwrap_or(0.0)
                + self.slots[source].node.height
                + 77.0;
            let drop = floor
                - self.slots[holder].node.y.unwrap_or(0.0)
                - self.slots[target].node.y.unwrap_or(0.0);
            if drop <= 0.0 {
                continue;
            }
            let ty = self.slots[target].node.y.unwrap_or(0.0);
            let lowered: Vec<usize> = self.slots[holder]
                .children
                .iter()
                .copied()
                .filter(|&c| self.slots[c].node.y.unwrap_or(0.0) >= ty)
                .collect();
            for &c in &lowered {
                self.slots[c].node.y = Some(self.slots[c].node.y.unwrap_or(0.0) + drop);
            }
            self.slots[holder].node.height += drop;
            moved.extend(lowered);
        }
        for node in moved {
            self.unroute(node);
        }
        Ok(())
    }

    /// A state that follows another composite stands below it; its
    /// container's ELK route to it is dropped.
    fn unroute(&mut self, node: usize) {
        let node_id = self.slots[node].node.id.clone();
        let Some(owner) = self.parent.get(&node_id).cloned().flatten() else { return };
        let Some(level) = self
            .levels
            .iter_mut()
            .find(|(key, _)| key.as_deref() == Some(owner.as_str()))
        else {
            return;
        };
        let Some(elk) = &mut level.1.elk else { return };
        for edge in &mut elk.edges {
            if edge.targets.clone() == vec![node_id.clone()] {
                edge.sections = None;
            }
        }
    }
}

// ---- axis_solver.rb ----

/// An item at a rank, `x` being its centre (Ruby `AxisSolver::Item`).
#[derive(Clone, Copy)]
struct SolverItem {
    rank: i64,
    width: f64,
    x: f64,
    fixed: bool,
    low: Option<f64>,
    high: Option<f64>,
    track: Option<u8>,
}

/// A link between two items, each joined at an offset from its centre.
struct SolverLink {
    from: usize,
    to: usize,
    from_offset: f64,
    to_offset: f64,
    weight: i64,
}

const SWEEPS: usize = 100;
const WARMUP: usize = 30;
const EPSILON: f64 = 1e-6;

/// Assigns the x of the items of a layered graph the way graphviz does:
/// it minimises the summed, weighted horizontal length of the links while
/// keeping the items of a rank apart by their widths and a gap.
struct AxisSolver {
    items: Vec<SolverItem>,
    gap: f64,
    floor: f64,
    chain_weight: i64,
    ranks: Vec<Vec<usize>>,
    row_of: HashMap<usize, usize>,
    links: HashMap<usize, Vec<usize>>,
    link_list: Vec<SolverLink>,
}

impl AxisSolver {
    fn new(
        items: Vec<SolverItem>,
        links: Vec<SolverLink>,
        gap: f64,
        floor: f64,
        chain_weight: i64,
    ) -> AxisSolver {
        // The order of a rank is the order of the items' starting x.
        let mut by_rank: HashMap<(i64, Option<u8>), Vec<usize>> = HashMap::new();
        for (i, item) in items.iter().enumerate() {
            by_rank.entry((item.rank, item.track)).or_default().push(i);
        }
        let mut ranks: Vec<Vec<usize>> = by_rank.into_values().collect();
        for row in &mut ranks {
            row.sort_by(|&a, &b| items[a].x.partial_cmp(&items[b].x).unwrap_or(std::cmp::Ordering::Equal));
        }
        let mut row_of: HashMap<usize, usize> = HashMap::new();
        for (r, row) in ranks.iter().enumerate() {
            for &item in row {
                row_of.insert(item, r);
            }
        }
        let mut by_item: HashMap<usize, Vec<usize>> = HashMap::new();
        for (l, link) in links.iter().enumerate() {
            by_item.entry(link.from).or_default().push(l);
            by_item.entry(link.to).or_default().push(l);
        }
        AxisSolver { items, gap, floor, chain_weight, ranks, row_of, links: by_item, link_list: links }
    }

    fn solve(&mut self) {
        for row in self.ranks.clone() {
            self.clear(&row);
        }
        for _ in 0..WARMUP {
            self.relax();
        }
        for _ in 0..SWEEPS {
            if !self.improved() {
                break;
            }
        }
    }

    fn relax(&mut self) {
        let rows = self.ranks.clone();
        for row in rows {
            for &item in &row {
                if self.links.contains_key(&item) {
                    let at = self.mean_pull(item);
                    self.put(item, at);
                }
            }
            self.clear(&row);
        }
    }

    fn mean_pull(&self, item: usize) -> f64 {
        let mut sum = 0.0;
        let mut weight_sum = 0.0;
        for &l in &self.links[&item] {
            let link = &self.link_list[l];
            let (at, weight) = if link.from == item {
                (self.items[link.to].x + link.to_offset - link.from_offset, link.weight as f64)
            } else {
                (self.items[link.from].x + link.from_offset - link.to_offset, link.weight as f64)
            };
            sum += at * weight;
            weight_sum += weight;
        }
        if weight_sum == 0.0 {
            0.0
        } else {
            sum / weight_sum
        }
    }

    /// Pushes the items of a rank apart, the fixed ones staying put.
    fn clear(&mut self, row: &[usize]) {
        for pair in row.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            let room = self.room(left, right);
            let at = self.items[right].x.max(self.items[left].x + room);
            self.put(right, at);
        }
        let mut rev = row.to_vec();
        rev.reverse();
        for pair in rev.windows(2) {
            let (right, left) = (pair[0], pair[1]);
            let room = self.room(left, right);
            let at = self.items[left].x.min(self.items[right].x - room);
            self.put(left, at);
        }
    }

    fn put(&mut self, item: usize, at: f64) {
        if self.items[item].fixed {
            return;
        }
        let (low, high) = self.bounds(item);
        self.items[item].x = at.clamp(low, high);
    }

    fn bounds(&self, item: usize) -> (f64, f64) {
        let item = &self.items[item];
        let low = item.low.unwrap_or(self.floor + item.width / 2.0);
        (low, item.high.unwrap_or(f64::INFINITY).max(low))
    }

    fn room(&self, left: usize, right: usize) -> f64 {
        ((self.items[left].width + self.items[right].width) / 2.0) + self.gap
    }

    fn improved(&mut self) -> bool {
        let all: Vec<usize> = self.ranks.iter().flatten().copied().collect();
        all.iter().any(|&item| self.improves(item))
    }

    fn improves(&mut self, item: usize) -> bool {
        // Ruby maps slides? over every seed x side, applying each
        // improving move before `.any?` answers.
        let mut any = false;
        for seed in self.seeds(item) {
            for side in [true, false] {
                if self.slides(&seed, side) {
                    any = true;
                }
            }
        }
        any
    }

    /// The sets of items that may move together from `item`: itself, and
    /// the whole chain of nodes a long transition has.
    fn seeds(&self, item: usize) -> Vec<Vec<usize>> {
        let chain = self.chain(item);
        let mut out = vec![vec![item]];
        if chain.len() > 1 {
            out.push(chain);
        }
        out
    }

    fn chain(&self, item: usize) -> Vec<usize> {
        let mut found = vec![item];
        let mut at = 0;
        while at < found.len() {
            let one = found[at];
            for other in self.tied(one) {
                if !found.contains(&other) {
                    found.push(other);
                }
            }
            at += 1;
        }
        found
    }

    fn tied(&self, item: usize) -> Vec<usize> {
        self.links
            .get(&item)
            .map(|ls| {
                ls.iter()
                    .filter(|&&l| self.link_list[l].weight == self.chain_weight)
                    .flat_map(|&l| [self.link_list[l].from, self.link_list[l].to])
                    .collect()
            })
            .unwrap_or_default()
    }

    /// How far the group may move and whether the move shortens its links.
    fn slides(&mut self, seed: &[usize], right: bool) -> bool {
        let mut group = self.pushed(seed, right);
        if group.iter().any(|&i| self.items[i].fixed) {
            return false;
        }
        let links = self.crossing(&group);
        let slack = self.slack(&group);
        let shift = best_shift(&links).clamp(slack.0, slack.1);
        if gain(&links, shift) <= 0.0 {
            return false;
        }
        for &item in &group {
            self.items[item].x += shift;
        }
        true
    }

    /// The seed and, toward `side`, the items it touches in a chain.
    fn pushed(&self, seed: &[usize], right: bool) -> Vec<usize> {
        let mut group: Vec<usize> = seed.to_vec();
        let mut at = 0;
        while at < group.len() {
            let item = group[at];
            let near = self.neighbour(item, right);
            if let Some(near) = near {
                if !group.contains(&near) && self.touching(item, near) {
                    group.push(near);
                }
            }
            at += 1;
        }
        group
    }

    fn neighbour(&self, item: usize, right: bool) -> Option<usize> {
        let row = &self.ranks[self.row_of[&item]];
        let at = row.iter().position(|&i| i == item)?;
        let next = if right { at + 1 } else { at.checked_sub(1)? };
        row.get(next).copied()
    }

    fn touching(&self, near: usize, far: usize) -> bool {
        (((self.items[near].x - self.items[far].x).abs()
            - self.room(near, far))
        .abs())
            < EPSILON
    }

    /// How far the group may move left (negative) and right.
    fn slack(&self, group: &[usize]) -> (f64, f64) {
        let lows = group
            .iter()
            .map(|&item| self.left_reach(item, group))
            .fold(f64::MIN, f64::max);
        let highs = group
            .iter()
            .map(|&item| self.right_reach(item, group))
            .fold(f64::MAX, f64::min);
        (lows.min(0.0), highs.max(0.0))
    }

    fn left_reach(&self, item: usize, group: &[usize]) -> f64 {
        let near = self.neighbour(item, false);
        let own = self.bounds(item).0 - self.items[item].x;
        match near {
            Some(near) if !group.contains(&near) => {
                own.max(self.items[near].x + self.room(near, item) - self.items[item].x)
            }
            _ => own,
        }
    }

    fn right_reach(&self, item: usize, group: &[usize]) -> f64 {
        let near = self.neighbour(item, true);
        let own = self.bounds(item).1 - self.items[item].x;
        match near {
            Some(near) if !group.contains(&near) => {
                own.min(self.items[near].x - self.room(item, near) - self.items[item].x)
            }
            _ => own,
        }
    }

    /// Each link that leaves the group as its gap and weight.
    fn crossing(&self, group: &[usize]) -> Vec<(f64, f64)> {
        let mut seen: Vec<usize> = vec![];
        let mut out = vec![];
        for &item in group {
            for &l in self.links.get(&item).map(|ls| ls.as_slice()).unwrap_or(&[]) {
                if seen.contains(&l) {
                    continue;
                }
                seen.push(l);
                let link = &self.link_list[l];
                let from_inside = group.contains(&link.from);
                let to_inside = group.contains(&link.to);
                if from_inside == to_inside {
                    continue;
                }
                let span = self.items[link.to].x + link.to_offset
                    - (self.items[link.from].x + link.from_offset);
                let gap = if from_inside { span } else { -span };
                out.push((gap, link.weight as f64));
            }
        }
        out
    }
}

fn best_shift(links: &[(f64, f64)]) -> f64 {
    if links.is_empty() {
        return 0.0;
    }
    let total: f64 = links.iter().map(|(_, w)| *w).sum();
    let mut ordered = links.to_vec();
    ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = 0.0;
    for (gap, weight) in &ordered {
        seen += weight;
        if seen * 2.0 >= total {
            return *gap;
        }
    }
    0.0
}

fn gain(links: &[(f64, f64)], shift: f64) -> f64 {
    cost(links, 0.0) - cost(links, shift)
}

fn cost(links: &[(f64, f64)], shift: f64) -> f64 {
    links.iter().map(|(gap, weight)| weight * (gap - shift).abs()).sum()
}


// ---- dot_axis.rb ----

/// Where graphviz puts the rows of one container along the x axis. It
/// ranks the states, gives every transition a node at each rank it
/// crosses, a label being the node at its middle, then places those nodes
/// so the transitions run as straight as they can.
const NODESEP: f64 = 35.0;
const CROSSING: f64 = 6.0;
const BAND: f64 = 4.0;
const DOT_MARGIN: f64 = 16.0;
const END_WEIGHT: i64 = 2;
const CROSSING_WEIGHT: i64 = 8;

/// One end of a segment of a transition: an item, the distance of the
/// joint from its centre, and whether it is a crossing.
#[derive(Clone, Copy)]
struct Joint {
    item: usize,
    offset: f64,
    crossing: bool,
}

pub struct DotAxis<'a> {
    edges: &'a [GEdge],
    nodes: &'a HashMap<String, usize>,
    parent: &'a HashMap<String, Option<String>>,
    notes: Notes,
    home: HashMap<String, usize>,
    row_ids: Vec<String>,
    centres: Vec<f64>,
    bands: Vec<f64>,
    items: HashMap<String, HashMap<i64, usize>>,
    entries: HashMap<String, usize>,
    solver_items: Vec<SolverItem>,
}

impl<'a> DotAxis<'a> {
    pub fn new(
        edges: &'a [GEdge],
        nodes: &'a HashMap<String, usize>,
        parent: &'a HashMap<String, Option<String>>,
        notes: Notes,
    ) -> DotAxis<'a> {
        DotAxis {
            edges,
            nodes,
            parent,
            notes,
            home: HashMap::new(),
            row_ids: vec![],
            centres: vec![],
            bands: vec![],
            items: HashMap::new(),
            entries: HashMap::new(),
            solver_items: vec![],
        }
    }

    /// Slides the rows to their place; returns the rows that moved.
    pub fn apply(&mut self, rows: &mut [Row], slots: &mut [Slot]) -> Vec<usize> {
        if !rows.iter().any(|row| Self::composite_row(row, slots)) {
            return vec![];
        }
        self.prepare(rows, slots);
        let mut links: Vec<SolverLink> = vec![];
        for edge in self.edges {
            links.extend(self.segments(edge, slots));
        }
        let items = std::mem::take(&mut self.solver_items);
        let floor = rows
            .iter()
            .map(|row| row.left(slots, &self.notes))
            .fold(f64::MAX, f64::min);
        let floor = if floor.is_finite() { floor } else { 0.0 };
        let mut solver = AxisSolver::new(items, links, NODESEP, floor, CROSSING_WEIGHT);
        solver.solve();
        self.solver_items = solver.items;
        let shifts: Vec<(usize, f64)> = (0..rows.len())
            .map(|r| (r, self.shift_of(r, rows, slots)))
            .filter(|(_, shift)| *shift != 0.0)
            .collect();
        for (r, shift) in &shifts {
            rows[*r].slide(slots, &self.notes, *shift);
        }
        shifts.into_iter().map(|(r, _)| r).collect()
    }

    fn prepare(&mut self, rows: &mut [Row], slots: &[Slot]) {
        self.home = HashMap::new();
        self.row_ids = rows.iter().map(|row| row.id.clone()).collect();
        self.centres = rows
            .iter()
            .map(|row| row.left(slots, &self.notes) + row.width(slots, &self.notes) / 2.0)
            .collect();
        for (r, row) in rows.iter().enumerate() {
            for &n in &row.nodes {
                self.home.insert(slots[n].node.id.clone(), r);
            }
        }
        // The distinct rows of states, top down: states whose centres are
        // a few points apart share one.
        let mut centres: Vec<f64> = rows
            .iter()
            .flat_map(|row| row.nodes.iter())
            .flat_map(|&n| self.leaf_centres(n, 0.0, slots))
            .collect();
        centres.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut bands: Vec<f64> = vec![];
        for c in centres {
            match bands.last() {
                Some(last) if c - *last <= BAND => {}
                _ => bands.push(c),
            }
        }
        self.bands = bands;
        self.items = HashMap::new();
        self.entries = HashMap::new();
        for r in 0..rows.len() {
            let row = Row { id: rows[r].id.clone(), nodes: rows[r].nodes.clone() };
            let items = self.items_of(&row, slots);
            if Self::composite_row(&row, slots) {
                let min_rank = items.keys().copied().min().unwrap_or(0);
                let entry = self.push_item(SolverItem {
                    rank: min_rank,
                    width: 0.0,
                    x: self.centres[r],
                    fixed: false,
                    low: Some(row.left(slots, &self.notes) + DOT_MARGIN),
                    high: Some(
                        row.left(slots, &self.notes)
                            + row.width(slots, &self.notes)
                            - DOT_MARGIN,
                    ),
                    track: Some(1),
                });
                self.entries.insert(rows[r].id.clone(), entry);
            }
            self.items.insert(rows[r].id.clone(), items);
        }
    }

    fn composite_row(row: &Row, slots: &[Slot]) -> bool {
        row.nodes.iter().any(|&n| !slots[n].children.is_empty())
    }

    fn anchored_row(row: &Row, slots: &[Slot]) -> bool {
        Self::composite_row(row, slots)
            || row
                .nodes
                .iter()
                .all(|&n| slots[n].node.meta.state_type == "start")
    }

    fn centre_of(&self, row: &Row, slots: &[Slot]) -> f64 {
        row.left(slots, &self.notes) + row.width(slots, &self.notes) / 2.0
    }

    /// The items of a row by rank: one for a row that moves, one for each
    /// rank a composite spans.
    fn items_of(&mut self, row: &Row, slots: &[Slot]) -> HashMap<i64, usize> {
        let band_numbers: Vec<i64> = row
            .nodes
            .iter()
            .flat_map(|&n| self.leaf_centres(n, 0.0, slots))
            .map(|centre| self.band_of(centre))
            .collect();
        let min = band_numbers.iter().copied().min().unwrap_or(0);
        let max = band_numbers.iter().copied().max().unwrap_or(0);
        let fixed = Self::anchored_row(row, slots);
        let centre = self.centres[self.row_ids.iter().position(|id| id == &row.id).unwrap_or(0)];
        let width = row.width(slots, &self.notes);
        ((2 * min)..=(2 * max))
            .map(|rank| {
                (
                    rank,
                    self.push_item(SolverItem {
                        rank,
                        width,
                        x: centre,
                        fixed,
                        low: None,
                        high: None,
                        track: None,
                    }),
                )
            })
            .collect()
    }

    fn push_item(&mut self, item: SolverItem) -> usize {
        self.solver_items.push(item);
        self.solver_items.len() - 1
    }

    /// The y of the centre of each state a node holds, or of the node
    /// itself when it holds none.
    fn leaf_centres(&self, node: usize, top: f64, slots: &[Slot]) -> Vec<f64> {
        let slot = &slots[node];
        let y = top + slot.node.y.unwrap_or(0.0);
        if slot.children.is_empty() {
            return vec![y + slot.node.height / 2.0];
        }
        slot.children
            .iter()
            .flat_map(|&c| self.leaf_centres(c, y, slots))
            .collect()
    }

    fn band_of(&self, centre: f64) -> i64 {
        self.bands
            .iter()
            .rposition(|band| *band <= centre + BAND)
            .map(|p| p as i64)
            .unwrap_or(0)
    }

    /// The links of the chain of nodes a transition makes between the
    /// ranks of its two ends, the upper one first.
    fn segments(&mut self, edge: &GEdge, slots: &[Slot]) -> Vec<SolverLink> {
        let (Some(sf), Some(st)) = (edge.sources.first(), edge.targets.first()) else {
            return vec![];
        };
        let keys: Vec<String> = self.home.keys().cloned().collect();
        if RoutesHelper::level_pair(edge, self.parent, &keys).is_none() {
            return vec![];
        }
        let mut ends = [self.joint(sf, slots), self.joint(st, slots)];
        ends.sort_by(|a, b| {
            self.solver_items[a.item]
                .rank
                .cmp(&self.solver_items[b.item].rank)
        });
        let (upper, lower) = (ends[0], ends[1]);
        if self.solver_items[upper.item].rank == self.solver_items[lower.item].rank {
            return vec![];
        }
        let first = self.solver_items[upper.item].rank;
        let last = self.solver_items[lower.item].rank;
        let mut joints = vec![upper];
        for rank in (first + 1)..last {
            let width = if rank == (first + last) / 2 {
                edge.labels.first().map(|l| l.width).unwrap_or(CROSSING)
            } else {
                CROSSING
            };
            let x = self.along(rank, &upper, &lower);
            let item = self.push_item(SolverItem {
                rank,
                width,
                x,
                fixed: false,
                low: None,
                high: None,
                track: None,
            });
            joints.push(Joint { item, offset: 0.0, crossing: true });
        }
        joints.push(lower);
        joints
            .windows(2)
            .map(|pair| {
                let weight = if pair[0].crossing && pair[1].crossing {
                    CROSSING_WEIGHT
                } else {
                    END_WEIGHT
                };
                SolverLink {
                    from: pair[0].item,
                    to: pair[1].item,
                    from_offset: pair[0].offset,
                    to_offset: pair[1].offset,
                    weight,
                }
            })
            .collect()
    }

    /// Where the straight line between two joints crosses a rank.
    fn along(&self, rank: i64, upper: &Joint, lower: &Joint) -> f64 {
        let top = self.solver_items[upper.item].rank;
        let by =
            (rank - top) as f64 / ((self.solver_items[lower.item].rank - top) as f64);
        self.spot(upper) + ((self.spot(lower) - self.spot(upper)) * by)
    }

    fn spot(&self, joint: &Joint) -> f64 {
        self.solver_items[joint.item].x + joint.offset
    }

    fn joint(&mut self, id: &str, slots: &[Slot]) -> Joint {
        let keys: Vec<String> = self.home.keys().cloned().collect();
        let member = member_of(id, self.parent, &keys).unwrap_or_else(|| id.to_string());
        let row = self.home[&member];
        let row_id = self.row_ids[row].clone();
        let children = self
            .nodes
            .get(id)
            .map(|&s| slots[s].children.clone())
            .unwrap_or_default();
        // A transition to or from a composite itself meets its cluster at
        // the entry, not at the cluster's box.
        if id == member && !children.is_empty() {
            let item = self.entries[&row_id];
            return Joint { item, offset: 0.0, crossing: false };
        }
        let items = &self.items[&row_id];
        let centre = self.level_at(id, &member, true, slots);
        let rank = self.band_of(centre);
        let (min, max) = (
            items.keys().copied().min().unwrap_or(0),
            items.keys().copied().max().unwrap_or(0),
        );
        let clamped = rank.clamp(min, max);
        let item = items[&clamped];
        let offset = self.level_at(id, &member, true, slots) - self.centres[row];
        Joint { item, offset, crossing: false }
    }

    /// The centre of a state along an axis, in the coordinates of the
    /// container its row stands in.
    fn level_at(&self, id: &str, member: &str, horizontal: bool, slots: &[Slot]) -> f64 {
        let mut id = id.to_string();
        let Some(&at) = self.nodes.get(&id) else { return 0.0 };
        let slot = &slots[at];
        let mut value = if horizontal {
            slot.node.x.unwrap_or(0.0) + slot.node.width / 2.0
        } else {
            slot.node.y.unwrap_or(0.0) + slot.node.height / 2.0
        };
        while id != member {
            let Some(owner) = self.parent.get(&id).cloned().flatten() else { break };
            let Some(&owner_slot) = self.nodes.get(&owner) else { break };
            let owner_node = &slots[owner_slot].node;
            value += if horizontal {
                owner_node.x.unwrap_or(0.0)
            } else {
                owner_node.y.unwrap_or(0.0)
            };
            id = owner;
        }
        value
    }

    fn shift_of(&self, r: usize, rows: &[Row], slots: &[Slot]) -> f64 {
        let row = &rows[r];
        let Some(row_items) = self.items.get(&row.id) else { return 0.0 };
        let Some(&first) = row_items.values().next() else { return 0.0 };
        let x = self.solver_items[first].x;
        let centre = self.centre_of(row, slots);
        ((x - centre) * 1000.0).round() / 1000.0
    }
}


