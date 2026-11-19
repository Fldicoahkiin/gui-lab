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
    assert!(p.stage2.len() <= dirty::MAX_BOXES);
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
