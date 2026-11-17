//! The `render` tool's core (DESIGN.md §1.17): deterministic frames, scripted events, and errors that say
//! what to write instead.
use amenui::drive::{Input, RenderSpec, parse_events, render};
use amenui::error::{DriveError, LookError};
use amenui::offscreen::Offscreen;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn spec(page: &str) -> RenderSpec { RenderSpec { page: page.into(), w: 480, h: 360, scale: 1.0, material: Some("dottr".into()), theme: Some("light".into()), at_ms: 700.0, events: vec![], locale: None } }

#[test]
fn the_same_arguments_give_the_same_bytes() {
    let Ok(mut o) = Offscreen::try_new(480, 360) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut s = spec("recipe-counter");
    s.events = parse_events(r#"[{"at_ms": 200, "kind": "press", "target": "inc"}, {"at_ms": 230, "kind": "release", "target": "inc"}]"#, "inline").unwrap();
    let a = render(&s, fonts(), Some(&mut o)).expect("render");
    let b = render(&s, fonts(), Some(&mut o)).expect("render");
    assert_eq!(a.png, b.png, "two renders differ");
    // A fresh renderer gives the same bytes too.
    let mut o2 = Offscreen::try_new(480, 360).expect("adapter");
    assert_eq!(render(&s, fonts(), Some(&mut o2)).expect("render").png, a.png);
    // The events were played: the count went up.
    assert_eq!(a.tree.find("count").and_then(|n| n.text.clone()).as_deref(), Some("10"));
    // A different moment is a different frame.
    let mut later = s.clone();
    later.at_ms = 260.0;
    assert_ne!(render(&later, fonts(), Some(&mut o)).expect("render").png, a.png);
}

#[test]
fn events_parse_and_errors_say_what_to_write() {
    let ev = parse_events(r#"[{"at_ms": 0, "kind": "key", "key": "Tab"}, {"at_ms": 5, "kind": "text", "text": "hi"}, {"at_ms": 9, "kind": "wheel", "x": 1, "y": 2, "dy": 40}]"#, "x").unwrap();
    assert_eq!(ev[0].input, Input::Key { key: "Tab".into(), shift: false, cmd: false, alt: false });
    assert!(matches!(ev[2].input, Input::Wheel { dy, .. } if dy == 40.0));
    let bad = parse_events(r#"[{"at_ms": 0, "kind": "poke"}]"#, "s.json").unwrap_err();
    assert!(bad.to_string().contains("s.json") && bad.to_string().contains("press, release"), "{bad}");
    assert!(matches!(amenui::drive::material("dott"), Err(LookError::UnknownMaterial { .. })));
    let m = amenui::drive::material("dott").unwrap_err().to_string();
    assert!(m.contains("dottr") && m.contains("halftonr"), "{m}");
    assert!(amenui::drive::theme("sepia").unwrap_err().to_string().contains("light"));
    assert_eq!(amenui::drive::theme("light"), Ok(1));
    let e = match amenui::drive::open("buton", fonts(), 100, 100, 1.0, None, None) { Err(e) => e, Ok(_) => panic!("opened") };
    assert!(e.to_string().contains("recipe-form"), "{e}");
    // A target that is not there names keys that are.
    let mut d = amenui::drive::open("recipe-counter", fonts(), 400, 300, 1.0, None, None).expect("open");
    let err = d.apply(&Input::Press { x: None, y: None, target: Some("incc".into()), shift: false }).unwrap_err();
    assert!(matches!(err, DriveError::UnknownTarget { .. }) && err.to_string().contains("inc"), "{err}");
    assert!(d.apply(&Input::Key { key: "Entr".into(), shift: false, cmd: false, alt: false }).unwrap_err().to_string().contains("Enter"));
    // Zero size.
    let mut z = spec("recipe-form");
    z.w = 0;
    assert!(render(&z, fonts(), None).err().expect("an error").to_string().contains("--size"));
    assert!(Offscreen::try_new(0, 10).is_err());
    // A missing font directory.
    assert!(amenui::text::load_bundled("/nowhere").err().expect("an error").to_string().contains("assets"));
}

#[test]
fn a_drag_through_the_field_map_gives_the_same_bytes_three_times() {
    let Ok(mut o) = Offscreen::try_new(900, 640) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut s = RenderSpec { page: "recipe-gesture".into(), w: 900, h: 640, scale: 1.0, material: Some("dottr".into()), theme: Some("light".into()), at_ms: 600.0, events: vec![], locale: None };
    let mut ev = vec![r#"{"at_ms": 100, "kind": "press", "target": "pip/label"}"#.to_owned()];
    for i in 1..=12 { ev.push(format!(r#"{{"at_ms": {}, "kind": "move", "x": {}, "y": {}}}"#, 100 + i * 16, 60 + i * 12, 150 + i * 8)); }
    ev.push(r#"{"at_ms": 320, "kind": "release", "x": 204, "y": 246}"#.to_owned());
    s.events = parse_events(&format!("[{}]", ev.join(",")), "inline").unwrap();
    let a = render(&s, fonts(), Some(&mut o)).expect("render");
    let b = render(&s, fonts(), Some(&mut o)).expect("render");
    let mut o2 = Offscreen::try_new(900, 640).expect("adapter");
    let c = render(&s, fonts(), Some(&mut o2)).expect("render");
    assert!(a.png == b.png && b.png == c.png, "three renders of a drag differ");
    // The drag stirred the map: the same moment without it is another frame.
    let mut still = s.clone();
    still.events.clear();
    assert_ne!(render(&still, fonts(), Some(&mut o)).expect("render").png, a.png);
}

/// The ink pixels of the `Send` label on the form recipe's solid button, drawn in `material` and in
/// plain, `theme` (0 dark, 1 light).
fn send_ink(o: &mut amenui::offscreen::Offscreen, material: amenui::style::Material, theme: usize) -> usize {
    let mut ui = amenui::recipes::form::ui(fonts(), 480, 360, 1.0);
    ui.tick(1.5);
    let look = amenui::style::Look { material, theme: amenui::style::THEMES[theme], ..ui.look() };
    o.r.partial = false;
    let px = o.render(&mut ui, &look);
    o.r.hide_glyphs = true;
    ui.invalidate();
    let ground = o.render(&mut ui, &look);
    o.r.hide_glyphs = false;
    let node = ui.nodes().iter().position(|n| n.text() == Some("Send")).expect("the Send label");
    let boxes: Vec<[f32; 4]> = ui.glyphs().log.iter().filter(|g| g.node == node).map(|g| amenui::form::ink_box(&ui.scene().instances[g.inst])).collect();
    assert_eq!(boxes.len(), 4, "four glyphs");
    amenui::audit::ink_pixels(&px, &ground, ui.device_size(), &boxes)
}

#[test]
fn near_keeps_a_label_on_a_solid_button_whole() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 360) else { eprintln!("no GPU adapter here: skipped"); return };
    use amenui::style::Material;
    for theme in [0, 1] {
        let plain = send_ink(&mut o, Material::Plain, theme);
        let near = send_ink(&mut o, Material::Near, theme);
        assert!(plain > 60, "theme {theme}: plain draws the label ({plain} px)");
        assert!(near as f32 >= 0.9 * plain as f32, "theme {theme}: near draws {near} of plain's {plain} ink px");
    }
}

/// A page with a line of text to read in the middle and, when `block`, a status line along the bottom
/// right; the readout on, in `at`, `inset` from the edges.
mod readout {
    use amenui::el::{El, Paint, col, space, text};
    use amenui::style::{Corner, Look, Material};
    pub struct S { pub at: Corner, pub inset: [f32; 2], pub block: bool }
    pub fn view(s: &S) -> El<()> {
        let mut page = col().key("page").pad(160.0, 150.0).fill_width().fill_height()
            .child(text("A line to read in the middle", 15.0, Paint::TEXT).key("middle"));
        if s.block { page = page.child(space(0.0, 0.0).key("gap").fill_height()).child(text("status: 3 notes saved, last at 12:04", 13.0, Paint::DIM).key("status").absolute(None, None, Some(10.0), Some(20.0))); }
        page
    }
    pub fn look(s: &S) -> Look { Look { material: Material::Plain, overlay: 1, overlay_at: s.at, overlay_inset: s.inset, ..Look::default() } }
}

#[test]
fn the_readout_sits_in_its_corner_at_its_inset_and_never_covers_text_to_read() {
    use amenui::style::Corner;
    let (w, h) = (640u32, 400u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let ink = |ui: &amenui::ui::Ui<readout::S, ()>, key: &str| {
        let sp = amenui::text::SPREAD as f32;
        let r = ui.instances_of(key).iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |m, i| { let p = if i.paint[3] == amenui::scene::GLYPH { sp } else { 0.0 }; [m[0].min(i.rect[0] + p), m[1].min(i.rect[1] + p), m[2].max(i.rect[0] + i.rect[2] - p), m[3].max(i.rect[1] + i.rect[3] - p)] });
        amenui::ui::Rect::new(r[0], r[1], r[2] - r[0], r[3] - r[1])
    };
    let shot = |o: &mut Offscreen, at: Corner, inset: [f32; 2], block: bool| {
        let mut ui = amenui::ui::Ui::new(readout::S { at, inset, block }, readout::view, |_, _| {}, fonts(), w, h, 1.0).on_look(readout::look);
        ui.tick(1.0);
        let look = ui.look();
        let _ = o.render(&mut ui, &look);
        ui
    };
    for (at, inset) in [(Corner::TopLeft, [12.0, 12.0]), (Corner::TopRight, [20.0, 8.0]), (Corner::BottomRight, [12.0, 12.0]), (Corner::BottomLeft, [30.0, 16.0])] {
        let ui = shot(&mut o, at, inset, false);
        let b = ui.overlay_box().expect("the readout is drawn");
        let (dx, dy) = match at {
            Corner::TopLeft => (b.x, b.y),
            Corner::TopRight => (w as f32 - b.right(), b.y),
            Corner::BottomRight => (w as f32 - b.right(), h as f32 - b.bottom()),
            Corner::BottomLeft => (b.x, h as f32 - b.bottom()),
        };
        assert!((dx - inset[0]).abs() <= 1.0 && (dy - inset[1]).abs() <= 1.0, "{at:?}: {dx}, {dy} from the edges, asked {inset:?}");
        assert!(b.intersect(&ink(&ui, "middle")).w <= 0.0 || b.intersect(&ink(&ui, "middle")).h <= 0.0, "{at:?}: it covers the text");
    }
    // A status line in the bottom right: the readout asked there moves on clockwise. The bottom left
    // would cover it too (the readout is wider than half the window), so it goes on to the top left.
    let ui = shot(&mut o, Corner::BottomRight, [12.0, 12.0], true);
    let b = ui.overlay_box().expect("drawn");
    let status = ink(&ui, "status");
    let r = b.intersect(&status);
    assert!(r.w <= 0.0 || r.h <= 0.0, "the readout {b:?} covers the status line {status:?}");
    assert!(b.x - 12.0 > -1.0 && (b.x - 12.0).abs() <= 1.0 && (b.y - 12.0).abs() <= 1.0, "it went to the top left: {b:?}");
}

/// Two overlapping squares (ink below, accent on top) in a box set see-through as one, and a text
/// beside them; `op` None writes no opacity at all, `gone` leaves the box out.
mod seethrough {
    use amenui::el::{El, Paint, col, space, text};
    use amenui::style::{Look, Material};
    pub struct S { pub op: Option<f32>, pub gone: bool }
    pub fn view(s: &S) -> El<()> {
        let pair = col().key("pair").width(140.0).height(100.0).absolute(Some(60.0), Some(60.0), None, None)
            .child(space(80.0, 60.0).fill(Paint::INK).key("under").absolute(Some(0.0), Some(0.0), None, None))
            .child(space(80.0, 60.0).fill(Paint::ACCENT).key("over").absolute(Some(40.0), Some(30.0), None, None));
        let pair = match s.op { Some(k) => pair.opacity(k), None => pair };
        let page = col().key("page").fill_width().fill_height().child(text("beside it", 16.0, Paint::TEXT).key("note").absolute(Some(260.0), Some(80.0), None, None));
        if s.gone { page } else { page.child(pair) }
    }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
}

#[test]
fn opacity_composites_the_subtree_as_one() {
    let (w, h) = (360u32, 240u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, op: Option<f32>, gone: bool| {
        let mut ui = amenui::ui::Ui::new(seethrough::S { op, gone }, seethrough::view, |_, _| {}, fonts(), w, h, 1.0).on_look(seethrough::look);
        ui.tick(3.0);
        let look = ui.look();
        (o.render(&mut ui, &look), look)
    };
    let (none, look) = shot(&mut o, None, false);
    assert!(shot(&mut o, Some(1.0), false).0 == none, "opacity(1) is the same as none written");
    assert!(shot(&mut o, Some(0.0), false).0 == shot(&mut o, None, true).0, "opacity(0) is the same as the element left out");
    // At 0.5 the overlap is the top square at half over the ground, not both layered.
    let (half, _) = shot(&mut o, Some(0.5), false);
    let px = |f: &[u8], x: u32, y: u32| { let i = ((y * w + x) * 4) as usize; [f[i] as f32, f[i + 1] as f32, f[i + 2] as f32] };
    let ground = px(&none, 20, 20);
    let (top, under) = (px(&none, 130, 110), px(&none, 70, 70));
    assert!(top != under, "the two squares differ");
    let want: Vec<f32> = (0..3).map(|k| ground[k] * 0.5 + top[k] * 0.5).collect();
    let got = px(&half, 130, 110);
    for k in 0..3 { assert!((got[k] - want[k]).abs() <= 3.0, "the overlap is {got:?}, the top at half is {want:?} (top {top:?}, under {under:?}, ground {ground:?})"); }
    // Where only the lower one is, it is the lower one at half.
    let (got, want) = (px(&half, 70, 70), (0..3).map(|k| ground[k] * 0.5 + under[k] * 0.5).collect::<Vec<_>>());
    for k in 0..3 { assert!((got[k] - want[k]).abs() <= 3.0, "{got:?} vs {want:?}"); }
    let _ = look;
}
