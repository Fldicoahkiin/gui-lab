//! `scribblr`'s thread (DESIGN.md §1.30), and long paths drawn by progress. No GPU here: the runtime
//! turns what this returns into round-capped bar instances (as `Mark::Line` is drawn), so the thread has a
//! true outline in the first pass like any shape.
//!
//! An element's thread at looseness `k`:
//!  1. its rest path: each glyph's true outline (the 0.5 iso-line of its coverage, sampled `grain` apart
//!     by `morph::cover_at` and simplified to half a pixel), shapes' outlines as they are;
//!  2. joined into one polyline in reading order, a straight bridge from where one loop closes to the
//!     nearest vertex of the next; every vertex ranked by how far it stands from the chord it would be
//!     dropped onto (one Ramer–Douglas–Peucker pass over the whole line, a child never above its parent);
//!  3. up to `k = 0.3` the line lies on the outline; past it the tolerance grows to `0.3 ×` the
//!     element's line height and each vertex below it slides onto its chord and is dropped, so the two
//!     sides of a stroke become one line and then long chords across letters; the vertices left move by
//!     `reach` (a smooth curl, weight 0.4, and a jag along the path, 0.3–1 each) and with the map's flow;
//!  4. one to three knots where the map's energy is highest along the line, the most energetic the
//!     largest and the others smaller by `knot_fall`, each off the line above or below it and moved
//!     clear of text to read: tangles (`tangle`) of one thin line looping through itself dozens of
//!     times, a few strokes escaping past its radius (`escape`); a knot of fewer than `MIN_KNOT`
//!     points is not drawn;
//!  5. two tails: one from the scope's edge into the line's start, one from its end out to an edge,
//!     one long slow curve with `tail_turns` gentle bends and a slight waver of the hand, stopping
//!     before text to read; drawn at `tail_tone` of the line's opacity, so the knots stand out bright
//!     against quiet lines;
//!  6. past `k = 0.3` the line bends toward the curve through its vertices (`curve`), by
//!     `(1 − angular) × smooth(0.3, 0.7, k)`; then any piece that would cross text to read goes round it.
//!
//! Every curve is a uniform cubic B-spline drawn as short straight pieces (`curve`); when the budget is
//! short the pieces turn up to 30° instead of 15°, then the knots lose points, then lose themselves.
//!
//! At `k = 0` there is nothing: no segment exists, so rest is plain's by construction.
#![warn(missing_docs)]
use crate::field::hash;
use crate::scene::Instance;

/// The material's numbers: the eight in `MAT_PARAMS` order (logical px where they are lengths), and the
/// thread's shape (`ThreadShape`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrandFx {
    /// The line's width.
    pub weight: f32,
    /// How finely outlines are sampled.
    pub grain: f32,
    /// How far loose vertices wander.
    pub reach: f32,
    /// How large and dense the knots are (0: no knots).
    pub knot: f32,
    /// The largest knot's radius.
    pub knot_r: f32,
    /// How long the tails are at least.
    pub tail: f32,
    /// How much the map's flow carries the line.
    pub pull: f32,
    /// How sharp its corners stay (1: as they are; less bends the loose line into a curve).
    pub angular: f32,
    /// Each knot after the largest is this much the size of the one before (about: 1, 0.55, 0.35).
    pub knot_fall: f32,
    /// Strokes of each knot that escape to 1.5–2.2 times its radius.
    pub escape: f32,
    /// How many gentle bends (each at most 25°) a tail has.
    pub tail_turns: f32,
    /// The tails' opacity, as a share of the line's.
    pub tail_tone: f32,
}
impl Default for StrandFx {
    fn default() -> Self { Self::of(crate::style::MaterialParams::defaults(crate::style::Material::Scribblr).0) }
}
impl StrandFx {
    /// The material's eight numbers (`MAT_PARAMS` order), with the default shape.
    pub fn of(p: [f32; 8]) -> Self {
        Self { weight: p[0], grain: p[1], reach: p[2], knot: p[3], knot_r: p[4], tail: p[5], pull: p[6], angular: p[7], knot_fall: 0.0, escape: 0.0, tail_turns: 0.0, tail_tone: 0.0 }.shaped(ThreadShape::default())
    }
    /// The same with the thread's shape `sh` (clamped to its ranges).
    pub fn shaped(self, sh: ThreadShape) -> Self {
        let sh = sh.clamped();
        Self { knot_fall: sh.knot_fall, escape: sh.escape, tail_turns: sh.tail_turns, tail_tone: sh.tail_tone, ..self }
    }
}

/// The shape of `scribblr`'s thread beyond the material's eight numbers (`StrandFx::shaped`).
///
/// | name | what | range | step | default |
/// |---|---|---|---|---|
/// | `knot_fall` | each knot after the largest is this much the size of the one before | 0.3–1 | 0.05 | 0.55 |
/// | `escape` | strokes of each knot that escape to 1.5–2.2 times its radius | 0–4 | 1 | 2 |
/// | `tail_turns` | gentle bends in a tail (each at most 25°) | 1–6 | 1 | 3 |
/// | `tail_tone` | the tails' opacity as a share of the line's (the knots stay whole) | 0.2–1 | 0.05 | 0.5 |
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadShape {
    /// Each knot after the largest is this much the size of the one before.
    pub knot_fall: f32,
    /// Strokes of each knot that escape to 1.5–2.2 times its radius.
    pub escape: f32,
    /// Gentle bends in a tail.
    pub tail_turns: f32,
    /// The tails' opacity as a share of the line's.
    pub tail_tone: f32,
}
impl Default for ThreadShape { fn default() -> Self { Self { knot_fall: 0.55, escape: 2.0, tail_turns: 3.0, tail_tone: 0.5 } } }
impl ThreadShape {
    /// The numbers' ranges and steps, for controls: (name, min, max, step).
    pub const PARAMS: [(&'static str, f32, f32, f32); 4] = [("knot_fall", 0.3, 1.0, 0.05), ("escape", 0.0, 4.0, 1.0), ("tail_turns", 1.0, 6.0, 1.0), ("tail_tone", 0.2, 1.0, 0.05)];
    /// The same with every number inside its range (the counts whole).
    pub fn clamped(self) -> Self {
        Self { knot_fall: self.knot_fall.clamp(0.3, 1.0), escape: self.escape.round().clamp(0.0, 4.0), tail_turns: self.tail_turns.round().clamp(1.0, 6.0), tail_tone: self.tail_tone.clamp(0.2, 1.0) }
    }
}

/// What a piece of the thread is: along the line (an outline or a chord across it), a bridge between
/// outlines, a knot, or a tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Along the line: an outline, or a chord across it.
    Line,
    /// A bridge between two outlines.
    Bridge,
    /// A knot.
    Knot,
    /// A tail.
    Tail,
}
/// One straight piece, device px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg {
    /// Where it starts.
    pub a: [f32; 2],
    /// Where it ends.
    pub b: [f32; 2],
    /// What it is.
    pub kind: Kind,
}

/// The rest path: one polyline through every outline; `bridge[i]` says the piece arriving at `pts[i]` is
/// a bridge. `loops` is how many outlines it went round. `keep[i]` is the tolerance (device px) up to
/// which vertex `i` stays, `chord[i]` the two vertices whose chord it is dropped onto, and `order` the
/// vertices from the most to the least kept (a vertex after both ends of its chord).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rest {
    /// The vertices.
    pub pts: Vec<[f32; 2]>,
    /// Whether the piece arriving at each vertex is a bridge.
    pub bridge: Vec<bool>,
    /// How many outlines it went round.
    pub loops: usize,
    /// The tolerance (device px) up to which each vertex stays.
    pub keep: Vec<f32>,
    /// The two vertices whose chord each one is dropped onto.
    pub chord: Vec<(usize, usize)>,
    /// The vertices from the most to the least kept.
    pub order: Vec<usize>,
}

/// A glyph's or shape's outlines (closed, the last point equal to the first), device px, sampled on a
/// `grain` lattice over its bounds and simplified to half a pixel.
pub fn outline(inst: &Instance, atlas: &[u8], grain: f32) -> Vec<Vec<[f32; 2]>> {
    let g = grain.max(0.5);
    let b = inst.bounds(g);
    let (cols, rows) = ((((b[2] - b[0]) / g).ceil() as usize + 1).max(2), (((b[3] - b[1]) / g).ceil() as usize + 1).max(2));
    let mut grid = crate::field::Grid::new(cols, rows, (cols - 1) as f32 * g, (rows - 1) as f32 * g);
    for j in 0..rows {
        for i in 0..cols {
            // The outermost ring is outside the ink, so every contour closes.
            let v = if i == 0 || j == 0 || i == cols - 1 || j == rows - 1 { 0.0 } else { crate::morph::cover_at(inst, b[0] + i as f32 * g, b[1] + j as f32 * g, atlas) };
            grid.v[j * cols + i] = v;
        }
    }
    grid.contours(0.5).into_iter().filter(|c| c.len() >= 4).map(|c| {
        let mut p: Vec<[f32; 2]> = c.into_iter().map(|(x, y)| [b[0] + x, b[1] + y]).collect();
        if p.first() != p.last() { let f = p[0]; p.push(f); }
        simplify(&p, 0.5)
    }).filter(|c| c.len() >= 4).collect()
}

/// A box's outline (rounded corners of radius `r`), turned by its own transform, points about `step`
/// apart, device px.
pub fn box_outline(rect: [f32; 4], r: f32, xf: [f32; 4], step: f32) -> Vec<[f32; 2]> {
    let [x, y, w, h] = rect;
    let r = r.clamp(0.0, 0.5 * w.min(h));
    let st = step.max(1.0);
    let mut pts = vec![];
    // Corner centres, clockwise from the top left; each corner's arc then the straight edge after it.
    let cs = [(x + r, y + r, std::f32::consts::PI), (x + w - r, y + r, 1.5 * std::f32::consts::PI), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 0.5 * std::f32::consts::PI)];
    for (k, &(cx, cy, a0)) in cs.iter().enumerate() {
        let n = if r > 0.0 { ((r * std::f32::consts::FRAC_PI_2) / st).ceil().max(1.0) as usize } else { 0 };
        for i in 0..=n { let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / n.max(1) as f32; pts.push([cx + r * a.cos(), cy + r * a.sin()]); }
        let (nx, ny, _) = cs[(k + 1) % 4];
        let next_start = { let a = cs[(k + 1) % 4].2; [nx + r * a.cos(), ny + r * a.sin()] };
        let last = *pts.last().expect("a point");
        let len = ((next_start[0] - last[0]).powi(2) + (next_start[1] - last[1]).powi(2)).sqrt();
        let m = (len / st).floor() as usize;
        for i in 1..m { let t = i as f32 / m as f32; pts.push([last[0] + (next_start[0] - last[0]) * t, last[1] + (next_start[1] - last[1]) * t]); }
    }
    let f = pts[0];
    pts.push(f);
    let (cx, cy) = (x + w * 0.5, y + h * 0.5);
    if xf != [0.0; 4] {
        for p in &mut pts { let (dx, dy) = (p[0] - cx, p[1] - cy); *p = [cx + (1.0 + xf[0]) * dx + xf[1] * dy, cy + xf[2] * dx + (1.0 + xf[3]) * dy]; }
    }
    pts
}

/// Ramer–Douglas–Peucker to within `tol` px (the ends are kept).
pub fn simplify(p: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    if p.len() < 3 { return p.to_vec(); }
    let mut keep = vec![false; p.len()];
    keep[0] = true;
    *keep.last_mut().expect("points") = true;
    let mut stack = vec![(0, p.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 { continue; }
        let (pa, pb) = (p[a], p[b]);
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let l2 = dx * dx + dy * dy;
        let (mut far, mut at) = (0.0, a);
        for (i, q) in p.iter().enumerate().take(b).skip(a + 1) {
            let d = if l2 < 1e-9 { ((q[0] - pa[0]).powi(2) + (q[1] - pa[1]).powi(2)).sqrt() } else { ((q[0] - pa[0]) * dy - (q[1] - pa[1]) * dx).abs() / l2.sqrt() };
            if d > far { far = d; at = i; }
        }
        if far > tol { keep[at] = true; stack.push((a, at)); stack.push((at, b)); }
    }
    p.iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| *q).collect()
}

