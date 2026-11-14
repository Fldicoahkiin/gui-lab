# AGENTS.md — writing interfaces with gui-lab

gui-lab is a self-drawn interface framework in Rust (wgpu; the same code runs natively and on the web).
Everything on screen — glyphs, shapes, pictures — is an instance with a true outline that can loosen,
morph and condense again; how it looks is a swappable material. You write a view (state → tree) and an
update (state, message → state); the runtime does layout, input, animation and drawing.

## The mental model

```text
view(&state) → El tree → layout (taffy + parley) → instances (glyph / shape / picture)
  → stage 1: ink + near images (true outlines) → stage 2: material → stage 3: annotation layer
input → hit → message → update(&mut state) → view again        input → field (fronts, lens)
```

There is one way to make things move: events put things in the field (a front from a press, the lens),
and commands ask the runtime (`Cmd::Morph`, `Cmd::Flow`). Widgets never animate themselves.

## The ten things you will actually write

Every block below is compiled by `cargo test` (this file is a doc-test of `lab::agents`).

1. A view and an update (Elm style; messages are plain values):
```rust
use lab::el::{El, Paint, col, text};
use lab::widgets::button;
#[derive(Clone)] enum Msg { Inc }
struct S { n: u32 }
fn view(s: &S) -> El<Msg> {
    col().pad(24.0, 24.0).gap(12.0)
        .child(text(format!("{}", s.n), 32.0, Paint::INK).key("count"))
        .child(button("inc", "加一", Msg::Inc))
}
fn update(s: &mut S, m: Msg) { match m { Msg::Inc => s.n += 1 } }
let _ = (view(&S { n: 0 }), update as fn(&mut S, Msg));
```

2. A runtime, headless (tests, tools); press by position, read the tree:
```rust
# use lab::el::{El, Paint, col, text}; use lab::widgets::button;
# #[derive(Clone)] enum Msg { Inc } struct S { n: u32 }
# fn view(s: &S) -> El<Msg> { col().child(text(s.n.to_string(), 20.0, Paint::INK).key("count")).child(button("inc", "+", Msg::Inc)) }
# fn update(s: &mut S, _: Msg) { s.n += 1 }
let faces = lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts");
let mut ui = lab::ui::Ui::new(S { n: 0 }, view, update, faces, 400, 300, 1.0);
let (x, y) = ui.node("inc").unwrap().rect.center();
ui.press(x, y);
ui.release(x, y);
assert_eq!(ui.export_tree().find("count").unwrap().text.as_deref(), Some("1"));
```

3. Layout: rows, columns, sizes, gaps, padding (logical px; nothing shrinks unless asked):
```rust
use lab::el::{El, Paint, row, col, space, text};
let bar: El<()> = row().fill_width().pad(16.0, 8.0).gap(12.0)
    .child(text("标题", 18.0, Paint::INK).grow(1.0))
    .child(space(80.0, 30.0).radius(4.0).fill(Paint::ACCENT));
let page: El<()> = col().fill_width().fill_height().child(bar).child(col().grow(1.0).scroll());
let _ = page;
```

4. Keys: give every element you will look up, animate, or that holds state a stable key:
```rust
use lab::el::{El, Paint, col, text};
let rows: El<()> = (0..3).fold(col(), |c, i| c.child(text(format!("第 {i} 行"), 14.0, Paint::TEXT).key(format!("row/{i}"))));
let _ = rows;
```

5. Numbers are number controls, never sliders as the main way (Draft while typing, Set on commit):
```rust
use lab::el::El;
use lab::widgets::{Edit, Spec, number};
#[derive(Clone)] enum Msg { Width(Edit) }
const W: Spec = Spec { min: 0.0, max: 64.0, step: 1.0, decimals: 0, unit: "px" };
let draft: Option<String> = None;
let n: El<Msg> = number("width", "宽度", 12.0, W, draft.as_deref(), Msg::Width).width(320.0);
let _ = n;
```

6. Text fields keep a `TextEdit` in the state:
```rust
use lab::el::El;
use lab::widgets::{TextEdit, input};
#[derive(Clone)] enum Msg { Name(TextEdit) }
let name = TextEdit::new("林");
let field: El<Msg> = input("name", &name, "名称", 280.0, 40, Msg::Name);
let _ = field;
```

