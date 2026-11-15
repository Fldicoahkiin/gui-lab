//! badge: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El, row};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 三种 · Three kinds
pub fn ex_0() -> El<Msg> {
    row().gap(8.0).child(badge("bd1", "新", Variant::Accent)).child(badge("bd2", "草稿", Variant::Default)).child(badge("bd3", "12", Variant::Quiet))
}
