//! `scribblr`'s thread (DESIGN.md §1.30), and long paths drawn by progress. No GPU here: the runtime
//! turns what this returns into round-capped bar instances (as `Mark::Line` is drawn), so the thread has a
//! true outline in the first pass like any shape.
//!
//! An element's thread at looseness `k`:
//!  1. its rest path: each glyph's true outline (the 0.5 iso-line of its coverage, sampled `grain` apart
//!     by `morph::cover_at` and simplified to half a pixel), shapes' outlines as they are;
//!  2. joined into one polyline in reading order, a straight bridge from where one loop closes to the
//!     nearest vertex of the next;
//!  3. every vertex moved by `k·reach·W(v, t) + k·pull·drift·flow(v)`, `W` the curl of a smooth noise (so
//!     neighbours bend together and the line goes in arcs);
//!  4. after every vertex `round(8·knot·e·k)` points wandering within `knot_r·k²` (knots come undone
//!     before the wander when it returns);
//!  5. a tail `tail·k` long along the flow where it ends (or toward the nearest window edge);
//!  6. corners cut by `1 − angular`.
//!
//! At `k = 0` there is nothing: no segment exists, so rest is plain's by construction.
use crate::field::hash;
use crate::scene::Instance;

/// The material's numbers, in `MAT_PARAMS` order (logical px where they are lengths).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrandFx { pub weight: f32, pub grain: f32, pub reach: f32, pub knot: f32, pub knot_r: f32, pub tail: f32, pub pull: f32, pub angular: f32 }
impl Default for StrandFx {
    fn default() -> Self { Self::of(crate::style::MaterialParams::defaults(crate::style::Material::Scribblr).0) }
}
impl StrandFx {
    pub fn of(p: [f32; 8]) -> Self { Self { weight: p[0], grain: p[1], reach: p[2], knot: p[3], knot_r: p[4], tail: p[5], pull: p[6], angular: p[7] } }
}

/// What a piece of the thread is: along an outline, a bridge between outlines, or the tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Line, Bridge, Tail }
/// One straight piece, device px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg { pub a: [f32; 2], pub b: [f32; 2], pub kind: Kind }

/// The rest path: one polyline through every outline; `bridge[i]` says the piece arriving at `pts[i]` is
/// a bridge. `loops` is how many outlines it went round.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rest { pub pts: Vec<[f32; 2]>, pub bridge: Vec<bool>, pub loops: usize }

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
    r
}
fn d2(a: [f32; 2], b: [f32; 2]) -> f32 { (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) }

