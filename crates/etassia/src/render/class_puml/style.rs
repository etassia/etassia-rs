//! Port of Ruby `Notation::PlantUML::Style` (`style/*.rb`): how a source's
//! `skinparam` lines and `<style>` blocks style a diagram.
use std::collections::HashMap;
use std::sync::OnceLock;

/// Ruby `Style.selector`.
fn selector(element: &str, stereotype: Option<&str>) -> String {
    match stereotype {
        None => element.to_string(),
        Some(name) => {
            let lowered = name.to_lowercase();
            if element.is_empty() {
                format!(".{lowered}")
            } else {
                format!("{element}<<{lowered}>>")
            }
        }
    }
}

/// The properties a renderer asks the sheet for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Prop {
    Background,
    Border,
    FontColour,
    FontName,
    FontSize,
    LineStyle,
    RoundCorner,
    LineThickness,
}

impl Prop {
    fn of(name: &str) -> Option<Prop> {
        Some(match name {
            "backgroundcolor" => Prop::Background,
            "bordercolor" | "linecolor" => Prop::Border,
            "fontcolor" => Prop::FontColour,
            "fontname" => Prop::FontName,
            "fontsize" => Prop::FontSize,
            "linestyle" => Prop::LineStyle,
            "roundcorner" => Prop::RoundCorner,
            "linethickness" | "borderthickness" | "thickness" => Prop::LineThickness,
            _ => return None,
        })
    }
}

/// A parsed value: colour text, a number, or a name.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Colour(String),
    Number(f64),
    /// A Ruby Integer (its `to_s` has no fractional part).
    Int(i64),
    Name(String),
    Style(&'static str),
}

/// Ruby `Style::Sheet`.
#[derive(Clone, Debug, Default)]
pub struct Sheet {
    skin: HashMap<String, HashMap<Prop, Value>>,
    style: HashMap<String, HashMap<Prop, Value>>,
    elements: HashMap<String, HashMap<String, Value>>,
}

impl Sheet {
    /// Ruby `Style::Reader.read(source)`.
    pub fn read(source: &str) -> Sheet {
        Reader::new().read(source)
    }

    /// Ruby `styled?`.
    pub fn styled(&self) -> bool {
        !self.skin.is_empty() || !self.style.is_empty() || !self.elements.is_empty()
    }

