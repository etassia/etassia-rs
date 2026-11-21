//! Port of `activity/{ranking,columns,reach,row_spread,column_centres,
//! exits,row_gaps,legacy_row_gaps,bar_columns,lane_steps,lane_titles,
//! placement}.rb`: rows from the ranking, columns from the walk, the
//! spacing PlantUML leaves, and every node_box at its place.

use std::collections::HashMap;

use super::metrics::{ruby_sum, Box, Metrics};
use super::model::{Diagram, Direction, Kind};

/// A column key: the lane and the column. Columns are whole or half
/// numbers, so the pair hashes by exact bits and orders as Ruby compares
/// the arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    lane: i64,
    column: u64,
}

impl Key {
    pub fn new(lane: i64, column: f64) -> Self {
        Key {
            lane,
            column: column.to_bits(),
        }
    }

    pub fn lane(self) -> i64 {
        self.lane
    }

    pub fn column(self) -> f64 {
        f64::from_bits(self.column)
    }

    pub fn cmp_key(self, other: Key) -> std::cmp::Ordering {
        self.lane.cmp(&other.lane).then(
            // The values, not their bits: a negative half column sorts
            // before its neighbours.
            self.column()
                .partial_cmp(&other.column())
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    }
}

/// A row key: a rank, or half a rank past it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowKey(u64);

impl RowKey {
    pub fn new(row: f64) -> Self {
        RowKey(row.to_bits())
    }
}
/// Decides which edges are loops and which row each node sits on
/// (`Ranking`).
pub struct Ranking {
    diagram_legacy: bool,
    out: HashMap<usize, Vec<usize>>,
    back: Vec<usize>,
    levels: Vec<usize>,
    level_in: HashMap<usize, usize>,
    pub rank: HashMap<usize, i64>,
}

impl Ranking {
    pub fn new(diagram: &Diagram) -> Self {
        let mut out: HashMap<usize, Vec<usize>> = HashMap::new();
        for (index, edge) in diagram.edges.iter().enumerate() {
            out.entry(edge.from).or_default().push(index);
        }
        let mut ranking = Ranking {
            diagram_legacy: diagram.legacy,
            out,
            back: vec![],
            levels: vec![],
            level_in: Default::default(),
            rank: Default::default(),
        };
        ranking.walk(diagram);
        let constraints = ranking.constraints(diagram);
        let paths = ranking.longest_paths(diagram, constraints);
        ranking.rank = paths;
        ranking
    }

    /// Whether the edge closes a loop.
    pub fn back(&self, edge: usize) -> bool {
        self.back.contains(&edge)
    }

