//! Port of `Etassia::Notation::Mermaid::TYPES` patterns, in detection order.
//! Names are the corpus directory names.

use regex::Regex;
use std::sync::OnceLock;

const TYPES: &[(&str, &str)] = &[
    ("flowchart", r"\A(?-u:\s)*(?:flowchart-elk|(?i:graph|flowchart))(?:[ \t\n\r\x0b\x0c<>^]|\z)"),
    ("sequence", r"(?i)\A(?-u:\s)*sequenceDiagram"),
    ("class_diagram", r"(?i)\A(?-u:\s)*classDiagram"),
    ("state_diagram", r"(?i)\A(?-u:\s)*stateDiagram"),
    ("er_diagram", r"(?i)\A(?-u:\s)*erDiagram"),
    ("user_journey", r"(?i)\A(?-u:\s)*journey"),
    ("gantt", r"(?i)\A(?-u:\s)*gantt(?:(?-u:\s)|\z)"),
    ("pie", r"(?i)\A(?-u:\s)*pie(?:(?-u:\s)|\z)"),
    ("timeline", r"(?i)\A(?-u:\s)*timeline(?:(?-u:\s)|(?m:$))"),
    ("quadrant", r"(?i)\A(?-u:\s)*quadrantChart"),
    ("gitgraph", r"(?i)\A(?-u:\s)*gitGraph"),
    ("mindmap", r"(?i)\A(?-u:\s)*mindmap"),
    ("kanban", r"(?i)\A(?-u:\s)*kanban"),
    ("radar", r"(?i)\A(?-u:\s)*radar-beta"),
    ("block", r"(?i)\A(?-u:\s)*block-beta"),
    ("requirement", r"(?i)\A(?-u:\s)*requirementDiagram"),
    ("xychart", r"(?i)\A(?-u:\s)*xychart-beta"),
    ("architecture", r"(?i)\A(?-u:\s)*architecture-beta"),
    ("sankey", r"(?i)\A(?-u:\s)*sankey-beta"),
    ("packet", r"(?i)\A(?-u:\s)*packet-beta"),
    ("treemap", r"(?i)\A(?-u:\s)*treemap"),
    (
        "c4",
        r"(?i)\A(?-u:\s)*(?:C4Context|C4Container|C4Component|C4Dynamic|C4Deployment|C4(?-u:\s)+diagram)",
    ),
    ("info", r"(?i)\A(?-u:\s)*info"),
    ("error", r"(?i)\A(?-u:\s)*(?:error)"),
];

fn table() -> &'static Vec<(&'static str, Regex)> {
    static CELL: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    CELL.get_or_init(|| {
        TYPES
            .iter()
            .map(|(name, pattern)| (*name, Regex::new(pattern).expect("detect pattern")))
            .collect()
    })
}

/// The PlantUML wrapper lane: a source opening (after a BOM and blank or
/// quote-comment lines) with @startgit claims the git type.
fn git_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"\A(?:ï»¿)?(?:[ 	]*(?:'[^
]*)?(?:
|
|
))*[ 	]*@startgit").expect("git detect pattern")
    })
}

/// Whether the raw source opens with the git wrapper.
pub fn detect_git(source: &str) -> bool {
    git_re().is_match(source)
}

/// The PlantUML wrapper lane for ditaa: a source opening (after a BOM and
/// blank or quote-comment lines) with @startditaa claims the ditaa type.
fn ditaa_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"\A(?:ï»¿)?(?:[ 	]*(?:'[^
]*)?(?:
|
|
))*[ 	]*@startditaa").expect("ditaa detect pattern")
    })
}

/// Whether the raw source opens with the ditaa wrapper.
pub fn detect_ditaa(source: &str) -> bool {
    ditaa_re().is_match(source)
}

