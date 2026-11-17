//! Sample sheets check themselves (`amenui::sample`): every material's sheet at 1, 0.5 and 0 with its
//! arrival cells holds what it asserts; `scribblr`'s at 1, 0.7, 0.4, 0.2 and 0 has a thread when fully
//! loose and is plain at rest; a sheet whose cells are all the same fails. Needs a GPU adapter
//! (lavapipe will do); without one the tests say so and pass.
use amenui::offscreen::Offscreen;
use amenui::sample::{CELL, SampleError, sheet};
use amenui::style::{MATERIALS, Material};

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[test]
fn every_material_s_sheet_holds_what_it_asserts() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let s = sheet(&mut o, &fonts, Material::Scribblr, &[1.0, 0.7, 0.4, 0.2, 0.0], 0, true).expect("scribblr's sheet");
    assert_eq!(s.cells.len(), 8);
    assert_eq!(s.px.len(), (s.w * s.h * 4) as usize);
    for (m, name) in MATERIALS {
        for theme in [0, 1] {
            if let Err(e) = sheet(&mut o, &fonts, m, &[1.0, 0.5, 0.0], theme, true) { panic!("{name}, theme {theme}: {e}"); }
        }
    }
}

#[test]
fn a_sheet_that_shows_nothing_fails() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let e = sheet(&mut o, &fonts, Material::Scribblr, &[0.0, 0.0], 0, false).err();
    assert!(matches!(e, Some(SampleError::Same { .. })), "{e:?}");
}

/// One cell of the sample page, drawn: the page itself (for its elements' boxes) and its pixels.
fn cell(o: &mut Offscreen, material: Material, theme: usize, k: f32) -> (amenui::ui::Ui<amenui::sample::Sample, ()>, Vec<u8>) {
    let look = amenui::style::Look { material, theme: amenui::style::THEMES[theme], ..amenui::style::Look::default() };
    let mut ui = amenui::ui::Ui::new(amenui::sample::Sample { k, label: format!("{} · loose {k:.2}", material.name()), look, cmds: vec![] }, amenui::sample::view, |_, _| {}, fonts(), CELL.0, CELL.1, 1.0).on_look(|s| s.look);
    ui.relayout();
    let mut t = 0.0;
    while t < 0.5 { t += 1.0 / 60.0; ui.tick(t); }
    let px = o.render(&mut ui, &look);
    (ui, px)
}
/// The ink box (device px) of the instances of the element `key` (a glyph's without its spread).
fn ink_of(ui: &amenui::ui::Ui<amenui::sample::Sample, ()>, key: &str) -> [i32; 4] {
    let sp = amenui::text::SPREAD as f32;
    let b = ui.scene().instances.iter().enumerate().filter(|(i, _)| ui.owner_key(*i) == Some(key)).fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |a, (_, x)| {
        let p = if x.paint[3] == amenui::scene::GLYPH { sp } else { 0.0 };
        [a[0].min(x.rect[0] + p), a[1].min(x.rect[1] + p), a[2].max(x.rect[0] + x.rect[2] - p), a[3].max(x.rect[1] + x.rect[3] - p)]
    });
    [b[0].floor() as i32, b[1].floor() as i32, b[2].ceil() as i32, b[3].ceil() as i32]
}

