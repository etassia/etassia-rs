//! The PlantUML timing diagram: one track per participant (robust,
//! concise, clock, binary or analog) over a shared time axis. Port of
//! `notation/plantuml/timing/{reader,model,axis,signals,layout,arrows}.rb`
//! over the shared `Drawing` pen/scene/renderer.

use std::collections::HashMap;

use crate::svg::{ruby_float, write_num, Elem};
use crate::text::Face;

const INK: &str = "#181818";
/// `Signals::INK`: the grey of lane names and analog lines.
const SIGNAL_INK: &str = "#333333";
const FONT_SIZE: i64 = 12;

/// `Drawing::Face.width`.
fn face_width(text: &str, size: f64, bold: bool) -> f64 {
    crate::text::measure(
        text,
        size,
        if bold { Face::DejaVuBold } else { Face::DejaVu },
    )
    .width
}

/// `Drawing::Face.middle`: a middle-anchored y for a baseline.
fn face_middle(baseline: f64, size: f64) -> f64 {
    baseline - 0.3418 * size
}

// ---- model (timing/model.rb) ----

#[derive(Clone, Debug)]
struct Participant {
    id: String,
    title: String,
    kind: String,
    period: f64,
    pulse: f64,
    offset: f64,
}

#[derive(Clone, Debug)]
struct Change {
    id: String,
    time: f64,
    state: String,
    fill: Option<String>,
    stroke: Option<String>,
}

#[derive(Clone, Debug)]
struct Message {
    from: String,
    to: String,
    time: f64,
    arrives: Option<f64>,
    text: String,
}

#[derive(Clone, Debug)]
struct Highlight {
    from: f64,
    to: f64,
    fill: Option<String>,
    stroke: Option<String>,
    caption: Option<String>,
}

#[derive(Default)]
struct Diagram {
    participants: Vec<(String, Participant)>,
    changes: Vec<Change>,
    messages: Vec<Message>,
    highlights: Vec<Highlight>,
    pixels_per_unit: Option<f64>,
    dated: bool,
    scale_units: Option<i64>,
}

impl Diagram {
    fn participant(&self, id: &str) -> Option<&Participant> {
        self.participants
            .iter()
            .find(|(known, _)| known == id)
            .map(|(_, participant)| participant)
    }

    fn participant_mut(&mut self, id: &str) -> Option<&mut Participant> {
        self.participants
            .iter_mut()
            .find(|(known, _)| known == id)
            .map(|(_, participant)| participant)
    }
}

// ---- reader (timing/reader.rb) ----

/// The significant lines: `State::Reader.lines_of` — stripped, blanks and
/// `'` comments dropped, from the line after `@start` up to `@end`.
fn significant_lines(source: &str) -> Vec<(String, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let numbered: Vec<(String, usize)> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| (line.trim().to_string(), i + 1))
        .collect();
    let first = numbered.iter().position(|(text, _)| text.starts_with("@start"));
    let body: Vec<(String, usize)> = match first {
        Some(at) => numbered.into_iter().skip(at + 1).collect(),
        None => numbered,
    };
    body.into_iter()
        .take_while(|(text, _)| !text.starts_with("@end"))
        .filter(|(text, _)| !text.is_empty() && !text.starts_with('\''))
        .collect()
}

fn parse_time(text: &str) -> Option<f64> {
    let trimmed = text.strip_prefix('+').unwrap_or(text);
    if trimmed.is_empty() || !trimmed.chars().next()?.is_ascii_digit() {
        return None;
    }
    let (head, rest) = match trimmed.split_once('.') {
        Some((head, rest)) => (head, Some(rest)),
        None => (trimmed, None),
    };
    let whole: f64 = head.parse().ok()?;
    let fraction: f64 = match rest {
        Some(frac) if !frac.is_empty() => {
            if !frac.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let mut value = 0.0;
            let mut scale = 0.1;
            for digit in frac.chars() {
                value += digit.to_digit(10)? as f64 * scale;
                scale /= 10.0;
            }
            value
        }
        _ => 0.0,
    };
    Some(whole + fraction)
}

fn is_number(text: &str) -> bool {
    parse_time(text)
        .is_some_and(|value| !text.starts_with('+') && value.is_finite())
}

/// The participant declaration line: `kind ("title" as )?id [clock clause]`.
struct Declaration<'a> {
    kind: &'a str,
    title: Option<String>,
    id: String,
    period: Option<f64>,
    pulse: Option<f64>,
    offset: Option<f64>,
    rest_end: usize,
}

fn match_declaration(text: &str) -> Option<Declaration<'_>> {
    let lowered = text.to_lowercase();
    let kind = ["robust", "concise", "clock", "binary", "analog"]
        .iter()
        .find(|kind| lowered.starts_with(**kind))?;
    let mut at = kind.len();
    if !text[at..].starts_with(|c: char| c == ' ' || c == '\t') {
        return None;
    }
    at += text[at..].len() - text[at..].trim_start_matches([' ', '\t']).len();
    let mut title = None;
    if text[at..].starts_with('"') {
        let Some(close) = text[at + 1..].find('"') else { return None };
        title = Some(text[at + 1..at + 1 + close].to_string());
        at += close + 2;
        let trimmed = text[at..].trim_start_matches([' ', '\t']);
        at = text.len() - trimmed.len();
        let rest = &lowered[at..];
        if !rest.starts_with("as") {
            return None;
        }
        at += 2;
        let trimmed = text[at..].trim_start_matches([' ', '\t']);
        at = text.len() - trimmed.len();
    }
    let id_len = identifier_len(&text[at..])?;
    let id = text[at..at + id_len].to_string();
    at += id_len;
    let mut period = None;
    let mut pulse = None;
    let mut offset = None;
    let rest = text[at..].trim_start_matches([' ', '\t']);
    if !rest.is_empty() {
        let mut words = rest.split_whitespace();
        if words.next()?.to_lowercase() != "with" {
            return None;
        }
        if words.next()?.to_lowercase() != "period" {
            return None;
        }
        let value = words.next()?;
        if !is_number(value) {
            return None;
        }
        period = Some(value.parse().ok()?);
        let mut words = words.peekable();
        if words.peek().is_some_and(|word| word.to_lowercase() == "pulse") {
            words.next()?;
            let value = words.next()?;
            if !is_number(value) {
                return None;
            }
            pulse = Some(value.parse().ok()?);
        }
        if words.peek().is_some_and(|word| word.to_lowercase() == "and") {
            words.next()?;
        }
        if words.peek().is_some_and(|word| word.to_lowercase() == "offset") {
            words.next()?;
            let value = words.next()?;
            if !is_number(value) {
                return None;
            }
            offset = Some(value.parse().ok()?);
        }
    }
    Some(Declaration { kind, title, id, period, pulse, offset, rest_end: at })
}

