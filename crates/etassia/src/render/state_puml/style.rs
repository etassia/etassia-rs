//! Port of `lib/etassia/notation/plantuml/style/` — the shared reading of
//! how a PlantUML diagram is styled: `Colour`, `ColourNames`, `Inline`,
//! `Reader` (`skinparam` lines and `<style>` blocks), `Sheet` and
//! `Defaults`.

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

// ---- colour_names.rb ----

/// The CSS colour names PlantUML accepts, with the hex its SVG writes.
fn colour_names() -> &'static HashMap<&'static str, &'static str> {
    static CELL: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
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
    })
}

// ---- colour.rb ----

/// A resolved PlantUML colour: the `#RRGGBB` its SVG writes, or `"none"`
/// for `transparent`; `None` when the text names no colour.
pub fn colour_resolve(text: &str) -> Option<String> {
    let word = text
        .trim()
        .strip_prefix('#')
        .unwrap_or(text.trim())
        .split(['|', '\\', '/', '-'])
        .next()
        .unwrap_or("");
    if word.is_empty() {
        return None;
    }
    if word.eq_ignore_ascii_case("transparent") {
        return Some("none".to_string());
    }
    let hex = word.len() == 3 || word.len() == 6;
    if hex && word.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(expand(word));
    }
    colour_names().get(word.to_ascii_lowercase().as_str()).map(|s| s.to_string())
}

fn expand(digits: &str) -> String {
    let digits: String = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_string()
    };
    format!("#{}", digits.to_ascii_uppercase())
}

// ---- inline.rb ----

/// The colour a declaration carries on its own line: `#pink`,
/// `#pink;line:blue;line.dashed;text:green`, `##blue`, `##[dashed]blue`,
/// `#pink|white`.
pub fn inline_parse(text: &str) -> Props {
    let line_only = Regex::new(r"\A##(?:\[(\w+)\])?(\S+)\z").unwrap();
    if let Some(m) = line_only.captures(text) {
        let mut props = Props::default();
        props.border = colour_resolve(&m[2]);
        props.line_style = line_styles(m.get(1).map(|g| g.as_str()).unwrap_or("")).map(str::to_string);
        return props;
    }
    let mut props = Props::default();
    let mut parts = text.split(';');
    let first = parts.next().unwrap_or("");
    props.background = colour_resolve(first);
    for part in parts {
        inline_property(part, &mut props);
    }
    props
}

fn inline_property(part: &str, props: &mut Props) {
    let (key, value) = match part.split_once(':') {
        Some((k, v)) => (k, Some(v)),
        None => (part, None),
    };
    match key.to_ascii_lowercase().as_str() {
        "line" => props.border = value.and_then(colour_resolve),
        "text" => props.font_colour = value.and_then(colour_resolve),
        "line.dashed" | "line.dotted" => {
            props.line_style = line_styles(key.split('.').next_back().unwrap_or(""))
                .map(str::to_string);
        }
        _ => {}
    }
}

fn line_styles(word: &str) -> Option<&'static str> {
    match word {
        "dashed" => Some("7,7"),
        "dotted" => Some("1,3"),
        "bold" | "plain" => None,
        _ => None,
    }
}

// ---- property values ----

/// The properties a rule can carry, as written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    pub background: Option<String>,
    pub border: Option<String>,
    pub font_colour: Option<String>,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub line_style: Option<String>,
    pub round_corner: Option<f64>,
    pub line_thickness: Option<f64>,
    /// Ruby's arrow default carries an Integer thickness; skinparam
    /// values are Floats, and `to_s` writes them differently.
    pub line_thickness_int: bool,
    pub stereotype: Option<String>,
}

impl Props {


    fn get(&self, name: Prop) -> Option<Value> {
        match name {
            Prop::Background => self.background.clone().map(Value::Str),
            Prop::Border => self.border.clone().map(Value::Str),
            Prop::FontColour => self.font_colour.clone().map(Value::Str),
            Prop::FontName => self.font_name.clone().map(Value::Str),
            Prop::FontSize => self.font_size.map(Value::Num),
            Prop::LineStyle => self.line_style.clone().map(Value::Str),
            Prop::RoundCorner => self.round_corner.map(Value::Num),
            Prop::LineThickness => {
                if self.line_thickness_int {
                    self.line_thickness.map(Value::Int)
                } else {
                    self.line_thickness.map(Value::Num)
                }
            }
        }
    }

