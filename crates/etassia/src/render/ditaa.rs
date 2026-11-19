//! Ditaa renderer: port of Ruby `Diagram::Ditaa` (`Grid`, `Pattern`,
//! `CellSet`, `Builder`) and `PlantUML::DitaaRenderer`. The ASCII-art
//! interpretation is algorithmic, so the grammar carries the wrapper and
//! the verbatim body and this module performs the shape pipeline the Ruby
//! side performs.
use crate::render::pie::text_of;
use crate::svg::{ruby_float, write_num, Document, Elem};
use crate::Error;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;

// ---- Options ----

#[derive(Clone, Copy)]
struct Options {
    #[allow(dead_code)]
    separation: bool,
    shadows: bool,
    round_corners: bool,
    transparent: bool,
    scale: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { separation: true, shadows: true, round_corners: false, transparent: false, scale: 1.0 }
    }
}

/// Ruby `options(start_line)`: the flags off the captured start line.
fn options_from(start_line: &str) -> Options {
    let mut o = Options::default();
    let flag = |long: &str, short: &str| start_line.contains(long) || start_line.contains(short);
    if flag("--no-separation", "-E") { o.separation = false; }
    if flag("--no-shadows", "-S") { o.shadows = false; }
    if flag("--round-corners", "-r") { o.round_corners = true; }
    if flag("--transparent", "-T") { o.transparent = true; }
    if let Some(m) = regex::Regex::new(r"scale=([\d.]+)").unwrap().captures(start_line) {
        o.scale = to_f(m.get(1).map(|g| g.as_str()).unwrap_or("1.0"));
    }
    o
}

/// Ruby `String#to_f`: the longest numeric prefix, 0.0 when none.
fn to_f(s: &str) -> f64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') { i += 1; }
    let digits = |i: &mut usize| {
        let st = *i;
        while *i < b.len() && (b[*i].is_ascii_digit() || (b[*i] == b'_' && *i > st)) { *i += 1; }
        *i > st
    };
    let int = digits(&mut i);
    let mut end = i;
    if int && i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        if digits(&mut j) { i = j; end = j; }
    }
    if int && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') { j += 1; }
        if digits(&mut j) { end = j; }
    }
    t[..end].replace('_', "").parse().unwrap_or(0.0)
}

// ---- CellSet ----

/// Ruby `CellSet`: an insertion-ordered set of `(col, row)` cells.
#[derive(Clone, Default)]
struct CellSet {
    cells: Vec<(i64, i64)>,
    index: HashSet<(i64, i64)>,
}

impl CellSet {
    fn new() -> Self { CellSet { cells: vec![], index: HashSet::new() } }

    fn add(&mut self, col: i64, row: i64) {
        let cell = (col, row);
        if self.index.contains(&cell) { return; }
        self.cells.push(cell);
        self.index.insert(cell);
    }

    fn delete(&mut self, cell: (i64, i64)) {
        self.index.remove(&cell);
        self.cells.retain(|c| *c != cell);
    }

    fn len(&self) -> usize { self.cells.len() }

    fn first(&self) -> (i64, i64) { self.cells[0] }

    fn include(&self, cell: (i64, i64)) -> bool { self.index.contains(&cell) }

    fn subtract_set(&mut self, other: &CellSet) {
        for cell in &other.cells { self.delete(*cell); }
    }

    fn has_common_cells(&self, other: &CellSet) -> bool {
        self.cells.iter().any(|c| other.include(*c))
    }

    fn min_x(&self) -> i64 { self.cells.iter().map(|c| c.0).min().unwrap() }
    fn max_x(&self) -> i64 { self.cells.iter().map(|c| c.0).max().unwrap() }
    fn min_y(&self) -> i64 { self.cells.iter().map(|c| c.1).min().unwrap() }
    fn max_y(&self) -> i64 { self.cells.iter().map(|c| c.1).max().unwrap() }

    /// Ruby `CellSet.remove_duplicates`: keeps first occurrences in order.
    fn remove_duplicates(sets: Vec<CellSet>) -> Vec<CellSet> {
        let mut unique: Vec<CellSet> = vec![];
        for set in sets {
            let key: Vec<(i64, i64)> = {
                let mut sorted = set.cells.clone();
                sorted.sort();
                sorted
            };
            let dup = unique.iter().any(|u| {
                let mut sorted = u.cells.clone();
                sorted.sort();
                sorted == key
            });
            if !dup { unique.push(set); }
        }
        unique
    }

    /// Ruby `break_into_distinct`: connected components over cell
    /// adjacency, in the cells' order.
    fn break_into_distinct(&self) -> Vec<CellSet> {
        let mut seen: HashSet<(i64, i64)> = HashSet::new();
        let mut out = vec![];
        for cell in &self.cells {
            if seen.contains(cell) { continue; }
            if let Some(c) = self.component_from(*cell, &mut seen) { out.push(c); }
        }
        out
    }

    fn component_from(&self, cell: (i64, i64), seen: &mut HashSet<(i64, i64)>) -> Option<CellSet> {
        let mut component = CellSet::new();
        let mut stack = vec![cell];
        while let Some(current) = stack.pop() {
            if seen.contains(&current) || !self.include(current) { continue; }
            seen.insert(current);
            component.add(current.0, current.1);
            for n in component_neighbours(current) { stack.push(n); }
        }
        if component.len() == 0 { None } else { Some(component) }
    }

    /// Ruby `make_into_grid`: a grid holding `*` at the set's cells.
    fn make_into_grid(&self) -> Grid {
        let rows = vec![vec![' '; (self.max_x() + 2) as usize]; (self.max_y() + 2) as usize];
        let mut grid = Grid { rows };
        for (col, row) in &self.cells { grid.set(*col, *row, '*'); }
        grid
    }

    /// Ruby `make_scaled_one_third_equivalent`.
    fn make_scaled_one_third_equivalent(&self) -> CellSet {
        let grid_big = self.make_into_grid();
        let small_rows = vec![
            vec![' '; ((grid_big.width() as i64 + 2) / 3).max(0) as usize];
            ((grid_big.height() as i64 + 2) / 3).max(0) as usize
        ];
        let mut small = Grid { rows: small_rows };
        for row in 0..grid_big.height() {
            for col in 0..grid_big.width() {
                if !grid_big.blank_cell(col as i64, row as i64) {
                    small.set(col as i64 / 3, row as i64 / 3, '*');
                }
            }
        }
        small.all_non_blank()
    }

    /// Ruby `break_into_distinct_boundaries(grid)`: components through the
    /// abstraction grid, where each cell occupies only its drawn marks.
    fn break_into_distinct_boundaries(&self, source_grid: &Grid) -> Vec<CellSet> {
        let buffer = abstraction_buffer(source_grid, self);
        buffer
            .all_non_blank()
            .break_into_distinct()
            .iter()
            .map(|component| {
                let mut scaled = CellSet::new();
                for cell in &component.cells { scaled.add(cell.0 / 3, cell.1 / 3); }
                scaled
            })
            .collect()
    }

    /// Ruby `type(grid)`: trace first, fill to decide.
    fn cell_type(&self, grid: &Grid) -> CellType {
        if self.len() < 2 { return CellType::Open; }
        let traced = self.type_by_trace(grid);
        if matches!(traced, CellType::Open | CellType::Closed) { return traced; }
        let fill = self.type_by_fill(grid);
        if fill == CellType::HasClosedArea { return CellType::Mixed; }
        fill
    }

    fn type_by_trace(&self, grid: &Grid) -> CellType {
        let mut work = Grid::blank_like(grid);
        grid.copy_cells_to(self, &mut work);
        let start = self.trace_start(&work);
        let next = work.follow_cell(start.0, start.1, None);
        match next {
            None => CellType::Open,
            Some(ref n) if n.len() == 0 => CellType::Open,
            Some(ref n) => self.trace_round(&work, start, n.cells[0]),
        }
    }

    fn trace_start(&self, work: &Grid) -> (i64, i64) {
        let mut found: Option<(i64, i64)> = None;
        for cell in &self.cells {
            if work.lines_end(cell.0, cell.1) { found = Some(*cell); }
        }
        found.unwrap_or(self.first())
    }

    fn trace_round(&self, work: &Grid, start: (i64, i64), cell: (i64, i64)) -> CellType {
        let mut previous = start;
        let mut cell = cell;
        while cell != start {
            let next = work.follow_cell(cell.0, cell.1, Some(previous));
            let Some(next) = next else { return CellType::Open; };
            if next.len() == 0 { return CellType::Open; }
            if next.len() > 1 { return CellType::Undetermined; }
            previous = cell;
            cell = next.cells[0];
        }
        CellType::Closed
    }

    fn type_by_fill(&self, grid: &Grid) -> CellType {
        let mut buffer = self.fill_buffer(grid);
        let (fc, fr) = first_blank(&buffer);
        buffer.fill_continuous_area(fc, fr, '*');
        if buffer.has_blank_cells() { CellType::HasClosedArea } else { CellType::Open }
    }

    fn fill_buffer(&self, grid: &Grid) -> Grid {
        let mut temp = self.clone();
        temp.translate(1 - self.min_x(), 1 - self.min_y());
        let sub = grid
            .sub_grid(self.min_x() - 1, self.min_y() - 1,
                      (self.max_x() - self.min_x() + 3) as usize,
                      (self.max_y() - self.min_y() + 3) as usize)
            .unwrap_or_else(|| grid.clone());
        abstraction_buffer(&sub, &temp)
    }

    fn translate(&mut self, dx: i64, dy: i64) {
        for c in &mut self.cells { c.0 += dx; c.1 += dy; }
        self.index = self.cells.iter().copied().collect();
    }

    /// Ruby `filled_equivalent(grid)`.
    fn filled_equivalent(&self, grid: &Grid) -> CellSet {
        if self.cell_type(grid) == CellType::Open { return self.clone(); }
        let mut grid = self.make_into_grid();
        let seed = interior_seed(&grid);
        if let Some((sc, sr)) = seed {
            grid.fill_continuous_area(sc + 1, sr, '*');
            grid.all_non_blank()
        } else {
            self.clone()
        }
    }

    /// Ruby `break_truly_mixed(grid)`.
    fn break_truly_mixed(&self, grid: &Grid) -> Vec<CellSet> {
        let mut work = Grid::blank_like(grid);
        grid.copy_cells_to(self, &mut work);
        let mut result = self.mixed_parts(&work);
        let mut whats_left = self.clone();
        for set in &result { whats_left.subtract_set(set); }
        result.push(whats_left);
        result
    }

    fn mixed_parts(&self, work: &Grid) -> Vec<CellSet> {
        let mut result = vec![];
        let mut visited_ends = CellSet::new();
        for cell in &self.cells {
            if visited_ends.include(*cell) || !work.lines_end(cell.0, cell.1) { continue; }
            self.mixed_trace(work, *cell, &mut visited_ends, &mut result);
        }
        result
    }

    fn mixed_trace(&self, work: &Grid, cell: (i64, i64), visited: &mut CellSet, result: &mut Vec<CellSet>) {
        let mut set = CellSet::new();
        set.add(cell.0, cell.1);
        let next = work.follow_cell(cell.0, cell.1, None);
        if let Some(next) = next {
            if let Some(&first) = next.cells.first() {
                self.grow_mixed(work, &mut set, cell, first, visited);
            }
        }
        result.push(set);
    }

    fn grow_mixed(&self, work: &Grid, set: &mut CellSet, previous: (i64, i64), cell: (i64, i64), visited: &mut CellSet) {
        set.add(cell.0, cell.1);
        if work.lines_end(cell.0, cell.1) { return; }
        let follow = work.follow_cell(cell.0, cell.1, Some(previous));
        if let Some(follow) = follow {
            if follow.len() == 1 {
                self.leg_end(work, set, cell, follow.cells[0], visited);
            }
        }
    }

    fn leg_end(&self, work: &Grid, set: &mut CellSet, previous: (i64, i64), cell: (i64, i64), visited: &mut CellSet) {
        if work.lines_end(cell.0, cell.1) {
            visited.add(cell.0, cell.1);
            set.add(cell.0, cell.1);
        } else {
            self.grow_mixed(work, set, previous, cell, visited);
        }
    }
}

