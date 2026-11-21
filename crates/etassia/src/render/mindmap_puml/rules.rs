//! What the `<style>` blocks of a mindmap source say about its nodes and
//! connectors. Port of `lib/etassia/notation/plantuml/tree/rules.rb` and
//! `lib/etassia/notation/plantuml/style/{colour,colour_names}.rb`.

use crate::render::pie::strip;
use std::sync::OnceLock;

/// A resolved rule: the selector path it hangs on, the property it sets
/// and the order it was read in.
#[derive(Clone, Debug)]
struct Rule {
    path: Vec<Vec<String>>,
    property: (&'static str, String),
    order: usize,
}

/// What a source's `<style>` blocks set. A rule's selectors nest
/// (`mindmapDiagram { node { BackGroundColor lightgray } }`); it applies
/// to an element every selector of its path names: `node`, `arrow`,
/// `boxless`, `:depth(n)`, and the diagram wrapper. A later rule wins
/// over an earlier one of the same specificity, a longer path over a
/// shorter one. A selector this struct does not know never matches.
#[derive(Default)]
pub struct Rules {
    rules: Vec<Rule>,
}

const WRAPPERS: [&str; 5] = ["wbsdiagram", "mindmapdiagram", "document", "root", "element"];
const KINDS: [&str; 3] = ["node", "arrow", "boxless"];

/// The property names the reader knows, keyed by their lower-cased
/// spelling in the source.
fn property_name(name: &str) -> Option<&'static str> {
    match name {
        "linecolor" => Some("stroke"),
        "backgroundcolor" => Some("fill"),
        "fontcolor" => Some("font_colour"),
        "linethickness" => Some("stroke_width"),
        "linestyle" => Some("dasharray"),
        "roundcorner" => Some("round_corner"),
        _ => None,
    }
}

impl Rules {
    /// Adds one `<style>` block's text.
    pub fn add(&mut self, text: &str) {
        let mut path: Vec<Vec<String>> = vec![];
        for line in tokens(text) {
            path = self.read(&line, path);
        }
    }

    /// The properties the node's rules set: `depth` 0 for the root.
    pub fn node(&self, depth: usize, boxless: bool) -> Look {
        let mut tags = vec![format!("depth({depth})"), "node".to_string()];
        if boxless {
            tags.push("boxless".to_string());
        }
        self.resolve(&tags)
    }

    /// The properties the connector's rules set; `depth` is the depth of
    /// the parent the connector leaves, 0 for the root.
    pub fn arrow(&self, depth: usize) -> Look {
        self.resolve(&[format!("depth({depth})"), "arrow".to_string()])
    }

    fn read(&mut self, line: &str, mut path: Vec<Vec<String>>) -> Vec<Vec<String>> {
        if line == "}" {
            path.pop();
            return path;
        }
        if let Some(selector) = opening(line) {
            // `opening[1].downcase.split(/[ \t]*,[ \t]*/)`: the selector's
            // comma-split alternatives.
            static COMMA: OnceLock<regex::Regex> = OnceLock::new();
            let re =
                COMMA.get_or_init(|| regex::Regex::new(r"[ \t]*,[ \t]*").expect("comma pattern"));
            let lowered = selector.to_lowercase();
            let alternatives: Vec<String> =
                re.split(&lowered).map(|part| part.to_string()).collect();
            path.push(alternatives);
            return path;
        }
        self.store(&path, line);
        path
    }

    fn store(&mut self, path: &[Vec<String>], line: &str) {
        let Some((name, value)) = property(line) else {
            return;
        };
        let Some(name) = property_name(&name.to_lowercase()) else {
            return;
        };
        self.rules.push(Rule {
            path: path.to_vec(),
            property: (name, value.to_string()),
            order: self.rules.len(),
        });
    }

    fn resolve(&self, tags: &[String]) -> Look {
        let mut matching: Vec<&Rule> =
            self.rules.iter().filter(|rule| applies(&rule.path, tags)).collect();
        matching.sort_by_key(|rule| (specificity(rule), rule.order));
        let mut look = Look::default();
        for rule in matching {
            apply(&mut look, rule);
        }
        look
    }
}

