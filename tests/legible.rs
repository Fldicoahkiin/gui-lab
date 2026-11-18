//! Text to read holds the ink under it and a ring round its true outline (DESIGN.md §1.15): a label on a
//! loosened shape reads as at rest, and the shape beyond `r1` loosens as if the label were not there.
use amenui::el::{El, Paint, col, text};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, LIGHT, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 260;
const H: u32 = 120;
/// The eight materials that loosen.
const LOOSENING: [Material; 8] = [Material::Dottr, Material::Halftonr, Material::Charr, Material::Smudgr, Material::Carvr, Material::Glitchr, Material::Near, Material::Scribblr];

#[derive(Clone)]
struct S { m: Material, dark: bool, k: f32, size: f32, label: Option<&'static str>, legible: bool }
fn view(s: &S) -> El<()> {
    let mut shape = col().key("shape").width(220.0).height(84.0).radius(14.0).fill(Paint::INK).loose(s.k).center();
    if let Some(l) = s.label {
        let t = text(l, s.size, Paint::GROUND).key("label");
        shape = shape.child(if s.legible { t } else { t.legible(false) });
    }
    col().key("page").fill_width().fill_height().pad(20.0, 18.0).child(shape)
}
fn look(s: &S) -> Look { Look { material: s.m, theme: if s.dark { DARK } else { LIGHT }, ..Look::default() } }
fn shot(o: &mut Offscreen, s: S) -> Vec<u8> {
    let mut ui = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
    ui.tick(3.0);
    let l = ui.look();
    o.r.partial = false;
    o.render(&mut ui, &l)
}
fn same(a: &[u8], b: &[u8], i: usize) -> bool { (0..3).all(|c| (a[i * 4 + c] as i32 - b[i * 4 + c] as i32).abs() <= 2) }
/// The label's ink by its true outline: the pixels it covers more than half (it changes the frame by
/// more than half of the most it changes any pixel).
fn label_ink(rest: &[u8], bare: &[u8]) -> Vec<bool> {
    let d: Vec<i32> = (0..(W * H) as usize).map(|i| (0..3).map(|c| (rest[i * 4 + c] as i32 - bare[i * 4 + c] as i32).abs()).sum()).collect();
    let top = d.iter().copied().max().unwrap_or(0);
    d.iter().map(|&v| v * 2 > top).collect()
}
/// Each pixel's distance (px, 8-connected chamfer) to the nearest pixel of `mask`.
fn distance(mask: &[bool]) -> Vec<f32> {
    let (w, h) = (W as usize, H as usize);
    let mut d: Vec<f32> = mask.iter().map(|&m| if m { 0.0 } else { 1e9 }).collect();
    let (a, b) = (1.0f32, std::f32::consts::SQRT_2);
    for y in 0..h { for x in 0..w {
        let i = y * w + x;
        if x > 0 { d[i] = d[i].min(d[i - 1] + a); }
        if y > 0 {
            d[i] = d[i].min(d[i - w] + a);
            if x > 0 { d[i] = d[i].min(d[i - w - 1] + b); }
            if x + 1 < w { d[i] = d[i].min(d[i - w + 1] + b); }
        }
    } }
    for y in (0..h).rev() { for x in (0..w).rev() {
        let i = y * w + x;
        if x + 1 < w { d[i] = d[i].min(d[i + 1] + a); }
        if y + 1 < h {
            d[i] = d[i].min(d[i + w] + a);
            if x + 1 < w { d[i] = d[i].min(d[i + w + 1] + b); }
            if x > 0 { d[i] = d[i].min(d[i + w - 1] + b); }
        }
    } }
    d
}
fn radii(size: f32) -> (f32, f32) { let r0 = (0.25 * size).max(3.0); (r0, r0 + (0.5 * size).max(6.0)) }

#[test]
fn a_label_holds_its_ink_and_a_ring_round_it_on_a_loosened_shape() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut worst = (1.0f32, 1.0f32, 1.0f32, String::new());
    for m in LOOSENING { for dark in [false, true] { for size in [11.0f32, 14.0, 20.0] {
        let base = S { m, dark, k: 0.0, size, label: Some("Tj dolly"), legible: true };
        let rest = shot(&mut o, base.clone());
        let bare_rest = shot(&mut o, S { label: None, ..base.clone() });
        let n = (W * H) as usize;
        let ink = label_ink(&rest, &bare_rest);
        let d = distance(&ink);
        let (r0, r1) = radii(size);
        let r1_drawn = r1.min(amenui::text::SPREAD as f32);
        for k in [0.3f32, 0.6, 1.0] {
            let f = shot(&mut o, S { k, ..base.clone() });
            let bare = shot(&mut o, S { k, label: None, ..base.clone() });
            let share = |pick: &dyn Fn(usize) -> bool, a: &[u8], b: &[u8]| { let (mut t, mut s) = (0, 0); for i in 0..n { if pick(i) { t += 1; if same(a, b, i) { s += 1; } } } s as f32 / t.max(1) as f32 };
            let own = share(&|i| ink[i], &f, &rest);
            let ring = share(&|i| !ink[i] && d[i] <= r0, &f, &rest);
            let beyond = share(&|i| d[i] > r1_drawn + 2.0, &f, &bare);
            let tag = format!("{m:?} {} {size} px at {k}", if dark { "dark" } else { "light" });
            if own < worst.0 { worst.0 = own; worst.3 = tag.clone(); }
            worst.1 = worst.1.min(ring); worst.2 = worst.2.min(beyond);
            assert!(own >= 0.99, "{tag}: {:.3} of the label's ink is as at rest", own);
            assert!(ring >= 0.97, "{tag}: {:.3} within r0 is as at rest", ring);
            assert!(beyond >= 0.97, "{tag}: {:.3} beyond r1 + 2 is as without the label", beyond);
            // What the label holds is within r1 + 1 of its ink.
            let held_far = (0..n).filter(|&i| !ink[i] && d[i] > r1_drawn + 1.0 && !same(&f, &bare, i) && same(&f, &rest, i) && !same(&bare, &rest, i)).count();
            assert!(held_far as f32 <= 0.01 * n as f32, "{tag}: {held_far} px held beyond r1 + 1");
        }
    } } }
    eprintln!("least: own ink {:.3} ({}), within r0 {:.3}, beyond r1 {:.3}", worst.0, worst.3, worst.1, worst.2);
}