    /// Edges leaving the node, loops excluded.
    pub fn forward(&self, node: usize) -> Vec<usize> {
        self.out
            .get(&node)
            .map(|edges| {
                edges
                    .iter()
                    .copied()
                    .filter(|edge| !self.back(*edge))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the node sits between two rows: in the legacy syntax, an
    /// `->` target whose own arrow runs two rows or more further down is
    /// drawn half a row below its source.
    pub fn parked(&self, diagram: &Diagram, node: usize) -> bool {
        self.diagram_legacy
            && self.level_in.contains_key(&node)
            && self
                .forward(node)
                .iter()
                .any(|edge| self.rank[&diagram.edges[*edge].to] - self.rank[&node] >= 2)
    }

    /// The row a node's node_box lies on: a parked node's is half a row below
    /// its rank.
    pub fn row(&self, diagram: &Diagram, node: usize) -> f64 {
        if self.parked(diagram, node) {
            self.rank[&node] as f64 + 0.5
        } else {
            self.rank[&node] as f64
        }
    }

    /// The edge that puts the node beside another.
    pub fn level_in(&self, node: usize) -> Option<usize> {
        self.level_in.get(&node).copied()
    }

    fn walk(&mut self, diagram: &Diagram) {
        let mut state: HashMap<usize, u8> = HashMap::new();
        for node in 0..diagram.nodes.len() {
            if state.get(&node).copied().unwrap_or(0) == 0 {
                self.visit(diagram, node, &mut state);
            }
        }
    }

    fn visit(&mut self, diagram: &Diagram, node: usize, state: &mut HashMap<usize, u8>) {
        state.insert(node, 1);
        let edges = self.out.get(&node).cloned().unwrap_or_default();
        for edge in edges {
            match state.get(&diagram.edges[edge].to).copied().unwrap_or(0) {
                1 => self.back.push(edge),
                0 => self.visit(diagram, diagram.edges[edge].to, state),
                _ => {}
            }
        }
        state.insert(node, 2);
    }

    /// Every forward edge, and for each loop the exits of the loop head
    /// that cannot reach the loop's body.
    fn constraints(&mut self, diagram: &Diagram) -> Vec<(usize, usize)> {
        self.find_levels(diagram);
        let mut pairs: Vec<(usize, usize)> = self.forward_pairs(diagram);
        let back = self.back.clone();
        for edge in back {
            pairs.extend(self.exit_pairs(diagram, edge));
        }
        pairs
    }

    fn forward_pairs(&self, diagram: &Diagram) -> Vec<(usize, usize)> {
        diagram
            .edges
            .iter()
            .enumerate()
            .filter(|(index, edge)| !self.back(*index) && !edge.level())
            .map(|(_, edge)| (edge.from, edge.to))
            .collect()
    }

    fn find_levels(&mut self, diagram: &Diagram) {
        self.levels = diagram
            .edges
            .iter()
            .enumerate()
            .filter(|(index, edge)| edge.level() && !self.back(*index))
            .map(|(index, _)| index)
            .collect();
        self.level_in = Default::default();
        for edge in self.levels.iter().rev() {
            self.level_in.insert(diagram.edges[*edge].to, *edge);
        }
    }

    fn exit_pairs(&self, diagram: &Diagram, loop_edge: usize) -> Vec<(usize, usize)> {
        self.forward(diagram.edges[loop_edge].to)
            .iter()
            .filter_map(|exit_edge| {
                let exit_to = diagram.edges[*exit_edge].to;
                if self.reaches(
                    diagram,
                    exit_to,
                    diagram.edges[loop_edge].from,
                    &mut Vec::new(),
                ) {
                    return None;
                }
                Some((diagram.edges[loop_edge].from, exit_to))
            })
            .collect()
    }

    fn reaches(
        &self,
        diagram: &Diagram,
        from: usize,
        target: usize,
        seen: &mut Vec<usize>,
    ) -> bool {
        if from == target {
            return true;
        }
        if seen.contains(&from) {
            return false;
        }
        seen.push(from);
        self.forward(from)
            .iter()
            .any(|edge| self.reaches(diagram, diagram.edges[*edge].to, target, seen))
    }

    fn longest_paths(&self, diagram: &Diagram, pairs: Vec<(usize, usize)>) -> HashMap<usize, i64> {
        let mut rank: HashMap<usize, i64> =
            (0..diagram.nodes.len()).map(|node| (node, 0)).collect();
        for _ in 0..=diagram.nodes.len() {
            if self.settle(diagram, &mut rank, &pairs) == 0 {
                break;
            }
        }
        rank
    }

    /// One pass over the constraints; the number of nodes it moved.
    fn settle(
        &self,
        diagram: &Diagram,
        rank: &mut HashMap<usize, i64>,
        pairs: &[(usize, usize)],
    ) -> usize {
        let mut moved = 0;
        for (from, to) in pairs {
            moved += self.lift(rank, *from, *to) as usize;
        }
        for edge in &self.levels {
            moved += self.level(rank, diagram.edges[*edge].from, diagram.edges[*edge].to) as usize;
        }
        moved
    }

    /// Puts both nodes on the lower of their two rows.
    fn level(&self, rank: &mut HashMap<usize, i64>, first: usize, second: usize) -> bool {
        if rank[&first] == rank[&second] {
            return false;
        }
        let shared = rank[&first].max(rank[&second]);
        rank.insert(first, shared);
        rank.insert(second, shared);
        true
    }

    /// Puts `to` one row below `from`.
    fn lift(&self, rank: &mut HashMap<usize, i64>, from: usize, to: usize) -> bool {
        if rank[&to] > rank[&from] {
            return false;
        }
        rank.insert(to, rank[&from] + 1);
        true
    }
}

/// Gives each node a column (`Columns`).
pub struct Columns<'a> {
    ranking: &'a Ranking,
    /// Lane index and column inside the lane, per node.
    pub key: HashMap<usize, Key>,
    /// For each decision that splits into side by side branches, its
    /// first and last branch.
    pub spans: HashMap<usize, (usize, usize)>,
    taken: Vec<(Key, i64)>,
    trunks: Vec<Key>,
    last: HashMap<Key, i64>,
    widest: HashMap<i64, i64>,
    joins: HashMap<usize, f64>,
    after: HashMap<usize, usize>,
    wide: Vec<usize>,
    open: Vec<usize>,
}

impl<'a> Columns<'a> {
    pub fn new(diagram: &Diagram, ranking: &'a Ranking) -> Self {
        let mut columns = Columns {
            ranking,
            key: Default::default(),
            spans: Default::default(),
            taken: vec![],
            trunks: vec![],
            last: Default::default(),
            widest: Default::default(),
            joins: Default::default(),
            after: Default::default(),
            wide: vec![],
            open: vec![],
        };
        columns.place_all(diagram);
        columns
    }

    /// Whether the decision splits three or more ways.
    pub fn wide(&self, node: usize) -> bool {
        self.wide.contains(&node)
    }

    fn place_all(&mut self, diagram: &Diagram) {
        let mut pending: Vec<usize> = (0..diagram.nodes.len()).collect();
        while !pending.is_empty() {
            let node = pending
                .iter()
                .copied()
                .find(|node| self.ready(diagram, *node))
                .unwrap_or(pending[0]);
            if !self.key.contains_key(&node) {
                let preferred = self.beside(diagram, node);
                self.place(diagram, node, preferred);
            }
            pending.retain(|node| !self.key.contains_key(node));
        }
    }

    /// A node reached along a row waits for the node it lies beside.
    fn ready(&self, diagram: &Diagram, node: usize) -> bool {
        match self.ranking.level_in(node) {
            None => true,
            Some(level) => self.key.contains_key(&diagram.edges[level].from),
        }
    }

    fn beside(&self, diagram: &Diagram, node: usize) -> f64 {
        let Some(edge) = self.ranking.level_in(node) else {
            return 0.0;
        };
        let Some(&found) = self.key.get(&diagram.edges[edge].from) else {
            return 0.0;
        };
        let (lane, column) = (found.lane(), found.column());
        let _ = lane;
        let step = if self.open.contains(&diagram.edges[edge].from) {
            0.5
        } else {
            1.0
        };
        if diagram.edges[edge].direction == Some(Direction::Left) {
            column - step
        } else {
            column + step
        }
    }

    fn place(&mut self, diagram: &Diagram, node: usize, preferred: f64) {
        let lane = Self::lane_of(diagram, node);
        let rank = self.ranking.rank[&node];
        let column = self.free_column(lane, preferred, rank);
        self.key.insert(node, Key::new(lane, column));
        let widest = self.widest.entry(lane).or_insert(-1);
        *widest = (*widest).max(column.ceil() as i64);
        let slot = self.last.entry(Key::new(lane, column)).or_insert(-1);
        *slot = (*slot).max(rank);
        self.hang_branches(diagram, node, lane, column);
        self.place_children(diagram, node, lane, column);
    }

    fn place_children(&mut self, diagram: &Diagram, node: usize, lane: i64, column: f64) {
        let heads = self.spans.get(&node).copied();
        let mut keep = heads.is_none();
        for edge in self.ranking.forward(node) {
            let child = diagram.edges[edge].to;
            if self.placed_or_held(diagram, edge) {
                continue;
            }
            let preferred = self.preference(diagram, child, lane, column, keep, node, heads);
            self.place(diagram, child, preferred);
            keep = false;
        }
        self.close_branches(diagram, node, column);
    }

    fn placed_or_held(&self, diagram: &Diagram, edge: usize) -> bool {
        let edge = &diagram.edges[edge];
        self.key.contains_key(&edge.to) || edge.level() || self.joins.contains_key(&edge.to)
    }

    fn preference(
        &self,
        diagram: &Diagram,
        child: usize,
        lane: i64,
        column: f64,
        keep: bool,
        node: usize,
        heads: Option<(usize, usize)>,
    ) -> f64 {
        if let Some((first, _)) = heads {
            if child == first {
                return column - 0.5;
            }
        }
        if Self::lane_of(diagram, child) != lane {
            return 0.0;
        }
        if keep {
            return column;
        }
        if heads.is_some() {
            return self.room_right(lane, column, child, self.open.contains(&node));
        }
        self.widest.get(&lane).copied().unwrap_or(-1) as f64 + 1.0
    }

    /// The nearest column on the right that no step of an earlier part of
    /// the flow still reaches down to, and that no decision hangs from.
    fn room_right(&self, lane: i64, column: f64, child: usize, adjacent: bool) -> f64 {
        let rank = self.ranking.rank[&child];
        let mut found = if adjacent {
            column + 0.5
        } else {
            column.floor() + 1.0
        };
        while self.trunks.contains(&Key::new(lane, found))
            || self.last.get(&Key::new(lane, found)).copied().unwrap_or(-1) >= rank
            || self.taken.contains(&(Key::new(lane, found), rank))
        {
            found += 1.0;
        }
        found
    }

    fn close_branches(&mut self, diagram: &Diagram, node: usize, column: f64) {
        let join = match self.after.get(&node).copied() {
            Some(join) if !self.key.contains_key(&join) => join,
            _ => return,
        };
        self.joins.remove(&join);
        self.place(diagram, join, column);
    }

    /// A decision with two or more branches of its own keeps its column
    /// for the flow that runs through it.
    fn hang_branches(&mut self, diagram: &Diagram, node: usize, lane: i64, column: f64) {
        let heads: Vec<usize> = self
            .ranking
            .forward(node)
            .iter()
            .map(|edge| diagram.edges[*edge].to)
            .collect();
        let join = self.split_join(diagram, node, &heads);
        let Some(join) = join else {
            return self.open_branches(diagram, node, &heads, lane, column);
        };
        if Self::lane_of(diagram, join) != lane {
            return;
        }
        self.hang(node, &heads, lane, column);
        self.joins.insert(join, column);
        self.after.insert(node, join);
    }

    /// A legacy decision of two steps that never meet, one of them parked
    /// beside it, hangs the same way: the steps stand either side of it,
    /// the parked one a half column away.
    fn open_branches(
        &mut self,
        diagram: &Diagram,
        node: usize,
        heads: &[usize],
        lane: i64,
        column: f64,
    ) {
        if !self.parked_pair(diagram, node, heads) {
            return;
        }
        self.open.push(node);
        self.hang(node, heads, lane, column);
    }

    fn parked_pair(&self, diagram: &Diagram, node: usize, heads: &[usize]) -> bool {
        diagram.legacy
            && self.forks(diagram, node, heads)
            && heads.len() == 2
            && heads
                .iter()
                .all(|head| diagram.nodes[*head].kind == Kind::Action)
            && heads.iter().any(|head| self.ranking.parked(diagram, *head))
    }

    fn hang(&mut self, node: usize, heads: &[usize], lane: i64, column: f64) {
        self.spans.insert(node, (heads[0], heads[heads.len() - 1]));
        if heads.len() > 2 {
            self.wide.push(node);
        }
        self.trunks.push(Key::new(lane, column));
    }

    /// The merge that closes a decision with two or more branches of its
    /// own, when it has one.
    fn split_join(&self, diagram: &Diagram, node: usize, heads: &[usize]) -> Option<usize> {
        if !self.forks(diagram, node, heads) {
            return None;
        }
        let join = self.join_of(diagram, heads)?;
        (diagram.nodes[join].kind == Kind::Merge).then_some(join)
    }

    fn forks(&self, diagram: &Diagram, node: usize, heads: &[usize]) -> bool {
        let is_decision = node != usize::MAX && diagram.nodes[node].kind == Kind::Decision;
        is_decision
            && heads.len() > 1
            && heads
                .iter()
                .all(|head| diagram.nodes[*head].kind != Kind::Merge)
    }

    /// The first node every branch reaches, nearest first.
    fn join_of(&self, diagram: &Diagram, heads: &[usize]) -> Option<usize> {
        let mut common: Vec<usize> = Vec::new();
        for (index, head) in heads.iter().enumerate() {
            let seen = self.downstream(diagram, *head, vec![]);
            if index == 0 {
                common = seen;
            } else {
                common.retain(|node| seen.contains(node));
            }
        }
        common
            .into_iter()
            .min_by_key(|node| self.ranking.rank[node])
    }

    fn downstream(&self, diagram: &Diagram, node: usize, mut seen: Vec<usize>) -> Vec<usize> {
        if seen.contains(&node) {
            return seen;
        }
        seen.push(node);
        for edge in self.ranking.forward(node) {
            seen = self.downstream(diagram, diagram.edges[edge].to, seen);
        }
        seen
    }

    fn free_column(&mut self, lane: i64, preferred: f64, rank: i64) -> f64 {
        let mut column = preferred;
        while self.taken.contains(&(Key::new(lane, column), rank)) {
            column += 1.0;
        }
        self.taken.push((Key::new(lane, column), rank));
        column
    }

    fn lane_of(diagram: &Diagram, node: usize) -> i64 {
        match &diagram.nodes[node].lane {
            Some(lane) => diagram
                .lanes
                .iter()
                .position(|(name, _)| name == lane)
                .map(|at| at as i64)
                .unwrap_or(-1),
            None => -1,
        }
    }
}

/// How far each column reaches left and right of its centre (`Reach`).
pub struct Reach {
    /// Whether the columns clear each other row by row rather than as
    /// wholes.
    pub by_row: bool,
    /// The whole column.
    pub columns: HashMap<Key, (f64, f64)>,
    /// Per row.
    pub rows: HashMap<Key, HashMap<RowKey, (f64, f64)>>,
}

impl Reach {
    pub fn new(keys: &[Key], by_row: bool) -> Self {
        Reach {
            by_row,
            columns: keys.iter().map(|key| (*key, (0.0, 0.0))).collect(),
            rows: keys.iter().map(|key| (*key, HashMap::new())).collect(),
        }
    }

    /// Widens a column, on one row, to at least `value` on a side.
    pub fn grow(&mut self, key: Key, row: f64, side: usize, value: f64) {
        let column = self.columns.entry(key).or_insert((0.0, 0.0));
        if side == 0 {
            column.0 = column.0.max(value);
        } else {
            column.1 = column.1.max(value);
        }
        let row_reach = self
            .rows
            .entry(key)
            .or_default()
            .entry(RowKey::new(row))
            .or_insert((0.0, 0.0));
        if side == 0 {
            row_reach.0 = row_reach.0.max(value);
        } else {
            row_reach.1 = row_reach.1.max(value);
        }
    }

    /// Widens a whole column, and every row it has, by `extra` on a side.
    pub fn widen_column(&mut self, key: Key, side: usize, extra: f64) {
        if let Some(column) = self.columns.get_mut(&key) {
            if side == 0 {
                column.0 += extra;
            } else {
                column.1 += extra;
            }
        }
        if let Some(rows) = self.rows.get_mut(&key) {
            for reach in rows.values_mut() {
                if side == 0 {
                    reach.0 += extra;
                } else {
                    reach.1 += extra;
                }
            }
        }
    }

    /// Stands a line on a column's rows, as wide as nothing.
    pub fn line(&mut self, key: Key, rows: impl Iterator<Item = f64>) {
        for row in rows {
            self.grow(key, row, 0, 0.0);
        }
    }
}

/// Stands columns left to right, each clear of every earlier one only on
/// the rows the two share (`RowSpread`).
pub struct RowSpread<'a> {
    rows: &'a HashMap<Key, HashMap<RowKey, (f64, f64)>>,
    widths: &'a HashMap<Key, (f64, f64)>,
    margin: f64,
}

impl<'a> RowSpread<'a> {
    pub fn new(
        rows: &'a HashMap<Key, HashMap<RowKey, (f64, f64)>>,
        widths: &'a HashMap<Key, (f64, f64)>,
        margin: f64,
    ) -> Self {
        RowSpread {
            rows,
            widths,
            margin,
        }
    }

