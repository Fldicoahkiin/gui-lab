//! Motes (DESIGN.md §1.22) through the runtime, on a page of their own: a sign dragged over a page of
//! text sheds marks only while it moves, they fall and are gone after their life, none comes within
//! `clear` of a line of text, and nothing is asked of the frame loop when it is still; a flow field
//! moves while its speed is above 0 and stands still (no frames) at 0.
use lab::el::{El, Paint, col, layers, space, text};
use lab::motes::{FlowField, Shed};
use lab::ui::Ui;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone)] enum M { Move(f32, f32) }
struct Page { at: (f32, f32), speed: f32 }
const WORDS: &str = "A short page of text set in two blocks, so the shed marks have lines to keep away from while the sign moves over the margin between them. ";
fn page(s: &Page) -> El<M> {
    let sign = space(60.0, 60.0).radius(30.0).fill(Paint::INK).key("sign").shed(Shed { rate: 6.0, clear: 10.0, ..Shed::default() })
        .on_drag(M::Move).absolute(Some(s.at.0), Some(s.at.1), None, None);
    let body = col().key("text").pad(40.0, 40.0).gap(160.0)
        .child(text(WORDS.repeat(3), 14.0, Paint::INK).max_width(900.0).key("body"))
        .child(text(WORDS.repeat(3), 14.0, Paint::INK).max_width(900.0).key("body2"));
    let ground = space(1200.0, 200.0).key("ground").flow_field(FlowField { speed: s.speed, ..FlowField::default() }).absolute(Some(0.0), Some(420.0), None, None);
    layers([body, ground, sign]).width(1280.0).height(640.0)
}
fn update(s: &mut Page, m: M) { let M::Move(x, y) = m; s.at = (x - 30.0, y - 30.0); }
fn open(speed: f32) -> Ui<Page, M> {
    let mut ui = Ui::new(Page { at: (300.0, 180.0), speed }, page, update, fonts(), 1280, 640, 1.0);
    for i in 1..=60 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
fn run(ui: &mut Ui<Page, M>, secs: f32) -> usize {
    let (mut t, to, mut drawn) = (ui.now(), ui.now() + secs, 0);
    while t < to { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; } ui.frame_drawn(); }
    drawn
}

#[test]
fn a_dragged_sign_sheds_and_a_still_one_does_not() {
    let mut ui = open(0.0);
    run(&mut ui, 1.0);
    assert!(ui.motes().is_empty(), "a still sign sheds nothing");
    assert!(!ui.motes_moving());
    let (x, y) = ui.node("sign").expect("sign").rect.center();
    let mut t = ui.now();
    ui.press(x, y);
    let mut most = 0;
    for i in 1..=30 { t += 1.0 / 60.0; ui.pointer_move(x + i as f32 * 6.0, y + i as f32 * 1.5); ui.tick(t); ui.frame_drawn(); most = most.max(ui.motes().len()); }
    ui.release(x + 180.0, y + 45.0);
    assert!(most >= 4, "dragging 185 px shed {most} marks");
    let y0: f32 = ui.motes().iter().map(|m| m.y).sum::<f32>() / ui.motes().len().max(1) as f32;
    run(&mut ui, 0.3);
    if !ui.motes().is_empty() { let y1: f32 = ui.motes().iter().map(|m| m.y).sum::<f32>() / ui.motes().len() as f32; assert!(y1 > y0, "they fall: {y0} → {y1}"); }
    run(&mut ui, 2.0);
    assert!(ui.motes().is_empty(), "{} marks left", ui.motes().len());
    let t = ui.now();
    assert!(!ui.needs_frame(t) && !ui.motes_moving(), "still after the marks are gone");
}

#[test]
fn shed_marks_keep_clear_of_the_text() {
    let mut ui = open(0.0);
    let (x, y) = ui.node("sign").expect("sign").rect.center();
    let mut t = ui.now();
    ui.press(x, y);
    let mut seen = 0;
    for i in 1..=90 {
        t += 1.0 / 60.0;
        let k = i as f32;
        ui.pointer_move(x + (k * 0.2).sin() * 160.0, y + (k * 0.13).cos() * 40.0);
        ui.tick(t);
        ui.frame_drawn();
        let mut lines = vec![];
        for key in ["body", "body2"] { lines.extend(ui.line_boxes(key)); }
        for m in ui.motes() {
            seen += 1;
            for l in &lines {
                let d = lab::motes::distance(m.x, m.y, [l.x, l.y, l.right(), l.bottom()]);
                assert!(d > m.size, "a mark at ({:.1}, {:.1}) is {d:.1} px from a line of text", m.x, m.y);
            }
        }
    }
    ui.release(x, y);
    assert!(seen > 0, "the drag shed something");
}

#[test]
fn a_flow_field_drifts_only_with_speed() {
    let marks = |ui: &Ui<Page, M>| -> Vec<[f32; 2]> { ui.scene().instances.iter().filter(|i| i.resp == [0.0, -1.0, 0.0, 0.0] && i.rect[2] <= 3.0).map(|i| [i.rect[0], i.rect[1]]).collect() };
    let mut ui = open(18.0);
    assert!(ui.motes_moving(), "the flow field moves with speed");
    let before = marks(&ui);
    assert!(before.len() > 50, "{} ground marks", before.len());
    run(&mut ui, 0.5);
    assert!(before != marks(&ui), "the marks moved");
    let mut ui = open(0.0);
    assert!(!ui.motes_moving());
    run(&mut ui, 1.0);
    assert_eq!(run(&mut ui, 2.0), 0, "a still field draws nothing");
}
