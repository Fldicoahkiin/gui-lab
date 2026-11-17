//! Recipe: a dot chart (docs/recipes/chart.md). The series is a few control points in the state; the
//! chart draws the area under the curve as dots, scrubs a marker under the pointer and drags the points.
use amenui::el::{El, Font, Paint, col, text};
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::{ChartMsg, DotChart, dot_chart};

pub struct App { pub chart: DotChart }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Chart(ChartMsg) }

pub fn view(s: &App) -> El<Msg> {
    let c = &s.chart;
    col().key("page").pad(24.0, 24.0).gap(12.0)
        .child(text("Load", 20.0, Paint::INK).font(Font::Display).key("title"))
        .child(dot_chart("chart", c, Msg::Chart))
        .child(text(format!("Area over {:.2}–{:.2}: {:.3}", c.range.0, c.range.1, c.range_area()), 12.0, Paint::DIM).font(Font::Mono).key("area"))
}

pub fn update(s: &mut App, m: Msg) {
    let Msg::Chart(m) = m;
    s.chart.update(m);
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<App, Msg> {
    Ui::new(App { chart: DotChart::default() }, view, update, faces, w, h, scale)
}