    /// Each column's centre, the `gap` closure standing in for the caller.
    pub fn call(&self, solid: &[Key], gap: &dyn Fn(Key, Key) -> f64) -> HashMap<Key, f64> {
        let mut placed: HashMap<Key, f64> = HashMap::new();
        for (index, key) in solid.iter().enumerate() {
            placed.insert(*key, self.centre(&solid[..index], *key, &placed, gap));
        }
        placed
    }

    fn centre(
        &self,
        earlier: &[Key],
        key: Key,
        placed: &HashMap<Key, f64>,
        gap: &dyn Fn(Key, Key) -> f64,
    ) -> f64 {
        let mut clear: Vec<f64> = earlier
            .iter()
            .enumerate()
            .map(|(at, prior)| {
                let g = gap(*prior, key);
                placed[prior] + self.clearance(*prior, key, g, at == earlier.len() - 1)
            })
            .collect();
        clear.push(self.margin + self.widths[&key].0);
        clear.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }

    /// How far right of `prior` the column's centre stands.
    fn clearance(&self, prior: Key, key: Key, gap: f64, adjacent: bool) -> f64 {
        let shared: Vec<RowKey> = self
            .rows
            .get(&prior)
            .map(|rows| {
                rows.keys()
                    .copied()
                    .filter(|row| {
                        self.rows
                            .get(&key)
                            .map(|rows| rows.contains_key(row))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        if shared.is_empty() {
            return if adjacent { 0.0 } else { f64::NEG_INFINITY };
        }
        let reach = shared
            .iter()
            .map(|row| self.rows[&prior][row].1 + self.rows[&key][row].0)
            .fold(f64::NEG_INFINITY, f64::max);
        reach + gap
    }
}

/// Where each column's centre falls (`ColumnCentres`).
pub struct ColumnCentres<'a> {
    columns: &'a Columns<'a>,
    node_boxes: &'a HashMap<usize, Box>,
    reach: &'a Reach,
    margin: f64,
    bars: &'a BarColumns<'a>,
    trunks: HashMap<Key, Vec<usize>>,
}

const COLUMN_GAP: f64 = 20.0;
const LANE_GAP: f64 = 30.0;
/// What a label's far end adds to the block a wide decision centres on.
const LABEL_PAD: f64 = 3.5;

impl<'a> ColumnCentres<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        columns: &'a Columns<'a>,
        node_boxes: &'a HashMap<usize, Box>,
        reach: &'a Reach,
        margin: f64,
        bars: &'a BarColumns<'a>,
    ) -> Self {
        let mut trunks: HashMap<Key, Vec<usize>> = HashMap::new();
        for node in columns.spans.keys() {
            trunks.entry(columns.key[node]).or_default().push(*node);
        }
        ColumnCentres {
            columns,
            node_boxes,
            reach,
            margin,
            bars,
            trunks,
        }
    }

    /// Every column key's centre, in order.
    pub fn call(&self, keys: &[Key]) -> HashMap<Key, f64> {
        let solid: Vec<Key> = keys
            .iter()
            .copied()
            .filter(|key| !self.trunks.contains_key(key))
            .collect();
        let placed = self.settle(&solid);
        keys.iter()
            .map(|key| {
                (
                    *key,
                    placed
                        .get(key)
                        .copied()
                        .unwrap_or_else(|| self.trunk_centre(*key, &placed, &mut vec![])),
                )
            })
            .collect()
    }

    /// Spreads the solid columns, moving some right until every decision's
    /// branches stand far enough apart.
    fn settle(&self, solid: &[Key]) -> HashMap<Key, f64> {
        let mut extra: HashMap<Key, f64> = HashMap::new();
        let mut placed = self.spread(solid, &extra);
        for _ in 0..(solid.len() * 2) {
            let deficit = match self.branch_deficit(&placed, solid) {
                Some(deficit) => Some(deficit),
                None => self.lane_deficit(&placed, solid),
            };
            let Some((key, short)) = deficit else {
                break;
            };
            *extra.entry(key).or_insert(0.0) += short;
            placed = self.spread(solid, &extra);
        }
        placed
    }

    fn spread(&self, solid: &[Key], extra: &HashMap<Key, f64>) -> HashMap<Key, f64> {
        let gap = |prior: Key, key: Key| self.gap(prior, key, extra);
        if self.reach.by_row {
            RowSpread::new(&self.reach.rows, &self.reach.columns, self.margin).call(solid, &gap)
        } else {
            self.spread_columns(solid, extra)
        }
    }

    fn spread_columns(&self, solid: &[Key], extra: &HashMap<Key, f64>) -> HashMap<Key, f64> {
        let mut cursor = self.margin;
        let mut placed = HashMap::new();
        for (index, key) in solid.iter().enumerate() {
            if index > 0 {
                cursor += self.gap(solid[index - 1], *key, extra);
            }
            let centre = cursor + self.reach.columns[key].0;
            cursor = centre + self.reach.columns[key].1;
            placed.insert(*key, centre);
        }
        placed
    }

    fn gap(&self, previous: Key, key: Key, extra: &HashMap<Key, f64>) -> f64 {
        COLUMN_GAP
            + if previous.lane() == key.lane() {
                0.0
            } else {
                LANE_GAP
            }
            + extra.get(&key).copied().unwrap_or(0.0)
    }

    /// The first column that has to move right so that some decision's
    /// branches stand apart, and by how much.
    fn branch_deficit(&self, placed: &HashMap<Key, f64>, solid: &[Key]) -> Option<(Key, f64)> {
        for (node, (first, last)) in &self.columns.spans {
            let short = self.shortfall(*node, *first, *last, placed);
            let key_first = self.columns.key[first];
            // `[a, b].max` compares the keys lexicographically.
            let edge = self
                .bars
                .fan_keys(*first)
                .unwrap_or_else(|| vec![key_first])
                .into_iter()
                .max_by(|a, b| a.cmp_key(*b))
                .unwrap_or(key_first);
            let anchor = solid
                .iter()
                .copied()
                .find(|key| key.cmp_key(edge) == std::cmp::Ordering::Greater);
            if short > 0.0 {
                if let Some(anchor) = anchor {
                    return Some((anchor, short));
                }
            }
        }
        None
    }

    /// The first column of a lane that has to move right so that the lane
    /// before it ends before its band begins.
    fn lane_deficit(&self, placed: &HashMap<Key, f64>, solid: &[Key]) -> Option<(Key, f64)> {
        for pair in solid.windows(2) {
            let (prior, key) = (pair[0], pair[1]);
            if prior.lane() == key.lane() {
                continue;
            }
            let short = self.lane_reach(prior.lane(), placed) + (2.0 * BAND_PAD)
                - (placed[&key] - self.reach.columns[&key].0);
            if short > 0.0 {
                return Some((key, short));
            }
        }
        None
    }

    fn lane_reach(&self, lane: i64, placed: &HashMap<Key, f64>) -> f64 {
        let mut keys: Vec<Key> = placed.keys().copied().collect();
        keys.extend(self.trunks.keys().copied());
        keys.retain(|key| key.lane() == lane);
        keys.iter()
            .map(|key| self.centre(*key, placed, &mut vec![]) + self.reach.columns[key].1)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn shortfall(&self, node: usize, left: usize, right: usize, placed: &HashMap<Key, f64>) -> f64 {
        self.node_boxes[&node].width + COLUMN_GAP
            - (self.head_centre(right, placed, &mut vec![])
                - self.head_centre(left, placed, &mut vec![]))
    }

    fn centre(&self, key: Key, placed: &HashMap<Key, f64>, seen: &mut Vec<Key>) -> f64 {
        placed
            .get(&key)
            .copied()
            .unwrap_or_else(|| self.trunk_centre(key, placed, seen))
    }

    /// Where the arrow into a branch lands: a bar takes it midway between
    /// the branches it fans out to.
    fn head_centre(&self, node: usize, placed: &HashMap<Key, f64>, seen: &mut Vec<Key>) -> f64 {
        let keys = self.bars.fan_keys(node);
        let Some(keys) = keys else {
            return self.centre(self.columns.key[&node], placed, seen);
        };
        let centres: Vec<f64> = keys
            .iter()
            .map(|key| self.centre(*key, placed, seen))
            .collect();
        let low = centres.iter().copied().fold(f64::INFINITY, f64::min);
        let high = centres.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (low + high) / 2.0
    }

    fn trunk_centre(&self, key: Key, placed: &HashMap<Key, f64>, seen: &mut Vec<Key>) -> f64 {
        if seen.contains(&key) {
            return 0.0;
        }
        seen.push(key);
        let mids: Vec<f64> = self
            .trunks
            .get(&key)
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|node| self.mid(*node, placed, seen))
                    .collect()
            })
            .unwrap_or_default();
        if mids.is_empty() {
            0.0
        } else {
            ruby_sum(&mids) / mids.len() as f64
        }
    }

