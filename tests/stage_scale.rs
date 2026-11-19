//! A stage's own device scale (DESIGN.md §1.35, the design's Y.5 row 12): the governor steps it down
//! when frames are missed and up after 5 s when the level up would fit, twice at most, and the renderer
//! shades only the stage at that scale; text, the panel and what is drawn over the stage stay at the
//! device's.
use amenui::el::{El, Paint, canvas, col, row, text};
use amenui::offscreen::Offscreen;
use amenui::perf::StageScale;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const VSYNC: f64 = 1000.0 / 60.0;

/// Frames on a 60 Hz display for `secs` s on a 2× device, a stage costing `at2` ms at scale 2 and by its
/// pixels below; `down` says when a pointer is down. The scale after each frame, with its time.
fn run(g: &mut StageScale, at2: f32, secs: f64, down: impl Fn(f64) -> bool) -> Vec<(f64, f32)> {
    run_by(g, |s, _| { let k = s / 2.0; at2 * k * k }, secs, down)
}
/// The same, a stage costing `cost(scale, t)` ms.
fn run_by(g: &mut StageScale, cost: impl Fn(f32, f64) -> f32, secs: f64, down: impl Fn(f64) -> bool) -> Vec<(f64, f32)> {
    let (mut t, mut out) = (0.0, vec![]);
    while t < secs * 1000.0 {
        let cost = cost(g.scale(), t);
        g.feed(t, cost, 2.0, down(t));
        out.push((t, g.scale()));
        t += (f64::from(cost) / VSYNC).ceil().max(1.0) * VSYNC;
    }
    out
}
fn changes(v: &[(f64, f32)]) -> Vec<(f64, f32)> { v.windows(2).filter(|w| w[0].1 != w[1].1).map(|w| (w[1].0, w[1].1)).collect() }

#[test]
fn a_stage_that_misses_at_2_and_fits_at_1_5_settles_at_1_5() {
    // 24 ms at 2 misses every frame; 13.5 ms at 1.5 fits (81 % of the frame, so it does not step back up).
    let mut g = StageScale::default();
    let v = run(&mut g, 24.0, 64.0, |_| false);
    let c = changes(&v);
    assert!(!c.is_empty() && c[0].1 == 1.5 && c[0].0 <= 4000.0, "settles at 1.5 within 4 s: {c:?}");
    let after: Vec<_> = c.iter().skip(1).filter(|(t, _)| *t <= c[0].0 + 60_000.0).collect();
    assert!(after.len() <= 3, "at most 3 more changes in 60 s: {after:?}");
    assert_eq!(v.last().map(|x| x.1), Some(1.5));
    assert!((g.frame_ms - VSYNC as f32).abs() < 0.5, "the display frame is measured: {}", g.frame_ms);
}

fn ups(c: &[(f64, f32)]) -> Vec<f64> { c.windows(2).filter(|w| w[1].1 > w[0].1).map(|w| w[1].0).collect() }

#[test]
fn a_level_up_that_misses_at_once_is_not_tried_again() {
    // 20 ms at 2 misses; at 1.5 7 ms, which by the pixels would fit at 2 (12.4 ms, under 85 % of the
    // frame), so it goes up after 5 s, misses, comes down within 5 s, and stays at 1.5.
    let mut g = StageScale::default();
    let c = changes(&run_by(&mut g, |s, _| if s > 1.9 { 20.0 } else { 7.0 }, 120.0, |_| false));
    assert_eq!(ups(&c).len(), 1, "one step up, then it holds: {c:?}");
    let up = ups(&c)[0];
    let down = c.iter().find(|x| x.0 > up).expect("down again");
    assert!(down.0 - up < 5000.0 && down.1 == 1.5, "down within 5 s of going up: {c:?}");
    assert!(g.held && c.last().map(|x| x.1) == Some(1.5));
    // A new route or window size starts from the device's scale again, and may step up again.
    g.reset();
    assert!(g.scale() == 2.0 && !g.held);
}

