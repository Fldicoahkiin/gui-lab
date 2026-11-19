//! A stage that is slow to draw (`El::stage`, DESIGN.md §1.35, the design's Y.5 row 3): with 40 ms added to
//! its draw, a 2 s drag of a slider beside it at 60 Hz answers in every frame that does not redraw the stage,
//! the stage redraws at most once per hold during the drag and on the first frame after the release, and a
//! stage that draws within the frame is drawn every frame.
use amenui::el::{El, Paint, col, row, space, text};
use amenui::style::{Look, Material};
use amenui::ui::Ui;
use amenui::widgets::{Spec, slider};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const SPEC: Spec = Spec { min: 0.0, max: 100.0, step: 0.1, decimals: 1, unit: "" };

struct S { v: f32, t: f32 }
#[derive(Clone)]
enum Msg { V(f32) }
fn view(s: &S) -> El<Msg> {
    // The piece: dots that move every frame.
    let mut piece = col().key("stage").stage().width(300.0).height(120.0).gap(2.0);
    for k in 0..12 {
        let x = ((s.t * 3.0 + k as f32 * 0.7).sin() * 0.5 + 0.5) * 280.0;
        piece = piece.child(row().child(space(x, 6.0)).child(space(6.0, 6.0).fill(Paint::TEXT)));
    }
    col().key("page").pad(20.0, 20.0).gap(12.0)
        .child(piece)
        .child(slider("level", s.v, SPEC, 240.0, Msg::V))
        .child(text(format!("{:.1}", s.v), 13.0, Paint::TEXT).key("value"))
}
fn open() -> Ui<S, Msg> {
    let mut u = Ui::new(S { v: 0.0, t: 0.0 }, view, |s, m| match m { Msg::V(v) => s.v = v }, fonts(), 400, 300, 1.0)
        .on_tick(|s, t| { s.t = t; true })
        .on_look(|_| Look { material: Material::Plain, ..Look::default() });
    u.tick(1.0);
    u.settle();
    u
}
/// The knob's drawn left edge, and the value's glyphs, as the scene has them.
fn shown(u: &Ui<S, Msg>) -> (f32, Vec<[f32; 8]>) {
    let nodes = u.nodes();
    let level = nodes.iter().position(|n| n.key == "level").expect("the slider");
    let knob = nodes.iter().enumerate().filter(|(_, n)| n.parent == Some(level)).nth(1).expect("the knob").0;
    let value = nodes.iter().position(|n| n.key == "value").expect("the value");
    let insts = &u.scene().instances;
    let of = |j: usize| u.owners().iter().enumerate().filter(move |(_, o)| o.0 == j).map(|(k, _)| { let i = &insts[k]; [i.rect[0], i.rect[1], i.rect[2], i.rect[3], i.uv[0], i.uv[1], i.uv[2], i.uv[3]] }).collect::<Vec<_>>();
    (of(knob).first().map_or(-1.0, |r| r[0]), of(value))
}
fn piece(u: &Ui<S, Msg>) -> Vec<[f32; 4]> {
    let nodes = u.nodes();
    let st = nodes.iter().position(|n| n.key == "stage").expect("the stage");
    let insts = &u.scene().instances;
    u.owners().iter().enumerate().filter(|(_, o)| { let mut p = Some(o.0); while let Some(q) = p { if q == st { return true; } p = nodes[q].parent; } false }).map(|(k, _)| insts[k].rect).collect()
}