    fn mid(&self, node: usize, placed: &HashMap<Key, f64>, seen: &mut Vec<Key>) -> f64 {
        let (first, last) = self.columns.spans[&node];
        let left = self.head_centre(first, placed, seen);
        let right = self.head_centre(last, placed, seen);
        if !self.columns.wide(node) {
            return (left + right) / 2.0;
        }
        let block = self.block(first, last, [left, right]);
        ruby_sum(&block) / 2.0
    }

    /// The edges of the steps a wide decision's branches stand in, the
    /// outer labels' width counted.
    fn block(&self, first: usize, last: usize, heads: [f64; 2]) -> Vec<f64> {
        vec![
            heads[0] - self.reach.columns[&self.columns.key[&first]].0,
            heads[1] + self.reach.columns[&self.columns.key[&last]].1 + LABEL_PAD,
        ]
    }
}

/// The lexicographic order of two column keys, as Ruby compares arrays.
pub(crate) fn column_cmp(a: Key, b: Key) -> std::cmp::Ordering {
    a.cmp_key(b)
}

/// How an arrow leaves its node, once the columns are centred (`Exits`).
pub struct Exits<'a> {
    columns: &'a Columns<'a>,
    boxes: &'a HashMap<usize, Box>,
}

impl<'a> Exits<'a> {
    pub fn new(columns: &'a Columns<'a>, boxes: &'a HashMap<usize, Box>) -> Self {
        Exits { columns, boxes }
    }

    /// A step that stands over the diamond closing the branches drops into
    /// its top instead of its side; a bar runs round to the side.
    pub fn top_entry(from: &Box, to: &Box, diagram: &Diagram, edge: usize) -> bool {
        top_entry(from, to, diagram, edge)
    }

    /// Whether the arrow runs beside its row: out of a decision's side, or
    /// into the side of the diamond that closes branches.
    pub fn beside(&self, diagram: &Diagram, edge: usize) -> bool {
        !self.same_column(diagram, edge)
            && !self.under(diagram, edge)
            && self.sideways(diagram, edge)
    }

    /// Whether the arrow leaves a decision's foot and has to jog sideways
    /// to reach its step.
    pub fn jog(&self, diagram: &Diagram, edge: usize) -> bool {
        self.under(diagram, edge) && !self.same_column(diagram, edge)
    }

    fn same_column(&self, diagram: &Diagram, edge: usize) -> bool {
        self.columns.key[&diagram.edges[edge].from] == self.columns.key[&diagram.edges[edge].to]
    }

    fn sideways(&self, diagram: &Diagram, edge: usize) -> bool {
        let (from, to) = (
            &self.boxes[&diagram.edges[edge].from],
            &self.boxes[&diagram.edges[edge].to],
        );
        if diagram.edges[edge].to_kind(diagram) != Kind::Merge {
            return diagram.edges[edge].from_kind(diagram) == Kind::Decision;
        }
        !Self::top_entry(from, to, diagram, edge)
    }

