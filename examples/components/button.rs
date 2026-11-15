//! button: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 默认 · Default
pub fn ex_0() -> El<Msg> {
    button_as("b1", "保存", Variant::Default, Msg::Nothing)
}

/// 安静 · Quiet
pub fn ex_1() -> El<Msg> {
    button_as("b2", "取消", Variant::Quiet, Msg::Nothing)
}

/// 强调 · Accent
pub fn ex_2() -> El<Msg> {
    button_as("b3", "发布", Variant::Accent, Msg::Nothing)
}

/// 不可用 · Disabled
pub fn ex_3() -> El<Msg> {
    button_as("b4", "发布", Variant::Accent, Msg::Nothing).disabled(true)
}
