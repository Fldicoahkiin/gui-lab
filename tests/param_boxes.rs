//! Number controls in the panels that hold them (`widgets::caption`, the Lab's `experiment::frame`,
//! a bare `widgets::number`): however long a label and however narrow a width, no control's box
//! overlaps another's or reaches out of its parent, at 1 and 2 device px a px, left to right and
//! right to left. A label that leaves its steps no room puts them on a line under it; one wider than
//! the control ends in an ellipsis and is read whole.
use amenui::el::{El, col};
use amenui::experiment::{Params, frame, spec};
use amenui::fieldmap::MAP_PARAMS;
use amenui::ui::Rect;
use amenui::style::{Look, Material};
use amenui::ui::Ui;
use amenui::widgets::{CaptionParam, Spec, caption, number};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const PX: Spec = Spec { min: 0.0, max: 100.0, step: 1.0, decimals: 0, unit: "px" };
/// The decode page's numbers, as the site has them.
const DECODE: [(&str, Spec); 8] = [("u", spec(0.0, 1.0, 0.01, 2, "")), ("tier0", spec(2.0, 64.0, 1.0, 0, "px")), ("tier1", spec(2.0, 64.0, 1.0, 0, "px")), ("tier2", spec(2.0, 64.0, 1.0, 0, "px")), ("tier3", spec(2.0, 64.0, 1.0, 0, "px")),
    ("stagger", spec(0.0, 400.0, 5.0, 0, "ms")), ("duration", spec(100.0, 4000.0, 10.0, 0, "ms")), ("seed", spec(0.0, 999.0, 1.0, 0, ""))];
/// A Lab experiment's numbers: short, long and one longer than any control.
static LAB: [(&str, Spec); 5] = [("k", spec(0.0, 1.0, 0.01, 2, "")), ("stirr_radius", spec(0.0, 100.0, 1.0, 0, "px")), ("breath_s", spec(0.0, 10.0, 0.1, 1, "s")),
    ("the_radius_of_the_field_in_device_pixels", spec(0.0, 100.0, 1.0, 0, "px")), ("drift", spec(0.0, 1.0, 0.05, 2, ""))];

#[derive(Clone, Copy)]
struct Page { rtl: bool }
fn view(p: &Page) -> El<()> {
    let stirr: Vec<CaptionParam> = MAP_PARAMS.iter().map(|&(n, min, max, step, decimals, unit)| CaptionParam::new(n, min, Spec { min, max, step, decimals, unit })).collect();
    let decode: Vec<CaptionParam> = DECODE.iter().map(|&(n, s)| CaptionParam::new(n, s.min, s)).collect();
    let repro = [CaptionParam::new("stirr_radius", 48.0, PX), CaptionParam::new("flow", 1.0, PX)];
    let mut lab = Params::new(&LAB, &[0.3, 48.0, 2.0, 12.0, 0.5]);
    lab.open = true;
    let formula = "v = (1 − (d/R)²)², R = 48 + ⅓ short side · k = max(own, e) · breath = 0.30 sin(2π t / breath_s)".to_owned();
    col().key("page").width(1000.0).gap(12.0).rtl(p.rtl)
        .child(col().key("box/repro").width(720.0).child(caption("repro", "x", &repro, |_, _| ())))
        .child(col().key("box/stirr").width(720.0).child(caption("stirr", "x", &stirr, |_, _| ())))
        .child(col().key("box/decode").width(480.0).child(caption("decode", "x", &decode, |_, _| ())))
        .child(col().key("box/lab").width(360.0).child(frame("lab", "mountain", "drag", col(), formula, &lab, |_, _| (), "dottr")))
        .child(number("bare", "stirr_radius", 48.0, PX, None, |_| ()).width(208.0))
        .child(number("narrow", "stirr_radius", 48.0, PX, None, |_| ()).width(120.0))
}
fn open(dpr: f32, rtl: bool) -> Ui<Page, ()> {
    let mut u = Ui::new(Page { rtl }, view, |_, _| {}, fonts(), (1000.0 * dpr) as u32, (2400.0 * dpr) as u32, dpr).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    u.tick(1.0);
    u.settle();
    u
}
fn overlap(a: Rect, b: Rect) -> bool { a.x < b.right() - 0.5 && b.x < a.right() - 0.5 && a.y < b.bottom() - 0.5 && b.y < a.bottom() - 0.5 }
fn inside(a: Rect, b: Rect) -> bool { a.x >= b.x - 0.5 && a.y >= b.y - 0.5 && a.right() <= b.right() + 0.5 && a.bottom() <= b.bottom() + 0.5 }

