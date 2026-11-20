//! Splits PlantUML source into significant lines and expands `!define`
//! macros. Port of `node_graph/{reader,macros}.rb`.

use regex::Regex;
use std::sync::OnceLock;

/// `(text, 1-based line number)` of the first diagram: after `@start`,
/// before `@end`, blanks and `'` comments dropped, an open quote or
/// bracket merged with the following line.
pub fn entries(source: &str) -> Vec<(String, usize)> {
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
    let kept: Vec<(String, usize)> = body
        .into_iter()
        .take_while(|(text, _)| !text.starts_with("@end"))
        .filter(|(text, _)| !text.is_empty() && !text.starts_with('\''))
        .collect();
    expand(merge(kept))
}

/// While a line leaves a quote or bracket open, the next line joins it.
fn merge(entries: Vec<(String, usize)>) -> Vec<(String, usize)> {
    let mut queue: std::collections::VecDeque<(String, usize)> = entries.into();
    let mut result = vec![];
    while let Some((mut text, number)) = queue.pop_front() {
        while open(&text) && !queue.is_empty() {
            let (next, _) = queue.pop_front().unwrap();
            text = format!("{text}\n{next}");
        }
        result.push((text, number));
    }
    result
}

fn open(text: &str) -> bool {
    static OPEN_QUOTE: OnceLock<Regex> = OnceLock::new();
    static OPEN_BRACKET: OnceLock<Regex> = OnceLock::new();
    let open_quote =
        OPEN_QUOTE.get_or_init(|| Regex::new(r#"\A[[:alpha:]]+\s.*"[^"]*\z"#).unwrap());
    let open_bracket =
        OPEN_BRACKET.get_or_init(|| Regex::new(r"\A[[:alpha:]]+\s+\S+\s+\[[^\]]*\z").unwrap());
    open_bracket.is_match(text)
        || (open_quote.is_match(text) && text.matches('"').count() % 2 == 1)
}

/// `!define NAME body`: each later whole-word `NAME` becomes `body`. A
/// macro with parameters is left alone.
pub fn expand(entries: Vec<(String, usize)>) -> Vec<(String, usize)> {
    static DEFINE: OnceLock<Regex> = OnceLock::new();
    let define = DEFINE.get_or_init(|| Regex::new(r"\A!define\s+(?P<name>\w+)(?:\s+(?P<body>.*))?\z").unwrap());
    let mut table: Vec<(String, String)> = vec![];
    let mut result = vec![];
    for (text, number) in entries {
        if let Some(captures) = define.captures(&text) {
            let name = captures["name"].to_string();
            let body = captures.name("body").map(|m| m.as_str().to_string()).unwrap_or_default();
            let body = substitute(&body, &table);
            table.retain(|(existing, _)| *existing != name);
            table.push((name, body));
            continue;
        }
        result.push((substitute(&text, &table), number));
    }
    result
}

fn substitute(text: &str, table: &[(String, String)]) -> String {
    let mut line = text.to_string();
    for (name, body) in table {
        line = replace_whole_word(&line, name, body);
    }
    line
}

/// `line.gsub(/(?<!\w)NAME(?!\w)/) { body }` — every whole-word `NAME`.
fn replace_whole_word(line: &str, name: &str, body: &str) -> String {
    if name.is_empty() {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    while at < line.len() {
        if line[at..].starts_with(name) {
            let before_ok = at == 0 || {
                let previous = line[..at].chars().next_back().unwrap();
                !(previous.is_alphanumeric() || previous == '_')
            };
            let after_at = at + name.len();
            let after_ok = after_at >= line.len() || {
                let next = line[after_at..].chars().next().unwrap();
                !(next.is_alphanumeric() || next == '_')
            };
            if before_ok && after_ok {
                out.push_str(body);
                at = after_at;
                continue;
            }
        }
        let ch = line[at..].chars().next().unwrap();
        out.push(ch);
        at += ch.len_utf8();
    }
    let _ = bytes;
    out
}
