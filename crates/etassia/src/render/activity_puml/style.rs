//! Port of `notation/plantuml/style.rb` and its `style/` modules as the
//! activity renderer reads them: `Style::Reader` turns `skinparam` lines
//! and `<style>` blocks into a `Sheet`, and a renderer or the metrics ask
//! the sheet for a property. The element-declaration lane (`Style::Inline`,
//! reached from a `class`/`state` declaration line) is out of scope: the
//! activity readers refuse those lines, so an activity source never
//! carries element rules.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

/// The colour names PlantUML knows, resolved case-insensitively (the
/// exact `style/colour_names.rb` table).
fn colour_names() -> &'static HashMap<&'static str, &'static str> {
    static CELL: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        [
            ("aliceblue", "#F0F8FF"),
            ("antiquewhite", "#FAEBD7"),
            ("aqua", "#00FFFF"),
            ("aquamarine", "#7FFFD4"),
            ("azure", "#F0FFFF"),
            ("beige", "#F5F5DC"),
            ("bisque", "#FFE4C4"),
            ("black", "#000000"),
            ("blanchedalmond", "#FFEBCD"),
            ("blue", "#0000FF"),
            ("blueviolet", "#8A2BE2"),
            ("brown", "#A52A2A"),
            ("burlywood", "#DEB887"),
            ("cadetblue", "#5F9EA0"),
            ("chartreuse", "#7FFF00"),
            ("chocolate", "#D2691E"),
            ("coral", "#FF7F50"),
            ("cornflowerblue", "#6495ED"),
            ("cornsilk", "#FFF8DC"),
            ("crimson", "#DC143C"),
            ("cyan", "#00FFFF"),
            ("darkblue", "#00008B"),
            ("darkcyan", "#008B8B"),
            ("darkgoldenrod", "#B8860B"),
            ("darkgray", "#A9A9A9"),
            ("darkgreen", "#006400"),
            ("darkgrey", "#A9A9A9"),
            ("darkkhaki", "#BDB76B"),
            ("darkmagenta", "#8B008B"),
            ("darkolivegreen", "#556B2F"),
            ("darkorange", "#FF8C00"),
            ("darkorchid", "#9932CC"),
            ("darkred", "#8B0000"),
            ("darksalmon", "#E9967A"),
            ("darkseagreen", "#8FBC8F"),
            ("darkslateblue", "#483D8B"),
            ("darkslategray", "#2F4F4F"),
            ("darkslategrey", "#2F4F4F"),
            ("darkturquoise", "#00CED1"),
            ("darkviolet", "#9400D3"),
            ("deeppink", "#FF1493"),
            ("deepskyblue", "#00BFFF"),
            ("dimgray", "#696969"),
            ("dimgrey", "#696969"),
            ("dodgerblue", "#1E90FF"),
            ("firebrick", "#B22222"),
            ("floralwhite", "#FFFAF0"),
            ("forestgreen", "#228B22"),
            ("fuchsia", "#FF00FF"),
            ("gainsboro", "#DCDCDC"),
            ("ghostwhite", "#F8F8FF"),
            ("gold", "#FFD700"),
            ("goldenrod", "#DAA520"),
            ("gray", "#808080"),
            ("green", "#008000"),
            ("greenyellow", "#ADFF2F"),
            ("grey", "#808080"),
            ("honeydew", "#F0FFF0"),
            ("hotpink", "#FF69B4"),
            ("indianred", "#CD5C5C"),
            ("indigo", "#4B0082"),
            ("ivory", "#FFFFF0"),
            ("khaki", "#F0E68C"),
            ("lavender", "#E6E6FA"),
            ("lavenderblush", "#FFF0F5"),
            ("lawngreen", "#7CFC00"),
            ("lemonchiffon", "#FFFACD"),
            ("lightblue", "#ADD8E6"),
            ("lightcoral", "#F08080"),
            ("lightcyan", "#E0FFFF"),
            ("lightgoldenrodyellow", "#FAFAD2"),
            ("lightgray", "#D3D3D3"),
            ("lightgreen", "#90EE90"),
            ("lightgrey", "#D3D3D3"),
            ("lightpink", "#FFB6C1"),
            ("lightsalmon", "#FFA07A"),
            ("lightseagreen", "#20B2AA"),
            ("lightskyblue", "#87CEFA"),
            ("lightslategray", "#778899"),
            ("lightslategrey", "#778899"),
            ("lightsteelblue", "#B0C4DE"),
            ("lightyellow", "#FFFFE0"),
            ("lime", "#00FF00"),
            ("limegreen", "#32CD32"),
            ("linen", "#FAF0E6"),
            ("magenta", "#FF00FF"),
            ("maroon", "#800000"),
            ("mediumaquamarine", "#66CDAA"),
            ("mediumblue", "#0000CD"),
            ("mediumorchid", "#BA55D3"),
            ("mediumpurple", "#9370D8"),
            ("mediumseagreen", "#3CB371"),
            ("mediumslateblue", "#7B68EE"),
            ("mediumspringgreen", "#00FA9A"),
            ("mediumturquoise", "#48D1CC"),
            ("mediumvioletred", "#C71585"),
            ("midnightblue", "#191970"),
            ("mintcream", "#F5FFFA"),
            ("mistyrose", "#FFE4E1"),
            ("moccasin", "#FFE4B5"),
            ("navajowhite", "#FFDEAD"),
            ("navy", "#000080"),
            ("oldlace", "#FDF5E6"),
            ("olive", "#808000"),
            ("olivedrab", "#6B8E23"),
            ("orange", "#FFA500"),
            ("orangered", "#FF4500"),
            ("orchid", "#DA70D6"),
            ("palegoldenrod", "#EEE8AA"),
            ("palegreen", "#98FB98"),
            ("paleturquoise", "#AFEEEE"),
            ("palevioletred", "#D87093"),
            ("papayawhip", "#FFEFD5"),
            ("peachpuff", "#FFDAB9"),
            ("peru", "#CD853F"),
            ("pink", "#FFC0CB"),
            ("plum", "#DDA0DD"),
            ("powderblue", "#B0E0E6"),
            ("purple", "#800080"),
            ("red", "#FF0000"),
            ("rosybrown", "#BC8F8F"),
            ("royalblue", "#4169E1"),
            ("saddlebrown", "#8B4513"),
            ("salmon", "#FA8072"),
            ("sandybrown", "#F4A460"),
            ("seagreen", "#2E8B57"),
            ("seashell", "#FFF5EE"),
            ("sienna", "#A0522D"),
            ("silver", "#C0C0C0"),
            ("skyblue", "#87CEEB"),
            ("slateblue", "#6A5ACD"),
            ("slategray", "#708090"),
            ("slategrey", "#708090"),
            ("snow", "#FFFAFA"),
            ("springgreen", "#00FF7F"),
            ("steelblue", "#4682B4"),
            ("tan", "#D2B48C"),
            ("teal", "#008080"),
            ("thistle", "#D8BFD8"),
            ("tomato", "#FF6347"),
            ("turquoise", "#40E0D0"),
            ("violet", "#EE82EE"),
            ("wheat", "#F5DEB3"),
            ("white", "#FFFFFF"),
            ("whitesmoke", "#F5F5F5"),
            ("yellow", "#FFFF00"),
            ("yellowgreen", "#9ACD32"),
        ]
        .into_iter()
        .collect()
    })
}

