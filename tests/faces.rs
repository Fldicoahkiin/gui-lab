//! Faces that come after the page starts (DESIGN.md §1.6.5): declared, their text is set in a stand-in;
//! when their bytes come each line sweeps over to them in 240 ms, and then the page is the page that
//! had them from the start.
use amenui::el::{El, Font, Paint, col, text};
use amenui::offscreen::Offscreen;
use amenui::scene::GLYPH;
use amenui::style::{Look, Material};
use amenui::text::{Face, GEIST_FILES, SERIF, SERIF_FILE};
use amenui::ui::Ui;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
fn read(f: &str) -> Vec<u8> { std::fs::read(format!("{DIR}/{f}")).expect("font") }
fn main3() -> Vec<Face> { amenui::text::bundled(read(amenui::text::SANS_FILE), read(amenui::text::PIXEL_FILE), read(amenui::text::MONO_FILE)) }
/// What the web page starts with: Geist Pixel and the serif declared.
fn declared() -> Vec<Face> { amenui::text::with_declared(main3()) }
/// Everything there from the start.
fn builtin() -> Vec<Face> { amenui::text::load_bundled(DIR).expect("fonts") }

/// A serif title and two lines of body text in Geist Pixel Square; `faces` overrides the two faces
/// (to write the page directly in the stand-ins).
#[derive(Clone, Copy)]
struct S { m: Material, title: u8, body: u8, body2: Option<u8> }
fn view(s: &S) -> El<()> {
    let mut page = col().key("page").fill_width().fill_height().pad(24.0, 20.0).gap(12.0)
        .child(text("Arrival of a face", 30.0, Paint::INK).font(Font::Face(s.title)).key("title"))
        .child(text("Body text set in its stand-in until the face comes, then swept over line by line.", 15.0, Paint::TEXT).font(Font::Face(s.body)).width(360.0).key("body"));
    if let Some(b) = s.body2 { page = page.child(text("A second face on the same page", 15.0, Paint::TEXT).font(Font::Face(b)).key("body2")); }
    page
}
fn look(s: &S) -> Look { Look { material: s.m, theme: amenui::style::LIGHT, ..Look::default() } }
const W: u32 = 460;
const H: u32 = 220;
fn mk(s: S, faces: Vec<Face>) -> Ui<S, ()> { let mut ui = Ui::new(s, view, |_, _| {}, faces, W, H, 1.0).on_look(look); ui.tick(1.0); ui }
fn page(m: Material) -> S { S { m, title: SERIF, body: amenui::text::GEIST_PIXEL[0], body2: None } }
fn frame(o: &mut Offscreen, ui: &mut Ui<S, ()>, t: f32) -> Vec<u8> { ui.tick(t); let l = ui.look(); let px = o.render(ui, &l); ui.frame_drawn(); px }

#[test]
fn a_declared_face_is_drawn_as_its_stand_in_and_is_wanted_only_when_used() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Plain, Material::Dottr] {
        let mut a = mk(page(m), declared());
        // The same page written in the stand-ins: the pixel face for Geist Pixel, the sans for the serif.
        let mut b = mk(S { title: amenui::text::SANS, body: amenui::text::PIXEL, ..page(m) }, builtin());
        assert!(frame(&mut o, &mut a, 2.0) == frame(&mut o, &mut b, 2.0), "{m:?}: not the stand-ins' page");
        let mut want = a.faces_wanted();
        want.sort();
        assert_eq!(want, vec![GEIST_FILES[0].to_owned(), SERIF_FILE.to_owned()], "exactly the faces the page uses");
    }
}

#[test]
fn a_face_added_before_the_first_frame_is_never_seen_standing_in() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut a = mk(page(Material::Dottr), declared());
    a.add_face(SERIF_FILE, read(SERIF_FILE)).expect("a font");
    a.add_face(GEIST_FILES[0], read(GEIST_FILES[0])).expect("a font");
    let mut b = mk(page(Material::Dottr), builtin());
    for f in 0..20 { let t = 1.0 + f as f32 / 60.0; assert!(frame(&mut o, &mut a, t) == frame(&mut o, &mut b, t), "frame {f} differs"); }
    assert!(a.faces_wanted().is_empty());
}