    fn set(&mut self, name: Prop, value: Value) {
        match name {
            Prop::Background => self.background = value.as_str(),
            Prop::Border => self.border = value.as_str(),
            Prop::FontColour => self.font_colour = value.as_str(),
            Prop::FontName => self.font_name = value.as_str(),
            Prop::FontSize => self.font_size = value.as_num(),
            Prop::LineStyle => self.line_style = value.as_str(),
            Prop::RoundCorner => self.round_corner = value.as_num(),
            Prop::LineThickness => {
                self.line_thickness = value.as_num();
                self.line_thickness_int = matches!(value, Value::Int(_));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    Num(f64),
    /// A Ruby Integer: `to_s` writes no decimal point.
    Int(f64),
}

impl Value {
    pub fn as_str(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
}

/// A named rule: `any`, an element kind, `element<<stereotype>>` or a
/// bare `.stereotype`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Selector {
    pub element: Option<String>,
    pub stereotype: Option<String>,
}

impl Selector {
    fn bare(element: &str) -> Selector {
        Selector { element: Some(element.to_string()), stereotype: None }
    }
    fn any() -> Selector {
        Selector::bare("any")
    }
    fn key(element: Option<&str>, stereotype: Option<&str>) -> Selector {
        Selector {
            element: element.map(str::to_string),
            stereotype: stereotype.map(str::to_string),
        }
    }
}

/// The key a rule is stored under (Ruby `Style.selector`).
fn selector(element: Option<&str>, stereotype: Option<&str>) -> Selector {
    Selector::key(element, stereotype)
}

type Rules = HashMap<Selector, Props>;

// ---- defaults.rb ----

/// The look PlantUML gives an unstyled diagram.
pub fn defaults_rules(name: &str) -> Option<&'static Props> {
    static CELL: OnceLock<HashMap<&'static str, Props>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut any = Props::default();
        any.font_name = Some("sans-serif".into());
        any.font_size = Some(14.0);
        any.font_colour = Some("#000000".into());
        any.background = Some("#F1F1F1".into());
        any.border = Some("#181818".into());
        any.line_thickness = Some(0.5);
        let mut document = Props::default();
        document.background = Some("#FFFFFF".into());
        let mut state = Props::default();
        state.background = Some("#F1F1F1".into());
        state.border = Some("#181818".into());
        state.font_colour = Some("#000000".into());
        state.line_thickness = Some(0.5);
        state.round_corner = Some(12.5);
        let mut class = state.clone();
        class.round_corner = Some(2.5);
        let mut activity = state.clone();
        activity.font_size = Some(12.0);
        let mut note = Props::default();
        note.background = Some("#FEFFDD".into());
        note.border = Some("#181818".into());
        note.line_thickness = Some(0.5);
        let mut arrow = Props::default();
        arrow.border = Some("#181818".into());
        arrow.font_colour = Some("#000000".into());
        arrow.line_thickness = Some(1.0);
        arrow.line_thickness_int = true;
        let mut start = Props::default();
        start.background = Some("#222222".into());
        start.border = Some("#222222".into());
        HashMap::from([
            ("any", any),
            ("document", document),
            ("state", state),
            ("class", class),
            ("activity", activity),
            ("note", note),
            ("arrow", arrow),
            ("start", start),
        ])
    })
    .get(name)
}

/// `Style::Defaults::BOX`, which the renderer compares its background to.
pub fn defaults_box_background() -> &'static str {
    "#F1F1F1"
}

// ---- sheet.rb ----

/// What a diagram's source says about its look: defaults, then
/// `skinparam`, then `<style>`, then the colours written on one element.
#[derive(Clone, Default)]
pub struct Sheet {
    skin: Rules,
    style: Rules,
    elements: HashMap<String, Props>,
}

impl Sheet {
    pub fn new(skin: Rules, style: Rules, elements: HashMap<String, Props>) -> Sheet {
        Sheet { skin, style, elements }
    }