/// The properties a node or connector carries, after conversion.
#[derive(Default, Debug)]
pub struct Look {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub font_colour: Option<String>,
    pub stroke_width: Option<f64>,
    pub round_corner: Option<f64>,
    pub dasharray: Option<String>,
}

fn apply(look: &mut Look, rule: &Rule) {
    let (name, value) = &rule.property;
    match *name {
        "stroke" => look.stroke = resolve_colour(value),
        "fill" => look.fill = resolve_colour(value),
        "font_colour" => look.font_colour = resolve_colour(value),
        "stroke_width" => look.stroke_width = number(value),
        "round_corner" => look.round_corner = number(value),
        "dasharray" => look.dasharray = dash(value),
        _ => {}
    }
}

fn applies(path: &[Vec<String>], tags: &[String]) -> bool {
    path.iter().all(|alternatives| {
        alternatives.iter().any(|selector| names(selector, tags))
    })
}

fn names(selector: &str, tags: &[String]) -> bool {
    if WRAPPERS.contains(&selector) {
        return true;
    }
    if let Some(depth) = depth_selector(selector) {
        return tags.contains(&format!("depth({depth})"));
    }
    KINDS.contains(&selector) && tags.iter().any(|tag| tag == selector)
}

/// `:depth(2)`: the depth in the selector, when it is one.
fn depth_selector(selector: &str) -> Option<usize> {
    selector.strip_prefix(":depth(")?.strip_suffix(')')?.parse().ok()
}

fn specificity(rule: &Rule) -> usize {
    rule.path.iter().filter(|alternatives| !wrapper(alternatives)).count()
}

fn wrapper(alternatives: &[String]) -> bool {
    alternatives.iter().all(|selector| WRAPPERS.contains(&selector.as_str()))
}

