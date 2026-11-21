//! Walks the items of a diagram down the page and places each one, given
//! where the lifelines are. Port of `sequence/walker.rb` and
//! `sequence/bar_tracker.rb`.

use super::geometry;
use super::layout::{
    Arrow, Bar, Delay, Divider, Fragment as FragmentScene, LayoutConfig, Mark, NoteScene, Segment,
    Text,
};
use super::model::*;
use std::collections::HashMap;

pub const ROW: f64 = 40.0;
pub const SELF_WIDTH: f64 = 42.0;
const CLASSIC_UNLABELLED_SELF_ROW: f64 = 27.0;
const CLASSIC_ROW_SPARE: f64 = 12.3105;
const CLASSIC_TAIL: f64 = 17.0;
const CLASSIC_GUARD_ROW: f64 = 13.0;
const CLASSIC_SELF_TAIL: f64 = 30.0;
const CLASSIC_LABEL_RISE: f64 = 4.74;
const CLASSIC_DELAY_ROW: f64 = 28.0;
const CLASSIC_DELAY_LABEL_ROW: f64 = 40.9551;
const CLASSIC_DELAY_TEXT_DROP: f64 = 24.6348;
const CLASSIC_LABEL_ROW: f64 = 21.3105;
const SELF_HEIGHT: f64 = 20.0;
const LOOP_HEIGHT: f64 = 13.0;
const CLOSING_LOOP_DROP: f64 = 5.0;
const CLOSING_LOOP_BAR_REACH: f64 = 6.0;
pub const TAB_HEIGHT: f64 = 20.0;
const CROSS_HALF: f64 = 9.0;
const ACTIVATION_END_INSET: f64 = 7.0;
const TOP_OFFSET: f64 = 20.0;
const NOTE_ABOVE: f64 = 35.0;
const ATTACHED_NOTE_PAD: f64 = 3.0;
const ATTACHED_NOTE_RISE: f64 = 5.6895;
const STANDALONE_NOTE_RISE: f64 = 16.3105;
const STANDALONE_NOTE_TAIL: f64 = 26.6211;
const LEFT_EDGE: f64 = 0.0;
const TEOZ_NESTED_HEADER_ROW: f64 = 31.3105;
const CREATION_EXTRA_ROW: f64 = 12.18;
const TEOZ_MESSAGE_HEADER_ROW: f64 = 28.6211;
const TEOZ_EMPTY_HEADER_ROW: f64 = 13.3105;
const TEOZ_MESSAGE_BASE_ROW: f64 = 14.0;
const TEOZ_FRAME_BOTTOM_PAD: f64 = 6.0;
const TEOZ_ROOT_CLOSE_GAP: f64 = 16.6895;
const TEOZ_NESTED_CLOSE_GAP: f64 = 14.0;
const TEOZ_LABEL_HEIGHT: f64 = 15.3105;
const TEOZ_SELF_CONTACT_INSET: f64 = 2.5;
const TEOZ_CONTACT_PADDING: f64 = 2.0;
pub const BAR_WIDTH: f64 = 10.0;

fn rf(value: f64) -> String {
    crate::svg::ruby_float(value)
}

/// `Create.messages`: each message that creates a participant, keyed by
/// the message's item index.
pub fn creation_messages(items: &[Item]) -> HashMap<usize, String> {
    let mut found = HashMap::new();
    let mut pending: Option<String> = None;
    for (index, item) in items.iter().enumerate() {
        if let Item::Create(create) = item {
            pending = Some(create.participant.clone());
        }
        if let Item::Message(message) = item {
            if pending.as_deref() == Some(message.to.id()) {
                found.insert(index, pending.clone().unwrap());
                pending = None;
            }
        }
    }
    found
}

/// `Walker.following_activations`.
pub fn following_activations<'a>(items: &'a [Item], parallel_notes: bool) -> Vec<&'a Item> {
    items
        .iter()
        .take_while(|item| match item {
            Item::Activation(_) => true,
            Item::Note(note) => parallel_notes && note.parallel,
            _ => false,
        })
        .collect()
}

/// `BarTracker`.
struct BarTracker<'m> {
    bars: Vec<Bar>,
    open: HashMap<String, Vec<(f64, Option<String>)>>,
    /// Ruby's `@open` Hash keeps insertion order; finish emits in it.
    order: Vec<String>,
    centre: &'m dyn Fn(&str) -> f64,
}

impl<'m> BarTracker<'m> {
    fn new(centre: &'m dyn Fn(&str) -> f64) -> Self {
        BarTracker {
            bars: vec![],
            open: HashMap::new(),
            order: vec![],
            centre,
        }
    }

    fn depth(&self, id: &str) -> i64 {
        self.open.get(id).map(Vec::len).unwrap_or(0) as i64
    }

    fn apply(&mut self, activation: &Activation, at: f64) {
        let id = activation.participant.clone();
        if activation.on() {
            let entry = self.open.entry(id.clone()).or_default();
            if entry.is_empty() && !self.order.contains(&id) {
                self.order.push(id.clone());
            }
            entry.push((at, activation.color.clone()));
        } else {
            let popped = self.open.entry(id.clone()).or_default().pop();
            if let Some((top, color)) = popped {
                let shift = self
                    .open
                    .get(&id)
                    .map(|tops| tops.len() as f64 * (BAR_WIDTH / 2.0))
                    .unwrap_or(0.0);
                self.bars.push(Bar {
                    fill: color,
                    x: (self.centre)(&id) - BAR_WIDTH / 2.0 + shift,
                    y: top,
                    width: BAR_WIDTH,
                    height: (at - top).max(0.0),
                });
            }
        }
    }

    fn finish(&mut self, bottom: f64) {
        let ids: Vec<String> = self.order.clone();
        for id in ids {
            loop {
                let popped = self.open.get_mut(&id).and_then(|tops| tops.pop());
                let Some((top, color)) = popped else { break };
                let shift = self
                    .open
                    .get(&id)
                    .map(|tops| tops.len() as f64 * (BAR_WIDTH / 2.0))
                    .unwrap_or(0.0);
                self.bars.push(Bar {
                    fill: color,
                    x: (self.centre)(&id) - BAR_WIDTH / 2.0 + shift,
                    y: top,
                    width: BAR_WIDTH,
                    height: (bottom - top).max(0.0),
                });
            }
        }
    }

