//! Drawing only what changed, by element (DESIGN.md §1.19): a drawing whose strokes change in number dirties
//! its own box and not everything drawn after it, two cells that change far apart are two boxes, and a
//! frame drawn in those boxes is the frame drawn whole, byte for byte.
use amenui::dirty::{self, Region};
use amenui::el::{El, Paint, canvas, col, row, text};
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 640;
const H: u32 = 420;

/// Which cells step: each a count of strokes that changes when it steps.
#[derive(Clone, Copy)]
struct S { n: [usize; 3], mat: Material }
fn cell(k: usize, n: usize) -> El<()> {
    let mut d = amenui::draw::Drawing::new(180.0, 120.0);
    for j in 0..n { let y = 6.0 + (j * 7 % 110) as f32; d.line((8.0, y), (170.0 - (j * 13 % 60) as f32, y + 3.0), 1.5, Paint::TEXT); }
    col().key(format!("cell{k}")).width(190.0).gap(4.0)
        .child(canvas(d).key(format!("cell{k}/d")).width(180.0))
        .child(text(format!("cell {k}, a caption under it"), 12.0, Paint::DIM))
}
fn view(s: &S) -> El<()> {
    col().key("page").pad(10.0, 10.0).gap(10.0)
        .child(row().key("cells").gap(14.0).child(cell(0, s.n[0])).child(cell(1, s.n[1])).child(cell(2, s.n[2])))
        .child(text("Everything after the cells in the list: a paragraph of text to read, long enough to wrap over a couple of lines here.", 14.0, Paint::TEXT).key("after").width(600.0))
}
/// The stroke counts frame by frame (the state's tick takes the one for its second).
const SEQ: [[usize; 3]; 9] = [[20, 20, 20], [20, 20, 20], [29, 20, 20], [12, 20, 31], [12, 26, 31], [40, 3, 31], [0, 3, 0], [20, 20, 20], [20, 20, 20]];
fn tick(s: &mut S, t: f32) -> bool { let n = SEQ[(t as usize).min(SEQ.len() - 1)]; let c = n != s.n; s.n = n; c }
fn open(s: S) -> Ui<S, ()> {
    let mut u = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(|s| Look { material: s.mat, ..Look::default() }).on_tick(tick);
    u.tick(1.0);
    u.settle();
    u
}
fn area(b: dirty::Bx) -> f32 { if dirty::is_empty(b) { 0.0 } else { (b[2] - b[0]) * (b[3] - b[1]) } }

#[test]
fn a_drawing_that_changes_its_strokes_in_number_dirties_only_itself_and_two_far_apart_are_two_boxes() {
    let mut u = open(S { n: [20, 20, 20], mat: Material::Plain });
    let look = u.look();
    let prev = u.dirty_inputs(&look);
    // The first cell grows by 9 strokes: everything after it in the list is the same.
    u.tick(2.0);
    let cur = u.dirty_inputs(&look);
    let p = dirty::parts(&prev, &cur, 1.0).expect("boxes");
    let c0 = u.node("cell0").expect("cell 0").rect;
    let Region::Boxes { stage1, .. } = p.region() else { panic!() };
    assert!(stage1[2] < c0.right() + 20.0 && stage1[3] < c0.bottom() + 20.0, "the first cell's change reaches {stage1:?} (cell {c0:?})");
    // The first and the last cell: two boxes, together much less than the window.
    let prev = cur;
    u.tick(3.0);
    let cur = u.dirty_inputs(&look);
    let p = dirty::parts(&prev, &cur, 1.0).expect("boxes");
    assert_eq!(p.stage1.len(), 2, "two boxes: {:?}", p.stage1);
    let total: f32 = p.stage1.iter().map(|b| area(*b)).sum();
    assert!(total < 0.25 * (W * H) as f32, "{total} px² of {}", W * H);
    assert!(p.stage2.len() <= dirty::MAX_SHADE_BOXES);
}

#[test]
fn frames_drawn_in_the_boxes_are_the_frames_drawn_whole() {
    let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(W, H), Offscreen::try_new(W, H)) else { eprintln!("no GPU adapter here: skipped"); return };
    whole.r.partial = false;
    for mat in [Material::Plain, Material::Charr, Material::Halftonr] {
        // The same page twice, one drawn in its boxes and one drawn whole.
        let (mut u, mut v) = (open(S { n: [20, 20, 20], mat }), open(S { n: [20, 20, 20], mat }));
        let look = u.look();
        let _ = part.render(&mut u, &look);
        let _ = whole.render(&mut v, &look);
        for (f, n) in SEQ.iter().enumerate().skip(2) {
            u.tick(f as f32);
            v.tick(f as f32);
            let a = part.render(&mut u, &look);
            let b = whole.render(&mut v, &look);
            let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            assert_eq!(diff, 0, "{mat:?} frame {f} ({n:?}): {diff} bytes differ from the frame drawn whole");
        }
    }
}

