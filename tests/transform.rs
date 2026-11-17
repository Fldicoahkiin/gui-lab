//! Acceptance for transforms without a GPU: the morph's phases and hand-over times, reversal from where
//! the tiles are, flows between pages (interpolated rects, birth times kept, the rest dissolving), and
//! the gallery's two pages issuing their commands.
use amenui::el::{El, Paint, col, space, text};
use amenui::morph::{self, Cmd, Ease, FlowSpec, MorphSpec};
use amenui::ui::{Ui, ramp};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

// ── Morph ────────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Two { cmds: Vec<Cmd> }
fn two(_: &Two) -> El<()> {
    col().key("root").pad(20.0, 20.0).gap(40.0)
        .child(text("变形", 64.0, Paint::INK).key("a"))
        .child(space(160.0, 80.0).key("b").radius(12.0).fill(Paint::ACCENT).hidden_until_morphed(true))
}
fn two_ui() -> Ui<Two, ()> {
    let mut ui = Ui::new(Two::default(), two, |_, _| {}, fonts(), 600, 400, 1.0).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(10.0);
    ui
}
fn loose_of(ui: &Ui<Two, ()>, key: &str) -> Option<f32> { ui.instances_of(key).first().map(|i| i.extra[2]) }

#[test]
fn morph_hands_over_at_the_right_times() {
    let mut ui = two_ui();
    let spec = MorphSpec::default();
    assert!(loose_of(&ui, "b").is_none(), "the target is laid out but hidden");
    assert!(ui.node("b").is_some());
    ui.morph("a", "b", spec.clone());
    let run = ui.morph_run().expect("a run");
    assert!(!run.particles.is_empty());
    assert_eq!(run.tile, 5.0, "tiles are the lattice cells");
    let (l, d, t) = (spec.loosen_ms / 1000.0, spec.duration_ms / 1000.0, spec.tighten_ms / 1000.0);
    // Phase 1: the source loosens, the target is hidden, no tiles yet.
    ui.tick(10.0 + l * 0.5);
    let k = loose_of(&ui, "a").expect("source drawn");
    assert!((k - 0.5).abs() < 0.02, "half-way loose: {k}");
    assert!(loose_of(&ui, "b").is_none());
    assert!(ui.morph_run().unwrap().clock(ui.now()).is_none());
    ui.tick(10.0 + l - 0.001);
    assert!(loose_of(&ui, "a").expect("source drawn") > 0.99);
    // Phase 2: from the hand-over on, only the tiles.
    ui.tick(10.0 + l + 0.001);
    assert!(loose_of(&ui, "a").is_none() && loose_of(&ui, "b").is_none());
    let clock = ui.morph_run().unwrap().clock(ui.now()).expect("in flight");
    assert!(clock < 0.01);
    // Every tile is at its source cell when the flight starts and at its target cell when it ends.
    let run = ui.morph_run().unwrap();
    for p in &run.particles {
        assert_eq!(morph::progress(p, 0.0, spec.ease), 0.0);
        assert_eq!(morph::progress(p, d - 0.001, spec.ease), 1.0, "landed before the flight is over: {p:?}");
    }
    // Phase 3: the target, fully loose, tightening; it does not condense on top of that.
    ui.tick(10.0 + l + d + 0.001);
    let b = ui.instances_of("b");
    assert!(!b.is_empty() && loose_of(&ui, "a").is_none());
    assert!(b[0].extra[2] > 0.99, "{}", b[0].extra[2]);
    assert!(b[0].extra[3] < 0.0, "fully born");
    ui.tick(10.0 + l + d + t * 0.5);
    assert!((loose_of(&ui, "b").unwrap() - 0.5).abs() < 0.02);
    // Over: the view decides again (here the source is not hidden, the target is).
    ui.tick(10.0 + l + d + t + 0.01);
    assert!(ui.morph_run().is_none());
    assert_eq!(loose_of(&ui, "a"), Some(0.0));
}

