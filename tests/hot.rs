//! The work a frame does on a page at rest and under a moving pointer: what is the same from one paint to
//! the next is not worked out again, and the pointer moves a frame brings together cost one layout.
use amenui::el::{El, Paint, Point, Reveal, col, text};
use amenui::ui::{PointerKind, Ui};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const PROSE: &str = "Words come in whole: a pose fades each word as one, and the edge of the window takes a word away entire, never half of it. ";

#[derive(Clone, Default)]
struct S { n: u32, at: Vec<(f32, f32, f32)> }
#[derive(Clone)]
enum Msg { At(Option<Point>) }
fn view(s: &S) -> El<Msg> {
    col().key("page").fill_width().fill_height().pad(12.0, 12.0).gap(8.0)
        .child(text(format!("{} moves", s.at.len()), 14.0, Paint::INK).key("count"))
        .child(text(format!("frame {}", s.n), 14.0, Paint::INK).key("n"))
        .child(text(PROSE.repeat(8), 14.0, Paint::TEXT).key("prose").fill_width().reveal_by(Reveal::Word).on_point(|p| Some(Msg::At(p))))
}
fn update(s: &mut S, m: Msg) { match m { Msg::At(Some(p)) => s.at.push((p.x, p.y, p.t)), Msg::At(None) => {} } }
fn mk() -> Ui<S, Msg> { let mut ui = Ui::new(S::default(), view, update, fonts(), 480, 640, 1.0); ui.tick(1.0); ui }

#[test]
fn a_texts_words_are_found_once_over_many_paints() {
    let mut ui = mk();
    let first = ui.counters().reveals;
    assert_eq!(first, 1, "the one text cut into words");
    // Many paints of a page whose text by words does not change (another text does).
    for k in 0..40 { ui.state.n = k; ui.relayout(); ui.tick(1.0 + k as f32 / 60.0); }
    assert!(ui.counters().layouts >= 40);
    assert_eq!(ui.counters().reveals, first, "the words were found again for an unchanged text");
    // A new width cuts it again, once.
    ui.resize(400, 640, 1.0);
    for k in 0..5 { ui.state.n = 100 + k; ui.relayout(); }
    assert_eq!(ui.counters().reveals, first + 1);
}

#[test]
fn the_pointer_moves_of_one_frame_cost_one_layout() {
    let mut ui = mk();
    ui.coalesce(true);
    let prose = ui.node("prose").expect("laid out").rect;
    let before = ui.counters().layouts;
    // A host ticks before each event; eight moves come in one frame, each telling the state.
    let t0 = 2.0;
    for k in 0..8 {
        let t = t0 + k as f32 * 0.002;
        ui.tick(t);
        ui.event_time(t);
        ui.pointer_moved(1, PointerKind::Mouse, prose.x + 20.0 + 10.0 * k as f32, prose.y + 10.0);
    }
    assert_eq!(ui.state.at.len(), 8, "every move reached the state");
    assert_eq!(ui.counters().layouts, before, "laid out before the frame");
    assert!(ui.needs_frame(t0 + 0.016));
    // The frame: one layout for all eight.
    ui.settle();
    assert_eq!(ui.counters().layouts, before + 1);
    assert_eq!(ui.node("count").and_then(|n| n.text()), Some("8 moves"));
    // Each move has its own time, so a speed is measured between them.
    let ts: Vec<f32> = ui.state.at.iter().map(|a| a.2).collect();
    assert!(ts.windows(2).all(|w| w[1] > w[0]), "the moves' times: {ts:?}");
    // A press lays out what the moves before it left waiting, then acts.
    ui.pointer_moved(1, PointerKind::Mouse, prose.x + 30.0, prose.y + 12.0);
    ui.pointer_down(1, PointerKind::Mouse, prose.x + 30.0, prose.y + 12.0, false);
    // (The press tells the state too, and lays out at once.)
    let shown = format!("{} moves", ui.state.at.len());
    assert!(ui.state.at.len() >= 10);
    assert_eq!(ui.node("count").and_then(|n| n.text()), Some(shown.as_str()));
}

#[test]
fn a_pointers_speed_is_measured_between_the_moves_own_times() {
    let mut p = amenui::experiment::Pointer::new(0.0, 0.0, 200.0, 20.0);
    let at = |x: f32, t: f32| Some(Point { x, y: 0.0, down: false, glyph: None, z: 0.0, t });
    // Two moves handled in the same frame (the same `now`), 8 ms apart when they happened.
    p.hear(at(0.0, 1.000), 1.016);
    p.hear(at(8.0, 1.008), 1.016);
    assert!((p.vx - 1000.0).abs() < 1.0, "speed {} px/s", p.vx);
}

#[test]
fn meshes_go_into_vertex_buffers_no_larger_than_the_cap() {
    let mesh = |n: usize, v: f32| vec![[v; 6]; n];
    let (chunks, ranges) = amenui::render::mesh_chunks([mesh(30, 1.0), mesh(12, 2.0), mesh(75, 3.0), mesh(6, 4.0)], 32);
    assert!(chunks.iter().all(|c| c.len() <= 30), "{:?}", chunks.iter().map(Vec::len).collect::<Vec<_>>());
    // Each mesh's pieces are its vertices, in order, in whole triangles.
    for (k, parts) in ranges.iter().enumerate() {
        let v = (k + 1) as f32;
        let n: u32 = parts.iter().map(|p| p.2).sum();
        assert_eq!(n as usize, [30, 12, 75, 6][k]);
        for &(c, first, count) in parts {
            assert_eq!(count % 3, 0);
            assert!(chunks[c as usize][first as usize..(first + count) as usize].iter().all(|x| x[0] == v));
        }
    }
    // A mesh that fits a buffer is not cut.
    assert_eq!(ranges[1].len(), 1);
    assert_eq!(ranges[3].len(), 1);
}
