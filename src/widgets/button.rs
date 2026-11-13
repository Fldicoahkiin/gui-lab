use super::{CONTROL_H, LABEL, LINE, RADIUS};
use crate::el::{El, Paint, row, text};

/// A labelled button that sends `msg` when pressed (pointer, Enter or Space).
pub fn button<M: Clone>(key: impl Into<String>, label: &str, msg: M) -> El<M> {
    row().key(key).center().height(CONTROL_H).pad(14.0, 0.0).radius(RADIUS)
        .border(LINE, Paint::DIM)
        .on_press(msg)
        .child(text(label, LABEL, Paint::TEXT))
}
