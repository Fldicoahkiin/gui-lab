//! select: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 关着 · Closed
pub fn ex_0() -> El<Msg> {
    select("se1", &["北京", "上海", "广州"], Some(0), false, 0, Msg::Nothing, Msg::Nothing, Msg::Pick, Msg::Pick)
}
