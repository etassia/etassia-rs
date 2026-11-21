//! The PlantUML gantt chart (`@startgantt`): port of
//! `notation/plantuml/gantt/reader.rb`, the scheduling core of
//! `layout/gantt.rb` the PlantUML scene reuses, and `gantt/layout.rb` +
//! `gantt/renderer.rb` over the shared SVG layer. Dates are civil day
//! numbers (`Date` arithmetic is whole days throughout); float geometry
//! follows the Ruby operation order, and the array sums run Ruby's
//! Kahan-Babuska compensation (`Array#sum`).

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::svg::{ruby_float, Document, Elem};
use crate::text::Face;
use crate::Error;

// ---- reader (notation/plantuml/gantt/reader.rb) ----

const ANCHOR: &str = "2000-01-01";

/// The reader's regex table, keyed as the Ruby constants.
fn table() -> &'static HashMap<&'static str, Regex> {
    static CELL: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CELL.get_or_init(|| {
        let english = "(?:\\d{1,2})(?:st|nd|rd|th)?[ \\t]+of[ \\t]+(?:january|february|march|april|may|june|july|august|september|october|november|december)(?:[ \\t]+(?:\\d{4}))?";
        let entries: &[(&str, String)] = &[
            ("english", format!("(?i)(\\d{{1,2}})(?:st|nd|rd|th)?[ \\t]+of[ \\t]+(january|february|march|april|may|june|july|august|september|october|november|december)(?:[ \\t]+(\\d{{4}}))?")),
            ("project", format!("(?i)\\AProject[ \\t]+starts[ \\t]+(?:the[ \\t]+|on[ \\t]+)?(?:the[ \\t]+)?(\\d{{4}}-\\d{{2}}-\\d{{2}}|{english})\\z")),
            ("section", r"(?i)\A--+[ \t]*(.*?)[ \t]*--+\z".to_string()),
            ("arrow", r"(?i)\A\[([^\]]+)\][ \t]*-+>[ \t]*\[([^\]]+)\]\z".to_string()),
            ("task", r"(?i)\A(then[ \t]+)?\[([^\]]+)\](?:[ \t]+as[ \t]+\[([^\]]+)\])?(?:[ \t]+(.*))?\z".to_string()),
            ("skipped", "(?i)\\A(?:title|header|footer|caption|legend|skinparam|scale|printscale|ganttscale|projectscale|language|hide|show|labels[ \\t]+on|!\\$|today[ \\t]+is|(?:mon|tues|wednes|thurs|fri|satur|sun)day[ \\t]+(?:is|are)|From[ \\t]+\\d{1,2}:\\d{2}[ \\t]+to[ \\t]+\\d{1,2}:\\d{2}|(?:\\d{4}/\\d{2}/\\d{2}|\\d{4}-\\d{2}-\\d{2}).*[ \\t](?:is|are)\\b|then[ \\t]+\\d+[ \\t]+days?[ \\t]+(?:is|are)[ \\t]+colored|\\d{4}-\\d{2}-\\d{2}[ \\t]+is)\\b".to_string()),
            ("note_open", r"(?i)\Anote[ \t]+\S+\z".to_string()),
            ("style_open", r"(?i)\A<style>\z".to_string()),
            ("note_close", r"(?i)\Aend[ \t]+note\z".to_string()),
            ("style_close", r"(?i)\A</style>\z".to_string()),
            ("on_brace", r"(?i)\Aon[ \t]+\{[^}]*\}".to_string()),
            ("displays", r"(?i)\Adisplays[ \t]+on[ \t]+same[ \t]+row[ \t]+as[ \t]+\[([^\]]+)\]".to_string()),
            ("occurs", r"(?i)\Aoccurs[ \t]+from[ \t]+\[([^\]]+)\][ \t]+to[ \t]+\[([^\]]+)\]".to_string()),
            ("duration", r"(?i)\A(?:requires|lasts)[ \t]+(\d+)[ \t]+((?:day|week)s?)(?:[ \t]+and[ \t]+(\d+)[ \t]+((?:day|week)s?))?".to_string()),
            ("follow", r"(?i)\Astarts(?:[ \t]+at|[ \t]+on)?[ \t]+\[([^\]]+)\]'s[ \t]+(start|end)".to_string()),
            ("happens_ref", r"(?i)\Ahappens(?:[ \t]+at)?[ \t]+\[([^\]]+)\]'s[ \t]+(start|end)".to_string()),
            ("happens_offset", format!("(?i)\\Ahappens[ \\t]+(\\d+)[ \\t]+((?:day|week)s?)(?:[ \\t]+and[ \\t]+(\\d+)[ \\t]+((?:day|week)s?))?[ \\t]+after[ \\t]+(?:\\[([^\\]]+)\\]'s[ \\t]+(start|end)|(start)\\z)")),
            ("starts_date", format!("(?i)\\Astarts(?:[ \\t]+at|[ \\t]+on)?[ \\t]+((?:the[ \\t]+)?(?:\\d{{4}}-\\d{{2}}-\\d{{2}}|{english}))(?:[ \\tT]\\d{{1,2}}:\\d{{2}}(?::\\d{{2}})?\\b)?")),
            ("ends_date", format!("(?i)\\Aends(?:[ \\t]+at|[ \\t]+on)?[ \\t]+((?:the[ \\t]+)?(?:\\d{{4}}-\\d{{2}}-\\d{{2}}|{english}))(?:[ \\tT]\\d{{1,2}}:\\d{{2}}(?::\\d{{2}})?\\b)?")),
            ("happens_date", format!("(?i)\\Ahappens(?:[ \\t]+at|[ \\t]+on)?[ \\t]+((?:the[ \\t]+)?(?:\\d{{4}}-\\d{{2}}-\\d{{2}}|{english}))(?:[ \\tT]\\d{{1,2}}:\\d{{2}}(?::\\d{{2}})?\\b)?")),
            ("complete", r"(?i)\Ais[ \t]+\d+%[ \t]+complete(?:d)?".to_string()),
            ("colored", r"(?i)\Ais[ \t]+colored[ \t]+in[ \t]+\S+".to_string()),
            ("time_only", r"(?i)\AT?\d{1,2}:\d{2}(?::\d{2})?\z".to_string()),
            ("iso_only", r"\A\d{4}-\d{2}-\d{2}\z".to_string()),
        ];
        entries
            .iter()
            .map(|(name, pattern)| (*name, Regex::new(pattern).expect("gantt reader pattern")))
            .collect()
    })
}

