//! Puts the items of one scope into ranks and orders each rank, from the
//! links between them; gives each link that skips ranks a waypoint in
//! every rank it skips. Port of `node_graph/{ranking,skipped_ranks}.rb`.
//! Source order breaks every tie, so the result is deterministic.

use std::collections::HashMap;

/// An item of a rank: a node index, or a waypoint (an allocated id).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Item {
    Node(usize),
    Waypoint(usize),
}

pub const SWEEPS: usize = 4;

/// A link between two ranking items with the ranks it asks for.
pub type Pair = (Item, Item, Option<usize>);

/// A laid-out scope's ranking.
pub struct Ranking {
    pub layers: Vec<Vec<Item>>,
    /// For each `[from, to]` (node indices) spanning more than one rank,
    /// its waypoints from the rank after `from`.
    pub chains: Vec<((usize, usize), Vec<Item>)>,
    /// The links the order is settled by.
    pub links: Vec<Pair>,
}

/// Ranks `items` (source order) from `pairs` (node-index links,
/// `[from, to, step]`).
pub fn rank(items: &[Item], pairs: &[(usize, usize, Option<usize>)]) -> Ranking {
    let pairs: Vec<Pair> = pairs
        .iter()
        .map(|(a, b, step)| (Item::Node(*a), Item::Node(*b), *step))
        .collect();
    let merged_pairs = merged(&pairs);
    let forward = acyclic(&merged_pairs, items);
    let mut rank_of: HashMap<Item, usize> = items.iter().map(|item| (*item, 0)).collect();
    settle_ranks(&mut rank_of, &forward, items.len() + 1);
    sink_sources(&mut rank_of, &forward, items);
    let tie_order = tie_order(&merged_pairs);
    let mut layers = if merged_pairs.is_empty() {
        grid(items)
    } else {
        group(items, &rank_of, &tie_order)
    };
    let skipped = SkippedRanks::build(&mut layers, &rank_of, &merged_pairs, &forward);
    order(&mut layers, &skipped.links);
    Ranking {
        layers,
        chains: skipped.chains,
        links: skipped.links,
    }
}

/// Each pair once, with the longest step asked of it; first occurrence
/// order kept (Ruby `group_by` + `max_by`).
fn merged(pairs: &[Pair]) -> Vec<Pair> {
    let mut keys: Vec<(Item, Item)> = vec![];
    let mut groups: Vec<Vec<usize>> = vec![];
    for (index, (a, b, _)) in pairs.iter().enumerate() {
        if a == b {
            continue;
        }
        let key = (*a, *b);
        match keys.iter().position(|existing| *existing == key) {
            Some(at) => groups[at].push(index),
            None => {
                keys.push(key);
                groups.push(vec![index]);
            }
        }
    }
    groups
        .into_iter()
        .map(|indices| {
            // Ruby max_by keeps the first maximal element.
            let mut best = indices[0];
            for &candidate in &indices[1..] {
                if pairs[candidate].2.unwrap_or(1) > pairs[best].2.unwrap_or(1) {
                    best = candidate;
                }
            }
            pairs[best]
        })
        .collect()
}

/// The links that remain once every cycle-closing link, found depth first
/// in source order, is dropped.
fn acyclic(pairs: &[Pair], items: &[Item]) -> Vec<Pair> {
    let mut adjacency: Vec<(Item, Vec<Item>)> = vec![];
    for (a, b, _) in pairs {
        match adjacency.iter_mut().find(|(key, _)| *key == *a) {
            Some((_, list)) => list.push(*b),
            None => adjacency.push((*a, vec![*b])),
        }
    }
    let mut state: HashMap<Item, u8> = HashMap::new();
    let mut kept = vec![];
    for item in items {
        if state.get(item).copied().is_none() {
            descend(item, &adjacency, pairs, &mut state, &mut kept);
        }
    }
    kept
}

fn descend(
    item: &Item,
    adjacency: &[(Item, Vec<Item>)],
    pairs: &[Pair],
    state: &mut HashMap<Item, u8>,
    kept: &mut Vec<Pair>,
) {
    state.insert(*item, 1);
    let neighbours = adjacency
        .iter()
        .find(|(key, _)| key == item)
        .map(|(_, list)| list.clone())
        .unwrap_or_default();
    for neighbour in neighbours {
        follow(item, &neighbour, adjacency, pairs, state, kept);
    }
    state.insert(*item, 2);
}

fn follow(
    from: &Item,
    to: &Item,
    adjacency: &[(Item, Vec<Item>)],
    pairs: &[Pair],
    state: &mut HashMap<Item, u8>,
    kept: &mut Vec<Pair>,
) {
    if state.get(to).copied() == Some(1) {
        return;
    }
    let step = pairs
        .iter()
        .find(|(a, b, _)| a == from && b == to)
        .and_then(|(_, _, step)| *step);
    kept.push((*from, *to, step));
    if state.get(to).copied().is_none() {
        descend(to, adjacency, pairs, state, kept);
    }
}