/// One arrival: each frame's pixels, the title's glyph instances, layouts, the frame at rest.
type Arrival = (Vec<Vec<u8>>, Vec<Vec<amenui::scene::Instance>>, u64, Vec<u8>);
/// Each frame from frame 30 at 60 fps with the serif added then: the pixels, the title's glyph instances.
fn arrival(o: &mut Offscreen, m: Material, both: bool) -> Arrival {
    let s = S { body2: both.then_some(amenui::text::GEIST_PIXEL[1]), ..page(m) };
    let mut ui = mk(s, declared());
    let mut t = 1.0;
    for _ in 0..30 { t += 1.0 / 60.0; frame(o, &mut ui, t); }
    let before = ui.counters().layouts;
    ui.add_face(SERIF_FILE, read(SERIF_FILE)).expect("a font");
    if both { ui.add_face(GEIST_FILES[1], read(GEIST_FILES[1])).expect("a font"); }
    let (mut px, mut glyphs) = (vec![], vec![]);
    for _ in 0..20 { t += 1.0 / 60.0; px.push(frame(o, &mut ui, t)); glyphs.push(ui.instances_of("title").into_iter().filter(|i| i.paint[3] == GLYPH).collect()); }
    let layouts = ui.counters().layouts - before;
    // Then the page rests.
    let mut drawn = 0;
    for _ in 0..120 { t += 1.0 / 60.0; ui.tick(t); if ui.needs_frame(t) { drawn += 1; frame(o, &mut ui, t); } }
    assert_eq!(drawn, 0, "{m:?}: {drawn} frames in the 2 s after the sweep");
    let last = frame(o, &mut ui, t);
    (px, glyphs, layouts, last)
}

#[test]
fn a_face_arriving_sweeps_each_line_in_240_ms_and_then_the_page_is_as_built_in() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Plain, Material::Dottr] {
        let (px, glyphs, layouts, last) = arrival(&mut o, m, false);
        assert_eq!(layouts, 1, "{m:?}: laid out {layouts} times");
        // From 240 ms (and the 60 ms the last glyph draws back in) on, the title is as built in.
        let mut reference = mk(page(m), declared());
        reference.add_face(SERIF_FILE, read(SERIF_FILE)).expect("a font");
        let r = frame(&mut o, &mut reference, 2.0);
        assert!(*px.last().unwrap() == r, "{m:?}: 330 ms in, not the page that had the face");
        assert!(last == r, "{m:?}: at rest, not the page that had the face");
        if m != Material::Plain { continue; }
        // Under plain: one set of glyphs at every moment, changed ones left of unchanged ones, and the
        // ink never less than 80 % of the lesser of the two faces'.
        let n0 = glyphs[0].len();
        let ink = |g: &[amenui::scene::Instance]| g.iter().map(|i| i.rect[2] * i.rect[3]).sum::<f32>();
        let (a0, a1) = (ink(&glyphs[0]), ink(glyphs.last().unwrap()));
        for (f, g) in glyphs.iter().enumerate() {
            assert_eq!(g.len(), n0, "frame {f}: {} glyphs, the title has {n0}", g.len());
            assert!(ink(g) >= 0.8 * a0.min(a1), "frame {f}: ink {:.0} of {:.0} / {:.0}", ink(g), a0, a1);
        }
        let serif_uv = |i: &amenui::scene::Instance, last: &[amenui::scene::Instance]| last.iter().any(|l| l.uv == i.uv);
        for (f, g) in glyphs.iter().enumerate() {
            let fin = glyphs.last().unwrap();
            let changed: Vec<f32> = g.iter().filter(|i| serif_uv(i, fin)).map(|i| i.rect[0]).collect();
            let not: Vec<f32> = g.iter().filter(|i| !serif_uv(i, fin)).map(|i| i.rect[0]).collect();
            if let (Some(a), Some(b)) = (changed.iter().cloned().reduce(f32::max), not.iter().cloned().reduce(f32::min)) { assert!(a < b + 1.0, "frame {f}: a changed glyph at {a} right of one not changed at {b}"); }
        }
    }
}

#[test]
fn two_faces_in_one_frame_change_in_one_layout_together() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let (_, glyphs, layouts, _) = arrival(&mut o, Material::Plain, true);
    assert_eq!(layouts, 1);
    // Both sweeps start at once: the first frame already shows the change starting in the title.
    assert!(glyphs.len() > 2);
}

#[test]
fn bytes_that_are_not_a_font_change_nothing_and_are_not_asked_for_again() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut ui = mk(page(Material::Dottr), declared());
    let before = frame(&mut o, &mut ui, 2.0);
    assert!(ui.add_face(SERIF_FILE, b"not a font".to_vec()).is_err());
    let after = frame(&mut o, &mut ui, 2.0 + 1.0 / 60.0);
    assert!(before == after, "the page changed");
    ui.relayout();
    assert_eq!(ui.faces_wanted(), vec![GEIST_FILES[0].to_owned()], "the broken face is not asked for again");
}

#[test]
fn the_same_arrival_three_times_gives_the_same_frames() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    let a = arrival(&mut o, Material::Dottr, false).0;
    for _ in 0..2 { assert!(arrival(&mut o, Material::Dottr, false).0 == a, "a run differs"); }
}

#[test]
fn assets_are_kept_by_name_and_asked_for_until_they_come() {
    let mut ui = mk(page(Material::Plain), declared());
    assert!(ui.asset("registry.json").is_none());
    assert_eq!(ui.assets_wanted(), vec!["registry.json".to_owned()]);
    ui.add_asset("registry.json", b"{}".to_vec());
    assert_eq!(ui.asset("registry.json"), Some(&b"{}"[..]));
    assert!(ui.assets_wanted().is_empty());
}
