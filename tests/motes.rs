//! Motes (DESIGN.md §1.22) through the runtime: the zine's sign sheds marks only while it is dragged,
//! they fall and are gone after their life, and nothing is asked of the frame loop when it is still;
//! the monitor's flow field moves while the chapter plays and stands still (no frames) when paused.
use lab::gallery::film::{self, FMsg};
use lab::gallery::{self, GalleryUi, Msg};

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn open(ch: usize, play: bool) -> GalleryUi {
    let mut ui = gallery::new_ui(fonts(), 1280, 640, 1.0);
    ui.pointer_leave();
    assert!(ui.state.route("film"));
    if ui.state.film.playing != play { gallery::update(&mut ui.state, Msg::Film(FMsg::Play)); }
    film::go(&mut ui.state, ch, None);
    ui.relayout();
    for i in 1..=180 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
fn run(ui: &mut GalleryUi, secs: f32) -> usize {
    let mut drawn = 0;
    let to = ui.now() + secs;
    let mut t = ui.now();
    while t < to { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; } ui.frame_drawn(); }
    drawn
}

#[test]
fn a_dragged_sign_sheds_and_a_still_one_does_not() {
    let mut ui = open(6, false);
    run(&mut ui, 2.0);
    assert!(ui.motes().is_empty(), "a still sign sheds nothing");
    assert!(!ui.motes_moving());
    let (x, y) = ui.node("film/c06/sign").expect("sign").rect.center();
    let mut t = ui.now();
    ui.press(x, y);
    let mut most = 0;
    for i in 1..=30 { t += 1.0 / 60.0; ui.pointer_move(x + i as f32 * 6.0, y + i as f32 * 2.0); ui.tick(t); ui.frame_drawn(); most = most.max(ui.motes().len()); }
    ui.release(x + 180.0, y + 60.0);
    assert!(most >= 10, "dragging 190 px shed {most} marks");
    // They fall: below where they started, a moment later.
    let y0: f32 = ui.motes().iter().map(|m| m.y).sum::<f32>() / ui.motes().len() as f32;
    run(&mut ui, 0.3);
    if !ui.motes().is_empty() { let y1: f32 = ui.motes().iter().map(|m| m.y).sum::<f32>() / ui.motes().len() as f32; assert!(y1 > y0, "they fall: {y0} → {y1}"); }
    // Gone after their life, and then the page is still.
    run(&mut ui, 2.0);
    assert!(ui.motes().is_empty(), "{} marks left", ui.motes().len());
    let t = ui.now();
    assert!(!ui.needs_frame(t) && !ui.motes_moving(), "still after the marks are gone");
}

#[test]
fn the_monitor_ground_drifts_only_while_it_plays() {
    let mut ui = open(8, true);
    assert!(ui.motes_moving(), "the flow field moves while the chapter plays");
    let before: Vec<_> = ui.scene().instances.iter().filter(|i| i.resp == [0.0, -1.0, 0.0, 0.0] && i.rect[2] <= 3.0).map(|i| [i.rect[0], i.rect[1]]).collect();
    assert!(before.len() > 50, "{} ground marks", before.len());
    run(&mut ui, 0.5);
    let after: Vec<_> = ui.scene().instances.iter().filter(|i| i.resp == [0.0, -1.0, 0.0, 0.0] && i.rect[2] <= 3.0).map(|i| [i.rect[0], i.rect[1]]).collect();
    assert!(before != after, "the marks moved");
    // Paused: the marks stand still and the chapter asks for no frames.
    let mut ui = open(8, false);
    assert!(!ui.motes_moving());
    run(&mut ui, 1.0);
    let drawn = run(&mut ui, 2.0);
    assert_eq!(drawn, 0, "a paused monitor drew {drawn} frames");
}
