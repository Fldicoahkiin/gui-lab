//! GPU acceptance for DESIGN.md §1.6.1, §1.6.2, §1.6.3, §1.7 and §1.10, by reading pixels back. Exits non-zero on failure.
//!  1. Light theme, a fully loosened picture: no pixel inside the picture equals the page ground.
//!  2. Pitch 1 at any looseness: the picture is the original, within 1/255.
//!  3. Pixel faces at 1× and 2×: inside their text, only the ink colour and the ground colour.
//!  4. Morphs (§1.10), plain and dot: the hand-over frame equals the source at looseness 1 and the
//!     landing frame the target at looseness 1, within 2/255 over each element; mid-flight, under the
//!     light theme, tiles are drawn outside and between the two elements.
//!
//!  5. Rest (step 5): for every material but chrome and liquid, body text at looseness 0 is drawn
//!     exactly as `plain` draws it (print: within its paper grain; dot: the glyphs' own pixels, since
//!     its ground has units at rest); with a control that loosened text
//!     does differ.
//!  6. Every material, light theme: a fully loosened picture shows no page-ground pixel.
//!  7. Annotation frames lie on the layout rects to the pixel.
//!
//! usage: check
use lab::demo::{BUST, PICTURE_SIZES, SKY, SPHERE, TERRAIN};
use lab::el::{El, Font, Paint, col, picture, row, space, text_lh};
use lab::morph::MorphSpec;
use lab::marks::PALETTES;
use lab::offscreen::Offscreen;
use lab::style::{DARK, Gaze, GazeMode, LIGHT, Look, MATERIAL_COUNT, Material, PicFx, Theme, gaze, set};
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

    // ── 4. Morphs hand over without a visible change ────────────────────────────────────────────────
    let before = failed;
    let spec = MorphSpec { loosen_ms: 250.0, duration_ms: 1000.0, ..MorphSpec::default() };
    // Times that are exact in binary, so the clocks of the compared frames agree to the bit.
    let (t0, t1, t2) = (50.0f32, 50.25f32, 51.25f32);
    let landed = t2 - 1.0 / 64.0;
    let mut worst = 0;
    for (pair, name) in PAIR_NAMES.iter().enumerate() {
        for material in [Material::Plain, Material::Dot] {
            let look = Look { theme: DARK, material, ..Look::default() };
            let new = |s: Stage| { let mut ui = Ui::new(s, stage, |_, _| {}, fonts(), W, H, 1.0); ui.pointer_leave(); ui };
            let mut moving = new(Stage { pair, loose: [0.0, 0.0], shown: [true, false] });
            moving.tick(t0);
            moving.morph("a", "b", spec);
            let (ra, rb) = (moving.node("a").expect("a").rect, moving.node("b").expect("b").rect);
            // (a) The hand-over.
            let mut src = new(Stage { pair, loose: [1.0, 0.0], shown: [true, false] });
            src.tick(t1);
            moving.tick(t1);
            if moving.morph_run().and_then(|r| r.clock(t1)) != Some(0.0) { eprintln!("FAIL morph {pair}: not in flight at the hand-over"); failed += 1; }
            let d = diff(&o.render(&mut src, &look), &o.render(&mut moving, &look), ra);
            // (b) The landing.
            let mut dst = new(Stage { pair, loose: [0.0, 1.0], shown: [false, true] });
            dst.tick(landed);
            moving.tick(landed);
            let e = diff(&o.render(&mut dst, &look), &o.render(&mut moving, &look), rb);
            eprintln!("   morph {} {:?}: hand-over {d}/255, landing {e}/255", name, material);
            worst = worst.max(d).max(e);
            if d > 2 || e > 2 { eprintln!("FAIL morph {}: {:?} differs", name, material); failed += 1; }
        }
        // (c) Mid-flight, light theme: tiles outside both elements' rects.
        let look = Look { theme: LIGHT, material: Material::Dot, ..Look::default() };
        let mut moving = Ui::new(Stage { pair, loose: [0.0, 0.0], shown: [true, false] }, stage, |_, _| {}, fonts(), W, H, 1.0);
        moving.pointer_leave();
        moving.tick(t0);
        moving.morph("a", "b", spec);
        let (ra, rb) = (moving.node("a").expect("a").rect, moving.node("b").expect("b").rect);
        moving.tick(t1 + 0.5);
        let img = o.render(&mut moving, &look);
        let ground = rgb8(LIGHT.ground);
        let out = |r: lab::ui::Rect, x: u32, y: u32| (x as f32) < r.x - 16.0 || (x as f32) >= r.right() + 16.0 || (y as f32) < r.y - 16.0 || (y as f32) >= r.bottom() + 16.0;
        // The ground units of the dot material are within about 70/255 of the ground; tiles are much further.
        let far = |c: [u8; 3]| (0..3).any(|i| (c[i] as i32 - ground[i] as i32).abs() > 100);
        let n = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| out(ra, x, y) && out(rb, x, y) && far(px(&img, W, x, y))).count();
        eprintln!("   morph {} mid-flight: {n} tile pixels outside both elements", name);
        if n < 300 { eprintln!("FAIL morph {}: nothing in the air", name); failed += 1; }
        // Control: the same count at rest finds nothing.
        let mut rest = Ui::new(Stage { pair, loose: [0.0, 0.0], shown: [true, false] }, stage, |_, _| {}, fonts(), W, H, 1.0);
        rest.pointer_leave();
        rest.tick(t0);
        let img = o.render(&mut rest, &look);
        let n = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| out(ra, x, y) && out(rb, x, y) && far(px(&img, W, x, y))).count();
        if n > 0 { eprintln!("FAIL control: {n} far pixels outside the elements at rest"); failed += 1; }
    }
    eprintln!("4. morphs: largest difference {worst}/255, {} failing", failed - before);

    // ── 5. Every material is restrained at rest ─────────────────────────────────────────────────────
    let before = failed;
    for theme in [DARK, LIGHT] {
        let mut ui = Ui::new(Rest { loose: 0.0 }, rest, |_, _| {}, fonts(), W, H, 1.0);
        ui.pointer_leave();
        ui.tick(100.0);
        let r = ui.node("body").expect("body").rect;
        let plain = o.render(&mut ui, &Look { theme, material: Material::Plain, ..Look::default() });
        for m in 0..MATERIAL_COUNT as u8 {
            let mat = Material::from_id(m);
            if matches!(mat, Material::Chrome | Material::Liquid | Material::Plain) { continue; }
            let look = Look { theme, material: mat, ..Look::default() };
            let img = o.render(&mut ui, &look);
            // The dot material's ground has its units at rest (§1.6): there only the glyphs' own
            // pixels are compared.
            let d = if mat == Material::Dot { diff_ink(&plain, &img, r, rgb8(theme.ground)) } else { diff(&plain, &img, r) };
            // Print's paper grain moves the ground by at most grain × 2.75 % (7/255 at the default).
            let allowed = if mat == Material::Print { 8 } else { 0 };
            eprintln!("   rest {} {:?}: body differs from plain by {d}/255", theme.name, mat);
            if d > allowed { eprintln!("FAIL rest: {:?} body text is not exact at rest ({d}/255)", mat); failed += 1; }
            if theme.name == DARK.name && !matches!(mat, Material::Collage | Material::Xray) {
                // Control: loosened, the same comparison sees the material.
                ui.state.loose = 1.0; ui.relayout(); ui.tick(100.0);
                let d1 = diff(&plain, &o.render(&mut ui, &look), r);
                ui.state.loose = 0.0; ui.relayout(); ui.tick(100.0);
                if d1 <= allowed { eprintln!("FAIL control: {:?} loosened looks like plain", mat); failed += 1; }
            }
        }
    }
    eprintln!("5. rest: {} failing", failed - before);

    // ── 6. No material shows the ground inside a loosened picture ───────────────────────────────────
    let before = failed;
    let ground = rgb8(LIGHT.ground);
    let mut ui = Ui::new(One { id: BUST, loose: 1.0 }, one, |_, _| {}, fonts(), W, H, 1.0);
    ui.pointer_leave();
    ui.tick(100.0);
    let (x0, y0, x1, y1) = device_rect(ui.node("pic").expect("pic").rect, 1.0);
    for m in 0..MATERIAL_COUNT as u8 {
        for f in [PicFx::default(), PicFx { pitch: 12.0, ground: 0.0, ..PicFx::with_set(set::CHARS) }, PicFx { pitch: 6.0, ..PicFx::with_set(set::STROKE) }] {
            let mut look = Look { theme: LIGHT, material: Material::from_id(m), ..Look::default() };
            look.pics[0] = f;
            // A front mid-way, too, for the materials that move things.
            ui.press(30.0, 30.0); ui.release(30.0, 30.0); ui.tick(100.3);
            let img = o.render(&mut ui, &look);
            let n = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| close(px(&img, W, x, y), ground)).count();
            if n > 0 { eprintln!("FAIL ground shows in a loosened picture under {:?}: {n} px", Material::from_id(m)); failed += 1; }
            ui.tick(110.0);
        }
    }
    eprintln!("6. pictures under every material: {} failing", failed - before);

    // ── 7. Annotation frames on the layout rects ────────────────────────────────────────────────────
    let before = failed;
    let magenta = [255u8, 0, 255];
    let g = Gaze { mode: GazeMode::All, kinds: gaze::FRAME, density: 20.0, draw_ms: 100.0, colour: [1.0, 0.0, 1.0], only_marked: false };
    let mut ui = Ui::new(g, framed, |_, _| {}, fonts(), W, H, 1.0).on_gaze(|g| *g);
    ui.pointer_leave();
    ui.tick(5.0);
    let look = Look { theme: DARK, material: Material::Plain, ..Look::default() };
    let img = o.render(&mut ui, &look);
    let mut frames = 0;
    for a in ui.annots() {
        let (x0, y0, x1, y1) = device_rect(a.rect, 1.0);
        let corners = [(x0, y0), (x1 - 1, y0), (x0, y1 - 1), (x1 - 1, y1 - 1)];
        let outside = [(x0 - 1, y0), (x0, y0 - 1), (x1, y1 - 1), (x1 - 1, y1)];
        let on = corners.iter().all(|&(x, y)| px(&img, W, x, y) == magenta);
        let off = outside.iter().all(|&(x, y)| px(&img, W, x, y) != magenta);
        if !on || !off { eprintln!("FAIL frame of {} is not on its rect ({on}, {off})", a.key); failed += 1; }
        frames += 1;
    }
    if frames < 4 { eprintln!("FAIL only {frames} frames"); failed += 1; }
    eprintln!("7. annotation frames: {frames} checked, {} failing", failed - before);

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

