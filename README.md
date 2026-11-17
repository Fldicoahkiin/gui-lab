<div align="center">

# amenui

**A self-drawn interface framework in Rust. Every glyph, shape and picture has a true outline that can loosen, morph and come back together.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

</div>

amenui draws every pixel itself on wgpu, so the same code runs in a native window (Vulkan, Metal) and on
the web (WebGPU, or WebGL2 where WebGPU is missing). You write a view, a function from state to a tree of
plain values, and an update, a function from state and a message to the next state. The runtime does
layout, input, animation and drawing.

What sets it apart is that nothing on screen is a flat bitmap. Text, shapes, pictures and 3D meshes are
instances with real outlines, drawn first into a coverage buffer and a signed distance field, then
shaded by a material. That lets a press send a ripple through a page, a picture dissolve into ASCII or
halftone marks, and one element break into particles that fly onto the shape of another, while text you
need to read stays legible.

## Features

- **Elm-style views.** A view is `fn(&State) -> El<Msg>`, an update is `fn(&mut State, Msg)`. Layout is
  [taffy](https://github.com/DioxusLabs/taffy) (flexbox and grid), text is shaped by
  [parley](https://github.com/linebender/parley) at any size, and the layout is kept between updates so
  only what changed is laid out again.
- **Materials.** `plain`, `dot`, `halftone`, `ascii`, `print`, `lino`, `glitch`, `scribblr` and a
  debugging view of the distance field. At rest, body text in every material is identical to `plain`;
  a material shows when something loosens. Any subtree can use its own.
- **Themes and style presets as data.** A theme is colors, three type roles, a type scale and line
  widths. A preset bundles a theme, a material and its numbers, the annotation layer, interaction
  character and a transition, and exports to JSON.
- **Transforms.** Any element can break into particles and re-form as another (`Cmd::Morph`), and the
  parts two pages share move to their new places when the page changes (`Cmd::Flow`). Large morphs run
  on the GPU, up to a million particles.
- **Components as source.** Widgets from buttons and text fields to menus, tables, dialogs and a
  command palette. `amenui add` copies a widget's file and what it depends on into your project,
  shadcn-style, so you own and edit the code.
- **Text input.** Selection by click, drag, double and triple click; word jumps; clipboard; at least
  50 steps of undo; input method composition drawn at the caret; multi-line fields.
- **Pointer, touch and keyboard.** Mouse, pen and touch share one pointer model with momentum scrolling.
  Focus moves with Tab and Shift-Tab, Enter and Space activate, and widgets handle their arrow keys.
- **Accessibility mirror.** The runtime derives roles, names, values and states from the tree. On the
  web they become a hidden DOM tree that screen readers use, with focus kept in sync both ways.
- **Languages.** Message catalogs with CLDR plural rules, number and date formatting, right-to-left
  rows, and text that flows around the true outline of a picture or shape. English and Simplified
  Chinese ship with the framework, and applications install their own catalogs.
- **Draws only when needed.** The runtime says when the next frame is due. A still page draws no frames,
  and when only part of the page changes, only that part is drawn.
- **Built to be checked by models.** The `render` tool plays scripted input on a fixed clock and writes
  a PNG and the laid-out tree as JSON. The same arguments always give the same bytes.

## Install

amenui is not on crates.io yet. Depend on the repository:

```toml
[dependencies]
amenui = { git = "https://github.com/amenui-dev/amenui" }
```

It needs a recent stable Rust (edition 2024).

## Quick start

A counter, driven without a window or a GPU:

```rust
use amenui::el::{El, Paint, col, text};
use amenui::widgets::button;

#[derive(Clone)]
enum Msg { Inc }
struct Counter { n: u32 }

fn view(s: &Counter) -> El<Msg> {
    col().pad(24.0, 24.0).gap(12.0)
        .child(text(s.n.to_string(), 32.0, Paint::INK).key("count"))
        .child(button("inc", "Add one", Msg::Inc))
}

fn update(s: &mut Counter, m: Msg) {
    match m { Msg::Inc => s.n += 1 }
}

let faces = amenui::text::load_bundled(amenui::text::ASSETS).expect("bundled fonts");
let mut ui = amenui::ui::Ui::new(Counter { n: 0 }, view, update, faces, 400, 300, 1.0);
let (x, y) = ui.node("inc").unwrap().rect.center();
ui.press(x, y);
ui.release(x, y);
assert_eq!(ui.export_tree().find("count").unwrap().text.as_deref(), Some("1"));
```

Render one frame of a recipe and its tree:

```sh
cargo run --release --bin render -- --page recipe-counter --size 1280x800 --at 1000 --out out.png --tree out.json
```

Copy components into your own project:

```sh
cargo run --release --bin amenui -- add button dialog --to src/widgets
```

Run it in a native window (the `window` feature, on by default):

```rust,no_run
# use amenui::el::{El, col};
# #[derive(Clone)] enum Msg {}
# struct Counter;
# fn view(_: &Counter) -> El<Msg> { col() }
# fn update(_: &mut Counter, _: Msg) {}
impl amenui::app::App for Counter {}

fn main() -> Result<(), amenui::error::WindowError> {
    let window = amenui::app::Window::new("Counter").size(400.0, 300.0).from_args();
    amenui::app::native::run(window, |_| {}, |faces, w, h, scale| {
        amenui::ui::Ui::new(Counter, view, update, faces, w, h, scale)
    })
}
```

On the web, `amenui::web_app!(Counter, Msg, setup, make)` in the crate built as a `cdylib` generates
the module's exports, and [`web/amenui.js`](web/) (the `amenui` npm package) is the page side: one
canvas, input, text and the input method, the clipboard and the accessibility mirror.

## Documentation

- [Writing interfaces](AGENTS.md): the mental model, the ten things you write, the rules, and how to
  verify. Written for coding agents and just as useful to people.
- [Recipes](docs/recipes/): small complete applications, one file each in `examples/`.
- [Development](docs/guides/development.md): building, testing, and the tools in this repository.
- [Architecture](docs/reference/architecture.md): the pipeline and what each module owns.
- [Design](docs/DESIGN.md): the design contract, with every number and its acceptance check.
- [Documentation index](docs/README.md): guides, references and design records.
- [Changelog](CHANGELOG.md)

The [website](https://github.com/amenui-dev/website) shows every component, mark set, material and style,
with a documentation site in English and Simplified Chinese.

## Limitations

- No accessibility in the native window yet. The web mirror works; AccessKit is planned.
- Native clipboard and input methods are built but not yet verified with a real input method and a
  second application. Native touch has not been tried on a touch screen.
- Only the bundled fonts are used, so scripts they don't cover don't render yet: Arabic, Hebrew, Thai,
  Devanagari and emoji, and Hangul outside the pixel face.

## License

amenui is licensed under [MIT](LICENSE). The bundled fonts are under the SIL Open Font License 1.1;
see [fonts](docs/reference/fonts.md).