#[test]
fn reversal_turns_the_tiles_around_where_they_are() {
    let mut ui = two_ui();
    let spec = MorphSpec { ease: Ease::OutExpo, ..MorphSpec::default() };
    ui.morph("a", "b", spec.clone());
    let t1 = 10.0 + spec.loosen_ms / 1000.0;
    ui.tick(t1 + 0.4);
    let clock = ui.morph_run().unwrap().clock(ui.now()).expect("in flight");
    let before: Vec<f32> = ui.morph_run().unwrap().particles.iter().map(|p| morph::progress(p, clock, spec.ease)).collect();
    assert!(before.iter().any(|&u| u > 0.2 && u < 1.0), "mid-flight");
    // The reverse of the morph in progress, as `update` would ask for it.
    ui.state.cmds.push(Cmd::Morph { from: "b".into(), to: "a".into(), spec: spec.clone() });
    ui.relayout();
    let run = ui.morph_run().expect("still running");
    assert_eq!((run.from.as_str(), run.to.as_str()), ("b", "a"));
    let after: Vec<f32> = run.particles.iter().map(|p| morph::progress(p, 0.0, spec.ease)).collect();
    assert!(before == after, "the reverse starts from the current positions");
    assert!(run.particles.iter().all(|p| p.run[1] == 0.0));
    // It ends with the source back, tightening.
    let end = run.end;
    ui.tick(run.dst_from + 0.001);
    assert!(loose_of(&ui, "a").unwrap() > 0.99 && loose_of(&ui, "b").is_none());
    ui.tick(end + 0.01);
    assert!(ui.morph_run().is_none());
}

#[test]
fn reversal_while_loosening_tightens_again() {
    let mut ui = two_ui();
    let spec = MorphSpec::default();
    ui.morph("a", "b", spec.clone());
    ui.tick(10.0 + 0.11);
    let k = loose_of(&ui, "a").unwrap();
    ui.morph("b", "a", spec.clone());
    let run = ui.morph_run().unwrap();
    assert!(run.flight.is_none());
    assert!((ramp(run.src_ramp, ui.now()) - k).abs() < 1e-4, "from where it was");
    ui.tick(10.0 + 0.11 + spec.tighten_ms / 1000.0 * k * 0.5);
    assert!(loose_of(&ui, "a").unwrap() < k);
    ui.tick(11.5);
    assert!(ui.morph_run().is_none());
    assert_eq!(loose_of(&ui, "a"), Some(0.0));
}

// ── Flow ─────────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Pages { second: bool, cmds: Vec<Cmd> }
fn pages(s: &Pages) -> El<bool> {
    let root = col().key("root").pad(20.0, 20.0).gap(20.0);
    if !s.second {
        root.child(text("标题", 16.0, Paint::INK).key("p1/title").flow("t"))
            .child(space(80.0, 40.0).key("p1/box").fill(Paint::INK).flow("box"))
            .child(text("只在第一页", 16.0, Paint::TEXT).key("p1/only"))
    } else {
        root.child(space(1.0, 200.0))
            .child(space(240.0, 120.0).key("p2/box").fill(Paint::INK).flow("box"))
            .child(text("标题", 32.0, Paint::INK).key("p2/title").flow("t"))
    }
}
fn go(s: &mut Pages, second: bool) { s.second = second; s.cmds.push(Cmd::Flow(FlowSpec::default())); }

