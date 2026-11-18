//! A flow from an index of windows to a piece and back (`Cmd::Flow`, DESIGN.md §1.10): the window's
//! picture grows into the piece's as one picture in one rect on whole pixels, the index's ground around
//! it; the notes over the picture keep their own size and colour, the old ones riding with the picture
//! as they fade and the new ones coming in once it has landed; the title is drawn once, over the picture;
//! the picture does not start over.
use amenui::el::{El, Font, Paint, col, picture, text};
use amenui::morph::{Cmd, Ease, FlowSpec};
use amenui::offscreen::Offscreen;
use amenui::scene::{Instance, PICTURE};
use amenui::style::{DARK, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 800;
const H: u32 = 600;
const PIC_A: u32 = 41;
const PIC_B: u32 = 42;
const INDEX_GROUND: [f32; 3] = [0.32, 0.26, 0.58];
const PIECE_GROUND: [f32; 3] = [0.92, 0.84, 0.30];
/// The site's settings for the index → piece flow.
const SPEC: FlowSpec = FlowSpec { flow_ms: 420.0, flow_loose: 0.0, out_ms: 180.0, fade_ms: 180.0 };

#[derive(Default)]
struct Site { piece: bool, cmds: Vec<Cmd> }
fn at<M: 'static>(e: El<M>, x: f32, y: f32) -> El<M> { e.absolute(Some(x), Some(y), None, None) }
/// A picture with a note and a box over it, as the site's picture layer: `w × h`, the note and the box
/// where the page puts them.
fn layer(key: &str, (w, h): (f32, f32), pic: u32, note: (f32, f32), bx: [f32; 4]) -> El<()> {
    col().key(key).width(w).height(h)
        .child(at(picture(pic, 0.75).key(format!("{key}/picture")).width(w), 0.0, 0.0))
        .child(at(text("note", 11.0, Paint::ACCENT).font(Font::Mono).key(format!("{key}/note")), note.0, note.1))
        .child(at(col().key(format!("{key}/box")).width(bx[2]).height(bx[3]).border(1.0, Paint::INK), bx[0], bx[1]))
}
fn view(s: &Site) -> El<()> {
    let root = col().key("root").width(W as f32).height(H as f32).clip();
    if !s.piece {
        root.child(at(text("works", 30.0, Paint::INK).key("works/title"), 32.0, 40.0))
            .child(at(col().key("w/0").width(240.0).height(300.0).clip().flow("work/a")
                .child(at(layer("w/0/layer", (480.0, 360.0), PIC_A, (150.0, 40.0), [180.0, 120.0, 40.0, 30.0]), -120.0, 0.0)), 32.0, 120.0))
            .child(at(col().key("w/1").width(240.0).height(300.0).clip().flow("work/b")
                .child(at(layer("w/1/layer", (480.0, 360.0), PIC_B, (100.0, 60.0), [60.0, 80.0, 40.0, 30.0]), -120.0, 0.0)), 288.0, 120.0))
            .child(at(text("Alpha", 16.0, Paint::TEXT).key("w/0/title").flow("work/a/title"), 32.0, 440.0))
            .child(at(text("Beta", 16.0, Paint::TEXT).key("w/1/title").flow("work/b/title"), 288.0, 440.0))
    } else {
        root.child(at(col().key("page").width(W as f32).height(H as f32).clip().calm()
                .child(layer("page/layer", (W as f32, H as f32), PIC_A, (420.0, 100.0), [500.0, 260.0, 60.0, 45.0]).flow("work/a")), 0.0, 0.0))
            .child(at(text("Alpha", 30.0, Paint::INK).key("page/title").flow("work/a/title"), 32.0, 520.0))
            .child(at(text("back", 11.0, Paint::DIM).font(Font::Mono).key("page/back"), 720.0, 24.0))
    }
}
fn look(s: &Site) -> Look {
    let mut l = Look { material: Material::Plain, theme: DARK, ..Look::default() };
    let g = if s.piece { PIECE_GROUND } else { INDEX_GROUND };
    (l.theme.ground, l.theme.ground2) = (g, g);
    l
}
fn open() -> Ui<Site, ()> {
    let mut u = Ui::new(Site::default(), view, |_, _| {}, fonts(), W, H, 1.0).on_look(look).on_commands(|s| std::mem::take(&mut s.cmds));
    u.tick(3.0);
    u.settle();
    u
}
fn go(u: &mut Ui<Site, ()>, piece: bool) {
    u.state.piece = piece;
    u.state.cmds.push(Cmd::Flow(SPEC));
    u.relayout();
}
/// A picture: blue-grey with a brightness that steps along x.
fn pixels(w: u32, h: u32, tint: u8) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for _y in 0..h { for x in 0..w { let v = (60 + (x * 7 % 120)) as u8; px.extend_from_slice(&[v / 2, v, tint, 255]); } }
    px
}
fn shows(i: &Instance) -> [f32; 4] { [i.rect[0].max(i.clip[0]), i.rect[1].max(i.clip[1]), (i.rect[0] + i.rect[2]).min(i.clip[2]), (i.rect[1] + i.rect[3]).min(i.clip[3])] }
fn visible(i: &Instance) -> bool { let s = shows(i); i.paint[2] > 0.002 && s[2] > s[0] && s[3] > s[1] }
fn pictures(u: &Ui<Site, ()>, id: u32) -> Vec<Instance> { u.scene().instances.iter().filter(|i| i.paint[3] == PICTURE && i.extra[1] as u32 == id && visible(i)).copied().collect() }

