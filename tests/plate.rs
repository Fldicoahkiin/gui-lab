//! Plates (DESIGN.md §1.33): what a plate's subtree draws stays from frame to frame, ages out over
//! `fade_s`, and is emptied at once by `Cmd::ClearPlate`.
use amenui::el::{El, Paint, Plate, col, space, text};
use amenui::morph::Cmd;
use amenui::offscreen::Offscreen;
use amenui::style::{Look, Material, MATERIALS};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 240;
const H: u32 = 160;

/// A full-window plate with the strokes the state holds (each a dab at (x, y)), a line of text beside
/// it outside the plate, and `plates` more small plates along the bottom.
#[derive(Clone)]
struct S { m: Material, dark: bool, fade: f32, plate: bool, strokes: Vec<(f32, f32)>, loose: f32, cmds: Vec<Cmd>, invert: bool, plates: usize, beat: bool }
fn s0(m: Material) -> S { S { m, dark: false, fade: 0.0, plate: true, strokes: vec![], loose: 0.0, cmds: vec![], invert: false, plates: 0, beat: false } }
fn view(s: &S) -> El<()> {
    let mut board = col().key("board").fill_width().fill_height();
    for (k, &(x, y)) in s.strokes.iter().enumerate() { board = board.child(space(28.0, 10.0).radius(5.0).fill(Paint::INK).loose(s.loose).key(format!("dab{k}")).absolute(Some(x), Some(y), None, None)); }
    let board = if s.plate { board.plate(Plate { fade_s: s.fade }) } else { board };
    let mut page = col().key("page").fill_width().fill_height().child(board).child(text("Beside the board", 12.0, Paint::TEXT).key("note").absolute(Some(8.0), Some(4.0), None, None));
    for k in 0..s.plates {
        let mut small = col().key(format!("small{k}")).width(30.0).height(30.0).plate(Plate::default()).absolute(Some(8.0 + 40.0 * k as f32), Some(120.0), None, None);
        if !s.strokes.is_empty() { small = small.child(space(20.0, 20.0).fill(Paint::ACCENT).key(format!("small{k}/dab"))); }
        page = page.child(small);
    }
    page
}
fn look(s: &S) -> Look {
    let mut l = Look { material: s.m, theme: if s.dark { amenui::style::DARK } else { amenui::style::LIGHT }, ..Look::default() };
    l.post.invert = s.invert;
    if s.beat { l.beat = amenui::beat::Beat { bpm: 120.0, steps: 4, swing: 0.0 }; }
    l
}
fn mk(s: S) -> Ui<S, ()> { let mut ui = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(look).on_commands(|s| std::mem::take(&mut s.cmds)); ui.tick(1.0); ui }
fn frame(o: &mut Offscreen, ui: &mut Ui<S, ()>, t: f32) -> Vec<u8> { ui.tick(t); let l = ui.look(); o.render(ui, &l) }
fn set(ui: &mut Ui<S, ()>, f: impl FnOnce(&mut S)) { f(&mut ui.state); ui.relayout(); }
fn diff(a: &[u8], b: &[u8]) -> f32 { a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count() as f32 / (a.len() / 4) as f32 }
/// How much ink there is in the box (x, y, w, h): the summed difference from the empty board.
fn ink_in(px: &[u8], empty: &[u8], r: (u32, u32, u32, u32)) -> f32 {
    let mut s = 0.0;
    for y in r.1..r.1 + r.3 { for x in r.0..r.0 + r.2 { let i = ((y * W + x) * 4) as usize; for c in 0..3 { s += (px[i + c] as f32 - empty[i + c] as f32).abs(); } } }
    s
}

