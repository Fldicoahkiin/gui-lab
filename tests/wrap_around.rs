//! Text flowing around the zine's sign (DESIGN.md §1.9): at twenty places of the sign, no two glyphs
//! of the paragraph overlap, none is under the sign, the text keeps the same distance from the outline
//! on its left and its right in every band, the pieces beside the sign are justified flush on both
//! edges, and a sub-pixel nudge does not move a line break.
use lab::gallery::film::{self, FMsg};
use lab::gallery::{self, GalleryUi, Msg};

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn open() -> GalleryUi {
    let mut ui = gallery::new_ui(fonts(), 1280, 640, 1.0);
    ui.pointer_leave();
    assert!(ui.state.route("film"));
    if ui.state.film.playing { gallery::update(&mut ui.state, Msg::Film(FMsg::Play)); }
    film::go(&mut ui.state, 6, None);
    ui.relayout();
    for i in 1..=180 { ui.tick(i as f32 / 60.0); ui.frame_drawn(); }
    ui
}
/// The glyph boxes of the paragraph, ink only (the spread padding taken off).
fn glyphs(ui: &GalleryUi) -> Vec<[f32; 4]> {
    let p = lab::text::SPREAD as f32 + 0.5;
    ui.instances_of("film/c06/body").iter().map(|i| [i.rect[0] + p, i.rect[1] + p, i.rect[0] + i.rect[2] - p, i.rect[1] + i.rect[3] - p]).filter(|r| r[2] > r[0] && r[3] > r[1]).collect()
}

#[test]
fn the_paragraph_never_overlaps_itself_or_the_sign() {
  for locale in lab::i18n::Locale::ALL {
    let mut ui = open();
    gallery::update(&mut ui.state, Msg::Locale(locale));
    let mut t = 3.0;
    for i in 0..20 {
        let (x, y) = (-60.0 + (i % 5) as f32 * 120.0, -40.0 + (i / 5) as f32 * 110.0);
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(x, y)));
        ui.relayout();
        for _ in 0..30 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let g = glyphs(&ui);
        assert!(g.len() > 100, "position {i}: {} glyphs", g.len());
        for a in 0..g.len() { for b in a + 1..g.len() {
            let (p, q) = (g[a], g[b]);
            let ox = p[2].min(q[2]) - p[0].max(q[0]);
            let oy = p[3].min(q[3]) - p[1].max(q[1]);
            assert!(ox <= 1.0 || oy <= 1.0, "{locale:?} position {i} ({x}, {y}): glyphs {a} {p:?} and {b} {q:?} overlap");
        } }
        let s = ui.node("film/c06/sign").expect("sign").rect;
        let (cx, cy, r) = (s.x + s.w / film::SIGN_W * 0.5, s.y + s.w / film::SIGN_W * 0.5, s.w / film::SIGN_W * 0.5);
        for (j, q) in g.iter().enumerate() {
            let (gx, gy) = ((q[0] + q[2]) * 0.5, (q[1] + q[3]) * 0.5);
            assert!(((gx - cx).powi(2) + (gy - cy).powi(2)).sqrt() > r, "{locale:?} position {i}: glyph {j} under the disc");
        }
    }
  }
}

#[test]
fn a_subpixel_nudge_keeps_every_line_break() {
    let mut ui = open();
    let mut t = 3.0;
    let mut last: Option<Vec<[f32; 4]>> = None;
    for i in 0..12 {
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(200.0 + i as f32 * 0.05, 120.0 + i as f32 * 0.03)));
        ui.relayout();
        for _ in 0..20 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let g = glyphs(&ui);
        if let Some(l) = &last { assert_eq!(l.len(), g.len()); for (a, b) in l.iter().zip(&g) { assert!((a[1] - b[1]).abs() < 0.5 && (a[0] - b[0]).abs() < 2.0, "a glyph hopped at nudge {i}: {a:?} → {b:?}"); } }
        last = Some(g);
    }
}