#[test]
fn a_window_grows_into_its_piece_as_one_picture_over_the_index_ground() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    o.r.set_picture(PIC_A, 96, 72, &pixels(96, 72, 200));
    o.r.set_picture(PIC_B, 96, 72, &pixels(96, 72, 90));
    let mut u = open();
    let render = |o: &mut Offscreen, u: &mut Ui<Site, ()>| { let l = u.look(); o.r.partial = false; o.render(u, &l) };
    let index = render(&mut o, &mut u);
    let px = |f: &[u8], x: u32, y: u32| { let k = ((y * W + x) * 4) as usize; [f[k] as i32, f[k + 1] as i32, f[k + 2] as i32] };
    let ground = px(&index, 700, 560);
    let old = pictures(&u, PIC_A);
    assert_eq!(old.len(), 1);
    let (born, from) = (old[0].extra[3], shows(&old[0]));
    // The sizes and colours of everything drawn at rest on either page.
    let mut rest: Vec<Instance> = u.scene().instances.clone();
    let t0 = u.now();
    go(&mut u, true);
    let mut v = open();
    v.state.piece = true;
    v.relayout();
    v.tick(3.0);
    rest.extend(v.scene().instances.iter().copied());
    let to = shows(&pictures(&v, PIC_A)[0]);
    let title_glyphs = 5;
    for ms in [40.0f32, 80.0, 200.0, 350.0] {
        u.tick(t0 + ms / 1000.0);
        let f = render(&mut o, &mut u);
        if let Ok(d) = std::env::var("DUMP") { std::fs::write(format!("{d}/flow-{ms}.png"), amenui::offscreen::png_bytes(W, H, &f)).unwrap(); }
        // One picture, in one rect on whole pixels between the window and the full view.
        let p = pictures(&u, PIC_A);
        assert_eq!(p.len(), 1, "{ms} ms: {} pictures of the piece", p.len());
        let r = shows(&p[0]);
        let e = Ease::InOutCubic.at(ms / SPEC.flow_ms);
        for k in 0..4 {
            assert_eq!(r[k], r[k].round(), "{ms} ms: the picture's edges are not on whole pixels: {r:?}");
            assert!((r[k] - (from[k] + (to[k] - from[k]) * e)).abs() <= 0.5, "{ms} ms: the picture shows in {r:?}, from {from:?} to {to:?} at {e}");
        }
        assert_eq!(p[0].extra[3], born, "{ms} ms: the picture started over");
        // The index's ground is around it; the look says so too.
        assert_eq!(u.look().theme.ground, INDEX_GROUND, "{ms} ms: the ground");
        let title: Vec<Instance> = u.scene().instances.iter().filter(|i| i.paint[3] == 0.0 && visible(i) && i.rect[1] > 380.0 && i.rect[0] < 260.0).copied().collect();
        let near_title = |x: u32, y: u32| title.iter().any(|t| (x as f32) >= t.rect[0] - 2.0 && (x as f32) <= t.rect[0] + t.rect[2] + 2.0 && (y as f32) >= t.rect[1] - 2.0 && (y as f32) <= t.rect[1] + t.rect[3] + 2.0);
        if ms >= 200.0 {
            let mut off = 0;
            for y in (0..H).step_by(3) { for x in (0..W).step_by(3) {
                let inside = (x as f32) >= r[0] - 1.0 && (x as f32) < r[2] + 1.0 && (y as f32) >= r[1] - 1.0 && (y as f32) < r[3] + 1.0;
                if inside || near_title(x, y) { continue; }
                if (0..3).any(|c| (px(&f, x, y)[c] - ground[c]).abs() > 2) { off += 1; if off < 4 { eprintln!("{ms} ms: ({x}, {y}) is {:?}, the ground {ground:?}", px(&f, x, y)); } }
            } }
            assert_eq!(off, 0, "{ms} ms: pixels outside the picture that are not the index's ground");
        } else {
            for (x, y) in [(700, 560), (760, 300), (600, 30)] { assert_eq!(px(&f, x, y), ground, "{ms} ms: ({x}, {y})"); }
        }
        // Every note and box at its own size and colour, crisp, inside the picture; nothing stretched
        // (the title alone goes from one size to the other).
        for i in u.scene().instances.iter().filter(|i| visible(i) && i.paint[3] != PICTURE && !title.contains(i)) {
            let same = rest.iter().any(|q| (q.rect[2] - i.rect[2]).abs() < 0.01 && (q.rect[3] - i.rect[3]).abs() < 0.01 && q.paint[0] == i.paint[0] && q.paint[1] == i.paint[1]);
            assert!(same, "{ms} ms: an instance of a size or colour neither page draws: {:?} {:?}", i.rect, i.paint);
            if i.clip == [r[0], r[1], r[2], r[3]] { assert_eq!(i.extra[2], 0.0, "{ms} ms: a note over the picture loosens"); }
        }
        // The title once, over the picture, and never cut.
        assert_eq!(title.len(), title_glyphs, "{ms} ms: the title's glyphs drawn {} times", title.len());
        let pic_at = u.scene().instances.iter().position(|i| i.paint[3] == PICTURE && i.extra[1] as u32 == PIC_A && visible(i)).unwrap_or(0);
        for t in &title {
            let at = u.scene().instances.iter().position(|i| i == t).unwrap_or(0);
            assert!(at > pic_at, "{ms} ms: the title is drawn under the picture");
            assert!(t.clip[0] <= t.rect[0] && t.clip[1] <= t.rect[1] && t.clip[2] >= t.rect[0] + t.rect[2] && t.clip[3] >= t.rect[1] + t.rect[3], "{ms} ms: the title is cut: {:?} in {:?}", t.rect, t.clip);
        }
    }
    // Landed: the piece as it is at rest, on its own ground.
    u.tick(t0 + 1.0);
    u.settle();
    assert_eq!(u.look().theme.ground, PIECE_GROUND);
    assert_eq!(shows(&pictures(&u, PIC_A)[0]), to);
    assert_eq!(u.instances_of("page/layer/note").len(), 4);
    assert!(u.instances_of("page/layer/note").iter().all(|i| i.paint[2] > 0.99), "the new notes came in");
}

#[test]
fn and_back_the_piece_shrinks_into_its_window() {
    let mut u = open();
    go(&mut u, true);
    let t0 = u.now();
    u.tick(t0 + 1.0);
    u.settle();
    let from = shows(&pictures(&u, PIC_A)[0]);
    assert_eq!(from, [0.0, 0.0, W as f32, H as f32]);
    let t1 = u.now();
    go(&mut u, false);
    for ms in [40.0f32, 200.0, 350.0] {
        u.tick(t1 + ms / 1000.0);
        let p = pictures(&u, PIC_A);
        assert_eq!(p.len(), 1, "{ms} ms: {} pictures of the piece", p.len());
        let r = shows(&p[0]);
        assert!(r[2] - r[0] < W as f32 && r[0] >= 0.0 && r[1] >= 0.0, "{ms} ms: {r:?}");
        assert_eq!(u.look().theme.ground, PIECE_GROUND, "{ms} ms: the piece's ground stays around it");
    }
    u.tick(t1 + 1.0);
    assert_eq!(shows(&pictures(&u, PIC_A)[0]), [32.0, 120.0, 272.0, 420.0]);
    assert_eq!(u.look().theme.ground, INDEX_GROUND);
}
