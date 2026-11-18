//! Acceptance for components and the gallery, without a GPU: drag capture, text editing, the modal layer (focus trap,
//! dismissal, beneath held loose), tabs / list / slider / number scrub, disabled controls, horizontal
//! scroll, commits, and the gallery's hash routes.
use amenui::el::{Drag, El, Key, Paint, Phase, col, space};
use amenui::ui::Ui;
use amenui::widgets::TextEdit;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

// ── Drag capture ─────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Drags { seen: Vec<Drag> }
fn drags(_: &Drags) -> El<Drag> {
    col().key("root").pad(20.0, 20.0)
        .child(space(100.0, 40.0).key("handle").fill(Paint::INK).on_drag_with(Some))
        .child(space(100.0, 40.0).key("other").on_press(Drag { phase: Phase::End, x: -1.0, y: -1.0, w: 0.0, h: 0.0, dx: 0.0, dy: 0.0, t: 0.0 }))
}

#[test]
fn drag_is_captured_until_release() {
    let mut ui = Ui::new(Drags::default(), drags, |s: &mut Drags, d| s.seen.push(d), fonts(), 400, 300, 1.0);
    ui.press(30.0, 30.0);
    assert_eq!(ui.dragging(), Some("handle"));
    // Far outside the element, and over another pressable one: the drag keeps going.
    ui.pointer_move(300.0, 80.0);
    ui.release(300.0, 80.0);
    ui.pointer_move(10.0, 10.0);
    let s = &ui.state.seen;
    assert_eq!(s.iter().map(|d| d.phase).collect::<Vec<_>>(), [Phase::Start, Phase::Move, Phase::End]);
    assert_eq!((s[0].x, s[0].y, s[0].w, s[0].h), (10.0, 10.0, 100.0, 40.0));
    assert_eq!((s[1].dx, s[1].dy), (270.0, 50.0));
    assert_eq!(s[1].fx(), 1.0, "fraction is clamped");
    assert_eq!(ui.dragging(), None);
}

// ── Text input ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn text_edit_keys() {
    let e = TextEdit::new("中文ab");
    assert_eq!(e.caret, 4);
    let e = e.apply(Key::Left, 99).unwrap().apply(Key::Left, 99).unwrap();
    assert_eq!(e.caret, 2);
    let e = e.apply(Key::Char('x'), 99).unwrap();
    assert_eq!((e.text.as_str(), e.caret), ("中文xab", 3));
    let e = e.apply(Key::Backspace, 99).unwrap().apply(Key::Backspace, 99).unwrap();
    assert_eq!((e.text.as_str(), e.caret), ("中ab", 1));
    let e = e.apply(Key::Delete, 99).unwrap();
    assert_eq!(e.text, "中b");
    assert_eq!(e.apply(Key::Home, 99).unwrap().caret, 0);
    assert_eq!(e.apply(Key::End, 99).unwrap().caret, 2);
    assert_eq!(e.apply(Key::Char('z'), 2), None, "full");
    assert_eq!(e.apply(Key::Home, 99).unwrap().apply(Key::Backspace, 99), None, "nothing before the caret");
    assert_eq!(e.apply(Key::Char('\u{8}'), 99), None, "control characters are not typed");
}