    /// Ruby `resolve`: the winning value, with the defaults under it.
    pub fn resolve(
        &self,
        element: &str,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Option<Value> {
        self.explicit(element, property, id, stereotype)
            .or_else(|| from_defaults(element, property))
    }

    /// Ruby `explicit`: nil when the source itself sets nothing.
    pub fn explicit(
        &self,
        element: &str,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Option<Value> {
        let mut candidates: Vec<Option<Value>> = vec![];
        for layer in [&self.skin, &self.style] {
            for selector in selectors(element, stereotype) {
                candidates.push(layer.get(&selector).and_then(|rules| rules.get(&property)).cloned());
            }
        }
        if let Some(id) = id {
            candidates.push(
                self.elements
                    .get(id)
                    .and_then(|props| props.get(prop_key(property)).cloned()),
            );
        }
        candidates.into_iter().flatten().last()
    }

    /// Ruby `element(id)`: the properties written on that element.
    pub fn element(&self, id: &str) -> HashMap<String, Value> {
        self.elements.get(id).cloned().unwrap_or_default()
    }
}

fn selectors(element: &str, stereotype: Option<&str>) -> Vec<String> {
    let mut out = vec![selector("any", None), selector(element, None)];
    if let Some(stereotype) = stereotype {
        out.push(selector(element, Some(stereotype)));
        out.push(selector("", Some(stereotype)));
    }
    out
}

fn prop_key(property: Prop) -> &'static str {
    match property {
        Prop::Background => "background",
        Prop::Border => "border",
        Prop::FontColour => "font_colour",
        Prop::FontName => "font_name",
        Prop::FontSize => "font_size",
        Prop::LineStyle => "line_style",
        Prop::RoundCorner => "round_corner",
        Prop::LineThickness => "line_thickness",
    }
}

/// Ruby `Defaults::RULES`: PlantUML 1.2026.6's own look.
pub fn from_defaults(element: &str, property: Prop) -> Option<Value> {
    // `any` rules first, then the element's own.
    let any: Option<Value> = match property {
        Prop::FontName => Some(Value::Name("sans-serif".into())),
        Prop::FontSize => Some(Value::Number(14.0)),
        Prop::FontColour => Some(Value::Colour("#000000".into())),
        Prop::Background => Some(Value::Colour("#F1F1F1".into())),
        Prop::Border => Some(Value::Colour("#181818".into())),
        Prop::LineThickness => Some(Value::Number(0.5)),
        _ => None,
    };
    let own: Option<Value> = match (element, property) {
        ("document", Prop::Background) => Some(Value::Colour("#FFFFFF".into())),
        (_, Prop::Background) if element == "class" || element == "state" => {
            Some(Value::Colour("#F1F1F1".into()))
        }
        ("class", Prop::RoundCorner) => Some(Value::Number(2.5)),
        ("state", Prop::RoundCorner) => Some(Value::Number(12.5)),
        (_, Prop::FontColour) if element == "class" || element == "state" => {
            Some(Value::Colour("#000000".into()))
        }
        ("class", Prop::Border) | ("state", Prop::Border) => {
            Some(Value::Colour("#181818".into()))
        }
        ("class", Prop::LineThickness) | ("state", Prop::LineThickness) => {
            Some(Value::Number(0.5))
        }
        ("activity", Prop::FontSize) => Some(Value::Number(12.0)),
        ("note", Prop::Background) => Some(Value::Colour("#FEFFDD".into())),
        ("note", Prop::Border) => Some(Value::Colour("#181818".into())),
        ("note", Prop::LineThickness) => Some(Value::Number(0.5)),
        ("arrow", Prop::Border) => Some(Value::Colour("#181818".into())),
        ("arrow", Prop::FontColour) => Some(Value::Colour("#000000".into())),
        ("arrow", Prop::LineThickness) => Some(Value::Int(1)),
        ("start", Prop::Background) => Some(Value::Colour("#222222".into())),
        ("start", Prop::Border) => Some(Value::Colour("#222222".into())),
        _ => None,
    };
    match (any, own) {
        (Some(any), Some(own)) => {
            // Ruby: [any, element].compact.last — the element's own wins
            // only when the element rule carries the property; both Some
            // means the element's own.
            let _ = any;
            Some(own)
        }
        (Some(any), None) => Some(any),
        (None, own) => own,
    }
}

/// Ruby `Style::Colour.resolve`.
pub fn colour_resolve(text: &str) -> Option<String> {
    let word = split_by_gradient(text.trim().strip_prefix('#').unwrap_or(text.trim()));
    let Some(word) = word.filter(|w| !w.is_empty()) else { return None };
    if word.eq_ignore_ascii_case("transparent") {
        return Some("none".to_string());
    }
    if is_hex_word(&word) {
        return Some(expand_hex(&word));
    }
    colour_name(&word.to_lowercase())
}

fn split_by_gradient(text: &str) -> Option<String> {
    let run: String = text
        .chars()
        .take_while(|c| !matches!(c, '|' | '\\' | '/' | '-'))
        .collect();
    Some(run)
}

fn is_hex_word(word: &str) -> bool {
    (word.len() == 3 || word.len() == 6)
        && word.chars().all(|c| c.is_ascii_hexdigit())
}

fn expand_hex(word: &str) -> String {
    let digits = if word.len() == 3 {
        word.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        word.to_string()
    };
    format!("#{}", digits.to_uppercase())
}

/// Ruby `ColourNames::TABLE`.
pub fn colour_name(word: &str) -> Option<String> {
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        HashMap::from([
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
        ])
    });
    table.get(word).map(|value| value.to_string())
}

