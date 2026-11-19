//! A cached subtree (DESIGN.md §1.36): drawn once and laid on from its plate while it stays as it is,
//! drawn again when it changes; `el::cached_view` reads its coverage nearest; `El::clip_round`.
use amenui::el::{El, Paint, cached_view, col, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 260;
const H: u32 = 200;
/// The glass: 80 px across at the given place, showing 20 × 20 px of the page at ×4.
const GLASS: [f32; 2] = [170.0, 100.0];
const FROM: [f32; 4] = [10.0, 30.0, 20.0, 20.0];

#[derive(Clone)]
struct S { cached: bool, n: u32, words: &'static str, glass: bool }
fn view(s: &S) -> El<()> {
    let mut small = col().key("small").gap(1.0).pad(10.0, 4.0);
    for k in 0..40 { small = small.child(text(format!("{} {k} {}", s.words, "fn main() { let x = 1; }"), 6.0, Paint::TEXT)); }
    let small = if s.cached { small.cached() } else { small };
    let mut page = col().key("page").fill_width().fill_height()
        .child(text(format!("Counter {}", s.n), 11.0, Paint::TEXT).key("count"))
        .child(small);
    if s.glass {
        page = page.child(col().key("glass").width(80.0).height(80.0).clip_round(40.0).absolute(Some(GLASS[0]), Some(GLASS[1]), None, None)
            .child(cached_view("small", FROM, Paint::TEXT).width(80.0).height(80.0)));
    }
    page
}
fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
fn mk(s: S) -> Ui<S, ()> { let mut ui = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look); ui.tick(2.0); ui }
fn frame(o: &mut Offscreen, ui: &mut Ui<S, ()>) -> Vec<u8> { ui.tick(2.0); let l = ui.look(); o.render(ui, &l) }
fn set(ui: &mut Ui<S, ()>, f: impl Fn(&mut S)) { f(&mut ui.state); ui.relayout(); }
/// The share of pixels that differ by more than 2 in a channel.
fn diff(a: &[u8], b: &[u8]) -> f32 { a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 2)).count() as f32 / (a.len() / 4) as f32 }

#[test]
fn a_cached_page_is_drawn_once_and_looks_as_drawn_every_frame() {
    if Offscreen::try_new(W, H).is_err() { eprintln!("no GPU adapter here: skipped"); return; }
    let s = S { cached: true, n: 0, words: "alpha", glass: false };
    let (mut oc, mut op) = (Offscreen::new(W, H), Offscreen::new(W, H));
    let (mut c, mut p) = (mk(s.clone()), mk(S { cached: false, ..s }));
    let (a, b) = (frame(&mut oc, &mut c), frame(&mut op, &mut p));
    assert!(diff(&a, &b) < 1e-3, "the first frame draws it: {}", diff(&a, &b));
    let span = |ui: &Ui<S, ()>| ui.scene().plates.iter().map(|p| p.end - p.start).sum::<u32>();
    let drawn = span(&c);
    // Something else changes: the cached page is laid on, not drawn.
    set(&mut c, |s| s.n += 1);
    set(&mut p, |s| s.n += 1);
    let (a, b) = (frame(&mut oc, &mut c), frame(&mut op, &mut p));
    let kept = span(&c);
    assert!(drawn > 100 && kept == 0, "kept: {kept} instances, drawn: {drawn}");
    assert!(diff(&a, &b) < 1e-3, "as drawn: {}", diff(&a, &b));
    // Again, and its own words change: drawn again, with nothing of the old ink left.
    let (a2, _) = (frame(&mut oc, &mut c), ());
    assert!(diff(&a, &a2) == 0.0, "a frame with nothing new is the same");
    set(&mut c, |s| s.words = "omega");
    set(&mut p, |s| s.words = "omega");
    let (a, b) = (frame(&mut oc, &mut c), frame(&mut op, &mut p));
    assert!(span(&c) > 100, "drawn again when it changed");
    assert!(diff(&a, &b) < 1e-3, "no old ink: {}", diff(&a, &b));
    // A new window size empties the plate: drawn again.
    let (mut oc2, mut op2) = (Offscreen::new(W + 20, H), Offscreen::new(W + 20, H));
    c.resize(W + 20, H, 1.0);
    p.resize(W + 20, H, 1.0);
    let (a, b) = (frame(&mut oc2, &mut c), frame(&mut op2, &mut p));
    assert!(diff(&a, &b) < 1e-3, "after a new size: {}", diff(&a, &b));
}