#[test]
fn a_stage_at_60_percent_at_half_scale_never_steps_up() {
    // Misses at 2 and at 1.5; at 1 (half the device's scale) 59 % of the frame, which at 1.5 would be
    // 2.25 times that.
    let mut g = StageScale::default();
    let c = changes(&run_by(&mut g, |s, _| if s > 1.9 { 30.0 } else if s > 1.4 { 20.0 } else { 0.59 * VSYNC as f32 }, 120.0, |_| false));
    assert_eq!(c.iter().map(|x| x.1).collect::<Vec<_>>(), vec![1.5, 1.0], "down twice, never up: {c:?}");
}

#[test]
fn a_stage_steps_up_twice_at_most_on_a_visit() {
    // Heavy for the first 5 s of every 20 (missing at 2), light the rest: each heavy spell takes it
    // down, long after it last went up, and each light one up again, until the second step up; the next
    // heavy spell takes it down for good.
    let mut g = StageScale::default();
    let c = changes(&run_by(&mut g, |s, t| if t % 20_000.0 < 5000.0 && s > 1.9 { 24.0 } else { 4.0 }, 120.0, |_| false));
    assert_eq!(ups(&c).len(), 2, "two steps up in 120 s: {c:?}");
    assert!(c.len() >= 3 && c.last().map(|x| x.1) == Some(1.5), "down for good after the second: {c:?}");
}

#[test]
fn it_never_steps_up_while_a_pointer_is_down_and_a_1x_device_has_one_step() {
    let mut g = StageScale::default();
    let c = changes(&run(&mut g, 17.5, 30.0, |t| t > 2000.0));
    assert_eq!(c.iter().map(|x| x.1).collect::<Vec<_>>(), vec![1.5], "down while held, never up: {c:?}");
    assert_eq!(StageScale::levels(1.0), vec![1.0, 0.75]);
    assert_eq!(StageScale::levels(2.0), vec![2.0, 1.5, 1.0, 0.75]);
}

mod page {
    use super::*;
    #[derive(Default)]
    pub struct S { pub q: f32 }
    pub fn view(_: &S) -> El<()> {
        let mut d = amenui::draw::Drawing::new(200.0, 150.0);
        for j in 0..14 { let y = 8.0 + j as f32 * 10.0; d.line((10.0, y), (190.0, y + 6.0 - (j % 3) as f32 * 4.0), 1.25, Paint::TEXT); }
        row().key("page").pad(10.0, 10.0).gap(16.0)
            .child(col().key("panel").width(120.0).gap(6.0).pad(8.0, 8.0).fill(Paint::FAINT)
                .child(text("Damping 0.25", 12.0, Paint::TEXT)).child(text("ω = 1.20", 12.0, Paint::DIM)))
            .child(col().key("stage").stage().width(200.0).height(150.0).child(canvas(d).key("d")))
            // Drawn over the stage, after it: a tip, which stays at the device's scale.
            .child(col().key("tip").absolute(Some(200.0), Some(30.0), None, None).pad(6.0, 4.0).fill(Paint::GROUND).border(1.0, Paint::DIM)
                .child(text("tip over the stage", 11.0, Paint::TEXT)))
    }
    pub fn look(s: &S) -> Look { Look { material: Material::Dottr, quality: s.q, ..Look::default() } }
}