#[test]
fn four_materials_go_on_coming_apart_past_half_loose() {
    // Swept from 0 to 1 in four steps: the title comes apart more from 0.25 to 0.5 (the units form),
    // and the last two steps (the units themselves changing) each still change at least a tenth of
    // what the whole sweep changed. (How much differs from rest saturates once the title is all units,
    // so the steps past half are measured against the step before.)
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    for m in [Material::Dottr, Material::Charr, Material::Halftonr, Material::Glitchr] {
        for theme in [0, 1] {
            let s = amenui::sample::sweep(&mut o, &fonts, m, 4, theme).expect("the sweep");
            let c: Vec<f32> = s.changed.iter().map(|c| c.expect("a looseness cell")).collect();
            let mv: Vec<f32> = s.moved.iter().skip(1).map(|c| c.expect("a step")).collect();
            eprintln!("{} (theme {theme}): changed {c:.3?}, step by step {mv:.3?}", m.name());
            assert!(c[1] > 0.0 && c[2] > c[1], "{}: the title does not come apart from 0.25 to 0.5: {c:?}", m.name());
            for (i, v) in mv.iter().enumerate().skip(2) { assert!(*v >= 0.1 * c[4], "{}: the step to {} changes {v:.3}, under a tenth of {:.3}", m.name(), (i + 1) as f32 * 0.25, c[4]); }
        }
    }
}

#[test]
fn halftonr_keeps_to_the_ink_and_the_accent() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let (ui, px) = cell(&mut o, Material::Halftonr, 0, 1.0);
    let at = |x: i32, y: i32| { let i = ((y as u32 * CELL.0 + x as u32) * 4) as usize; [px[i] as f32 / 255.0, px[i + 1] as f32 / 255.0, px[i + 2] as f32 / 255.0] };
    // In the title, no pixel lit in one channel only.
    let t = ink_of(&ui, "title");
    let mut worst = 0.0f32;
    for y in t[1]..t[3] { for x in t[0]..t[2] { let c = at(x, y); worst = worst.max(c.iter().copied().fold(0.0, f32::max) - c.iter().copied().fold(1.0, f32::min)); } }
    assert!(worst <= 0.55, "a pixel of the title is {worst:.2} from grey: one channel lit");
    // In the rule, every coloured pixel is the accent's hue.
    let hue = |c: [f32; 3]| { let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2])); let d = mx - mn; if d < 1e-4 { return None; } let h = if mx == c[0] { ((c[1] - c[2]) / d).rem_euclid(6.0) } else if mx == c[1] { (c[2] - c[0]) / d + 2.0 } else { (c[0] - c[1]) / d + 4.0 }; Some(h * 60.0) };
    let accent = hue(amenui::style::THEMES[0].accent).expect("a coloured accent");
    let r = ink_of(&ui, "rule");
    let mut n = 0;
    for y in r[1]..r[3] { for x in r[0]..r[2] {
        let c = at(x, y);
        if c.iter().copied().fold(0.0, f32::max) - c.iter().copied().fold(1.0, f32::min) < 0.15 { continue; }
        let h = hue(c).expect("coloured");
        let d = (h - accent).abs().min(360.0 - (h - accent).abs());
        assert!(d <= 10.0, "a pixel of the rule at ({x}, {y}) is {d:.0}° from the accent's hue");
        n += 1;
    } }
    assert!(n > 20, "{n} coloured pixels in the rule");
}

