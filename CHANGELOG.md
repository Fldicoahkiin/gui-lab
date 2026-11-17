# Changelog

Newest first. The acceptance checks for each part are in [DESIGN.md §7](docs/DESIGN.md).

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