    fn under(&self, diagram: &Diagram, edge: usize) -> bool {
        let from = &self.boxes[&diagram.edges[edge].from];
        let to = &self.boxes[&diagram.edges[edge].to];
        diagram.edges[edge].from_kind(diagram) == Kind::Decision
            && (to.cx - from.cx).abs() < from.width / 2.0
    }
}

/// A step that stands over the diamond closing the branches drops into
/// its top instead of its side; a bar runs round to the side.
/// (`Exits.top_entry?`)
pub fn top_entry(from: &Box, to: &Box, diagram: &Diagram, edge: usize) -> bool {
    let edge_ref = &diagram.edges[edge];
    diagram.nodes[edge_ref.to].kind == Kind::Merge
        && diagram.nodes[edge_ref.from].kind != Kind::Bar
        && from.spans(to.cx)
}

/// A tiny indirection so `Exits::top_entry` reads like the Ruby.
trait EdgeKinds {
    fn to_kind(&self, diagram: &Diagram) -> Kind;
    fn from_kind(&self, diagram: &Diagram) -> Kind;
}

impl EdgeKinds for super::model::Edge {
    fn to_kind(&self, diagram: &Diagram) -> Kind {
        diagram.nodes[self.to].kind
    }

    fn from_kind(&self, diagram: &Diagram) -> Kind {
        diagram.nodes[self.from].kind
    }
}

const STEP: f64 = 20.0;
const BRANCH: f64 = 10.0;
const CLOSE: f64 = 6.0;
const BAR_CLOSE: f64 = 23.1328;
const LABEL_MARGIN: f64 = 28.33;
const LABEL_BELOW_DECISION: f64 = 11.45;

/// The space above each row, as PlantUML leaves it (`RowGaps`).
pub struct RowGaps<'a> {
    asks: Vec<Vec<f64>>,
    ranking: &'a Ranking,
    exits: &'a Exits<'a>,
    metrics: &'a Metrics<'a>,
    frame: f64,
}

impl<'a> RowGaps<'a> {
    pub fn new(
        diagram: &Diagram,
        ranking: &'a Ranking,
        exits: &'a Exits<'a>,
        metrics: &'a Metrics<'a>,
        frame: f64,
    ) -> Self {
        let rows = ranking
            .rank
            .values()
            .copied()
            .max()
            .map(|max| max as usize + 1)
            .unwrap_or(1);
        let mut gaps = RowGaps {
            asks: vec![vec![]; rows],
            ranking,
            exits,
            metrics,
            frame,
        };
        for index in 0..diagram.edges.len() {
            gaps.add(diagram, index);
        }
        gaps
    }

    /// The space above each row; the first is 0.
    pub fn to_a(&self) -> Vec<f64> {
        self.asks
            .iter()
            .enumerate()
            .map(|(row, own)| {
                if row == 0 {
                    0.0
                } else {
                    own.iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max)
                        .max_finite_or(STEP)
                }
            })
            .collect()
    }

    fn add(&mut self, diagram: &Diagram, edge: usize) {
        if self.ranking.back(edge) {
            return;
        }
        let row = self.ranking.rank[&diagram.edges[edge].to];
        if row - self.ranking.rank[&diagram.edges[edge].from] != 1 {
            return;
        }
        let ask = self.crossing(diagram, edge).max(self.arrow(diagram, edge));
        self.asks[row as usize].push(ask);
    }

    fn crossing(&self, diagram: &Diagram, edge: usize) -> f64 {
        if diagram.nodes[diagram.edges[edge].from].partition
            == diagram.nodes[diagram.edges[edge].to].partition
        {
            0.0
        } else {
            self.frame
        }
    }

    /// Where the arrow goes decides how far it has to run.
    fn arrow(&self, diagram: &Diagram, edge: usize) -> f64 {
        let height = self.label_height(diagram, edge);
        if self.exits.beside(diagram, edge) {
            return self.side_run(diagram, edge, height);
        }
        if diagram.nodes[diagram.edges[edge].from].kind == Kind::Decision {
            return self.drop(diagram, edge, height);
        }
        STEP.max(labelled(height))
    }

    /// A branch that drops from the decision's foot runs straight to its
    /// step, or jogs sideways first when its step stands off to one side.
    fn drop(&self, diagram: &Diagram, edge: usize, height: f64) -> f64 {
        let below = height + LABEL_BELOW_DECISION;
        if self.exits.jog(diagram, edge) {
            return below + LABEL_BELOW_DECISION;
        }
        STEP.max(below)
    }

    fn side_run(&self, diagram: &Diagram, edge: usize, height: f64) -> f64 {
        if diagram.nodes[diagram.edges[edge].to].kind == Kind::Merge {
            return self.close(diagram, edge, height);
        }
        if diagram.nodes[diagram.edges[edge].from].kind == Kind::Decision
            && !diagram.nodes[diagram.edges[edge].from].bare
        {
            BRANCH
        } else {
            STEP
        }
    }

    fn close(&self, diagram: &Diagram, edge: usize, height: f64) -> f64 {
        let base = if diagram.nodes[diagram.edges[edge].from].kind == Kind::Bar {
            BAR_CLOSE
        } else {
            CLOSE
        };
        base.max(labelled(height))
    }

    /// The text between two rows: the arrow's own label, or a bare
    /// decision's question.
    fn label_height(&self, diagram: &Diagram, edge: usize) -> f64 {
        let mut lines = super::model::edge_label_lines(diagram.edges[edge].label.as_ref());
        if diagram.nodes[diagram.edges[edge].to].bare && lines.is_empty() {
            lines = super::model::node_lines(diagram.nodes[diagram.edges[edge].to].text.as_deref());
        }
        if lines.is_empty() {
            0.0
        } else {
            self.metrics.text_height(&lines, Some("arrow"))
        }
    }
}

/// Ruby's `own.max || STEP`: an empty ask list falls to the step.
trait MaxFiniteOr {
    fn max_finite_or(self, step: f64) -> f64;
}

impl MaxFiniteOr for f64 {
    fn max_finite_or(self, step: f64) -> f64 {
        if self.is_finite() {
            self
        } else {
            step
        }
    }
}

fn labelled(height: f64) -> f64 {
    if height == 0.0 {
        0.0
    } else {
        height + LABEL_MARGIN
    }
}

/// Two half-step separations.
const LEGACY_STEP: f64 = 40.5;

/// The space above each row in the legacy syntax (`LegacyRowGaps`).
pub struct LegacyRowGaps<'a> {
    contents: Vec<Vec<f64>>,
    ranking: &'a Ranking,
    metrics: &'a Metrics<'a>,
}

impl<'a> LegacyRowGaps<'a> {
    pub fn new(diagram: &Diagram, ranking: &'a Ranking, metrics: &'a Metrics<'a>) -> Self {
        let rows = ranking
            .rank
            .values()
            .copied()
            .max()
            .map(|max| max as usize + 1)
            .unwrap_or(1);
        let mut gaps = LegacyRowGaps {
            contents: vec![vec![0.0]; rows],
            ranking,
            metrics,
        };
        for index in 0..diagram.edges.len() {
            gaps.label(diagram, index);
        }
        for node in 0..diagram.nodes.len() {
            gaps.park(diagram, node);
        }
        gaps
    }

    /// The space above each row; the first is 0.
    pub fn to_a(&self) -> Vec<f64> {
        self.contents
            .iter()
            .enumerate()
            .map(|(row, own)| {
                if row == 0 {
                    0.0
                } else {
                    LEGACY_STEP + own.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                }
            })
            .collect()
    }

    /// An arrow that spans an odd number of rows has its label in a gap;
    /// over an even number it falls on a row, which it does not grow.
    fn label(&mut self, diagram: &Diagram, edge: usize) {
        let Some(row) = self.middle_gap(diagram, edge) else {
            return;
        };
        let lines = super::model::edge_label_lines(diagram.edges[edge].label.as_ref());
        if lines.is_empty() {
            return;
        }
        self.contents[row].push(self.metrics.text_height(&lines, Some("arrow")));
    }

