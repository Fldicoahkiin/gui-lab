//! The field map (DESIGN.md §1.29) through the runtime at 60 Hz: a floating box dragged over a page
//! leaves a trail that loosens the decoration beside its path (and never the text to read past its cap,
//! the nudged text, what ignores the field, what is far from the path, or the box itself); the map is
//! all zero again soon after it is let go and asks for no frames; Normcore writes nothing; a press
//! without a drag stirs nothing; with reduced motion there is never any flow.
use amenui::el::{El, Paint, Response, col, layers, row, space, text};
use amenui::style::Look;
use amenui::ui::Ui;
use amenui::widgets::{FloatMsg, Floating, floating};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone)] enum M { F(FloatMsg) }
struct S { f: Floating, t: f32, look: Look }
fn view(s: &S) -> El<M> {
    let page = col().key("page").pad(40.0, 40.0).gap(24.0)
        .child(row().gap(30.0)
            .child(space(60.0, 60.0).radius(30.0).fill(Paint::hue(0)).key("near-shape").respond(Response::Loosen))
            .child(space(60.0, 60.0).fill(Paint::hue(1)).key("ignore").respond(Response::Ignore))
            .child(text("To read", 16.0, Paint::INK).key("nudge"))
            .child(text("Loosen, legible", 16.0, Paint::INK).key("legible").respond(Response::Loosen)))
        .child(space(1.0, 300.0))
        .child(space(60.0, 60.0).fill(Paint::hue(2)).key("far-shape").respond(Response::Loosen));
    let box_ = floating("float", &s.f, col().pad(12.0, 12.0).child(text("box", 14.0, Paint::INK).key("float/label")), M::F);
    layers([page, box_]).width(1280.0).height(800.0)
}
fn update(s: &mut S, m: M) { let M::F(f) = m; let t = s.t; s.f.update(f, t); }
fn open(look: Look) -> Ui<S, M> {
    let mut f = Floating::default();
    (f.size, f.area) = ((160.0, 96.0), (1280.0, 800.0));
    f.place(0);
    let mut ui = Ui::new(S { f, t: 0.0, look }, view, update, fonts(), 1280, 800, 1.0)
        .on_look(|s| s.look)
        .on_tick(|s, t| { s.t = t; s.f.step(t) });
    for i in 1..=60 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
fn run(ui: &mut Ui<S, M>, secs: f32) -> usize {
    let (mut t, to, mut drawn) = (ui.now(), ui.now() + secs, 0);
    while t < to { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; } ui.frame_drawn(); }
    drawn
}
/// Drag the box from its place 185 px to the right along the row of decoration, at 60 Hz; the most
/// looseness each key saw on the way.
fn drag(ui: &mut Ui<S, M>, keys: &[&str]) -> Vec<f32> {
    let (x, y) = ui.node("float").expect("box").rect.center();
    let mut most = vec![0.0f32; keys.len()];
    let mut t = ui.now();
    ui.press(x, y);
    for i in 1..=37 {
        t += 1.0 / 60.0;
        ui.pointer_move(x + i as f32 * 5.0, y + i as f32 * 0.4);
        ui.tick(t);
        ui.frame_drawn();
        for (k, key) in keys.iter().enumerate() { most[k] = most[k].max(ui.looseness(key, t)); }
    }
    ui.release(x + 185.0, y + 15.0);
    for _ in 0..30 {
        t += 1.0 / 60.0;
        ui.tick(t);
        ui.frame_drawn();
        for (k, key) in keys.iter().enumerate() { most[k] = most[k].max(ui.looseness(key, t)); }
    }
    most
}
fn plain() -> Look { Look { material: amenui::style::Material::Plain, ..Look::default() } }

