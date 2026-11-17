//! Text on a form (DESIGN.md §1.26), without a GPU: a passage wrapped on a sphere turned a whole way
//! round never draws a glyph that faces away (`back` 0), never leaves two drawn glyphs covering more of
//! each other's ink than `overlap`, draws far to near, and gives the same instances every time; with the
//! default depth and facing, a pose is drawn exactly as before.
use amenui::el::{El, GlyphAt, Paint, Pose, Reveal, col, text};
use amenui::form::{Form, FormFx};
use amenui::recipes::form3d;
use amenui::scene::Instance;
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

/// The recipe's sphere at one turn, settled.
fn sphere(yaw: f32) -> Ui<form3d::Form3d, form3d::Msg> {
    let mut ui = form3d::ui(fonts(), 720, 640, 1.0);
    ui.state.spin = [yaw, 8.0];
    ui.relayout();
    ui.tick(2.0);
    ui
}

/// The fraction of `a`'s ink that `b`'s ink covers, on a fine grid.
fn covered(a: &Instance, b: &Instance, atlas: &[u8]) -> f32 {
    let (mut a, mut b) = (*a, *b);
    a.clip = [-1e9, -1e9, 1e9, 1e9];
    b.clip = a.clip;
    let ab = amenui::form::ink_box(&a);
    let (mut ink, mut hid) = (0.0, 0.0);
    for iy in 0..12 { for ix in 0..12 {
        let (x, y) = (ab[0] + (ix as f32 + 0.5) * (ab[2] - ab[0]) / 12.0, ab[1] + (iy as f32 + 0.5) * (ab[3] - ab[1]) / 12.0);
        if amenui::morph::cover_at(&a, x, y, atlas) < 0.5 { continue; }
        ink += 1.0;
        if amenui::morph::cover_at(&b, x, y, atlas) >= 0.5 { hid += 1.0; }
    } }
    if ink < 4.0 { 0.0 } else { hid / ink }
}

#[test]
fn a_turning_sphere_shows_only_its_front_and_thins_its_rim() {
    let fx = FormFx::default();
    for step in 0..12 {
        let yaw = step as f32 * 30.0;
        let ui = sphere(yaw);
        let node = ui.nodes().iter().position(|n| n.key == "passage").expect("passage");
        let log: Vec<_> = ui.glyphs().log.iter().filter(|g| g.node == node).copied().collect();
        // The rim fades from `edge` 0.6: the front keeps over 120 glyphs at any turn.
        assert!(log.len() > 120, "yaw {yaw}: only {} glyphs drawn", log.len());
        let insts = &ui.scene().instances;
        // Far to near: depths never fall along the drawing order.
        for w in log.windows(2) { assert!(w[0].z <= w[1].z + 1e-4, "yaw {yaw}: drawn out of depth order"); }
        // Nothing that faces away: every drawn glyph's place on the sphere faces the camera.
        let form = form3d::form([yaw, 8.0]);
        for g in &log {
            let (_, laid, _) = ui.glyphs().laid.iter().find(|(n, ..)| *n == node).expect("laid");
            let span = laid.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, &(x, y)| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]);
            let p = form.pose_of(GlyphAt { i: g.i, n: laid.len(), x: g.at.0, y: g.at.1, w: form3d::SIZE, h: form3d::SIZE, span });
            assert!(p.facing >= 0.0, "yaw {yaw}: glyph {} faces away ({}) and is drawn", g.i, p.facing);
        }
        // No two drawn glyphs cover more of each other than `overlap` (with a little room for the
        // coarser grid the runtime measures on).
        for (k, a) in log.iter().enumerate() {
            for b in &log[k + 1..] {
                let (ia, ib) = (&insts[a.inst], &insts[b.inst]);
                let (ba, bb) = (amenui::form::ink_box(ia), amenui::form::ink_box(ib));
                if ba[0] >= bb[2] || bb[0] >= ba[2] || ba[1] >= bb[3] || bb[1] >= ba[3] { continue; }
                // The farther one is the one that could be hidden.
                let (far, near) = if a.z <= b.z { (ia, ib) } else { (ib, ia) };
                let c = covered(far, near, ui.atlas());
                assert!(c <= fx.overlap + 0.15, "yaw {yaw}: glyphs {} and {} overlap {c:.2}", a.i, b.i);
            }
        }
    }
}