fn re(name: &str) -> &'static Regex {
    table().get(name).expect("gantt reader pattern")
}

/// The clause kinds, in the reader's CLAUSES order.
#[derive(Clone, Copy)]
enum Clause {
    Ignored,
    Occurs,
    Duration,
    Follow,
    HappensRef,
    HappensOffset,
    Start,
    Finish,
    Happens,
}

/// The first clause pattern matching `rest`, with its kind and match.
fn clause_match(rest: &str) -> Option<(Clause, regex::Match<'_>)> {
    let order: [(&str, Clause); 13] = [
        ("on_brace", Clause::Ignored),
        ("displays", Clause::Ignored),
        ("occurs", Clause::Occurs),
        ("duration", Clause::Duration),
        ("follow", Clause::Follow),
        ("happens_ref", Clause::HappensRef),
        ("happens_offset", Clause::HappensOffset),
        ("starts_date", Clause::Start),
        ("ends_date", Clause::Finish),
        ("happens_date", Clause::Happens),
        ("complete", Clause::Ignored),
        ("colored", Clause::Ignored),
        ("time_only", Clause::Ignored),
    ];
    order
        .iter()
        .find_map(|(name, kind)| re(name).find(rest).map(|found| (*kind, found)))
}

/// A gantt task as the reader fills it and the layout schedules it.
#[derive(Clone, Default)]
struct Task {
    description: String,
    id: String,
    start_date: Option<String>,
    duration: Option<String>,
    after_task: Option<String>,
    tags: Vec<String>,
    /// Scheduling results, in civil day numbers.
    calculated_start: Option<i64>,
    calculated_end: Option<i64>,
}

impl Task {
    fn milestone(&self) -> bool {
        self.tags.iter().any(|tag| tag == "milestone")
    }
}

struct Section {
    name: Option<String>,
    tasks: Vec<usize>,
}

struct Reader {
    arena: Vec<Task>,
    sections: Vec<Section>,
    tasks: HashMap<String, usize>,
    section: Option<usize>,
    last: Option<usize>,
    start: Option<String>,
    closing: Option<&'static str>,
    deferred: Vec<(usize, Clause, String)>,
}

impl Reader {
    fn new() -> Self {
        Reader {
            arena: vec![],
            sections: vec![],
            tasks: HashMap::new(),
            section: None,
            last: None,
            start: None,
            closing: None,
            deferred: vec![],
        }
    }

    /// Ruby `Reader#parse`: read every significant line, then the
    /// defaults and the deferred point clauses. Returns the sections in
    /// order, the task arena they index, and whether the source named
    /// the project's first day.
    fn parse(mut self, source: &str) -> Result<(Vec<Section>, Vec<Task>, bool), Error> {
        for (text, number) in numbered_lines(source) {
            self.read(&text, number)?;
        }
        let dated = self.start.is_some();
        self.finish();
        Ok((self.sections, self.arena, dated))
    }

    fn read(&mut self, text: &str, number: usize) -> Result<(), Error> {
        if let Some(closing) = self.closing {
            if re(closing).is_match(text) {
                self.closing = None;
            }
            return Ok(());
        }
        if re("style_open").is_match(text) {
            self.closing = Some("style_close");
            return Ok(());
        }
        if re("note_open").is_match(text) {
            self.closing = Some("note_close");
            return Ok(());
        }
        if re("skipped").is_match(text) {
            return Ok(());
        }
        if let Some(found) = re("project").captures(text) {
            self.start = Some(stamp(&found[1])?);
            return Ok(());
        }
        if let Some(found) = re("section").captures(text) {
            self.open_section(Some(found[1].to_string()));
            return Ok(());
        }
        self.read_task(text, number)
    }

