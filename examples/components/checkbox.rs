//! checkbox: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 未选 · Unchecked
pub fn ex_0() -> El<Msg> {
    checkbox("cb1", "同意条款", false, Msg::Nothing)
}

/// 已选 · Checked
pub fn ex_1() -> El<Msg> {
    checkbox("cb2", "同意条款", true, Msg::Nothing)
}

/// 不可用 · Disabled
pub fn ex_2() -> El<Msg> {
    checkbox("cb3", "同意条款", true, Msg::Nothing).disabled(true)
}