fn component_neighbours(cell: (i64, i64)) -> Vec<(i64, i64)> {
    vec![(cell.0, cell.1 - 1), (cell.0, cell.1 + 1), (cell.0 + 1, cell.1), (cell.0 - 1, cell.1)]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CellType {
    Open,
    Closed,
    Mixed,
    HasClosedArea,
    Undetermined,
}

fn first_blank(buffer: &Grid) -> (i64, i64) {
    for row in 0..buffer.height() {
        for col in 0..buffer.width() {
            if buffer.blank_cell(col as i64, row as i64) { return (col as i64, row as i64); }
        }
    }
    (0, 0)
}

fn interior_seed(grid: &Grid) -> Option<(i64, i64)> {
    for row in 0..grid.height() {
        for col in 0..grid.width() {
            let blank = |c: i64, r: i64| grid.blank_cell(c, r);
            if !grid.blank_cell(col as i64, row as i64)
                && blank(col as i64 + 1, row as i64)
                && blank(col as i64 - 1, row as i64)
            {
                return Some((col as i64, row as i64));
            }
        }
    }
    None
}

/// Ruby `AbstractCell.stamp_buffer`: a 3x buffer with `*` at every marked
/// footprint cell of the set.
fn abstraction_buffer(source_grid: &Grid, cell_set: &CellSet) -> Grid {
    let rows = vec![
        vec![' '; (source_grid.width() * 3) as usize];
        (source_grid.height() * 3) as usize
    ];
    let mut buffer = Grid { rows };
    for cell in &cell_set.cells { stamp_cell(&mut buffer, source_grid, *cell); }
    buffer
}

fn stamp_cell(buffer: &mut Grid, source_grid: &Grid, cell: (i64, i64)) {
    if source_grid.blank_cell(cell.0, cell.1) { return; }
    if let Some(footprint) = abstract_cell_footprint(source_grid, cell) {
        for (dc, dr) in footprint {
            buffer.set(cell.0 * 3 + dc, cell.1 * 3 + dr, '*');
        }
    }
}

/// Ruby `AbstractCell.for_cell`: the 3x3 footprints, chosen by class.
fn abstract_cell_footprint(grid: &Grid, cell: (i64, i64)) -> Option<Vec<(i64, i64)>> {
    const HORIZONTAL: [(i64, i64); 3] = [(0, 1), (1, 1), (2, 1)];
    const VERTICAL: [(i64, i64); 3] = [(1, 0), (1, 1), (1, 2)];
    let junction = |grid: &Grid, c: i64, r: i64| -> Option<Vec<(i64, i64)>> {
        if grid.cross(c, r) {
            Some(vec![(1, 0), (1, 1), (1, 2), (0, 1), (2, 1)])
        } else if grid.intersection_t(c, r) {
            Some(vec![(0, 1), (1, 1), (2, 1), (1, 2)])
        } else if grid.intersection_k(c, r) {
            Some(vec![(1, 0), (1, 1), (1, 2), (2, 1)])
        } else if grid.intersection_inverse_t(c, r) {
            Some(vec![(0, 1), (1, 1), (2, 1), (1, 0)])
        } else if grid.intersection_inverse_k(c, r) {
            Some(vec![(1, 0), (1, 1), (1, 2), (0, 1)])
        } else {
            None
        }
    };
    if let Some(j) = junction(grid, cell.0, cell.1) { return Some(j); }
    let cornerish = |grid: &Grid, c: i64, r: i64| -> Option<Vec<(i64, i64)>> {
        if grid.corner1(c, r) {
            Some(vec![(1, 1), (1, 2), (2, 1)])
        } else if grid.corner2(c, r) {
            Some(vec![(0, 1), (1, 1), (1, 2)])
        } else if grid.corner3(c, r) {
            Some(vec![(0, 1), (1, 0), (1, 1)])
        } else if grid.corner4(c, r) {
            Some(vec![(1, 0), (1, 1), (2, 1)])
        } else {
            None
        }
    };
    if let Some(c) = cornerish(grid, cell.0, cell.1) { return Some(c); }
    if grid.horizontal_line(cell.0, cell.1) { return Some(HORIZONTAL.to_vec()); }
    if grid.vertical_line(cell.0, cell.1) { return Some(VERTICAL.to_vec()); }
    if grid.cross_on_line(cell.0, cell.1) {
        return Some(vec![(1, 0), (1, 1), (1, 2), (0, 1), (2, 1)]);
    }
    if grid.star_on_line(cell.0, cell.1) {
        return Some(vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1), (0, 2), (1, 2), (2, 2)]);
    }
    None
}

// ---- Grid ----

const BOUNDARIES: [char; 7] = ['/', '\\', '|', '-', '*', '=', ':'];
const HORIZONTAL_LINES: [char; 2] = ['-', '='];
const VERTICAL_LINES: [char; 2] = ['|', ':'];
const DASHED_LINES: [char; 3] = [':', '~', '='];
const POINT_MARKERS: [char; 1] = ['*'];
const HUMAN_COLOR_CODES: [(&str, &str); 6] = [
    ("cGRE", "c9D9"), ("cBLU", "c55B"), ("cPNK", "cFAA"),
    ("cRED", "cE32"), ("cYEL", "cFF3"), ("cBLK", "c000"),
];

/// Ruby `Grid`: the character grid behind the whole pipeline.
#[derive(Clone)]
struct Grid {
    rows: Vec<Vec<char>>,
}

impl Grid {
    fn width(&self) -> usize { self.rows.first().map(|r| r.len()).unwrap_or(0) }
    fn height(&self) -> usize { self.rows.len() }

    /// Out-of-bounds reads return None, Java's char 0.
    fn get(&self, col: i64, row: i64) -> Option<char> {
        if col < 0 || row < 0 { return None; }
        if col > self.width() as i64 - 1 || row > self.height() as i64 - 1 { return None; }
        self.rows[row as usize].get(col as usize).copied()
    }

    fn set(&mut self, col: i64, row: i64, ch: char) {
        if col < 0 || row < 0 { return; }
        if col > self.width() as i64 - 1 || row > self.height() as i64 - 1 { return; }
        self.rows[row as usize][col as usize] = ch;
    }

    fn blank_cell(&self, col: i64, row: i64) -> bool {
        self.get(col, row) == Some(' ')
    }

    fn string_at(&self, col: i64, row: i64, len: usize) -> Option<String> {
        if col < 0 || row < 0 { return None; }
        if col > self.width() as i64 - 1 || row > self.height() as i64 - 1 { return None; }
        let r = &self.rows[row as usize];
        let start = col as usize;
        Some(r[start..(start + len).min(r.len())].iter().collect())
    }

    fn is_blankish(ch: Option<char>) -> bool { ch.is_none() || ch == Some(' ') }

