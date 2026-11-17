//! toast: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use amenui::el::{El, col};
use amenui::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 显示 · Shown
pub fn ex_0() -> El<Msg> {
    col().width(380.0).height(110.0).child(toast("to1", "已保存", "三项改动已写入", true, Msg::Nothing))
}
