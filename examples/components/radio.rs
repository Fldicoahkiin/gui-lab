//! radio: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use lab::el::{El};
use lab::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Pick(usize), Edit(TextEdit), Num(Edit), Value(f32) }

/// 选中第二个 · The second chosen
pub fn ex_0() -> El<Msg> {
    radio_group("r1", &["小", "中", "大"], 1, Msg::Pick)
}
