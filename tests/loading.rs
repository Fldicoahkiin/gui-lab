//! Loading states (DESIGN.md §1.23) through the runtime, with the loading recipe: the static parts draw
//! at once and the rows not at all for `skeleton_after_ms`; then each row is its own skeleton — bars as
//! wide as its measured lines, a block for its picture — in exactly the boxes the content then takes
//! (zero layout shift); the rows swap in `swap_stagger_ms` apart; at rest nothing asks for frames.
use lab::recipes::loading::{App, Msg, ROWS, ui};
use lab::ui::Ui;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn boxes(ui: &Ui<App, Msg>) -> Vec<(String, [f32; 4])> {
    ui.nodes().iter().filter(|n| n.key.starts_with("row")).map(|n| (n.key.clone(), [n.rect.x, n.rect.y, n.rect.w, n.rect.h])).collect()
}

#[test]
fn skeleton_then_content_in_the_same_boxes() {
    let mut ui = ui(fonts(), 640, 600, 1.0);
    let mut t = 0.0;
    ui.tick(t);
    let (x, y) = ui.node("reload").unwrap().rect.center();
    ui.press(x, y);
    ui.release(x, y);
    let step = |ui: &mut Ui<App, Msg>, t: &mut f32, to: f32| { while *t < to - 1e-4 { *t += 1.0 / 60.0; ui.tick(*t); ui.frame_drawn(); } };
    step(&mut ui, &mut t, 0.2);
    assert!(ui.instances_of("row0").is_empty(), "no skeleton before skeleton_after_ms");
    assert!(!ui.instances_of("reload").is_empty(), "the static parts are drawn at once");
    step(&mut ui, &mut t, 0.5);
    let skel = boxes(&ui);
    // Each title is bars as wide as its lines.
    for i in 0..ROWS.len() {
        let k = format!("row{i}/title");
        let lines = ui.line_boxes(&k);
        let bars = ui.instances_of(&k);
        assert_eq!(bars.len(), lines.len(), "{k}: one bar a line");
        for (b, l) in bars.iter().zip(&lines) { assert!((b.rect[2] - l.w).abs() <= 1.0, "{k}: bar {} for a line of {}", b.rect[2], l.w); }
        assert_eq!(ui.instances_of(&format!("row{i}/pic")).len(), 1);
    }
    // The data comes at 0.9 s; row 0 at once, row 4 four staggers later.
    step(&mut ui, &mut t, 0.95);
    let first = ui.instances_of("row0/title").len();
    assert!(first > ui.line_boxes("row0/title").len(), "row 0 shows its glyphs");
    assert_eq!(ui.instances_of("row4/title").len(), ui.line_boxes("row4/title").len(), "row 4 is still a skeleton");
    step(&mut ui, &mut t, 1.5);
    assert_eq!(skel, boxes(&ui), "zero layout shift between skeleton and content");
    assert!(!ui.needs_frame(t + 0.5), "at rest nothing asks for frames");
}
