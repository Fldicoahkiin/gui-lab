//! Recipe: another material on one element (docs/recipes/material.md). The page draws with the look's
//! material; the card says `.material(..)` and draws with halftone. Its button sends fronts over the
//! card only, and the card is decoration (`Loosen`, not legible), so the front shows the material.
use amenui::el::{El, Paint, Response, Scope, col, text};
use amenui::style::Material;
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::{Variant, button_as, card};

pub struct Page { pub pressed: u32 }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Ripple }

pub fn view(s: &Page) -> El<Msg> {
    let body = text("这张卡片用半色调画；页面其余部分用全局材质。", 15.0, Paint::TEXT);
    col().key("page").pad(32.0, 28.0).gap(16.0)
        .child(text("页面上的字：全局材质", 15.0, Paint::DIM))
        .child(card("card", Some("半色调"), body).width(360.0).material(Material::Halftone.id()).respond(Response::Loosen).legible(false))
        .child(button_as("ripple", format!("涟漪 · {}", s.pressed).as_str(), Variant::Default, Msg::Ripple).ripple(Scope::Container(Some("card".into()))))
}

pub fn update(s: &mut Page, m: Msg) {
    match m { Msg::Ripple => s.pressed += 1 }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Page, Msg> {
    Ui::new(Page { pressed: 0 }, view, update, faces, w, h, scale)
}
