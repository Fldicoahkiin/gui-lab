//! Blades (`draw::Blade`, DESIGN.md §1.26) and gusts (`Cmd::Gust`, DESIGN.md §1.29): a blade's tip
//! leans by its lean, its foot by a third of it, it keeps its length and bends only above its stiff part; the GPU draws it where its
//! shape says; a gust leans it and its wake sways it back once, and then the map is as it was and asks
//! for nothing; a gust leaves a path's schedule as it was; the same schedule draws the same bytes; a
//! field of 3,000 blades is one draw that costs the runtime nothing while only the map moves.
use amenui::draw::{Blade, Drawing, Head};
use amenui::el::{El, Font, Paint, Scope, canvas, col};
use amenui::fieldmap::Gust;
use amenui::morph::{Cmd, StirAt};
use amenui::offscreen::Offscreen;
use amenui::style::{DARK, Look, Material};
use amenui::ui::Ui;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }
fn deg(r: f32) -> f32 { r.to_degrees() }
fn length(p: &[[f32; 2]]) -> f32 { p.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum() }

#[test]
fn a_blade_leans_by_its_tip_keeps_its_length_and_bends_where_it_is_not_stiff() {
    let dir = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).atan2(a[1] - b[1]);
    for stiff in [0.0, 0.5, 1.0] {
        for a in [0.0f32, 19.0, 38.0, 57.0, 72.0] {
            for sign in [1.0, -1.0] {
                let lean = sign * a.to_radians();
                let at = |s: f32| Blade::at(s, lean, stiff, 80.0);
                let tip = deg(dir(at(0.995), at(1.0)));
                assert!((tip - sign * a).abs() < 1.0, "stiff {stiff}, {a}°: the tip's tangent is {tip}°");
                let foot = deg(dir(at(0.0), at(4.0 / 80.0)));
                assert!((foot - 0.35 * sign * a).abs() < 2.0, "stiff {stiff}, {a}°: the lowest 4 px lean {foot}°");
                if stiff == 1.0 {
                    // The lower 48 px on the line from the root at the root's angle.
                    let (ux, uy) = ((0.35 * lean).sin(), -(0.35 * lean).cos());
                    let off = (0..=48).map(|k| { let p = at(k as f32 / 80.0); (p[0] * uy - p[1] * ux).abs() }).fold(0.0, f32::max);
                    assert!(off < 0.5, "{a}°: the stiff lower 48 px are {off} px off a line");
                }
                let p = Blade::shape(lean, stiff, 80.0, 1.0);
                assert_eq!(p.len(), 11, "an 80 px blade is ten segments");
                let turns: Vec<f32> = p.windows(3).map(|w| deg(dir(w[1], w[2]) - dir(w[0], w[1])).abs()).collect();
                assert!(turns.iter().all(|t| *t <= 12.0), "stiff {stiff}, {a}°: segments turn {turns:?}");
                assert!((length(&p) - 80.0).abs() < 1.6, "stiff {stiff}, {a}°: {} px long", length(&p));
                assert!((deg(dir(p[9], p[10])) - sign * a).abs() < 7.0, "stiff {stiff}, {a}°: the last segment");
            }
        }
    }
    assert_eq!((Blade::segments(60.0), Blade::segments(30.0), Blade::segments(12.0)), (10, 6, 2));
}