/// Per line band of the paragraph: the outline's widest extent over the band's full height, and the
/// lines (fragments) on each side of it. (band top, left clearance, right clearance) for every band
/// with text on both sides; the clearance is from the outline to the fragment's set text.
fn clearances(ui: &mut GalleryUi) -> Vec<(f32, f32, f32)> {
    let s = ui.node("film/c06/sign").expect("sign").rect;
    let body = ui.node("film/c06/body").expect("body").inner;
    let d = s.w / film::SIGN_W;
    let sil = film::sign_outline(s.x - body.x, s.y - body.y, d);
    let lines = ui.line_boxes("film/c06/body");
    assert!(lines.len() > 4, "{} lines", lines.len());
    let mut out = vec![];
    let mut seen = vec![];
    for l in &lines {
        if seen.iter().any(|&y: &f32| (y - l.y).abs() < 0.5) { continue; }
        seen.push(l.y);
        let Some((a, b)) = sil.span(l.y - body.y, l.bottom() - body.y) else { continue };
        let (a, b) = (a + body.x, b + body.x);
        let band: Vec<_> = lines.iter().filter(|m| (m.y - l.y).abs() < 0.5 && m.w > 0.0).collect();
        let left = band.iter().filter(|m| m.right() <= a + 0.5).map(|m| m.right()).fold(f32::NEG_INFINITY, f32::max);
        let right = band.iter().filter(|m| m.x >= b - 0.5).map(|m| m.x).fold(f32::INFINITY, f32::min);
        for m in &band { assert!(m.right() <= a + 0.5 || m.x >= b - 0.5, "a line at y {} ({}..{}) runs over the outline {a}..{b}", m.y, m.x, m.right()); }
        if left.is_finite() && right.is_finite() { out.push((l.y, a - left, right - b)); }
    }
    out
}

#[test]
fn the_clearance_is_the_same_on_both_sides() {
  for locale in lab::i18n::Locale::ALL {
    let mut ui = open();
    gallery::update(&mut ui.state, Msg::Locale(locale));
    let mut t = 3.0;
    let mut checked = 0;
    for i in 0..20 {
        let (x, y) = (40.0 + (i % 5) as f32 * 60.0, -30.0 + (i / 5) as f32 * 70.0);
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(x, y)));
        ui.relayout();
        for _ in 0..30 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let (w, h) = ui.logical_size(); let gap = ui.state.film.gap * film::geo(w, h).k;
        for (y0, l, r) in clearances(&mut ui) {
            // A left piece justification could not fill within its cap is start-aligned: its gap is
            // wider (counted, and bounded, in `pieces_beside_the_sign_are_flush_on_both_edges`).
            if l > gap + crate_step() + 0.5 { continue; }
            checked += 1;
            assert!((l - r).abs() <= 2.0, "position {i} ({x}, {y}), band at {y0:.0}: {l:.1} px from the outline on the left, {r:.1} on the right");
            // On the wrap's grid the obstacle sits up to a step from the drawn sign.
            assert!((r - gap).abs() <= crate_step() + 0.5, "position {i}, band at {y0:.0}: the right piece starts {r:.1} px from the outline, gap {gap:.1}");
        }
    }
    assert!(checked > 20, "{locale:?}: {checked} bands with text on both sides and both pieces justified");
  }
}