    /// The winning value of a property, or None when none is set.
    pub fn resolve(
        &self,
        element: &str,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Option<Value> {
        self.explicit(element, property, id, stereotype)
            .or_else(|| self.from_defaults(element, property))
    }

    /// The winning value the source itself sets, ignoring the defaults.
    pub fn explicit(
        &self,
        element: &str,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Option<Value> {
        self.candidates(element, property, id, stereotype)
            .into_iter()
            .flatten()
            .next_back()
    }

    /// The properties written on one element.
    pub fn element(&self, id: &str) -> Props {
        self.elements.get(id).cloned().unwrap_or_default()
    }

    /// Whether any layer above the defaults is set.
    pub fn styled(&self) -> bool {
        !self.skin.is_empty() || !self.style.is_empty() || !self.elements.is_empty()
    }

    fn candidates(
        &self,
        element: &str,
        property: Prop,
        id: Option<&str>,
        stereotype: Option<&str>,
    ) -> Vec<Option<Value>> {
        let mut out = vec![];
        for layer in [&self.skin, &self.style] {
            for selector in selectors(element, stereotype) {
                out.push(layer.get(&selector).and_then(|props| props.get(property)));
            }
        }
        out.push(
            id.and_then(|id| self.elements.get(id))
                .and_then(|props| props.get(property)),
        );
        out
    }

    fn from_defaults(&self, element: &str, property: Prop) -> Option<Value> {
        defaults_rules(element)
            .and_then(|props| props.get(property))
            .or_else(|| defaults_rules("any").and_then(|props| props.get(property)))
    }
}

fn selectors(element: &str, stereotype: Option<&str>) -> Vec<Selector> {
    let mut out = vec![selector(Some("any"), None), selector(Some(element), None)];
    if let Some(st) = stereotype {
        out.push(selector(Some(element), Some(st)));
        out.push(selector(None, Some(st)));
    }
    out
}

// ---- reader.rb ----

fn re(pattern: &str) -> &'static Regex {
    static CELL: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CELL.get_or_init(|| {
        HashMap::from([
            ("skinparam", Regex::new(r"(?i)\Askinparam[ \t]+(\S+?)(?:[ \t]*(\{)|[ \t]+(.+))?\z").unwrap()),
            ("single", Regex::new(r"\A(\w+)(?:<<(\w+)>>)?\z").unwrap()),
            ("style_open", Regex::new(r"(?i)\A<style>\z").unwrap()),
            ("style_close", Regex::new(r"(?i)\A</style>\z").unwrap()),
            ("opening", Regex::new(r"\A([\w.]+(?:[ \t]*,[ \t]*[\w.]+)*)[ \t]*\{\z").unwrap()),
            ("property", Regex::new(r"\A(\w+)(?:<<(\w+)>>)?[ \t:]+(.+?)[ \t;]*\z").unwrap()),
            ("inline", Regex::new(r"\A([\w.]+)[ \t]*\{[ \t]*(\w+)[ \t:]+(.+?)[ \t;]*\}\z").unwrap()),
            ("declaration", Regex::new(concat!(
                r#"(?i)\A(?:abstract[ \t]+class|static[ \t]+class|abstract|"#,
                r#"class|interface|enum|annotation|state)[ \t]+"#,
                r#"(?:"[^"]+"[ \t]+as[ \t]+)?([A-Za-z_][\w.]*)"#,
                r#"(?:<[^<>]+>)?(?:[ \t]*<<([^<>]+)>>)*"#,
                r#"(?:[ \t]*(\#\S+))?"#,
            )).unwrap()),
        ])
    })
    .get(pattern)
    .expect("style reader pattern")
}

/// The properties a rule name carries (Ruby `PROPERTIES`), None for a
/// name the reader leaves out.
fn property_of(name: &str) -> Option<Prop> {
    match name {
        "backgroundcolor" => Some(Prop::Background),
        "bordercolor" | "linecolor" | "color" => Some(Prop::Border),
        "fontcolor" => Some(Prop::FontColour),
        "fontname" => Some(Prop::FontName),
        "fontsize" => Some(Prop::FontSize),
        "linestyle" => Some(Prop::LineStyle),
        "roundcorner" => Some(Prop::RoundCorner),
        "linethickness" | "borderthickness" | "thickness" => Some(Prop::LineThickness),
        _ => None,
    }
}

/// A selector that names the whole diagram styles its own element.
fn wrapper(name: &str) -> Option<&'static str> {
    match name {
        "document" | "root" => Some("document"),
        "element" => Some("any"),
        "classdiagram" => Some("class"),
        "statediagram" => Some("state"),
        "activitydiagram" => Some("activity"),
        _ => None,
    }
}

