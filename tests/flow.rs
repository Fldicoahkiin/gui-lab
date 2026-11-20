//! A flow from an index of windows to a piece and back (`Cmd::Flow`, DESIGN.md §1.10): the window's
//! picture grows into the piece's as one picture in one rect on whole pixels, the index's ground around
//! it; over the picture, type keeps its size and a drawing's lines go with the picture's transform, their
//! pen kept, the old notes riding with the picture as they fade and the new ones coming in once it has
//! landed; the title is drawn once, over the picture;
//! the picture does not start over. Coming back, the other window waits for the picture to land: its
//! ray, a ramp in a colour of its own, is not drawn before, and comes in in that colour.
use amenui::draw::Drawing;
use amenui::el::{Cells, El, Font, Paint, Stirr, canvas, cells, col, picture, text};
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
struct Site { piece: bool, cmds: Vec<Cmd>, birds: u32, stirr: bool, pointer: bool }
fn at<M: 'static>(e: El<M>, x: f32, y: f32) -> El<M> { e.absolute(Some(x), Some(y), None, None) }
/// The other window's own colour (green, which nothing else here is) and its ray's ends in its layer.
const RAY: [f32; 3] = [0.1, 0.85, 0.3];
const RAY_AT: ((f32, f32), (f32, f32)) = ((150.0, 100.0), (330.0, 100.0));
/// A faint shade, at a real alpha.
const SHADE: Paint = Paint { tone: 1.0, accent: 0.0, alpha: 0.35, hue: 0 };
/// A picture with notes over it, as the site's picture layer: `w × h`; a note's text placed where the
/// page puts it, a box and a faint line drawn over the picture, and a faint shaded block.
/// `birds` more notes below the scan, as the site's flight adds while it moves.
fn layer(key: &str, (w, h): (f32, f32), pic: u32, note: (f32, f32), bx: [f32; 4], birds: u32) -> El<()> {
    let mut d = Drawing::new(w, h);
    let (x0, y0, x1, y1) = (bx[0], bx[1], bx[0] + bx[2], bx[1] + bx[3]);
    d.line((x0, y0), (x1, y0), 1.0, Paint::INK); d.line((x1, y0), (x1, y1), 1.0, Paint::INK);
    d.line((x1, y1), (x0, y1), 1.0, Paint::INK); d.line((x0, y1), (x0, y0), 1.0, Paint::INK);
    d.line((0.0, y1 + 20.0), (w * 0.4, y1 + 20.0), 1.0, SHADE);
    // The other window's ray, as the site's curtain: a ramp in a colour of its own through a middle stop,
    // fading out along it.
    if key == "w/1/layer" {
        d.palette([RAY]);
        let own = Paint { tone: 1.0, accent: 1.0, alpha: 0.97, hue: 3 };
        d.ramp(RAY_AT.0, RAY_AT.1, 8.0, own, Some((Paint { alpha: 0.6, ..own }, 0.3)), Paint { alpha: 0.0, ..own });
    }
    col().key(key).width(w).height(h)
        .child(at(picture(pic, 0.75).key(format!("{key}/picture")).width(w), 0.0, 0.0))
        .child(at(canvas(d).key(format!("{key}/marks")).width(w).pass(true), 0.0, 0.0))
        .child(at(col().key(format!("{key}/shade")).width(30.0).height(12.0).fill(SHADE), note.0, note.1 + 20.0))
        .child(at(text("note", 11.0, Paint::ACCENT).font(Font::Mono).key(format!("{key}/note")), note.0, note.1))
        .child(at(cells(Cells { over: format!("{key}/picture"), pitch: [7.0, 10.0], ramp: ":=+*".into(), size: 9.0, ..Cells::default() }, 6, 3).key(format!("{key}/scan")), note.0, note.1 + 40.0))
        .children((0..birds).map(|b| at(text("bird", 11.0, Paint::ACCENT).font(Font::Mono).key(format!("{key}/bird{b}")), note.0, note.1 + 80.0 + 14.0 * b as f32)))
}
/// The window's frame: four lines in the accent, drawn over it.
fn frame(key: &str, w: f32, h: f32) -> El<()> {
    let mut d = Drawing::new(w, h);
    let (x1, y1) = (w - 1.0, h - 1.0);
    d.line((0.0, 0.0), (x1, 0.0), 1.0, Paint::ACCENT); d.line((x1, 0.0), (x1, y1), 1.0, Paint::ACCENT);
    d.line((x1, y1), (0.0, y1), 1.0, Paint::ACCENT); d.line((0.0, y1), (0.0, 0.0), 1.0, Paint::ACCENT);
    at(canvas(d).key(key).width(w).pass(true), 0.0, 0.0)
}
/// The pointer's box and its label, as the site's over its scan: a box drawn with a cross, and a
/// mono label of its own size beside it, at (x, y). The same keys on both pages; on the index it is in
/// the window's layer, on the piece's page over its frame, outside the subtree that flows.
fn pointer(x: f32, y: f32) -> El<()> {
    let mut d = Drawing::new(W as f32, H as f32);
    let (x0, y0, x1, y1) = (x - 16.0, y - 16.0, x + 16.0, y + 16.0);
    d.line((x0, y0), (x1, y0), 1.0, Paint::ACCENT); d.line((x1, y0), (x1, y1), 1.0, Paint::ACCENT);
    d.line((x1, y1), (x0, y1), 1.0, Paint::ACCENT); d.line((x0, y1), (x0, y0), 1.0, Paint::ACCENT);
    d.line((x - 9.0, y), (x + 9.0, y), 1.0, Paint::ACCENT); d.line((x, y - 9.0), (x, y + 9.0), 1.0, Paint::ACCENT);
    col().key("pointer").width(W as f32).height(H as f32).pass(true)
        .child(at(col().key("pointer/light").width(W as f32).height(H as f32).pass(true).child(at(canvas(d).key("pointer/light/box").width(W as f32).pass(true), 0.0, 0.0)), 0.0, 0.0))
        .child(at(text("HUMAN? 0.58", 11.0, Paint::INK).font(Font::Mono).key("pointer/human").own_size(0.0, 0.0).pass(true), x + 22.0, y - 16.0))
}
fn view(s: &Site) -> El<()> {
    let root = col().key("root").width(W as f32).height(H as f32).clip();
    if !s.piece {
        root.child(at(text("works", 30.0, Paint::INK).key("works/title"), 32.0, 40.0))
            .child(at(col().key("w/0").width(240.0).height(300.0).clip().flow("work/a")
                .child(at(layer("w/0/layer", (480.0, 360.0), PIC_A, (150.0, 40.0), [180.0, 120.0, 40.0, 30.0], s.birds), -120.0, 0.0))
                .children(s.pointer.then(|| at(pointer(200.0, 150.0), -120.0, 0.0)))
                .child(frame("w/0/frame", 240.0, 300.0)), 32.0, 120.0))
            // A thing that stirs the field, as the site's index has: what is drawn after it is a layer.
            .children(s.stirr.then(|| at(col().key("works/stirr").width(12.0).height(12.0).fill(Paint::DIM).stirr(Stirr { energy: 0.0, ..Stirr::default() }), 760.0, 20.0)))
            .child(at(col().key("w/1").width(240.0).height(300.0).clip().flow("work/b")
                .child(at(layer("w/1/layer", (480.0, 360.0), PIC_B, (100.0, 60.0), [60.0, 80.0, 40.0, 30.0], 0), -120.0, 0.0)), 288.0, 120.0))
            .child(at(text("Alpha", 16.0, Paint::TEXT).key("w/0/title").flow("work/a/title"), 32.0, 440.0))
            .child(at(text("LIVE · OPEN", 11.0, Paint::ACCENT).font(Font::Mono).key("w/0/live"), 420.0, 444.0))
            .child(at(text("Beta", 16.0, Paint::TEXT).key("w/1/title").flow("work/b/title"), 288.0, 440.0))
    } else {
        root.child(at(col().key("page").width(W as f32).height(H as f32).clip().calm()
                .child(layer("page/layer", (W as f32, H as f32), PIC_A, (420.0, 100.0), [500.0, 260.0, 60.0, 45.0], s.birds).flow("work/a")), 0.0, 0.0))
            .child(at(text("Alpha", 30.0, Paint::INK).key("page/title").flow("work/a/title"), 32.0, 520.0))
            .child(at(text("back", 11.0, Paint::DIM).font(Font::Mono).key("page/back"), 720.0, 24.0))
            .children(s.pointer.then(|| at(pointer(640.0, 440.0), 0.0, 0.0)))
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
fn pictures(u: &Ui<Site, ()>, id: u32) -> Vec<Instance> { u.scene().instances.iter().filter(|i| i.paint[3] == PICTURE && i.extra[0] <= 0.5 && i.extra[1] as u32 == id && visible(i)).copied().collect() }

/// The flow one way (`back`: from the piece to the index) at 40, 80, 200 and 350 ms: one picture, in
/// one rect on whole pixels where the ease puts it, never starting over; the index's ground around it;
/// the window's frame gone; everything else at a size and colour one of the pages draws (a drawing's line
/// over the picture at its old length times the picture's scale, its width as drawn), crisp over the
/// picture, the notes still in their colour; the title once, over the picture, whole and opaque.
fn flight(back: bool) {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    o.r.set_picture(PIC_A, 96, 72, &pixels(96, 72, 200));
    o.r.set_picture(PIC_B, 96, 72, &pixels(96, 72, 90));
    // The pictures are red at half their green; the accent over them, at any opacity, is redder than
    // that and than their blue, where the ink is not.
    let orange = |c: [i32; 3]| c[0] - c[1] / 2 > 20 && c[0] > c[2];
    let render = |o: &mut Offscreen, u: &mut Ui<Site, ()>| { let l = u.look(); o.r.partial = false; o.render(u, &l) };
    let px = |f: &[u8], x: u32, y: u32| { let k = ((y * W + x) * 4) as usize; [f[k] as i32, f[k + 1] as i32, f[k + 2] as i32] };
    // Both pages at rest: what they draw, and the index's ground.
    let mut index = open();
    let index_frame = render(&mut o, &mut index);
    let ground = px(&index_frame, 700, 560);
    // The window's frame and live line, where they are drawn; the other window.
    let mut chrome = index.instances_of("w/0/frame");
    chrome.extend(index.instances_of("w/0/live"));
    let others = index.instances_of("w/1");
    let frame_px: std::collections::HashSet<(u32, u32)> = (0..W * H).map(|k| (k % W, k / W)).filter(|&(x, y)| {
        let c = px(&index_frame, x, y);
        (x <= 33 || x >= 270 || y <= 121 || y >= 418) && (32..272).contains(&x) && (120..420).contains(&y) && c[0] > c[1] + 30 && c[0] > c[2] + 30
    }).collect();
    assert!(frame_px.len() > 400, "the window's frame at rest: {} px", frame_px.len());
    let mut piece = open();
    piece.state.piece = true;
    piece.relayout();
    piece.tick(3.0);
    let mut rest: Vec<Instance> = index.scene().instances.clone();
    rest.extend(piece.scene().instances.iter().copied());
    // And both with the note the flight adds.
    for on in [false, true] {
        let mut b = open();
        (b.state.piece, b.state.birds) = (on, 1);
        b.relayout();
        b.tick(3.0);
        rest.extend(b.scene().instances.iter().copied());
    }
    let (win, full) = (shows(&pictures(&index, PIC_A)[0]), shows(&pictures(&piece, PIC_A)[0]));
    let (from, to) = if back { (full, win) } else { (win, full) };
    // The old page's picture box and its drawing's lines (thin, turned bars), which ride its transform.
    let (old_page, old_pic) = if back { (&piece, pictures(&piece, PIC_A)[0].rect) } else { (&index, pictures(&index, PIC_A)[0].rect) };
    let old_lines: Vec<Instance> = old_page.scene().instances.iter().filter(|i| i.paint[3] == 1.0 && i.rect[3] <= 2.0 && i.rect[2] > i.rect[3] + 0.5).copied().collect();
    let mut u = open();
    if back { go(&mut u, true); let t = u.now(); u.tick(t + 1.0); u.settle(); }
    let born = pictures(&u, PIC_A)[0].extra[3];
    let t0 = u.now();
    go(&mut u, !back);
    let way = if back { "back" } else { "in" };
    for ms in [40.0f32, 80.0, 200.0, 350.0] {
        // Mid-flight the new page lays out a note it did not have when the flow began: it comes in
        // with the rest of the new subtree, not before.
        if ms == 200.0 { u.tick(t0 + 0.19); u.state.birds = 1; u.relayout(); }
        u.tick(t0 + ms / 1000.0);
        let f = render(&mut o, &mut u);
        if let Ok(d) = std::env::var("DUMP") { std::fs::write(format!("{d}/flow-{way}-{ms}.png"), amenui::offscreen::png_bytes(W, H, &f)).unwrap(); }
        let p = pictures(&u, PIC_A);
        assert_eq!(p.len(), 1, "{way} {ms} ms: {} pictures of the piece", p.len());
        let r = shows(&p[0]);
        let e = Ease::InOutCubic.at(ms / SPEC.flow_ms);
        for k in 0..4 {
            assert_eq!(r[k], r[k].round(), "{way} {ms} ms: the picture's edges are not on whole pixels: {r:?}");
            assert!((r[k] - (from[k] + (to[k] - from[k]) * e)).abs() <= 0.5, "{way} {ms} ms: the picture shows in {r:?}, from {from:?} to {to:?} at {e}");
        }
        assert_eq!(p[0].extra[3], born, "{way} {ms} ms: the picture started over");
        assert_eq!(u.look().theme.ground, INDEX_GROUND, "{way} {ms} ms: the ground");
        let title: Vec<Instance> = u.scene().instances.iter().filter(|i| i.paint[3] == 0.0 && visible(i) && i.rect[1] > 380.0 && i.rect[0] < 260.0 && i.paint[1] == 0.0).copied().collect();
        let near_title = |x: u32, y: u32| title.iter().any(|t| (x as f32) >= t.rect[0] - 2.0 && (x as f32) <= t.rect[0] + t.rect[2] + 2.0 && (y as f32) >= t.rect[1] - 2.0 && (y as f32) <= t.rect[1] + t.rect[3] + 2.0);
        let inside = |x: u32, y: u32, m: f32| (x as f32) >= r[0] - m && (x as f32) < r[2] + m && (y as f32) >= r[1] - m && (y as f32) < r[3] + m;
        // Around the picture, the index's ground.
        if ms >= 200.0 {
            let mut off = 0;
            for y in (0..H).step_by(3) { for x in (0..W).step_by(3) {
                if inside(x, y, 0.0) || near_title(x, y) { continue; }
                if (0..3).any(|c| (px(&f, x, y)[c] - ground[c]).abs() > 2) { off += 1; if off < 4 { eprintln!("{way} {ms} ms: ({x}, {y}) is {:?}, the ground {ground:?}", px(&f, x, y)); } }
            } }
            // No rim: every pixel just outside the picture's edges is the ground.
            let (x0, y0, x1, y1) = (r[0] as i32 - 1, r[1] as i32 - 1, r[2] as i32, r[3] as i32);
            let ring = (x0..=x1).flat_map(|x| [(x, y0), (x, y1)]).chain((y0..=y1).flat_map(|y| [(x0, y), (x1, y)]));
            for (x, y) in ring {
                if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 || near_title(x as u32, y as u32) { continue; }
                let c = px(&f, x as u32, y as u32);
                if (0..3).any(|k| (c[k] - ground[k]).abs() > 2) { off += 1; if off < 4 { eprintln!("{way} {ms} ms: the rim at ({x}, {y}) is {c:?}, the ground {ground:?}"); } }
            }
            assert_eq!(off, 0, "{way} {ms} ms: pixels outside the picture that are not the index's ground");
        } else {
            for (x, y) in [(700, 560), (760, 300), (600, 30), (780, 10)] { if !inside(x, y, 1.0) { assert_eq!(px(&f, x, y), ground, "{way} {ms} ms: ({x}, {y})"); } }
        }
        // The window's frame is not drawn in the picture; nothing over it turns another colour.
        let frame = u.scene().instances.iter().filter(|i| visible(i) && i.paint[3] == 1.0 && i.paint[1] == 1.0 && i.clip == [r[0], r[1], r[2], r[3]]).count();
        assert_eq!(frame, 0, "{way} {ms} ms: the window's frame is drawn in the growing picture");
        let green = (0..W * H).filter(|&k| { let (x, y) = (k % W, k / W); inside(x, y, -1.0) && { let c = px(&f, x, y); c[1] > c[0] + 40 && c[1] > c[2] + 40 } }).count();
        assert_eq!(green, 0, "{way} {ms} ms: green over the picture");
        if ms <= 80.0 {
            let notes = (0..W * H).filter(|&k| { let (x, y) = (k % W, k / W); inside(x, y, -1.0) && orange(px(&f, x, y)) }).count();
            assert!(notes >= 4, "{way} {ms} ms: the notes over the picture are not in their colour ({notes} px)");
        }
        let k = p[0].rect[2] / old_pic[2];
        for i in u.scene().instances.iter().filter(|i| visible(i) && i.paint[3] != PICTURE && !title.contains(i)) {
            let same = rest.iter().any(|q| (q.rect[2] - i.rect[2]).abs() < 0.01 && (q.rect[3] - i.rect[3]).abs() < 0.01 && q.paint[0] == i.paint[0] && q.paint[1] == i.paint[1]);
            // A drawing's line riding over the picture: its old length times the picture's scale, its pen as drawn.
            let rides = i.clip == [r[0], r[1], r[2], r[3]] && i.paint[3] == 1.0 && old_lines.iter().any(|q| (q.rect[3] - i.rect[3]).abs() < 0.01 && q.paint[0] == i.paint[0] && q.paint[1] == i.paint[1]
                && ((q.rect[2] - q.rect[3]) * k - (i.rect[2] - i.rect[3])).abs() <= 1.0);
            let same = same || rides;
            assert!(same, "{way} {ms} ms: an instance of a size or colour neither page draws: {:?} {:?}", i.rect, i.paint);
            if i.clip == [r[0], r[1], r[2], r[3]] { assert_eq!(i.extra[2], 0.0, "{way} {ms} ms: a note over the picture loosens"); }
        }
        // What rides keeps its shape: the box of four lines over the picture is still closed, each
        // side's ends on the other two (one size of box, not its sides closing up apart).
        // A line is drawn turned (`xf`): its rect is its length along, about its centre.
        let lines: Vec<(bool, [f32; 4])> = u.scene().instances.iter().filter(|i| visible(i) && i.paint[3] == 1.0 && i.paint[1] == 0.0 && i.paint[2] > 0.9 && i.clip == [r[0], r[1], r[2], r[3]] && i.rect[3] <= 2.0)
            .map(|i| (i.xf[2].abs() > 0.5, [i.rect[0] + i.rect[2] * 0.5, i.rect[1] + i.rect[3] * 0.5, (i.rect[2] - i.rect[3]) * 0.5, i.rect[3]])).collect();
        let down: Vec<[f32; 4]> = lines.iter().filter(|l| l.0).map(|l| l.1).collect();
        for (_, a) in lines.iter().filter(|l| !l.0 && l.1[2] > 10.0 && l.1[2] < 100.0) {
            for end in [a[0] - a[2], a[0] + a[2]] {
                assert!(down.iter().any(|d| (d[0] - end).abs() <= 1.0 && (d[1] - a[1]).abs() <= d[2] + 1.0),
                    "{way} {ms} ms: a ridden box is not closed: a side about {a:?} has no side at x {end} among {down:?}");
            }
        }
        // The window's frame and its live line leave by their opacity within `out_ms`, under the picture
        // (going in), or wait for it to land (going back): never drawn over it.
        let pic_at = u.scene().instances.iter().position(|i| i.paint[3] == PICTURE && i.extra[0] <= 0.5 && i.extra[1] as u32 == PIC_A && visible(i)).unwrap_or(0);
        let overlaps = |i: &Instance| { let s = shows(i); s[0] < r[2] && s[2] > r[0] && s[1] < r[3] && s[3] > r[1] };
        for (k, i) in u.scene().instances.iter().enumerate().filter(|(_, i)| visible(i)) {
            let same = |q: &Instance| q.rect == i.rect && q.paint[..2] == i.paint[..2] && q.paint[3] == i.paint[3];
            if chrome.iter().any(same) {
                assert!(!back && ms < SPEC.out_ms, "{way} {ms} ms: the window's frame or live line is drawn: {:?} at {}", i.rect, i.paint[2]);
                assert!(k < pic_at || !overlaps(i), "{way} {ms} ms: the window's frame or live line is drawn over the picture: {:?}", i.rect);
            }
            // No other window over it.
            if others.iter().any(same) { assert!(k < pic_at || !overlaps(i), "{way} {ms} ms: another window is drawn over the picture: {:?}", i.rect); }
        }
        let edge = (0..W * H).filter(|&k| { let (x, y) = (k % W, k / W); inside(x, y, -1.0) && frame_px.contains(&(x, y)) && orange(px(&f, x, y)) }).count();
        assert_eq!(edge, 0, "{way} {ms} ms: the window's frame line shows in the picture");
        let beta = (0..W * H).filter(|&k| { let (x, y) = (k % W, k / W); inside(x, y, -1.0) && { let c = px(&f, x, y); (80..=100).contains(&c[2]) && (c[0] - c[1] / 2).abs() <= 4 && c[1] > 60 } }).count();
        assert_eq!(beta, 0, "{way} {ms} ms: the other window's picture shows in the picture");
        // Each note once, over the picture, inside it: the text's four glyphs and the scan's cells.
        let over: Vec<&Instance> = u.scene().instances[pic_at + 1..].iter().filter(|i| visible(i) && !title.contains(i)).collect();
        let notes = over.iter().filter(|i| i.paint[3] == 0.0 && i.paint[1] == 1.0 && overlaps(i)).count();
        let scans: Vec<&&Instance> = over.iter().filter(|i| i.paint[3] == PICTURE && i.extra[0] > 0.5).collect();
        assert_eq!(notes, 4, "{way} {ms} ms: the note's glyphs over the picture drawn {notes} times");
        assert_eq!(scans.len(), 1, "{way} {ms} ms: the scan's cells drawn {} times over the picture", scans.len());
        for c in &scans { let sh = shows(c); assert!(sh[0] >= r[0] && sh[1] >= r[1] && sh[2] <= r[2] && sh[3] <= r[3], "{way} {ms} ms: the scan's cells are drawn outside the picture: {sh:?} in {r:?}"); }
        // The title once, over the picture, whole and opaque.
        assert_eq!(title.len(), 5, "{way} {ms} ms: the title's glyphs drawn {} times", title.len());
        for t in &title {
            let at = u.scene().instances.iter().position(|i| i == t).unwrap_or(0);
            assert!(at > pic_at, "{way} {ms} ms: the title is drawn under the picture");
            assert!(t.paint[2] > 0.99, "{way} {ms} ms: the title at {} of its opacity", t.paint[2]);
            assert!(t.clip[0] <= t.rect[0] && t.clip[1] <= t.rect[1] && t.clip[2] >= t.rect[0] + t.rect[2] && t.clip[3] >= t.rect[1] + t.rect[3], "{way} {ms} ms: the title is cut: {:?} in {:?}", t.rect, t.clip);
        }
    }
    // Landed: the new page as it is at rest, on its own ground.
    u.tick(t0 + 1.0);
    u.settle();
    assert_eq!(u.look().theme.ground, if back { INDEX_GROUND } else { PIECE_GROUND });
    assert_eq!(shows(&pictures(&u, PIC_A)[0]), to);
    let notes = if back { "w/0/layer/note" } else { "page/layer/note" };
    assert!(!u.instances_of(notes).is_empty() && u.instances_of(notes).iter().all(|i| i.paint[2] > 0.99), "the new notes came in");
}

#[test]
fn a_window_grows_into_its_piece_as_one_picture_over_the_index_ground() { flight(false); }

/// Back to an index whose other window is drawn in a layer (after something that stirs), as the site's:
/// its ray is not drawn while the picture flies (a ramp's every stop waits with it), and comes in in its
/// own colour, never the accent, while its picture fades in under it.
#[test]
fn coming_back_the_other_window_waits_and_keeps_its_own_colour() {
    let Ok(mut o) = Offscreen::try_new(W, H) else { eprintln!("no GPU adapter here: skipped"); return };
    o.r.set_picture(PIC_A, 96, 72, &pixels(96, 72, 200));
    o.r.set_picture(PIC_B, 96, 72, &pixels(96, 72, 90));
    let px = |f: &[u8], x: u32, y: u32| { let k = ((y * W + x) * 4) as usize; [f[k] as i32, f[k + 1] as i32, f[k + 2] as i32] };
    let green = |c: [i32; 3]| c[1] > c[0] + 40 && c[1] > c[2] + 40;
    let orange = |c: [i32; 3]| c[0] - c[1] / 2 > 20 && c[0] > c[2];
    let mut u = open();
    u.state.stirr = true;
    go(&mut u, true);
    let t = u.now();
    u.tick(t + 1.0);
    u.settle();
    let t0 = u.now();
    go(&mut u, false);
    // The ray's first half on the screen: the window at (288, 120), its layer at -120.
    let ray = |f: &[u8]| (0..90u32).map(|k| px(f, 288 + 30 + k, 120 + 100)).collect::<Vec<_>>();
    let mut render = |u: &mut Ui<Site, ()>, ms: f32| { u.tick(t0 + ms / 1000.0); let l = u.look(); o.r.partial = false; o.render(u, &l) };
    for ms in [0.0, 40.0, 80.0, 200.0, 350.0] {
        let f = render(&mut u, ms);
        let n = (0..W * H).filter(|&k| green(px(&f, k % W, k / W))).count();
        assert_eq!(n, 0, "back {ms} ms: the other window's ray is drawn while the picture flies ({n} px)");
    }
    for ms in [SPEC.flow_ms + 40.0, SPEC.flow_ms + SPEC.fade_ms * 0.5, SPEC.flow_ms + SPEC.fade_ms + 40.0] {
        let r = ray(&render(&mut u, ms));
        assert!(r.iter().all(|&c| !orange(c)), "back {ms} ms: the ray is drawn in the accent: {:?}", &r[..4]);
    }
    let r = ray(&render(&mut u, 1000.0));
    assert!(r.iter().filter(|&&c| green(c)).count() >= 30, "back, at rest: the ray is not in its own colour: {:?}", &r[..4]);
}

#[test]
fn and_back_the_piece_shrinks_into_its_window_over_the_index_ground() { flight(true); }

/// A note the new page lays out only after the flow began (the site's labels shown once what they
/// follow is in view, a drawing's new marks), outside the part of the picture that shows: it is not
/// drawn until the picture has landed, as what was new on the page when the flow began, and then it is.
#[test]
fn a_note_laid_out_mid_flight_outside_the_growing_picture_waits_for_it_to_land() {
    let mut u = open();
    let t0 = u.now();
    go(&mut u, true);
    u.tick(t0 + 0.055);
    u.state.birds = 1;
    u.relayout();
    let mut seen = 0;
    for ms in [60.0f32, 100.0, 140.0, 200.0, 300.0, 400.0] {
        u.tick(t0 + ms / 1000.0);
        let r = shows(&pictures(&u, PIC_A)[0]);
        let bird = u.instances_of("page/layer/bird0");
        assert!(!bird.is_empty(), "{ms} ms: the late note is laid out");
        for i in bird.iter().filter(|i| visible(i)) {
            let s = shows(i);
            assert!(s[0] >= r[0] && s[1] >= r[1] && s[2] <= r[2] && s[3] <= r[3], "{ms} ms: the late note is drawn outside the growing picture: {s:?} in {r:?}");
            seen += 1;
        }
        if ms < SPEC.flow_ms { assert!(bird.iter().all(|i| i.paint[2] <= 0.002), "{ms} ms: the late note is drawn before the picture landed"); }
    }
    assert_eq!(seen, 0, "the note lies outside the picture until it lands");
    u.tick(t0 + 1.0);
    u.settle();
    assert!(u.instances_of("page/layer/bird0").iter().all(|i| i.paint[2] > 0.99), "landed, the late note is there");
}

/// The pointer's box and label, keyed the same on both pages: in the window's layer on the index, and
/// over the piece's frame on its page, outside the subtree that flows (the site's scan and pointer). An
/// instance that rode on the growing picture on the old page is carried by it to its new place, never
/// drawn at once where it is outside the part of the picture that shows; landed, it is there.
#[test]
fn a_pointer_over_the_window_goes_with_the_picture_to_its_place_over_the_page() {
    let mut u = open();
    u.state.pointer = true;
    u.relayout();
    u.settle();
    assert!(!u.instances_of("pointer/light").is_empty() && !u.instances_of("pointer/human").is_empty());
    let t0 = u.now();
    go(&mut u, true);
    for ms in [0.0f32, 17.0, 40.0, 100.0, 200.0, 300.0, 400.0] {
        u.tick(t0 + ms / 1000.0);
        let r = shows(&pictures(&u, PIC_A)[0]);
        for key in ["pointer/light", "pointer/human"] {
            for i in u.instances_of(key).iter().filter(|i| visible(i)) {
                let s = shows(i);
                assert!(s[0] >= r[0] - 0.5 && s[1] >= r[1] - 0.5 && s[2] <= r[2] + 0.5 && s[3] <= r[3] + 0.5, "{ms} ms: {key} is drawn outside the growing picture: {s:?} in {r:?}");
            }
        }
        // Mid-flight the pointer moves (a new view while the flow is on its way): still with the picture.
        if ms == 100.0 { u.state.cmds.clear(); u.relayout(); }
    }
    u.tick(t0 + 1.0);
    u.settle();
    for key in ["pointer/light", "pointer/human"] {
        let is = u.instances_of(key);
        assert!(!is.is_empty() && is.iter().all(|i| i.paint[2] > 0.99), "landed, {key} is there");
    }
    let b = u.instances_of("pointer/light");
    assert!(b.iter().any(|i| (i.rect[0] - 624.0).abs() < 1.5 || (i.rect[0] + i.rect[2] * 0.5 - 640.0).abs() < 17.0), "landed, the box is where the page puts it: {:?}", b.iter().map(|i| i.rect).collect::<Vec<_>>());
}
