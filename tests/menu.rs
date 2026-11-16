//! The nested menu's safe zone through the runtime (DESIGN.md §1.8.5): pointer moves at 60 Hz over the
//! submenu recipe. A diagonal path across sibling rows toward the open submenu keeps it open; a path
//! outside the triangle switches rows after `submenu_delay_ms`; nothing asks for frames while nothing
//! waits; the overlay draws the triangle in the annotation layer.
use lab::recipes::submenu::{App, Msg, ui};
use lab::ui::Ui;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
struct Run { ui: Ui<App, Msg>, t: f32 }
impl Run {
    fn new() -> Self { let mut ui = ui(fonts(), 800, 600, 1.0); ui.tick(0.0); Run { ui, t: 0.0 } }
    fn frame(&mut self) { self.t += 1.0 / 60.0; self.ui.tick(self.t); self.ui.frame_drawn(); }
    fn origin(&self) -> (f32, f32) { let r = self.ui.node("menu").unwrap().rect; (r.x, r.y) }
    /// Move through menu-local points, one a frame.
    fn walk(&mut self, pts: &[(f32, f32)]) { let (ox, oy) = self.origin(); for &(x, y) in pts { self.frame(); self.ui.pointer_move(ox + x, oy + y); } }
    fn rest(&mut self, secs: f32) { let end = self.t + secs; while self.t < end { self.frame(); } }
    fn hot(&self, l: usize) -> Option<usize> { self.ui.state.menu.hot(l) }
}

#[test]
fn a_diagonal_path_keeps_the_submenu_open() {
    let mut r = Run::new();
    r.walk(&[(100.0, 18.0)]);
    r.rest(0.15);
    assert!(r.ui.node("menu/m1").is_some(), "the first row's submenu opened after the delay");
    let path: Vec<(f32, f32)> = (1..=10).map(|i| (100.0 + 9.5 * i as f32, 18.0 + 7.0 * i as f32)).collect();
    r.walk(&path);
    assert_eq!(r.hot(0), Some(0), "rows crossed inside the triangle did not take over");
    // The overlay is on: the triangle is in the annotation layer.
    assert!(!r.ui.scene().annot.is_empty(), "the safe zone is drawn");
    r.walk(&[(210.0, 92.0)]);
    r.rest(0.5);
    assert_eq!((r.hot(0), r.hot(1)), (Some(0), Some(2)));
    assert!(!r.ui.needs_frame(r.t + 0.1), "nothing waits, nothing draws");
}

#[test]
fn a_path_outside_switches_after_the_delay() {
    let mut r = Run::new();
    r.walk(&[(100.0, 18.0)]);
    r.rest(0.15);
    r.walk(&[(100.0, 40.0), (100.0, 60.0), (100.0, 78.0)]);
    assert_eq!(r.hot(0), Some(0));
    assert!(r.ui.next_wake(r.t).is_some(), "a waiting switch wakes the page");
    r.rest(0.14);
    assert_eq!(r.hot(0), Some(2), "the row under the pointer took over");
    assert!(r.ui.node("menu/m1").is_some());
}