    fn read_task(&mut self, text: &str, number: usize) -> Result<(), Error> {
        if let Some(found) = re("arrow").captures(text) {
            // `[A] -> [B]`: B follows A. The lookups run in the Ruby
            // order, so a first mention on either side creates there.
            let from = self.lookup(&found[1], None);
            let to = self.lookup(&found[2], None);
            self.add_predecessor(to, from);
            return Ok(());
        }
        if let Some(found) = re("task").captures(text) {
            self.task(&found)?;
            return Ok(());
        }
        Err(refused(number, text))
    }

    fn open_section(&mut self, name: Option<String>) {
        self.sections.push(Section {
            name,
            tasks: vec![],
        });
        self.section = Some(self.sections.len() - 1);
    }

    fn task(&mut self, found: &regex::Captures<'_>) -> Result<(), Error> {
        let then_ = found.get(1).is_some();
        let name = &found[2];
        let alias = found.get(3).map(|m| m.as_str());
        let rest = found.get(4).map(|m| m.as_str()).unwrap_or("");
        let current = self.lookup(name, alias);
        if then_ {
            if let Some(last) = self.last {
                let id = self.arena[last].id.clone();
                self.arena[current].after_task = Some(id);
            }
        }
        self.clauses(current, rest, name)?;
        self.last = Some(current);
        Ok(())
    }

    fn clauses(&mut self, task: usize, rest: &str, name: &str) -> Result<(), Error> {
        let mut rest = rest.to_string();
        while !rest.is_empty() {
            let Some((kind, found)) = clause_match(&rest) else {
                return Err(unsupported_clause(name, &rest));
            };
            self.apply(task, kind, &rest)?;
            rest = strip_clause_gap(&rest[found.end()..]).to_string();
        }
        Ok(())
    }

    fn apply(&mut self, task: usize, kind: Clause, rest: &str) -> Result<(), Error> {
        match kind {
            Clause::Ignored => Ok(()),
            Clause::Duration => {
                let found = re("duration").captures(rest).unwrap();
                let days = span_days(&found[1], &found[2])
                    + found
                        .get(3)
                        .zip(found.get(4))
                        .map(|(count, unit)| span_days(count.as_str(), unit.as_str()))
                        .unwrap_or(0);
                self.arena[task].duration = Some(format!("{days}d"));
                Ok(())
            }
            Clause::Follow => {
                let found = re("follow").captures(rest).unwrap();
                if !matches!(&found[2], "start" | "end") {
                    return Err(unsupported_clause(
                        &self.arena[task].description,
                        &format!("starts at [{}]'s {}", &found[1], &found[2]),
                    ));
                }
                let point = self.lookup(&found[1], None);
                self.add_predecessor(task, point);
                Ok(())
            }
            Clause::HappensRef => {
                let found = re("happens_ref").captures(rest).unwrap();
                let point = self.lookup(&found[1], None);
                let point_start = self.arena[point].start_date.clone();
                match point_start {
                    Some(start) => {
                        let span = self.arena[point].duration.clone().unwrap_or_default();
                        let date = shift_days(&start, parse_days(&span))?;
                        self.happen(task, date);
                    }
                    None => self.deferred.push((task, kind, rest.to_string())),
                }
                Ok(())
            }
            Clause::HappensOffset => {
                let found = re("happens_offset").captures(rest).unwrap();
                match self.offset_date(&found)? {
                    Some(date) => {
                        self.arena[task].start_date = Some(date);
                        self.arena[task].duration = Some("0d".to_string());
                        self.arena[task].tags.push("milestone".to_string());
                    }
                    None => self.deferred.push((task, kind, rest.to_string())),
                }
                Ok(())
            }
            Clause::Start => {
                let found = re("starts_date").captures(rest).unwrap();
                self.arena[task].start_date = Some(stamp(&found[1])?);
                Ok(())
            }
            Clause::Finish => {
                let found = re("ends_date").captures(rest).unwrap();
                self.finish_on(task, stamp(&found[1])?);
                Ok(())
            }
            Clause::Happens => {
                let found = re("happens_date").captures(rest).unwrap();
                self.happen(task, stamp(&found[1])?);
                Ok(())
            }
            Clause::Occurs => {
                let found = re("occurs").captures(rest).unwrap();
                let from = self.lookup(&found[1], None);
                let to = self.lookup(&found[2], None);
                let from_start = self.arena[from].start_date.clone();
                let to_start = self.arena[to].start_date.clone();
                match (from_start, to_start) {
                    (Some(from_date), Some(to_date)) => {
                        let from_day =
                            parse_iso(&from_date).ok_or_else(|| bad_stamp(&from_date))?;
                        let to_day = parse_iso(&to_date).ok_or_else(|| bad_stamp(&to_date))?;
                        self.arena[task].start_date = Some(from_date);
                        // Ruby interpolates the Rational a Date
                        // subtraction yields: `"10/1d"`, never `"10d"`.
                        let span = to_day - from_day;
                        self.arena[task].duration = Some(if span > 0 {
                            format!("{span}/1d")
                        } else {
                            "0d".to_string()
                        });
                    }
                    _ => self.deferred.push((task, kind, rest.to_string())),
                }
                Ok(())
            }
        }
    }