/// Two elements side by side, either of which can be drawn loose or hidden: the source "a" and the
/// target "b" of a morph.
struct Stage { pair: usize, loose: [f32; 2], shown: [bool; 2] }
const PAIRS: usize = 3;
const PAIR_NAMES: [&str; PAIRS] = ["text→picture", "shape→paragraph", "picture→text"];
/// `loose` is not inherited, so it goes on the leaf.
fn element(kind: u8, k: f32) -> El<()> {
    match kind {
        0 => text_lh("变换", 96.0, 120.0, Paint::INK).loose(k),
        1 => { let (w, h) = PICTURE_SIZES[SKY as usize]; col().width(260.0).child(picture(SKY, h as f32 / w as f32).loose(k)) }
        2 => space(160.0, 160.0).radius(80.0).fill(Paint::ACCENT).loose(k),
        _ => col().width(240.0).child(text_lh("屏幕上的任何东西都能散成粒子，飞到别处，变成另一样东西。", 16.0, 28.0, Paint::TEXT).loose(k)),
    }
}
fn stage(s: &Stage) -> El<()> {
    let (a, b) = [(0, 1), (2, 3), (1, 0)][s.pair];
    row().pad(30.0, 60.0).gap(200.0).align(taffy::AlignItems::CENTER)
        .child(element(a, s.loose[0]).key("a").hidden_until_morphed(!s.shown[0]))
        .child(element(b, s.loose[1]).key("b").hidden_until_morphed(!s.shown[1]))
}
/// Largest channel difference over a rect (logical = device px here).
fn diff(a: &[u8], b: &[u8], r: lab::ui::Rect) -> i32 {
    let (x0, y0, x1, y1) = device_rect(r, 1.0);
    let mut d = 0;
    for y in y0..y1.min(H) { for x in x0..x1.min(W) {
        let (p, q) = (px(a, W, x, y), px(b, W, x, y));
        for c in 0..3 { d = d.max((p[c] as i32 - q[c] as i32).abs()); }
    } }
    d
}