#[test]
fn shared_parts_flow_between_pages() {
    let mut ui = Ui::new(Pages::default(), pages, go, fonts(), 800, 600, 1.0).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(5.0);
    let old_box = ui.instances_of("p1/box")[0];
    let old_title = ui.instances_of("p1/title");
    assert_eq!(old_title.len(), 2);
    let born = old_box.extra[3];
    // A page change, as a press would send it.
    ui.state.second = true;
    ui.state.cmds.push(Cmd::Flow(FlowSpec::default()));
    ui.relayout();
    let spec = FlowSpec::default();
    let target = {
        // Where layout puts it, once the flow is over.
        let r = ui.node("p2/box").unwrap().rect;
        [r.x, r.y, r.w, r.h]
    };
    let at = |ui: &Ui<Pages, bool>| ui.instances_of("p2/box")[0];
    // t = 0: still where it was, and it kept its birth time (no condensing again).
    let b = at(&ui);
    assert_eq!(b.rect, old_box.rect);
    assert_eq!(b.extra[3], born);
    assert_eq!(b.extra[2], 0.0);
    // Half-way: half-way there (in-out cubic is ½ at ½), loosened by the bump's peak.
    ui.tick(5.0 + spec.flow_ms / 1000.0 * 0.5);
    let b = at(&ui);
    for k in 0..4 { assert!((b.rect[k] - (old_box.rect[k] + target[k]) * 0.5).abs() < 0.01, "{k}: {:?} {:?}", b.rect, target); }
    assert!((b.extra[2] - spec.flow_loose).abs() < 1e-4);
    // The end: exactly the new rect, crisp.
    ui.tick(5.0 + spec.flow_ms / 1000.0 + 0.001);
    let b = at(&ui);
    assert_eq!(b.rect, target);
    assert_eq!(b.extra[2], 0.0);
    assert_eq!(ui.flowing(), 0);
    // The title's glyphs (same text, twice the size) flowed too, and kept their birth times.
    let t = ui.instances_of("p2/title");
    assert_eq!(t.len(), 2);
    assert!(t.iter().zip(&old_title).all(|(n, o)| n.extra[3] == o.extra[3]));
}

#[test]
fn unshared_parts_dissolve() {
    let mut ui = Ui::new(Pages::default(), pages, go, fonts(), 800, 600, 1.0).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(5.0);
    let only = ui.instances_of("p1/only").len();
    assert!(only >= 4);
    ui.state.second = true;
    ui.state.cmds.push(Cmd::Flow(FlowSpec::default()));
    ui.relayout();
    assert_eq!(ui.dissolving(), only, "every glyph of the text only on the first page");
    // They loosen and fade, then they are gone.
    ui.tick(5.09);
    let fading = ui.scene().instances.iter().filter(|i| i.extra[2] > 0.4 && i.paint[2] < 0.6).count();
    assert_eq!(fading, only);
    ui.tick(5.2);
    assert_eq!(ui.dissolving(), 0);
    // Without a flow command a page change just replaces (nothing dissolves).
    ui.state.second = false;
    ui.relayout();
    assert_eq!(ui.dissolving(), 0);
}


// ── Sampling follows transforms and poses (DESIGN.md §1.26) ────────────────────────────────────────

#[derive(Default)]
struct Posed { cmds: Vec<Cmd> }
/// A word whose glyphs are moved 120 px right and down and turned a quarter: its tiles must start
/// where it is drawn, not where the layout put it.
fn posed(_: &Posed) -> El<()> {
    col().key("root").pad(20.0, 20.0).gap(40.0)
        .child(text("III", 48.0, Paint::INK).key("a").pose(|_| amenui::el::Pose { dx: 120.0, dy: 140.0, angle: std::f32::consts::FRAC_PI_2, ..Default::default() }))
        .child(space(80.0, 40.0).key("b").fill(Paint::ACCENT).hidden_until_morphed(true))
}