#[test]
fn glitchr_moves_only_what_is_drawn() {
    // Fully loose, and arriving: every pixel that differs from the material's rest lies within an
    // element's ink, grown across by one shift and down by one row.
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let p = amenui::style::Look::default().numbers(Material::Glitchr);
    let (row, shift) = (p.0[0].ceil() as i32, p.0[1].ceil() as i32);
    for theme in [0, 1] {
        let (rest_ui, rest) = cell(&mut o, Material::Glitchr, theme, 0.0);
        let keys: Vec<String> = rest_ui.nodes().iter().map(|n| n.key.clone()).filter(|k| !k.is_empty()).collect();
        let boxes: Vec<[i32; 4]> = keys.iter().map(|k| ink_of(&rest_ui, k)).filter(|b| b[2] > b[0]).map(|b| [b[0] - shift, b[1] - row, b[2] + shift, b[3] + row]).collect();
        let check = |px: &[u8], at: &str| {
            for y in 34..CELL.1 as i32 { for x in 0..CELL.0 as i32 {
                let i = ((y as u32 * CELL.0 + x as u32) * 4) as usize;
                if px[i..i + 4] == rest[i..i + 4] { continue; }
                assert!(boxes.iter().any(|b| x >= b[0] && x < b[2] && y >= b[1] && y < b[3]), "{at} (theme {theme}): ({x}, {y}) changed away from anything drawn");
            } }
        };
        let (_, loose) = cell(&mut o, Material::Glitchr, theme, 1.0);
        check(&loose, "loose 1");
        // Arriving: held at the caps, then condensing frame by frame.
        let look = amenui::style::Look { material: Material::Glitchr, theme: amenui::style::THEMES[theme], ..amenui::style::Look::default() };
        let mut ui = amenui::ui::Ui::new(amenui::sample::Sample { k: 0.0, label: "glitchr · loose 0.00".into(), look, cmds: vec![] }, amenui::sample::view, |_, _| {}, fonts(), CELL.0, CELL.1, 1.0).on_look(|s| s.look);
        ui.relayout();
        ui.tick(3.0);
        ui.condense_all();
        let start = ui.now();
        for ms in [0.0f32, 60.0, 120.0, 300.0] {
            let mut t = ui.now();
            while t < start + ms / 1000.0 - 1e-6 { t = (t + 1.0 / 60.0).min(start + ms / 1000.0); ui.tick(t); }
            let px = o.render(&mut ui, &look);
            check(&px, &format!("arriving +{ms} ms"));
        }
    }
}

fn hue_of(c: [f32; 3]) -> Option<f32> {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = mx - mn;
    if d < 0.15 { return None; }
    let h = if mx == c[0] { ((c[1] - c[2]) / d).rem_euclid(6.0) } else if mx == c[1] { (c[2] - c[0]) / d + 2.0 } else { (c[0] - c[1]) / d + 4.0 };
    Some(h * 60.0)
}
fn rgb_at(px: &[u8], x: i32, y: i32) -> [f32; 3] { let i = ((y as u32 * CELL.0 + x as u32) * 4) as usize; [px[i] as f32 / 255.0, px[i + 1] as f32 / 255.0, px[i + 2] as f32 / 255.0] }
fn hue_gap(a: f32, b: f32) -> f32 { let d = (a - b).abs(); d.min(360.0 - d) }

#[test]
fn halftonr_on_light_paper_has_no_yellow_and_keeps_the_rule_in_the_accent() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let (ui, px) = cell(&mut o, Material::Halftonr, 1, 1.0);
    let t = ink_of(&ui, "title");
    for y in t[1]..t[3] { for x in t[0]..t[2] {
        if let Some(h) = hue_of(rgb_at(&px, x, y)) { assert!(!(40.0..=90.0).contains(&h), "a pixel of the title at ({x}, {y}) is yellow ({h:.0}°)"); }
    } }
    let accent = hue_of(amenui::style::THEMES[1].accent).expect("a coloured accent");
    let r = ink_of(&ui, "rule");
    let mut n = 0;
    for y in r[1]..r[3] { for x in r[0]..r[2] {
        if let Some(h) = hue_of(rgb_at(&px, x, y)) { assert!(hue_gap(h, accent) <= 10.0, "a pixel of the rule at ({x}, {y}) is {:.0}° from the accent", hue_gap(h, accent)); n += 1; }
    } }
    assert!(n > 20, "{n} coloured pixels in the rule");
}

