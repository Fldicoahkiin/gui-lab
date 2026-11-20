//! The default theme's colours (DESIGN.md §4): the greys between the ground and the ink are the theme's
//! own opaque colours, drawn exactly where their paints are and straight between them; the scheme's
//! extreme; and the fills that answer the pointer (`El::fill_hover`, `El::fill_held`): a primary
//! button's plate under the pointer, a slider's thumb while it is dragged.
use amenui::el::{El, Paint, col, row, space};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, Greys, LIGHT, Look, Material, NORMCORE, Theme, hex};
use amenui::ui::Ui;
use amenui::widgets::{Spec, Variant, button_as, slider};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 480;
const H: u32 = 160;

#[derive(Clone, Default)]
struct Page { light: bool, theme: Option<Theme>, v: f32 }
#[derive(Clone, Debug)]
enum Msg { V(f32) }

const PAINTS: [Paint; 9] = [Paint::GROUND, Paint::RAISED, Paint::HAIRLINE, Paint::DIM, Paint::TEXT, Paint::INK, Paint::EXTREME, Paint::ACCENT, Paint::tone(0.76)];
fn swatches(_: &Page) -> El<Msg> {
    let mut r = row().key("row").pad(10.0, 10.0).gap(10.0);
    for (i, p) in PAINTS.iter().enumerate() { r = r.child(space(40.0, 40.0).key(format!("s{i}")).fill(*p)); }
    col().key("page").width(W as f32).height(H as f32).child(r)
}
fn controls(s: &Page) -> El<Msg> {
    col().key("page").width(W as f32).height(H as f32).pad(20.0, 20.0).gap(20.0)
        .child(button_as("save", "Save", Variant::Primary, Msg::V(0.0)))
        .child(slider("level", s.v, Spec { min: 0.0, max: 1.0, step: 0.01, decimals: 2, unit: "" }, 240.0, Msg::V))
}
fn theme(p: &Page) -> Theme { p.theme.unwrap_or(if p.light { LIGHT } else { DARK }) }
fn open(p: Page, view: fn(&Page) -> El<Msg>) -> Ui<Page, Msg> {
    let mut u = Ui::new(p, view, |s: &mut Page, m| match m { Msg::V(v) => s.v = v }, fonts(), W, H, 1.0)
        .on_look(|p| Look { material: Material::Plain, theme: theme(p), ..Look::default() });
    u.tick(3.0);
    u.settle();
    u
}
fn render(o: &mut Offscreen, u: &mut Ui<Page, Msg>) -> Vec<u8> { u.settle(); let l = u.look(); o.r.partial = false; o.render(u, &l) }
fn px(f: &[u8], x: f32, y: f32) -> [u8; 3] { let k = ((y as u32 * W + x as u32) * 4) as usize; [f[k], f[k + 1], f[k + 2]] }
fn byte(c: [f32; 3]) -> [u8; 3] { c.map(|v| (v * 255.0).round() as u8) }
fn near(a: [u8; 3], b: [u8; 3]) -> bool { (0..3).all(|k| a[k].abs_diff(b[k]) <= 1) }
fn centre(u: &Ui<Page, Msg>, key: &str) -> (f32, f32) { let r = u.node(key).expect(key).rect; (r.x + r.w * 0.5, r.y + r.h * 0.5) }

#[test]
fn the_greys_are_the_themes_own_colours_where_their_paints_are_drawn() {
    let mut o = Offscreen::new(W, H);
    for light in [false, true] {
        let mut u = open(Page { light, ..Page::default() }, swatches);
        let f = render(&mut o, &mut u);
        let th = if light { LIGHT } else { DARK };
        let g = th.greys;
        let want = [th.ground, g.raised, g.hairline, g.dim, g.text, th.ink, th.extreme(), th.accent];
        for (i, w) in want.iter().enumerate() {
            let (x, y) = centre(&u, &format!("s{i}"));
            assert!(near(px(&f, x, y), byte(*w)), "{} paint {i}: {:?}, the theme's {:?}", th.name, px(&f, x, y), byte(*w));
            assert!(near(byte(th.colour(PAINTS[i])), byte(*w)), "the CPU's colour of paint {i}");
        }
        // A tone between two greys is drawn straight between their colours (here dim and text).
        let (x, y) = centre(&u, "s8");
        let f8 = (0.76 - Paint::DIM.tone) / (Paint::TEXT.tone - Paint::DIM.tone);
        let mid: [f32; 3] = std::array::from_fn(|k| g.dim[k] + (g.text[k] - g.dim[k]) * f8);
        assert!(near(px(&f, x, y), byte(mid)), "between dim and text: {:?}, want {:?}", px(&f, x, y), byte(mid));
        assert!(near(byte(th.grey(0.76)), byte(mid)));
    }
    // The table the defaults take (AA.9).
    assert_eq!((DARK.ground, DARK.greys.dim, DARK.ink, DARK.accent, DARK.third), (hex(0x1a1b1d), hex(0x989da4), hex(0xf2f2ee), hex(0xf47a3c), hex(0xf2c94c)));
    assert_eq!((LIGHT.ground, LIGHT.greys.hairline, LIGHT.ink, LIGHT.accent, LIGHT.second), (hex(0xf1f0eb), hex(0xcbc9c1), hex(0x141517), hex(0xb23f0d), hex(0x1a5cc7)));
    assert_eq!((DARK.extreme(), LIGHT.extreme()), ([1.0; 3], [0.0; 3]));
}

