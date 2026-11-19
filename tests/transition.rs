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
/// The largest difference of two pixels' colour channels.
fn gap_of(a: &[u8], b: &[u8]) -> i32 { a.iter().zip(b).take(3).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0) }

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
    // Past the middle few pieces are on their way: whole (under 15 % gone) or gone (over 85 %). The
    // pieces start to leave at 0.28, so this is 0.53.
    let f6 = at_progress(0.53);
    let mut gone = vec![0usize; sizes.len()];
    for i in 0..n { if id[i] != usize::MAX && new(f6, i) { gone[id[i]] += 1; } }
    let pieces: Vec<f32> = sizes.iter().zip(&gone).filter(|(s, _)| **s >= 200).map(|(s, g)| *g as f32 / *s as f32).collect();
    let between = pieces.iter().filter(|f| **f > 0.15 && **f < 0.85).count();
    eprintln!("{} pieces, {between} on their way at 0.53", pieces.len());
    assert!(pieces.len() >= 6, "only {} pieces", pieces.len());
    assert!(between * 4 <= pieces.len(), "{between} of {} pieces are partly gone at 0.53", pieces.len());
}

/// A note on the left and, on the right, a panel marked `commit_scope` whose button commits a new
/// material for the panel alone.
mod scoped {
    use amenui::el::{El, Paint, col, row, text};
    use amenui::style::{Look, Material};
    use amenui::widgets::button;
    pub struct S { pub alt: bool, pub scoped: bool }
    pub fn view(s: &S) -> El<bool> {
        let mut panel = col().key("panel").width(220.0).pad(16.0, 16.0).gap(12.0).border(1.0, Paint::FAINT)
            .material(if s.alt { Material::Halftonr } else { Material::Plain }.id())
            .child(text(if s.alt { "Halftone" } else { "Plain" }, 22.0, Paint::INK).key("panel/title"))
            .child(button("swap", "Swap", true).commit());
        if s.scoped { panel = panel.commit_scope(); }
        row().key("page").pad(24.0, 24.0).gap(40.0)
            .child(col().key("left").width(160.0).child(text("Outside the panel. Nothing here changes.", 15.0, Paint::TEXT).key("note")))
            .child(panel)
    }
    pub fn update(s: &mut S, m: bool) { s.alt = m; }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
}

#[test]
fn a_scoped_commit_plays_inside_its_box_and_nothing_outside_it_changes() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = amenui::ui::Ui::new(scoped::S { alt: false, scoped: true }, scoped::view, scoped::update, fonts(), W, H, 1.0).on_look(scoped::look);
    let mut t = 0.0;
    while t < 1.0 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    let before = o.render(&mut ui, &look);
    let b = ui.node("panel").expect("panel").rect;
    let within = |i: usize, g: f32| { let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5); x >= b.x.floor() - g && y >= b.y.floor() - g && x < b.right().ceil() + g && y < b.bottom().ceil() + g };
    let inside = |i: usize| within(i, 0.0);
    // What the material draws round the box (its nearness reach and a front's push) belongs to it.
    let reach = amenui::text::SPREAD as f32 + ui.look().fx.push;
    let (x, y) = ui.node("swap").expect("swap").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    assert!(ui.state.alt);
    let mut moved = 0;
    let mut last = vec![];
    for k in 0..150 {
        t += 1.0 / 60.0;
        ui.tick(t);
        let look = ui.look();
        let px = o.render(&mut ui, &look);
        let outside = px.chunks_exact(4).zip(before.chunks_exact(4)).enumerate().filter(|(i, (a, b))| !within(*i, reach) && a != b).count();
        assert_eq!(outside, 0, "frame {k}: {outside} px outside the panel changed");
        if px != last { moved += 1; }
        last = px;
    }
    assert!(moved > 10, "the panel changes over frames ({moved})");
    // Inside, the end is the new state drawn directly.
    let mut fresh = amenui::ui::Ui::new(scoped::S { alt: true, scoped: true }, scoped::view, scoped::update, fonts(), W, H, 1.0).on_look(scoped::look);
    let mut o2 = Offscreen::new(W, H);
    let mut tf = 0.0;
    // The pressed button kept the focus: so does the one drawn directly.
    if let Some(k) = ui.focus() { fresh.command(amenui::morph::Cmd::Focus { key: k.to_owned() }); }
    while tf < t - 1e-4 { tf += 1.0 / 60.0; fresh.tick(tf); }
    fresh.tick(t);
    let look = fresh.look();
    let direct = o2.render(&mut fresh, &look);
    let differ = last.chunks_exact(4).zip(direct.chunks_exact(4)).enumerate().filter(|(i, (a, b))| inside(*i) && a != b).count();
    assert_eq!(differ, 0, "{differ} px inside the panel differ from the new state drawn directly");
}

