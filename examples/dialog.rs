//! Recipe: a dialog (docs/recipes/dialog.md). Declared next to the button that opens it; the runtime lays
//! it over the window, keeps focus inside, and Esc or 关闭 send the close message.
use lab::el::{El, Paint, col, text};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::{TextEdit, Variant, button_as, dialog, input};

pub struct Rename { pub open: bool, pub name: TextEdit, pub saved: String }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Open, Close, Name(TextEdit), Save }

pub fn view(s: &Rename) -> El<Msg> {
    let body = col().gap(14.0)
        .child(input("rename/name", &s.name, "名称", 296.0, 40, Msg::Name))
        .child(button_as("rename/save", "保存", Variant::Accent, Msg::Save));
    col().key("page").pad(32.0, 28.0).gap(14.0)
        .child(text(format!("当前名称：{}", s.saved), 16.0, Paint::TEXT).key("page/name"))
        .child(button_as("page/open", "重命名…", Variant::Default, Msg::Open))
        .child(dialog("rename", s.open, "重命名", 340.0, 0.35, Msg::Close, body))
}

pub fn update(s: &mut Rename, m: Msg) {
    match m {
        Msg::Open => s.open = true,
        Msg::Close => s.open = false,
        Msg::Name(t) => s.name = t,
        Msg::Save => { s.saved = s.name.text.clone(); s.open = false; }
    }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Rename, Msg> {
    Ui::new(Rename { open: true, name: TextEdit::new("工作室 4"), saved: "工作室 4".into() }, view, update, faces, w, h, scale)
}