/// Motes drifting all over a window-wide drawing, each a mark of its own (the home page's dust).
#[derive(Clone, Copy)]
struct M { t: f32, mat: Material }
fn motes(s: &M) -> El<()> {
    let (w, h) = (W as f32, H as f32);
    let mut d = amenui::draw::Drawing::new(w, h);
    for i in 0..16u32 {
        let (hx, hy) = ((i.wrapping_mul(2654435761) % 1000) as f32 / 1000.0, ((i * 40503 + 17) % 1000) as f32 / 1000.0);
        d.dot((hx * w + s.t * 9.0).rem_euclid(w), (hy * h - s.t * 4.0).rem_euclid(h), 1.4, Paint::TEXT);
    }
    col().key("page").child(canvas(d).key("motes").width(w))
}
fn drift(s: &mut M, t: f32) -> bool { s.t = t; true }
fn open_motes(mat: Material) -> Ui<M, ()> {
    let mut u = Ui::new(M { t: 1.0, mat }, motes, |_, _| {}, fonts(), W, H, 1.0).on_look(|s| Look { material: s.mat, ..Look::default() }).on_tick(drift);
    u.tick(1.0);
    u.settle();
    u
}

#[test]
fn motes_drifting_all_over_are_shaded_where_they_are_and_drawn_as_the_frame_drawn_whole() {
    let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(W, H), Offscreen::try_new(W, H)) else { eprintln!("no GPU adapter here: skipped"); return };
    whole.r.partial = false;
    for mat in [Material::Plain, Material::Dottr] {
        let (mut u, mut v) = (open_motes(mat), open_motes(mat));
        let look = u.look();
        let _ = part.render(&mut u, &look);
        let _ = whole.render(&mut v, &look);
        for f in 1..5 {
            let t = 1.0 + f as f32 / 60.0;
            u.tick(t);
            v.tick(t);
            let a = part.render(&mut u, &look);
            let b = whole.render(&mut v, &look);
            let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            assert_eq!(diff, 0, "{mat:?} frame {f}: {diff} bytes differ from the frame drawn whole");
            // 16 motes, each shaded in a box round it as far as the material reads (dottr's ground reads
            // farther): well under the window, which one box holding them all would be.
            let share = part.r.cost.material as f64 / (W * H) as f64;
            assert!(share < if mat == Material::Plain { 0.3 } else { 0.7 }, "{mat:?} frame {f}: the material shaded {:.0}% of the window", share * 100.0);
        }
    }
}

/// A picture given after its page is drawn (the site makes them as they come into view): the next frame
/// draws again only where it is, not the whole frame, and is the frame of a renderer that had it from the
/// start, byte for byte.
mod late_picture {
    use super::*;
    pub const PIC: u32 = 7;
    pub fn view(_: &()) -> El<()> {
        col().key("page").pad(10.0, 10.0).gap(10.0)
            .child(amenui::el::picture(PIC, 0.75).key("pic").width(200.0))
            .child(text("A caption beside the picture, and a paragraph after it to read.", 14.0, Paint::TEXT).key("after").width(600.0))
    }
    pub fn pixels() -> Vec<u8> { (0..48 * 36).flat_map(|k| { let v = (40 + (k * 7 % 160)) as u8; [v, v / 2, 200 - v / 2, 255] }).collect() }
}

