//! A stage that steps by itself (`Ui::on_step`, `Ui::stage_view`, DESIGN.md §1.35): when only its
//! piece changed, only it is viewed and drawn again, with no layout, and the frame is the one a whole
//! view, layout and paint of the page gives, instance for instance; a step that lays the stage out
//! otherwise views the whole page. A count beside the stage that changes with it (`El::steps`) is viewed
//! and drawn with it, as long as it measures the same.
use amenui::el::{El, Font, Paint, canvas, col, row, text};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone, Default)]
struct S { t: f32, n: [usize; 3], tall: bool }
/// Cell `k`'s stage: strokes that change in number and place as it steps, and a line of text.
fn stage(s: &S, k: usize) -> El<()> {
    let mut d = amenui::draw::Drawing::new(180.0, 120.0);
    for j in 0..s.n[k] {
        let y = 6.0 + (j * 7 % 100) as f32 + if k == 1 { (s.t * 3.0 + j as f32).sin() * 4.0 } else { 0.0 };
        d.line((8.0, y), (170.0 - (j * 13 % 60) as f32, y + 3.0), 1.5, Paint::TEXT);
    }
    col().key(format!("cell{k}/stage")).stage().width(180.0).height(if s.tall && k == 1 { 150.0 } else { 130.0 }).gap(2.0)
        .child(canvas(d).key(format!("cell{k}/d")).width(180.0))
        .child(text("ω = 1.20", 11.0, Paint::DIM))
}
/// Cell `k`'s count, beside its stage and changing with it (`El::steps`): figures of one width, so it
/// measures the same whatever the count.
fn count(s: &S, k: usize) -> El<()> { text(format!("n = {:>2}", s.n[k]), 11.0, Paint::DIM).font(Font::Mono).key(format!("cell{k}/n")).steps() }
fn view(s: &S) -> El<()> {
    let cell = |k: usize| col().key(format!("cell{k}")).width(190.0).gap(4.0).flow(format!("cell-{k}"))
        .child(text(format!("[ CELL {k} ]"), 12.0, Paint::TEXT))
        .child(stage(s, k))
        .child(count(s, k))
        .child(text("a caption under the stage, long enough to wrap onto a second line", 12.0, Paint::DIM));
    col().key("page").pad(10.0, 10.0).gap(10.0)
        .child(text("The lab", 20.0, Paint::TEXT))
        .child(row().key("cells").gap(14.0).child(cell(0)).child(cell(1)).child(cell(2)))
        .child(text("Everything after the cells: a paragraph of text, long enough to wrap over a couple of lines here.", 14.0, Paint::TEXT).key("after").width(600.0))
}
/// Cell 1 steps; at 3 s it grows taller (laid out otherwise).
fn step(s: &mut S, t: f32) { s.t = t; s.n[1] = 12 + ((t * 7.0) as usize % 9); s.tall = t > 3.0; }
fn open(staged: bool) -> Ui<S, ()> {
    let s = S { t: 0.0, n: [20, 12, 16], tall: false };
    let u = Ui::new(s, view, |_, _| {}, fonts(), 640, 420, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    let mut u = if staged {
        u.on_step(|s, t| { step(s, t); vec!["cell1/stage".to_owned(), "cell1/n".to_owned()] })
            .stage_view(|s, key| (0..3).find_map(|k| if key == format!("cell{k}/stage") { Some(stage(s, k)) } else if key == format!("cell{k}/n") { Some(count(s, k)) } else { None }))
    } else {
        u.on_tick(|s, t| { step(s, t); true })
    };
    u.tick(1.0);
    u.settle();
    u
}
/// A frame: its instances, their owners, and what text holds.
type Frame = (Vec<amenui::scene::Instance>, Vec<(usize, u64)>, Vec<(u32, [f32; 2])>);
fn frame(u: &Ui<S, ()>) -> Frame {
    (u.scene().instances.clone(), u.owners().to_vec(), u.scene().holds.clone())
}

#[test]
fn a_stage_that_steps_alone_is_viewed_and_drawn_alone_and_the_frame_is_the_whole_one() {
    let (mut a, mut b) = (open(true), open(false));
    let mut t = 1.0;
    for f in 0..150 {
        t += 1.0 / 60.0;
        let c0 = a.perf().counters;
        a.tick(t);
        b.tick(t);
        a.settle();
        b.settle();
        let c1 = a.perf().counters;
        if t <= 3.0 {
            assert_eq!((c1.views - c0.views, c1.layouts - c0.layouts, c1.restages - c0.restages), (0, 0, 1), "frame {f}: one stage viewed alone");
        } else if t > 3.0 + 1.0 / 30.0 {
            assert_eq!(c1.views - c0.views, 0, "frame {f}: settled again");
        }
        let (fa, fb) = (frame(&a), frame(&b));
        assert_eq!(fa.0.len(), fb.0.len(), "frame {f}: instances");
        let diff = fa.0.iter().zip(&fb.0).position(|(x, y)| x != y);
        assert!(diff.is_none(), "frame {f}: instance {diff:?} differs from the whole page's: {:?} / {:?}", diff.map(|k| fa.0[k]), diff.map(|k| fb.0[k]));
        assert_eq!(fa.1, fb.1, "frame {f}: owners");
        assert_eq!(fa.2, fb.2, "frame {f}: what text holds");
        let boxes = |u: &Ui<S, ()>| u.nodes().iter().map(|n| (n.key.clone(), n.rect)).collect::<Vec<_>>();
        assert_eq!(boxes(&a), boxes(&b), "frame {f}: boxes");
    }
    // The taller stage was laid out with the page.
    assert!(a.node("cell1/stage").expect("the stage").rect.h > 140.0);
}

/// A stage whose chips move by their absolute offsets alone (the Lab's Weld): each step is restaged with
/// no layout, and the chips are where a whole layout puts them.
mod chips {
    use super::*;
    #[derive(Clone, Default)]
    pub struct C { pub t: f32 }
    pub fn stage(c: &C) -> El<()> {
        let mut st = col().key("weld").stage().width(300.0).height(200.0).pad(6.0, 6.0);
        for k in 0..3 {
            let x = 20.0 + 60.0 * k as f32 + (c.t * 2.0 + k as f32).sin() * 30.0;
            let y = 40.0 + (c.t * 3.0).cos() * 20.0 * k as f32;
            st = st.child(col().key(format!("chip{k}")).width(40.0).height(24.0).fill(Paint::FAINT).absolute(Some(x), Some(y), None, None)
                .child(text(format!("c{k}"), 11.0, Paint::TEXT)));
        }
        // One placed from the far edges.
        st.child(col().key("far").width(30.0).height(20.0).fill(Paint::FAINT).absolute(None, None, Some(10.0 + c.t.sin() * 8.0), Some(10.0)))
    }
    pub fn view(c: &C) -> El<()> { col().key("page").pad(10.0, 10.0).gap(8.0).child(text("Weld", 16.0, Paint::TEXT)).child(stage(c)) }
}

#[test]
fn chips_moved_by_their_offsets_are_restaged_without_a_layout() {
    let mk = |staged: bool| {
        let u = Ui::new(chips::C::default(), chips::view, |_, _| {}, fonts(), 400, 300, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
        let mut u = if staged { u.on_step(|c, t| { c.t = t; vec!["weld".to_owned()] }).stage_view(|c, k| (k == "weld").then(|| chips::stage(c))) } else { u.on_tick(|c, t| { c.t = t; true }) };
        u.tick(1.0);
        u.settle();
        u
    };
    let (mut a, mut b) = (mk(true), mk(false));
    let mut t = 1.0;
    let c0 = a.perf().counters;
    for _ in 0..60 {
        t += 1.0 / 60.0;
        a.tick(t);
        b.tick(t);
        a.settle();
        b.settle();
        // The chips and the text in one (its key is its path), which moves with it.
        let inner = a.nodes().iter().find(|n| n.key.starts_with("chip2/")).map(|n| n.key.clone()).expect("chip2's text");
        for key in ["chip0".to_owned(), "chip1".to_owned(), "chip2".to_owned(), inner, "far".to_owned()] {
            let (ra, rb) = (a.node(&key).unwrap().rect, b.node(&key).unwrap().rect);
            assert!((ra.x - rb.x).abs() < 0.51 && (ra.y - rb.y).abs() < 0.51 && ra.w == rb.w, "{key} at {t}: {ra:?} / {rb:?}");
        }
    }
    let c1 = a.perf().counters;
    assert_eq!((c1.views - c0.views, c1.layouts - c0.layouts, c1.restages - c0.restages), (0, 0, 60));
}

/// A pointer moving over a page that shows nothing of where it is asks for no frames.
#[test]
fn a_hover_with_nothing_changing_asks_for_no_frames() {
    let view = |_: &()| -> El<()> { col().key("page").pad(10.0, 10.0).child(text("Nothing here follows the pointer", 14.0, Paint::TEXT)).child(col().key("box").width(200.0).height(100.0).fill(Paint::FAINT)) };
    let mut u = Ui::new((), view, |_, _| {}, fonts(), 400, 300, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    let mut t = 1.0;
    u.tick(t);
    u.pointer_move(100.0, 60.0);
    while u.needs_frame(t) { t += 1.0 / 60.0; u.tick(t); u.frame_drawn(); }
    let mut asked = 0;
    for k in 0..60 {
        u.pointer_move(300.0 + (k % 10) as f32, 60.0 + (k / 10) as f32);
        if u.needs_frame(t) { asked += 1; }
    }
    assert_eq!(asked, 0, "moves over the page with nothing under them that shows the pointer");
    // Over a picture, frames follow the pointer only while the lens is on.
    for (lens, want) in [(0.0, false), (1.0, true)] {
        let view = |_: &f32| -> El<()> { col().key("page").pad(10.0, 10.0).child(amenui::el::picture(1, 0.5).key("pic").width(200.0)) };
        let mut u = Ui::new(lens, view, |_, _| {}, fonts(), 400, 300, 1.0).on_look(|l: &f32| { let mut k = Look { material: Material::Plain, ..Look::default() }; k.fx.lens = *l; k });
        let mut t = 1.0;
        u.tick(t);
        u.pointer_move(100.0, 40.0);
        while u.needs_frame(t) && t < 5.0 { t += 1.0 / 60.0; u.tick(t); u.frame_drawn(); }
        u.pointer_move(104.0, 42.0);
        assert_eq!(u.needs_frame(t), want, "lens {lens}");
    }
}

/// A stage whose chip the hand drags by its offsets (the Lab's Weld): its messages are its own
/// (`El::local_messages`), so each drag frame is restaged with no view of the page and no layout, the
/// chip where a whole layout puts it; a count beside it (`El::steps`) changes with it.
mod dragged {
    use super::*;
    #[derive(Clone, Default)]
    pub struct D { pub x: f32, pub y: f32, pub n: u32 }
    pub fn stage(d: &D, local: bool) -> El<(f32, f32)> {
        let st = col().key("weld").stage().width(300.0).height(200.0).on_drag_with(|g| Some((g.x, g.y)))
            .child(col().key("chip").width(40.0).height(24.0).fill(Paint::FAINT).absolute(Some(d.x - 20.0), Some(d.y - 12.0), None, None)
                .child(text("chip", 11.0, Paint::TEXT)));
        if local { st.local_messages() } else { st }
    }
    pub fn count(d: &D) -> El<(f32, f32)> { text(format!("moves {:>3}", d.n), 11.0, Paint::DIM).font(Font::Mono).key("count").steps() }
    pub fn view(d: &D, local: bool) -> El<(f32, f32)> { col().key("page").pad(10.0, 10.0).gap(8.0).child(text("Weld", 16.0, Paint::TEXT)).child(stage(d, local)).child(count(d)) }
}

#[test]
fn a_stage_whose_messages_are_its_own_is_restaged_as_the_hand_drags_it() {
    use amenui::ui::PointerKind;
    let mk = |local: bool| {
        let view: fn(&dragged::D) -> El<(f32, f32)> = if local { |d| dragged::view(d, true) } else { |d| dragged::view(d, false) };
        let mut u = Ui::new(dragged::D { x: 60.0, y: 60.0, n: 0 }, view, |d, (x, y)| { (d.x, d.y) = (x, y); d.n += 1; }, fonts(), 400, 300, 1.0)
            .on_look(|_| Look { material: Material::Plain, ..Look::default() })
            .stage_view(|d, k| match k { "weld" => Some(dragged::stage(d, true)), "count" => Some(dragged::count(d)), _ => None });
        u.tick(1.0);
        u.settle();
        u
    };
    let (mut a, mut b) = (mk(true), mk(false));
    let mut t = 1.0;
    let at = |u: &Ui<dragged::D, (f32, f32)>| { let r = u.node("weld").expect("stage").rect; (r.x, r.y) };
    let (ox, oy) = at(&a);
    for u in [&mut a, &mut b] { u.pointer_down(1, PointerKind::Mouse, ox + 60.0, oy + 60.0, false); u.settle(); }
    let (c0, b0) = (a.perf().counters, b.perf().counters);
    for f in 0..30 {
        t += 1.0 / 60.0;
        let (x, y) = (ox + 60.0 + f as f32 * 4.0, oy + 60.0 + f as f32 * 2.0);
        for u in [&mut a, &mut b] { u.pointer_moved(1, PointerKind::Mouse, x, y); u.tick(t); u.settle(); }
        for key in ["chip", "count"] {
            let (ra, rb) = (a.node(key).unwrap().rect, b.node(key).unwrap().rect);
            assert!((ra.x - rb.x).abs() < 0.51 && (ra.y - rb.y).abs() < 0.51 && ra.w == rb.w, "{key} at frame {f}: {ra:?} / {rb:?}");
        }
        assert_eq!(a.state.n, b.state.n);
    }
    let c1 = a.perf().counters;
    assert!(c1.restages - c0.restages >= 29, "the drag frames are restaged: {}", c1.restages - c0.restages);
    // At most once as the drag starts (the pointer's state), never per frame.
    assert!(c1.views - c0.views <= 1 && c1.layouts - c0.layouts <= 1, "no view of the page and no layout while it is dragged: {} views, {} layouts", c1.views - c0.views, c1.layouts - c0.layouts);
    // Without it, each drag frame views and lays out the page.
    let b1 = b.perf().counters;
    assert!(b1.layouts - b0.layouts >= 29, "without it each drag frame lays the page out: {}", b1.layouts - b0.layouts);
}
