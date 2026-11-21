//! The box chrome rules: ports of Ruby `MemberIcon`, `MemberPitch`,
//! `MemberRule` and `StereotypeHead` (`member_icon.rb`, `member_pitch.rb`,
//! `member_rule.rb`, `stereotype_head.rb`).
use super::model::{Kind, MemberKind, Visibility};
use super::scene::{Icon, Segment};

// ---- MemberIcon ----

pub mod member_icon {
    use super::*;

    pub const CENTRE_INSET: f64 = 11.0;
    pub const TEXT_INSET: f64 = 20.0;
    pub const TEXT_PLAIN: f64 = 6.0;
    /// Offset of the icon's centre above the row's baseline.
    pub fn centre_rise(shape: &str) -> f64 {
        match shape {
            "circle" | "square" => 3.79,
            _ => 4.79,
        }
    }

    pub fn shape_of(visibility: Visibility) -> &'static str {
        match visibility {
            Visibility::Public => "circle",
            Visibility::Private => "square",
            Visibility::Protected => "diamond",
            Visibility::Package => "triangle",
        }
    }

    /// Ruby `DISABLED`: `skinparam classAttributeIconSize 0`.
    pub fn enabled(directives: &[String]) -> bool {
        static DISABLED: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let disabled = DISABLED.get_or_init(|| {
            regex::Regex::new(r"(?i)\Askinparam[ \t]+classAttributeIconSize[ \t]+0\z")
                .unwrap()
        });
        !directives.iter().any(|line| disabled.is_match(line))
    }

    /// Ruby `insets`: how far each row's text starts from the box's left
    /// edge; `None` for a separator.
    pub fn insets(
        kinds: &[MemberKind],
        visibilities: &[Option<Visibility>],
        enabled: bool,
    ) -> Vec<Option<f64>> {
        let iconed: Vec<MemberKind> = if enabled { iconed_kinds(kinds, visibilities) } else { vec![] };
        kinds
            .iter()
            .map(|kind| {
                if *kind == MemberKind::Separator {
                    return None;
                }
                Some(if iconed.contains(kind) { TEXT_INSET } else { TEXT_PLAIN })
            })
            .collect()
    }

    fn iconed_kinds(
        kinds: &[MemberKind],
        visibilities: &[Option<Visibility>],
    ) -> Vec<MemberKind> {
        kinds
            .iter()
            .zip(visibilities.iter())
            .filter_map(|(kind, visibility)| visibility.map(|_| *kind))
            .collect()
    }

    /// Ruby `visible_text`: the row's text without the prefix an icon
    /// replaces.
    pub fn visible_text(content: &str, visibility: Option<Visibility>, enabled: bool) -> String {
        let Some(visibility) = visibility else { return content.to_string() };
        if !enabled {
            return content.to_string();
        }
        content
            .strip_prefix(visibility.mark())
            .unwrap_or(content)
            .to_string()
    }

    /// Ruby `at`: the icon before a member, `None` without a visibility.
    pub fn at(
        visibility: Option<Visibility>,
        kind: MemberKind,
        left: f64,
        baseline: f64,
    ) -> Option<Icon> {
        let visibility = visibility?;
        let shape = shape_of(visibility);
        Some(Icon {
            shape: shape.to_string(),
            visibility: visibility.as_str().to_string(),
            filled: kind == MemberKind::Method,
            cx: left + CENTRE_INSET,
            cy: baseline - centre_rise(shape),
        })
    }
}

// ---- MemberPitch ----

pub mod member_pitch {
    use super::MemberKind;

    pub const ROW: f64 = 16.48828125;
    pub const GROUP_GAP: f64 = 8.0;
    pub const TITLED_DROP: f64 = 3.5;
    pub const TITLED_EXTRA: f64 = 4.0;
    pub const BARE_GAP: f64 = 8.0;
    pub const BARE_RULE_DROP: f64 = 10.95;

    /// Ruby `layout`: each row's offset from the first row's baseline, and
    /// the extent the rows add to a box.
    pub fn layout(kinds: &[MemberKind], titles: &[&str], gaps: bool) -> (Vec<f64>, f64) {
        let mut cursor = if gaps { lead(kinds) } else { 0.0 };
        let mut last = -ROW;
        let mut offsets = vec![];
        for index in 0..kinds.len() {
            if index > 0 && kinds[index - 1] == MemberKind::Field && kinds[index] == MemberKind::Method {
                cursor += GROUP_GAP;
            }
            let (offset, next, next_last) = place(
                kinds[index],
                titles.get(index).copied().unwrap_or(""),
                cursor,
                last,
            );
            offsets.push(offset);
            cursor = next;
            last = next_last;
        }
        (offsets, cursor + if gaps { tail(kinds) } else { 0.0 })
    }

