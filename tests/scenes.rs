//! The showcase scenes (DESIGN.md §6), without a GPU: routes, Esc, keyboard reach, and each scene's
//! interaction doing what it says.
use lab::el::Key;
use lab::gallery::{self, GalleryUi, Msg, Page, SCENES};

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn at(i: u8, w: u32, h: u32) -> GalleryUi {
    let mut ui = gallery::new_ui(fonts(), w, h, 1.0);
    gallery::update(&mut ui.state, Msg::Go(Page::Scene(i)));
    ui.relayout();
    ui
}
fn run(ui: &mut GalleryUi, secs: f32) { let to = ui.now() + secs; let mut t = ui.now(); while t < to { t += 1.0 / 30.0; ui.tick(t); } }
fn click(ui: &mut GalleryUi, k: &str) { let (x, y) = ui.node(k).unwrap_or_else(|| panic!("no node {k}")).rect.center(); ui.press(x, y); ui.release(x, y); }

#[test]
fn routes_and_escape() {
    for (i, (name, ..)) in SCENES.iter().enumerate() {
        assert_eq!(Page::from_hash(&format!("#scene-{name}")), Some(Page::Scene(i as u8)));
        let mut ui = at(i as u8, 1280, 800);
        // The scene's default style is applied.
        assert_eq!(ui.state.style, Some(SCENES[i].3));
        assert!(ui.key(Key::Escape, false));
        assert_eq!(ui.state.page, Page::Scenes);
    }
    assert_eq!(Page::from_hash("#scenes"), Some(Page::Scenes));
}

#[test]
fn every_control_takes_focus() {
    for (i, (name, ..)) in SCENES.iter().enumerate() {
        for (w, h) in [(1280, 800), (420, 800)] {
            let ui = at(i as u8, w, h);
            // Pressable things are reachable by Tab, except the morph stage (its "next" button does the same)
            // and the items of segmented controls and tabs (one focus place each, arrows inside).
            let stray: Vec<_> = ui.nodes().iter().filter(|n| n.pressable() && !n.focusable && n.key != "mo/stage" && !n.key.starts_with("rail") && !n.key.contains(':')).map(|n| n.key.clone()).collect();
            assert!(stray.is_empty(), "{name} {w}: {stray:?}");
            assert!(ui.nodes().iter().filter(|n| n.focusable).count() >= 6, "{name}");
        }
    }
}

#[test]
fn list_cards_open_their_scene_with_a_flying_title() {
    let mut ui = gallery::new_ui(fonts(), 1280, 800, 1.0);
    gallery::update(&mut ui.state, Msg::Go(Page::Scenes));
    ui.relayout();
    run(&mut ui, 1.0);
    click(&mut ui, "scenes/card2");
    assert_eq!(ui.state.page, Page::Scene(2));
    assert!(ui.flowing() > 0, "the title flies");
}

#[test]
fn opening_builds_and_operates() {
    let mut ui = at(0, 1280, 800);
    run(&mut ui, 1.0);
    click(&mut ui, "op/start");
    assert!(ui.morph_run().is_some(), "the dots fly to the title");
    run(&mut ui, 3.0);
    assert_eq!(ui.state.sc.shown, 2);
    run(&mut ui, 9.0);
    assert_eq!(ui.state.sc.op_text.text, "点与线");
    assert!(ui.state.sc.op_switch);
    assert_eq!(ui.state.sc.op_count, 1);
}

#[test]
fn reading_reflows_when_dragged() {
    let mut ui = at(1, 1280, 800);
    let before = ui.instances_of("read/text");
    let (x, y) = ui.node("read/bust").unwrap().rect.center();
    ui.press(x, y);
    ui.pointer_move(x - 150.0, y + 30.0);
    ui.release(x - 150.0, y + 30.0);
    assert_eq!(ui.state.sc.obj[0], (330.0 - 150.0, 120.0 + 30.0));
    assert_ne!(before.iter().map(|i| i.rect).collect::<Vec<_>>(), ui.instances_of("read/text").iter().map(|i| i.rect).collect::<Vec<_>>());
    // Keys move it too.
    click(&mut ui, "read/bust");
    ui.key(Key::Right, false);
    assert_eq!(ui.state.sc.obj[0].0, 190.0);
}

#[test]
fn printing_seams_and_xray_hold() {
    let mut ui = at(2, 1280, 800);
    let (x, y) = ui.node("pr/seam1").unwrap().rect.center();
    ui.press(x, y);
    ui.pointer_move(x - 60.0, y);
    ui.release(x - 60.0, y);
    assert!(ui.state.sc.seams[1] < 0.4);
    assert!(ui.state.sc.seams[1] > ui.state.sc.seams[0]);

    let mut ui = at(4, 1280, 800);
    assert!(ui.key(Key::Char(' '), false));
    assert!(ui.state.sc.apart);
    assert!(ui.node("xr/near/app").is_some() && ui.flowing() > 0);
    ui.key_up(Key::Char(' '));
    assert!(!ui.state.sc.apart);
}

#[test]
fn morph_steps_and_sound_beats() {
    let mut ui = at(3, 1280, 800);
    click(&mut ui, "mo/next");
    assert_eq!(ui.state.sc.form, 1);
    assert!(ui.morph_run().is_some());

    let mut ui = at(5, 1280, 800);
    run(&mut ui, 0.5);
    assert!(ui.state.sc.levels.iter().all(|&v| v == 0.0), "silent at rest");
    ui.key(Key::Char(' '), false);
    run(&mut ui, 1.2);
    assert!(ui.state.sc.levels.iter().any(|&v| v > 0.2));
    assert!(ui.field().fronts.iter().any(|f| f[3] > 0.0), "a beat put a front in the field");
}