#[test]
fn a_stroke_stays_when_the_view_no_longer_has_it_and_clearing_is_as_never_drawn() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Plain, Material::Dottr] {
        let mut ui = mk(S { strokes: vec![(60.0, 60.0)], ..s0(m) });
        let drawn = frame(&mut o, &mut ui, 2.0);
        set(&mut ui, |s| s.strokes.clear());
        let after = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
        assert!(after == drawn, "{m:?}: the stroke went with the view ({:.4} differs)", diff(&after, &drawn));
        let mut never = mk(s0(m));
        let empty = frame(&mut o, &mut never, 2.0 + 2.0 / 60.0);
        assert!(empty != drawn);
        set(&mut ui, |s| s.cmds.push(Cmd::ClearPlate { key: "board".into() }));
        let cleared = frame(&mut o, &mut ui, 2.0 + 2.0 / 60.0);
        assert!(cleared == empty, "{m:?}: the cleared frame is not as never drawn ({:.4} differs)", diff(&cleared, &empty));
    }
}

#[test]
fn an_empty_plate_is_as_no_plate() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for (m, _) in MATERIALS.iter() { for dark in [false, true] {
        let mut a = mk(S { dark, ..s0(*m) });
        let mut b = mk(S { dark, plate: false, ..s0(*m) });
        assert!(frame(&mut o, &mut a, 3.0) == frame(&mut o, &mut b, 3.0), "{m:?} dark {dark}: an empty plate shows");
    } }
}

#[test]
fn ink_ages_out_over_fade_s_and_then_asks_for_no_frames() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let fade = 4.0;
    let mut never = mk(S { fade, ..s0(Material::Plain) });
    let empty = frame(&mut o, &mut never, 2.0);
    let mut ui = mk(S { fade, strokes: vec![(60.0, 60.0)], ..s0(Material::Plain) });
    let mut t = 2.0;
    let fresh = frame(&mut o, &mut ui, t);
    set(&mut ui, |s| s.strokes.clear());
    let at = |a: f32| 2.0 + a * fade;
    let mut last_ink = f32::MAX;
    let mut drawn = 0;
    let dab = (56, 56, 36, 18);
    while t < at(1.0) + 0.05 {
        t += 1.0 / 60.0;
        ui.tick(t);
        if !ui.needs_frame(t) { continue; }
        drawn += 1;
        let px = frame(&mut o, &mut ui, t);
        let a = (t - 2.0) / fade;
        let ink = ink_in(&px, &empty, dab);
        if a <= 0.25 + 1e-3 { assert!(px == fresh, "a {a:.3}: not as just drawn"); }
        if a < 0.5 { assert!((ink - ink_in(&fresh, &empty, dab)).abs() < 1.0, "a {a:.3}: the ink changed before half its fade"); }
        assert!(ink <= last_ink + 1.0, "a {a:.3}: the ink grew");
        last_ink = ink;
    }
    let gone = frame(&mut o, &mut ui, t);
    assert!(gone == empty, "at the end of its fade the plate is not empty ({:.4} differs)", diff(&gone, &empty));
    // Only the ageing asked for frames: at most 30 a second.
    assert!(drawn as f32 <= 30.0 * fade + 3.0, "{drawn} frames over {fade} s of ageing");
    let mut more = 0;
    for _ in 0..120 { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { more += 1; frame(&mut o, &mut ui, t); } }
    assert_eq!(more, 0, "frames after the ink had gone");
}

#[test]
fn fade_2_is_empty_two_seconds_after_the_last_stroke() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut never = mk(S { fade: 2.0, ..s0(Material::Dottr) });
    let empty = frame(&mut o, &mut never, 2.0);
    let mut ui = mk(S { fade: 2.0, strokes: vec![(30.0, 40.0)], ..s0(Material::Dottr) });
    frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| s.strokes.clear());
    let mut t = 2.0;
    let mut drawn = 0;
    while t < 4.0 { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; frame(&mut o, &mut ui, t); } }
    assert!(drawn <= 61, "{drawn} frames in the 2 s of ageing");
    let px = frame(&mut o, &mut ui, 4.05);
    assert!(px == empty, "2 s after: not an empty plate ({:.4} differs)", diff(&px, &empty));
}