/// The style sheet's token lines: `//` comments dropped, each brace on its
/// own line, blanks stripped, empties gone.
fn tokens(text: &str) -> Vec<String> {
    let mut sheet = strip_comments(text).replace('{', "{\n").replace('}', "\n}\n");
    sheet.push('\n');
    let mut out = vec![];
    for line in sheet.split('\n') {
        let line = line.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0'));
        if !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

/// `gsub(%r{//[^\n]*}, "")`.
fn strip_comments(text: &str) -> String {
    static COMMENT: OnceLock<regex::Regex> = OnceLock::new();
    let re = COMMENT.get_or_init(|| regex::Regex::new(r"//[^\n]*").expect("comment pattern"));
    re.replace_all(text, "").into_owned()
}

/// `OPENING = /\A(.+?)[ \t]*\{\z/`: a selector line, however much of the
/// trailing blanks it keeps.
fn opening(line: &str) -> Option<&str> {
    let rest = line.strip_suffix('{')?;
    let trimmed = rest.trim_end_matches([' ', '\t']);
    if trimmed.is_empty() {
        return None;
    }
    // The lazy `.+?` needs at least one character, which the emptiness
    // check already covers; the suffix must be the only thing after it.
    Some(trimmed)
}

/// `PROPERTY = /\A(\w+)[ \t:]+(.+?)[ \t;]*\z/`: a property name, its
/// separator, and the value with the trailing blanks and semicolons gone.
fn property(line: &str) -> Option<(String, String)> {
    static PROPERTY: OnceLock<regex::Regex> = OnceLock::new();
    let re = PROPERTY.get_or_init(|| {
        regex::Regex::new(r"\A((?-u:\w+))[ \t:]+(.+?)[ \t;]*\z").expect("property pattern")
    });
    let captures = re.captures(line)?;
    Some((captures[1].to_string(), captures[2].to_string()))
}

/// The CSS colour names PlantUML accepts, with the hex value its
/// 1.2026.6 SVG output writes for each (measured against the JAR).
fn colour_name(word: &str) -> Option<&'static str> {
    static TABLE: OnceLock<std::collections::HashMap<&'static str, &'static str>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        [
            ("aliceblue", "#F0F8FF"), ("antiquewhite", "#FAEBD7"),
            ("aqua", "#00FFFF"), ("aquamarine", "#7FFFD4"),
            ("azure", "#F0FFFF"), ("beige", "#F5F5DC"),
            ("bisque", "#FFE4C4"), ("black", "#000000"),
            ("blanchedalmond", "#FFEBCD"), ("blue", "#0000FF"),
            ("blueviolet", "#8A2BE2"), ("brown", "#A52A2A"),
            ("burlywood", "#DEB887"), ("cadetblue", "#5F9EA0"),
            ("chartreuse", "#7FFF00"), ("chocolate", "#D2691E"),
            ("coral", "#FF7F50"), ("cornflowerblue", "#6495ED"),
            ("cornsilk", "#FFF8DC"), ("crimson", "#DC143C"),
            ("cyan", "#00FFFF"), ("darkblue", "#00008B"),
            ("darkcyan", "#008B8B"), ("darkgoldenrod", "#B8860B"),
            ("darkgray", "#A9A9A9"), ("darkgreen", "#006400"),
            ("darkgrey", "#A9A9A9"), ("darkkhaki", "#BDB76B"),
            ("darkmagenta", "#8B008B"), ("darkolivegreen", "#556B2F"),
            ("darkorange", "#FF8C00"), ("darkorchid", "#9932CC"),
            ("darkred", "#8B0000"), ("darksalmon", "#E9967A"),
            ("darkseagreen", "#8FBC8F"), ("darkslateblue", "#483D8B"),
            ("darkslategray", "#2F4F4F"), ("darkslategrey", "#2F4F4F"),
            ("darkturquoise", "#00CED1"), ("darkviolet", "#9400D3"),
            ("deeppink", "#FF1493"), ("deepskyblue", "#00BFFF"),
            ("dimgray", "#696969"), ("dimgrey", "#696969"),
            ("dodgerblue", "#1E90FF"), ("firebrick", "#B22222"),
            ("floralwhite", "#FFFAF0"), ("forestgreen", "#228B22"),
            ("fuchsia", "#FF00FF"), ("gainsboro", "#DCDCDC"),
            ("ghostwhite", "#F8F8FF"), ("gold", "#FFD700"),
            ("goldenrod", "#DAA520"), ("gray", "#808080"),
            ("green", "#008000"), ("greenyellow", "#ADFF2F"),
            ("grey", "#808080"), ("honeydew", "#F0FFF0"),
            ("hotpink", "#FF69B4"), ("indianred", "#CD5C5C"),
            ("indigo", "#4B0082"), ("ivory", "#FFFFF0"),
            ("khaki", "#F0E68C"), ("lavender", "#E6E6FA"),
            ("lavenderblush", "#FFF0F5"), ("lawngreen", "#7CFC00"),
            ("lemonchiffon", "#FFFACD"), ("lightblue", "#ADD8E6"),
            ("lightcoral", "#F08080"), ("lightcyan", "#E0FFFF"),
            ("lightgoldenrodyellow", "#FAFAD2"), ("lightgray", "#D3D3D3"),
            ("lightgreen", "#90EE90"), ("lightgrey", "#D3D3D3"),
            ("lightpink", "#FFB6C1"), ("lightsalmon", "#FFA07A"),
            ("lightseagreen", "#20B2AA"), ("lightskyblue", "#87CEFA"),
            ("lightslategray", "#778899"), ("lightslategrey", "#778899"),
            ("lightsteelblue", "#B0C4DE"), ("lightyellow", "#FFFFE0"),
            ("lime", "#00FF00"), ("limegreen", "#32CD32"),
            ("linen", "#FAF0E6"), ("magenta", "#FF00FF"),
            ("maroon", "#800000"), ("mediumaquamarine", "#66CDAA"),
            ("mediumblue", "#0000CD"), ("mediumorchid", "#BA55D3"),
            ("mediumpurple", "#9370D8"), ("mediumseagreen", "#3CB371"),
            ("mediumslateblue", "#7B68EE"), ("mediumspringgreen", "#00FA9A"),
            ("mediumturquoise", "#48D1CC"), ("mediumvioletred", "#C71585"),
            ("midnightblue", "#191970"), ("mintcream", "#F5FFFA"),
            ("mistyrose", "#FFE4E1"), ("moccasin", "#FFE4B5"),
            ("navajowhite", "#FFDEAD"), ("navy", "#000080"),
            ("oldlace", "#FDF5E6"), ("olive", "#808000"),
            ("olivedrab", "#6B8E23"), ("orange", "#FFA500"),
            ("orangered", "#FF4500"), ("orchid", "#DA70D6"),
            ("palegoldenrod", "#EEE8AA"), ("palegreen", "#98FB98"),
            ("paleturquoise", "#AFEEEE"), ("palevioletred", "#D87093"),
            ("papayawhip", "#FFEFD5"), ("peachpuff", "#FFDAB9"),
            ("peru", "#CD853F"), ("pink", "#FFC0CB"), ("plum", "#DDA0DD"),
            ("powderblue", "#B0E0E6"), ("purple", "#800080"),
            ("red", "#FF0000"), ("rosybrown", "#BC8F8F"),
            ("royalblue", "#4169E1"), ("saddlebrown", "#8B4513"),
            ("salmon", "#FA8072"), ("sandybrown", "#F4A460"),
            ("seagreen", "#2E8B57"), ("seashell", "#FFF5EE"),
            ("sienna", "#A0522D"), ("silver", "#C0C0C0"),
            ("skyblue", "#87CEEB"), ("slateblue", "#6A5ACD"),
            ("slategray", "#708090"), ("slategrey", "#708090"),
            ("snow", "#FFFAFA"), ("springgreen", "#00FF7F"),
            ("steelblue", "#4682B4"), ("tan", "#D2B48C"),
            ("teal", "#008080"), ("thistle", "#D8BFD8"),
            ("tomato", "#FF6347"), ("turquoise", "#40E0D0"),
            ("violet", "#EE82EE"), ("wheat", "#F5DEB3"),
            ("white", "#FFFFFF"), ("whitesmoke", "#F5F5F5"),
            ("yellow", "#FFFF00"), ("yellowgreen", "#9ACD32"),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>()
    });
    table.get(word).copied()
}

