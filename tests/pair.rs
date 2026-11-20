//! Distances between glyph outlines (`pair`, DESIGN.md §1.6.6), checked against the coverage the text
//! system rasters: set `target` px apart at their closest by `pair::spaced`, the letters' ink pixels are
//! that far apart to within a pixel, and set by `pair::spaced_area` they hold that white between them.
use amenui::el::Font;
use amenui::pair::{area, gap, spaced, spaced_area};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Default)]
struct S { faces: Option<amenui::pair::Faces> }

/// The ink pixels (coverage at least half) of `ch` at `px`, from the pen at `x0`, as pixel centres
/// (x right, y down from the baseline).
fn ink(u: &mut Ui<S, ()>, ch: char, px: f32, x0: f32) -> Vec<(f32, f32)> {
    let r = u.text_mut().raster(0, ch, px).expect("raster");
    let mut v = vec![];
    for y in 0..r.h { for x in 0..r.w {
        if r.data[(y * r.w + x) as usize] >= 128 { v.push((x0 + r.left as f32 + x as f32 + 0.5, -r.top as f32 + y as f32 + 0.5)); }
    } }
    v
}

#[test]
fn a_pair_set_by_its_gap_is_that_far_apart_in_its_coverage() {
    let mut u: Ui<S, ()> = Ui::new(S::default(), |_| amenui::el::col(), |_, _| {}, fonts(), 400, 300, 1.0).on_faces(|s, f| s.faces = Some(f));
    let faces = u.state.faces.clone().expect("the faces were handed over");
    let px = 176.0;
    let word: Vec<char> = "Voyage".chars().collect();
    for target in [0.0f32, 8.0, 24.0] {
        for w in word.windows(2) {
            let (a, b) = (faces.glyph(Font::Face(0), w[0], px).expect("a"), faces.glyph(Font::Face(0), w[1], px).expect("b"));
            let dx = spaced(&a, &b, target);
            assert!((gap(&a, &b, dx) - target).abs() < 0.05, "{}{}: the gap at its place is {}", w[0], w[1], gap(&a, &b, dx));
            // The coverage: the nearest ink pixels of the two, centre to centre.
            let (ia, ib) = (ink(&mut u, w[0], px, 0.0), ink(&mut u, w[1], px, dx));
            let near = ia.iter().flat_map(|p| ib.iter().map(move |q| (p.0 - q.0).hypot(p.1 - q.1))).fold(f32::INFINITY, f32::min);
            // Pixel centres are up to a pixel further than the edges, and touching pixels are 1 apart.
            assert!(near >= target - 1.0 && near <= target + 1.5, "{}{} at {target}: the ink is {near} apart", w[0], w[1]);
        }
    }
    // By the white between them.
    let (a, b) = (faces.glyph(Font::Face(0), 'o', px).unwrap(), faces.glyph(Font::Face(0), 'y', px).unwrap());
    let dx = spaced_area(&a, &b, 1240.0);
    assert!((area(&a, &b, dx) - 1240.0).abs() < 40.0, "{}", area(&a, &b, dx));
    assert!(gap(&a, &b, 0.0) < 0.0, "drawn over each other they overlap");
    // Nine pairs measured in a frame's time.
    let t0 = std::time::Instant::now();
    let mut sum = 0.0;
    for k in 0..9 { sum += gap(&a, &b, dx + k as f32); }
    eprintln!("nine pair distances: {:.3} ms (debug build) {sum}", t0.elapsed().as_secs_f64() * 1000.0);
}

/// A role resolves in `Faces` as the page's root resolves it (its `El::fonts`), and follows it.
#[test]
fn faces_resolve_roles_as_the_page_root() {
    use amenui::text::{MONO, SANS};
    #[derive(Default)]
    struct R { faces: Option<amenui::pair::Faces>, mono: bool }
    let view = |s: &R| -> amenui::el::El<()> { let c = amenui::el::col().child(amenui::el::text("Kern", 20.0, amenui::el::Paint::TEXT)); if s.mono { c.fonts([MONO, MONO, MONO]) } else { c } };
    let mut u: Ui<R, ()> = Ui::new(R { faces: None, mono: true }, view, |_, _| {}, fonts(), 200, 100, 1.0).on_faces(|s, f| s.faces = Some(f));
    let f = u.state.faces.clone().expect("faces");
    assert_eq!(f.face_of(Font::Display), MONO, "the page sets display to mono");
    u.state.mono = false;
    u.relayout();
    assert_eq!(f.face_of(Font::Display), SANS, "the faces kept follow the root");
}

