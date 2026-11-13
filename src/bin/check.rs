//! GPU acceptance for DESIGN.md §1.6.1 and §1.6.2, by reading pixels back. Exits non-zero on failure.
//!  1. Light theme, a fully loosened picture: no pixel inside the picture equals the page ground.
//!  2. Pitch 1 at any looseness: the picture is the original, within 1/255.
//!  3. Pixel faces at 1× and 2×: inside their text, only the ink colour and the ground colour.
//!
//! usage: check
use lab::demo::{BUST, PICTURE_SIZES, SKY, SPHERE, TERRAIN};
use lab::el::{El, Font, Paint, col, picture, text_lh};
use lab::marks::PALETTES;
use lab::offscreen::Offscreen;
use lab::style::{DARK, LIGHT, Look, Material, PicFx, Theme, set};
use lab::text::{MONO, PIXEL, SANS, load_bundled};
use lab::ui::Ui;

const W: u32 = 700;
const H: u32 = 470;

struct One { id: u32, loose: f32 }
fn one(s: &One) -> El<()> {
    let (w, h) = PICTURE_SIZES[s.id as usize];
    col().pad(20.0, 20.0).child(picture(s.id, h as f32 / w as f32).key("pic").loose(s.loose))
}

fn device_rect(r: lab::ui::Rect, scale: f32) -> (u32, u32, u32, u32) {
    ((r.x * scale).round() as u32, (r.y * scale).round() as u32, (r.right() * scale).round() as u32, (r.bottom() * scale).round() as u32)
}
fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] { let o = ((y * w + x) * 4) as usize; [img[o], img[o + 1], img[o + 2]] }
fn close(a: [u8; 3], b: [u8; 3]) -> bool { (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= 1) }
fn rgb8(c: [f32; 3]) -> [u8; 3] { [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8] }

