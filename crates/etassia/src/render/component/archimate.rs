//! `archimate #Layer "Name" as id <<archimate-type>>`: the layer name is
//! a colour filled the way the jar fills it, and the type's icon comes
//! from the generated vocabulary. Port of `node_graph/archimate.rb`.

use super::archimate_sprites::{Sprite, TABLE};

pub const SPRITE_ROOM: f64 = 20.0;

/// The jar's fill for each ArchiMate layer name.
pub fn layer_color(color: &str) -> Option<&'static str> {
    match color.to_ascii_lowercase().as_str() {
        "business" => Some("#FFFFCC"),
        "application" => Some("#C2F0FF"),
        "technology" => Some("#C9FFC9"),
        "physical" => Some("#97FF97"),
        "motivation" => Some("#CCCCFF"),
        "strategy" => Some("#F8E7C0"),
        "implementation" => Some("#FFE0E0"),
        "migration" => Some("#FFE0E0"),
        "other" => Some("#FFFFFF"),
        _ => None,
    }
}

/// Ruby `Archimate.color`: the colour a layer name stands for, else
/// `color` itself, always without the leading `#`; `None` for nil.
pub fn color(color: Option<&str>) -> Option<String> {
    let color = color?;
    let lowered = color.trim().to_lowercase();
    let resolved = layer_color(&lowered).unwrap_or(color);
    Some(resolved.strip_prefix('#').unwrap_or(resolved).to_string())
}

/// The type's sprite row — icon path plus placement geometry — when the
/// stereotype carries an icon. The jar's `$archimate/` prefix is optional.
pub fn sprite(stereotype: Option<&str>) -> Option<&'static Sprite> {
    let stereotype = stereotype?;
    let name = stereotype.trim().to_lowercase();
    let name = name.strip_prefix("$archimate/").unwrap_or(&name);
    TABLE.iter().find(|(known, _)| *known == name).map(|(_, row)| row)
}
