//! Incremental update and long lists (DESIGN.md §1.13), without a GPU: the taffy tree is kept and only
//! re-run when a size changed; unchanged text is never shaped again; the wheel moves things without
//! layout or shaping; a virtual list of 10,000 rows builds only what it shows, with the whole list's
//! scroll extent, keyboard navigation and scrolling a row into view; lines a clip hides are not made
//! into instances.
use amenui::el::{El, Paint, col, space, text};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

struct Paints { on: bool }
fn paints(s: &Paints) -> El<()> {
    col().key("root").pad(20.0, 20.0).gap(8.0)
        .child(text("a label that stays", 14.0, Paint::TEXT).key("label"))
        .child(space(120.0, 30.0).key("swatch").fill(if s.on { Paint::ACCENT } else { Paint::DIM }).on_press(()))
}

#[test]
fn a_change_of_paint_only_skips_layout() {
    let mut ui = Ui::new(Paints { on: false }, paints, |s: &mut Paints, _| s.on = !s.on, fonts(), 400, 300, 1.0);
    let (x, y) = ui.node("swatch").unwrap().rect.center();
    let c0 = ui.counters();
    ui.press(x, y);
    ui.release(x, y);
    let c1 = ui.counters();
    assert!(ui.state.on);
    assert_eq!(c1.views, c0.views + 1, "a new view");
    assert_eq!((c1.layouts, c1.shaped), (c0.layouts, c0.shaped), "nothing changed size: no layout, no shaping");
    assert!(c1.layouts_skipped > c0.layouts_skipped);
}