#[test]
fn ink_on_a_plate_loosens_with_its_material() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, loose: f32| { let mut ui = mk(S { loose, strokes: vec![(60.0, 60.0)], ..s0(Material::Dottr) }); frame(o, &mut ui, 2.0); set(&mut ui, |s| s.strokes.clear()); frame(o, &mut ui, 2.0 + 1.0 / 60.0) };
    assert!(shot(&mut o, 1.0) != shot(&mut o, 0.0), "kept ink at looseness 1 and 0 is the same");
}

#[test]
fn clearing_and_inverting_in_one_frame_changes_little() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = mk(S { strokes: vec![(60.0, 60.0), (120.0, 90.0)], ..s0(Material::Plain) });
    let before = frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| { s.strokes.clear(); s.cmds.push(Cmd::ClearPlate { key: "board".into() }); s.invert = true; });
    let after = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
    // Inverted, the page is a different picture everywhere; what the cut changes beyond that is the
    // strokes' few pixels. Compare against the inverted frame of the page as it was.
    let mut inv = mk(S { strokes: vec![(60.0, 60.0), (120.0, 90.0)], invert: true, ..s0(Material::Plain) });
    let was = frame(&mut o, &mut inv, 2.0);
    let _ = before;
    assert!(diff(&after, &was) < 0.05, "{:.3} of the frame changed with the cut", diff(&after, &was));
}

#[test]
fn a_hold_keeps_a_plate_page_byte_for_byte() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = mk(S { fade: 2.0, beat: true, strokes: vec![(60.0, 60.0)], ..s0(Material::Dottr) });
    frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| { s.strokes.clear(); s.cmds.push(Cmd::Hold(8)); });
    let first = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
    for f in 2..40 { let px = frame(&mut o, &mut ui, 2.0 + f as f32 / 60.0); assert!(px == first, "frame {f} of the hold differs"); }
}

#[test]
fn five_plates_keep_four_and_draw_the_fifth_plainly() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = mk(S { plate: false, plates: 5, strokes: vec![(0.0, 0.0)], ..s0(Material::Plain) });
    frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| s.strokes.clear());
    let px = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
    let mut never = mk(S { plate: false, plates: 5, ..s0(Material::Plain) });
    let empty = frame(&mut o, &mut never, 2.0);
    for k in 0..5u32 {
        let kept = ink_in(&px, &empty, (13 + 40 * k, 125, 16, 16)) > 100.0;
        assert_eq!(kept, k < 4, "plate {k}: kept {kept}");
    }
    assert!(ui.problems().iter().any(|p| matches!(p, amenui::error::ViewProblem::PlateOver { key } if key == "small4")));
}

#[test]
fn a_new_window_size_empties_the_plates() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = mk(S { strokes: vec![(60.0, 60.0)], ..s0(Material::Plain) });
    frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| s.strokes.clear());
    ui.resize(W, H + 20, 1.0);
    ui.resize(W, H, 1.0);
    let mut o2 = Offscreen::try_new(W, H + 20).expect("adapter");
    let _ = frame(&mut o2, &mut ui, 2.0 + 1.0 / 60.0);
    // Back at the first size on the first renderer, which saw the window change: the plate is empty.
    let px = frame(&mut o, &mut ui, 2.0 + 2.0 / 60.0);
    let mut never = mk(s0(Material::Plain));
    let e = frame(&mut o, &mut never, 2.0 + 2.0 / 60.0);
    assert!(px == e, "the plate kept its ink across the new size ({:.4} differs)", diff(&px, &e));
}