/// `Style::Colour.resolve`: the `#RRGGBB` the text names, `"none"` for
/// `transparent`, or None; a gradient reads as its first colour.
pub fn colour_resolve(text: &str) -> Option<String> {
    static GRADIENT: OnceLock<Regex> = OnceLock::new();
    let gradient = GRADIENT.get_or_init(|| Regex::new(r"[|\\/-]").expect("gradient pattern"));
    let head = gradient.split(text.trim()).next().unwrap_or("");
    let word = head.strip_prefix('#').unwrap_or(head);
    if word.is_empty() {
        return None;
    }
    if word.eq_ignore_ascii_case("transparent") {
        return Some("none".to_string());
    }
    let hex = |digits: &str| digits.chars().all(|c| c.is_ascii_hexdigit());
    if (word.len() == 3 || word.len() == 6) && hex(word) {
        let digits = if word.len() == 3 {
            word.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            word.to_string()
        };
        return Some(format!("#{}", digits.to_uppercase()));
    }
    colour_names()
        .get(&word.to_lowercase().as_str())
        .map(|c| c.to_string())
}

/// A written property value: the text of a colour or face, or a number.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Num(f64),
}

/// The selector a rule is stored under: `any`, the element, or
/// `element<<stereotype>>` / `.stereotype` (`Style.selector`).
fn selector(element: Option<&str>, stereotype: Option<&str>) -> String {
    match (element, stereotype) {
        (Some(element), Some(stereotype)) => {
            format!("{element}<<{}>>", stereotype.to_lowercase())
        }
        (Some(element), None) => element.to_string(),
        (None, Some(stereotype)) => format!(".{}", stereotype.to_lowercase()),
        (None, None) => String::new(),
    }
}

