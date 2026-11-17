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
fn crack(o: &mut Offscreen, ms: f32) -> (Vec<u8>, Vec<Vec<u8>>) { let (b, f, _) = crack_from(o, ms); (b, f) }
/// The same, and where the press was (device px). Frame `k` (from 0) is at `(k + 1) / 48` of the
/// transition.
fn crack_from(o: &mut Offscreen, ms: f32) -> (Vec<u8>, Vec<Vec<u8>>, [f32; 2]) {
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
    (before, out, [x, y])
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
    assert_eq!(StepKind::CRACK, [120.0, 70.0, 0.6, 2.0, 1.0]);
    assert!(StepKind::ALL.contains(&StepKind::Crack));
    let p = amenui::recipes::preset::cracked();
    assert_eq!(p.transition.steps[0].kind, StepKind::Crack);
    assert!(p.transition.duration_ms <= 900.0);
}

#[test]
fn the_cracks_run_out_from_the_press_and_the_pieces_leave_whole() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let (before, frames, at) = crack_from(&mut o, 1000.0);
    let last = frames.last().expect("frames").clone();
    let n = (W * H) as usize;
    let new = |f: &[u8], i: usize| px(f, i) == px(&last, i) && px(f, i) != px(&before, i);
    let at_progress = |x: f32| &frames[((x * 48.0).round() as usize).saturating_sub(1)];
    let diag = ((W * W + H * H) as f32).sqrt();
    let far = |i: usize| { let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5); ((x - at[0]).powi(2) + (y - at[1]).powi(2)).sqrt() > 0.6 * diag };
    // A fifth of the way in the cracks have not reached what is far from the press.
    let f2 = at_progress(0.2);
    let reached = (0..n).filter(|&i| far(i) && new(f2, i)).count();
    assert_eq!(reached, 0, "{reached} px of the new page show far from the press at 0.2");
    // At 0.3 the cracks are thin: under 12 % of the page shows the new one.
    let f3 = at_progress(0.3);
    let shown = (0..n).filter(|&i| new(f3, i)).count() as f32 / n as f32;
    assert!(shown < 0.12, "{:.1} % shows at 0.3", shown * 100.0);
    // The pieces: what is still the old page once the cracks are all there (0.35), split by them.
    let f35 = at_progress(0.35);
    let mut id = vec![usize::MAX; n];
    let mut sizes = vec![];
    for s0 in 0..n {
        // Where the two pages are alike says nothing: it parts pieces like a crack.
        if id[s0] != usize::MAX || new(f35, s0) || px(&before, s0) == px(&last, s0) { continue; }
        let k = sizes.len();
        let mut stack = vec![s0];
        id[s0] = k;
        let mut size = 0;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i as u32 % W, i as u32 / W);
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 { continue; }
                let j = (ny as u32 * W + nx as u32) as usize;
                if id[j] == usize::MAX && !new(f35, j) && px(&before, j) != px(&last, j) { id[j] = k; stack.push(j); }
            }
        }
        sizes.push(size);
    }
    // At 0.6 few pieces are on their way: whole (under 15 % gone) or gone (over 85 %).
    let f6 = at_progress(0.6);
    let mut gone = vec![0usize; sizes.len()];
    for i in 0..n { if id[i] != usize::MAX && new(f6, i) { gone[id[i]] += 1; } }
    let pieces: Vec<f32> = sizes.iter().zip(&gone).filter(|(s, _)| **s >= 200).map(|(s, g)| *g as f32 / *s as f32).collect();
    let between = pieces.iter().filter(|f| **f > 0.15 && **f < 0.85).count();
    eprintln!("{} pieces, {between} on their way at 0.6", pieces.len());
    assert!(pieces.len() >= 6, "only {} pieces", pieces.len());
    assert!(between * 4 <= pieces.len(), "{between} of {} pieces are partly gone at 0.6", pieces.len());
}
