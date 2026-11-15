//! popover: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El, Paint, text};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 关着 · Closed
pub fn ex_0() -> El<Msg> {
    popover("po1", "分享", false, Msg::Nothing, Msg::Nothing, text("链接", 13.0, Paint::TEXT))
}
