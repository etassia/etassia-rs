//! The edge-drawing helpers: ports of Ruby `EndTexts`, `QualifierBox`,
//! `LabelClearance`, `StyledRelationCurve`, `RelationPathPoints`,
//! `SegmentBox` and `RelationLabelSeparator`.
use super::model::{Direction, Relation};
use super::rank_chain::Engine;
use super::scene::{Box, Marker, Relation as SceneRelation, Segment, Text};

// ---- EndTexts ----

pub mod end_texts {
    use super::*;

    /// Ruby `EndTexts`: the multiplicity and `/"role"` texts at a
    /// relation's two ends, in the order PlantUML draws them.
    pub struct EndTexts {
        /// The written sides (`true` = :left) of the link's first and
        /// second class.
        pub sides: [bool; 2],
    }

    #[derive(Clone, Debug)]
    pub struct Entry {
        pub side: bool,
        pub kind: EntryKind,
        pub pieces: Vec<String>,
        pub multiline: bool,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum EntryKind {
        Multiplicity,
        Role,
        Qualifier,
    }

    impl EndTexts {
        pub fn new(relation: &Relation, engine: Engine) -> EndTexts {
            let sides = match relation.direction {
                Some(Direction::Up) | Some(Direction::Left) => [false, true],
                _ => [true, false],
            };
            let _ = engine;
            EndTexts { sides }
        }

        pub fn entries(&self, relation: &Relation, engine: Engine) -> Vec<Entry> {
            // The order the engine draws the four texts in: smetana draws
            // the second end's multiplicity, the first's, the first's
            // role, then the second's; dot draws the first end's
            // multiplicity and role, then the second's.
            let order: [(bool, EntryKind); 4] = match engine {
                Engine::Smetana => [
                    (self.sides[1], EntryKind::Multiplicity),
                    (self.sides[0], EntryKind::Multiplicity),
                    (self.sides[0], EntryKind::Role),
                    (self.sides[1], EntryKind::Role),
                ],
                Engine::Dot => [
                    (self.sides[0], EntryKind::Multiplicity),
                    (self.sides[0], EntryKind::Role),
                    (self.sides[1], EntryKind::Multiplicity),
                    (self.sides[1], EntryKind::Role),
                ],
            };
            let mut entries: Vec<Entry> = order
                .into_iter()
                .filter_map(|(side, kind)| {
                    let content = if side {
                        if kind == EntryKind::Multiplicity {
                            relation.left_multiplicity.clone()
                        } else {
                            relation.left_role.clone()
                        }
                    } else if kind == EntryKind::Multiplicity {
                        relation.right_multiplicity.clone()
                    } else {
                        relation.right_role.clone()
                    }?;
                    Some(self.entry(side, kind, &content))
                })
                .collect();
            let qualifiers: Vec<Entry> = entries
                .iter()
                .filter(|entry| entry.kind == EntryKind::Qualifier)
                .cloned()
                .collect();
            entries.retain(|entry| entry.kind != EntryKind::Qualifier);
            entries.extend(qualifiers);
            entries
        }

        fn entry(&self, side: bool, kind: EntryKind, content: &str) -> Entry {
            let qualifier = content.starts_with('[') && content.ends_with(']');
            let stripped = if qualifier {
                &content[1..content.len() - 1]
            } else {
                content
            };
            let lines: Vec<&str> = stripped.split("\\n").collect();
            Entry {
                side,
                kind: if qualifier { EntryKind::Qualifier } else { kind },
                pieces: pieces(&lines),
                multiline: lines.len() > 1,
            }
        }
    }

    /// Ruby `pieces`: a numbered item's `1.` and its text.
    fn pieces(lines: &[&str]) -> Vec<String> {
        if lines.len() > 1 {
            return lines.iter().map(|l| l.to_string()).collect();
        }
        let content = lines[0];
        let hashes = content.chars().take_while(|c| *c == '#').count();
        if hashes > 0 {
            let spaces = content[hashes..]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            let rest = &content[hashes + spaces..];
            if !rest.is_empty() {
                return vec!["1.".to_string(), rest.to_string()];
            }
        }
        vec![content.to_string()]
    }
}

// ---- QualifierBox ----

pub mod qualifier_box {
    use super::*;

    const HEIGHT_RATIO: f64 = 18.4883 / 14.0;
    const BASELINE_RATIO: f64 = 14.5352 / 14.0;
    const INSET: f64 = 2.0;
    const OVERLAP: f64 = 0.5;

    pub struct Placed {
        pub bx: f64,
        pub by: f64,
        pub bw: f64,
        pub bh: f64,
        pub text: Text,
        pub clearance: f64,
        pub outward: [f64; 2],
    }