/// The suffix a combined `skinparam` name ends with, longest first.
fn suffix_of(name: &str) -> Option<&'static str> {
    let mut suffixes = [
        "backgroundcolor", "bordercolor", "linecolor", "fontcolor",
        "fontname", "fontsize", "linestyle", "roundcorner", "linethickness",
        "borderthickness", "thickness", "color",
    ];
    suffixes.sort_by_key(|w| std::cmp::Reverse(w.len()));
    suffixes.iter().copied().find(|word| name.ends_with(word))
}

/// The default-carrying `skinparam` names.
fn default_of(name: &str) -> Option<Prop> {
    match name {
        "defaultfontname" => Some(Prop::FontName),
        "defaultfontsize" => Some(Prop::FontSize),
        "defaultfontcolor" => Some(Prop::FontColour),
        _ => None,
    }
}

/// Reads the `skinparam` lines and `<style>` blocks of a source into a
/// [`Sheet`]. A name or property it does not know is left out.
pub fn read(source: &str) -> Sheet {
    let mut reader = StyleReader::default();
    for text in style_lines(source) {
        reader.feed(&text);
    }
    Sheet::new(reader.skin, reader.style, reader.elements)
}

/// The stripped, non-empty, non-comment lines of the whole source.
fn style_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('\''))
        .collect()
}

#[derive(Default)]
struct StyleReader {
    skin: Rules,
    style: Rules,
    block: Option<(Option<String>, Option<String>)>,
    path: Vec<String>,
    in_style: bool,
    elements: HashMap<String, Props>,
}

impl StyleReader {
    fn feed(&mut self, text: &str) {
        if self.in_style {
            return self.style_line(text);
        }
        if self.block.is_some() {
            return self.skin_block_line(text);
        }
        if re("style_open").is_match(text) {
            self.in_style = true;
            self.path.clear();
            return;
        }
        self.statement(text);
    }

    fn statement(&mut self, text: &str) {
        if let Some(m) = re("skinparam").captures(text) {
            self.skinparam(&m);
        } else if let Some(m) = re("declaration").captures(text) {
            self.declaration(&m);
        }
    }

    fn declaration(&mut self, m: &regex::Captures) {
        let mut written = inline_parse(m.get(3).map(|g| g.as_str()).unwrap_or(""));
        if let Some(stereo) = m.get(2) {
            written.stereotype = Some(stereo.as_str().to_lowercase());
        }
        if written != Props::default() || written.stereotype.is_some() {
            self.elements.insert(m[1].to_string(), written);
        }
    }

    fn skinparam(&mut self, m: &regex::Captures) {
        if m.get(2).is_some() {
            let (name, stereotype) = single(m.get(1).map(|g| g.as_str()).unwrap_or(""));
            self.block = Some((name, stereotype));
            return;
        }
        let Some(value) = m.get(3) else { return };
        let Some(single) = re("single").captures(&m[1]) else { return };
        let name = single[1].to_lowercase();
        let stereotype = single.get(2).map(|g| g.as_str().to_string());
        let (element, property) = split_name(&name);
        let Some(property) = property else { return };
        Self::store(
            &mut self.skin,
            element.as_deref(),
            property,
            stereotype.as_deref(),
            value.as_str(),
        );
    }