/// What a diagram's source says about its look, in the order PlantUML
/// applies it: the `skin` and `style` layers (`style/sheet.rb`). The
/// element rules the other families carry live outside this port.
#[derive(Default, Clone)]
pub struct Sheet {
    skin: HashMap<String, HashMap<String, Value>>,
    style: HashMap<String, HashMap<String, Value>>,
}

impl Sheet {
    fn rule(
        &self,
        layer: &HashMap<String, HashMap<String, Value>>,
        element: &str,
        property: &str,
        stereotype: Option<&str>,
    ) -> Option<Value> {
        // `compact.last` over the selector list: the element's own rule
        // wins over the global one.
        for selector in [
            selector(Some(element), stereotype),
            selector(Some("any"), None),
        ] {
            if let Some(found) = layer.get(&selector).and_then(|rules| rules.get(property)) {
                return Some(found.clone());
            }
        }
        None
    }

    /// `Sheet#explicit`: the winning written value, None when the source
    /// sets nothing. A stereotype rule sits above the element rule; the
    /// activity marks carry none, so `stereotype` stays None here.
    pub fn explicit(&self, element: &str, property: &str) -> Option<Value> {
        // `compact.last` over the layers: the `<style>` layer wins over
        // the skinparam one.
        self.rule(&self.style, element, property, None)
            .or_else(|| self.rule(&self.skin, element, property, None))
    }

    /// `Sheet#from_style`: the value a `<style>` rule sets, ignoring
    /// skinparam, for what PlantUML cascades from the root only.
    pub fn from_style(&self, element: &str, property: &str) -> Option<Value> {
        self.rule(&self.style, element, property, None)
    }

    fn store(
        &mut self,
        layer: &str,
        element: Option<&str>,
        property: &str,
        stereotype: Option<&str>,
        value: Value,
    ) {
        // `store` keeps only properties with an element or a stereotype.
        if element.is_none() && stereotype.is_none() {
            return;
        }
        // A bare `BackgroundColor` is the canvas, not every element.
        let element = match (element, property) {
            (Some("any"), "background") => Some("document"),
            (element, _) => element,
        };
        let key = selector(element, stereotype);
        let rules = match layer {
            "skin" => &mut self.skin,
            _ => &mut self.style,
        };
        rules
            .entry(key)
            .or_default()
            .insert(property.to_string(), value);
    }
}

/// The property names the reader knows, mapped to their symbols.
fn property_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "backgroundcolor" => "background",
        "bordercolor" | "linecolor" => "border",
        "fontcolor" => "font_colour",
        "fontname" => "font_name",
        "fontsize" => "font_size",
        "linestyle" => "line_style",
        "roundcorner" => "round_corner",
        "linethickness" | "borderthickness" | "thickness" => "line_thickness",
        "color" => "border",
        "fontcolour" => "font_colour",
        "background" => "background",
        "border" => "border",
        _ => return None,
    })
}

