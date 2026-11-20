//! A card stack's outlines, card by card (`Stack::outlines`, `MeshInstances::outline`): by default every
//! card is raised, the top one outlined in the ink and the ones under it in the hairline grey, each drawn
//! exactly in its grey; a turned card's outline is a smooth line.
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
        // grey: in ink it is nearer the ink than in hairline wherever they differ.
        let top_h = shot(&mut o, fanned(vec![Paint::HAIRLINE, Paint::HAIRLINE]));
        let ink = th.ink.map(|v| (v * 255.0).round() as i32);
        let off_ink = |p: &[u8]| (0..3).map(|k| (p[k] as i32 - ink[k]).abs()).sum::<i32>();
        let diff: Vec<usize> = (0..both.len() / 4).filter(|&i| both[i * 4..i * 4 + 3] != top_h[i * 4..i * 4 + 3]).collect();
        assert!(diff.len() > 100, "{}: the top card's outline changes with its grey: {} px", th.name, diff.len());
        assert!(diff.iter().all(|&i| off_ink(&both[i * 4..i * 4 + 3]) < off_ink(&top_h[i * 4..i * 4 + 3])), "{}: the top card's ink outline is toward the ink", th.name);
        assert!(count(&both, th.greys.raised) > 4000, "{}: the cards are raised", th.name);
    }
}

#[test]
fn a_turned_cards_outline_is_a_smooth_line() {
    // Along the turned cards' upper left edge, each row: the ground, then a pixel partly toward the
    // hairline (the edge's share of it). Where the edge falls in a row, from that share, lies on a straight
    // line to within 0.45 px; steps of whole texels are 0.5 px off it at the least (1.1 at the old 0.5).
    let mut o = Offscreen::new(W, H);
    for light in [false, true] {
        let th = if light { LIGHT } else { DARK };
        let mut u = Ui::new(S { deck: fanned(vec![Paint::INK, Paint::HAIRLINE]), light }, view, |_, _| {}, fonts(), W, H, 1.0)
            .on_look(|s| Look { material: Material::Plain, theme: if s.light { LIGHT } else { DARK }, ..Look::default() });
        u.tick(2.0);
        u.settle();
        let l = u.look();
        o.r.partial = false;
        let f = o.render(&mut u, &l);
        let (g, h) = ((th.ground[0] * 255.0).round(), (th.greys.hairline[0] * 255.0).round());
        let at = |x: u32, y: u32| f[((y * W + x) * 4) as usize] as f32;
        let (mut edge, mut soft) = (vec![], 0);
        for y in 40..70u32 {
            let x = (0..W).find(|&x| (at(x, y) - g).abs() > 1.0).expect("the card");
            let share = ((at(x, y) - g) / (h - g)).clamp(0.0, 1.0);
            if share < 0.95 { soft += 1; }
            edge.push((y as f32, x as f32 + 1.0 - share));
        }
        let n = edge.len() as f32;
        let (my, mx) = (edge.iter().map(|e| e.0).sum::<f32>() / n, edge.iter().map(|e| e.1).sum::<f32>() / n);
        let slope = edge.iter().map(|e| (e.0 - my) * (e.1 - mx)).sum::<f32>() / edge.iter().map(|e| (e.0 - my).powi(2)).sum::<f32>();
        let off = edge.iter().map(|e| (e.1 - (mx + slope * (e.0 - my))).abs()).fold(0.0, f32::max);
        assert!(soft >= 27, "{}: the edge's first pixel is its share of the line: {soft} of 30 rows", th.name);
        assert!(slope < -0.5 && off < 0.45, "{}: the edge is a straight line: slope {slope}, {off} px off it at most", th.name);
    }
}
