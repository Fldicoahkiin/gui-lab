# Changelog

Newest first. The acceptance checks for each part are in [DESIGN.md §7](docs/DESIGN.md).

Versions follow [Semantic Versioning](https://semver.org) as Cargo reads it: before 1.0, a change to
the second number (0.1 → 0.2) may break code, and a change to the third (0.1.0 → 0.1.1) does not. The
crate and the `amenui` npm package (`web/`) always carry the same version, because the page side must
match the module that `web_app!` generates. The minimum Rust is the one CI builds with (`rust-version`
in Cargo.toml); raising it is a breaking change before 1.0 only in the second number.

## Controls that hold under effects, the warm-up in the hosts, opacity and scoped commits

- Breaking: `Cmd::Commit` has a `scope: Option<String>` (`None` is the old behaviour); `MorphSpec` is
  `Clone`, not `Copy`, and has `from_last`; `Look` has `overlay_at` and `overlay_inset`, and `Gaze` has
  `echo` and `echo_ms`, so struct literals of them need the new fields or `..Default::default()`.
  A drive file's `commit` without `scope` reads as before.
- Both hosts warm the renderer up a piece a frame within 6 ms (`Renderer::warm_frame`, `WARM_MS`):
  the first frame is the plain page. `Ui::warm` and `Ui::on_warm` say how far it is; the web export
  `warm()` gives `[done, total]`, and `web/amenui.js` sets `aria-busy` and calls `opts.onWarm`.
- A frame drawn while something moves owes one more, so a motion that ends between frames ends on the
  settled page; a host no longer needs to redraw by hand after one.
- An element that leaves with its page takes its field map and decode with it; one taken away alone
  decodes out with its own spec inside its clip. `audit::off_palette` counts colours no theme makes.
- Controls inside a subtree marked `legible(false)` keep their fill and label as at rest; a tab list's
  heads and rule are the control.
- `El::pass` lets the pointer through to what is under it; `El::opacity` fades a subtree as one (at
  0 it takes no pointer); `El::commit_scope` keeps a commit inside one element's box.
- `Ui::morph_from_last` and `MorphSpec::from_last` fly an element from where it was drawn last frame,
  across a route change.
- The readout goes in `Look::overlay_at`'s corner and moves on clockwise rather than cover text to read.
- The INK leader is dashed and routed round text to read; `Gaze::echo` keeps fading earlier INK boxes.
- With reduced motion, what comes and goes fades over 150 ms.
- Looks: the thread's clearances move its control points, so the curve stays smooth; a thin accent rule
  under `charr` is one unbroken row; the Crack's pieces start to leave at 0.28; a press's flash in
  `plain` keeps a filled control's label at 4.5:1.

## Materials named for what they do, and as source

- Breaking: the six printing materials are named for what they do: `dot` → `dottr`, `halftone` →
  `halftonr`, `ascii` → `charr`, `print` → `smudgr`, `lino` → `carvr`, `glitch` → `glitchr` (the
  `Material` variants and their strings); `plain`, `near` and `scribblr` keep theirs. `PicFx` is
  `Markr` (`El::markr`), the finishing pass's `Dither` is `Dithr` (`post.dithr`, `DithrKind`),
  `Cmd::Front` / `Cmd::FrontAt` are `Cmd::Ripplr` / `Cmd::RipplrAt`, and `El::ripple`,
  `El::ripple_scope` and `El::wipe` are `El::ripplr`, `El::ripplr_scope` and `El::wipr`. No aliases: an
  old material name is refused and the error names the new one, and `LookError` gives the closest name
  for any other. Look recipes start with `look:v2:`; a `look:v1:` recipe is refused with the renames
  listed. `Material` has a variant for added materials, so cast with `Material::id()`, not `as`.
- Materials as source (§1.6.4): each material is `src/materials/<name>.wgsl` with its name and numbers
  in a header; `amenui add material <name>` copies one into a project, and
  `Look::material_source(name, wgsl, params)` adds one back under a name of its own (ids 9–15,
  `Material::Custom`). `Look::numbers(m)` reads the numbers a material is drawn with.
- `El::effect(Effect { material, response, scope })` puts a material, a response and a scope on a
  subtree in one line, and `el::wrap_text` turns plain text or Markdown into a page to read.
- The thread (§1.30): one large lopsided knot and smaller ones (`knot_fall`), a few chords escaping
  (`escape`), long flat tails with a few gentle bends (`tail_turns`) at half the line's tone
  (`tail_tone`), all in `strand::ThreadShape`; it no longer lingers between a half-drawn title and a
  thread when it draws back (`strand::shown`), and it passes behind pictures without changing colour.
- Sample sheets that check themselves (`amenui::sample`, `sheet --material <name> --loose … --arrival`).
- The audit finds text a material wears away at rest (`Eroded`, P1 under 85 % of plain's ink); a
  clipped finding names the side, the distance and what cuts it. Annotation tags and `INK` readouts
  keep off text to read and off each other.
- `near` keeps a label in the ground's colour on a solid button whole. Text on a form fades its rim
  from `edge` 0.6 (range 0–0.8), and in `form3d` the pointer's light falls off smoothly.
- The warm-up (§1.23): the material arrives on the page at rest, then over 120 ms (`ui::RAMP`) the page
  loosens to its caps and condenses, so the arrival no longer jumps.
- Every public item of `strand`, `fieldmap`, `field`, `hit`, `audit`, `form` and `materials` has a doc
  comment.

## Text on a form, the warm-up and the audit

- Glyph poses have a depth and a facing (`Pose::z`, `Pose::facing`; the defaults draw as before), and a
  text element's glyphs are drawn far to near. `El::form(FormFx { back, edge, overlap, fog })` keeps text
  on a form legible: what faces away is left out, what is edge on fades, a glyph mostly covered by nearer
  ones is not drawn (measured on the true ink), the far side is dimmer (§1.26).
- `amenui::form`: `Form` wraps a block of text onto a sphere, a surface function or a mesh seen by a
  `mesh::Camera` (`Form::pose_of`), and `Form::hit` finds the pointer on the surface. The recipe `form3d`.
- `El::reveal_by(Reveal::Word | Line)`: clips, poses and the window show words and lines whole.
- Hits follow what is drawn: `Ui::glyph_at`, `Point::glyph` and `Point::z`; `El::turn` turns an element
  and its subtree, and it is pressed where it is drawn; `MeshHit` has `point`, `normal`, `t` and `uv`, and
  `Mesh::ray` casts a ray at a mesh.
- A button or link inside an element that drags (a scene with `on_point`) takes its own press.
- The warm-up (§1.23): `Renderer::begin` draws plain at once, `Warmup::step` / `Renderer::warm_step` make
  the rest a piece at a time, and the page condenses into its look when it is ready;
  `Offscreen::try_warming`. `Ui::prepare`, `Ui::ready` and `Ui::readiness` run an application's own work
  in the frames' spare time; the annotation layer reads `READY`.
- The audit (§1.31): `amenui::audit`, `drive::audit`, `render --audit` (with `--strict`, a P0 or P1 fails):
  text cut off, contrast, glyphs over each other, text too small, a page that never settles. The
  annotation layer's `INK` kind draws the same measured ink box.
- What the audit found: `Paint::DIM` is 0.66 (it was 0.5, 3.2:1 on light); the light theme's accent is
  darker (5.6:1) and the selection tint 12 %; `near` keeps text's ink whole; the loading recipe no longer
  asks for a frame every tick.

## Running applications

- `amenui::app`: the framework runs applications itself. `app::native::run` opens a winit window
  (feature `window`, on by default) with the system clipboard, the input method, touch, frames on
  demand, and `--shot` / `--exit-after` for checks under Xvfb. `app::web::Host` and the `web_app!`
  macro generate a wasm module's exports from an application's state and message types. The `App`
  trait is how a shell routes, reads the route, takes clipboard requests, and sets the look, the
  language and reduced motion; every method has a default.
- Native screen readers: `app::access` builds an AccessKit tree from the accessibility mirror for the
  window (UI Automation, NSAccessibility, AT-SPI) and turns Focus, Click, Increment and Decrement into the
  runtime's focus, activation and arrow keys.
- The mirror says more: `El::expanded`, `El::description`, `El::live` (`Role::Status` is polite by
  default), `El::invalid`, `El::required`. The accordion, select and popover report `expanded` instead of
  `checked`.
- Fonts beyond the bundled ones: `Face::fallback` for faces tried after the bundled ones, and
  `Face::system()` for the system's installed fonts (feature `system-fonts`, on by default; the native
  window adds it). `Text::missing_glyphs`. The `complex-scripts` feature for Thai, Lao, Khmer and Burmese
  line breaking.
- Any language: `Locale::Other(Tag)` for any BCP 47 tag, falling back to English, with CLDR plural rules,
  right-to-left direction by language or script, and its own `locale.name` and `locale.date`. Arabic ships
  (`Locale::AR`, `assets/i18n/ar.txt`). `Locale::ALL` lists the three shipped locales;
  `i18n::installed()` lists the application's. `Locale::name` returns a `String`.
- Focus: `El::tab_order` (positive first, 0 in document order, negative out of Tab), `El::skip_to`
  and `El::focus_shown`, and the `skip_link` component.
- `App::set_appearance` (`Appearance { dark, high_contrast }`), `App::files_hovering` and
  `App::files_dropped` (`Dropped` with a path natively, name and bytes on the web), from both shells.
- The native window draws with the bundled faces compiled in (`text::embedded`), so a shipped binary
  finds its fonts anywhere. `WindowError::Font` is gone.
- `amenui add` writes from the sources compiled into it (`sources::file`, `add::write` replaces
  `add::copy`), so an installed command works without the repository.
- Crate metadata for crates.io (description, keywords, categories, `rust-version`); the package
  leaves out CI (8.7 MiB compressed, under the 10 MiB limit).
- The native window recovers from a lost GPU device: new device and renderer, `setup` again, the `Ui`
  untouched. `native::run`'s `setup` is now `Fn`.
- `web/amenui.js`, the `amenui` npm package: the page side of the web entry (`mount`, `stage`, `css`).
- `offscreen::read_back`: one frame drawn into a texture of a given format and read back.
- `error::WindowError`.
- CI: clippy for native and web, rustdoc, the tests on lavapipe, and the website's tests against each
  framework commit.

## The framework and the website in two repositories

- The site (the gallery shell and its pages, the skeleton demo, Body and the Lab, the site documentation,
  the web shell, the native window, and the sample, acceptance and timing tools) moved to
  [amenui-dev/website](https://github.com/amenui-dev/website), with its history.
- `i18n::install`: an application installs its own message catalogs, looked up before the framework's.
  The framework's catalogs keep only the components' own strings (`widget.*`).
- `amenui::pictures`: sample pictures computed in code (formerly in `demo.rs`). `amenui::sources`:
  `AGENTS.md`, `llms.txt`, `registry.json`, `CHANGELOG.md` and every component's source, compiled into
  the library. `text::ASSETS`: where the fonts are on disk.
- `drive::render_with` takes an application's own pages; `drive::open` opens recipes only, and so does
  the `render` tool.
- The library is no longer a `cdylib`. The web entry point (`wasm-bindgen`), `winit` and `arboard` moved
  to the website repository with the native window.

## The field map and trails, the thread material, small type, Loose Thread

- Body is English only, whatever the interface language. Its language marking reads
  `LANGUAGE  EN / ZH`, and pressing it changes the language of the rest of the gallery.
- The field map (`src/fieldmap.rs`): a coarse grid over the window whose nodes hold energy and a flow
  direction. It steps at a fixed 1/60 s, spreads, decays by lifetime, meanders (`meander`) and creeps
  along ink (`creep`), returns to exactly zero within a bounded time, and needs no frames at rest.
- Trails: an element with `El::stirr` stirs the map along the path it moved. `Cmd::Stir` stirs a
  point, an element or a stretch of a path. The floating box, the bottom sheet and the card stack trail
  by default; the preset sets how strongly (Normcore has none).
- Reading the map: in the G-buffer stage it is one more source of looseness (still bounded by the
  legibility cap and each element's response), and instances that can be pushed drift along its flow
  (pictures and 3D elements loosen in place). The `dot` material's ground dots and `glitch`'s ground
  read it; `Ui::looseness` uses the same value. An element that trails and has a ground fill is drawn
  on a layer above, so loose decoration it passes over does not show through it.
- The `#stirr` page, with twelve numbers as number controls; one more line in the F2 readout; `check`
  item 19 (the former item 19, effects, is now item 17), `bench … map` and `craft <dir> stirr`.
- The `scribblr` thread material (`src/strand.rs`). At rest it is byte-identical to `plain`. Loosened,
  the fill fades and the ink becomes one continuous thin line: it first follows the glyphs' true
  outlines, bridged into one stroke (clean outlined letters); looser, it drops vertices step by step and
  merges into long polylines across several glyphs that wander off. The one to three places with the
  most energy knot into large stars, one long tail enters from the scope's edge and another leaves
  through the opposite edge, and it writes itself back in reverse. Text to read and controls never turn
  into thread, fade or move. Eight numbers are in the material parameters (`reach` 40, `knot_r` 48,
  `tail` 480, `angular` 0.9), and `Look::hold_ms` holds the thread's pose for a beat.
- Long paths with `El::trace`: a polyline drawn by progress, solid or dashed (`dash`, `gap`, by arc
  length from the start), bent by the flow, stirring the map at its head. "Draw a line" on `#stirr`.
- The thread starts from the outline as the G-buffer stage drew it, plus the offset by which the map
  pushed the instance. Vertices are ranked by one Ramer–Douglas–Peucker pass, slide onto their chord
  and are then removed, so the line follows its chords and nothing jumps on the way back. Knots are
  long chords through a center. Tails are cut at the scope's edge and stop before text to read.
  `strand::figure` draws a tangle from a fixed seed as page content.
- The `#material-scribblr` page; `check` item 20, `bench … strand`, `craft <dir> scribblr` and
  `tests/strand.rs`.
- Letter spacing with `El::tracking`. Quiet type with `El::whisper()`: lowercase, 0.6 em tracking, in a
  serif (Source Serif 4 at its 8 pt optical size, Latin only, 82 KB, OFL).
- Three new readouts in the annotation layer: edge notes `EDGE` (a hairline, ticks and a pointer mark at
  the scope's right edge, with the scope size, the pointer position, the material and theme, the
  loosest element and the number of non-zero map nodes set vertically), a waveform `OSC` (the energy of
  one row of nodes as a one-device-pixel polyline) and a level bar `LEVEL` (the mean energy). The
  annotation layer honours instance transforms, used only to turn the edge notes by 90°. The exported
  tree gains `field: { level, nonzero }`.
- The tenth preset, Loose Thread: white thread on black (or paper white), one accent, `scribblr`, only
  edge notes and corner marks on hover; the map meanders, creeps along ink and lives two seconds (a title
  the pointer sweeps loosens past 0.8 and comes back within three seconds); the thread holds every other
  beat; a 700 ms transition loosens into thread and then draws the outline. Its style page is English
  only: a lowercase serif title, one picture, one sentence, three controls as one line of small type in
  the footer, and a drawn tangle at rest.
- The finishing pass gains black and white only (`binary`: a black-and-white palette, no dither, line
  widths rounded; text, thread and annotations are thresholded by coverage rather than brightness, and
  pictures get a two-level ordered dither) and invert (`invert`: `1 − colour` at the end).
- Annotation corner marks sit 6 px outside the element's rect, off its own text.
- Fixed theme ids and style-page descriptions that had drifted after themes and styles were removed.
- `check` items 7.2 and 7.3 (edge notes, waveform, level bar, edge notes at 1080p) and 20.6 and 20.7
  (black and white only, invert), three tests in `tests/gaze.rs`, the Loose Thread page in
  `tests/i18n.rs`, `craft <dir> loose-thread` and `craft <dir> thread-ref`.

## Foundation leftovers, the Lab as a wall, Body

- Sampling follows transforms: `morph::cover_at` maps points back through each instance's own
  transform, and `cells` and `ink_bounds` use the turned bounding box, so a posed or turned glyph
  loosens from where it is drawn, and masks and decode work there too.
- Hover and focus corner marks land on their target in the frame the pointer arrives (they used to chase
  it every frame and trail behind), with only a 70 ms close-in and fade.
- The lens touches only pictures and 3D elements that may loosen and that the pointer is not operating,
  added per instance in the G-buffer stage as fronts are. Looseness belongs to the instance's own ink
  (its ink wins over another's distance band) and no longer spills onto neighbouring decoration.
- Dragging, throwing and settling the floating box joined `check`'s partial-redraw comparison (every
  material, two at 2× scale).
- Decode counts coverage of the outline, not colour, so a dialog filled with the ground colour is
  covered too at the finest level.
- DESIGN.md no longer mentions the removed chrome, liquid and collage materials.
- The Lab: each stage takes over 70% of its cell and the piece fills it, the formula is one line and the
  numbers fold into `[ params ]`. Cells that can be seen keep running and stop when scrolled away (they
  rest with reduced motion). Every cell was redone to fill its stage, with the blue under the pointer;
  decode's exploded view became planes seen at an angle, each with a leader and a label.
- Removed the film (the prologue and chapters, the slate, the timeline, before-and-after, each chapter's
  caption), the xray material and its preset (Transparent Electronics), and the tests, checks, docs and
  strings that existed only for them. The exploded scene no longer uses xray.
- Body (`#body`) is the new full-bleed landing page. A page of readable text appears, then gathers into
  a slowly turning, luminous body made of its own glyphs (the skin, glyph count conserved) and more than
  ten thousand volume marks, with a key light, rim light, fog, bloom, a reflection and halo on the
  ground, three layers of dust, parallax and a breathing lens. Small markings in the picture (form,
  seed, gather / release, language, Lab, components); an opening at the pointer shows the original text.
- Bloom samples at a per-pixel noise angle, so text no longer shows ghost copies. `Pose::hue` gives one
  glyph an accent colour.

## Fields, the Lab, decode, a narrower scope, the frame budget

- Per-instance transforms (`Instance::xf`: turn, scale and shear about the instance's own center) and
  glyph poses (`El::pose`), so text can follow a path, wrap a sphere and be lit while its outlines stay
  true.
- Drawings (`el::canvas(Drawing)`: dots and strokes are shapes with true outlines) and `El::on_point`
  (move, press-drag and leave in one handler, in the canvas's coordinates).
- Fields (`amenui::field`): noise, bumps, metaballs, curl, grids, marching-squares contours, vector
  flows, advected swarms and masked Gray–Scott.
- `El::on_visible` (run only while seen) and `El::on_outline` (an element's true outline handed to the
  state as a mask).
- The frame budget (`amenui::budget`): morph tiles, shed marks, flow fields, swarms and drawing marks all
  draw from one budget, which shrinks when a frame runs over and recovers when there is room; the readout
  shows it.
- The Lab (`#lab`): ten experiments (lit text, contours, silk, grown shapes, mercury, text on a path,
  text on a sphere, orbits, a spring, the card stack), one example file each, each with its own page.
- The decode transition (`El::decode`): an element resolves from coarse to fine over its true outline
  and disappears by the same backwards; its documentation page has an exploded 3D view and a timeline.
- Removed the chrome, liquid and collage materials, the Neo-Y2K and Mixed media presets, and film
  chapters 11 and 12 (with the finale).

## Interaction craft

- Gesture physics (`amenui::gesture`): an analytically stepped spring with velocity hand-off on
  release, a rubber band, throw projection and velocity tracking.
- A bottom sheet (detents, dismissal by projection, rubber banding, a scrim, hand-off from scrolling)
  and a floating box (the corner picked by projection).
- The formula caption: a line under a demo that prints its real parameters, each editable.
- `el::layers`: children stacked in one box.
- English text around obstacles: Liang hyphenation (en-US) and up to 0.08 em of letter spacing; pieces
  that are not justified fall below 15%.
- A dot chart: a dot's size and brightness are the area under the curve, a sweeping line, draggable
  control points.
- The zine chapter's toner halved and kept off the text (`Shed.clear`), with a one-line caption.
- A fanning card stack: angle0 × falloff^n, delay_ms × n, three looks that follow the material.
- Skeletons: `El::skeleton()` draws the real tree as a skeleton (bars are measured line widths), the two
  `Loading` numbers, zero layout shift, and a comparison of four ways to load.
- Nested menus: a safe-zone triangle, two delays, the keyboard, and a triangle that can be drawn
  (`El::on_move`, `El::guide`).

## The prologue, display not slogans, guidance, wrap, the readout, look recipes

- The film's prologue: one small interface, as it is on the left and in a material on the right, with
  a draggable divider and a row of material names.
- No slogans: a chapter's title is one element that recomposes glyph by glyph across cuts; chapters 9,
  10 and 11 redone.
- Guidance in the bottom bar: previous and next chapter, autoplay progress, hints shown only until the
  first input, bracket hints, invisible edges.
- Text around obstacles follows the true distance, leaves no slivers and does not hop while dragged;
  body text never condenses glyph by glyph; nothing is cut off at 330 px wide.
- The F2 readout: one line or every number, with a sparkline, and no frames at rest.
- `look:v1:` recipes (in a URL or a PNG text chunk); eight new mark sets grouped by family; five Geist
  Pixel faces; the `<amenui-view>` custom element.

## Scrolling, the sign and the mechanism, languages, components and docs

- Slow scrolling moves a picture by whole pixels; fast scrolling leaves each material's own trace
  (§1.8.3, `check` item 18).
- The film's third actor is a flat sign; chapter 13 is a real 3D mechanism (gears, a cam, a follower).
- Message catalogs, `t!`, plurals, a fallback chain and right-to-left rows (§1.20).
- Eighteen new components, `amenui add`, and a documentation site laid out after shadcn/ui.

## Fixes and the film

Bounded recomposition, particles emitted from mesh surfaces, local clocks, and the twelve-chapter film.

## 3D, GPU particles, dirty boxes, presets and transitions

## Ripple rules, recomposition, the model-facing surface

## Interaction and performance

Drawing only when something changed, the frame-time readout, text input, incremental layout and long
lists, touch, and the accessibility mirror.

## Annotation, every material, twelve presets, scenes

## Transforms: particles and flows

## Components and the gallery

## Marks, pixel type, themes and materials as data

## The skeleton

The element tree, taffy, parley, scrolling, clicks, focus and the number control.

## The vertical slice

Two-stage drawing, waves that follow outlines, and one codebase for native and web.
