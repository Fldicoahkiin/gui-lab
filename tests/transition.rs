//! The Crack transition (DESIGN.md §4.1): the `preset` recipe's Crack button commits a look that
//! arrives through cracks. Its last frame is the new page drawn directly; a tenth of the way in, less
//! than a tenth of what changes shows the new page; what shows the new page only grows; the same
//! numbers give the same bytes. Needs a GPU adapter (lavapipe will do).
use amenui::offscreen::Offscreen;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 480;
const H: u32 = 300;

/// The frames from the press on the Crack button to `ms` after it, one every 1/60 s, and the frame
/// before the press.
fn crack(o: &mut Offscreen, ms: f32) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut ui = amenui::recipes::preset::ui(fonts(), W, H, 1.0);
    ui.relayout();
    let mut t = 0.0;
    while t < 1.0 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
    let look = ui.look();
    let before = o.render(&mut ui, &look);
    let (x, y) = ui.node("crack").expect("the Crack button").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    let start = t;
    let mut out = vec![];
    while t < start + ms / 1000.0 - 1e-6 {
        t += 1.0 / 60.0;
        ui.tick(t);
        let look = ui.look();
        out.push(o.render(&mut ui, &look));
        ui.frame_drawn();
    }
    (before, out)
}

fn px(f: &[u8], i: usize) -> &[u8] { &f[i * 4..i * 4 + 4] }

#[test]
fn the_page_cracks_open_onto_the_new_one() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let (before, frames) = crack(&mut o, 1400.0);
    let last = frames.last().expect("frames").clone();
    // What the transition changes: where the page before and after differ.
    let changing: Vec<usize> = (0..(W * H) as usize).filter(|&i| px(&before, i) != px(&last, i)).collect();
    assert!(changing.len() > (W * H / 4) as usize, "only {} px change", changing.len());
    let shown = |f: &[u8]| changing.iter().filter(|&&i| px(f, i) == px(&last, i)).count() as f32 / changing.len() as f32;
    let shares: Vec<f32> = frames.iter().map(|f| shown(f)).collect();
    // It starts at the commit and takes its 800 ms: the first frame that shows any of the new page.
    let first = shares.iter().position(|s| *s > 0.0).expect("the new page shows");
    eprintln!("shown from frame {first}: {:?}", shares.iter().map(|s| (s * 1000.0).round() / 1000.0).collect::<Vec<_>>());
    // A tenth of the way in: under a tenth.
    let tenth = first + (0.1f32 * 48.0).round() as usize;
    assert!(shares[tenth] < 0.1, "a tenth of the way in, {:.1} % shows the new page", shares[tenth] * 100.0);
    // It only grows.
    for w in shares.windows(2) { assert!(w[1] + 1e-6 >= w[0], "what shows the new page shrank: {shares:?}"); }
    // The end of the transition (800 ms = 48 frames) is the new page as drawn directly afterwards.
    let end = first + 48;
    assert!(frames[end] == last, "the frame at the end of the transition is not the new page (it differs in {} px)", (0..(W * H) as usize).filter(|&i| px(&frames[end], i) != px(&last, i)).count());
    // Halfway, the page is still between the two.
    let mid = shares[first + 24];
    assert!(mid > 0.1 && mid < 0.95, "halfway {mid}");
    // The same numbers, the same bytes.
    let mut o2 = Offscreen::try_new(W, H).expect("adapter");
    let (_, again) = crack(&mut o2, 1400.0);
    for (i, (a, b)) in frames.iter().zip(&again).enumerate() { assert!(a == b, "frame {i} differs between two runs"); }
}

#[test]
fn crack_has_its_numbers() {
    use amenui::preset::StepKind;
    assert_eq!(StepKind::Crack.params(), ["裂块", "角度", "偏向", "毛边", "硬边"]);
    assert_eq!(StepKind::CRACK, [120.0, 70.0, 0.6, 3.0, 1.0]);
    assert!(StepKind::ALL.contains(&StepKind::Crack));
    let p = amenui::recipes::preset::cracked();
    assert_eq!(p.transition.steps[0].kind, StepKind::Crack);
    assert!(p.transition.duration_ms <= 900.0);
}