    fn has_blank_cells(&self) -> bool {
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.blank_cell(col as i64, row as i64) { return true; }
            }
        }
        false
    }

    fn horizontal_line(&self, col: i64, row: i64) -> bool {
        matches!(self.get(col, row), Some(c) if HORIZONTAL_LINES.contains(&c))
    }

    fn vertical_line(&self, col: i64, row: i64) -> bool {
        matches!(self.get(col, row), Some(c) if VERTICAL_LINES.contains(&c))
    }

    fn line(&self, col: i64, row: i64) -> bool {
        self.horizontal_line(col, row) || self.vertical_line(col, row)
    }

    fn dashed_line_char(&self, col: i64, row: i64) -> bool {
        matches!(self.get(col, row), Some(c) if DASHED_LINES.contains(&c))
    }

    fn contains_dashed_line(&self, set: &CellSet) -> bool {
        set.cells.iter().any(|c| self.dashed_line_char(c.0, c.1))
    }

    fn copy_cells_to(&self, set: &CellSet, other: &mut Grid) {
        for (col, row) in &set.cells {
            if let Some(ch) = self.get(*col, *row) { other.set(*col, *row, ch); }
        }
    }

    fn fill_cells_with(&mut self, set: &CellSet, ch: char) {
        for (col, row) in &set.cells { self.set(*col, *row, ch); }
    }

    /// Ruby `fill_continuous_area`: flood over the seed's character.
    fn fill_continuous_area(&mut self, col: i64, row: i64, ch: char) -> CellSet {
        let old = self.get(col, row);
        let mut filled = CellSet::new();
        if old.is_none() || old == Some(ch) { return filled; }
        let mut stack = vec![(col, row)];
        while let Some(visit) = stack.pop() {
            self.set(visit.0, visit.1, ch);
            filled.add(visit.0, visit.1);
            for n in self.flood_neighbours(visit, old.unwrap()) { stack.push(n); }
        }
        filled
    }

    fn flood_neighbours(&self, cell: (i64, i64), old: char) -> Vec<(i64, i64)> {
        [(0, -1), (0, 1), (1, 0), (-1, 0)]
            .iter()
            .map(|(dc, dr)| (cell.0 + dc, cell.1 + dr))
            .filter(|n| self.get(n.0, n.1) == Some(old))
            .collect()
    }

    /// Ruby `boundaries_expanding_from`: the `*` cells a flood would hit.
    fn boundaries_expanding_from(&mut self, col: i64, row: i64) -> CellSet {
        let old = self.get(col, row);
        let mut boundaries = CellSet::new();
        let Some(old) = old else { return boundaries; };
        let mut stack = vec![(col, row)];
        while let Some(visit) = stack.pop() {
            self.set(visit.0, visit.1, '\u{1}');
            for n in self.expansion_neighbours(visit, old, &mut boundaries) {
                stack.push(n);
            }
        }
        boundaries
    }

    fn expansion_neighbours(&self, cell: (i64, i64), old: char, boundaries: &mut CellSet) -> Vec<(i64, i64)> {
        let mut out = vec![];
        for (dc, dr) in [(0, -1), (0, 1), (1, 0), (-1, 0)] {
            let next = (cell.0 + dc, cell.1 + dr);
            match self.get(next.0, next.1) {
                Some(c) if c == old => out.push(next),
                Some('*') => boundaries.add(next.0, next.1),
                _ => {}
            }
        }
        out
    }

    /// Ruby `all_boundaries`.
    fn all_boundaries(&self) -> CellSet {
        let mut cells = CellSet::new();
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.boundary(col as i64, row as i64) { cells.add(col as i64, row as i64); }
            }
        }
        cells
    }

    fn all_non_blank(&self) -> CellSet {
        let mut cells = CellSet::new();
        for row in 0..self.height() {
            for col in 0..self.width() {
                if !self.blank_cell(col as i64, row as i64) { cells.add(col as i64, row as i64); }
            }
        }
        cells
    }

    fn blanks_between_characters(&self) -> CellSet {
        let mut cells = CellSet::new();
        for row in 0..self.height() {
            for col in 0..self.width() {
                if !self.blank_cell(col as i64, row as i64) { continue; }
                if self.blank_cell(col as i64 + 1, row as i64)
                    || self.blank_cell(col as i64 - 1, row as i64)
                {
                    continue;
                }
                cells.add(col as i64, row as i64);
            }
        }
        cells
    }

    fn boundary(&self, col: i64, row: i64) -> bool {
        let Some(ch) = self.get(col, row) else { return false; };
        if ch == '+' || ch == '\\' || ch == '/' {
            self.shaped_boundary(col, row)
        } else {
            BOUNDARIES.contains(&ch) && !self.lone_diagonal(col, row)
        }
    }

    fn shaped_boundary(&self, col: i64, row: i64) -> bool {
        self.intersection(col, row) || self.corner(col, row) || self.stub(col, row)
            || self.cross_on_line(col, row)
    }

    fn arrowhead(&self, col: i64, row: i64) -> bool {
        self.north_arrowhead(col, row) || self.south_arrowhead(col, row)
            || self.west_arrowhead(col, row) || self.east_arrowhead(col, row)
    }

    fn north_arrowhead(&self, col: i64, row: i64) -> bool { self.get(col, row) == Some('^') }
    fn east_arrowhead(&self, col: i64, row: i64) -> bool { self.get(col, row) == Some('>') }
    fn west_arrowhead(&self, col: i64, row: i64) -> bool { self.get(col, row) == Some('<') }

    fn south_arrowhead(&self, col: i64, row: i64) -> bool {
        matches!(self.get(col, row), Some('v') | Some('V')) && self.vertical_line(col, row - 1)
    }

    fn arrowheads(&self) -> Vec<(i64, i64)> {
        let mut cells = vec![];
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.arrowhead(col as i64, row as i64) { cells.push((col as i64, row as i64)); }
            }
        }
        cells
    }

    fn color_codes(&self) -> Vec<((i64, i64), String)> {
        let mut codes = vec![];
        let re = regex::Regex::new(r"\Ac[A-F0-9]{3}\z").unwrap();
        for row in 0..self.height() {
            for col in 0..(self.width() as i64 - 3).max(0) {
                if let Some(pair) = self.color_code_at(&re, col, row as i64) { codes.push(pair); }
            }
        }
        codes
    }

    fn color_code_at(&self, re: &regex::Regex, col: i64, row: i64) -> Option<((i64, i64), String)> {
        let text = self.string_at(col, row, 4)?;
        if !re.is_match(&text) { return None; }
        Some(((col, row), hex_color(&text)))
    }

    fn markup_tags(&self) -> Vec<((i64, i64), String)> {
        const MARKUP_TAGS: [&str; 7] = ["d", "s", "io", "c", "mo", "tr", "o"];
        let mut tags = vec![];
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.get(col as i64, row as i64) != Some('{') { continue; }
                let rest: String = self.rows[row][col..].iter().collect();
                let Some(close) = rest.find('}') else { continue; };
                let tag = &rest[1..close];
                if !MARKUP_TAGS.contains(&tag) { continue; }
                tags.push(((col as i64, row as i64), tag.to_string()));
            }
        }
        tags
    }

    /// Ruby `remove_non_text`.
    fn remove_non_text(&mut self) {
        self.remove_arrowheads();
        self.remove_color_codes();
        self.remove_boundaries();
        self.remove_markup_tags();
    }

    fn remove_arrowheads(&mut self) {
        let heads = self.arrowheads();
        for (col, row) in heads { self.set(col, row, ' '); }
    }

    fn remove_color_codes(&mut self) {
        let cells: Vec<(i64, i64)> = self.color_codes().into_iter().map(|(c, _)| c).collect();
        for (col, row) in cells {
            for step in 0..4 { self.set(col + step, row, ' '); }
        }
    }

    /// Removed in two stages: the boundary decision reads neighbours.
    fn remove_boundaries(&mut self) {
        let mut doomed = vec![];
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.boundary(col as i64, row as i64) { doomed.push((col as i64, row as i64)); }
            }
        }
        for (col, row) in doomed { self.set(col, row, ' '); }
    }

    fn remove_markup_tags(&mut self) {
        let tags = self.markup_tags();
        for ((col, row), tag) in tags {
            for step in 0..(tag.len() + 2) { self.set(col + step as i64, row, ' '); }
        }
    }

    /// Ruby `strings`: continuous non-blank runs; a run ends at TWO blanks.
    fn strings(&self) -> Vec<((i64, i64), String)> {
        let mut runs = vec![];
        for row in 0..self.height() { runs.extend(self.row_runs(row)); }
        runs
    }

    fn row_runs(&self, row: usize) -> Vec<((i64, i64), String)> {
        let mut runs = vec![];
        let mut col: i64 = 0;
        while (col as usize) < self.width() {
            if self.blank_cell(col, row as i64) {
                col += 1;
            } else {
                let run = self.string_run(col, row as i64);
                col = run.0.0 + run.1.chars().count() as i64 + 1;
                runs.push(run);
            }
        }
        runs
    }

    fn string_run(&self, col: i64, row: i64) -> ((i64, i64), String) {
        let string = self.get(col, row).map(|c| c.to_string()).unwrap_or_default();
        let ch = self.get(col + 1, row);
        ((col, row), self.grow_string(string, ch, col + 2, row))
    }

    fn grow_string(&self, mut string: String, ch: Option<char>, mut cursor: i64, row: i64) -> String {
        let mut ch = ch;
        loop {
            string.push(ch.unwrap_or('\0'));
            let c = self.get(cursor, row);
            let following = self.get(cursor + 1, row);
            cursor += 1;
            if Self::is_blankish(c) && Self::is_blankish(following) { break; }
            ch = c;
        }
        string
    }

    fn sub_grid(&self, col: i64, row: i64, sub_w: usize, sub_h: usize) -> Option<Grid> {
        if col < 0 || row < 0
            || col + sub_w as i64 > self.width() as i64
            || row + sub_h as i64 > self.height() as i64
        {
            return None;
        }
        let rows = (0..sub_h)
            .map(|offset| self.rows[(row as usize) + offset][col as usize..(col as usize) + sub_w].to_vec())
            .collect();
        Some(Grid { rows })
    }

    fn testing_sub_grid(&self, col: i64, row: i64) -> Option<Grid> {
        self.sub_grid(col - 1, row - 1, 3, 3)
    }

    fn blank_like(other: &Grid) -> Grid {
        Grid {
            rows: vec![vec![' '; other.width()]; other.height()],
        }
    }

    fn entry_point(&self, col: i64, row: i64, id: i64) -> bool {
        const E1: [char; 1] = ['\\'];
        const E2: [char; 5] = ['|', ':', '+', '\\', '/'];
        const E3: [char; 1] = ['/'];
        const E4: [char; 5] = ['-', '=', '+', '\\', '/'];
        let ch = self.get(col, row);
        match id {
            1 => matches!(ch, Some(c) if E1.contains(&c)),
            2 | 6 => matches!(ch, Some(c) if E2.contains(&c)),
            3 | 7 => matches!(ch, Some(c) if E3.contains(&c)),
            4 | 8 => matches!(ch, Some(c) if E4.contains(&c)),
            _ => false,
        }
    }

    /// Ruby `follow_cell`: the cells a trace step can move to.
    fn follow_cell(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> Option<CellSet> {
        let mut result = if self.intersection(col, row) {
            self.follow_intersection(col, row, blocked)
        } else if self.corner(col, row) {
            self.follow_corner(col, row, blocked)
        } else if self.line(col, row) {
            self.follow_line(col, row, blocked)
        } else if self.stub(col, row) {
            self.follow_stub(col, row, blocked)
        } else if self.cross_on_line(col, row) {
            self.follow_cross_on_line(col, row, blocked)
        } else {
            return None;
        };
        if let Some(b) = blocked { result.delete(b); }
        Some(result)
    }

    fn follow_intersection(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> CellSet {
        let arms = [(0, -1, 6), (0, 1, 2), (1, 0, 8), (-1, 0, 4)];
        let mut result = CellSet::new();
        for (dc, dr, entry) in arms {
            let hit = (col + dc, row + dr);
            if self.entry_point(hit.0, hit.1, entry) { result.add(hit.0, hit.1); }
        }
        if let Some(b) = blocked { result.delete(b); }
        result
    }

    fn follow_line(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> CellSet {
        let steps: [(i64, i64); 2] = if self.horizontal_line(col, row) {
            [(1, 0), (-1, 0)]
        } else {
            [(0, -1), (0, 1)]
        };
        let mut result = CellSet::new();
        for (dc, dr) in steps {
            if self.boundary(col + dc, row + dr) { result.add(col + dc, row + dr); }
        }
        if let Some(b) = blocked { result.delete(b); }
        result
    }

    fn follow_corner(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> CellSet {
        let mut result = CellSet::new();
        if self.corner1(col, row) {
            if blocked != Some((col, row + 1)) { result.add(col, row + 1); }
            if blocked != Some((col + 1, row)) { result.add(col + 1, row); }
        } else if self.corner2(col, row) {
            if blocked != Some((col, row + 1)) { result.add(col, row + 1); }
            if blocked != Some((col - 1, row)) { result.add(col - 1, row); }
        } else if self.corner3(col, row) {
            if blocked != Some((col, row - 1)) { result.add(col, row - 1); }
            if blocked != Some((col - 1, row)) { result.add(col - 1, row); }
        } else {
            if blocked != Some((col, row - 1)) { result.add(col, row - 1); }
            if blocked != Some((col + 1, row)) { result.add(col + 1, row); }
        }
        result
    }

    fn follow_stub(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> CellSet {
        let probes = [(1, 0), (-1, 0), (0, -1), (0, 1)];
        let mut result = CellSet::new();
        if let Some((dc, dr)) = probes.iter().find(|(dc, dr)| self.boundary(col + dc, row + dr)) {
            result.add(col + dc, row + dr);
        }
        if let Some(b) = blocked { result.delete(b); }
        result
    }

    fn follow_cross_on_line(&self, col: i64, row: i64, blocked: Option<(i64, i64)>) -> CellSet {
        let mut result = CellSet::new();
        if self.horizontal_cross_on_line(col, row) {
            result.add(col + 1, row);
            result.add(col - 1, row);
        } else {
            result.add(col, row - 1);
            result.add(col, row + 1);
        }
        if let Some(b) = blocked { result.delete(b); }
        result
    }


    fn horizontal_cross_on_line(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &HORIZONTAL_CROSS_ON_LINE)
    }

    fn normal_corner(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &NORMAL_CORNER)
    }

    fn round_corner(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &ROUND_CORNER)
    }

    fn corner1(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &CORNER1) }
    fn corner2(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &CORNER2) }
    fn corner3(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &CORNER3) }
    fn corner4(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &CORNER4) }
    fn corner(&self, col: i64, row: i64) -> bool {
        self.normal_corner(col, row) || self.round_corner(col, row)
    }

    fn intersection(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &INTERSECTION)
    }

    fn intersection_t(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &T_GROUP) }
    fn intersection_k(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &K_GROUP) }
    fn intersection_inverse_t(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &INVERSE_T)
    }
    fn intersection_inverse_k(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &INVERSE_K)
    }

    fn stub(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &STUB) }

    fn lines_end(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &LINES_END)
    }

    fn cross_on_line(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &CROSS_ON_LINE)
    }

    fn star_on_line(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &STAR_ON_LINE)
    }

    fn lone_diagonal(&self, col: i64, row: i64) -> bool {
        matches_any_multi(self, col, row, &LONE_DIAGONAL)
    }

    fn cross(&self, col: i64, row: i64) -> bool { matches_any_multi(self, col, row, &CROSS_GROUP) }

    /// Ruby `replace_bullets`.
    fn replace_bullets(&mut self) {
        for row in 0..self.height() {
            for col in 0..self.width() {
                if !self.bullet(col as i64, row as i64) { continue; }
                self.set(col as i64, row as i64, ' ');
                self.set(col as i64 + 1, row as i64, '\u{2022}');
            }
        }
    }

    fn bullet(&self, col: i64, row: i64) -> bool {
        if !matches!(self.get(col, row), Some('o') | Some('*')) { return false; }
        if !(self.blank_cell(col + 1, row) && self.blank_cell(col - 1, row)) { return false; }
        matches!(self.get(col + 2, row), Some(c) if c.is_alphanumeric())
    }

    /// Ruby `replace_human_color_codes`.
    fn replace_human_color_codes(&mut self) {
        for row in &mut self.rows {
            for (human, hex) in HUMAN_COLOR_CODES {
                let pattern = human.to_string();
                *row = replace_all(row, &pattern, hex);
            }
        }
    }

    /// Ruby `replace_type_on_line`.
    fn replace_type_on_line(&mut self) {
        for row in 0..self.height() {
            for col in 0..self.width() {
                if let Some(ch) = self.type_on_line_char(col as i64, row as i64) {
                    self.set(col as i64, row as i64, ch);
                }
            }
        }
    }

    fn type_on_line_char(&self, col: i64, row: i64) -> Option<char> {
        if !matches!(self.get(col, row), Some(c) if c.is_alphanumeric()) { return None; }
        self.between_lines(col, row)
    }

    fn between_lines(&self, col: i64, row: i64) -> Option<char> {
        let horizontal = self.hrun(col, row);
        if horizontal && self.vrun(col, row) { return Some('+'); }
        if horizontal { return Some('-'); }
        if self.vrun(col, row) { Some('|') } else { None }
    }

    fn hrun(&self, col: i64, row: i64) -> bool {
        self.horizontal_line(col - 1, row) && self.horizontal_line(col + 1, row)
    }

    fn vrun(&self, col: i64, row: i64) -> bool {
        self.vertical_line(col, row - 1) && self.vertical_line(col, row + 1)
    }

    /// Ruby `replace_point_markers_on_line`.
    fn replace_point_markers_on_line(&mut self) {
        for row in 0..self.height() {
            for col in 0..self.width() {
                if let Some(ch) = self.marker_on_line_char(col as i64, row as i64) {
                    self.set(col as i64, row as i64, ch);
                }
            }
        }
    }

    fn marker_on_line_char(&self, col: i64, row: i64) -> Option<char> {
        if !matches!(self.get(col, row), Some(c) if POINT_MARKERS.contains(&c)) { return None; }
        if !self.star_on_line(col, row) { return None; }
        self.beside_lines(col, row)
    }

    fn beside_lines(&self, col: i64, row: i64) -> Option<char> {
        let horizontal = self.horizontal_line(col + 1, row) || self.horizontal_line(col - 1, row);
        let vertical = self.vertical_line(col, row - 1) || self.vertical_line(col, row + 1);
        if horizontal && vertical { return Some('+'); }
        if horizontal { return Some('-'); }
        Some('|')
    }

    fn marker_on_line(&self, col: i64, row: i64) -> bool {
        matches!(self.get(col, row), Some(c) if POINT_MARKERS.contains(&c))
            && self.star_on_line(col, row)
    }

    fn point_markers_on_line(&self) -> CellSet {
        let mut cells = CellSet::new();
        for row in 0..self.height() {
            for col in 0..self.width() {
                if self.marker_on_line(col as i64, row as i64) { cells.add(col as i64, row as i64); }
            }
        }
        cells
    }

    /// Ruby `Grid.from_lines` + `Dedent` handled by the caller.
    fn from_body_lines(body_lines: &[String]) -> Grid {
        let trimmed = trim_trailing_blank_rows(body_lines);
        let tabbed: Vec<String> = trimmed.iter().map(|l| expand_tabs(l)).collect();
        let mut grid = Grid::from_bordered(&tabbed);
        grid.replace_bullets();
        grid.replace_human_color_codes();
        grid
    }

    fn from_bordered(tabbed: &[String]) -> Grid {
        let width = tabbed.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let blank: Vec<char> = vec![' '; width + 4];
        let mut rows = vec![blank.clone(), blank.clone()];
        for line in tabbed {
            let mut row: Vec<char> = vec![' ', ' '];
            row.extend(line.chars());
            row.resize(width + 4, ' ');
            rows.push(row);
        }
        rows.push(blank.clone());
        rows.push(blank);
        Grid { rows }
    }

    /// Ruby `blank_like`.
    fn blank_like_grid(other: &Grid) -> Grid {
        Grid { rows: vec![vec![' '; other.width()]; other.height()] }
    }
}

