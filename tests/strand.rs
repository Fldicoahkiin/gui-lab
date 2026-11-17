//! scribblr's thread (DESIGN.md §1.30) through the runtime at 60 Hz: nothing at rest and no frames; a
//! floating window dragged past a decorative title writes it out as a thread whose tail runs with the
//! map's flow; text to read has no thread and holds (its fill, its place); it all goes back to nothing
//! when the map settles; the budget keeps it one line; recomposing and morphing elements have no
//! thread; a beat holds the pose for a step; 回 is four outlines (two rings, each an outer and an inner
//! edge) and three bridges; a 64 px title loosened is a few long chords well off its outline; the Loose
//! Thread page loosens its title all the way, holds it, and lets it go, with its tangle at rest.
use amenui::el::{El, Paint, Response, col, layers, row, space, text};
use amenui::strand::Kind;
use amenui::style::{Look, Material};
use amenui::ui::Ui;
use amenui::widgets::{FloatMsg, Floating, floating};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone)] enum M { F(FloatMsg) }
struct S { f: Floating, t: f32, hold: f32, word: String, cmds: Vec<amenui::morph::Cmd> }
fn view(s: &S) -> El<M> {
    let page = col().key("page").pad(40.0, 40.0).gap(24.0)
        .child(text("Thread", 72.0, Paint::INK).key("title").legible(false).respond(Response::Loosen))
        .child(row().gap(20.0).child(text("To read beside it", 16.0, Paint::INK).key("legible").respond(Response::Loosen)).child(space(40.0, 40.0).fill(Paint::hue(0)).key("deco")))
        .child(text(s.word.clone(), 28.0, Paint::INK).key("word").legible(false).respond(Response::Loosen));
    let box_ = floating("float", &s.f, col().pad(10.0, 10.0).child(text("box", 12.0, Paint::INK)), M::F);
    layers([page, box_]).width(1000.0).height(600.0)
}
fn update(s: &mut S, m: M) { let M::F(f) = m; let t = s.t; s.f.update(f, t); }
fn look(s: &S) -> Look { Look { material: Material::Scribblr, beat: amenui::beat::Beat::of_hold(s.hold), ..Look::default() } }
fn open(hold: f32) -> Ui<S, M> {
    let mut f = Floating::default();
    (f.size, f.area) = ((140.0, 80.0), (1000.0, 600.0));
    f.place(0);
    let mut ui = Ui::new(S { f, t: 0.0, hold, word: "one".into(), cmds: vec![] }, view, update, fonts(), 1000, 600, 1.0)
        .on_look(look)
        .on_commands(|s| std::mem::take(&mut s.cmds))
        .on_tick(|s, t| { s.t = t; s.f.step(t) });
    for i in 1..=60 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
fn run(ui: &mut Ui<S, M>, secs: f32) -> usize {
    let (mut t, to, mut drawn) = (ui.now(), ui.now() + secs, 0);
    while t < to { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; } ui.frame_drawn(); }
    drawn
}
fn thread_of<'a>(ui: &'a Ui<S, M>, key: &str) -> Option<&'a (String, f32, Vec<amenui::strand::Seg>)> { ui.strands().iter().find(|s| s.0 == key) }

