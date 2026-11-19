//! The material pass made for one material (`Renderer::specialise`): once the global material has held
//! with nothing overriding it, the material pass is a pipeline of its own for it, and draws the frame the
//! pass for any material draws, byte for byte; an override on the page goes back to the pass for any.
use amenui::el::{El, Paint, col, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

struct S { mat: Material, over: bool }
fn view(s: &S) -> El<()> {
    let mut c = col().key("page").fill_width().fill_height().pad(16.0, 16.0).gap(8.0).fill(Paint::GROUND)
        .child(text("A heading set large", 28.0, Paint::INK).key("h").loose(0.6))
        .child(text("A line of text to read, at rest.", 14.0, Paint::TEXT).key("p"));
    if s.over { c = c.child(text("Set in its own material", 14.0, Paint::TEXT).key("o").material(Material::Halftonr.id()).loose(0.6)); }
    c
}
fn open(mat: Material, over: bool) -> Ui<S, ()> {
    let mut u = Ui::new(S { mat, over }, view, |_, _| {}, fonts(), 320, 200, 1.0).on_look(|s| Look { material: s.mat, ..Look::default() });
    u.pointer_leave();
    u.tick(5.0);
    u
}

#[test]
fn a_material_pass_made_for_the_material_draws_what_the_pass_for_any_draws() {
    let (Ok(mut spec), Ok(mut any)) = (Offscreen::try_new(320, 200), Offscreen::try_new(320, 200)) else { eprintln!("no GPU adapter here: skipped"); return };
    spec.r.specialise = Some(1);
    for mat in [Material::Dottr, Material::Plain, Material::Charr] {
        let (mut u, mut v) = (open(mat, false), open(mat, false));
        let look = u.look();
        let mut t = 5.0;
        for f in 0..3 {
            t += 1.0 / 60.0;
            u.tick(t);
            v.tick(t);
            let a = spec.render(&mut u, &look);
            let b = any.render(&mut v, &look);
            assert_eq!(a.iter().zip(&b).filter(|(p, q)| p != q).count(), 0, "{mat:?}, frame {f}: the frames differ");
            if f > 0 { assert_eq!(spec.r.specialised(), Some(mat.id()), "{mat:?}, frame {f}: drawn with its own pass"); }
            assert_eq!(any.r.specialised(), None);
        }
    }
    // An override on the page: the pass for any material.
    let mut u = open(Material::Dottr, true);
    let look = u.look();
    for _ in 0..3 { let _ = spec.render(&mut u, &look); }
    assert_eq!(spec.r.specialised(), None, "an override draws with the pass for any material");
}
