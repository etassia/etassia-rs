//! The PlantUML class diagram model: port of Ruby `Notation::PlantUML::
//! {Diagram, Klass, Member, Relation, Junction, Note, Package, Caption,
//! Arrow}` (`lib/etassia/notation/plantuml/*.rb`).
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Class,
    Abstract,
    Interface,
    Enum,
    Annotation,
}

impl Kind {
    /// Ruby `KINDS.fetch(match[1].split.join(" "))`.
    pub fn of_keyword(word: &str) -> Option<Kind> {
        Some(match word {
            "class" | "static class" => Kind::Class,
            "abstract class" | "abstract" => Kind::Abstract,
            "interface" => Kind::Interface,
            "enum" => Kind::Enum,
            "annotation" => Kind::Annotation,
            _ => return None,
        })
    }

    /// The `spot_names_kind?` glyph key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Abstract => "abstract",
            Kind::Interface => "interface",
            Kind::Enum => "enum",
            Kind::Annotation => "annotation",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

impl Visibility {
    pub fn of(mark: &str) -> Option<Visibility> {
        Some(match mark {
            "+" => Visibility::Public,
            "-" => Visibility::Private,
            "#" => Visibility::Protected,
            "~" => Visibility::Package,
            _ => return None,
        })
    }

    /// Ruby `MemberIcon::MARKS`.
    pub fn mark(&self) -> &'static str {
        match self {
            Visibility::Public => "+",
            Visibility::Private => "-",
            Visibility::Protected => "#",
            Visibility::Package => "~",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Package => "package",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemberKind {
    Field,
    Method,
    Separator,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypePosition {
    Suffix,
    Prefix,
}

/// Ruby `Member`.
#[derive(Clone, Debug)]
pub struct Member {
    pub kind: MemberKind,
    pub visibility: Option<Visibility>,
    pub name: String,
    pub kind_type: Option<String>,
    pub parameters: Option<String>,
    pub modifiers: Vec<&'static str>,
    pub type_position: TypePosition,
    pub type_separator: String,
}

impl Member {
    pub fn separator(name: &str, character: &str) -> Member {
        Member {
            kind: MemberKind::Separator,
            visibility: None,
            name: name.to_string(),
            kind_type: Some(character.to_string()),
            parameters: None,
            modifiers: vec![],
            type_position: TypePosition::Suffix,
            type_separator: ": ".into(),
        }
    }
}

/// Ruby `Klass`.
#[derive(Clone, Debug)]
pub struct Klass {
    pub name: String,
    pub kind: Kind,
    pub body: Vec<Member>,
    pub stereotypes: Vec<String>,
    pub generics: Option<String>,
    pub tags: Vec<String>,
    pub package: Option<String>,
}

/// Ruby `Junction`.
#[derive(Clone, Debug)]
pub struct Junction {
    pub from: String,
    pub to: String,
    pub owner: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrowKind {
    Extension,
    Implementation,
    Association,
    Dependency,
    Aggregation,
    Composition,
    Nesting,
}

impl ArrowKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArrowKind::Extension => "extension",
            ArrowKind::Implementation => "implementation",
            ArrowKind::Association => "association",
            ArrowKind::Dependency => "dependency",
            ArrowKind::Aggregation => "aggregation",
            ArrowKind::Composition => "composition",
            ArrowKind::Nesting => "nesting",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// The marker kinds per side, left first. Ruby keeps a Hash with `:left`
/// and `:right` keys; `None` is the Ruby hash's absent key.
#[derive(Clone, Debug, Default)]
pub struct Markers {
    pub left: Option<ArrowKind>,
    pub right: Option<ArrowKind>,
}

impl Markers {
    pub fn get(&self, left: bool) -> Option<ArrowKind> {
        if left { self.left } else { self.right }
    }

    /// `markers.slice(*sides)`: only the asked sides, in `sides` order.
    pub fn slice(&self, sides: &[bool]) -> Vec<(bool, ArrowKind)> {
        sides
            .iter()
            .copied()
            .filter_map(|side| self.get(side).map(|kind| (side, kind)))
            .collect()
    }

    pub fn has_value(&self, kind: ArrowKind) -> bool {
        self.left == Some(kind) || self.right == Some(kind)
    }

    pub fn size(&self) -> usize {
        self.left.iter().chain(self.right.iter()).count()
    }
}

/// Ruby `Arrow.parse`'s result.
#[derive(Clone, Debug)]
pub struct Arrow {
    pub kind: Option<ArrowKind>,
    pub head_is_left: Option<bool>,
    pub both: bool,
    pub markers: Markers,
    pub dashed: bool,
    pub plain: bool,
    pub direction: Option<Direction>,
    pub length: f64,
    pub color: Option<String>,
    pub width: Option<f64>,
    pub dotted: bool,
}

/// Ruby `Relation`.
#[derive(Clone, Debug)]
pub struct Relation {
    pub left: String,
    pub right: String,
    pub kind: Option<ArrowKind>,
    pub head_is_left: Option<bool>,
    pub both: bool,
    pub markers: Markers,
    pub left_multiplicity: Option<String>,
    pub right_multiplicity: Option<String>,
    pub label: Option<String>,
    pub left_role: Option<String>,
    pub right_role: Option<String>,
    pub direction: Option<Direction>,
    pub length: f64,
    pub color: Option<String>,
    pub width: Option<f64>,
    pub dashed: bool,
    pub plain: bool,
    pub dotted: bool,
    pub layout_only: bool,
}

impl Relation {
    pub fn plain_relation(base: &Relation) -> Relation {
        let mut rel = base.clone();
        rel.layout_only = true;
        rel
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteSide {
    Left,
    Right,
    Top,
    Bottom,
    Free,
}

/// Ruby `Note`.
#[derive(Clone, Debug)]
pub struct Note {
    pub side: NoteSide,
    pub target: Option<String>,
    pub member: Option<String>,
    pub color: Option<String>,
    pub stereotype: Option<String>,
    pub lines: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackageShape {
    Folder,
    Frame,
}

/// Ruby `Package`.
#[derive(Clone, Debug)]
pub struct Package {
    pub id: String,
    pub title: String,
    pub shape: PackageShape,
    pub icon: bool,
    pub parent: Option<String>,
    pub color: Option<String>,
}

impl Package {
    /// Ruby `Package.chain`: the ids around `id`, outermost first and `id`
    /// last; empty for `None`.
    pub fn chain(id: &Option<String>, packages: &[Package]) -> Vec<String> {
        let mut chain: Vec<String> = vec![];
        let mut cursor = id.clone();
        while let Some(current) = cursor {
            chain.insert(0, current.clone());
            cursor = packages
                .iter()
                .find(|package| package.id == current)
                .and_then(|package| package.parent.clone());
        }
        chain
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CaptionKind {
    Title,
    Header,
    Footer,
    Caption,
    Legend,
}

impl CaptionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptionKind::Title => "title",
            CaptionKind::Header => "header",
            CaptionKind::Footer => "footer",
            CaptionKind::Caption => "caption",
            CaptionKind::Legend => "legend",
        }
    }
}

/// Ruby `Caption`.
#[derive(Clone, Debug)]
pub struct Caption {
    pub kind: CaptionKind,
    pub text: String,
}

/// Ruby `Notation::PlantUML::StyleSheet` (what a `<style>` block sets).
#[derive(Clone, Debug, Default)]
pub struct StyleSheet {
    pub background: Option<String>,
    pub rules: HashMap<CaptionKind, RuleProps>,
}

/// The properties a caption-kind style rule may set.
#[derive(Clone, Debug, Default)]
pub struct RuleProps {
    pub background: Option<String>,
    pub colour: Option<String>,
    pub size: Option<f64>,
}

/// Ruby `Diagram`.
#[derive(Clone, Debug, Default)]
pub struct Diagram {
    pub classes: Vec<Klass>,
    pub relations: Vec<Relation>,
    pub junctions: Vec<Junction>,
    pub directives: Vec<String>,
    pub notes: Vec<Note>,
    pub packages: Vec<Package>,
    pub captions: Vec<Caption>,
    pub style: StyleSheet,
}

impl Diagram {
    pub fn package_chain(&self, id: &Option<String>) -> Vec<String> {
        Package::chain(id, &self.packages)
    }
}