#[test]
fn nothing_at_rest_a_thread_behind_the_window_and_nothing_after() {
    let mut ui = open(0.0);
    assert!(ui.strands().is_empty(), "threads at rest");
    assert_eq!(run(&mut ui, 1.0), 0, "frames at rest");
    // Drag the window across the title, slowly enough to stay over it.
    let (x, y) = ui.node("float").expect("box").rect.center();
    let mut t = ui.now();
    ui.press(x, y);
    let (mut tail_ok, mut tails, mut held, mut trailed) = (0, 0, true, false);
    // Text to read as it is at rest: its instances' fill and place.
    let legible = |ui: &Ui<S, M>| -> Vec<([f32; 4], f32, f32)> { ui.scene().instances.iter().enumerate().filter(|(i, _)| ui.owner_key(*i) == Some("legible")).map(|(_, x)| (x.rect, x.paint[2], x.look[3])).collect() };
    let at_rest = legible(&ui);
    assert!(!at_rest.is_empty());
    for i in 1..=70 {
        t += 1.0 / 60.0;
        ui.pointer_move(x + i as f32 * 6.0, y + 20.0 + i as f32 * 0.5);
        ui.tick(t);
        ui.frame_drawn();
        held &= thread_of(&ui, "legible").is_none() && legible(&ui) == at_rest && at_rest.iter().all(|x| x.2 > 0.5);
        if let Some((_, k, segs)) = thread_of(&ui, "title") {
            assert!(*k > 0.0);
            // One line.
            for (j, w) in segs.windows(2).enumerate() { assert_eq!(w[0].b, w[1].a, "the title's thread is broken at frame {i}, piece {j} of {}: {:?} {:?}", segs.len(), w[0], w[1]); }
            // The tail out of the end (the last run of tail pieces).
            let tail: Vec<_> = segs.iter().rev().take_while(|s| s.kind == Kind::Tail).collect::<Vec<_>>().into_iter().rev().collect();
            // Counted where the map has a flow at the tail's root and the window is not in its way.
            if let (Some(a), Some(b)) = (tail.first(), tail.last()) {
                trailed = true;
                let d = [b.b[0] - a.a[0], b.b[1] - a.a[1]];
                let f = ui.fieldmap().flow_at(a.a[0], a.a[1]);
                // Where the window is not in the flow's way (a tail stops before it).
                let fl = (f[0] * f[0] + f[1] * f[1]).sqrt();
                let w = ui.node("float").expect("box").rect;
                let blocked = (1..=15).any(|j| { let (px, py) = (a.a[0] + f[0] / fl.max(1e-6) * 10.0 * j as f32, a.a[1] + f[1] / fl.max(1e-6) * 10.0 * j as f32); px >= w.x && py >= w.y && px <= w.right() && py <= w.bottom() });
                if fl > 0.05 && !blocked { tails += 1; if d[0] * f[0] + d[1] * f[1] > 0.0 { tail_ok += 1; } }
            }
        }
    }
    ui.release(x + 420.0, y + 55.0);
    assert!(trailed && tails > 0, "the title never trailed off along a flow");
    assert!(tail_ok * 2 >= tails, "the tail ran with the flow in only {tail_ok} of {tails} frames");
    assert!(held, "text to read had a thread, faded, or was carried by the map");
    // Let go: it glides to a corner, then the map settles and the threads are gone.
    let mut n = 0;
    while ui.state.f.moving() && n < 300 { run(&mut ui, 1.0 / 60.0); n += 1; }
    run(&mut ui, 1.6);
    assert!(ui.strands().is_empty(), "{} threads left", ui.strands().len());
    let t = ui.now();
    assert!(!ui.needs_frame(t), "frames after the threads were gone");
}

#[test]
fn a_small_budget_keeps_one_line_and_recomposing_text_has_none() {
    let mut ui = open(0.0);
    amenui::budget::set(amenui::budget::MAX_MARKS, 0.25);
    stir(&mut ui);
    let t = ui.now() + 1.0 / 60.0;
    ui.tick(t);
    let (_, _, segs) = thread_of(&ui, "title").expect("a thread");
    assert!(!segs.is_empty());
    for w in segs.windows(2) { assert_eq!(w[0].b, w[1].a, "broken under a small budget"); }
    assert!(amenui::budget::usage().max as f32 * 0.25 >= segs.len() as f32);
    amenui::budget::set(amenui::budget::MAX_MARKS, 1.0);
    // Change the keyed word: while it recomposes, it has no thread (before, it had one).
    assert!(thread_of(&ui, "word").is_some(), "the word was not loose");
    ui.state.word = "two".into();
    ui.relayout();
    let t = ui.now() + 1.0 / 60.0;
    ui.tick(t);
    assert!(thread_of(&ui, "word").is_none(), "recomposing text drew a thread");
}

#[test]
fn morphing_elements_have_no_thread() {
    let mut ui = open(0.0);
    stir(&mut ui);
    let t = ui.now() + 1.0 / 60.0;
    ui.tick(t);
    assert!(thread_of(&ui, "title").is_some() && thread_of(&ui, "deco").is_some(), "the title and the square were not loose");
    // A morph from the title to the square: while it runs, neither end has a thread (they loosen as plain).
    ui.state.cmds.push(amenui::morph::Cmd::Morph { from: "title".into(), to: "deco".into(), spec: amenui::morph::MorphSpec::default() });
    ui.relayout();
    for _ in 0..10 {
        let t = ui.now() + 1.0 / 60.0;
        ui.tick(t);
        assert!(thread_of(&ui, "title").is_none() && thread_of(&ui, "deco").is_none(), "a morphing element drew a thread");
    }
}