fn main() {
    let mut failed = 0;
    let mut o = Offscreen::new(W, H);
    let fonts = || load_bundled("assets").expect("fonts");

    // ── 1. The page's ground never shows inside a loosened picture ──────────────────────────────────
    let ground = rgb8(LIGHT.ground);
    let mut configs: Vec<(String, PicFx)> = Vec::new();
    for s in 0..set::COUNT as u8 {
        for (pitch, ground_k) in [(8.0, 0.0), (13.0, 0.3), (40.0, 1.0)] {
            for c in 0..4 {
                configs.push((format!("set {} pitch {pitch} ground {ground_k} colour {c}", set::NAMES[s as usize]),
                    PicFx { pitch, ground: ground_k, colour: c as f32, levels: 12.0, spread: 0.5, ..PicFx::with_set(s) }));
            }
        }
        configs.push((format!("set {} relief glow follow", set::NAMES[s as usize]), PicFx { pitch: 12.0, relief: 1.5, glow: 2.0, follow: 1.0, ..PicFx::with_set(s) }));
    }
    for (i, p) in PALETTES.iter().enumerate() {
        configs.push((format!("palette {}", p.0), PicFx { pitch: 6.0, set: set::PIXEL as f32, palette: i as f32, colours: p.1 as f32, spread: 0.6, ..PicFx::default() }));
    }
    configs.push(("regions by hash".into(), PicFx { pitch: 7.0, regions: 5.0, region_by: 0.0, ..PicFx::default() }));
    configs.push(("regions by brightness".into(), PicFx { pitch: 7.0, regions: 5.0, region_by: 1.0, ..PicFx::default() }));
    let mut ui = Ui::new(One { id: SKY, loose: 1.0 }, one, |_, _| {}, fonts(), W, H, 1.0);
    let mut worst = 0usize;
    let mut checked = 0usize;
    for id in [SPHERE, SKY, TERRAIN, BUST] {
        ui.state.id = id;
        ui.relayout();
        ui.tick(100.0);
        let (x0, y0, x1, y1) = device_rect(ui.node("pic").expect("pic").rect, 1.0);
        for (name, f) in &configs {
            let mut look = Look { theme: LIGHT, material: Material::Dot, ..Look::default() };
            look.pics[0] = *f;
            let img = o.render(&mut ui, &look);
            let n = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| close(px(&img, W, x, y), ground)).count();
            // Control: the same comparison finds the ground in the margin around the picture.
            let margin = (0..H).flat_map(|y| (0..x0).map(move |x| (x, y))).filter(|&(x, y)| close(px(&img, W, x, y), ground)).count();
            if margin == 0 { eprintln!("FAIL control: no ground found outside the picture"); failed += 1; }
            checked += 1;
            if n > 0 { eprintln!("FAIL ground shows: picture {id}, {name}: {n} pixels"); failed += 1; worst = worst.max(n); }
        }
    }
    eprintln!("1. ground rule: {checked} renders, {} failing (worst {worst} px)", failed);

    // ── 2. Pitch 1 is the picture itself, however loose ─────────────────────────────────────────────
    let before = failed;
    let mut max_diff = 0;
    for id in [SKY, BUST] {
        for f in [PicFx::with_set(set::CHARS), PicFx { glow: 2.0, relief: 2.0, follow: 1.0, depth: 10.0, ..PicFx::with_set(set::GLOW) },
                  PicFx { palette: 1.0, colours: 3.0, spread: 1.0, ..PicFx::with_set(set::DITHER) }, PicFx { regions: 4.0, ..PicFx::default() }] {
            let f = PicFx { pitch: 1.0, ..f };
            let mut look = Look { theme: LIGHT, material: Material::Dot, ..Look::default() };
            look.pics[0] = f;
            let mut ui = Ui::new(One { id, loose: 0.0 }, one, |_, _| {}, fonts(), W, H, 1.0);
            ui.tick(100.0);
            ui.pointer_move(300.0, 200.0);
            let (x0, y0, x1, y1) = device_rect(ui.node("pic").expect("pic").rect, 1.0);
            // The lens is part of looseness too; take the original with the pointer away.
            ui.pointer_leave();
            let base = o.render(&mut ui, &look);
            for k in [0.25, 0.5, 0.75, 1.0] {
                ui.state.loose = k;
                ui.relayout();
                ui.pointer_move(300.0, 200.0);
                let img = o.render(&mut ui, &look);
                let mut d = 0;
                for y in y0..y1 { for x in x0..x1 {
                    let (a, b) = (px(&img, W, x, y), px(&base, W, x, y));
                    for c in 0..3 { d = d.max((a[c] as i32 - b[c] as i32).abs()); }
                } }
                max_diff = max_diff.max(d);
                if d > 1 { eprintln!("FAIL pitch 1 differs: picture {id}, set {}, looseness {k}: {d}/255", f.set); failed += 1; }
            }
        }
    }
    // Control: at pitch 8 the same comparison does see the marks.
    {
        let mut look = Look { theme: LIGHT, material: Material::Dot, ..Look::default() };
        look.pics[0] = PicFx::with_set(set::CHARS);
        let mut ui = Ui::new(One { id: BUST, loose: 0.0 }, one, |_, _| {}, fonts(), W, H, 1.0);
        ui.tick(100.0);
        let base = o.render(&mut ui, &look);
        ui.state.loose = 1.0;
        ui.relayout();
        let img = o.render(&mut ui, &look);
        let d = base.iter().zip(&img).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
        if d <= 1 { eprintln!("FAIL control: pitch 8 looks like the original"); failed += 1; }
    }
    eprintln!("2. pitch 1: largest difference {max_diff}/255, {} failing", failed - before);

    // ── 3. Pixel type edges are ink or ground ───────────────────────────────────────────────────────
    let before = failed;
    for scale in [1.0f32, 2.0] {
        for theme in [DARK, LIGHT] {
            let (w, h) = ((W as f32 * scale) as u32, (H as f32 * scale) as u32);
            let mut o2 = Offscreen::new(w, h);
            let mut ui = Ui::new(theme, type_page, |_, _| {}, fonts(), w, h, scale);
            ui.tick(100.0);
            let look = Look { theme, material: Material::Plain, ..Look::default() };
            let img = o2.render(&mut ui, &look);
            for (key, pixel) in [("fusion12", true), ("fusion24", true), ("mono11", true), ("mono22", true), ("sans", false)] {
                let (x0, y0, x1, y1) = device_rect(ui.node(key).expect("text").rect, scale);
                let mut colours: Vec<[u8; 3]> = Vec::new();
                for y in y0..y1.min(h) { for x in x0..x1.min(w) {
                    let c = px(&img, w, x, y);
                    if !colours.iter().any(|&k| close(k, c)) { colours.push(c); }
                } }
                let ok = colours.len() <= 2 && colours.iter().any(|&c| close(c, rgb8(theme.ground)));
                if pixel && !ok { eprintln!("FAIL pixel type {key} at {scale}×, theme {}: {} colours", theme.name, colours.len()); failed += 1; }
                if pixel { eprintln!("   {key} {scale}× {}: {} colours", theme.name, colours.len()); }
                else { eprintln!("   ({key}, an outline face, has {} colours: anti-aliased, as it should be)", colours.len()); }
            }
        }
    }
    eprintln!("3. pixel type: {} failing", failed - before);

    if failed > 0 { eprintln!("{failed} checks failed"); std::process::exit(1); }
    eprintln!("all checks passed");
}

const ZH: &str = "像素字体按整数倍显示，边缘只有两种颜色。";
fn type_page(_: &Theme) -> El<()> {
    col().pad(10.0, 10.0).gap(8.0)
        .child(text_lh(ZH, 12.0, 20.0, Paint::TEXT).font(Font::Face(PIXEL)).key("fusion12"))
        .child(text_lh(ZH, 24.0, 32.0, Paint::INK).font(Font::Face(PIXEL)).key("fusion24"))
        .child(text_lh("Departure Mono 0123 ABC xyz", 11.0, 18.0, Paint::TEXT).font(Font::Face(MONO)).key("mono11"))
        .child(text_lh("Departure Mono 0123 ABC xyz", 22.0, 30.0, Paint::DIM).font(Font::Face(MONO)).key("mono22"))
        .child(text_lh(ZH, 16.0, 26.0, Paint::TEXT).font(Font::Face(SANS)).key("sans"))
}