#[test]
fn the_same_timeline_gives_the_same_frames_three_times() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let run = |o: &mut Offscreen| -> Vec<Vec<u8>> {
        let mut ui = mk(S { fade: 1.5, ..s0(Material::Dottr) });
        (0..300).map(|f| {
            if f % 20 == 0 { set(&mut ui, |s| s.strokes = vec![(10.0 + f as f32 * 0.6, 40.0 + (f % 60) as f32)]); }
            if f % 20 == 1 { set(&mut ui, |s| s.strokes.clear()); }
            frame(o, &mut ui, 2.0 + f as f32 / 60.0)
        }).collect()
    };
    let a = run(&mut o);
    for _ in 0..2 { let b = run(&mut o); for (f, (x, y)) in a.iter().zip(&b).enumerate() { assert!(x == y, "frame {f} differs"); } }
}

/// A page with a plate that holds a stroke; its button commits a page without the plate through the
/// Crack transition.
mod cracked {
    use amenui::el::{El, Paint, Plate, col, space, text};
    use amenui::style::{Look, Material};
    use amenui::widgets::button;
    pub struct S { pub alt: bool }
    pub fn view(s: &S) -> El<bool> {
        let mut page = col().key("page").fill_width().fill_height().pad(16.0, 16.0).gap(12.0)
            .child(button("swap", "Swap", !s.alt).commit());
        if s.alt { page = page.child(text("The page without a plate", 18.0, Paint::INK).key("other")); }
        else { page = page.child(col().key("board").width(400.0).height(200.0).child(space(300.0, 120.0).radius(12.0).fill(Paint::INK).key("dab").absolute(Some(40.0), Some(30.0), None, None)).plate(Plate::default())); }
        page
    }
    pub fn update(s: &mut S, m: bool) { s.alt = m; }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, transition: amenui::recipes::preset::cracked().transition, ..Look::default() } }
}

#[test]
fn halfway_through_a_crack_the_plates_ink_is_only_in_the_pieces_not_yet_turned() {
    const CW: u32 = 480;
    const CH: u32 = 300;
    let Ok(mut o) = Offscreen::try_new(CW, CH) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = Ui::new(cracked::S { alt: false }, cracked::view, cracked::update, fonts(), CW, CH, 1.0).on_look(cracked::look);
    let mut t = 0.0;
    let mut shot = |ui: &mut Ui<cracked::S, bool>, t: f32| { ui.tick(t); let l = ui.look(); let px = o.render(ui, &l); ui.frame_drawn(); px };
    while t < 1.0 { t += 1.0 / 60.0; shot(&mut ui, t); }
    let before = shot(&mut ui, t);
    let (x, y) = ui.node("swap").expect("the button").rect.center();
    ui.press(x, y);
    ui.release(x, y);
    let mut frames = vec![];
    for _ in 0..120 { t += 1.0 / 60.0; frames.push(shot(&mut ui, t)); }
    let after = frames.last().unwrap().clone();
    // The stroke's box, inside it.
    let (bx, by, bw, bh) = (16 + 40 + 8, 16 + 44 + 30 + 8, 284u32, 104u32);
    let mid = &frames[23];
    let (mut old, mut new, mut other) = (0, 0, 0);
    for yy in by..by + bh { for xx in bx..bx + bw {
        let i = ((yy * CW + xx) * 4) as usize;
        let p = &mid[i..i + 4];
        if p == &before[i..i + 4] { old += 1; } else if p == &after[i..i + 4] { new += 1; } else { other += 1; }
    } }
    assert!(old > 0 && new > 0, "halfway: {old} px still the old page, {new} the new");
    assert!(other as f32 <= 0.02 * (bw * bh) as f32, "{other} px of the stroke's box are neither page");
    // At the end the plate has gone with its page.
    let mut fresh = Ui::new(cracked::S { alt: true }, cracked::view, cracked::update, fonts(), CW, CH, 1.0).on_look(cracked::look);
    fresh.tick(0.5);
    let l = fresh.look();
    let f = o.render(&mut fresh, &l);
    // (The button keeps the focus the press gave it, so only the stroke's box is compared.)
    let same = (by..by + bh).all(|yy| { let i = ((yy * CW + bx) * 4) as usize; let j = i + (bw * 4) as usize; f[i..j] == after[i..j] });
    assert!(same, "the last frame still shows the old page's plate");
}