/// `HEX = /\A\h{3}(?:\h{3})?\z/`.
fn is_hex(word: &str) -> bool {
    (word.len() == 3 || word.len() == 6)
        && word.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Reads a PlantUML colour (`#red`, `LightYellow`, `#F80`, `#a1b2c3`,
/// `#white|red` for a gradient) as the `#RRGGBB` PlantUML writes into its
/// SVG, or "none" for `transparent`. A gradient reads as its first colour.
pub fn resolve_colour(text: &str) -> Option<String> {
    static GRADIENT: OnceLock<regex::Regex> = OnceLock::new();
    let re = GRADIENT.get_or_init(|| regex::Regex::new(r"[|\\/-]").expect("gradient pattern"));
    let word = strip(text).strip_prefix('#').unwrap_or(strip(text));
    let word = re.split(word).next().unwrap_or("");
    if word.is_empty() {
        return None;
    }
    if word.eq_ignore_ascii_case("transparent") {
        return Some("none".to_string());
    }
    if is_hex(word) {
        let digits: String = if word.len() == 3 {
            word.chars().flat_map(|d| [d, d]).collect()
        } else {
            word.to_string()
        };
        return Some(format!("#{}", digits.to_uppercase()));
    }
    colour_name(&word.to_lowercase()).map(str::to_string)
}

/// Ruby `Float(value)`, nil when it does not read.
fn number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

/// `dash`: a positive length reads as the pair its integer spells.
fn dash(value: &str) -> Option<String> {
    let length = number(value)?;
    if length > 0.0 {
        Some(format!("{},{}", length as i64, length as i64))
    } else {
        None
    }
}
