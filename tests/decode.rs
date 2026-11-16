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

/// A panel filled in the ground's own tone is still an outline: at the finest tier its hollow interior
/// is covered too, not only the text and the border (round 16).
#[test]
fn the_panel_interior_is_covered() {
    let mut ui = lab::ui::Ui::new(S { open: false }, view, update, fonts(), 800, 600, 1.0);
    let mut t = 0.0;
    let mut go = |ui: &mut lab::ui::Ui<S, M>, secs: f32| { let end = t + secs; while t < end { t += 1.0 / 60.0; ui.tick(t); } };
    go(&mut ui, 1.0);
    ui.state.open = true;
    ui.relayout();
    // Just before the content shows (C = 495 ms): every tier is on.
    go(&mut ui, 0.47);
    let card = ui.node("d/card").expect("card").rect;
    let body = ui.node("d/body").expect("body").rect;
    // A point inside the card with no ink of its own: right of the short body text, above the bottom.
    let (px, py) = (card.right() - 14.0, card.bottom() - 10.0);
    assert!(px > body.x + 90.0);
    let blocks: Vec<_> = ui.scene().instances.iter().filter(|i| i.paint[3] == lab::scene::SHAPE && i.resp[1] < 0.0 && i.rect[2] == i.rect[3] && i.rect[2] <= 32.0).collect();
    let bb = blocks.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |m, b| [m[0].min(b.rect[0]), m[1].min(b.rect[1]), m[2].max(b.rect[0] + b.rect[2]), m[3].max(b.rect[1] + b.rect[3])]);
    eprintln!("{} blocks within {bb:?}, card {card:?}", blocks.len());
    assert!(blocks.iter().any(|b| px >= b.rect[0] && px < b.rect[0] + b.rect[2] && py >= b.rect[1] && py < b.rect[1] + b.rect[3]), "the interior at ({px}, {py}) is open");
}
