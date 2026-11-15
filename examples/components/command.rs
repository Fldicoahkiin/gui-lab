//! command: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 筛选中 · Filtering
pub fn ex_0() -> El<Msg> {
    command("cp1", &TextEdit::new("sa"), &["open file", "save all", "close window"], 0, Msg::Edit, Msg::Pick)
}
