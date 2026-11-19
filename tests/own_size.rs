//! Own-size elements (`El::own_size`, DESIGN.md §1.23): a label on a zoomed drawing keeps its type and
//! box, pixel for pixel, at any zoom and device pixel ratio; its anchor goes where the zoomed box would
//! put it, on whole device pixels; `El::fit` measures it at its own size; one in a row that the zoom
//! pushes out of its parent is reported. Pixel faces never draw smaller at a higher ratio than at 1.
use amenui::el::{El, Fit, Paint, col, row, space, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::text::{PIXEL, Text};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: f32 = 400.0;
const H: f32 = 240.0;
/// Where the label's anchor (its bottom middle) is on the drawing at 1.
const AT: (f32, f32) = (150.0, 120.0);
/// Where the drawing is on the page.
const MAP: (f32, f32) = (7.3, 5.0);

#[derive(Clone, Copy)]
struct S { k: f32 }
fn view(s: &S) -> El<()> {
    let map = col().key("map").width(W).height(H)
        .child(space(W, 2.0).key("rule").fill(Paint::DIM))
        .child(text("Ridge 1240", 13.0, Paint::INK).key("label").own_size(0.5, 1.0).absolute(Some(AT.0 - 40.0), Some(AT.1 - 21.0), None, None).width(80.0));
    col().key("page").fill_width().fill_height().child(map.zoom(s.k).absolute(Some(MAP.0), Some(MAP.1), None, None))
}
fn open(k: f32, dpr: f32) -> Ui<S, ()> {
    let mut u = Ui::new(S { k }, view, |_, _| {}, fonts(), (W * dpr) as u32, (H * dpr) as u32, dpr).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    u.tick(3.0);
    u.settle();
    u
}

#[test]
fn a_label_keeps_its_own_size_and_sits_at_its_zoomed_anchor_on_whole_device_pixels() {
    let Ok(_) = Offscreen::try_new(8, 8) else { eprintln!("no GPU adapter here: skipped"); return };
    for dpr in [1.0f32, 1.5, 2.0] {
        let (dw, dh) = ((W * dpr) as u32, (H * dpr) as u32);
        let mut o = Offscreen::new(dw, dh);
        let shot = |o: &mut Offscreen, k: f32| {
            let mut u = open(k, dpr);
            let l = u.look();
            o.r.partial = false;
            let px = o.render(&mut u, &l);
            let r = u.node("label").expect("label").rect;
            (px, r)
        };
        let (px1, r1) = shot(&mut o, 1.0);
        let crop = |px: &[u8], r: amenui::ui::Rect| -> Vec<u8> {
            let (x0, y0) = ((r.x * dpr).round() as u32, (r.y * dpr).round() as u32);
            let (w, h) = ((r.w * dpr).round() as u32, (r.h * dpr).round() as u32);
            (y0..y0 + h).flat_map(|y| (x0..x0 + w).flat_map(move |x| { let i = ((y * dw + x) * 4) as usize; [px[i], px[i + 1], px[i + 2]] })).collect()
        };
        let glyphs1 = crop(&px1, r1);
        assert!(glyphs1.iter().any(|&c| c < 128), "dpr {dpr}: the label is drawn");
        for k in [0.8f32, 0.64, 0.4] {
            let (px, r) = shot(&mut o, k);
            assert!((r.w - r1.w).abs() < 1e-3 && (r.h - r1.h).abs() < 1e-3, "dpr {dpr}, zoom {k}: its box {r:?} is not its own {r1:?}");
            for v in [r.x, r.y] { assert!(((v * dpr) - (v * dpr).round()).abs() < 1e-3, "dpr {dpr}, zoom {k}: {v} is not on a device pixel"); }
            // Its anchor (bottom middle) where the zoomed drawing puts it (the drawing is at `MAP`, not on
            // a whole pixel), within a device pixel.
            let (anchor, want) = ((r.x + 0.5 * r.w, r.bottom()), (MAP.0 + AT.0 * k, MAP.1 + AT.1 * k));
            assert!(((anchor.0 - want.0) * dpr).abs() < 1.0 && ((anchor.1 - want.1) * dpr).abs() < 1.0, "dpr {dpr}, zoom {k}: anchor at {anchor:?}, the zoomed box puts it at {want:?}");
            assert!(crop(&px, r) == glyphs1, "dpr {dpr}, zoom {k}: its type is not drawn as at 1");
        }
    }
}

#[test]
fn a_pixel_face_is_never_smaller_at_a_higher_device_pixel_ratio() {
    let mut t = Text::new(fonts());
    let native = t.native(PIXEL).expect("a pixel face");
    for size in [5.0f32, 7.0, 12.0, 15.0, 18.0, 30.0] {
        t.set_scale(1.0);
        let at1 = t.size_for(PIXEL, size);
        let m1 = (size / native).round().max(1.0);
        assert_eq!(at1, m1 * native, "{size}");
        for dpr in [1.25f32, 1.5, 2.0, 2.5, 3.0] {
            t.set_scale(dpr);
            let s = t.size_for(PIXEL, size);
            let m = s * dpr / native;
            assert!((m - m.round()).abs() < 1e-3, "{size} at {dpr}: {m} native sizes, not a whole number");
            let want = (size * dpr / native).round().max((m1 * dpr).floor()).max(1.0);
            assert_eq!(m.round(), want, "{size} at {dpr}");
            if dpr.fract() == 0.0 { assert!(s >= at1 - 1e-3, "{size} at {dpr}: {s} logical px, smaller than {at1} at 1"); }
        }
    }
    // 7 px of a 12 px face at 2 is two of its pixels a pixel, as at 1, not one.
    t.set_scale(2.0);
    assert_eq!(t.size_for(PIXEL, 7.0) * 2.0, 2.0 * native);
}

#[derive(Clone, Copy)]
struct F { w: f32 }
fn fit_view(s: &F) -> El<()> {
    // A row of four 100 px bars and a label at its own size between them, fitted into `w`.
    let bars = (0..4).map(|i| space(100.0, 40.0).key(format!("bar/{i}")).fill(Paint::DIM));
    let inner = row().key("strip").gap(10.0).children(bars.take(2)).child(text("Ridge", 13.0, Paint::INK).key("tag").own_size(0.5, 0.5).nowrap())
        .children((2..4).map(|i| space(100.0, 40.0).key(format!("bar/{i}")).fill(Paint::DIM)));
    col().key("page").fill_width().fill_height().child(col().key("box").width(s.w).height(120.0).fit(Fit { min: 0.1, ..Fit::default() }).child(inner))
}

#[test]
fn fit_measures_own_size_elements_at_their_own_size_and_an_overflow_is_reported() {
    let mut big = Ui::new(F { w: 2000.0 }, fit_view, |_, _| {}, fonts(), 2100, 300, 1.0);
    big.tick(1.0);
    let natural = big.node("strip").expect("strip").rect;
    let tag = big.node("tag").expect("tag").rect;
    for w in [300.0f32, 220.0, 160.0] {
        let mut u = Ui::new(F { w }, fit_view, |_, _| {}, fonts(), 2100, 300, 1.0);
        u.tick(1.0);
        let k = u.fitted("box").expect("fitted");
        let strip = u.node("strip").expect("strip").rect;
        let b = u.node("box").expect("box").inner;
        // The zoomed row is as wide as its box (within a pixel), not narrower: the label did not shrink.
        assert!(strip.w <= b.w + 0.5 && strip.w >= b.w - 1.5, "{w}: the row is {} wide in {}", strip.w, b.w);
        let t = u.node("tag").expect("tag").rect;
        assert!((t.w - tag.w).abs() < 1e-3, "{w}: the label is {} wide, its own is {}", t.w, tag.w);
        assert!(k * (natural.w - tag.w) + tag.w <= b.w + 1.5, "{w}: k {k}");
    }
    // At a zoom that leaves the label wider than its row's box, the audit says so.
    let probe = |v: fn(&F) -> El<()>| { let mut u = Ui::new(F { w: 0.0 }, v, |_, _| {}, fonts(), 400, 300, 1.0); u.tick(1.0); u.problems() };
    let squeezed = |_: &F| col().key("page").child(row().key("holder").width(60.0).child(text("A label at its own size", 13.0, Paint::INK).key("wide").own_size(0.0, 0.0).nowrap()).zoom(0.5));
    assert!(probe(squeezed).iter().any(|p| matches!(p, amenui::error::ViewProblem::OwnSizeOver { key, parent } if key == "wide" && parent == "holder")), "{:?}", probe(squeezed));
    let placed = |_: &F| col().key("page").child(col().key("holder").width(60.0).height(40.0).child(text("A label at its own size", 13.0, Paint::INK).key("wide").own_size(0.0, 0.0).absolute(Some(4.0), Some(4.0), None, None)).zoom(0.5));
    assert!(probe(placed).is_empty(), "{:?}", probe(placed));
}
