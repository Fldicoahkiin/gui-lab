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

/// The A/B switch for measuring (`Renderer::spec_force`): on, the pass for the global material is used
/// from the second frame even over an override (and even where `specialise` is off); off, never.
#[test]
fn the_specialised_pass_can_be_forced_on_and_off_for_measuring() {
    let Ok(mut o) = Offscreen::try_new(320, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    assert_eq!(o.r.specialise, None, "offscreen does not specialise by itself");
    o.r.spec_force = Some(true);
    let mut u = open(Material::Dottr, true);
    let look = u.look();
    let _ = o.render(&mut u, &look);
    let _ = o.render(&mut u, &look);
    assert_eq!(o.r.specialised(), Some(Material::Dottr.id()), "forced on: the global material's own pass, over an override");
    assert!(u.perf().path.line().contains("spec yes dottr"), "{}", u.perf().path.line());
    o.r.spec_force = Some(false);
    let mut v = open(Material::Plain, false);
    let look = v.look();
    for _ in 0..3 { let _ = o.render(&mut v, &look); }
    assert_eq!(o.r.specialised(), None, "forced off");
    assert!(v.perf().path.line().contains("spec no (forced off)"), "{}", v.perf().path.line());
}

/// A page of several materials (the gallery's plain chrome round a cell in its own): a pass for each,
/// each drawing only its own pixels, draws what the pass for any draws, byte for byte, at rest, loose and
/// scrolled fast past its trace with the rows held to the lattice.
mod mixed {
    use super::*;
    pub fn view(_: &Material) -> El<()> {
        let rows = (0..40).fold(col().key("rows").gap(6.0).pad(12.0, 12.0), |c, i| {
            let t = text(format!("Row {i}: a line of text to read as it scrolls past"), 14.0, Paint::TEXT).key(format!("r{i}"));
            c.child(match i % 5 { 1 => t.material(Material::Plain.id()), 3 => t.material(Material::Charr.id()).loose(0.6), _ => t })
        });
        col().key("page").fill_width().fill_height().child(text("Chrome in plain", 16.0, Paint::INK).key("bar").material(Material::Plain.id()))
            .child(col().key("list").fill_width().grow(1.0).scroll().child(rows))
    }
}

#[test]
fn a_page_of_several_materials_is_drawn_by_a_pass_for_each_as_the_pass_for_any_draws_it() {
    let (Ok(mut each), Ok(mut any)) = (Offscreen::try_new(480, 360), Offscreen::try_new(480, 360)) else { eprintln!("no GPU adapter here: skipped"); return };
    each.r.specialise = Some(1);
    any.r.partial = false;
    for global in [Material::Dottr, Material::Halftonr] {
        let mk = || { let mut u = Ui::new(global, mixed::view, |_, _| {}, fonts(), 480, 360, 1.0).on_look(|m| Look { material: *m, ..Look::default() }); u.pointer_leave(); u.tick(5.0); u };
        let (mut u, mut v) = (mk(), mk());
        let look = u.look();
        let mut t = 5.0;
        let mut gated = 0;
        for f in 0..36 {
            t += 1.0 / 60.0;
            // Still while the passes are made, then fast down and up, then still again.
            let dy = if (8..16).contains(&f) { 40.0 } else if (16..24).contains(&f) { -40.0 } else { 0.0 };
            for ui in [&mut u, &mut v] { if dy != 0.0 { ui.wheel(240.0, 200.0, dy); } ui.tick(t); }
            let a = each.render(&mut u, &look);
            let b = any.render(&mut v, &look);
            let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
            assert_eq!(diff, 0, "{global:?}, frame {f}: {diff} bytes differ ({})", u.perf().path.line());
            if u.perf().path.gated != 0 { gated += 1; }
        }
        let want = (1u16 << global.id()) | (1 << Material::Plain.id()) | (1 << Material::Charr.id());
        assert_eq!(u.perf().path.gated, want, "{global:?}: a pass for each of its materials ({})", u.perf().path.line());
        assert!(gated >= 20, "{global:?}: only {gated} of 36 frames drawn by a pass for each");
        assert!(u.perf().path.line().contains("spec each "), "{}", u.perf().path.line());
    }
}
