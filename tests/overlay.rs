//! `stack` and `layers` on flexbox alone: a stack is as wide as its widest child and as tall as its
//! tallest, each child centred in it, and follows them when they change; layers are pinned at the top
//! left of a box that does not grow with them.
use amenui::el::{El, Paint, col, layers, row, space, stack, text};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const LINE: &str = "a line of words wider than the boxes";
#[derive(Clone, Default)]
struct S { wide: bool }
fn view(s: &S) -> El<()> {
    let label = if s.wide { text(LINE, 14.0, Paint::TEXT).key("t") } else { text("short", 14.0, Paint::TEXT).key("t") };
    col().key("page").pad(10.0, 10.0).gap(10.0)
        .child(row().key("ref").child(text(LINE, 14.0, Paint::TEXT).key("alone")))
        .child(row().key("r")
            .child(stack([space(40.0, 10.0).key("a"), label, space(10.0, 60.0).key("b")]).key("s"))
            .child(space(20.0, 20.0).key("after")))
        .child(stack([space(30.0, 30.0).key("c"), space(80.0, 10.0).key("d")]).key("sized").width(240.0).height(120.0))
        .child(layers([space(50.0, 30.0).key("l0"), col().key("l1").width(60.0).height(300.0)]).key("ls").fill_width().height(100.0))
        .child(space(10.0, 10.0).key("below"))
}

#[test]
fn a_stack_is_its_largest_child_and_centres_the_rest() {
    let mut u: Ui<S, ()> = Ui::new(S { wide: true }, view, |_, _| {}, fonts(), 800, 900, 1.0);
    u.tick(0.0);
    let r = |u: &Ui<S, ()>, k: &str| u.node(k).unwrap_or_else(|| panic!("{k}")).rect;
    let check = |u: &Ui<S, ()>, w: f32| {
        let (s, t) = (r(u, "s"), r(u, "t"));
        assert!((s.w - w).abs() < 0.6 && (s.h - 60.0).abs() < 0.6, "the stack is {}×{}, its widest child {w}", s.w, s.h);
        for k in ["a", "t", "b"] {
            let c = r(u, k);
            assert!((c.x + c.w / 2.0 - (s.x + s.w / 2.0)).abs() < 0.6 && (c.y + c.h / 2.0 - (s.y + s.h / 2.0)).abs() < 0.6, "{k} centred: {c:?} in {s:?}");
        }
        assert!((r(u, "after").x - (s.x + s.w)).abs() < 0.6, "what follows the stack starts after it");
        t
    };
    let alone = r(&u, "alone");
    let t = check(&u, alone.w);
    assert!((t.h - alone.h).abs() < 0.6, "the text is not wrapped in the stack: {} / {}", t.h, alone.h);
    // Narrower children: the stack follows them back down.
    u.state.wide = false;
    u.tick(0.1);
    check(&u, r(&u, "a").w.max(r(&u, "t").w));
    u.state.wide = true;
    u.tick(0.2);
    check(&u, alone.w);
    // Sized, it keeps its size and centres its children.
    let s = r(&u, "sized");
    assert!((s.w - 240.0).abs() < 0.6 && (s.h - 120.0).abs() < 0.6);
    for k in ["c", "d"] {
        let c = r(&u, k);
        assert!((c.x + c.w / 2.0 - (s.x + 120.0)).abs() < 0.6 && (c.y + c.h / 2.0 - (s.y + 60.0)).abs() < 0.6, "{k} centred in the sized stack");
    }
}

#[test]
fn layers_lie_at_the_top_left_of_a_box_that_does_not_grow() {
    let mut u: Ui<S, ()> = Ui::new(S::default(), view, |_, _| {}, fonts(), 800, 900, 1.0);
    u.tick(0.0);
    let r = |k: &str| u.node(k).unwrap_or_else(|| panic!("{k}")).rect;
    let ls = r("ls");
    assert!((ls.h - 100.0).abs() < 0.6 && (ls.w - 780.0).abs() < 0.6, "the box is its own size: {ls:?}");
    for k in ["l0", "l1"] { assert!((r(k).x - ls.x).abs() < 0.01 && (r(k).y - ls.y).abs() < 0.01, "{k} at the top left"); }
    assert!((r("l1").h - 300.0).abs() < 0.6, "the taller layer hangs out of the box");
    assert!((r("below").y - (ls.y + 110.0)).abs() < 0.6, "what follows starts after the box, not the layer");
}