#[test]
fn charr_keeps_the_rule_whole_and_the_picture_s_edge() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    let pitch = amenui::style::Look::default().numbers(Material::Charr).0[0];
    for theme in [0, 1] {
        let (rest_ui, rest) = cell(&mut o, Material::Charr, theme, 0.0);
        let rule = ink_of(&rest_ui, "rule");
        let pic = ink_of(&rest_ui, "picture");
        let accent = hue_of(amenui::style::THEMES[theme].accent).expect("a coloured accent");
        for k in [0.25f32, 0.5, 0.75, 1.0] {
            let (_, px) = cell(&mut o, Material::Charr, theme, k);
            // The largest cell there is at this looseness.
            let p = if k >= 0.7 { (pitch * 1.5).round() } else { pitch } as i32;
            let mut x0 = rule[0];
            while x0 + p <= rule[2] {
                let found = (rule[1] - p..rule[3] + p).any(|y| (x0..x0 + p).any(|x| hue_of(rgb_at(&px, x, y)).is_some_and(|h| hue_gap(h, accent) <= 20.0)));
                assert!(found, "theme {theme}, loose {k}: the rule has no accent between x {x0} and {}", x0 + p);
                x0 += p;
            }
            // Around the picture, within a cell of its box, nothing is drawn that was not there at rest.
            for y in (pic[1] - p).max(0)..(pic[3] + p).min(CELL.1 as i32) { for x in (pic[0] - p).max(0)..(pic[2] + p).min(CELL.0 as i32) {
                if x >= pic[0] && x < pic[2] && y >= pic[1] && y < pic[3] { continue; }
                let i = ((y as u32 * CELL.0 + x as u32) * 4) as usize;
                assert!(px[i..i + 3] == rest[i..i + 3], "theme {theme}, loose {k}: ({x}, {y}) beside the picture changed");
            } }
        }
    }
}

#[test]
fn pictures_look_their_own_way_in_each_material() {
    let Ok(mut o) = Offscreen::try_new(CELL.0, CELL.1) else { eprintln!("no GPU adapter here: skipped"); return };
    for theme in [0, 1] {
        let ms = [Material::Dottr, Material::Charr, Material::Halftonr];
        let shots: Vec<_> = ms.iter().map(|m| cell(&mut o, *m, theme, 1.0)).collect();
        let b = ink_of(&shots[0].0, "picture");
        let area = ((b[2] - b[0]) * (b[3] - b[1])) as f32;
        for i in 0..3 { for j in i + 1..3 {
            let mut d = 0;
            for y in b[1]..b[3] { for x in b[0]..b[2] { let k = ((y as u32 * CELL.0 + x as u32) * 4) as usize; if shots[i].1[k..k + 3] != shots[j].1[k..k + 3] { d += 1; } } }
            assert!(d as f32 > 0.25 * area, "theme {theme}: {:?} and {:?} differ in only {:.0} % of the picture", ms[i], ms[j], d as f32 / area * 100.0);
        } }
        // halftonr's dots: the largest at least twice as wide as the smallest (blobs of the dot colour
        // that do not touch the box's edge).
        let (_, px) = &shots[2];
        let g = rgb_at(px, b[0] - 6, b[1] + 2);
        let dot = |x: i32, y: i32| { let c = rgb_at(px, x, y); (0..3).map(|k| (c[k] - g[k]).abs()).fold(0.0, f32::max) > 0.08 };
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        let mut seen = vec![false; (w * h) as usize];
        let mut sizes = vec![];
        for y in 0..h { for x in 0..w {
            if seen[(y * w + x) as usize] || !dot(b[0] + x, b[1] + y) { continue; }
            let (mut st, mut n, mut edge) = (vec![(x, y)], 0, false);
            seen[(y * w + x) as usize] = true;
            while let Some((cx, cy)) = st.pop() {
                n += 1;
                if cx == 0 || cy == 0 || cx == w - 1 || cy == h - 1 { edge = true; }
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[(ny * w + nx) as usize] || !dot(b[0] + nx, b[1] + ny) { continue; }
                    seen[(ny * w + nx) as usize] = true;
                    st.push((nx, ny));
                }
            }
            if !edge && n >= 2 { sizes.push(n as f32); }
        } }
        let (mx, mn) = sizes.iter().fold((0.0f32, f32::INFINITY), |a, s| (a.0.max(*s), a.1.min(*s)));
        assert!(sizes.len() >= 4 && (mx / mn).sqrt() >= 2.0, "theme {theme}: {} dots, widths {:.1}–{:.1}", sizes.len(), mn.sqrt(), mx.sqrt());
    }
}
