//! avatar
//!
//! Purpose: Stand for a person by their initials (no pictures of people).
//! Props: `key`, `name`, `size`, `hue` (0, 1, 2).
//! States: none.
//! Messages: none.
//! Usage: `avatar("me", "Lin Mo", 32.0, 1)`
//!
//! A disc in one of the theme's hues with up to two initials (the first letters of the first two
//! words; for a name without spaces, its first character). The name is its accessible name.
use crate::el::{El, Font, Paint, Role, row, text};

pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        [] => "?".into(),
        [one] => one.chars().take(if one.is_ascii() { 2 } else { 1 }).collect::<String>().to_uppercase(),
        [a, b, ..] => format!("{}{}", a.chars().next().unwrap_or('?'), b.chars().next().unwrap_or('?')).to_uppercase(),
    }
}
pub fn avatar<M>(key: &str, name: &str, size: f32, hue: u8) -> El<M> {
    row().key(key).width(size).height(size).radius(size * 0.5).fill(Paint::hue(hue)).center().role(Role::Img).label(name)
        .child(text(initials(name), size * 0.4, Paint::GROUND).font(Font::Display))
}