#[test]
fn the_beat_holds_the_pose_for_a_step() {
    // Frames 1/120 s apart while the map moves; the pose follows the material clock.
    for hold in [83.0f32, 0.0] {
        let mut ui = open(hold);
        stir(&mut ui);
        let mut t = ui.now();
        let mut seen: Vec<(i64, Vec<amenui::strand::Seg>)> = vec![];
        for _ in 0..30 {
            t += 1.0 / 120.0;
            ui.tick(t);
            let step = amenui::beat::Beat::of_hold(hold).step_s() * 1000.0;
            let beat = if hold > 0.0 { (ui.material_clock() * 1000.0 / step).floor() as i64 } else { (ui.material_clock() * 1e6) as i64 };
            seen.push((beat, thread_of(&ui, "title").expect("a thread").2.clone()));
        }
        let (mut same, mut across) = (0, 0);
        for w in seen.windows(2) {
            if w[0].0 == w[1].0 { assert_eq!(w[0].1, w[1].1, "hold {hold}: two frames of one beat differ"); same += 1; }
            else { assert_ne!(w[0].1, w[1].1, "hold {hold}: the next beat kept the pose"); across += 1; }
        }
        if hold > 0.0 { assert!(same > 0 && across > 0, "{same} within, {across} across"); } else { assert_eq!(same, 0); }
    }
}

/// Stir the title hard through the map, so it is loose without a window.
fn stir(ui: &mut Ui<S, M>) {
    ui.state.cmds.push(amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Key("title".into()), v: [1.0, 0.2], energy: 1.0, radius: 240.0, scope: amenui::el::Scope::Global });
    ui.relayout();
}

#[test]
fn hui_is_four_outlines_and_three_bridges() {
    // Loose enough for the bridges, not yet for chords across them.
    fn v(_: &()) -> El<()> { col().pad(40.0, 40.0).child(text("回", 96.0, Paint::INK).key("hui").legible(false).loose(0.25)) }
    let mut ui = Ui::new((), v, |_, _| {}, fonts(), 400, 300, 1.0).on_look(|_| Look { material: Material::Scribblr, ..Look::default() });
    ui.tick(5.0);
    let (_, _, segs) = ui.strands().iter().find(|s| s.0 == "hui").expect("a thread");
    // Runs of bridge pieces: 回 is two rings, each with an outer and an inner edge, so four outlines
    // joined by three bridges, in one line.
    let runs = segs.windows(2).filter(|w| w[0].kind != Kind::Bridge && w[1].kind == Kind::Bridge).count() + (segs[0].kind == Kind::Bridge) as usize;
    assert_eq!(runs, 3, "{runs} bridges");
    for w in segs.windows(2) { assert_eq!(w[0].b, w[1].a); }
}

/// The thread of a 64 px "Loose thread" with no knots and no tails, as plain polylines (`angular` 1).
fn title_thread(k: f32) -> Vec<amenui::strand::Seg> { title_with(k, Some(1.0)) }
/// The same title's thread with no knots and no tails, at `angular` (`None`: the material's default).
fn title_with(k: f32, angular: Option<f32>) -> Vec<amenui::strand::Seg> {
    fn v(s: &(f32, Option<f32>)) -> El<()> { col().pad(60.0, 60.0).child(text("Loose thread", 64.0, Paint::INK).font(amenui::el::Font::Display).key("title").legible(false).loose(s.0)) }
    fn look(s: &(f32, Option<f32>)) -> Look {
        let mut l = Look { material: Material::Scribblr, ..Look::default() };
        let p = &mut l.params[Material::Scribblr.id() as usize].0;
        (p[3], p[5]) = (0.0, 0.0);
        if let Some(a) = s.1 { p[7] = a; }
        l
    }
    let mut ui = Ui::new((k, angular), v, |_, _| {}, fonts(), 900, 300, 1.0).on_look(look);
    ui.pointer_leave();
    ui.tick(5.0);
    ui.strands().iter().find(|s| s.0 == "title").map(|s| s.2.clone()).unwrap_or_default()
}