    /// Ruby `Reader#offset_date`: the date an `after` offset points at,
    /// from the project start or from a task's start or end.
    fn offset_date(&mut self, found: &regex::Captures<'_>) -> Result<Option<String>, Error> {
        let offset = span_days(&found[1], &found[2])
            + found
                .get(3)
                .zip(found.get(4))
                .map(|(count, unit)| span_days(count.as_str(), unit.as_str()))
                .unwrap_or(0);
        match found.get(5).map(|m| m.as_str()) {
            None => match &self.start {
                Some(start) => shift_days(start, offset).map(Some),
                None => Ok(None),
            },
            Some(name) => {
                let point = self.lookup(name, None);
                let Some(point_start) = self.arena[point].start_date.clone() else {
                    return Ok(None);
                };
                let point_days = parse_iso(&point_start).ok_or_else(|| bad_stamp(&point_start))?;
                let at_end = found.get(6).map(|m| m.as_str()) == Some("end");
                let base = if at_end {
                    point_days + parse_days(&self.arena[point].duration.clone().unwrap_or_default())
                } else {
                    point_days
                };
                Ok(Some(iso_of(base + offset)))
            }
        }
    }

    fn finish_on(&mut self, task: usize, date: String) {
        let from = self.arena[task]
            .start_date
            .clone()
            .or_else(|| self.start.clone())
            .unwrap_or_else(|| ANCHOR.to_string());
        let from_day = parse_iso(&from).unwrap_or(0);
        let to_day = parse_iso(&date).unwrap_or(0);
        self.arena[task].duration = Some(format!("{}d", (to_day - from_day).max(0)));
    }

    fn happen(&mut self, task: usize, date: String) {
        self.arena[task].start_date = Some(date);
        self.arena[task].duration = Some("0d".to_string());
        self.arena[task].tags.push("milestone".to_string());
    }

    fn add_predecessor(&mut self, task: usize, predecessor: usize) {
        let pred_id = self.arena[predecessor].id.clone();
        let joined = match self.arena[task].after_task.clone() {
            Some(existing) => format!("{existing} {pred_id}"),
            None => pred_id,
        };
        self.arena[task].after_task = Some(joined);
    }

    /// Ruby `Reader#lookup`: tasks are created on first mention, in a
    /// section made on demand.
    fn lookup(&mut self, name: &str, alias: Option<&str>) -> usize {
        let found = self
            .tasks
            .get(name)
            .copied()
            .or_else(|| alias.and_then(|a| self.tasks.get(a).copied()));
        let index = found.unwrap_or_else(|| self.create(name));
        if let Some(alias) = alias {
            self.tasks.insert(alias.to_string(), index);
        }
        index
    }

    fn create(&mut self, name: &str) -> usize {
        if self.section.is_none() {
            self.open_section(None);
        }
        let id = format!("task{}", self.distinct_count() + 1);
        let index = self.arena.len();
        self.arena.push(Task {
            description: name.to_string(),
            id,
            start_date: None,
            duration: Some("1d".to_string()),
            after_task: None,
            tags: vec![],
            calculated_start: None,
            calculated_end: None,
        });
        self.tasks.insert(name.to_string(), index);
        let section = self.section.unwrap();
        self.sections[section].tasks.push(index);
        index
    }

    /// `@tasks.values.uniq.length`: the distinct tasks the name map holds.
    fn distinct_count(&self) -> usize {
        let mut seen: Vec<usize> = self.tasks.values().copied().collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    /// Ruby `Reader#finish`: the defaults first, then the point clauses
    /// that referenced a dateless task get their best-effort resolution.
    /// The Ruby loop re-defers an unresolved clause while it iterates;
    /// here the queue drains once, which reaches the same settled state
    /// without the pathological re-defer cycle.
    fn finish(&mut self) {
        let first = self.start.clone().unwrap_or_else(|| ANCHOR.to_string());
        let count = self.arena.len();
        for index in 0..count {
            if self.arena[index].start_date.is_none() && self.arena[index].after_task.is_none() {
                self.arena[index].start_date = Some(first.clone());
            }
        }
        let deferred = std::mem::take(&mut self.deferred);
        for (task, kind, rest) in deferred {
            let _ = self.apply(task, kind, &rest);
        }
    }
}

/// Ruby `Reader#numbered_lines` + `lines`: stripped lines with their
/// numbers, from the line after `@start`, up to `@end`, blanks and
/// quote comments dropped.
fn numbered_lines(source: &str) -> Vec<(String, usize)> {
    let numbered: Vec<(String, usize)> = source
        .lines()
        .enumerate()
        .map(|(index, line)| (line.trim().to_string(), index + 1))
        .collect();
    let first = numbered
        .iter()
        .position(|(text, _)| text.starts_with("@start"));
    let body: Vec<(String, usize)> = match first {
        Some(at) => numbered.into_iter().skip(at + 1).collect(),
        None => numbered,
    };
    body.into_iter()
        .take_while(|(text, _)| !text.starts_with("@end"))
        .filter(|(text, _)| !text.is_empty() && !text.starts_with('\''))
        .collect()
}

/// Ruby's `rest.sub(/\A[ \t]+(?:and[ \t]+)?/i, "")` after a clause: the
/// blank run is required, the `and` slot optional, another blank run
/// after it.
fn strip_clause_gap(after: &str) -> &str {
    if !after.starts_with([' ', '\t']) {
        return after;
    }
    let after = after.trim_start_matches([' ', '\t']);
    if after.len() >= 3 && after[..3].eq_ignore_ascii_case("and") {
        let raw_tail = &after[3..];
        if raw_tail.starts_with([' ', '\t']) {
            return raw_tail.trim_start_matches([' ', '\t']);
        }
    }
    after
}

fn span_days(count: &str, unit: &str) -> i64 {
    let value: i64 = count.parse().unwrap_or(0);
    if unit.to_lowercase().starts_with('w') {
        value * 7
    } else {
        value
    }
}

/// `GanttDuration.parse` + `add_value` in whole days, the only units the
/// reader emits (`"3d"`, `"2w"`). A duration the pattern does not read
/// (`"10/1d"`, the Rational a Date subtraction interpolates to) adds
/// nothing, exactly as `GanttDuration.add` returns the time unchanged.
fn parse_days(duration: &str) -> i64 {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"\A(\d+(?:\.\d+)?)(ms|[Mdhmswy])?\z").expect("duration pattern")
    });
    let Some(found) = pattern.captures(duration.trim()) else {
        return 0;
    };
    let value: f64 = found[1].parse().unwrap_or(0.0);
    match found.get(2).map(|m| m.as_str()) {
        Some("w") => (value * 7.0).round() as i64,
        Some("d") | None => value.round() as i64,
        Some("h") => (value * 3600.0 / 86400.0) as i64,
        Some("m") => (value * 60.0 / 86400.0) as i64,
        Some("s") => (value / 86400.0) as i64,
        Some("ms") => (value * 0.001 / 86400.0) as i64,
        // `M` and `y` move the calendar by whole months; the reader
        // never emits them, and a plain day floor keeps them harmless.
        Some("M") => (value * 30.0) as i64,
        Some("y") => (value * 365.0) as i64,
        _ => 0,
    }
}