fn identifier_len(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    Some(
        chars
            .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
            .map(|(i, _)| i)
            .unwrap_or(text.len()),
    )
}

fn parse_date_seconds(text: &str) -> Option<f64> {
    // `YYYY/MM/DD` at midnight UTC, seconds since the epoch.
    let (year, rest) = text.split_once('/')?;
    let (month, day) = rest.split_once('/')?;
    let year: i64 = year.parse().ok()?;
    let month: i64 = month.parse().ok()?;
    let day: i64 = day.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from 1970-01-01 by the civil-years algorithm (Howard Hinnant's
    // days_from_civil), which matches Ruby's Date arithmetic exactly for
    // proleptic Gregorian dates.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) as f64 * 86_400.0)
}

fn colours(spec: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(spec) = spec else { return (None, None) };
    let body = spec.strip_prefix('#').unwrap_or(spec);
    let (fill, stroke) = body.split_once(";line:").unwrap_or((body, ""));
    (css_colour(fill), css_colour(stroke))
}

fn css_colour(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    let hex = (name.len() == 3 || name.len() == 6)
        && name.chars().all(|c| c.is_ascii_hexdigit());
    if hex {
        return Some(format!("#{name}"));
    }
    (name.chars().all(|c| c.is_ascii_alphabetic())).then(|| name.to_string())
}

struct ParsedTiming {
    diagram: Diagram,
}

fn read_timing(source: &str) -> Result<ParsedTiming, crate::Error> {
    let mut diagram = Diagram::default();
    let mut now = 0.0f64;
    let mut lines = significant_lines(source).into_iter().peekable();
    let mut in_style = false;
    while let Some((text, number)) = lines.next() {
        if in_style {
            if text.eq_ignore_ascii_case("</style>") {
                in_style = false;
            }
            continue;
        }
        if text.eq_ignore_ascii_case("<style>") {
            in_style = true;
            continue;
        }
        let lowered = text.to_lowercase();
        let word_starts = ["title", "hide", "skinparam", "header", "footer", "caption"]
            .iter()
            .any(|word| {
                lowered.starts_with(word)
                    && text.as_bytes().get(word.len())
                        .is_some_and(|b| !b.is_ascii_alphanumeric())
            });
        let constraint = text.starts_with('@') && {
            let rest = &text[1..];
            let token: String =
                rest.chars().take_while(|c| !c.is_whitespace()).collect();
            parse_time(&token).is_some_and(|_| {
                rest[token.len()..]
                    .trim_start_matches([' ', '\t'])
                    .starts_with("<->")
            })
        };
        if word_starts || constraint {
            continue;
        }
        // participant
        if let Some(declaration) = match_declaration(&text) {
            let kind = declaration.kind.to_lowercase();
            if kind == "clock" && declaration.period.unwrap_or(0.0) as i64 <= 0
            {
                return Err(refused(number, &text));
            }
            let id = declaration.id.clone();
            let period = declaration.period.unwrap_or(0.0);
            let participant = Participant {
                id: id.clone(),
                title: declaration.title.clone().unwrap_or_else(|| id.clone()),
                kind,
                period,
                pulse: declaration.pulse.unwrap_or(period / 2.0),
                offset: declaration.offset.unwrap_or(0.0),
            };
            diagram.participants.push((id, participant));
            continue;
        }
        // @time
        if let Some(rest) = text.strip_prefix('@') {
            let word = rest.split([' ', '\t']).next().unwrap_or("");
            if !word.is_empty() {
                if let Some(value) = parse_time(word) {
                    now = if word.starts_with('+') { now + value } else { value };
                    continue;
                }
                // @Participant anchors where that participant last changed.
                if identifier_len(word).is_some_and(|len| len == word.len()) {
                    let id = word.to_string();
                    if diagram.participant(&id).is_none() {
                        return Err(refused(number, &text));
                    }
                    now = diagram
                        .changes
                        .iter()
                        .filter(|change| change.id == id)
                        .map(|change| change.time)
                        .fold(f64::NEG_INFINITY, f64::max);
                    if !now.is_finite() {
                        now = 0.0;
                    }
                    continue;
                }
            }
        }
        // `id is state`
        if let Some(space) = text.find(' ') {
            let (id, rest) = (text[..space].trim(), text[space..].trim());
            if !id.is_empty()
                && identifier_len(id).is_some_and(|len| len == id.len())
                && rest.starts_with("is")
                && rest[2..].starts_with(|c: char| c == ' ' || c == '\t')
            {
                if diagram.participant(id).is_none() {
                    return Err(refused(number, &text));
                }
                let rest = rest[2..].trim_start();
                let fill_spec = rest
                    .rfind(|c: char| c.is_whitespace())
                    .and_then(|at| {
                        let candidate = rest[at..].trim_start();
                        candidate.starts_with('#').then(|| candidate.to_string())
                    });
                let (fill, stroke) = colours(fill_spec.as_deref());
                let state = fill_spec
                    .and_then(|spec| rest.strip_suffix(&spec))
                    .unwrap_or(rest)
                    .trim()
                    .trim_matches('"')
                    .to_string();
                diagram.changes.push(Change {
                    id: id.to_string(),
                    time: now,
                    state,
                    fill,
                    stroke,
                });
                continue;
            }
        }
        // messages: `from -> to[@arrives] : label`
        if let Some(parsed) = match_message(&text) {
            if diagram.participant(&parsed.0).is_none() {
                return Err(refused(number, &text));
            }
            if diagram.participant(&parsed.1).is_none() {
                return Err(refused(number, &text));
            }
            diagram.messages.push(Message {
                from: parsed.0,
                to: parsed.1,
                time: now,
                arrives: parsed.2,
                text: parsed.3,
            });
            continue;
        }
        // highlight `from` to `to` (#fill;line:stroke)? (: caption)?
        if lowered.starts_with("highlight")
            && text.as_bytes().get("highlight".len()) == Some(&b' ')
        {
            let rest = text["highlight".len()..].trim_start();
            let (from, tail) = time_token(rest).ok_or_else(|| refused(number, &text))?;
            let tail = tail.trim_start();
            let tail = tail.strip_prefix("to").ok_or_else(|| refused(number, &text))?;
            let tail = tail.strip_prefix([' ', '\t']).unwrap_or(tail);
            let (to, tail) = time_token(tail).ok_or_else(|| refused(number, &text))?;
            let tail = tail.trim_start();
            let mut colour = None;
            let mut caption = None;
            let mut tail = tail;
            if let Some(after) = tail.strip_prefix('#') {
                let word: String =
                    after.chars().take_while(|c| !c.is_whitespace()).collect();
                colour = Some(word.clone());
                tail = &after[word.len()..];
                tail = tail.trim_start();
            }
            if let Some(after) = tail.strip_prefix(':') {
                caption = Some(after.trim().to_string());
            }
            let (fill, stroke) = colours(colour.as_deref());
            diagram.highlights.push(Highlight {
                from,
                to,
                fill,
                stroke,
                caption,
            });
            continue;
        }
        // scale `units` as `pixels` pixels
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() == 5
            && words[0].eq_ignore_ascii_case("scale")
            && words[2] == "as"
            && words[4].eq_ignore_ascii_case("pixels")
            && is_number(words[1])
            && is_number(words[3])
        {
            let units: i64 = words[1].parse().map_err(|_| refused(number, &text))?;
            let pixels: f64 = words[3].parse().map_err(|_| refused(number, &text))?;
            diagram.scale_units = Some(units);
            diagram.pixels_per_unit = Some(pixels / units as f64);
            continue;
        }
        return Err(refused(number, &text));
    }
    Ok(ParsedTiming { diagram })
}

fn refused(line: usize, text: &str) -> crate::Error {
    crate::Error::Parse {
        class: "UnsupportedConstructError".into(),
        message: format!("unsupported construct: timing statement at line {line}: {text}"),
        line,
        col: 0,
    }
}

type MessageParts = (String, String, Option<f64>, String);

/// `from -> to[@arrives] : label`, anchored at both ends like the Ruby
/// rule: the label colon is the one that follows the target token.
fn match_message(text: &str) -> Option<MessageParts> {
    let arrow = text.find("->")?;
    let from = text[..arrow].trim();
    if from.is_empty() || identifier_len(from).is_none_or(|len| len != from.len())
    {
        return None;
    }
    let rest = text[arrow + 2..].trim_start();
    let id_len = identifier_len(rest)?;
    let to = rest[..id_len].to_string();
    let mut tail = &rest[id_len..];
    let mut arrives = None;
    if let Some(after) = tail.strip_prefix('@') {
        let (word, remainder) = time_token(after)?;
        arrives = Some(word);
        tail = remainder;
    }
    let mut label = String::new();
    let trimmed = tail.trim_start();
    if let Some(after) = trimmed.strip_prefix(':') {
        label = after.trim().to_string();
        tail = &tail[tail.len() - trimmed.len() + 1..];
        let _ = tail;
    } else if !trimmed.is_empty() {
        return None;
    }
    Some((from.to_string(), to, arrives, label))
}

/// A leading `TIME` token (`\+?\d+(?:\.\d+)?` or a date): its value and
/// the text after it.
fn time_token(text: &str) -> Option<(f64, &str)> {
    let bytes = text.as_bytes();
    let mut end = 0;
    if bytes.first() == Some(&b'+') {
        end = 1;
    }
    let digits_from = |mut at: usize| -> Option<usize> {
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        (at > start).then_some(at)
    };
    end = digits_from(end)?;
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(|b| b.is_ascii_digit()) {
        end += 1;
        end = digits_from(end)?;
    }
    if end == text.len() || bytes[end] != b'/' {
        let value: f64 = text[..end].parse().ok()?;
        return Some((value, &text[end..]));
    }
    // a date `YYYY/MM/DD`
    let mut cursor = end;
    for _ in 0..2 {
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == start {
            return None;
        }
    }
    let value = parse_date_seconds(&text[..cursor])?;
    Some((value, &text[cursor..]))
}

// ---- axis (timing/axis.rb) ----

struct Axis {
    start: f64,
    finish: f64,
    step: Option<f64>,
    pixels: f64,
    left: f64,
    dated: bool,
    has_scale: bool,
    named_times: Vec<f64>,
}

impl Axis {
    fn new(diagram: &Diagram, left: f64, plot: f64) -> Axis {
        let mut times: Vec<f64> = moments(diagram);
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let start = times.first().copied().unwrap_or(0.0);
        let last = times.iter().copied().fold(start, f64::max);
        let span = if last > start { last - start } else { 100.0 };
        let named = named_times(diagram);
        let dated = diagram.dated;
        let step = if let Some(units) = diagram.scale_units {
            Some(units as f64)
        } else if dated {
            None
        } else {
            common_divisor(&named)
        };
        let finish = last + step.unwrap_or(span * 0.25);
        let pixels =
            diagram.pixels_per_unit.unwrap_or_else(|| match step {
                Some(step) => 50.0 / step,
                None => plot / (finish - start),
            });
        Axis { start, finish, step, pixels, left, dated, has_scale: diagram.pixels_per_unit.is_some(), named_times: named }
    }

    fn x(&self, time: f64) -> f64 {
        self.left + (time - self.start) * self.pixels
    }

    fn ticks(&self, diagram: &Diagram) -> Vec<f64> {
        match self.step {
            None => {
                let mut times = moments(diagram);
                times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                times.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
                times
            }
            Some(step) => {
                let count = ((self.finish - self.start) / step).floor();
                (0..=count as i64)
                    .map(|index| self.start + index as f64 * step)
                    .collect()
            }
        }
    }

    fn labelled(&self, time: f64) -> bool {
        if !(self.step.is_some() && !self.has_scale) {
            return true;
        }
        self.named_times
            .iter()
            .any(|named| (named - time).abs() < 1e-9)
    }

    fn label(&self, time: f64) -> String {
        if self.dated {
            return date_label(time);
        }
        if time == time.floor() {
            (time as i64).to_string()
        } else {
            ruby_float(time)
        }
    }
}

fn moments(diagram: &Diagram) -> Vec<f64> {
    let mut times: Vec<f64> = diagram.changes.iter().map(|c| c.time).collect();
    for message in &diagram.messages {
        times.push(message.time);
        if let Some(arrives) = message.arrives {
            times.push(arrives);
        }
    }
    for highlight in &diagram.highlights {
        times.push(highlight.from);
        times.push(highlight.to);
    }
    times
}

fn named_times(diagram: &Diagram) -> Vec<f64> {
    let mut times: Vec<f64> = diagram
        .participants
        .iter()
        .filter_map(|(_, p)| {
            let period = p.period;
            (period > 0.0).then_some(period)
        })
        .collect();
    times.extend(diagram.changes.iter().map(|c| c.time));
    times
}

fn common_divisor(times: &[f64]) -> Option<f64> {
    if !times.iter().all(|time| *time == time.floor()) {
        return None;
    }
    let step = times
        .iter()
        .map(|time| *time as i64)
        .fold(0i64, gcd);
    (step > 0).then(|| step as f64)
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// Ruby `Time.at(time).utc.strftime("%m/%d")` for a UTC instant.
fn date_label(time: f64) -> String {
    let days = (time / 86_400.0).floor() as i64;
    // Civil-from-days (Howard Hinnant), the inverse of the reader's
    // days_from_civil.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let _ = y;
    format!("{:02}/{:02}", m, (doy - (153 * mp + 2) / 5 + 1))
}

// ---- the scene painter (Drawing::Pen over rs Elem) ----

struct Marks {
    marks: Vec<Elem>,
}

impl Marks {
    fn put(&mut self, element: Elem) {
        self.marks.push(element);
    }

    fn rect(&mut self, left: f64, top: f64, width: f64, height: f64, fill: &str, stroke: &str) {
        let mut element =
            Elem::new("rect").num("x", left).num("y", top).num("width", width).num("height", height);
        element = apply_style(element, StyleBits::fill(fill).stroke(stroke).width("1"));
        self.put(element);
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, style: StyleBits) {
        let mut element =
            Elem::new("line").num("x1", x1).num("y1", y1).num("x2", x2).num("y2", y2);
        // Ruby's Pen.line has no fill default: the attribute is absent.
        element = apply_style(element, StyleBits::none().stroke(INK).width("1").overlay(style));
        self.put(element);
    }

    fn path(&mut self, data: String, style: StyleBits) {
        let mut element = Elem::new("path").attr("d", data);
        element = apply_style(element, StyleBits::fill("none").stroke(INK).width("1").overlay(style));
        self.put(element);
    }

    fn polygon(&mut self, points: String, style: StyleBits) {
        let mut element = Elem::new("polygon").attr("points", points);
        element = apply_style(element, StyleBits::fill(INK).stroke(INK).width("1").overlay(style));
        self.put(element);
    }

    fn text(&mut self, content: &str, left: f64, middle: f64, options: TextOptions) {
        let mut element = Elem::new("text")
            .num("x", left)
            .num("y", middle)
            .content(content)
            .attr("text-anchor", options.anchor)
            .attr("font-family", options.family)
            .attr("font-size", &options.size.to_string())
            .attr("fill", options.fill)
            .attr("dominant-baseline", "middle");
        let _ = options.weight;
        if options.bold {
            element = element.attr("font-weight", "bold");
        }
        self.put(element);
    }
}

struct StyleBits {
    fill: Option<String>,
    stroke: Option<String>,
    width: Option<String>,
    dasharray: Option<String>,
}

impl StyleBits {
    fn none() -> StyleBits {
        StyleBits { fill: None, stroke: None, width: None, dasharray: None }
    }

    fn fill(fill: &str) -> StyleBits {
        StyleBits { fill: Some(fill.to_string()), stroke: None, width: None, dasharray: None }
    }

    fn stroke(self, stroke: &str) -> StyleBits {
        StyleBits { stroke: Some(stroke.to_string()), ..self }
    }

    fn width(self, width: &str) -> StyleBits {
        StyleBits { width: Some(width.to_string()), ..self }
    }

    fn dasharray(self, dash: &str) -> StyleBits {
        StyleBits { dasharray: Some(dash.to_string()), ..self }
    }

    fn overlay(self, over: StyleBits) -> StyleBits {
        StyleBits {
            fill: over.fill.or(self.fill),
            stroke: over.stroke.or(self.stroke),
            width: over.width.or(self.width),
            dasharray: over.dasharray.or(self.dasharray),
        }
    }
}

fn apply_style(element: Elem, style: StyleBits) -> Elem {
    let mut element = element;
    if let Some(fill) = style.fill {
        element = element.attr("fill", &fill);
    }
    if let Some(stroke) = style.stroke {
        element = element.attr("stroke", &stroke);
    }
    if let Some(width) = style.width {
        element = element.attr("stroke-width", &width);
    }
    if let Some(dash) = style.dasharray {
        element = element.attr("stroke-dasharray", &dash);
    }
    element
}

struct TextOptions {
    size: i64,
    anchor: &'static str,
    bold: bool,
    family: &'static str,
    fill: String,
    weight: Option<&'static str>,
}

impl Default for TextOptions {
    fn default() -> TextOptions {
        TextOptions {
            size: FONT_SIZE,
            anchor: "start",
            bold: false,
            family: "sans-serif",
            fill: INK.to_string(),
            weight: None,
        }
    }
}

// ---- signals (timing/signals.rb) ----

const GREEN: &str = "#006400";
const LAVENDER: &str = "#E2E2F0";
const ROW: f64 = 20.0;
const HIGH: f64 = 25.49;
const LOW: f64 = 39.49;
const BOTTOM: f64 = 18.0;
const TIP: f64 = 12.0;
const LABEL_X: f64 = 25.0;
const NAME_DROP: f64 = 5.54;
const SEGMENT_MIDDLE: f64 = 34.49;
const CONCISE_RISE: f64 = 12.0;
const ANALOG_LOW: f64 = 109.49;
const MAX_EDGES: i64 = 2000;

struct Signals<'a> {
    diagram: &'a Diagram,
    axis: Option<&'a Axis>,
    left: f64,
}

impl<'a> Signals<'a> {
    fn new(diagram: &'a Diagram, axis: Option<&'a Axis>, left: f64) -> Signals<'a> {
        Signals { diagram, axis, left }
    }

    fn height(&self, participant: &Participant) -> f64 {
        if participant.kind != "robust" {
            return match participant.kind.as_str() {
                "clock" | "binary" => 47.49,
                "concise" => 56.49,
                "analog" => 117.49,
                _ => 47.49,
            };
        }
        HIGH + ROW * self.states(participant).len().max(1) as f64
    }

    fn lead(&self, participant: &Participant) -> f64 {
        let lead = match participant.kind.as_str() {
            "clock" | "binary" | "analog" => 30.0,
            "concise" | "robust" => 25.0,
            _ => 25.0,
        };
        lead + self.stand(participant)
    }

    fn draw_labels(&self, pen: &mut Marks, participant: &Participant, top: f64) {
        for (name, y) in self.labels(participant, top) {
            if participant.kind == "robust" {
                pen.text(
                    &name,
                    LABEL_X,
                    face_middle(y + NAME_DROP, 12.0),
                    TextOptions { fill: SIGNAL_INK.to_string(), ..TextOptions::default() },
                );
            } else {
                pen.text(
                    &name,
                    self.left - 2.0,
                    y,
                    TextOptions { size: 14, anchor: "end", bold: true, fill: SIGNAL_INK.to_string(), ..TextOptions::default() },
                );
            }
        }
    }

    fn draw(&self, pen: &mut Marks, participant: &Participant, top: f64) {
        match participant.kind.as_str() {
            "binary" => self.draw_steps_like(pen, participant, top, "2", SignalFlavour::Binary),
            "robust" => self.draw_steps_like(pen, participant, top, "2", SignalFlavour::Robust),
            "clock" => self.draw_clock(pen, participant, top),
            "concise" => self.draw_concise(pen, participant, top),
            "analog" => self.draw_analog(pen, participant, top),
            _ => {}
        }
    }

    fn y_at(&self, participant: &Participant, top: f64, time: f64) -> f64 {
        if participant.kind == "concise" {
            return self.concise_y(participant, top, time);
        }
        if !matches!(participant.kind.as_str(), "binary" | "robust") {
            return top + self.height(participant) / 2.0;
        }
        let state = self
            .changes(participant)
            .iter()
            .filter(|change| change.time <= time)
            .last()
            .map(|change| change.state.clone());
        self.level_y(participant, top, state.as_deref())
    }

    fn concise_y(&self, participant: &Participant, top: f64, time: f64) -> f64 {
        let middle = top + SEGMENT_MIDDLE;
        let times: Vec<f64> = self.changes(participant).iter().map(|c| c.time).collect();
        if times.is_empty() || time <= times[0] || times.contains(&time) {
            return middle;
        }
        middle - CONCISE_RISE
    }

    fn stand(&self, participant: &Participant) -> f64 {
        let (size, bold) = if participant.kind == "analog" { (14.0, true) } else { (12.0, false) };
        if !matches!(participant.kind.as_str(), "robust" | "analog") {
            return 0.0;
        }
        self.labels(participant, 0.0)
            .iter()
            .map(|(name, _)| face_width(name, size, bold))
            .fold(0.0f64, f64::max)
    }

    fn labels(&self, participant: &Participant, top: f64) -> Vec<(String, f64)> {
        match participant.kind.as_str() {
            "robust" => self
                .states(participant)
                .iter()
                .map(|state| (state.clone(), self.level_y(participant, top, Some(state))))
                .collect(),
            "analog" => {
                let (low, high) = self.values(participant);
                vec![
                    (float_label(high), face_middle(top + 30.78, 14.0)),
                    (float_label(low), face_middle(top + 114.78, 14.0)),
                ]
            }
            _ => vec![],
        }
    }

    fn changes(&self, participant: &Participant) -> Vec<Change> {
        let mut changes: Vec<Change> = self
            .diagram
            .changes
            .iter()
            .filter(|change| change.id == participant.id)
            .cloned()
            .collect();
        changes.sort_by(|a, b| {
            a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal)
        });
        changes
    }

    fn states(&self, participant: &Participant) -> Vec<String> {
        let mut states: Vec<String> = vec![];
        for change in self.changes(participant) {
            if !states.contains(&change.state) {
                states.push(change.state);
            }
        }
        states
    }

    fn values(&self, participant: &Participant) -> (f64, f64) {
        let list: Vec<f64> = self
            .changes(participant)
            .iter()
            .filter_map(|change| change.state.parse::<f64>().ok())
            .collect();
        if list.is_empty() {
            return (0.0, 0.0);
        }
        let low = list.iter().copied().fold(f64::INFINITY, f64::min);
        let high = list.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (low, high)
    }

    fn level_y(&self, participant: &Participant, top: f64, state: Option<&str>) -> f64 {
        if participant.kind == "binary" {
            return if binary_high(state) { top + HIGH } else { top + LOW };
        }
        let index = state
            .and_then(|state| self.states(participant).iter().position(|known| known == state))
            .unwrap_or(0);
        top + self.height(participant) - BOTTOM - index as f64 * ROW
    }

    fn draw_steps_like(
        &self,
        pen: &mut Marks,
        participant: &Participant,
        top: f64,
        width: &str,
        flavour: SignalFlavour,
    ) {
        let Some(axis) = self.axis else { return };
        let mut points: Vec<(f64, f64)> = self
            .changes(participant)
            .iter()
            .map(|change| (change.time, self.level_y(participant, top, Some(&change.state))))
            .collect();
        if flavour == SignalFlavour::Binary
            && (points.is_empty() || points[0].0 > axis.start)
        {
            let resting = self.level_y(participant, top, None);
            points.insert(0, (axis.start, resting));
        }
        self.steps(pen, axis, &points, width);
    }

    fn draw_clock(&self, pen: &mut Marks, participant: &Participant, top: f64) {
        let Some(axis) = self.axis else { return };
        let high = top + HIGH;
        let low = top + LOW;
        let points: Vec<(f64, f64)> = self
            .clock_edges(participant, axis)
            .iter()
            .map(|(time, on)| (*time, if *on { high } else { low }))
            .collect();
        self.steps(pen, axis, &points, "1.5");
    }

    fn clock_edges(&self, participant: &Participant, axis: &Axis) -> Vec<(f64, bool)> {
        let start = axis.start;
        let edges = self.edges_of(participant, axis);
        let inside: Vec<(f64, bool)> = edges
            .iter()
            .copied()
            .filter(|(time, _)| *time > start && *time <= axis.finish)
            .collect();
        let mut all = self.opening(participant, &edges, start);
        all.extend(inside);
        all
    }

    fn opening(&self, participant: &Participant, edges: &[(f64, bool)], start: f64) -> Vec<(f64, bool)> {
        let level = self.high_at(participant, start);
        if edges.iter().any(|(time, _)| *time == start) {
            return vec![(start, !level), (start, level)];
        }
        vec![(start, level)]
    }

    fn high_at(&self, clock: &Participant, time: f64) -> bool {
        if time < clock.offset {
            return false;
        }
        ((time - clock.offset) % clock.period) < clock.pulse
    }

    fn edges_of(&self, clock: &Participant, axis: &Axis) -> Vec<(f64, bool)> {
        self.rises_of(clock, axis)
            .into_iter()
            .flat_map(|rise| vec![(rise, true), (rise + clock.pulse, false)])
            .collect()
    }

    fn rises_of(&self, clock: &Participant, axis: &Axis) -> Vec<f64> {
        let first = ((axis.start - clock.offset) / clock.period).floor().max(0.0) as i64;
        let mut rises = vec![];
        for cycle in first..(first + MAX_EDGES) {
            let rise = clock.offset + cycle as f64 * clock.period;
            if rise > axis.finish {
                break;
            }
            rises.push(rise);
        }
        rises
    }

    fn steps(&self, pen: &mut Marks, axis: &Axis, points: &[(f64, f64)], width: &str) {
        if points.is_empty() {
            return;
        }
        let mut data = format!(
            "M {} {}",
            ruby_float(axis.x(points[0].0)),
            ruby_float(points[0].1)
        );
        for pair in points.windows(2) {
            data.push_str(&format!(
                " L {} {} L {} {}",
                ruby_float(axis.x(pair[1].0)),
                ruby_float(pair[0].1),
                ruby_float(axis.x(pair[1].0)),
                ruby_float(pair[1].1)
            ));
        }
        data.push_str(&format!(
            " L {} {}",
            ruby_float(axis.x(axis.finish)),
            ruby_float(points[points.len() - 1].1)
        ));
        pen.path(data, StyleBits::fill("none").stroke(GREEN).width(width));
    }

    fn draw_concise(&self, pen: &mut Marks, participant: &Participant, top: f64) {
        let Some(axis) = self.axis else { return };
        let list = self.changes(participant);
        for (index, change) in list.iter().enumerate() {
            let finish = list.get(index + 1).map(|next| next.time).unwrap_or(axis.finish);
            self.segment(pen, axis, change, finish, top, list.get(index + 1));
        }
    }

    fn segment(
        &self,
        pen: &mut Marks,
        axis: &Axis,
        change: &Change,
        finish: f64,
        top: f64,
        closed: Option<&Change>,
    ) {
        let left = axis.x(change.time);
        let right = axis.x(finish);
        pen.path(
            hexagon(left, right, top, closed.is_some()),
            StyleBits::fill("none")
                .stroke(INK)
                .width("1")
                .overlay(StyleBits::fill(change.fill.as_deref().unwrap_or(LAVENDER))
                    .stroke(change.stroke.as_deref().unwrap_or(GREEN))
                    .width("1.5")),
        );
        let (x, anchor) = if closed.is_some() {
            ((left + right) / 2.0, "middle")
        } else {
            (left + TIP, "start")
        };
        pen.text(
            &change.state,
            x,
            face_middle(top + SEGMENT_MIDDLE + 4.54, 12.0),
            TextOptions { size: 12, anchor, bold: true, fill: SIGNAL_INK.to_string(), ..TextOptions::default() },
        );
    }

    fn draw_analog(&self, pen: &mut Marks, participant: &Participant, top: f64) {
        let Some(axis) = self.axis else { return };
        let (low, high) = self.values(participant);
        let range = if high > low { high - low } else { 1.0 };
        let changes = self.changes(participant);
        if changes.is_empty() {
            return;
        }
        let mut data = String::new();
        for (index, change) in changes.iter().enumerate() {
            let share = (change.state.parse::<f64>().unwrap_or(0.0) - low) / range;
            let y = top + ANALOG_LOW - share * (ANALOG_LOW - HIGH);
            let verb = if index == 0 { "M" } else { "L" };
            data.push_str(&format!(
                "{} {} {} ",
                verb,
                ruby_float(axis.x(change.time)),
                ruby_float(y)
            ));
        }
        data.push_str(&format!(
            "L {} {}",
            ruby_float(axis.x(axis.finish)),
            ruby_float(changes.last().map(|change| {
                let share = (change.state.parse::<f64>().unwrap_or(0.0) - low) / range;
                top + ANALOG_LOW - share * (ANALOG_LOW - HIGH)
            }).unwrap_or(top))
        ));
        pen.path(data, StyleBits::fill("none").stroke(SIGNAL_INK).width("0.5"));
    }
}