fn replace_all(row: &[char], pattern: &str, hex: &str) -> Vec<char> {
    let text: String = row.iter().collect();
    text.replace(pattern, hex).chars().collect()
}

fn trim_trailing_blank_rows(lines: &[String]) -> Vec<String> {
    let last = lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(0);
    lines[..last].to_vec()
}

fn expand_tabs(line: &str) -> String {
    let mut expanded = String::new();
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = 8 - (expanded.chars().count() % 8);
            expanded.push_str(&" ".repeat(spaces));
        } else {
            expanded.push(ch);
        }
    }
    expanded
}

// ---- Pattern ----

/// Ruby `Pattern`: three row patterns compiled to regexps, three tokens per
/// row read.
struct Pattern {
    regexps: [regex::Regex; 3],
}

fn pattern_class(two: &str) -> Option<&'static str> {
    Some(match two {
        "." => ".",
        "[" => "[^|:]",
        "|" => "[|:]",
        "-" => "[-=]",
        "!" => "[^-=\\/\\\\+|:]",
        "b" => "[-=\\/\\\\+|:]",
        "^" => "[\\/\\\\+|:]",
        "(" => "[-=\\/\\\\+]",
        "~" => ".",
        "+" => "\\+",
        "\\" => "\\\\",
        "/" => "/",
        "s" => "[-=+|:]",
        "S" => "[\\/\\\\]",
        "*" => "\\*",
        "1" => "[\\\\]",
        "2" => "[|:+\\/\\\\]",
        "3" => "[\\/]",
        "4" => "[-=+\\/\\\\]",
        "5" => "[\\\\]",
        "6" => "[|:+\\/\\\\]",
        "7" => "[\\/]",
        "8" => "[-=+\\/\\\\]",
        "%1" => "[^\\\\]",
        "%2" => "[^|:+\\/\\\\]",
        "%3" => "[^\\/]",
        "%4" => "[^-=+\\/\\\\]",
        "%5" => "[^\\\\]",
        "%6" => "[^|:+\\/\\\\]",
        "%7" => "[^\\/]",
        "%8" => "[^|:+\\/\\\\]",
        _ => return None,
    })
}

fn to_regexp(pattern: &str) -> String {
    let mut source = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut cursor = 0;
    let mut token = 0;
    while cursor < chars.len() && token < 3 {
        let two: String = chars[cursor..(cursor + 2).min(chars.len())].iter().collect();
        if let Some(class) = pattern_class(&two) {
            source.push_str(class);
            cursor += 2;
        } else {
            let one = chars[cursor].to_string();
            match pattern_class(&one) {
                Some(class) => source.push_str(class),
                None => source.push_str(&regex::escape(&one)),
            }
            cursor += 1;
        }
        token += 1;
    }
    source
}

impl Pattern {
    fn new(row1: &str, row2: &str, row3: &str) -> Pattern {
        Pattern {
            regexps: [
                regex::Regex::new(&format!("\\A{}\\z", to_regexp(row1))).unwrap(),
                regex::Regex::new(&format!("\\A{}\\z", to_regexp(row2))).unwrap(),
                regex::Regex::new(&format!("\\A{}\\z", to_regexp(row3))).unwrap(),
            ],
        }
    }

    fn matched_by(&self, grid: &Grid) -> bool {
        self.regexps.iter().enumerate().all(|(row, regexp)| {
            let text = grid.rows.get(row).map(|r| r.iter().collect::<String>()).unwrap_or_default();
            regexp.is_match(&text)
        })
    }
}

static CROSS_GROUP: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".6.", "4+8", ".2.")]);
static K_GROUP: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".6.", "%4+8", ".2.")]);
static INVERSE_K: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".6.", "4+%8", ".2.")]);
static T_GROUP: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".%6.", "4+8", ".2.")]);
static INVERSE_T: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".6.", "4+8", ".%2.")]);

static NORMAL_CORNER: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".[.", "~+(", ".^."),
    Pattern::new(".[.", "(+~", ".^."),
    Pattern::new(".^.", "(+~", ".[."),
    Pattern::new(".^.", "~+(", ".[."),
]);
static ROUND_CORNER: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".[.", "~/4", ".2."),
    Pattern::new(".[.", "4\\~", ".2."),
    Pattern::new(".6.", "4/~", ".[."),
    Pattern::new(".6.", "~\\8", ".[."),
]);
static STUB: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new("!^!", "!+!", ".!."),
    Pattern::new("!^!", "!+!", ".-."),
    Pattern::new("!!.", "(+!", "!!."),
    Pattern::new("!!.", "(+|", "!!."),
    Pattern::new(".!.", "!+!", "!^!"),
    Pattern::new(".-.", "!+!", "!^!"),
    Pattern::new(".!!", "!+(", ".!!"),
    Pattern::new(".!!", "|+(", ".!!"),
]);
#[allow(dead_code)]
static VERTICAL_LINES_END: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".^.", ".|.", ".!."),
    Pattern::new(".^.", ".|.", ".-."),
    Pattern::new(".!.", ".|.", ".^."),
    Pattern::new(".-.", ".|.", ".^."),
]);
#[allow(dead_code)]
static HORIZONTAL_LINES_END: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new("...", "(-!", "..."),
    Pattern::new("...", "(-|", "..."),
    Pattern::new("...", "!-(", "..."),
    Pattern::new("...", "|-(", "..."),
]);
static HORIZONTAL_CROSS_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new("...", "(+(", "...")]);
#[allow(dead_code)]
static VERTICAL_CROSS_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![Pattern::new(".^.", ".+.", ".^.")]);
#[allow(dead_code)]
static HORIZONTAL_STAR_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new("...", "(*(", "..."),
    Pattern::new("...", "!*(", "..."),
    Pattern::new("...", "(*!", "..."),
]);
#[allow(dead_code)]
static VERTICAL_STAR_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".^.", ".*.", ".^."),
    Pattern::new(".!.", ".*.", ".^."),
    Pattern::new(".^.", ".*.", ".!."),
]);
static LONE_DIAGONAL: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".%6%7", "%4/%8", "%3%2."),
    Pattern::new("%1%6.", "%4\\%8", ".%2%5"),
]);

static CORNER1: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".[.", "~+(", ".^."), Pattern::new(".[.", "~/4", ".2."),
]);
static CORNER2: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".[.", "(+~", ".^."), Pattern::new(".[.", "4\\~", ".2."),
]);
static CORNER3: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".^.", "(+~", ".[."), Pattern::new(".6.", "4/~", ".[."),
]);
static CORNER4: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".^.", "~+(", ".[."), Pattern::new(".6.", "~\\8", ".[."),
]);
static INTERSECTION: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".6.", "4+8", ".2."),
    Pattern::new(".6.", "%4+8", ".2."),
    Pattern::new(".%6.", "4+8", ".2."),
    Pattern::new(".6.", "4+%8", ".2."),
    Pattern::new(".6.", "4+8", ".%2."),
]);
static CROSS_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new("...", "(+(", "..."),
    Pattern::new(".^.", ".+.", ".^."),
]);
static STAR_ON_LINE: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new("...", "(*(", "..."),
    Pattern::new("...", "!*(", "..."),
    Pattern::new("...", "(*!", "..."),
    Pattern::new(".^.", ".*.", ".^."),
    Pattern::new(".!.", ".*.", ".^."),
    Pattern::new(".^.", ".*.", ".!."),
]);
static LINES_END: LazyLock<Vec<Pattern>> = LazyLock::new(|| vec![
    Pattern::new(".^.", ".|.", ".!."),
    Pattern::new(".^.", ".|.", ".-."),
    Pattern::new(".!.", ".|.", ".^."),
    Pattern::new(".-.", ".|.", ".^."),
    Pattern::new("...", "(-!", "..."),
    Pattern::new("...", "(-|", "..."),
    Pattern::new("...", "!-(", "..."),
    Pattern::new("...", "|-(", "..."),
    Pattern::new("!^!", "!+!", ".!."),
    Pattern::new("!^!", "!+!", ".-."),
    Pattern::new("!!.", "(+!", "!!."),
    Pattern::new("!!.", "(+|", "!!."),
    Pattern::new(".!.", "!+!", "!^!"),
    Pattern::new(".-.", "!+!", "!^!"),
    Pattern::new(".!!", "!+(", ".!!"),
    Pattern::new(".!!", "|+(", ".!!"),
]);

/// Ruby `matches_any?`: the 3x3 sub grid around the cell against a group.
fn matches_any_multi(grid: &Grid, col: i64, row: i64, group: &LazyLock<Vec<Pattern>>) -> bool {
    let Some(sub) = grid.testing_sub_grid(col, row) else { return false; };
    group.iter().any(|p| p.matched_by(&sub))
}

// ---- Builder ----

const CELL_WIDTH: f64 = 10.0;
const CELL_HEIGHT: f64 = 14.0;
const ASCENT_RATIO: f64 = 0.78;
const WIDTH_RATIO: f64 = 0.55;

#[derive(Clone, Copy, PartialEq, Debug)]
enum PointType {
    Normal,
    Round,
}

#[derive(Clone, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
    kind: PointType,
    locked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ShapeKind {
    Rectangle,
    Document,
    Storage,
    Io,
    Decision,
    ManualOperation,
    Trapezoid,
    Ellipse,
    Line,
    Arrowhead,
    PointMarker,
}

#[derive(Clone, Debug)]
struct Shape {
    points: Vec<Point>,
    kind: ShapeKind,
    closed: bool,
    dashed: bool,
    fill: Option<String>,
    stroke: Option<String>,
}

impl Shape {
    fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut xs: Vec<f64> = self.points.iter().map(|p| p.x).collect();
        let mut ys: Vec<f64> = self.points.iter().map(|p| p.y).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (xs[0], ys[0], xs[xs.len() - 1], ys[ys.len() - 1])
    }

    fn calculate_area(&self) -> f64 {
        let mut sum = 0.0;
        let n = self.points.len();
        for (index, point) in self.points.iter().enumerate() {
            let other = &self.points[(index + 1) % n];
            sum += (point.x * other.y) - (other.x * point.y);
        }
        sum
    }

    fn area(&self) -> f64 {
        (self.calculate_area().abs() / 2.0).floor()
    }

    fn drops_shadow(&self) -> bool {
        self.closed
            && !matches!(self.kind, ShapeKind::Arrowhead | ShapeKind::PointMarker)
            && !self.dashed
    }

    fn new_line() -> Shape {
        Shape {
            points: vec![],
            kind: ShapeKind::Line,
            closed: false,
            dashed: false,
            fill: None,
            stroke: Some("#000000".into()),
        }
    }
}

#[derive(Clone, Debug)]
struct TextRun {
    x: f64,
    y: f64,
    string: String,
    font_size: f64,
    anchor: &'static str,
    fill: String,
}