#[test]
fn a_dragged_box_loosens_what_its_trail_reaches_and_nothing_else() {
    let mut ui = open(plain());
    // The box starts over the row of decoration (corner 0 is the top left).
    let keys = ["near-shape", "nudge", "legible", "ignore", "far-shape", "float", "float/label"];
    let most = drag(&mut ui, &keys);
    let cap = ui.state.look.legible_cap;
    assert!(most[0] > 0.3, "the decoration beside the path loosened only {}", most[0]);
    assert_eq!(most[1], 0.0, "nudged text loosened");
    assert!(most[2] > 0.0 && most[2] <= cap + 1e-4, "legible text loosened {} (cap {cap})", most[2]);
    assert_eq!(most[3], 0.0, "what ignores the field loosened");
    assert_eq!(most[4], 0.0, "a shape far from the path loosened");
    assert_eq!(most[5].max(most[6]), 0.0, "the box loosened by its own trail");
    assert!(ui.fieldmap().written);
    // Let go (it glides to a corner, still stirring): a second and a half after it stops, the map is
    // all zero and no frame is asked for.
    let mut n = 0;
    while ui.state.f.moving() && n < 300 { run(&mut ui, 1.0 / 60.0); n += 1; }
    run(&mut ui, 1.5);
    assert_eq!(ui.fieldmap().nonzero(), 0, "still {} nodes after 1.5 s", ui.fieldmap().nonzero());
    let t = ui.now();
    assert!(!ui.needs_frame(t), "frames asked for after the map settled");
    assert_eq!(run(&mut ui, 2.0), 0);
}

#[test]
fn normcore_writes_nothing_and_a_press_alone_stirs_nothing() {
    let presets = amenui::preset::presets();
    let normcore = presets.iter().find(|p| p.hash == "normcore").expect("normcore").apply(&Look::default());
    let mut ui = open(normcore);
    drag(&mut ui, &["near-shape"]);
    assert!(!ui.fieldmap().written, "Normcore's map was written");
    run(&mut ui, 1.0);
    assert_eq!(run(&mut ui, 2.0), 0, "Normcore drew frames at rest");
    // A press with no drag.
    let mut ui = open(plain());
    let (x, y) = ui.node("float").expect("box").rect.center();
    ui.press(x, y);
    run(&mut ui, 0.2);
    ui.release(x, y);
    run(&mut ui, 0.3);
    assert!(!ui.fieldmap().written, "a press alone stirred the map");
}

#[test]
fn reduced_motion_keeps_the_energy_and_puts_no_flow() {
    let mut ui = open(plain());
    ui.set_reduced_motion(true);
    let (x, y) = ui.node("float").expect("box").rect.center();
    let mut t = ui.now();
    ui.press(x, y);
    let mut energy = 0.0f32;
    for i in 1..=37 {
        t += 1.0 / 60.0;
        ui.pointer_move(x + i as f32 * 5.0, y);
        ui.tick(t);
        let m = ui.fieldmap();
        for j in 0..m.rows { for k in 0..m.cols { let (e, f) = m.node(k, j); energy = energy.max(e); assert_eq!(f, [0.0, 0.0]); } }
    }
    ui.release(x + 185.0, y);
    assert!(energy > 0.3, "reduced motion: the energy is {energy}");
}

#[test]
fn a_stir_command_reaches_its_scope() {
    #[derive(Default)] struct P { cmds: Vec<amenui::morph::Cmd> }
    fn v(_: &P) -> El<()> { col().key("p").child(space(200.0, 200.0).key("deco").fill(Paint::hue(0)).respond(Response::Loosen)) }
    let mut ui = Ui::new(P::default(), v, |_, _| {}, fonts(), 640, 400, 1.0).on_look(|_| plain()).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(1.0);
    assert_eq!(ui.looseness("deco", 1.0), 0.0);
    ui.state.cmds.push(amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Key("deco".into()), v: [1.0, 0.0], energy: 1.0, radius: 60.0, scope: amenui::el::Scope::Global });
    ui.relayout();
    assert!(ui.looseness("deco", 1.0) > 0.9, "a stir at its centre: {}", ui.looseness("deco", 1.0));
    // Along a path: only the part between `from` and `to` is stirred.
    let mut ui = Ui::new(P::default(), v, |_, _| {}, fonts(), 640, 400, 1.0).on_look(|_| plain()).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(1.0);
    let path = std::rc::Rc::new(vec![[0.0, 300.0], [600.0, 300.0]]);
    ui.state.cmds.push(amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Path { points: path, from: 0.0, to: 0.5 }, v: [1.0, 0.0], energy: 1.0, radius: 20.0, scope: amenui::el::Scope::Global });
    ui.relayout();
    let m = ui.fieldmap();
    assert!(m.energy_at(150.0, 300.0) > 0.5 && m.energy_at(580.0, 300.0) == 0.0, "{} {}", m.energy_at(150.0, 300.0), m.energy_at(580.0, 300.0));
    assert!(m.flow_at(150.0, 300.0)[0] > 0.3, "the flow follows the path: {:?}", m.flow_at(150.0, 300.0));
}

