//! The nested menu's safe zone through the runtime (DESIGN.md §1.8.5): pointer moves at 60 Hz over the
//! submenu recipe. A diagonal path across sibling rows toward the open submenu keeps it open; a path
//! outside the triangle switches rows after `submenu_delay_ms`; nothing asks for frames while nothing
//! waits; the overlay draws the triangle in the annotation layer.
use amenui::recipes::submenu::{App, Msg, ui};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
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

/// A page with a button outside and a container that is a preview of a sheet and a context menu, or an
/// empty container of the same size (`empty`).
mod preview {
    use amenui::el::{El, Paint, col, text};
    use amenui::widgets::{button, context_menu, sheet};
    #[derive(Clone, Debug, PartialEq)]
    pub enum M { Out, Close, Pick(usize), Active(usize) }
    pub struct S { pub empty: bool, pub got: Vec<M>, pub shown: bool }
    pub fn view(s: &S) -> El<M> {
        let mut page = col().key("page").fill_width().fill_height().pad(24.0, 24.0).gap(16.0)
            .child(text("Components", 18.0, Paint::INK)).child(button("outside", "Outside", M::Out));
        let boxed = col().key("pv").width(420.0).height(260.0).border(1.0, Paint::FAINT);
        let boxed = if s.empty || !s.shown { boxed } else {
            boxed.preview(true)
                .child(col().key("pv/sheet-box").height(150.0).fill_width().child(sheet("pv-sheet", true, "Filters", 200.0, M::Close, text("Body", 13.0, Paint::TEXT))))
                .child(col().key("pv/menu-box").height(100.0).fill_width().child(context_menu("pv-menu", Some((20.0, 10.0)), &["Open", "Rename"], 0, M::Pick, M::Active, M::Close)))
        };
        page = page.child(boxed);
        page
    }
    pub fn update(s: &mut S, m: M) { s.got.push(m); }
}

#[test]
fn a_preview_draws_modal_components_in_place_without_their_layer() {
    use preview::{M, S, update, view};
    let mk = |empty: bool| { let mut ui = Ui::new(S { empty, got: vec![], shown: true }, view, update, fonts(), 640, 480, 1.0); ui.tick(2.0); ui };
    let ui = mk(false);
    let pv = ui.node("pv").unwrap().rect;
    for k in ["pv-sheet/panel", "pv-menu/menu"] {
        let r = ui.node(k).unwrap_or_else(|| panic!("{k} is drawn")).rect;
        assert!(r.x >= pv.x - 0.5 && r.y >= pv.y - 0.5 && r.right() <= pv.right() + 0.5 && r.bottom() <= pv.bottom() + 0.5, "{k} {r:?} is inside the preview {pv:?}");
    }
    assert!(ui.a11y().iter().all(|n| !n.modal), "nothing in the tree is modal");
    // The focus is where it was when the preview came.
    let mut ui = Ui::new(S { empty: false, got: vec![], shown: false }, view, update, fonts(), 640, 480, 1.0);
    ui.tick(1.0);
    ui.focus_key("outside");
    ui.state.shown = true;
    ui.relayout();
    assert_eq!(ui.focus(), Some("outside"), "a preview takes no focus");
    // A press outside reaches the button, and no dismiss is sent; nor is one on Esc.
    let (x, y) = ui.node("outside").unwrap().rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.key(amenui::el::Key::Escape, false);
    assert!(ui.state.got.contains(&M::Out), "the button outside got its press: {:?}", ui.state.got);
    assert!(!ui.state.got.contains(&M::Close), "no dismiss: {:?}", ui.state.got);
    // Outside the preview's box, the page is the page with an empty box of that size.
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(640, 480) else { eprintln!("no GPU adapter here: skipped"); return };
    let look = amenui::style::Look { material: amenui::style::Material::Plain, ..amenui::style::Look::default() };
    let (mut a, mut b) = (mk(false), mk(true));
    let (fa, fb) = (o.render(&mut a, &look), o.render(&mut b, &look));
    let mut diff = 0;
    for y in 0..480u32 { for x in 0..640u32 {
        if (x as f32) >= pv.x && (x as f32) < pv.right() && (y as f32) >= pv.y && (y as f32) < pv.bottom() { continue; }
        let i = ((y * 640 + x) * 4) as usize;
        if fa[i..i + 3] != fb[i..i + 3] { diff += 1; }
    } }
    assert_eq!(diff, 0, "{diff} px outside the preview differ from the page with an empty box");
}