#[derive(PartialEq, Clone, Copy)]
enum SignalFlavour {
    Binary,
    Robust,
}

fn binary_high(state: Option<&str>) -> bool {
    matches!(
        state.unwrap_or("").to_lowercase().as_str(),
        "high" | "1" | "true"
    )
}

fn float_label(value: f64) -> String {
    ruby_float(value)
}

fn hexagon(left: f64, right: f64, top: f64, closed: bool) -> String {
    let upper = top + SEGMENT_MIDDLE - 12.0;
    let lower = top + SEGMENT_MIDDLE + 12.0;
    let middle = top + SEGMENT_MIDDLE;
    let notch = [(left + TIP, upper), (left, middle), (left + TIP, lower)];
    let points: Vec<(f64, f64)> = if closed {
        let mut all = notch.to_vec();
        all.extend([(right - TIP, lower), (right, middle), (right - TIP, upper)]);
        all
    } else {
        let mut all = vec![(right, upper)];
        all.extend(notch);
        all.push((right, lower));
        all
    };
    let mut data = String::new();
    for (index, (x, y)) in points.iter().enumerate() {
        data.push_str(&format!(
            "{} {} {} ",
            if index == 0 { "M" } else { "L" },
            ruby_float(*x),
            ruby_float(*y)
        ));
    }
    if closed {
        data.push('Z');
    }
    data.trim_end().to_string()
}