/// A pose is told each glyph's pen as laid out, kerning and all: moving each glyph by `target − pen`
/// puts its pen at `target`. In Noto Sans SC, `AVATAR` is kerned.
mod pens {
    use std::cell::RefCell;
    use std::rc::Rc;
    pub struct P { pub shift: f32, pub posed: bool, pub seen: Rc<RefCell<Vec<amenui::el::GlyphAt>>> }
    pub fn view(s: &P) -> amenui::el::El<()> {
        let t = amenui::el::text("AVATAR", 48.0, amenui::el::Paint::TEXT).font(amenui::el::Font::Face(amenui::text::SANS)).key("w");
        let (shift, rec) = (s.shift, s.seen.clone());
        let t = if s.posed { t.pose(move |g| { rec.borrow_mut().push(g); amenui::el::Pose { dx: (g.pen + shift) - g.pen, ..Default::default() } }) } else { t };
        amenui::el::col().pad(20.0, 20.0).child(t)
    }
}

#[test]
fn posing_by_the_pen_puts_each_glyph_where_its_pen_is_set() {
    let faces = || { let u: Ui<S, ()> = Ui::new(S::default(), |_| amenui::el::col(), |_, _| {}, fonts(), 100, 100, 1.0); u.faces() };
    for (shift, scale) in [(0.0f32, 1.0f32), (0.3, 1.0), (0.0, 2.0), (0.37, 2.0)] {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
        let inst = |posed: bool| {
            let mut u: Ui<pens::P, ()> = Ui::new(pens::P { shift, posed, seen: seen.clone() }, pens::view, |_, _| {}, fonts(), (300.0 * scale) as u32, (120.0 * scale) as u32, scale);
            u.tick(1.0);
            let i = u.nodes().iter().position(|n| n.key == "w").expect("text");
            let mut v: Vec<[f32; 4]> = u.owners().iter().zip(&u.scene().instances).filter(|(o, _)| o.0 == i).map(|(_, r)| r.rect).collect();
            v.sort_by(|a, b| a[0].total_cmp(&b[0]));
            v
        };
        let (plain, posed) = (inst(false), inst(true));
        assert_eq!(plain.len(), 6);
        assert_eq!(posed.len(), 6);
        for (a, b) in plain.iter().zip(&posed) {
            assert!((b[0] - a[0] - shift * scale).abs() < 0.5 && (b[1] - a[1]).abs() < 0.5, "shift {shift} at {scale}x: {a:?} / {b:?}");
        }
        let g = seen.borrow();
        let g: Vec<_> = g.iter().take(6).collect();
        // The pens are as laid out: kerned (A V closer than A's advance), in order, on one baseline.
        // The advance is the one laid out, kerning in it: A before V takes less than A's own.
        let own = amenui::pair::Faces::clone(&faces()).glyph(Font::Face(amenui::text::SANS), 'A', 48.0).expect("A").advance;
        assert!((g[1].pen - g[0].pen - g[0].advance).abs() < 1e-3, "pens advance by the advance");
        assert!(g[0].advance < own - 0.5, "A V is kerned: {} / {own}", g[0].advance);
        for w in g.windows(2) { assert!(w[1].pen > w[0].pen && w[1].base == w[0].base); }
    }
}

/// A cell made alone (`experiment::ui`, as the Lab's contact sheet makes it) is handed the faces, and in
/// them display is the Lab theme's mono.
#[test]
fn a_cell_made_alone_is_handed_the_faces() {
    #[derive(Clone, Default)]
    struct K { faces: Option<amenui::pair::Faces> }
    impl amenui::experiment::Experiment for K {
        type Msg = ();
        fn view(&self, key: &str) -> amenui::el::El<()> { amenui::el::text("Kern", 20.0, amenui::el::Paint::TEXT).key(key) }
        fn update(&mut self, _: (), _: f32) {}
        fn step(&mut self, _: f32) -> bool { false }
        fn moving(&self) -> bool { false }
        fn faces(&mut self, f: amenui::pair::Faces) { self.faces = Some(f); }
    }
    let u = amenui::experiment::ui::<K>(fonts(), 300, 200, 1.0);
    let f = u.state.e.faces.clone().expect("handed the faces");
    assert_eq!(f.face_of(Font::Display), amenui::style::LAB.display);
    assert!(f.glyph(Font::Display, 'K', 40.0).is_some());
}