#[test]
fn a_theme_without_greys_of_its_own_mixes_them_from_its_ground_and_ink() {
    let th = NORMCORE;
    assert_eq!(th.greys, Greys::mixed(th.ground, th.ink));
    let mut o = Offscreen::new(W, H);
    let mut u = open(Page { theme: Some(th), ..Page::default() }, swatches);
    let f = render(&mut o, &mut u);
    for (i, p) in PAINTS.iter().enumerate().filter(|(i, _)| ![6, 7].contains(i)) {
        let want: [f32; 3] = std::array::from_fn(|k| th.ground[k] + (th.ink[k] - th.ground[k]) * p.tone);
        let (x, y) = centre(&u, &format!("s{i}"));
        assert!(near(px(&f, x, y), byte(want)), "paint {i}: {:?}, the mix {:?}", px(&f, x, y), byte(want));
    }
}

#[test]
fn a_primary_button_is_an_ink_plate_that_turns_to_the_extreme_under_the_pointer() {
    let mut o = Offscreen::new(W, H);
    for light in [false, true] {
        let th = if light { LIGHT } else { DARK };
        let mut u = open(Page { light, ..Page::default() }, controls);
        let r = u.node("save").expect("save").rect;
        // The plate beside its label.
        let (x, y) = (r.x + 4.0, r.y + r.h * 0.5);
        let at = |u: &mut Ui<Page, Msg>, o: &mut Offscreen| px(&render(o, u), x, y);
        assert!(near(at(&mut u, &mut o), byte(th.ink)), "{} at rest: {:?}", th.name, at(&mut u, &mut o));
        u.pointer_move(x, y);
        assert!(near(at(&mut u, &mut o), byte(th.extreme())), "{} under the pointer: {:?}", th.name, at(&mut u, &mut o));
        u.pointer_move(W as f32 - 4.0, H as f32 - 4.0);
        assert!(near(at(&mut u, &mut o), byte(th.ink)), "{} left: {:?}", th.name, at(&mut u, &mut o));
    }
}

#[test]
fn a_slider_rests_in_ink_and_its_thumb_turns_to_the_accent_while_dragged() {
    let mut o = Offscreen::new(W, H);
    let th = DARK;
    let mut u = open(Page { v: 0.5, ..Page::default() }, controls);
    let r = u.node("level").expect("level").rect;
    let (x, y) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
    let f = render(&mut o, &mut u);
    assert!(near(px(&f, x, y), byte(th.ink)), "the thumb at rest: {:?}", px(&f, x, y));
    assert!(near(px(&f, r.x + 20.0, y), byte(th.ink)), "the filled run: {:?}", px(&f, r.x + 20.0, y));
    assert!(near(px(&f, r.right() - 20.0, y), byte(th.greys.hairline)), "the rest of the run: {:?}", px(&f, r.right() - 20.0, y));
    u.pointer_down(1, amenui::ui::PointerKind::Mouse, x, y, false);
    let f = render(&mut o, &mut u);
    assert!(near(px(&f, x, y), byte(th.accent)), "the thumb held: {:?}", px(&f, x, y));
    u.pointer_up(1, amenui::ui::PointerKind::Mouse, x, y);
    u.pointer_move(W as f32 - 4.0, H as f32 - 4.0);
    let f = render(&mut o, &mut u);
    assert!(near(px(&f, x, y), byte(th.ink)), "the thumb let go: {:?}", px(&f, x, y));
}