    /// The row a gap lies above.
    fn middle_gap(&self, diagram: &Diagram, edge: usize) -> Option<usize> {
        if self.ranking.back(edge) {
            return None;
        }
        let from = self.ranking.rank[&diagram.edges[edge].from];
        let span = self.ranking.rank[&diagram.edges[edge].to] - from;
        if span % 2 != 0 {
            Some((from + ((span + 1) / 2)) as usize)
        } else {
            None
        }
    }

    fn park(&mut self, diagram: &Diagram, node: usize) {
        if !self.ranking.parked(diagram, node) {
            return;
        }
        let height = self.metrics.box_for(&diagram.nodes[node]).height;
        self.contents[self.ranking.rank[&node] as usize + 1].push(height);
    }
}

/// How far a bar reaches past the boxes it joins.
const OVERHANG: f64 = 13.0;

/// Where a bar stands (`BarColumns`).
pub struct BarColumns<'a> {
    diagram: &'a Diagram,
    ranking: &'a Ranking,
    columns: &'a Columns<'a>,
    boxes: &'a HashMap<usize, Box>,
}

impl<'a> BarColumns<'a> {
    pub fn new(
        diagram: &'a Diagram,
        ranking: &'a Ranking,
        columns: &'a Columns<'a>,
        boxes: &'a HashMap<usize, Box>,
    ) -> Self {
        BarColumns {
            diagram,
            ranking,
            columns,
            boxes,
        }
    }

    /// The nodes on the side of the bar where several branches meet it, or
    /// all its neighbours when no side has more than the other.
    pub fn fan(&self, bar: usize) -> Vec<usize> {
        let joined = self.joined(bar);
        let mut sides: Vec<Vec<usize>> = vec![];
        for node in joined {
            let side = match self.ranking.rank[&node].cmp(&self.ranking.rank[&bar]) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Greater => 2,
            };
            match sides.get_mut(side) {
                Some(list) => list.push(node),
                None => {
                    while sides.len() < side {
                        sides.push(vec![]);
                    }
                    sides.push(vec![node]);
                }
            }
        }
        let widest = sides.iter().map(Vec::len).max().unwrap_or(0);
        sides
            .into_iter()
            .filter(|side| side.len() == widest)
            .flatten()
            .collect()
    }

    /// The columns of the boxes a bar fans out to or gathers; None for any
    /// other node.
    pub fn fan_keys(&self, node: usize) -> Option<Vec<Key>> {
        if self.diagram.nodes[node].kind != Kind::Bar {
            return None;
        }
        Some(
            self.fan(node)
                .into_iter()
                .map(|other| self.columns.key[&other])
                .collect(),
        )
    }

    /// Gives the outermost columns a bar joins room for its overhang.
    pub fn widen(&self, reach: &mut Reach) {
        for bar in 0..self.diagram.nodes.len() {
            if self.diagram.nodes[bar].kind == Kind::Bar {
                self.widen_bar(reach, bar);
            }
        }
    }

    fn widen_bar(&self, reach: &mut Reach, bar: usize) {
        let mut boxes: Vec<(Key, usize)> = self
            .fan(bar)
            .into_iter()
            .map(|node| (self.columns.key[&node], node))
            .collect();
        if boxes.is_empty() {
            return;
        }
        boxes.sort_by(|a, b| column_cmp(a.0, b.0));
        self.overhang(reach, boxes[0], 0);
        self.overhang(reach, boxes[boxes.len() - 1], 1);
    }

    fn overhang(&self, reach: &mut Reach, (key, node): (Key, usize), side: usize) {
        reach.grow(
            key,
            self.ranking.row(self.diagram, node),
            side,
            (self.boxes[&node].width / 2.0) + OVERHANG,
        );
    }

    fn joined(&self, bar: usize) -> Vec<usize> {
        self.diagram
            .edges
            .iter()
            .filter_map(|edge| {
                let other = if edge.from == bar { edge.to } else { edge.from };
                ((edge.from == bar || edge.to == bar)
                    && self.diagram.nodes[other].kind != Kind::Bar)
                    .then_some(other)
            })
            .collect()
    }
}

/// The two steps of a two-step side lane PlantUML hangs from the lane's
/// ends rather than from the rows of the lane beside it (`LaneSteps`).
pub struct LaneSteps {
    /// Each step hung from the decision, with the step that follows it.
    heads: HashMap<usize, usize>,
    /// Each step hung from the diamond closing its branches, with that
    /// diamond.
    tails: HashMap<usize, usize>,
    ranks: HashMap<usize, i64>,
}

impl LaneSteps {
    /// An empty step table, replaced once the columns are placed.
    pub fn empty() -> Self {
        LaneSteps {
            heads: Default::default(),
            tails: Default::default(),
            ranks: Default::default(),
        }
    }

    pub fn new(diagram: &Diagram, ranking: &Ranking, exits: &Exits<'_>) -> Self {
        let edges: Vec<usize> = if diagram.legacy {
            vec![]
        } else {
            (0..diagram.edges.len()).collect()
        };
        let mut steps = LaneSteps {
            heads: Default::default(),
            tails: Default::default(),
            ranks: ranking.rank.clone(),
        };
        let closing: Vec<usize> = edges
            .iter()
            .copied()
            .filter(|edge| steps.closing(diagram, ranking, exits, *edge))
            .collect();
        for edge in closing {
            steps.hang(diagram, ranking, exits, edge);
        }
        steps
    }

    /// Whether the step hangs from either end.
    pub fn hung(&self, node: usize) -> bool {
        self.heads.contains_key(&node) || self.tails.contains_key(&node)
    }

    /// Where the step's centre stands, or None when the rows place it.
    pub fn centre(&self, node: usize, height: f64, tops: &[f64]) -> Option<f64> {
        if self.heads.contains_key(&node) {
            return Some(tops[self.ranks[&node] as usize] + (height / 2.0));
        }
        let diamond = *self.tails.get(&node)?;
        Some(tops[self.ranks[&diamond] as usize] - CLOSE - (height / 2.0))
    }

    fn hang(&mut self, diagram: &Diagram, ranking: &Ranking, exits: &Exits<'_>, tail_edge: usize) {
        let tail_from = diagram.edges[tail_edge].from;
        let head = (0..diagram.edges.len())
            .find(|entry| self.lane(diagram, ranking, exits, *entry, tail_from));
        let Some(head) = head else {
            return;
        };
        self.tails.insert(tail_from, diagram.edges[tail_edge].to);
        self.heads.insert(diagram.edges[head].to, tail_from);
    }

    /// A step that reaches the closing diamond beside its row, from more
    /// than one row up.
    fn closing(
        &self,
        diagram: &Diagram,
        ranking: &Ranking,
        exits: &Exits<'_>,
        edge: usize,
    ) -> bool {
        diagram.nodes[diagram.edges[edge].to].kind == Kind::Merge
            && !ranking.back(edge)
            && ranking.rank[&diagram.edges[edge].to] - ranking.rank[&diagram.edges[edge].from] > 1
            && exits.beside(diagram, edge)
    }

    /// The edge that brings a decision's side branch to the first step of
    /// a lane ending at `tail`; the lane is those two steps.
    fn lane(
        &self,
        diagram: &Diagram,
        ranking: &Ranking,
        exits: &Exits<'_>,
        entry: usize,
        tail: usize,
    ) -> bool {
        let step = diagram.edges[entry].to;
        diagram.nodes[diagram.edges[entry].from].kind == Kind::Decision
            && exits.beside(diagram, entry)
            && ranking
                .forward(step)
                .iter()
                .map(|edge| diagram.edges[*edge].to)
                .collect::<Vec<_>>()
                == [tail]
    }
}

/// Keeps each swimlane band as wide as its title (`LaneTitles`).
pub struct LaneTitles<'a> {
    metrics: &'a Metrics<'a>,
    size: f64,
    pad: f64,
}

impl<'a> LaneTitles<'a> {
    pub fn new(metrics: &'a Metrics<'a>, size: f64, pad: f64) -> Self {
        LaneTitles { metrics, size, pad }
    }

