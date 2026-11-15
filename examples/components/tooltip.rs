//! tooltip: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 显示 · Shown
pub fn ex_0() -> El<Msg> {
    tooltip("tt1", button("tb", "保存", Msg::Nothing), "保存到本机", true)
}
