//! accordion: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use amenui::el::{El, Paint, text};
use amenui::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 一节打开 · One open
pub fn ex_0() -> El<Msg> {
    accordion("a1", vec![("什么是材质", text("第二遍的画法。", 13.0, Paint::TEXT)), ("什么是预设", text("一组数据。", 13.0, Paint::TEXT))], &[true, false], Msg::Pick).width(320.0)
}
