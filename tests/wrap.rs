//! Text flowing around the true outline of other things (`wrap`), without a GPU: the free pieces of a
//! band, and every inked texel of every glyph at least `gap - 1` px from the obstacle's ink.
use lab::el::{El, Paint, col, space, text_lh};
use lab::scene::GLYPH;
use lab::text::{ATLAS, SPREAD};
use lab::ui::Ui;
use lab::wrap::{Around, Silhouette};

fn font() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const TEXT: &str = "文字绕着东西的真实轮廓走，而不是绕着它的方框。每一行只拿到这一条带子里没有被挡住的部分；被挡成两段时，左右各排一行。Text keeps its distance from the ink of whatever is in its way, whatever shape that ink has, line by line, as the obstacle moves. 拖动它，文字就重新排。";

#[test]
fn free_pieces() {
    let a = Around { obstacles: vec![Silhouette::rounded(100.0, 0.0, 50.0, 40.0, 0.0)], gap: 10.0, min: 20.0 };
    assert_eq!(a.free(300.0, 0.0, 20.0), vec![(0.0, 90.0), (160.0, 300.0)]);
    // Below the obstacle (and its gap), the whole width.
    assert_eq!(a.free(300.0, 60.0, 80.0), vec![(0.0, 300.0)]);
    // Within the gap below it, still blocked.
    assert_eq!(a.free(300.0, 45.0, 60.0), vec![(0.0, 90.0), (160.0, 300.0)]);
    // A piece narrower than `min` is not used.
    let b = Around { obstacles: vec![Silhouette::rounded(15.0, 0.0, 50.0, 40.0, 0.0)], gap: 0.0, min: 20.0 };
    assert_eq!(b.free(300.0, 0.0, 20.0), vec![(65.0, 300.0)]);
}

#[test]
fn circle_rows() {
    let c = Silhouette::circle(50.0, 50.0, 20.0);
    let (a, b) = c.span(49.0, 51.0).expect("middle row");
    assert!((a - 30.0).abs() < 0.01 && (b - 70.0).abs() < 0.01);
    assert!(c.span(0.0, 29.0).is_none());
    assert!(c.covers(50.0, 50.0) && !c.covers(31.0, 33.0));
}

struct S { cx: f32, cy: f32, r: f32, gap: f32 }
fn page(s: &S) -> El<()> {
    let around = Around::new(vec![Silhouette::circle(s.cx, s.cy, s.r)], s.gap);
    col().key("root").pad(20.0, 20.0)
        .child(col().key("col").width(520.0)
            .child(space(s.r * 2.0, s.r * 2.0).radius(s.r).fill(Paint::ACCENT).key("disc").absolute(Some(s.cx - s.r), Some(s.cy - s.r), None, None))
            .child(text_lh(TEXT.repeat(3), 17.0, 28.0, Paint::TEXT).key("body").fill_width().around(around)))
}

/// The smallest distance from an inked glyph texel to the disc's ink, logical px; and how many lines
/// had text on both sides of the disc.
fn nearest(ui: &mut Ui<S, ()>) -> f32 {
    let body = ui.node("body").expect("body").inner;
    let disc = ui.node("disc").expect("disc").rect;
    let (cx, cy, r) = (disc.x + disc.w * 0.5, disc.y + disc.h * 0.5, disc.w * 0.5);
    assert!((body.x - 20.0).abs() < 0.01);
    let glyphs: Vec<_> = ui.instances_of("body").into_iter().filter(|i| i.paint[3] == GLYPH).collect();
    assert!(glyphs.len() > 200, "{} glyphs", glyphs.len());
    let px = &ui.text_mut().pixels;
    let mut best = f32::INFINITY;
    for g in glyphs {
        let (ax, ay) = ((g.uv[0] * ATLAS as f32).round() as i64, (g.uv[1] * ATLAS as f32).round() as i64);
        for ty in 0..g.rect[3] as i64 {
            for tx in 0..g.rect[2] as i64 {
                if px[(((ay + ty) * ATLAS as i64 + ax + tx) * 2) as usize] < 128 { continue; }
                let (x, y) = (g.rect[0] + tx as f32 + 0.5, g.rect[1] + ty as f32 + 0.5);
                best = best.min(((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r);
            }
        }
    }
    let _ = SPREAD;
    best
}

#[test]
fn glyphs_keep_the_gap() {
    for (cx, cy, gap) in [(260.0, 200.0, 14.0), (90.0, 140.0, 8.0), (430.0, 330.0, 20.0)] {
        let mut ui = Ui::new(S { cx, cy, r: 90.0, gap }, page, |_, _| {}, font(), 900, 1400, 1.0);
        let d = nearest(&mut ui);
        assert!(d >= gap - 1.0, "disc at {cx},{cy}: a glyph is {d:.1} px from the ink, gap {gap}");
        // Control: with no gap and the obstacle ignored, text runs over the disc.
        let mut ui = Ui::new(S { cx, cy, r: 90.0, gap: -1000.0 }, page, |_, _| {}, font(), 900, 1400, 1.0);
        assert!(nearest(&mut ui) < 0.0, "the control finds text over the disc");
    }
}

#[test]
fn text_on_both_sides_and_moves_with_it() {
    let mut ui = Ui::new(S { cx: 260.0, cy: 200.0, r: 80.0, gap: 12.0 }, page, |_, _| {}, font(), 900, 1400, 1.0);
    let glyphs = ui.instances_of("body");
    let (cx, cy) = (20.0 + 260.0, 20.0 + 200.0);
    let left = glyphs.iter().any(|g| (g.rect[1] - cy).abs() < 20.0 && g.rect[0] + g.rect[2] < cx - 80.0);
    let right = glyphs.iter().any(|g| (g.rect[1] - cy).abs() < 20.0 && g.rect[0] > cx + 80.0);
    assert!(left && right, "a band beside the disc has text on both sides");
    let h0 = ui.node("body").unwrap().rect.h;
    // Moving the obstacle moves the text: another layout.
    ui.state.cx = 120.0;
    ui.relayout();
    let moved = ui.instances_of("body");
    assert_ne!(glyphs.iter().map(|g| g.rect).collect::<Vec<_>>(), moved.iter().map(|g| g.rect).collect::<Vec<_>>());
    assert!(ui.node("body").unwrap().rect.h >= h0 - 28.0);
}