#[test]
fn a_cached_view_reads_coverage_nearest_inside_a_round_clip() {
    if Offscreen::try_new(W, H).is_err() { eprintln!("no GPU adapter here: skipped"); return; }
    let s = S { cached: true, n: 0, words: "alpha", glass: true };
    let mut o = Offscreen::new(W, H);
    let mut ui = mk(s.clone());
    // The first frame draws the page; the glass reads it from the next.
    frame(&mut o, &mut ui);
    set(&mut ui, |s| s.n += 1);
    let px = frame(&mut o, &mut ui);
    let mut bare = mk(S { glass: false, ..s });
    let ground = frame(&mut Offscreen::new(W, H), &mut bare);
    let at = |v: &[u8], x: u32, y: u32| { let i = ((y * W + x) * 4) as usize; [v[i], v[i + 1], v[i + 2]] };
    let (gx, gy) = (GLASS[0] as u32, GLASS[1] as u32);
    // Inside the circle each source pixel is a 4 × 4 square of one colour, and there is ink.
    let (mut blocks, mut inked) = (0, 0);
    for by in 4..16 { for bx in 4..16 {
        let (x0, y0) = (gx + bx * 4, gy + by * 4);
        let (cx, cy) = (x0 as f32 + 2.0 - gx as f32 - 40.0, y0 as f32 + 2.0 - gy as f32 - 40.0);
        if cx.hypot(cy) > 30.0 { continue; }
        let c = at(&px, x0, y0);
        for dy in 0..4 { for dx in 0..4 { assert_eq!(at(&px, x0 + dx, y0 + dy), c, "block ({bx}, {by}) is one colour"); } }
        blocks += 1;
        if c != at(&ground, x0, y0) { inked += 1; }
    } }
    assert!(blocks > 80 && inked > 10, "{blocks} blocks, {inked} inked");
    // Outside the circle, in the box's corners, the page shows as without the glass.
    for (x, y) in [(gx + 2, gy + 2), (gx + 77, gy + 2), (gx + 2, gy + 77), (gx + 77, gy + 77)] { assert_eq!(at(&px, x, y), at(&ground, x, y), "corner ({x}, {y})"); }
}

/// A page set in a pixel face under its native size (Departure Mono, 11 px, at 2.75): outside a cached
/// subtree it is drawn at its native size, inside one at the size asked, by its outline's coverage.
fn tiny(cached: &bool) -> El<()> {
    let mut page = col().key("tiny").gap(0.0).pad(4.0, 4.0);
    for k in 0..12 { page = page.child(text(format!("{k:02} fn main() {{ let x = {k}; }}"), 2.75, Paint::TEXT).font(amenui::el::Font::Face(amenui::text::MONO)).key(format!("tiny/{k}")).nowrap()); }
    let page = if *cached { page.cached() } else { page };
    col().key("page").fill_width().fill_height().child(page)
}

#[test]
fn a_pixel_face_under_its_native_size_is_drawn_at_the_size_asked_in_a_cached_page() {
    let mk = |cached: bool| { let mut ui = Ui::new(cached, tiny, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() }); ui.tick(2.0); ui };
    let (mut c, mut n) = (mk(true), mk(false));
    let (hc, hn) = (c.node("tiny").expect("cached page").rect.h, n.node("tiny").expect("page").rect.h);
    // 12 lines: at 2.75 px about 40 px with the padding; at 11 px four times that.
    assert!(hc < 0.4 * hn, "the cached page's lines are set at 2.75 px: {hc} px tall, {hn} px at the native size");
    let line = c.node("tiny/0").expect("a line").rect;
    assert!(line.h < 5.0, "a line at 2.75 px: {line:?}");
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let l = c.look();
    let px = o.render(&mut c, &l);
    let _ = o.render(&mut n, &l);
    // It is drawn: within its lines there is ink, in more than two tones (coverage, not the native
    // raster's on or off).
    let r = c.node("tiny").expect("page").rect;
    let mut tones = std::collections::BTreeSet::new();
    for y in r.y as u32..(r.y + hc) as u32 { for x in r.x as u32..(r.x + r.w).min(W as f32) as u32 { tones.insert(px[((y * W + x) * 4) as usize]); } }
    assert!(tones.len() > 4, "the small page is drawn by coverage: {} tones", tones.len());
}