    fn destroy(&mut self, id: &str, bottom: f64) {
        loop {
            let popped = self.open.get_mut(id).and_then(|tops| tops.pop());
            let Some((top, color)) = popped else { break };
            let shift = self
                .open
                .get(id)
                .map(|tops| tops.len() as f64 * (BAR_WIDTH / 2.0))
                .unwrap_or(0.0);
            self.bars.push(Bar {
                fill: color,
                x: (self.centre)(id) - BAR_WIDTH / 2.0 + shift,
                y: top,
                width: BAR_WIDTH,
                height: (bottom - top).max(0.0),
            });
        }
    }
}

/// What the walk produces.
#[derive(Default)]
pub struct Flow {
    pub arrows: Vec<Arrow>,
    pub notes: Vec<NoteScene>,
    pub fragments: Vec<FragmentScene>,
    pub dividers: Vec<Divider>,
    pub delays: Vec<Delay>,
    pub crosses: Vec<Segment>,
    pub page_breaks: Vec<f64>,
    pub bars: Vec<Bar>,
    pub left: f64,
    pub right: f64,
    pub y: f64,
    pub created: HashMap<String, f64>,
}

struct BlockFrame {
    keyword: String,
    label: String,
    top: f64,
    low: f64,
    high: f64,
    branches: Vec<(f64, String)>,
    depth: usize,
    resume: Option<f64>,
    tab: geometry::TabSize,
    text_height: f64,
}

struct Walker<'m> {
    ids: Vec<String>,
    centers: Vec<f64>,
    measure: &'m dyn Fn(&str) -> f64,
    font_size: f64,
    config_message_measure: Option<&'m dyn Fn(&str) -> f64>,
    message_font_size: f64,
    message_family: Option<&'static str>,
    label_inset: Option<f64>,
    teoz_fragment_cadence: bool,
    activation_edges: bool,
    note_font_size: f64,
    row: f64,
    self_row: f64,
    unlabelled_row: Option<f64>,
    following_unlabelled_row: Option<f64>,
    fragment_row: f64,
    branch_gap: f64,
    fragment_close_gap: f64,
    divider_row: f64,
    group_fragment_row: Option<f64>,
    group_close_gap: Option<f64>,
    deactivation_gap: f64,
    bare_deactivation_gap: f64,
    frame_top_offset: f64,
    frame_bottom_offset: f64,
    branch_offset: f64,
    bounds: (f64, f64),
    head_halves: HashMap<String, f64>,
    flow: Flow,
    tracker: BarTracker<'m>,
    blocks: Vec<BlockFrame>,
    headroom: f64,
    spare_row: f64,
    item_index: usize,
    following_activations: Vec<Item>,
    next_non_activation: Option<Item>,
    mark_y: Option<f64>,
    previous_self_message: bool,
    resume: Option<f64>,
    row_line: f64,
    last_y: f64,
    self_drop: f64,
    loop_drop: f64,
    message_activations: Vec<String>,
    edge_right: Option<f64>,
    appearance_max_message: Option<i64>,
    appearance_tab_size: Option<f64>,
    appearance_tab_fill: Option<String>,
    appearance_tab_colour: Option<String>,
    fragmented: bool,
    creations: HashMap<usize, String>,
    fragment_contacts: HashMap<usize, f64>,
    closed: Option<(f64, f64, f64)>,
}

/// `Walker#run`.
#[allow(clippy::too_many_arguments)]
pub fn walk<'m>(
    items: &[Item],
    lifelines: Vec<(String, f64)>,
    edge_right: Option<f64>,
    appearance: &Appearance,
    font_size: f64,
    config: &LayoutConfig,
    start_y: f64,
    measure: &'m dyn Fn(&str) -> f64,
    message_measure: &'m dyn Fn(&str) -> f64,
) -> Flow {
    let ids: Vec<String> = lifelines.iter().map(|(id, _)| id.clone()).collect();
    let centers: Vec<f64> = lifelines.iter().map(|(_, centre)| *centre).collect();
    let config_message_measure: Option<&'m dyn Fn(&str) -> f64> = Some(message_measure);
    let message_font_size = config.message_font_size;
    let centre_ids = ids.clone();
    let centre_centers = centers.clone();
    let centre = move |id: &str| -> f64 {
        let index = centre_ids.iter().position(|known| known == id).unwrap_or(0);
        centre_centers.get(index).copied().unwrap_or(0.0)
    };
    let mut walker = Walker {
        ids,
        centers,
        measure,
        font_size,
        config_message_measure,
        message_font_size,
        message_family: None,
        label_inset: config.label_inset,
        teoz_fragment_cadence: config.teoz_fragment_cadence,
        activation_edges: config.activation_edges,
        note_font_size: config.note_font_size,
        row: config.row,
        self_row: config.self_row,
        unlabelled_row: config.unlabelled_row,
        following_unlabelled_row: config.following_unlabelled_row,
        fragment_row: config.fragment_row,
        branch_gap: config.branch_gap,
        fragment_close_gap: config.fragment_close_gap,
        divider_row: config.divider_row,
        group_fragment_row: config.group_fragment_row,
        group_close_gap: config.group_close_gap,
        deactivation_gap: config.deactivation_gap,
        bare_deactivation_gap: config.bare_deactivation_gap,
        frame_top_offset: config.frame_top_offset.unwrap_or(TOP_OFFSET),
        frame_bottom_offset: config.frame_bottom_offset.unwrap_or(TOP_OFFSET),
        branch_offset: config.branch_offset.unwrap_or(10.0),
        bounds: config.bounds,
        head_halves: config.head_halves.clone(),
        flow: Flow {
            left: f64::INFINITY,
            right: f64::NEG_INFINITY,
            y: start_y,
            ..Flow::default()
        },
        tracker: BarTracker::new(&centre),
        blocks: vec![],
        headroom: 0.0,
        spare_row: 0.0,
        item_index: 0,
        following_activations: vec![],
        next_non_activation: None,
        mark_y: None,
        previous_self_message: false,
        resume: None,
        row_line: 0.0,
        last_y: 0.0,
        self_drop: 0.0,
        loop_drop: 0.0,
        message_activations: vec![],
        edge_right,
        appearance_max_message: appearance.max_message,
        appearance_tab_size: appearance.tab_size,
        appearance_tab_fill: appearance.tab_fill.clone(),
        appearance_tab_colour: appearance.tab_colour.clone(),
        fragmented: items.iter().any(|item| matches!(item, Item::Fragment(_))),
        creations: creation_messages(items),
        fragment_contacts: HashMap::new(),
        closed: None,
    };
    walker.fragment_contacts = walker.build_fragment_contacts(items);
    walker.walk_items(items);
    walker.tracker.finish(walker.flow.y - 20.0);
    walker.flow.y -= walker.classic_tail_trim(items);
    let mut flow = std::mem::take(&mut walker.flow);
    flow.bars = std::mem::take(&mut walker.tracker.bars);
    flow
}

