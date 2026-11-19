//! A stage that steps by itself (`Ui::on_step`, `Ui::stage_view`, DESIGN.md §1.35): when only its
//! piece changed, only it is viewed and drawn again, with no layout, and the frame is the one a whole
//! view, layout and paint of the page gives, instance for instance; a step that lays the stage out
//! otherwise views the whole page.
use amenui::el::{El, Paint, canvas, col, row, text};
use amenui::style::{Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Clone, Default)]
struct S { t: f32, n: [usize; 3], tall: bool }
/// Cell `k`'s stage: strokes that change in number and place as it steps, and a line of text.
fn stage(s: &S, k: usize) -> El<()> {
    let mut d = amenui::draw::Drawing::new(180.0, 120.0);
    for j in 0..s.n[k] {
        let y = 6.0 + (j * 7 % 100) as f32 + if k == 1 { (s.t * 3.0 + j as f32).sin() * 4.0 } else { 0.0 };
        d.line((8.0, y), (170.0 - (j * 13 % 60) as f32, y + 3.0), 1.5, Paint::TEXT);
    }
    col().key(format!("cell{k}/stage")).stage().width(180.0).height(if s.tall && k == 1 { 150.0 } else { 130.0 }).gap(2.0)
        .child(canvas(d).key(format!("cell{k}/d")).width(180.0))
        .child(text("ω = 1.20", 11.0, Paint::DIM))
}
fn view(s: &S) -> El<()> {
    let cell = |k: usize| col().key(format!("cell{k}")).width(190.0).gap(4.0).flow(format!("cell-{k}"))
        .child(text(format!("[ CELL {k} ]"), 12.0, Paint::TEXT))
        .child(stage(s, k))
        .child(text("a caption under the stage, long enough to wrap onto a second line", 12.0, Paint::DIM));
    col().key("page").pad(10.0, 10.0).gap(10.0)
        .child(text("The lab", 20.0, Paint::TEXT))
        .child(row().key("cells").gap(14.0).child(cell(0)).child(cell(1)).child(cell(2)))
        .child(text("Everything after the cells: a paragraph of text, long enough to wrap over a couple of lines here.", 14.0, Paint::TEXT).key("after").width(600.0))
}
/// Cell 1 steps; at 3 s it grows taller (laid out otherwise).
fn step(s: &mut S, t: f32) { s.t = t; s.n[1] = 12 + ((t * 7.0) as usize % 9); s.tall = t > 3.0; }
fn open(staged: bool) -> Ui<S, ()> {
    let s = S { t: 0.0, n: [20, 12, 16], tall: false };
    let u = Ui::new(s, view, |_, _| {}, fonts(), 640, 420, 1.0).on_look(|_| Look { material: Material::Plain, ..Look::default() });
    let mut u = if staged {
        u.on_step(|s, t| { step(s, t); vec!["cell1/stage".to_owned()] })
            .stage_view(|s, key| (0..3).find(|&k| key == format!("cell{k}/stage")).map(|k| stage(s, k)))
    } else {
        u.on_tick(|s, t| { step(s, t); true })
    };
    u.tick(1.0);
    u.settle();
    u
}
/// A frame: its instances, their owners, and what text holds.
type Frame = (Vec<amenui::scene::Instance>, Vec<(usize, u64)>, Vec<(u32, [f32; 2])>);
fn frame(u: &Ui<S, ()>) -> Frame {
    (u.scene().instances.clone(), u.owners().to_vec(), u.scene().holds.clone())
}

#[test]
fn a_stage_that_steps_alone_is_viewed_and_drawn_alone_and_the_frame_is_the_whole_one() {
    let (mut a, mut b) = (open(true), open(false));
    let mut t = 1.0;
    for f in 0..150 {
        t += 1.0 / 60.0;
        let c0 = a.perf().counters;
        a.tick(t);
        b.tick(t);
        a.settle();
        b.settle();
        let c1 = a.perf().counters;
        if t <= 3.0 {
            assert_eq!((c1.views - c0.views, c1.layouts - c0.layouts, c1.restages - c0.restages), (0, 0, 1), "frame {f}: one stage viewed alone");
        } else if t > 3.0 + 1.0 / 30.0 {
            assert_eq!(c1.views - c0.views, 0, "frame {f}: settled again");
        }
        let (fa, fb) = (frame(&a), frame(&b));
        assert_eq!(fa.0.len(), fb.0.len(), "frame {f}: instances");
        let diff = fa.0.iter().zip(&fb.0).position(|(x, y)| x != y);
        assert!(diff.is_none(), "frame {f}: instance {diff:?} differs from the whole page's: {:?} / {:?}", diff.map(|k| fa.0[k]), diff.map(|k| fb.0[k]));
        assert_eq!(fa.1, fb.1, "frame {f}: owners");
        assert_eq!(fa.2, fb.2, "frame {f}: what text holds");
        let boxes = |u: &Ui<S, ()>| u.nodes().iter().map(|n| (n.key.clone(), n.rect)).collect::<Vec<_>>();
        assert_eq!(boxes(&a), boxes(&b), "frame {f}: boxes");
    }
    // The taller stage was laid out with the page.
    assert!(a.node("cell1/stage").expect("the stage").rect.h > 140.0);
}