const W: u32 = 240;
const H: u32 = 140;
#[derive(Clone, Copy)]
struct One { lean: f32, head: bool }
fn one(s: &One) -> El<()> {
    let mut d = Drawing::new(W as f32, H as f32);
    let head = s.head.then_some(Head { ch: 'l', size: 14.0, font: Font::Mono, paint: Paint::INK });
    d.blade(Blade { root: (120.0, 120.0), len: 80.0, width: 3.0, lean: s.lean.to_radians(), max_lean: 72f32.to_radians(), lean_paint: Paint::ACCENT, head, ..Blade::default() });
    col().key("page").width(W as f32).height(H as f32).child(canvas(d).key("field").width(W as f32))
}
fn shot(o: &mut Offscreen, s: One) -> Vec<u8> {
    let mut u = Ui::new(s, one, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, theme: DARK, ..Look::default() });
    u.tick(3.0);
    let l = u.look();
    o.r.partial = false;
    o.render(&mut u, &l)
}
/// The pixel the point is in.
fn px(f: &[u8], x: f32, y: f32) -> [i32; 3] { let i = ((y.floor() as u32 * W + x.floor() as u32) * 4) as usize; [f[i] as i32, f[i + 1] as i32, f[i + 2] as i32] }
fn inked(f: &[u8], bare: &[u8], x: f32, y: f32) -> bool { let (a, b) = (px(f, x, y), px(bare, x, y)); (0..3).map(|c| (a[c] - b[c]).abs()).max().unwrap_or(0) > 40 }

#[test]
fn the_gpu_draws_a_blade_where_its_shape_says_with_its_head_on_the_tip() {
    let mut o = Offscreen::new(W, H);
    let bare = { let mut u = Ui::new(One { lean: 0.0, head: false }, |_: &One| -> El<()> { col().key("page").width(W as f32).height(H as f32) }, |_, _| {}, fonts(), W, H, 1.0).on_look(|_| Look { material: Material::Plain, theme: DARK, ..Look::default() }); u.tick(3.0); let l = u.look(); o.r.partial = false; o.render(&mut u, &l) };
    for a in [0.0f32, 19.0, 38.0, 57.0, 72.0, -38.0] {
        let f = shot(&mut o, One { lean: a, head: false });
        if let Ok(d) = std::env::var("DUMP") { std::fs::write(format!("{d}/stem-{a}.png"), amenui::offscreen::png_bytes(W, H, &f)).unwrap(); }
        // On the shape from just above the root to just below the tip; not on its mirror.
        let p: Vec<[f32; 2]> = (1..20).map(|k| Blade::at(k as f32 / 20.0, a.to_radians(), 0.5, 80.0)).collect();
        for q in &p {
            assert!(inked(&f, &bare, 120.0 + q[0], 120.0 + q[1]), "{a}°: no ink at {q:?}");
            if q[0].abs() > 6.0 { assert!(!inked(&f, &bare, 120.0 - q[0], 120.0 + q[1]), "{a}°: ink on the mirror of {q:?}"); }
        }
        // Nothing past the tip along its tangent.
        let t = Blade::at(1.0, a.to_radians(), 0.5, 80.0);
        let (ux, uy) = (a.to_radians().sin(), -a.to_radians().cos());
        assert!(!inked(&f, &bare, 120.0 + t[0] + ux * 4.0, 120.0 + t[1] + uy * 4.0), "{a}°: ink past the tip");
    }
    // The head: its ink starts at the tip and goes on along the tip's tangent, not down the stem; its
    // colour goes from its paint (ink) to the blade's lean paint (the accent) as the stroke's does.
    for a in [0.0f32, 57.0, 72.0] {
        let (f, stem) = (shot(&mut o, One { lean: a, head: true }), shot(&mut o, One { lean: a, head: false }));
        if let Ok(d) = std::env::var("DUMP") { std::fs::write(format!("{d}/head-{a}.png"), amenui::offscreen::png_bytes(W, H, &f)).unwrap(); }
        let t = Blade::at(1.0, a.to_radians(), 0.5, 80.0);
        let (sx, sy) = (a.to_radians().sin(), -a.to_radians().cos());
        let head = |x: f32, y: f32| inked(&f, &stem, x, y);
        let pts: Vec<(f32, f32)> = (2..14).map(|d| (120.0 + t[0] + sx * d as f32, 120.0 + t[1] + sy * d as f32)).filter(|&(x, y)| head(x, y)).collect();
        assert!(pts.len() >= 2, "{a}°: the head's ink along the tip's tangent: {} px", pts.len());
        assert!(!head(120.0 + t[0] - sx * 3.0, 120.0 + t[1] - sy * 3.0), "{a}°: the head runs down the stem");
        // The accent (orange) at a thin stroke's coverage: red well over blue; the ink is white.
        let orange = pts.iter().filter(|&&(x, y)| { let q = px(&f, x, y); q[0] - q[2] > 40 }).count();
        if a == 0.0 { assert_eq!(orange, 0, "upright, the head is not in the lean paint"); }
        if a == 72.0 { assert!(orange * 5 >= pts.len() * 4, "leaning its most, the head is in the lean paint: {orange} of {} px", pts.len()); }
    }
}

