//! Recipe: a list with selection (docs/recipes/list.md). The list is one focus stop; Up / Down move the
//! selection, a press picks a row, and the state says which row is selected.
use lab::el::{El, Paint, col, text};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::list;

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