/// A text that says where the shaper set its pens (`El::on_pens`), posed or not, and a glyph moved and
/// one turned by a pose (`Ui::glyphs_drawn`).
mod told {
    use std::cell::RefCell;
    use std::rc::Rc;
    #[derive(Clone)]
    pub struct T { pub word: &'static str, pub posed: bool, pub told: Vec<Rc<Vec<f32>>>, pub seen: Rc<RefCell<Vec<f32>>> }
    pub fn view(s: &T) -> amenui::el::El<Rc<Vec<f32>>> {
        let t = amenui::el::text(s.word, 48.0, amenui::el::Paint::INK).font(amenui::el::Font::Face(amenui::text::SANS)).key("w").on_pens(Some);
        let rec = s.seen.clone();
        let t = if s.posed {
            t.pose(move |g| {
                if g.i == 0 { rec.borrow_mut().clear(); }
                rec.borrow_mut().push(g.pen);
                amenui::el::Pose { dx: 10.0 * g.i as f32, angle: if g.i == 2 { 0.4 } else { 0.0 }, ..Default::default() }
            })
        } else { t };
        amenui::el::col().pad(20.0, 30.0).child(t)
    }
    pub fn update(s: &mut T, p: Rc<Vec<f32>>) { s.told.push(p); }
}

#[test]
fn a_text_tells_the_state_its_pens_as_the_shaper_set_them_once_each_time_they_change() {
    use told::*;
    let mut u = Ui::new(T { word: "AVATAR", posed: true, told: vec![], seen: Default::default() }, view, update, fonts(), 400, 120, 2.0);
    let mut t = 1.0;
    u.tick(t);
    assert_eq!(u.state.told.len(), 1, "told after the first paint");
    let pens = u.state.told[0].clone();
    assert_eq!(pens.len(), 6);
    // The pens a pose is told: the same numbers, logical px in the content box.
    let seen = u.state.seen.borrow().clone();
    assert!(pens.iter().zip(&seen).all(|(a, b)| (a - b).abs() < 1e-4), "{pens:?} / {seen:?}");
    // Kerned: A to V is less than A's own advance.
    let a = u.faces().glyph(amenui::el::Font::Face(amenui::text::SANS), 'A', 48.0).expect("A").advance;
    assert!(pens[0] == 0.0 && pens[1] < a - 0.5, "A V kerned: {} / {a}", pens[1]);
    // Unposed, the same.
    let mut plain = Ui::new(T { posed: false, ..u.state.clone() }, view, update, fonts(), 400, 120, 2.0);
    plain.tick(t);
    assert_eq!(plain.state.told.last().map(|p| p.as_slice()), Some(pens.as_slice()));
    // Frames that lay out nothing new tell nothing.
    for _ in 0..30 { t += 1.0 / 60.0; u.tick(t); u.frame_drawn(); }
    u.relayout();
    assert_eq!(u.state.told.len(), 1, "told once while they are the same");
    // Other words, other pens: told again.
    u.state.word = "AWAY";
    u.relayout();
    t += 1.0 / 60.0;
    u.tick(t);
    assert_eq!(u.state.told.len(), 2);
    assert_eq!(u.state.told[1].len(), 4);
}

#[test]
fn the_glyphs_drawn_are_where_their_poses_put_them_and_hold_all_their_coverage() {
    use amenui::offscreen::Offscreen;
    use told::*;
    let (w, h) = (400u32, 120u32);
    let Ok(mut o) = Offscreen::try_new(w, h) else { eprintln!("no GPU adapter here: skipped"); return };
    let shot = |o: &mut Offscreen, posed: bool| {
        let mut u = Ui::new(T { word: "AVATAR", posed, told: vec![], seen: Default::default() }, view, update, fonts(), w, h, 1.0)
            .on_look(|_| amenui::style::Look { material: amenui::style::Material::Plain, ..amenui::style::Look::default() });
        u.tick(2.0);
        let l = u.look();
        o.r.partial = false;
        let f = o.render(&mut u, &l);
        (u.glyphs_drawn("w"), f, u.look().theme.ground)
    };
    let (plain, _, _) = shot(&mut o, false);
    let (posed, f, ground) = shot(&mut o, true);
    assert_eq!((plain.len(), posed.len()), (6, 6));
    assert!(Ui::<T, std::rc::Rc<Vec<f32>>>::new(T { word: "A", posed: false, told: vec![], seen: Default::default() }, view, update, fonts(), w, h, 1.0).glyphs_drawn("none").is_empty());
    for (a, b) in plain.iter().zip(&posed) {
        assert_eq!(a.i, b.i);
        assert!((b.alpha - 1.0).abs() < 1e-3);
        // Moved by its pose (the turned one's box is the box round it: about the same centre).
        let (ca, cb) = (a.rect.x + a.rect.w * 0.5, b.rect.x + b.rect.w * 0.5);
        assert!((cb - ca - 10.0 * a.i as f32).abs() < 0.6, "glyph {}: {:?} / {:?}", a.i, a.rect, b.rect);
        if a.i == 2 { assert!(b.rect.w > a.rect.w + 4.0, "turned: the box round it is wider"); }
    }
    // Every pixel of the posed word's coverage is inside some glyph's box.
    let g = ground.map(|v| (v * 255.0).round() as i32);
    let mut inked = 0;
    for y in 0..h { for x in 0..w {
        let i = ((y * w + x) * 4) as usize;
        if (0..3).all(|k| (f[i + k] as i32 - g[k]).abs() <= 1) { continue; }
        inked += 1;
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        assert!(posed.iter().any(|d| px > d.rect.x - 0.5 && px < d.rect.right() + 0.5 && py > d.rect.y - 0.5 && py < d.rect.bottom() + 0.5), "ink at ({x}, {y}) outside every glyph's box");
    } }
    assert!(inked > 300, "the word is drawn: {inked} px");
}
