//! The `render` tool's core (DESIGN.md §1.17): deterministic frames, scripted events, and errors that say
//! what to write instead.
use lab::drive::{Input, RenderSpec, parse_events, render};
use lab::error::{DriveError, LookError};
use lab::offscreen::Offscreen;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn spec(page: &str) -> RenderSpec { RenderSpec { page: page.into(), w: 480, h: 360, scale: 1.0, material: Some("dot".into()), theme: Some("light".into()), at_ms: 700.0, events: vec![] } }

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
    assert!(matches!(lab::drive::material("dott"), Err(LookError::UnknownMaterial { .. })));
    let m = lab::drive::material("dott").unwrap_err().to_string();
    assert!(m.contains("dot") && m.contains("halftone"), "{m}");
    assert!(lab::drive::theme("sepia").unwrap_err().to_string().contains("light"));
    assert_eq!(lab::drive::theme("light"), Ok(1));
    let e = match lab::drive::open("buton", fonts(), 100, 100, 1.0, None, None) { Err(e) => e, Ok(_) => panic!("opened") };
    assert!(e.to_string().contains("recipe-form"), "{e}");
    // A target that is not there names keys that are.
    let mut d = lab::drive::open("recipe-counter", fonts(), 400, 300, 1.0, None, None).expect("open");
    let err = d.apply(&Input::Press { x: None, y: None, target: Some("incc".into()), shift: false }).unwrap_err();
    assert!(matches!(err, DriveError::UnknownTarget { .. }) && err.to_string().contains("inc"), "{err}");
    assert!(d.apply(&Input::Key { key: "Entr".into(), shift: false, cmd: false, alt: false }).unwrap_err().to_string().contains("Enter"));
    // Zero size.
    let mut z = spec("recipe-form");
    z.w = 0;
    assert!(render(&z, fonts(), None).err().expect("an error").to_string().contains("--size"));
    assert!(Offscreen::try_new(0, 10).is_err());
    // A missing font directory.
    assert!(lab::text::load_bundled("/nowhere").err().expect("an error").to_string().contains("assets"));
}