/// Ruby `Style::Inline.parse`: the colour written on one declaration line.
fn inline_parse(text: &str) -> HashMap<String, Value> {
    let mut properties = HashMap::new();
    let mut parts = text.split(';');
    let first = parts.next().unwrap_or("");
    if let Some(colour) = colour_resolve(first) {
        properties.insert("background".to_string(), Value::Colour(colour));
    }
    for part in parts {
        let Some((key, value)) = part.split_once(':') else { continue };
        match key.to_lowercase().as_str() {
            "line" => {
                if let Some(colour) = colour_resolve(value) {
                    properties.insert("border".to_string(), Value::Colour(colour));
                }
            }
            "text" => {
                if let Some(colour) = colour_resolve(value) {
                    properties.insert("font_colour".to_string(), Value::Colour(colour));
                }
            }
            "line.dashed" => {
                properties.insert("line_style".to_string(), Value::Style("7,7"));
            }
            "line.dotted" => {
                properties.insert("line_style".to_string(), Value::Style("1,3"));
            }
            _ => {}
        }
    }
    properties
}

struct Reader {
    skin: HashMap<String, HashMap<Prop, Value>>,
    style: HashMap<String, HashMap<Prop, Value>>,
    path: Option<Vec<String>>,
    block: Option<(Option<String>, Option<String>)>,
    elements: HashMap<String, HashMap<String, Value>>,
}

impl Reader {
    fn new() -> Reader {
        Reader {
            skin: HashMap::new(),
            style: HashMap::new(),
            path: None,
            block: None,
            elements: HashMap::new(),
        }
    }

    fn read(mut self, source: &str) -> Sheet {
        for text in source.lines().map(str::trim) {
            if text.is_empty() || text.starts_with('\'') {
                continue;
            }
            self.feed(text);
        }
        Sheet {
            skin: self.skin,
            style: self.style,
            elements: self.elements,
        }
    }

    fn feed(&mut self, text: &str) {
        if self.path.is_some() {
            self.style_line(text);
            return;
        }
        if self.block.is_some() {
            self.skin_block_line(text);
            return;
        }
        if text.eq_ignore_ascii_case("<style>") {
            self.path = Some(vec![]);
            return;
        }
        self.statement(text);
    }

    fn statement(&mut self, text: &str) {
        static SKINPARAM: OnceLock<regex::Regex> = OnceLock::new();
        let skinparam = SKINPARAM.get_or_init(|| {
            regex::Regex::new(
                r"(?i)\Askinparam[ \t]+(\S+?)(?:[ \t]*(\{)|[ \t]+(.+))?\z",
            )
            .unwrap()
        });
        static DECLARATION: OnceLock<regex::Regex> = OnceLock::new();
        let declaration = DECLARATION.get_or_init(|| {
            regex::Regex::new(
                r#"(?i)\A(?:abstract[ \t]+class|static[ \t]+class|abstract|class|interface|enum|annotation|state)[ \t]+(?:"[^"]+"[ \t]+as[ \t]+)?([A-Za-z_][\w.]*)(?:<[^<>]+>)?(?:[ \t]*<<([^<>]+)>>)*(?:[ \t]*(#\S+))?"#,
            )
            .unwrap()
        });
        if let Some(m) = skinparam.captures(text) {
            let name = m.get(1).map(|g| g.as_str()).unwrap_or("");
            if m.get(2).is_some() {
                let (element, stereotype) = single(name);
                self.block = Some((element, stereotype));
                return;
            }
            let Some(value) = m.get(3) else { return };
            let (element, stereotype) = single(name);
            let Some(element) = element else { return };
            let (element, property) = self.split_name(&element);
            let Some((element, property)) = element.zip(property) else { return };
            Self::store(
                &mut self.skin,
                Some(element.as_str()),
                property.as_str(),
                stereotype.as_deref(),
                value.as_str(),
            );
            return;
        }
        if let Some(m) = declaration.captures(text) {
            let name = m.get(1).map(|g| g.as_str()).unwrap_or("");
            let colour = m.get(3).map(|g| g.as_str()).unwrap_or("");
            let mut written = inline_parse(colour);
            if let Some(stereotype) = m.get(2) {
                written
                    .insert("stereotype".to_string(), Value::Name(stereotype.as_str().to_lowercase()));
            }
            if !written.is_empty() {
                self.elements.insert(name.to_string(), written);
            }
        }
    }