#[test]
fn a_picture_given_late_draws_again_only_where_it_is() {
    let (Ok(mut late), Ok(mut early)) = (Offscreen::try_new(W, H), Offscreen::try_new(W, H)) else { eprintln!("no GPU adapter here: skipped"); return };
    early.r.set_picture(late_picture::PIC, 48, 36, &late_picture::pixels());
    let mk = || { let mut u = Ui::new((), late_picture::view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() }); u.pointer_leave(); u.tick(5.0); u.settle(); u };
    let (mut u, mut v) = (mk(), mk());
    let look = u.look();
    for _ in 0..3 { let _ = late.render(&mut u, &look); let _ = early.render(&mut v, &look); }
    late.r.set_picture(late_picture::PIC, 48, 36, &late_picture::pixels());
    u.invalidate();
    v.invalidate();
    let a = late.render(&mut u, &look);
    let b = early.render(&mut v, &look);
    let pic = u.node("pic").expect("the picture").rect;
    assert!(u.perf().dirty < 0.5, "only the picture is drawn again: {:.0} % of the frame ({})", u.perf().dirty * 100.0, u.perf().path.line());
    assert!(u.perf().dirty * (W * H) as f32 >= 0.9 * pic.w * pic.h, "the picture is drawn again: {:.0} % of the frame", u.perf().dirty * 100.0);
    let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
    assert_eq!(diff, 0, "the picture given late draws what one given first draws: {diff} bytes differ");
}

/// 3D elements that come on the page or go (scrolled into view, a section opened or closed), each in its
/// own place: the frame draws again only where one came or went, not the whole frame, and is the frame
/// drawn whole, byte for byte. One that goes before another moves the other's number on the page, not
/// what it draws: that one is not drawn again.
mod meshes_come {
    use super::*;
    use amenui::mesh::MeshId;
    #[derive(Clone, Copy)]
    pub struct S { pub ids: Option<(MeshId, MeshId)>, pub on: [bool; 3] }
    fn at(e: El<()>, x: f32, y: f32) -> El<()> { e.absolute(Some(x), Some(y), None, None) }
    pub fn view(s: &S) -> El<()> {
        let mut c = col().key("page").width(W as f32).height(H as f32)
            .child(at(text("A heading over the objects, and a line of text after it.", 14.0, Paint::TEXT).key("head").width(600.0), 10.0, 10.0));
        if let Some((a, b)) = s.ids {
            for (k, (id, x)) in [(a, 10.0), (b, 220.0), (a, 430.0)].into_iter().enumerate() {
                if s.on[k] { c = c.child(at(amenui::el::mesh(id).key(format!("m{k}")).width(160.0).height(120.0), x, 60.0)); }
            }
        }
        c.child(at(text("A paragraph after them, to read.", 14.0, Paint::TEXT).key("after").width(600.0), 10.0, 300.0))
    }
}

#[test]
fn a_3d_element_that_comes_or_goes_draws_again_only_where_it_is() {
    let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(W, H), Offscreen::try_new(W, H)) else { eprintln!("no GPU adapter here: skipped"); return };
    whole.r.partial = false;
    let mk = || {
        let mut u = Ui::new(meshes_come::S { ids: None, on: [true, true, false] }, meshes_come::view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
        let ids = (u.add_mesh(amenui::mesh::cube()), u.add_mesh(amenui::mesh::icosphere(1)));
        u.state.ids = Some(ids);
        u.relayout();
        u.pointer_leave();
        u.tick(5.0);
        u.settle();
        u
    };
    let (mut u, mut v) = (mk(), mk());
    let look = u.look();
    for _ in 0..3 { let _ = part.render(&mut u, &look); let _ = whole.render(&mut v, &look); }
    // One object's box, with the material's reach round it: what one that comes or goes may draw again.
    let one = (160.0 + 2.0 * 40.0) * (120.0 + 2.0 * 40.0) / (W * H) as f32;
    // A third comes after the two; the first goes (the others' numbers move); the first comes back
    // before them; the last goes.
    for on in [[true, true, true], [false, true, true], [true, true, true], [true, true, false]] {
        // From a page at rest (what came last has come in), the frame of the change.
        for ui in [&mut u, &mut v] { let t = ui.now(); ui.tick(t + 2.0); ui.settle(); }
        let _ = (part.render(&mut u, &look), whole.render(&mut v, &look));
        for ui in [&mut u, &mut v] { ui.state.on = on; ui.relayout(); ui.settle(); }
        let a = part.render(&mut u, &look);
        let b = whole.render(&mut v, &look);
        let line = u.perf().path.line();
        assert!(!line.contains("meshes"), "{on:?}: {line}");
        assert!(u.perf().dirty <= one, "{on:?}: only where the object came or went is drawn again: {:.0} %, one object's {:.0} % ({line})", u.perf().dirty * 100.0, one * 100.0);
        assert!(u.perf().dirty > 0.0, "{on:?}: nothing is drawn again ({line})");
        let diff = a.iter().zip(&b).filter(|(p, q)| p != q).count();
        assert_eq!(diff, 0, "{on:?}: {diff} bytes differ from the frame drawn whole");
    }
}