/// Longest-path ranks, bounded so links that cannot all hold cannot loop.
fn settle_ranks(rank_of: &mut HashMap<Item, usize>, forward: &[Pair], rounds: usize) {
    for _ in 0..rounds {
        let mut any = false;
        for (from, to, step) in forward {
            if hold(rank_of, *from, *to, *step) {
                any = true;
            }
        }
        if !any {
            break;
        }
    }
}

fn hold(
    rank_of: &mut HashMap<Item, usize>,
    from: Item,
    to: Item,
    step: Option<usize>,
) -> bool {
    let step = step.unwrap_or(1);
    let want = rank_of[&from] + step;
    let ends: Vec<Item> = if step == 0 { vec![from, to] } else { vec![to] };
    let target = ends
        .iter()
        .map(|item| rank_of[item])
        .chain(std::iter::once(want))
        .max()
        .unwrap_or(want);
    let mut moved = false;
    for item in &ends {
        if rank_of[item] < target {
            rank_of.insert(*item, target);
            moved = true;
        }
    }
    moved
}

/// A group of items with no link in drops to the rank just before its
/// nearest target, which shortens every link it has.
fn sink_sources(rank_of: &mut HashMap<Item, usize>, forward: &[Pair], items: &[Item]) {
    let groups = same_rank_groups(forward, items);
    let mut sorted: Vec<&Vec<Item>> = groups.iter().collect();
    sorted.sort_by(|a, b| rank_of[&b[0]].cmp(&rank_of[&a[0]]));
    for group in sorted {
        let Some(target) = sink_rank(group, forward, rank_of) else { continue };
        let first_rank = rank_of[&group[0]];
        if target > first_rank {
            for item in group {
                rank_of.insert(*item, target);
            }
        }
    }
}

fn sink_rank(group: &[Item], forward: &[Pair], rank_of: &HashMap<Item, usize>) -> Option<usize> {
    if fed(group, forward) {
        return None;
    }
    let mut best: Option<usize> = None;
    for (from, to, step) in forward {
        if group.contains(from) && !group.contains(to) {
            let candidate = rank_of[to].saturating_sub(step.unwrap_or(1));
            best = Some(best.map_or(candidate, |current| current.min(candidate)));
        }
    }
    best
}

fn fed(group: &[Item], forward: &[Pair]) -> bool {
    forward
        .iter()
        .any(|(from, to, _)| group.contains(to) && !group.contains(from))
}

fn same_rank_groups(forward: &[Pair], items: &[Item]) -> Vec<Vec<Item>> {
    let mut groups: HashMap<Item, Vec<Item>> =
        items.iter().map(|item| (*item, vec![*item])).collect();
    for (from, to, step) in forward {
        if step.unwrap_or(1) != 0 {
            continue;
        }
        let mut union = groups[from].clone();
        for item in &groups[to] {
            if !union.contains(item) {
                union.push(*item);
            }
        }
        for item in &union {
            groups.insert(*item, union.clone());
        }
    }
    let mut unique: Vec<Vec<Item>> = vec![];
    for group in groups.values() {
        if !unique.iter().any(|existing| existing == group) {
            unique.push(group.clone());
        }
    }
    unique
}

/// Items with no links between them, `ceil(sqrt(n))` to a rank, in source
/// order.
fn grid(items: &[Item]) -> Vec<Vec<Item>> {
    let per_rank = ((items.len() as f64).sqrt().ceil() as usize).max(1);
    items.chunks(per_rank).map(|chunk| chunk.to_vec()).collect()
}

fn group(
    items: &[Item],
    rank_of: &HashMap<Item, usize>,
    tie_order: &HashMap<Item, usize>,
) -> Vec<Vec<Item>> {
    let mut by_rank: HashMap<usize, Vec<Item>> = HashMap::new();
    for item in items {
        by_rank.entry(rank_of[item]).or_default().push(*item);
    }
    let max = by_rank.keys().copied().max().unwrap_or(0);
    (0..=max)
        .map(|rank| {
            let layer = by_rank.remove(&rank).unwrap_or_default();
            let mut ordered: Vec<(Item, usize)> =
                layer.into_iter().enumerate().map(|(at, item)| (item, at)).collect();
            // A rank with its tied items first, in the order their first
            // tie was written.
            ordered.sort_by(|(a, at_a), (b, at_b)| {
                let key = |item: &Item, at: usize| -> (usize, usize, usize) {
                    match tie_order.get(item) {
                        Some(tie) => (0, *tie, at),
                        None => (1, 0, at),
                    }
                };
                key(a, *at_a).cmp(&key(b, *at_b))
            });
            ordered.into_iter().map(|(item, _)| item).collect()
        })
        .collect()
}

/// Each item tied to another by a step-0 link, to the position of its
/// first such link.
fn tie_order(pairs: &[Pair]) -> HashMap<Item, usize> {
    let mut order = HashMap::new();
    for (at, (from, to, step)) in pairs.iter().enumerate() {
        if step.unwrap_or(1) != 0 {
            continue;
        }
        order.entry(*from).or_insert(at);
        order.entry(*to).or_insert(at);
    }
    order
}

