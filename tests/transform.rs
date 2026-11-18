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
    /// The second page without a title: the ground the titles are measured against.
    pub fn bare(_: &S) -> El<()> {
        col().key("b").fill_width().fill_height()
            .child(text("the new page's body", 14.0, Paint::TEXT).key("b/body").absolute(Some(300.0), Some(320.0), None, None))
    }
    /// The first page without its title.
    pub fn bare_first(_: &S) -> El<()> {
        col().key("a").fill_width().fill_height()
            .child(text("the old page's body", 14.0, Paint::TEXT).key("a/body").absolute(Some(40.0), Some(300.0), None, None))
    }
    pub fn dotted(_: &S) -> Look { Look { material: Material::Dottr, ..Look::default() } }
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
/// The ink in the top part of the frame: how much of it there is (each pixel by how far it is from the
/// ground, 0–255 in its most different channel) and its centroid.
fn ink_mass(px: &[u8], ground: &[u8], w: u32) -> (f32, [f32; 2]) {
    let (mut m, mut x, mut y) = (0.0f32, 0.0f32, 0.0f32);
    for (i, (p, g)) in px.chunks_exact(4).zip(ground.chunks_exact(4)).enumerate() {
        let (px_, py) = ((i as u32 % w) as f32 + 0.5, (i as u32 / w) as f32 + 0.5);
        if py >= 260.0 { continue; }
        let d = p.iter().zip(g).take(3).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0) as f32;
        if d <= 8.0 { continue; }
        m += d; x += d * px_; y += d * py;
    }
    (m, if m > 0.0 { [x / m, y / m] } else { [0.0; 2] })
}

/// Two pages' titles, the old drawn, then a morph from the last frame and the new view: the frame
/// before, every frame after at 60 Hz for 1.5 s, the glyph instances drawn in the titles' part of the
/// window in each (their ink boxes, in the title's order; no glyph is drawn there but the title's), and
/// the time of the last frame.
type FromLast = (Vec<u8>, Vec<Vec<u8>>, Vec<Vec<[f32; 4]>>, f32);
fn from_last(o: &mut amenui::offscreen::Offscreen, look: fn(&pages::S) -> amenui::style::Look, reduced: bool) -> FromLast {
    let (w, h) = (560u32, 380u32);
    amenui::budget::set(amenui::budget::MAX_MARKS, 1.0);
    let spec = MorphSpec { duration_ms: 600.0, tighten_ms: 200.0, ..Default::default() };
    o.r.partial = false;
    let mut ui = Ui::new(pages::S { second: false }, pages::view, |_, _| {}, fonts(), w, h, 1.0).on_look(look);
    ui.set_reduced_motion(reduced);
    let mut t = 0.0;
    while t < 1.5 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    let old = o.render(&mut ui, &look);
    ui.morph_from_last("title", spec);
    ui.state.second = true;
    ui.relayout();
    let (mut frames, mut glyphs) = (vec![], vec![]);
    for _ in 0..90 {
        t += 1.0 / 60.0;
        ui.tick(t);
        let look = ui.look();
        frames.push(o.render(&mut ui, &look));
        // A glyph's box less its spread, at the scale it is drawn (its rect over its atlas cell).
        let sp = amenui::text::SPREAD as f32;
        let atlas = amenui::text::ATLAS as f32;
        let drawn = ui.scene().instances.iter().filter(|i| i.paint[3] == amenui::scene::GLYPH && i.paint[2] > 0.0 && i.rect[2] > 0.0 && i.rect[1] < 260.0).count();
        let title = ui.instances_of("title");
        assert!(reduced || drawn == title.len(), "{drawn} glyphs drawn in the titles' part, the title has {}", title.len());
        glyphs.push(title.iter().filter(|i| i.paint[3] == amenui::scene::GLYPH && i.paint[2] > 0.0 && i.rect[2] > 0.0)
            .map(|i| {
                let (sx, sy) = (i.rect[2] / ((i.uv[2] - i.uv[0]) * atlas).max(1e-3), i.rect[3] / ((i.uv[3] - i.uv[1]) * atlas).max(1e-3));
                [i.rect[0] + sp * sx, i.rect[1] + sp * sy, i.rect[0] + i.rect[2] - sp * sx, i.rect[1] + i.rect[3] - sp * sy]
            }).collect());
    }
    (old, frames, glyphs, t)
}
/// The second page drawn directly, at time `t`.
fn direct_at(look: fn(&pages::S) -> amenui::style::Look, t: f32) -> Vec<u8> { page_at(pages::view, look, t) }
fn page_at(view: fn(&pages::S) -> El<()>, look: fn(&pages::S) -> amenui::style::Look, t: f32) -> Vec<u8> {
    let (w, h) = (560u32, 380u32);
    let mut o = amenui::offscreen::Offscreen::new(w, h);
    let mut ui = Ui::new(pages::S { second: true }, view, |_, _| {}, fonts(), w, h, 1.0).on_look(look);
    let mut tf = 0.0;
    while tf < t - 1e-4 { tf += 1.0 / 60.0; ui.tick(tf); }
    let look = ui.look();
    o.render(&mut ui, &look)
}

