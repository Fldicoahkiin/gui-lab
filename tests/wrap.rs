//! Text flowing around the true outline of other things (`wrap`), without a GPU: the free pieces of a
//! band, and every inked texel of every glyph at least `gap - 1` px from the obstacle's ink: across
//! its line's band from the obstacle's widest extent over that band, or straight to it above and below.
use amenui::el::{El, Paint, col, space, text_lh};
use amenui::scene::GLYPH;
use amenui::text::{ATLAS, SPREAD};
use amenui::ui::Ui;
use amenui::wrap::{Around, Silhouette};

fn font() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

const TEXT: &str = "文字绕着东西的真实轮廓走，而不是绕着它的方框。每一行只拿到这一条带子里没有被挡住的部分；被挡成两段时，左右各排一行。Text keeps its distance from the ink of whatever is in its way, whatever shape that ink has, line by line, as the obstacle moves. 拖动它，文字就重新排。";

#[test]
fn free_pieces() {
    let a = Around { obstacles: vec![Silhouette::rounded(100.0, 0.0, 50.0, 40.0, 0.0)], gap: 10.0, min: 20.0, ..Around::new(vec![], 0.0) };
    assert_eq!(a.free(300.0, 0.0, 20.0, a.min), vec![(0.0, 90.0), (160.0, 300.0)]);
    // Below the obstacle (and its gap), the whole width.
    assert_eq!(a.free(300.0, 60.0, 80.0, a.min), vec![(0.0, 300.0)]);
    // Within the gap below it, blocked by what is left of the gap at that distance (a disc swept along
    // the outline): 5 px below, sqrt(10² − 5²) to each side.
    let f = a.free(300.0, 45.0, 60.0, a.min);
    let e = (100.0f32 - 25.0).sqrt();
    assert!(f.len() == 2 && (f[0].1 - (100.0 - e)).abs() < 0.01 && (f[1].0 - (150.0 + e)).abs() < 0.01, "{f:?}");
    // A piece narrower than `min` is not used.
    let b = Around { obstacles: vec![Silhouette::rounded(15.0, 0.0, 50.0, 40.0, 0.0)], gap: 0.0, min: 20.0, ..Around::new(vec![], 0.0) };
    assert_eq!(b.free(300.0, 0.0, 20.0, b.min), vec![(65.0, 300.0)]);
}

#[test]
fn circle_rows() {
    let c = Silhouette::circle(50.0, 50.0, 20.0);
    let (a, b) = c.span(49.0, 51.0).expect("middle row");
    assert!((a - 30.0).abs() < 0.01 && (b - 70.0).abs() < 0.01);
    assert!(c.span(0.0, 29.0).is_none());
    assert!(c.covers(50.0, 50.0) && !c.covers(31.0, 33.0));
}

struct S { cx: f32, cy: f32, r: f32, gap: f32 }
fn page(s: &S) -> El<()> {
    let around = Around::new(vec![Silhouette::circle(s.cx, s.cy, s.r)], s.gap);
    col().key("root").pad(20.0, 20.0)
        .child(col().key("col").width(520.0)
            .child(space(s.r * 2.0, s.r * 2.0).radius(s.r).fill(Paint::ACCENT).key("disc").absolute(Some(s.cx - s.r), Some(s.cy - s.r), None, None))
            .child(text_lh(TEXT.repeat(3), 17.0, 28.0, Paint::TEXT).key("body").fill_width().around(around)))
}

