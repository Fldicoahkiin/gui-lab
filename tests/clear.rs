//! Labels shown only while clear (`El::show_clear`, DESIGN.md §1.2): checked every frame against their
//! view and each other by rank, with 4 px more to come back, fading linearly over 150 ms from where
//! they are; a hidden one takes no pointer and is still read.
use amenui::el::{El, Paint, col, text};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
#[derive(Clone, Copy)]
struct S { s: f32 }
fn label(key: &str, s: &str, x: f32, rank: u8) -> El<()> {
    text(s, 12.0, Paint::INK).key(key).width(50.0).own_size(0.0, 1.0).show_clear(8.0, rank).absolute(Some(x), Some(100.0), None, None)
}
fn view(s: &S) -> El<()> {
    col().key("page").fill_width().fill_height().child(col().key("view").width(400.0).height(300.0).view_scale(s.s)
        .child(label("b", "Pass", 160.0, 1)).child(label("a", "Ridge", 100.0, 0))
        // Far out: never 8 px inside, at any scale but 1 it is inside.
        .child(label("c", "Edge", 395.0, 0)))
}
fn open(s: f32) -> Ui<S, ()> {
    let mut u = Ui::new(S { s }, view, |_, _| {}, fonts(), 400, 300, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    u.tick(1.0);
    u.settle();
    u
}

#[test]
fn of_two_that_would_overlap_the_lower_rank_is_shown() {
    let u = open(1.0);
    assert_eq!((u.clear_opacity("a"), u.clear_opacity("b"), u.clear_opacity("c")), (Some(1.0), Some(1.0), Some(0.0)), "at 1 both clear, the edge one out");
    let u = open(0.64);
    assert_eq!((u.clear_opacity("a"), u.clear_opacity("b")), (Some(1.0), Some(0.0)), "at 0.64 they would overlap: rank 0 stays");
    // Hidden: takes no pointer, still in the accessibility tree.
    let b = u.node("b").expect("b");
    assert_eq!(b.opacity_all, 0.0);
    assert!(u.a11y().iter().any(|n| n.key == "b"), "still read");
}

#[test]
fn a_label_fades_linearly_from_where_it_is_and_does_not_flicker() {
    let mut u = open(1.0);
    let mut t = 2.0;
    let (mut last, mut dir, mut turns) = ([1.0f32; 2], [0i8; 2], [0; 2]);
    let mut seen_mid = false;
    // 1 to 0.64 over a second and back, at 60 frames a second.
    for f in 0..=120 {
        let p = f as f32 / 60.0;
        u.state.s = if p <= 1.0 { 1.0 - 0.36 * p } else { 0.64 + 0.36 * (p - 1.0) };
        u.relayout();
        t += 1.0 / 60.0;
        u.tick(t);
        for (k, key) in ["a", "b"].iter().enumerate() {
            let op = u.clear_opacity(key).expect("a label");
            let d = if op > last[k] + 1e-4 { 1 } else if op < last[k] - 1e-4 { -1 } else { 0 };
            if d != 0 {
                if dir[k] != 0 && d != dir[k] { turns[k] += 1; }
                dir[k] = d;
                // Linear over 150 ms: a frame moves it by a ninth.
                assert!((op - last[k]).abs() <= 1.0 / 9.0 + 1e-3, "{key}: {} to {op} in one frame", last[k]);
            }
            if op > 0.05 && op < 0.95 { seen_mid = true; }
            last[k] = op;
        }
        u.frame_drawn();
    }
    assert!(turns.iter().all(|&n| n <= 2), "direction changes {turns:?}");
    assert!(seen_mid, "it fades, it does not jump");
    assert_eq!(u.clear_opacity("a"), Some(1.0));
}
