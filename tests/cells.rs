//! Character marks over a picture at a rectangular pitch (`el::cells`, DESIGN.md §1.6.1): one instance
//! however many cells, each cell's character by the brightness under it and its colour mixed toward the
//! ink, and nothing laid out or drawn while it is still.
use amenui::el::{Cells, El, Paint, cells, col, picture};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, Look};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const W: u32 = 357;
const H: u32 = 238;
const PIC: u32 = 901;
const COLS: u32 = 42;
const ROWS: u32 = 14;
const RAMP: &str = "-+#";

#[derive(Clone, Copy)]
struct S { x: f32, y: f32, on: bool }
fn view(s: &S) -> El<()> {
    let c = Cells { over: "pic".into(), ramp: RAMP.into(), ink: Paint::ACCENT, ..Cells::default() };
    let page = col().key("page").width(W as f32).height(H as f32)
        .child(picture(PIC, H as f32 / W as f32).key("pic").absolute(Some(0.0), Some(0.0), None, None).width(W as f32));
    if s.on { page.child(cells(c, COLS, ROWS).key("scan").absolute(Some(s.x), Some(s.y), None, None)) } else { page }
}
fn ui(s: S) -> Ui<S, ()> {
    Ui::new(s, view, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { theme: DARK, ..Look::default() }).on_pointer(|s, x, y| { s.x = x; s.y = y; true })
}
/// The picture: 2 × 2 texels for each 7 × 14 px cell of the window, so a cell's four samples are
/// texel centres: a blue-grey whose brightness steps along x every cell.
fn picture_px() -> (u32, u32, Vec<u8>) {
    let (w, h) = (W / 7 * 2, H / 14 * 2);
    let mut px = Vec::new();
    for _y in 0..h { for x in 0..w {
        let v = ((x / 2) * 6 % 250) as u8;
        px.extend_from_slice(&[v, v, v.saturating_add(20), 255]);
    } }
    (w, h, px)
}
fn rgb(a: &[u8], x: u32, y: u32) -> [i32; 3] { let i = ((y * W + x) * 4) as usize; [a[i] as i32, a[i + 1] as i32, a[i + 2] as i32] }