#[test]
fn the_same_turn_gives_the_same_instances() {
    let a = sphere(40.0).scene().instances.clone();
    let b = sphere(40.0).scene().instances.clone();
    let c = sphere(40.0).scene().instances.clone();
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_ne!(a, sphere(70.0).scene().instances);
}

#[test]
fn words_on_a_form_come_and_go_whole() {
    let ui = sphere(25.0);
    let node = ui.nodes().iter().position(|n| n.key == "passage").expect("passage");
    let (_, laid, drawn) = ui.glyphs().laid.iter().find(|(n, ..)| *n == node).cloned().expect("laid");
    let mut ui = ui;
    let groups = ui.reveal_groups(node, Reveal::Word);
    let origin = { let n = &ui.nodes()[node]; (n.inner.x, n.inner.y) };
    let mut checked = 0;
    for bs in &groups {
        let inw: Vec<bool> = laid.iter().zip(&drawn).filter(|((x, y), _)| bs.iter().any(|r| r.inset(-0.5, -0.5, -0.5, -0.5).contains(x + origin.0, y + origin.1))).map(|(_, d)| *d).collect();
        if inw.is_empty() { continue; }
        checked += 1;
        assert!(inw.iter().all(|d| *d) || inw.iter().all(|d| !*d), "a word is drawn in part");
    }
    assert!(checked > 100);
}

// A pose with the default depth and facing is drawn exactly as the same pose always was.
struct P;
fn posed(_: &P) -> El<()> {
    col().pad(20.0, 20.0).child(text("on a path the glyphs keep their outline", 22.0, Paint::INK).key("t")
        .pose(|g: GlyphAt| Pose { dy: (g.x * 0.05).sin() * 12.0, angle: (g.x * 0.05).cos() * 0.3, tone: 0.9, ..Pose::default() }))
}
fn posed_form(_: &P) -> El<()> {
    col().pad(20.0, 20.0).child(text("on a path the glyphs keep their outline", 22.0, Paint::INK).key("t")
        .pose(|g: GlyphAt| Pose { dy: (g.x * 0.05).sin() * 12.0, angle: (g.x * 0.05).cos() * 0.3, tone: 0.9, ..Pose::default() })
        // Every glyph faces the viewer at depth 0: nothing to thin, fade or dim.
        .form(FormFx { overlap: 1.0, ..FormFx::default() }))
}
fn flat(_: &P) -> El<()> {
    col().pad(20.0, 20.0).child(text("on a path the glyphs keep their outline", 22.0, Paint::INK).key("t"))
}
#[test]
fn default_depth_and_facing_change_nothing() {
    let run = |v: fn(&P) -> El<()>| { let mut ui = Ui::new(P, v, |_, _| {}, fonts(), 640, 200, 1.0); ui.tick(1.0); ui.scene().instances.clone() };
    let (a, b) = (run(posed), run(posed_form));
    assert_eq!(a, b, "a form with nothing to do draws the pose as it is");
    // A pose that does nothing is the flat text.
    fn none(_: &P) -> El<()> { col().pad(20.0, 20.0).child(text("on a path the glyphs keep their outline", 22.0, Paint::INK).key("t").pose(|_| Pose::default())) }
    assert_eq!(run(none), run(flat));
}

