//! The beat (DESIGN.md §1.32). At 120 bpm and four steps a beat, a front's radius moves a whole step
//! or not at all from one frame to the next; `Hold(2)` keeps the frame byte for byte; the frame after
//! `Rewind(1)` is the frame one beat before; `Set` of `post.invert` turns every pixel over and a second
//! one turns it back; an event at `"3.2.1"` is the event at its ms. Off (`bpm` 0), a look draws as it
//! did before there was a beat. Needs a GPU adapter (lavapipe will do).
use amenui::beat::Beat;
use amenui::drive::{Driven, parse_events, play};
use amenui::el::{El, Paint, Response, col, text};
use amenui::morph::Cmd;
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 480;
const H: u32 = 300;

#[derive(Clone)]
struct S { beat: Beat, material: Material }
fn view(_: &S) -> El<()> {
    col().pad(40.0, 40.0).gap(16.0)
        .child(text("On the beat", 56.0, Paint::INK).key("title").legible(false).respond(Response::Loosen))
        .child(text("The fronts move a step at a time.", 18.0, Paint::INK).key("body"))
}
fn look(s: &S) -> Look { Look { material: s.material, beat: s.beat, ..Look::default() } }
fn open(beat: Beat, material: Material) -> Ui<S, ()> {
    let mut ui = Ui::new(S { beat, material }, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look);
    ui.relayout();
    ui
}
const ON: Beat = Beat { bpm: 120.0, steps: 4, swing: 0.0 };

/// Settle the page to `t` s, one tick each 1/60 s.
fn settle(ui: &mut Ui<S, ()>, t: f32) { let mut k = 1; while (k as f32) / 60.0 <= t + 1e-6 { ui.tick(k as f32 / 60.0); ui.frame_drawn(); k += 1; } }
fn frame(o: &mut Offscreen, ui: &mut Ui<S, ()>) -> Vec<u8> { let l = ui.look(); let px = o.render(ui, &l); ui.frame_drawn(); px }

#[test]
fn a_front_moves_a_whole_step_or_not_at_all() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = open(ON, Material::Plain);
    settle(&mut ui, 2.0);
    ui.command(Cmd::RipplrAt { key: "title".into(), strength: 1.0 });
    let (mut same, mut moved) = (0, 0);
    let mut last: Option<(f32, Vec<u8>)> = None;
    for k in 1..=60 {
        ui.tick(2.0 + k as f32 / 60.0);
        let g = ui.beat_grid();
        let px = frame(&mut o, &mut ui);
        if let Some((g0, p0)) = &last {
            let d = g - g0;
            assert!(d.abs() < 1e-5 || (d - ON.step_s()).abs() < 1e-4, "the clock the front is drawn at went {d} s");
            if d.abs() < 1e-5 { assert!(px == *p0, "frame {k}: two frames of one step differ"); same += 1; } else if px != *p0 { moved += 1; }
        }
        last = Some((g, px));
    }
    assert!(same > 30 && moved > 3, "{same} frames held a step, {moved} moved");
}

#[test]
fn a_hold_keeps_the_frame_and_a_rewind_goes_back_a_beat() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = open(ON, Material::Plain);
    settle(&mut ui, 2.0);
    ui.command(Cmd::RipplrAt { key: "title".into(), strength: 1.0 });
    // The frames of the front's first second, kept by beat time.
    let mut seen: Vec<(f32, Vec<u8>)> = vec![];
    for k in 1..=48 { ui.tick(2.0 + k as f32 / 60.0); let g = ui.beat_grid(); seen.push((g, frame(&mut o, &mut ui))); }
    // Hold two steps (a quarter second): every frame the same, then it goes on.
    let t0 = 2.0 + 48.0 / 60.0;
    ui.command(Cmd::Hold(2));
    let held = frame(&mut o, &mut ui);
    let mut n = 0;
    for k in 1..=14 {
        let t = t0 + k as f32 / 60.0;
        ui.tick(t);
        let px = frame(&mut o, &mut ui);
        if t < t0 + 2.0 * ON.step_s() - 1e-4 { assert!(px == held, "a frame during the hold changed"); n += 1; }
    }
    assert!(n >= 13, "{n} frames in the hold");
    // Back a beat: the frame drawn at that beat time before.
    let back = ui.beat_grid() - ON.beat_s();
    ui.command(Cmd::Rewind(1.0));
    assert!((ui.beat_grid() - back).abs() < 1e-4, "rewound to {} not {back}", ui.beat_grid());
    ui.tick(t0 + 15.0 / 60.0);
    let g = ui.beat_grid();
    let px = frame(&mut o, &mut ui);
    let (_, then) = seen.iter().find(|(s, _)| (s - g).abs() < 1e-4).expect("that step was drawn before");
    let off = px.chunks(4).zip(then.chunks(4)).filter(|(a, b)| a != b).count();
    assert!(off == 0, "the frame after the rewind differs from the one a beat before in {off} px");
}

