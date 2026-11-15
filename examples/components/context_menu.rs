//! context_menu: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 关着 · Closed
pub fn ex_0() -> El<Msg> {
    context_menu("cm1", None, &["打开", "重命名"], 0, Msg::Pick, Msg::Pick, Msg::Nothing)
}
