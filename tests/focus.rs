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

fn cui() -> Ui<(), M> { ui() }

#[test]
fn clearing_the_focus_leaves_nothing_focused_and_tab_goes_on_from_there() {
    let mut ui = cui();
    ui.tick(1.0);
    ui.key(Key::Tab, false);
    ui.key(Key::Tab, false);
    ui.key(Key::Tab, false);
    assert_eq!(ui.focus(), Some("a"));
    ui.command(amenui::morph::Cmd::ClearFocus);
    assert_eq!(ui.focus(), None, "the command clears the focus");
    ui.key(Key::Tab, false);
    assert_eq!(ui.focus(), Some("b"), "Tab goes on after the element that had it");
    // Nothing focused: the command asks for nothing.
    let mut ui = cui();
    ui.tick(2.0);
    let before = ui.needs_frame(2.0);
    ui.clear_focus();
    assert_eq!(ui.needs_frame(2.0), before, "clearing no focus asks for nothing");
}

#[test]
fn the_corners_leave_a_cleared_focus_as_they_leave_any_focus_and_the_page_is_as_never_focused() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(600, 400) else { eprintln!("no GPU adapter here: skipped"); return };
    let look = amenui::style::Look { material: amenui::style::Material::Plain, ..amenui::style::Look::default() };
    let run = |o: &mut amenui::offscreen::Offscreen, how: u8| -> Vec<Vec<u8>> {
        let mut ui = cui();
        ui.tick(1.0);
        if how > 0 {
            for _ in 0..3 { ui.key(Key::Tab, false); }
            ui.tick(1.5);
            let _ = o.render(&mut ui, &look);
            match how {
                1 => ui.clear_focus(),
                _ => ui.command(amenui::morph::Cmd::ClearFocus),
            }
        }
        (0..8).map(|f| { ui.tick(1.5 + 0.016 * (f + 1) as f32); o.render(&mut ui, &look) }).collect()
    };
    let (by_call, by_cmd, never) = (run(&mut o, 1), run(&mut o, 2), run(&mut o, 0));
    assert!(by_call == by_cmd, "the method and the command leave the same way");
    assert!(by_call.last() == never.last(), "once the corners have gone the page is as never focused");
}

/// A page whose skip link comes first, as `widgets::skip_link` is meant to be used, with no tab order
/// of its own; `preview` puts the same page in a preview box.
fn first_view(preview: bool) -> El<M> {
    let page = col().key("pg").child(skip_link("skip", "Skip to content", "main"))
        .child(col().child(button("a", "A", M::Go)).child(button("b", "B", M::Go)))
        .child(col().key("main").child(button("d", "D", M::Go)));
    if preview { col().child(button("before", "Before", M::Go)).child(col().key("box").width(400.0).height(300.0).preview(true).child(page)) } else { page }
}

/// Outside a preview the skip link stays first in tab order: drawing focus-shown elements last in a
/// preview (so they cover what is under them) changes nothing else.
#[test]
fn a_skip_link_written_first_is_the_first_tab_stop() {
    let mut ui = Ui::new((), |_: &()| first_view(false), |_: &mut (), _| {}, fonts(), 600, 400, 1.0);
    ui.tick(1.0);
    assert_eq!(tabs(&mut ui, 4, false), ["skip", "a", "b", "d"]);
    // In a preview it is still the first stop inside the box.
    let mut pv = Ui::new((), |_: &()| first_view(true), |_: &mut (), _| {}, fonts(), 600, 400, 1.0);
    pv.tick(1.0);
    assert_eq!(tabs(&mut pv, 5, false), ["before", "skip", "a", "b", "d"]);
}

/// The state is told where the focus is (`Ui::on_focus`) as it moves, and what it changes is shown: a
/// cell that runs while its control has the focus.
#[test]
fn the_state_hears_where_the_focus_goes_and_the_view_follows() {
    #[derive(Default)]
    struct S { at: Option<String>, told: usize }
    fn view(s: &S) -> El<M> {
        col().child(button("a", "A", M::Go)).child(button("b", "B", M::Go))
            .child(text(if s.at.as_deref() == Some("b") { "running" } else { "at rest" }, 14.0, Paint::TEXT).key("cell"))
    }
    let mut ui = Ui::new(S::default(), view, |_: &mut S, _| {}, fonts(), 600, 400, 1.0)
        .on_focus(|s, k| { s.told += 1; let c = s.at.as_deref() != k; s.at = k.map(str::to_owned); c });
    let shown = |ui: &Ui<S, M>| ui.nodes().iter().find(|n| n.key == "cell").and_then(|n| n.text().map(str::to_owned)).unwrap_or_default();
    ui.key(Key::Tab, false);
    assert_eq!((ui.state.at.as_deref(), shown(&ui).as_str()), (Some("a"), "at rest"));
    ui.key(Key::Tab, false);
    assert_eq!((ui.state.at.as_deref(), shown(&ui).as_str()), (Some("b"), "running"));
    ui.clear_focus();
    assert_eq!((ui.state.at.as_deref(), shown(&ui).as_str()), (None, "at rest"));
    assert_eq!(ui.state.told, 3);
}
