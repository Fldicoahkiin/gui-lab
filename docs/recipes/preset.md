# A custom preset

A style preset is plain data (`amenui::preset::Preset`): theme, material and its numbers, faces, annotation, the interaction character (press feedback, fronts, default responses, recomposition) and a transition. Start from one of the twelve and change what you want, or read one from JSON (`Preset::from_json`; the gallery's 风格切换 page exports any preset). `Preset::apply` gives the look; when a commit applies it, its transition plays between the frame as it was and the new one — here rows that reveal from the top. A second press in the middle goes on from what is on the screen.

A preset also carries `post`, the finishing pass (`amenui::post::PostSpec`: bloom → chromatic offset → scanlines → grain → vignette → curvature, then a dither and a palette, each off by default), and `marks` (`depth_scale`, `depth_dim`) for 3D elements. They are numbers like the rest: `p.set_number("post.scanlines.strength", 0.2)`, and they travel in the `look:v1:` recipe.

The code is `examples/preset.rs`, compiled into the library as `amenui::recipes::preset`; `tests/recipes.rs` renders it, plays its interaction, and fails if this block differs from the file.

```rust
//! Recipe: a custom preset (docs/recipes/preset.md). A preset is data: start from one of the twelve, change
//! its numbers (or read it from JSON), and give the runtime the look it makes. Its transition plays when
//! a commit applies it — here a button that switches between the plain look and the custom one.
use amenui::el::{El, Paint, col, text};
use amenui::preset::{Preset, Step, StepKind, Transition, presets};
use amenui::style::Look;
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::{Variant, button_as};

pub struct App { pub custom: Preset, pub on: bool }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Switch }

/// Bauhaus's look, arriving by rows of 18 px that reveal top to bottom, softly, in 600 ms.
pub fn custom() -> Preset {
    let mut p = presets().into_iter().find(|p| p.hash == "bauhaus").expect("a preset");
    p.name = "Rows".into();
    p.transition = Transition::of(600.0, &[Step { kind: StepKind::Reveal, from: 0.0, to: 1.0, a: 2.0, b: 0.0, c: 18.0, d: 24.0, e: 0.0 }]);
    p.interaction.press_ms = 240.0;
    p
}

pub fn view(s: &App) -> El<Msg> {
    col().key("page").pad(32.0, 28.0).gap(16.0)
        .child(text(if s.on { "自己的风格" } else { "原来的样子" }, 28.0, Paint::INK).key("title"))
        .child(text("按下按钮：新的样子按自己的过渡出现。", 15.0, Paint::DIM))
        // A commit: the frame as it was stays, and the preset's transition reveals the new one.
        .child(button_as("switch", "切换", Variant::Accent, Msg::Switch).commit())
}

pub fn update(s: &mut App, m: Msg) {
    match m { Msg::Switch => s.on = !s.on }
}

fn look(s: &App) -> Look { if s.on { s.custom.apply(&Look::default()) } else { Look::default() } }

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<App, Msg> {
    Ui::new(App { custom: custom(), on: false }, view, update, faces, w, h, scale).on_look(look)
}
```

See it (mid-transition at 400 ms):

```sh
cargo run --release --bin render -- --page recipe-preset --events switch.json --at 400 --out rows.png
```

`switch.json`: `[{"at_ms": 100, "kind": "press", "target": "switch"}, {"at_ms": 130, "kind": "release", "target": "switch"}]`