    impl Placed {
        /// Ruby `clear`: the endpoint moved clear of the box.
        pub fn clear(&self, endpoint: [f64; 2]) -> [f64; 2] {
            [
                endpoint[0] + self.outward[0] * self.clearance,
                endpoint[1] + self.outward[1] * self.clearance,
            ]
        }

        pub fn box_of(&self, id: &str) -> Box {
            Box {
                id: id.to_string(),
                x: self.bx,
                y: self.by,
                width: self.bw,
                height: self.bh,
                texts: vec![self.text.clone()],
                separators: vec![],
                kind: "qualifier",
                fill: None,
                spot: None,
            }
        }
    }

    /// Ruby `place`.
    pub fn place(
        content: &str,
        endpoint: [f64; 2],
        outward: [f64; 2],
        text_width: f64,
        size: f64,
    ) -> Placed {
        let axis = if outward[1].abs() >= outward[0].abs() { 1 } else { 0 };
        let (width, height) = (text_width + 2.0 * INSET, size * HEIGHT_RATIO);
        let sign = outward[axis];
        let (left, top, reach) = if axis == 1 {
            let top = if sign > 0.0 {
                endpoint[1] - OVERLAP
            } else {
                endpoint[1] - height + OVERLAP
            };
            (endpoint[0] - width / 2.0, top, height - OVERLAP)
        } else {
            let left = if sign > 0.0 {
                endpoint[0] - OVERLAP
            } else {
                endpoint[0] - width + OVERLAP
            };
            (left, endpoint[1] - height / 2.0, width - OVERLAP)
        };
        let clearance = reach / outward[axis].abs();
        Placed {
            bx: left,
            by: top,
            bw: width,
            bh: height,
            text: Text::new(
                content,
                left + INSET,
                top + size * BASELINE_RATIO,
                "qualifier",
                "start",
            ),
            clearance,
            outward,
        }
    }
}

// ---- LabelClearance ----

pub mod label_clearance {
    /// Ruby `top_between`: the y of the line's highest point between the x
    /// values.
    pub fn top_between(from: [f64; 2], to: [f64; 2], left: f64, right: f64) -> f64 {
        let lowest = from[1].min(to[1]);
        [left, right]
            .iter()
            .map(|x| across(from, to, 0, *x, lowest))
            .fold(f64::INFINITY, f64::min)
    }

    /// Ruby `right_between`: the x of the line's rightmost point between
    /// the y values.
    pub fn right_between(from: [f64; 2], to: [f64; 2], top: f64, bottom: f64) -> f64 {
        let widest = from[0].max(to[0]);
        [top, bottom]
            .iter()
            .map(|y| across(from, to, 1, *y, widest))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn across(from: [f64; 2], to: [f64; 2], axis: usize, value: f64, flat: f64) -> f64 {
        let Some(share) = share_of(from[axis], to[axis], value) else {
            return flat;
        };
        let other = 1 - axis;
        from[other] + (to[other] - from[other]) * share
    }

    fn share_of(start: f64, stop: f64, value: f64) -> Option<f64> {
        let span = stop - start;
        if span == 0.0 {
            return None;
        }
        Some(((value - start) / span).clamp(0.0, 1.0))
    }
}

// ---- StyledRelationCurve ----

pub mod styled_relation_curve {
    use super::*;

    const SHALLOW_SLOPE: f64 = 0.5;
    const REACH: f64 = 0.4;
    const DEPARTURE: f64 = 0.3;
    const ARRIVAL: f64 = 0.6;
    const LABEL_AT: f64 = 0.65;

    fn label_weights() -> [f64; 4] {
        let one = 1.0 - LABEL_AT;
        [
            one * one * one,
            3.0 * one * one * LABEL_AT,
            3.0 * one * (LABEL_AT * LABEL_AT),
            LABEL_AT * LABEL_AT * LABEL_AT,
        ]
    }

    fn styled(relation: &Relation) -> bool {
        relation.color.is_some() || relation.width.is_some() || relation.dotted
    }

    /// Ruby `controls`: the two cubic control points of a styled, shallow
    /// link.
    pub fn controls(relation: &Relation, from: [f64; 2], to: [f64; 2]) -> Option<[[f64; 2]; 2]> {
        if !styled(relation) {
            return None;
        }
        let run = to[0] - from[0];
        let rise = to[1] - from[1];
        if !(run.abs() * SHALLOW_SLOPE > rise.abs() && rise != 0.0) {
            return None;
        }
        Some([
            [from[0] + run * REACH, from[1] + rise * DEPARTURE],
            [to[0] - run * REACH, to[1] - rise * ARRIVAL],
        ])
    }

