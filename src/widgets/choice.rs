use super::{CONTROL_H, LABEL, LINE, RADIUS};
use crate::el::{El, Paint, row, text};
use taffy::AlignItems;

/// One of several named options, as a row of small buttons that wraps; the current one is marked
/// in the accent colour. `msg` gets the index pressed.
pub fn choice<M: Clone>(key: &str, label: &str, options: &[&str], current: usize, msg: impl Fn(usize) -> M) -> El<M> {
    let mut opts = row().key(format!("{key}/options")).wrap().gap(6.0).grow(1.0);
    for (i, o) in options.iter().enumerate() {
        let on = i == current;
        opts = opts.child(row().key(format!("{key}/{i}")).center().height(CONTROL_H - 4.0).pad(10.0, 0.0).radius(RADIUS)
            .fill(if on { Paint { tone: 0.0, accent: 1.0, alpha: 0.14 } } else { Paint::GROUND })
            .border(LINE, if on { Paint::ACCENT } else { Paint::FAINT })
            .on_press(msg(i))
            .child(text(*o, LABEL - 1.0, if on { Paint::ACCENT } else { Paint::TEXT })));
    }
    row().key(key).gap(10.0).align(AlignItems::START)
        .child(row().width(76.0).height(CONTROL_H - 4.0).align(AlignItems::CENTER).child(text(label, LABEL, Paint::DIM)))
        .child(opts)
}
