//! Text flowing around the zine's sign (DESIGN.md §1.9): at twenty places of the sign, no two glyphs
//! of the paragraph overlap, none is under the sign, and a sub-pixel nudge does not move a line break.
use lab::gallery::film::{self, FMsg};
use lab::gallery::{self, GalleryUi, Msg};

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn open() -> GalleryUi {
    let mut ui = gallery::new_ui(fonts(), 1280, 640, 1.0);
    ui.pointer_leave();
    assert!(ui.state.route("film"));
    if ui.state.film.playing { gallery::update(&mut ui.state, Msg::Film(FMsg::Play)); }
    film::go(&mut ui.state, 6, None);
    ui.relayout();
    for i in 1..=180 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
/// The glyph boxes of the paragraph, ink only (the spread padding taken off).
fn glyphs(ui: &GalleryUi) -> Vec<[f32; 4]> {
    let p = lab::text::SPREAD as f32 + 0.5;
    ui.instances_of("film/c06/body").iter().map(|i| [i.rect[0] + p, i.rect[1] + p, i.rect[0] + i.rect[2] - p, i.rect[1] + i.rect[3] - p]).filter(|r| r[2] > r[0] && r[3] > r[1]).collect()
}

#[test]
fn the_paragraph_never_overlaps_itself_or_the_sign() {
    let mut ui = open();
    let mut t = 3.0;
    for i in 0..20 {
        let (x, y) = (-60.0 + (i % 5) as f32 * 120.0, -40.0 + (i / 5) as f32 * 110.0);
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(x, y)));
        ui.relayout();
        for _ in 0..30 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let g = glyphs(&ui);
        assert!(g.len() > 100, "position {i}: {} glyphs", g.len());
        for a in 0..g.len() { for b in a + 1..g.len() {
            let (p, q) = (g[a], g[b]);
            let ox = p[2].min(q[2]) - p[0].max(q[0]);
            let oy = p[3].min(q[3]) - p[1].max(q[1]);
            assert!(ox <= 1.0 || oy <= 1.0, "position {i} ({x}, {y}): glyphs {a} {p:?} and {b} {q:?} overlap");
        } }
        let s = ui.node("film/c06/sign").expect("sign").rect;
        let (cx, cy, r) = (s.x + s.w / film::SIGN_W * 0.5, s.y + s.w / film::SIGN_W * 0.5, s.w / film::SIGN_W * 0.5);
        for (j, q) in g.iter().enumerate() {
            let (gx, gy) = ((q[0] + q[2]) * 0.5, (q[1] + q[3]) * 0.5);
            assert!(((gx - cx).powi(2) + (gy - cy).powi(2)).sqrt() > r, "position {i}: glyph {j} under the disc");
        }
    }
}

#[test]
fn a_subpixel_nudge_keeps_every_line_break() {
    let mut ui = open();
    let mut t = 3.0;
    let mut last: Option<Vec<[f32; 4]>> = None;
    for i in 0..12 {
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(200.0 + i as f32 * 0.05, 120.0 + i as f32 * 0.03)));
        ui.relayout();
        for _ in 0..20 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let g = glyphs(&ui);
        if let Some(l) = &last { assert_eq!(l.len(), g.len()); for (a, b) in l.iter().zip(&g) { assert!((a[1] - b[1]).abs() < 0.5 && (a[0] - b[0]).abs() < 2.0, "a glyph hopped at nudge {i}: {a:?} → {b:?}"); } }
        last = Some(g);
    }
}
