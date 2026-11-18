//! Effects turned off (`Look::off`, DESIGN.md §1.34): each goes straight to its end state, the frame
//! byte for byte the frame the effect settles on, and a page at rest with it off asks for no frames.
//! Needs a GPU adapter (lavapipe will do).
use amenui::decode::Decode;
use amenui::el::{El, Paint, col, space, text};
use amenui::morph::{Cmd, FlowSpec, MorphSpec};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, LIGHT, Look, Material, Off};
use amenui::ui::Ui;
use amenui::widgets::button;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 320;
const H: u32 = 340;

#[derive(Clone, Default)]
struct S { off: Off, m: u8, dark: bool, second: bool, morphed: bool, count: u32, deco: bool, bloom: bool, beat: bool, cmds: Vec<Cmd> }
#[derive(Clone)]
enum Msg { Go }
fn view(s: &S) -> El<Msg> {
    let mut page = col().key("page").fill_width().fill_height().pad(16.0, 16.0).gap(10.0)
        .child(button("go", "Go", Msg::Go))
        .child(text(format!("{} items", s.count), 16.0, Paint::INK).key("count"))
        .child({ let mut tap = space(60.0, 20.0).fill(Paint::ACCENT).key("tap").on_press(Msg::Go); tap.focusable = false; tap });
    page = if !s.second {
        page.child(text("A title", 16.0, Paint::INK).key("p1/title").flow("t")).child(space(80.0, 30.0).fill(Paint::INK).key("a").flow("box"))
    } else {
        page.child(space(200.0, 60.0).fill(Paint::INK).key("b").flow("box")).child(text("A title", 24.0, Paint::INK).key("p2/title").flow("t"))
    };
    // A morph's two ends, both laid out, the view showing one.
    page = page.child(text("Morph", 18.0, Paint::INK).key("ma").hidden_until_morphed(s.morphed))
        .child(space(90.0, 24.0).radius(8.0).fill(Paint::ACCENT).key("mb").hidden_until_morphed(!s.morphed));
    if s.deco { page = page.child(text("Decoded label", 14.0, Paint::TEXT).key("dec").decode(Decode::default())); }
    page
}
fn look(s: &S) -> Look {
    let mut l = Look { material: if s.m == 1 { Material::Dottr } else { Material::Plain }, theme: if s.dark { DARK } else { LIGHT }, off: s.off, ..Look::default() };
    if s.bloom { l.post.bloom = amenui::post::Bloom { strength: 1.0, threshold: 0.3, radius: 8.0 }; }
    if s.beat { l.beat = amenui::beat::Beat { bpm: 120.0, steps: 4, swing: 0.0 }; }
    l
}
fn mk(s: S) -> Ui<S, Msg> {
    let mut ui = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look).on_commands(|s| std::mem::take(&mut s.cmds));
    ui.tick(1.0);
    ui
}
fn shot(o: &mut Offscreen, ui: &mut Ui<S, Msg>, t: f32) -> Vec<u8> { ui.tick(t); let l = ui.look(); let px = o.render(ui, &l); ui.frame_drawn(); px }
fn diff(a: &[u8], b: &[u8]) -> f32 { a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count() as f32 / (a.len() / 4) as f32 }

/// The page from `s` with `off` turned off, `go` done at 2 s: the frame after it, and how many frames it
/// asks for in the 2 s after; the same page with the effect on, long after `go`.
fn run(o: &mut Offscreen, s: S, off: Off, go: fn(&mut Ui<S, Msg>)) -> (Vec<u8>, usize, Vec<u8>) {
    let mut a = mk(S { off, ..s.clone() });
    shot(o, &mut a, 2.0);
    go(&mut a);
    let first = shot(o, &mut a, 2.0 + 1.0 / 60.0);
    let mut asked = 0;
    for f in 2..120 { let t = 2.0 + f as f32 / 60.0; a.tick(t); if a.needs_frame(t) { asked += 1; shot(o, &mut a, t); } }
    let mut b = mk(s);
    shot(o, &mut b, 2.0);
    go(&mut b);
    for f in 1..60 { shot(o, &mut b, 2.0 + f as f32 / 60.0); }
    for f in 60..420 { b.tick(2.0 + f as f32 / 60.0); b.frame_drawn(); }
    let settled = shot(o, &mut b, 9.0);
    (first, asked, settled)
}
fn check(name: &str, s: S, off: Off, go: fn(&mut Ui<S, Msg>)) {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let (first, asked, settled) = run(&mut o, s, off, go);
    assert!(first == settled, "{name}: the frame after is not the settled frame ({:.4} differs)", diff(&first, &settled));
    assert_eq!(asked, 0, "{name}: frames asked for with the effect off");
}

// What comes in new (a changed glyph, a page's parts) condenses unless `close` is off too, so the
// tests of the other effects turn it off with them.
fn with(f: impl Fn(&mut Off)) -> Off { let mut o = Off { close: true, ..Off::default() }; f(&mut o); o }

#[test]
fn close_off_new_things_are_there_at_once() {
    check("close", S::default(), Off { close: true, ..Off::default() }, |ui| { ui.state.deco = false; ui.state.second = true; ui.relayout(); });
}