struct Built {
    shapes: Vec<Shape>,
    composites: Vec<Vec<Shape>>,
    texts: Vec<TextRun>,
    width: f64,
    height: f64,
    #[allow(dead_code)]
    transparent: bool,
    #[allow(dead_code)]
    shadows: bool,
    cell_width: f64,
    cell_height: f64,
}

#[derive(Clone, Copy)]
struct BuilderOptions {
    separation: bool,
    round_corners: bool,
    cell_width: f64,
    cell_height: f64,
}

struct Builder {
    options: BuilderOptions,
}

impl Builder {
    fn new(o: &Options) -> Builder {
        Builder {
            options: BuilderOptions {
                separation: o.separation,
                round_corners: o.round_corners,
                cell_width: (CELL_WIDTH * o.scale).round(),
                cell_height: (CELL_HEIGHT * o.scale).round(),
            },
        }
    }
    #[allow(dead_code)]
    fn unused_options(&self) -> Options {
        Options { separation: false, shadows: false, round_corners: false, transparent: false, scale: 1.0 }
    }

    fn cell_left(&self, col: i64) -> f64 { col as f64 * self.options.cell_width }
    fn cell_top(&self, row: i64) -> f64 { row as f64 * self.options.cell_height }



    fn build(&self, grid: &Grid, options: &Options) -> Built {
        let work = self.prepare_work(grid);
        let (mut closed_sets, open_sets) = self.boundary_sets(&work);
        self.remove_obsolete_shapes(&work, &mut closed_sets);
        let mut shapes = self.closed_shapes(&work, &closed_sets);
        if options.separation { self.separate_common_edges(&mut shapes); }
        let (extras, composites) = self.open_geometry(&work, grid, &open_sets);
        shapes.extend(extras);
        self.decorate(grid, &work, &mut shapes);
        self.finish(grid, shapes, composites, options)
    }

    fn prepare_work(&self, grid: &Grid) -> Grid {
        let mut work = grid.clone();
        work.replace_type_on_line();
        work.replace_point_markers_on_line();
        work
    }

    fn closed_shapes(&self, work: &Grid, closed_sets: &[CellSet]) -> Vec<Shape> {
        closed_sets
            .iter()
            .filter_map(|set| self.closed_shape(work, set))
            .collect()
    }

    /// Ruby `closed_shape`: the set is CLOSED, a single cycle; trace it.
    fn closed_shape(&self, work: &Grid, set: &CellSet) -> Option<Shape> {
        let mut shape = Shape {
            points: vec![],
            kind: ShapeKind::Rectangle,
            closed: true,
            dashed: work.contains_dashed_line(set),
            fill: None,
            stroke: Some("#000000".into()),
        };
        let start = set.first();
        self.add_corner_point(work, &mut shape, start);
        let next = work.follow_cell(start.0, start.1, None)?;
        self.walk_closed(work, &mut shape, start, next.cells[0], start);
        Some(shape)
    }

    fn add_corner_point(&self, work: &Grid, shape: &mut Shape, cell: (i64, i64)) {
        if work.corner(cell.0, cell.1) {
            shape.points.push(self.point_for_cell(work, cell.0, cell.1));
        }
    }

    fn walk_closed(&self, work: &Grid, shape: &mut Shape, start: (i64, i64), cell: (i64, i64), previous: (i64, i64)) {
        self.add_corner_point(work, shape, cell);
        let mut cell = cell;
        let mut previous = previous;
        while cell != start {
            let next_cells = work.follow_cell(cell.0, cell.1, Some(previous));
            previous = cell;
            let Some(next) = next_cells else { break; };
            cell = next.cells[0];
            if cell != start { self.add_corner_point(work, shape, cell); }
        }
    }

    // ---- boundary sets ----

    fn boundary_sets(&self, work: &Grid) -> (Vec<CellSet>, Vec<CellSet>) {
        let mut candidates = self.candidate_boundaries(work);
        let (closed, _) = classify_boundaries(&candidates, work);
        let mixed: Vec<CellSet> = candidates
            .iter()
            .filter(|set| set.cell_type(work) == CellType::Mixed)
            .cloned()
            .collect();
        for set in &mixed { self.resolve_mixed(set, &closed, &mut candidates, work); }
        classify_boundaries(&candidates, work)
    }

    fn candidate_boundaries(&self, work: &Grid) -> Vec<CellSet> {
        let mut found = vec![];
        for set in self.distinct_shapes(work) {
            found.extend(self.region_boundaries(work, &set));
        }
        CellSet::remove_duplicates(found)
    }

    fn resolve_mixed(&self, set: &CellSet, closed: &[CellSet], candidates: &mut Vec<CellSet>, work: &Grid) {
        if closed.is_empty() {
            candidates.retain(|c| !cells_equal(c, set));
            for part in set.break_truly_mixed(work) { candidates.push(part); }
        } else {
            self.subtract_and_split(set, closed, candidates, work);
        }
    }

    fn subtract_and_split(&self, set: &CellSet, closed: &[CellSet], candidates: &mut Vec<CellSet>, work: &Grid) {
        let mut set = set.clone();
        for done in closed { set.subtract_set(done); }
        if set.cell_type(work) != CellType::Open { return; }
        candidates.retain(|c| !cells_equal(c, &set));
        for part in set.break_into_distinct_boundaries(work) { candidates.push(part); }
    }

    fn region_boundaries(&self, work: &Grid, set: &CellSet) -> Vec<CellSet> {
        let mut found = vec![];
        let mut buffer = Grid {
            rows: vec![vec![' '; work.width() * 3]; work.height() * 3],
        };
        for row in 0..work.height() * 3 {
            for col in 0..work.width() * 3 {
                found.extend(self.flood_region(&mut buffer, work, set, col as i64, row as i64));
            }
        }
        found
    }

    fn flood_region(&self, buffer: &mut Grid, work: &Grid, set: &CellSet, col: i64, row: i64) -> Vec<CellSet> {
        if !buffer.blank_cell(col, row) { return vec![]; }
        let mut source = abstraction_buffer(work, set);
        let boundaries = source.boundaries_expanding_from(col, row);
        if boundaries.len() == 0 { return vec![]; }
        let filled = source.fill_continuous_area(col, row, '*');
        buffer.fill_cells_with(&filled, '*');
        buffer.fill_cells_with(&boundaries, '-');
        vec![boundaries.make_scaled_one_third_equivalent()]
    }

    fn distinct_shapes(&self, work: &Grid) -> Vec<CellSet> {
        let all = work.all_boundaries();
        let buffer = abstraction_buffer(work, &all);
        buffer
            .all_non_blank()
            .break_into_distinct()
            .iter()
            .map(|component| {
                let mut scaled = CellSet::new();
                for cell in &component.cells { scaled.add(cell.0 / 3, cell.1 / 3); }
                scaled
            })
            .collect()
    }

    fn remove_obsolete_shapes(&self, work: &Grid, closed_sets: &mut Vec<CellSet>) {
        let filled: Vec<CellSet> = closed_sets.iter().map(|set| set.filled_equivalent(work)).collect();
        let mut doomed = vec![];
        for index in 0..filled.len() {
            if self.obsolete(&filled, index) { doomed.push(index); }
        }
        for index in doomed.into_iter().rev() { closed_sets.remove(index); }
    }

    fn obsolete(&self, filled: &[CellSet], index: usize) -> bool {
        let mine = &filled[index];
        let others: Vec<&CellSet> = filled
            .iter()
            .enumerate()
            .filter(|(other_index, other)| *other_index != index && other.has_common_cells(mine))
            .map(|(_, other)| other)
            .collect();
        if others.len() < 2 { return false; }
        let mut family: Vec<&CellSet> = vec![mine];
        family.extend(others);
        let largest = family
            .iter()
            .max_by_key(|set| set.len())
            .unwrap();
        let mut sum = CellSet::new();
        for part in &family {
            if !std::ptr::eq(*part, *largest) {
                for cell in &part.cells { sum.add(cell.0, cell.1); }
            }
        }
        cells_equal(largest, &sum)
    }

    // ---- open geometry ----

    fn open_geometry(&self, work: &Grid, grid: &Grid, open_sets: &[CellSet]) -> (Vec<Shape>, Vec<Vec<Shape>>) {
        let mut extras = vec![];
        let mut composites = vec![];
        for set in open_sets {
            self.place_open(work, grid, set, &mut extras, &mut composites);
        }
        (extras, composites)
    }

    fn place_open(&self, work: &Grid, grid: &Grid, set: &CellSet, extras: &mut Vec<Shape>, composites: &mut Vec<Vec<Shape>>) {
        if set.len() == 1 {
            if let Some(line) = self.small_line(work, grid, set) { extras.push(line); }
        } else {
            let members = self.composite(work, set);
            if !members.is_empty() { composites.push(members); }
        }
    }

    fn composite(&self, work: &Grid, set: &CellSet) -> Vec<Shape> {
        let mut members = self.open_shapes(work, set);
        for member in &mut members { self.connect_ends_to_anchors(work, member); }
        members
    }

    fn small_line(&self, work: &Grid, grid: &Grid, set: &CellSet) -> Option<Shape> {
        let cell = set.first();
        if grid.dashed_line_char(cell.0, cell.1) { return None; }
        let mut line = self.line_shape(work, cell)?;
        self.connect_ends_to_anchors(work, &mut line);
        Some(line)
    }

    fn line_shape(&self, grid: &Grid, cell: (i64, i64)) -> Option<Shape> {
        let mut shape = Shape::new_line();
        if grid.horizontal_line(cell.0, cell.1) {
            self.add_horizontal_points(&mut shape, cell.0, cell.1);
        } else if grid.vertical_line(cell.0, cell.1) {
            self.add_vertical_points(&mut shape, cell.0, cell.1);
        }
        if shape.points.is_empty() { None } else { Some(shape) }
    }

    fn add_horizontal_points(&self, shape: &mut Shape, col: i64, row: i64) {
        let mid_y = self.cell_top(row) + self.options.cell_height / 2.0;
        shape.points.push(self.cell_point(self.cell_left(col), mid_y));
        shape.points.push(self.cell_point(self.line_end_x(col), mid_y));
    }

    fn add_vertical_points(&self, shape: &mut Shape, col: i64, row: i64) {
        let mid_x = self.cell_left(col) + self.options.cell_width / 2.0;
        shape.points.push(self.cell_point(mid_x, self.cell_top(row)));
        shape.points.push(self.cell_point(mid_x, self.line_end_y(row)));
    }

    fn line_end_x(&self, col: i64) -> f64 { self.cell_left(col) + self.options.cell_width - 1.0 }
    fn line_end_y(&self, row: i64) -> f64 { self.cell_top(row) + self.options.cell_height - 1.0 }

    fn cell_point(&self, x: f64, y: f64) -> Point {
        Point { x, y, kind: PointType::Normal, locked: false }
    }

    // ---- the open trace ----

    fn open_shapes(&self, grid: &Grid, set: &CellSet) -> Vec<Shape> {
        let mut work = Grid::blank_like_grid(grid);
        grid.copy_cells_to(set, &mut work);
        let Some(start) = set.cells.iter().copied().find(|c| work.lines_end(c.0, c.1)) else {
            return vec![];
        };
        let Some(next) = work.follow_cell(start.0, start.1, None) else { return vec![]; };
        if next.len() == 0 { return vec![]; }
        let mut visited: HashSet<(i64, i64)> = HashSet::new();
        let mut drawn: Vec<Shape> = vec![];
        self.grow_edges(&mut Trace { work: &mut work, visited: &mut visited, drawn: &mut drawn }, next.cells[0], start);
        infect_dashes(drawn)
    }

    fn grow_edges(&self, trace: &mut Trace, cell: (i64, i64), previous_cell: (i64, i64)) {
        trace.visited.insert(previous_cell);
        let mut segment = self.open_segment(trace.work, previous_cell);
        let mut state = WalkState { cell, previous: previous_cell, steps: 0 };
        let outcome = self.segment_walk(trace, &mut segment, &mut state);
        if outcome.is_some() { trace.drawn.push(segment); }
    }

    fn open_segment(&self, work: &Grid, cell: (i64, i64)) -> Shape {
        let mut shape = Shape::new_line();
        shape.points.push(self.point_for_cell(work, cell.0, cell.1));
        if work.dashed_line_char(cell.0, cell.1) { shape.dashed = true; }
        shape
    }

