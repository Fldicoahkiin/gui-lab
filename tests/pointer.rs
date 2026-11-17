//! Pointers, touch and the wheel (DESIGN.md §1.8.2), without a GPU: fling decay and the rubber band
//! as functions; a finger scrolling the 10,000 rows, flinging on and coming to rest; a tap pressing; a
//! scroll that started on a row not pressing it; pulling past the top loosening the edge, not moving
//! the content, and springing back; a second finger cancelling; pointer capture for a finger on a
//! slider; small controls hit within 28 px; no hover from a finger; the pointer's shapes; a button
//! inside an element that drags (a scene hearing the pointer) taking its own press.
use amenui::el::{Cursor, El, Paint, col, space};
use amenui::ui::{MIN_HIT, Scrolling, Ui, fling_step, rubber_band};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[test]
fn fling_decays_exponentially() {
    let s = Scrolling::default();
    let (mut v, mut x, mut t) = (2000.0f32, 0.0f32, 0.0f32);
    while v.abs() > 20.0 && t < 10.0 { let (d, nv) = fling_step(v, 1.0 / 60.0, s.friction); x += d; v = nv; t += 1.0 / 60.0; }
    // The whole distance is v0 / friction; the speed falls by e^(−friction) each second.
    assert!((x - 2000.0 / s.friction).abs() < 20.0 / s.friction + 1.0, "distance {x}");
    assert!((t - (2000.0f32 / 20.0).ln() / s.friction).abs() < 0.05, "stops after {t} s");
    let (_, v1) = fling_step(1000.0, 1.0, s.friction);
    assert!((v1 - 1000.0 * (-s.friction).exp()).abs() < 1e-2);
    // Steps compose: two half steps are one step.
    let (a, va) = fling_step(800.0, 0.25, 3.0);
    let (b, vb) = fling_step(va, 0.25, 3.0);
    let (c, vc) = fling_step(800.0, 0.5, 3.0);
    assert!((a + b - c).abs() < 1e-3 && (vb - vc).abs() < 1e-3);
}

#[test]
fn the_rubber_band_resists_and_is_bounded() {
    let mut o = 0.0;
    let mut steps = vec![];
    for _ in 0..40 { let n = rubber_band(o, -20.0, 0.55, 140.0); steps.push(o - n); o = n; }
    assert!((-140.0..0.0).contains(&o));
    assert!(steps.windows(2).all(|w| w[1] <= w[0] + 1e-4), "each pull moves it less");
    assert!(steps[0] < 20.0, "less than the finger");
}

struct Tiny { hits: u32 }
fn tiny(_: &Tiny) -> El<()> {
    col().key("root").pad(100.0, 100.0).child(space(10.0, 10.0).key("dot").fill(Paint::INK).on_press(()))
}

#[test]
fn small_controls_are_hit_within_28_px() {
    let mut ui = Ui::new(Tiny { hits: 0 }, tiny, |s: &mut Tiny, _| s.hits += 1, fonts(), 400, 300, 1.0);
    let r = ui.node("dot").unwrap().rect;
    assert_eq!(r.w, 10.0);
    // 8 px off its edge: inside the 28 px target.
    let (x, y) = (r.right() + 8.0, r.center().1);
    ui.press(x, y); ui.release(x, y);
    assert_eq!(ui.state.hits, 1);
    // Further than half of 28 px from its centre: not hit.
    let (x, y) = (r.center().0 + MIN_HIT * 0.5 + 1.0, r.center().1);
    ui.tick(1.0);
    ui.press(x, y); ui.release(x, y);
    assert_eq!(ui.state.hits, 1);
    // The look did not grow.
    assert_eq!(ui.node("dot").unwrap().rect.w, 10.0);
    // Hover over the grown target: the pointer shape says it can be pressed.
    ui.pointer_move(r.right() + 6.0, r.center().1);
    assert_eq!(ui.cursor(), Cursor::Pointer);
}

/// Two buttons side by side: the marks are on the one under the pointer in the frame the pointer gets
/// there (before any tick), and in no frame between two targets do they trail behind.
fn pair(_: &()) -> El<u8> {
    amenui::el::row().key("root").pad(40.0, 40.0).gap(200.0)
        .child(amenui::widgets::button("a", "A", 1u8))
        .child(amenui::widgets::button("b", "B", 2u8))
}