#[test]
fn no_number_control_overlaps_another_or_leaves_its_parent() {
    for dpr in [1.0f32, 2.0] { for rtl in [false, true] {
        let u = open(dpr, rtl);
        let nodes = u.nodes();
        let at = |i: usize| nodes[i].rect;
        // Each number control: its box, and its parts (label, steps, field).
        let controls: Vec<usize> = (0..nodes.len()).filter(|&i| nodes.iter().any(|n| n.key == format!("{}/more", nodes[i].key))).collect();
        assert!(controls.len() >= 12 + 8 + 2 + 5 + 2, "dpr {dpr} rtl {rtl}: {} controls", controls.len());
        let mut parts: Vec<(usize, String)> = vec![];
        for &c in &controls {
            let key = &nodes[c].key;
            // Every box under the control lies inside it, and the control inside its parent.
            for (j, n) in nodes.iter().enumerate() {
                let mut p = n.parent;
                while let Some(q) = p { if q == c { break; } p = nodes[q].parent; }
                if p != Some(c) || n.rect.w <= 0.0 { continue; }
                assert!(inside(n.rect, at(c)), "dpr {dpr} rtl {rtl}: `{}` {:?} reaches out of `{key}` {:?}", n.key, n.rect, at(c));
                if n.parent == Some(c) && n.key != format!("{key}/value") { parts.push((j, key.clone())); }
            }
            if let Some(p) = nodes[c].parent { assert!(inside(at(c), nodes[p].rect), "dpr {dpr} rtl {rtl}: `{key}` {:?} reaches out of `{}` {:?}", at(c), nodes[p].key, nodes[p].rect); }
            for k in ["less", "value", "more"] { parts.push((nodes.iter().position(|n| n.key == format!("{key}/{k}")).expect("a part"), key.clone())); }
        }
        // No two controls overlap, and within a control no two parts do.
        for (a, &x) in controls.iter().enumerate() { for &y in &controls[a + 1..] {
            assert!(!overlap(at(x), at(y)), "dpr {dpr} rtl {rtl}: `{}` {:?} overlaps `{}` {:?}", nodes[x].key, at(x), nodes[y].key, at(y));
        } }
        let leaves: Vec<&(usize, String)> = parts.iter().filter(|(j, _)| nodes[*j].rect.w > 0.0 && !controls.contains(j)).collect();
        for (a, (x, kx)) in leaves.iter().enumerate() { for (y, ky) in &leaves[a + 1..] {
            if kx != ky || nodes[*x].parent == Some(*y) || nodes[*y].parent == Some(*x) { continue; }
            assert!(!overlap(at(*x), at(*y)), "dpr {dpr} rtl {rtl}: in `{kx}`, `{}` {:?} overlaps `{}` {:?}", nodes[*x].key, at(*x), nodes[*y].key, at(*y));
        } }
        // The panels hold what they show: the Lab's formula wraps inside its cell.
        let cell = u.node("lab").expect("the cell").rect;
        let f = u.node("lab/formula").expect("the formula").rect;
        assert!(inside(f, cell), "dpr {dpr} rtl {rtl}: the formula {f:?} leaves its cell {cell:?}");
        assert!(f.h > 20.0, "dpr {dpr} rtl {rtl}: the formula is cut to one line ({f:?})");
        // A bare number at 208 keeps its +; one at 120 is as wide as its steps and field.
        let (bare, more) = (u.node("bare").expect("bare").rect, u.node("bare/more").expect("+").rect);
        assert!(inside(more, bare), "dpr {dpr} rtl {rtl}: the bare number's + {more:?} outside {bare:?}");
        assert!(u.node("narrow").expect("narrow").rect.w >= 2.0 * 30.0 + 96.0 + 12.0 - 0.5);
    } }
}

#[test]
fn a_label_wider_than_its_control_ends_in_an_ellipsis_and_is_read_whole() {
    let u = open(1.0, false);
    let long = "the_radius_of_the_field_in_device_pixels";
    let i = u.nodes().iter().position(|n| n.key.starts_with("lab/p3/")).expect("the long label");
    let key = u.nodes()[i].key.clone();
    let n = &u.nodes()[i];
    assert!(u.clamped(&key), "`{key}` is not cut: {:?} in {:?}", n.rect, u.node("lab/p3").map(|c| c.rect));
    assert!(u.a11y().iter().any(|n| n.name.contains(long) || n.text.as_deref().is_some_and(|t| t.contains(long))), "the whole label is read");
}
