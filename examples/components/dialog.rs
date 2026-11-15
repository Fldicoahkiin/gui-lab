//! dialog: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El, Paint, col, text};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 关着，一个打开它的按钮 · Closed, with a button that opens it
pub fn ex_0() -> El<Msg> {
    col().gap(8.0).child(button("rename", "重命名…", Msg::Nothing)).child(dialog("d1", false, "重命名", 320.0, 0.35, Msg::Nothing, text("新的名字", 14.0, Paint::TEXT)))
}
