//! choice: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use amenui::el::{El};
use amenui::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 选项 · Choice
pub fn ex_0() -> El<Msg> {
    choice("c1", "大小", &["小", "中", "大"], 1, Msg::Pick)
}

/// 分段 · Segmented
pub fn ex_1() -> El<Msg> {
    segmented("c2", &["日", "周", "月"], 0, 28.0, Msg::Pick)
}