/// The `default…` skinparams that name the whole diagram (`Reader::DEFAULTS`).
fn default_property(name: &str) -> Option<&'static str> {
    match name {
        "defaultfontname" => Some("font_name"),
        "defaultfontsize" => Some("font_size"),
        "defaultfontcolor" => Some("font_colour"),
        _ => None,
    }
}

/// The property suffixes, longest first, ties in declaration order.
fn suffixes() -> &'static Vec<&'static str> {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut names: Vec<&str> = vec![
            "backgroundcolor",
            "bordercolor",
            "linecolor",
            "fontcolor",
            "fontname",
            "fontsize",
            "linestyle",
            "roundcorner",
            "linethickness",
            "borderthickness",
            "thickness",
            "color",
        ];
        names.sort_by_key(|name| std::cmp::Reverse(name.len()));
        names
    })
}

/// The `<style>` selectors that name the whole diagram or another family
/// (`Reader::WRAPPERS`); any other selector names itself.
fn wrapper(selector: &str) -> &'static str {
    match selector {
        "document" | "root" => "document",
        "element" => "any",
        "classdiagram" => "class",
        "statediagram" => "state",
        "activitydiagram" => "activity",
        _ => "",
    }
}

fn regexes() -> &'static HashMap<&'static str, Regex> {
    static CELL: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CELL.get_or_init(|| {
        let entries: &[(&str, &str)] = &[
            (
                "skinparam",
                r"(?i)\Askinparam[ \t]+(\S+?)(?:[ \t]*(\{)|[ \t]+(.+))?\z",
            ),
            ("single", r"\A(\w+)(?:<<(\w+)>>)?\z"),
            ("style_open", r"(?i)\A<style>\z"),
            ("style_close", r"(?i)\A</style>\z"),
            ("opening", r"\A([\w.]+(?:[ \t]*,[ \t]*[\w.]+)*)[ \t]*\{\z"),
            ("property", r"\A(\w+)(?:<<(\w+)>>)?[ \t:]+(.+?)[ \t;]*\z"),
            (
                "inline",
                r"\A([\w.]+)[ \t]*\{[ \t]*(\w+)[ \t:]+(.+?)[ \t;]*\}\z",
            ),
        ];
        entries
            .iter()
            .map(|(name, pattern)| (*name, Regex::new(pattern).expect("style reader pattern")))
            .collect()
    })
}

fn re(name: &str) -> &'static Regex {
    regexes().get(name).expect("style reader pattern")
}

/// The line styles a `linestyle` property or a colour line names.
fn line_style_of(word: &str) -> Option<Value> {
    match word.to_lowercase().as_str() {
        "dotted" => Some(Value::Text("1,3".into())),
        "dashed" => Some(Value::Text("7,7".into())),
        "bold" | "plain" => None,
        _ => None,
    }
}

/// Parses one written value into what the sheet stores (`Reader#parse`).
fn parse_value(name: &str, value: &str) -> Option<Value> {
    match name {
        "background" | "border" | "font_colour" => colour_resolve(value).map(Value::Text),
        "font_size" | "round_corner" | "line_thickness" => {
            value.parse::<f64>().ok().map(Value::Num)
        }
        "font_name" => Some(Value::Text(value.trim_matches('"').to_string())),
        "line_style" => line_style_of(value),
        _ => None,
    }
}

/// Reads the `skinparam` lines and `<style>` blocks of a source into a
/// [`Sheet`] (`Style::Reader`). A name or property it does not know is
/// left out, never an error.
pub struct StyleReader {
    sheet: Sheet,
    block: Option<(Option<String>, Option<String>)>,
    /// The `<style>` path; None outside the block, an empty vector
    /// between `<style>` and the first selector (`@path = nil` / `[]`).
    path: Option<Vec<String>>,
}