/// The smallest distance from an inked glyph texel to the disc's ink, logical px; and how many lines
/// had text on both sides of the disc.
fn nearest(ui: &mut Ui<S, ()>) -> f32 {
    let body = ui.node("body").expect("body").inner;
    let disc = ui.node("disc").expect("disc").rect;
    let (cx, cy, r) = (disc.x + disc.w * 0.5, disc.y + disc.h * 0.5, disc.w * 0.5);
    assert!((body.x - 20.0).abs() < 0.01);
    let glyphs: Vec<_> = ui.instances_of("body").into_iter().filter(|i| i.paint[3] == GLYPH).collect();
    assert!(glyphs.len() > 200, "{} glyphs", glyphs.len());
    let bands = ui.line_boxes("body");
    let px = &ui.text_mut().pixels;
    let mut best = f32::INFINITY;
    for g in glyphs {
        let (ax, ay) = ((g.uv[0] * ATLAS as f32).round() as i64, (g.uv[1] * ATLAS as f32).round() as i64);
        for ty in 0..g.rect[3] as i64 {
            for tx in 0..g.rect[2] as i64 {
                if px[(((ay + ty) * ATLAS as i64 + ax + tx) * 2) as usize] < 128 { continue; }
                let (x, y) = (g.rect[0] + tx as f32 + 0.5, g.rect[1] + ty as f32 + 0.5);
                // Beside the disc, the distance across the line's band to the disc's widest extent
                // over that band; above or below it, the distance to the disc.
                let band = bands.iter().find(|b| y >= b.y && y < b.bottom());
                let dy = band.map(|b| if cy < b.y { b.y - cy } else if cy > b.bottom() { cy - b.bottom() } else { 0.0 });
                let d = match dy { Some(dy) if dy < r => { let w = (r * r - dy * dy).sqrt(); (cx - w - x).max(x - cx - w) } _ => ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r };
                best = best.min(d);
            }
        }
    }
    let _ = SPREAD;
    best
}

#[test]
fn glyphs_keep_the_gap() {
    for (cx, cy, gap) in [(260.0, 200.0, 14.0), (90.0, 140.0, 8.0), (430.0, 330.0, 20.0)] {
        let mut ui = Ui::new(S { cx, cy, r: 90.0, gap }, page, |_, _| {}, font(), 900, 1400, 1.0);
        let d = nearest(&mut ui);
        assert!(d >= gap - 1.0, "disc at {cx},{cy}: a glyph is {d:.1} px from the ink, gap {gap}");
        // Control: with no gap and the obstacle ignored, text runs over the disc.
        let mut ui = Ui::new(S { cx, cy, r: 90.0, gap: -1000.0 }, page, |_, _| {}, font(), 900, 1400, 1.0);
        assert!(nearest(&mut ui) < 0.0, "the control finds text over the disc");
    }
}

#[test]
fn text_on_both_sides_and_moves_with_it() {
    let mut ui = Ui::new(S { cx: 260.0, cy: 200.0, r: 80.0, gap: 12.0 }, page, |_, _| {}, font(), 900, 1400, 1.0);
    let glyphs = ui.instances_of("body");
    let (cx, cy) = (20.0 + 260.0, 20.0 + 200.0);
    let left = glyphs.iter().any(|g| (g.rect[1] - cy).abs() < 20.0 && g.rect[0] + g.rect[2] < cx - 80.0);
    let right = glyphs.iter().any(|g| (g.rect[1] - cy).abs() < 20.0 && g.rect[0] > cx + 80.0);
    assert!(left && right, "a band beside the disc has text on both sides");
    let h0 = ui.node("body").unwrap().rect.h;
    // Moving the obstacle moves the text: another layout.
    ui.state.cx = 120.0;
    ui.relayout();
    let moved = ui.instances_of("body");
    assert_ne!(glyphs.iter().map(|g| g.rect).collect::<Vec<_>>(), moved.iter().map(|g| g.rect).collect::<Vec<_>>());
    assert!(ui.node("body").unwrap().rect.h >= h0 - 28.0);
}

#[test]
fn an_effect_loosens_the_title_and_keeps_the_text_to_read() {
    use amenui::el::{Effect, Response, Scope, wrap_text};
    const DOC: &str = "# Notes kept as they were\n\nA paragraph that was already written, in plain Markdown.\n\n- a list item\n- and another, with `code` in it\n\n```\nfn main() {}\n```\n";
    fn view(_: &()) -> El<()> {
        wrap_text::<()>(DOC).effect(Effect { material: Some(amenui::style::Material::Smudgr), response: Some(Response::Loosen), scope: Some(Scope::Container(None)) }).loosen(0.6)
    }
    let mut ui = Ui::new((), view, |_, _| {}, font(), 720, 480, 1.0);
    ui.tick(1.0);
    let cap = amenui::style::Look::default().legible_cap;
    let t = ui.now();
    assert!((ui.looseness("wrap/h0", t) - 0.6).abs() < 1e-4, "the title is {} loose", ui.looseness("wrap/h0", t));
    for key in ["wrap/b1/text", "wrap/b2/0/t/text", "wrap/b2/1/t/text", "wrap/b3/l0"] {
        let k = ui.looseness(key, t);
        assert!(ui.node(key).is_some(), "no `{key}`");
        assert!(k <= cap + 1e-4, "`{key}` is {k} loose, past the cap {cap}");
    }
}

