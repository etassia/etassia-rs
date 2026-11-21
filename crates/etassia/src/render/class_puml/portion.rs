//! Port of Ruby `Notation::PlantUML::PortionVisibility`: what the `hide`
//! and `show` lines take out of a class.
use super::model::{Klass, MemberKind};

pub struct PortionVisibility {
    rules: Vec<Rule>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Portion {
    Field,
    Method,
    Stereotype,
}

#[derive(Clone, Debug)]
struct Rule {
    show: bool,
    target: Option<String>,
    portions: &'static [Portion],
}

const MEMBERS: [Portion; 2] = [Portion::Field, Portion::Method];
const FIELDS: [Portion; 1] = [Portion::Field];
const METHODS: [Portion; 1] = [Portion::Method];
const STEREOTYPES: [Portion; 1] = [Portion::Stereotype];

impl PortionVisibility {
    /// Ruby `RULE` over the diagram's directive lines.
    pub fn new(directives: &[String]) -> PortionVisibility {
        let mut rules = vec![];
        for line in directives {
            let lowered = line.to_lowercase();
            let Some(first) = lowered.split_whitespace().next() else {
                continue;
            };
            let show = first == "show";
            if first != "hide" && first != "show" {
                continue;
            }
            // The target keeps its written case; the words read lowercased.
            let written: Vec<&str> = line.split_whitespace().collect();
            let mut words: Vec<&str> =
                lowered.split_whitespace().skip(1).collect();
            if words.first() == Some(&"empty") {
                continue;
            }
            let last = match words.pop() {
                Some(word) => word,
                None => continue,
            };
            let portions: &[Portion] = match last {
                "members" => &MEMBERS,
                "attributes" => &FIELDS,
                "fields" => &FIELDS,
                "methods" => &METHODS,
                "stereotype" => &STEREOTYPES,
                _ => continue,
            };
            // Ruby's RULE: the optional target is exactly one word: a name
            // or `<<stereotype>>` (one token, no whitespace inside).
            let target = match words.len() {
                0 => None,
                1 => {
                    // Ruby RULE keeps the target's written case.
                    let word_lower = words[0];
                    let Some(word) = written.get(1) else { continue };
                    if word_lower.starts_with("<<") && word_lower.ends_with(">>") {
                        Some(word.to_string())
                    } else if word.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        Some(word.to_string())
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };
            rules.push(Rule { show, target, portions });
        }
        PortionVisibility { rules }
    }

    /// Ruby `apply`: `klass` without its hidden members and stereotype
    /// labels; `klass` itself when nothing is hidden.
    pub fn apply(&self, klass: Klass) -> Klass {
        if self.rules.is_empty() {
            return klass;
        }
        let kept: Vec<bool> = klass
            .body
            .iter()
            .map(|member| self.member_shown(&klass, member.kind))
            .collect();
        let shown_stereotypes: Vec<bool> = klass
            .stereotypes
            .iter()
            .map(|label| self.stereotype_shown(&klass, label))
            .collect();
        let mut applied = klass;
        applied.body = applied
            .body
            .into_iter()
            .zip(kept)
            .filter_map(|(member, shown)| shown.then_some(member))
            .collect();
        applied.stereotypes = applied
            .stereotypes
            .into_iter()
            .zip(shown_stereotypes)
            .filter_map(|(label, shown)| shown.then_some(label))
            .collect();
        applied
    }

    fn member_shown(&self, klass: &Klass, kind: MemberKind) -> bool {
        let portion = match kind {
            MemberKind::Field => Portion::Field,
            MemberKind::Method => Portion::Method,
            MemberKind::Separator => return true,
        };
        self.shown(portion, &|target| self.covers(klass, target))
    }

    fn stereotype_shown(&self, klass: &Klass, label: &str) -> bool {
        let wanted = format!("<<{label}>>");
        self.shown(Portion::Stereotype, &|target| {
            let named = target.as_deref() == Some(wanted.as_str());
            let not_stereotype = !target
                .as_deref()
                .map(|t| t.starts_with("<<"))
                .unwrap_or(false);
            named || (not_stereotype && self.covers(klass, target))
        })
    }

    /// The last rule for `portion` whose target the check accepts.
    fn shown(&self, portion: Portion, covers: &dyn Fn(&Option<String>) -> bool) -> bool {
        let mut shown = true;
        for rule in &self.rules {
            if !rule.portions.contains(&portion) {
                continue;
            }
            if covers(&rule.target) {
                shown = rule.show;
            }
        }
        shown
    }

    fn covers(&self, klass: &Klass, target: &Option<String>) -> bool {
        let Some(target) = target.as_deref() else { return true };
        if let Some(stripped) = target.strip_prefix("<<") {
            return klass
                .stereotypes
                .iter()
                .any(|label| label == stripped.strip_suffix(">>").unwrap_or(stripped));
        }
        klass.name == *target
    }
}
