//! The warm-up and the application's own preparation (DESIGN.md §1.23). A renderer made a piece at a
//! time (`Renderer::begin`) draws its first frames as plain, the same bytes as the plain look; once
//! warm and settled it draws the same bytes as one made at once (`Renderer::new`); it takes a bounded
//! number of steps, none of them doing most of the work; the material arrives on the page at rest, then
//! the page loosens to its caps over `RAMP` and condenses, a little each frame. `Ui::prepare` runs a job
//! a piece a frame and `Ui::ready` reads how far it is. The GPU tests need an adapter (lavapipe will do).
use amenui::offscreen::Offscreen;
use amenui::render::Stage;
use amenui::style::{Look, Material};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 640;
const H: u32 = 400;

fn page() -> amenui::ui::Ui<amenui::recipes::counter::Counter, amenui::recipes::counter::Msg> {
    amenui::recipes::counter::ui(fonts(), W, H, 1.0)
}

#[test]
fn a_warming_renderer_starts_as_plain_and_ends_as_the_whole_one() {
    let (Ok(mut cold), Ok(mut whole)) = (Offscreen::try_warming(W, H), Offscreen::try_new(W, H)) else { eprintln!("no GPU adapter here: skipped"); return };
    let (mut a, mut b) = (page(), page());
    a.tick(0.5);
    b.tick(0.5);
    // A look with a material, so plain and the whole look differ.
    let look = Look { material: Material::Dottr, ..a.look() };
    // The first frame, before any step: the plain look's frame, byte for byte.
    assert!(cold.r.warming());
    let first = cold.render(&mut a, &look);
    let plain = whole.render(&mut b, &amenui::render::plain(&look));
    assert!(first == plain, "the first frame of a warming renderer is the plain frame");
    // Step it (one piece a call), a frame between steps, timing each step.
    let mut steps = vec![];
    let mut t = 0.5;
    loop {
        let t0 = std::time::Instant::now();
        let p = cold.r.warm_step(0.0, a.text_mut());
        steps.push(t0.elapsed().as_secs_f64() * 1000.0);
        t += 1.0 / 60.0;
        a.tick(t);
        cold.render(&mut a, &look);
        if p.stage == Stage::Ready { break; }
        assert!(steps.len() < 40, "the warm-up ends");
    }
    assert!(steps.len() <= 32, "{} steps", steps.len());
    // Each step is one piece, and short even on a software renderer (a fixed cap: the whole warm-up
    // put back into one step would pass it only on a fast GPU). The longest piece is one pipeline.
    let total: f64 = steps.iter().sum();
    let most = steps.iter().copied().fold(0.0, f64::max);
    eprintln!("warm-up: {} steps, {total:.0} ms, longest {most:.0} ms: {steps:.0?}", steps.len());
    assert!(most <= 3000.0, "a step took {most:.0} ms");
    // Settled (the condensing done), both draw the same bytes.
    let end = t + 3.0;
    a.tick(end);
    b.tick(end);
    let warm = cold.render(&mut a, &look);
    let made = whole.render(&mut b, &look);
    assert!(warm == made, "a warmed renderer draws as one made at once");
}

#[test]
fn a_prepared_job_runs_a_piece_a_frame_and_reports_how_far_it_is() {
    struct S { done: u32 }
    fn view(s: &S) -> amenui::el::El<()> { amenui::el::text(format!("{}", s.done), 14.0, amenui::el::Paint::INK).key("n") }
    let mut ui = amenui::ui::Ui::new(S { done: 0 }, view, |_, _| {}, fonts(), 200, 100, 1.0);
    // Ten pieces of about a frame's budget each: never all in one frame.
    ui.set_prepare_budget(0.0);
    ui.prepare("marks", |s: &mut S| { s.done += 1; s.done as f32 / 10.0 });
    assert_eq!(ui.ready("marks"), 0.0);
    assert!(ui.needs_frame(0.0));
    let mut seen = vec![];
    let mut t = 0.0;
    while ui.ready("marks") < 1.0 {
        t += 1.0 / 60.0;
        ui.tick(t);
        seen.push(ui.ready("marks"));
        assert!(seen.len() <= 10, "done within ten frames");
    }
    assert_eq!(ui.state.done, 10);
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "progress only goes forward: {seen:?}");
    assert!(seen[0] < 0.5, "not all at once");
    // Done: nothing more to run, the page rests, and a key never asked for reads as ready.
    t += 1.0 / 60.0;
    ui.tick(t);
    ui.frame_drawn();
    assert!(!ui.needs_frame(t + 1.0 / 60.0) || ui.ongoing());
    assert_eq!(ui.ready("other"), 1.0);
    assert_eq!(ui.readiness(), 1.0);
}