#[test]
fn hover_marks_land_in_the_pointers_frame() {
    let mut ui = Ui::new((), pair, |_, _| {}, fonts(), 600, 200, 1.0);
    ui.tick(1.0);
    let (a, b) = (ui.node("a").unwrap().rect, ui.node("b").unwrap().rect);
    ui.pointer_move(a.center().0, a.center().1);
    let (r, alpha) = ui.marks_frame().expect("marks on A at once");
    assert_eq!(r, a);
    assert!(alpha >= 0.5, "seen in the first frame: {alpha}");
    let mut t = 1.0;
    for _ in 0..10 { t += 1.0 / 60.0; ui.tick(t); }
    // Straight onto B: the very same frame, no glide across the gap.
    ui.pointer_move(b.center().0, b.center().1);
    assert_eq!(ui.marks_frame().expect("marks").0, b);
    let start = t;
    let mut settled = None;
    for _ in 0..12 {
        t += 1.0 / 60.0;
        ui.tick(t);
        ui.frame_drawn();
        assert_eq!(ui.marks_frame().expect("marks").0, b, "never between A and B");
        if settled.is_none() && ui.marks_frame().expect("marks").1 >= 1.0 { settled = Some(t - start); }
    }
    let settled = settled.expect("the marks settle");
    assert!(settled <= amenui::ui::MARKS_SETTLE + 1.5 / 60.0 && amenui::ui::MARKS_SETTLE < 0.08, "settled after {settled} s");
    // Leaving: gone within the settle.
    ui.pointer_move(300.0, 190.0);
    for _ in 0..6 { t += 1.0 / 60.0; ui.tick(t); }
    assert!(ui.marks_frame().is_none());
}

/// A scene that hears the pointer (`on_point`, which drags) with a button and a caption in it.
#[derive(Clone, Debug, PartialEq)]
enum Heard { Pressed, Point(bool) }
fn scene(_: &Vec<Heard>) -> El<Heard> {
    col().key("scene").pad(60.0, 60.0).gap(40.0).width(500.0).height(400.0)
        .on_point(|p| p.map(|p| Heard::Point(p.down)))
        .child(amenui::widgets::button("go", "Go", Heard::Pressed))
        .child(amenui::el::text("a caption that takes no press", 14.0, Paint::TEXT).key("caption"))
}
fn held(log: &[Heard]) -> usize { log.iter().filter(|h| **h == Heard::Point(true)).count() }

#[test]
fn a_button_inside_a_pointer_scene_takes_its_own_press() {
    let mut ui = Ui::new(vec![], scene, |s: &mut Vec<Heard>, m| s.push(m), fonts(), 640, 480, 1.0);
    ui.tick(1.0);
    let b = ui.node("go").unwrap().rect;
    let (x, y) = b.center();
    ui.press(x, y);
    ui.pointer_move(x + 30.0, y + 10.0);
    ui.release(x + 30.0, y + 10.0);
    assert_eq!(ui.state.iter().filter(|h| **h == Heard::Pressed).count(), 1, "the button is pressed: {:?}", ui.state);
    assert_eq!(held(&ui.state), 0, "the scene does not take the button's press");
    assert!(ui.dragging().is_none());
}

#[test]
fn a_pointer_scene_still_drags_from_its_own_area_and_from_what_takes_no_press() {
    let mut ui = Ui::new(vec![], scene, |s: &mut Vec<Heard>, m| s.push(m), fonts(), 640, 480, 1.0);
    ui.tick(1.0);
    let s = ui.node("scene").unwrap().rect;
    let c = ui.node("caption").unwrap().rect;
    for (x, y) in [(s.x + 20.0, s.bottom() - 20.0), c.center()] {
        ui.state.clear();
        ui.press(x, y);
        assert_eq!(ui.dragging(), Some("scene"));
        ui.pointer_move(x + 40.0, y);
        ui.pointer_move(x + 80.0, y);
        ui.release(x + 80.0, y);
        assert!(held(&ui.state) >= 3, "pressed and moved: {:?}", ui.state);
        assert!(!ui.state.contains(&Heard::Pressed));
        assert_eq!(ui.state.last(), Some(&Heard::Point(false)), "the drag ends");
    }
}
/// A square turned 30°: it presses where it is drawn, not where its box was.
struct Turned { hits: u32 }
fn turned(_: &Turned) -> El<()> {
    col().key("root").pad(120.0, 120.0).child(space(80.0, 80.0).key("sq").fill(Paint::INK).radius(0.0).turn(30f32.to_radians()).on_press(()))
}

#[test]
fn a_turned_element_is_pressed_where_it_is_drawn() {
    let mut ui = Ui::new(Turned { hits: 0 }, turned, |s: &mut Turned, _| s.hits += 1, fonts(), 400, 400, 1.0);
    ui.tick(1.0);
    let r = ui.node("sq").unwrap().rect;
    let (cx, cy) = r.center();
    // The drawn square, from the instance's own transform (box offset → screen offset).
    let inst = ui.scene().instances.iter().find(|i| i.xf != [0.0; 4]).copied().expect("a turned instance");
    let m = [[1.0 + inst.xf[0], inst.xf[1]], [inst.xf[2], 1.0 + inst.xf[3]]];
    assert!((m[0][0] - 30f32.to_radians().cos()).abs() < 1e-4, "drawn turned 30°: {m:?}");
    // 4 px inside the drawn top-left corner, and 4 px inside the box's own (now empty) corner.
    let (ox, oy) = (r.x + 4.0 - cx, r.y + 4.0 - cy);
    let drawn = (cx + m[0][0] * ox + m[0][1] * oy, cy + m[1][0] * ox + m[1][1] * oy);
    let empty = (r.x + 4.0, r.y + 4.0);
    ui.press(empty.0, empty.1); ui.release(empty.0, empty.1);
    assert_eq!(ui.state.hits, 0, "the box's empty corner does not press it");
    ui.tick(2.0);
    ui.press(drawn.0, drawn.1); ui.release(drawn.0, drawn.1);
    assert_eq!(ui.state.hits, 1, "its drawn corner does");
}