/// Ruby `Reader#stamp`: the ISO date a captured stamp holds: `2020-10-15`
/// or `29th of September 2018` (a year-less english stamp anchors to the
/// reader's `ANCHOR` year).
fn stamp(text: &str) -> Result<String, Error> {
    if re("iso_only").is_match(text) {
        return Ok(text.trim().to_string());
    }
    let Some(english) = re("english").captures(text) else {
        return Err(bad_stamp(text));
    };
    let month = month_index(&english[2]).ok_or_else(|| bad_stamp(text))?;
    let year: i64 = match english.get(3) {
        Some(year) => year.as_str().parse().map_err(|_| bad_stamp(text))?,
        None => 2000,
    };
    let day: i64 = english[1].parse().map_err(|_| bad_stamp(text))?;
    let days = civil_to_days(year, month, day).ok_or_else(|| bad_stamp(text))?;
    Ok(iso_of(days))
}

fn bad_stamp(text: &str) -> Error {
    Error::Parse {
        class: "ArgumentError".into(),
        message: format!("invalid date: {text}"),
        line: 0,
        col: 0,
    }
}

fn unsupported_clause(name: &str, rest: &str) -> Error {
    Error::Parse {
        class: "UnsupportedConstructError".into(),
        message: format!("unsupported construct: gantt clause: [{name}] {rest}"),
        line: 0,
        col: 0,
    }
}

fn month_index(name: &str) -> Option<i64> {
    let months = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    months
        .iter()
        .position(|month| *month == name.to_lowercase())
        .map(|at| at as i64 + 1)
}

/// Ruby `Date.iso8601` parsing, in civil day numbers.
fn parse_iso(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    civil_to_days(year, month, day)
}