    /// Ruby `label_point`: the curve's point at `LABEL_AT`.
    pub fn label_point(relation: &Relation, from: [f64; 2], to: [f64; 2]) -> Option<[f64; 2]> {
        let points = controls(relation, from, to)?;
        let weights = label_weights();
        let chain = [from, points[0], points[1], to];
        Some([
            crate::render::pie::fsum(
                chain.iter().zip(weights.iter()).map(|(p, w)| p[0] * w),
            ),
            crate::render::pie::fsum(
                chain.iter().zip(weights.iter()).map(|(p, w)| p[1] * w),
            ),
        ])
    }
}

// ---- RelationPathPoints ----

pub mod relation_path_points {
    const STEPS: usize = 16;

    /// Ruby `call`: the points a relation's SVG path passes through.
    pub fn call(path: Option<&str>) -> Vec<[f64; 2]> {
        let Some(path) = path else { return vec![] };
        let mut out: Vec<[f64; 2]> = vec![];
        let chars: Vec<char> = path.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            let command = chars[index];
            if !matches!(command, 'M' | 'L' | 'C' | 'Q') {
                index += 1;
                continue;
            }
            index += 1;
            let mut numbers: Vec<f64> = vec![];
            let mut run = String::new();
            while index < chars.len() && !matches!(chars[index], 'M' | 'L' | 'C' | 'Q') {
                let c = chars[index];
                if c.is_ascii_digit() || c == '-' || c == '.' {
                    run.push(c);
                } else if !run.is_empty() {
                    if let Ok(v) = run.parse::<f64>() {
                        numbers.push(v);
                    }
                    run.clear();
                }
                index += 1;
            }
            if !run.is_empty() {
                if let Ok(v) = run.parse::<f64>() {
                    numbers.push(v);
                }
            }
            let order = match command {
                'M' | 'L' => 1,
                'C' => 3,
                _ => 2,
            };
            let pairs: Vec<[f64; 2]> = numbers.chunks(2).filter(|p| p.len() == 2).map(|p| [p[0], p[1]]).collect();
            for controls in pairs.chunks(order) {
                out.extend(curve(out.last().copied(), controls));
            }
        }
        out
    }

    /// The points from `start` through `controls`, a Bezier when `start`
    /// is known and more than the end is given.
    fn curve(start: Option<[f64; 2]>, controls: &[[f64; 2]]) -> Vec<[f64; 2]> {
        if start.is_none() || controls.len() <= 1 {
            return controls.to_vec();
        }
        let start = start.unwrap();
        let mut points: Vec<[f64; 2]> = Vec::with_capacity(controls.len() + 1);
        points.push(start);
        points.extend(controls.iter().copied());
        (1..=STEPS)
            .map(|step| bezier(&points, step as f64 / STEPS as f64))
            .collect()
    }

    fn bezier(points: &[[f64; 2]], share: f64) -> [f64; 2] {
        let mut points = points.to_vec();
        while points.len() > 1 {
            points = reduce(&points, share);
        }
        points[0]
    }

    fn reduce(points: &[[f64; 2]], share: f64) -> Vec<[f64; 2]> {
        let mut out = vec![];
        for pair in points.windows(2) {
            out.push([
                pair[0][0] + (pair[1][0] - pair[0][0]) * share,
                pair[0][1] + (pair[1][1] - pair[0][1]) * share,
            ]);
        }
        out
    }

    /// Ruby `along`: the point that far along the polyline, by length.
    pub fn along(points: &[[f64; 2]], share: f64) -> Option<[f64; 2]> {
        if points.len() < 2 {
            return points.first().copied();
        }
        let legs: Vec<([f64; 2], [f64; 2], f64)> = points
            .windows(2)
            .map(|pair| {
                (
                    pair[0],
                    pair[1],
                    crate::js_math::hypot2(
                        pair[1][0] - pair[0][0],
                        pair[1][1] - pair[0][1],
                    ),
                )
            })
            .collect();
        let total = crate::render::pie::fsum(legs.iter().map(|(_, _, len)| *len));
        let mut remaining = total * share;
        for (from, to, span) in &legs {
            if remaining <= *span {
                let share = remaining / span;
                let share = if share.is_nan() { 0.0 } else { share };
                return Some(lerp(*from, *to, share));
            }
            remaining -= span;
        }
        points.last().copied()
    }

