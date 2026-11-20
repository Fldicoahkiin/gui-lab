//! How selected text looks (`Look::select`, DESIGN.md §1.8.1): an opaque halo round each selected
//! glyph, the accent at 0.34 over the ground as one colour (#5f2d18 on the dark theme), the glyphs in
//! the ink; no box anywhere and no darker band where halos meet; there at once and gone at once.
use amenui::el::{El, Font, Paint, col, text};
use amenui::offscreen::Offscreen;
use amenui::scene::SHAPE;
use amenui::style::{DARK, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 420;
const H: u32 = 200;
fn view(_: &()) -> El<()> {
    col().key("page").fill_width().fill_height().pad(20.0, 20.0)
        .child(text("Typography by nearness, quietly glyph by glyph", 22.0, Paint::DIM).font(Font::Body).width(300.0).selectable().key("one"))
}
fn open() -> Ui<(), ()> {
    let mut u = Ui::new((), view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    u.tick(1.0);
    u.settle();
    u
}
fn render(o: &mut Offscreen, u: &mut Ui<(), ()>) -> Vec<u8> { let l = u.look(); o.r.partial = false; o.render(u, &l) }
fn px(f: &[u8], x: u32, y: u32) -> [f32; 3] { let k = ((y * W + x) * 4) as usize; [f[k] as f32, f[k + 1] as f32, f[k + 2] as f32] }
fn lum(c: [f32; 3]) -> f32 {
    let l = |v: f32| { let v = v / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
    0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
}
fn near(a: [f32; 3], b: [f32; 3], d: f32) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() <= d) }

#[test]
fn a_selection_across_two_lines_is_an_opaque_halo_in_one_colour_with_the_glyphs_in_ink() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut u = open();
    let r = u.node("one").expect("the text").rect;
    assert!(r.h > 50.0, "two lines: {r:?}");
    let before = render(&mut o, &mut u);
    // Across both lines: from the first line's start to the second's end.
    u.press(r.x + 1.0, r.y + 6.0);
    u.pointer_move(r.right() - 1.0, r.bottom() - 4.0);
    u.release(r.right() - 1.0, r.bottom() - 4.0);
    assert!(!u.selection().is_empty(), "a selection");
    let t = u.now();
    u.tick(t + 1.0 / 60.0);
    let f = render(&mut o, &mut u);
    // No box: the selection adds glyph instances and nothing else.
    assert_eq!(u.scene().instances.iter().filter(|i| i.paint[3] == SHAPE && i.paint[2] > 0.0).count(), 0, "a shape drawn for the selection");
    let ground: [f32; 3] = DARK.ground.map(|v| v * 255.0);
    let halo: [f32; 3] = std::array::from_fn(|k| (DARK.ground[k] * 0.66 + DARK.accent[k] * 0.34) * 255.0);
    assert!(near(halo, [0x64 as f32, 0x3b as f32, 0x28 as f32], 1.0), "the halo colour {halo:?}");
    // Two halos over each other at 0.34 would be this: it is nowhere.
    let band: [f32; 3] = std::array::from_fn(|k| halo[k] * 0.66 + DARK.accent[k] * 255.0 * 0.34);
    let (mut flat, mut banded, mut brightest) = (0, 0, [0.0f32; 3]);
    let mut gaps = 0;
    for y in r.y as u32..r.bottom() as u32 { for x in r.x as u32..r.right() as u32 {
        let c = px(&f, x, y);
        if near(c, halo, 2.0) { flat += 1; }
        if near(c, band, 3.0) { banded += 1; }
        if lum(c) > lum(brightest) { brightest = c; }
        // Where nothing was drawn before and the halo is not, the ground shows through: no rectangle.
        if near(px(&before, x, y), ground, 1.0) && near(c, ground, 1.0) { gaps += 1; }
    } }
    assert!(flat > 400, "the halo within 2/255 of #5f2d18: {flat} px");
    assert_eq!(banded, 0, "a darker band where halos meet: {banded} px");
    assert!(gaps > 200, "the halo fills its box: only {gaps} px of ground left inside it");
    // Dim text selected is drawn in the ink: at least 7 : 1 on the halo.
    let ratio = (lum(brightest) + 0.05) / (lum(halo) + 0.05);
    assert!(ratio >= 7.0, "selected dim text at {ratio:.2} : 1 on the halo ({brightest:?})");
    // Cleared at once: a press elsewhere, and the next frame has no halo.
    u.press(5.0, H as f32 - 5.0);
    u.release(5.0, H as f32 - 5.0);
    u.tick(t + 2.0 / 60.0);
    let g = render(&mut o, &mut u);
    let left = (0..W * H).filter(|&k| near(px(&g, k % W, k / W), halo, 2.0)).count();
    assert_eq!(left, 0, "the halo outlives the selection: {left} px");
}

#[test]
fn the_halo_is_there_in_the_first_frame_without_condensing() {
    let mut u = open();
    let r = u.node("one").expect("the text").rect;
    u.press(r.x + 1.0, r.y + 6.0);
    u.pointer_move(r.right() - 1.0, r.bottom() - 4.0);
    let t = u.now();
    u.tick(t + 1.0 / 60.0);
    let halos: Vec<_> = u.scene().instances.iter().filter(|i| i.paint[3] == 0.0 && (i.paint[1] - 0.34).abs() < 1e-4 && i.paint[2] == 1.0).copied().collect();
    assert!(!halos.is_empty(), "halo instances");
    assert!(halos.iter().all(|h| u.now() - h.extra[3] > 10.0), "a halo condenses in");
}
