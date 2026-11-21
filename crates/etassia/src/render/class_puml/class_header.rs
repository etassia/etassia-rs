//! Port of Ruby `Notation::PlantUML::ClassHeader` (`class_header.rb`): the
//! circled kind letter and the rules of a class's empty compartments,
//! placed after layout from the final box geometry.
use super::model::{Diagram, Kind};
use super::scene::{Scene, Segment, Spot};

pub struct Glyph {
    pub fill: &'static str,
    pub outline: &'static str,
}

pub const RADIUS: f64 = 11.0;
const CENTRE_ABOVE_BASELINE: f64 = 5.291;
const NAME_GAP: f64 = 3.0;
const EMPTY_HEIGHT: f64 = 48.0;
const EMPTY_RULES: [f64; 2] = [32.0, 40.0];
const BLOCK_SHIFT: f64 = 0.5;

/// Ruby `GLYPHS`: outlines are relative to the circle's centre.
pub fn glyph(kind: Kind) -> Option<Glyph> {
    Some(match kind {
        Kind::Class => Glyph {
            fill: "#ADD1B2",
            outline: "M2.4731,6.1431 Q1.8921,6.4419 1.2529,6.5913 Q0.6138,6.7407 -0.0918,6.7407 Q-2.5986,6.7407 -3.9185,5.0889 Q-5.2383,3.437 -5.2383,0.3159 Q-5.2383,-2.8135 -3.9185,-4.4653 Q-2.5986,-6.1172 -0.0918,-6.1172 Q0.6138,-6.1172 1.2612,-5.9678 Q1.9087,-5.8184 2.4731,-5.5195 L2.4731,-2.7969 Q1.8423,-3.3779 1.2488,-3.6477 Q0.6553,-3.9175 0.0244,-3.9175 Q-1.3203,-3.9175 -2.0051,-2.8508 Q-2.6899,-1.7842 -2.6899,0.3159 Q-2.6899,2.4077 -2.0051,3.4744 Q-1.3203,4.541 0.0244,4.541 Q0.6553,4.541 1.2488,4.2712 Q1.8423,4.0015 2.4731,3.4204 Z",
        },
        Kind::Abstract => Glyph {
            fill: "#A9DCDF",
            outline: "M-0.1367,-4.6519 L-1.2905,0.4199 L1.0254,0.4199 Z M-1.6309,-6.8931 L1.3657,-6.8931 L4.7109,5.5 L2.2622,5.5 L1.4985,2.437 L-1.7803,2.437 L-2.5273,5.5 L-4.9761,5.5 Z",
        },
        Kind::Interface => Glyph {
            fill: "#B4A7E5",
            outline: "M-3.5723,-3.7349 L-3.5723,-5.8931 L3.8071,-5.8931 L3.8071,-3.7349 L1.3418,-3.7349 L1.3418,4.3418 L3.8071,4.3418 L3.8071,6.5 L-3.5723,6.5 L-3.5723,4.3418 L-1.1069,4.3418 L-1.1069,-3.7349 Z",
        },
        Kind::Enum => Glyph {
            fill: "#EB937F",
            outline: "M3.6143,6.5 L-4.1055,6.5 L-4.1055,-5.8931 L3.6143,-5.8931 L3.6143,-3.7349 L-1.6567,-3.7349 L-1.6567,-1.062 L3.1162,-1.062 L3.1162,1.0962 L-1.6567,1.0962 L-1.6567,4.3418 L3.6143,4.3418 Z",
        },
        Kind::Annotation => Glyph {
            fill: "#E3664A",
            outline: "M2.5767,0.2261 Q2.5767,-0.7119 2.1533,-1.2432 Q1.73,-1.7744 0.9912,-1.7744 Q0.2524,-1.7744 -0.1667,-1.2432 Q-0.5859,-0.7119 -0.5859,0.2261 Q-0.5859,1.1724 -0.1667,1.7036 Q0.2524,2.2349 0.9912,2.2349 Q1.73,2.2349 2.1533,1.7036 Q2.5767,1.1724 2.5767,0.2261 Z M4.1206,3.6294 L2.4937,3.6294 L2.4937,2.9487 Q2.1782,3.3887 1.7507,3.592 Q1.3232,3.7954 0.7256,3.7954 Q-0.6357,3.7954 -1.47,2.8159 Q-2.3042,1.8364 -2.3042,0.2261 Q-2.3042,-1.376 -1.4741,-2.3513 Q-0.644,-3.3267 0.7256,-3.3267 Q1.3149,-3.3267 1.7632,-3.1233 Q2.2114,-2.9199 2.4937,-2.5298 L2.4937,-2.8701 Q2.4937,-3.999 1.8752,-4.6133 Q1.2568,-5.2275 0.1113,-5.2275 Q-1.6152,-5.2275 -2.7068,-3.7085 Q-3.7983,-2.1895 -3.7983,0.2427 Q-3.7983,2.791 -2.5366,4.2976 Q-1.2749,5.8042 0.8252,5.8042 Q1.4893,5.8042 2.1118,5.6091 Q2.7344,5.4141 3.3071,5.0239 L4.0708,6.4849 Q3.3984,6.9414 2.6057,7.1697 Q1.813,7.3979 0.9082,7.3979 Q-1.9971,7.3979 -3.7236,5.4639 Q-5.4502,3.5298 -5.4502,0.2427 Q-5.4502,-2.9697 -3.8979,-4.8997 Q-2.3457,-6.8296 0.2109,-6.8296 Q2.0205,-6.8296 3.0706,-5.738 Q4.1206,-4.6465 4.1206,-2.7622 Z",
        },
    })
}