    fn lerp(from: [f64; 2], to: [f64; 2], share: f64) -> [f64; 2] {
        [
            from[0] + (to[0] - from[0]) * share,
            from[1] + (to[1] - from[1]) * share,
        ]
    }
}

// ---- SegmentBox ----

pub mod segment_box {
    /// Ruby `SegmentBox.crosses?`: Liang-Barsky.
    pub fn crosses(box_: &[f64; 4], from: [f64; 2], to: [f64; 2]) -> bool {
        let mut span = [0.0_f64, 1.0];
        (0..2).all(|axis| inside(&mut span, from[axis], to[axis], box_[axis], box_[axis + 2]))
    }

    /// Ruby `within?`: the segment lies within `gap` of the box.
    pub fn within(box_: &[f64; 4], from: [f64; 2], to: [f64; 2], gap: f64) -> bool {
        distance(box_, from, to) <= gap
    }

    fn distance(box_: &[f64; 4], from: [f64; 2], to: [f64; 2]) -> f64 {
        if crosses(box_, from, to) {
            return 0.0;
        }
        let corners = [
            [box_[0], box_[1]],
            [box_[2], box_[1]],
            [box_[2], box_[3]],
            [box_[0], box_[3]],
        ];
        let mut candidates: Vec<f64> =
            corners.iter().map(|corner| to_segment(corner, from, to)).collect();
        candidates.push(to_box(from, box_));
        candidates.push(to_box(to, box_));
        candidates.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    fn to_box(point: [f64; 2], box_: &[f64; 4]) -> f64 {
        crate::js_math::hypot2(
            f64::max(f64::max(box_[0] - point[0], 0.0), point[0] - box_[2]),
            f64::max(f64::max(box_[1] - point[1], 0.0), point[1] - box_[3]),
        )
    }

    fn to_segment(point: &[f64; 2], from: [f64; 2], to: [f64; 2]) -> f64 {
        let near = nearest_on(point, from, to);
        crate::js_math::hypot2(point[0] - near[0], point[1] - near[1])
    }

    fn nearest_on(point: &[f64; 2], from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
        let run = [to[0] - from[0], to[1] - from[1]];
        let gone = [point[0] - from[0], point[1] - from[1]];
        let share = share_of(gone, run);
        [from[0] + share * run[0], from[1] + share * run[1]]
    }

    fn share_of(gone: [f64; 2], run: [f64; 2]) -> f64 {
        let length = run[0] * run[0] + run[1] * run[1];
        if length == 0.0 {
            return 0.0;
        }
        let dot = gone[0] * run[0] + gone[1] * run[1];
        (dot / length).clamp(0.0, 1.0)
    }

    fn inside(span: &mut [f64; 2], start: f64, finish: f64, low: f64, high: f64) -> bool {
        let delta = finish - start;
        if delta == 0.0 {
            return (low..=high).contains(&start);
        }
        overlaps(span, [(low - start) / delta, (high - start) / delta])
    }

    fn overlaps(span: &mut [f64; 2], bounds: [f64; 2]) -> bool {
        let (first, last) = if bounds[0] <= bounds[1] {
            (bounds[0], bounds[1])
        } else {
            (bounds[1], bounds[0])
        };
        span[0] = span[0].max(first);
        span[1] = span[1].min(last);
        span[0] <= span[1]
    }
}

// ---- RelationLabelSeparator ----

pub mod label_separator {
    use super::relation_path_points;
    use super::segment_box;
    use super::super::scene::{Box, Relation as SceneRelation, Text};

    const ROLE: &str = "relation_label";
    const MARGIN: f64 = 1.0;
    const MAX_STEPS: usize = 24;
    const REACH: f64 = 20.0;
    const SLIDES: [f64; 9] = [0.0, 0.1, -0.1, 0.2, -0.2, 0.3, -0.3, 0.4, -0.4];

    pub struct Separator<'a> {
        pub size: f64,
        pub boxes: &'a [Box],
        pub width_of: &'a dyn Fn(&str) -> f64,
        pub drawn_width_of: &'a dyn Fn(&str) -> f64,
    }

    /// A relation's own points, and with them the lines of all the others,
    /// precomputed (the paths themselves never move).
    struct Track {
        own: Vec<[f64; 2]>,
        reach: f64,
        others: Vec<(Vec<[f64; 2]>, f64)>,
    }

    fn track_of(scene: &SceneRelation, scenes: &[SceneRelation]) -> Track {
        Track {
            own: relation_path_points::call(Some(scene.path.as_str())),
            reach: scene.stroke_width.unwrap_or(0.0) / 2.0 + REACH,
            others: scenes
                .iter()
                .filter(|other| !std::ptr::eq(*other, scene))
                .map(|other| {
                    (
                        relation_path_points::call(Some(other.path.as_str())),
                        other.stroke_width.unwrap_or(0.0) / 2.0,
                    )
                })
                .collect(),
        }
    }

