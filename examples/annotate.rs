//! Recipe: the annotation layer on one subtree (docs/recipes/annotate.md). `on_gaze` turns the layer on;
//! `gaze_scope` limits it to the panel; `annotate` gives an element its own callout.
use amenui::el::{El, Paint, col, row, space, text};
use amenui::style::{Gaze, GazeMode, gaze};
use amenui::text::Face;
use amenui::ui::Ui;

pub struct Panel;

pub fn view(_: &Panel) -> El<()> {
    let panel = col().key("panel").gaze_scope().pad(20.0, 18.0).gap(12.0).border(1.0, Paint::FAINT).width(360.0)
        .child(text("电源", 18.0, Paint::INK).key("panel/title").annotate("标题"))
        .child(row().gap(12.0)
            .child(space(80.0, 48.0).fill(Paint::ACCENT).key("panel/switch").annotate("主开关"))
            .child(space(120.0, 48.0).fill(Paint::hue(1)).key("panel/meter").annotate("电量表")));
    col().key("page").pad(32.0, 28.0).gap(16.0)
        .child(text("面板外的字不标注。", 14.0, Paint::DIM))
        .child(panel)
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Panel, ()> {
    Ui::new(Panel, view, |_, _| {}, faces, w, h, scale)
        .on_gaze(|_| Gaze { mode: GazeMode::All, kinds: gaze::FRAME | gaze::INDEX | gaze::KIND | gaze::SIZE, only_marked: true, ..Gaze::default() })
}