/// A Lab-like index cell (a decoded drawing that grew, and an element that stirs the map), and the
/// page the route goes to instead.
mod route {
    use amenui::el::{El, Paint, Scope, Stirr, canvas, col, space, text};
    #[derive(Clone)] pub struct R { pub index: bool }
    pub fn view(s: &R) -> El<()> {
        if !s.index { return col().key("reader").pad(24.0, 24.0).child(text("Lumen: light on text", 18.0, Paint::INK).key("reader/title")); }
        let mut d = amenui::draw::Drawing::new(120.0, 90.0);
        for i in 0..12 { d.dot(10.0 + i as f32 * 8.0, 45.0, 3.0, Paint::ACCENT); }
        col().key("index").pad(24.0, 24.0).gap(12.0)
            .child(col().key("cell/grown").width(160.0).child(canvas(d).key("grown/canvas").decode(amenui::decode::Decode::default())))
            .child(space(40.0, 40.0).key("stylus").stirr(Stirr { energy: 1.0, radius: Some(60.0), scope: Scope::Global }))
            .child(text("Reader", 14.0, Paint::INK).key("go").on_press(()))
    }
    pub fn look(_: &R) -> amenui::style::Look { amenui::style::Look { material: amenui::style::Material::Plain, ..amenui::style::Look::default() } }
}

#[test]
fn an_element_that_leaves_with_its_page_leaves_no_field() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(400, 300) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = Ui::new(route::R { index: true }, route::view, |s, _| s.index = !s.index, fonts(), 400, 300, 1.0).on_look(route::look);
    let mut t = 0.0;
    let step = |ui: &mut Ui<route::R, ()>, t: &mut f32| { *t += 1.0 / 60.0; ui.tick(*t); };
    // The decode has run and the map holds the stir.
    // No frame draws a colour the theme cannot make (a field in its own colour, a debug layer).
    let off = |px: &[u8], look: &amenui::style::Look| amenui::audit::off_palette(px, 400, &look.theme, &[], 6.0);
    for _ in 0..60 { step(&mut ui, &mut t); let look = ui.look(); let px = o.render(&mut ui, &look); ui.frame_drawn(); assert_eq!(off(&px, &look), 0); }
    ui.command(amenui::morph::Cmd::Stir { at: amenui::morph::StirAt::Key("stylus".into()), v: [1.0, 0.0], energy: 1.0, radius: 60.0, scope: amenui::el::Scope::Global });
    step(&mut ui, &mut t);
    assert!(ui.fieldmap().nonzero() > 0, "the stylus stirred the map");
    // The route changes: this very frame is the new page drawn directly.
    assert!(ui.activate("go"));
    step(&mut ui, &mut t);
    let look = ui.look();
    let after = o.render(&mut ui, &look);
    ui.frame_drawn();
    assert_eq!(off(&after, &look), 0, "the frame the page changes draws only the theme's colours");
    let mut fresh = Ui::new(route::R { index: false }, route::view, |s, _| s.index = !s.index, fonts(), 400, 300, 1.0).on_look(route::look);
    fresh.tick(t);
    // Its own renderer: a renderer keeps the text atlas of the page it draws.
    let mut o2 = amenui::offscreen::Offscreen::new(400, 300);
    let look = fresh.look();
    let direct = o2.render(&mut fresh, &look);
    // Nothing of the old page anywhere: outside the new title (which is being born, as any new
    // element is) every pixel is the new page's.
    let b = ui.node("reader/title").expect("the new title").rect;
    let outside = |i: usize| { let (x, y) = ((i % 400) as f32, (i / 400) as f32); x < b.x - 4.0 || y < b.y - 4.0 || x > b.right() + 4.0 || y > b.bottom() + 4.0 };
    let differ = after.chunks_exact(4).zip(direct.chunks_exact(4)).enumerate().filter(|(i, (a, b))| outside(*i) && a != b).count();
    assert_eq!(differ, 0, "{differ} pixels beside the new page differ from it drawn directly");
    // Once the new page has come in (born, its text set), the whole frame is the new page's.
    for i in 0..180 {
        step(&mut ui, &mut t); fresh.tick(t);
        let look = ui.look(); let px = o.render(&mut ui, &look); ui.frame_drawn();
        let n = off(&px, &look);
        assert_eq!(n, 0, "frame {i} after the change: {n} pixels in no colour of the theme");
    }
    let look = ui.look();
    let after = o.render(&mut ui, &look);
    ui.frame_drawn();
    let look = fresh.look();
    let direct = o2.render(&mut fresh, &look);
    let differ = after.chunks_exact(4).zip(direct.chunks_exact(4)).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} pixels differ from the new page drawn directly, once it has come in");
    assert_eq!(ui.dissolving(), 0, "nothing of the old page is played out");
    // And then it asks for no frames for two seconds.
    let mut asked = 0;
    for _ in 0..120 { step(&mut ui, &mut t); if ui.needs_frame(t) { asked += 1; } let look = ui.look(); let _ = o.render(&mut ui, &look); ui.frame_drawn(); }
    assert_eq!(asked, 0, "{asked} frames asked for after the page went");
}