#[test]
fn a_loose_title_is_one_curving_line() {
    // At the default `angular`, fully loose: nine in ten neighbouring pieces of the line turn by 25° or less.
    let all = title_with(1.0, None);
    let turn = |a: &amenui::strand::Seg, b: &amenui::strand::Seg| {
        let (u, v) = ([a.b[0] - a.a[0], a.b[1] - a.a[1]], [b.b[0] - b.a[0], b.b[1] - b.a[1]]);
        let (lu, lv) = ((u[0] * u[0] + u[1] * u[1]).sqrt(), (v[0] * v[0] + v[1] * v[1]).sqrt());
        ((u[0] * v[0] + u[1] * v[1]) / (lu * lv).max(1e-9)).clamp(-1.0, 1.0).acos().to_degrees()
    };
    let pairs: Vec<f32> = all.windows(2).filter(|w| w[0].kind == Kind::Line && w[1].kind == Kind::Line).map(|w| turn(&w[0], &w[1])).collect();
    let gentle = pairs.iter().filter(|a| **a <= 25.0).count();
    eprintln!("the loose title: {gentle} of {} turns at most 25°", pairs.len());
    assert!(!pairs.is_empty() && gentle * 10 >= pairs.len() * 9, "{gentle} of {} turns at most 25°", pairs.len());
    // Below 0.3 the line is the letters' outline: the default draws it as `angular` 1 does.
    let (a, b) = (title_with(0.2, None), title_with(0.2, Some(1.0)));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) { for (p, q) in [(x.a, y.a), (x.b, y.b)] { assert!(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() < 0.5, "{p:?} against {q:?}"); } }
}

/// The sample page (`amenui::sample`) under scribblr, its decoration held at `k` and stirred through the
/// title, half a second on at 60 Hz.
fn sample_at(k: f32) -> Ui<amenui::sample::Sample, ()> {
    let look = Look { material: Material::Scribblr, ..Look::default() };
    let cmds = vec![amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Key("title".into()), v: [1.0, 0.15], energy: 1.0, radius: 200.0, scope: amenui::el::Scope::Global }];
    let (w, h) = amenui::sample::CELL;
    let mut ui = Ui::new(amenui::sample::Sample { k, label: "scribblr".into(), look, cmds }, amenui::sample::view, |_, _| {}, fonts(), w, h, 1.0)
        .on_look(|s| s.look).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.relayout();
    let mut t = 0.0;
    while t < 0.5 { t += 1.0 / 60.0; ui.tick(t); }
    ui
}

#[test]
fn no_piece_of_thread_enters_text_to_read() {
    for k in [1.0f32, 0.7, 0.4] {
        let ui = sample_at(k);
        let tree = ui.export_tree();
        // Each legible element's ink: its instances' boxes (a glyph's without the spread around it), 4 px larger.
        let spread = amenui::text::SPREAD as f32;
        let mut ink: Vec<(String, [f32; 4])> = vec![];
        for (i, inst) in ui.scene().instances.iter().enumerate() {
            let Some(key) = ui.owner_key(i) else { continue };
            if key.is_empty() || !tree.find(key).is_some_and(|n| n.legible) || inst.paint[2] <= 0.0 { continue; }
            let p = if inst.paint[3] == amenui::scene::GLYPH { spread } else { 0.0 };
            let b = [inst.rect[0] + p, inst.rect[1] + p, inst.rect[0] + inst.rect[2] - p, inst.rect[1] + inst.rect[3] - p];
            match ink.iter_mut().find(|e| e.0 == key) {
                Some(e) => e.1 = [e.1[0].min(b[0]), e.1[1].min(b[1]), e.1[2].max(b[2]), e.1[3].max(b[3])],
                None => ink.push((key.to_owned(), b)),
            }
        }
        assert!(ink.iter().any(|e| e.0 == "body") && ink.iter().any(|e| e.0 == "label"), "{:?}", ink.iter().map(|e| &e.0).collect::<Vec<_>>());
        let boxes: Vec<(String, [f32; 4])> = ink.into_iter().map(|(k, b)| (k, [b[0] - 4.0, b[1] - 4.0, b[2] + 4.0, b[3] + 4.0])).collect();
        assert!(!ui.strands().is_empty(), "k {k}: no thread");
        for (key, _, segs) in ui.strands() {
            for s in segs {
                for (name, b) in &boxes {
                    // Liang–Barsky: does the piece pass through the box's inside?
                    let d = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
                    let (mut t0, mut t1) = (0.0f32, 1.0f32);
                    let mut hit = true;
                    for (p, q) in [(-d[0], s.a[0] - b[0]), (d[0], b[2] - s.a[0]), (-d[1], s.a[1] - b[1]), (d[1], b[3] - s.a[1])] {
                        if p == 0.0 { if q <= 0.0 { hit = false; } continue; }
                        let r = q / p;
                        if p < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
                    }
                    assert!(!(hit && t0 < t1), "k {k}: a {:?} piece of `{key}` enters `{name}`: {:?} → {:?}", s.kind, s.a, s.b);
                }
            }
        }
    }
}

