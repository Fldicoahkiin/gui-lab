//! A slider for a value in a range. Drag anywhere on it, or with focus use Left / Right (Shift for
//! ×10) and Home / End. It is the secondary way to set a value; `number` is the exact one.
use super::{LINE, Spec};
use crate::el::{Drag, El, Key, Paint, row, space};
use taffy::AlignItems;

const KNOB: f32 = 14.0;

/// The value for a drag position: the knob's centre follows the pointer, snapped to the step.
pub fn slider_value(d: &Drag, spec: Spec) -> f32 {
    let f = ((d.x - KNOB * 0.5) / (d.w - KNOB).max(1.0)).clamp(0.0, 1.0);
    let steps = ((spec.max - spec.min) * f / spec.step).round();
    spec.clamp(spec.min + steps * spec.step)
}

pub fn slider<M: Clone + 'static>(key: &str, value: f32, spec: Spec, width: f32, msg: impl Fn(f32) -> M + 'static) -> El<M> {
    let msg = std::rc::Rc::new(msg);
    let f = ((value - spec.min) / (spec.max - spec.min).max(1e-6)).clamp(0.0, 1.0);
    let run = (width - KNOB).max(0.0);
    let keys = {
        let msg = msg.clone();
        move |k: Key, shift: bool| -> Option<M> {
            let n = if shift { 10.0 } else { 1.0 };
            let v = match k {
                Key::Left | Key::Down => value - spec.step * n,
                Key::Right | Key::Up => value + spec.step * n,
                Key::Home => spec.min,
                Key::End => spec.max,
                _ => return None,
            };
            let v = spec.clamp(v);
            (v != value).then(|| msg(v))
        }
    };
    let drag = move |d: Drag| Some(msg(slider_value(&d, spec)));
    row().key(key).width(width).height(24.0).align(AlignItems::CENTER).on_key(keys).on_drag_with(drag)
        .child(space(run * f, 2.0).fill(Paint::ACCENT))
        .child(space(KNOB, KNOB).radius(KNOB * 0.5).fill(Paint::GROUND).border(LINE + 1.0, Paint::ACCENT))
        .child(space(0.0, 2.0).grow(1.0).fill(Paint::FAINT))
}