#[test]
fn an_effect_leaves_the_text_to_read_as_the_material_rests() {
    // Seven materials that loosen, the page loosened to 0.6: in the ink of every element of text to
    // read (legible and held at the cap; 2 px larger), at least 99 % of the pixels are the material's own frame at rest (loosen 0). Text held at
    // the legibility cap keeps the resting look; only the title comes apart.
    use amenui::el::{Effect, Response, Scope, wrap_text};
    use amenui::offscreen::Offscreen;
    use amenui::style::{Look, Material};
    const DOC: &str = "# Notes kept as they were\n\nA paragraph that was already written, in plain Markdown, now drawn by the framework with a material and a field of its own. Nothing in it was changed to take the effect.\n\n- a list item\n- and another, with code in it\n";
    struct P { m: Material, k: f32 }
    fn view(s: &P) -> El<()> {
        wrap_text::<()>(DOC).effect(Effect { material: Some(s.m), response: Some(Response::Loosen), scope: Some(Scope::Container(None)) }).loosen(s.k)
    }
    let (w, h) = (680u32, 330u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let look = Look::default();
    let shot = |o: &mut Offscreen, m: Material, k: f32| {
        let mut ui = Ui::new(P { m, k }, view, |_, _| {}, font(), w, h, 1.0);
        ui.tick(6.0);
        let px = o.render(&mut ui, &look);
        (ui, px)
    };
    let mut worst = (1.0f32, String::new());
    for m in [Material::Dottr, Material::Halftonr, Material::Charr, Material::Smudgr, Material::Carvr, Material::Glitchr, Material::Scribblr] {
        let (_, rest) = shot(&mut o, m, 0.0);
        let (ui, px) = shot(&mut o, m, 0.6);
        let tree = ui.export_tree();
        let (cap, t) = (look.legible_cap, ui.now());
        let mut ink: Vec<(String, [f32; 4])> = vec![];
        for (i, inst) in ui.scene().instances.iter().enumerate() {
            let Some(key) = ui.owner_key(i) else { continue };
            // The text to read: legible and held at the cap (the title loosens as far as it is told).
            if key.is_empty() || !tree.find(key).is_some_and(|n| n.legible) || ui.looseness(key, t) > cap + 1e-4 || inst.paint[2] <= 0.0 { continue; }
            let p = if inst.paint[3] == GLYPH { SPREAD as f32 } else { 0.0 };
            let b = [inst.rect[0] + p, inst.rect[1] + p, inst.rect[0] + inst.rect[2] - p, inst.rect[1] + inst.rect[3] - p];
            match ink.iter_mut().find(|e| e.0 == key) {
                Some(e) => e.1 = [e.1[0].min(b[0]), e.1[1].min(b[1]), e.1[2].max(b[2]), e.1[3].max(b[3])],
                None => ink.push((key.to_owned(), b)),
            }
        }
        assert!(ink.iter().any(|e| e.0 == "wrap/b1/text"), "{ink:?}");
        for (key, b) in &ink {
            let r = [(b[0] - 2.0).floor() as i32, (b[1] - 2.0).floor() as i32, (b[2] + 2.0).ceil() as i32, (b[3] + 2.0).ceil() as i32];
            let (mut same, mut all) = (0usize, 0usize);
            for y in r[1].max(0)..r[3].min(h as i32) {
                for x in r[0].max(0)..r[2].min(w as i32) {
                    let i = ((y as u32 * w + x as u32) * 4) as usize;
                    all += 1;
                    if px[i..i + 4] == rest[i..i + 4] { same += 1; }
                }
            }
            let share = same as f32 / all.max(1) as f32;
            let at = format!("{} `{key}`", m.name());
            if share < worst.0 { worst = (share, at.clone()); }
            assert!(share >= 0.99, "{at}: the same as at rest in {:.2} % of its pixels", share * 100.0);
        }
    }
    eprintln!("the least alike to its rest: {:.2} % ({})", worst.0 * 100.0, worst.1);
}

#[test]
fn controls_in_a_loosened_subtree_keep_their_fill_and_label_as_at_rest() {
    // Controls (buttons, a field, a switch, tabs) under an effect, the subtree loosened to 0.3, 0.45,
    // 0.6 and 1, in the seven materials that loosen: in each control's box (2 px larger) at least 99 %
    // of the pixels are the material's own frame at rest, and the audit finds no P0 or P1.
    use amenui::drive::Driven;
    use amenui::el::{Effect, Response, Scope};
    use amenui::offscreen::Offscreen;
    use amenui::style::Material;
    use amenui::widgets::{TextEdit, Variant, button, button_as, input, switch, tabs};
    // Under an effect, and in a subtree marked decoration as a whole (a specimen of the material).
    struct P { m: Material, k: f32, decoration: bool }
    const KEYS: [&str; 5] = ["send", "save", "name", "live", "views"];
    fn view(s: &P) -> El<()> {
        let root = col().key("root");
        let root = if s.decoration { root.legible(false) } else { root };
        root.pad(24.0, 24.0).gap(16.0)
            .child(text_lh("A heading the effect may take apart", 26.0, 32.0, Paint::INK).key("title").role(amenui::el::Role::Heading))
            .child(amenui::el::row().key("bar").gap(12.0).child(button_as("send", "Send", Variant::Accent, ())).child(button("save", "Save", ())))
            .child(input("name", &TextEdit::new("Ada Lovelace"), "Name", 260.0, 64, |_| ()))
            .child(switch("live", "Live", true, None, ()))
            .child(tabs("views", &["Notes", "Pictures", "Links"], 0, 16.0, 2.0, |_| (), col()))
            .effect(Effect { material: Some(s.m), response: Some(Response::Loosen), scope: Some(Scope::Container(None)) }).loosen(s.k)
    }
    let (w, h) = (520u32, 340u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, m: Material, k: f32, decoration: bool| {
        let mut ui = Ui::new(P { m, k, decoration }, view, |_, _| {}, font(), w, h, 1.0);
        ui.tick(6.0);
        let look = ui.look();
        let px = o.render(&mut ui, &look);
        (ui, px)
    };
    let mut worst = (1.0f32, String::new());
    let mut fails = vec![];
    for (m, decoration) in [Material::Dottr, Material::Halftonr, Material::Charr, Material::Smudgr, Material::Carvr, Material::Glitchr, Material::Scribblr].into_iter().flat_map(|m| [(m, false), (m, true)]) {
        let (_, rest) = shot(&mut o, m, 0.0, decoration);
        for k in [0.3, 0.45, 0.6, 1.0] {
            let (mut ui, px) = shot(&mut o, m, k, decoration);
            for key in KEYS {
                let b = ui.node(key).expect(key).rect;
                let (mut same, mut all) = (0usize, 0usize);
                for y in ((b.y - 2.0).floor() as i32).max(0)..((b.bottom() + 2.0).ceil() as i32).min(h as i32) {
                    for x in ((b.x - 2.0).floor() as i32).max(0)..((b.right() + 2.0).ceil() as i32).min(w as i32) {
                        let i = ((y as u32 * w + x as u32) * 4) as usize;
                        all += 1;
                        if px[i..i + 4] == rest[i..i + 4] { same += 1; }
                    }
                }
                let share = same as f32 / all.max(1) as f32;
                let at = format!("{}{} at {k} `{key}`", m.name(), if decoration { " (decoration)" } else { "" });
                if share < worst.0 { worst = (share, at.clone()); }
                if share < 0.99 { fails.push(format!("{at}: {:.2} %", share * 100.0)); }
            }
            let look = ui.look();
            let found = ui.audit(&mut o, &look, &px);
            for f in found.iter().filter(|f| f.level <= amenui::audit::Level::P1 && f.key != "title") { fails.push(format!("{} at {k}: {:?} {} {:?}", m.name(), f.level, f.key, f.kind)); }
        }
    }
    eprintln!("the least alike to its rest: {:.2} % ({})", worst.0 * 100.0, worst.1);
    assert!(fails.is_empty(), "{fails:#?}");
}