#[test]
fn a_rule_comes_apart_without_knots_or_tails() {
    let ui = sample_at(1.0);
    let (_, _, rule) = ui.strands().iter().find(|s| s.0 == "rule").expect("the rule has a thread");
    assert!(!rule.is_empty());
    assert!(rule.iter().all(|s| s.kind != Kind::Knot && s.kind != Kind::Tail), "the rule has a knot or a tail");
    let (_, _, title) = ui.strands().iter().find(|s| s.0 == "title").expect("the title has a thread");
    assert!(title.iter().any(|s| s.kind == Kind::Knot), "the title has no knot");
}

#[test]
fn a_loosened_title_is_a_few_long_chords_off_its_outline() {
    let rest = title_thread(0.001);
    let all = title_thread(1.0);
    assert!(!rest.is_empty() && !all.is_empty());
    let reach = amenui::strand::StrandFx::default().reach;
    assert!(all.len() * 100 <= (rest.len() + 1) * 15, "{} pieces against {} rest vertices", all.len(), rest.len() + 1);
    let mut lens: Vec<f32> = all.iter().map(|s| ((s.b[0] - s.a[0]).powi(2) + (s.b[1] - s.a[1]).powi(2)).sqrt()).collect();
    lens.sort_by(f32::total_cmp);
    assert!(lens[lens.len() / 2] >= 24.0, "median piece {}", lens[lens.len() / 2]);
    // Its ink, a pixel apart along every piece: at most a quarter within 2 px of the outline at rest.
    let off = |p: [f32; 2]| rest.iter().map(|g| {
        let (ex, ey) = (g.b[0] - g.a[0], g.b[1] - g.a[1]);
        let t = (((p[0] - g.a[0]) * ex + (p[1] - g.a[1]) * ey) / (ex * ex + ey * ey).max(1e-9)).clamp(0.0, 1.0);
        ((g.a[0] + ex * t - p[0]).powi(2) + (g.a[1] + ey * t - p[1]).powi(2)).sqrt()
    }).fold(f32::INFINITY, f32::min);
    let (mut near, mut total) = (0, 0);
    for s in &all {
        let l = ((s.b[0] - s.a[0]).powi(2) + (s.b[1] - s.a[1]).powi(2)).sqrt();
        for j in 0..l.ceil() as usize { let f = j as f32 / l.max(1.0); total += 1; if off([s.a[0] + (s.b[0] - s.a[0]) * f, s.a[1] + (s.b[1] - s.a[1]) * f]) <= 2.0 { near += 1; } }
    }
    assert!(near * 4 <= total, "{near} of {total} px within 2 px of the outline");
    // It reaches past the letters on every side by at least 0.6 × reach.
    let bbox = |s: &[amenui::strand::Seg]| s.iter().flat_map(|g| [g.a, g.b]).fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let (a, b) = (bbox(&rest), bbox(&all));
    let grow = [a[0] - b[0], a[1] - b[1], b[2] - a[2], b[3] - a[3]];
    assert!(grow.iter().all(|g| *g >= 0.6 * reach), "it reaches past the letters by {grow:?}");
}

#[test]
fn a_retracting_title_hurries_past_its_half_drawn_looks() {
    let mut ui = open(0.0);
    stir(&mut ui);
    let mut t = ui.now();
    let (mut band, mut peak) = (0, 0.0f32);
    for _ in 0..600 {
        t += 1.0 / 60.0;
        ui.tick(t);
        ui.frame_drawn();
        let k = thread_of(&ui, "title").map_or(0.0, |s| s.1);
        peak = peak.max(k);
        // Counted on the way back, once it was nearly all the way loose.
        if peak > 0.9 && (0.15..=0.4).contains(&k) { band += 1; }
    }
    assert!(peak > 0.9, "the stir loosened the title to {peak}");
    assert!(band <= 8, "{band} frames between 0.15 and 0.4 on the way back");
}