impl<'m> Walker<'m> {
    fn build_fragment_contacts(&self, items: &[Item]) -> HashMap<usize, f64> {
        let teoz_cadence = self.teoz_fragment_cadence;
        if !teoz_cadence {
            return HashMap::new();
        }
    let mut stack: Vec<usize> = vec![];
    let mut ends: HashMap<usize, usize> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if let Item::Fragment(fragment) = item {
            match fragment.phase {
                FragPhase::Open => stack.push(index),
                FragPhase::Close => {
                    if let Some(open) = stack.pop() {
                        ends.insert(open, index);
                    }
                }
                FragPhase::Else => {}
            }
        }
    }
    let mut contacts = HashMap::new();
    for (open, close) in ends {
        let following = items[close + 1..]
            .iter()
            .find(|item| !matches!(item, Item::Activation(_)));
        let message = match following {
            Some(Item::Message(message)) if message.parallel && message.self_message() => {
                Some(message)
            }
            _ => None,
        };
        contacts.insert(
            open,
            message
                .map(|message| self.label_lines(message).len() as f64 * TEOZ_LABEL_HEIGHT
                    + (LOOP_HEIGHT / 2.0)
                    + TEOZ_CONTACT_PADDING)
                .unwrap_or(0.0),
        );
    }
    contacts
    }

    fn centre(&self, id: &str) -> f64 {
        let index = self.ids.iter().position(|known| known == id).unwrap_or(0);
        self.centers.get(index).copied().unwrap_or(0.0)
    }

    fn classic(&self) -> bool {
        self.label_inset.is_none() && !self.teoz_fragment_cadence
    }

    fn wrap_measure(&self) -> Box<dyn Fn(&str) -> f64 + '_> {
        match self.config_message_measure {
            Some(measure) => Box::new(move |text| measure(text)),
            None => Box::new(move |text| (self.measure)(text)),
        }
    }

    fn label_lines(&self, message: &Message) -> Vec<String> {
        if message.label.is_empty() {
            return vec![];
        }
        let measure = self.message_measure();
        let wrap = geometry::MessageWrap::new(self.appearance_max_message, &measure);
        wrap.lines(&message.label)
    }

    fn message_measure(&self) -> Box<dyn Fn(&str) -> f64 + '_> {
        Box::new(move |text| self.message_measure_width(text))
    }

    fn message_measure_width(&self, text: &str) -> f64 {
        if self.label_inset.is_some() && self.message_family != Some("monospace") {
            return geometry::widest_line(text, self.message_font_size, false);
        }
        match self.config_message_measure {
            Some(measure) => measure(text),
            None => (self.measure)(text),
        }
    }

    fn line_step(&self) -> f64 {
        self.message_font_size * geometry::LINE_LEADING
    }

    fn label_width(&self, message: &Message) -> f64 {
        let measure = self.message_measure();
        let extra = match &message.number {
            Some(number) => geometry::numbered_extra(number, &measure),
            None => 0.0,
        };
        let wrap = geometry::MessageWrap::new(self.appearance_max_message, &measure);
        extra + wrap.width(&message.label)
    }

    fn walk_items(&mut self, items: &[Item]) {
        let mut previous: Option<Item> = None;
        for index in 0..items.len() {
            self.prepare_item(items, index);
            let item = items[index].clone();
            self.visit(&item, previous.as_ref());
            if !matches!(item, Item::Activation(_)) {
                previous = Some(item);
            }
        }
    }

    fn prepare_item(&mut self, items: &[Item], index: usize) {
        let following = &items[index + 1..];
        let parallel_notes = self.label_inset.is_some() && matches!(items[index], Item::Message(_));
        self.following_activations = following_activations(following, parallel_notes)
            .into_iter()
            .cloned()
            .collect();
        self.next_non_activation = following
            .iter()
            .find(|candidate| !matches!(candidate, Item::Activation(_)))
            .cloned();
        if !anchoring(&items[index]) {
            self.mark_y = None;
        }
        self.headroom = headroom(items, index, self.teoz_fragment_cadence, self.note_font_size);
        self.item_index = index;
    }

    fn visit(&mut self, item: &Item, previous: Option<&Item>) {
        if !continues_self_run(item) {
            self.previous_self_message = false;
        }
        self.resume = None;
        let offset = row_offset(item);
        if let Some(offset) = offset {
            self.begin_row(item, offset, previous);
        }
        self.place(item, previous);
        if self.resume.is_some() && !opens_block(item) {
            let resume = self.resume.unwrap();
            self.flow.y = if parallel_side_note(item) {
                resume
            } else {
                self.flow.y.max(resume)
            };
        }
    }

    fn begin_row(&mut self, item: &Item, offset: f64, previous: Option<&Item>) {
        let parallel = match item {
            Item::Message(message) => message.parallel,
            Item::Note(note) => note.parallel,
            Item::Fragment(fragment) => fragment.parallel,
            _ => false,
        };
        if parallel {
            self.resume = Some(self.flow.y);
            self.flow.y =
                self.row_line + offset + self.parallel_contact_offset(item, previous);
        } else {
            let attached = matches!(item, Item::Note(note) if note.attached());
            self.row_line = if attached { self.last_y } else { self.flow.y - offset };
        }
    }

    fn parallel_contact_offset(&mut self, item: &Item, previous: Option<&Item>) -> f64 {
        if !self.teoz_fragment_cadence {
            return 0.0;
        }
        let self_before_frame = matches!(item, Item::Fragment(_))
            && previous.is_some_and(|previous| {
                matches!(previous, Item::Message(m) if m.self_message())
            });
        if self_before_frame {
            return TEOZ_SELF_CONTACT_INSET;
        }
        let self_after_frame = matches!(item, Item::Message(m) if m.self_message())
            && previous.is_some_and(|previous| {
                matches!(previous, Item::Fragment(f) if f.phase == FragPhase::Close)
            });
        if !self_after_frame {
            return 0.0;
        }
        -TEOZ_SELF_CONTACT_INSET - (self.self_top() - self.flow.y)
    }

    fn self_top(&self) -> f64 {
        if self.teoz_fragment_cadence {
            return self.flow.y;
        }
        if !(self.label_inset.is_some() && self.fragmented) {
            return self.flow.y;
        }
        self.flow.y + ((SELF_HEIGHT - LOOP_HEIGHT) / 2.0)
    }

    fn place(&mut self, item: &Item, previous: Option<&Item>) {
        match item {
            Item::Note(note) => self.note(note, previous),
            Item::Message(message) => self.message(item, message),
            Item::Activation(activation) => self.activation(item, activation),
            Item::Destroy(destroy) => self.destroy(destroy),
            Item::Create(_) => {}
            Item::Fragment(fragment) => self.fragment(fragment),
            Item::Ref(ref_) => self.ref_line(ref_),
            Item::PageBreak(_) => self.flow.page_breaks.push(self.flow.y - 20.0),
            Item::Delay(delay) => self.delay(delay),
            Item::Spacing(spacing) => self.flow.y += spacing.room(),
            Item::Divider(divider) => self.divider(divider),
        }
    }

    fn message(&mut self, item: &Item, message: &Message) {
        // apply_message_font: a compact (teoz) diagram measures each
        // label in its own family.
        if self.label_inset.is_some() {
            self.message_family = message.label_family.or(Some("sans-serif"));
        }
        self.start_row(message);
        self.message_activations = self
            .following_activations
            .iter()
            .filter_map(|item| item.as_activation())
            .filter(|activation| activation.on())
            .map(|activation| activation.participant.clone())
            .collect();
        let (from_id, to_id) = (
            message.from.id().to_string(),
            message.to.id().to_string(),
        );
        let mut from = self.place_of(&message.from, message);
        let mut to = self.place_of(&message.to, message);
        // created_end
        if let Some(id) = self.creations.get(&self.item_index) {
            self.flow.created.insert(id.clone(), self.flow.y);
            let half = self.head_halves.get(id).copied().unwrap_or(0.0);
            let sign = if to > from { 1.0 } else if to < from { -1.0 } else { 0.0 };
            to -= sign * half;
        }
        if !message.style.hidden {
            let (path, marks, label) = if message.self_message() {
                self.self_arrow(message, from)
            } else {
                self.straight(message, from, to)
            };
            self.flow.arrows.push(self.arrow_record(message, path, marks, label));
        }
        let (low, high) = self.extent(message, from, to, &from_id, &to_id);
        self.touch(low, high);
        let row = self.message_row(message) + self.creation_extra(message);
        self.advance_by(message, row);
        let _ = item;
    }

    fn extent(&self, message: &Message, from: f64, to: f64, _from_id: &str, _to_id: &str) -> (f64, f64) {
        if self.loops_left(message) {
            return (from - self.reach(message), from);
        }
        let reached: Vec<f64> = if matches!(message.edge(), Some(edge) if edge.global()) {
            vec![self.centre(message.participants().first().copied().unwrap_or_default())]
        } else {
            vec![from, to]
        };
        let (low, high) = (
            reached.iter().cloned().fold(f64::INFINITY, f64::min),
            reached.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        let extra = if message.self_message() { self.reach(message) } else { 0.0 };
        (low, high + extra)
    }

    fn reach(&self, message: &Message) -> f64 {
        if self.label_inset.is_some() {
            return SELF_WIDTH + 6.0 + self.label_width(message);
        }
        SELF_WIDTH.max(super::layout::CLASSIC_LABEL_INSET + self.label_width(message))
    }

    fn advance_by(&mut self, message: &Message, row: f64) {
        self.spare_row = row - self.tail_of(message);
        self.flow.y += row - self.loop_drop;
    }

    fn tail_of(&self, message: &Message) -> f64 {
        if message.self_message() { CLASSIC_SELF_TAIL } else { CLASSIC_TAIL }
    }

    fn creation_extra(&self, _message: &Message) -> f64 {
        if self.creations.contains_key(&self.item_index) {
            CREATION_EXTRA_ROW
        } else {
            0.0
        }
    }

    fn message_row(&self, message: &Message) -> f64 {
        if self.teoz_fragment_cadence {
            return self.teoz_message_row(message);
        }
        if message.self_message() {
            return self.self_message_row(message);
        }
        self.plain_message_row(message)
    }

    fn plain_message_row(&self, message: &Message) -> f64 {
        if self.following_unlabelled_row.is_some() && self.following_unlabelled_message() {
            return self.following_unlabelled_row.unwrap();
        }
        if self.unlabelled_row.is_some() && message.label.is_empty() {
            return self.unlabelled_row.unwrap();
        }
        if self.label_inset.is_some() && message.label_family != Some("monospace") {
            return 29.3105;
        }
        self.row
    }

    fn teoz_message_row(&self, message: &Message) -> f64 {
        let text_height = self.teoz_row_text_height(message);
        let loop_height = if message.self_message() { LOOP_HEIGHT } else { 0.0 };
        TEOZ_MESSAGE_BASE_ROW + text_height + loop_height
    }

    fn teoz_row_text_height(&self, message: &Message) -> f64 {
        if let Some(Item::Message(following)) = &self.next_non_activation {
            if !following.parallel {
                return if following.label.is_empty() {
                    0.0
                } else {
                    TEOZ_LABEL_HEIGHT
                };
            }
        }
        self.label_lines(message).len() as f64 * TEOZ_LABEL_HEIGHT
    }

    fn self_message_row(&self, message: &Message) -> f64 {
        if self.label_inset.is_some() {
            return self.self_row;
        }
        let labelled = match &self.next_non_activation {
            Some(Item::Message(following)) => !following.label.is_empty(),
            _ => !message.label.is_empty(),
        };
        if labelled {
            self.self_row
        } else {
            CLASSIC_UNLABELLED_SELF_ROW
        }
    }

    fn following_unlabelled_message(&self) -> bool {
        match &self.next_non_activation {
            Some(Item::Message(following)) => {
                (self.label_inset.is_none() || !following.self_message())
                    && following.label.is_empty()
            }
            _ => false,
        }
    }

    fn start_row(&mut self, message: &Message) {
        self.flow.y += self.headroom;
        self.flow.y += (self.label_lines(message).len() as f64 - 1.0).max(0.0) * self.line_step();
        self.self_drop = if message.self_message() { LOOP_HEIGHT } else { 0.0 };
        self.last_y = self.flow.y;
        self.mark_y = Some(self.flow.y);
        self.loop_drop = self.closing_loop_drop(message);
        self.flow.y += self.loop_drop;
        if self.teoz_fragment_cadence && !self.blocks.is_empty() {
            let text_height = self.teoz_text_height(message);
            self.blocks.last_mut().unwrap().text_height = text_height;
        }
    }

    fn teoz_text_height(&self, message: &Message) -> f64 {
        self.label_lines(message).len() as f64 * TEOZ_LABEL_HEIGHT
    }

    fn closing_loop_drop(&self, message: &Message) -> f64 {
        if !(self.classic() && message.self_message()) {
            return 0.0;
        }
        let closes = self.following_activations.iter().any(|item| {
            matches!(item, Item::Activation(a) if !a.on() && a.participant == message.from.id())
        });
        if closes { CLOSING_LOOP_DROP } else { 0.0 }
    }

    fn place_of(&self, place: &Place, message: &Message) -> f64 {
        let Place::Id(id) = place else {
            let edge = match place {
                Place::Edge(edge) => *edge,
                _ => unreachable!(),
            };
            let inward = if edge.left() { 1.0 } else { -1.0 };
            let ring = if message.edge_end().circle {
                geometry::EDGE_RING_INSET
            } else {
                0.0
            };
            return self.edge_x(&edge, message) + inward * ring;
        };
        let at = self.centre(id);
        if self.label_inset.is_some() && message.self_message() {
            return self.compact_self_place(place, message, at);
        }
        if self.label_inset.is_some() || self.activation_edges {
            return self.compact_activation_place(place, message, at);
        }
        if !self.activated_end(place, message) {
            return at;
        }
        let other = if *place == message.to {
            message.from.id()
        } else {
            message.to.id()
        };
        let direction = if self.centre(other) < at { -1.0 } else { 1.0 };
        at + direction * ACTIVATION_END_INSET
    }

    fn compact_self_place(&self, place: &Place, message: &Message, at: f64) -> f64 {
        let depth = self.compact_activation_depth(place);
        if depth == 0 {
            return at;
        }
        let side = if message.leftward() { -1.0 } else { 1.0 };
        at + ((depth as f64 - 1.0 + side) * (BAR_WIDTH / 2.0))
    }

    fn compact_activation_place(&self, place: &Place, message: &Message, at: f64) -> f64 {
        let depth = self.compact_activation_depth(place);
        if depth == 0 {
            return at;
        }
        let toward = self.activation_direction(place, message, at);
        at + ((depth as f64 - 1.0) * (BAR_WIDTH / 2.0)) + (toward * (BAR_WIDTH / 2.0))
    }

    /// Ruby `activation_direction`: an Edge end points inward by its side;
    /// a participant end points toward the other end's centre.
    fn activation_direction(&self, place: &Place, message: &Message, at: f64) -> f64 {
        let other = if *place == message.to {
            message.from.clone()
        } else {
            message.to.clone()
        };
        match other {
            Place::Edge(edge) => {
                if edge.left() {
                    -1.0
                } else {
                    1.0
                }
            }
            Place::Id(id) => {
                if self.centre(&id) < at {
                    -1.0
                } else {
                    1.0
                }
            }
        }
    }

    fn compact_activation_depth(&self, place: &Place) -> i64 {
        let Place::Id(id) = place else { return 0 };
        let opening = self
            .following_activations
            .iter()
            .filter_map(|item| item.as_activation())
            .filter(|activation| activation.on() && &activation.participant == id)
            .count() as i64;
        self.tracker.depth(id) + opening
    }

    fn activated_end(&self, place: &Place, message: &Message) -> bool {
        !matches!(place, Place::Edge(_))
            && self.label_inset.is_none()
            && !message.self_message()
            && self
                .message_activations
                .iter()
                .any(|id| Some(id.as_str()) == place_id(place))
    }

    fn edge_x(&self, edge: &Edge, message: &Message) -> f64 {
        if edge.local {
            let run = self.label_width(message) + geometry::EDGE_RUN_PADDING;
            let participant = message
                .participants()
                .first()
                .copied()
                .unwrap_or_default()
                .to_string();
            return self.centre(&participant) + if edge.left() { -run } else { run };
        }
        if !edge.left() {
            return self.edge_right.unwrap_or(0.0);
        }
        if self.label_inset.is_some() { 10.0 } else { LEFT_EDGE }
    }

    fn activation(&mut self, _item: &Item, activation: &Activation) {
        let at = self.bar_edge(activation);
        self.tracker.apply(activation, at);
        if activation.on() {
            return;
        }
        if matches!(&self.next_non_activation, Some(Item::Divider(_))) {
            self.flow.y += match self.mark_y {
                Some(_) => self.deactivation_gap,
                None => self.bare_deactivation_gap,
            };
        }
    }

    fn bar_edge(&self, activation: &Activation) -> f64 {
        let Some(mark_y) = self.mark_y else {
            return self.flow.y - 20.0;
        };
        let loop_closing = !activation.on()
            && self.classic()
            && self.self_drop > 0.0;
        if loop_closing {
            mark_y + CLOSING_LOOP_BAR_REACH
        } else {
            mark_y
        }
    }

    fn destroy(&mut self, destroy: &Destroy) {
        let middle = match self.mark_y {
            Some(mark_y) => mark_y + self.self_drop,
            None => self.flow.y - 20.0,
        };
        let x = self.centre(&destroy.participant);
        if self.activation_edges {
            self.tracker.destroy(&destroy.participant, middle);
        }
        self.touch(x - CROSS_HALF, x + CROSS_HALF);
        for slant in [-CROSS_HALF, CROSS_HALF] {
            self.flow.crosses.push(Segment {
                x1: x - CROSS_HALF,
                y1: middle - slant,
                x2: x + CROSS_HALF,
                y2: middle + slant,
                dash: None,
            });
        }
    }

    fn straight(&self, message: &Message, from: f64, to: f64) -> (String, Vec<Mark>, (f64, f64, &'static str)) {
        let sign = if to >= from { 1.0 } else { -1.0 };
        let to = to - if self.compact_filled_head(message) { sign * 2.0 } else { 0.0 };
        let (tail, start) = arrow_marks_for(&message.style.tail, from, self.flow.y, sign);
        let (head, stop) = arrow_marks_for(&message.style.head, to, self.flow.y, -sign);
        let mut marks = tail;
        marks.extend(head);
        let label = if self.label_inset.is_none() {
            ((from + to) / 2.0, self.flow.y - 6.0, "middle")
        } else {
            (from.min(to) + self.label_inset.unwrap(), self.flow.y - 5.0659, "start")
        };
        (format!("M {} {} L {} {}", rf(start), rf(self.flow.y), rf(stop), rf(self.flow.y)), marks, label)
    }

    fn compact_filled_head(&self, message: &Message) -> bool {
        self.label_inset.is_some()
            && message.style.head.glyph == Some("filled")
            && !message.style.head.circle
    }

    fn self_arrow(
        &self,
        message: &Message,
        centre: f64,
    ) -> (String, Vec<Mark>, (f64, f64, &'static str)) {
        let side = if self.loops_left(message) { -1.0 } else { 1.0 };
        let far = centre + side * self.self_reach(message);
        let top = self.self_top();
        let bottom = top + self.self_height();
        let (tail, start) = arrow_marks_for(&message.style.tail, centre, top, side);
        let (head, stop) = arrow_marks_for(&message.style.head, centre, bottom, side);
        let mut marks = tail;
        marks.extend(head);
        let label_centre = far - side * SELF_WIDTH;
        let rise = if self.label_inset.is_some() { 5.0659 } else { CLASSIC_LABEL_RISE };
        let inset = self.label_inset.unwrap_or(super::layout::CLASSIC_LABEL_INSET);
        let label = (
            label_centre + side * inset,
            self.flow.y - rise,
            if side > 0.0 { "start" } else { "end" },
        );
        (
            format!(
                "M {} {} L {} {} L {} {} L {} {}",
                rf(start),
                rf(top),
                rf(far),
                rf(top),
                rf(far),
                rf(bottom),
                rf(stop),
                rf(bottom)
            ),
            marks,
            label,
        )
    }

    fn loops_left(&self, message: &Message) -> bool {
        message.self_message() && message.leftward()
    }

    fn self_reach(&self, message: &Message) -> f64 {
        if !(self.label_inset.is_some() && message.leftward()) {
            return SELF_WIDTH;
        }
        let spread = (self.compact_activation_depth(&message.from) - 1).max(0);
        SELF_WIDTH + spread as f64 * (BAR_WIDTH / 2.0)
    }

    fn self_height(&self) -> f64 {
        LOOP_HEIGHT
    }

    fn arrow_record(
        &self,
        message: &Message,
        path: String,
        marks: Vec<Mark>,
        label: (f64, f64, &'static str),
    ) -> Arrow {
        let texts = self.label_texts(message, label);
        Arrow {
            id: format!("message-{}", self.flow.arrows.len() + 1),
            path,
            dashed: message.dashed(),
            colour: message.style.colour.clone(),
            marks,
            texts,
        }
    }

    fn label_texts(&self, message: &Message, label: (f64, f64, &'static str)) -> Vec<Text> {
        if message.label.is_empty() {
            return vec![];
        }
        if let Some(number) = &message.number {
            let weight = number.bold.then_some("bold");
            let size = self.message_family.map(|_| self.message_font_size);
            return geometry::numbered_texts(
                number,
                &message.label,
                label.0,
                label.1,
                label.2,
                weight,
                size,
                self.message_family.map(str::to_string),
                &self.message_measure(),
            );
        }
        let lines = self.label_lines(message);
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let up = (lines.len() - 1 - index) as f64 * self.line_step();
                Text {
                    content: line.clone(),
                    x: label.0,
                    y: label.1 - up,
                    role: "message_label",
                    anchor: label.2,
                    size: self.message_family.map(|_| self.message_font_size),
                    family: self.message_family.map(str::to_string),
                    ..Text::default()
                }
            })
            .collect()
    }

    fn touch(&mut self, low: f64, high: f64) {
        self.flow.left = self.flow.left.min(low);
        self.flow.right = self.flow.right.max(high);
        if let Some(block) = self.blocks.last_mut() {
            block.low = block.low.min(low);
            block.high = block.high.max(high);
        }
    }

    fn note(&mut self, note: &Note, previous: Option<&Item>) {
        let on_block = note.attached() && matches!(previous, Some(Item::Fragment(_)));
        let (x, width) = if on_block {
            let (shape_x, shape_width, _top) = self.closed.as_ref().expect("closed block");
            let natural = geometry::natural_width(note, self.note_font_size);
            geometry::beside(*shape_x, *shape_width, natural)
        } else {
            let ids: Vec<&str> = self.ids.iter().map(String::as_str).collect();
            let span = geometry::span(note, previous, &ids);
            let natural = geometry::natural_width(note, self.note_font_size);
            let centers: Vec<f64> = span
                .0
                .max(span.1)
                .checked_sub(span.0)
                .map(|_| ())
                .map(|_| (span.0..=span.1).map(|index| self.centers[index]).collect())
                .unwrap_or_default();
            let centers = if centers.is_empty() {
                vec![self.centers[span.0.min(self.centers.len() - 1)]]
            } else {
                centers
            };
            geometry::note_box(note, &centers, natural)
        };
        let height = geometry::height(note);
        let top = self.note_top(note, on_block, height);
        self.flow.notes.push(self.note_record(note, x, top, width, height));
        self.touch(x, x + width);
        let pad = if on_block || !note.attached() {
            0.0
        } else {
            self.attached_note_pad_for(previous, note)
        };
        let tail = if note.parallel || note.attached() { 22.0 } else { STANDALONE_NOTE_TAIL };
        self.flow.y = (self.flow.y.max(top + height + tail)) + pad;
    }

    fn note_top(&self, note: &Note, on_block: bool, height: f64) -> f64 {
        if on_block {
            let (_, _, top) = self.closed.as_ref().expect("closed block");
            return *top + 5.0;
        }
        if note.attached() && note.picture.is_some() {
            return self.last_y - height / 2.0 + 10.0;
        }
        if note.attached() {
            return self.last_y - height + ATTACHED_NOTE_RISE;
        }
        self.flow.y - STANDALONE_NOTE_RISE
    }

    fn attached_note_pad_for(&self, previous: Option<&Item>, note: &Note) -> f64 {
        if self.teoz_fragment_cadence || note.picture.is_some() {
            return 0.0;
        }
        if let Some(Item::Message(message)) = previous {
            if message.self_message() || message.parallel {
                return 0.0;
            }
        }
        ATTACHED_NOTE_PAD
    }

    fn note_record(&self, note: &Note, x: f64, top: f64, width: f64, height: f64) -> NoteScene {
        let picture = note.picture.as_ref().map(|picture| {
            let inset = geometry::picture_inset(picture);
            (
                x + geometry::PAD - inset,
                top + 7.0 - inset,
                picture.scale,
                (*picture.scene).clone(),
            )
        });
        NoteScene {
            path: geometry::note_outline(note.shape, x, top, width, height),
            fold_path: geometry::note_fold(note.shape, x, top, width),
            fill: note.fill.as_ref().map(|fill| fill.colour.clone()),
            fill_opacity: note.fill.as_ref().and_then(|fill| fill.opacity),
            texts: self.note_texts(note, x + geometry::PAD, top),
            picture,
        }
    }

    fn note_texts(&self, note: &Note, left: f64, top: f64) -> Vec<Text> {
        if note.picture.is_some() {
            return vec![];
        }
        let step = geometry::line_height(self.note_font_size);
        note.lines()
            .iter()
            .enumerate()
            .map(|(index, line)| Text {
                content: line.to_string(),
                x: left,
                y: top + geometry::FIRST_BASELINE + index as f64 * step,
                role: "note",
                anchor: "start",
                ..Text::default()
            })
            .collect()
    }

    fn delay(&mut self, item: &super::model::Delay) {
        let label = item.label.clone();
        if self.label_inset.is_some() || self.teoz_fragment_cadence {
            // plain_delay
            if !label.is_empty() {
                let y = self.flow.y;
                let record = self.delay_record(&label, y, y, 5.0);
                self.flow.delays.push(record);
            }
            self.flow.y += self.divider_row;
            return;
        }
        let gap = if label.is_empty() { CLASSIC_DELAY_ROW } else { CLASSIC_DELAY_LABEL_ROW };
        let top = self.flow.y - CLASSIC_LABEL_ROW;
        let record = self.delay_record(&label, top, top + gap, CLASSIC_DELAY_TEXT_DROP);
        self.flow.delays.push(record);
        let clearance = if self.following_unlabelled_message() { 6.0 } else { CLASSIC_LABEL_ROW };
        self.flow.y = top + gap + clearance;
    }

    fn delay_record(&mut self, label: &str, top: f64, bottom: f64, drop: f64) -> Delay {
        let texts = if label.is_empty() {
            vec![]
        } else {
            let middle = (self.bounds.0 + self.bounds.1) / 2.0;
            let half = (self.measure)(label) / 2.0;
            self.touch(middle - half, middle + half);
            vec![Text {
                content: label.to_string(),
                x: middle,
                y: top + drop,
                role: "delay",
                anchor: "middle",
                ..Text::default()
            }]
        };
        Delay { y1: top, y2: bottom, texts }
    }

    fn divider(&mut self, item: &super::model::Divider) {
        let label = item.label.clone();
        let (left, right) = self.bounds;
        let width = if label.is_empty() {
            0.0
        } else {
            (self.measure)(&label) + 20.0
        };
        let middle = (left + right) / 2.0;
        let lines = [-2.0, 2.0]
            .iter()
            .map(|dy| Segment { x1: left, y1: self.flow.y + dy, x2: right, y2: self.flow.y + dy, dash: None })
            .collect();
        let texts = if label.is_empty() {
            vec![]
        } else {
            vec![Text {
                content: label.clone(),
                x: middle,
                y: self.flow.y + 5.0,
                role: "divider",
                anchor: "middle",
                ..Text::default()
            }]
        };
        self.flow.dividers.push(Divider {
            lines,
            x: middle - width / 2.0,
            y: self.flow.y - 10.0,
            width,
            height: 20.0,
            texts,
        });
        self.flow.y += self.divider_row;
    }

    fn ref_line(&mut self, ref_: &Ref) {
        let span: Vec<f64> = ref_.targets.iter().map(|id| self.centre(id)).collect();
        let low = span.iter().cloned().fold(f64::INFINITY, f64::min);
        let high = span.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let measure = |text: &str| (self.measure)(text);
        let shape = geometry::RefShape::new(ref_.label.clone(), self.flow.y - self.frame_bottom_offset, (low, high));
        self.flow.fragments.push(shape.scene());
        self.touch(shape.x, shape.x + shape.width);
        let _ = &measure;
        self.flow.y += geometry::REF_HEIGHT + geometry::REF_TAIL;
    }

    fn fragment(&mut self, fragment: &Fragment) {
        match fragment.phase {
            FragPhase::Open => self.open_block(fragment),
            FragPhase::Else => self.branch(fragment),
            FragPhase::Close => self.close_block(fragment),
        }
    }

    fn open_block(&mut self, fragment: &Fragment) {
        self.flow.y += self.fragment_contacts.get(&self.item_index).copied().unwrap_or(0.0);
        let tab = geometry::TabSize::new(self.appearance_tab_size, self.font_size);
        let _ = &tab;
        self.blocks.push(BlockFrame {
            keyword: fragment.keyword.clone(),
            label: fragment.label.clone().unwrap_or_default(),
            top: self.flow.y - self.frame_top_offset,
            low: f64::INFINITY,
            high: f64::NEG_INFINITY,
            branches: vec![],
            depth: 0,
            resume: self.resume,
            tab: geometry::TabSize::new(self.appearance_tab_size, self.font_size),
            text_height: 0.0,
        });
        let tab = geometry::TabSize::new(self.appearance_tab_size, self.font_size);
        self.flow.y += self.fragment_row_for(fragment) + tab.height_growth();
    }

    fn fragment_row_for(&self, fragment: &Fragment) -> f64 {
        if self.teoz_fragment_cadence {
            return self.teoz_header_row();
        }
        if fragment.keyword == "group" {
            if let Some(row) = self.group_fragment_row {
                return row;
            }
        }
        self.fragment_row
    }

    fn teoz_header_row(&self) -> f64 {
        match &self.next_non_activation {
            Some(Item::Fragment(f)) if f.phase == FragPhase::Open => TEOZ_NESTED_HEADER_ROW,
            Some(Item::Message(m)) if !m.label.is_empty() => TEOZ_MESSAGE_HEADER_ROW,
            _ => TEOZ_EMPTY_HEADER_ROW,
        }
    }

    fn branch(&mut self, fragment: &Fragment) {
        let label = fragment.label.clone().unwrap_or_default();
        if let Some(block) = self.blocks.last_mut() {
            block.branches.push((self.flow.y - self.branch_offset, label.clone()));
        }
        let guard_row = if self.label_inset.is_some() || label.trim().is_empty() {
            0.0
        } else {
            CLASSIC_GUARD_ROW
        };
        let row = if self.teoz_fragment_cadence {
            match &self.next_non_activation {
                Some(Item::Message(m)) if m.label.is_empty() => self.branch_gap - TEOZ_LABEL_HEIGHT,
                _ => self.branch_gap,
            }
        } else {
            self.branch_gap
        };
        self.flow.y += row + guard_row;
    }

    fn close_block(&mut self, fragment: &Fragment) {
        let Some(mut block) = self.blocks.pop() else {
            let _ = fragment;
            return;
        };
        let keyword = block.keyword.clone();
        let _ = keyword;
        let bottom = self.flow.y - self.fragment_bottom_offset(&block);
        let close_gap = self.close_gap(&block);
        let max_resume = match block.resume {
            Some(resume) => (self.flow.y + close_gap).max(resume),
            None => self.flow.y + close_gap,
        };
        self.flow.y = max_resume;
        self.row_line = block.top;
        let centers = self.centers.clone();
        let walker_measure = self.measure;
        let measure = move |text: &str| walker_measure(text);
        let record = geometry::FragmentShape::new(
            geometry::BlockRecord {
                keyword: block.keyword.clone(),
                label: block.label.clone(),
                top: block.top,
                low: block.low,
                high: block.high,
                branches: block.branches.clone(),
                depth: block.depth,
                tab: block.tab.clone(),
            },
            bottom,
            centers,
            measure,
            self.appearance_tab_fill.clone(),
            self.appearance_tab_colour.clone(),
        );
        self.flow.fragments.push(record.scene());
        let (shape_x, shape_width) = (record.x, record.width);
        self.closed = Some((shape_x, shape_width, block.top));
        self.touch(shape_x, shape_x + shape_width);
        // deepen_parent
        if let Some(parent) = self.blocks.last_mut() {
            parent.depth = parent.depth.max(block.depth + 1);
            parent.text_height = block.text_height;
        }
    }



    fn fragment_bottom_offset(&self, block: &BlockFrame) -> f64 {
        if !self.teoz_fragment_cadence {
            return self.frame_bottom_offset;
        }
        block.text_height + TEOZ_FRAME_BOTTOM_PAD
    }

    fn close_gap(&self, block: &BlockFrame) -> f64 {
        if self.teoz_fragment_cadence {
            if self.blocks.is_empty() {
                return match &self.next_non_activation {
                    Some(Item::Message(m)) if !m.parallel => {
                        let next_text = if m.label.is_empty() { 0.0 } else { TEOZ_LABEL_HEIGHT };
                        TEOZ_NESTED_CLOSE_GAP + next_text - block.text_height
                    }
                    _ => TEOZ_ROOT_CLOSE_GAP,
                };
            }
            return TEOZ_NESTED_CLOSE_GAP;
        }
        if block.keyword == "group" {
            if let Some(gap) = self.group_close_gap {
                return gap;
            }
        }
        self.fragment_close_gap
    }

    fn classic_tail_trim(&self, items: &[Item]) -> f64 {
        if self.label_inset.is_some() || self.teoz_fragment_cadence {
            return 0.0;
        }
        let Some(last) = items.iter().rev().find(|item| !matches!(item, Item::Activation(_))) else {
            return 0.0;
        };
        if self.bar_after(items, last) {
            return 0.0;
        }
        match last {
            Item::Fragment(_) => CLASSIC_ROW_SPARE,
            Item::Message(_) => self.spare_row,
            Item::Destroy(_) | Item::Spacing(_) | Item::Delay(_) => CLASSIC_ROW_SPARE,
            _ => 0.0,
        }
    }

    fn bar_after(&self, items: &[Item], last: &Item) -> bool {
        let index = items
            .iter()
            .position(|item| std::ptr::eq(item, last) || items_equivalent(item, last))
            .unwrap_or(0);
        let trailing = &items[index + 1..];
        let any_on = trailing.iter().any(|item| matches!(item, Item::Activation(a) if a.on()));
        let any_off = trailing
            .iter()
            .any(|item| matches!(item, Item::Activation(a) if !a.on()));
        any_on && any_off
    }
}

fn items_equivalent(_a: &Item, _b: &Item) -> bool {
    false
}

fn anchoring(item: &Item) -> bool {
    matches!(item, Item::Message(_) | Item::Activation(_) | Item::Destroy(_))
}

fn continues_self_run(item: &Item) -> bool {
    matches!(item, Item::Message(_) | Item::Activation(_))
}

fn opens_block(item: &Item) -> bool {
    matches!(item, Item::Fragment(f) if f.phase == FragPhase::Open)
}

fn parallel_side_note(item: &Item) -> bool {
    matches!(item, Item::Note(note) if !note.targets.is_empty() && matches!(note.side, NoteSide::Left | NoteSide::Right))
}

fn row_offset(item: &Item) -> Option<f64> {
    match item {
        Item::Message(_) => Some(0.0),
        Item::Note(_) => Some(STANDALONE_NOTE_RISE),
        Item::Ref(_) => Some(TOP_OFFSET),
        Item::Fragment(f) if f.phase == FragPhase::Open => Some(TOP_OFFSET),
        _ => None,
    }
}

fn headroom(items: &[Item], index: usize, teoz_cadence: bool, _note_font_size: f64) -> f64 {
    if !matches!(items[index], Item::Message(_)) {
        return 0.0;
    }
    let following = items[index + 1..]
        .iter()
        .find(|other| !matches!(other, Item::Activation(_)));
    let Some(Item::Note(note)) = following else {
        return 0.0;
    };
    if !note.attached() {
        return 0.0;
    }
    let pad = if teoz_cadence || note.picture.is_some() {
        0.0
    } else {
        match &items[index] {
            Item::Message(message) if message.self_message() || message.parallel => 0.0,
            _ => ATTACHED_NOTE_PAD,
        }
    };
    let height = geometry::height(note);
    let clearance = if note.picture.is_some() { height / 2.0 } else { height };
    pad + (clearance - NOTE_ABOVE).max(0.0)
}

fn arrow_marks_for(end: &ArrowEnd, at_x: f64, at_y: f64, inward: f64) -> (Vec<Mark>, f64) {
    geometry::arrow_marks(end.glyph, end.circle, at_x, at_y, inward)
}

fn place_id(place: &Place) -> Option<&str> {
    match place {
        Place::Id(id) => Some(id.as_str()),
        Place::Edge(_) => None,
    }
}

use super::layout as layout_mod;
#[allow(unused_imports)]
use layout_mod as _layout_mod;