impl StyleReader {
    pub fn new() -> Self {
        StyleReader {
            sheet: Sheet::default(),
            block: None,
            path: None,
        }
    }

    /// `Style::Reader.read`.
    pub fn read(mut self, source: &str) -> Sheet {
        for text in Self::lines(source) {
            self.feed(&text);
        }
        self.sheet
    }

    /// The stripped, non-empty, non-comment lines.
    fn lines(source: &str) -> Vec<String> {
        source
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('\''))
            .collect()
    }

    /// `Style::Reader.styling?`: for each stripped line, whether it
    /// belongs to a `skinparam` or `<style>` rule, which this reader owns.
    pub fn styling(texts: &[String]) -> Vec<bool> {
        let mut closing: Option<Regex> = None;
        texts
            .iter()
            .map(|text| {
                if let Some(closer) = &closing {
                    if closer.is_match(text) {
                        closing = None;
                    }
                    return true;
                }
                closing = Self::closer(text);
                closing.is_some() || re("skinparam").is_match(text)
            })
            .collect()
    }

    /// What ends the block this line opens.
    fn closer(text: &str) -> Option<Regex> {
        if re("style_open").is_match(text) {
            return Some(re("style_close").clone());
        }
        let found = re("skinparam").captures(text)?;
        if found.get(2).is_some() {
            return Some(Regex::new(r"\A\}\z").expect("brace pattern"));
        }
        None
    }

    fn feed(&mut self, text: &str) {
        if self.path.is_some() {
            return self.style_line(text);
        }
        if self.block.is_some() {
            return self.skin_block_line(text);
        }
        if re("style_open").is_match(text) {
            self.path = Some(vec![]);
            return;
        }
        self.statement(text);
    }

    fn statement(&mut self, text: &str) {
        if let Some(found) = re("skinparam").captures(text) {
            self.skinparam(&found);
            return;
        }
        // The declaration lane (`Reader#declaration`) reads class, state
        // and interface lines with their inline colours; the activity
        // readers refuse those lines, so nothing reaches it here.
    }

    fn skinparam(&mut self, found: &regex::Captures<'_>) {
        match (found.get(2), found.get(3)) {
            (Some(_), _) => {
                let written = &found[1];
                let single = re("single").captures(written);
                let name = single
                    .as_ref()
                    .and_then(|m| m.get(1))
                    .map(|m| m.as_str().to_lowercase());
                let stereotype = single
                    .and_then(|m| m.get(2))
                    .map(|m| m.as_str().to_string());
                self.block = Some((name, stereotype));
            }
            (None, Some(value)) => {
                let single = re("single").captures(&found[1]);
                let Some(single) = single else { return };
                let name = single.get(1).map(|m| m.as_str().to_lowercase());
                let Some(name) = name else { return };
                let stereotype = single.get(2).map(|m| m.as_str().to_string());
                let (element, property) = self.split(&name);
                if let (Some(element), Some(property)) = (element, property) {
                    if let Some(value) = parse_value(property, value.as_str()) {
                        self.sheet.store(
                            "skin",
                            Some(&element),
                            property,
                            stereotype.as_deref(),
                            value,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// `Reader#split`: the element a skinparam name carries and its
    /// property, `default…` naming the whole diagram.
    fn split(&self, name: &str) -> (Option<String>, Option<&'static str>) {
        if let Some(property) = default_property(name) {
            return (Some("any".to_string()), Some(property));
        }
        let Some(suffix) = suffixes().iter().find(|word| name.ends_with(**word)) else {
            return (None, None);
        };
        let element = &name[..name.len() - suffix.len()];
        let property = property_of(suffix);
        (
            if element.is_empty() {
                Some("any".to_string())
            } else {
                Some(element.to_string())
            },
            property,
        )
    }

    fn skin_block_line(&mut self, text: &str) {
        if text == "}" {
            self.block = None;
            return;
        }
        let Some(found) = re("property").captures(text) else {
            return;
        };
        let Some((element, stereotype)) = self.block.clone() else {
            return;
        };
        // `arrow…` names the arrow element, the rest of the name its
        // property (`Reader#block_property`).
        let mut property = found[1].to_lowercase();
        let mut element = element;
        if property.starts_with("arrow") {
            element = Some("arrow".to_string());
            property = property["arrow".len()..].to_string();
        }
        let stereotype = found.get(2).map(|m| m.as_str().to_string()).or(stereotype);
        let Some(property) = property_of(&property) else {
            return;
        };
        if let Some(value) = parse_value(property, &found[3]) {
            self.sheet.store(
                "skin",
                element.as_deref(),
                property,
                stereotype.as_deref(),
                value,
            );
        }
    }

    fn style_line(&mut self, text: &str) {
        if re("style_close").is_match(text) {
            self.path = None;
            return;
        }
        let mut path = std::mem::take(&mut self.path).unwrap_or_default();
        if let Some(found) = re("inline").captures(text) {
            path.push(found[1].to_lowercase());
            self.path = Some(path.clone());
            let property = found[2].to_lowercase();
            let value = found[3].to_string();
            self.style_property(&property, &value);
            path.pop();
        } else if let Some(found) = re("opening").captures(text) {
            path.push(found[1].to_lowercase());
        } else if text == "}" {
            path.pop();
        } else if let Some(found) = re("property").captures(text) {
            self.path = Some(path.clone());
            let property = found[1].to_lowercase();
            let value = found[3].to_string();
            self.style_property(&property, &value);
        }
        self.path = Some(path);
    }

    fn style_property(&mut self, property: &str, value: &str) {
        let Some(selector) = self.path.as_ref().and_then(|path| path.last()).cloned() else {
            return;
        };
        let selector: &str = &selector;
        // PlantUML's `<style>` has no BorderColor; only skinparam does.
        if property == "bordercolor" {
            return;
        }
        let Some(property) = property_of(property) else {
            return;
        };
        let (element, stereotype) = Self::target_of(selector);
        if let Some(value) = parse_value(property, value) {
            self.sheet.store(
                "style",
                element.as_deref(),
                property,
                stereotype.as_deref(),
                value,
            );
        } else {
            eprintln!("DBG parse_value none");
        }
    }

    /// `Reader#target_of`: a wrapper selector names an element, a
    /// `.dotted` one a stereotype, anything else itself.
    fn target_of(selector: &str) -> (Option<String>, Option<String>) {
        if let Some(stereotype) = selector.strip_prefix('.') {
            return (None, Some(stereotype.to_string()));
        }
        let mapped = wrapper(selector);
        let element = if mapped.is_empty() { selector } else { mapped };
        (Some(element.to_string()), None)
    }
}

impl Default for StyleReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_style_block_reaches_arrow() {
        let sheet = StyleReader::new().read(
            "<style>\nactivityDiagram {\n  arrow {\n    LineColor green\n    FontColor blue\n  }\n}\n</style>\n",
        );
        assert_eq!(
            sheet.explicit("arrow", "border"),
            Some(Value::Text("#008000".to_string()))
        );
        assert_eq!(
            sheet.explicit("arrow", "font_colour"),
            Some(Value::Text("#0000FF".to_string()))
        );
    }

    #[test]
    fn skinparam_names_the_document_canvas() {
        let sheet = StyleReader::new().read("skinparam backgroundColor white\n");
        assert_eq!(sheet.from_style("activity", "background"), None);
    }

    #[test]
    fn colour_gradients_read_their_first_colour() {
        assert_eq!(colour_resolve("#red|blue").as_deref(), Some("#FF0000"));
        assert_eq!(colour_resolve("transparent").as_deref(), Some("none"));
        assert_eq!(colour_resolve("#f80").as_deref(), Some("#FF8800"));
    }
}
