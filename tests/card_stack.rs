//! A card stack's outlines, card by card (`Stack::outlines`, `MeshInstances::outline`): by default every
//! card is raised, the top one outlined in the ink and the ones under it in the hairline grey, each drawn
//! exactly in its grey.
use amenui::el::{El, Paint, col};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, LIGHT, Look, Material};
use amenui::ui::Ui;
use amenui::widgets::{Stack, StackMsg, card_stack};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 360;
const H: u32 = 240;
#[derive(Clone)]
struct S { deck: Stack, light: bool }
fn view(s: &S) -> El<()> { col().key("page").width(W as f32).height(H as f32).pad(20.0, 20.0).child(card_stack("cs", &s.deck, 320.0, 200.0, |_| ())) }
/// The stack fanned under its top card, so each card's edge shows.
fn fanned(outlines: Vec<Paint>) -> Stack {
    let mut d = Stack::default();
    d.outlines = outlines;
    d.update(StackMsg::Hover(Some(0)), 0.0);
    let mut t = 0.0;
    while d.moving() && t < 5.0 { t += 1.0 / 60.0; d.step(t); }
    d
}
/// Pixels within 1 of `c` in the stack's box.
fn count(f: &[u8], c: [f32; 3]) -> usize {
    let c = c.map(|v| (v * 255.0).round() as i32);
    f.chunks_exact(4).filter(|p| (0..3).all(|k| (p[k] as i32 - c[k]).abs() <= 1)).count()
}

#[test]
fn the_top_card_is_outlined_in_ink_and_the_ones_under_it_in_hairline() {
    assert_eq!((Stack::default().paints, Stack::default().outlines), (vec![Paint::RAISED], vec![Paint::INK, Paint::HAIRLINE]));
    let mut o = Offscreen::new(W, H);
    for light in [false, true] {
        let th = if light { LIGHT } else { DARK };
        let shot = |o: &mut Offscreen, deck: Stack| {
            let mut u = Ui::new(S { deck, light }, view, |_, _| {}, fonts(), W, H, 1.0)
                .on_look(|s| Look { material: Material::Plain, theme: if s.light { LIGHT } else { DARK }, ..Look::default() });
            u.tick(2.0);
            u.settle();
            let l = u.look();
            o.r.partial = false;
            o.render(&mut u, &l)
        };
        // The cards under the top one, turned, have whole pixels of their outline: exactly the grey asked.
        let both = shot(&mut o, fanned(vec![Paint::INK, Paint::HAIRLINE]));
        let under_ink = shot(&mut o, fanned(vec![Paint::HAIRLINE, Paint::INK]));
        let (bh, ui) = (count(&both, th.greys.hairline), count(&under_ink, th.ink));
        eprintln!("{}: hairline under {bh} hairline px; ink under {ui} ink px", th.name);
        assert!(bh > 150, "{}: the cards under the top one in the hairline grey: {bh} px", th.name);
        assert!(ui > 100, "{}: asked for ink, the cards under the top one are the ink: {ui} px", th.name);
        // The top card (square to the pixels, its outline on no whole pixel) alone changes with its own
        // grey: ink is further from the raised grey than hairline wherever they differ.
        let top_h = shot(&mut o, fanned(vec![Paint::HAIRLINE, Paint::HAIRLINE]));
        let raised = th.greys.raised.map(|v| (v * 255.0).round() as i32);
        let far = |p: &[u8]| (0..3).map(|k| (p[k] as i32 - raised[k]).abs()).sum::<i32>();
        let diff: Vec<usize> = (0..both.len() / 4).filter(|&i| both[i * 4..i * 4 + 3] != top_h[i * 4..i * 4 + 3]).collect();
        assert!(diff.len() > 100, "{}: the top card's outline changes with its grey: {} px", th.name, diff.len());
        assert!(diff.iter().all(|&i| far(&both[i * 4..i * 4 + 3]) > far(&top_h[i * 4..i * 4 + 3])), "{}: the top card's ink outline is toward the ink", th.name);
        assert!(count(&both, th.greys.raised) > 4000, "{}: the cards are raised", th.name);
    }
}