    fn split_name(&self, name: &str) -> (Option<String>, Option<String>) {
        let name = name.to_lowercase();
        // `defaultfontname` and friends style `any`.
        if matches!(
            name.as_str(),
            "defaultfontname" | "defaultfontsize" | "defaultfontcolor"
        ) {
            let property = match name.as_str() {
                "defaultfontname" => "fontname",
                "defaultfontsize" => "fontsize",
                _ => "fontcolor",
            };
            return (Some("any".to_string()), Some(property.to_string()));
        }
        // The longest known suffix wins.
        const PROPERTIES: [&str; 12] = [
            "backgroundcolor", "bordercolor", "linecolor", "fontcolor",
            "fontname", "fontsize", "linestyle", "roundcorner",
            "linethickness", "borderthickness", "thickness", "color",
        ];
        let mut suffix: Option<(&str, usize)> = None;
        for candidate in PROPERTIES {
            if name.ends_with(candidate)
                && suffix.map(|(_, cut)| candidate.len() > cut).unwrap_or(true)
            {
                suffix = Some((candidate, name.len() - candidate.len()));
            }
        }
        let Some((word, cut)) = suffix else { return (None, None) };
        let element = &name[..cut];
        (
            Some(if element.is_empty() { "any".to_string() } else { element.to_string() }),
            Some(word.to_string()),
        )
    }