#[test]
fn at_rest_and_as_decoration_nothing_changes() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in LOOSENING {
        // Marked decoration, the label comes apart with the shape: the hold is not drawn, so the frame
        // is the one where nothing holds (the same label laid over the shape without a hold).
        let deco = S { m, dark: false, k: 0.6, size: 14.0, label: Some("Live"), legible: false };
        let f = shot(&mut o, deco.clone());
        let mut ui = Ui::new(deco.clone(), view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
        ui.tick(3.0);
        assert!(ui.scene().holds.is_empty(), "{m:?}: a label marked decoration holds nothing");
        let _ = f;
        let held = S { legible: true, ..deco };
        let mut ui = Ui::new(held, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
        ui.tick(3.0);
        assert!(!ui.scene().holds.is_empty(), "{m:?}: a label to read on a shape holds");
    }
}

#[test]
fn the_hold_is_an_island_round_the_glyphs_not_a_box() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let base = S { m: Material::Dottr, dark: false, k: 1.0, size: 14.0, label: Some("Tj dolly"), legible: true };
    let rest = shot(&mut o, S { k: 0.0, ..base.clone() });
    let bare_rest = shot(&mut o, S { k: 0.0, label: None, ..base.clone() });
    let n = (W * H) as usize;
    let ink = label_ink(&rest, &bare_rest);
    let d = distance(&ink);
    let (_, r1) = radii(14.0);
    let (mut x0, mut y0, mut x1, mut y1) = (W as f32, H as f32, 0.0f32, 0.0f32);
    for (i, _) in ink.iter().enumerate().take(n).filter(|(_, k)| **k) { let (x, y) = ((i as u32 % W) as f32, (i as u32 / W) as f32); x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1.0); y1 = y1.max(y + 1.0); }
    let rect = (x1 - x0 + 2.0 * r1) * (y1 - y0 + 2.0 * r1);
    let island = (0..n).filter(|&i| d[i] <= r1).count() as f32;
    eprintln!("hold area {island} of the grown ink box {rect}: {:.2}", island / rect);
    assert!(island <= 0.92 * rect, "the hold is {island} px of the box's {rect}");
}

