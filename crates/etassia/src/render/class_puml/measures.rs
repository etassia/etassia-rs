//! The measurements the class lane reads: Ruby `Drawing::Face` (DejaVu,
//! the pinned reference face), `Layout::Base#measure_text` (the Arial
//! table) and its monospace form.
use crate::text::{self, Face};

/// Ruby `Drawing::Face.width`: the widest advance in DejaVu.
pub fn face_width(text: &str, size: f64, bold: bool) -> f64 {
    let face = if bold { Face::DejaVuBold } else { Face::DejaVu };
    text::measure(text, size, face).width
}

/// Ruby `Face.widest_line`: the widest line of the text, split at a
/// newline or a literal `\n`.
pub fn face_widest_line(text: &str, size: f64, bold: bool) -> f64 {
    let lines = split_display_lines(text);
    lines
        .iter()
        .map(|line| face_width(line, size, bold))
        .fold(0.0_f64, f64::max)
}

/// Ruby `text.to_s.split(/\\n|\n/, -1)`.
pub fn split_display_lines(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut run = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            out.push(std::mem::take(&mut run));
            continue;
        }
        if c == '\\' {
            if let Some(next) = chars.peek() {
                if *next == 'n' {
                    chars.next();
                    out.push(std::mem::take(&mut run));
                    continue;
                }
            }
        }
        run.push(c);
    }
    out.push(run);
    out
}

/// Ruby `Layout::Base#measure_text` (the Arial measurement).
pub fn measured(text: &str, font_size: f64) -> f64 {
    text::measure(text, font_size, Face::Arial).width
}

/// Ruby `TextMeasurement.measure(.., monospace: true)`.
pub fn measured_monospace(text: &str, font_size: f64) -> f64 {
    text::measure_monospace(text, font_size).width
}
