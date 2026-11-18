//! The `render` tool's core (DESIGN.md §1.17): deterministic frames, scripted events, and errors that say
//! what to write instead.
use amenui::drive::{Input, RenderSpec, parse_events, render};
use amenui::error::{DriveError, LookError};
use amenui::offscreen::Offscreen;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn spec(page: &str) -> RenderSpec { RenderSpec { page: page.into(), w: 480, h: 360, scale: 1.0, material: Some("dottr".into()), theme: Some("light".into()), at_ms: 700.0, events: vec![], locale: None } }

#[test]
fn the_same_arguments_give_the_same_bytes() {
    let Ok(mut o) = Offscreen::try_new(480, 360) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut s = spec("recipe-counter");
    s.events = parse_events(r#"[{"at_ms": 200, "kind": "press", "target": "inc"}, {"at_ms": 230, "kind": "release", "target": "inc"}]"#, "inline").unwrap();
    let a = render(&s, fonts(), Some(&mut o)).expect("render");
    let b = render(&s, fonts(), Some(&mut o)).expect("render");
    assert_eq!(a.png, b.png, "two renders differ");
    // A fresh renderer gives the same bytes too.
    let mut o2 = Offscreen::try_new(480, 360).expect("adapter");
    assert_eq!(render(&s, fonts(), Some(&mut o2)).expect("render").png, a.png);
    // The events were played: the count went up.
    assert_eq!(a.tree.find("count").and_then(|n| n.text.clone()).as_deref(), Some("10"));
    // A different moment is a different frame.
    let mut later = s.clone();
    later.at_ms = 260.0;
    assert_ne!(render(&later, fonts(), Some(&mut o)).expect("render").png, a.png);
}

#[test]
fn events_parse_and_errors_say_what_to_write() {
    let ev = parse_events(r#"[{"at_ms": 0, "kind": "key", "key": "Tab"}, {"at_ms": 5, "kind": "text", "text": "hi"}, {"at_ms": 9, "kind": "wheel", "x": 1, "y": 2, "dy": 40}]"#, "x").unwrap();
    assert_eq!(ev[0].input, Input::Key { key: "Tab".into(), shift: false, cmd: false, alt: false });
    assert!(matches!(ev[2].input, Input::Wheel { dy, .. } if dy == 40.0));
    let bad = parse_events(r#"[{"at_ms": 0, "kind": "poke"}]"#, "s.json").unwrap_err();
    assert!(bad.to_string().contains("s.json") && bad.to_string().contains("press, release"), "{bad}");
    assert!(matches!(amenui::drive::material("dott"), Err(LookError::UnknownMaterial { .. })));
    let m = amenui::drive::material("dott").unwrap_err().to_string();
    assert!(m.contains("dottr") && m.contains("halftonr"), "{m}");
    assert!(amenui::drive::theme("sepia").unwrap_err().to_string().contains("light"));
    assert_eq!(amenui::drive::theme("light"), Ok(1));
    let e = match amenui::drive::open("buton", fonts(), 100, 100, 1.0, None, None) { Err(e) => e, Ok(_) => panic!("opened") };
    assert!(e.to_string().contains("recipe-form"), "{e}");
    // A target that is not there names keys that are.
    let mut d = amenui::drive::open("recipe-counter", fonts(), 400, 300, 1.0, None, None).expect("open");
    let err = d.apply(&Input::Press { x: None, y: None, target: Some("incc".into()), shift: false }).unwrap_err();
    assert!(matches!(err, DriveError::UnknownTarget { .. }) && err.to_string().contains("inc"), "{err}");
    assert!(d.apply(&Input::Key { key: "Entr".into(), shift: false, cmd: false, alt: false }).unwrap_err().to_string().contains("Enter"));
    // Zero size.
    let mut z = spec("recipe-form");
    z.w = 0;
    assert!(render(&z, fonts(), None).err().expect("an error").to_string().contains("--size"));
    assert!(Offscreen::try_new(0, 10).is_err());
    // A missing font directory.
    assert!(amenui::text::load_bundled("/nowhere").err().expect("an error").to_string().contains("assets"));
}

#[test]
fn a_drag_through_the_field_map_gives_the_same_bytes_three_times() {
    let Ok(mut o) = Offscreen::try_new(900, 640) else { eprintln!("no GPU adapter here: skipped"); return };
    let mut s = RenderSpec { page: "recipe-gesture".into(), w: 900, h: 640, scale: 1.0, material: Some("dottr".into()), theme: Some("light".into()), at_ms: 600.0, events: vec![], locale: None };
    let mut ev = vec![r#"{"at_ms": 100, "kind": "press", "target": "pip/label"}"#.to_owned()];
    for i in 1..=12 { ev.push(format!(r#"{{"at_ms": {}, "kind": "move", "x": {}, "y": {}}}"#, 100 + i * 16, 60 + i * 12, 150 + i * 8)); }
    ev.push(r#"{"at_ms": 320, "kind": "release", "x": 204, "y": 246}"#.to_owned());
    s.events = parse_events(&format!("[{}]", ev.join(",")), "inline").unwrap();
    let a = render(&s, fonts(), Some(&mut o)).expect("render");
    let b = render(&s, fonts(), Some(&mut o)).expect("render");
    let mut o2 = Offscreen::try_new(900, 640).expect("adapter");
    let c = render(&s, fonts(), Some(&mut o2)).expect("render");
    assert!(a.png == b.png && b.png == c.png, "three renders of a drag differ");
    // The drag stirred the map: the same moment without it is another frame.
    let mut still = s.clone();
    still.events.clear();
    assert_ne!(render(&still, fonts(), Some(&mut o)).expect("render").png, a.png);
}

/// The ink pixels of the `Send` label on the form recipe's solid button, drawn in `material` and in
/// plain, `theme` (0 dark, 1 light).
fn send_ink(o: &mut amenui::offscreen::Offscreen, material: amenui::style::Material, theme: usize) -> usize {
    let mut ui = amenui::recipes::form::ui(fonts(), 480, 360, 1.0);
    ui.tick(1.5);
    let look = amenui::style::Look { material, theme: amenui::style::THEMES[theme], ..ui.look() };
    o.r.partial = false;
    let px = o.render(&mut ui, &look);
    o.r.hide_glyphs = true;
    ui.invalidate();
    let ground = o.render(&mut ui, &look);
    o.r.hide_glyphs = false;
    let node = ui.nodes().iter().position(|n| n.text() == Some("Send")).expect("the Send label");
    let boxes: Vec<[f32; 4]> = ui.glyphs().log.iter().filter(|g| g.node == node).map(|g| amenui::form::ink_box(&ui.scene().instances[g.inst])).collect();
    assert_eq!(boxes.len(), 4, "four glyphs");
    amenui::audit::ink_pixels(&px, &ground, ui.device_size(), &boxes)
}

#[test]
fn near_keeps_a_label_on_a_solid_button_whole() {
    let Ok(mut o) = amenui::offscreen::Offscreen::try_new(480, 360) else { eprintln!("no GPU adapter here: skipped"); return };
    use amenui::style::Material;
    for theme in [0, 1] {
        let plain = send_ink(&mut o, Material::Plain, theme);
        let near = send_ink(&mut o, Material::Near, theme);
        assert!(plain > 60, "theme {theme}: plain draws the label ({plain} px)");
        assert!(near as f32 >= 0.9 * plain as f32, "theme {theme}: near draws {near} of plain's {plain} ink px");
    }
}

/// A page with a line of text to read in the middle and, when `block`, a status line along the bottom
/// right; the readout on, in `at`, `inset` from the edges.
mod readout {
    use amenui::el::{El, Paint, col, space, text};
    use amenui::style::{Corner, Look, Material};
    pub struct S { pub at: Corner, pub inset: [f32; 2], pub block: bool, pub keep: bool, pub mode: u8 }
    pub fn view(s: &S) -> El<()> {
        let mut page = col().key("page").pad(160.0, 150.0).fill_width().fill_height()
            .child(text("A line to read in the middle", 15.0, Paint::TEXT).key("middle"));
        if s.block { page = page.child(space(0.0, 0.0).key("gap").fill_height()).child(text("status: 3 notes saved, last at 12:04", 13.0, Paint::DIM).key("status").absolute(None, None, Some(10.0), Some(20.0))); }
        page
    }
    pub fn look(s: &S) -> Look { Look { material: Material::Plain, overlay: s.mode, overlay_at: s.at, overlay_inset: s.inset, overlay_keep: s.keep, ..Look::default() } }
}

#[test]
fn the_readout_sits_in_its_corner_at_its_inset_and_never_covers_text_to_read() {
    use amenui::style::Corner;
    let (w, h) = (640u32, 400u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let ink = |ui: &amenui::ui::Ui<readout::S, ()>, key: &str| {
        let sp = amenui::text::SPREAD as f32;
        let r = ui.instances_of(key).iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |m, i| { let p = if i.paint[3] == amenui::scene::GLYPH { sp } else { 0.0 }; [m[0].min(i.rect[0] + p), m[1].min(i.rect[1] + p), m[2].max(i.rect[0] + i.rect[2] - p), m[3].max(i.rect[1] + i.rect[3] - p)] });
        amenui::ui::Rect::new(r[0], r[1], r[2] - r[0], r[3] - r[1])
    };
    let shot = |o: &mut Offscreen, at: Corner, inset: [f32; 2], block: bool| {
        let mut ui = amenui::ui::Ui::new(readout::S { at, inset, block, keep: false, mode: 1 }, readout::view, |_, _| {}, fonts(), w, h, 1.0).on_look(readout::look);
        ui.tick(1.0);
        let look = ui.look();
        let _ = o.render(&mut ui, &look);
        ui
    };
    for (at, inset) in [(Corner::TopLeft, [12.0, 12.0]), (Corner::TopRight, [20.0, 8.0]), (Corner::BottomRight, [12.0, 12.0]), (Corner::BottomLeft, [30.0, 16.0])] {
        let ui = shot(&mut o, at, inset, false);
        let b = ui.overlay_box().expect("the readout is drawn");
        let (dx, dy) = match at {
            Corner::TopLeft => (b.x, b.y),
            Corner::TopRight => (w as f32 - b.right(), b.y),
            Corner::BottomRight => (w as f32 - b.right(), h as f32 - b.bottom()),
            Corner::BottomLeft => (b.x, h as f32 - b.bottom()),
        };
        assert!((dx - inset[0]).abs() <= 1.0 && (dy - inset[1]).abs() <= 1.0, "{at:?}: {dx}, {dy} from the edges, asked {inset:?}");
        assert!(b.intersect(&ink(&ui, "middle")).w <= 0.0 || b.intersect(&ink(&ui, "middle")).h <= 0.0, "{at:?}: it covers the text");
    }
    // A status line in the bottom right: the readout asked there moves on clockwise. The bottom left
    // would cover it too (the readout is wider than half the window), so it goes on to the top left.
    let ui = shot(&mut o, Corner::BottomRight, [12.0, 12.0], true);
    let b = ui.overlay_box().expect("drawn");
    let status = ink(&ui, "status");
    let r = b.intersect(&status);
    assert!(r.w <= 0.0 || r.h <= 0.0, "the readout {b:?} covers the status line {status:?}");
    assert!(b.x - 12.0 > -1.0 && (b.x - 12.0).abs() <= 1.0 && (b.y - 12.0).abs() <= 1.0, "it went to the top left: {b:?}");
}

#[test]
fn the_readout_can_keep_its_corner_and_fits_a_narrow_window() {
    use amenui::style::Corner;
    let mk = |w: u32, h: u32, block: bool, keep: bool, mode: u8| {
        let Ok(mut o) = Offscreen::try_new(w, h) else { return None };
        let mut ui = amenui::ui::Ui::new(readout::S { at: Corner::BottomRight, inset: [12.0, 12.0], block, keep, mode }, readout::view, |_, _| {}, fonts(), w, h, 1.0).on_look(readout::look);
        ui.tick(1.0);
        let look = ui.look();
        let _ = o.render(&mut ui, &look);
        Some(ui.overlay_box().expect("drawn"))
    };
    let Some(b) = mk(640, 400, true, true, 1) else { eprintln!("no GPU adapter here: skipped"); return };
    assert!((640.0 - b.right() - 12.0).abs() <= 1.0 && (400.0 - b.bottom() - 12.0).abs() <= 1.0, "kept in the bottom right: {b:?}");
    // At 390 px both forms fit inside the window, at the inset.
    for mode in [1u8, 2] {
        let b = mk(390, 844, false, false, mode).unwrap();
        assert!(b.x >= 11.0 && b.right() <= 390.0 - 11.0 && b.y >= 0.0 && b.bottom() <= 844.0, "mode {mode}: {b:?} is not inside 390 px");
    }
}

/// Two overlapping squares (ink below, accent on top) in a box set see-through as one, and a text
/// beside them; `op` None writes no opacity at all, `gone` leaves the box out.
mod seethrough {
    use amenui::el::{El, Paint, col, space, text};
    use amenui::style::{Look, Material};
    pub struct S { pub op: Option<f32>, pub gone: bool }
    pub fn view(s: &S) -> El<()> {
        let pair = col().key("pair").width(140.0).height(100.0).absolute(Some(60.0), Some(60.0), None, None)
            .child(space(80.0, 60.0).fill(Paint::INK).key("under").absolute(Some(0.0), Some(0.0), None, None))
            .child(space(80.0, 60.0).fill(Paint::ACCENT).key("over").absolute(Some(40.0), Some(30.0), None, None));
        let pair = match s.op { Some(k) => pair.opacity(k), None => pair };
        let page = col().key("page").fill_width().fill_height().child(text("beside it", 16.0, Paint::TEXT).key("note").absolute(Some(260.0), Some(80.0), None, None));
        if s.gone { page } else { page.child(pair) }
    }
    pub fn look(_: &S) -> Look { Look { material: Material::Plain, ..Look::default() } }
}

#[test]
fn opacity_composites_the_subtree_as_one() {
    let (w, h) = (360u32, 240u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, op: Option<f32>, gone: bool| {
        let mut ui = amenui::ui::Ui::new(seethrough::S { op, gone }, seethrough::view, |_, _| {}, fonts(), w, h, 1.0).on_look(seethrough::look);
        ui.tick(3.0);
        let look = ui.look();
        (o.render(&mut ui, &look), look)
    };
    let (none, look) = shot(&mut o, None, false);
    assert!(shot(&mut o, Some(1.0), false).0 == none, "opacity(1) is the same as none written");
    assert!(shot(&mut o, Some(0.0), false).0 == shot(&mut o, None, true).0, "opacity(0) is the same as the element left out");
    // At 0.5 the overlap is the top square at half over the ground, not both layered.
    let (half, _) = shot(&mut o, Some(0.5), false);
    let px = |f: &[u8], x: u32, y: u32| { let i = ((y * w + x) * 4) as usize; [f[i] as f32, f[i + 1] as f32, f[i + 2] as f32] };
    let ground = px(&none, 20, 20);
    let (top, under) = (px(&none, 130, 110), px(&none, 70, 70));
    assert!(top != under, "the two squares differ");
    let want: Vec<f32> = (0..3).map(|k| ground[k] * 0.5 + top[k] * 0.5).collect();
    let got = px(&half, 130, 110);
    for k in 0..3 { assert!((got[k] - want[k]).abs() <= 3.0, "the overlap is {got:?}, the top at half is {want:?} (top {top:?}, under {under:?}, ground {ground:?})"); }
    // Where only the lower one is, it is the lower one at half.
    let (got, want) = (px(&half, 70, 70), (0..3).map(|k| ground[k] * 0.5 + under[k] * 0.5).collect::<Vec<_>>());
    for k in 0..3 { assert!((got[k] - want[k]).abs() <= 3.0, "{got:?} vs {want:?}"); }
    let _ = look;
}

/// A card with a shifted badge in it, written at `k` by hand (`hand`) or at 1 and zoomed by `k`.
mod zoomed {
    use amenui::el::{El, Paint, col, row, space, text_lh};
    use amenui::style::{Look, Material};
    pub struct S { pub k: f32, pub hand: bool }
    pub fn card(k: f32) -> El<()> {
        col().key("card").pad(16.0 * k, 12.0 * k).gap(8.0 * k).radius(10.0 * k).fill(Paint::FAINT)
            .child(text_lh("Shifted", 14.0 * k, 22.0 * k, Paint::TEXT).key("title"))
            .child(row().key("holder").gap(6.0 * k)
                .child(space(60.0 * k, 24.0 * k).radius(4.0 * k).fill(Paint::ACCENT).key("badge").shift(40.0 * k, -24.0 * k))
                .child(space(30.0 * k, 24.0 * k).fill(Paint::INK).key("plain")))
    }
    pub fn view(s: &S) -> El<()> {
        let c = if s.hand { card(s.k) } else { card(1.0).zoom(s.k) };
        col().key("page").pad(40.0, 60.0).fill_width().fill_height().child(c)
    }
    pub fn look(_: &S) -> Look { Look { material: Material::Dottr, ..Look::default() } }
}

#[test]
fn zoom_scales_a_shift_with_every_other_length() {
    let (w, h) = (480u32, 360u32);
    let mk = |k: f32, hand: bool| { let mut ui = amenui::ui::Ui::new(zoomed::S { k, hand }, zoomed::view, |_, _| {}, fonts(), w, h, 1.0).on_look(zoomed::look); ui.tick(2.0); ui };
    for k in [0.5f32, 0.75, 1.5, 2.0] {
        let ui = mk(k, false);
        let (b, p) = (ui.node("badge").unwrap().rect, ui.node("holder").unwrap().rect);
        // Unshifted, the badge sits at the holder's top left: its offset is the shift alone.
        let (dx, dy) = (b.x - p.x, b.y - p.y);
        assert!((dx - 40.0 * k).abs() < 0.5 && (dy + 24.0 * k).abs() < 0.5, "k {k}: the badge is moved by ({dx}, {dy}), want {:?}", (40.0 * k, -24.0 * k));
    }
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, k: f32, hand: bool| { let mut ui = mk(k, hand); let look = ui.look(); o.render(&mut ui, &look) };
    assert!(shot(&mut o, 1.0, false) == shot(&mut o, 1.0, true), "zoom(1) changes nothing");
    for k in [0.5f32, 0.75, 1.5, 2.0] {
        assert!(shot(&mut o, k, false) == shot(&mut o, k, true), "k {k}: zoomed and written by hand differ");
    }
}

/// A row that may wrap, of 3 to 12 texts and boxes with fractional widths and gaps, in a column as
/// wide as its content (or `less` px narrower than that).
mod wrapfit {
    use amenui::el::{El, Paint, col, row, space, text};
    pub struct S { pub seed: u64, pub zoom: f32, pub less: Option<f32> }
    fn rnd(s: &mut u64) -> f32 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; (*s % 10000) as f32 / 10000.0 }
    pub fn line(seed: u64) -> El<()> {
        let mut s = seed.wrapping_mul(2654435761).max(1);
        let n = 3 + (rnd(&mut s) * 10.0) as usize;
        let mut r = row().key("wrap").wrap().gap(3.0 + rnd(&mut s) * 9.0);
        let words = ["Alpha", "beta", "Gamma", "delta", "iiii", "WWW", "x", "mm", "Tj", "dolly", "e"];
        for i in 0..n {
            r = r.child(if rnd(&mut s) < 0.3 { space(4.0 + rnd(&mut s) * 40.0, 10.0).fill(Paint::FAINT).key(format!("w{i}")) }
                else { text(words[(rnd(&mut s) * 11.0) as usize % 11], 11.0 + rnd(&mut s) * 7.3, Paint::TEXT).tracking(rnd(&mut s) * 0.05).key(format!("w{i}")) });
        }
        r
    }
    pub fn view(s: &S) -> El<()> {
        let mut c = col().key("box").child(line(s.seed));
        if let Some(w) = s.less { c = c.width(w); }
        col().key("page").fill_width().fill_height().align(taffy::AlignItems::START).child(c.zoom(s.zoom))
    }
}

#[test]
fn a_row_in_a_box_as_wide_as_its_content_stays_on_one_line() {
    let lines = |ui: &amenui::ui::Ui<wrapfit::S, ()>| {
        let mut ys: Vec<f32> = ui.nodes().iter().filter(|n| n.key.len() > 1 && n.key.starts_with('w') && n.key[1..].bytes().all(|c| c.is_ascii_digit())).map(|n| n.rect.y).collect();
        ys.sort_by(f32::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        ys.len()
    };
    for seed in 0..200u64 {
        for scale in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
            for zoom in [1.0f32, 0.75, 1.5] {
                let mk = |less| { let mut ui = amenui::ui::Ui::new(wrapfit::S { seed, zoom, less }, wrapfit::view, |_, _| {}, fonts(), (2400.0 * scale) as u32, (300.0 * scale) as u32, scale); ui.tick(0.1); ui };
                let ui = mk(None);
                let (b, r) = (ui.node("box").unwrap().rect, ui.node("wrap").unwrap().rect);
                let one = ui.node("w0").unwrap().rect.h.max(1.0);
                assert_eq!(lines(&ui), 1, "seed {seed} scale {scale} zoom {zoom}: the row wraps in a box {b:?} as wide as it");
                assert!(r.h <= b.h + 0.01 && r.h <= one.max(r.h), "seed {seed} scale {scale} zoom {zoom}: the row {r:?} is taller than its box {b:?}");
                if scale == 1.0 && zoom == 1.0 {
                    // One px narrower than the content: it wraps as it should.
                    let ui = mk(Some(b.w - 1.0));
                    assert_eq!(lines(&ui), 2, "seed {seed}: a box 1 px too narrow keeps the row on one line");
                }
            }
        }
    }
}

/// A toast pinned by two of its four insets inside a card, the card `depth` levels down, in a scroll
/// node or not, zoomed by `zoom`, the whole moved by `dx`.
mod pinned {
    use amenui::el::{El, Paint, col, text};
    pub struct S { pub pair: usize, pub depth: usize, pub scroll: bool, pub zoom: f32, pub dx: f32 }
    pub const PAIRS: [[bool; 4]; 6] = [[true, true, false, false], [true, false, true, false], [true, false, false, true], [false, true, true, false], [false, true, false, true], [false, false, true, true]];
    pub fn view(s: &S) -> El<()> {
        let p = PAIRS[s.pair];
        let v = |on: bool| on.then_some(12.0);
        let mut inner = col().key("card").width(320.0).height(200.0).pad(14.0, 10.0).fill(Paint::FAINT)
            .child(text("Card", 14.0, Paint::TEXT))
            .child(text("Saved", 13.0, Paint::TEXT).key("toast").absolute(v(p[0]), v(p[1]), v(p[2]), v(p[3])));
        for d in 1..s.depth { inner = col().key(format!("wrap{d}")).pad(10.0, 10.0).child(inner); }
        let inner = inner.zoom(s.zoom);
        let mut page = col().key("page").pad(60.0 + s.dx, 40.0).fill_width().fill_height();
        page = if s.scroll { page.child(col().key("scroller").scroll().height(500.0).child(inner)) } else { page.child(inner) };
        page
    }
}

#[test]
fn an_absolute_element_is_pinned_by_all_four_insets_to_its_parent() {
    let mk = |s: pinned::S| { let mut ui = amenui::ui::Ui::new(s, pinned::view, |_, _| {}, fonts(), 900, 700, 1.0); ui.tick(0.1); ui };
    for pair in 0..6 { for depth in 1..=3 { for scroll in [false, true] { for zoom in [1.0f32, 0.75, 1.5] {
        let ui = mk(pinned::S { pair, depth, scroll, zoom, dx: 0.0 });
        // The padding box: the card has no border, so its rect.
        let (c, t) = (ui.node("card").unwrap().rect, ui.node("toast").unwrap().rect);
        let want = 12.0 * zoom;
        let p = pinned::PAIRS[pair];
        let got = [t.x - c.x, t.y - c.y, c.right() - t.right(), c.bottom() - t.bottom()];
        for k in 0..4 { if p[k] { assert!((got[k] - want).abs() < 0.5, "pair {pair} depth {depth} scroll {scroll} zoom {zoom}: edge {k} is {} from the card's padding box, want {want}", got[k]); } }
        let moved = mk(pinned::S { pair, depth, scroll, zoom, dx: 100.0 });
        let t2 = moved.node("toast").unwrap().rect;
        assert!((t2.x - t.x - 100.0).abs() < 0.5 && (t2.y - t.y).abs() < 0.5, "pair {pair}: the card moved 100 px, the toast {}", t2.x - t.x);
    } } } }
}

#[test]
fn a_box_whose_only_child_was_absolute_lays_out_once_the_child_goes() {
    use amenui::el::{El, Paint, col, space};
    fn view(s: &(u8, bool)) -> El<()> {
        let mut b = col().key("board").fill_width().fill_height();
        if s.1 {
            let d = space(28.0, 10.0).fill(Paint::INK).key("dab");
            b = b.child(if s.0 == 0 { d.absolute(Some(5.0), Some(5.0), None, None) } else { d.radius(5.0).absolute(None, None, Some(5.0), Some(5.0)) });
        }
        col().key("page").fill_width().fill_height().child(b)
    }
    for k in 0..2u8 {
        let mut ui = amenui::ui::Ui::new((k, true), view, |_, _| {}, fonts(), 200, 100, 1.0);
        ui.tick(1.0);
        assert!(ui.node("dab").is_some());
        ui.state.1 = false;
        ui.relayout();
        ui.tick(1.1);
        assert!(ui.node("dab").is_none() && ui.node("board").is_some());
    }
}

/// A box of `w` × `h` with one of three subtrees fitted into it (`El::fit`), or the same subtree
/// zoomed by hand by `hand` (scale, left, top).
mod fitted {
    use amenui::el::{El, Fit, Paint, col, row, space, text};
    pub struct S { pub w: f32, pub h: f32, pub which: usize, pub hand: Option<(f32, f32, f32)>, pub max: f32 }
    pub fn subtree(which: usize) -> El<()> {
        match which {
            // A stage of a fixed size.
            0 => col().key("stage").width(1440.0).height(900.0).pad(60.0, 50.0).gap(24.0).fill(Paint::FAINT).radius(18.0)
                .child(text("A stage at its own size", 64.0, Paint::INK).key("stage/t"))
                .child(row().gap(20.0).child(space(300.0, 200.0).fill(Paint::ACCENT).radius(12.0)).child(space(200.0, 200.0).fill(Paint::INK).shift(12.0, -8.0))),
            // A column as large as its content.
            1 => col().key("stage").pad(16.0, 12.0).gap(8.0).fill(Paint::FAINT)
                .child(text("Fitted by its content", 28.0, Paint::INK)).child(text("A second line of body text, longer than the first", 15.0, Paint::TEXT)),
            // A wide, short strip.
            _ => row().key("stage").width(900.0).height(120.0).gap(10.0).child(space(400.0, 120.0).fill(Paint::ACCENT)).child(text("strip", 40.0, Paint::INK)),
        }
    }
    pub fn view(s: &S) -> El<()> {
        let b = col().key("box").width(s.w).height(s.h).border(1.0, Paint::DIM);
        let b = match s.hand {
            None => b.fit(Fit { max: s.max, ..Fit::default() }).child(subtree(s.which)),
            Some((k, x, y)) => b.clip().child(subtree(s.which).zoom(k).absolute(Some(x), Some(y), None, None)),
        };
        col().key("page").fill_width().fill_height().pad(10.0, 10.0).child(b)
    }
}

#[test]
fn a_fitted_subtree_fills_its_box_in_two_layouts_and_grows_with_it() {
    use fitted::{S, view};
    let mk = |w: f32, h: f32, which: usize, hand: Option<(f32, f32, f32)>, max: f32| { let mut ui = amenui::ui::Ui::new(S { w, h, which, hand, max }, view, |_, _| {}, fonts(), 1500, 960, 1.0); ui.tick(1.0); ui };
    let sizes: Vec<(f32, f32)> = (0..20).map(|i| (160.0 + 1280.0 * i as f32 / 19.0, 120.0 + 780.0 * ((i * 7) % 20) as f32 / 19.0)).collect();
    for which in 0..3 {
        for &(w, h) in &sizes {
            let mut ui = mk(w.round(), h.round(), which, None, 8.0);
            let (b, c) = (ui.node("box").unwrap().inner, ui.node("stage").unwrap().rect);
            let k = ui.fitted("box").expect("a scale");
            // At `min` it is drawn at `min` and clipped (the audit reports it).
            // At `min` it is drawn at `min`; what it then cuts (and only that) the audit reports.
            if k <= 0.25 {
                let cut = c.right() > b.right() + 0.5 || c.bottom() > b.bottom() + 0.5;
                assert_eq!(ui.problems().iter().any(|p| matches!(p, amenui::error::ViewProblem::FitShort { .. })), cut, "{which} in {w} x {h}: cut {cut}, reported otherwise");
                continue;
            }
            assert!(c.x >= b.x - 0.5 && c.y >= b.y - 0.5 && c.right() <= b.right() + 0.5 && c.bottom() <= b.bottom() + 0.5, "{which} in {w} x {h}: {c:?} is outside {b:?} (k {k})");
            let fill = (c.w / b.w).max(c.h / b.h);
            { assert!(fill >= 0.97, "{which} in {w} x {h}: fills {fill:.3}"); }
            // One more layout of the same state lays out twice at most.
            let before = ui.counters().layouts;
            ui.relayout();
            assert!(ui.counters().layouts - before <= 2, "{which}: {} layouts", ui.counters().layouts - before);
        }
        // A wider box never gets a smaller scale.
        let mut last = 0.0f32;
        for w in (160..=1440).step_by(7) {
            let k = mk(w as f32, 600.0, which, None, 1.0).fitted("box").unwrap();
            assert!(k >= last - 1e-6, "{which}: at {w} px k {k} after {last}");
            last = k;
        }
    }
    // The same as zoomed by hand.
    let Ok(mut o) = Offscreen::try_new(1500, 960) else { eprintln!("no GPU adapter here: skipped"); return };
    let look = amenui::style::Look { material: amenui::style::Material::Dottr, ..amenui::style::Look::default() };
    for which in 0..3 { for &(w, h) in &[(420.0f32, 300.0f32), (900.0, 500.0), (1300.0, 880.0)] {
        let mut a = mk(w, h, which, None, 1.0);
        let k = a.fitted("box").unwrap();
        let (b, c) = (a.node("box").unwrap().rect, a.node("stage").unwrap().rect);
        // The box's border is 1 px: insets are from inside it.
        let mut hand = mk(w, h, which, Some((k, c.x - b.x - 1.0, c.y - b.y - 1.0)), 1.0);
        assert!(o.render(&mut a, &look) == o.render(&mut hand, &look), "{which} in {w} x {h}: fitted and zoomed by hand by {k} differ");
    } }
}

mod fitstage {
    use amenui::el::{El, Fit, Font, Paint, col, row, space, text};
    pub struct S { pub w: f32, pub h: f32, pub pixel: bool }
    fn stage(pixel: bool) -> El<()> {
        let t = text("A stage at its own size", 64.0, Paint::INK).key("title");
        let t = if pixel { t.font(Font::Face(amenui::text::PIXEL)) } else { t };
        col().key("stage").width(1440.0).height(900.0).pad(60.0, 50.0).gap(24.0).fill(Paint::FAINT).child(t)
            .child(row().gap(20.0).child(space(300.0, 200.0).fill(Paint::ACCENT)).child(space(200.0, 200.0).fill(Paint::INK)))
    }
    pub fn view(s: &S) -> El<()> {
        col().key("page").fill_width().fill_height().pad(10.0, 10.0).child(col().key("box").width(s.w).height(s.h).border(1.0, Paint::DIM).fit(Fit::default()).child(stage(s.pixel)))
    }
}

#[test]
fn a_fit_keeps_its_scale_with_pixel_type_starts_what_overflows_and_reports_only_what_is_cut() {
    use fitstage::{S, view};
    let mk = |w: f32, h: f32, pixel: bool| { let mut ui = amenui::ui::Ui::new(S { w, h, pixel }, view, |_, _| {}, fonts(), 900, 700, 1.0); ui.tick(1.0); ui };
    let short = |ui: &amenui::ui::Ui<S, ()>| ui.problems().iter().any(|p| matches!(p, amenui::error::ViewProblem::FitShort { .. }));
    // Pixel type: the scale is the one without it, the stage fills the box, only the type is smaller.
    let (plain, pixel) = (mk(420.0, 280.0, false), mk(420.0, 280.0, true));
    let (kp, kx) = (plain.fitted("box").unwrap(), pixel.fitted("box").unwrap());
    assert!((kp - kx).abs() < 1e-4, "pixel type changed the scale: {kx} for {kp}");
    let (b, c, t) = (pixel.node("box").unwrap().inner, pixel.node("stage").unwrap().rect, pixel.node("title").unwrap().rect);
    assert!(c.w >= 0.97 * b.w, "the stage is {} of the box's {}", c.w, b.w);
    assert!(t.right() <= c.right() + 0.5, "the pixel title {t:?} overflows the stage {c:?}");
    // At the least scale, a direction that overflows starts at the box's start; the other follows align.
    for (w, h) in [(250.0f32, 250.0f32), (200.0, 120.0), (480.0, 160.0)] {
        let ui = mk(w, h, false);
        let (b, c) = (ui.node("box").unwrap().inner, ui.node("stage").unwrap().rect);
        assert!(short(&ui), "{w} x {h}: cut and not reported");
        if c.w > b.w + 0.5 { assert!((c.x - b.x).abs() < 0.5, "{w} x {h}: overflowing across, starts at {} not {}", c.x, b.x); }
        else { assert!((c.x - (b.x + 0.5 * (b.w - c.w))).abs() < 1.0, "{w} x {h}: centred across"); }
        if c.h > b.h + 0.5 { assert!((c.y - b.y).abs() < 0.5, "{w} x {h}: overflowing down, starts at {} not {}", c.y, b.y); }
        else { assert!((c.y - (b.y + 0.5 * (b.h - c.h))).abs() < 1.0, "{w} x {h}: centred down"); }
    }
    // Exactly at the least scale and in the box (360 wide inside its 1 px border): not cut, not reported.
    let ui = mk(362.0, 300.0, false);
    let (b, c) = (ui.node("box").unwrap().inner, ui.node("stage").unwrap().rect);
    assert!(c.right() <= b.right() + 0.5 && c.bottom() <= b.bottom() + 0.5, "{c:?} in {b:?}");
    assert!(!short(&ui), "362 x 300 at k {}: in the box and reported", ui.fitted("box").unwrap());
}