    fn skin_block_line(&mut self, text: &str) {
        if text == "}" {
            self.block = None;
            return;
        }
        static PROPERTY: OnceLock<regex::Regex> = OnceLock::new();
        let property = PROPERTY
            .get_or_init(|| regex::Regex::new(r"\A(\w+)(?:<<(\w+)>>)?[ \t:]+(.+?)[ \t;]*\z").unwrap());
        let Some(m) = property.captures(text) else { return };
        let (element, block_stereotype) = self.block.clone().unwrap();
        let mut name = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
        let mut element = element;
        if name.starts_with("arrow") {
            element = Some("arrow".to_string());
            name = name["arrow".len()..].to_string();
        }
        let stereotype = m
            .get(2)
            .map(|g| g.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or(block_stereotype);
        let value = m.get(3).map(|g| g.as_str()).unwrap_or("");
                let mut_map = &mut self.skin;
        Self::store(mut_map, element.as_deref(), name.as_str(), stereotype.as_deref(), value);
    }

    fn style_line(&mut self, text: &str) {
        if text.eq_ignore_ascii_case("</style>") {
            self.path = None;
            return;
        }
        static INLINE: OnceLock<regex::Regex> = OnceLock::new();
        let inline = INLINE
            .get_or_init(|| regex::Regex::new(r"\A([\w.]+)[ \t]*\{[ \t]*(\w+)[ \t:]+(.+?)[ \t;]*\}\z").unwrap());
        static OPENING: OnceLock<regex::Regex> = OnceLock::new();
        let opening = OPENING
            .get_or_init(|| regex::Regex::new(r"\A([\w.]+(?:[ \t]*,[ \t]*[\w.]+)*)[ \t]*\{\z").unwrap());
        static PROPERTY: OnceLock<regex::Regex> = OnceLock::new();
        let property = PROPERTY
            .get_or_init(|| regex::Regex::new(r"\A(\w+)(?:<<(\w+)>>)?[ \t:]+(.+?)[ \t;]*\z").unwrap());
        if let Some(m) = inline.captures(text) {
            let selector = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
            self.path.as_mut().unwrap().push(selector);
            let prop = m.get(2).map(|g| g.as_str()).unwrap_or("").to_lowercase();
            let value = m.get(3).map(|g| g.as_str()).unwrap_or("");
            self.style_property(&prop, value);
            self.path.as_mut().unwrap().pop();
        } else if let Some(m) = opening.captures(text) {
            let selector = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
            self.path.as_mut().unwrap().push(selector);
        } else if text == "}" {
            self.path.as_mut().unwrap().pop();
        } else if let Some(m) = property.captures(text) {
            let prop = m.get(1).map(|g| g.as_str()).unwrap_or("").to_lowercase();
            let value = m.get(3).map(|g| g.as_str()).unwrap_or("");
            self.style_property(&prop, value);
        }
    }

    fn style_property(&mut self, property: &str, value: &str) {
        // PlantUML's <style> has no BorderColor; only skinparam does.
        let Some(path) = self.path.as_ref() else { return };
        let Some(selector) = path.last() else { return };
        if property == "bordercolor" {
            return;
        }
        let (element, stereotype) = target_of(selector);
        Self::store(
            &mut self.style,
            element.as_deref(),
            property,
            stereotype.as_deref(),
            value,
        );
    }

    fn store(
        rules: &mut HashMap<String, HashMap<Prop, Value>>,
        element: Option<&str>,
        property: &str,
        stereotype: Option<&str>,
        value: &str,
    ) {
        let Some(name) = Prop::of(property) else { return };
        if element.is_none() && stereotype.is_none() {
            return;
        }
        let Some(parsed) = parse_value(name, value) else { return };
        // A bare `BackgroundColor` is the canvas, not every element.
        let canvas = match element {
            Some("any") if name == Prop::Background => "document".to_string(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        let key = selector(&canvas, stereotype);
        rules.entry(key).or_default().insert(name, parsed);
    }
}

fn parse_value(name: Prop, value: &str) -> Option<Value> {
    match name {
        Prop::Background | Prop::Border | Prop::FontColour => {
            colour_resolve(value).map(Value::Colour)
        }
        Prop::FontSize | Prop::RoundCorner | Prop::LineThickness => {
            value.parse::<f64>().ok().map(Value::Number)
        }
        Prop::FontName => Some(Value::Name(value.trim_matches('"').to_string())),
        Prop::LineStyle => {
            static STYLES: [(&str, &str); 2] = [("dotted", "1,3"), ("dashed", "7,7")];
            STYLES
                .iter()
                .find(|(word, _)| value.eq_ignore_ascii_case(word))
                .map(|(_, style)| Value::Style(style))
        }
    }
}

/// Ruby `SINGLE`: `Name` or `Name<<Stereo>>`.
fn single(written: &str) -> (Option<String>, Option<String>) {
    static SINGLE: OnceLock<regex::Regex> = OnceLock::new();
    let re = SINGLE.get_or_init(|| regex::Regex::new(r"\A(\w+)(?:<<(\w+)>>)?\z").unwrap());
    match re.captures(written) {
        Some(m) => (
            m.get(1).map(|g| g.as_str().to_lowercase()),
            m.get(2).map(|g| g.as_str().to_lowercase()),
        ),
        None => (None, None),
    }
}

/// Ruby `target_of`: a leading `.` styles a stereotype; the wrappers name
/// the whole diagram's own element.
fn target_of(selector: &str) -> (Option<String>, Option<String>) {
    if let Some(name) = selector.strip_prefix('.') {
        return (None, Some(name.to_string()));
    }
    let element = match selector {
        "document" | "root" => "document",
        "element" => "any",
        "classdiagram" => "class",
        "statediagram" => "state",
        "activitydiagram" => "activity",
        other => other,
    };
    (Some(element.to_string()), None)
}