/// The PlantUML wrapper lane for the Tree notation's mindmap: a source
/// opening (after a BOM and blank or quote-comment lines) with @startmindmap
/// claims the mindmap type. Run after the Mermaid body patterns, the same
/// order the Ruby resolution runs - the Mermaid sources claim themselves
/// first, the tagged PlantUML kinds before the families.
fn mindmap_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(r"\A(?:\x{FEFF})?(?:[ \t\x0b\x0c\x00]*(?:'[^\r]*)?(?:\r\n|\r|\n))*[ \t\x0b\x0c\x00]*@startmindmap").expect("mindmap detect pattern")
    })
}

/// Whether the raw source opens with the mindmap wrapper.
pub fn detect_mindmap(source: &str) -> bool {
    mindmap_re().is_match(source)
}

/// The PlantUML wrapper lane for gantt: a source opening (after a BOM and
/// blank or quote-comment lines) with @startgantt claims the gantt type.
/// The Mermaid body patterns run first (a mermaid gantt is a mermaid
/// kind); this checks the raw source, before the component family lanes,
/// whose `[Task]` markers would otherwise steal a gantt source.
fn gantt_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(concat!(
            r"\A(?:\x{FEFF})?",
            r"(?:[ \t\x0B\x0C\x00]*(?:'[^\r\n]*)?(?:\r\n|\r|\n))*",
            r"[ \t\x0B\x0C\x00]*@startgantt",
        ))
        .expect("gantt detect pattern")
    })
}

/// Whether the raw source opens with the gantt wrapper.
pub fn detect_gantt(source: &str) -> bool {
    gantt_re().is_match(source)
}

/// The timing lane, first of the plain-picture families (the Ruby
/// Further group runs network, timing and ebnf ahead of State, Usecase
/// and Component): a participant declaration. Ruby `Timing.timing?`.
pub fn detect_timing(source: &str) -> bool {
    let entries = significant_lines(source);
    let kinds = ["robust", "concise", "clock", "binary", "analog"];
    entries.iter().any(|text| {
        let lowered = text.to_lowercase();
        kinds.iter().any(|kind| {
            lowered.starts_with(kind)
                && text.as_bytes().get(kind.len()) == Some(&b' ')
                && starts_declaration(&text[kind.len() + 1..])
        })
    })
}