#[test]
fn a_picture_is_drawn_as_characters_in_cells_seven_by_fourteen() {
    let mut o = Offscreen::new(W, H);
    let (pw, ph, px) = picture_px();
    // The picture is stretched over the window: one picture texel pair per cell only where the window
    // is a whole number of cells from the left, so the window is placed at whole cells.
    o.r.set_picture(PIC, pw, ph, &px);
    let mut u = ui(S { x: 14.0, y: 28.0, on: true });
    u.tick(3.0);
    let l = u.look();
    o.r.partial = false;
    let bare = { let mut b = ui(S { x: 14.0, y: 28.0, on: false }); b.tick(3.0); let l = b.look(); o.render(&mut b, &l) };
    let f = o.render(&mut u, &l);
    if let Ok(d) = std::env::var("DUMP") {
        let mut ppm = format!("P6 {W} {H} 255\n").into_bytes();
        for y in 0..H { for x in 0..W { ppm.extend(rgb(&f, x, y).map(|v| v as u8)); } }
        std::fs::write(format!("{d}/cells.ppm"), ppm).unwrap();
    }
    // One instance for all 588 cells.
    let n = u.scene().instances.iter().filter(|i| i.paint[3] == amenui::scene::PICTURE).count();
    assert_eq!(n, 2, "the picture and one instance for the cells");
    let ink = DARK.accent.map(|v| (v * 255.0).round() as i32);
    // Each cell: the pixels its character covers, and their colour: the cell's colour (its four picture
    // texels' mean) mixed 0.62 toward the ink (the accent here), over the picture by the character's coverage.
    let mut inks = vec![vec![]; RAMP.len()];
    let mut bad = 0;
    for j in 0..ROWS { for i in 0..COLS {
        let (x0, y0) = (14 + i * 7, 28 + j * 14);
        let (tx, ty) = (x0 / 7 * 2, y0 / 14 * 2);
        let texel = |x: u32, y: u32| { let k = ((y * pw + x) * 4) as usize; [px[k] as f32, px[k + 1] as f32, px[k + 2] as f32] };
        let cell: [f32; 3] = std::array::from_fn(|c| (texel(tx, ty)[c] + texel(tx + 1, ty)[c] + texel(tx, ty + 1)[c] + texel(tx + 1, ty + 1)[c]) / 4.0);
        let l = (0.299 * cell[0] + 0.587 * cell[1] + 0.114 * cell[2]) / 255.0;
        let level = (((1.0 - l) * RAMP.len() as f32).floor() as usize).min(RAMP.len() - 1);
        let want: [f32; 3] = std::array::from_fn(|c| cell[c] + (ink[c] as f32 - cell[c]) * 0.62);
        let mut count = 0;
        let mut contrast = 0.0f32;
        for y in y0..y0 + 14 { for x in x0..x0 + 7 {
            let p = rgb(&f, x, y);
            let b = rgb(&bare, x, y);
            contrast = contrast.max((0..3).map(|c| (want[c] - b[c] as f32).abs()).fold(0.0, f32::max));
            if (0..3).all(|c| (p[c] - b[c]).abs() <= 2) { continue; }
            // p = b + a (want − b) for one coverage a in every channel.
            let k = (0..3).max_by(|&u, &v| (want[u] - b[u] as f32).abs().total_cmp(&(want[v] - b[v] as f32).abs())).unwrap_or(0);
            let a = ((p[k] - b[k]) as f32 / (want[k] - b[k] as f32)).clamp(0.0, 1.0);
            if a > 0.3 { count += 1; }
            if (0..3).any(|c| (p[c] as f32 - (b[c] as f32 + a * (want[c] - b[c] as f32))).abs() > 2.5) { bad += 1; if bad < 5 { eprintln!("cell {i},{j} ({x},{y}): {p:?} over {b:?}, want {want:?}"); } }
        } }
        // Where the ink and the picture differ enough to see the character's shape.
        if contrast >= 60.0 { inks[level].push(count); }
    } }
    assert_eq!(bad, 0, "pixels of characters not in their cell's colour mixed 0.62 toward the ink");
    // Each level is one character: the same ink in every cell of it, and more ink the denser it is.
    // (Counted above a third of coverage, an antialiased edge may count one pixel either way.)
    let per: Vec<usize> = inks.iter().map(|v| { let lo = *v.iter().min().expect("a level with no cell"); assert!(v.iter().all(|&c| c <= lo + 1), "{v:?}"); lo }).collect();
    assert!(per[0] > 0 && per[0] + 2 < per[1] && per[1] + 2 < per[2], "ink per level {per:?}");
    // Outside the window the picture is as it is.
    for (x, y) in [(5, 5), (350, 232), (13, 100), (14 + COLS * 7, 100)] { assert_eq!(rgb(&f, x, y), rgb(&bare, x, y), "({x}, {y})"); }
}

#[test]
fn moving_the_cells_lays_nothing_out_and_still_they_ask_for_nothing() {
    let mut u = ui(S { x: 0.0, y: 0.0, on: true });
    u.tick(3.0);
    u.settle();
    let shaped = u.counters().shaped;
    let t = std::time::Instant::now();
    for k in 0..120 {
        u.pointer_move(10.0 + k as f32, 20.0 + (k % 7) as f32);
        u.tick(3.0 + k as f32 / 60.0);
        u.settle();
    }
    let per = t.elapsed().as_secs_f64() * 1e3 / 120.0;
    eprintln!("a move of the cells: {per:.3} ms of the runtime's time a frame");
    assert_eq!(u.counters().shaped, shaped, "moving the cells shaped text");
    assert_eq!(u.node("scan").map(|n| n.rect.x), Some(129.0));
    let after = 3.0 + 200.0 / 60.0;
    u.tick(after);
    u.settle();
    let _ = u.needs_frame(after);
    // Drawn once more, then nothing is needed until something changes.
    let mut o = Offscreen::new(W, H);
    let (pw, ph, px) = picture_px();
    o.r.set_picture(PIC, pw, ph, &px);
    let l = u.look();
    o.draw(&mut u, &l);
    assert!(!u.needs_frame(after + 0.5), "a still window asks for frames");
}
