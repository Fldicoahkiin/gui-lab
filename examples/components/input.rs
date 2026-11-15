//! input: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 一行 · One line
pub fn ex_0() -> El<Msg> {
    input("i1", &TextEdit::new("林"), "名称", 280.0, 40, Msg::Edit)
}

/// 空的 · Empty
pub fn ex_1() -> El<Msg> {
    input("i2", &TextEdit::default(), "搜索…", 280.0, 40, Msg::Edit)
}

/// 多行 · Several lines
pub fn ex_2() -> El<Msg> {
    textarea("i3", &TextEdit::new("第一行\n第二行"), "备注", 280.0, 3, 400, Msg::Edit)
}
