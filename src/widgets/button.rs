//! A button: a label that sends a message when pressed (pointer, Enter or Space). To make it
//! unavailable, call `.disabled(true)` on what this returns: it then takes no input and is drawn faint.
use super::{CONTROL_H, LABEL, LINE, RADIUS};
use crate::el::{El, Paint, row, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Outlined, for most actions.
    Default,
    /// No outline, for actions that should not draw the eye.
    Quiet,
    /// Filled with the accent colour, for the one main action of a view.
    Accent,
}

pub fn button<M: Clone>(key: impl Into<String>, label: &str, msg: M) -> El<M> { button_as(key, label, Variant::Default, msg) }

pub fn button_as<M: Clone>(key: impl Into<String>, label: &str, v: Variant, msg: M) -> El<M> {
    let b = row().key(key).center().height(CONTROL_H).pad(14.0, 0.0).radius(RADIUS).on_press(msg);
    match v {
        Variant::Default => b.fill(Paint::GROUND).border(LINE, Paint::DIM).child(text(label, LABEL, Paint::TEXT)),
        Variant::Quiet => b.child(text(label, LABEL, Paint::TEXT)),
        // The label is set in the page's ground colour, so it reads on the accent in every theme.
        Variant::Accent => b.fill(Paint::ACCENT).child(text(label, LABEL, Paint::GROUND)),
    }
}