    fn segment_walk(&self, trace: &mut Trace, segment: &mut Shape, state: &mut WalkState) -> Option<()> {
        state.steps += 1;
        if state.steps > 1000 { return None; }
        if trace.work.lines_end(state.cell.0, state.cell.1) {
            self.visit_open_cell(trace, segment, state.cell);
            return Some(());
        }
        self.visit_open_cell(trace, segment, state.cell);
        self.step_open(trace, segment, state)
    }

    fn step_open(&self, trace: &mut Trace, segment: &mut Shape, state: &mut WalkState) -> Option<()> {
        let next_cells = trace.work.follow_cell(state.cell.0, state.cell.1, Some(state.previous));
        match next_cells {
            Some(ref n) if n.len() == 1 => {
                state.previous = state.cell;
                state.cell = n.cells[0];
                self.segment_walk(trace, segment, state)
            }
            next_cells => {
                if let Some(n) = next_cells {
                    for next_cell in &n.cells {
                        self.grow_edges(trace, *next_cell, state.cell);
                    }
                }
                Some(())
            }
        }
    }

    fn visit_open_cell(&self, trace: &mut Trace, segment: &mut Shape, cell: (i64, i64)) {
        trace.visited.insert(cell);
        if self.point_cell(trace.work, cell) {
            segment.points.push(self.point_for_cell(trace.work, cell.0, cell.1));
        }
        if trace.work.dashed_line_char(cell.0, cell.1) { segment.dashed = true; }
    }

    fn point_cell(&self, grid: &Grid, cell: (i64, i64)) -> bool {
        grid.corner(cell.0, cell.1) || grid.intersection(cell.0, cell.1)
            || grid.stub(cell.0, cell.1) || grid.lines_end(cell.0, cell.1)
    }

    // ---- anchors ----

    fn connect_ends_to_anchors(&self, grid: &Grid, shape: &mut Shape) {
        if shape.points.len() < 2 { return; }
        let last = shape.points.len() - 1;
        let (next_x, next_y) = (shape.points[1].x, shape.points[1].y);
        let next = Point { x: next_x, y: next_y, kind: PointType::Normal, locked: false };
        self.anchor_end_to(grid, &next, &mut shape.points[0]);
        let (prev_x, prev_y) = (shape.points[last - 1].x, shape.points[last - 1].y);
        let prev = Point { x: prev_x, y: prev_y, kind: PointType::Normal, locked: false };
        self.anchor_end_to(grid, &prev, &mut shape.points[last]);
    }

    fn anchor_end_to(&self, grid: &Grid, next_point: &Point, lines_end: &mut Point) {
        let anchor_cell = self.possible_anchor_cell(next_point, lines_end);
        if !self.anchor(grid, anchor_cell) { return; }
        lines_end.x = px_mid(anchor_cell.0 as i64, self.options.cell_width);
        lines_end.y = px_mid(anchor_cell.1 as i64, self.options.cell_height);
        lines_end.locked = true;
    }

    fn anchor(&self, grid: &Grid, cell: (f64, f64)) -> bool {
        grid.arrowhead(cell.0 as i64, cell.1 as i64)
            || grid.corner(cell.0 as i64, cell.1 as i64)
            || grid.intersection(cell.0 as i64, cell.1 as i64)
    }

    fn possible_anchor_cell(&self, next_point: &Point, lines_end: &Point) -> (f64, f64) {
        let (dx, dy) = self.anchor_shift(next_point, lines_end);
        (
            (lines_end.x + dx) / self.options.cell_width,
            (lines_end.y + dy) / self.options.cell_height,
        )
    }

    fn anchor_shift(&self, next_point: &Point, lines_end: &Point) -> (f64, f64) {
        if next_point.y < lines_end.y {
            (0.0, self.options.cell_height)
        } else if next_point.y > lines_end.y {
            (0.0, -self.options.cell_height)
        } else if next_point.x < lines_end.x {
            (self.options.cell_width, 0.0)
        } else {
            (-self.options.cell_width, 0.0)
        }
    }

    // ---- points ----

    fn point_for_cell(&self, grid: &Grid, col: i64, row: i64) -> Point {
        let kind = self.point_type(grid, col, row)
            .unwrap_or_else(|| panic!("cannot make point for cell {},{}", col, row));
        Point {
            x: px_mid(col, self.options.cell_width),
            y: px_mid(row, self.options.cell_height),
            kind,
            locked: false,
        }
    }

    fn point_type(&self, grid: &Grid, col: i64, row: i64) -> Option<PointType> {
        if let Some(corner) = self.corner_type(grid, col, row) { return Some(corner); }
        if grid.lines_end(col, row) || grid.intersection(col, row) {
            return Some(PointType::Normal);
        }
        None
    }

    fn corner_type(&self, grid: &Grid, col: i64, row: i64) -> Option<PointType> {
        if grid.corner(col, row) && self.options.round_corners { return Some(PointType::Round); }
        if grid.normal_corner(col, row) { return Some(PointType::Normal); }
        if grid.round_corner(col, row) { return Some(PointType::Round); }
        None
    }

    // ---- separation ----

    fn separate_common_edges(&self, shapes: &mut Vec<Shape>) {
        let offset = self.options.cell_width.min(self.options.cell_height) / 5.0;
        // (shape index, start point index, end point index)
        let mut edges: Vec<(usize, usize, usize)> = vec![];
        for (si, shape) in shapes.iter().enumerate() {
            let n = shape.points.len();
            for i in 0..n {
                edges.push((si, i, (i + 1) % n));
            }
        }
        let pairs: Vec<(usize, usize)> = edges
            .iter()
            .enumerate()
            .flat_map(|(index, edge)| {
                edges[index + 1..]
                    .iter()
                    .enumerate()
                    .filter(|(_, other)| self.edges_touch(shapes, edge, other))
                    .map(move |(other_offset, other)| (index, index + 1 + other_offset))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (first, second) in pairs {
            let (f, s) = (edges[first], edges[second]);
            self.move_inwards(shapes, f, offset);
            self.move_inwards(shapes, s, offset);
        }
    }

    fn edges_touch(&self, shapes: &[Shape], a: &(usize, usize, usize), b: &(usize, usize, usize)) -> bool {
        if a == b { return true; }
        let (a1, a2) = edge_pair(shapes, *a);
        let (b1, b2) = edge_pair(shapes, *b);
        let horizontal_a = a1.y == a2.y;
        let vertical_a = a1.x == a2.x;
        let horizontal_b = b1.y == b2.y;
        let vertical_b = b1.x == b2.x;
        if (horizontal_a && vertical_b) || (horizontal_b && vertical_a) { return false; }
        let dist_a = if horizontal_a { a1.y } else { a1.x };
        let dist_b = if horizontal_b { b1.y } else { b1.x };
        if dist_a != dist_b { return false; }
        self.overlapping(shapes, *a, *b)
    }

    fn overlapping(&self, shapes: &[Shape], a: (usize, usize, usize), b: (usize, usize, usize)) -> bool {
        let (a1, a2) = edge_pair(shapes, a);
        let (b1, b2) = edge_pair(shapes, b);
        let a_horizontal = a1.y == a2.y;
        let (a_first, a_last) = ordered_points(a1, a2, a_horizontal);
        let (b_first, b_last) = ordered_points(b1, b2, a_horizontal);
        let a_starts_first = if a_horizontal { a_first.x <= b_first.x } else { a_first.y <= b_first.y };
        let ((f_start, f_end), (s_start, s_end)) = if a_starts_first {
            ((a_first, a_last), (b_first, b_last))
        } else {
            ((b_first, b_last), (a_first, a_last))
        };
        if f_end == s_start { return false; }
        self.within_any(shapes, a, s_start, s_end) || self.within_any(shapes, b, f_start, f_end)
    }

    fn within_any(&self, shapes: &[Shape], e: (usize, usize, usize), start: &Point, end: &Point) -> bool {
        let (s, t) = edge_pair(shapes, e);
        self.within_edge(s, start, end) || self.within_edge(t, start, end)
    }

    fn within_edge(&self, point: &Point, start: &Point, end: &Point) -> bool {
        if start.y == end.y {
            point.y == start.y && point.x >= start.x.min(end.x) && point.x <= start.x.max(end.x)
        } else {
            point.x == start.x && point.y >= start.y.min(end.y) && point.y <= start.y.max(end.y)
        }
    }

    fn move_inwards(&self, shapes: &mut [Shape], e: (usize, usize, usize), offset: f64) {
        let Some(shift) = self.inward_shift(shapes, e, offset) else { return; };
        let (si, pi, ti) = e;
        let n = shapes[si].points.len();
        for index in [pi % n, ti % n] {
            shapes[si].points[index].x += shift.0;
            shapes[si].points[index].y += shift.1;
        }
    }

    fn inward_shift(&self, shapes: &[Shape], e: (usize, usize, usize), offset: f64) -> Option<(f64, f64)> {
        let (start, end) = edge_pair(shapes, e);
        let horizontal = start.y == end.y;
        let vertical = start.x == end.x;
        let mid = ((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
        if horizontal {
            Some((0.0, self.probe_shift(shapes, e, offset, Some(mid.0), None)))
        } else if vertical {
            Some((self.probe_shift(shapes, e, offset, None, Some(mid.1)), 0.0))
        } else {
            None
        }
    }

    fn probe_shift(&self, shapes: &[Shape], e: (usize, usize, usize), offset: f64, probe_x: Option<f64>, probe_y: Option<f64>) -> f64 {
        if self.inside_probe(shapes, e, probe_x, probe_y, -0.05) { return -offset; }
        if self.inside_probe(shapes, e, probe_x, probe_y, 0.05) { return offset; }
        0.0
    }

    fn inside_probe(&self, shapes: &[Shape], e: (usize, usize, usize), probe_x: Option<f64>, probe_y: Option<f64>, delta: f64) -> bool {
        let (start, end) = edge_pair(shapes, e);
        let si = e.0;
        let point_x = probe_x.map(|x| x + delta).unwrap_or((start.x + end.x) / 2.0);
        let point_y = probe_y.map(|y| y + delta).unwrap_or((start.y + end.y) / 2.0);
        polygon_contains(&shapes[si], point_x, point_y)
    }

    // ---- decorate ----

    fn decorate(&self, grid: &Grid, work: &Grid, shapes: &mut Vec<Shape>) {
        self.assign_colors(grid, shapes);
        self.assign_markup(grid, shapes);
        self.build_arrowheads(work, shapes);
        self.build_point_markers(grid, shapes);
    }

    fn assign_colors(&self, grid: &Grid, shapes: &mut Vec<Shape>) {
        for (cell, color) in grid.color_codes() {
            let mid = (px_mid(cell.0, self.options.cell_width), px_mid(cell.1, self.options.cell_height));
            if let Some(i) = self.smallest_shape_containing(shapes, mid.0, mid.1) {
                shapes[i].fill = Some(color);
            }
        }
    }

    fn assign_markup(&self, grid: &Grid, shapes: &mut Vec<Shape>) {
        for (cell, tag) in grid.markup_tags() {
            let mid = (px_mid(cell.0, self.options.cell_width), px_mid(cell.1, self.options.cell_height));
            let Some(i) = self.smallest_shape_containing(shapes, mid.0, mid.1) else { continue; };
            shapes[i].kind = markup_type(&tag);
        }
    }

    fn smallest_shape_containing(&self, shapes: &[Shape], px: f64, py: f64) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (index, shape) in shapes.iter().enumerate() {
            if !polygon_contains(shape, px, py) { continue; }
            let better = best.is_none_or(|b| bounds_area(shape) < bounds_area(&shapes[b]));
            if better { best = Some(index); }
        }
        best
    }

    fn build_arrowheads(&self, grid: &Grid, shapes: &mut Vec<Shape>) {
        for cell in grid.arrowheads() {
            shapes.push(self.arrowhead_shape(grid, cell));
        }
    }

    fn arrowhead_shape(&self, grid: &Grid, cell: (i64, i64)) -> Shape {
        let points = self.arrowhead_points(grid, cell);
        Shape {
            points: points
                .iter()
                .map(|(x, y)| Point { x: *x, y: *y, kind: PointType::Normal, locked: false })
                .collect(),
            kind: ShapeKind::Arrowhead,
            closed: true,
            dashed: false,
            fill: Some("#000000".into()),
            stroke: Some("#000000".into()),
        }
    }

    fn arrowhead_points(&self, grid: &Grid, cell: (i64, i64)) -> Vec<(f64, f64)> {
        let left = cell.0 as f64 * self.options.cell_width;
        let top = cell.1 as f64 * self.options.cell_height;
        let mid_x = px_mid(cell.0, self.options.cell_width);
        let mid_y = px_mid(cell.1, self.options.cell_height);
        let right = left + self.options.cell_width;
        let bottom = top + self.options.cell_height;
        if grid.north_arrowhead(cell.0, cell.1) {
            vec![(mid_x, top), (left, bottom), (right, bottom)]
        } else if grid.south_arrowhead(cell.0, cell.1) {
            vec![(left, top), (mid_x, bottom), (right, top)]
        } else if grid.west_arrowhead(cell.0, cell.1) {
            vec![(right, top), (left, mid_y), (right, bottom)]
        } else {
            vec![(left, top), (right, mid_y), (left, bottom)]
        }
    }

    fn build_point_markers(&self, grid: &Grid, shapes: &mut Vec<Shape>) {
        for cell in grid.point_markers_on_line().cells {
            shapes.push(Shape {
                points: vec![Point {
                    x: px_mid(cell.0, self.options.cell_width),
                    y: px_mid(cell.1, self.options.cell_height),
                    kind: PointType::Normal,
                    locked: false,
                }],
                kind: ShapeKind::PointMarker,
                closed: false,
                dashed: false,
                fill: Some("#ffffff".into()),
                stroke: Some("#000000".into()),
            });
        }
    }

    // ---- finish ----

    fn finish(&self, grid: &Grid, shapes: Vec<Shape>, composites: Vec<Vec<Shape>>, options: &Options) -> Built {
        let shapes = dedupe_shapes(shapes);
        let texts = self.extract_texts(grid, &shapes);
        Built {
            width: grid.width() as f64 * self.options.cell_width,
            height: grid.height() as f64 * self.options.cell_height,
            shapes,
            composites,
            texts,
            transparent: options.transparent,
            shadows: options.shadows,
            cell_width: self.options.cell_width,
            cell_height: self.options.cell_height,
        }
    }

    fn extract_texts(&self, grid: &Grid, shapes: &[Shape]) -> Vec<TextRun> {
        let mut work = grid.clone();
        work.remove_non_text();
        let base = font_for(self.options.cell_height);
        let mut runs = self.text_runs(&work, base);
        self.color_text_runs(&mut runs, shapes);
        runs
    }

    fn text_runs(&self, work: &Grid, base_size: f64) -> Vec<TextRun> {
        let mut runs = vec![];
        for isolation in self.text_groups(work) {
            for (cell, string) in isolation.strings() {
                runs.push(self.text_run(&isolation, cell, &string, base_size));
            }
        }
        runs
    }

    fn text_groups(&self, work: &Grid) -> Vec<Grid> {
        let mut grouped = work.clone();
        let blanks = grouped.blanks_between_characters();
        grouped.fill_cells_with(&blanks, '|');
        grouped
            .all_non_blank()
            .break_into_distinct()
            .iter()
            .map(|component| {
                let mut isolation = Grid::blank_like_grid(work);
                work.copy_cells_to(component, &mut isolation);
                isolation
            })
            .collect()
    }

    fn text_run(&self, isolation: &Grid, cell: (i64, i64), string: &str, base_size: f64) -> TextRun {
        let span = self.run_span(cell, string);
        let (anchor, anchor_x) = self.anchor_for(isolation, cell, string, &span);
        TextRun {
            x: anchor_x,
            y: self.baseline(cell.1, base_size),
            string: string.to_string(),
            font_size: base_size,
            anchor,
            fill: "#000000".into(),
        }
    }

    fn run_span(&self, cell: (i64, i64), string: &str) -> (f64, f64) {
        let col = cell.0;
        let last_col = col + string.chars().count() as i64 - 1;
        (
            col as f64 * self.options.cell_width,
            (last_col + 1) as f64 * self.options.cell_width,
        )
    }

    fn anchor_for(&self, isolation: &Grid, cell: (i64, i64), string: &str, span: &(f64, f64)) -> (&'static str, f64) {
        let last_col = cell.0 + string.chars().count() as i64 - 1;
        let starts = self.other_strings_starting_at(isolation, cell.0, cell.1);
        let ends = self.other_strings_ending_at(isolation, last_col, cell.1);
        let anchor = text_anchor(starts, ends);
        let position = if anchor == "end" { span.1 } else { (span.0 + span.1) / 2.0 };
        (anchor, position)
    }

    fn other_strings_starting_at(&self, isolation: &Grid, col: i64, own_row: i64) -> usize {
        (0..isolation.height() as i64)
            .filter(|row| {
                *row != own_row && col != 0
                    && !isolation.blank_cell(col, *row)
                    && isolation.blank_cell(col - 1, *row)
            })
            .count()
    }

    fn other_strings_ending_at(&self, isolation: &Grid, last_col: i64, own_row: i64) -> usize {
        (0..isolation.height() as i64)
            .filter(|row| {
                *row != own_row
                    && !isolation.blank_cell(last_col, *row)
                    && isolation.blank_cell(last_col + 1, *row)
            })
            .count()
    }

    fn baseline(&self, row: i64, size: f64) -> f64 {
        let top = row as f64 * self.options.cell_height;
        let bottom = top + self.options.cell_height;
        let center = (bottom - top) / 2.0;
        let z_height = 0.72 * size;
        bottom - (center - (z_height / 2.0)).abs()
    }

    fn color_text_runs(&self, runs: &mut [TextRun], shapes: &[Shape]) {
        for run in runs {
            let container = self.smallest_shape_intersecting(shapes, run);
            let Some(i) = container else { continue; };
            let Some(fill) = &shapes[i].fill else { continue; };
            if dark_color(fill) { run.fill = "#ffffff".into(); }
        }
    }

    fn smallest_shape_intersecting(&self, shapes: &[Shape], run: &TextRun) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (index, shape) in shapes.iter().enumerate() {
            if !self.intersects_rect(shape, run) { continue; }
            let better = best.is_none_or(|b| bounds_area(shape) < bounds_area(&shapes[b]));
            if better { best = Some(index); }
        }
        best
    }

    fn intersects_rect(&self, shape: &Shape, run: &TextRun) -> bool {
        let (min_x, min_y, max_x, max_y) = shape.bounds();
        let right = run.x + run.string.chars().count() as f64 * run.font_size * WIDTH_RATIO;
        if run.x > max_x || right < min_x { return false; }
        if run.y - run.font_size > max_y || run.y + 2.0 < min_y { return false; }
        true
    }
}

struct Trace<'a> {
    work: &'a mut Grid,
    visited: &'a mut HashSet<(i64, i64)>,
    drawn: &'a mut Vec<Shape>,
}

struct WalkState {
    cell: (i64, i64),
    previous: (i64, i64),
    steps: usize,
}

fn edge_pair(shapes: &[Shape], e: (usize, usize, usize)) -> (&Point, &Point) {
    (&shapes[e.0].points[e.1], &shapes[e.0].points[e.2])
}

fn ordered_points<'p>(a: &'p Point, b: &'p Point, horizontal: bool) -> (&'p Point, &'p Point) {
    if horizontal {
        if a.x <= b.x { (a, b) } else { (b, a) }
    } else if a.y <= b.y {
        (a, b)
    } else {
        (b, a)
    }
}