// ---- arrows (timing/arrows.rb) ----

const ARROW_BLUE: &str = "#00008B";
const ARROW_HEAD: f64 = 7.5;
const ARROW_WING: f64 = 2.74;
const ARROW_PULL: f64 = 7.518;
const ARROW_BELOW: f64 = 7.98;
const ARROW_ABOVE: f64 = -8.11;

fn arrows_draw(
    pen: &mut Marks,
    axis: &Axis,
    signals: &Signals,
    tracks: &HashMap<String, (f64, f64)>,
    diagram: &Diagram,
) {
    for message in &diagram.messages {
        let Some(from) = diagram.participant(&message.from) else { continue };
        let Some(to) = diagram.participant(&message.to) else { continue };
        let from_top = tracks[&message.from].0;
        let to_top = tracks[&message.to].0;
        let tail = (axis.x(message.time), signals.y_at(from, from_top, message.time));
        let tip_time = message.arrives.unwrap_or(message.time);
        let tip = (axis.x(tip_time), signals.y_at(to, to_top, tip_time));
        pen.line(
            tail.0, tail.1, tip.0, tip.1,
            StyleBits::none().stroke(ARROW_BLUE).width("1.5"),
        );
        // head
        if tail != tip {
            let dir = ((tip.0 - tail.0) / hypot(tail, tip), (tip.1 - tail.1) / hypot(tail, tip));
            let base = (tip.0 - ARROW_HEAD * dir.0, tip.1 - ARROW_HEAD * dir.1);
            let wing = |side: f64| {
                (
                    ruby_float(base.0 - side * dir.1),
                    ruby_float(base.1 + side * dir.0),
                )
            };
            let points = format!(
                "{},{} {},{} {},{}",
                ruby_float(tip.0),
                ruby_float(tip.1),
                wing(ARROW_WING).0,
                wing(ARROW_WING).1,
                wing(-ARROW_WING).0,
                wing(-ARROW_WING).1
            );
            pen.polygon(
                points,
                StyleBits::fill("none")
                    .stroke(INK)
                    .width("1")
                    .overlay(StyleBits::fill(ARROW_BLUE).stroke(ARROW_BLUE).width("1.5")),
            );
        }
        // label: serif, under the tip of a rising arrow, above a falling one
        let dir = if tail == tip {
            (0.0, -1.0)
        } else {
            ((tip.0 - tail.0) / hypot(tail, tip), (tip.1 - tail.1) / hypot(tail, tip))
        };
        let drop = if dir.1.is_sign_negative() || dir.1 == 0.0 { ARROW_BELOW } else { ARROW_ABOVE };
        let drop = if dir.1 > 0.0 { ARROW_ABOVE } else { ARROW_BELOW };
        pen.text(
            &message.text,
            tip.0 - ARROW_PULL * dir.0,
            tip.1 + drop - ARROW_PULL * dir.1,
            TextOptions { size: 14, family: "Serif", fill: ARROW_BLUE.to_string(), ..TextOptions::default() },
        );
    }
}