/// A field page: blades in rows over the window's lower part, `n` of them, and a gust's scope.
#[derive(Clone)]
struct Field { n: usize, cmds: Vec<Cmd>, life: Option<(f32, f32)>, corner: bool }
const FW: u32 = 640;
const FH: u32 = 200;
fn field(s: &Field) -> El<()> {
    let mut d = Drawing::new(FW as f32, FH as f32);
    let per = 100.max(s.n / 15);
    for k in 0..s.n {
        let (i, j) = (k % per, k / per);
        let x = 3.0 + i as f32 * (FW as f32 - 6.0) / per as f32 + (j % 2) as f32 * 2.0;
        d.blade(Blade { root: (x, 60.0 + j as f32 * 9.0), len: 14.0 + (k * 7 % 11) as f32, width: 0.75 + 0.75 * ((k * 13 % 17) as f32 / 16.0), max_lean: 72f32.to_radians(), paint: Paint::DIM, lean_paint: Paint::INK, ..Blade::default() });
    }
    // One blade alone, rooted on a node of the map.
    if s.n == 1 { d = Drawing::new(FW as f32, FH as f32); d.blade(Blade { root: (256.0, 96.0), len: 40.0, max_lean: 72f32.to_radians(), ..Blade::default() }); }
    let page = col().key("page").width(FW as f32).height(FH as f32).child(canvas(d).key("field").width(FW as f32));
    // A box with lives of its own in the lower right corner, away from everything else.
    let page = if s.corner { page.child(col().key("corner").width(64.0).height(32.0).absolute(Some(FW as f32 - 64.0), Some(FH as f32 - 32.0), None, None).map_life(6.0, 6.0)) } else { page };
    if let Some((e, f)) = s.life { page.map_life(e, f) } else { page }
}
fn open(n: usize, life: bool) -> Ui<Field, ()> { open_field(Field { n, cmds: vec![], life: life.then_some((6.0, 6.0)), corner: false }) }
fn open_field(f: Field) -> Ui<Field, ()> {
    let mut u = Ui::new(f, field, |_, _| {}, fonts(), FW, FH, 1.0)
        .on_look(|_| Look { material: Material::Plain, theme: DARK, ..Look::default() })
        .on_commands(|s| std::mem::take(&mut s.cmds));
    for k in 1..=60 { u.tick(k as f32 / 60.0); u.settle(); u.frame_drawn(); }
    u
}
fn gust(strength: f32) -> Cmd { Cmd::Gust { gust: Gust { strength, ..Gust::default() }, scope: Scope::Global } }
/// The lean the GPU gives the blade rooted at (256, 96): 72° × the map's flow there.
fn lean(u: &Ui<Field, ()>) -> f32 { 72.0 * u.fieldmap().flow_at(256.0, 96.0)[0].clamp(-1.0, 1.0) }