fn px_mid(cell: i64, size: f64) -> f64 { cell as f64 * size + size / 2.0 }

fn markup_type(tag: &str) -> ShapeKind {
    match tag {
        "d" => ShapeKind::Document,
        "s" => ShapeKind::Storage,
        "io" => ShapeKind::Io,
        "c" => ShapeKind::Decision,
        "mo" => ShapeKind::ManualOperation,
        "tr" => ShapeKind::Trapezoid,
        "o" => ShapeKind::Ellipse,
        _ => ShapeKind::Rectangle,
    }
}

fn polygon_contains(shape: &Shape, probe_x: f64, probe_y: f64) -> bool {
    let points = &shape.points;
    if points.len() < 3 { return false; }
    let mut inside = false;
    let mut previous = points.len() - 1;
    for (index, vertex) in points.iter().enumerate() {
        let crossed = crosses_ray(vertex, &points[previous], probe_x, probe_y);
        if crossed { inside = !inside; }
        previous = index;
    }
    inside
}

fn crosses_ray(vertex: &Point, other: &Point, probe_x: f64, probe_y: f64) -> bool {
    if !((vertex.y > probe_y) != (other.y > probe_y)) { return false; }
    probe_x < (((other.x - vertex.x) * (probe_y - vertex.y)) / (other.y - vertex.y)) + vertex.x
}

fn bounds_area(shape: &Shape) -> f64 {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    (max_x - min_x) * (max_y - min_y)
}

fn cells_equal(a: &CellSet, b: &CellSet) -> bool {
    let mut sa = a.cells.clone();
    let mut sb = b.cells.clone();
    sa.sort();
    sb.sort();
    sa == sb
}

fn dedupe_shapes(shapes: Vec<Shape>) -> Vec<Shape> {
    let mut kept: Vec<Shape> = vec![];
    for shape in shapes {
        let dup = kept.iter().any(|other| same_points(&shape, other));
        if !dup { kept.push(shape); }
    }
    kept
}

fn same_points(shape: &Shape, other: &Shape) -> bool {
    if shape.points.len() != other.points.len() { return false; }
    let mut a: Vec<(i64, i64)> = shape.points.iter().map(|p| (p.x as i64, p.y as i64)).collect();
    let mut b: Vec<(i64, i64)> = other.points.iter().map(|p| (p.x as i64, p.y as i64)).collect();
    a.sort();
    b.sort();
    a == b
}

fn infect_dashes(mut members: Vec<Shape>) -> Vec<Shape> {
    if members.iter().any(|m| m.dashed) {
        for m in &mut members { m.dashed = true; }
    }
    members
}

fn text_anchor(starts: usize, ends: usize) -> &'static str {
    if (starts == 0 && ends == 0) || (ends == starts && ends > 0) {
        return "middle";
    }
    if ends > 0 && (starts == 0 || ends > starts) {
        return "end";
    }
    "start"
}

fn font_for(pixel_height: f64) -> f64 {
    let size = 12.0;
    let direction = if size * ASCENT_RATIO > pixel_height { -1.0 } else { 1.0 };
    size_search(size + direction, direction, pixel_height)
}

fn size_search(mut size: f64, direction: f64, pixel_height: f64) -> f64 {
    while size > 0.0 {
        let ascent = size * ASCENT_RATIO;
        if direction == 1.0 {
            if ascent > pixel_height { return size - 0.5; }
        } else if ascent < pixel_height {
            return size;
        }
        size += direction * 0.5;
    }
    size
}

fn dark_color(hex: &str) -> bool {
    let body = hex.strip_prefix('#').unwrap_or(hex);
    (0..body.len()).step_by(2)
        .filter_map(|i| u8::from_str_radix(&body[i..(i + 2).min(body.len())], 16).ok())
        .max()
        .map(|m| m < 200)
        .unwrap_or(false)
}

fn hex_color(text: &str) -> String {
    let digits: Vec<i64> = text[1..].chars().map(|c| c.to_digit(16).unwrap_or(0) as i64).collect();
    let channel = |d: i64| d * 17;
    format!("#{:02x}{:02x}{:02x}", channel(digits[0]), channel(digits[1]), channel(digits[2]))
}

fn classify_boundaries(sets: &[CellSet], work: &Grid) -> (Vec<CellSet>, Vec<CellSet>) {
    let mut closed = vec![];
    let mut open = vec![];
    for set in sets {
        match set.cell_type(work) {
            CellType::Closed => closed.push(set.clone()),
            CellType::Open => open.push(set.clone()),
            _ => {}
        }
    }
    (closed, open)
}

// ---- the pipeline ----

/// Ruby `Ditaa.parse`: dedent, grid, build.
fn ditaa_parse(body_lines: &[String], options: &Options) -> Built {
    let dedented = dedent(body_lines);
    let grid = Grid::from_body_lines(&dedented);
    Builder::new(options).build(&grid, options)
}

fn dedent(body_lines: &[String]) -> Vec<String> {
    let pad = body_lines
        .iter()
        .map(|l| l.chars().take_while(|c| *c == ' ').count())
        .min()
        .unwrap_or(0);
    if pad == 0 {
        return body_lines.to_vec();
    }
    body_lines
        .iter()
        .map(|l| l.chars().skip(pad).collect())
        .collect()
}