/// Where and when a thread is worked out: its looseness, the (held) time, a seed, logical → device
/// scale, the map's `drift` (logical px), the scope box and calm boxes (device px), and the window.
/// `fine` weighs the wander's fine octave (1; text to read takes 0, so its outline moves a word at a
/// time and reads as one clean line beside the glyphs rather than a jittered copy).
pub struct Ctx<'a> { pub k: f32, pub t: f32, pub seed: u32, pub scale: f32, pub drift: f32, pub scope: [f32; 4], pub calm: &'a [[f32; 4]], pub window: [f32; 4], pub fine: f32 }

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
/// The wander: the curl of a smooth noise in two octaves (features 60 and 18 logical px apart,
/// drifting with `t` as `field::noise_t` does), at most 1 long. The coarse one bends whole words, the
/// fine one each stroke.
pub fn wander(x: f32, y: f32, t: f32, seed: u32, scale: f32, fine: f32) -> [f32; 2] {
    let o = hash(seed) * 91.0;
    let (kt, ft) = { let tt = t * 0.5; (tt.floor(), tt - tt.floor()) };
    let ft = ft * ft * (3.0 - 2.0 * ft);
    let curl = |feature: f32, off: f32| {
        let f = 1.0 / (feature * scale.max(1e-3));
        let (px, py) = (x * f + o + off, y * f - o);
        let (_, ax, ay) = noise_grad(px + kt * 17.31, py - kt * 9.77);
        let (_, bx, by) = noise_grad(px + (kt + 1.0) * 17.31, py - (kt + 1.0) * 9.77);
        let (dx, dy) = (ax + (bx - ax) * ft, ay + (by - ay) * ft);
        [dy, -dx]
    };
    let (a, b) = (curl(60.0, 0.0), curl(18.0, 37.0));
    let (u, v) = (1.6 * (a[0] + 0.7 * fine * b[0]), 1.6 * (a[1] + 0.7 * fine * b[1]));
    let l = (u * u + v * v).sqrt();
    if l > 1.0 { [u / l, v / l] } else { [u, v] }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
/// How opaque the thread's outline pieces are at `k`, and its bridges and tail.
pub fn opacity(k: f32) -> (f32, f32) { (smooth(0.0, 0.12, k), smooth(0.1, 0.3, k)) }
/// How much of a loose element's fill is left at `k`.
pub fn fill(k: f32) -> f32 { 1.0 - smooth(0.05, 0.35, k) }
/// How many knot points follow a vertex with energy `e` at `k`.
pub fn knots(knot: f32, e: f32, k: f32) -> usize { (8.0 * knot * e * k).round().max(0.0) as usize }

/// The thread for a rest path: at most `max` pieces, one connected line (when the full line would
/// need more, every n-th rest point is kept and the knots shrink, never pieces dropped at random).
/// `field(x, y)` gives the map's energy and flow at a device point.
pub fn thread(rest: &Rest, fx: &StrandFx, c: &Ctx, field: &dyn Fn(f32, f32) -> (f32, [f32; 2]), max: usize) -> Vec<Seg> {
    let k = c.k.clamp(0.0, 1.0);
    if k <= 0.0 || rest.pts.len() < 2 || max == 0 { return vec![]; }
    let s = c.scale;
    let inside = |p: [f32; 2]| [p[0].clamp(c.scope[0], c.scope[2]), p[1].clamp(c.scope[1], c.scope[3])];
    let calm = |p: [f32; 2]| c.calm.iter().any(|b| p[0] >= b[0] && p[1] >= b[1] && p[0] < b[2] && p[1] < b[3]);
    // How many rest points to keep and how many knot points each may have, to stay within `max`.
    let n_rest = rest.pts.len();
    let at: Vec<(f32, [f32; 2])> = rest.pts.iter().map(|p| field(p[0], p[1])).collect();
    let e_max = at.iter().map(|a| a.0).fold(0.0, f32::max);
    let mut kn = knots(fx.knot, e_max, k);
    let tail_n = if fx.tail > 0.0 { 8 } else { 0 };
    let cut = fx.angular < 1.0;
    let per = |kn: usize| (1 + kn) * if cut { 2 } else { 1 };
    let mut every = 1;
    while (n_rest / every) * per(kn) + tail_n * 2 > max {
        if kn > 0 { kn -= 1; } else { every += 1; }
        if every > n_rest { return vec![]; }
    }
    let r_knot = fx.knot_r * s * k * k;
    let mut pts: Vec<([f32; 2], Kind)> = Vec::with_capacity(n_rest / every * (1 + kn) + tail_n);
    let mut i = 0;
    while i < n_rest {
        let v = rest.pts[i];
        // A bridge that the subsampling skipped still arrives as a bridge.
        let bridged = (i + 1 - every.min(i + 1)..=i).any(|j| rest.bridge[j]) && i > 0;
        let (e, fl) = at[i];
        let w = wander(v[0], v[1], c.t, c.seed, s, c.fine);
        let p = inside([v[0] + k * fx.reach * s * w[0] + k * fx.pull * c.drift * s * fl[0], v[1] + k * fx.reach * s * w[1] + k * fx.pull * c.drift * s * fl[1]]);
        pts.push((p, if bridged { Kind::Bridge } else { Kind::Line }));
        // The knot: a walk about the vertex by a hash of where it is on the path, each step turning a
        // little from the last and kept within `r_knot`, so it tangles in loops rather than spikes.
        let nk = knots(fx.knot, e, k).min(kn);
        let (mut q, mut a) = (p, hash(c.seed ^ (i as u32).wrapping_mul(2_654_435_761)) * std::f32::consts::TAU);
        for j in 0..nk {
            let h = c.seed.wrapping_mul(2_654_435_761) ^ (i as u32).wrapping_mul(97) ^ (j as u32).wrapping_mul(31_337);
            a += (hash(h) - 0.5) * 1.0 + 0.9;
            let step = r_knot * (0.3 + 0.25 * hash(h ^ 0x5bd1_e995));
            let mut n = [q[0] + step * a.cos(), q[1] + step * a.sin()];
            let (dx, dy) = (n[0] - p[0], n[1] - p[1]);
            let d = (dx * dx + dy * dy).sqrt();
            if d > r_knot { n = [p[0] + dx / d * r_knot, p[1] + dy / d * r_knot]; a += std::f32::consts::PI * 0.6; }
            q = inside(n);
            pts.push((q, Kind::Line));
        }
        if i == n_rest - 1 { break; }
        i = (i + every).min(n_rest - 1);
    }
    // The tail, along the flow where the line ends (or toward the nearest window edge), cut where it
    // leaves the scope and before a calm box. Its direction and wander are read at the rest point it
    // starts from, so every tail point moves in proportion to `k` and it draws back in a straight way.
    if tail_n > 0 {
        let end = pts.last().expect("a point").0;
        let from = *rest.pts.last().expect("a rest point");
        let (_, fl) = field(from[0], from[1]);
        let fl_len = (fl[0] * fl[0] + fl[1] * fl[1]).sqrt();
        let dir = if fl_len > 0.05 { [fl[0] / fl_len, fl[1] / fl_len] } else {
            let w = c.window;
            let d = [(from[0] - w[0], [-1.0, 0.0]), (w[2] - from[0], [1.0, 0.0]), (from[1] - w[1], [0.0, -1.0]), (w[3] - from[1], [0.0, 1.0])];
            d.iter().min_by(|a, b| a.0.total_cmp(&b.0)).map_or([1.0, 0.0], |x| x.1)
        };
        let len = fx.tail * s;
        let sc = c.scope;
        let out = |p: [f32; 2]| p[0] < sc[0] || p[1] < sc[1] || p[0] > sc[2] || p[1] > sc[3];
        let mut last = end;
        for j in 1..=tail_n {
            let f = j as f32 / tail_n as f32;
            let w = wander(from[0] + dir[0] * len * f, from[1] + dir[1] * len * f, c.t, c.seed ^ 77, s, c.fine);
            let p = [end[0] + k * f * (dir[0] * len + w[0] * fx.reach * s * 0.5), end[1] + k * f * (dir[1] * len + w[1] * fx.reach * s * 0.5)];
            if calm(p) { break; }
            if out(p) {
                // The piece that crosses the scope's edge, cut on it.
                let cross = |lo: f32, hi: f32, a: f32, b: f32| if b < lo { (lo - a) / (b - a) } else if b > hi { (hi - a) / (b - a) } else { 1.0 };
                let t = cross(sc[0], sc[2], last[0], p[0]).min(cross(sc[1], sc[3], last[1], p[1])).clamp(0.0, 1.0);
                if !out(last) { pts.push((inside([last[0] + (p[0] - last[0]) * t, last[1] + (p[1] - last[1]) * t]), Kind::Tail)); }
                break;
            }
            pts.push((p, Kind::Tail));
            last = p;
        }
    }
    // Corners cut by 1 − angular (one pass of corner cutting; 1 is the plain polyline).
    if cut && pts.len() > 2 {
        let q = 0.25 * (1.0 - fx.angular.clamp(0.0, 1.0));
        let mut out = Vec::with_capacity(pts.len() * 2);
        out.push(pts[0]);
        for w in pts.windows(2) {
            let ((a, _), (b, kb)) = (w[0], w[1]);
            out.push(([a[0] + (b[0] - a[0]) * q, a[1] + (b[1] - a[1]) * q], kb));
            out.push(([a[0] + (b[0] - a[0]) * (1.0 - q), a[1] + (b[1] - a[1]) * (1.0 - q)], kb));
        }
        out.push(*pts.last().expect("points"));
        pts = out;
    }
    pts.windows(2).filter(|w| d2(w[0].0, w[1].0) > 1e-6).map(|w| Seg { a: w[0].0, b: w[1].0, kind: w[1].1 }).take(max).collect()
}

/// A long path drawn by progress (DESIGN.md §1.30): points in the element's box (logical px), how much is
/// drawn (0..1 of its length), the line's width, dashes (`dash` long, `gap` apart, by arc length from the
/// start; `dash` 0 is solid), how far the map's flow pushes it (× `drift`), and how hard its head stirs
/// the map as it advances (0: not at all).
#[derive(Clone, Debug, PartialEq)]
pub struct Trace { pub points: std::rc::Rc<Vec<[f32; 2]>>, pub progress: f32, pub weight: f32, pub dash: f32, pub gap: f32, pub pull: f32, pub stir: f32, pub paint: crate::el::Paint }
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
    fn ctx(k: f32) -> Ctx<'static> { Ctx { k, t: 1.0, seed: 3, scale: 1.0, drift: 6.0, scope: [-1e4, -1e4, 1e4, 1e4], calm: &[], window: [0.0, 0.0, 800.0, 600.0], fine: 1.0 } }
    fn still(_: f32, _: f32) -> (f32, [f32; 2]) { (0.0, [0.0; 2]) }

    #[test]
    fn nothing_at_rest_and_on_the_outline_when_barely_loose() {
        let r = join(&[square(10.0, 10.0, 40.0)]);
        let fx = StrandFx::default();
        assert!(thread(&r, &fx, &ctx(0.0), &still, 10_000).is_empty());
        let segs = thread(&r, &StrandFx { angular: 1.0, ..fx }, &ctx(0.001), &still, 10_000);
        assert!(!segs.is_empty());
        for s in &segs {
            for p in [s.a, s.b] {
                let off = r.pts.iter().map(|q| d2(*q, p)).fold(f32::INFINITY, f32::min).sqrt();
                assert!(off < 0.5, "a vertex {off} px off the outline");
            }
        }
    }

    #[test]
    fn one_line_with_a_bridge_between_two_loops() {
        // "回": an outer and an inner loop.
        let r = join(&[square(0.0, 0.0, 60.0), square(20.0, 20.0, 20.0)]);
        assert_eq!(r.loops, 2);
        assert_eq!(r.bridge.iter().filter(|b| **b).count(), 1);
        let segs = thread(&r, &StrandFx { tail: 0.0, ..StrandFx::default() }, &ctx(0.5), &still, 10_000);
        for w in segs.windows(2) { assert_eq!(w[0].b, w[1].a, "the thread is one line"); }
        assert!(segs.iter().any(|s| s.kind == Kind::Bridge));
        // The bridge goes to the inner loop's nearest vertex.
        let i = r.bridge.iter().position(|b| *b).expect("a bridge");
        assert_eq!(r.pts[i], [20.0, 20.0]);
    }

    #[test]
    fn knots_follow_the_formula_and_everything_stays_in_scope() {
        let r = join(&[square(100.0, 100.0, 50.0)]);
        let fx = StrandFx { angular: 1.0, tail: 0.0, ..StrandFx::default() };
        let hot = |_: f32, _: f32| (1.0, [0.0, 0.0]);
        let segs = thread(&r, &fx, &ctx(1.0), &hot, 100_000);
        assert_eq!(knots(0.6, 1.0, 1.0), 5);
        assert_eq!(segs.len() + 1, r.pts.len() * (1 + knots(fx.knot, 1.0, 1.0)), "every vertex followed by its knot");
        let sc = [90.0, 90.0, 160.0, 160.0];
        let c = Ctx { scope: sc, ..ctx(1.0) };
        let segs = thread(&r, &StrandFx { tail: 400.0, ..fx }, &c, &|_, _| (1.0, [1.0, 0.0]), 100_000);
        for s in segs { for p in [s.a, s.b] { assert!(p[0] >= sc[0] && p[1] >= sc[1] && p[0] <= sc[2] && p[1] <= sc[3], "{p:?} outside the scope"); } }
    }

    #[test]
    fn the_budget_keeps_one_line_and_the_same_input_the_same_bytes() {
        let r = join(&[box_outline([0.0, 0.0, 300.0, 200.0], 12.0, [0.0; 4], 2.0)]);
        let hot = |x: f32, _: f32| (1.0, [(x * 0.01).sin(), 0.3]);
        let all = thread(&r, &StrandFx::default(), &ctx(1.0), &hot, usize::MAX);
        let few = thread(&r, &StrandFx::default(), &ctx(1.0), &hot, all.len() / 4);
        assert!(few.len() <= all.len() / 4 && !few.is_empty());
        for w in few.windows(2) { assert_eq!(w[0].b, w[1].a); }
        assert_eq!(thread(&r, &StrandFx::default(), &ctx(1.0), &hot, usize::MAX), all);
        // The tail runs along the flow.
        let tail: Vec<&Seg> = all.iter().filter(|s| s.kind == Kind::Tail).collect();
        assert!(!tail.is_empty());
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
        let w = wander(120.0, 40.0, 2.0, 1, 1.0, 1.0);
        assert!((w[0] * w[0] + w[1] * w[1]).sqrt() <= 1.0 + 1e-5);
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
}