7. Ripples: a press reaches only its own control unless you say more; decide what loosens:
```rust
use lab::el::{El, Paint, Response, Scope, row, space};
use lab::widgets::button;
let area: El<()> = row().key("area").ripple_scope().gap(16.0)
    .child(button("go", "按下", ()).ripple(Scope::Container(None)))
    .child(space(60.0, 40.0).fill(Paint::ACCENT).respond(Response::Loosen))
    .child(space(60.0, 40.0).fill(Paint::hue(1)).respond(Response::Ignore));
let _ = area;
```

8. Commands for what moves: keep them in the state, hand them over with `on_commands`:
```rust
use lab::morph::{Cmd, FlowSpec, MorphSpec};
let mut cmds: Vec<Cmd> = vec![];
cmds.push(Cmd::Morph { from: "a".into(), to: "b".into(), spec: MorphSpec::default() });
cmds.push(Cmd::Flow(FlowSpec::default()));     // before a page change: shared `.flow(id)` parts move
cmds.push(Cmd::Press { key: "go".into() });    // press an element as a person would
assert_eq!(cmds.len(), 3);
```

9. Materials and the look: one global material, overrides per subtree:
```rust
use lab::el::{El, Paint, text};
use lab::style::{Look, Material, LIGHT};
let look = Look { theme: LIGHT, material: Material::Dot, legible_cap: 0.15, ..Look::default() };
let card: El<()> = text("半色调", 20.0, Paint::INK).material(Material::Halftone.id());
let _ = (look, card);
```

10. Content that changes recomposes when it is keyed; turn it off where it must be immediate:
```rust
use lab::el::{El, Paint, text};
let count: El<()> = text("12", 28.0, Paint::INK).key("count");            // recomposes 12 → 13
let clock: El<()> = text("09:41", 14.0, Paint::DIM).key("clock").recompose(false);
let _ = (count, clock);
```

## The rules

- One way to do each thing: state in the state, changes in `update`, motion through the field and `Cmd`.
- Key everything you will find again (tests, `--events` targets, flows, morphs, recomposition). Keys must
  be unique on a page; `Ui::problems()` lists duplicates and says what to write.
- Every numeric parameter is a named field with a default and is shown as a number control.
- Text to read and controls stay legible: fronts nudge them and loosen them at most `Look::legible_cap`.
  Mark decoration `respond(Response::Loosen)`; mark a showcase `legible(false)`.
- No third-party or character images in the repository.
- Verify with pictures *and* data: render a frame and read its tree.

## How to verify

```sh
cargo run --release --bin render -- --page button --size 1280x800 --at 1000 --out out.png --tree out.json
cargo run --release --bin render -- --page recipe-counter --events inc.json --at 300 --out c.png --tree c.json
```

`inc.json`: `[{"at_ms": 100, "kind": "press", "target": "inc"}, {"at_ms": 130, "kind": "release", "target": "inc"}]`
(kinds: press, release, move — `x`, `y` or `target`; key — `key` as a DOM name, `shift`, `cmd`, `alt`;
wheel — `dx`, `dy`; text — `text`). The clock is fixed at 60 Hz from 0, so the same arguments give the
same bytes. The tree (`--tree`, or `Ui::export_tree()`) has each element's key, kind, role and label,
rect, text, material, response, legibility, focus / hover / press, scroll offset and looseness.
`--material` takes `plain`, `dot`, `halftone`… (or a number), `--theme` takes `dark`, `light`, `paper`…

## Commands

```sh
cargo test                                   # everything that needs no GPU (and a few that use one if present)
cargo clippy --all-targets                   # no warnings allowed
cargo run --release --bin check              # pixel acceptance on the GPU (lavapipe works)
cargo run --release --bin render -- --help   # one frame + tree
cargo run --release --bin film -- out ripple # filmstrips of ripples and recomposition
cargo run --release --bin window -- button   # a native window
```

## Where things live

- `src/el.rs` the tree (`El`, builders), `src/ui.rs` the runtime, `src/scene.rs` instances and the field,
  `src/morph.rs` morphs, flows, recomposition, `src/style.rs` themes, materials, `Look`,
  `src/shader.wgsl` the three passes, `src/text.rs` type, `src/tree.rs` the exported tree,
  `src/drive.rs` scripted input and `render`, `src/error.rs` errors.
- `src/widgets/` one file per widget (header: purpose, props, states, messages, usage); `registry.json`.
- `examples/` the recipes, `docs/recipes/` their pages; `src/gallery/` the gallery app.
- `docs/DESIGN.md` the contract (read §1.15–§1.17 for ripples, recomposition and this surface).