/// The indices of the points Ramer–Douglas–Peucker keeps to within `tol` px (the ends are kept).
fn simplified(p: &[[f32; 2]], tol: f32) -> Vec<usize> {
    if p.len() < 3 { return (0..p.len()).collect(); }
    let mut keep = vec![false; p.len()];
    keep[0] = true;
    keep[p.len() - 1] = true;
    let mut stack = vec![(0, p.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 { continue; }
        let (mut far, mut at) = (0.0, a);
        for i in a + 1..b { let d = seg_dist(p[i], p[a], p[b]); if d > far { far = d; at = i; } }
        if far > tol { keep[at] = true; stack.push((a, at)); stack.push((at, b)); }
    }
    (0..p.len()).filter(|&i| keep[i]).collect()
}

/// One polyline through every outline, in order: round each loop back to where it started, then a
/// bridge to the nearest vertex of the next loop (which is walked from there).
pub fn join(loops: &[Vec<[f32; 2]>]) -> Rest {
    let mut r = Rest::default();
    for l in loops.iter().filter(|l| l.len() >= 2) {
        // A closed loop without its repeated last point.
        let body = if l.first() == l.last() { &l[..l.len() - 1] } else { &l[..] };
        let start = match r.pts.last() {
            None => 0,
            Some(e) => (0..body.len()).min_by(|&a, &b| d2(body[a], *e).total_cmp(&d2(body[b], *e))).unwrap_or(0),
        };
        for k in 0..=body.len() {
            let p = body[(start + k) % body.len()];
            r.bridge.push(k == 0 && !r.pts.is_empty());
            r.pts.push(p);
        }
        r.loops += 1;
    }
    rank(&mut r);
    r
}
fn d2(a: [f32; 2], b: [f32; 2]) -> f32 { (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) }

/// Rank the rest path's vertices (`Rest::keep`, `chord`, `order`): one Ramer–Douglas–Peucker pass over
/// the whole line, where a vertex's value is its distance from the chord it splits, capped by its
/// parent's, so the vertices kept at a tolerance always include those kept at a larger one.
fn rank(r: &mut Rest) {
    let n = r.pts.len();
    r.keep = vec![f32::INFINITY; n];
    r.chord = (0..n).map(|i| (i, i)).collect();
    r.order = if n > 1 { vec![0, n - 1] } else { (0..n).collect() };
    if n < 3 { return; }
    let mut stack = vec![(0, n - 1, f32::INFINITY)];
    while let Some((a, b, cap)) = stack.pop() {
        if b <= a + 1 { continue; }
        let (mut far, mut at) = (-1.0f32, a + 1);
        for i in a + 1..b {
            let d = seg_dist(r.pts[i], r.pts[a], r.pts[b]);
            if d > far { far = d; at = i; }
        }
        let v = far.min(cap);
        r.keep[at] = v;
        r.chord[at] = (a, b);
        r.order.push(at);
        stack.push((at, b, v));
        stack.push((a, at, v));
    }
}
fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / (ex * ex + ey * ey).max(1e-9)).clamp(0.0, 1.0);
    d2(p, [a[0] + ex * t, a[1] + ey * t]).sqrt()
}
fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t] }

/// Where and when a thread is worked out: its looseness, the (held) time, the element's seed, logical →
/// device scale, the map's `drift` (logical px), the scope box and the boxes the tails keep out of (device
/// px), and the element's line height (device px: a text's line, a shape's short side).
pub struct Ctx<'a> {
    /// Its looseness.
    pub k: f32,
    /// The (held) time.
    pub t: f32,
    /// The element's seed.
    pub seed: u32,
    /// Logical px to device px.
    pub scale: f32,
    /// The map's `drift`, logical px.
    pub drift: f32,
    /// The scope box, device px.
    pub scope: [f32; 4],
    /// The boxes the tails keep out of, device px.
    pub calm: &'a [[f32; 4]],
    /// The element's line height, device px (a text's line, a shape's short side).
    pub lh: f32,
}

