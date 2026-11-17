//! card: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use amenui::el::{El, Paint, text};
use amenui::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 带标题 · With a title
pub fn ex_0() -> El<Msg> {
    card("k1", Some("天气"), text("晴，22°", 14.0, Paint::TEXT))
}
