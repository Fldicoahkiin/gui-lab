//! A drawing in a flowed subtree whose picture grows or shrinks (`Cmd::Flow`, DESIGN.md §1.10), with no
//! view scale or other setting: its marks are geometry, so they ride with the picture's transform, their
//! pen kept, and are never drawn at their own place or size over the other page.
use amenui::draw::Drawing;
use amenui::el::{El, Paint, canvas, col, picture};
use amenui::morph::{Cmd, FlowSpec};
use amenui::scene::{Instance, PICTURE, SHAPE};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
const W: u32 = 800;
const H: u32 = 600;
const PIC: u32 = 62;
const SPEC: FlowSpec = FlowSpec { flow_ms: 420.0, flow_loose: 0.0, out_ms: 180.0, fade_ms: 180.0 };
/// The window on the index.
const WIN: [f32; 4] = [40.0, 100.0, 200.0, 150.0];
/// The curtain: rays 2 px apart (so they touch), from y 100 to 400, 1.5 px wide.
const RAYS: [f32; 5] = [300.0, 302.0, 304.0, 306.0, 308.0];
const TOP: f32 = 100.0;
const LEN: f32 = 300.0;
const PEN: f32 = 1.5;

#[derive(Default)]
struct Site { piece: bool, cmds: Vec<Cmd> }
fn at<M: 'static>(e: El<M>, x: f32, y: f32) -> El<M> { e.absolute(Some(x), Some(y), None, None) }
/// The piece at `s` of its size, laid out for it (no view scale); the curtain only on its page (the
/// index's window shows its picture).
fn layer(s: f32, curtain: bool) -> El<()> {
    let mut l = col().key("layer").width(W as f32 * s).height(H as f32 * s)
        .child(at(picture(PIC, H as f32 / W as f32).key("pic").width(W as f32 * s), 0.0, 0.0));
    if curtain {
        let mut d = Drawing::new(W as f32, H as f32);
        for x in RAYS { d.line((x, TOP), (x, TOP + LEN), PEN, Paint::ACCENT); }
        l = l.child(at(canvas(d).key("rays").width(W as f32), 0.0, 0.0));
    }
    l
}
fn view(s: &Site) -> El<()> {
    let root = col().key("root").width(W as f32).height(H as f32).clip();
    if s.piece { root.child(at(col().key("page").width(W as f32).height(H as f32).clip().flow("work").child(layer(1.0, true)), 0.0, 0.0)) }
    else { root.child(at(col().key("win").width(WIN[2]).height(WIN[3]).clip().flow("work").child(at(layer(WIN[3] / H as f32, false), 0.0, 0.0)), WIN[0], WIN[1])) }
}
fn open(piece: bool) -> Ui<Site, ()> {
    let mut u = Ui::new(Site { piece, cmds: vec![] }, view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() }).on_commands(|s| std::mem::take(&mut s.cmds));
    u.tick(3.0);
    u.settle();
    u
}
/// The rays as drawn now: shapes with ink, inside their clip.
fn rays(u: &Ui<Site, ()>) -> Vec<Instance> {
    u.scene().instances.iter().filter(|i| i.paint[3] == SHAPE && i.paint[2] > 0.01 && i.rect[2] > 0.5 && i.clip[2] > i.clip[0] && i.clip[3] > i.clip[1]).copied().collect()
}
fn picture_now(u: &Ui<Site, ()>) -> [f32; 4] { u.scene().instances.iter().find(|i| i.paint[3] == PICTURE && i.extra[1] as u32 == PIC && i.rect[2] > 1.0).expect("the picture").rect }

#[test]
fn a_shrinking_picture_takes_its_view_s_drawing_with_its_transform_and_pen() {
    let mut u = open(true);
    let t0 = 5.0;
    u.tick(t0);
    let p0 = picture_now(&u);
    u.state.piece = false;
    u.state.cmds.push(Cmd::Flow(SPEC));
    u.relayout();
    for f in 0..=24 {
        let t = t0 + f as f32 / 60.0;
        u.tick(t);
        let p = picture_now(&u);
        let k = p[2] / p0[2];
        let drawn = rays(&u);
        assert_eq!(drawn.len(), RAYS.len(), "frame {f}: every ray drawn once");
        let mut drawn = drawn;
        drawn.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
        for (j, r) in drawn.iter().enumerate() {
            let (x, want) = (r.rect[0] + r.rect[2] * 0.5, p[0] + (RAYS[j] - p0[0]) * k);
            assert!((x - want).abs() < 1.0, "frame {f}: ray {j} at x {x} where the picture takes it to {want}");
            assert!((r.rect[1] + r.rect[3] * 0.5 - (p[1] + (TOP + LEN * 0.5 - p0[1]) * k)).abs() < 1.0, "frame {f}: ray {j}'s middle off the picture's transform");
            assert!((r.rect[2] - r.rect[3] - LEN * k).abs() < 1.0, "frame {f}: ray {j} is {} long, the view makes it {}", r.rect[2] - r.rect[3], LEN * k);
            assert!((r.rect[3] - PEN).abs() < 0.01, "frame {f}: the pen is kept, not {}", r.rect[3]);
        }
    }
    u.tick(t0 + 1.0);
    assert!(rays(&u).is_empty(), "landed: the window shows its picture alone");
    u.tick(t0 + 1.5);
    u.frame_drawn();
    assert!(!u.needs_frame(t0 + 1.6), "frames wanted after landing");
}

#[test]
fn a_growing_picture_s_new_drawing_waits_until_it_lands() {
    let mut u = open(false);
    let t0 = 5.0;
    u.tick(t0);
    u.state.piece = true;
    u.state.cmds.push(Cmd::Flow(SPEC));
    u.relayout();
    for f in 0..=24 {
        u.tick(t0 + f as f32 / 60.0);
        assert!(rays(&u).is_empty(), "frame {f}: the curtain is not drawn at its page place over the index");
    }
    u.tick(t0 + 1.0);
    let drawn = rays(&u);
    assert_eq!(drawn.len(), RAYS.len());
    assert!(drawn.iter().all(|r| (r.rect[2] - r.rect[3] - LEN).abs() < 0.5), "landed at the page's size");
}