/// Value noise and its gradient (the same lattice as `field::noise`), so the curl needs no differences.
fn noise_grad(x: f32, y: f32) -> (f32, f32, f32) {
    let lattice = |ix: i32, iy: i32| hash((ix as u32).wrapping_mul(73_856_093) ^ (iy as u32).wrapping_mul(19_349_663));
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (dsx, dsy) = (6.0 * fx * (1.0 - fx), 6.0 * fy * (1.0 - fy));
    let (a, b, c, d) = (lattice(ix, iy), lattice(ix + 1, iy), lattice(ix, iy + 1), lattice(ix + 1, iy + 1));
    let v = a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy;
    ((v), ((b - a) + (a - b - c + d) * sy) * dsx, ((c - a) + (a - b - c + d) * sx) * dsy)
}
/// The wander: the curl of a smooth noise (features 60 logical px apart, drifting with `t` as
/// `field::noise_t` does), at most 1 long. It bends whole words together.
pub fn wander(x: f32, y: f32, t: f32, seed: u32, scale: f32) -> [f32; 2] {
    let o = hash(seed) * 91.0;
    let (kt, ft) = { let tt = t * 0.5; (tt.floor(), tt - tt.floor()) };
    let ft = ft * ft * (3.0 - 2.0 * ft);
    let f = 1.0 / (60.0 * scale.max(1e-3));
    let (px, py) = (x * f + o, y * f - o);
    let (_, ax, ay) = noise_grad(px + kt * 17.31, py - kt * 9.77);
    let (_, bx, by) = noise_grad(px + (kt + 1.0) * 17.31, py - (kt + 1.0) * 9.77);
    let (u, v) = (1.6 * (ay + (by - ay) * ft), -1.6 * (ax + (bx - ax) * ft));
    let l = (u * u + v * v).sqrt();
    if l > 1.0 { [u / l, v / l] } else { [u, v] }
}
/// A vector at lattice point `i` of a path, keyed by time: a hashed direction, 0.3–1 long.
fn jag_at(seed: u32, i: i32, key: i32) -> [f32; 2] {
    let h = seed.wrapping_mul(0x2545_F491) ^ (i as u32).wrapping_mul(2_654_435_761) ^ (key as u32).wrapping_mul(40_503);
    let a = hash(h) * std::f32::consts::TAU;
    let m = 0.3 + 0.7 * hash(h ^ 0x68E3_1DA4);
    [m * a.cos(), m * a.sin()]
}
/// The jag at arc length `s` along a path: hashed vectors `step` apart, eased between, and between
/// one second's key and the next. Vertices a chord apart move independently; the ones dropped
/// between them move with them, so nothing jumps as they slide onto their chord.
fn jag(s: f32, step: f32, t: f32, seed: u32) -> [f32; 2] {
    let u = s / step.max(1e-3);
    let (i, f) = (u.floor() as i32, smooth(0.0, 1.0, u - u.floor()));
    let (kt, ft) = (t.floor() as i32, smooth(0.0, 1.0, t - t.floor()));
    let at = |key: i32| lerp2(jag_at(seed, i, key), jag_at(seed, i + 1, key), f);
    lerp2(at(kt), at(kt + 1), ft)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
/// The inverse of `smooth(0, 1, x)` on 0..1.
fn unsmooth(y: f32) -> f32 { 0.5 - ((1.0 - 2.0 * y.clamp(0.0, 1.0)).asin() / 3.0).sin() }
/// How opaque the thread's line is at `k`, and its bridges, knots and tails.
pub fn opacity(k: f32) -> (f32, f32) { (smooth(0.0, 0.12, k), smooth(0.1, 0.3, k)) }
/// How much of a loose element's fill is left at `k`.
pub fn fill(k: f32) -> f32 { 1.0 - smooth(0.02, 0.25, k) }
/// How many control points the knot with the most has, with energy `e` at `k` (36 at the defaults, fully
/// loose; each knot has 64–100 % of it, and one with fewer than `MIN_KNOT` is not drawn).
pub fn knots(knot: f32, e: f32, k: f32) -> usize { (60.0 * knot * (0.5 + 0.5 * e.clamp(0.0, 1.0)) * smooth(0.2, 0.6, k)).round().max(0.0) as usize }
/// The fewest control points a knot is drawn with: fewer would read as a star, so there is no knot.
pub const MIN_KNOT: usize = 12;
/// The looseness the thread shows for looseness `k`: the same at the ends, but through 0.15–0.4, where a
/// half-drawn letter and a few loose lines look neither like text nor like a thread, it hurries (that
/// band is crossed while `k` goes from 0.22 to 0.30), so loosening and drawing back stay there only a
/// few frames.
pub fn shown(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    if k <= 0.22 { k * (0.15 / 0.22) } else if k <= 0.30 { 0.15 + (k - 0.22) * (0.25 / 0.08) } else { 0.4 + (k - 0.30) * (0.6 / 0.7) }
}
/// The looseness a held pose (a step of `Look::beat`) is worked out at: as shown, but never inside 0.15–0.4 (the
/// nearer end of it), so no beat holds the in-between look.
pub fn held(k: f32) -> f32 { if k > 0.15 && k < 0.4 { if k < 0.275 { 0.15 } else { 0.4 } } else { k } }
/// How many knots a line `len` logical px long has.
pub fn knot_count(len: f32) -> usize { ((len / 200.0).round() as usize).clamp(1, 3) }
/// The tolerance the line is simplified to at `k` (device px), for line height `lh`.
pub fn tolerance(k: f32, lh: f32) -> f32 { 0.5 + smooth(0.3, 0.5, k) * 0.3 * lh }
/// How far vertex value `v` has slid onto its chord at `k` (0 on its own place, 1 dropped): it starts
/// as the tolerance passes it (between 0.3 and 0.5) and takes half the looseness, so every vertex
/// under the last tolerance is dropped at 1.
fn slid(v: f32, k: f32, lh: f32) -> f32 {
    let top = 0.3 * lh;
    if v >= 0.5 + top { return 0.0; }
    let k0 = 0.3 + 0.2 * unsmooth(((v - 0.5) / top.max(1e-3)).clamp(0.0, 1.0));
    smooth(k0, k0 + 0.5, k)
}

/// Every rest vertex where the thread puts it at this looseness, and how far it has slid onto its chord
/// (a vertex at 1 is dropped: it lies on the line between the vertices kept around it).
pub fn pose(rest: &Rest, fx: &StrandFx, c: &Ctx, field: &dyn Fn(f32, f32) -> (f32, [f32; 2])) -> (Vec<[f32; 2]>, Vec<f32>) {
    let n = rest.pts.len();
    let k = c.k.clamp(0.0, 1.0);
    let s = c.scale;
    let lh = c.lh.max(1.0);
    let go = smooth(0.3, 1.0, k);
    let inside = |p: [f32; 2]| [p[0].clamp(c.scope[0], c.scope[2]), p[1].clamp(c.scope[1], c.scope[3])];
    let step = (0.5 * lh).max(8.0 * s);
    let mut arc = 0.0;
    let own: Vec<[f32; 2]> = (0..n).map(|i| {
        let v = rest.pts[i];
        if i > 0 { arc += d2(rest.pts[i - 1], v).sqrt(); }
        if go <= 0.0 { return v; }
        let w = wander(v[0], v[1], c.t, c.seed, s);
        let j = jag(arc, step, c.t, c.seed);
        let (_, fl) = field(v[0], v[1]);
        let (r, d) = (fx.reach * s, fx.pull * c.drift * s);
        inside([v[0] + go * (r * (0.4 * w[0] + j[0]) + d * fl[0]), v[1] + go * (r * (0.4 * w[1] + j[1]) + d * fl[1])])
    }).collect();
    // Each vertex rides its chord: where the chord's ends are now, plus its own offset from the chord
    // (as unslid) by how much of it is left. So a vertex moves with the vertices it drops between, and
    // only its own offset shrinks as it slides.
    let mut at = own.clone();
    let mut sl = vec![0.0; n];
    for &i in &rest.order {
        let (a, b) = rest.chord[i];
        if a == b { continue; }
        let r = slid(rest.keep[i], k, lh);
        let (pa, pb, v) = (rest.pts[a], rest.pts[b], rest.pts[i]);
        let (ex, ey) = (pb[0] - pa[0], pb[1] - pa[1]);
        let t = (((v[0] - pa[0]) * ex + (v[1] - pa[1]) * ey) / (ex * ex + ey * ey).max(1e-9)).clamp(0.0, 1.0);
        let base = lerp2(own[a], own[b], t);
        let on = lerp2(at[a], at[b], t);
        at[i] = inside([on[0] + (1.0 - r) * (own[i][0] - base[0]), on[1] + (1.0 - r) * (own[i][1] - base[1])]);
        sl[i] = r;
    }
    (at, sl)
}

/// A curve through control points `ctrl` (device px): a uniform cubic B-spline whose end points are
/// repeated three times, so it leaves the first control point and arrives at the last; between, it does
/// not pass through them, so a place where the control points fold back becomes a narrow loop. Sampled
/// by how much it bends: neighbouring pieces turn by at most `max_turn` degrees (where the curve itself
/// is sharper than that over 1.5 px, a little more), no piece is shorter than 1.5 px, and no piece
/// strays more than a quarter pixel from the curve. The same input gives the same output.
pub fn curve(ctrl: &[[f32; 2]], max_turn: f32) -> Vec<[f32; 2]> {
    if ctrl.len() < 2 { return ctrl.to_vec(); }
    let (dense, keep) = smooth_spline(ctrl, max_turn);
    keep.into_iter().map(|i| dense[i].0).collect()
}
/// The control points with each end repeated twice more.
/// Where the control points fold straight back (the points either side of one nearly on top of each
/// other), the one after is moved aside, so the curve makes a narrow loop there and not a point.
fn padded(ctrl: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let (a, b) = (ctrl[0], ctrl[ctrl.len() - 1]);
    let mut c = ctrl.to_vec();
    let n = c.len();
    for i in 1..n.saturating_sub(1) {
        // The point that moves: the one after, or before the last, the one before (the ends stay).
        let (fixed, moved) = if i + 1 < n - 1 { (i - 1, i + 1) } else if i >= 2 { (i + 1, i - 1) } else { continue };
        let (p, q, r) = (c[fixed], c[i], c[moved]);
        let arm = d2(p, q).sqrt().max(d2(q, r).sqrt());
        let gap = d2(p, r).sqrt();
        if arm < 1e-3 || gap >= 0.15 * arm { continue; }
        // Aside: across the way in, to the side the moved point already leans to.
        let len = d2(p, q).sqrt().max(1e-3);
        let way = [(q[0] - p[0]) / len, (q[1] - p[1]) / len];
        let mut side = [-way[1], way[0]];
        if (r[0] - p[0]) * side[0] + (r[1] - p[1]) * side[1] < 0.0 { side = [-side[0], -side[1]]; }
        let along = (r[0] - p[0]) * way[0] + (r[1] - p[1]) * way[1];
        c[moved] = [p[0] + way[0] * along + side[0] * 0.15 * arm, p[1] + way[1] * along + side[1] * 0.15 * arm];
    }
    let mut out = Vec::with_capacity(n + 4);
    out.extend([a, a]);
    out.extend_from_slice(&c);
    out.extend([b, b]);
    out
}
/// The B-spline over padded control points `p` at `u` (0 to `p.len() − 3`): span `floor(u)`.
fn spline_at(p: &[[f32; 2]], u: f32) -> [f32; 2] {
    let spans = p.len() - 3;
    let s = (u.floor().max(0.0) as usize).min(spans - 1);
    let t = (u - s as f32).clamp(0.0, 1.0);
    let (t2, t3) = (t * t, t * t * t);
    let w = [(1.0 - t).powi(3) / 6.0, (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0, (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0, t3 / 6.0];
    let mut q = [0.0f32; 2];
    for (k, wk) in w.iter().enumerate() { q[0] += wk * p[s + k][0]; q[1] += wk * p[s + k][1]; }
    q
}
/// The spline sampled at most half a pixel apart, and finer where it turns sharply (each step turning at
/// most 4°, down to a five-hundredth of a pixel), with each sample's parameter `u`.
fn dense(p: &[[f32; 2]]) -> Vec<([f32; 2], f32)> {
    let spans = p.len() - 3;
    let mut out = Vec::with_capacity(spans * 8);
    fn refine(p: &[[f32; 2]], u0: f32, a: [f32; 2], u1: f32, b: [f32; 2], depth: u32, out: &mut Vec<([f32; 2], f32)>) {
        let um = 0.5 * (u0 + u1);
        let m = spline_at(p, um);
        let far = d2(a, b).sqrt();
        let bent = far > 0.002 && angle([m[0] - a[0], m[1] - a[1]], [b[0] - m[0], b[1] - m[1]]) > 4.0;
        if depth < 18 && (far > 0.5 || bent) {
            refine(p, u0, a, um, m, depth + 1, out);
            refine(p, um, m, u1, b, depth + 1, out);
        } else { out.push((b, u1)); }
    }
    out.push((p[0], 0.0));
    for s in 0..spans {
        let poly: f32 = (0..3).map(|k| d2(p[s + k], p[s + k + 1]).sqrt()).sum();
        let m = ((poly / 4.0).ceil() as usize).clamp(2, 400);
        for j in 0..m {
            let (u0, u1) = (s as f32 + j as f32 / m as f32, s as f32 + (j + 1) as f32 / m as f32);
            let a = out.last().expect("a sample").0;
            refine(p, u0, a, u1, spline_at(p, u1), 0, &mut out);
        }
    }
    // The ends exactly: the first and last control points.
    out.dedup_by(|b, a| d2(a.0, b.0) <= 1e-8);
    if let Some(l) = out.last_mut() { l.0 = p[p.len() - 1]; }
    out
}
/// The spline through `ctrl`, densely sampled, and which samples `decimate` keeps, with no point where
/// it stops and turns back: where two kept pieces still turn by more than 60° (a cusp, the curve
/// standing still), the control point nearest to it is moved aside a little, up to ten times, further
/// each time.
fn smooth_spline(ctrl: &[[f32; 2]], max_turn: f32) -> (Vec<([f32; 2], f32)>, Vec<usize>) {
    let mut pad = padded(ctrl);
    let mut d = dense(&pad);
    let mut keep = decimate(&d.iter().map(|x| x.0).collect::<Vec<_>>(), max_turn);
    for round in 0..10 {
        let cusp = keep.windows(3).find(|w| { let (a, b, c) = (d[w[0]].0, d[w[1]].0, d[w[2]].0); angle([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]) > 60.0 }).map(|w| d[w[1]].1);
        let Some(u) = cusp else { break };
        let s = (u.floor().max(0.0) as usize).min(pad.len() - 4);
        let at = spline_at(&pad, u);
        let Some(j) = (s..s + 4).filter(|&j| j >= 3 && j + 3 < pad.len()).min_by(|&a, &b| d2(pad[a], at).total_cmp(&d2(pad[b], at))) else { break };
        let arm = d2(pad[j - 1], pad[j]).sqrt().max(d2(pad[j], pad[j + 1]).sqrt()).max(1.0);
        let way = [pad[j + 1][0] - pad[j - 1][0], pad[j + 1][1] - pad[j - 1][1]];
        let l = (way[0] * way[0] + way[1] * way[1]).sqrt().max(1e-6);
        // Aside, across the way through it, a little further each time.
        let by = (0.05 + 0.025 * round as f32) * arm;
        pad[j] = [pad[j][0] - way[1] / l * by, pad[j][1] + way[0] / l * by];
        d = dense(&pad);
        keep = decimate(&d.iter().map(|x| x.0).collect::<Vec<_>>(), max_turn);
    }
    (d, keep)
}
/// The angle between two directions, degrees.
fn angle(u: [f32; 2], v: [f32; 2]) -> f32 {
    let (lu, lv) = ((u[0] * u[0] + u[1] * u[1]).sqrt(), (v[0] * v[0] + v[1] * v[1]).sqrt());
    if lu < 1e-9 || lv < 1e-9 { return 0.0; }
    ((u[0] * v[0] + u[1] * v[1]) / (lu * lv)).clamp(-1.0, 1.0).acos().to_degrees()
}
/// Which of the densely sampled points `d` to keep: from each kept point, the farthest next one whose
/// piece turns from the last by at most `max_turn` degrees, leaves and meets the curve within half of it,
/// and stays within a quarter pixel of the points it skips; 1.5–48 px long, or shorter only where the
/// curve is too tight for a piece that long (the tip of a narrow loop). The ends are kept.
fn decimate(d: &[[f32; 2]], max_turn: f32) -> Vec<usize> {
    const MIN: f32 = 1.5;
    const LEAST: f32 = 0.05;
    const MAX: f32 = 48.0;
    const TOL: f32 = 0.25;
    let last = d.len().saturating_sub(1);
    let mut out = vec![0];
    if last == 0 { return out; }
    let tangent = |j: usize| { let (a, b) = (d[j.saturating_sub(1)], d[(j + 1).min(last)]); [b[0] - a[0], b[1] - a[1]] };
    let (mut i, mut prev): (usize, Option<[f32; 2]>) = (0, None);
    while i < last {
        let (mut best, mut j) = (None, i + 1);
        let ti = tangent(i);
        while j <= last {
            let c = [d[j][0] - d[i][0], d[j][1] - d[i][1]];
            let l = (c[0] * c[0] + c[1] * c[1]).sqrt();
            let ok = l <= MAX && prev.is_none_or(|p| angle(p, c) <= max_turn) && angle(ti, c) <= 0.5 * max_turn && angle(c, tangent(j)) <= 0.5 * max_turn
                && (i + 1..j).all(|m| seg_dist(d[m], d[i], d[j]) <= TOL);
            if ok { best = Some(j); } else if l >= MIN { break; }
            j += 1;
        }
        let long = |b: usize| d2(d[b], d[i]).sqrt();
        let next = match best { Some(b) if long(b) > 1e-4 => b, _ => j.min(last) };
        prev = Some([d[next][0] - d[i][0], d[next][1] - d[i][1]]);
        out.push(next);
        i = next;
    }
    // A last piece shorter than half a pixel: its start dropped (the end is kept).
    if out.len() > 2 && d2(d[out[out.len() - 2]], d[last]).sqrt() < LEAST { out.remove(out.len() - 2); }
    out
}
/// A line through the points `pts` blended toward the curve through them by `w` (0: the polyline
/// itself, 1: `curve`), sampled as `curve` is. Each point it returns carries the index of the point of
/// `pts` it arrives toward (to keep what each piece is).
fn bend_line(pts: &[[f32; 2]], w: f32, max_turn: f32) -> Vec<([f32; 2], usize)> {
    let n = pts.len();
    if n < 2 { return pts.iter().map(|p| (*p, 0)).collect(); }
    let (sp, _) = smooth_spline(pts, max_turn);
    // The polyline with its corners rounded as far as the blend goes (up to half of the shorter edge
    // when it is all curve), so what is left of a corner after the blend is a turn of the pen, not a kink.
    let unit = |a: [f32; 2], b: [f32; 2]| { let l = d2(a, b).sqrt().max(1e-6); [(b[0] - a[0]) / l, (b[1] - a[1]) / l] };
    let round: Vec<f32> = (0..n).map(|i| if i == 0 || i == n - 1 { 0.0 } else { 0.5 * w * d2(pts[i - 1], pts[i]).sqrt().min(d2(pts[i], pts[i + 1]).sqrt()) }).collect();
    let bez = |p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], v: f32| lerp2(lerp2(p0, p1, v), lerp2(p1, p2, v), v);
    // Span `s` of the spline runs from about `pts[s − 1]` to about `pts[s]`; the polyline the same way.
    let poly = |u: f32| {
        let s = u.floor().max(0.0) as usize;
        let t = (u - s as f32).clamp(0.0, 1.0);
        let (i0, i1) = (s.saturating_sub(1).min(n - 1), s.min(n - 1));
        let (a, b) = (pts[i0], pts[i1]);
        let l = d2(a, b).sqrt();
        if i0 == i1 || l < 1e-6 { return lerp2(a, b, t); }
        let (ta, tb) = (round[i0] / l, round[i1] / l);
        let ab = unit(a, b);
        if t < ta {
            let din = unit(pts[i0 - 1], a);
            bez([a[0] - din[0] * round[i0], a[1] - din[1] * round[i0]], a, [a[0] + ab[0] * round[i0], a[1] + ab[1] * round[i0]], 0.5 + 0.5 * t / ta)
        } else if t > 1.0 - tb {
            let dout = unit(b, pts[i1 + 1]);
            bez([b[0] - ab[0] * round[i1], b[1] - ab[1] * round[i1]], b, [b[0] + dout[0] * round[i1], b[1] + dout[1] * round[i1]], 0.5 * (t - (1.0 - tb)) / tb)
        } else { lerp2(a, b, t) }
    };
    let d: Vec<([f32; 2], f32)> = sp.into_iter().map(|(q, u)| (lerp2(poly(u), q, w), u)).collect();
    let mut d = d;
    d.dedup_by(|b, a| d2(a.0, b.0) <= 1e-8);
    let keep = decimate(&d.iter().map(|x| x.0).collect::<Vec<_>>(), max_turn);
    keep.into_iter().map(|i| (d[i].0, (d[i].1.ceil().max(0.0) as usize).min(n - 1))).collect()
}

/// The control points of a knot about `centre` (`tangle` draws the curve through them).
fn tangle_ctrl(centre: [f32; 2], r: f32, n: usize, escape: usize, seed: u32, t: f32) -> Vec<[f32; 2]> {
    let (kt, ft) = (t.floor() as i32, smooth(0.0, 1.0, t - t.floor()));
    let h = |j: u32, salt: u32, key: i32| hash(seed.wrapping_mul(0x9E37_79B9) ^ j.wrapping_mul(0x85EB_CA6B) ^ salt ^ (key as u32).wrapping_mul(0xC2B2_AE35));
    let esc_n = escape.min(n / 4);
    // The escapes: evenly through the knot, each place moved by a hash.
    let esc: Vec<usize> = (0..esc_n).map(|e| ((e as f32 + 0.25 + 0.5 * hash(seed ^ 0x3C6E_F372 ^ e as u32)) * n as f32 / esc_n as f32) as usize % n.max(1)).collect();
    let out_at = |j: usize| esc.contains(&j);
    // A step about the centre at key `key`: through the knot, round it, or back the way it came.
    let step = |j: u32, key: i32| {
        let (u, v) = (h(j, 0x27D4_EB2F, key), h(j, 0x6A09_E667, key));
        // Out to an escape and back in: nearly straight out from the centre, so it is a long narrow
        // loop, a stroke that gets out of the knot.
        if out_at(j as usize) || out_at((j as usize + n - 1) % n.max(1)) { 0.05 + 0.15 * v } else if u < 0.55 { 1.9 + 0.9 * v } else if u < 0.85 { 0.5 + 0.8 * v } else { 2.9 + 0.5 * v }
    };
    // How far out: a step across the knot reaches 0.4–1 of the radius, one round it stays inside
    // (0.25–0.6: the small loops are in the middle), one back the way it came anywhere in 0.25–1.
    let radius = |j: u32, key: i32| {
        let (u, v) = (h(j, 0x27D4_EB2F, key), h(j, 0x1656_67B1, key));
        if out_at(j as usize) { 1.5 + 0.7 * v }
        else if u < 0.55 { 0.3 + 0.65 * v } else if u < 0.85 { 0.3 + 0.35 * v } else { 0.25 + 0.45 * v }
    };
    let eased = |f: &dyn Fn(u32, i32) -> f32, j: u32| f(j, kt) + (f(j, kt + 1) - f(j, kt)) * ft;
    // The centre wanders within a quarter of the radius, slowly (a new way every six points).
    let drift = |m: i32, key: i32| { let a = h(m as u32, 0x3243_F6A8, key) * std::f32::consts::TAU; let l = h(m as u32, 0x8858_6E5B, key); [l * a.cos(), l * a.sin()] };
    let wander = |j: u32| {
        let x = j as f32 / 6.0;
        let (m, f) = (x.floor() as i32, smooth(0.0, 1.0, x - x.floor()));
        let at = |key: i32| lerp2(drift(m, key), drift(m + 1, key), f);
        let p = lerp2(at(kt), at(kt + 1), ft);
        [0.25 * r * p[0], 0.25 * r * p[1]]
    };
    let turn = if hash(seed ^ 0x2B99_2DDF) < 0.5 { -1.0 } else { 1.0 };
    let lean = hash(seed ^ 0x5851_F42D) * std::f32::consts::PI;
    let (lc, ls) = (lean.cos(), lean.sin());
    let steps: Vec<f32> = (0..n as u32 + 1).map(|j| eased(&step, j)).collect();
    // Each place: its angle, and its distance from the centre. A B-spline lands at about
    // `(4 P + P₋ + P₊) / 6`, so the curve stays well inside the points across the knot (the dense
    // middle); an escape is set out by as much, so it gets as far as asked.
    let mut a = hash(seed ^ 0x1B87_3593) * std::f32::consts::TAU;
    let mut ang: Vec<f32> = Vec::with_capacity(n);
    for (j, st) in steps.iter().take(n).enumerate() {
        a += turn * st;
        // A step back the way it came lands beside the point before last, not on it, so it makes a
        // narrow loop rather than a point.
        if j >= 2 && { let d = (a - ang[j - 2]).rem_euclid(std::f32::consts::TAU); d.min(std::f32::consts::TAU - d) < 0.3 } { a += turn * 0.3; }
        ang.push(a);
    }
    (0..n).map(|j| {
        let (tin, tout) = (steps[j], steps[j + 1]);
        let raw = 6.0 / (4.0 + tin.cos() + tout.cos());
        let set_out = if out_at(j) { raw.clamp(1.4, 1.8) } else { raw.sqrt().clamp(1.0, 1.45) };
        let rr = r * eased(&radius, j as u32) * set_out;
        let (x, y) = (rr * ang[j].cos(), rr * ang[j].sin());
        // Stretched along the lean, squeezed across it (an escape is not squeezed: it gets out).
        let (u, v) = (x * lc + y * ls, -x * ls + y * lc);
        let (u, v) = if out_at(j) { (u, v) } else { (u * 1.3, v * 0.75) };
        // Only the escapes get out past 1.3 r.
        let (u, v) = { let l = (u * u + v * v).sqrt(); let most = if out_at(j) { f32::INFINITY } else { 1.3 * r }; if l > most { (u * most / l, v * most / l) } else { (u, v) } };
        let o = wander(j as u32);
        [centre[0] + o[0] + u * lc - v * ls, centre[1] + o[1] + u * ls + v * lc]
    }).collect()
}
/// A knot about `centre`: a tangle of one thin line, the curve (`curve`, 15°) through `n` control points
/// that each turn about the centre from the last: 55 % of the steps 1.9–2.8 radians (across the knot), 30 %
/// 0.5–1.3 (round it, making loops), 15 % 2.9–3.4 (back the way it came, a narrow loop); most at
/// `r × (0.25–1)` from it, `escape` of them set out so the curve gets to `r × (1.5–2.2)`; the centre itself wandering within
/// `0.25 r`; the whole stretched along a hashed way (1.3 against 0.75). Eased between one second's key
/// and the next, so it moves without jumps.
pub fn tangle(centre: [f32; 2], r: f32, n: usize, escape: usize, seed: u32, t: f32) -> Vec<[f32; 2]> {
    curve(&tangle_ctrl(centre, r, n, escape, seed, t), 15.0)
}

/// One knot of a thread: the rest vertex it hangs from, its radius (device px, before looseness), how many
/// points it has, and how far its centre sits off the line (device px, before looseness).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnotAt {
    /// The rest vertex it hangs from.
    pub at: usize,
    /// Its radius, device px, before looseness.
    pub r: f32,
    /// How many points it has.
    pub n: usize,
    /// How far its centre sits off the line, device px, before looseness.
    pub off: [f32; 2],
}
/// The knots of a rest path (`thread` draws them): up to `knot_count` vertices of the line where the
/// map's energy is highest (kept to the end, 80 px apart), the most energetic first and largest
/// (`knot_r`), each next one smaller by `knot_fall` (1, `knot_fall`, `knot_fall^1.75`), with 64–100 % of
/// `knots` points; each centre off the line by 0.5–0.9 of the largest radius, above and below in turn.
pub fn knot_plan(rest: &Rest, fx: &StrandFx, c: &Ctx, field: &dyn Fn(f32, f32) -> (f32, [f32; 2])) -> Vec<KnotAt> {
    let n = rest.pts.len();
    let s = c.scale;
    let k = c.k.clamp(0.0, 1.0);
    let top = 0.5 + 0.3 * c.lh.max(1.0);
    let len: f32 = rest.pts.windows(2).map(|w| d2(w[0], w[1]).sqrt()).sum();
    let energy: Vec<f32> = rest.pts.iter().map(|p| field(p[0], p[1]).0).collect();
    let mut cands: Vec<usize> = (0..n).filter(|&i| rest.keep[i] >= top).collect();
    cands.sort_by(|&a, &b| (energy[b] + 1e-3 * hash(b as u32)).total_cmp(&(energy[a] + 1e-3 * hash(a as u32))));
    let mut out: Vec<KnotAt> = vec![];
    let big = fx.knot_r * s;
    let up = if hash(c.seed ^ 0x7F4A_7C15) < 0.5 { -1.0 } else { 1.0 };
    for i in cands {
        if out.len() >= knot_count(len / s) { break; }
        if !out.iter().all(|kn| d2(rest.pts[i], rest.pts[kn.at]) >= (80.0 * s).powi(2)) { continue; }
        let rank = out.len();
        let fall = [1.0, fx.knot_fall, fx.knot_fall.powf(1.75)][rank.min(2)];
        let hn = hash(c.seed ^ (i as u32).wrapping_mul(0x632B_E5AB));
        let pts = (knots(fx.knot, energy[i], k) as f32 * (0.64 + 0.36 * hn)).round() as usize;
        if pts < MIN_KNOT { continue; }
        let side = if rank.is_multiple_of(2) { up } else { -up };
        let off = [(hash(c.seed ^ 0x1D8E_4E27 ^ i as u32) - 0.5) * 0.4 * big, side * (0.5 + 0.4 * hn) * big];
        out.push(KnotAt { at: i, r: big * fall, n: pts, off });
    }
    out
}

fn in_box(p: [f32; 2], boxes: &[[f32; 4]]) -> bool { boxes.iter().any(|b| p[0] >= b[0] && p[1] >= b[1] && p[0] < b[2] && p[1] < b[3]) }
/// Sixteen ways, 22.5° apart from +x: what a tail chooses between, so a way clear of text to read is
/// found between two that pass close to it.
fn ways16() -> [[f32; 2]; 16] { std::array::from_fn(|i| { let a = i as f32 * std::f32::consts::PI / 8.0; [a.cos(), a.sin()] }) }

/// A strand from `from` along `dir`, `len` long at full looseness: a long, slowly bending curve. Its bends
/// (`turns` of them, each 12–25°, at about even places along it, one way then the other) are the control
/// points of a `curve`, and a hand's wavering runs across it (0.6–1.5 % of its length, rising and
/// falling two or three times); it never strays more than 25° from `dir`. 24–32 points after `from`,
/// evenly along it (more of them the more bends it has), every one scaled toward `from` by `k`.
pub fn strand_path(from: [f32; 2], dir: [f32; 2], len: f32, k: f32, seed: u32, t: f32, turns: usize) -> Vec<[f32; 2]> {
    let turns = turns.min(8);
    let (kt, ft) = (t.floor() as i32, smooth(0.0, 1.0, t - t.floor()));
    let h = |m: u32, salt: u32, key: i32| hash(seed.wrapping_mul(0x2C1B_3C6D) ^ m.wrapping_mul(0x297A_2D39) ^ salt ^ (key as u32).wrapping_mul(0x7FEB_352D));
    let eased = |m: u32, salt: u32| h(m, salt, kt) + (h(m, salt, kt + 1) - h(m, salt, kt)) * ft;
    // The wavering: how far (a share of the length), how many rises and falls, from where.
    let (amp, bumps) = (0.006 + 0.009 * eased(0, 0x3d4f), 2.0 + eased(0, 0x5e2a));
    let lean = (std::f32::consts::PI * amp * bumps).atan().to_degrees();
    // The bends' headings stay within 25° of `dir` less what the wavering adds.
    let most = (25.0 - lean - 0.5).to_radians();
    let first = if h(0, 0x1b56, 0) < 0.5 { -1.0 } else { 1.0 };
    let (mut heading, mut bends) = (0.0f32, Vec::with_capacity(turns));
    for m in 1..=turns as u32 {
        let f = (m as f32 + 0.6 * (eased(m, 0x5bd1) - 0.5)) / (turns as f32 + 1.0);
        // One way then the other.
        let side = if heading == 0.0 { first } else { -heading.signum() };
        // Many bends are each gentler, so the curve turns as slowly however many it has.
        let a = side * (12.0 + 13.0 * eased(m, 0x2f6b)).min(100.0 / (turns as f32 + 1.0)).to_radians();
        let a = (heading + a).clamp(-most, most) - heading;
        heading += a;
        bends.push((f.clamp(0.05, 0.95), a));
    }
    // The bent line's vertices: the curve's control points.
    let mut line = vec![[0.0f32, 0.0f32]];
    let (mut d, mut at, mut done) = (dir, [0.0f32, 0.0f32], 0.0f32);
    for &(f, a) in &bends {
        let l = (f - done).max(0.0) * len;
        at = [at[0] + d[0] * l, at[1] + d[1] * l];
        line.push(at);
        d = [d[0] * a.cos() - d[1] * a.sin(), d[0] * a.sin() + d[1] * a.cos()];
        done = f.max(done);
    }
    let l = (1.0 - done).max(0.0) * len;
    line.push([at[0] + d[0] * l, at[1] + d[1] * l]);
    let pad = padded(&line);
    let spans = pad.len() - 3;
    let fine: Vec<[f32; 2]> = (0..=spans * 64).map(|j| spline_at(&pad, j as f32 / 64.0)).collect();
    let total = length(&fine).max(1e-3);
    // Evenly along it (more points the more it bends), each point moved across `dir` by the wavering.
    let m = (24 + turns + (3.0 * h(0, 0x71c3, 0)).floor() as usize).min(32);
    let across = [-dir[1], dir[0]];
    let mut cum = 0.0f32;
    let mut j = 1;
    let mut out = Vec::with_capacity(m);
    for i in 1..fine.len() {
        let l = d2(fine[i - 1], fine[i]).sqrt();
        while j <= m && cum + l >= total * j as f32 / m as f32 - 1e-4 {
            let sv = total * j as f32 / m as f32;
            let p = lerp2(fine[i - 1], fine[i], ((sv - cum) / l.max(1e-6)).clamp(0.0, 1.0));
            let w = amp * len * (std::f32::consts::PI * bumps * sv / total).sin();
            out.push([from[0] + k * (p[0] + across[0] * w), from[1] + k * (p[1] + across[1] * w)]);
            j += 1;
        }
        cum += l;
    }
    while out.len() < m { let p = fine[fine.len() - 1]; out.push([from[0] + k * p[0], from[1] + k * p[1]]); }
    out
}
/// A tail from `from` (drawn) along `dir`, at least `len` long and long enough to reach the scope's
/// edge, cut where it leaves the scope (the last piece on its edge) and before a box of text to read.
fn tail_from(from: [f32; 2], dir: [f32; 2], len: f32, k: f32, seed: u32, turns: usize, c: &Ctx) -> Vec<[f32; 2]> {
    let sc = c.scope;
    let edge = |p: f32, d: f32, lo: f32, hi: f32| if d > 1e-4 { (hi - p) / d } else if d < -1e-4 { (lo - p) / d } else { f32::INFINITY };
    // Just long enough to reach the edge however it bends (it strays at most 25° from `dir`), so its
    // bends fall inside the scope and the part that shows is a curve, not the straight first stretch
    // of a longer one.
    let to_edge = edge(from[0], dir[0], sc[0], sc[2]).min(edge(from[1], dir[1], sc[1], sc[3])) * 1.12 + 8.0;
    let len = if to_edge.is_finite() { to_edge } else { len };
    let out = |p: [f32; 2]| p[0] < sc[0] || p[1] < sc[1] || p[0] > sc[2] || p[1] > sc[3];
    let mut pts = vec![];
    let mut last = from;
    if in_box(from, c.calm) { return pts; }
    for p in strand_path(from, dir, len, k, seed, c.t * 0.5, turns) {
        // Stopped just before a box of text to read where the piece would enter it (not only where a
        // point lands in it, so a piece never jumps over a line of text).
        let enter = c.calm.iter().filter_map(|b| span_in(last, p, b).map(|t| t.0)).fold(f32::INFINITY, f32::min);
        if enter.is_finite() {
            let t = (enter - 1.0 / d2(last, p).sqrt().max(1.0)).max(0.0);
            if t > 0.0 { pts.push(lerp2(last, p, t)); }
            break;
        }
        if out(p) {
            let cross = |lo: f32, hi: f32, a: f32, b: f32| if b < lo { (lo - a) / (b - a) } else if b > hi { (hi - a) / (b - a) } else { 1.0 };
            let t = cross(sc[0], sc[2], last[0], p[0]).min(cross(sc[1], sc[3], last[1], p[1])).clamp(0.0, 1.0);
            if !out(last) { pts.push(lerp2(last, p, t)); }
            break;
        }
        pts.push(p);
        last = p;
    }
    pts
}

/// The thread for a rest path: at most `max` pieces, one connected line. When the full line would need
/// more, the curves are sampled more coarsely first (30° a piece), then the knots lose control points
/// (a knot left with fewer than `MIN_KNOT` is not drawn: never a star), then every n-th vertex of the line
/// is kept; pieces are never dropped at random. No piece enters a box of `Ctx::calm` grown by 4 px.
/// `field(x, y)` gives the map's energy and flow at a device point.
pub fn thread(rest: &Rest, fx: &StrandFx, c: &Ctx, field: &dyn Fn(f32, f32) -> (f32, [f32; 2]), max: usize) -> Vec<Seg> {
    let k = c.k.clamp(0.0, 1.0);
    let n = rest.pts.len();
    if k <= 0.0 || n < 2 || max == 0 || rest.keep.len() != n { return vec![]; }
    let s = c.scale;
    let (mut at, sl) = pose(rest, fx, c, field);
    // A vertex that would sit in text to read is put on the nearest edge of its box.
    for p in &mut at { *p = off_boxes(*p, c.calm, 0.0); }
    let inside = |p: [f32; 2]| [p[0].clamp(c.scope[0], c.scope[2]), p[1].clamp(c.scope[1], c.scope[3])];
    let plan = knot_plan(rest, fx, c, field);
    // Each knot's centre and radius, kept clear of text to read: moved away from it by up to three
    // radii, then made smaller; too small, and it is not drawn.
    let placed: Vec<Option<([f32; 2], f32)>> = plan.iter().map(|p| {
        let centre = inside([at[p.at][0] + p.off[0] * k, at[p.at][1] + p.off[1] * k]);
        clear_of(centre, p.r * k, c.calm, 6.0 * s, 12.0 * s, &inside)
    }).collect();
    let turns = fx.tail_turns.round().clamp(1.0, 6.0) as usize;
    let ends = [0, n - 1];
    let kept: Vec<usize> = (0..n).filter(|&i| sl[i] < 1.0 || ends.contains(&i) || plan.iter().any(|a| a.at == i)).collect();
    // The tails (worked out once: the budget does not change them).
    let (head, tail) = if fx.tail > 0.0 { tails(rest, &at, fx, c, field, turns) } else { (vec![], vec![]) };
    let esc = fx.escape.round().max(0.0) as usize;
    let blend = (1.0 - fx.angular.clamp(0.0, 1.0)) * smooth(0.3, 0.7, k);
    let build = |every: usize, counts: &[usize], turn: f32| -> Vec<([f32; 2], Kind)> {
        let mut pts: Vec<([f32; 2], Kind)> = head.iter().map(|p| (*p, Kind::Tail)).collect();
        let mut last = 0;
        for (m, &i) in kept.iter().enumerate() {
            let knot = plan.iter().position(|p| p.at == i);
            if m % every != 0 && i != n - 1 && knot.is_none() { continue; }
            // A bridge that the dropped vertices skipped still arrives as a bridge.
            let bridged = i > 0 && (last + 1..=i).any(|j| rest.bridge[j]);
            let kind = if m == 0 { if head.is_empty() { Kind::Line } else { Kind::Tail } } else if bridged { Kind::Bridge } else { Kind::Line };
            pts.push((at[i], kind));
            last = i;
            let Some(j) = knot else { continue };
            let (Some((centre, r)), true) = (placed[j], counts[j] >= MIN_KNOT) else { continue };
            let mut ctrl = vec![at[i]];
            ctrl.extend(tangle_ctrl(centre, r, counts[j], esc, c.seed ^ (i as u32).wrapping_mul(0x9E37), c.t));
            ctrl.push(at[i]);
            for q in curve(&ctrl, turn).into_iter().skip(1) { pts.push((inside(q), Kind::Knot)); }
        }
        // The piece from the start tail into the line is the tail's (set above); out from the end.
        pts.extend(tail.iter().map(|p| (*p, Kind::Tail)));
        pts.dedup_by(|b, a| d2(a.0, b.0) <= 1e-6);
        // Past a third loose, the whole line bends into a curve, by `1 − angular`.
        if blend > 0.0 && pts.len() > 2 {
            // Its corners are what it bends through: the vertices still sliding onto their chords lie
            // nearly on them, and a curve through those would keep every corner. So the line is first
            // simplified (to 2 px as it bends), then curved through what is left.
            let line: Vec<[f32; 2]> = pts.iter().map(|p| p.0).collect();
            let keep = simplified(&line, 2.0 * s * smooth(0.3, 0.7, k));
            let corners: Vec<[f32; 2]> = keep.iter().map(|&j| line[j]).collect();
            pts = bend_line(&corners, blend, turn).into_iter().map(|(q, j)| (q, pts[keep[j]].1)).collect();
        }
        let mut pts = detour(pts, c.calm, 5.0 * s);
        pts.dedup_by(|b, a| d2(a.0, b.0) <= 1e-6);
        pts
    };
    // Within the budget: coarser curves, then fewer knot points, then every n-th vertex.
    let mut counts: Vec<usize> = plan.iter().map(|p| p.n).collect();
    let (mut every, mut turn) = (1, 15.0);
    let pts = loop {
        let pts = build(every, &counts, turn);
        if pts.len().saturating_sub(1) <= max { break pts; }
        if turn < 30.0 { turn = 30.0; continue; }
        if let Some(j) = (0..counts.len()).filter(|&j| counts[j] >= MIN_KNOT).max_by_key(|&j| counts[j]) {
            counts[j] -= (counts[j] / 6).max(1);
            if counts[j] < MIN_KNOT { counts[j] = 0; }
            continue;
        }
        // At the last, the line is cut short (still one piece).
        if every >= kept.len() { break pts; }
        every += (every / 2).max(1);
    };
    pts.windows(2).map(|w| Seg { a: w[0].0, b: w[1].0, kind: w[1].1 }).take(max).collect()
}

/// The two tails of a thread whose line is at `at`: in from the scope's edge to the line's start
/// (ending just before it) and out from its end. Out from the end along the flow as far as it is free
/// (or the longest free way), in from the start along the way that gets farthest other than the end's;
/// neither back across the middle of its own word, and a way that would pass within 24 px of text to read
/// is worth a third as much; a way out to the scope's edge clear of text to read is taken over any that
/// is not. Directions are read at the rest points, so every tail point moves in
/// proportion to `k` and they draw back in a straight way.
fn tails(rest: &Rest, at: &[[f32; 2]], fx: &StrandFx, c: &Ctx, field: &dyn Fn(f32, f32) -> (f32, [f32; 2]), turns: usize) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
    let (k, s, n) = (c.k.clamp(0.0, 1.0), c.scale, rest.pts.len());
    let sc = c.scope;
    let full = (fx.tail * s).max(0.6 * (sc[2] - sc[0]).max(sc[3] - sc[1]));
    // How far a whole tail from a rest point would get that way, a third as far near text to read; a
    // way that reaches the scope's edge clear of text to read beats every way that is not, however short
    // (a tail is a way out to the edge, not a line drawn past the text).
    let on_edge = |p: [f32; 2]| p[0] <= sc[0] + 1.0 || p[1] <= sc[1] + 1.0 || p[0] >= sc[2] - 1.0 || p[1] >= sc[3] - 1.0;
    let reach = |from: [f32; 2], dir: [f32; 2], seed: u32| {
        let t = tail_from(from, dir, full, 1.0, seed, turns, c);
        let far = t.last().map_or(0.0, |p| d2(*p, from).sqrt());
        let close = t.iter().any(|p| c.calm.iter().any(|b| box_dist(*p, b) < 24.0 * s));
        let out = !close && t.last().is_some_and(|p| on_edge(*p));
        (if close { far * 0.3 } else { far }) + if out { 1e5 } else { 0.0 }
    };
    // Out of the word, not back across it: a way that does not cross the middle half of the line's box.
    let bb = rest.pts.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
    let (qw, qh) = ((bb[2] - bb[0]) * 0.25, (bb[3] - bb[1]) * 0.25);
    let core = [bb[0] + qw, bb[1] + qh, bb[2] - qw, bb[3] - qh];
    let span = (bb[2] - bb[0]) + (bb[3] - bb[1]);
    let away = |from: [f32; 2], d: &[f32; 2]| span_in(from, [from[0] + d[0] * span, from[1] + d[1] * span], &core).is_none();
    let best = |from: [f32; 2], skip: Option<[f32; 2]>, seed: u32| -> [f32; 2] {
        let ok = |d: &[f32; 2]| skip.is_none_or(|e| d[0] * e[0] + d[1] * e[1] < 0.9);
        let pick = |out: bool| ways16().iter().filter(|d| ok(d) && (!out || away(from, d))).map(|d| (*d, reach(from, *d, seed))).max_by(|a, b| a.1.total_cmp(&b.1)).map(|b| b.0);
        pick(true).or_else(|| pick(false)).unwrap_or([1.0, 0.0])
    };
    let (e0, e1) = (rest.pts[0], rest.pts[n - 1]);
    let (_, fl) = field(e1[0], e1[1]);
    let fl_len = (fl[0] * fl[0] + fl[1] * fl[1]).sqrt();
    // With a flow, the way that goes farthest weighed by how close it is to the flow (the flow itself
    // and 30° and 60° either side of it among them); without, the farthest of sixteen.
    let end_dir = if fl_len > 0.05 {
        let f = [fl[0] / fl_len, fl[1] / fl_len];
        let turn = |a: f32| [f[0] * a.cos() - f[1] * a.sin(), f[0] * a.sin() + f[1] * a.cos()];
        let ways: Vec<[f32; 2]> = [0.0f32, 30.0, -30.0, 60.0, -60.0].iter().map(|d| turn(d.to_radians())).chain(ways16()).collect();
        let score = |d: &[f32; 2]| reach(e1, *d, c.seed ^ 0x77) * (0.4 + 0.6 * (d[0] * f[0] + d[1] * f[1]).max(0.0));
        let pick = |out: bool| ways.iter().filter(|d| !out || away(e1, d)).map(|d| (*d, score(d))).max_by(|a, b| a.1.total_cmp(&b.1)).map(|b| b.0);
        pick(true).or_else(|| pick(false)).unwrap_or(f)
    } else { best(e1, None, c.seed ^ 0x77) };
    let start_dir = best(e0, Some(end_dir), c.seed ^ 0x51);
    let head: Vec<[f32; 2]> = tail_from(at[0], start_dir, full, k, c.seed ^ 0x51, turns, c).into_iter().rev().collect();
    let end = tail_from(at[n - 1], end_dir, full, k, c.seed ^ 0x77, turns, c);
    (head, end)
}