/// Every piece beside the sign but the paragraph's last line is justified: its outer edge on the
/// column's edge (within 1 px), its inner edge the gap from the outline (within a grid step). A piece
/// justification could not fill within `Around::justify_max` stays start-aligned — flush on its outer
/// side for a left piece, at the gap for a right one — and at most 15 % of pieces are like that, in
/// both languages. English stretches its spaces up to a quarter em, then its letters up to
/// `wrap::TRACK_MAX_EM`, and a piece that still falls short takes part of the next word with a hyphen
/// (`Around::hyphenate`, Liang's en-US patterns).
#[test]
fn pieces_beside_the_sign_are_flush_on_both_edges() {
  for locale in lab::i18n::Locale::ALL {
    let mut ui = open();
    gallery::update(&mut ui.state, Msg::Locale(locale));
    let mut t = 3.0;
    let (mut pieces, mut limited) = (0, 0);
    for i in 0..20 {
        let (x, y) = (-60.0 + (i % 5) as f32 * 120.0, -40.0 + (i / 5) as f32 * 110.0);
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(x, y)));
        ui.relayout();
        for _ in 0..30 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        let (w, h) = ui.logical_size(); let gap = ui.state.film.gap * film::geo(w, h).k;
        let s = ui.node("film/c06/sign").expect("sign").rect;
        let body = ui.node("film/c06/body").expect("body").inner;
        let sil = film::sign_outline(s.x - body.x, s.y - body.y, s.w / film::SIGN_W);
        let lines = ui.line_boxes("film/c06/body");
        let step = crate_step() + 0.5;
        for (k, m) in lines.iter().enumerate() {
            if k + 1 == lines.len() || m.w <= 0.0 { continue; }
            let Some((a, b)) = sil.span(m.y - body.y, m.bottom() - body.y) else { continue };
            let (a, b) = (a + body.x, b + body.x);
            let (outer, inner, left) = if m.right() <= a + 0.5 { (m.x - body.x, a - m.right(), true) }
                else if m.x >= b - 0.5 { (body.x + body.w - m.right(), m.x - b, false) }
                else { continue };
            pieces += 1;
            let at = format!("{locale:?} position {i} ({x}, {y}), {} piece at y {:.0} ({:.1}..{:.1})", if left { "left" } else { "right" }, m.y, m.x, m.right());
            if outer.abs() <= 1.0 && (inner - gap).abs() <= step { continue; }
            // Start-aligned: the start edge is where it belongs, the other is short of it.
            let (start, end) = if left { (outer, inner - gap) } else { (inner - gap, outer) };
            assert!(start.abs() <= if left { 1.0 } else { step } && end > 0.0, "{at}: {outer:.1} px from the column's edge, {inner:.1} from the outline (gap {gap:.1})");
            limited += 1;
        }
    }
    assert!(pieces > 40, "{locale:?}: {pieces} pieces beside the sign");
    let most = 15;
    assert!(limited * 100 <= pieces * most, "{locale:?}: {limited} of {pieces} pieces beside the sign are start-aligned");
    eprintln!("{locale:?}: {limited} of {pieces} pieces beside the sign start-aligned");
  }
}

fn crate_step() -> f32 { lab::wrap::STEP }

#[test]
fn english_pieces_are_hyphenated_where_the_patterns_allow() {
    assert_eq!(lab::wrap::hyphen_points("hyphenation"), vec![2, 6, 7]); // hy-phen-a-tion
    assert!(lab::wrap::hyphen_points("the").is_empty() && lab::wrap::hyphen_points("naïve").is_empty());
    assert!(!lab::wrap::hyphenated_at("out-line", 3) && lab::wrap::hyphenated_at("outline", 3));
    // Somewhere in the twenty positions a line beside the sign ends in a hyphen, drawn as a glyph.
    let mut ui = open();
    gallery::update(&mut ui.state, Msg::Locale(lab::i18n::Locale::En));
    let mut t = 3.0;
    let mut found = 0;
    for i in 0..20 {
        let (x, y) = (-60.0 + (i % 5) as f32 * 120.0, -40.0 + (i / 5) as f32 * 110.0);
        gallery::update(&mut ui.state, Msg::Film(FMsg::Obj(x, y)));
        ui.relayout();
        for _ in 0..30 { t += 1.0 / 60.0; ui.tick(t); ui.frame_drawn(); }
        found += ui.hyphenated_lines("film/c06/body");
    }
    assert!(found > 0, "no hyphenated line in twenty positions");
}
