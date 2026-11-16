//! The film (DESIGN.md §6), without a GPU: a prologue and thirteen chapters, each comes to rest; the
//! title word is one element that recomposes across cuts; the number and the sign are in every flat
//! chapter with the same flow ids (the number in the mechanism too); the
//! keyboard walks it; autoplay yields to input in the same frame; a burst of cuts leaves nothing
//! flying; the mechanism turns, picks, explodes and loosens a part.
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
fn a_prologue_and_eleven_chapters_in_order_with_their_presets() {
    assert_eq!(CHAPTERS, 12);
    assert_eq!(film::CH.len(), 12);
    let names: Vec<&str> = film::CH.iter().map(|c| c.name).collect();
    assert_eq!(names[0], "Prologue");
    assert_eq!(names[1], "Normcore");
    assert_eq!(names[10], "Bauhaus");
    assert_eq!(names[11], "Mechanism");
    let ui = open();
    assert_eq!(ui.state.page, Page::Film);
    for c in film::CH.iter().skip(1).take(10) { assert_eq!(ui.state.presets[c.style].name, c.name, "chapter in its own preset"); }
}

#[test]
fn the_actors_are_in_every_chapter_and_every_chapter_rests() {
    let mut ui = open();
    pause(&mut ui);
    for c in 0..CHAPTERS {
        film::go(&mut ui.state, c, None);
        ui.relayout();
        run(&mut ui, 0.05);
        let ids: &[&str] = if c < 11 { &["film-number", "film-sign"] } else { &["film-number"] };
        let st = ui.node(&format!("film/stage/{c:02}")).expect("stage").rect;
        let word = ui.node("film/word").expect("the title word");
        assert!(st.contains(word.rect.x + 1.0, word.rect.y + 1.0), "chapter {c}: the title outside the stage");
        assert_eq!(ui.export_tree().find("film/word").and_then(|n| n.text.clone()), Some(film::Chapter::word(c)));
        for &id in ids {
            let n = carrier(&ui, id).unwrap_or_else(|| panic!("chapter {}: no {id}", c + 1));
            assert!(n.key.starts_with(&format!("film/c{c:02}/")), "chapter {c}: {id} is {}", n.key);
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
        if ui.state.film.ch == 2 { ui.key(Key::Tab, false); }
        ui.key(Key::Right, false);
        run(&mut ui, 0.3);
        assert_eq!(ui.state.film.ch, c % CHAPTERS, "→ moved on");
    }
    // Number keys jump; every chapter has something to focus.
    for (k, c) in [('5', 5), ('0', 10), ('-', 11), ('1', 1)] {
        if ui.state.film.ch == 2 { ui.key(Key::Tab, false); }
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
    let (x, y) = ui.node("film/c01/preview-label").expect("label").rect.center();
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
    film::go(&mut ui.state, 7, None);
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
    film::go(&mut ui.state, 1, None);
    ui.relayout();
    run(&mut ui, 3.0);
    assert!(ui.state.film.playing && ui.state.film.inverted, "the ghost turned the switch on and still plays");
}

#[test]
fn the_mechanism_turns_picks_explodes_and_loosens() {
    use lab::gallery::mechanism::{MMsg, PARTS};
    let mut ui = open();
    pause(&mut ui);
    ui.key(Key::End, false);
    run(&mut ui, 1.0);
    assert_eq!(ui.state.film.ch, 11);
    let r = ui.node("film/c11/machine/view").expect("the mechanism's view").rect;
    // Somewhere on the machine is a gear: dragging it turns the whole train at its ratios.
    let m = &ui.state.film.mech;
    let gear = (1..20).flat_map(|i| (1..12).map(move |j| (i as f32 / 20.0, j as f32 / 12.0)))
        .find(|&(fx, fy)| m.scene().pick(lab::gallery::mechanism::library(), fx, fy, r.w / r.h).is_some_and(|h| (3..=5).contains(&h.object)))
        .expect("a gear in view");
    let (x, y) = (r.x + gear.0 * r.w, r.y + gear.1 * r.h);
    let (t0, a0) = (ui.state.film.mech.turn, ui.state.film.mech.angles());
    ui.press(x, y);
    for i in 1..=10 { ui.pointer_move(x + i as f32 * 6.0, y); run(&mut ui, 1.0 / 60.0); }
    ui.release(x + 60.0, y);
    let a1 = ui.state.film.mech.angles();
    assert!((ui.state.film.mech.turn - t0).abs() > 20.0, "the gear drag turned the train");
    let d = |k: usize| a1[k] - a0[k];
    assert!((d(1) + 2.0 * d(0)).abs() < 1e-2 && (d(2) - d(0) * 24.0 / 18.0).abs() < 1e-2, "ratios: {:?}", (d(0), d(1), d(2)));
    // Dragging the background orbits instead.
    let yaw = ui.state.film.mech.orbit.yaw;
    ui.press(r.x + 6.0, r.y + 6.0);
    for i in 1..=6 { ui.pointer_move(r.x + 6.0 + i as f32 * 10.0, r.y + 6.0); run(&mut ui, 1.0 / 60.0); }
    ui.release(r.x + 66.0, r.y + 6.0);
    assert!((ui.state.film.mech.orbit.yaw - yaw).abs() > 5.0, "the background drag orbits");
    // Hovering a part names it on a leader.
    let m = &ui.state.film.mech;
    let (fx, fy) = (1..20).flat_map(|i| (1..12).map(move |j| (i as f32 / 20.0, j as f32 / 12.0)))
        .find(|&(fx, fy)| m.scene().pick(lab::gallery::mechanism::library(), fx, fy, r.w / r.h).is_some_and(|h| (1..=8).contains(&h.object)))
        .expect("a part in view");
    let (x, y) = (r.x + fx * r.w, r.y + fy * r.h);
    ui.pointer_move(x, y);
    run(&mut ui, 0.1);
    assert!(ui.state.film.mech.hover.is_some(), "hover: {:?} at {x},{y} in {r:?}", ui.state.film.mech.hover);
    assert!(ui.node("film/c11/machine/leader-label").is_some(), "hover shows the part's name");
    // Exploding moves the parts apart and comes to rest; a pressed part loosens and forms again.
    gallery::update(&mut ui.state, Msg::Film(FMsg::Mech(MMsg::Explode(1.0))));
    ui.relayout();
    run(&mut ui, 2.0);
    assert!((ui.state.film.mech.explode - 1.0).abs() < 1e-3);
    gallery::update(&mut ui.state, Msg::Film(FMsg::Mech(MMsg::Loosen(6))));
    ui.relayout();
    run(&mut ui, 0.4);
    assert!(ui.morph_run().is_some(), "the part loosens into particles");
    run(&mut ui, 3.0);
    assert!(ui.morph_run().is_none() && ui.state.film.mech.loose.is_none());
    // Every part has a name; the keyboard turns, explodes and switches the material.
    assert_eq!(PARTS.len(), 8);
    ui.pointer_leave();
    ui.state.film.mech.hover = None;
    let k = "film/c11/machine/view";
    gallery::update(&mut ui.state, Msg::Film(FMsg::Go(11)));
    ui.relayout();
    run(&mut ui, 1.0);
    let (vx, vy) = ui.node(k).expect("view").rect.center();
    let _ = (vx, vy);
    ui.state.cmds.push(lab::morph::Cmd::Focus { key: k.into() });
    ui.relayout();
    assert_eq!(ui.focus(), Some(k));
    let t = ui.state.film.mech.turn;
    ui.key(Key::Char(']'), false);
    assert!(ui.state.film.mech.turn > t);
    ui.key(Key::Char('m'), false);
    assert_eq!(ui.state.film.mech.mat, 1);
    ui.key(Key::Char('e'), false);
    run(&mut ui, 4.0);
    let t = ui.now();
    assert!(!ui.needs_frame(t), "the mechanism comes to rest");
}

#[test]
fn scrubbing_a_morph_moves_its_clock_only() {
    let mut ui = open();
    pause(&mut ui);
    film::go(&mut ui.state, 7, None);
    ui.relayout();
    run(&mut ui, 2.0);
    let (x, y) = ui.node("film/c07/forms").expect("poster").rect.center();
    ui.press(x, y);
    ui.pointer_move(x + 6.0, y);
    run(&mut ui, 1.0 / 60.0);
    let first = ui.morph_run().map(|r| (r.id, r.upload, r.particles.len())).expect("the scrub started a morph");
    for i in 0..40 { ui.pointer_move(x + 6.0 + i as f32 * 5.0, y + (i % 3) as f32); run(&mut ui, 1.0 / 60.0); }
    let after = ui.morph_run().map(|r| (r.id, r.upload, r.particles.len())).expect("still held");
    assert_eq!(first, after, "pointer moves rebuilt the particle set");
    ui.release(x + 200.0, y);
    run(&mut ui, 4.0);
    assert!(ui.morph_run().is_none());
}

#[test]
fn the_title_recomposes_across_a_cut() {
    let mut ui = open();
    pause(&mut ui);
    film::go(&mut ui.state, 1, None);
    ui.relayout();
    run(&mut ui, 3.0);
    let before = ui.node("film/word").expect("title").rect;
    film::go(&mut ui.state, 2, None);
    ui.relayout();
    run(&mut ui, 1.0 / 60.0);
    assert!(ui.recompose_tiles() > 0, "the title recomposes glyph by glyph");
    assert_eq!(ui.node("film/word").expect("title").rect.x, before.x, "in the same place");
    run(&mut ui, 3.0);
    assert_eq!(ui.recompose_tiles(), 0);
}
