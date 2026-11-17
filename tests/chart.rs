//! The dot chart through the runtime (DESIGN.md §1.25): the field's dots are as big as their coverage
//! says, the pointer scrubs a marker whose label is the value there, a control point dragged on the
//! axis raises the curve and the dots grow under it, live.
use amenui::recipes::chart::{App, Msg, ui};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[test]
fn scrub_and_drag() {
    let mut ui: Ui<App, Msg> = ui(fonts(), 800, 400, 1.0);
    ui.tick(0.0);
    let c = ui.state.chart.clone();
    // Dot sizes follow the coverage.
    for (x, r) in [(5, 0), (20, 6), (40, 12)] {
        let n = ui.node(&format!("chart/d{x}.{r}")).unwrap();
        // Layout rounds boxes to whole pixels.
        assert!((n.rect.w - c.dot(c.coverage(x, r)).0).abs() <= 0.5, "dot {x},{r}: {} vs {}", n.rect.w, c.dot(c.coverage(x, r)).0);
    }
    // Scrubbing: the marker at the column under the pointer, labelled with its value.
    let b = ui.node("chart").unwrap().rect;
    ui.pointer_move(b.x + 20.5 * c.pitch, b.y + 30.0);
    assert_eq!(ui.state.chart.hover(), Some(20));
    let label = ui.export_tree().find("chart/value").and_then(|n| n.text.clone()).expect("label");
    assert_eq!(label, format!("{:.0}", c.value(20) * c.max));
    let m = ui.node("chart/marker").unwrap().rect;
    assert!((m.x - (b.x + 20.5 * c.pitch)).abs() <= 1.0);
    // Drag knot 1 up 50 px: the curve there rises and its column's dots grow.
    let k = ui.node("chart/knot1").unwrap().rect;
    let col = ((c.knot_x(1) / c.pitch) as usize).min(c.cols - 1);
    let r = (c.value(col) * c.rows as f32) as usize;
    let before = ui.node(&format!("chart/d{col}.{r}")).unwrap().rect.w;
    ui.press(k.center().0, k.center().1);
    ui.pointer_move(k.center().0, k.center().1 - 50.0);
    ui.release(k.center().0, k.center().1 - 50.0);
    assert!(ui.state.chart.value(col) > c.value(col) + 0.2, "the curve rose");
    assert!(ui.node(&format!("chart/d{col}.{r}")).unwrap().rect.w > before, "the dot grew");
    ui.pointer_leave();
    assert!(ui.node("chart/marker").is_none(), "leaving takes the marker away");
}
