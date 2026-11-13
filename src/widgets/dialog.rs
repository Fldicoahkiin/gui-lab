//! A modal dialog. While open it is the only thing that takes input: focus moves into it and stays
//! there, Esc or the close button send `close`, and everything beneath is held partly loose
//! (`beneath`, about 0.35). Declare it anywhere in the view, next to what opens it; the runtime lays
//! it over the whole window. When `open` is false it is nothing.
use super::{LABEL, LINE, Variant, button_as};
use crate::el::{El, Font, Paint, col, row, space, text};
use taffy::AlignItems;

pub fn dialog<M: Clone>(key: &str, open: bool, title: &str, width: f32, beneath: f32, close: M, body: El<M>) -> El<M> {
    if !open { return space(0.0, 0.0); }
    let head = row().key(format!("{key}/head")).gap(12.0).align(AlignItems::CENTER)
        .child(text(title, LABEL + 3.0, Paint::INK).font(Font::Display).grow(1.0))
        .child(button_as(format!("{key}/close"), "关闭", Variant::Quiet, close.clone()));
    // The card is filled with the page's ground, so what is beneath does not show through it.
    let card = col().key(format!("{key}/card")).width(width).gap(14.0).pad(22.0, 18.0).radius(8.0)
        .fill(Paint::GROUND).border(LINE, Paint::DIM)
        .child(head).child(body);
    col().key(key).center().modal(close, beneath).child(card)
}
