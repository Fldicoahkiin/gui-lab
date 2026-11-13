use super::{CONTROL_H, LABEL, LINE};
use crate::el::{El, Paint, row, space, text};
use taffy::prelude::JustifyContent;

/// A two-state switch with a label and the name of the current state.
pub fn toggle<M: Clone>(key: impl Into<String>, label: &str, on: bool, states: (&str, &str), msg: M) -> El<M> {
    let knob = space(12.0, 12.0).radius(6.0).fill(if on { Paint::ACCENT } else { Paint::TEXT });
    let track = row().width(34.0).height(18.0).pad(3.0, 0.0).radius(9.0).fill(Paint::GROUND).border(LINE, Paint::DIM).center()
        .justify(if on { JustifyContent::END } else { JustifyContent::START })
        .child(knob);
    row().key(key).height(CONTROL_H).gap(10.0).align(taffy::AlignItems::CENTER).on_press(msg)
        .child(text(label, LABEL, Paint::DIM).grow(1.0))
        .child(text(if on { states.1 } else { states.0 }, LABEL, Paint::TEXT))
        .child(track)
}