/// Ruby `DitaaRenderer`: shadows behind closed shapes, storage back to
/// front, the rest largest area first, then markers and texts.
fn paint(built: &Built, options: &Options) -> String {
    let min_cell = built.cell_width.min(built.cell_height);
    let stroke_weight = min_cell / 10.0;
    let dash_interval = min_cell / 2.0;
    let ink = "#000000";
    let mut doc = Document::new(built.width, built.height);

    if !options.transparent {
        doc.add(
            Elem::new("rect")
                .num("x", 0.0)
                .num("y", 0.0)
                .num("width", built.width)
                .num("height", built.height)
                .attr("fill", "#ffffff")
                .attr("stroke", "none"),
        );
    }

    let mut all: Vec<&Shape> = built.shapes.iter().collect();
    for composite in &built.composites {
        for shape in composite { all.push(shape); }
    }

    if options.shadows {
        let offset = min_cell / 3.333;
        for shape in &all {
            if !(shape.closed && shape.drops_shadow() && !shape.points.is_empty()) { continue; }
            let d = render_path(shape);
            doc.add(
                Elem::new("path")
                    .attr("d", d)
                    .attr("fill", "#969696")
                    .attr("stroke", "none")
                    .attr("transform", &format!("translate({},{})", ruby_float(offset), ruby_float(offset))),
            );
        }
    }

    // storage back to front: sort by descending centre y, stable on order
    let mut storage: Vec<(usize, &Shape)> = all
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == ShapeKind::Storage)
        .map(|(i, s)| (i, *s))
        .collect();
    storage.sort_by(|a, b| {
        let ay = centre_y(a.1);
        let by = centre_y(b.1);
        by.partial_cmp(&ay).unwrap().then(a.0.cmp(&b.0))
    });
    for (_, shape) in &storage { doc.add(paint_shape(shape, stroke_weight, dash_interval, ink)); }

    // the rest by descending area, stable on order
    let markers: Vec<(usize, &Shape)> = all
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == ShapeKind::PointMarker)
        .map(|(i, s)| (i, *s))
        .collect();
    let mut main: Vec<(usize, &Shape)> = all
        .iter()
        .enumerate()
        .filter(|(_, s)| !matches!(s.kind, ShapeKind::PointMarker | ShapeKind::Storage))
        .map(|(i, s)| (i, *s))
        .collect();
    main.sort_by(|a, b| {
        let aa = a.1.area();
        let ab = b.1.area();
        ab.partial_cmp(&aa).unwrap().then(a.0.cmp(&b.0))
    });
    for (_, shape) in &main { doc.add(paint_shape(shape, stroke_weight, dash_interval, ink)); }

    for (_, shape) in &markers {
        let point = &shape.points[0];
        let radius = 0.7 * min_cell / 2.0;
        doc.add(
            Elem::new("circle")
                .num("cx", point.x)
                .num("cy", point.y)
                .num("r", radius)
                .attr("fill", "#ffffff")
                .attr("stroke", ink)
                .attr("stroke-width", &write_num(stroke_weight)),
        );
    }

    for run in &built.texts {
        let mut t = Elem::new("text");
        t = t.num("x", run.x).num("y", run.y).content(&run.string);
        t = t.attr("text-anchor", run.anchor);
        t = t.attr("fill", &run.fill);
        t = t.attr("font-size", &write_num(run.font_size));
        t = t.attr("font-family", "sans-serif");
        t = t.attr("font-weight", "bold");
        doc.add(t);
    }

    doc.to_xml()
}

fn centre_y(shape: &Shape) -> f64 {
    let (_, min_y, _, max_y) = shape.bounds();
    (min_y + max_y) / 2.0
}

fn paint_shape(shape: &Shape, stroke_weight: f64, dash_interval: f64, ink: &str) -> Elem {
    let d = render_path(shape);
    let mut path = Elem::new("path").attr("d", d);
    path = if shape.closed && !shape.dashed {
        path.attr("fill", shape.fill.as_deref().unwrap_or("#ffffff"))
    } else {
        path.attr("fill", "none")
    };
    path = path.attr("stroke", shape.stroke.as_deref().unwrap_or(ink));
    path = path.attr("stroke-width", &write_num(stroke_weight));
    if shape.dashed {
        path = path.attr("stroke-dasharray", &write_num(dash_interval));
    } else {
        path = path.attr("stroke-linecap", "round");
        path = path.attr("stroke-linejoin", "round");
    }
    path
}

/// Ruby `render_path`: the special four-point shapes, else the traced
/// polygon with rounded corners as quadratic curves.
fn render_path(shape: &Shape) -> String {
    if shape.points.len() == 4 {
        let flow = match shape.kind {
            ShapeKind::Document => Some(document_path(shape)),
            ShapeKind::Decision => Some(decision_path(shape)),
            ShapeKind::ManualOperation => Some(trapezoid_path(shape, true)),
            _ => None,
        };
        if let Some(flow) = flow { return flow; }
        let flowless = match shape.kind {
            ShapeKind::Storage => Some(storage_path(shape)),
            ShapeKind::Io => Some(io_path(shape)),
            ShapeKind::Trapezoid => Some(trapezoid_path(shape, false)),
            ShapeKind::Ellipse => Some(ellipse_path(shape)),
            _ => None,
        };
        if let Some(flowless) = flowless { return flowless; }
    }
    polygon_path(shape)
}

fn polygon_path(shape: &Shape) -> String {
    let points = &shape.points;
    let mut commands = opening_commands(points);
    for index in 1..points.len() {
        commands.push_str(&point_commands(points, index));
    }
    if shape.closed && points.len() > 2 { commands.push('Z'); }
    commands.trim().to_string()
}

fn opening_commands(points: &[Point]) -> String {
    let first = &points[0];
    if first.kind == PointType::Round {
        let entry = cell_edge_point(first, points.last().unwrap());
        let exit = cell_edge_point(first, &points[1]);
        format!("M {} {} Q {} {} {} {} ", entry.0, entry.1, first.x, first.y, exit.0, exit.1)
    } else {
        format!("M {} {} ", first.x, first.y)
    }
}

fn point_commands(points: &[Point], index: usize) -> String {
    let point = &points[index];
    if point.kind != PointType::Round {
        return format!("L {} {} ", point.x, point.y);
    }
    let following = if index < points.len() - 1 { &points[index + 1] } else { &points[0] };
    let entry = cell_edge_point(point, &points[index - 1]);
    let exit = cell_edge_point(point, following);
    let mut out = format!("L {} {} Q {} {} {} {} ", entry.0, entry.1, point.x, point.y, exit.0, exit.1);
    if following.kind == PointType::Normal {
        out.push_str(&format!("L {} {} ", following.x, following.y));
    } else {
        let next_entry = cell_edge_point(following, point);
        out.push_str(&format!("L {} {} ", next_entry.0, next_entry.1));
    }
    out
}

fn cell_edge_point(point: &Point, other: &Point) -> (f64, f64) {
    let left = (point.x / CELL_WIDTH).floor() * CELL_WIDTH;
    let top = (point.y / CELL_HEIGHT).floor() * CELL_HEIGHT;
    let right = left + CELL_WIDTH;
    let bottom = top + CELL_HEIGHT;
    if other.y < point.y {
        (point.x, top)
    } else if other.y > point.y {
        (point.x, bottom)
    } else if other.x < point.x {
        (left, point.y)
    } else {
        (right, point.y)
    }
}

fn document_path(shape: &Shape) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let mid_x = (min_x + max_x) / 2.0;
    let control_dx = (max_x - min_x) / 6.0;
    let control_dy = (max_y - min_y) / 8.0;
    format!(
        "M {} {} L {} {} L {} {} Q {} {} {} {} Q {} {} {} {} Z",
        ruby_float(min_x), ruby_float(min_y),
        ruby_float(max_x), ruby_float(min_y),
        ruby_float(max_x), ruby_float(max_y),
        ruby_float(mid_x + control_dx), ruby_float(max_y - control_dy),
        ruby_float(mid_x), ruby_float(max_y),
        ruby_float(mid_x - control_dx), ruby_float(max_y + control_dy),
        ruby_float(min_x), ruby_float(max_y),
    )
}

fn storage_path(shape: &Shape) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let cp_x = (max_x - min_x) / 6.0;
    let cp_top = CELL_HEIGHT / 2.0;
    let first = format!("C {} {} {} {} {} {}", ruby_float(min_x + cp_x), ruby_float(min_y + cp_top), ruby_float(max_x - cp_x), ruby_float(min_y + cp_top), ruby_float(max_x), ruby_float(min_y));
    let second = format!("C {} {} {} {} {} {}", ruby_float(max_x - cp_x), ruby_float(min_y - cp_top), ruby_float(min_x + cp_x), ruby_float(min_y - cp_top), ruby_float(min_x), ruby_float(min_y));
    format!("M {} {} {} {} ", ruby_float(min_x), ruby_float(min_y), first, second)
}

fn io_path(shape: &Shape) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let offset = CELL_WIDTH * 0.5;
    format!(
        "M {} {} L {} {} L {} {} L {} {} Z",
        ruby_float(min_x + offset), ruby_float(min_y),
        ruby_float(max_x + offset), ruby_float(min_y),
        ruby_float(max_x - offset), ruby_float(max_y),
        ruby_float(min_x - offset), ruby_float(max_y),
    )
}

fn decision_path(shape: &Shape) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let mid_x = (min_x + max_x) / 2.0;
    let mid_y = (min_y + max_y) / 2.0;
    format!(
        "M {} {} L {} {} L {} {} L {} {} Z",
        ruby_float(min_x), ruby_float(mid_y),
        ruby_float(mid_x), ruby_float(min_y),
        ruby_float(max_x), ruby_float(mid_y),
        ruby_float(mid_x), ruby_float(max_y),
    )
}

fn trapezoid_path(shape: &Shape, inverted: bool) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let mut offset = CELL_WIDTH * 0.5;
    if inverted { offset = -offset; }
    format!(
        "M {} {} L {} {} L {} {} L {} {} Z",
        ruby_float(min_x + offset), ruby_float(min_y),
        ruby_float(max_x - offset), ruby_float(min_y),
        ruby_float(max_x + offset), ruby_float(max_y),
        ruby_float(min_x - offset), ruby_float(max_y),
    )
}

fn ellipse_path(shape: &Shape) -> String {
    let (min_x, min_y, max_x, max_y) = shape.bounds();
    let mid_x = (min_x + max_x) / 2.0;
    let mid_y = (min_y + max_y) / 2.0;
    let x_off = ((max_x - min_x) * 0.5 * KAPPA).round_to(6);
    let y_off = ((max_y - min_y) * 0.5 * KAPPA).round_to(6);
    let arcs = [
        (mid_x + x_off, min_y, max_x, mid_y - y_off, max_x, mid_y),
        (max_x, mid_y + y_off, mid_x + x_off, max_y, mid_x, max_y),
        (mid_x - x_off, max_y, min_x, mid_y + y_off, min_x, mid_y),
        (min_x, mid_y - y_off, mid_x - x_off, min_y, mid_x, min_y),
    ];
    let mut out = format!("M {} {} ", ruby_float(mid_x), ruby_float(min_y));
    for (c1x, c1y, c2x, c2y, x, y) in arcs {
        out.push_str(&format!("C {} {} {} {} {} {} ", ruby_float(c1x), ruby_float(c1y), ruby_float(c2x), ruby_float(c2y), ruby_float(x), ruby_float(y)));
    }
    out.push('Z');
    out
}

const KAPPA: f64 = 4.0 * (std::f64::consts::SQRT_2 - 1.0) / 3.0;

trait RoundTo {
    fn round_to(self, places: i32) -> f64;
}

impl RoundTo for f64 {
    fn round_to(self, places: i32) -> f64 {
        let factor = 10f64.powi(places);
        (self * factor).round() / factor
    }
}

// ---- entry ----

/// The body lines and the start line, off the artifact tree.
fn body_and_start(tree: &Value) -> (String, Vec<String>) {
    let mut start_line = String::new();
    let mut lines: Vec<String> = vec![];
    if let Value::Array(items) = tree {
        for item in items {
            let Some(object) = item.as_object() else { continue; };
            if let Some(v) = object.get("ditaa_start_line") {
                start_line = text_of(v);
            } else if let Some(v) = object.get("line") {
                lines.push(text_of(v));
            }
        }
    }
    (start_line, lines)
}

pub fn render(source: &str) -> Result<String, Error> {
    let tree = crate::parse::parse(source, "ditaa")?;
    let (start_line, lines) = body_and_start(&tree);
    let options = options_from(&start_line);
    let built = ditaa_parse(&lines, &options);
    Ok(paint(&built, &options))
}