/// A panel marked `commit_scope` beside a note; its button commits the Crack for the panel alone, and
/// the panel's ground changes with it, so every pixel of it changes.
mod scoped_crack {
    use amenui::el::{El, Paint, col, row, text};
    use amenui::style::{Look, Material};
    use amenui::widgets::{Variant, button, button_as};
    pub struct S { pub alt: bool }
    pub fn view(s: &S) -> El<bool> {
        // The same elements before and after (nothing is born): only the panel's ground changes.
        let panel = col().key("panel").width(220.0).height(220.0).pad(16.0, 16.0).gap(12.0)
            .fill(Paint::tone(if s.alt { 0.3 } else { 0.08 }))
            .child(text("A panel", 22.0, Paint::INK).key("panel/title"))
            .child(button("swap", "Swap", !s.alt).commit())
            .commit_scope();
        row().key("page").pad(24.0, 24.0).gap(40.0)
            .child(col().key("left").width(160.0).child(text("Outside the panel. Nothing here changes.", 15.0, Paint::TEXT).key("note")))
            .child(panel)
    }
    /// The button itself changes too: the page's ground with a line round it, then the accent.
    pub fn view_button(s: &S) -> El<bool> {
        let panel = col().key("panel").width(220.0).height(220.0).pad(16.0, 16.0).gap(12.0)
            .fill(Paint::tone(if s.alt { 0.3 } else { 0.08 }))
            .child(text("A panel", 22.0, Paint::INK).key("panel/title"))
            .child(button_as("swap", "Swap", if s.alt { Variant::Accent } else { Variant::Default }, !s.alt).commit())
            .commit_scope();
        row().key("page").pad(24.0, 24.0).gap(40.0)
            .child(col().key("left").width(160.0).child(text("Outside the panel. Nothing here changes.", 15.0, Paint::TEXT).key("note")))
            .child(panel)
    }
    pub fn update(s: &mut S, m: bool) { s.alt = m; }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, transition: amenui::recipes::preset::cracked().transition, ..Look::default() } }
}

#[test]
fn a_scoped_crack_breaks_the_scope_into_pieces_of_its_own_size() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = amenui::ui::Ui::new(scoped_crack::S { alt: false }, scoped_crack::view, scoped_crack::update, fonts(), W, H, 1.0).on_look(scoped_crack::look);
    let mut t = 0.0;
    while t < 1.0 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    let before = o.render(&mut ui, &look);
    let b = ui.node("panel").expect("panel").rect;
    let (x, y) = ui.node("swap").expect("swap").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    let mut frames = vec![];
    for _ in 0..60 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); frames.push(o.render(&mut ui, &look)); }
    let last = frames.last().expect("frames").clone();
    // The transition takes the scope's share of the look's 800 ms.
    let page = (W as f32).hypot(H as f32);
    let dur = 0.8 * (b.w.hypot(b.h) / page).clamp(0.5, 1.0);
    let at = |x: f32| &frames[((x * dur * 60.0).round() as usize).clamp(1, frames.len()) - 1];
    let inside = |i: usize| { let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5); x >= b.x && y >= b.y && x < b.right() && y < b.bottom() };
    let changing: Vec<usize> = (0..(W * H) as usize).filter(|&i| inside(i) && gap_of(px(&before, i), px(&last, i)) >= 12).collect();
    assert!(changing.len() as f32 > 0.8 * b.w * b.h, "the panel changes: {} px", changing.len());
    // The new page shows where a pixel is nearer to it than to the old one (the press's front and the
    // focus marks still move a little over both).
    let new = |f: &[u8], i: usize| gap_of(px(f, i), px(&last, i)) < gap_of(px(f, i), px(&before, i));
    let shown = |f: &[u8]| changing.iter().filter(|&&i| new(f, i)).count() as f32 / changing.len() as f32;
    // At 0.3 the cracks are thin.
    let s3 = shown(at(0.3));
    assert!(s3 < 0.12, "{:.1} % of the panel shows at 0.3", s3 * 100.0);
    // No frame shows more than a third more of it than the one before: it never changes at a stroke.
    let shares: Vec<f32> = frames.iter().map(|f| shown(f)).collect();
    for (k, w) in shares.windows(2).enumerate() { assert!(w[1] - w[0] <= 0.35, "frame {k} to {}: {:.2} to {:.2}", k + 1, w[0], w[1]); }
    // The pieces, as the cracks have parted them once their run is done (0.4).
    let f35 = at(0.4);
    let n = (W * H) as usize;
    let mut id = vec![usize::MAX; n];
    let mut sizes = vec![];
    // A crack is anything that has moved off the old page (a thin one is drawn blended).
    let live = |i: usize| inside(i) && gap_of(px(f35, i), px(&before, i)) <= 6 && gap_of(px(&before, i), px(&last, i)) >= 12;
    for s0 in 0..n {
        if id[s0] != usize::MAX || !live(s0) { continue; }
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
                if id[j] == usize::MAX && live(j) { id[j] = k; stack.push(j); }
            }
        }
        sizes.push(size);
    }
    let f6 = at(0.6);
    let mut gone = vec![0usize; sizes.len()];
    for i in 0..n { if id[i] != usize::MAX && new(f6, i) { gone[id[i]] += 1; } }
    let pieces: Vec<f32> = sizes.iter().zip(&gone).filter(|(s, _)| **s >= 100).map(|(s, g)| *g as f32 / *s as f32).collect();
    let between = pieces.iter().filter(|f| **f > 0.15 && **f < 0.85).count();
    eprintln!("{} pieces, {between} on their way at 0.6; shown {:?}", pieces.len(), shares.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>());
    assert!(pieces.len() >= 8, "only {} pieces in the panel", pieces.len());
    assert!(between * 4 <= pieces.len(), "{between} of {} pieces are partly gone at 0.6", pieces.len());
}

