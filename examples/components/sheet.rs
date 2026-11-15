//! sheet: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El, Paint, text};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 关着 · Closed
pub fn ex_0() -> El<Msg> {
    sheet("sh1", false, "筛选", 320.0, Msg::Nothing, text("条件", 13.0, Paint::TEXT))
}
