//! Gesture physics through the runtime (DESIGN.md §1.8.4): simulated drags on the gesture recipe's
//! bottom sheet and floating box, with the clock stepped at 60 Hz, so every run is the same. A slow drag
//! below the threshold returns; a fast flick dismisses; a pull past the top is rubber-banded and never
//! exceeds its bound; the corner is chosen by the projection, not the release point; a finger pulling
//! the sheet's list down at its top drags the sheet (the hand-off); at rest nothing asks for a frame.
use amenui::recipes::gesture::{App, Msg, ui};
use amenui::ui::{PointerKind, Ui};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

struct Run { ui: Ui<App, Msg>, t: f32 }
impl Run {
    fn new() -> Self { let mut ui = ui(fonts(), 360, 600, 1.0); ui.tick(0.0); Run { ui, t: 0.0 } }
    fn frame(&mut self) { self.t += 1.0 / 60.0; self.ui.tick(self.t); self.ui.frame_drawn(); }
    fn settle(&mut self) { for _ in 0..600 { self.frame(); if !self.ui.needs_frame(self.t) { return; } } panic!("still moving after 10 s"); }
    fn open(&mut self) {
        let (x, y) = self.ui.node("open").expect("open").rect.center();
        self.ui.press(x, y);
        self.ui.release(x, y);
        self.settle();
    }
    fn panel_top(&self) -> f32 { self.ui.node("sheet/panel").map_or(f32::NAN, |n| n.rect.y) }
    /// A mouse drag from (x, y) by (dx, dy) over `frames` frames.
    fn drag(&mut self, x: f32, y: f32, dx: f32, dy: f32, frames: usize) {
        self.ui.press(x, y);
        for i in 1..=frames {
            self.frame();
            let k = i as f32 / frames as f32;
            self.ui.pointer_move(x + dx * k, y + dy * k);
        }
        self.ui.release(x + dx, y + dy);
    }
}

#[test]
fn the_sheet_presents_returns_dismisses_and_rests() {
    let mut r = Run::new();
    assert!(r.ui.node("sheet/panel").is_none());
    r.open();
    let low = 600.0 * 0.45;
    assert!((r.panel_top() - (600.0 - low)).abs() < 0.5, "rests on the lower detent: top {}", r.panel_top());
    assert!(!r.ui.needs_frame(r.t + 0.5), "at rest nothing asks for a frame");
    // Slow: 80 px down over 1.2 s — its projection stays above half the detent, so it returns.
    let (hx, hy) = r.ui.node("sheet/handle").unwrap().rect.center();
    r.drag(hx, hy, 0.0, 80.0, 72);
    assert!(r.ui.state.sheet.is_open());
    r.settle();
    assert!((r.panel_top() - (600.0 - low)).abs() < 0.5, "returned: top {}", r.panel_top());
    // Fast: 60 px in three frames (1200 px/s) — a shorter drag, but its projection is far below.
    let (hx, hy) = r.ui.node("sheet/handle").unwrap().rect.center();
    r.drag(hx, hy, 0.0, 60.0, 3);
    assert!(!r.ui.state.sheet.is_open(), "a fast flick dismisses");
    assert!(r.ui.state.sheet.velocity() < -600.0, "the flick's velocity carries on: {}", r.ui.state.sheet.velocity());
    r.settle();
    assert!(r.ui.node("sheet/panel").is_none(), "gone once it has sprung away");
    assert!(!r.ui.needs_frame(r.t + 0.5));
}

#[test]
fn a_pull_past_the_top_is_banded_and_bounded() {
    let mut r = Run::new();
    r.open();
    let (hx, hy) = r.ui.node("sheet/handle").unwrap().rect.center();
    r.drag(hx, hy, 0.0, -120.0, 6);
    r.settle();
    let top = 600.0 - 600.0 * 0.9;
    assert!((r.panel_top() - top).abs() < 0.5, "a flick up goes to the top detent: {}", r.panel_top());
    // Now 2000 px further up: the sheet follows less and less and never passes top − height.
    let (hx, hy) = r.ui.node("sheet/handle").unwrap().rect.center();
    r.ui.press(hx, hy);
    let mut last = r.panel_top();
    for i in 1..=100 {
        r.frame();
        r.ui.pointer_move(hx, hy - 20.0 * i as f32);
        let y = r.panel_top();
        assert!(y <= last + 1e-3 && y > top - 600.0, "step {i}: {y}");
        last = y;
    }
    assert!(top - last < 2000.0 * 0.55, "banded, not followed: {}", top - last);
    r.ui.release(hx, hy - 2000.0);
    r.settle();
    assert!((r.panel_top() - top).abs() < 0.5, "springs back to the top detent");
}

#[test]
fn a_finger_hands_the_list_over_to_the_sheet_at_its_top() {
    let mut r = Run::new();
    r.open();
    let (hx, hy) = r.ui.node("sheet/handle").unwrap().rect.center();
    r.drag(hx, hy, 0.0, -120.0, 6);
    r.settle();
    let top = r.panel_top();
    let (x, y) = r.ui.node("item3").unwrap().rect.center();
    // Up first: the list scrolls, the sheet stays.
    r.ui.pointer_down(1, PointerKind::Touch, x, y, false);
    for i in 1..=10 { r.frame(); r.ui.pointer_moved(1, PointerKind::Touch, x, y - 6.0 * i as f32); }
    assert!(r.ui.scroll_offset("sheet/body") > 30.0, "the list scrolled: {}", r.ui.scroll_offset("sheet/body"));
    assert!((r.panel_top() - top).abs() < 0.5);
    // Back down past its top: the list stops at 0 and the sheet comes down with the finger.
    for i in 1..=30 { r.frame(); r.ui.pointer_moved(1, PointerKind::Touch, x, y - 60.0 + 6.0 * i as f32); }
    assert_eq!(r.ui.scroll_offset("sheet/body"), 0.0);
    assert!(r.panel_top() > top + 60.0, "the sheet was dragged down: {} from {top}", r.panel_top());
    r.ui.pointer_up(1, PointerKind::Touch, x, y + 120.0);
    r.settle();
    assert!(r.ui.state.sheet.is_open());
}

#[test]
fn the_floating_box_goes_where_the_throw_points() {
    let mut r = Run::new();
    // It starts top right (corner 1). A slow drag 60 px left stays nearer the right: back right.
    let b = r.ui.node("pip").unwrap().rect;
    let (x, y) = b.center();
    r.drag(x, y, -60.0, 0.0, 120);
    assert_eq!(r.ui.state.pip.corner(), 1);
    r.settle();
    assert_eq!(r.ui.node("pip").unwrap().rect, b);
    // A flick of 60 px left in three frames lets go on the right half, and goes to the left corner.
    r.drag(x, y, -60.0, 0.0, 3);
    let at = r.ui.state.pip.at().0;
    assert!(at > 120.0, "let go on the right half (the middle is 120): {at}");
    assert_eq!(r.ui.state.pip.corner(), 0, "the projection, not the release point, picks the corner");
    r.settle();
    assert!((r.ui.node("pip").unwrap().rect.x - 16.0).abs() < 0.5);
    assert!(!r.ui.needs_frame(r.t + 0.5));
}
