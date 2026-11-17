//! Recipe: a counter whose digits recompose and whose presses ripple only its row (docs/recipes/counter.md).
//! The count is keyed text, so a new value recomposes the changed digits; the row is a ripple scope and
//! the button sends its fronts there; the picture beside it loosens, the label only nudges.
use amenui::el::{El, Font, Paint, Scope, col, row, space, text};
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::{Variant, button_as};

pub struct Counter { pub n: u32 }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Inc }

pub fn view(s: &Counter) -> El<Msg> {
    col().key("page").pad(32.0, 28.0).child(row().key("row").ripple_scope().gap(24.0).pad(20.0, 16.0).border(1.0, Paint::FAINT)
        .child(button_as("inc", "加一", Variant::Default, Msg::Inc).ripple(Scope::Container(None)))
        .child(text(s.n.to_string(), 40.0, Paint::INK).font(Font::Display).key("count"))
        .child(space(60.0, 40.0).radius(6.0).fill(Paint::ACCENT).key("decor")))
}

pub fn update(s: &mut Counter, m: Msg) {
    match m { Msg::Inc => s.n += 1 }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Counter, Msg> {
    Ui::new(Counter { n: 9 }, view, update, faces, w, h, scale)
}