#[test]
fn only_the_stage_is_shaded_at_its_scale() {
    let (w, h) = (380u32, 180u32);
    let Ok(mut o) = Offscreen::try_new(w * 2, h * 2) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, q: f32| {
        let mut ui = Ui::new(page::S { q }, page::view, |_, _| {}, fonts(), w * 2, h * 2, 2.0).on_look(page::look);
        ui.tick(3.0);
        let look = ui.look();
        let px = o.render(&mut ui, &look);
        let r = |k: &str| { let r = ui.node(k).expect(k).rect; [(r.x * 2.0) as u32, (r.y * 2.0) as u32, (r.right() * 2.0).ceil() as u32, (r.bottom() * 2.0).ceil() as u32] };
        (px, r("stage"), r("tip"), ui.perf().stage_label())
    };
    let (full, stage, tip, l1) = shot(&mut o, 1.0);
    let (low, _, _, l2) = shot(&mut o, 0.75);
    assert_eq!(l1, None, "at the device's scale the status line shows nothing");
    assert_eq!(l2.as_deref(), Some("stage 1.5x"));
    let inside = |b: [u32; 4], x: u32, y: u32| x >= b[0] && x < b[2] && y >= b[1] && y < b[3];
    let (mut outside_diff, mut tip_diff, mut stage_diff) = (0, 0, 0);
    for y in 0..h * 2 {
        for x in 0..w * 2 {
            let i = ((y * w * 2 + x) * 4) as usize;
            if full[i..i + 4] == low[i..i + 4] { continue; }
            if inside(tip, x, y) { tip_diff += 1; } else if inside(stage, x, y) { stage_diff += 1; } else { outside_diff += 1; }
        }
    }
    assert_eq!(outside_diff, 0, "text, the panel and the page outside the stage are as at the device's scale");
    assert_eq!(tip_diff, 0, "what is drawn over the stage is at the device's scale");
    assert!(stage_diff > 200, "the stage is shaded at 1.5: {stage_diff} pixels differ");
}

mod scrolling {
    use super::*;
    #[derive(Default)]
    pub struct S { pub q: f32 }
    pub fn view(_: &S) -> El<()> {
        let mut body = col().key("body").gap(8.0).pad(10.0, 10.0);
        for k in 0..30 { body = body.child(text(format!("Line {k} of the scrolled list"), 13.0, Paint::TEXT)); }
        col().key("page").pad(8.0, 8.0).gap(8.0)
            .child(text("Header outside the scroll area", 13.0, Paint::TEXT))
            .child(col().key("list").scroll().height(150.0).child(body))
    }
    pub fn look(s: &S) -> Look { Look { material: Material::Dottr, quality: s.q, ..Look::default() } }
}

/// Fable's ruling on scrolling under the automatic level (DESIGN.md §1.12): the scroll area is shaded
/// softened while it scrolls, the rest of the page and a fixed level never are, and 100 ms after the
/// last delta a frame is due and drawn sharp.
#[test]
fn a_scroll_area_softens_only_while_it_scrolls_under_the_automatic_level() {
    let (w, h) = (300u32, 220u32);
    if Offscreen::try_new(w, h).is_err() { eprintln!("no GPU adapter here: skipped"); return; }
    // A frame of the page, `wheel` scrolled at 3 s, drawn at `t`, with the automatic level at `level`;
    // `moves`: the frame may move the last one (`dirty::Move`), and then it is not softened.
    let shot_in = |q: f32, level: usize, t: f32, moves: bool| {
        let mut o = Offscreen::new(w, h);
        o.r.auto.level = level;
        o.r.partial = moves;
        let mut ui = Ui::new(scrolling::S { q }, scrolling::view, |_, _| {}, fonts(), w, h, 1.0).on_look(scrolling::look);
        ui.tick(2.9);
        let look = ui.look();
        o.render(&mut ui, &look);
        ui.wheel(150.0, 150.0, 40.0);
        ui.tick(3.0);
        let soft_now = ui.scroll_soft();
        if t > 3.0 { o.render(&mut ui, &look); assert!(ui.needs_frame(t), "a sharp frame is due once it stops"); ui.tick(t); }
        let px = o.render(&mut ui, &look);
        let r = ui.node("list").expect("list").rect;
        (px, [r.x as u32, r.y as u32, r.right().ceil() as u32, r.bottom().ceil() as u32], soft_now, ui.scroll_soft())
    };
    let shot = |q: f32, level: usize, t: f32| shot_in(q, level, t, false);
    let (sharp, list, moving, _) = shot(0.0, 0, 3.0);
    assert!(moving, "the wheel moved the list");
    let (soft, ..) = shot(0.0, 2, 3.0);
    let (fixed, ..) = shot(0.5, 0, 3.0);
    let (stopped, _, _, still) = shot(0.0, 2, 3.2);
    let (stopped_sharp, ..) = shot(0.0, 0, 3.2);
    assert!(!still);
    let inside = |x: u32, y: u32| x >= list[0] && x < list[2] && y >= list[1] && y < list[3];
    let (mut in_diff, mut out_diff) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if sharp[i..i + 4] != soft[i..i + 4] { if inside(x, y) { in_diff += 1; } else { out_diff += 1; } }
        }
    }
    assert_eq!(out_diff, 0, "outside the scroll area nothing softens");
    assert!(in_diff > 50, "the scroll area is shaded softened (the dottr ground between the lines): {in_diff} pixels differ");
    assert!(sharp == fixed, "a level the look fixes never softens scrolling");
    assert!(stopped == stopped_sharp, "the frame after it stops is sharp");
    // When the frame can move the last one, it does, sharp: softening stands down.
    let (moved, ..) = shot_in(0.0, 2, 3.0, true);
    assert!(moved == sharp, "a frame that moves the last one is not softened");
}