#[test]
fn a_posed_glyph_loosens_from_where_it_is_drawn() {
    let mut ui = Ui::new(Posed::default(), posed, |_, _| {}, fonts(), 600, 400, 1.0).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(10.0);
    let insts = ui.instances_of("a");
    assert_eq!(insts.len(), 3);
    // Every instance is turned (its own transform) and moved.
    assert!(insts.iter().all(|i| i.xf != [0.0; 4]));
    let laid = ui.node("a").unwrap().rect;
    let src = morph::cells(&insts, 4.0, ui.atlas());
    let inked: Vec<&morph::Cell> = src.iter().filter(|c| c.cover > 0.2).collect();
    assert!(!inked.is_empty());
    // The covered cells are near the posed place, not over the laid-out text.
    for c in &inked {
        assert!(c.x > laid.x + 60.0 && c.y > laid.y + 80.0, "a cell at ({}, {}) is where the layout put the text ({laid:?})", c.x, c.y);
    }
    // A vertical bar turned a quarter is a horizontal bar: the ink spans more across than down.
    let (x0, x1) = inked.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.x), a.1.max(c.x)));
    let (y0, y1) = inked.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.y), a.1.max(c.y)));
    assert!(x1 - x0 > 1.5 * (y1 - y0), "turned glyphs lie across: {}×{}", x1 - x0, y1 - y0);
    // The morph starts its tiles there too.
    ui.morph("a", "b", MorphSpec::default());
    let run = ui.morph_run().expect("a run");
    let n = run.particles.len() as f32;
    let cy = run.particles.iter().map(|p| p.ab[1]).sum::<f32>() / n;
    assert!(cy > laid.y + 80.0, "tiles start at the posed glyphs (mean y {cy}, laid out at {})", laid.y);
}

#[test]
fn a_turned_shape_covers_its_turned_box() {
    let i = amenui::scene::Instance { rect: [100.0, 100.0, 80.0, 10.0], paint: [1.0, 0.0, 1.0, amenui::scene::SHAPE], clip: [-1e5, -1e5, 1e5, 1e5], xf: amenui::scene::Instance::turned(std::f32::consts::FRAC_PI_2, 1.0, 1.0), ..Default::default() };
    // Centre (140, 105): turned, it runs from y 65 to 145 at x 140.
    assert!(morph::cover_at(&i, 140.0, 70.0, &[]) > 0.9);
    assert!(morph::cover_at(&i, 105.0, 105.0, &[]) < 0.01, "the untransformed ends are empty");
    // A fill in the ground's tone is an outline too; only `ink_at` leaves it out.
    let ground = amenui::scene::Instance { paint: [0.0, 0.0, 1.0, amenui::scene::SHAPE], xf: [0.0; 4], ..i };
    assert!(morph::cover_at(&ground, 140.0, 105.0, &[]) > 0.9);
    assert_eq!(morph::ink_at(&ground, 140.0, 105.0, &[]), 0.0);
}

/// Two pages, each with a title under the same key, in a different place and size; and a body each,
/// far below.
mod pages {
    use amenui::el::{El, Paint, col, text};
    use amenui::style::{Look, Material};
    pub struct S { pub second: bool }
    pub fn view(s: &S) -> El<()> {
        if !s.second {
            col().key("a").fill_width().fill_height()
                .child(text("Index", 18.0, Paint::INK).key("title").absolute(Some(40.0), Some(40.0), None, None))
                .child(text("the old page's body", 14.0, Paint::TEXT).key("a/body").absolute(Some(40.0), Some(300.0), None, None))
        } else {
            col().key("b").fill_width().fill_height()
                .child(text("Index", 40.0, Paint::INK).key("title").absolute(Some(220.0), Some(120.0), None, None))
                .child(text("the new page's body", 14.0, Paint::TEXT).key("b/body").absolute(Some(300.0), Some(320.0), None, None))
        }
    }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
}

/// The box of what is not the ground in the top part of the frame (where the titles are).
fn ink_box(px: &[u8], ground: &[u8], w: u32) -> Option<[f32; 4]> {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for (i, (p, g)) in px.chunks_exact(4).zip(ground.chunks_exact(4)).enumerate() {
        let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
        if y >= 260.0 || p.iter().zip(g).all(|(a, b)| (*a as i32 - *b as i32).abs() <= 24) { continue; }
        b = [b[0].min(x), b[1].min(y), b[2].max(x + 1.0), b[3].max(y + 1.0)];
    }
    (b[2] > b[0]).then_some(b)
}

