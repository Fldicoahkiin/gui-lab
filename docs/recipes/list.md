# A list with selection

`list` is one focus stop: Up / Down / Home / End move the selection, a press picks a row. Rows are keyed `key:i`, so a script can press `files/list:3`.

The code is `examples/list.rs`, compiled into the library as `amenui::recipes::list`; `tests/recipes.rs` renders it, plays its interaction, and fails if this block differs from the file.

```rust
//! Recipe: a list with selection (docs/recipes/list.md). The list is one focus stop; Up / Down move the
//! selection, a press picks a row, and the state says which row is selected.
use amenui::el::{El, Paint, col, text};
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::list;

pub struct Files { pub picked: Option<usize> }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Pick(usize) }

const ROWS: [(&str, &str); 6] = [("草图.png", "2.1 MB"), ("提纲.md", "4 KB"), ("预算.csv", "18 KB"), ("封面.jpg", "860 KB"), ("录音.m4a", "12 MB"), ("合同.pdf", "320 KB")];

pub fn view(s: &Files) -> El<Msg> {
    let picked = s.picked.map_or("没有选中".to_owned(), |i| format!("选中：{}", ROWS[i].0));
    col().key("files").pad(32.0, 28.0).gap(14.0)
        .child(list("files/list", &ROWS, s.picked, 36.0, 220.0, Msg::Pick).width(360.0))
        .child(text(picked, 14.0, Paint::DIM).key("files/picked"))
}

pub fn update(s: &mut Files, m: Msg) {
    match m { Msg::Pick(i) => s.picked = Some(i) }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Files, Msg> {
    Ui::new(Files { picked: Some(1) }, view, update, faces, w, h, scale)
}
```

See it:

```sh
cargo run --release --bin render --page recipe-list --events pick.json  # [{"at_ms":300,"kind":"press","target":"files/list:3"}]
```
