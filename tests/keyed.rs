//! Keyed the same on both pages of a flow that grows (`Cmd::Flow`, DESIGN.md §1.10): an element the
//! index window and the piece both draw goes from its place in the window to its place on the page
//! with the picture's transform, its size on the same cubic, drawn once in every frame, and lands within
//! half a pixel of the new layout with no frame wanted after.
use amenui::el::{El, Paint, col, picture};
use amenui::morph::{Cmd, FlowSpec};
use amenui::scene::{Instance, PICTURE, SHAPE};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 800;
const H: u32 = 600;
const PIC: u32 = 61;
const SPEC: FlowSpec = FlowSpec { flow_ms: 420.0, flow_loose: 0.0, out_ms: 180.0, fade_ms: 180.0 };
/// The window: where it is on the index and how tall (its scale is its height over the page's).
const WIN: [f32; 4] = [40.0, 100.0, 200.0, 150.0];
/// The bird's box on the piece's one layout.
const BIRD: [f32; 4] = [500.0, 260.0, 60.0, 45.0];

#[derive(Default)]
struct Site { piece: bool, cmds: Vec<Cmd> }
fn at<M: 'static>(e: El<M>, x: f32, y: f32) -> El<M> { e.absolute(Some(x), Some(y), None, None) }
/// The piece's one layout, shown at `s`.
fn layer(s: f32) -> El<()> {
    col().key("layer").width(W as f32).height(H as f32).view_scale(s)
        .child(at(picture(PIC, H as f32 / W as f32).key("pic").width(W as f32), 0.0, 0.0))
        .child(at(col().key("bird").width(BIRD[2]).height(BIRD[3]).fill(Paint::ACCENT), BIRD[0], BIRD[1]))
}
fn view(s: &Site) -> El<()> {
    let root = col().key("root").width(W as f32).height(H as f32).clip();
    if s.piece { root.child(at(col().key("page").width(W as f32).height(H as f32).clip().flow("work").child(layer(1.0)), 0.0, 0.0)) }
    else { root.child(at(col().key("win").width(WIN[2]).height(WIN[3]).clip().flow("work").child(at(layer(WIN[3] / H as f32), 0.0, 0.0)), WIN[0], WIN[1])) }
}
fn open() -> Ui<Site, ()> {
    let mut u = Ui::new(Site::default(), view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() }).on_commands(|s| std::mem::take(&mut s.cmds));
    u.tick(3.0);
    u.settle();
    u
}
fn shapes(u: &Ui<Site, ()>) -> Vec<Instance> {
    u.scene().instances.iter().filter(|i| i.paint[3] == SHAPE && i.paint[2] > 0.01 && i.rect[2] > 0.5 && { let s = [i.rect[0].max(i.clip[0]), (i.rect[0] + i.rect[2]).min(i.clip[2])]; s[1] > s[0] }).copied().collect()
}
fn picture_now(u: &Ui<Site, ()>) -> [f32; 4] { u.scene().instances.iter().find(|i| i.paint[3] == PICTURE && i.extra[1] as u32 == PIC && i.rect[2] > 1.0).expect("the picture").rect }

fn flight(back: bool) {
    let mut u = open();
    if back { u.state.piece = true; u.relayout(); u.tick(4.0); u.settle(); }
    let t0 = 5.0;
    u.tick(t0);
    let old = u.instances_of("bird")[0].rect;
    let p0 = picture_now(&u);
    u.state.piece = !back;
    u.state.cmds.push(Cmd::Flow(SPEC));
    u.relayout();
    let mut p1 = [0.0; 4];
    for f in 0..=40 {
        let t = t0 + f as f32 / 60.0;
        u.tick(t);
        let birds = shapes(&u);
        assert_eq!(birds.len(), 1, "back {back}, frame {f}: the bird is drawn once, not {}", birds.len());
        let b = birds[0].rect;
        let p = picture_now(&u);
        if f == 0 { assert!((0..4).all(|k| (b[k] - old[k]).abs() < 0.5), "back {back}: at +0 the bird is where it was: {b:?}, {old:?}"); }
        if f == 40 { p1 = p; }
        // Its centre where the picture takes it; its size on the picture's cubic.
        let e = if (p[2] - p0[2]).abs() < 1e-3 { 0.0 } else { (p[2] - p0[2]) / (if back { WIN[2] } else { W as f32 } - p0[2]) };
        let (u0, v0) = ((old[0] + old[2] * 0.5 - p0[0]) / p0[2], (old[1] + old[3] * 0.5 - p0[1]) / p0[3]);
        let c = (p[0] + u0 * p[2], p[1] + v0 * p[3]);
        assert!((b[0] + b[2] * 0.5 - c.0).abs() < 1.5 && (b[1] + b[3] * 0.5 - c.1).abs() < 1.5, "back {back}, frame {f}: centre {:?}, the picture puts it at {c:?}", (b[0] + b[2] * 0.5, b[1] + b[3] * 0.5));
        let to = if back { BIRD[2] * WIN[3] / H as f32 } else { BIRD[2] };
        assert!((b[2] - (old[2] + (to - old[2]) * e)).abs() < 1.0, "back {back}, frame {f}: {} wide at e {e}", b[2]);
    }
    let _ = p1;
    let new = u.instances_of("bird")[0].rect;
    let want = if back { [WIN[0] + BIRD[0] * 0.25, WIN[1] + BIRD[1] * 0.25, BIRD[2] * 0.25, BIRD[3] * 0.25] } else { BIRD };
    assert!((0..4).all(|k| (new[k] - want[k]).abs() <= 0.5), "back {back}: landed at {new:?}, the layout has {want:?}");
    u.tick(t0 + 1.5);
    u.frame_drawn();
    assert!(!u.needs_frame(t0 + 1.6), "back {back}: frames wanted after landing");
}

#[test]
fn a_keyed_element_rides_the_growing_picture_onto_its_new_place() { flight(false); }
#[test]
fn and_back() { flight(true); }
