//! Recipe: a 3D object in a card (docs/recipes/mesh.md). The object is an element like any other: it
//! takes the card's width, the global material draws it, fronts loosen it, and the camera is state that
//! the orbit widget reports drags, the wheel and arrow keys to.
use amenui::el::{El, Paint, col, row, text};
use amenui::mesh::{Mat4, MeshId, Scene3d};
use amenui::text::Face;
use amenui::ui::Ui;
use amenui::widgets::{Orbit, OrbitMsg, Variant, button_as, card, orbit};

pub struct Shop { pub orbit: Orbit, pub turns: u32, pub clock: f32 }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Orbit(OrbitMsg), Turn }

pub fn view(s: &Shop) -> El<Msg> {
    let mut torus = Scene3d::single(MeshId::TORUS);
    torus.items[0].transforms = vec![Mat4::rotate_x(25.0).mul(&Mat4::uniform(2.2))];
    let body = col().gap(12.0)
        .child(orbit("ring", torus, &s.orbit, Msg::Orbit).height(220.0))
        .child(row().gap(12.0)
            .child(button_as("turn", "转 45°", Variant::Default, Msg::Turn))
            .child(text(format!("转过 {} 次", s.turns), 14.0, Paint::DIM).key("turns")));
    col().key("page").pad(32.0, 28.0).child(card("ring-card", Some("一枚戒指"), body).width(360.0))
}

pub fn update(s: &mut Shop, m: Msg) {
    match m {
        Msg::Orbit(o) => s.orbit.update(o, s.clock),
        Msg::Turn => { s.orbit.yaw += 45.0; s.turns += 1; }
    }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<Shop, Msg> {
    Ui::new(Shop { orbit: Orbit::default(), turns: 0, clock: 0.0 }, view, update, faces, w, h, scale)
        // The orbit coasts after a fast drag: step it on every tick, and ask for frames while it moves.
        .on_tick(|s, t| { s.clock = t; s.orbit.step(t) })
        .on_wake(|s, t| s.orbit.moving().then_some(t))
}