    /// How many bands were short.
    pub fn widen(&self, bands: &[Band], reach: &mut Reach) -> usize {
        bands
            .iter()
            .filter(|band| self.widen_band(band, reach))
            .count()
    }

    fn widen_band(&self, band: &Band, reach: &mut Reach) -> bool {
        let half = self.shortfall(band) / 2.0;
        if half <= 0.0 {
            return false;
        }
        reach.widen_column(band.columns[0], 0, half);
        reach.widen_column(band.columns[band.columns.len() - 1], 1, half);
        true
    }

    fn shortfall(&self, band: &Band) -> f64 {
        self.metrics
            .text_width_sized(&[band.name.clone()], None, self.size)
            + self.pad
            - (band.right - band.left)
    }
}

/// A swimlane band: its name, colour, columns, left and right.
#[derive(Clone)]
pub struct Band {
    pub name: String,
    pub colour: Option<String>,
    pub columns: Vec<Key>,
    pub left: f64,
    pub right: f64,
}

/// The room a lane band keeps about its edges.
pub const BAND_PAD: f64 = 14.0;
const LABEL_OFFSET: f64 = 5.0;
const BARE_LABEL_GAP: f64 = 13.0;
const NOTE_GAP: f64 = 20.0;
pub const TITLE_HEIGHT: f64 = 26.0;
pub const HEADER_HEIGHT: f64 = 28.0;
pub const PARTITION_BELOW: f64 = 10.0;
/// The size the JAR draws a swimlane's title at, and the room it keeps
/// about the title inside the band.
pub const LANE_TITLE: f64 = 18.0;
const LANE_TITLE_PAD: f64 = 10.0;
/// The height each line of a PlantUML notice adds above the drawing.
pub const WARNING_PITCH: f64 = 21.6406;

/// Puts every node's node_box at its row and column, then fits each bar to the
/// nodes it joins and each note beside its node (`Placement`).
pub struct Placement {
    pub boxes: HashMap<usize, Box>,
    /// The bottom of each row.
    pub bottoms: Vec<f64>,
    /// The space above each row; the first is 0.
    pub gaps: Vec<f64>,
    /// Per node, its notes and where each sits.
    pub notes: Vec<(usize, Vec<(super::model::Note, Box)>)>,
    /// The top of the first row.
    pub top: f64,
    /// A name, colour, left and right per lane.
    pub bands: Vec<Band>,
    centres: HashMap<Key, f64>,
    lane_steps: LaneSteps,
}

impl Placement {
    pub fn new(
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        margin: f64,
        metrics: &Metrics<'_>,
    ) -> Self {
        let boxes: HashMap<usize, Box> = (0..diagram.nodes.len())
            .map(|node| (node, metrics.box_for(&diagram.nodes[node])))
            .collect();
        let mut placement = Placement {
            boxes,
            bottoms: vec![],
            gaps: vec![],
            notes: vec![],
            top: margin + Placement::header_height(diagram, metrics),
            bands: vec![],
            centres: Default::default(),
            // The Ruby reads these through memoized accessors, first
            // called once the columns have placed the boxes; `lay_out`
            // builds them at the same point.
            lane_steps: LaneSteps::empty(),
        };
        placement.lay_out(diagram, ranking, columns, margin, metrics);
        placement
    }

    /// Room above a partition's nodes for its title.
    pub fn head(&self, metrics: &Metrics<'_>) -> f64 {
        metrics.size(Some("partition")) + 10.0
    }

    fn header_height(diagram: &Diagram, metrics: &Metrics<'_>) -> f64 {
        let mut height = 0.0;
        if diagram.title.is_some() {
            height += TITLE_HEIGHT;
        }
        height += diagram.warnings.len() as f64 * WARNING_PITCH;
        if !diagram.lanes.is_empty() {
            height += HEADER_HEIGHT;
        }
        if diagram.nodes.iter().any(|node| node.partition.is_some()) {
            height += metrics.size(Some("partition")) + 10.0;
        }
        height
    }

    fn lay_out(
        &mut self,
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        margin: f64,
        metrics: &Metrics<'_>,
    ) {
        self.place_columns(diagram, ranking, columns, margin, metrics);
        let exits = Exits::new(columns, &self.boxes);
        self.gaps = if diagram.legacy {
            LegacyRowGaps::new(diagram, ranking, metrics).to_a()
        } else {
            let frame = self.head(metrics) + PARTITION_BELOW + 8.0;
            RowGaps::new(diagram, ranking, &exits, metrics, frame).to_a()
        };
        self.lane_steps = LaneSteps::new(diagram, ranking, &exits);
        self.place_rows(diagram, ranking);
        self.fit_bars(diagram, ranking, columns);
        self.place_notes(diagram, metrics);
    }

    fn place_columns(
        &mut self,
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        margin: f64,
        metrics: &Metrics<'_>,
    ) {
        let mut keys: Vec<Key> = columns.key.values().copied().collect();
        keys.sort_by(|a, b| column_cmp(*a, *b));
        keys.dedup();
        let bar_columns = BarColumns::new(diagram, ranking, columns, &self.boxes);
        let mut reach = self.reaches(diagram, ranking, columns, &bar_columns, &keys, metrics);
        let mut centres =
            ColumnCentres::new(columns, &self.boxes, &reach, margin, &bar_columns).call(&keys);
        let bands = self.lane_bands(&centres, diagram, &keys, &reach.columns);
        let short = LaneTitles::new(metrics, LANE_TITLE, LANE_TITLE_PAD).widen(&bands, &mut reach);
        if short > 0 {
            centres =
                ColumnCentres::new(columns, &self.boxes, &reach, margin, &bar_columns).call(&keys);
        }
        self.centres = centres;
        for node in 0..diagram.nodes.len() {
            let centre = self.centres[&columns.key[&node]];
            self.boxes.get_mut(&node).unwrap().cx = centre;
        }
        self.bands = self.lane_bands(&self.centres, diagram, &keys, &reach.columns);
    }

    /// How far each column reaches left and right of its centre,
    /// including notes and the labels hung on its arrows.
    fn reaches(
        &self,
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        bar_columns: &BarColumns<'_>,
        keys: &[Key],
        metrics: &Metrics<'_>,
    ) -> Reach {
        let mut reach = Reach::new(keys, diagram.legacy);
        for node in 0..diagram.nodes.len() {
            if diagram.nodes[node].kind == Kind::Bar {
                continue;
            }
            let half = self.boxes[&node].width / 2.0;
            let left = half + self.note_room(diagram, metrics, node, super::model::Side::Left);
            let right = half + self.note_room(diagram, metrics, node, super::model::Side::Right);
            let row = ranking.row(diagram, node);
            reach.grow(columns.key[&node], row, 0, left);
            reach.grow(columns.key[&node], row, 1, right);
        }
        self.reach_lines(diagram, ranking, columns, &mut reach);
        let mut reach = self.reach_labels(diagram, ranking, columns, metrics, reach);
        bar_columns.widen(&mut reach);
        reach
    }

    /// An arrow that runs past rows stands on them as a line, in its own
    /// column and where it lands, as wide as nothing; one that runs back
    /// up the page passes none.
    fn reach_lines(
        &self,
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        reach: &mut Reach,
    ) {
        if !diagram.legacy {
            return;
        }
        for edge in 0..diagram.edges.len() {
            let (from, to) = (diagram.edges[edge].from, diagram.edges[edge].to);
            let rows = (ranking.rank[&from] + 1)..ranking.rank[&to];
            for node in [from, to] {
                reach.line(columns.key[&node], rows.clone().map(|row| row as f64));
            }
        }
    }