    fn skin_block_line(&mut self, text: &str) {
        if text == "}" {
            self.block = None;
            return;
        }
        let Some(m) = re("property").captures(text) else { return };
        let Some((element, stereotype)) = self.block.clone() else { return };
        let mut property = m[1].to_lowercase();
        let mut element: Option<String> = element;
        let mut stereotype: Option<String> = stereotype;
        if property.starts_with("arrow") {
            element = Some("arrow".to_string());
            property = property
                .strip_prefix("arrow")
                .unwrap_or(&property)
                .to_string();
        }
        let stereo = m.get(2).map(|g| g.as_str().to_string()).or(stereotype);
        Self::store(
            &mut self.skin,
            element.as_deref(),
            &property,
            stereo.as_deref(),
            m.get(3).map(|g| g.as_str()).unwrap_or(""),
        );
    }

    fn style_line(&mut self, text: &str) {
        if re("style_close").is_match(text) {
            self.in_style = false;
            return;
        }
        if let Some(m) = re("inline").captures(text) {
            self.path.push(m[1].to_lowercase());
            self.style_property(&m[2].to_lowercase(), &m[3]);
            self.path.pop();
        } else if let Some(m) = re("opening").captures(text) {
            self.path.push(m[1].to_lowercase());
        } else if text == "}" {
            self.path.pop();
        } else if let Some(m) = re("property").captures(text) {
            self.style_property(&m[1].to_lowercase(), &m.get(3).map(|g| g.as_str()).unwrap_or(""));
        }
    }

    fn style_property(&mut self, property: &str, value: &str) {
        let Some(selector) = self.path.last().cloned() else { return };
        // PlantUML's <style> has no BorderColor; only skinparam does.
        if property == "bordercolor" {
            return;
        }
        let (element, stereotype) = target_of(&selector);
        Self::store(
            &mut self.style,
            element.as_deref(),
            property,
            stereotype.as_deref(),
            value,
        );
    }

    fn store(
        rules: &mut Rules,
        element: Option<&str>,
        property: &str,
        stereotype: Option<&str>,
        value: &str,
    ) {
        let Some(name) = property_of(property) else { return };
        if element.is_none() && stereotype.is_none() {
            return;
        }
        let Some(parsed) = parse_value(name, value) else { return };
        let canvas = match (element, name) {
            (Some("any"), Prop::Background) => "document",
            _ => element.unwrap_or(""),
        };
        let key = selector(
            if canvas.is_empty() { None } else { Some(canvas) },
            stereotype,
        );
        rules.entry(key).or_default().set(name, parsed);
    }
}

fn single(written: &str) -> (Option<String>, Option<String>) {
    match re("single").captures(written) {
        Some(m) => (
            Some(m[1].to_lowercase()),
            m.get(2).map(|g| g.as_str().to_string()),
        ),
        None => (None, None),
    }
}

/// `split(name)`: `[element, property]` for a combined skinparam name.
fn split_name(name: &str) -> (Option<String>, Option<&'static str>) {
    if let Some(prop) = default_of(name) {
        let prop = match prop {
            Prop::FontName => "fontname",
            Prop::FontSize => "fontsize",
            Prop::FontColour => "fontcolor",
            _ => unreachable!(),
        };
        return (Some("any".to_string()), Some(prop));
    }
    let Some(suffix) = suffix_of(name) else {
        return (None, None);
    };
    let element = name.trim_end_matches(suffix);
    let element = if element.is_empty() { "any" } else { element };
    (Some(element.to_string()), Some(suffix))
}

fn target_of(selector: &str) -> (Option<String>, Option<String>) {
    if let Some(rest) = selector.strip_prefix('.') {
        return (None, Some(rest.to_string()));
    }
    (Some(wrapper(selector).unwrap_or(selector).to_string()), None)
}

fn parse_value(name: Prop, value: &str) -> Option<Value> {
    match name {
        Prop::Background | Prop::Border | Prop::FontColour => {
            colour_resolve(value).map(Value::Str)
        }
        Prop::FontSize | Prop::RoundCorner | Prop::LineThickness => {
            value.trim().parse::<f64>().ok().map(Value::Num)
        }
        Prop::FontName => Some(Value::Str(value.replace('"', ""))),
        Prop::LineStyle => line_styles(value.to_ascii_lowercase().as_str())
            .map(str::to_string)
            .map(Value::Str),
    }
}