#[test]
fn in_a_scoped_crack_the_pressed_button_goes_over_with_the_pieces_it_is_in() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = amenui::ui::Ui::new(scoped_crack::S { alt: false }, scoped_crack::view_button, scoped_crack::update, fonts(), W, H, 1.0).on_look(scoped_crack::look);
    let mut t = 0.0;
    while t < 1.0 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    let before = o.render(&mut ui, &look);
    let p = ui.node("panel").expect("panel").rect;
    let bb = ui.node("swap").expect("swap").rect;
    let (x, y) = bb.center();
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    let mut frames = vec![];
    for _ in 0..60 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); frames.push(o.render(&mut ui, &look)); }
    let last = frames.last().expect("frames").clone();
    let dur = 0.8 * (p.w.hypot(p.h) / (W as f32).hypot(H as f32)).clamp(0.5, 1.0);
    let at = |x: f32| ((x * dur * 60.0).round() as usize).clamp(1, frames.len()) - 1;
    let ix = |x: f32, y: f32| (y as u32 * W + x as u32) as usize;
    let changes = |i: usize| gap_of(px(&before, i), px(&last, i)) >= 24;
    let new = |f: &[u8], i: usize| gap_of(px(f, i), px(&last, i)) < gap_of(px(f, i), px(&before, i));
    // The button's own pixels that differ between the pages (its ground, line and label).
    let button: Vec<usize> = (bb.y as u32 + 1..bb.bottom() as u32 - 1).flat_map(|y| (bb.x as u32 + 1..bb.right() as u32 - 1).map(move |x| ix(x as f32 + 0.5, y as f32 + 0.5))).filter(|&i| changes(i)).collect();
    assert!(button.len() as f32 > 0.6 * bb.w * bb.h, "the button changes: {} px", button.len());
    let share = |f: &[u8], set: &[usize]| set.iter().filter(|&&i| new(f, i)).count() as f32 / set.len().max(1) as f32;
    // Until the pieces start to go the button is the old page's, as the panel round it is.
    for (f, frame) in frames.iter().enumerate().take(at(0.25)) {
        let s = share(frame, &button);
        assert!(s < 0.12, "frame {f}: {:.0} % of the button is the new one already", s * 100.0);
    }
    // A piece goes over whole, the button's part of it with the ground round it: just above and below
    // the button (4 px out and in), the two sides of its edge show the same page but where a crack runs.
    let (mut pairs, mut apart) = (0usize, 0usize);
    let mut parted = false;
    for f in &frames[..at(1.0)] {
        let s = share(f, &button);
        parted |= s > 0.15 && s < 0.85;
        for xx in (bb.x as u32 + 8..bb.right() as u32 - 8).step_by(2) {
            for (yi, yo) in [(bb.y + 4.0, bb.y - 4.0), (bb.bottom() - 5.0, bb.bottom() + 4.0)] {
                let (a, b) = (ix(xx as f32 + 0.5, yi), ix(xx as f32 + 0.5, yo));
                if !changes(a) || !changes(b) { continue; }
                pairs += 1;
                if new(f, a) != new(f, b) { apart += 1; }
            }
        }
    }
    eprintln!("button shown {:?}; {apart} of {pairs} edge pairs apart", frames[..at(1.0)].iter().map(|f| (share(f, &button) * 100.0).round()).collect::<Vec<_>>());
    assert!(parted, "the button is never part old and part new: it does not go with the pieces");
    assert!(pairs > 200 && (apart as f32) < 0.12 * pairs as f32, "{apart} of {pairs} pairs across the button's edge show different pages");
}