#[test]
fn a_press_front_that_outlives_its_page_runs_on_as_a_whole_ring() {
    // The press on "Reader" changes the page; 50 ms later its front shows on the new page all round the
    // press, in all four quadrants, not only along the old row.
    let (w, h) = (400u32, 300u32);
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    o.r.partial = false;
    let look = |_: &route::R| Look::default();
    let mut ui = Ui::new(route::R { index: true }, route::view, |s, _| s.index = !s.index, fonts(), w, h, 1.0).on_look(look);
    let mut t = 0.0;
    for _ in 0..60 { t += 1.0 / 60.0; ui.tick(t); let l = ui.look(); let _ = o.render(&mut ui, &l); }
    let (px, py) = ui.node("go").expect("go").rect.center();
    ui.press(px, py);
    ui.release(px, py);
    ui.pointer_leave();
    assert!(!ui.state.index, "the page changed");
    let mut at50 = vec![];
    for k in 1..=180 { t += 1.0 / 60.0; ui.tick(t); let l = ui.look(); let f = o.render(&mut ui, &l); if k == 3 { at50 = f; } }
    let l = ui.look();
    let rest = o.render(&mut ui, &l);
    let title = ui.node("reader/title").expect("title").rect;
    let mut quads = [0usize; 4];
    let mut far = [0usize; 2];
    for (i, (a, b)) in at50.chunks_exact(4).zip(rest.chunks_exact(4)).enumerate() {
        if a == b { continue; }
        let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
        // The new title is still coming in: not the front.
        if x >= title.x - 4.0 && x <= title.right() + 4.0 && y >= title.y - 4.0 && y <= title.bottom() + 4.0 { continue; }
        quads[(if x < px { 0 } else { 1 }) + (if y < py { 0 } else { 2 })] += 1;
        if (y - py).abs() > 40.0 { if y < py { far[0] += 1; } else { far[1] += 1; } }
    }
    eprintln!("more than 40 px above and below the press: {far:?}");
    assert!(far[0] > 0 && far[1] > 0, "the front reaches beyond the old row, above and below: {far:?}");
    eprintln!("pixels changed by quadrant about the press: {quads:?}");
    assert!(quads.iter().all(|q| *q > 0), "the front shows in every quadrant about the press: {quads:?}");
}
