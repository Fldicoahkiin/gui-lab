//! A run of frames on the fixed clock (`drive::frames`, `render --frames`): the same timeline gives the
//! same bytes every time; frame 60 at 60 fps is the frame `render` draws at 1000 ms; frame k at 30 fps
//! is frame 2k at 60; a rate that does not step the 60 Hz clock evenly is refused with the ones that do.
use amenui::drive::{FPS, RenderSpec, frames, open_in, parse_events, render};
use amenui::error::DriveError;
use amenui::offscreen::Offscreen;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn spec() -> RenderSpec {
    let events = parse_events(r#"[{"at_ms": 200, "kind": "press", "target": "inc"}, {"at_ms": 230, "kind": "release", "target": "inc"}, {"at_ms": 640, "kind": "move", "x": 120, "y": 80}]"#, "inline").expect("events");
    RenderSpec { page: "recipe-counter".into(), w: 320, h: 200, scale: 1.0, material: Some("dottr".into()), theme: Some("light".into()), at_ms: 1000.0, events, locale: None }
}
fn run(o: &mut Offscreen, range: std::ops::Range<u32>, fps: u32) -> Vec<(u32, Vec<u8>)> {
    let mut out = vec![];
    frames(&spec(), range, fps, fonts(), Some(o), &open_in, &mut |n, r| out.push((n, r.png.clone()))).expect("frames");
    out
}

#[test]
fn a_timeline_gives_the_same_frames_and_frame_60_is_the_frame_at_1000_ms() {
    let Ok(mut o) = Offscreen::try_new(320, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    let a = run(&mut o, 0..72, 60);
    assert_eq!(a.len(), 72);
    assert_eq!(a.iter().map(|f| f.0).collect::<Vec<_>>(), (0..72).collect::<Vec<_>>());
    // Three times, the second and third with renderers of their own: every frame the same bytes.
    for _ in 0..2 {
        let mut o2 = Offscreen::try_new(320, 200).expect("adapter");
        let b = run(&mut o2, 0..72, 60);
        for (x, y) in a.iter().zip(&b) { assert!(x.1 == y.1, "frame {} differs between runs", x.0); }
    }
    // Something happens in it: the press's front.
    assert!(a[10].1 != a[20].1, "frames 10 and 20 are the same");
    let single = render(&spec(), fonts(), Some(&mut o)).expect("render");
    assert!(a[60].1 == single.png, "frame 60 is not the frame at 1000 ms");
    // A run that starts later plays the same clock.
    let late = run(&mut o, 55..61, 60);
    assert!(late.last().expect("frame 60").1 == single.png);
}

#[test]
fn half_the_rate_is_every_other_frame_and_other_rates_are_refused() {
    let Ok(mut o) = Offscreen::try_new(320, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    let at60 = run(&mut o, 0..41, 60);
    let at30 = run(&mut o, 0..21, 30);
    for (k, f) in at30.iter().enumerate() { assert!(f.1 == at60[2 * k].1, "frame {k} at 30 fps is not frame {} at 60", 2 * k); }
    let at12 = run(&mut o, 0..5, 12);
    for (k, f) in at12.iter().enumerate() { assert!(f.1 == at60[5 * k].1, "frame {k} at 12 fps is not frame {} at 60", 5 * k); }
    let e = frames(&spec(), 0..10, 24, fonts(), Some(&mut o), &open_in, &mut |_, _| {}).unwrap_err();
    assert!(matches!(e, DriveError::BadFps { given: 24 }));
    let m = e.to_string();
    for f in FPS { assert!(m.contains(&f.to_string()), "{m}"); }
    assert!(matches!(frames(&spec(), 5..5, 60, fonts(), Some(&mut o), &open_in, &mut |_, _| {}), Err(DriveError::NoFrames { .. })));
}

#[test]
fn a_motion_that_ends_between_two_frames_still_gets_its_settled_frame() {
    // A host draws only when `needs_frame` says so. A frame drawn while something moves owes one more:
    // if the motion ends before the next frame comes, that frame is still drawn, and it is the page
    // at rest, not the last frame of the motion left on the screen.
    let Ok(mut o) = Offscreen::try_new(320, 200) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = amenui::recipes::counter::ui(fonts(), 320, 200, 1.0);
    let mut t = 0.0;
    let frame = |ui: &mut amenui::ui::Ui<amenui::recipes::counter::Counter, amenui::recipes::counter::Msg>, o: &mut Offscreen, t: f32| { ui.tick(t); let look = amenui::style::Look { material: amenui::style::Material::Plain, ..ui.look() }; o.render(ui, &look) };
    for _ in 0..240 { t += 1.0 / 60.0; frame(&mut ui, &mut o, t); }
    assert!(ui.activate("inc"));
    t += 1.0 / 60.0;
    ui.tick(t);
    assert!(ui.needs_frame(t), "the press moves something");
    let mid = frame(&mut ui, &mut o, t);
    // The next frame comes long after the motion has ended.
    t += 4.0;
    ui.tick(t);
    assert!(ui.needs_frame(t), "the frame after a motion ended is drawn");
    let settled = frame(&mut ui, &mut o, t);
    assert!(mid != settled, "the settled frame is not the mid-motion one");
    t += 1.0 / 60.0;
    ui.tick(t);
    assert!(!ui.needs_frame(t) || ui.ongoing(), "then the page rests");
    let again = frame(&mut ui, &mut o, t);
    assert!(settled == again, "the frame drawn is the page at rest");
}
