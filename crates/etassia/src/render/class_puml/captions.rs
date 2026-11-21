//! Ports of Ruby `CaptionLayout` and `CaptionRows` (`caption_layout.rb`,
//! `caption_rows.rb`): captions placed around an already laid out Scene.
use super::model::{CaptionKind, Diagram};
use super::scene::{Panel, Scene};

pub const GAP: f64 = 10.0;
const PADDING: f64 = 5.0;
const SMALL_SIZE: f64 = 10.0;
const LEGEND_FILL: &str = "#DDDDDD";
const SMALL_COLOUR: &str = "#888888";

pub struct CaptionLayout<'a> {
    diagram: &'a Diagram,
    normal_size: f64,
    measure: &'a dyn Fn(&str, f64) -> f64,
}

impl<'a> CaptionLayout<'a> {
    pub fn new(
        diagram: &'a Diagram,
        normal_size: f64,
        measure: &'a dyn Fn(&str, f64) -> f64,
    ) -> CaptionLayout<'a> {
        CaptionLayout { diagram, normal_size, measure }
    }

    /// Ruby `dress`: the Scene, as large as content and captions.
    pub fn dress(&self, scene: &mut Scene) {
        let panels = self.sized_panels();
        if panels.is_empty() && self.diagram.style.background.is_none() {
            return;
        }
        let mut width = scene.width;
        let widest = panels.iter().map(|panel| panel.width).fold(0.0_f64, f64::max);
        width = width.max(widest + GAP * 2.0);
        let rows = CaptionRows::new(panels, width, scene.height);
        scene.content_x = (width - scene.width) / 2.0;
        scene.content_y = rows.content_y;
        scene.width = width;
        scene.height = rows.bottom_edge();
        scene.panels = rows.panels();
        scene.background = self.diagram.style.background.clone();
    }

    fn sized_panels(&self) -> Vec<Panel> {
        self.diagram
            .captions
            .iter()
            .map(|caption| self.panel(caption.kind, &caption.text))
            .collect()
    }

    fn panel(&self, kind: CaptionKind, content: &str) -> Panel {
        let rule = self.diagram.style.rules.get(&kind);
        let size = self.size_of(kind, rule);
        let width = (self.measure)(content, size) + PADDING * 2.0;
        let height = size * 1.2 + PADDING * 2.0;
        let is_small = matches!(kind, CaptionKind::Header | CaptionKind::Footer);
        Panel {
            id: format!("caption-{}", kind.as_str()),
            kind: kind.as_str(),
            x: 0.0,
            y: 0.0,
            width,
            height,
            fill: rule
                .and_then(|rule| rule.background.clone())
                .or((kind == CaptionKind::Legend).then(|| LEGEND_FILL.to_string())),
            bordered: kind == CaptionKind::Legend,
            colour: rule
                .and_then(|rule| rule.colour.clone())
                .or(is_small.then(|| SMALL_COLOUR.to_string())),
            font_size: size,
            bold: kind == CaptionKind::Title,
            content: content.to_string(),
        }
    }

    fn size_of(&self, kind: CaptionKind, rule: Option<&super::model::RuleProps>) -> f64 {
        let from_rule = rule.and_then(|rule| rule.size);
        from_rule.unwrap_or(self.default_size(kind))
    }

    fn default_size(&self, kind: CaptionKind) -> f64 {
        if matches!(kind, CaptionKind::Header | CaptionKind::Footer) {
            SMALL_SIZE
        } else {
            self.normal_size
        }
    }
}

pub struct CaptionRows {
    top: Vec<Panel>,
    bottom: Vec<Panel>,
    content_y: f64,
    bottom_edge: f64,
}

impl CaptionRows {
    pub fn new(mut panels: Vec<Panel>, width: f64, content_height: f64) -> CaptionRows {
        let top = stack(&mut panels, &[CaptionKind::Header, CaptionKind::Title], GAP, width);
        let content_y = if top.is_empty() {
            0.0
        } else {
            let last = top.last().unwrap();
            last.y + last.height
        };
        let below = content_y + content_height;
        let bottom = stack(
            &mut panels,
            &[CaptionKind::Legend, CaptionKind::Caption, CaptionKind::Footer],
            below,
            width,
        );
        let bottom_edge = if bottom.is_empty() {
            below
        } else {
            let last = bottom.last().unwrap();
            last.y + last.height + GAP
        };
        CaptionRows { top, bottom, content_y, bottom_edge }
    }

    pub fn panels(&self) -> Vec<Panel> {
        let mut out = self.top.clone();
        out.extend(self.bottom.iter().cloned());
        out
    }

    pub fn bottom_edge(&self) -> f64 {
        self.bottom_edge
    }
}

fn stack(panels: &mut [Panel], order: &[CaptionKind], start: f64, width: f64) -> Vec<Panel> {
    let mut cursor = start;
    let mut out = vec![];
    for kind in order {
        let Some(position) = panels.iter().position(|panel| panel.kind == kind.as_str()) else {
            continue;
        };
        let panel = panels[position].clone();
        let panel = at(&panel, cursor, width);
        cursor += panel.height + GAP;
        panels[position] = panel.clone();
        out.push(panel);
    }
    out
}

fn at(panel: &Panel, top: f64, width: f64) -> Panel {
    let mut positioned = panel.clone();
    let mut left = (width - panel.width) / 2.0;
    if panel.kind == "header" {
        left = width - GAP - panel.width;
    }
    positioned.x = left;
    positioned.y = top;
    positioned
}
