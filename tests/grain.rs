//! Grain in a region (`El::grain`, DESIGN.md §1.21): once an element is marked, the look's grain is drawn
//! only over what marked elements draw, the same grain as over the whole frame; with none marked it
//! covers the frame.
use amenui::el::{El, Paint, col};
use amenui::offscreen::Offscreen;
use amenui::post::{Grain, PostSpec};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 320;
const H: u32 = 200;
/// The window: where it is, and whether it is marked.
#[derive(Clone, Copy)]
struct S { at: (f32, f32), marked: bool }
fn view(s: &S) -> El<()> {
    let win = col().key("window").width(120.0).height(80.0).fill(Paint::tone(0.5)).absolute(Some(s.at.0), Some(s.at.1), None, None);
    col().key("page").fill_width().fill_height().child(if s.marked { win.grain() } else { win })
}
fn look(_: &S) -> Look { Look { material: Material::Plain, post: PostSpec { grain: Grain { amount: 0.2, size: 1.0 }, ..PostSpec::default() }.look(), ..Look::default() } }
fn shot(o: &mut Offscreen, s: S, dpr: f32) -> (Vec<u8>, Ui<S, ()>) {
    let mut u = Ui::new(s, view, |_, _| {}, fonts(), (W as f32 * dpr) as u32, (H as f32 * dpr) as u32, dpr).on_look(look);
    u.tick(2.0);
    u.settle();
    let l = u.look();
    o.r.partial = false;
    (o.render(&mut u, &l), u)
}

#[test]
fn grain_is_drawn_only_over_marked_elements_and_is_the_page_grain_there() {
    let Ok(_) = Offscreen::try_new(8, 8) else { eprintln!("no GPU adapter here: skipped"); return };
    for dpr in [1.0f32, 2.0] {
        let (w, h) = ((W as f32 * dpr) as u32, (H as f32 * dpr) as u32);
        let mut o = Offscreen::new(w, h);
        let (all, _) = shot(&mut o, S { at: (100.0, 60.0), marked: false }, dpr);
        let (only, u) = shot(&mut o, S { at: (100.0, 60.0), marked: true }, dpr);
        let r = u.look().post.region.expect("a region");
        assert_eq!(r, [100.0 * dpr, 60.0 * dpr, 220.0 * dpr, 140.0 * dpr], "dpr {dpr}: the window's box");
        let px = |f: &[u8], x: u32, y: u32| { let i = ((y * w + x) * 4) as usize; [f[i], f[i + 1], f[i + 2]] };
        let inside = |x: u32, y: u32| (x as f32) >= r[0] && (y as f32) >= r[1] && (x as f32) < r[2] && (y as f32) < r[3];
        let (mut outside, mut grained) = (std::collections::HashSet::new(), 0);
        for y in 0..h {
            for x in 0..w {
                if inside(x, y) {
                    // The same grain as over the whole frame, pixel for pixel.
                    assert_eq!(px(&only, x, y), px(&all, x, y), "dpr {dpr}: ({x}, {y}) inside");
                    if px(&only, x, y) != px(&only, r[0] as u32 + 2, r[1] as u32 + 2) { grained += 1; }
                } else {
                    outside.insert(px(&only, x, y));
                }
            }
        }
        assert_eq!(outside.len(), 1, "dpr {dpr}: outside the window the ground is one colour, no grain");
        assert!(grained > 100, "dpr {dpr}: the window is grained");
        // Over the whole frame without a mark: the ground outside the window is grained too.
        let outside_all: std::collections::HashSet<[u8; 3]> = (0..w).map(|x| px(&all, x, 4)).collect();
        assert!(outside_all.len() > 1, "dpr {dpr}: no mark, grain everywhere");
    }
}

#[test]
fn the_region_follows_the_element_and_is_empty_when_it_is_not_drawn() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let (_, u) = shot(&mut o, S { at: (10.0, 20.0), marked: true }, 1.0);
    assert_eq!(u.look().post.region, Some([10.0, 20.0, 130.0, 100.0]));
    let (_, u) = shot(&mut o, S { at: (400.0, 20.0), marked: true }, 1.0);
    assert_eq!(u.look().post.region, Some([0.0; 4]), "off the window: no grain anywhere");
    let (_, u) = shot(&mut o, S { at: (10.0, 20.0), marked: false }, 1.0);
    assert_eq!(u.look().post.region, None);
}
