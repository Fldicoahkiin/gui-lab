//! The decode transition (DESIGN.md §1.28) through the runtime: a dialog appears under blocks, its
//! content shows only after every tier is on, nothing is left at rest, and closing plays it backwards.
use lab::decode::Decode;
use lab::el::{El, Paint, col, text};
use lab::widgets::dialog;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
#[derive(Clone)] enum M { Open(bool) }
struct S { open: bool }
fn view(s: &S) -> El<M> {
    col().fill_width().fill_height().child(text("page", 14.0, Paint::INK))
        .child(dialog("d", s.open, "Title", 320.0, 0.35, M::Open(false), text("body text", 14.0, Paint::INK).key("d/body")).decode(Decode::default()))
}
fn update(s: &mut S, m: M) { let M::Open(o) = m; s.open = o; }

#[test]
fn a_dialog_decodes_in_and_out() {
    let mut ui = lab::ui::Ui::new(S { open: false }, view, update, fonts(), 800, 600, 1.0);
    let mut t = 0.0;
    let mut go = |ui: &mut lab::ui::Ui<S, M>, secs: f32| { let end = t + secs; while t < end { t += 1.0 / 60.0; ui.tick(t); } };
    go(&mut ui, 1.0);
    let closed = ui.scene().instances.len();
    ui.state.open = true;
    ui.relayout();
    go(&mut ui, 0.2);
    let mid = ui.scene().instances.len();
    let shown_body = ui.scene().instances.iter().filter(|i| i.paint[3] == 0.0 && i.paint[2] > 0.0).count();
    assert!(mid > closed + 50, "blocks cover it: {mid} vs {closed}");
    go(&mut ui, 1.0);
    let open = ui.scene().instances.len();
    let glyphs_open = ui.scene().instances.iter().filter(|i| i.paint[3] == 0.0 && i.paint[2] > 0.0).count();
    assert!(glyphs_open > shown_body, "the content shows after: {glyphs_open} > {shown_body}");
    assert!(open < mid, "no blocks left at rest: {open} < {mid}");
    // Closing: blocks again, then nothing of it.
    ui.state.open = false;
    ui.relayout();
    go(&mut ui, 0.3);
    assert!(ui.scene().instances.len() > closed + 50, "it goes under blocks");
    go(&mut ui, 1.0);
    assert_eq!(ui.scene().instances.len(), closed, "gone, nothing left");
}
