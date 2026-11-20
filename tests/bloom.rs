//! Bloom in a region (`El::bloom`, DESIGN.md §1.21): once an element is marked, the look's bloom glows
//! only inside the box round what marked elements draw and only from what is inside it, so a bright
//! control beside it stays its own colour; with none marked it covers the frame. The region follows
//! what is marked as it scrolls, and a scroll's moved frame is the frame drawn whole.
use amenui::el::{El, Paint, col, row, space};
use amenui::offscreen::Offscreen;
use amenui::post::{Bloom, PostSpec};
use amenui::style::{DARK, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 320;
const H: u32 = 200;
#[derive(Clone, Copy)]
struct S { marked: bool }
/// A bright stone and, 6 px from it, a bright console plate, both in the ink, in a page that scrolls.
fn view(s: &S) -> El<()> {
    let stone = space(100.0, 80.0).key("stone").fill(Paint::INK);
    let pair = row().key("pair").gap(6.0).pad(30.0, 40.0).child(if s.marked { stone.bloom() } else { stone }).child(space(100.0, 80.0).key("console").fill(Paint::INK));
    col().key("page").fill_width().height(H as f32).scroll().child(col().key("content").child(pair).child(space(10.0, 400.0)))
}
fn look(_: &S) -> Look { Look { material: Material::Plain, post: PostSpec { bloom: Bloom { strength: 0.5, threshold: 0.7, radius: 8.0 }, ..PostSpec::default() }.look(), ..Look::default() } }
fn open(s: S) -> Ui<S, ()> {
    let mut u = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
    u.tick(2.0);
    u.settle();
    u
}
fn shot(o: &mut Offscreen, u: &mut Ui<S, ()>, partial: bool) -> Vec<u8> { u.settle(); let l = u.look(); o.r.partial = partial; o.render(u, &l) }
fn px(f: &[u8], x: f32, y: f32) -> [u8; 3] { let i = ((y as u32 * W + x as u32) * 4) as usize; [f[i], f[i + 1], f[i + 2]] }
fn ink() -> [u8; 3] { DARK.ink.map(|v| (v * 255.0).round() as u8) }

#[test]
fn bloom_glows_only_inside_the_marked_element_and_from_it() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut all = open(S { marked: false });
    let fa = shot(&mut o, &mut all, false);
    let mut only = open(S { marked: true });
    let fo = shot(&mut o, &mut only, false);
    let (st, co) = (only.node("stone").expect("stone").rect, only.node("console").expect("console").rect);
    assert_eq!(only.look().post.bloom_region, Some([st.x, st.y, st.right(), st.bottom()]), "the stone's box");
    assert_eq!(all.look().post.bloom_region, None);
    let (cx, cy) = (co.x + co.w * 0.5, co.y + co.h * 0.5);
    // Unmarked, everything blooms: the console is lit past the ink (what the site saw).
    assert!(px(&fa, cx, cy)[0] > ink()[0] + 4, "no mark: the console blooms: {:?}", px(&fa, cx, cy));
    // Marked: the console and the ground beside the stone are as drawn; the stone glows.
    assert_eq!(px(&fo, cx, cy), ink(), "the console stays the ink");
    assert_eq!(px(&fo, co.x + 1.0, cy), ink(), "the console's edge by the stone stays the ink");
    let gap = px(&fo, st.right() + 3.0, cy);
    assert_eq!(gap, px(&fo, 4.0, 4.0), "the ground between them is unlit");
    let (sx, sy) = (st.x + st.w * 0.5, st.y + st.h * 0.5);
    assert!(px(&fo, sx, sy)[0] > ink()[0] + 4, "the stone blooms: {:?}", px(&fo, sx, sy));
    // Deeper inside it than the bloom reaches, it is the bloom over the whole frame, pixel for pixel.
    for y in (st.y as u32 + 10)..(st.bottom() as u32 - 10) {
        for x in (st.x as u32 + 10)..(st.right() as u32 - 10) { assert_eq!(px(&fo, x as f32, y as f32), px(&fa, x as f32, y as f32), "({x}, {y})"); }
    }
}

#[test]
fn the_region_follows_a_scroll_and_the_moved_frame_is_the_frame_drawn_whole() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut u = open(S { marked: true });
    let _ = shot(&mut o, &mut u, true);
    let mut t = 2.0;
    u.wheel(160.0, 100.0, 20.0);
    let mut moved = vec![];
    for _ in 0..30 { t += 1.0 / 60.0; u.tick(t); moved = shot(&mut o, &mut u, true); }
    let st = u.node("stone").expect("stone").rect;
    assert_eq!(st.y, 20.0, "scrolled by 20");
    assert_eq!(u.look().post.bloom_region, Some([st.x, st.y, st.right(), st.bottom()]), "the region where the stone is now");
    let whole = shot(&mut o, &mut u, false);
    assert!(moved == whole, "the scrolled frame is the frame drawn whole");
    let co = u.node("console").expect("console").rect;
    assert_eq!(px(&whole, co.x + co.w * 0.5, co.y + co.h * 0.5), ink(), "the console stays the ink");
}