/// A morph from the last frame, plain or dotted: the title flies, whole, from the old place to the new.
fn the_title_flies(look: fn(&pages::S) -> amenui::style::Look, name: &str) {
    let (w, h) = (560u32, 380u32);
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let (old, frames, glyphs, t) = from_last(&mut o, look, false);
    // The frame at 600 ms is the 36th: it and every one after are the new page drawn directly.
    let at = 35;
    let t_at = t - (frames.len() - 1 - at) as f32 / 60.0;
    let direct = direct_at(look, t_at);
    // The ground as it is drawn with no title, on each page (dottr's dots are part of it, and they make
    // room round text), for the ink; one flat colour for where the titles are.
    let ground = page_at(pages::bare, look, t_at);
    let before = page_at(pages::bare_first, look, t - frames.len() as f32 / 60.0);
    let flat: Vec<u8> = { let g = &direct[..4]; g.iter().copied().cycle().take(direct.len()).collect() };
    let (a, b) = (ink_box(&old, &flat, w).expect("old title"), ink_box(&direct, &flat, w).expect("new title"));
    let first = ink_box(&frames[0], &flat, w).expect("first frame");
    let near = |p: [f32; 4], q: [f32; 4], tol: f32| (0..4).all(|k| (p[k] - q[k]).abs() <= tol);
    assert!(near(first, a, 3.0), "{name}: the first frame's title {first:?} is the old one {a:?}");
    // The centroid of the ink goes along the line between the two titles' centroids, forward in every
    // frame, and from a quarter to three quarters of the time it is out of both ends' 10 %.
    let ((ma, ca), (mb, cb)) = (ink_mass(&old, &before, w), ink_mass(&direct, &ground, w));
    let (dx, dy) = (cb[0] - ca[0], cb[1] - ca[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let mut last = -1.0f32;
    for (k, f) in frames.iter().enumerate().take(at + 1) {
        let (m, c) = ink_mass(f, &ground, w);
        // Ink all the way: never under 60 % of the smaller title's.
        assert!(m >= 0.6 * ma.min(mb), "{name} frame {k}: ink {m:.0}, the titles have {ma:.0} and {mb:.0}");
        let along = ((c[0] - ca[0]) * dx + (c[1] - ca[1]) * dy) / (len * len);
        let off = (dx * (ca[1] - c[1]) - (ca[0] - c[0]) * dy).abs() / len;
        assert!(off <= 0.1 * len, "{name} frame {k}: the centroid {c:?} is {off:.1} px off the line ({len:.0} px long)");
        assert!(along >= last - 0.005, "{name} frame {k}: the centroid goes back, from {last:.3} to {along:.3} of the way");
        let time = (k + 1) as f32 / 36.0;
        if (0.25..=0.75).contains(&time) { assert!(along > 0.1 && along < 0.9, "{name} at {:.0} %: the centroid is {along:.2} of the way", time * 100.0); }
        last = along;
    }
    // Each glyph is drawn once: never an old and a new copy at the same time. On the way the word stays a
    // word: no two glyphs' ink boxes overlap by more than 15 % of the smaller, and they keep their order
    // from left to right.
    for (k, g) in glyphs.iter().enumerate() {
        assert!(g.len() <= 5, "{name} frame {k}: {} glyphs drawn in the titles' part", g.len());
        let area = |b: &[f32; 4]| (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
        for i in 0..g.len() {
            for j in i + 1..g.len() {
                let (a, b) = (g[i], g[j]);
                let o = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0) * (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
                assert!(o <= 0.15 * area(&a).min(area(&b)), "{name} frame {k}: glyphs {i} and {j} overlap by {:.0} %", 100.0 * o / area(&a).min(area(&b)));
            }
        }
        let cx: Vec<f32> = g.iter().map(|b| (b[0] + b[2]) * 0.5).collect();
        assert!(cx.windows(2).all(|w| w[0] < w[1]), "{name} frame {k}: the glyphs are out of order: {cx:?}");
    }
    // At the duration, and after: the new page drawn directly, byte for byte.
    assert!(frames[at] == direct, "{name}: the frame at 600 ms is the new page drawn directly");
    assert!(frames[at - 1] != direct, "{name}: the frame before it is not");
    let end = direct_at(look, t);
    assert!(frames[frames.len() - 1] == end, "{name}: so is the last");
    assert!(near(ink_box(&frames[frames.len() - 1], &flat, w).expect("last"), b, 3.0), "{name}: the last frame's title is the new one");
    // The new page's body is there, as it is drawn directly, from the first frame; the old one is not.
    let region = |px: &[u8], x0: u32, x1: u32| px.chunks_exact(4).enumerate().filter(|(i, _)| { let (x, y) = (*i as u32 % w, *i as u32 / w); (x0..x1).contains(&x) && (290..345).contains(&y) }).flat_map(|(_, p)| p.to_vec()).collect::<Vec<u8>>();
    for (k, f) in frames.iter().enumerate() {
        assert!(region(f, 280, w) == region(&direct, 280, w), "{name} frame {k}: the new body is not as drawn directly");
        assert!(region(f, 0, 280) == region(&direct, 0, 280), "{name} frame {k}: the old body is still drawn");
    }
    // The same input again: the same bytes.
    let mut o2 = amenui::offscreen::Offscreen::new(w, h);
    assert!(from_last(&mut o2, look, false).1 == frames, "{name}: the same frames again");
}

#[test]
fn a_morph_from_the_last_frame_flies_the_old_title_to_the_new_one() { the_title_flies(pages::look, "plain"); }

#[test]
fn in_dottr_the_title_flies_too() { the_title_flies(pages::dotted, "dottr"); }

#[test]
fn in_dottr_the_flying_title_comes_apart_into_dots_and_plain_does_not() {
    // Halfway (300 ms), the dotted title differs from the plain one in more than a quarter of the pixels
    // either has ink in; at the start each is its old page's title and at the end its new page.
    let (w, h) = (560u32, 380u32);
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut o2 = amenui::offscreen::Offscreen::new(w, h);
    let (plain, dotted) = (from_last(&mut o, pages::look, false), from_last(&mut o2, pages::dotted, false));
    let ground: Vec<u8> = { let g = &plain.1[0][..4]; g.iter().copied().cycle().take(plain.1[0].len()).collect() };
    let ink = |px: &[u8], i: usize| px[i * 4..i * 4 + 3].iter().zip(&ground[i * 4..i * 4 + 3]).any(|(a, b)| (*a as i32 - *b as i32).abs() > 24);
    let (p, d) = (&plain.1[17], &dotted.1[17]);
    let (mut any, mut differ) = (0usize, 0usize);
    for i in 0..(w * 260) as usize {
        if !ink(p, i) && !ink(d, i) { continue; }
        any += 1;
        if p[i * 4..i * 4 + 3].iter().zip(&d[i * 4..i * 4 + 3]).any(|(a, b)| (*a as i32 - *b as i32).abs() > 24) { differ += 1; }
    }
    assert!(differ as f32 > 0.25 * any as f32, "halfway, {differ} of {any} title pixels differ");
    // Progress 0 and 1: as the old and the new page in each material.
    for (name, run, look) in [("plain", &plain, pages::look as fn(&pages::S) -> amenui::style::Look), ("dottr", &dotted, pages::dotted)] {
        let top = |px: &[u8]| px[..(w * 260 * 4) as usize].to_vec();
        assert!(top(&run.0) == top(&old_title(look)), "{name}: the old page's title, before");
        assert!(run.1[35] == direct_at(look, run.3 - (run.1.len() - 36) as f32 / 60.0), "{name}: at 600 ms the new page");
    }
}
/// The first page's titles' part drawn on its own after 1.5 s, as `from_last` draws it before the morph.
fn old_title(look: fn(&pages::S) -> amenui::style::Look) -> Vec<u8> {
    let (w, h) = (560u32, 380u32);
    let mut o = amenui::offscreen::Offscreen::new(w, h);
    o.r.partial = false;
    let mut ui = Ui::new(pages::S { second: false }, pages::view, |_, _| {}, fonts(), w, h, 1.0).on_look(look);
    let mut t = 0.0;
    while t < 1.5 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    o.render(&mut ui, &look)
}

#[test]
fn with_less_motion_the_title_fades_out_and_in_where_each_is() {
    // Nothing flies: the old title fades where it was and the new one where it is, both over 150 ms;
    // from then on, the new page drawn directly.
    let (w, h) = (560u32, 380u32);
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let (old, frames, _, t) = from_last(&mut o, pages::look, true);
    let direct = direct_at(pages::look, t);
    let ground: Vec<u8> = { let g = &direct[..4]; g.iter().copied().cycle().take(direct.len()).collect() };
    let (a, b) = (ink_box(&old, &ground, w).expect("old title"), ink_box(&direct, &ground, w).expect("new title"));
    let inside = |p: [f32; 2], r: [f32; 4]| p[0] >= r[0] - 1.0 && p[1] >= r[1] - 1.0 && p[0] <= r[2] + 1.0 && p[1] <= r[3] + 1.0;
    for (k, f) in frames.iter().enumerate().take(9) {
        // Every pixel of ink is in one title's box or the other's: nothing between.
        for (i, (p, g)) in f.chunks_exact(4).zip(ground.chunks_exact(4)).enumerate() {
            let q = [(i as u32 % w) as f32, (i as u32 / w) as f32];
            if q[1] >= 260.0 || p.iter().zip(g).all(|(x, y)| (*x as i32 - *y as i32).abs() <= 24) { continue; }
            assert!(inside(q, a) || inside(q, b), "frame {k}: ink at {q:?}, in neither title");
        }
    }
    assert!(frames[8] == direct, "at 150 ms, the new page drawn directly");
}