/// Body text on the ground, with a heading, a picture and a control around it.
struct Rest { loose: f32 }
fn rest(s: &Rest) -> El<()> {
    let (w, h) = PICTURE_SIZES[SKY as usize];
    col().pad(30.0, 20.0).gap(20.0).loosen(s.loose)
        .child(text_lh("松开与合拢", 52.0, 64.0, Paint::INK).key("head"))
        .child(row().gap(30.0)
            .child(col().width(330.0).child(text_lh("材质决定松开时画什么。静止时，页面是一张干净、可读的页面；波前经过时按轮廓松开。Body text 0123.", 16.0, 27.0, Paint::TEXT).key("body")))
            .child(col().width(260.0).child(picture(SKY, h as f32 / w as f32).key("pic"))))
        .child(space(120.0, 30.0).radius(4.0).fill(Paint::ACCENT))
}
/// Separate elements to frame.
fn framed(_: &Gaze) -> El<()> {
    col().pad(40.0, 40.0).gap(40.0)
        .child(space(200.0, 60.0).fill(Paint::FAINT).key("f1"))
        .child(row().gap(50.0).child(space(90.0, 90.0).radius(45.0).fill(Paint::ACCENT).key("f2")).child(text_lh("标注", 30.0, 40.0, Paint::INK).key("f3")))
        .child(space(310.0, 37.0).border(1.0, Paint::DIM).key("f4"))
}
/// Largest channel difference over the pixels of a rect where `a` is not the ground.
fn diff_ink(a: &[u8], b: &[u8], r: lab::ui::Rect, ground: [u8; 3]) -> i32 {
    let (x0, y0, x1, y1) = device_rect(r, 1.0);
    let mut d = 0;
    for y in y0..y1.min(H) { for x in x0..x1.min(W) {
        let (p, q) = (px(a, W, x, y), px(b, W, x, y));
        if close(p, ground) { continue; }
        for c in 0..3 { d = d.max((p[c] as i32 - q[c] as i32).abs()); }
    } }
    d
}
