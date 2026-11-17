//! Tab order (`El::tab_order`), skip links (`El::skip_to`, `widgets::skip_link`) and elements drawn
//! only while focused (`El::focus_shown`).
use amenui::el::{El, Key, Paint, col, stack, text};
use amenui::ui::Ui;
use amenui::widgets::{button, skip_link};

#[derive(Clone)]
enum M { Go }
fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

fn view(_: &()) -> El<M> {
    let nav = col().child(button("a", "A", M::Go)).child(button("b", "B", M::Go)).child(button("c", "C", M::Go).tab_order(-1));
    let main = col().key("main").child(text("Content", 16.0, Paint::TEXT)).child(button("d", "D", M::Go)).child(button("e", "E", M::Go).tab_order(2));
    stack([col().child(nav).child(main), skip_link("skip", "Skip to content", "main").tab_order(1)])
}

fn ui() -> Ui<(), M> { Ui::new((), view, |_: &mut (), _| {}, fonts(), 600, 400, 1.0) }

fn tabs(ui: &mut Ui<(), M>, n: usize, back: bool) -> Vec<String> {
    (0..n).map(|_| { ui.key(Key::Tab, back); ui.focus().unwrap_or_default().to_owned() }).collect()
}

#[test]
fn tab_visits_positive_orders_first_then_document_order_and_skips_negative_ones() {
    let mut ui = ui();
    assert_eq!(tabs(&mut ui, 6, false), ["skip", "e", "a", "b", "d", "skip"]);
    assert_eq!(tabs(&mut ui, 2, true), ["d", "b"]);
    // A negative order still takes focus when asked.
    ui.focus_key("c");
    assert_eq!(ui.focus(), Some("c"));
}

#[test]
fn a_skip_link_moves_focus_into_its_target_and_is_drawn_only_while_focused() {
    let mut ui = ui();
    assert!(ui.instances_of("skip").is_empty(), "drawn before it had focus");
    assert!(ui.a11y().iter().any(|n| n.key == "skip"), "screen readers find it");
    ui.key(Key::Tab, false);
    assert_eq!(ui.focus(), Some("skip"));
    assert!(!ui.instances_of("skip").is_empty(), "not drawn while focused");
    assert!(ui.key(Key::Enter, false));
    assert_eq!(ui.focus(), Some("d"), "the first focusable element inside the target");
    assert!(ui.instances_of("skip").is_empty(), "still drawn after focus left it");
    // Assistive technology's activation does the same.
    ui.focus_key("a");
    assert!(ui.activate("skip"));
    assert_eq!(ui.focus(), Some("d"));
}
