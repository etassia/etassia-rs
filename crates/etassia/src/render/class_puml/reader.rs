//! The class reader's state machine: port of Ruby
//! `Notation::PlantUML::Parser` (a line phase machine) feeding Ruby
//! `Notation::PlantUML::DiagramBuilder`.
use super::arrow;
use super::model::*;
use super::reader_support::*;
use super::sheet::{BlockReader, SheetFromBlock};
use crate::Error;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

const NAME: &str = r"[A-Za-z_][A-Za-z0-9_$]*";
const STEREOTYPE: &str = r"<<[^<>]+>>";
const END_TEXT: &str = r#"(?:"([^"]+)"(?:/"([^"]+)")?|/"([^"]+)"|\[[^\]]+\])"#;

struct Patterns {
    class_declaration: Regex,
    package: Regex,
    quoted_declaration: Regex,
    hide_tag: Regex,
    note: Regex,
    free_note: Regex,
    member_of: Regex,
    last_note: Regex,
    end_note: Regex,
    junction: Regex,
    plain_text: Regex,
    modifier: Regex,
    method: Regex,
    field: Regex,
    typed_field: Regex,
    typed_method: Regex,
    name_at: Regex,
}

fn patterns() -> &'static Patterns {
    static CELL: OnceLock<Patterns> = OnceLock::new();
    CELL.get_or_init(|| Patterns {
        class_declaration: Regex::new(&format!(
            r#"\A(abstract[ \t]+class|static[ \t]+class|abstract|class|interface|enum|annotation)[ \t]+({})(?:<([^<>]+)>)?((?:[ \t]*{})*)((?:[ \t]+\${})*)(?:[ \t]+#\S+)?(?:[ \t]*(\{{))?\z"#,
            NAME, STEREOTYPE, NAME
        ))
        .unwrap(),
        package: Regex::new(&format!(
            r#"(?i)\A(\+)?(package|rectangle)[ \t]+(?:"([^"]+)"(?:[ \t]+as[ \t]+({}))?|({}(?:\.{})*))((?:[ \t]*<<[^<>]+>>)*)?(?:[ \t]+(#[A-Za-z0-9]+))?[ \t]*\{{\z"#,
            NAME, NAME, NAME
        ))
        .unwrap(),
        quoted_declaration: Regex::new(
            r#"\A(abstract[ \t]+class|static[ \t]+class|class|interface)[ \t]+"([^"]+)"\z"#,
        )
        .unwrap(),
        hide_tag: Regex::new(&format!(r#"(?i)\Ahide[ \t]+\$({})\z"#, NAME)).unwrap(),
        note: Regex::new(&format!(
            r#"(?i)\Anote[ \t]+(left|right|top|bottom)[ \t]+of[ \t]+({})(?:::({}|"[^"]+"))?(?:[ \t]+(#[A-Za-z0-9]+))?(?:[ \t]*({}))?(?:[ \t]*:[ \t]*(.+))?\z"#,
            NAME, NAME, STEREOTYPE
        ))
        .unwrap(),
        free_note: Regex::new(&format!(r#"(?i)\Anote[ \t]+as[ \t]+({})\z"#, NAME))
            .unwrap(),
        member_of: Regex::new(&format!(r#"\A({})[ \t]*:[ \t]*(\S.*)\z"#, NAME))
            .unwrap(),
        last_note: Regex::new(
            r"(?i)\Anote[ \t]+(left|right|top|bottom)(?:[ \t]*:[ \t]*(.+))?\z",
        )
        .unwrap(),
        end_note: Regex::new(r"(?i)\Aend[ \t]?note\z").unwrap(),
        junction: Regex::new(&format!(
            r#"\A\(({})[ \t]*,[ \t]*({})\)[ \t]*\.{{1,2}}[ \t]*({})\z"#,
            NAME, NAME, NAME
        ))
        .unwrap(),
        plain_text: Regex::new(r#"\A[\p{Alphabetic}\p{Nd}\p{M}\[][^<>{}"!$():@;]*\z"#).unwrap(),
        modifier: Regex::new(r"(?i)\{(?:static|abstract|field|method)\}").unwrap(),
        method: Regex::new(&format!(
            r#"\A([+\-#~])?[ \t]*({})[ \t]*\(([^()]*)\)(?:([ \t]*:[ \t]*)(.+))?\z"#,
            NAME
        ))
        .unwrap(),
        field: Regex::new(&format!(
            r#"\A([+\-#~])?[ \t]*({})(?:([ \t]*:[ \t]*)([^()]+))?\z"#,
            NAME
        ))
        .unwrap(),
        typed_field: Regex::new(&format!(
            r#"\A([+\-#~])?[ \t]*({}(?:\[\])*)[ \t]+({})\z"#,
            NAME, NAME
        ))
        .unwrap(),
        typed_method: Regex::new(&format!(
            r#"\A([+\-#~])?[ \t]*({}(?:\[\])*)[ \t]+({})[ \t]*\(([^()]*)\)\z"#,
            NAME, NAME
        ))
        .unwrap(),
        name_at: Regex::new(&format!(r"\A{}", NAME)).unwrap(),
    })
}

/// Ruby `Parser#parse`: the diagram, or a refusal.
pub fn read(source: &str) -> Result<Diagram, Error> {
    let mut builder = Builder::new();
    let mut phase = Phase::Before;
    for (index, line) in lines_of(source).iter().enumerate() {
        phase = step(phase, &mut builder, line.trim(), index + 1)?;
    }
    finish(phase, builder)
}

/// Ruby `lines_of`: invalid bytes are refused (a Rust `&str` is always
/// valid UTF-8), the BOM goes, the line ends split.
fn lines_of(source: &str) -> Vec<&str> {
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    split_lines(stripped)
}

fn split_lines(text: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(position) = rest.find(['\r', '\n']) {
        let (line, after) = rest.split_at(position);
        out.push(line);
        rest = after
            .strip_prefix("\r\n")
            .or_else(|| after.strip_prefix('\n'))
            .or_else(|| after.strip_prefix('\r'))
            .unwrap_or(after);
    }
    out.push(rest);
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Before,
    Statements,
    Body,
    Note,
    Style,
    Skin,
    After,
}

fn step(
    phase: Phase,
    builder: &mut Builder,
    text: &str,
    number: usize,
) -> Result<Phase, Error> {
    let _ = number;
    if text.is_empty() {
        return Ok(phase);
    }
    refuse_continuation(phase, text)?;
    if text.starts_with('\'') && !matches!(phase, Phase::Note | Phase::Style) {
        return Ok(phase);
    }
    if phase == Phase::Skin {
        return Ok(if text == "}" { Phase::Statements } else { Phase::Skin });
    }
    read_line(phase, builder, text)
}

fn refuse_continuation(phase: Phase, text: &str) -> Result<(), Error> {
    if text.ends_with('\\')
        && matches!(phase, Phase::Statements | Phase::Body | Phase::Note | Phase::Style)
    {
        return Err(refused());
    }
    Ok(())
}

fn read_line(phase: Phase, builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    match phase {
        Phase::Before => before(text),
        Phase::Statements => statement(builder, text),
        Phase::Body => body_line(builder, text),
        Phase::Note => note_line(builder, text),
        Phase::Style => style_line(builder, text),
        _ => after(text),
    }
}

fn before(text: &str) -> Result<Phase, Error> {
    // A name after @startuml only names PlantUML's output file.
    if is_startuml(text) {
        return Ok(Phase::Statements);
    }
    Err(refused())
}

fn statement(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    if text == "@enduml" {
        return end_of_diagram(builder);
    }
    if is_startuml(text) {
        return Err(refused());
    }
    if text.contains("/'") {
        return Err(refused());
    }
    // The block's lines are read by Style::Reader, not here.
    static SKIN_BLOCK: OnceLock<Regex> = OnceLock::new();
    let skin_block =
        SKIN_BLOCK.get_or_init(|| Regex::new(r"(?i)\Askinparam[ \t]+\S+[ \t]*\{\z").unwrap());
    if skin_block.is_match(text) {
        return Ok(Phase::Skin);
    }
    if directive_matches(text) {
        builder.directive(text);
        return Ok(Phase::Statements);
    }
    line_statement(builder, text)
}

fn line_statement(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    if let Some(caption) = caption_read(text) {
        builder.caption(caption);
        return Ok(Phase::Statements);
    }
    static STYLE_OPEN: OnceLock<Regex> = OnceLock::new();
    let style_open =
        STYLE_OPEN.get_or_init(|| Regex::new(r"(?i)\A<style>\z").unwrap());
    if style_open.is_match(text) {
        builder.open_style()?;
        return Ok(Phase::Style);
    }
    if let Some(phase) = block_line(builder, text)? {
        return Ok(phase);
    }
    if let Some(phase) = quoted_declaration(builder, text) {
        return Ok(phase);
    }
    declaration_or_relation(builder, text)
}

/// Ruby `block_line`: a `hide $tag`, a package open or a package close;
/// `None` when the line is none of those.
fn block_line(builder: &mut Builder, text: &str) -> Result<Option<Phase>, Error> {
    let patterns = patterns();
    if patterns.hide_tag.is_match(text) {
        let tag = patterns.hide_tag.captures(text).and_then(|m| m.get(1)).map(|g| g.as_str()).unwrap_or("");
        builder.hide_tag(tag);
        return Ok(Some(Phase::Statements));
    }
    if let Some(m) = patterns.package.captures(text) {
        builder.open_package(package_of(&m, text)?)?;
        return Ok(Some(Phase::Statements));
    }
    if text == "}" && builder.package_open() {
        builder.close_package()?;
        return Ok(Some(Phase::Statements));
    }
    Ok(None)
}

fn package_of(m: &regex::Captures, _text: &str) -> Result<Package, Error> {
    let group = |index: usize| m.get(index).map(|g| g.as_str());
    let id = group(4).or(group(5)).or(group(3)).unwrap_or("").to_string();
    let title = group(3)
        .map(str::to_string)
        .or_else(|| group(5).map(|dotted| dotted.rsplit('.').next().unwrap_or(dotted).to_string()))
        .unwrap_or_default();
    let shape = if group(2).map(|word| word.eq_ignore_ascii_case("rectangle")).unwrap_or(false)
        || group(6)
            .map(|st| {
                st.to_lowercase().replace(' ', "").contains("<<frame>>")
            })
            .unwrap_or(false)
    {
        PackageShape::Frame
    } else {
        PackageShape::Folder
    };
    let color = match group(7) {
        Some(written) => Some(package_color(written).ok_or_else(refused)?),
        None => None,
    };
    Ok(Package {
        id,
        title,
        shape,
        icon: group(1).is_some(),
        parent: None,
        color,
    })
}

fn quoted_declaration(builder: &mut Builder, text: &str) -> Option<Phase> {
    let patterns = patterns();
    let m = patterns.quoted_declaration.captures(text)?;
    let name = m.get(2).map(|g| g.as_str()).unwrap_or("");
    if class_name::escapes(name) {
        return None; // refusal surfaces through declaration_or_relation's fallback
    }
    let kind = Kind::of_keyword(m.get(1).map(|g| g.as_str()).unwrap_or(""))?;
    let entry = DeclEntry {
        name: name.to_string(),
        kind,
        body: false,
        generics: None,
        stereotypes: vec![],
        tags: vec![],
    };
    builder.declare(entry, text).ok()?;
    Some(Phase::Statements)
}

fn declaration_or_relation(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    let patterns = patterns();
    if let Some(m) = patterns.class_declaration.captures(text) {
        return declare(builder, &m, text);
    }
    if let Some(m) = patterns.junction.captures(text) {
        let group = |index: usize| m.get(index).map(|g| g.as_str()).unwrap_or("");
        builder.junction(Junction {
            from: group(1).to_string(),
            to: group(2).to_string(),
            owner: group(3).to_string(),
        });
        return Ok(Phase::Statements);
    }
    if let Some(m) = patterns.note.captures(text) {
        return note(builder, &m, text);
    }
    last_note_or_relation(builder, text)
}

fn declare(builder: &mut Builder, m: &regex::Captures, text: &str) -> Result<Phase, Error> {
    let group = |index: usize| m.get(index).map(|g| g.as_str());
    let kind = Kind::of_keyword(group(1).unwrap_or("")).ok_or_else(refused)?;
    let entry = DeclEntry {
        name: group(2).unwrap_or("").to_string(),
        kind,
        body: group(6).is_some(),
        generics: group(3).map(str::to_string),
        stereotypes: stereotypes_of(group(4).unwrap_or("")),
        tags: group(5)
            .unwrap_or("")
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
    };
    let body = entry.body;
    builder.declare(entry, text)?;
    if body {
        Ok(Phase::Body)
    } else {
        Ok(Phase::Statements)
    }
}

fn stereotypes_of(text: &str) -> Vec<String> {
    static STEREO: OnceLock<Regex> = OnceLock::new();
    let stereo = STEREO.get_or_init(|| Regex::new(STEREOTYPE).unwrap());
    stereo
        .find_iter(text)
        .map(|m| m.as_str()[2..m.as_str().len() - 2].to_string())
        .collect()
}

/// Ruby `last_note_or_relation`.
fn last_note_or_relation(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    let patterns = patterns();
    if patterns.free_note.is_match(text) {
        builder.open_note(
            NoteHead {
                side: NoteSide::Free,
                target: None,
                member: None,
                color: None,
                stereotype: None,
            },
            vec![],
        )?;
        return Ok(Phase::Note);
    }
    if let Some(m) = patterns.last_note.captures(text) {
        let side = note_side(m.get(1).map(|g| g.as_str()).unwrap_or(""));
        let Some(target) = builder.last_defined() else { return Err(refused()) };
        let inline = m
            .get(2)
            .map(|g| plain_note_text(g.as_str()));
        let has_inline = inline.is_some();
        builder.open_note(
            NoteHead {
                side,
                target: Some(target),
                member: None,
                color: None,
                stereotype: None,
            },
            inline.into_iter().collect(),
        )?;
        if has_inline {
            builder.close_note();
            return Ok(Phase::Statements);
        }
        return Ok(Phase::Note);
    }
    member_statement(builder, text)?.map_or_else(
        || relation(builder, text),
        |phase| Ok(phase),
    )
}

fn note_side(word: &str) -> NoteSide {
    match word.to_lowercase().as_str() {
        "left" => NoteSide::Left,
        "right" => NoteSide::Right,
        "top" => NoteSide::Top,
        _ => NoteSide::Bottom,
    }
}

/// Ruby `member_statement`: `Name : text`.
fn member_statement(builder: &mut Builder, text: &str) -> Result<Option<Phase>, Error> {
    let patterns = patterns();
    let m = match patterns.member_of.captures(text) {
        Some(m) => m,
        None => return Ok(None),
    };
    if relation_matches(text) {
        return Ok(None);
    }
    let Some(member) = member_from(m.get(2).map(|g| g.as_str()).unwrap_or("")) else {
        return Err(refused());
    };
    builder.add_member_to(m.get(1).map(|g| g.as_str()).unwrap_or(""), member)?;
    Ok(Some(Phase::Statements))
}

fn note(builder: &mut Builder, m: &regex::Captures, _text: &str) -> Result<Phase, Error> {
    let group = |index: usize| m.get(index).map(|g| g.as_str());
    let inline = group(6).map(|line| plain_note_text(line));
    let head = NoteHead {
        side: note_side(group(1).unwrap_or("")),
        target: group(2).map(str::to_string),
        member: group(3).map(str::to_string),
        color: group(4).map(str::to_string),
        stereotype: group(5).map(|st| st[2..st.len() - 2].to_string()),
    };
    if inline.is_some() && head.member.is_some() {
        return Err(refused());
    }
    builder.open_note(head, inline.clone().into_iter().collect())?;
    if inline.is_none() {
        return Ok(Phase::Note);
    }
    builder.close_note();
    Ok(Phase::Statements)
}

fn note_line(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    let patterns = patterns();
    if patterns.end_note.is_match(text) {
        builder.close_note();
        return Ok(Phase::Statements);
    }
    if text == "@enduml" || text.starts_with('\'') || text.contains("/'") {
        return Err(refused());
    }
    builder.add_note_line(plain_note_text(text));
    Ok(Phase::Note)
}

fn style_line(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    if text == "@enduml" {
        return Err(refused());
    }
    let Some(reader) = builder.style.as_mut() else { return Err(refused()) };
    match reader.feed(text) {
        Ok(()) => Ok(Phase::Style),
        Err(()) => {
            builder.close_style();
            Ok(Phase::Statements)
        }
    }
}

fn end_of_diagram(builder: &mut Builder) -> Result<Phase, Error> {
    if builder.empty() {
        return Err(refused());
    }
    if builder.open_package_line().is_some() {
        return Err(refused());
    }
    Ok(Phase::After)
}

fn body_line(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    if text == "}" {
        return Ok(Phase::Statements);
    }
    if text == "@enduml" || is_startuml(text) || text.contains("/'") {
        return Err(refused());
    }
    let Some(member) = member_from(text) else { return Err(refused()) };
    builder.add_member(member);
    Ok(Phase::Body)
}

fn after(_text: &str) -> Result<Phase, Error> {
    Err(refused())
}

fn finish(phase: Phase, builder: Builder) -> Result<Diagram, Error> {
    match phase {
        Phase::After => builder.diagram(),
        _ => Err(refused()),
    }
}

/// Ruby `relation_from`: the whole relation line, scanned in the regex's
/// own order.
fn relation(builder: &mut Builder, text: &str) -> Result<Phase, Error> {
    let Some(found) = scan_relation(text) else { return Err(refused()) };
    let Some(parsed) = arrow::parse(&found.glyph) else {
        return Err(refused());
    };
    let unquote = |name: &str| name.trim_matches('"').to_string();
    let relation = Relation {
        left: unquote(&found.left),
        right: unquote(&found.right),
        kind: parsed.kind,
        head_is_left: parsed.head_is_left,
        both: parsed.both,
        markers: parsed.markers,
        left_multiplicity: found.left_text,
        right_multiplicity: found.right_text,
        label: found.label,
        left_role: found.left_role,
        right_role: found.right_role,
        direction: parsed.direction,
        length: parsed.length,
        color: parsed.color,
        width: parsed.width,
        dashed: parsed.dashed,
        plain: parsed.plain,
        dotted: parsed.dotted,
        layout_only: false,
    };
    builder.relate(relation, text)?;
    Ok(Phase::Statements)
}

#[derive(Debug)]
struct RelationScan {
    left: String,
    left_text: Option<String>,
    left_role: Option<String>,
    glyph: String,
    right_text: Option<String>,
    right_role: Option<String>,
    right: String,
    label: Option<String>,
}

/// Ruby `RELATION = /\A(NAME|QUOTED)[ \t]+(?:END_TEXT[ \t]+)?PATTERN
/// [ \t]*(?:END_TEXT[ \t]+)?(NAME|QUOTED)(?:[ \t]*:[ \t]*(.+))?\z/`,
/// scanned by hand (the arrow pattern holds the one lookahead). The two
/// optional end texts fall away right to left, and inside them the
/// arrow's own greedy choices replay the regex's backtracking.
fn scan_relation(text: &str) -> Option<RelationScan> {
    let patterns = patterns();
    let (left, mut pos) = scan_name(text, 0, &patterns.name_at)?;
    let blanks = blank_run(text, pos);
    if blanks == 0 {
        return None;
    }
    pos += blanks;
    for (left_try, right_try) in [(true, true), (true, false), (false, true), (false, false)]
    {
        for (with_direction, with_right) in
            [(true, true), (true, false), (false, true), (false, false)]
        {
            let mut at = pos;
            let mut left_text = None;
            let mut left_role = None;
            if left_try {
                let Some((found, after)) = scan_end_text(text, at) else { continue };
                if blank_run(text, after) == 0 {
                    continue;
                }
                left_text = found.0;
                left_role = found.1;
                at = after + blank_run(text, after);
            }
            let Some(glyph) = arrow::scan(text, at, with_direction, with_right) else {
                continue;
            };
            at = glyph.end;
            at += blank_run(text, at);
            let mut right_text = None;
            let mut right_role = None;
            if right_try {
                let Some((found, after)) = scan_end_text(text, at) else { continue };
                if blank_run(text, after) == 0 {
                    continue;
                }
                right_text = found.0;
                right_role = found.1;
                at = after + blank_run(text, after);
            }
            let Some((right, after)) = scan_name(text, at, &patterns.name_at) else {
                continue;
            };
            at = after;
            // (?:[ \t]*:[ \t]*(.+))?\z
            let mut cursor = at + blank_run(text, at);
            let mut label = None;
            if text[cursor..].starts_with(':') {
                cursor += 1;
                cursor += blank_run(text, cursor);
                label = Some(text[cursor..].to_string());
                cursor = text.len();
            }
            if cursor != text.len() {
                continue;
            }
            return Some(RelationScan {
                left,
                left_text,
                left_role,
                glyph: glyph.text,
                right_text,
                right_role,
                right,
                label,
            });
        }
    }
    None
}

/// Ruby `RELATION.match?` for `member_statement`'s guard.
pub(crate) fn relation_matches(text: &str) -> bool {
    scan_relation(text).is_some()
}

fn scan_name(text: &str, at: usize, name_at: &Regex) -> Option<(String, usize)> {
    let rest = &text[at..];
    if let Some(m) = name_at.find(rest) {
        return Some((m.as_str().to_string(), at + m.end()));
    }
    static QUOTED: OnceLock<Regex> = OnceLock::new();
    let quoted = QUOTED.get_or_init(|| Regex::new(r#"\A"[^"]+""#).unwrap());
    let m = quoted.find(rest)?;
    Some((m.as_str().to_string(), at + m.end()))
}

fn blank_run(text: &str, at: usize) -> usize {
    text[at..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| c.len_utf8())
        .sum()
}

/// Ruby `END_TEXT`: `(quoted (/role)? | /role | [bracket])`, and the
/// bracket text is the whole `[...]` including brackets.
type EndText = (Option<String>, Option<String>);

fn scan_end_text(text: &str, at: usize) -> Option<(EndText, usize)> {
    static QUOTED: OnceLock<Regex> = OnceLock::new();
    let quoted = QUOTED.get_or_init(|| {
        Regex::new(r#"\A"([^"]+)"(?:/"([^"]+)")?"#).unwrap()
    });
    static ROLE: OnceLock<Regex> = OnceLock::new();
    let role = ROLE.get_or_init(|| Regex::new(r#"\A/"([^"]+)""#).unwrap());
    static BRACKET: OnceLock<Regex> = OnceLock::new();
    let bracket = BRACKET.get_or_init(|| Regex::new(r"\A(\[[^\]]+\])").unwrap());
    let rest = &text[at..];
    if let Some(m) = quoted.captures(rest) {
        let whole = m.get(0).map(|g| g.end()).unwrap_or(0);
        return Some((
            (
                m.get(1).map(|g| g.as_str().to_string()),
                m.get(2).map(|g| g.as_str().to_string()),
            ),
            at + whole,
        ));
    }
    if let Some(m) = role.captures(rest) {
        let whole = m.get(0).map(|g| g.end()).unwrap_or(0);
        return Some((
            (
                None,
                m.get(1).map(|g| g.as_str().to_string()),
            ),
            at + whole,
        ));
    }
    if let Some(m) = bracket.captures(rest) {
        let whole = m.get(0).map(|g| g.end()).unwrap_or(0);
        let inner = m.get(1).map(|g| g.as_str()).unwrap_or("");
        return Some(((Some(inner.to_string()), None), at + whole));
    }
    None
}

/// Ruby `SEPARATOR = /\A([-=._])\1(?:[ \t]*(\S.*?)[ \t]*\1\1)?\z/`,
/// by hand (the regex crate has no backreferences). Returns the character
/// and the title, as the reader uses them.
fn separator_match(text: &str) -> Option<(char, Option<String>)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if !matches!(first, '-' | '=' | '.' | '_') {
        return None;
    }
    if chars.next() != Some(first) {
        return None;
    }
    let rest = &text[first.len_utf8() * 2..];
    if rest.is_empty() {
        return Some((first, None));
    }
    // The optional group must run to the end: spaces, a non-space title,
    // spaces, then two of the character.
    let bytes = rest.as_bytes();
    let mut p = 0;
    while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
        p += 1;
    }
    if p >= bytes.len() {
        return None;
    }
    // The lazy title: the shortest run whose tail is spaces plus the pair.
    let mut k = 1;
    while p + k <= bytes.len() {
        let after = &rest[p + k..];
        let trimmed = after.trim_end_matches(|c| c == ' ' || c == '\t');
        if trimmed.len() >= 2
            && trimmed.ends_with(&first.to_string().repeat(2))
            && trimmed[trimmed.len() - first.len_utf8() * 2..]
                .chars()
                .all(|c| c == first)
        {
            return Some((first, Some(rest[p..p + k].to_string())));
        }
        k += 1;
    }
    None
}

/// Ruby `member_from`: a separator first, then the modifiers lifted out
/// of the line wherever they sit.
pub(crate) fn member_from(text: &str) -> Option<Member> {
    if let Some((character, title)) = separator_match(text) {
        return Some(Member::separator(&title.unwrap_or_default(), &character.to_string()));
    }
    if member_construct(text) {
        return None;
    }
    modified_member(text)
}

fn modified_member(text: &str) -> Option<Member> {
    let patterns = patterns();
    let words: Vec<String> = patterns
        .modifier
        .find_iter(text)
        .map(|m| m.as_str().trim_matches(|c| c == '{' || c == '}').to_lowercase())
        .collect();
    let stripped = patterns.modifier.replace_all(text, " ").to_string();
    let rest = squeeze_spaces(stripped.trim());
    let member = named_member(&rest)
        .or_else(|| typed_member(&rest))
        .or_else(|| text_member(&rest));
    if words.is_empty() {
        return member;
    }
    let member = member?;
    // Ruby `lifted`: a `{field}` on a method-shaped line refuses.
    let has = |word: &str| words.iter().any(|own| own == word);
    if has("field") && (has("method") || member.parameters.is_some()) {
        return None;
    }
    Some(retype(&member, &words.iter().map(String::as_str).collect::<Vec<_>>()))
}

fn retype(member: &Member, words: &[&str]) -> Member {
    let mut out = member.clone();
    out.modifiers = ["abstract", "static"]
        .iter()
        .filter(|word| words.contains(word))
        .copied()
        .collect();
    if words.contains(&"method") {
        out.kind = MemberKind::Method;
    }
    out
}

fn named_member(text: &str) -> Option<Member> {
    let patterns = patterns();
    let visibility = |m: &regex::Captures| {
        m.get(1).and_then(|g| Visibility::of(g.as_str()))
    };
    if let Some(m) = patterns.method.captures(text) {
        return Some(Member {
            kind: MemberKind::Method,
            visibility: visibility(&m),
            name: m.get(2).map(|g| g.as_str()).unwrap_or("").to_string(),
            kind_type: m.get(5).map(|g| g.as_str().to_string()),
            parameters: m.get(3).map(|g| g.as_str().to_string()),
            modifiers: vec![],
            type_position: TypePosition::Suffix,
            type_separator: m
                .get(4)
                .map(|g| g.as_str().to_string())
                .unwrap_or_else(|| ": ".to_string()),
        });
    }
    if let Some(m) = patterns.field.captures(text) {
        return Some(Member {
            kind: MemberKind::Field,
            visibility: visibility(&m),
            name: m.get(2).map(|g| g.as_str()).unwrap_or("").to_string(),
            kind_type: m.get(4).map(|g| g.as_str().to_string()),
            parameters: None,
            modifiers: vec![],
            type_position: TypePosition::Suffix,
            type_separator: m
                .get(3)
                .map(|g| g.as_str().to_string())
                .unwrap_or_else(|| ": ".to_string()),
        });
    }
    None
}

/// Ruby `typed_member`: `int x` and `void run(int a)`, the type first.
fn typed_member(text: &str) -> Option<Member> {
    let patterns = patterns();
    let visibility = |m: &regex::Captures| {
        m.get(1).and_then(|g| Visibility::of(g.as_str()))
    };
    if let Some(m) = patterns.typed_method.captures(text) {
        return Some(Member {
            kind: MemberKind::Method,
            visibility: visibility(&m),
            name: m.get(3).map(|g| g.as_str()).unwrap_or("").to_string(),
            kind_type: m.get(2).map(|g| g.as_str().to_string()),
            parameters: m.get(4).map(|g| g.as_str().to_string()),
            modifiers: vec![],
            type_position: TypePosition::Prefix,
            type_separator: ": ".to_string(),
        });
    }
    if let Some(m) = patterns.typed_field.captures(text) {
        return Some(Member {
            kind: MemberKind::Field,
            visibility: visibility(&m),
            name: m.get(3).map(|g| g.as_str()).unwrap_or("").to_string(),
            kind_type: m.get(2).map(|g| g.as_str().to_string()),
            parameters: None,
            modifiers: vec![],
            type_position: TypePosition::Prefix,
            type_separator: ": ".to_string(),
        });
    }
    None
}

/// Ruby `text_member`: any other plain line of a body, shown as written.
fn text_member(text: &str) -> Option<Member> {
    let patterns = patterns();
    if !patterns.plain_text.is_match(text) {
        return None;
    }
    Some(Member {
        kind: MemberKind::Field,
        visibility: None,
        name: text.to_string(),
        kind_type: None,
        parameters: None,
        modifiers: vec![],
        type_position: TypePosition::Suffix,
        type_separator: ": ".to_string(),
    })
}

// ---- the builder (Ruby DiagramBuilder) ----

#[derive(Clone, Debug)]
struct DeclEntry {
    name: String,
    kind: Kind,
    body: bool,
    generics: Option<String>,
    stereotypes: Vec<String>,
    tags: Vec<String>,
}

#[derive(Clone, Debug)]
struct NoteHead {
    side: NoteSide,
    target: Option<String>,
    member: Option<String>,
    color: Option<String>,
    stereotype: Option<String>,
}

struct Builder {
    kinds: Vec<(String, Kind)>,
    bodies: HashMap<String, Vec<Member>>,
    explicit: Vec<String>,
    extras: HashMap<String, Extra>,
    captions: HashMap<CaptionKind, Caption>,
    relations: Vec<Relation>,
    junctions: Vec<Junction>,
    directives: Vec<String>,
    notes: Vec<Note>,
    hidden_tags: Vec<String>,
    packages: Vec<Package>,
    scopes: Vec<Package>,
    style: Option<BlockReader>,
    style_sheet: Option<SheetFromBlock>,
    open: Option<String>,
    note: Option<(NoteHead, Vec<String>)>,
    class_evidence: bool,
    sequence_arrow: bool,
}

#[derive(Clone, Debug, Default)]
struct Extra {
    generics: Option<String>,
    stereotypes: Vec<String>,
    tags: Vec<String>,
    package: Option<String>,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            kinds: vec![],
            bodies: HashMap::new(),
            explicit: vec![],
            extras: HashMap::new(),
            captions: HashMap::new(),
            relations: vec![],
            junctions: vec![],
            directives: vec![],
            notes: vec![],
            hidden_tags: vec![],
            packages: vec![],
            scopes: vec![],
            style: None,
            style_sheet: None,
            open: None,
            note: None,
            class_evidence: false,
            sequence_arrow: false,
        }
    }

    fn known(&self, name: &str) -> bool {
        self.kinds.iter().any(|(known, _)| known == name)
    }

    fn kind_of(&self, name: &str) -> Option<Kind> {
        self.kinds.iter().find(|(known, _)| known == name).map(|(_, kind)| *kind)
    }

    fn mention(&mut self, name: &str) {
        if !self.known(name) {
            self.kinds.push((name.to_string(), Kind::Class));
        }
        self.bodies.entry(name.to_string()).or_default();
    }

    fn last_defined(&self) -> Option<String> {
        self.kinds.last().map(|(name, _)| name.clone())
    }

    fn package_named(&self, id: &str) -> bool {
        self.packages.iter().any(|p| p.id == id)
    }

    fn package_open(&self) -> bool {
        !self.scopes.is_empty()
    }

    fn scope_id(&self) -> Option<String> {
        self.scopes.last().map(|p| p.id.clone())
    }

    fn chain_of(&self, id: &Option<String>) -> Vec<String> {
        Package::chain(id, &self.packages)
    }

    fn empty(&self) -> bool {
        self.kinds.is_empty()
    }

    fn open_package_line(&self) -> Option<usize> {
        if self.scopes.is_empty() { None } else { Some(0) }
    }

    fn caption(&mut self, caption: Caption) {
        self.captions.insert(caption.kind, caption);
    }

    fn directive(&mut self, text: &str) {
        self.directives.push(text.to_string());
    }

    fn open_style(&mut self) -> Result<(), Error> {
        if self.style.is_some() {
            // Ruby refuses a second style block.
            return Err(refused());
        }
        self.style = Some(BlockReader::new());
        Ok(())
    }

    fn close_style(&mut self) {
        if let Some(reader) = self.style.take() {
            self.style_sheet = Some(reader.sheet());
        }
    }

    fn hide_tag(&mut self, tag: &str) {
        self.hidden_tags.push(tag.to_string());
    }

    fn junction(&mut self, junction: Junction) {
        self.mention(&junction.owner);
        self.junctions.push(junction);
    }

    fn open_note(&mut self, head: NoteHead, lines: Vec<String>) -> Result<(), Error> {
        if let Some(target) = &head.target {
            if !self.known(target) {
                return Err(refused());
            }
        }
        self.note = Some((head, lines));
        Ok(())
    }

    fn add_note_line(&mut self, text: String) {
        if let Some((_, lines)) = self.note.as_mut() {
            lines.push(text);
        }
    }

    fn close_note(&mut self) {
        let Some((head, lines)) = self.note.take() else { return };
        self.notes.push(Note {
            side: head.side,
            target: head.target,
            member: head.member,
            color: head.color,
            stereotype: head.stereotype,
            lines,
        });
    }

    fn declare(&mut self, entry: DeclEntry, _text: &str) -> Result<(), Error> {
        let home = self.home_of(&entry.name)?;
        self.refuse_package_clash(&entry.name, &home)?;
        self.mention(&entry.name);
        if self.explicit.contains(&entry.name)
            && self.kind_of(&entry.name) != Some(entry.kind)
        {
            return Err(refused());
        }
        if let Some(slot) = self.kinds.iter_mut().find(|(known, _)| known == &entry.name) {
            slot.1 = entry.kind;
        }
        self.explicit.push(entry.name.clone());
        self.class_evidence = true;
        self.extras.insert(
            entry.name.clone(),
            Extra {
                generics: entry.generics,
                stereotypes: entry.stereotypes,
                tags: entry.tags,
                package: self.scope_id(),
            },
        );
        if entry.body {
            self.open = Some(entry.name);
        }
        Ok(())
    }

    fn refuse_package_clash(
        &self,
        name: &str,
        home: &Option<String>,
    ) -> Result<(), Error> {
        if self.package_named(name) {
            return Err(refused());
        }
        if self.known(name) && self.extras.get(name).map(|e| &e.package) != Some(home) {
            return Err(refused());
        }
        Ok(())
    }

    /// Ruby `home_of`: a dotted name lives in the namespaces before its
    /// last dot, opened as packages.
    fn home_of(&mut self, name: &str) -> Result<Option<String>, Error> {
        if !name.contains('.') {
            return Ok(self.scope_id());
        }
        let parts: Vec<&str> = name.split('.').collect();
        if !self.scopes.is_empty() || parts.iter().any(|part| part.is_empty()) {
            return Err(refused());
        }
        if class_name::namespaces(name).iter().any(|part| part.contains('\\')) {
            return Err(refused());
        }
        self.implicit_parents(&parts[..parts.len() - 1])
    }

    fn implicit_parents(&mut self, outer: &[&str]) -> Result<Option<String>, Error> {
        let mut parent = self.scope_id();
        for index in 0..outer.len() {
            let id = outer[..=index].join(".");
            let known = self
                .packages
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.parent.clone());
            match known {
                None => {
                    let title =
                        id.rsplit('.').next().unwrap_or(&id).to_string();
                    self.packages.push(Package {
                        id: id.clone(),
                        title,
                        shape: PackageShape::Folder,
                        icon: false,
                        parent: parent.clone(),
                        color: None,
                    });
                }
                Some(known_parent) if known_parent != parent => {
                    return Err(refused());
                }
                _ => {}
            }
            parent = Some(id);
        }
        Ok(parent)
    }

    fn open_package(&mut self, package: Package) -> Result<(), Error> {
        if self.packages.iter().any(|known| known.id == package.id) {
            return Err(refused());
        }
        let parts: Vec<&str> = package.id.split('.').collect();
        let parent = if parts.len() > 1 {
            self.implicit_parents(&parts[..parts.len() - 1])?
        } else {
            self.scope_id()
        };
        let mut inside = package.clone();
        inside.parent = parent;
        self.packages.push(inside.clone());
        self.scopes.push(package);
        Ok(())
    }

    fn close_package(&mut self) -> Result<(), Error> {
        let Some(package) = self.scopes.pop() else { return Err(refused()) };
        let members = self
            .extras
            .values()
            .filter(|entry| self.chain_of(&entry.package).contains(&package.id))
            .count();
        if members == 0 {
            return Err(refused());
        }
        Ok(())
    }

    fn add_member(&mut self, member: Member) {
        let Some(name) = self.open.clone() else { return };
        if let Some(body) = self.bodies.get_mut(&name) {
            body.push(member);
        }
    }

    fn add_member_to(&mut self, name: &str, member: Member) -> Result<(), Error> {
        self.mention(name);
        self.class_evidence = true;
        let Some(body) = self.bodies.get_mut(name) else { return Err(refused()) };
        body.push(member);
        Ok(())
    }

    fn relate(&mut self, relation: Relation, text: &str) -> Result<(), Error> {
        if !self.scopes.is_empty()
            && ![&relation.left, &relation.right]
                .iter()
                .all(|name| self.known(name))
        {
            return Err(refused());
        }
        for name in [&relation.left, &relation.right] {
            if name.contains('.') && !self.known(name) && !self.package_named(name) {
                return Err(refused());
            }
        }
        for end in [&relation.left, &relation.right] {
            // `mention_end`: a name that is a package's id, with no class
            // of that name, is the package: the relation runs to its frame.
            if !self.known(end) && self.package_named(end) {
                continue;
            }
            self.mention(end);
        }
        let class_only = relation_is_class_only(&relation);
        if !class_only && !global_command(text) {
            self.sequence_arrow = true;
        }
        self.class_evidence = self.class_evidence || class_only;
        self.relations.push(relation);
        Ok(())
    }

    fn diagram(self) -> Result<Diagram, Error> {
        if self.sequence_arrow && !self.class_evidence {
            return Err(refused());
        }
        for junction in &self.junctions {
            let pair = sorted_pair(&junction.from, &junction.to);
            let related = self.relations.iter().any(|r| {
                sorted_pair(&r.left, &r.right) == pair
            });
            if !related {
                return Err(refused());
            }
        }
        let classes = self.build_classes();
        self.visible(classes)
    }



    fn build_classes(&self) -> Vec<Klass> {
        let visibility = super::portion::PortionVisibility::new(&self.directives);
        self.kinds
            .iter()
            .map(|(name, kind)| {
                let body = self.bodies.get(name).cloned().unwrap_or_default();
                let extra = self.extras.get(name).cloned().unwrap_or_default();
                visibility.apply(Klass {
                    name: name.clone(),
                    kind: *kind,
                    body: grouped(body),
                    package: extra.package.clone(),
                    stereotypes: extra.stereotypes,
                    generics: extra.generics,
                    tags: extra.tags,
                })
            })
            .collect()
    }

    fn visible(mut self, classes: Vec<Klass>) -> Result<Diagram, Error> {
        let hidden: Vec<String> = classes
            .iter()
            .filter(|klass| {
                self.hidden_tags
                    .iter()
                    .any(|tag| klass.tags.iter().any(|own| own == tag))
            })
            .map(|klass| klass.name.clone())
            .collect();
        // `refuse_attachments_to`: a note or association class on a hidden
        // class is not measured, so it is not drawn.
        let mut owners: Vec<String> =
            self.junctions.iter().map(|j| j.owner.clone()).collect();
        owners.extend(self.notes.iter().filter_map(|note| note.target.clone()));
        if owners.iter().any(|owner| hidden.contains(owner)) {
            return Err(refused());
        }
        if self.smetana_pragma() {
            return Ok(self.smetana_visible(classes, &hidden));
        }
        let kept: Vec<Klass> = classes
            .into_iter()
            .filter(|klass| !hidden.contains(&klass.name))
            .collect();
        let mut gone = hidden.clone();
        gone.extend(self.unoccupied_ids(&kept));
        let relations: Vec<Relation> = self
            .relations
            .iter()
            .filter(|rel| !gone.contains(&rel.left) && !gone.contains(&rel.right))
            .cloned()
            .collect();
        Ok(self.assemble(kept, relations))
    }

    fn smetana_visible(self, classes: Vec<Klass>, hidden: &[String]) -> Diagram {
        let gone = self.unoccupied_ids(&classes);
        let relations: Vec<Relation> = self
            .relations
            .iter()
            .filter(|relation| {
                !gone.contains(&relation.left) && !gone.contains(&relation.right)
            })
            .map(|relation| {
                if hidden.contains(&relation.left) || hidden.contains(&relation.right) {
                    Relation { layout_only: true, ..relation.clone() }
                } else {
                    relation.clone()
                }
            })
            .collect();
        self.assemble(classes, relations)
    }

    fn unoccupied_ids(&self, classes: &[Klass]) -> Vec<String> {
        let package_ids: Vec<String> =
            self.packages.iter().map(|p| p.id.clone()).collect();
        let occupied: Vec<String> = classes
            .iter()
            .flat_map(|klass| self.chain_of(&klass.package))
            .collect();
        package_ids
            .into_iter()
            .filter(|id| !occupied.contains(id))
            .collect()
    }

    fn assemble(self, classes: Vec<Klass>, relations: Vec<Relation>) -> Diagram {
        let occupied: Vec<String> = classes
            .iter()
            .flat_map(|klass| self.chain_of(&klass.package))
            .collect();
        let mut kinds = [
            CaptionKind::Title,
            CaptionKind::Header,
            CaptionKind::Footer,
            CaptionKind::Caption,
            CaptionKind::Legend,
        ];
        let mut captions = vec![];
        for kind in kinds.iter_mut() {
            if let Some(caption) = self.captions.get(kind) {
                captions.push(caption.clone());
            }
        }
        Diagram {
            classes,
            relations,
            junctions: self.junctions,
            directives: self.directives,
            notes: self.notes,
            packages: self
                .packages
                .into_iter()
                .filter(|package| occupied.contains(&package.id))
                .collect(),
            captions,
            style: self
                .style_sheet
                .map(|sheet| -> StyleSheet { sheet.into() })
                .unwrap_or_default(),
        }
    }

    fn smetana_pragma(&self) -> bool {
        static SMETANA: OnceLock<Regex> = OnceLock::new();
        let smetana =
            SMETANA.get_or_init(|| Regex::new(r"(?i)\A!pragma[ \t]+layout[ \t]+smetana\z").unwrap());
        self.directives.iter().any(|line| smetana.is_match(line))
    }
}

fn sorted_pair(a: &str, b: &str) -> [String; 2] {
    let mut pair = [a.to_string(), b.to_string()];
    pair.sort();
    pair
}

/// Ruby `Klass#grouped`: a body with no `--`-style rule lists its fields
/// before its methods.
pub(crate) fn grouped(body: Vec<Member>) -> Vec<Member> {
    if body.iter().any(|member| member.kind == MemberKind::Separator) {
        return body;
    }
    let mut out: Vec<Member> = body
        .iter()
        .filter(|member| member.kind == MemberKind::Field)
        .cloned()
        .collect();
    out.extend(
        body.iter()
            .filter(|member| member.kind == MemberKind::Method)
            .cloned(),
    );
    out
}

/// Ruby `DiagramBuilder#class_only?`: a multiplicity, or an arrow other
/// than a plain `-->` or `<--`.
fn relation_is_class_only(relation: &Relation) -> bool {
    relation.left_multiplicity.is_some()
        || relation.right_multiplicity.is_some()
        || !relation.plain
}