/// How far `p` is from box `b` (0 inside it).
fn box_dist(p: [f32; 2], b: &[f32; 4]) -> f32 {
    let (dx, dy) = ((b[0] - p[0]).max(p[0] - b[2]).max(0.0), (b[1] - p[1]).max(p[1] - b[3]).max(0.0));
    (dx * dx + dy * dy).sqrt()
}
fn grow(b: &[f32; 4], m: f32) -> [f32; 4] { [b[0] - m, b[1] - m, b[2] + m, b[3] + m] }
/// `p`, or where it is inside a box grown by `m`, the nearest point on that box's edge.
fn off_boxes(p: [f32; 2], boxes: &[[f32; 4]], m: f32) -> [f32; 2] {
    let mut p = p;
    for b in boxes.iter().map(|b| grow(b, m)) {
        if p[0] > b[0] && p[1] > b[1] && p[0] < b[2] && p[1] < b[3] {
            let d = [p[0] - b[0], b[2] - p[0], p[1] - b[1], b[3] - p[1]];
            let i = (0..4).min_by(|&x, &y| d[x].total_cmp(&d[y])).unwrap_or(0);
            p = match i { 0 => [b[0], p[1]], 1 => [b[2], p[1]], 2 => [p[0], b[1]], _ => [p[0], b[3]] };
        }
    }
    p
}
/// A knot's centre and radius clear of the boxes (grown by `pad`): moved away from the nearest one it
/// would reach (its circle is `2.2 r`) by up to `3 r` in all, then its radius made small enough; `None`
/// when that is less than `least`.
fn clear_of(centre: [f32; 2], r: f32, boxes: &[[f32; 4]], pad: f32, least: f32, inside: &dyn Fn([f32; 2]) -> [f32; 2]) -> Option<([f32; 2], f32)> {
    let grown: Vec<[f32; 4]> = boxes.iter().map(|b| grow(b, pad)).collect();
    let (mut c, mut moved) = (centre, 0.0f32);
    for _ in 0..12 {
        let Some(b) = grown.iter().filter(|b| box_dist(c, b) < 2.2 * r).min_by(|x, y| box_dist(c, x).total_cmp(&box_dist(c, y))) else { break };
        let near = [c[0].clamp(b[0], b[2]), c[1].clamp(b[1], b[3])];
        let mut d = [c[0] - near[0], c[1] - near[1]];
        if d[0].abs() + d[1].abs() < 1e-3 {
            // Inside it: out the nearest side.
            let e = [c[0] - b[0], b[2] - c[0], c[1] - b[1], b[3] - c[1]];
            d = [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]][(0..4).min_by(|&x, &y| e[x].total_cmp(&e[y])).unwrap_or(2)];
            let l = e.iter().copied().fold(f32::INFINITY, f32::min);
            d = [d[0] * (1.0 + l), d[1] * (1.0 + l)];
        }
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
        let step = (2.2 * r - box_dist(c, b) + 0.5).min(3.0 * r - moved);
        if step <= 0.0 { break; }
        let next = inside([c[0] + d[0] / l * step, c[1] + d[1] / l * step]);
        moved += step;
        if d2(next, c) < 1e-6 { break; }
        c = next;
    }
    let room = grown.iter().map(|b| box_dist(c, b)).fold(f32::INFINITY, f32::min) / 2.2;
    let r = r.min(room);
    (r >= least).then_some((c, r))
}
/// The line `pts` kept out of the boxes grown by `m`: a point inside one goes to its nearest edge, and a
/// piece that would cross one goes round its edge instead, the shorter way.
fn detour(pts: Vec<([f32; 2], Kind)>, boxes: &[[f32; 4]], m: f32) -> Vec<([f32; 2], Kind)> {
    if boxes.is_empty() || pts.len() < 2 { return pts; }
    let round: Vec<[f32; 4]> = boxes.iter().map(|b| grow(b, m)).collect();
    let test: Vec<[f32; 4]> = boxes.iter().map(|b| grow(b, m - 0.5)).collect();
    let mut pts: Vec<([f32; 2], Kind)> = pts.into_iter().map(|(p, kd)| (off_boxes(p, &round, 0.0), kd)).collect();
    for _ in 0..4 {
        let mut changed = false;
        let mut out = Vec::with_capacity(pts.len());
        out.push(pts[0]);
        for w in pts.windows(2) {
            let (a, (b, kb)) = (w[0].0, w[1]);
            if let Some(j) = (0..test.len()).find(|&j| span_in(a, b, &test[j]).is_some()) {
                for q in around(a, b, &round[j]) { out.push((q, kb)); }
                changed = true;
            }
            out.push((b, kb));
        }
        out.dedup_by(|b, a| d2(a.0, b.0) <= 1e-6);
        pts = out;
        if !changed { break; }
    }
    pts
}
/// The way round box `b` from `a` to `b2` (both outside it): the nearest point of its edge to each, and
/// the corners between, the shorter way.
fn around(a: [f32; 2], b2: [f32; 2], b: &[f32; 4]) -> Vec<[f32; 2]> {
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    let per = 2.0 * (w + h);
    let onto = |p: [f32; 2]| [p[0].clamp(b[0], b[2]), p[1].clamp(b[1], b[3])];
    // Where on the edge, clockwise from the top left corner.
    let at = |p: [f32; 2]| {
        let d = [(p[1] - b[1]).abs(), (p[0] - b[2]).abs(), (p[1] - b[3]).abs(), (p[0] - b[0]).abs()];
        match (0..4).min_by(|&x, &y| d[x].total_cmp(&d[y])).unwrap_or(0) { 0 => p[0] - b[0], 1 => w + p[1] - b[1], 2 => w + h + b[2] - p[0], _ => 2.0 * w + h + b[3] - p[1] }
    };
    let (pa, pb) = (onto(a), onto(b2));
    let (ua, ub) = (at(pa), at(pb));
    let fwd = (ub - ua).rem_euclid(per);
    let corners = [(0.0, [b[0], b[1]]), (w, [b[2], b[1]]), (w + h, [b[2], b[3]]), (2.0 * w + h, [b[0], b[3]])];
    let mut way: Vec<(f32, [f32; 2])> = if fwd <= per * 0.5 {
        corners.iter().map(|&(u, q)| ((u - ua).rem_euclid(per), q)).filter(|(d, _)| *d > 1e-4 && *d < fwd - 1e-4).collect()
    } else {
        corners.iter().map(|&(u, q)| ((ua - u).rem_euclid(per), q)).filter(|(d, _)| *d > 1e-4 && *d < per - fwd - 1e-4).collect()
    };
    way.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = vec![pa];
    out.extend(way.into_iter().map(|w| w.1));
    out.push(pb);
    out
}