/// The label above with a front passing: a page that sends one from the shape's left edge.
#[derive(Clone)]
struct F { m: Material, front: bool }
fn fview(s: &F) -> El<()> {
    let t = text("Tj dolly", 14.0, Paint::GROUND).key("label").respond(amenui::el::Response::Ignore);
    let _ = s.front;
    col().key("page").fill_width().fill_height().pad(20.0, 18.0)
        .child(col().key("shape").width(220.0).height(84.0).radius(14.0).fill(Paint::INK).loose(0.6).center().child(t))
}
fn flook(s: &F) -> Look { Look { material: s.m, theme: LIGHT, ..Look::default() } }

#[test]
fn marks_pushed_by_a_front_do_not_come_into_the_ring() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Dottr, Material::Halftonr, Material::Charr, Material::Glitchr] {
        let shot_at = |o: &mut Offscreen, front: bool, t: f32| {
            let mut ui = Ui::new(F { m, front }, fview, |_, _| {}, fonts(), W, H, 1.0).on_look(flook);
            ui.tick(3.0);
            if front { ui.command(amenui::morph::Cmd::Ripplr { x: 20.0, y: 60.0, strength: 1.0 }); }
            ui.tick(3.0 + t);
            let l = ui.look();
            o.r.partial = false;
            o.render(&mut ui, &l)
        };
        // At rest the shape is not loose: its frame is the one with the hold's cap everywhere under the label.
        let still = shot_at(&mut o, false, 0.0);
        let rest = { let mut ui = Ui::new(S { m, dark: false, k: 0.0, size: 14.0, label: Some("Tj dolly"), legible: true }, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look); ui.tick(3.0); let l = ui.look(); o.r.partial = false; o.render(&mut ui, &l) };
        let bare = { let mut ui = Ui::new(S { m, dark: false, k: 0.0, size: 14.0, label: None, legible: true }, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look); ui.tick(3.0); let l = ui.look(); o.r.partial = false; o.render(&mut ui, &l) };
        let ink = label_ink(&rest, &bare);
        let d = distance(&ink);
        let (r0, _) = radii(14.0);
        let n = (W * H) as usize;
        for t in [0.05f32, 0.12, 0.2, 0.3] {
            let f = shot_at(&mut o, true, t);
            let (mut tot, mut ok) = (0, 0);
            for i in 0..n { if !ink[i] && d[i] <= r0 { tot += 1; if same(&f, &still, i) { ok += 1; } } }
            let share = ok as f32 / tot.max(1) as f32;
            assert!(share >= 0.97, "{m:?} {t} s into the front: {share:.3} within r0 as with no front");
        }
    }
}

#[test]
fn the_audit_finds_no_label_it_cannot_read() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in LOOSENING { for dark in [false, true] { for k in [0.3f32, 0.6, 1.0] {
        let mut ui = Ui::new(S { m, dark, k, size: 14.0, label: Some("Tj dolly"), legible: true }, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
        ui.tick(3.0);
        let l = ui.look();
        o.r.partial = false;
        let px = o.render(&mut ui, &l);
        o.r.hide_glyphs = true;
        ui.invalidate();
        let ground = o.render(&mut ui, &l);
        o.r.hide_glyphs = false;
        let f = amenui::audit::audit(&mut ui, &px, &ground);
        let bad: Vec<_> = f.iter().filter(|f| f.key == "label" && f.kind == amenui::audit::Kind::Contrast && f.level <= amenui::audit::Level::P1).collect();
        assert!(bad.is_empty(), "{m:?} {} {k}: {bad:?}", if dark { "dark" } else { "light" });
    } } }
}