    /// The gap above the rows of a box with methods and no fields.
    pub fn lead(kinds: &[MemberKind]) -> f64 {
        if kinds.contains(&MemberKind::Separator) {
            return 0.0;
        }
        if kinds.contains(&MemberKind::Method) && !kinds.contains(&MemberKind::Field) {
            GROUP_GAP
        } else {
            0.0
        }
    }

    /// The gap below the rows of a box with fields and no methods.
    pub fn tail(kinds: &[MemberKind]) -> f64 {
        if kinds.contains(&MemberKind::Separator) {
            return 0.0;
        }
        if kinds.contains(&MemberKind::Field) && !kinds.contains(&MemberKind::Method) {
            GROUP_GAP
        } else {
            0.0
        }
    }

    fn place(kind: MemberKind, title: &str, cursor: f64, last: f64) -> (f64, f64, f64) {
        if kind != MemberKind::Separator {
            return (cursor, cursor + ROW, cursor);
        }
        if title.is_empty() {
            return (last + BARE_RULE_DROP, cursor + BARE_GAP, last);
        }
        (cursor + TITLED_DROP, cursor + ROW + TITLED_EXTRA, cursor)
    }
}

// ---- MemberRule ----

pub mod member_rule {
    use super::Segment;

    #[derive(Clone, Copy)]
    pub struct Style {
        pub width: f64,
        pub dash: Option<&'static str>,
        pub double: bool,
    }

    const DOUBLE_GAP: f64 = 2.0;

    pub fn style_of(character: &str) -> Style {
        match character {
            "." => Style { width: 1.0, dash: Some("1,2"), double: false },
            "=" => Style { width: 1.0, dash: None, double: true },
            "_" => Style { width: 0.5, dash: None, double: false },
            _ => Style { width: 1.0, dash: None, double: false },
        }
    }

    /// Ruby `style`: the rule's parts as drawn.
    pub fn apply(character: &str, pieces: &[Segment]) -> Vec<Segment> {
        let rule = style_of(character);
        let mut drawn: Vec<Segment> = pieces
            .iter()
            .map(|piece| Segment {
                x1: piece.x1,
                y1: piece.y1,
                x2: piece.x2,
                y2: piece.y2,
                width: Some(rule.width),
                dash: rule.dash,
            })
            .collect();
        if rule.double {
            let lowered: Vec<Segment> = drawn
                .iter()
                .map(|piece| Segment {
                    x1: piece.x1,
                    y1: piece.y1 + DOUBLE_GAP,
                    x2: piece.x2,
                    y2: piece.y2 + DOUBLE_GAP,
                    width: piece.width,
                    dash: piece.dash,
                })
                .collect();
            drawn.extend(lowered);
        }
        drawn
    }
}

// ---- StereotypeHead ----

pub mod stereotype_head {
    use super::super::model::Klass;
    use super::super::reader_support::class_name;
    use super::Kind;

    pub const STEREOTYPE_BASELINE: f64 = 16.6016;
    pub const NAME_BASELINE: f64 = 32.668;
    pub const CIRCLE_X: f64 = 15.0;
    pub const CIRCLE_Y: f64 = 20.3105;
    pub const BLOCK_LEFT: f64 = 27.0;
    pub const STEREOTYPE_SIDE: f64 = 28.0;
    pub const NAME_SIDE: f64 = 32.0;
    pub const NAME_LEFT: f64 = 29.0;

    /// Ruby `applies?`: a lone class carrying one `<<stereotype>>`.
    pub fn applies(klass: &Klass, circle: bool) -> bool {
        circle
            && klass.kind == Kind::Class
            && klass.stereotypes.len() == 1
            && class_name::lines(&klass.name).len() == 1
    }

    /// Ruby `width`: the width the header alone asks of the box.
    pub fn width(stereotype_width: f64, name_width: f64) -> f64 {
        (stereotype_width + STEREOTYPE_SIDE).max(name_width + NAME_SIDE)
    }

    /// Ruby `centre`: the centre line the two rows share.
    pub fn centre(stereotype_width: f64, name_width: f64) -> f64 {
        let left = if name_width + 4.0 > stereotype_width {
            NAME_LEFT
        } else {
            BLOCK_LEFT
        };
        left + (stereotype_width.max(name_width) / 2.0)
    }

    /// Ruby `baselines`: the stereotype's and the name's.
    pub fn baselines() -> [f64; 2] {
        [STEREOTYPE_BASELINE, NAME_BASELINE]
    }

    /// Ruby `circle_centre`.
    pub fn circle_centre() -> [f64; 2] {
        [CIRCLE_X, CIRCLE_Y]
    }
}

