//! A fitted box (`El::fit`) whose subtree changes only what it draws costs one view and no layout a frame:
//! the scales found last time stand while nothing layout reads changes, and the boxes are those a fresh
//! layout gives. A change that moves the layout (a longer label) probes again.
use amenui::el::{El, Fit, FitMode, Paint, canvas, col, text};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone)]
struct S { t: f32, long: bool }
fn view(s: &S) -> El<()> {
    let piece = |key: &str| {
        let mut d = amenui::draw::Drawing::new(400.0, 240.0);
        for k in 0..40 { d.dot(200.0 + (s.t * 2.0 + k as f32).sin() * 180.0, 6.0 * k as f32, 3.0, Paint::TEXT); }
        col().key(format!("{key}/piece")).width(400.0).gap(4.0)
            .child(canvas(d).key(format!("{key}/canvas")).width(400.0))
            .child(text(if s.long { "a label that is now a good deal longer than it was, so it wraps" } else { "a label" }, 14.0, Paint::TEXT).key(format!("{key}/label")))
    };
    col().key("page").pad(10.0, 10.0).gap(10.0)
        .child(col().key("contain").width(200.0).height(140.0).fit(Fit::default()).child(piece("contain")))
        .child(col().key("reflow").width(220.0).height(160.0).fit(Fit { mode: FitMode::Reflow, min: 0.4, ..Fit::default() }).child(piece("reflow")))
}
fn tick(s: &mut S, t: f32) -> bool { s.t = t; s.long = t > 3.0; true }
fn open(t: f32) -> Ui<S, ()> {
    let mut u = Ui::new(S { t: 0.0, long: false }, view, |_, _| {}, fonts(), 500, 400, 1.0).on_tick(tick);
    u.tick(t);
    u.settle();
    u
}
fn boxes(u: &Ui<S, ()>) -> Vec<(String, [f32; 4])> { u.nodes().iter().map(|n| (n.key.clone(), [n.rect.x, n.rect.y, n.rect.w, n.rect.h])).collect() }

#[test]
fn a_fitted_piece_that_only_draws_differently_is_one_view_and_no_layout_a_frame() {
    let mut u = open(1.0);
    let mut t = 1.0;
    for f in 0..20 {
        t += 1.0 / 60.0;
        let c0 = u.perf().counters.clone();
        u.tick(t);
        let c1 = u.perf().counters.clone();
        assert_eq!(c1.views - c0.views, 1, "frame {f}: views");
        assert_eq!(c1.layouts - c0.layouts, 0, "frame {f}: layouts");
        assert_eq!(boxes(&u), boxes(&open(t)), "frame {f}: the boxes are not a fresh layout's");
    }
    // A longer label: laid out again, and the boxes are a fresh layout's.
    let c0 = u.perf().counters.clone();
    u.tick(3.1);
    assert!(u.perf().counters.layouts > c0.layouts, "a label that wraps lays out again");
    assert_eq!(boxes(&u), boxes(&open(3.1)));
    let c0 = u.perf().counters.clone();
    u.tick(3.2);
    assert_eq!((u.perf().counters.views - c0.views, u.perf().counters.layouts - c0.layouts), (1, 0), "settled again after the change");
}