    /// A label sits to the right of the line it rides, and a bare
    /// decision's question to the left of the line that enters it.
    fn reach_labels(
        &self,
        diagram: &Diagram,
        ranking: &Ranking,
        columns: &Columns<'_>,
        metrics: &Metrics<'_>,
        mut reach: Reach,
    ) -> Reach {
        for edge in 0..diagram.edges.len() {
            let (from, to) = (diagram.edges[edge].from, diagram.edges[edge].to);
            if diagram.nodes[from].kind == Kind::Bar || diagram.nodes[to].kind == Kind::Bar {
                continue;
            }
            let lines = super::model::edge_label_lines(diagram.edges[edge].label.as_ref());
            if !lines.is_empty() {
                let owner = if diagram.nodes[from].kind == Kind::Decision {
                    to
                } else {
                    from
                };
                let row = ranking.row(diagram, owner);
                reach.grow(
                    columns.key[&owner],
                    row,
                    1,
                    LABEL_OFFSET + self.width_of(metrics, &lines),
                );
            }
            if diagram.nodes[to].bare {
                let lines = super::model::node_lines(diagram.nodes[to].text.as_deref());
                let row = ranking.row(diagram, to);
                reach.grow(
                    columns.key[&to],
                    row,
                    0,
                    BARE_LABEL_GAP + self.width_of(metrics, &lines),
                );
            }
        }
        reach
    }

    fn width_of(&self, metrics: &Metrics<'_>, lines: &[String]) -> f64 {
        if lines.is_empty() {
            0.0
        } else {
            metrics.text_width(lines, Some("arrow"))
        }
    }

    fn note_room(
        &self,
        diagram: &Diagram,
        metrics: &Metrics<'_>,
        node: usize,
        side: super::model::Side,
    ) -> f64 {
        let widths: Vec<f64> = diagram.nodes[node]
            .notes
            .iter()
            .filter(|note| note.side == side)
            .map(|note| metrics.note_box(note).width)
            .collect();
        if widths.is_empty() {
            0.0
        } else {
            widths.iter().copied().fold(f64::NEG_INFINITY, f64::max) + NOTE_GAP
        }
    }

    fn lane_bands(
        &self,
        centres: &HashMap<Key, f64>,
        diagram: &Diagram,
        keys: &[Key],
        reach: &HashMap<Key, (f64, f64)>,
    ) -> Vec<Band> {
        let mut bands: Vec<Band> = vec![];
        for (index, (name, colour)) in diagram.lanes.iter().enumerate() {
            let own: Vec<Key> = keys
                .iter()
                .copied()
                .filter(|key| key.lane() == index as i64)
                .collect();
            if own.is_empty() {
                continue;
            }
            let first = own[0];
            let last = own[own.len() - 1];
            bands.push(Band {
                name: name.clone(),
                colour: colour.clone(),
                columns: own,
                left: centres[&first] - reach[&first].0 - BAND_PAD,
                right: centres[&last] + reach[&last].1 + BAND_PAD,
            });
        }
        // `abut`: a decision centred on branches in the lane beside it
        // reaches over the border; the next lane starts where this one
        // ends.
        for index in 1..bands.len() {
            let prior_right = bands[index - 1].right;
            bands[index].left = bands[index].left.max(prior_right);
        }
        bands
    }

    fn place_rows(&mut self, diagram: &Diagram, ranking: &Ranking) {
        let heights = self.row_heights(diagram, ranking);
        let tops: Vec<f64> = (0..heights.len())
            .map(|row| self.row_top(&heights, row))
            .collect();
        self.bottoms = tops
            .iter()
            .zip(heights.iter())
            .map(|(top, height)| top + height)
            .collect();
        for node in 0..diagram.nodes.len() {
            let row = ranking.rank[&node];
            let offset = if ranking.parked(diagram, node) {
                heights[row as usize]
                    + (self.gaps.get(row as usize + 1).copied().unwrap_or(0.0) / 2.0)
            } else {
                heights[row as usize] / 2.0
            };
            let height = self.boxes[&node].height;
            let node_box = self.boxes.get_mut(&node).unwrap();
            node_box.cy = tops[row as usize] + offset;
            if let Some(centre) = self.lane_steps.centre(node, height, &tops) {
                node_box.cy = centre;
            }
        }
    }

    fn row_top(&self, heights: &[f64], row: usize) -> f64 {
        self.top
            + ruby_sum(&heights[..row])
            + ruby_sum(&self.gaps[..(row + 1).min(self.gaps.len())])
    }

    fn row_heights(&self, diagram: &Diagram, ranking: &Ranking) -> Vec<f64> {
        let mut tall: HashMap<i64, Vec<usize>> = HashMap::new();
        for node in 0..diagram.nodes.len() {
            if ranking.parked(diagram, node) || self.lane_steps.hung(node) {
                continue;
            }
            tall.entry(ranking.rank[&node]).or_default().push(node);
        }
        let rows = tall.keys().copied().max().map(|max| max + 1).unwrap_or(0);
        (0..rows)
            .map(|row| {
                tall.get(&row)
                    .map(|nodes| {
                        nodes
                            .iter()
                            .map(|node| self.boxes[node].height)
                            .fold(f64::NEG_INFINITY, f64::max)
                    })
                    .unwrap_or(0.0)
            })
            .collect()
    }

    /// A bar stretches over the boxes it fans out to or gathers.
    fn fit_bars(&mut self, diagram: &Diagram, ranking: &Ranking, columns: &Columns<'_>) {
        let bars: Vec<usize> = (0..diagram.nodes.len())
            .filter(|node| diagram.nodes[*node].kind == Kind::Bar)
            .collect();
        for bar in bars {
            let bar_columns = BarColumns::new(diagram, ranking, columns, &self.boxes);
            let fan = bar_columns.fan(bar);
            if fan.is_empty() {
                continue;
            }
            let mut left = f64::INFINITY;
            let mut right = f64::NEG_INFINITY;
            let centres: Vec<f64> = fan.iter().map(|node| self.boxes[node].cx).collect();
            for node in &fan {
                left = left.min(self.boxes[node].left() - OVERHANG);
                right = right.max(self.boxes[node].right() + OVERHANG);
            }
            let legacy = diagram.nodes[bar].legacy;
            let low = centres.iter().copied().fold(f64::INFINITY, f64::min);
            let high = centres.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let node_box = self.boxes.get_mut(&bar).unwrap();
            node_box.cx = (left + right) / 2.0;
            if !legacy {
                node_box.width = (right - left).max(node_box.width);
            }
            // The boxes the bar spans, and the middle of them where a lone
            // neighbour's arrow meets it.
            node_box.fan = fan;
            node_box.anchor = Some((low + high) / 2.0);
        }
    }

    fn place_notes(&mut self, diagram: &Diagram, metrics: &Metrics<'_>) {
        for node in 0..diagram.nodes.len() {
            let mut pairs = vec![];
            for note in &diagram.nodes[node].notes {
                let mut note_box = metrics.note_box(note);
                let anchor = &self.boxes[&node];
                note_box.cy = anchor.cy;
                note_box.cx = if note.side == super::model::Side::Right {
                    anchor.right() + NOTE_GAP + (note_box.width / 2.0)
                } else {
                    anchor.left() - NOTE_GAP - (note_box.width / 2.0)
                };
                pairs.push((note.clone(), note_box));
            }
            self.notes.push((node, pairs));
        }
    }
}

/// The margin the layout keeps about every mark (`Layout::MARGIN`).
pub const MARGIN: f64 = 14.0;

/// Places an activity diagram top to bottom (`Layout`): rows from the
/// ranking, columns from the walk, routes from the router. The result is
/// deterministic and uses fixed spacing.
pub struct Layout;

impl Layout {
    pub fn call(
        diagram: &Diagram,
        sheet: &super::style::Sheet,
    ) -> (Vec<super::marks::Mark>, f64, f64) {
        let ranking = Ranking::new(diagram);
        let columns = Columns::new(diagram, &ranking);
        let metrics = Metrics::new(sheet);
        let placement = Placement::new(diagram, &ranking, &columns, MARGIN, &metrics);
        let mut marks = super::marks::Marks::new(diagram, &placement, &metrics);
        marks.build(&ranking, &columns);
        let reached = marks.min_x();
        if reached >= MARGIN {
            return marks.scene();
        }
        // A mark left of the margin shifts the margin and the layout is
        // built again.
        let placement = Placement::new(
            diagram,
            &ranking,
            &columns,
            (2.0 * MARGIN) - reached,
            &metrics,
        );
        let mut marks = super::marks::Marks::new(diagram, &placement, &metrics);
        marks.build(&ranking, &columns);
        marks.scene()
    }
}