    impl<'a> Separator<'a> {
        /// Ruby `call`: moves colliding labels in place.
        pub fn call(&self, scenes: &mut [SceneRelation]) {
            let taken: Vec<[f64; 4]> = self
                .boxes
                .iter()
                .map(|box_| [box_.x, box_.y, box_.x + box_.width, box_.y + box_.height])
                .collect();
            let tracks: Vec<Track> =
                scenes.iter().map(|scene| track_of(scene, scenes)).collect();
            for (index, scene) in scenes.iter_mut().enumerate() {
                for text_index in 0..scene.texts.len() {
                    if scene.texts[text_index].role != ROLE {
                        continue;
                    }
                    self.free_box(&mut scene.texts[text_index], &taken, &tracks[index]);
                }
            }
        }

        fn free_box(&self, text: &mut Text, taken: &[[f64; 4]], track: &Track) {
            let home = [text.x, text.y];
            let best = self
                .best_move(text, home, taken, track)
                .unwrap_or([0.0, 0.0]);
            text.x = home[0] + best[0];
            text.y = home[1] + best[1];
        }

        fn best_move(
            &self,
            text: &mut Text,
            home: [f64; 2],
            taken: &[[f64; 4]],
            track: &Track,
        ) -> Option<[f64; 2]> {
            let mut fewest: Option<([f64; 2], usize)> = None;
            for action in moves(&track.own, self.size) {
                text.x = home[0] + action[0];
                text.y = home[1] + action[1];
                let Some(count) = self.clashes(text, taken, track) else {
                    continue;
                };
                if count == 0 {
                    return Some(action);
                }
                if fewest.is_none() || count < fewest.unwrap().1 {
                    fewest = Some((action, count));
                }
            }
            fewest.map(|(action, _)| action)
        }

        fn clashes(&self, text: &Text, taken: &[[f64; 4]], track: &Track) -> Option<usize> {
            let box_ = self.box_of(text, &self.width_of);
            if taken.iter().any(|other| overlap(&box_, other)) {
                return None;
            }
            let drawn = self.box_of(text, &self.drawn_width_of);
            if !beside(&drawn, track) {
                return None;
            }
            Some(
                track
                    .others
                    .iter()
                    .filter(|(line, half)| crossed(&drawn, line, *half))
                    .count(),
            )
        }

        fn box_of(&self, text: &Text, measure: &dyn Fn(&str) -> f64) -> [f64; 4] {
            let width = measure(&text.content);
            let left = left_edge(text, width);
            [
                left - MARGIN,
                text.y - self.size,
                left + width + MARGIN,
                text.y + self.size / 4.0,
            ]
        }
    }

    fn moves(points: &[[f64; 2]], size: f64) -> Vec<[f64; 2]> {
        if points.len() < 2 {
            return offsets(size).into_iter().map(|down| [0.0, down]).collect();
        }
        let mut out = vec![];
        for slide in SLIDES {
            let home = relation_path_points::along(points, 0.5).unwrap();
            let at = relation_path_points::along(points, 0.5 + slide).unwrap();
            let (across, down) = (at[0] - home[0], at[1] - home[1]);
            for step in offsets(size) {
                out.push([across, down + step]);
            }
        }
        out
    }

    fn offsets(size: f64) -> Vec<f64> {
        let line = size * 1.3;
        let mut out = vec![0.0];
        for step in 1..=MAX_STEPS {
            out.push(step as f64 * line);
            out.push(-(step as f64) * line);
        }
        out
    }

    fn beside(drawn: &[f64; 4], track: &Track) -> bool {
        let own = &track.own;
        if own.is_empty() {
            return true;
        }
        !crossed(drawn, own, 0.0)
            && own.windows(2).any(|pair| {
                segment_box::within(drawn, pair[0], pair[1], track.reach)
            })
    }

    fn crossed(box_: &[f64; 4], points: &[[f64; 2]], half: f64) -> bool {
        let padded = [
            box_[0] - half,
            box_[1] - half,
            box_[2] + half,
            box_[3] + half,
        ];
        points.windows(2).any(|pair| segment_box::crosses(&padded, pair[0], pair[1]))
    }

    fn left_edge(text: &Text, width: f64) -> f64 {
        match text.anchor.as_str() {
            "middle" => text.x - width / 2.0,
            "end" => text.x - width,
            _ => text.x,
        }
    }

    fn overlap(first: &[f64; 4], second: &[f64; 4]) -> bool {
        first[0] < second[2]
            && second[0] < first[2]
            && first[1] < second[3]
            && second[1] < first[3]
    }
}
