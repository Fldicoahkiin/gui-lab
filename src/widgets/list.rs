//! A list of rows, one of which can be selected; it scrolls when it holds more than it shows. The list
//! is one focus stop: Up / Down / Home / End move the selection, and the selected row is scrolled into
//! view. Rows are pressed with the pointer.
use super::{LABEL, LINE, SELECTED};
use crate::el::{El, Font, Key, Paint, col, row, text};
use std::rc::Rc;
use taffy::AlignItems;

/// Each row is a label and a dim note. Rows are keyed `key:i`.
pub fn list<M: Clone + 'static>(key: &str, rows: &[(&str, &str)], selected: Option<usize>, row_h: f32, height: f32, msg: impl Fn(usize) -> M + 'static) -> El<M> {
    let msg: Rc<dyn Fn(usize) -> M> = Rc::new(msg);
    let n = rows.len();
    let keys = {
        let msg = msg.clone();
        move |k: Key, _: bool| -> Option<M> {
            if n == 0 { return None; }
            let to = match (k, selected) {
                (Key::Down, None) | (Key::Home, _) => 0,
                (Key::Up, None) | (Key::End, _) => n - 1,
                (Key::Down, Some(i)) if i + 1 < n => i + 1,
                (Key::Up, Some(i)) if i > 0 => i - 1,
                _ => return None,
            };
            (Some(to) != selected).then(|| msg(to))
        }
    };
    let mut l = col().key(key).scroll().bar().height(height).radius(6.0).border(LINE, Paint::FAINT).pad(0.0, 4.0).on_key(keys);
    for (i, (label, note)) in rows.iter().enumerate() {
        let on = selected == Some(i);
        let mut r = row().key(format!("{key}:{i}")).height(row_h).pad(14.0, 0.0).gap(12.0).align(AlignItems::CENTER)
            .on_press(msg(i)).focusable(false).reveal(on)
            .child(text(*label, LABEL + 1.0, if on { Paint::ACCENT } else { Paint::TEXT }).grow(1.0))
            .child(text(*note, LABEL - 1.0, Paint::DIM).font(Font::Mono));
        if on { r = r.fill(SELECTED); }
        l = l.child(r);
    }
    l
}