/// A panel marked `commit_scope` whose button commits `near` for the panel alone: near draws bands round
/// the panel's border, outside its box.
mod scoped_near {
    use amenui::el::{El, Paint, col, row, text};
    use amenui::style::{Look, Material};
    use amenui::widgets::button;
    pub struct S { pub alt: bool }
    pub fn view(s: &S) -> El<bool> {
        let panel = col().key("panel").width(200.0).pad(16.0, 16.0).gap(12.0).border(1.0, Paint::FAINT)
            .material(if s.alt { Material::Near } else { Material::Plain }.id())
            .child(text("A panel", 18.0, Paint::INK).key("panel/title"))
            .child(button("swap", "Swap", true).commit())
            .commit_scope();
        row().key("page").pad(24.0, 24.0).child(panel)
    }
    pub fn update(s: &mut S, m: bool) { s.alt = m; }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
}

#[test]
fn a_scoped_commit_is_over_when_its_front_has_crossed_the_box_and_the_marks_round_it_come_with_it() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = amenui::ui::Ui::new(scoped_near::S { alt: false }, scoped_near::view, scoped_near::update, fonts(), W, H, 1.0).on_look(scoped_near::look);
    let mut t = 0.0;
    while t < 1.0 { t += 1.0 / 60.0; ui.tick(t); let look = ui.look(); let _ = o.render(&mut ui, &look); }
    let look = ui.look();
    let before = o.render(&mut ui, &look);
    let b = ui.node("panel").expect("panel").rect;
    let reach = amenui::text::SPREAD as f32 + look.fx.push;
    // The ring the material draws round the box: outside it, within its reach.
    let ring = |i: usize| {
        let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5);
        let inside = |g: f32| x >= b.x.floor() - g && y >= b.y.floor() - g && x < b.right().ceil() + g && y < b.bottom().ceil() + g;
        inside(reach) && !inside(0.0)
    };
    let (x, y) = ui.node("swap").expect("swap").rect.center();
    let far = [(b.x, b.y), (b.right(), b.y), (b.x, b.bottom()), (b.right(), b.bottom())].iter().map(|c| (c.0 - reach - x).abs().hypot((c.1 - reach - y).abs()).max((c.0 + reach - x).abs().hypot((c.1 + reach - y).abs()))).fold(0.0, f32::max);
    let crossed = far / look.fx.wave_speed;
    let window = (W as f32).hypot(H as f32) / look.fx.wave_speed;
    assert!(crossed + 0.1 < window, "the test needs the box well inside the window ({crossed} s against {window} s)");
    ui.press(x, y);
    ui.release(x, y);
    ui.pointer_leave();
    let t0 = t;
    let (mut arrived, mut whole_after) = (None, None);
    // Frames draw only what changed again: a commit draws them whole until it is over.
    o.r.partial = true;
    while t < t0 + window - 0.05 {
        t += 1.0 / 60.0;
        ui.tick(t);
        let look = ui.look();
        let px = o.render(&mut ui, &look);
        let changed = px.chunks_exact(4).zip(before.chunks_exact(4)).enumerate().filter(|(i, (a, b))| ring(*i) && a != b).count();
        if changed > 0 && arrived.is_none() { arrived = Some(t - t0); }
        if t - t0 > crossed + 0.1 && o.r.last_region.is_none() && whole_after.is_none() { whole_after = Some(t - t0); }
    }
    let arrived = arrived.expect("near's marks round the panel never arrived");
    assert!(arrived <= crossed + 3.0 / 60.0, "near's marks round the panel arrived {arrived:.3} s after the press; the front crossed the box at {crossed:.3} s");
    assert!(whole_after.is_none(), "{:.3} s after the press the commit still draws whole frames; its front crossed the box at {crossed:.3} s", whole_after.unwrap_or(0.0));
}
