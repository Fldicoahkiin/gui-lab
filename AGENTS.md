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
cmds.push(Cmd::Press { key: "go".into() });    // press an element as a person would (not counted as input)
cmds.push(Cmd::Commit { x: 400.0, y: 300.0 });  // keep the frame, play the new look's transition from here
cmds.push(Cmd::Focus { key: "go".into() });     // focus an element after this layout
cmds.push(Cmd::Scrub(Some(0.5)));               // hold the morph in progress half way (None: let it go on)
assert_eq!(cmds.len(), 6);
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

## Meshes, particles, presets

A 3D object is one more element: `el::mesh(MeshId::TORUS)` or `el::scene3d(Scene3d { items, camera, light, spec, picture })`,
sized like any element. Meshes are generated in code (`MeshId::CUBE`, `ICO[0..4]`, `TORUS`…; `Ui::add_mesh`,
`mesh::load_gltf`); `MeshInstances { mesh, transforms, paint, id }` draws many copies in one call. The global
material draws it (its outline and creases are edges in the nearness). Hear the pointer with
`.on_pick(|Pick| ..)` (`Pick::Hover` / `Pick::Press` carry `MeshHit { object, instance }`); turn it with
`widgets::orbit(key, scene, &state.orbit, Msg::Orbit)` and call `Orbit::update` / `Orbit::step` (see
`docs/recipes/mesh.md`). Morphs with many tiles run on the GPU by themselves (`Look::particles.gpu_above`).
A scene that changes into another with shared actors is a commit plus a flow (`docs/recipes/chapter.md`).
`Ui::on_pointer` tells the state where the pointer is.
A style preset is data: `preset::presets()`, change a number (`Preset::set_number("transition.steps.0.a", 3.0)`)
or read JSON (`Preset::from_json`), then `preset.apply(&look)`; a commit (`El::commit`) that applies it
plays its transition (`docs/recipes/preset.md`).
A preset's `post` is the finishing pass (bloom, chromatic offset, scanlines, grain, vignette, curvature, then a
dither and a palette — `lab::post`, all off by default, DESIGN.md §1.21) and its `marks` make marks on 3D elements
follow depth. `El::shed(Shed)` makes a moving element drop marks, `El::flow_field(FlowField)` carries marks along a
noise field over a box (speed 0: still), and `El::shift(dx, dy)` moves a subtree after layout (pointer parallax
without a layout).
Gestures are state (`lab::gesture`: `Spring`, `Animated`, `RubberBand`, `project`): `widgets::bottom_sheet` and `widgets::floating`
report drags, `Sheet::update` / `Floating::update` take them with the time, `step` runs on the tick while `moving()`; put them
after the page in `el::layers([...])` (docs/recipes/gesture.md). Under a live demo, `El::caption_params` prints the numbers that
drive it and lets them be edited (DESIGN.md §5.2). `widgets::nested_menu` steers submenus by a safe-zone triangle
(`El::on_move` hears the pointer over an element; `El::guide` draws an outline in the annotation layer). While data
loads, draw the real rows with `.skeleton()` as `widgets::Loading::show` says (docs/recipes/loading.md).
Fields and drawings (DESIGN.md §1.26): `el::canvas(Drawing)` draws dots and strokes in its own space (each a shape with
a true outline; its pointer handlers get points in that space; `El::on_point` hears move, press-drag and leave in one handler);
`lab::field` samples noise, bumps and metaballs on a `Grid`, traces iso-contours, advects a `Swarm` along `curl` or a stirred `Flow`,
and grows `GrayScott` inside a mask. A text element's glyphs can be posed one by one (`El::pose(|GlyphAt| Pose)`: move, turn,
scale, tone, alpha; no layout). `El::on_visible` says when an element can be seen (step a demo only then), `El::on_outline` sends a
subtree's true outline as a coverage grid (a mask). Every small mark draws from one frame budget (`budget::grant(n)`). `El::decode(Decode)`
makes an element appear by decoding and disappear by the same backwards. The Lab's experiments (`examples/lab/`) show each:
`experiment::frame(key, name, hint, piece, formula, &params, msg, label)` puts a piece on a 4:3 stage it fills, the formula on one
line and the numbers behind `[ params ]`; seen cells step every frame (with reduced motion they rest). Sampling (`morph::cover_at`,
`cells`, `on_outline`, decode) follows `Instance::xf` and poses, so a posed glyph loosens from where it is drawn; `cover_at` counts any
fill (a ground-toned panel too), `ink_at` only ink that shows. The lens touches only pictures and 3D elements that may loosen and that
no handler operates. The landing page and first scene, Body (`examples/body.rs`, `#body`), is written the same way: one posed text element (the
skin, its glyph count conserved), drawings for its volume, reflection, dust and the instrument markings, springs for `gather`
and the opening, a signed-distance form sampled to a spiral of surface points (DESIGN.md §6.2). `Pose::hue` picks a glyph's accent.
The field map (DESIGN.md §1.29, `lab::fieldmap`) is a coarse grid of energy and flow over the window that settles to exactly zero:
an element with `.stirr(Stirr::default())` stirs it behind itself as it moves (floating, bottom sheet and card stack do by default;
`Look::map` / a preset's `interaction.map` set how strong), `Cmd::Stir { at: StirAt::Point | Key | Path, .. }` stirs it from the state,
and it loosens and drifts what may loosen as fronts do (caps and responses unchanged; `Ui::looseness` reads it too). The `scribblr`
material (DESIGN.md §1.30, `lab::strand`) is plain at rest; loosened, an element's ink becomes one thread of constant width that knots
where the map has energy and trails off along its flow, then writes itself back (`Ui::strands()` lists the threads drawn; `Look::hold_ms`
holds its pose for a beat). `El::trace(Trace { points, progress, weight, dash, gap, pull, stir, paint })` draws a long path by progress
under any material, solid or dashed by arc length, pushed by the map and stirring it at its head.
Small type: `.tracking(em)` spaces letters (laid out by parley); `.whisper()` is lowercase, 0.6 em, in the bundled Latin serif
(`Font::Whisper`, `text::SERIF`). The annotation layer's scope-wide readouts (DESIGN.md §4.2–§4.3): `gaze::EDGE` edge notes (true values
only: scope size, pointer, material and theme, loosest element, non-zero map nodes), `gaze::OSC` a waveform of the map's energy along one
row, `gaze::LEVEL` a bar of its mean; `Ui::readout()` returns what they drew and `export_tree()`'s root has `field: { level, nonzero }`.
The tenth preset, Loose Thread (`loose-thread`), puts these together; `post.binary` (black and white only) and `post.invert` are
finishing-pass switches.
Text flows around true outlines with `.around(Around::new(vec![Silhouette::circle(..)], gap))`; a piece of a line
beside an obstacle is justified so both its edges are flush, adding at most `Around::justify_max` ems per gap
(default 0.25; inter-character next to CJK, inter-word at spaces, between Latin letters only with
`Around::justify_latin_letters`); Latin pieces then take up to `Around::track_max` (0.08 em) between letters and, if still short,
hyphenate the next word (`Around::hyphenate`, on by default); a piece that would need more stays start-aligned (DESIGN.md §1.2, "around").

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
cargo run --release --bin lab -- add dialog --to ../my-app   # copy a component (and what it needs) into a project
```

## Where things live

- `src/el.rs` the tree (`El`, builders), `src/ui.rs` the runtime, `src/scene.rs` instances and the field,
  `src/morph.rs` morphs, flows, recomposition, `src/style.rs` themes, materials, `Look`,
  `src/shader.wgsl` the three passes, `src/text.rs` type, `src/tree.rs` the exported tree,
  `src/drive.rs` scripted input and `render`, `src/error.rs` errors.
- `src/widgets/` one file per widget (header: purpose, props, states, messages, usage); `registry.json`.
- `examples/` the recipes, `docs/recipes/` their pages; `src/gallery/` the gallery app.
- `docs/DESIGN.md` the contract (read §1.15–§1.17 for ripples, recomposition and this surface).