fn hypot(from: (f64, f64), to: (f64, f64)) -> f64 {
    (to.0 - from.0).hypot(to.1 - from.1)
}

// ---- layout (timing/layout.rb) ----

const TOP: f64 = 20.0;
const MARGIN: f64 = 20.0;
const REACH: f64 = 5.0;
const FOOT: f64 = 33.0;
const PLOT: f64 = 420.0;
const MIN_LEFT: f64 = 25.0;
const RULE: &str = "#333333";
const TAB_HEIGHT: f64 = 17.5;
const TAB_SLANT: f64 = 10.0;
const GRID: &str = "3,5";

pub fn render(source: &str) -> Result<String, crate::Error> {
    let ParsedTiming { diagram } = read_timing(source)?;
    let left = {
        let signals = Signals::new(&diagram, None, 0.0);
        let leads = diagram
            .participants
            .iter()
            .map(|(_, p)| signals.lead(p))
            .fold(0.0f64, f64::max);
        leads.max(MIN_LEFT)
    };
    let axis = Axis::new(&diagram, left, PLOT);
    let signals = Signals::new(&diagram, Some(&axis), left);
    let mut top = TOP;
    let mut tracks: HashMap<String, (f64, f64)> = HashMap::new();
    for (id, participant) in &diagram.participants {
        let size = signals.height(participant);
        tracks.insert(id.clone(), (top, size));
        top += size;
    }
    let bottom = diagram
        .participants
        .iter()
        .filter_map(|(id, _)| tracks.get(id).map(|(top, size)| top + size))
        .fold(TOP, f64::max);
    let right = axis.x(axis.finish) + REACH;
    let tabs = diagram
        .participants
        .iter()
        .map(|(_, p)| tab_edge(&p.title))
        .fold(0.0f64, f64::max);
    let canvas_width =
        (right.max(tabs + TAB_SLANT) + MARGIN).floor() as i64 + 1;
    let canvas_height = (bottom + FOOT).ceil() as i64;

    let mut pen = Marks { marks: vec![] };
    // backdrop
    for highlight in &diagram.highlights {
        let hl = axis.x(highlight.from);
        let hr = axis.x(highlight.to);
        pen.rect(hl, TOP, hr - hl, bottom - TOP,
                 highlight.fill.as_deref().unwrap_or("#EEEEEE"), "none");
        for x in [hl, hr] {
            pen.line(x, TOP, x, bottom,
                     StyleBits::none()
                         .stroke(highlight.stroke.as_deref().unwrap_or("#555555"))
                         .width("1.5").dasharray("5,3"));
        }
    }
    let ticks = axis.ticks(&diagram);
    for time in &ticks {
        let across = axis.x(*time);
        pen.line(across, TOP, across, bottom,
                 StyleBits::none().stroke(RULE).width("0.5").dasharray(GRID));
    }
    // foreground
    for (_, participant) in &diagram.participants {
        let (track_top, _) = tracks[&participant.id];
        pen.line(MARGIN, track_top, right, track_top,
                 StyleBits::none().stroke(RULE).width("0.5"));
        pen.text(&participant.title, MARGIN + 5.0, track_top + 8.75,
                 TextOptions { size: 14, bold: true, fill: RULE.to_string(), ..TextOptions::default() });
        // tab
        let edge = tab_edge(&participant.title);
        let low = track_top + TAB_HEIGHT;
        pen.line(MARGIN, low, edge, low,
                 StyleBits::none().stroke(RULE).width("0.5"));
        pen.line(edge, low, edge + TAB_SLANT, track_top,
                 StyleBits::none().stroke(RULE).width("0.5"));
        signals.draw_labels(&mut pen, participant, track_top);
        signals.draw(&mut pen, participant, track_top);
    }
    for x in [MARGIN, right] {
        pen.line(x, TOP, x, bottom, StyleBits::none().stroke(RULE).width("0.5"));
    }
    arrows_draw(&mut pen, &axis, &signals, &tracks, &diagram);
    // axis line
    pen.line(axis.x(axis.start), bottom, axis.x(axis.finish), bottom,
             StyleBits::none().stroke(RULE).width("2"));
    for time in &ticks {
        let x = axis.x(*time);
        pen.line(x, bottom, x, bottom + 5.0,
                 StyleBits::none().stroke(INK).width("1"));
        if axis.labelled(*time) {
            pen.text(&axis.label(*time), x, bottom + 12.87,
                     TextOptions { size: 11, anchor: "middle", fill: RULE.to_string(), ..TextOptions::default() });
        }
    }
    for highlight in &diagram.highlights {
        let Some(caption) = &highlight.caption else { continue };
        for (row, entry) in caption.split("\\n").enumerate() {
            pen.text(entry, axis.x(highlight.from) + 4.0, TOP + 10.0 + row as f64 * 14.0,
                     TextOptions::default());
        }
    }
    // the scene renderer: document with integer canvas, marks in order
    let mut document = crate::svg::Document::new(canvas_width as f64, canvas_height as f64);
    document.view_box = Some(format!("0 0 {canvas_width} {canvas_height}"));
    for mark in pen.marks {
        document.add(mark);
    }
    Ok(document.to_xml())
}

fn tab_edge(title: &str) -> f64 {
    MARGIN + 6.0 + face_width(title, 14.0, true)
}