#[test]
fn the_material_arrives_without_a_jump() {
    let Ok(mut cold) = Offscreen::try_warming(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut a = page();
    a.tick(0.5);
    let look = Look { material: Material::Dottr, ..a.look() };
    let mut t = 0.5;
    let mut prev = cold.render(&mut a, &look);
    // Warm up a step a frame; the first frame drawn warm is the one the material arrives in.
    let arrived = loop {
        cold.r.warm_step(0.0, a.text_mut());
        t += 1.0 / 60.0;
        a.tick(t);
        let warm = !cold.r.warming();
        let px = cold.render(&mut a, &look);
        if warm { break px; }
        prev = px;
    };
    let differ = prev.chunks(4).zip(arrived.chunks(4)).filter(|(x, y)| x != y).count();
    let share = differ as f32 / (W * H) as f32;
    assert!(share < 0.02, "the frame the material arrives in differs from plain's in {:.1} % of the pixels", share * 100.0);
    // Then at 60 Hz: the page loosens toward its caps and condenses, no element by more than 0.2 a frame.
    let keys: Vec<String> = a.nodes().iter().map(|n| n.key.clone()).filter(|k| !k.is_empty()).collect();
    let mut last: Vec<f32> = keys.iter().map(|k| a.looseness(k, t)).collect();
    let mut most = 0.0f32;
    let mut peak = 0.0f32;
    for _ in 0..90 {
        cold.r.warm_step(0.0, a.text_mut());
        t += 1.0 / 60.0;
        a.tick(t);
        cold.render(&mut a, &look);
        let now: Vec<f32> = keys.iter().map(|k| a.looseness(k, t)).collect();
        for (x, y) in last.iter().zip(&now) { most = most.max((x - y).abs()); }
        peak = peak.max(now.iter().copied().fold(0.0, f32::max));
        last = now;
    }
    assert!(most <= 0.2, "an element's looseness changed by {most:.2} in one frame");
    assert!(peak > 0.3, "the page comes apart a little as the material arrives ({peak:.2})");
}

/// Each legible element's ink on the sample page (its instances' boxes, a glyph's without the spread
/// around it), 2 px larger, device px.
fn legible_ink(ui: &amenui::ui::Ui<amenui::sample::Sample, ()>) -> Vec<(String, [i32; 4])> {
    let tree = ui.export_tree();
    let spread = amenui::text::SPREAD as f32;
    let mut ink: Vec<(String, [f32; 4])> = vec![];
    for (i, inst) in ui.scene().instances.iter().enumerate() {
        let Some(key) = ui.owner_key(i) else { continue };
        if key.is_empty() || !tree.find(key).is_some_and(|n| n.legible) || inst.paint[2] <= 0.0 { continue; }
        let p = if inst.paint[3] == amenui::scene::GLYPH { spread } else { 0.0 };
        let b = [inst.rect[0] + p, inst.rect[1] + p, inst.rect[0] + inst.rect[2] - p, inst.rect[1] + inst.rect[3] - p];
        match ink.iter_mut().find(|e| e.0 == key) {
            Some(e) => e.1 = [e.1[0].min(b[0]), e.1[1].min(b[1]), e.1[2].max(b[2]), e.1[3].max(b[3])],
            None => ink.push((key.to_owned(), b)),
        }
    }
    ink.into_iter().map(|(k, b)| (k, [(b[0] - 2.0).floor() as i32, (b[1] - 2.0).floor() as i32, (b[2] + 2.0).ceil() as i32, (b[3] + 2.0).ceil() as i32])).collect()
}

#[test]
fn text_to_read_and_controls_hold_still_as_the_material_arrives() {
    // The seven materials that loosen, dark and light: 0, 60, 120 and 300 ms after the material arrives,
    // the ink of every legible element (2 px larger) is its resting self: plain's frame for a material
    // that rests as plain, else the material's own rest (a grain or a ground of its own is not movement).
    let (w, h) = amenui::sample::CELL;
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let loosening = [Material::Dottr, Material::Halftonr, Material::Charr, Material::Smudgr, Material::Carvr, Material::Glitchr, Material::Scribblr];
    let page = |look: Look| { let mut ui = amenui::ui::Ui::new(amenui::sample::Sample { k: 0.0, label: "sample".into(), look, cmds: vec![] }, amenui::sample::view, |_, _| {}, fonts(), w, h, 1.0).on_look(|s| s.look); ui.relayout(); ui };
    let alike = |a: &[u8], b: &[u8], r: &[i32; 4]| {
        let (mut same, mut all) = (0usize, 0usize);
        for y in r[1].max(0)..r[3].min(h as i32) {
            for x in r[0].max(0)..r[2].min(w as i32) {
                let i = ((y as u32 * w + x as u32) * 4) as usize;
                all += 1;
                if a[i..i + 4] == b[i..i + 4] { same += 1; }
            }
        }
        same as f32 / all.max(1) as f32
    };
    let (mut worst, mut worst_plain) = ((1.0f32, String::new()), (1.0f32, String::new()));
    for material in loosening {
        let mut least = 1.0f32;
        for theme in [amenui::style::DARK, amenui::style::LIGHT] {
            let look = Look { material, theme, ..Look::default() };
            let plain = amenui::render::plain(&look);
            // At rest in the material, long settled.
            let mut settled = page(look);
            settled.tick(6.0);
            let rest = o.render(&mut settled, &look);
            let mut ui = page(look);
            ui.tick(3.0);
            // Before the material is ready, the page is drawn plain.
            let before = o.render(&mut ui, &plain);
            let rest = if amenui::sample::rests_as_plain(material) { before.clone() } else { rest };
            let boxes = legible_ink(&ui);
            assert!(boxes.iter().any(|b| b.0 == "body") && boxes.iter().any(|b| b.0 == "send"), "{boxes:?}");
            ui.condense_all();
            let start = ui.now();
            let mut t = start;
            let mut last: Vec<f32> = vec![];
            for ms in [0.0f32, 60.0, 120.0, 300.0] {
                let end = start + ms / 1000.0;
                while t < end - 1e-6 {
                    t = (t + 1.0 / 60.0).min(end);
                    ui.tick(t);
                    let now: Vec<f32> = ui.nodes().iter().map(|n| ui.looseness(&n.key, t)).collect();
                    if last.len() == now.len() { for (a, b) in last.iter().zip(&now) { assert!((a - b).abs() <= 0.2, "{}: a looseness jumped {a} → {b}", material.name()); } }
                    last = now;
                }
                let px = o.render(&mut ui, &look);
                for (key, b) in &boxes {
                    let (share, to_plain) = (alike(&px, &rest, b), alike(&px, &before, b));
                    let at = format!("{} ({}) `{key}` +{ms} ms", material.name(), theme.name);
                    if share < worst.0 { worst = (share, at.clone()); }
                    if to_plain < worst_plain.0 { worst_plain = (to_plain, at.clone()); }
                    least = least.min(to_plain);
                    assert!(share >= 0.99, "{at}: the same as at rest in {:.1} % of its pixels", share * 100.0);
                }
            }
        }
        eprintln!("{}: legible ink at least {:.1} % the same as plain's", material.name(), least * 100.0);
    }
    eprintln!("the least alike to its rest: {:.2} % ({}); to plain: {:.2} % ({})", worst.0 * 100.0, worst.1, worst_plain.0 * 100.0, worst_plain.1);
}