#[test]
fn fronts_off_a_front_sends_nothing_and_a_commit_shows_the_new_look_at_once() {
    check("ripplr", S::default(), with(|o| o.fronts = true), |ui| { ui.command(Cmd::Ripplr { x: 100.0, y: 100.0, strength: 1.0 }); });
    check("commit", S::default(), with(|o| o.fronts = true), |ui| { ui.state.dark = true; ui.state.cmds.push(Cmd::Commit { x: 100.0, y: 100.0, scope: None }); ui.relayout(); });
}

#[test]
fn flows_off_the_page_changes_at_once() {
    check("flow", S::default(), with(|o| o.flows = true), |ui| { ui.state.second = true; ui.state.cmds.push(Cmd::Flow(FlowSpec::default())); ui.relayout(); });
}

#[test]
fn morphs_off_the_new_picture_is_there_at_once() {
    check("morph", S::default(), with(|o| o.morphs = true), |ui| { ui.state.morphed = true; ui.state.cmds.push(Cmd::Morph { from: "ma".into(), to: "mb".into(), spec: MorphSpec::default() }); ui.relayout(); });
}

#[test]
fn ripples_and_loosening_off_a_press_changes_nothing_round_it() {
    // (Pressed where nothing takes the focus: the focus marks closing in are not one of these effects.)
    check("press", S::default(), with(|o| { o.ripples = true; o.loosen = true; }), |ui| { ui.command(Cmd::Press { key: "tap".into() }); });
    // Ripples off alone: no front is sent.
    let mut ui = mk(S { off: Off { ripples: true, ..Off::default() }, ..S::default() });
    ui.tick(2.0);
    ui.command(Cmd::Press { key: "tap".into() });
    ui.tick(2.0 + 1.0 / 60.0);
    assert!(ui.field().fronts.iter().all(|f| f[3] <= 0.0), "a front was sent");
}

#[test]
fn recompose_off_changed_text_is_replaced_at_once() {
    check("recompose", S { count: 12, ..S::default() }, with(|o| o.recompose = true), |ui| { ui.state.count = 13; ui.relayout(); });
}

#[test]
fn decode_off_the_text_is_there_at_once() {
    check("decode", S::default(), with(|o| o.decode = true), |ui| { ui.state.deco = true; ui.relayout(); });
}

#[test]
fn material_still_the_ground_stands_and_the_beat_stops() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let s = S { m: 1, beat: true, off: Off { material_still: true, ..Off::default() }, ..S::default() };
    // A front moves the whole ground; still, the ground is where it was once the front has gone.
    let mut a = mk(s.clone());
    shot(&mut o, &mut a, 2.0);
    a.command(Cmd::Ripplr { x: 100.0, y: 100.0, strength: 1.0 });
    for f in 1..300 { shot(&mut o, &mut a, 2.0 + f as f32 / 60.0); }
    let after = shot(&mut o, &mut a, 9.0);
    let mut c = mk(s);
    let rest = shot(&mut o, &mut c, 9.0);
    assert!(after == rest, "the ground moved ({:.4} differs)", diff(&after, &rest));
    assert!(a.look().beat.bpm == 0.0, "the beat runs");
    let mut asked = 0;
    for f in 0..120 { let t = 9.0 + f as f32 / 60.0; a.tick(t); if a.needs_frame(t) { asked += 1; shot(&mut o, &mut a, t); } }
    assert_eq!(asked, 0);
}

#[test]
fn bloom_off_no_bloom_is_drawn() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut a = mk(S { bloom: true, dark: true, off: Off { bloom: true, ..Off::default() }, ..S::default() });
    let mut b = mk(S { dark: true, ..S::default() });
    let mut c = mk(S { bloom: true, dark: true, ..S::default() });
    let (pa, pb, pc) = (shot(&mut o, &mut a, 3.0), shot(&mut o, &mut b, 3.0), shot(&mut o, &mut c, 3.0));
    assert!(pc != pb, "bloom draws nothing here");
    assert!(pa == pb, "bloom drawn while off ({:.4} differs)", diff(&pa, &pb));
    let mut asked = 0;
    for f in 0..120 { let t = 3.0 + f as f32 / 60.0; a.tick(t); if a.needs_frame(t) { asked += 1; shot(&mut o, &mut a, t); } }
    assert_eq!(asked, 0);
}

#[test]
fn all_off_by_default() { assert_eq!(Look::default().off, Off::default()); }

#[test]
fn turned_off_mid_flight_an_effect_ends_where_it_was_going() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut a = mk(S::default());
    shot(&mut o, &mut a, 2.0);
    a.state.second = true;
    a.state.cmds.push(Cmd::Flow(FlowSpec::default()));
    a.relayout();
    for f in 1..8 { shot(&mut o, &mut a, 2.0 + f as f32 / 60.0); }
    a.state.off = Off { flows: true, close: true, ..Off::default() };
    let cut = shot(&mut o, &mut a, 2.0 + 8.0 / 60.0);
    let mut b = mk(S { second: true, ..S::default() });
    let rest = shot(&mut o, &mut b, 9.0);
    assert!(cut == rest, "the flow went on ({:.4} differs)", diff(&cut, &rest));
}