#[test]
fn a_gust_leans_a_blade_and_its_wake_sways_it_back_once() {
    let mut u = open(1, false);
    let before = u.fieldmap().snapshot().clone();
    let t0 = u.now();
    u.command(gust(0.79));
    let mut seen = vec![];
    let mut t = t0;
    while t < t0 + 5.0 {
        t += 1.0 / 60.0;
        u.tick(t);
        u.settle();
        seen.push((t - t0, lean(&u)));
        u.frame_drawn();
    }
    let (tp, peak) = seen.iter().copied().fold((0.0, f32::MIN), |a, b| if b.1 > a.1 { b } else { a });
    eprintln!("peak {peak:.2}° at {tp:.3} s");
    assert!((peak - 57.0).abs() <= 0.5, "the peak: {peak}°");
    // The band is 330 px at 240 px/s: its trailing edge passes 165 px after its middle.
    let trailing = tp + 165.0 / 240.0;
    let (tw, low) = seen.iter().copied().fold((0.0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
    eprintln!("sway back {low:.2}° at {tw:.3} s, trailing edge at {:.3} s", tp + 165.0 / 240.0);
    assert!((low + 6.0).abs() <= 0.5, "the sway back: {low}°");
    assert!(tw > trailing && tw <= trailing + 0.5, "the sway back at {tw} s, the trailing edge at {trailing} s");
    // Once: forward, back, still.
    let mut turns = 0;
    let mut last = 0.0f32;
    for &(_, l) in &seen { if l != 0.0 && (last == 0.0 || l.signum() != last.signum()) { turns += 1; last = l; } }
    assert_eq!(turns, 2, "forward and back once");
    assert!(seen.iter().filter(|s| s.0 >= tp + 1.25).all(|s| s.1 == 0.0), "it is upright 1.25 s after the peak");
    // Gone without a trace.
    assert_eq!(u.fieldmap().gusts(), 0);
    assert_eq!(*u.fieldmap().snapshot(), before, "the map after the gust");
    assert!(!u.needs_frame(t + 0.1), "a still field asks for frames");
    // The strong one reaches the most lean.
    let mut u = open(1, false);
    let t0 = u.now();
    u.command(gust(1.0));
    let mut most = 0.0f32;
    for k in 1..300 { u.tick(t0 + k as f32 / 60.0); most = most.max(lean(&u)); }
    assert!((most - 72.0).abs() <= 0.5, "a strong gust: {most}°");
}

fn path() -> Cmd { Cmd::Stir { at: StirAt::Path { points: std::rc::Rc::new(vec![[40.0, 80.0], [600.0, 120.0]]), from: 0.0, to: 1.0 }, v: [1.0, 0.0], energy: 1.0, radius: 30.0, scope: Scope::Global } }
/// Run a schedule at 60 Hz for `secs`: the map's bytes after every tick.
fn schedule(u: &mut Ui<Field, ()>, secs: f32, at: &[(f32, Cmd)]) -> Vec<std::rc::Rc<Vec<u8>>> {
    let t0 = u.now();
    let mut out = vec![];
    let mut cmds: Vec<(f32, Cmd)> = at.to_vec();
    for k in 1..=(secs * 60.0).round() as usize {
        let t = t0 + k as f32 / 60.0;
        cmds.retain(|(w, c)| if *w <= k as f32 / 60.0 + 1e-4 { u.command(c.clone()); false } else { true });
        u.tick(t);
        u.settle();
        u.frame_drawn();
        out.push(u.fieldmap().snapshot().bytes.clone());
    }
    out
}

#[test]
fn a_gust_leaves_a_six_second_path_on_its_schedule() {
    let mut a = open(0, true);
    let alone = schedule(&mut a, 9.0, &[(0.0, path())]);
    let mut b = open(0, true);
    let fast = Cmd::Gust { gust: Gust { speed: 480.0, ..Gust::default() }, scope: Scope::Global };
    let with = schedule(&mut b, 9.0, &[(0.0, path()), (0.2, fast)]);
    let differs: Vec<usize> = (0..alone.len()).filter(|&k| alone[k] != with[k]).collect();
    assert!(!differs.is_empty(), "the gust shows");
    let last = *differs.last().unwrap_or(&0);
    // The gust crosses 640 px and its band and wake behind it by (640 + 330 + 240) / 480 s.
    assert!((last as f32 / 60.0) < 0.2 + 1210.0 / 480.0 + 0.1, "the map differs until {} s", last as f32 / 60.0);
    let (ca, cb) = (closes(&alone), closes(&with));
    assert_eq!(ca, cb, "the path closes when it would alone");
    assert!((ca - 6.0).abs() <= 0.1, "a path with 6 s lives closes at {ca} s");
    assert!(last as f32 / 60.0 < ca - 1.0, "the path outlives the gust");
}

/// A pointer's walk along y = `y` from x 40 to 600 over half a second, `width` px wide (W-01: `10 + 60 × 0.5`
/// = 40): each 60th of a second the part walked in it.
fn walk_at(y: f32, width: f32) -> Vec<(f32, Cmd)> {
    let line = std::rc::Rc::new(vec![[40.0, y], [600.0, y]]);
    (0..30).map(|k| (k as f32 / 60.0, Cmd::Stir { at: StirAt::Path { points: line.clone(), from: k as f32 / 30.0, to: (k + 1) as f32 / 30.0 }, v: [1.0, 0.0], energy: 1.0, radius: width * 0.5, scope: Scope::Global })).collect()
}
fn walk() -> Vec<(f32, Cmd)> { walk_at(96.0, 40.0) }
/// The flow along x at (x, y) in a frame of the map, read as the GPU does (bilinear between the nodes,
/// 16 px apart, two more columns than fit), as a lean of 72° × it.
fn lean_in(snap: &[u8], x: f32, y: f32) -> f32 {
    let cols = FW as usize / 16 + 2;
    let (qx, qy) = (x / 16.0, y / 16.0);
    let (i, j) = (qx.floor() as usize, qy.floor() as usize);
    let (fx, fy) = (qx - i as f32, qy - j as f32);
    let n = |i: usize, j: usize| snap[(j * cols + i) * 4 + 1] as f32 / 255.0;
    let v = (n(i, j) * (1.0 - fx) + n(i + 1, j) * fx) * (1.0 - fy) + (n(i, j + 1) * (1.0 - fx) + n(i + 1, j + 1) * fx) * fy;
    let f = (v * 255.0 - 128.0) / 127.0;
    if f.abs() < 0.5 / 127.0 { 0.0 } else { 72.0 * f.clamp(-1.0, 1.0) }
}
/// When the last byte of the map goes back to rest, s.
fn closes(frames: &[std::rc::Rc<Vec<u8>>]) -> f32 {
    let busy = |m: &std::rc::Rc<Vec<u8>>| m.chunks_exact(4).any(|p| p[0] != 0 || p[1] != 128 || p[2] != 128);
    frames.iter().rposition(busy).map_or(0.0, |k| (k + 1) as f32 / 60.0)
}

#[test]
fn a_path_with_six_second_lives_is_back_at_rest_at_six_seconds_where_it_was_walked_first_first() {
    let mut u = open(0, true);
    let before = u.fieldmap().snapshot().clone();
    let frames = schedule(&mut u, 8.0, &walk());
    // The last stalk on the path was walked over at 29/60 s; the first at 0.
    let (first, last) = (48.0, 592.0);
    let at = |t: f32| &frames[((t * 60.0).round() as usize).saturating_sub(1)];
    let walked = 29.0 / 60.0;
    // The look's flow curve slowed to end at 6 s: over 3° at 5.0 s (and 2.8° at 5.1 s), then down to 0.
    let l50 = lean_in(at(walked + 5.0), last, 96.0);
    assert!(l50 > 3.0, "5.0 s after the walk the last stalk leans {l50}°");
    let stands = |x: f32| (0..frames.len()).rev().find(|&k| lean_in(&frames[k], x, 96.0) != 0.0).map(|k| (k + 1) as f32 / 60.0).expect("it leans");
    let (sf, sl) = (stands(first), stands(last));
    eprintln!("the first stalk stands at {sf} s, the last at {sl} s; 5.0 s after the walk it leans {l50}°");
    assert!((sl - walked - 6.0).abs() <= 0.1, "the last stalk stands {} s after the walk", sl - walked);
    assert!(sf < sl, "the end walked first stands first: {sf} s, {sl} s");
    // Then the map is as it was and asks for nothing.
    assert_eq!(*u.fieldmap().snapshot(), before, "the map after the path");
    assert!(!u.needs_frame(u.now() + 0.1), "a closed path asks for frames");
    // Outside the box with lives of its own the map is the look's, byte for byte.
    let mut a = open(0, false);
    let mut b = open_field(Field { n: 0, cmds: vec![], life: None, corner: true });
    let (fa, fb) = (schedule(&mut a, 3.0, &walk()), schedule(&mut b, 3.0, &walk()));
    assert!(fa.iter().any(|f| f.chunks_exact(4).any(|p| p[1] != 128)), "the path shows");
    assert!(fa == fb, "a box with lives of its own away from the path changes the map");
}

#[test]
fn paths_of_any_width_close_together_and_keep_their_width() {
    let mut shut = vec![];
    for y in [96.0f32, 104.0, 101.0] {
        for width in [20.0f32, 40.0, 80.0] {
            let mut u = open(0, true);
            let frames = schedule(&mut u, 8.0, &walk_at(y, width));
            let c = closes(&frames);
            // How wide the stalks lean across the path at x = 320 (by half of the most they lean there),
            // while it is fresh and late.
            let across = |t: f32| {
                let f = &frames[(t * 60.0) as usize];
                let l: Vec<f32> = (0..=200).map(|d| lean_in(f, 320.0, y - 100.0 + d as f32 + 0.5)).collect();
                let most = l.iter().copied().fold(0.0, f32::max);
                l.iter().filter(|&&v| v >= most * 0.5 && most > 0.0).count() as f32
            };
            let (w1, w5) = (across(1.0), across(5.0));
            eprintln!("y {y}, {width} px: closes at {c} s; {w1} px wide at 1 s, {w5} px at 5 s");
            assert!((w1 - width).abs() <= 16.0 && (w5 - width).abs() <= 16.0, "y {y}, a {width} px path is {w1} px wide at 1 s, {w5} px at 5 s");
            shut.push(c);
        }
    }
    let (lo, hi) = shut.iter().fold((f32::MAX, f32::MIN), |a, &c| (a.0.min(c), a.1.max(c)));
    assert!(hi - lo <= 0.1, "paths close at {shut:?}");
    assert!((hi - 29.0 / 60.0 - 6.0).abs() <= 0.1, "paths close {} s after the walk", hi - 29.0 / 60.0);
}

#[test]
fn the_same_schedule_draws_the_same_bytes() {
    let run = || {
        let mut u = open(300, true);
        let maps = schedule(&mut u, 2.0, &[(0.0, path()), (0.3, gust(0.79))]);
        let mut o = Offscreen::new(FW, FH);
        let l = u.look();
        o.r.partial = false;
        (maps, o.render(&mut u, &l))
    };
    let a = run();
    for _ in 0..2 { let b = run(); assert!(a.0 == b.0, "the map"); assert!(a.1 == b.1, "the frame"); }
}

#[test]
fn frames_at_thirty_and_sixty_a_second_are_the_same_at_the_same_moment() {
    let shots = |fps: f32| {
        let mut u = open(300, true);
        u.ambient_fps(fps);
        let mut o = Offscreen::new(FW, FH);
        let l = u.look();
        let t0 = u.now();
        u.command(gust(0.79));
        let mut out = vec![];
        let mut asked = 0;
        for k in 1..=120 {
            let t = t0 + k as f32 / 60.0;
            // At 30 the host ticks only every other refresh.
            if fps < 60.0 && k % 2 == 1 { continue; }
            u.tick(t);
            if u.needs_frame(t) {
                asked += 1;
                if k % 2 == 0 { o.r.partial = false; out.push((k, o.render(&mut u, &l))); } else { u.settle(); u.frame_drawn(); }
            } else if k % 2 == 0 { out.push((k, vec![])); }
        }
        (out, asked)
    };
    let (sixty, n60) = shots(60.0);
    let (thirty, n30) = shots(30.0);
    assert!(n60 >= 110 && (55..=62).contains(&n30), "frames asked for: {n60} at 60, {n30} at 30");
    for ((k, a), (_, b)) in sixty.iter().zip(&thirty) {
        if b.is_empty() { continue; }
        assert!(a == b, "the frame at {k} / 60 s");
    }
    assert!(thirty.iter().filter(|s| !s.1.is_empty()).count() >= 55);
    // Input is never paced, nor what it starts until a second after it.
    let mut u = open(300, true);
    u.ambient_fps(30.0);
    let t0 = u.now();
    u.pointer_move(100.0, 100.0);
    u.command(gust(0.79));
    u.tick(t0 + 1.0 / 60.0);
    u.settle();
    u.frame_drawn();
    u.tick(t0 + 2.0 / 60.0);
    assert!(u.needs_frame(t0 + 2.0 / 60.0), "a frame right after input");
    for k in 3..90 { u.tick(t0 + k as f32 / 60.0); u.settle(); u.frame_drawn(); }
    u.tick(t0 + 90.0 / 60.0);
    u.settle();
    u.frame_drawn();
    u.tick(t0 + 91.0 / 60.0);
    assert!(!u.needs_frame(t0 + 91.0 / 60.0), "paced a second after input");
    assert!(u.next_wake(t0 + 91.0 / 60.0).is_some_and(|w| (w - (t0 + 92.0 / 60.0)).abs() < 1e-3), "the next paced frame");
}

#[test]
fn three_thousand_blades_are_one_draw_that_costs_nothing_while_only_the_map_moves() {
    let cost = |n: usize| {
        let mut u = open(n, false);
        let mut o = Offscreen::new(FW, FH);
        let l = u.look();
        o.draw(&mut u, &l);
        if n > 0 {
            let blades = u.scene().instances.iter().filter(|i| i.blade()).count();
            assert!(blades >= 2 * n, "{blades} instances for {n} blades");
            assert_eq!(u.scene().batches.len(), 1, "one draw");
        }
        let c0 = u.counters();
        let t0 = u.now();
        u.command(gust(1.0));
        let mut spent = vec![];
        for k in 1..=90 {
            let t = t0 + k as f32 / 60.0;
            let at = std::time::Instant::now();
            u.tick(t);
            u.settle();
            spent.push(at.elapsed().as_secs_f64() * 1e3);
            if k % 30 == 0 { o.draw(&mut u, &l); } else { u.frame_drawn(); }
        }
        let c1 = u.counters();
        assert!(c1.shaped == c0.shaped && c1.layouts <= c0.layouts + 1 && c1.scenes <= c0.scenes + 1, "{n} blades: only the gust's command laid out and built a scene: {c0:?} → {c1:?}");
        spent.sort_by(f64::total_cmp);
        spent[spent.len() / 2]
    };
    let (none, many) = (cost(0), cost(3000));
    eprintln!("a frame of the map moving: {none:.3} ms with no blades, {many:.3} ms with 3,000");
    assert!(many - none < 0.1, "{many:.3} ms with 3,000 blades, {none:.3} without");
    // Still, nothing is asked for.
    let mut u = open(3000, false);
    let t = u.now() + 0.5;
    u.tick(t);
    assert!(!u.needs_frame(t), "a still field asks for frames");
}