#[test]
fn setting_invert_turns_every_pixel_over_and_back() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = open(ON, Material::Dottr);
    settle(&mut ui, 3.0);
    let a = frame(&mut o, &mut ui);
    ui.command(Cmd::Set { name: "post.invert".into(), value: 1.0 });
    ui.tick(3.0 + 1.0 / 60.0);
    let b = frame(&mut o, &mut ui);
    let worst = a.chunks(4).zip(b.chunks(4)).map(|(p, q)| (0..3).map(|c| (255 - p[c] as i32 - q[c] as i32).abs()).max().unwrap_or(0)).max().unwrap_or(0);
    assert!(worst <= 1, "an inverted pixel is {worst}/255 off");
    ui.command(Cmd::Set { name: "post.invert".into(), value: 1.0 });
    ui.tick(3.0 + 2.0 / 60.0);
    let c = frame(&mut o, &mut ui);
    assert!(a == c, "turned over twice it is not the frame it was");
    assert!(ui.look().set("nothing.here", 1.0).is_err());
}

#[test]
fn an_event_at_a_place_on_the_beat_is_the_event_at_its_ms() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let run = |o: &mut Offscreen, json: &str| {
        o.r.partial = false;
        let mut ui = open(ON, Material::Dottr);
        play(&mut ui, &parse_events(json, "test").expect("events"), 4800.0).expect("plays");
        frame(o, &mut ui)
    };
    // Bar 3, beat 2, step 1 at 120 bpm: two bars (4 s) and a beat (0.5 s).
    let by_beat = run(&mut o, r#"[{"at": "3.2.1", "kind": "cmd", "cmd": {"ripplr_at": {"key": "title", "strength": 1}}}]"#);
    let by_ms = run(&mut o, r#"[{"at_ms": 4500, "kind": "cmd", "cmd": {"ripplr_at": {"key": "title", "strength": 1}}}]"#);
    assert!(by_beat == by_ms, "the two ways of writing the time give different frames");
    let none = run(&mut o, "[]");
    assert!(by_beat != none, "the front did not show");
    // Without a beat, a place on it is refused.
    let mut off = open(Beat::default(), Material::Dottr);
    let e = play(&mut off, &parse_events(r#"[{"at": "1.1.1", "kind": "cmd", "cmd": "retrigger"}]"#, "test").unwrap(), 100.0).unwrap_err().to_string();
    assert!(e.contains("beat"), "{e}");
}

#[test]
fn off_the_beat_changes_nothing() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    // Two pages through one renderer: each frame drawn whole.
    o.r.partial = false;
    // `bpm` 0 with other numbers is the same as no beat, and the beat's commands do nothing.
    let mut a = open(Beat::default(), Material::Dottr);
    let mut b = open(Beat { bpm: 0.0, steps: 7, swing: 0.4 }, Material::Dottr);
    for ui in [&mut a, &mut b] { settle(ui, 1.0); ui.command(Cmd::RipplrAt { key: "title".into(), strength: 1.0 }); }
    b.command(Cmd::Hold(4));
    b.command(Cmd::Rewind(2.0));
    for k in 1..=30 {
        let t = 1.0 + k as f32 / 60.0;
        a.tick(t);
        b.tick(t);
        assert!(a.beat_time() == t && b.beat_time() == t);
        let (pa, pb) = (frame(&mut o, &mut a), frame(&mut o, &mut b));
        assert!(pa == pb, "frame {k} differs");
    }
    assert_eq!(a.beat(), amenui::beat::BeatNow { bar: 1, beat: 1, step: 1, phase: 0.0 });
    let _ = Driven::now(&a);
}

#[test]
fn on_the_beat_text_to_read_stays_where_it_is() {
    // A front passing the body: where its ink lies (the centre of its ink, weighted) is where it was at
    // rest, within half a pixel, every frame.
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for beat in [ON, Beat::default()] {
        let mut ui = open(beat, Material::Plain);
        settle(&mut ui, 2.0);
        let r = ui.node("body").expect("the body").rect;
        let (x0, y0, x1, y1) = ((r.x - 10.0) as u32, (r.y - 10.0) as u32, ((r.right() + 10.0) as u32).min(W), ((r.bottom() + 10.0) as u32).min(H));
        let centre = |px: &[u8]| {
            let g = &px[((y0 * W + x0) * 4) as usize..((y0 * W + x0) * 4 + 3) as usize].to_vec();
            let (mut sx, mut sy, mut sw) = (0.0f64, 0.0f64, 0.0f64);
            for y in y0..y1 { for x in x0..x1 {
                let i = ((y * W + x) * 4) as usize;
                let w = (0..3).map(|c| (px[i + c] as f64 - g[c] as f64).abs()).sum::<f64>();
                sx += w * x as f64; sy += w * y as f64; sw += w;
            } }
            [sx / sw.max(1.0), sy / sw.max(1.0)]
        };
        let rest = centre(&frame(&mut o, &mut ui));
        ui.command(Cmd::RipplrAt { key: "body".into(), strength: 1.0 });
        let mut most = 0.0f64;
        for k in 1..=45 {
            ui.tick(2.0 + k as f32 / 60.0);
            let c = centre(&frame(&mut o, &mut ui));
            most = most.max((c[0] - rest[0]).abs().max((c[1] - rest[1]).abs()));
        }
        if beat.on() { assert!(most <= 0.5, "on the beat the body moved {most:.2} px"); } else { eprintln!("off the beat the body moves {most:.2} px"); }
    }
}