#[test]
fn form_hit_finds_the_text_drawn_under_the_pointer() {
    for spin in [[30.0, 0.0], [-75.0, 20.0], [160.0, -10.0]] {
        let f = Form::sphere(spin);
        let h = f.hit(200.0, 200.0, 400.0, 400.0).expect("the middle of the box is on the sphere");
        // The glyph laid at that place of a 1000 × 300 block is drawn at the middle of the box.
        let span = [0.0, 0.0, 1000.0, 300.0];
        let (x, y) = (h.uv[0].rem_euclid(1.0) * 1000.0, h.uv[1] * 300.0);
        let p = f.pose_of(GlyphAt { i: 0, n: 1, x, y, w: 400.0, h: 400.0, span });
        assert!((x + p.dx - 200.0).abs() < 0.5 && (y + p.dy - 200.0).abs() < 0.5, "spin {spin:?}: drawn at {} {}", x + p.dx, y + p.dy);
        assert!(p.facing > 0.99, "straight on: {}", p.facing);
    }
}

#[test]
fn every_clearly_drawn_glyph_keeps_most_of_its_width() {
    // With the default `edge`, a glyph drawn at (nearly) full opacity is never squeezed under 0.6 of its
    // width, at any turn.
    for step in 0..24 {
        let yaw = step as f32 * 15.0;
        let ui = sphere(yaw);
        let node = ui.nodes().iter().position(|n| n.key == "passage").expect("passage");
        for g in ui.glyphs().log.iter().filter(|g| g.node == node) {
            let inst = &ui.scene().instances[g.inst];
            if inst.paint[2] < 0.95 { continue; }
            let sx = (1.0 + inst.xf[0]).hypot(inst.xf[2]);
            assert!(sx >= 0.6, "yaw {yaw}: glyph {} drawn whole at {sx:.2} of its width", g.i);
        }
    }
}

#[test]
fn the_pointer_lights_glyphs_by_a_smooth_falloff() {
    let mut ui = sphere(-18.0);
    let node = ui.nodes().iter().position(|n| n.key == "passage").expect("passage");
    // The pointer in the middle of the sphere.
    let n = ui.nodes()[node].inner;
    let h = form3d::form(ui.state.spin).hit(n.w * 0.5, n.h * 0.5, n.w, n.h).expect("on the sphere");
    ui.state.hover = Some(h.uv);
    ui.relayout();
    ui.tick(2.5);
    let (_, laid, _) = ui.glyphs().laid.iter().find(|(n, ..)| *n == node).cloned().expect("laid");
    let span = laid.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, &(x, y)| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]);
    let at = |i: usize| { let (x, y) = laid[i]; let (gx, gy) = (span[0] + h.uv[0] * (span[2] - span[0]), span[1] + h.uv[1] * (span[3] - span[1])); let w = span[2] - span[0]; let dx = (x - gx).abs().min(w - (x - gx).abs()); dx.hypot(y - gy) };
    let accent: std::collections::HashMap<usize, f32> = ui.glyphs().log.iter().filter(|g| g.node == node).map(|g| (g.i, ui.scene().instances[g.inst].paint[1])).collect();
    let mut lit = 0;
    for (&i, &a) in &accent {
        if at(i) >= form3d::REACH { assert_eq!(a, 0.0, "glyph {i} {:.0} px away is lit {a}", at(i)); }
        if a > 0.5 { lit += 1; }
    }
    assert!(lit >= 6, "the pointer lights a few words: {lit}");
    // Neighbours in a word: no hard edge between them.
    let groups = ui.reveal_groups(node, Reveal::Word);
    let origin = { let n = &ui.nodes()[node]; (n.inner.x, n.inner.y) };
    let mut pairs = 0;
    for bs in &groups {
        let mut inw: Vec<usize> = (0..laid.len()).filter(|&i| bs.iter().any(|r| r.inset(-0.5, -0.5, -0.5, -0.5).contains(laid[i].0 + origin.0, laid[i].1 + origin.1))).collect();
        inw.sort();
        for w in inw.windows(2) {
            let (Some(a), Some(b)) = (accent.get(&w[0]), accent.get(&w[1])) else { continue };
            if *a > 0.0 || *b > 0.0 { pairs += 1; }
            assert!((a - b).abs() <= 0.35, "glyphs {} and {} of one word: accent {a:.2} then {b:.2}", w[0], w[1]);
        }
    }
    assert!(pairs > 10, "lit neighbours checked: {pairs}");
}
