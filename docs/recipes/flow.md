# A page change with a shared part

Give the part both pages share the same `flow` id, and push `Cmd::Flow` in `update` when the page changes. The runtime takes the commands before the next layout, pairs the instances by flow id and moves them; the rest of the old page loosens and fades.

The code is `examples/flow.rs`, compiled into the library as `lab::recipes::flow`; `tests/recipes.rs` renders it, plays its interaction, and fails if this block differs from the file.

```rust
//! Recipe: a page change with a shared part (docs/recipes/flow.md). The card's title carries a flow id
//! on both pages; `update` asks for `Cmd::Flow`, and the title moves to its new place instead of being
//! replaced, while the rest of the old page dissolves.
use lab::el::{El, Font, Paint, col, row, text};
use lab::morph::{Cmd, FlowSpec};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::{Variant, button_as, card};

pub struct Pages { pub open: Option<usize>, pub cmds: Vec<Cmd> }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Open(Option<usize>) }

const TITLES: [&str; 3] = ["山脊", "河口", "海岬"];

pub fn view(s: &Pages) -> El<Msg> {
    let title = |i: usize, size: f32| text(TITLES[i], size, Paint::INK).font(Font::Display).key(format!("title/{i}")).flow(format!("title-{i}"));
    match s.open {
        None => TITLES.iter().enumerate().fold(row().key("grid").pad(32.0, 28.0).gap(16.0), |r, (i, _)| {
            r.child(card(format!("card/{i}"), None, title(i, 20.0)).width(180.0).on_press(Msg::Open(Some(i))))
        }),
        Some(i) => col().key("detail").pad(32.0, 28.0).gap(16.0)
            .child(button_as("back", "‹ 返回", Variant::Quiet, Msg::Open(None)))
            .child(title(i, 44.0))
            .child(text("详情页：标题从卡片里流到这里。", 15.0, Paint::DIM)),
    }
}

pub fn update(s: &mut Pages, m: Msg) {
    match m { Msg::Open(o) => { if o != s.open { s.cmds.push(Cmd::Flow(FlowSpec::default())); } s.open = o; } }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Pages, Msg> {
    Ui::new(Pages { open: None, cmds: vec![] }, view, update, faces, w, h, scale).on_commands(|s| std::mem::take(&mut s.cmds))
}
```

See it:

```sh
cargo run --release --bin render --page recipe-flow --events open.json --at 1200
```
