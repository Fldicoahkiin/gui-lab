//! The film (DESIGN.md §6), without a GPU: twelve chapters, each comes to rest; the three actors are in
//! every chapter with the same flow ids; the keyboard walks it; autoplay yields to input in the same
//! frame; a burst of cuts leaves nothing flying.
use lab::el::Key;
use lab::gallery::film::{self, CHAPTERS, FMsg};
use lab::gallery::{self, GalleryUi, Msg, Page};

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn open() -> GalleryUi {
    let mut ui = gallery::new_ui(fonts(), 1280, 640, 1.0);
    ui.pointer_leave();
    assert!(ui.state.route("film"));
    ui.relayout();
    ui
}
fn run(ui: &mut GalleryUi, dt: f32) {
    let to = ui.now() + dt;
    let mut t = ui.now();
    while t < to { t = (t + 1.0 / 60.0).min(to); ui.tick(t); ui.frame_drawn(); }
}
fn pause(ui: &mut GalleryUi) { if ui.state.film.playing { gallery::update(&mut ui.state, Msg::Film(FMsg::Play)); ui.relayout(); } }
/// The node that carries flow id `id`.
fn carrier<'a>(ui: &'a GalleryUi, id: &str) -> Option<&'a lab::ui::Node<Msg>> {
    let ns = ui.nodes();
    ns.iter().find(|n| n.flow() == Some(id) && n.parent.is_none_or(|p| ns[p].flow() != Some(id)) && !n.hidden)
}

#[test]
fn twelve_chapters_in_order_with_their_presets() {
    assert_eq!(CHAPTERS, 12);
    assert_eq!(film::CH.len(), 12);
    let names: Vec<&str> = film::CH.iter().map(|c| c.name).collect();
    assert_eq!(names[0], "Normcore");
    assert_eq!(names[11], "Mixed media");
    let ui = open();
    assert_eq!(ui.state.page, Page::Film);
    for c in film::CH.iter() { assert_eq!(ui.state.presets[c.style].name, c.name, "chapter in its own preset"); }
}

#[test]
fn the_actors_are_in_every_chapter_and_every_chapter_rests() {
    let mut ui = open();
    pause(&mut ui);
    for c in 0..CHAPTERS {
        film::go(&mut ui.state, c, None);
        ui.relayout();
        run(&mut ui, 0.05);
        for id in ["film-sentence", "film-number", "film-object"] {
            let n = carrier(&ui, id).unwrap_or_else(|| panic!("chapter {}: no {id}", c + 1));
            assert!(n.key.starts_with(&format!("film/c{:02}/", c + 1)), "chapter {}: {id} is {}", c + 1, n.key);
            let st = ui.node(&format!("film/stage/{:02}", c + 1)).expect("stage").rect;
            let (x, y) = n.rect.center();
            assert!(st.contains(x, y), "chapter {}: {id} outside the stage", c + 1);
        }
        assert!(ui.problems().is_empty(), "chapter {}: {:?}", c + 1, ui.problems());
        run(&mut ui, 4.0);
        let t = ui.now();
        assert!(!ui.needs_frame(t), "chapter {} does not come to rest", c + 1);
        assert!(ui.next_wake(t).is_none_or(|w| w > t), "chapter {} wakes at once", c + 1);
    }
}

#[test]
fn the_keyboard_walks_the_film() {
    let mut ui = open();
    pause(&mut ui);
    for c in 1..=CHAPTERS {
        // The palette's field keeps ← and →; Escape-free: Tab away from it first.
        if ui.state.film.ch == 1 { ui.key(Key::Tab, false); }
        ui.key(Key::Right, false);
        run(&mut ui, 0.3);
        assert_eq!(ui.state.film.ch, c % CHAPTERS, "→ moved on");
    }
    // Number keys jump; every chapter has something to focus.
    for (k, c) in [('5', 4), ('0', 9), ('-', 10), ('=', 11), ('1', 0)] {
        if ui.state.film.ch == 1 { ui.key(Key::Tab, false); }
        ui.key(Key::Char(k), false);
        run(&mut ui, 0.3);
        assert_eq!(ui.state.film.ch, c, "{k}");
    }
    for c in 0..CHAPTERS {
        film::go(&mut ui.state, c, None);
        ui.relayout();
        run(&mut ui, 0.1);
        ui.key(Key::Tab, false);
        assert!(ui.focus().is_some(), "chapter {}: Tab reaches nothing", c + 1);
    }
    // Space plays and pauses; Escape goes to the gallery.
    film::go(&mut ui.state, 4, None);
    ui.relayout();
    ui.key(Key::Char(' '), false);
    assert!(ui.state.film.playing);
    ui.key(Key::Char(' '), false);
    assert!(!ui.state.film.playing && ui.state.film.held);
    ui.key(Key::Escape, false);
    assert_ne!(ui.state.page, Page::Film);
}

#[test]
fn autoplay_moves_on_and_yields_to_input_at_once() {
    let mut ui = open();
    assert!(ui.state.film.playing, "the film plays when it opens");
    let dwell = ui.state.film.spec.dwell_ms / 1000.0;
    run(&mut ui, dwell + 0.2);
    assert_eq!(ui.state.film.ch, 1, "autoplay went on to the next chapter");
    // A press: autoplay stops in the same frame, before any tick.
    let (x, y) = ui.node("film/c02/n-label").expect("label").rect.center();
    ui.press(x, y);
    assert!(!ui.state.film.playing, "input pauses autoplay at once");
    ui.release(x, y);
    run(&mut ui, dwell + 1.0);
    assert_eq!(ui.state.film.ch, 1, "paused: it stays");
    // It resumes after the idle time.
    let idle = ui.state.film.spec.idle_ms / 1000.0;
    run(&mut ui, idle);
    assert!(ui.state.film.playing, "autoplay resumed after {idle} s");
    // A key too.
    ui.key(Key::Down, false);
    assert!(!ui.state.film.playing);
}

#[test]
fn rapid_cuts_leave_nothing_flying() {
    let mut ui = open();
    pause(&mut ui);
    // Into the linocut poster, start its morph, then thirty cuts a frame apart.
    film::go(&mut ui.state, 6, None);
    ui.relayout();
    run(&mut ui, 1.0);
    gallery::update(&mut ui.state, Msg::Film(FMsg::Form));
    ui.relayout();
    run(&mut ui, 0.4);
    assert!(ui.morph_run().is_some(), "the poster's morph runs");
    for i in 0..30 {
        let m = if i % 3 == 2 { FMsg::Prev } else { FMsg::Next };
        gallery::update(&mut ui.state, Msg::Film(m));
        ui.relayout();
        run(&mut ui, 1.0 / 60.0);
    }
    run(&mut ui, 5.0);
    assert!(ui.morph_run().is_none(), "a morph is still running");
    assert_eq!(ui.recompose_tiles(), 0);
    let t = ui.now();
    assert!(!ui.needs_frame(t), "the film did not come to rest after the cuts");
    // The ghost presses do not count as a person's input.
    let mut ui = open();
    film::go(&mut ui.state, 0, None);
    ui.relayout();
    run(&mut ui, 3.0);
    assert!(ui.state.film.playing && ui.state.film.motion, "the ghost turned the switch on and still plays");
}