/// A drawn tangle for a page to show as content (DESIGN.md §4.4): a knot of radius `r` and a long
/// strand through it, `above` long up from the knot and `below` long down from it, made by the same knot
/// and tails as the thread at its default shape (`ThreadShape`), with a fixed seed. Points about the knot's
/// centre (logical px). The knot, its escapes aside, is within `2.5 r` of the centre.
pub fn figure(seed: u32, r: f32, above: f32, below: f32) -> Vec<[f32; 2]> {
    let sh = ThreadShape::default();
    let turns = sh.tail_turns as usize;
    let mut up = strand_path([0.0, 0.0], [0.0, -1.0], above, 1.0, seed ^ 0x0A, 0.0, turns);
    up.reverse();
    let mut pts = up;
    let mut ctrl = vec![[0.0, 0.0]];
    ctrl.extend(tangle_ctrl([0.0, 0.0], r, 36, sh.escape as usize, seed, 0.0));
    ctrl.push([0.0, 0.0]);
    pts.extend(curve(&ctrl, 15.0));
    pts.extend(strand_path([0.0, 0.0], [0.0, 1.0], below, 1.0, seed ^ 0x0B, 0.0, turns));
    pts
}

/// The span of `t` (0..1) along the piece from `a` to `b` inside a box `[x0, y0, x1, y1]` (Liang–Barsky).
fn span_in(a: [f32; 2], b: [f32; 2], bx: &[f32; 4]) -> Option<(f32, f32)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d[0], a[0] - bx[0]), (d[0], bx[2] - a[0]), (-d[1], a[1] - bx[1]), (d[1], bx[3] - a[1])] {
        if p == 0.0 { if q < 0.0 { return None; } continue; }
        let r = q / p;
        if p < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
    }
    (t0 < t1).then_some((t0, t1))
}
/// The parts of the piece from `a` to `b` outside every box (device px, `[x0, y0, x1, y1]`): where a
/// line passes a picture it goes behind it, since a picture is drawn from its own colours only.
pub fn outside(a: [f32; 2], b: [f32; 2], boxes: &[[f32; 4]]) -> Vec<([f32; 2], [f32; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut cut: Vec<(f32, f32)> = boxes.iter().filter_map(|bx| span_in(a, b, bx)).collect();
    if cut.is_empty() { return vec![(a, b)]; }
    cut.sort_by(|x, y| x.0.total_cmp(&y.0));
    let at = |t: f32| [a[0] + d[0] * t, a[1] + d[1] * t];
    let mut out = vec![];
    let mut from = 0.0f32;
    for (t0, t1) in cut {
        if t0 > from { out.push((at(from), at(t0))); }
        from = from.max(t1);
    }
    if from < 1.0 { out.push((at(from), b)); }
    out
}

/// A long path drawn by progress (DESIGN.md §1.30): points in the element's box (logical px), how much is
/// drawn (0..1 of its length), the line's width, dashes (`dash` long, `gap` apart, by arc length from the
/// start; `dash` 0 is solid), how far the map's flow pushes it (× `drift`), and how hard its head stirs
/// the map as it advances (0: not at all).
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The path, logical px in the element's box.
    pub points: std::rc::Rc<Vec<[f32; 2]>>,
    /// How much of it is drawn (0..1 of its length).
    pub progress: f32,
    /// The line's width (logical px).
    pub weight: f32,
    /// Dash length (0: solid).
    pub dash: f32,
    /// Gap between dashes.
    pub gap: f32,
    /// How far the map's flow pushes it (× the map's drift).
    pub pull: f32,
    /// How hard its head stirs the map as it advances.
    pub stir: f32,
    /// Its paint.
    pub paint: crate::el::Paint,
}
impl Default for Trace {
    fn default() -> Self { Self { points: std::rc::Rc::new(vec![]), progress: 1.0, weight: 1.0, dash: 0.0, gap: 0.0, pull: 1.0, stir: 0.0, paint: crate::el::Paint::INK } }
}
/// The path's total length.
pub fn length(p: &[[f32; 2]]) -> f32 { p.windows(2).map(|w| d2(w[0], w[1]).sqrt()).sum() }
/// The point at arc length `s` along `p`.
pub fn at_length(p: &[[f32; 2]], s: f32) -> [f32; 2] {
    let mut left = s.max(0.0);
    for w in p.windows(2) {
        let l = d2(w[0], w[1]).sqrt();
        if left <= l && l > 0.0 { let t = left / l; return [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t]; }
        left -= l;
    }
    p.last().copied().unwrap_or([0.0; 2])
}
/// The drawn part of `p` at `progress` as dashes: pieces of the polyline over arc lengths
/// `[n·(dash + gap), n·(dash + gap) + dash]`, the last one cut where the drawing ends; `dash` 0 is one
/// solid piece. Dashes are counted from the start, so drawing on never moves the ones already drawn.
pub fn dashes(p: &[[f32; 2]], progress: f32, dash: f32, gap: f32) -> Vec<Vec<[f32; 2]>> {
    if p.len() < 2 { return vec![]; }
    let total = length(p);
    let drawn = total * progress.clamp(0.0, 1.0);
    if drawn <= 0.0 { return vec![]; }
    let spans: Vec<(f32, f32)> = if dash <= 0.0 { vec![(0.0, drawn)] } else {
        let period = dash + gap.max(0.0);
        (0..).map(|n| n as f32 * period).take_while(|s| *s < drawn).map(|s| (s, (s + dash).min(drawn))).collect()
    };
    // Walk the polyline once, cutting each span out of it.
    let mut out = vec![];
    let mut cum = vec![0.0f32];
    for w in p.windows(2) { cum.push(cum.last().expect("a length") + d2(w[0], w[1]).sqrt()); }
    let mut seg = 0;
    for (a, b) in spans {
        while seg + 1 < cum.len() - 1 && cum[seg + 1] <= a { seg += 1; }
        let mut piece = vec![at_length(p, a)];
        let mut j = seg + 1;
        while j < cum.len() - 1 && cum[j] < b { piece.push(p[j]); j += 1; }
        piece.push(at_length(p, b));
        out.push(piece);
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    fn square(x: f32, y: f32, w: f32) -> Vec<[f32; 2]> { vec![[x, y], [x + w, y], [x + w, y + w], [x, y + w], [x, y]] }
    fn ctx(k: f32) -> Ctx<'static> { Ctx { k, t: 1.0, seed: 3, scale: 1.0, drift: 6.0, scope: [-1e4, -1e4, 1e4, 1e4], calm: &[], lh: 60.0 } }
    fn still(_: f32, _: f32) -> (f32, [f32; 2]) { (0.0, [0.0; 2]) }
    /// A wavy ring: many vertices, most of them close to their chords.
    fn ring(cx: f32, cy: f32, r: f32, n: usize) -> Vec<[f32; 2]> {
        let mut p: Vec<[f32; 2]> = (0..n).map(|i| { let a = i as f32 / n as f32 * std::f32::consts::TAU; let rr = r * (1.0 + 0.08 * (7.0 * a).sin()); [cx + rr * a.cos(), cy + rr * a.sin()] }).collect();
        p.push(p[0]);
        p
    }
    fn bare() -> StrandFx { StrandFx { knot: 0.0, tail: 0.0, angular: 1.0, ..StrandFx::default() } }
    /// The places along a run of pieces where the way turns by more than 10°, and by how much (degrees).
    fn bends(p: &[([f32; 2], [f32; 2])]) -> Vec<f32> {
        p.windows(2).filter_map(|w| {
            let (u, v) = ([w[0].1[0] - w[0].0[0], w[0].1[1] - w[0].0[1]], [w[1].1[0] - w[1].0[0], w[1].1[1] - w[1].0[1]]);
            let (lu, lv) = ((u[0] * u[0] + u[1] * u[1]).sqrt(), (v[0] * v[0] + v[1] * v[1]).sqrt());
            if lu < 1e-3 || lv < 1e-3 { return None; }
            let a = ((u[0] * v[0] + u[1] * v[1]) / (lu * lv)).clamp(-1.0, 1.0).acos().to_degrees();
            (a > 10.0).then_some(a)
        }).collect()
    }

    #[test]
    fn nothing_at_rest_and_on_the_outline_up_to_a_third() {
        let r = join(&[square(10.0, 10.0, 40.0), ring(200.0, 100.0, 50.0, 90)]);
        let fx = StrandFx::default();
        assert!(thread(&r, &fx, &ctx(0.0), &still, 10_000).is_empty());
        for k in [0.001f32, 0.15, 0.3] {
            let segs = thread(&r, &bare(), &ctx(k), &|_, _| (1.0, [0.7, 0.2]), 10_000);
            assert!(!segs.is_empty());
            for s in &segs {
                for p in [s.a, s.b] {
                    let off = r.pts.iter().map(|q| d2(*q, p)).fold(f32::INFINITY, f32::min).sqrt();
                    assert!(off < 0.5, "k {k}: a vertex {off} px off the outline");
                }
            }
        }
    }

    #[test]
    fn one_line_with_a_bridge_between_two_loops() {
        // "回": an outer and an inner loop.
        let r = join(&[square(0.0, 0.0, 60.0), square(20.0, 20.0, 20.0)]);
        assert_eq!(r.loops, 2);
        assert_eq!(r.bridge.iter().filter(|b| **b).count(), 1);
        let segs = thread(&r, &StrandFx { tail: 0.0, ..StrandFx::default() }, &ctx(0.25), &still, 10_000);
        for w in segs.windows(2) { assert_eq!(w[0].b, w[1].a, "the thread is one line"); }
        assert!(segs.iter().any(|s| s.kind == Kind::Bridge));
        // The bridge goes to the inner loop's nearest vertex.
        let i = r.bridge.iter().position(|b| *b).expect("a bridge");
        assert_eq!(r.pts[i], [20.0, 20.0]);
    }

    #[test]
    fn the_ranks_nest_and_a_chord_comes_before_what_drops_onto_it() {
        let r = join(&[ring(0.0, 0.0, 80.0, 120), ring(220.0, 0.0, 60.0, 90)]);
        let mut seen = vec![false; r.pts.len()];
        for &i in &r.order {
            let (a, b) = r.chord[i];
            if a != b { assert!(seen[a] && seen[b], "vertex {i} before its chord"); assert!(r.keep[i] <= r.keep[a].min(r.keep[b]) + 1e-6); }
            seen[i] = true;
        }
        assert!(seen.iter().all(|s| *s), "every vertex ranked");
    }

    #[test]
    fn loosened_it_is_a_few_long_chords_that_move_without_jumps() {
        let r = join(&[ring(0.0, 0.0, 60.0, 160), ring(160.0, 0.0, 50.0, 140), ring(300.0, 10.0, 55.0, 150)]);
        let fx = StrandFx { reach: 40.0, ..bare() };
        let segs = thread(&r, &fx, &ctx(1.0), &still, 100_000);
        assert!(segs.len() * 100 <= r.pts.len() * 15, "{} pieces of {} vertices", segs.len(), r.pts.len());
        let mut lens: Vec<f32> = segs.iter().map(|s| d2(s.a, s.b).sqrt()).collect();
        lens.sort_by(f32::total_cmp);
        assert!(lens[lens.len() / 2] >= 24.0, "median piece {}", lens[lens.len() / 2]);
        // From 1 back to 0 in 30 frames, a vertex moves at most reach / 6 a frame.
        let mut before = pose(&r, &fx, &ctx(1.0), &still).0;
        for f in 1..=30 {
            let now = pose(&r, &fx, &ctx(1.0 - f as f32 / 30.0), &still).0;
            let most = before.iter().zip(&now).map(|(a, b)| d2(*a, *b).sqrt()).fold(0.0, f32::max);
            assert!(most <= fx.reach / 6.0, "frame {f}: a vertex jumped {most} px");
            before = now;
        }
    }

    #[test]
    fn knots_are_few_tangles_and_everything_stays_in_scope() {
        let r = join(&[box_outline([100.0, 100.0, 500.0, 60.0], 8.0, [0.0; 4], 2.0)]);
        let fx = StrandFx { angular: 1.0, tail: 0.0, ..StrandFx::default() };
        // Energy peaks at x 150 and 450.
        let hot = |x: f32, _: f32| ((1.0 - ((x - 150.0) / 120.0).powi(2)).max(0.0).max(1.0 - ((x - 450.0) / 120.0).powi(2)), [0.0, 0.0]);
        let segs = thread(&r, &fx, &ctx(1.0), &hot, 100_000);
        assert_eq!(knots(0.6, 1.0, 1.0), 36);
        assert_eq!(knots(0.6, 1.0, 0.1), 0);
        let knot: Vec<&Seg> = segs.iter().filter(|s| s.kind == Kind::Knot).collect();
        // Runs of knot pieces: one per knot.
        let runs = segs.windows(2).filter(|w| w[0].kind != Kind::Knot && w[1].kind == Kind::Knot).count();
        assert!((1..=3).contains(&runs), "{runs} knots");
        let pieces: Vec<([f32; 2], [f32; 2])> = knot.iter().map(|s| (s.a, s.b)).collect();
        assert!(crossings(&pieces).len() >= 30 * runs, "{} crossings in {runs} knots", crossings(&pieces).len());
        // Half loose, a knot is still at least half its radius across.
        let half = thread(&r, &fx, &ctx(0.6), &hot, 100_000);
        let kp: Vec<[f32; 2]> = half.iter().filter(|s| s.kind == Kind::Knot).map(|s| s.b).collect();
        let wide = kp.iter().flat_map(|a| kp.iter().map(move |b| d2(*a, *b).sqrt())).fold(0.0, f32::max);
        assert!(wide >= 0.8 * fx.knot_r, "half loose, the widest knot is {wide} across");
        // Everything in scope, tails too.
        let sc = [90.0, 60.0, 640.0, 200.0];
        let c = Ctx { scope: sc, ..ctx(1.0) };
        let segs = thread(&r, &StrandFx { tail: 400.0, ..fx }, &c, &|_, _| (1.0, [1.0, 0.0]), 100_000);
        for s in segs { for p in [s.a, s.b] { assert!(p[0] >= sc[0] - 1e-3 && p[1] >= sc[1] - 1e-3 && p[0] <= sc[2] + 1e-3 && p[1] <= sc[3] + 1e-3, "{p:?} outside the scope"); } }
    }

    /// Where a run of pieces crosses itself: for each crossing, the arc length along the run between the
    /// two passes.
    fn crossings(p: &[([f32; 2], [f32; 2])]) -> Vec<f32> {
        let o = |p: [f32; 2], q: [f32; 2], r: [f32; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        let cross = |a: &([f32; 2], [f32; 2]), b: &([f32; 2], [f32; 2])| o(a.0, a.1, b.0) * o(a.0, a.1, b.1) < 0.0 && o(b.0, b.1, a.0) * o(b.0, b.1, a.1) < 0.0;
        let mut cum = vec![0.0f32];
        for q in p { cum.push(cum.last().expect("a length") + d2(q.0, q.1).sqrt()); }
        let mut out = vec![];
        for i in 0..p.len() { for j in i + 2..p.len() { if cross(&p[i], &p[j]) { out.push(cum[j] - cum[i + 1]); } } }
        out
    }
    /// The share of neighbouring pieces that turn by more than `deg` degrees, and the largest turn.
    fn sharp(p: &[[f32; 2]], deg: f32) -> (f32, f32) {
        let turns: Vec<f32> = p.windows(3).map(|w| angle([w[1][0] - w[0][0], w[1][1] - w[0][1]], [w[2][0] - w[1][0], w[2][1] - w[1][1]])).collect();
        (turns.iter().filter(|a| **a > deg).count() as f32 / turns.len().max(1) as f32, turns.iter().copied().fold(0.0, f32::max))
    }

    #[test]
    fn a_curve_bends_smoothly_from_its_first_point_to_its_last() {
        let (mut over, mut all, mut most) = (0usize, 0usize, 0.0f32);
        for set in 0..100u32 {
            let n = 4 + (hash(set ^ 0x51) * 30.0) as usize;
            let ctrl: Vec<[f32; 2]> = (0..n as u32).map(|j| [400.0 * hash(set.wrapping_mul(977) ^ j.wrapping_mul(31)), 300.0 * hash(set.wrapping_mul(613) ^ j.wrapping_mul(57) ^ 0x9e37)]).collect();
            let c = curve(&ctrl, 15.0);
            assert_eq!(c[0], ctrl[0], "set {set}: it leaves the first control point");
            assert_eq!(*c.last().expect("points"), ctrl[n - 1], "set {set}: it arrives at the last");
            assert_eq!(c, curve(&ctrl, 15.0), "the same input, the same output");
            // Shorter than 1.5 px only where it turns sharply (the tip of a narrow loop).
            for (i, w) in c.windows(2).enumerate().skip(1).take(c.len().saturating_sub(3)) {
                if d2(w[0], w[1]).sqrt() >= 1.5 - 1e-3 { continue; }
                let turn = |a: usize| angle([c[a][0] - c[a - 1][0], c[a][1] - c[a - 1][1]], [c[a + 1][0] - c[a][0], c[a + 1][1] - c[a][1]]);
                assert!(turn(i).max(turn(i + 1)) > 2.0, "set {set}: a piece {} long where the curve is gentle", d2(w[0], w[1]).sqrt());
            }
            let turns = c.windows(3).map(|w| angle([w[1][0] - w[0][0], w[1][1] - w[0][1]], [w[2][0] - w[1][0], w[2][1] - w[1][1]]));
            for a in turns { all += 1; if a > 20.0 { over += 1; } most = most.max(a); }
        }
        eprintln!("curves: {over} of {all} turns over 20°, the largest {most:.1}°");
        assert!(over * 100 < all * 3, "{over} of {all} turns over 20°");
        assert!(most <= 60.0, "a turn of {most}°");
    }

    #[test]
    fn a_knot_is_a_tangle_of_loops_in_loops() {
        let r = 48.0;
        let mut fails = vec![];
        for seed in 0..10u32 {
            let p = tangle([0.0, 0.0], r, 36, 2, seed, 0.0);
            let (over, most) = sharp(&p, 20.0);
            let pieces: Vec<([f32; 2], [f32; 2])> = p.windows(2).map(|w| (w[0], w[1])).collect();
            let x = crossings(&pieces);
            let loops = x.iter().filter(|l| **l < 3.0 * r).count();
            // Dense in the middle, with a few long escapes out past 1.4 r and back.
            let total = length(&p);
            let mid = pieces.iter().map(|q| (lerp2(q.0, q.1, 0.5), d2(q.0, q.1).sqrt())).fold(([0.0f32; 2], 0.0f32), |(c, w), (m, l)| ([c[0] + m[0] * l, c[1] + m[1] * l], w + l));
            let g = [mid.0[0] / mid.1, mid.0[1] / mid.1];
            let near: f32 = pieces.iter().filter(|q| d2(lerp2(q.0, q.1, 0.5), g).sqrt() <= 0.5 * r).map(|q| d2(q.0, q.1).sqrt()).sum();
            let out: Vec<bool> = p.iter().map(|q| d2(*q, g).sqrt() > 1.4 * r).collect();
            let escapes = out.windows(2).filter(|w| w[0] && !w[1]).count();
            eprintln!("seed {seed}: {:.1} % of turns over 20° (largest {most:.0}°), {} crossings, {loops} small loops, {:.0} % within r / 2, {escapes} escapes", over * 100.0, x.len(), near / total * 100.0);
            if !(over < 0.03 && x.len() >= 40 && loops >= 6 && near >= 0.35 * total && escapes >= 2) { fails.push(seed); }
        }
        assert!(fails.is_empty(), "seeds {fails:?} are not tangles (see above)");
    }

    #[test]
    fn two_tails_run_to_the_scope_edge_and_stop_before_text() {
        // A title at the scope's top left, text to read at the right.
        let r = join(&[box_outline([20.0, 30.0, 280.0, 40.0], 8.0, [0.0; 4], 2.0)]);
        let sc = [0.0, 0.0, 712.0, 494.0];
        let calm = [[380.0, 150.0, 640.0, 260.0]];
        let c = Ctx { scope: sc, calm: &calm, ..ctx(1.0) };
        let segs = thread(&r, &StrandFx { knot: 0.0, angular: 1.0, ..StrandFx::default() }, &c, &still, 100_000);
        let first_line = segs.iter().position(|s| s.kind != Kind::Tail).expect("a line");
        let (head, rest) = segs.split_at(first_line);
        let tail: Vec<&Seg> = rest.iter().skip_while(|s| s.kind != Kind::Tail).collect();
        assert!(!head.is_empty() && !tail.is_empty());
        let (a, b) = (tail[0].a, tail[tail.len() - 1].b);
        let reach = d2(a, b).sqrt();
        assert!(reach >= 300.0, "the end's tail reaches {reach}");
        let edge = (b[0] - sc[0]).min(sc[2] - b[0]).min(b[1] - sc[1]).min(sc[3] - b[1]);
        assert!(edge < 8.0, "it stops {edge} px from the edge: {a:?} → {b:?}");
        // A long curve: no sharp turn anywhere along it.
        let turns = bends(&tail.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>());
        assert!(turns.is_empty(), "turns over 10°: {turns:?}");
        let start = d2(head[0].a, head[head.len() - 1].b).sqrt();
        assert!(start >= reach * 0.5, "the start's tail is {start} against {reach}");
        for s in &segs { for p in [s.a, s.b] { assert!(!in_box(p, &calm), "{p:?} in text to read"); } }
        // A third loose, the end's tail is still long.
        let third = thread(&r, &StrandFx { knot: 0.0, angular: 1.0, ..StrandFx::default() }, &Ctx { k: 0.33, ..c }, &still, 100_000);
        let t3: Vec<&Seg> = third.iter().rev().take_while(|s| s.kind == Kind::Tail).collect();
        let l3 = d2(t3[0].b, t3[t3.len() - 1].a).sqrt();
        assert!(l3 >= 100.0, "a third loose, the tail is {l3}");
    }

    #[test]
    fn the_budget_keeps_one_line_and_the_same_input_the_same_bytes() {
        let r = join(&[box_outline([0.0, 0.0, 300.0, 200.0], 12.0, [0.0; 4], 2.0)]);
        let hot = |x: f32, _: f32| (1.0, [(x * 0.01).sin(), 0.3]);
        let all = thread(&r, &StrandFx::default(), &ctx(0.6), &hot, usize::MAX);
        let few = thread(&r, &StrandFx::default(), &ctx(0.6), &hot, all.len() / 4);
        assert!(few.len() <= all.len() / 4 && !few.is_empty());
        for w in few.windows(2) { assert_eq!(w[0].b, w[1].a); }
        assert_eq!(thread(&r, &StrandFx::default(), &ctx(0.6), &hot, usize::MAX), all);
        assert!(all.iter().any(|s| s.kind == Kind::Tail));
    }

    #[test]
    fn the_wander_is_the_curl_of_the_field_noise() {
        for &(x, y) in &[(0.3f32, 0.7f32), (5.2, -3.1), (17.9, 4.4)] {
            assert!((noise_grad(x, y).0 - crate::field::noise(x, y)).abs() < 1e-5);
            let e = 1e-3;
            let g = noise_grad(x, y);
            let (dx, dy) = ((crate::field::noise(x + e, y) - crate::field::noise(x - e, y)) / (2.0 * e), (crate::field::noise(x, y + e) - crate::field::noise(x, y - e)) / (2.0 * e));
            assert!((g.1 - dx).abs() < 2e-2 && (g.2 - dy).abs() < 2e-2, "{g:?} vs {dx} {dy}");
        }
        let w = wander(120.0, 40.0, 2.0, 1, 1.0);
        assert!((w[0] * w[0] + w[1] * w[1]).sqrt() <= 1.0 + 1e-5);
        for y in [0.0f32, 0.3, 0.77, 1.0] { assert!((smooth(0.0, 1.0, unsmooth(y)) - y).abs() < 1e-4); }
    }

    #[test]
    fn a_figure_is_a_knot_and_a_long_strand_the_same_every_time() {
        let f = figure(7, 72.0, 175.0, 285.0);
        assert_eq!(f, figure(7, 72.0, 175.0, 285.0));
        let (lo, hi) = f.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        assert!(hi - lo >= 0.9 * 460.0, "{}", hi - lo);
        // Every piece joins the next without a sharp turn; the knot has many loops.
        let (over, _) = sharp(&f, 20.0);
        assert!(over < 0.03, "{:.1} % of turns over 20°", over * 100.0);
        let near: Vec<([f32; 2], [f32; 2])> = f.windows(2).filter(|w| d2(w[0], [0.0; 2]).sqrt() <= 2.5 * 72.0 && d2(w[1], [0.0; 2]).sqrt() <= 2.5 * 72.0).map(|w| (w[0], w[1])).collect();
        assert!(crossings(&near).len() >= 40, "{} crossings in the knot", crossings(&near).len());
    }

    #[test]
    fn a_piece_goes_behind_boxes() {
        let p = outside([0.0, 10.0], [100.0, 10.0], &[[20.0, 0.0, 40.0, 20.0], [30.0, 0.0, 50.0, 20.0], [70.0, 0.0, 80.0, 20.0]]);
        assert_eq!(p, vec![([0.0, 10.0], [20.0, 10.0]), ([50.0, 10.0], [70.0, 10.0]), ([80.0, 10.0], [100.0, 10.0])]);
        assert_eq!(outside([0.0, 30.0], [100.0, 30.0], &[[20.0, 0.0, 40.0, 20.0]]).len(), 1);
        assert!(outside([25.0, 5.0], [35.0, 15.0], &[[20.0, 0.0, 40.0, 20.0]]).is_empty());
    }

    #[test]
    fn dashes_are_counted_from_the_start() {
        let p = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]];
        let d = dashes(&p, 1.0, 10.0, 5.0);
        for piece in &d[..d.len() - 1] { assert!((length(piece) - 10.0).abs() < 1e-3, "{}", length(piece)); }
        assert!(length(d.last().expect("a dash")) <= 10.0 + 1e-3);
        let a = dashes(&p, 0.5, 10.0, 5.0);
        let b = dashes(&p, 0.6, 10.0, 5.0);
        for (x, y) in a.iter().take(a.len() - 1).zip(&b) { assert_eq!(x, y, "a drawn dash moved"); }
        let solid = dashes(&p, 0.7, 0.0, 0.0);
        assert_eq!(solid.len(), 1);
        assert!((length(&solid[0]) - 140.0).abs() < 1e-3);
        assert_eq!(solid[0][..2], p[..2]);
    }

    #[test]
    fn a_box_outline_closes_and_simplify_keeps_the_corners() {
        let o = box_outline([0.0, 0.0, 40.0, 20.0], 0.0, [0.0; 4], 4.0);
        assert_eq!(o.first(), o.last());
        let s = simplify(&o, 0.5);
        assert_eq!(s.len(), 5, "{s:?}");
    }

    #[test]
    fn one_large_lopsided_knot_and_smaller_ones_off_the_line() {
        // A long line with three hot places, fully loose at full energy.
        let r = join(&[box_outline([100.0, 100.0, 900.0, 60.0], 8.0, [0.0; 4], 2.0)]);
        let fx = StrandFx { angular: 1.0, tail: 0.0, ..StrandFx::default() };
        let hot = |_: f32, _: f32| (1.0, [0.0, 0.0]);
        let c = ctx(1.0);
        let plan = knot_plan(&r, &fx, &c, &hot);
        assert!(plan.len() >= 2, "{} knots", plan.len());
        let mut radii: Vec<f32> = plan.iter().map(|k| k.r).collect();
        radii.sort_by(|a, b| b.total_cmp(a));
        assert!(radii[0] / radii[1] >= 1.6, "the largest knot against the next: {radii:?}");
        assert!((radii[0] - fx.knot_r).abs() < 1e-3);
        // Off the line, above and below, not all the same distance.
        let offs: Vec<f32> = plan.iter().map(|k| k.off[1]).collect();
        assert!(offs.iter().all(|o| o.abs() >= 0.5 * fx.knot_r), "{offs:?}");
        assert!(offs.windows(2).any(|w| (w[0].abs() - w[1].abs()).abs() > 1.0), "every knot as far off the line: {offs:?}");
        // Within each knot: 14–22 points, chords of very different lengths, a few escaping.
        let (at, _) = pose(&r, &fx, &c, &hot);
        let segs = thread(&r, &fx, &c, &hot, 100_000);
        let mut runs: Vec<Vec<[f32; 2]>> = vec![];
        for w in segs.windows(2) {
            if w[1].kind == Kind::Knot { if w[0].kind != Kind::Knot { runs.push(vec![]); } runs.last_mut().expect("a run").push(w[1].a); }
        }
        assert_eq!(runs.len(), plan.len());
        for kn in &plan { assert!((MIN_KNOT..=36).contains(&kn.n), "{} control points", kn.n); }
        let _ = at;
    }

    #[test]
    fn a_tail_is_one_long_slow_curve() {
        let dir = [std::f32::consts::FRAC_1_SQRT_2; 2];
        for seed in 0..40u32 {
            for turns in [1usize, 2, 3, 6] {
                let p = strand_path([10.0, 20.0], dir, 600.0, 1.0, seed, seed as f32 * 0.37, turns);
                assert!((24..=32).contains(&p.len()), "{} points", p.len());
                let mut all = vec![[10.0, 20.0]];
                all.extend(&p);
                let (_, most) = sharp(&all, 0.0);
                assert!(most <= 8.0, "seed {seed}, {turns} turns: neighbouring pieces turn by {most:.1}°");
                for w in all.windows(2) {
                    let off = angle([w[1][0] - w[0][0], w[1][1] - w[0][1]], dir);
                    assert!(off <= 25.0, "seed {seed}, {turns} turns: a piece {off:.1}° off its way");
                }
                assert!(d2(all[0], *all.last().expect("points")).sqrt() >= 0.8 * 600.0, "it goes a long way");
            }
        }
        // And in a thread: each tail, as drawn without the line's curve.
        let r = join(&[box_outline([20.0, 30.0, 280.0, 40.0], 8.0, [0.0; 4], 2.0)]);
        let c = Ctx { scope: [0.0, 0.0, 900.0, 700.0], ..ctx(1.0) };
        let fx = StrandFx { knot: 0.0, angular: 1.0, ..StrandFx::default() };
        let segs = thread(&r, &fx, &c, &still, 100_000);
        let head: Vec<[f32; 2]> = segs.iter().take_while(|s| s.kind == Kind::Tail).map(|s| s.a).collect();
        let end: Vec<[f32; 2]> = segs.iter().rev().take_while(|s| s.kind == Kind::Tail).map(|s| s.b).collect();
        for t in [head, end] {
            let (_, most) = sharp(&t, 0.0);
            assert!(t.len() > 8 && most <= 8.0, "{} points, turning {most:.1}°", t.len());
        }
    }

    #[test]
    fn the_thread_hurries_through_its_in_between_looks() {
        // The ends as they were; monotone; 0.15–0.4 crossed while k goes 0.22 to 0.30.
        assert_eq!(shown(0.0), 0.0);
        assert!((shown(1.0) - 1.0).abs() < 1e-6);
        let mut last = -1.0;
        for i in 0..=1000 { let v = shown(i as f32 / 1000.0); assert!(v >= last); last = v; }
        assert!((shown(0.22) - 0.15).abs() < 1e-5 && (shown(0.30) - 0.4).abs() < 1e-5);
        // A held pose is never inside it.
        for i in 0..=100 { let v = held(shown(i as f32 / 100.0)); assert!(!(v > 0.15 && v < 0.4), "{v}"); }
    }
}