#[test]
fn a_morph_from_the_last_frame_flies_the_old_title_to_the_new_one() {
    let (w, h) = (560u32, 380u32);
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    amenui::budget::set(amenui::budget::MAX_MARKS, 1.0);
    let spec = amenui::morph::MorphSpec { duration_ms: 600.0, tighten_ms: 200.0, ..Default::default() };
    let run = |o: &mut amenui::offscreen::Offscreen| {
        let mut ui = Ui::new(pages::S { second: false }, pages::view, |_, _| {}, fonts(), w, h, 1.0).on_look(pages::look);
        let mut t = 0.0;
        while t < 1.5 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
        let look = ui.look();
        let old = o.render(&mut ui, &look);
        ui.morph_from_last("title", spec.clone());
        ui.state.second = true;
        ui.relayout();
        let mut frames = vec![];
        for _ in 0..90 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); frames.push(o.render(&mut ui, &look)); }
        (old, frames, t)
    };
    let (old, frames, t) = run(&mut o);
    // The ground alone, and the new page drawn directly at the same time.
    let mut o2 = amenui::offscreen::Offscreen::new(w, h);
    let mut fresh = Ui::new(pages::S { second: true }, pages::view, |_, _| {}, fonts(), w, h, 1.0).on_look(pages::look);
    let mut tf = 0.0;
    while tf < t - 1e-4 { tf += 1.0 / 60.0; fresh.tick(tf); }
    let look = fresh.look();
    let direct = o2.render(&mut fresh, &look);
    let ground: Vec<u8> = { let g = &direct[..4]; g.iter().copied().cycle().take(direct.len()).collect() };
    let (a, b) = (ink_box(&old, &ground, w).expect("old title"), ink_box(&direct, &ground, w).expect("new title"));
    let first = ink_box(&frames[0], &ground, w).expect("first frame");
    let near = |p: [f32; 4], q: [f32; 4], tol: f32| (0..4).all(|k| (p[k] - q[k]).abs() <= tol);
    assert!(near(first, a, 3.0), "the first frame's title {first:?} is the old one {a:?}");
    // Each frame's ink centre stays near the line between the two centres.
    let c = |r: [f32; 4]| [(r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5];
    let (ca, cb) = (c(a), c(b));
    let len = ((cb[0] - ca[0]).powi(2) + (cb[1] - ca[1]).powi(2)).sqrt();
    for (k, f) in frames.iter().enumerate() {
        let Some(r) = ink_box(f, &ground, w) else { continue };
        let p = c(r);
        let d = ((cb[0] - ca[0]) * (ca[1] - p[1]) - (ca[0] - p[0]) * (cb[1] - ca[1])).abs() / len;
        assert!(d <= 0.1 * len, "frame {k}: the centre {p:?} is {d:.1} px off the line ({len:.0} px long)");
    }
    let last = frames.last().expect("frames");
    assert!(near(ink_box(last, &ground, w).expect("last"), b, 3.0), "the last frame's title is the new one");
    assert!(*last == direct, "done, it is the new page drawn directly");
    // The old page's body is not drawn after the first frame.
    let body = |px: &[u8]| px.chunks_exact(4).zip(direct.chunks_exact(4)).enumerate().filter(|(i, (p, d))| { let (x, y) = (*i as u32 % w, *i as u32 / w); x < 280 && (290..330).contains(&y) && p != d }).count();
    for (k, f) in frames.iter().enumerate().skip(1) { assert_eq!(body(f), 0, "frame {k} still shows the old body"); }
    // The same input three times: the same bytes.
    let mut o3 = amenui::offscreen::Offscreen::new(w, h);
    assert!(run(&mut o3).1 == frames, "the same frames again");
    let mut o4 = amenui::offscreen::Offscreen::new(w, h);
    assert!(run(&mut o4).1 == frames, "and a third time");
}