#[test]
fn what_is_on_a_plate_is_not_in_the_accessibility_tree() {
    use amenui::el::text;
    let v = |_: &u8| -> El<()> { col().key("page").child(text("Read me", 14.0, Paint::TEXT).key("off")).child(col().key("board").width(200.0).height(80.0).plate(Plate::default()).child(text("Drawn, not said", 14.0, Paint::TEXT).selectable().key("on"))) };
    let mut ui = Ui::new(0u8, v, |_, _| {}, fonts(), W, H, 1.0);
    ui.tick(1.0);
    let keys: Vec<String> = ui.a11y().into_iter().map(|n| n.key).collect();
    assert!(keys.iter().any(|k| k == "off") && !keys.iter().any(|k| k == "on" || k == "board"), "{keys:?}");
}

#[test]
fn new_ink_on_a_plate_is_there_at_once_without_condensing() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Plain, Material::Dottr] {
        let mut ui = mk(s0(m));
        frame(&mut o, &mut ui, 2.0);
        set(&mut ui, |s| s.strokes = vec![(60.0, 60.0)]);
        let first = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
        // The same stroke long settled on the page from the start.
        let mut settled = mk(S { strokes: vec![(60.0, 60.0)], ..s0(m) });
        let rest = frame(&mut o, &mut settled, 9.0);
        assert!(first == rest, "{m:?}: the stroke condensed ({:.4} differs)", diff(&first, &rest));
    }
}

/// The pixels of the box (x, y, w, h), RGB.
fn px_in(px: &[u8], r: (u32, u32, u32, u32)) -> Vec<[u8; 3]> {
    let mut v = vec![];
    for y in r.1..r.1 + r.3 { for x in r.0..r.0 + r.2 { let i = ((y * W + x) * 4) as usize; v.push([px[i], px[i + 1], px[i + 2]]); } }
    v
}
/// Tick from `from` up to `to` a frame at a time (the plate clock takes at most 0.1 s a tick), then draw.
fn frame_after(o: &mut Offscreen, ui: &mut Ui<S, ()>, from: f32, to: f32) -> Vec<u8> {
    let mut t = from;
    while t + 1.0 / 60.0 < to { t += 1.0 / 60.0; ui.tick(t); }
    frame(o, ui, to)
}
/// A dab's inside, away from its rounded ends and its edges.
const INSIDE: (u32, u32, u32, u32) = (66, 62, 16, 6);

#[test]
fn ageing_ink_fades_as_its_coverage_with_no_pattern() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let fade = 4.0;
    let mut never = mk(S { fade, ..s0(Material::Plain) });
    let empty = frame(&mut o, &mut never, 2.0);
    let mut ui = mk(S { fade, strokes: vec![(60.0, 60.0)], ..s0(Material::Plain) });
    let fresh = frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| s.strokes.clear());
    let (ink, paper) = (px_in(&fresh, INSIDE)[0], px_in(&empty, INSIDE)[0]);
    let mut last = f32::MAX;
    let mut at = 2.0;
    for k in 0..=10 {
        let a = 0.8 + 0.02 * k as f32;
        let px = frame_after(&mut o, &mut ui, at, 2.0 + a * fade);
        at = 2.0 + a * fade;
        let amount = ink_in(&px, &empty, (56, 56, 36, 18));
        assert!(amount <= last + 1.0, "a {a:.2}: the ink grew ({amount} after {last})");
        last = amount;
        if k != 5 { continue; }
        // a = 0.9: 1 − smoothstep(0.8, 1, 0.9) = a half; every pixel inside the stroke the same.
        let inside = px_in(&px, INSIDE);
        for c in 0..3 {
            let (lo, hi) = (inside.iter().map(|p| p[c]).min().unwrap(), inside.iter().map(|p| p[c]).max().unwrap());
            assert!(hi - lo <= 2, "a 0.9: the stroke's inside spans {lo}..{hi} in channel {c}: a pattern");
            let half = (ink[c] as f32 + paper[c] as f32) / 2.0;
            assert!((inside[0][c] as f32 - half).abs() <= 0.03 * 255.0, "a 0.9: {} in channel {c}, not halfway between ink {} and paper {}", inside[0][c], ink[c], paper[c]);
        }
    }
    // Just past the end of its fade (the plate clock goes in 1/120 s ticks): nothing.
    let px = frame_after(&mut o, &mut ui, at, 2.0 + 1.02 * fade);
    let left = ink_in(&px, &empty, (56, 56, 36, 18)); assert!(left < 1.0, "past the fade ink is left ({left}, {:.4} of the frame differs)", diff(&px, &empty));
}