struct SkippedRanks {
    chains: Vec<((usize, usize), Vec<Item>)>,
    links: Vec<Pair>,
}

impl SkippedRanks {
    fn build(
        layers: &mut Vec<Vec<Item>>,
        rank_of: &HashMap<Item, usize>,
        pairs: &[Pair],
        kept_pairs: &[Pair],
    ) -> SkippedRanks {
        let mut chains = vec![];
        let mut links = vec![];
        let mut next_waypoint = 0usize;
        for (from, to, step) in pairs {
            let span = rank_of[to] as i64 - rank_of[from] as i64;
            let is_kept = kept_pairs.iter().any(|(ka, kb, _)| ka == from && kb == to);
            if span <= 1 || !is_kept {
                links.push((*from, *to, *step));
                continue;
            }
            let mut points = vec![];
            for _ in 0..(span - 1) {
                let item = Item::Waypoint(next_waypoint);
                next_waypoint += 1;
                points.push(item);
                let at = rank_of[from] + points.len();
                if at < layers.len() {
                    layers[at].push(item);
                }
            }
            let ids: Vec<usize> = points
                .iter()
                .map(|item| match item {
                    Item::Waypoint(id) => *id,
                    Item::Node(_) => unreachable!(),
                })
                .collect();
            let node_from = match from {
                Item::Node(id) => *id,
                Item::Waypoint(_) => unreachable!(),
            };
            let node_to = match to {
                Item::Node(id) => *id,
                Item::Waypoint(_) => unreachable!(),
            };
            chains.push(((node_from, node_to), points));
            let mut chain: Vec<Pair> = vec![(*from, Item::Waypoint(ids[0]), Some(1))];
            for pair in ids.windows(2) {
                chain.push((Item::Waypoint(pair[0]), Item::Waypoint(pair[1]), Some(1)));
            }
            chain.push((Item::Waypoint(ids[ids.len() - 1]), *to, Some(1)));
            links.extend(chain);
        }
        SkippedRanks { chains, links }
    }
}

fn order(layers: &mut Vec<Vec<Item>>, links: &[Pair]) {
    for pass in 0..SWEEPS {
        let down = pass % 2 == 0;
        let indices: Vec<usize> = if down {
            (1..layers.len()).collect()
        } else {
            (0..layers.len().saturating_sub(1)).rev().collect()
        };
        for index in indices {
            sort_layer(layers, index, if down { -1 } else { 1 }, links);
        }
    }
}

fn sort_layer(layers: &mut Vec<Vec<Item>>, index: usize, step: i64, links: &[Pair]) {
    let neighbour_at = index as i64 + step;
    if !(0..layers.len() as i64).contains(&neighbour_at) {
        return;
    }
    let neighbour = layers[neighbour_at as usize].clone();
    let mut ordered: Vec<(Item, usize)> =
        layers[index].iter().copied().enumerate().map(|(at, item)| (item, at)).collect();
    ordered.sort_by(|(a, at_a), (b, at_b)| {
        let key = |item: &Item, at: usize| -> (f64, usize) {
            match barycentre(*item, &neighbour, links) {
                Some(value) => (value, at),
                None => (at as f64, at),
            }
        };
        key(a, *at_a).partial_cmp(&key(b, *at_b)).expect("finite keys")
    });
    let layer: Vec<Item> = ordered.into_iter().map(|(item, _)| item).collect();
    layers[index] = settled(&layer, links);
}

fn settled(layer: &[Item], links: &[Pair]) -> Vec<Item> {
    let mut rest: Vec<Item> = layer.to_vec();
    let mut last: Option<Item> = None;
    let mut result = vec![];
    for _ in 0..layer.len() {
        let picked = pick_next(&rest, last, links);
        rest.retain(|item| *item != picked);
        last = Some(picked);
        result.push(picked);
    }
    result
}

fn pick_next(rest: &[Item], last: Option<Item>, links: &[Pair]) -> Item {
    let ready: Vec<Item> = rest
        .iter()
        .copied()
        .filter(|item| {
            let before = before_items(*item, links);
            !before.iter().any(|before| rest.contains(before))
        })
        .collect();
    if let Some(last) = last {
        for item in &ready {
            if before_items(*item, links).contains(&last) {
                return *item;
            }
        }
    }
    ready
        .first()
        .copied()
        .or_else(|| rest.first().copied())
        .expect("non-empty rest")
}

fn before_items(item: Item, links: &[Pair]) -> Vec<Item> {
    links
        .iter()
        .filter_map(|(from, to, step)| {
            if step.unwrap_or(1) == 0 && *to == item {
                Some(*from)
            } else {
                None
            }
        })
        .collect()
}

fn barycentre(item: Item, layer: &[Item], links: &[Pair]) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0usize;
    for (a, b, _) in links {
        let other = if *a == item {
            Some(*b)
        } else if *b == item {
            Some(*a)
        } else {
            None
        };
        if let Some(other) = other {
            if let Some(at) = layer.iter().position(|entry| *entry == other) {
                sum += at as f64;
                count += 1;
            }
        }
    }
    (count > 0).then_some(sum / count as f64)
}