#[test]
fn after_a_quarter_turn_the_glyph_under_the_pointer_is_the_one_drawn_there() {
    use amenui::recipes::form3d;
    let at = |yaw: f32, at: Option<(f32, f32)>| {
        let mut ui = form3d::ui(fonts(), 720, 640, 1.0);
        ui.state.spin = [yaw, 8.0];
        ui.relayout();
        ui.tick(2.0);
        let r = ui.node("passage").unwrap().rect;
        // The first point near the middle of the sphere that is on a glyph (not between two lines).
        // At yaw 0, the first point near the middle that is on a glyph (not between two lines); after
        // the turn, that same point.
        let (x, y, h) = match at {
            Some((x, y)) => (x, y, ui.glyph_at(x, y).expect("a glyph at the same point after the turn")),
            None => (0..40).find_map(|k| { let (x, y) = (r.x + r.w * 0.5, r.y + r.h * 0.5 + k as f32 - 20.0); ui.glyph_at(x, y).map(|h| (x, y, h)) })
                .expect("a glyph near the middle of the sphere"),
        };
        assert_eq!(h.key, "passage");
        // The glyph it names is drawn there: its instance's ink box holds the point (within a px).
        let node = ui.nodes().iter().position(|n| n.key == "passage").unwrap();
        let g = ui.glyphs().log.iter().find(|g| g.node == node && g.i == h.i).copied().expect("drawn");
        let b = amenui::form::ink_box(&ui.scene().instances[g.inst]);
        assert!(b[0] - 1.0 <= x && x <= b[2] + 1.0 && b[1] - 1.0 <= y && y <= b[3] + 1.0, "yaw {yaw}: glyph {} at {b:?}, pointer {x} {y}", h.i);
        (h.i, (x, y))
    };
    let (a, p) = at(0.0, None);
    let (b, _) = at(90.0, Some(p));
    assert_ne!(a, b, "a quarter turn brings another glyph under the pointer");
}

/// A drawing over the whole window, laid over a button; `pass` says whether the pointer goes through it.
mod pass_page {
    use amenui::el::{El, Paint, canvas, layers, text};
    pub struct S { pub pass: bool, pub pressed: u32 }
    pub fn view(s: &S) -> El<u32> {
        let mut d = amenui::draw::Drawing::new(400.0, 300.0);
        for i in 0..20 { d.dot(20.0 + i as f32 * 18.0, 150.0, 6.0, Paint::ACCENT); }
        layers([
            text("Press", 16.0, Paint::INK).key("button").pad(40.0, 40.0).on_press(1),
            canvas(d).key("drawing").width(400.0).height(300.0).absolute(Some(0.0), Some(0.0), None, None).pass(s.pass),
        ]).key("root")
    }
}

#[test]
fn a_drawing_the_pointer_passes_through_lets_the_button_under_it_take_the_press() {
    let mut drawn = vec![];
    for pass in [true, false] {
        let mut ui = Ui::new(pass_page::S { pass, pressed: 0 }, pass_page::view, |s, m| s.pressed += m, fonts(), 400, 300, 1.0);
        ui.tick(1.0);
        let (x, y) = ui.node("button").expect("button").rect.center();
        ui.pointer_moved(1, amenui::ui::PointerKind::Mouse, x, y);
        ui.pointer_down(1, amenui::ui::PointerKind::Mouse, x, y, false);
        ui.tick(1.05);
        ui.pointer_up(1, amenui::ui::PointerKind::Mouse, x, y);
        ui.tick(1.1);
        assert_eq!(ui.state.pressed, if pass { 1 } else { 0 }, "pass({pass}): the press reaches the button {} times", ui.state.pressed);
        drawn.push(ui.instances_of("drawing"));
    }
    // The drawing itself is drawn the same either way.
    assert!(!drawn[0].is_empty());
    assert_eq!(drawn[0].len(), drawn[1].len());
    assert!(drawn[0].iter().zip(&drawn[1]).all(|(a, b)| a.rect == b.rect && a.paint == b.paint && a.uv == b.uv), "the drawing is drawn the same");
}
