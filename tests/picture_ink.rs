//! Ink drawn over a picture is the ink's own colour, blended with the picture only by its coverage and
//! its opacity, whatever its siblings are doing (DESIGN.md §1.6.1).
use amenui::el::{El, Font, Paint, Scope, Stirr, col, picture, space, text};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 320;
const H: u32 = 120;
const PIC: u32 = 900;
/// The picture: one flat colour, a dark ridge's.
const RIDGE: [u8; 3] = [81, 73, 111];

#[derive(Clone, Copy)]
struct S { m: Material, title: f32, small: Option<f32>, layer: bool, loose: f32 }
fn view(s: &S) -> El<()> {
    let pic = picture(PIC, H as f32 / W as f32).key("pic").loose(s.loose).absolute(Some(0.0), Some(0.0), None, None).width(W as f32);
    let mut page = col().key("page").width(W as f32).height(H as f32).child(pic);
    // What the site did to keep ink over its picture readable: the ink in a layer of its own.
    if s.layer { page = page.child(space(0.0, 0.0).key("over").fill(Paint::GROUND.alpha(0.0)).stirr(Stirr { energy: 0.0, radius: Some(0.0), scope: Scope::Off }).pass(true)); }
    let title = text("ground", 48.0, Paint::INK).font(Font::Whisper).tracking(0.3).key("title");
    page = page.child(if s.title < 1.0 { title.opacity(s.title) } else { title }.absolute(Some(16.0), Some(10.0), None, None));
    if let Some(op) = s.small {
        let small = text("W-03 · 18:40 · 5 RIDGES", 11.0, Paint::DIM).font(Font::Mono).key("small");
        page = page.child(if op < 1.0 { small.opacity(op) } else { small }.absolute(Some(16.0), Some(90.0), None, None));
    }
    page
}
fn shot(o: &mut Offscreen, s: S) -> Vec<u8> {
    let mut ui = Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(|s| Look { material: s.m, theme: DARK, ..Look::default() });
    ui.tick(3.0);
    let l = ui.look();
    o.r.partial = false;
    o.render(&mut ui, &l)
}
fn rgb(a: &[u8], i: usize) -> [u8; 3] { [a[i * 4], a[i * 4 + 1], a[i * 4 + 2]] }

/// The title's pixels: how many are its ink at its opacity (within 2 / 255), and how many are neither
/// that nor a blend of it with the ridge by one coverage.
fn title_px(f: &[u8], want: [u8; 3]) -> (usize, usize) {
    let (mut full, mut odd) = (0, 0);
    for y in 8..70 { for x in 10..300 {
        let p = rgb(f, (y * W + x) as usize);
        if (0..3).all(|c| (p[c] as i32 - want[c] as i32).abs() <= 2) { full += 1; continue; }
        // A blend by coverage t is the same t in each channel (to a few levels of rounding).
        let t: Vec<f32> = (0..3).map(|c| (p[c] as f32 - RIDGE[c] as f32) / (want[c] as f32 - RIDGE[c] as f32)).collect();
        let lo = t.iter().copied().fold(f32::MAX, f32::min);
        let hi = t.iter().copied().fold(f32::MIN, f32::max);
        if hi - lo > 0.06 || lo < -0.02 || hi > 1.02 { odd += 1; }
    } }
    (full, odd)
}

#[test]
fn ink_over_a_picture_is_its_own_colour_whatever_its_siblings_do() {
    let mut o = Offscreen::new(W, H);
    o.r.set_picture(PIC, 4, 4, &[RIDGE[0], RIDGE[1], RIDGE[2], 255].repeat(16));
    let ink = DARK.ink.map(|v| (v * 255.0).round() as u8);
    let base = S { m: Material::Plain, title: 1.0, small: None, layer: false, loose: 0.0 };
    let mut fulls = vec![];
    for m in [Material::Plain, Material::Dottr] {
        for layer in [false, true] {
            for small in [None, Some(1.0), Some(0.5), Some(0.0)] {
                for title in [1.0, 0.6] {
                    let f = shot(&mut o, S { m, title, small, layer, ..base });
                    let want: [u8; 3] = std::array::from_fn(|c| (ink[c] as f32 * title + RIDGE[c] as f32 * (1.0 - title)).round() as u8);
                    let (full, odd) = title_px(&f, want);
                    assert!(odd == 0, "{m:?}, own layer {layer}, small line {small:?}, title at {title}: {odd} pixels are neither {want:?} nor a blend of it with the picture");
                    assert!(full > 1000, "{m:?}, own layer {layer}, small line {small:?}, title at {title}: only {full} pixels of the title are {want:?}");
                    if title == 1.0 { fulls.push(full); }
                }
            }
        }
    }
    // The same pixels are full ink whatever the siblings do.
    assert!(fulls.iter().all(|&n| n == fulls[0]), "{fulls:?}");
    // Text to read over a picture loosened under it stays as it is.
    let held = shot(&mut o, S { m: Material::Dottr, loose: 1.0, ..base });
    let (full, _) = title_px(&held, ink);
    assert_eq!(full, fulls[0], "the title over a loosened picture");
}