/// The Loose Thread composition as the site shows it (a kicker, the title, a picture with its caption,
/// a paragraph beside it, a row of small controls along the foot, a tangle behind the picture's corner),
/// in the `loose-thread` preset, with a point under the pointer that stirs the map.
mod loose_page {
    use amenui::el::{El, Font, Paint, Response, Scope, Stirr, col, picture, row, space, text, text_lh};
    pub struct P { pub at: Option<(f32, f32)> }
    #[derive(Clone)] pub struct At(pub Option<(f32, f32)>);
    pub fn view(s: &P) -> El<At> {
        let (w, pic_w, side_w, height) = (640.0f32, 300.0f32, 260.0f32, 480.0f32);
        let mut comp = col().key("comp").width(w + 72.0).height(height).gaze_scope().ripplr_scope().on_move(|p| Some(At(p)));
        comp.style.padding.right = taffy::prelude::length(72.0);
        let (cx, cy, r) = (pic_w + 20.0, 0.66 * height, 70.0);
        let pts: Vec<[f32; 2]> = amenui::strand::figure(17, r, cy - 4.0, height - cy - 34.0).iter().map(|p| [p[0] + 90.0, p[1] + cy]).collect();
        comp = comp.child(space(180.0, height).key("tangle").trace(amenui::strand::Trace { points: std::rc::Rc::new(pts), progress: 1.0, weight: 1.0, pull: 2.5, ..Default::default() }).absolute(Some(cx - 90.0), Some(0.0), None, None));
        if let Some((x, y)) = s.at { comp = comp.child(space(1.0, 1.0).key("stylus").stirr(Stirr { energy: 1.0, radius: Some(72.0), scope: Scope::Container(Some("comp".into())) }).absolute(Some(x), Some(y), None, None)); }
        let word = |k: &str, t: &str, on: bool| text(t, 12.0, if on { Paint::ACCENT } else { Paint::TEXT }).whisper().key(k);
        let footer = row().key("footer").width(w).gap(28.0).align(taffy::AlignItems::BASELINE).justify(taffy::JustifyContent::END).absolute(Some(0.0), Some(height - 22.0), None, None)
            .child(row().gap(8.0).child(word("black", "black", true)).child(word("dot", "·", false)).child(word("paper", "paper", false)))
            .child(word("binary", "1-bit", false)).child(word("invert", "invert", false));
        comp.child(text("amenui — no. 10", 11.0, Paint::TEXT).whisper().key("kicker"))
            .child(space(1.0, 22.0))
            .child(text_lh("loose thread", 30.0, 40.0, Paint::INK).font(Font::Whisper).tracking(0.3).key("title").respond(Response::Loosen).legible(false))
            .child(space(1.0, 34.0))
            .child(row().key("spread").width(w).gap(w - pic_w - side_w).align(taffy::AlignItems::START)
                .child(col().key("fig").gap(10.0).child(col().width(pic_w).child(picture(amenui::pictures::SPHERE, 0.5).key("pic"))).child(text("fig. 1", 11.0, Paint::TEXT).whisper().key("caption")))
                .child(col().key("side").width(side_w).pad(0.0, 6.0).child(text_lh("At rest this page is only type and a picture. Where the pointer passes, the title comes apart into one thread, then gathers again.", 14.0, 24.0, Paint::TEXT).key("body"))))
            .child(footer)
    }
    pub fn look() -> amenui::style::Look {
        let p = amenui::preset::presets().into_iter().find(|p| p.hash == "loose-thread").expect("the loose-thread preset");
        p.apply(&amenui::style::Look::default())
    }
}

