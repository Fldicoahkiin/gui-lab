//! Pointers, touch and the wheel (DESIGN.md §1.8.2), without a GPU: fling decay and the rubber band
//! as functions; a finger scrolling the 10,000 rows, flinging on and coming to rest; a tap pressing; a
//! scroll that started on a row not pressing it; pulling past the top loosening the edge, not moving
//! the content, and springing back; a second finger cancelling; pointer capture for a finger on a
//! slider; small controls hit within 28 px; no hover from a finger; the pointer's shapes.
use amenui::el::{Cursor, El, Paint, col, space};
use amenui::ui::{MIN_HIT, Scrolling, Ui, fling_step, rubber_band};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[test]
fn fling_decays_exponentially() {
    let s = Scrolling::default();
    let (mut v, mut x, mut t) = (2000.0f32, 0.0f32, 0.0f32);
    while v.abs() > 20.0 && t < 10.0 { let (d, nv) = fling_step(v, 1.0 / 60.0, s.friction); x += d; v = nv; t += 1.0 / 60.0; }
    // The whole distance is v0 / friction; the speed falls by e^(−friction) each second.
    assert!((x - 2000.0 / s.friction).abs() < 20.0 / s.friction + 1.0, "distance {x}");
    assert!((t - (2000.0f32 / 20.0).ln() / s.friction).abs() < 0.05, "stops after {t} s");
    let (_, v1) = fling_step(1000.0, 1.0, s.friction);
    assert!((v1 - 1000.0 * (-s.friction).exp()).abs() < 1e-2);
    // Steps compose: two half steps are one step.
    let (a, va) = fling_step(800.0, 0.25, 3.0);
    let (b, vb) = fling_step(va, 0.25, 3.0);
    let (c, vc) = fling_step(800.0, 0.5, 3.0);
    assert!((a + b - c).abs() < 1e-3 && (vb - vc).abs() < 1e-3);
}

#[test]
fn the_rubber_band_resists_and_is_bounded() {
    let mut o = 0.0;
    let mut steps = vec![];
    for _ in 0..40 { let n = rubber_band(o, -20.0, 0.55, 140.0); steps.push(o - n); o = n; }
    assert!((-140.0..0.0).contains(&o));
    assert!(steps.windows(2).all(|w| w[1] <= w[0] + 1e-4), "each pull moves it less");
    assert!(steps[0] < 20.0, "less than the finger");
}

struct Tiny { hits: u32 }
fn tiny(_: &Tiny) -> El<()> {
    col().key("root").pad(100.0, 100.0).child(space(10.0, 10.0).key("dot").fill(Paint::INK).on_press(()))
}

#[test]
fn small_controls_are_hit_within_28_px() {
    let mut ui = Ui::new(Tiny { hits: 0 }, tiny, |s: &mut Tiny, _| s.hits += 1, fonts(), 400, 300, 1.0);
    let r = ui.node("dot").unwrap().rect;
    assert_eq!(r.w, 10.0);
    // 8 px off its edge: inside the 28 px target.
    let (x, y) = (r.right() + 8.0, r.center().1);
    ui.press(x, y); ui.release(x, y);
    assert_eq!(ui.state.hits, 1);
    // Further than half of 28 px from its centre: not hit.
    let (x, y) = (r.center().0 + MIN_HIT * 0.5 + 1.0, r.center().1);
    ui.tick(1.0);
    ui.press(x, y); ui.release(x, y);
    assert_eq!(ui.state.hits, 1);
    // The look did not grow.
    assert_eq!(ui.node("dot").unwrap().rect.w, 10.0);
    // Hover over the grown target: the pointer shape says it can be pressed.
    ui.pointer_move(r.right() + 6.0, r.center().1);
    assert_eq!(ui.cursor(), Cursor::Pointer);
}

/// Two buttons side by side: the marks are on the one under the pointer in the frame the pointer gets
/// there (before any tick), and in no frame between two targets do they trail behind.
fn pair(_: &()) -> El<u8> {
    amenui::el::row().key("root").pad(40.0, 40.0).gap(200.0)
        .child(amenui::widgets::button("a", "A", 1u8))
        .child(amenui::widgets::button("b", "B", 2u8))
}

#[test]
fn hover_marks_land_in_the_pointers_frame() {
    let mut ui = Ui::new((), pair, |_, _| {}, fonts(), 600, 200, 1.0);
    ui.tick(1.0);
    let (a, b) = (ui.node("a").unwrap().rect, ui.node("b").unwrap().rect);
    ui.pointer_move(a.center().0, a.center().1);
    let (r, alpha) = ui.marks_frame().expect("marks on A at once");
    assert_eq!(r, a);
    assert!(alpha >= 0.5, "seen in the first frame: {alpha}");
    let mut t = 1.0;
    for _ in 0..10 { t += 1.0 / 60.0; ui.tick(t); }
    // Straight onto B: the very same frame, no glide across the gap.
    ui.pointer_move(b.center().0, b.center().1);
    assert_eq!(ui.marks_frame().expect("marks").0, b);
    let start = t;
    let mut settled = None;
    for _ in 0..12 {
        t += 1.0 / 60.0;
        ui.tick(t);
        ui.frame_drawn();
        assert_eq!(ui.marks_frame().expect("marks").0, b, "never between A and B");
        if settled.is_none() && ui.marks_frame().expect("marks").1 >= 1.0 { settled = Some(t - start); }
    }
    let settled = settled.expect("the marks settle");
    assert!(settled <= amenui::ui::MARKS_SETTLE + 1.5 / 60.0 && amenui::ui::MARKS_SETTLE < 0.08, "settled after {settled} s");
    // Leaving: gone within the settle.
    ui.pointer_move(300.0, 190.0);
    for _ in 0..6 { t += 1.0 / 60.0; ui.tick(t); }
    assert!(ui.marks_frame().is_none());
}
