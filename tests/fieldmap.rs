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
