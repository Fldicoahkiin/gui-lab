//! Step 8's contact sheets (DESIGN.md §7):
//!   mesh         the same low-poly object under every material, at rest and loosened, and drawn as a
//!                picture under four mark sets → `mesh-materials.png`
//! usage: step8 <out_dir> [mesh]
use lab::el::{El, Paint, col, scene3d, text};
use lab::mesh::{Mat4, MeshId, Scene3d};
use lab::offscreen::{Offscreen, crop, save_png, sheet};
use lab::style::{DARK, Look, MATERIALS, Material, PicFx, set};
use lab::ui::Ui;

const S: u32 = 300;

/// One object in a box, held at a looseness, through the material or as a picture.
pub struct One { pub loose: f32, pub picture: bool, pub yaw: f32 }
pub fn object(yaw: f32) -> Scene3d {
    let mut sc = Scene3d::single(MeshId::ICO[1]);
    sc.items[0].transforms = vec![Mat4::rotate_y(yaw).mul(&Mat4::uniform(1.55))];
    sc
}
fn one(s: &One) -> El<()> {
    let mut sc = object(s.yaw);
    if s.picture { sc.picture = Some(0); }
    col().key("root").fill_width().fill_height().pad(10.0, 10.0).gap(4.0)
        .child(scene3d(sc).key("mesh").width(280.0).height(256.0).loose(s.loose))
        .child(text(if s.picture { "picture" } else if s.loose > 0.0 { "loose" } else { "rest" }, 12.0, Paint::DIM))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "step8".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let what = a.get(2).map(String::as_str).unwrap_or("mesh");
    if what == "mesh" { mesh(&out); }
}

fn mesh(out: &str) {
    let mut o = Offscreen::new(S, S);
    let faces = || lab::text::load_bundled("assets").expect("fonts");
    let mut frames = vec![];
    let mut shot = |state: One, look: Look, label: &str| {
        let mut ui = Ui::new(state, one, |_, _| {}, faces(), S, S, 1.0);
        ui.tick(2.0);
        let px = o.render(&mut ui, &look);
        let _ = label;
        frames.push(crop(&px, S, S, [0, 0, S, S]));
    };
    for (m, name) in MATERIALS {
        for loose in [0.0, 0.7] {
            let look = Look { theme: DARK, material: m, ..Look::default() };
            shot(One { loose, picture: false, yaw: 0.0 }, look, name);
        }
    }
    for s in [set::DOTS, set::CHARS, set::STROKE, set::PIXEL] {
        let mut look = Look { theme: DARK, material: Material::Plain, ..Look::default() };
        look.pics[0] = PicFx { pitch: if s == set::CHARS { 9.0 } else { 6.0 }, levels: 10.0, ground: 0.1, follow: if s == set::STROKE { 1.0 } else { 0.0 }, ..PicFx::with_set(s) };
        shot(One { loose: 1.0, picture: true, yaw: 0.0 }, look, "marks");
    }
    let (w, h, px) = sheet(&frames, 8);
    save_png(&format!("{out}/mesh-materials.png"), w, h, &px);
    eprintln!("wrote {out}/mesh-materials.png ({} frames: every material at rest and loose 0.7, then four mark sets)", frames.len());
}