#[test]
fn a_slow_stage_holds_while_the_hand_drags_and_the_slider_answers_every_frame() {
    let mut u = open();
    u.slow_stage(40.0);
    let r = u.node("level").expect("the slider").rect;
    // Frames at rest: the stage becomes slow.
    let mut t = u.now();
    for _ in 0..12 { t += 1.0 / 60.0; u.tick(t); }
    assert_eq!(u.stage_state("stage").map(|s| s.0), Some(true), "40 ms draws are slow");
    let (x0, y) = (r.x + 10.0, r.y + r.h * 0.5);
    u.press(x0, y);
    t += 1.0 / 60.0; u.tick(t);
    let mut before = shown(&u);
    let (mut late, mut redraws, mut last_redraw, mut held_frames) = (0, 0, t, 0);
    let mut gaps: Vec<f32> = vec![];
    for f in 1..=120 {
        let x = x0 + (r.w - 20.0) * f as f32 / 120.0;
        let drawn_before = u.stage_state("stage").map_or(0, |s| s.1);
        let w0 = std::time::Instant::now();
        u.pointer_move(x, y);
        t += 1.0 / 60.0;
        u.tick(t);
        let ms = w0.elapsed().as_secs_f32() * 1000.0;
        let redrew = u.stage_state("stage").map_or(0, |s| s.1) > drawn_before;
        let now = shown(&u);
        // Every frame shows the knob and the value where the hand put them.
        assert!(now.0 != before.0 || f > 116, "frame {f}: the knob did not move ({})", now.0);
        assert!(now.1 != before.1 || f > 116, "frame {f}: the value did not change");
        before = now;
        if redrew {
            redraws += 1;
            gaps.push(t - last_redraw);
            last_redraw = t;
        } else {
            held_frames += 1;
            if ms > 16.7 { late += 1; }
        }
    }
    eprintln!("stage redraws during a 2 s drag: {redraws} (gaps {gaps:.3?} s); frames that held: {held_frames}, of them over 16.7 ms: {late}");
    assert_eq!(late, 0, "a frame that did not redraw the stage was late");
    let hold = (6.0 * 0.040f32).clamp(0.1, 0.5);
    assert!(gaps.iter().skip(1).all(|g| *g >= hold - 1.0 / 60.0 - 1e-3), "two stage redraws closer than one hold ({hold} s): {gaps:?}");
    assert!(redraws >= 2 && redraws <= (2.0 / hold).ceil() as usize + 1, "{redraws} redraws in 2 s");
    // The release: the first frame after it draws the stage.
    let drawn = u.stage_state("stage").map_or(0, |s| s.1);
    let before = piece(&u);
    u.release(x0 + r.w - 20.0, y);
    t += 1.0 / 60.0; u.tick(t);
    assert!(u.stage_state("stage").map_or(0, |s| s.1) > drawn, "the frame after the release did not draw the stage");
    assert_ne!(piece(&u), before, "the stage shows the piece as it is now");
}

#[test]
fn a_stage_that_draws_in_time_is_drawn_every_frame_and_a_held_one_catches_up_at_rest() {
    let mut u = open();
    let r = u.node("level").expect("the slider").rect;
    let mut t = u.now();
    let (x0, y) = (r.x + 4.0, r.y + r.h * 0.5);
    u.press(x0, y);
    for f in 1..=20 {
        let drawn = u.stage_state("stage").map_or(0, |s| s.1);
        let before = piece(&u);
        u.pointer_move(x0 + f as f32 * 5.0, y);
        t += 1.0 / 60.0; u.tick(t);
        assert!(u.stage_state("stage").map_or(0, |s| s.1) > drawn, "frame {f}: a fast stage held");
        assert_ne!(piece(&u), before, "frame {f}: the piece did not move");
    }
    assert_eq!(u.stage_state("stage").map(|s| s.0), Some(false));
    // Slow, then input, then a frame without input: it draws again.
    u.slow_stage(40.0);
    for _ in 0..10 { t += 1.0 / 60.0; u.tick(t); }
    u.pointer_move(x0 + 120.0, y);
    t += 1.0 / 60.0; u.tick(t);
    let drawn = u.stage_state("stage").map_or(0, |s| s.1);
    assert!(u.needs_frame(t), "a held stage asks for a frame");
    t += 1.0 / 60.0; u.tick(t);
    assert!(u.stage_state("stage").map_or(0, |s| s.1) > drawn, "the first frame without input did not draw the stage");
}