/// The governor steps the stages' scale (down, and back up): the frame shades the stages again and
/// nothing else, and it is the frame drawn whole at that scale, byte for byte.
#[test]
fn a_new_stage_scale_shades_only_the_stages_again() {
    let (w, h) = (380u32, 180u32);
    let (Ok(mut part), Ok(mut whole)) = (Offscreen::try_new(w * 2, h * 2), Offscreen::try_new(w * 2, h * 2)) else { eprintln!("no GPU adapter here: skipped"); return };
    whole.r.partial = false;
    for o in [&mut part, &mut whole] { o.r.govern = true; }
    let mk = || { let mut ui = Ui::new(page::S { q: 1.0 }, page::view, |_, _| {}, fonts(), w * 2, h * 2, 2.0).on_look(page::look); ui.tick(3.0); ui };
    let (mut u, mut v) = (mk(), mk());
    let look = u.look();
    let mut t = 3.0;
    let mut frame = |u: &mut Ui<page::S, ()>, v: &mut Ui<page::S, ()>, part: &mut Offscreen, whole: &mut Offscreen| {
        t += 1.0 / 60.0;
        u.tick(t);
        v.tick(t);
        let (a, b) = (part.render(u, &look), whole.render(v, &look));
        a.iter().zip(&b).filter(|(p, q)| p != q).count()
    };
    for _ in 0..3 { assert_eq!(frame(&mut u, &mut v, &mut part, &mut whole), 0); }
    for level in [1usize, 2, 1, 0] {
        for ui in [&mut u, &mut v] { ui.stage_governor().set_level(level); }
        let diff = frame(&mut u, &mut v, &mut part, &mut whole);
        let path = u.perf().path;
        assert_eq!(diff, 0, "level {level}: {diff} bytes differ from the frame drawn whole ({})", path.line());
        assert_eq!(path.stage < 1.0, level > 0, "level {level}: the stages' scale ({})", path.line());
        let stage = u.node("stage").expect("stage").rect;
        let share = (stage.w + 4.0) * (stage.h + 4.0) / (w * h) as f32;
        assert!(path.shade > 0.0 && path.shade <= share + 0.02, "level {level}: the stage alone is shaded again: {} of the frame, the stage {share} ({})", path.shade, path.line());
        assert_eq!(frame(&mut u, &mut v, &mut part, &mut whole), 0, "level {level}, the frame after");
        assert_eq!(u.perf().path.shade, 0.0, "level {level}: the frame after shades nothing ({})", u.perf().path.line());
    }
}
