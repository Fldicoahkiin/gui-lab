//! tabs: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El, Paint, text};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 两页 · Two tabs
pub fn ex_0() -> El<Msg> {
    tabs("t1", &["概览", "设置"], 0, 18.0, 2.0, Msg::Pick, text("概览的内容", 13.0, Paint::TEXT))
}