#[test]
fn an_old_stroke_near_its_end_has_at_most_55_percent_of_a_new_ones_ink() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let fade = 4.0;
    let mut never = mk(S { fade, ..s0(Material::Plain) });
    let empty = frame(&mut o, &mut never, 2.0);
    let mut ui = mk(S { fade, strokes: vec![(30.0, 40.0)], ..s0(Material::Plain) });
    frame(&mut o, &mut ui, 2.0);
    set(&mut ui, |s| s.strokes = vec![(130.0, 90.0)]);
    let t_new = 2.0 + 0.4 * fade;
    let just = frame_after(&mut o, &mut ui, 2.0, t_new);
    set(&mut ui, |s| s.strokes.clear());
    // The old stroke at a 0.9, the new one at 0.5.
    let px = frame_after(&mut o, &mut ui, t_new, 2.0 + 0.9 * fade);
    let (old, new) = ((26, 36, 36, 18), (126, 86, 36, 18));
    let same = px_in(&px, new).iter().zip(px_in(&just, new)).filter(|(a, b)| **a == *b).count() as f32 / (new.2 * new.3) as f32;
    assert!(same >= 0.99, "the new stroke at a 0.5 is {:.1} % as just drawn", same * 100.0);
    let (io, inew) = (ink_in(&px, &empty, old), ink_in(&px, &empty, new));
    assert!(io <= 0.55 * inew, "the old stroke keeps {:.0} % of the new one's ink", io / inew * 100.0);
}

#[test]
fn a_new_stroke_makes_new_only_the_old_ink_under_its_own_outline() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let fade = 4.0;
    let mut never = mk(S { fade, ..s0(Material::Plain) });
    let empty = frame(&mut o, &mut never, 2.0);
    let mut ui = mk(S { fade, strokes: vec![(60.0, 60.0)], ..s0(Material::Plain) });
    let fresh = frame(&mut o, &mut ui, 2.0);
    // Later a second stroke overlaps the first one's right end (from x 74 on).
    set(&mut ui, |s| s.strokes = vec![(74.0, 64.0)]);
    frame_after(&mut o, &mut ui, 2.0, 2.0 + 0.5 * fade);
    set(&mut ui, |s| s.strokes.clear());
    let px = frame_after(&mut o, &mut ui, 2.0 + 0.5 * fade, 2.0 + 0.9 * fade);
    // The first stroke's inside 2 px or more left of the second's outline is as old as the first.
    let (ink, paper) = (px_in(&fresh, INSIDE)[0], px_in(&empty, INSIDE)[0]);
    for p in px_in(&px, (62, 61, 10, 8)) { for c in 0..3 {
        let half = (ink[c] as f32 + paper[c] as f32) / 2.0;
        assert!((p[c] as f32 - half).abs() <= 0.03 * 255.0, "old ink left of the new stroke is {} in channel {c}, not the half {half}: the new stroke made it new", p[c]);
    } }
}
