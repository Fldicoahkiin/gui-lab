# A dot chart

`widgets::dot_chart` draws a `DotChart` (a few control points, a Catmull-Rom curve through them) as a field of dots: in column c, dot r covers the band [r, r + 1) of the rows, and how much of that band lies under the curve sets its diameter (`pitch × (0.25 + 0.65 k)`) and tone (`0.25 + 0.75 k`), so the field is the area under the curve. The range is drawn in the accent colour; the pointer scrubs a marker with the value (`El::on_move`); the control points on the axis drag the curve up and down live.

The code is `examples/chart.rs`, compiled into the library as `lab::recipes::chart`; `tests/recipes.rs` renders it, plays its interaction, and fails if this block differs from the file.

```rust
//! Recipe: a dot chart (docs/recipes/chart.md). The series is a few control points in the state; the
//! chart draws the area under the curve as dots, scrubs a marker under the pointer and drags the points.
use lab::el::{El, Font, Paint, col, text};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::{ChartMsg, DotChart, dot_chart};

pub struct App { pub chart: DotChart }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Chart(ChartMsg) }

pub fn view(s: &App) -> El<Msg> {
    let c = &s.chart;
    col().key("page").pad(24.0, 24.0).gap(12.0)
        .child(text("负载", 20.0, Paint::INK).font(Font::Display).key("title"))
        .child(dot_chart("chart", c, Msg::Chart))
        .child(text(format!("范围 {:.2}–{:.2} 的面积 {:.3}", c.range.0, c.range.1, c.range_area()), 12.0, Paint::DIM).font(Font::Mono).key("area"))
}

pub fn update(s: &mut App, m: Msg) {
    let Msg::Chart(m) = m;
    s.chart.update(m);
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<App, Msg> {
    Ui::new(App { chart: DotChart::default() }, view, update, faces, w, h, scale)
}
```

See it:

```sh
cargo run --release --bin render -- --page recipe-chart --at 300 --out chart.png --tree chart.json
```