fn iso_of(days: i64) -> String {
    let (year, month, day) = days_to_civil(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn shift_days(date: &str, days: i64) -> Result<String, Error> {
    let base = parse_iso(date).ok_or_else(|| bad_stamp(date))?;
    Ok(iso_of(base + days))
}

/// Howard Hinnant's `days_from_civil`, with Ruby `Date.new` validation.
fn civil_to_days(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let lengths = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=lengths[(month - 1) as usize]).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn days_to_civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

fn refused(line: usize, text: &str) -> Error {
    Error::Parse {
        class: "UnsupportedConstructError".into(),
        message: format!("unsupported construct: gantt statement at line {line}: {text}"),
        line,
        col: 0,
    }
}

// ---- scheduling (layout/gantt.rb, the core the PlantUML scene reuses) ----

/// `calculate_task_dates` plus `resolve_dependencies` to a fixed point.
/// The PlantUML reader never sets `end_date`, `until_task`, `excludes`
/// or `weekend`, so the exclusions settle is the identity and an end is
/// always the start plus its duration.
fn calculate_dates(arena: &mut [Task], order: &[usize]) {
    for &index in order {
        let (start, duration, after) = {
            let task = &arena[index];
            (
                task.start_date.clone(),
                task.duration.clone(),
                task.after_task.is_some(),
            )
        };
        if let (Some(start), false) = (start, after) {
            let Some(start_day) = parse_iso(&start) else {
                continue;
            };
            arena[index].calculated_start = Some(start_day);
            if let Some(duration) = duration {
                arena[index].calculated_end = Some(start_day + parse_days(&duration));
            }
        }
    }
    for _ in 0..100 {
        if !resolve_pass(arena, order) {
            break;
        }
    }
}

fn resolve_pass(arena: &mut [Task], order: &[usize]) -> bool {
    let mut changed = false;
    for (index, &task) in order.iter().enumerate() {
        changed |= resolve_unsettled(arena, order, index, task);
    }
    changed
}

fn resolve_unsettled(arena: &mut [Task], order: &[usize], index: usize, task: usize) -> bool {
    if arena[task].calculated_start.is_some() {
        return false;
    }
    if arena[task].after_task.is_some() {
        return resolve_after(arena, task);
    }
    if arena[task].duration.is_none() || index == 0 {
        return false;
    }
    // An implicit duration starts at the previous task's end, by
    // position, not by id.
    let previous = order[index - 1];
    let Some(previous_end) = arena[previous].calculated_end else {
        return false;
    };
    arena[task].calculated_start = Some(previous_end);
    let duration = arena[task].duration.clone();
    if let Some(duration) = duration {
        arena[task].calculated_end = Some(previous_end + parse_days(&duration));
    }
    true
}

/// `"after a c"` starts the task after the latest of the referenced
/// ends; nil while any reference is unknown or unresolved, so the
/// caller's fixed-point loop tries again.
fn resolve_after(arena: &mut [Task], task: usize) -> bool {
    let after = arena[task].after_task.clone().unwrap_or_default();
    let by_id: HashMap<&str, usize> = arena
        .iter()
        .enumerate()
        .map(|(index, task)| (task.id.as_str(), index))
        .collect();
    let mut latest: Option<i64> = None;
    for id in after.split_whitespace() {
        let Some(&index) = by_id.get(id) else {
            return false;
        };
        let Some(end) = arena[index].calculated_end else {
            return false;
        };
        latest = Some(match latest {
            Some(best) if best >= end => best,
            _ => end,
        });
    }
    let Some(start) = latest else { return false };
    arena[task].calculated_start = Some(start);
    let duration = arena[task].duration.clone();
    if let Some(duration) = duration {
        arena[task].calculated_end = Some(start + parse_days(&duration));
    }
    true
}

// ---- scene (notation/plantuml/gantt/layout.rb) ----

const HEADER: f64 = 16.0;
const BAR: f64 = 12.955;
const REACH: f64 = BAR / 2.0;
const ROW: f64 = BAR + 4.0;
const SECTION_ROW: f64 = ROW + 16.0;
const DAY: f64 = 16.0;
const CELL_PAD: f64 = 10.0;
const TABLE_FONT: f64 = 10.0;
const TASK_FONT: f64 = 11.0;
const COLUMNS: [&str; 3] = ["Start", "End", "Duration"];

/// Ruby `Array#sum`: Kahan-Babuska compensation, elements in order.
fn ruby_sum(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut compensation = 0.0;
    for &value in values {
        let next = total + value;
        if total.abs() >= value.abs() {
            compensation += (total - next) + value;
        } else {
            compensation += (value - next) + total;
        }
        total = next;
    }
    total + compensation
}

fn measure(text: &str, size: f64) -> f64 {
    crate::text::measure(text, size, Face::Arial).width
}

struct Label {
    text: String,
    x: f64,
    y: f64,
    anchor: &'static str,
    size: f64,
}

struct Line {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct Scene {
    width: f64,
    height: f64,
    view_box: String,
    grid_lines: Vec<Line>,
    rules: Vec<Line>,
    bars: Vec<Rect>,
    diamonds: Vec<String>,
    texts: Vec<Label>,
}

/// One row of the frame: a section head or a task row.
enum Row {
    Section(String),
    Task(usize),
}

fn first_day(arena: &[Task], task: usize, anchor: i64) -> Result<i64, Error> {
    let start = arena[task]
        .calculated_start
        .ok_or_else(|| Error::Unported("gantt".into()))?;
    Ok(start - anchor + 1)
}

fn last_day(arena: &[Task], task: usize, anchor: i64) -> Result<i64, Error> {
    if arena[task].milestone() {
        return first_day(arena, task, anchor);
    }
    let end = arena[task]
        .calculated_end
        .ok_or_else(|| Error::Unported("gantt".into()))?;
    Ok((end - anchor).max(first_day(arena, task, anchor)?))
}

/// Ruby `Layout#scene`: the day grid, the dated table and the bars.
fn scene(arena: &mut [Task], sections: &[Section], dated: bool) -> Result<Scene, Error> {
    let mut rows: Vec<Row> = vec![];
    for section in sections {
        if let Some(name) = &section.name {
            if !name.is_empty() {
                rows.push(Row::Section(name.clone()));
            }
        }
        for task in &section.tasks {
            rows.push(Row::Task(*task));
        }
    }
    let task_rows: Vec<usize> = rows
        .iter()
        .filter_map(|row| match row {
            Row::Task(task) => Some(*task),
            Row::Section(_) => None,
        })
        .collect();

    let anchor = task_rows
        .iter()
        .filter_map(|task| arena[*task].calculated_start)
        .min()
        .ok_or_else(|| Error::Unported("gantt".into()))?;
    let mut days = 1i64;
    for task in &task_rows {
        days = days.max(last_day(arena, *task, anchor)?);
    }

    let mut cells: Vec<[String; 3]> = vec![];
    for task in &task_rows {
        let first = first_day(arena, *task, anchor)?;
        let last = last_day(arena, *task, anchor)?;
        let milestone = arena[*task].milestone();
        let length = if milestone { 0 } else { last - first + 1 };
        let end = arena[*task]
            .calculated_end
            .ok_or_else(|| Error::Unported("gantt".into()))?;
        let boundary = |dated_value: i64, day: i64| -> String {
            if dated {
                iso_of(dated_value)
            } else {
                format!("Day {day}")
            }
        };
        let start_day_value = arena[*task].calculated_start.unwrap();
        cells.push([
            boundary(start_day_value, first),
            boundary(if milestone { end } else { end - 1 }, last),
            format!("{length} {}", if length == 1 { "day" } else { "days" }),
        ]);
    }

    let widths: Vec<f64> = if dated {
        COLUMNS
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let mut contents = vec![title.to_string()];
                contents.extend(cells.iter().map(|cells| cells[index].clone()));
                let widest = contents
                    .iter()
                    .map(|content| measure(content, TABLE_FONT))
                    .fold(f64::NEG_INFINITY, f64::max);
                widest + CELL_PAD
            })
            .collect()
    } else {
        vec![]
    };
    let origin = ruby_sum(&widths);

    let mut body = HEADER;
    let mut grid_lines: Vec<Line> = vec![];
    let mut rules: Vec<Line> = vec![];
    let mut bars: Vec<Rect> = vec![];
    let mut diamonds: Vec<String> = vec![];
    let mut texts: Vec<Label> = vec![];

    // Ruby `header_texts`: `widths.each_index.map` — the column headers
    // exist only when the dated table does.
    for (index, title) in COLUMNS.iter().enumerate().take(widths.len()) {
        texts.push(Label {
            text: title.to_string(),
            x: ruby_sum(&widths[..index]) + 5.0,
            y: 11.779,
            anchor: "start",
            size: TABLE_FONT,
        });
    }
    let day_labels = |texts: &mut Vec<Label>, y_position: f64| {
        for day in 1..=days {
            let measured = measure(&day.to_string(), TABLE_FONT);
            let left = origin + (((day - 1) as f64) * DAY) + ((DAY - measured) / 2.0);
            texts.push(Label {
                text: day.to_string(),
                x: left,
                y: y_position,
                anchor: "start",
                size: TABLE_FONT,
            });
        }
    };
    day_labels(&mut texts, 9.668);

    let mut cell_iter = cells.iter();
    for row in &rows {
        match row {
            Row::Section(name) => {
                let middle = body + SECTION_ROW / 2.0;
                rules.push(Line {
                    x1: origin,
                    y1: middle,
                    x2: origin + 5.0,
                    y2: middle,
                });
                texts.push(Label {
                    text: name.clone(),
                    x: origin + 10.0,
                    y: middle + 4.157,
                    anchor: "start",
                    size: TASK_FONT,
                });
                let label_width = measure(name, TASK_FONT);
                rules.push(Line {
                    x1: origin + 15.0 + label_width,
                    y1: middle,
                    x2: origin + ((days as f64) * DAY) - 1.0,
                    y2: middle,
                });
                body += SECTION_ROW;
            }
            Row::Task(task) => {
                let cells = cell_iter.next().unwrap();
                if !widths.is_empty() {
                    for (index, cell) in cells.iter().enumerate() {
                        texts.push(Label {
                            text: cell.clone(),
                            x: ruby_sum(&widths[..index]) + 5.0,
                            y: body + 12.257,
                            anchor: "start",
                            size: TABLE_FONT,
                        });
                    }
                }
                let bar_y = body + 2.0;
                let task_ref = &arena[*task];
                let first = first_day(arena, *task, anchor)?;
                if task_ref.milestone() {
                    let centre = origin + (((first - 1) as f64) * DAY) + 2.0;
                    let middle = bar_y + REACH;
                    diamonds.push(
                        [
                            (centre - REACH, middle),
                            (centre, middle - REACH),
                            (centre + REACH, middle),
                            (centre, middle + REACH),
                        ]
                        .iter()
                        .map(|(x, y)| format!("{},{}", ruby_float(*x), ruby_float(*y)))
                        .collect::<Vec<_>>()
                        .join(" "),
                    );
                    texts.push(Label {
                        text: task_ref.description.clone(),
                        x: centre + REACH + 4.0,
                        y: bar_y + 10.635,
                        anchor: "start",
                        size: TASK_FONT,
                    });
                } else {
                    let last = last_day(arena, *task, anchor)?;
                    let left = origin + (((first - 1) as f64) * DAY) + 2.0;
                    let width = (((last - first + 1) as f64) * DAY) - 4.0;
                    bars.push(Rect {
                        x: left,
                        y: bar_y,
                        width,
                        height: BAR,
                    });
                    // The task's name sits inside its bar when it fits,
                    // after it when it does not.
                    let fits = measure(&task_ref.description, TASK_FONT) + 8.0 <= width;
                    let caption_x = if fits { left + 4.0 } else { left + width + 4.0 };
                    texts.push(Label {
                        text: task_ref.description.clone(),
                        x: caption_x,
                        y: bar_y + 10.635,
                        anchor: "start",
                        size: TASK_FONT,
                    });
                }
                body += ROW;
            }
        }
    }

    let width = origin + ((days as f64) * DAY) + 21.0;
    let height = body + 16.36;
    if origin != 0.0 {
        for y_position in [0.0, HEADER, body] {
            grid_lines.push(Line {
                x1: 0.0,
                y1: y_position,
                x2: origin,
                y2: y_position,
            });
        }
    }
    if !widths.is_empty() {
        let mut edges = vec![0.0];
        for index in 0..widths.len() {
            edges.push(ruby_sum(&widths[..=index]));
        }
        for x_position in edges {
            grid_lines.push(Line {
                x1: x_position,
                y1: 0.0,
                x2: x_position,
                y2: body,
            });
        }
    }
    for day in 0..=days {
        let x = origin + ((day as f64) * DAY);
        grid_lines.push(Line {
            x1: x,
            y1: 6.0,
            x2: x,
            y2: body + 8.0,
        });
    }
    day_labels(&mut texts, body + 12.668);

    Ok(Scene {
        width,
        height,
        view_box: format!(
            "0 0 {} {}",
            ruby_float(round3(width)),
            ruby_float(round3(height))
        ),
        grid_lines,
        rules,
        bars,
        diamonds,
        texts,
    })
}

/// Ruby `Float#round(3)`.
fn round3(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let binexp = crate::js_math::binexp(x);
    let limit = if binexp > 0 {
        binexp / 4
    } else {
        binexp / 3 - 1
    };
    if 3 >= 15 - limit {
        return x;
    }
    let under = if binexp > 0 {
        binexp / 3 + 1
    } else {
        binexp / 4
    };
    if 3 < -under {
        return 0.0;
    }
    let s = 1000.0f64;
    let mut f = (x * s).round();
    if x > 0.0 {
        if (f + 0.5) / s <= x {
            f += 1.0;
        }
    } else if (f - 0.5) / s >= x {
        f -= 1.0;
    }
    f / s
}

// ---- renderer (notation/plantuml/gantt/renderer.rb) ----

const GRID: &str = "#C0C0C0";
const INK: &str = "#181818";
const BAR_FILL: &str = "#E2E2F0";

/// Ruby `Renderer::Base#document_dimensions`: the layout's own rounded
/// pair when it matches the scene size, the exact size otherwise.
fn view_box_for(scene: &Scene) -> String {
    let existing: Vec<&str> = scene.view_box.split_whitespace().collect();
    let last_two: Vec<&str> = existing.iter().rev().take(2).rev().copied().collect();
    if last_two.len() == 2 {
        if let (Some(a), Some(b)) = (
            last_two[0].parse::<f64>().ok(),
            last_two[1].parse::<f64>().ok(),
        ) {
            if a == scene.width && b == scene.height {
                return format!("0 0 {} {}", last_two[0], last_two[1]);
            }
        }
    }
    let dimension = |value: f64| -> String {
        if value.trunc() == value {
            format!("{}", value as i64)
        } else {
            ruby_float(value)
        }
    };
    format!("0 0 {} {}", dimension(scene.width), dimension(scene.height))
}

fn text_element(label: &Label, fill: &str) -> Elem {
    Elem::new("text")
        .attr("fill", fill)
        .num("x", label.x)
        .num("y", label.y)
        .attr("text-anchor", label.anchor)
        .attr("font-family", "sans-serif")
        .attr("font-size", ruby_float(label.size))
        .content(&label.text)
}

pub fn render(source: &str) -> Result<String, Error> {
    let (sections, mut arena, dated) = Reader::new().parse(source)?;
    let order: Vec<usize> = sections
        .iter()
        .flat_map(|section| section.tasks.iter().copied())
        .collect();
    calculate_dates(&mut arena, &order);
    let scene = scene(&mut arena, &sections, dated)?;

    let theme = crate::svg::Theme::default_theme();
    let fill = theme.color("label_text").unwrap_or("").to_string();

    let mut doc = Document::new(scene.width, scene.height);
    doc.view_box = Some(view_box_for(&scene));
    for line in &scene.grid_lines {
        doc.add(
            Elem::new("line")
                .attr("stroke", GRID)
                .attr("stroke-width", "1")
                .num("x1", line.x1)
                .num("y1", line.y1)
                .num("x2", line.x2)
                .num("y2", line.y2),
        );
    }
    for bar in &scene.bars {
        doc.add(
            Elem::new("rect")
                .attr("fill", BAR_FILL)
                .attr("stroke", INK)
                .attr("stroke-width", "1")
                .num("x", bar.x)
                .num("y", bar.y)
                .num("width", bar.width)
                .num("height", bar.height),
        );
    }
    for points in &scene.diamonds {
        doc.add(
            Elem::new("polygon")
                .attr("fill", INK)
                .attr("stroke", INK)
                .attr("stroke-width", "1")
                .attr("points", points.clone()),
        );
    }
    for line in &scene.rules {
        doc.add(
            Elem::new("line")
                .attr("stroke", INK)
                .attr("stroke-width", "1")
                .num("x1", line.x1)
                .num("y1", line.y1)
                .num("x2", line.x2)
                .num("y2", line.y2),
        );
    }
    for label in &scene.texts {
        doc.add(text_element(label, &fill));
    }
    Ok(doc.to_xml())
}