/// Each legible element's ink (its instances' boxes, a glyph's without the spread), grown by `m`.
fn legible_boxes<S, M: Clone>(ui: &Ui<S, M>, m: f32) -> Vec<(String, [f32; 4])> {
    let tree = ui.export_tree();
    let spread = amenui::text::SPREAD as f32;
    let mut ink: Vec<(String, [f32; 4])> = vec![];
    for (i, inst) in ui.scene().instances.iter().enumerate() {
        let Some(key) = ui.owner_key(i) else { continue };
        if key.is_empty() || !tree.find(key).is_some_and(|n| n.legible) || inst.paint[2] <= 0.0 { continue; }
        let p = if inst.paint[3] == amenui::scene::GLYPH { spread } else { 0.0 };
        let b = [inst.rect[0] + p, inst.rect[1] + p, inst.rect[0] + inst.rect[2] - p, inst.rect[1] + inst.rect[3] - p];
        match ink.iter_mut().find(|e| e.0 == key) {
            Some(e) => e.1 = [e.1[0].min(b[0]), e.1[1].min(b[1]), e.1[2].max(b[2]), e.1[3].max(b[3])],
            None => ink.push((key.to_owned(), b)),
        }
    }
    ink.into_iter().map(|(k, b)| (k, [b[0] - m, b[1] - m, b[2] + m, b[3] + m])).collect()
}
/// The distance from a piece to a box (0 when it passes through it).
fn seg_box(a: [f32; 2], b: [f32; 2], r: &[f32; 4]) -> f32 {
    (0..=32).map(|j| { let t = j as f32 / 32.0; let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]; let (dx, dy) = ((r[0] - p[0]).max(p[0] - r[2]).max(0.0), (r[1] - p[1]).max(p[1] - r[3]).max(0.0)); (dx * dx + dy * dy).sqrt() }).fold(f32::INFINITY, f32::min)
}

#[test]
fn on_the_loose_thread_page_no_piece_enters_text_to_read_and_the_tails_keep_clear_of_it() {
    // The pointer through the title and down across the page, as a person would: every piece of every
    // thread (all four kinds) stays out of the ink of text to read (4 px larger), and no tail passes
    // within 12 px of it (a tail is a way out to the edge, not a line drawn past the text).
    let mut ui = Ui::new(loose_page::P { at: None }, loose_page::view, |s, m: loose_page::At| s.at = m.0, fonts(), 1000, 640, 1.0).on_look(|_| loose_page::look());
    let mut t = 0.0;
    let step = |ui: &mut Ui<loose_page::P, loose_page::At>, t: &mut f32| { *t += 1.0 / 60.0; ui.tick(*t); ui.frame_drawn(); };
    for _ in 0..90 { step(&mut ui, &mut t); }
    let comp = ui.node("comp").expect("comp").rect;
    let title = ui.node("title").expect("title").rect;
    let mut seen = 0;
    for i in 0..=40 {
        let k = (i.min(30)) as f32 / 30.0;
        ui.pointer_move(comp.x + 10.0 + (title.w + 40.0) * k, title.y + title.h * 0.5 + 220.0 * k * k);
        step(&mut ui, &mut t);
        if i % 5 != 0 { continue; }
        let (ink, near) = (legible_boxes(&ui, 4.0), legible_boxes(&ui, 12.0));
        assert!(ink.iter().any(|e| e.0 == "body") && ink.iter().any(|e| e.0 == "paper"), "{:?}", ink.iter().map(|e| &e.0).collect::<Vec<_>>());
        for (key, _, segs) in ui.strands() {
            for s in segs {
                seen += 1;
                for (name, b) in &ink { assert!(seg_box(s.a, s.b, b) > 0.0, "step {i}: a {:?} piece of `{key}` enters `{name}`: {:?} → {:?}", s.kind, s.a, s.b); }
                if s.kind == Kind::Tail { for (name, b) in &near { assert!(seg_box(s.a, s.b, b) > 0.0, "step {i}: a tail of `{key}` passes within 12 px of `{name}`: {:?} → {:?}", s.a, s.b); } }
                // The line goes round the picture, clear of it.
                let pic = ui.node("pic").expect("the picture").rect;
                let pb = [pic.x - 11.0, pic.y - 11.0, pic.right() + 11.0, pic.bottom() + 11.0];
                if key == "title" { assert!(seg_box(s.a, s.b, &pb) > 0.0, "step {i}: a {:?} piece of the title runs within 12 px of the picture: {:?} → {:?}", s.kind, s.a, s.b); }
            }
            // A tail's tip keeps 24 px from text to read; the line is one, piece after piece.
            let far = legible_boxes(&ui, 24.0);
            if let Some(last) = segs.last().filter(|s| s.kind == Kind::Tail) { for (name, b) in &far { assert!(seg_box(last.b, last.b, b) > 0.0, "step {i}: the tail of `{key}` ends within 24 px of `{name}` at {:?}", last.b); } }
            for w in segs.windows(2) { assert!(w[0].b == w[1].a, "step {i}: the line of `{key}` breaks between {:?} and {:?}", w[0].b, w[1].a); }
        }
    }
    assert!(seen > 100, "{seen} pieces: the title did not come apart");
}