/// Ruby `circles_hidden?`.
pub fn circles_hidden(diagram: &Diagram) -> bool {
    static HIDE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let hide = HIDE.get_or_init(|| regex::Regex::new(r"(?i)\Ahide[ \t]+circle\z").unwrap());
    diagram.directives.iter().any(|line| hide.is_match(line))
}

/// Ruby `apply`: the same scene, its class boxes dressed.
pub fn apply(diagram: &Diagram, scene: &mut Scene, measure: &dyn Fn(&str) -> f64, kind_in_spot: bool) {
    let kinds: Vec<(String, Kind)> = if circles_hidden(diagram) {
        vec![]
    } else {
        diagram
            .classes
            .iter()
            .map(|klass| (klass.name.clone(), klass.kind))
            .collect()
    };
    let positions: Vec<usize> = scene
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, box_)| box_.kind == "class")
        .map(|(index, _)| index)
        .collect();
    for index in positions {
        let kind = kinds
            .iter()
            .find(|(name, _)| *name == scene.boxes[index].id)
            .map(|(_, kind)| *kind);
        place(&mut scene.boxes[index], kind, measure);
        rule_empty_compartments(&mut scene.boxes[index]);
    }
    let _ = kind_in_spot;
}

fn rule_empty_compartments(box_: &mut super::scene::Box) {
    if !empty(box_) {
        return;
    }
    for offset in EMPTY_RULES {
        let y = box_.y + offset;
        box_.separators.push(Segment::new(box_.x, y, box_.x + box_.width, y));
    }
}

fn empty(box_: &super::scene::Box) -> bool {
    box_.separators.is_empty()
        && (box_.height - EMPTY_HEIGHT).abs() < 0.01
        && box_.texts.iter().all(|text| text.role == "class_name")
}

fn place(box_: &mut super::scene::Box, kind: Option<Kind>, measure: &dyn Fn(&str) -> f64) {
    let Some(kind) = kind else { return };
    if !matches!(kind, Kind::Class | Kind::Abstract | Kind::Interface | Kind::Enum | Kind::Annotation) {
        return;
    }
    let name_index = sole_name(box_);
    let Some(name_index) = name_index else { return };
    if box_.spot.is_some() {
        return;
    }
    let width = block_width(&box_.texts[name_index], measure);
    let left = box_.texts[name_index].x - width / 2.0 + BLOCK_SHIFT;
    let baseline = box_.texts[name_index].y;
    box_.spot = Some(Spot {
        kind: kind.as_str().to_string(),
        cx: left + RADIUS,
        cy: baseline - CENTRE_ABOVE_BASELINE,
    });
    box_.texts[name_index].x = left + 2.0 * RADIUS + NAME_GAP;
    box_.texts[name_index].anchor = "start".to_string();
}

fn sole_name(box_: &super::scene::Box) -> Option<usize> {
    let names: Vec<usize> = box_
        .texts
        .iter()
        .enumerate()
        .filter(|(_, text)| text.role == "class_name")
        .map(|(index, _)| index)
        .collect();
    if names.len() == 1 {
        Some(names[0])
    } else {
        None
    }
}

fn block_width(name: &super::scene::Text, measure: &dyn Fn(&str) -> f64) -> f64 {
    2.0 * RADIUS + NAME_GAP + measure(&name.content)
}