/// `("title" as )?Identifier` — the identifier may start anywhere the
/// blank run ends, exactly the Ruby declaration's tail.
fn starts_declaration(text: &str) -> bool {
    let mut rest = text.trim_start_matches([' ', '\t']);
    if let Some(after) = rest.strip_prefix('"') {
        let Some(close) = after.find('"') else { return false };
        rest = after[close + 1..].trim_start_matches([' ', '\t']);
        let Some(tail) = rest.strip_prefix("as") else { return false };
        rest = tail.strip_prefix([' ', '\t']).unwrap_or(tail);
    }
    rest.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

/// The use case lane, tried before component (the Ruby families run
/// Further, State, Usecase, Component in that order): no foreign keyword,
/// and at least one use case marker. Ruby `Usecase.usecase?`.
pub fn detect_usecase(source: &str) -> bool {
    let entries = significant_lines(source);
    let foreign = [
        "participant", "class", "abstract", "enum", "boundary", "control",
        "entity", "collections", "queue", "autonumber", "newpage", "alt",
        "loop", "opt", "par", "critical", "break", "ref", "fork",
        "partition", "repeat", "while", "switch",
    ];
    let plain_foreign = ["state", "start", "stop"];
    for text in &entries {
        if text.ends_with(';') || text.starts_with("(*") {
            return false;
        }
        let first_word = text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find(|word| !word.is_empty())
            .unwrap_or("");
        let lowered = first_word.to_ascii_lowercase();
        let word_boundary = text[first_word.len()..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        let is_foreign = foreign.contains(&lowered.as_str())
            || (plain_foreign.contains(&lowered.as_str())
                && text.len() == first_word.len());
        if is_foreign && word_boundary {
            return false;
        }
    }
    entries.iter().any(|text| usecase_marker(text))
}

/// `(A, B)` is a class diagram's association class, never a use case.
fn usecase_marker(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if lowered.starts_with("usecase")
        && text.as_bytes().get("usecase".len()) == Some(&b' ')
    {
        return true;
    }
    if lowered.starts_with(':') {
        // `:Actor:` — no semicolon inside, closed by a colon then space/end.
        let inner = &text[1..];
        if let Some(close) = inner.find(':') {
            let body = &inner[..close];
            if !body.contains(';') && !body.contains(':') {
                let after = inner[close + 1..].chars().next();
                if after.is_none() || after == Some(' ') {
                    return true;
                }
            }
        }
        return false;
    }
    if text.starts_with('(') {
        // A pair `(A, B)` is not a marker; any other `(name)` is.
        if let Some(close) = text.find(')') {
            let inner = &text[1..close];
            if !inner.contains('(') && !inner.contains('*') {
                let words: Vec<&str> =
                    inner.split(',').map(str::trim).collect();
                let is_pair = words.len() == 2
                    && words.iter().all(|w| {
                        !w.is_empty()
                            && w.chars().all(|c| c.is_alphanumeric() || c == '_')
                    });
                if is_pair {
                    return false;
                }
                let after = text[close + 1..].chars().next();
                if after.is_none() || after == Some(' ') {
                    return true;
                }
            }
        }
    }
    false
}

/// The component lane: the source's significant lines (the first
/// diagram, comments dropped) carry none of the foreign keywords and at
/// least one component marker. Ruby `Component.component?`.
pub fn detect_component(source: &str) -> bool {
    let entries = significant_lines(source);
    let foreign = [
        "participant", "class", "abstract", "enum", "boundary", "control",
        "entity", "collections", "queue", "autonumber", "newpage", "alt",
        "loop", "opt", "par", "critical", "break", "ref", "fork",
        "partition", "repeat", "while", "switch",
    ];
    let plain_foreign = ["state", "start", "stop"];
    for text in &entries {
        if text.ends_with(';') || text.starts_with("(*") {
            return false;
        }
        let first_word = text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find(|word| !word.is_empty())
            .unwrap_or("");
        let lowered = first_word.to_ascii_lowercase();
        let word_boundary = text[first_word.len()..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        let is_foreign = foreign.contains(&lowered.as_str())
            || (plain_foreign.contains(&lowered.as_str())
                && text.len() == first_word.len());
        if is_foreign && word_boundary {
            return false;
        }
    }
    entries.iter().any(|text| component_marker(text))
}

fn component_marker(text: &str) -> bool {
    if text.starts_with('[') {
        return text.contains(']');
    }
    let lowered = text.to_lowercase();
    if lowered.starts_with("component ")
        || lowered.starts_with("portin ")
        || lowered.starts_with("portout ")
        || lowered.starts_with("() ")
    {
        return true;
    }
    ["node", "cloud", "file", "folder", "frame", "storage", "artifact", "card", "rectangle", "archimate"]
        .iter()
        .any(|keyword| {
            lowered.starts_with(keyword)
                && lowered.as_bytes().get(keyword.len()) == Some(&b' ')
        })
}

/// Ruby `Reader.entries`: the first diagram's stripped significant lines.
fn significant_lines(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let first = lines.iter().position(|line| line.trim().starts_with("@start"));
    let body: Vec<&str> = match first {
        Some(at) => lines[at + 1..].iter().copied().collect(),
        None => lines.clone(),
    };
    body
        .iter()
        .map(|line| line.trim())
        .take_while(|line| !line.starts_with("@end"))
        .filter(|line| !line.is_empty() && !line.starts_with('\''))
        .map(str::to_string)
        .collect()
}

/// First type whose pattern matches the (preamble-free) body.
pub fn detect_body(body: &str) -> Option<&'static str> {
    table().iter().find(|(_, re)| re.is_match(body)).map(|(name, _)| *name)
}
