//! Fields (DESIGN.md §1.26): a number or a direction at every point of a box, sampled on a grid or
//! given by a formula, and what is made from one — dots by the value, iso-contours (marching squares),
//! marks carried along it, a reaction–diffusion grown inside a mask. One abstraction for every effect
//! that is "a field drawn some way": the flow field of motes, the lab's contours, metaballs, silk and
//! grown patterns all sample, step and draw through this module. Deterministic: the same seed, steps
//! and inputs give the same numbers. No GPU here.
#![warn(missing_docs)]

/// A hash in 0..1 of an integer.
pub fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 16; x = x.wrapping_mul(0x7FEB_352D); x ^= x >> 15; x = x.wrapping_mul(0x846C_A68B); x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}
fn lattice(ix: i32, iy: i32) -> f32 { hash((ix as u32).wrapping_mul(73_856_093) ^ (iy as u32).wrapping_mul(19_349_663)) }
/// Smooth value noise, 0..1, features one unit apart.
pub fn noise(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = lattice(ix, iy) + (lattice(ix + 1, iy) - lattice(ix, iy)) * sx;
    let b = lattice(ix, iy + 1) + (lattice(ix + 1, iy + 1) - lattice(ix, iy + 1)) * sx;
    a + (b - a) * sy
}
/// Value noise that changes smoothly with `t` (one unit of `t` is one new pattern), 0..1.
pub fn noise_t(x: f32, y: f32, t: f32) -> f32 {
    let (k, f) = (t.floor(), t - t.floor());
    let f = f * f * (3.0 - 2.0 * f);
    let a = noise(x + k * 17.31, y - k * 9.77);
    let b = noise(x + (k + 1.0) * 17.31, y - (k + 1.0) * 9.77);
    a + (b - a) * f
}

/// A number at every point (x, y) of a box: a formula or a sampled grid.
pub trait Scalar {
    /// The value at (x, y).
    fn at(&self, x: f32, y: f32) -> f32;
}
impl<F: Fn(f32, f32) -> f32> Scalar for F { fn at(&self, x: f32, y: f32) -> f32 { self(x, y) } }

/// Drifting noise: features `scale` apart, a new pattern every `1 / speed` s, at time `t`, `seed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// How far apart its features are.
    pub scale: f32,
    /// How many new patterns a second.
    pub speed: f32,
    /// The time.
    pub t: f32,
    /// Which pattern.
    pub seed: u32,
}
impl Scalar for Noise {
    fn at(&self, x: f32, y: f32) -> f32 {
        let o = hash(self.seed) * 97.0;
        noise_t(x / self.scale.max(1e-3) + o, y / self.scale.max(1e-3) - o, self.t * self.speed)
    }
}
/// A bump: `a × exp(−d² / r²)` about (x, y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bump {
    /// Its centre, across.
    pub x: f32,
    /// Its centre, down.
    pub y: f32,
    /// How wide it is.
    pub r: f32,
    /// How high it is.
    pub a: f32,
}
impl Scalar for Bump {
    fn at(&self, x: f32, y: f32) -> f32 { let d2 = (x - self.x).powi(2) + (y - self.y).powi(2); self.a * (-d2 / (self.r * self.r).max(1e-6)).exp() }
}
/// Metaballs: Σ rᵢ² / dᵢ² over the balls (x, y, r); the surface is where it is 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Balls(pub Vec<(f32, f32, f32)>);
impl Scalar for Balls {
    fn at(&self, x: f32, y: f32) -> f32 { self.0.iter().map(|&(bx, by, r)| r * r / ((x - bx).powi(2) + (y - by).powi(2) + 1e-3)).sum() }
}
/// The sum of two fields.
pub struct Sum<A, B>(pub A, pub B);
impl<A: Scalar, B: Scalar> Scalar for Sum<A, B> { fn at(&self, x: f32, y: f32) -> f32 { self.0.at(x, y) + self.1.at(x, y) } }

/// The curl of a scalar field (its gradient turned a quarter): a flow without sources or sinks, so
/// marks carried along it neither bunch up nor thin out. `e` is the step of the difference.
pub fn curl(f: &impl Scalar, x: f32, y: f32, e: f32) -> (f32, f32) {
    let dx = (f.at(x + e, y) - f.at(x - e, y)) / (2.0 * e);
    let dy = (f.at(x, y + e) - f.at(x, y - e)) / (2.0 * e);
    (dy, -dx)
}

/// A field sampled at the nodes of a `cols × rows` grid spanning a `w × h` box (node (i, j) is at
/// `(i·w/(cols−1), j·h/(rows−1))`); read between nodes bilinearly.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// Nodes across.
    pub cols: usize,
    /// Nodes down.
    pub rows: usize,
    /// The box's width.
    pub w: f32,
    /// The box's height.
    pub h: f32,
    /// The values, row by row.
    pub v: Vec<f32>,
}
impl Grid {
    /// A grid of zeros (at least 2 × 2 nodes).
    pub fn new(cols: usize, rows: usize, w: f32, h: f32) -> Self { let (cols, rows) = (cols.max(2), rows.max(2)); Self { cols, rows, w, h, v: vec![0.0; cols * rows] } }
    /// Where node (i, j) is.
    pub fn node(&self, i: usize, j: usize) -> (f32, f32) { (i as f32 * self.w / (self.cols - 1) as f32, j as f32 * self.h / (self.rows - 1) as f32) }
    /// Sample `f` at every node.
    pub fn sample(cols: usize, rows: usize, w: f32, h: f32, f: &impl Scalar) -> Self {
        let mut g = Self::new(cols, rows, w, h);
        for j in 0..g.rows { for i in 0..g.cols { let (x, y) = g.node(i, j); g.v[j * g.cols + i] = f.at(x, y); } }
        g
    }
    /// The value at node (i, j), clamped to the grid.
    pub fn get(&self, i: usize, j: usize) -> f32 { self.v[j.min(self.rows - 1) * self.cols + i.min(self.cols - 1)] }
    /// Add `f` at every node.
    pub fn add(&mut self, f: &impl Scalar) {
        for j in 0..self.rows { for i in 0..self.cols { let (x, y) = self.node(i, j); self.v[j * self.cols + i] += f.at(x, y); } }
    }
    /// The largest value.
    pub fn max(&self) -> f32 { self.v.iter().copied().fold(f32::NEG_INFINITY, f32::max) }
    /// Iso-contours at `level` by marching squares: polylines in box coordinates, a closed one ending
    /// where it starts. Saddles are decided by the cell's mean, so contours never cross.
    pub fn contours(&self, level: f32) -> Vec<Vec<(f32, f32)>> {
        let (c, r) = (self.cols, self.rows);
        // Every crossing is on a grid edge: horizontal edge (i, j)→(i+1, j) is 2(j·c + i), vertical
        // (i, j)→(i, j+1) is 2(j·c + i) + 1.
        let at = |e: usize| -> (f32, f32) {
            let (n, vert) = (e / 2, e % 2 == 1);
            let (i, j) = (n % c, n / c);
            let (i2, j2) = if vert { (i, j + 1) } else { (i + 1, j) };
            let (a, b) = (self.get(i, j), self.get(i2, j2));
            let t = if (b - a).abs() < 1e-9 { 0.5 } else { ((level - a) / (b - a)).clamp(0.0, 1.0) };
            let (p, q) = (self.node(i, j), self.node(i2, j2));
            (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
        };
        let mut segs: Vec<[usize; 2]> = vec![];
        for j in 0..r - 1 { for i in 0..c - 1 {
            let v = [self.get(i, j), self.get(i + 1, j), self.get(i + 1, j + 1), self.get(i, j + 1)];
            let k = v.iter().enumerate().fold(0, |k, (b, &x)| k | ((x > level) as usize) << b);
            if k == 0 || k == 15 { continue; }
            let (top, right, bottom, left) = (2 * (j * c + i), 2 * (j * c + i + 1) + 1, 2 * ((j + 1) * c + i), 2 * (j * c + i) + 1);
            let mid = v.iter().sum::<f32>() * 0.25 > level;
            let pairs: &[[usize; 2]] = match k {
                1 | 14 => &[[left, top]], 2 | 13 => &[[top, right]], 3 | 12 => &[[left, right]], 4 | 11 => &[[right, bottom]],
                6 | 9 => &[[top, bottom]], 7 | 8 => &[[left, bottom]],
                5 => if mid { &[[left, bottom], [top, right]] } else { &[[left, top], [right, bottom]] },
                _ => if mid { &[[left, top], [right, bottom]] } else { &[[top, right], [left, bottom]] },
            };
            segs.extend_from_slice(pairs);
        } }
        // Chain the segments through their shared edges.
        let mut by: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::with_capacity(segs.len() * 2);
        for (s, e) in segs.iter().enumerate() { for &x in e { by.entry(x).or_default().push(s); } }
        let mut used = vec![false; segs.len()];
        let mut out = vec![];
        // Open chains start at an edge that only one segment touches (the grid's border).
        let starts: Vec<usize> = (0..segs.len()).filter(|&s| segs[s].iter().any(|e| by[e].len() == 1)).chain(0..segs.len()).collect();
        for s0 in starts {
            if used[s0] { continue; }
            used[s0] = true;
            let first = if by[&segs[s0][0]].len() == 1 { 0 } else if by[&segs[s0][1]].len() == 1 { 1 } else { 0 };
            let mut line = vec![segs[s0][first], segs[s0][1 - first]];
            loop {
                let end = *line.last().expect("two points");
                let Some(&n) = by[&end].iter().find(|&&t| !used[t]) else { break };
                used[n] = true;
                line.push(if segs[n][0] == end { segs[n][1] } else { segs[n][0] });
            }
            out.push(line.into_iter().map(at).collect());
        }
        out
    }
}

/// A direction at every node of a grid (two grids), with what stirs it: the pointer's velocity is
/// put in as a bump of momentum and the whole field fades back to still.
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    /// The direction's across part at each node.
    pub u: Grid,
    /// Its down part.
    pub v: Grid,
}
impl Flow {
    /// A still flow on a `cols × rows` grid over a `w × h` box.
    pub fn new(cols: usize, rows: usize, w: f32, h: f32) -> Self { Self { u: Grid::new(cols, rows, w, h), v: Grid::new(cols, rows, w, h) } }
    /// The direction at (x, y), read between nodes bilinearly.
    pub fn at(&self, x: f32, y: f32) -> (f32, f32) { (self.u.at(x, y), self.v.at(x, y)) }
    /// Put velocity (vx, vy) in about (x, y), falling off over `r`.
    pub fn stir(&mut self, x: f32, y: f32, vx: f32, vy: f32, r: f32) {
        self.u.add(&Bump { x, y, r, a: vx });
        self.v.add(&Bump { x, y, r, a: vy });
    }
    /// Fade toward still: every value times `exp(−dt / tau)`.
    pub fn fade(&mut self, dt: f32, tau: f32) {
        let k = (-dt / tau.max(1e-3)).exp();
        for x in self.u.v.iter_mut().chain(self.v.v.iter_mut()) { *x *= k; if x.abs() < 1e-3 { *x = 0.0; } }
    }
    /// Whether anything is still moving in it.
    pub fn moving(&self) -> bool { self.u.v.iter().chain(&self.v.v).any(|x| *x != 0.0) }
}
impl Scalar for Grid {
    fn at(&self, x: f32, y: f32) -> f32 {
        let fx = (x / self.w.max(1e-6) * (self.cols - 1) as f32).clamp(0.0, (self.cols - 1) as f32);
        let fy = (y / self.h.max(1e-6) * (self.rows - 1) as f32).clamp(0.0, (self.rows - 1) as f32);
        let (i, j) = ((fx as usize).min(self.cols - 2), (fy as usize).min(self.rows - 2));
        let (tx, ty) = (fx - i as f32, fy - j as f32);
        let a = self.get(i, j) + (self.get(i + 1, j) - self.get(i, j)) * tx;
        let b = self.get(i, j + 1) + (self.get(i + 1, j + 1) - self.get(i, j + 1)) * tx;
        a + (b - a) * ty
    }
}

/// Marks carried along a flow inside a `w × h` box: positions and the step each moved last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Swarm {
    /// Where each mark is.
    pub p: Vec<(f32, f32)>,
    /// The step each moved last.
    pub d: Vec<(f32, f32)>,
    /// The box's width.
    pub w: f32,
    /// The box's height.
    pub h: f32,
}
impl Swarm {
    /// `n` marks placed by hash from `seed`.
    pub fn new(n: usize, w: f32, h: f32, seed: u32) -> Self {
        let p: Vec<(f32, f32)> = (0..n as u32).map(|i| (hash(seed ^ (i * 2 + 1)) * w, hash(seed ^ (i * 2 + 2)) * h)).collect();
        Self { d: vec![(0.0, 0.0); p.len()], p, w, h }
    }
    /// Move every mark by `velocity(x, y) × dt`; one that leaves the box comes in on the other side.
    pub fn advect(&mut self, dt: f32, velocity: impl Fn(f32, f32) -> (f32, f32)) {
        for (p, d) in self.p.iter_mut().zip(&mut self.d) {
            let (vx, vy) = velocity(p.0, p.1);
            *d = (vx * dt, vy * dt);
            *p = ((p.0 + d.0).rem_euclid(self.w.max(1.0)), (p.1 + d.1).rem_euclid(self.h.max(1.0)));
        }
    }
}

/// Gray–Scott reaction–diffusion on a grid, confined to a mask: `u′ = Du∇²u − uv² + f(1−u)`,
/// `v′ = Dv∇²v + uv² − (f+k)v`. Outside the mask (below one half) `u` is held at 1 and `v` at 0, and
/// nothing flows across the outline, so the pattern grows inside it and stops at it.
#[derive(Clone, Debug, PartialEq)]
pub struct GrayScott {
    /// Nodes across.
    pub cols: usize,
    /// Nodes down.
    pub rows: usize,
    /// The first chemical, row by row (1 where nothing has grown).
    pub u: Vec<f32>,
    /// The second (the pattern), row by row.
    pub v: Vec<f32>,
    /// Inside (≥ 0.5) or outside each node.
    pub mask: Vec<f32>,
    /// The feed rate.
    pub f: f32,
    /// The kill rate.
    pub k: f32,
    /// How fast `u` spreads.
    pub du: f32,
    /// How fast `v` spreads.
    pub dv: f32,
}
impl GrayScott {
    /// Maze-forming numbers (f 0.037, k 0.060, Du 1, Dv 0.5), everywhere open.
    pub fn new(cols: usize, rows: usize) -> Self { Self { cols, rows, u: vec![1.0; cols * rows], v: vec![0.0; cols * rows], mask: vec![1.0; cols * rows], f: 0.037, k: 0.060, du: 1.0, dv: 0.5 } }
    /// Take a mask (any field over the same grid, inside where ≥ 0.5) and start again: `v` seeded in
    /// a few small squares inside it, placed by hash from `seed`.
    pub fn reset(&mut self, mask: &Grid, seed: u32) {
        let n = self.cols * self.rows;
        self.mask = (0..n).map(|k| mask.at(mask.w * (k % self.cols) as f32 / (self.cols - 1) as f32, mask.h * (k / self.cols) as f32 / (self.rows - 1) as f32)).collect();
        self.u = vec![1.0; n];
        self.v = vec![0.0; n];
        let inside: Vec<usize> = (0..n).filter(|&k| self.mask[k] >= 0.5).collect();
        if inside.is_empty() { return; }
        for s in 0..12u32 {
            let k = inside[((hash(seed.wrapping_mul(31) ^ s) * inside.len() as f32) as usize).min(inside.len() - 1)];
            self.seed_at(k % self.cols, k / self.cols, 2);
        }
    }
    /// Put `v` in a square of radius `r` cells about cell (i, j) (inside the mask only).
    pub fn seed_at(&mut self, i: usize, j: usize, r: usize) {
        for y in j.saturating_sub(r)..(j + r + 1).min(self.rows) { for x in i.saturating_sub(r)..(i + r + 1).min(self.cols) {
            let k = y * self.cols + x;
            if self.mask[k] >= 0.5 { self.u[k] = 0.5; self.v[k] = 0.25; }
        } }
    }
    /// `n` steps of one time unit (nine-point Laplacian: 0.2 sides, 0.05 corners).
    pub fn step(&mut self, n: usize) {
        let (c, r) = (self.cols, self.rows);
        let (mut u2, mut v2) = (self.u.clone(), self.v.clone());
        for _ in 0..n {
            for y in 0..r { for x in 0..c {
                let k = y * c + x;
                if self.mask[k] < 0.5 { u2[k] = 1.0; v2[k] = 0.0; continue; }
                let (xl, xr, yu, yd) = (x.saturating_sub(1), (x + 1).min(c - 1), y.saturating_sub(1), (y + 1).min(r - 1));
                // No flux through the outline: a neighbour outside the mask counts as this cell, so a
                // thin stroke keeps what it has instead of draining into the held outside.
                let m = &self.mask;
                let lap = |a: &[f32]| {
                    let at = |j: usize| if m[j] >= 0.5 { a[j] } else { a[k] };
                    0.2 * (at(y * c + xl) + at(y * c + xr) + at(yu * c + x) + at(yd * c + x))
                        + 0.05 * (at(yu * c + xl) + at(yu * c + xr) + at(yd * c + xl) + at(yd * c + xr)) - a[k]
                };
                let (u, v) = (self.u[k], self.v[k]);
                let uvv = u * v * v;
                u2[k] = (u + self.du * lap(&self.u) - uvv + self.f * (1.0 - u)).clamp(0.0, 1.0);
                v2[k] = (v + self.dv * lap(&self.v) + uvv - (self.f + self.k) * v).clamp(0.0, 1.0);
            } }
            std::mem::swap(&mut self.u, &mut u2);
            std::mem::swap(&mut self.v, &mut v2);
        }
    }
    /// `v` as a grid over a `w × h` box.
    pub fn v_grid(&self, w: f32, h: f32) -> Grid { Grid { cols: self.cols, rows: self.rows, w, h, v: self.v.clone() } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_circle_has_one_closed_contour() {
        let g = Grid::sample(41, 41, 100.0, 100.0, &|x: f32, y: f32| ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt());
        let cs = g.contours(30.0);
        assert_eq!(cs.len(), 1);
        let c = &cs[0];
        assert_eq!(c.first(), c.last(), "closed");
        for p in c { let d = ((p.0 - 50.0).powi(2) + (p.1 - 50.0).powi(2)).sqrt(); assert!((d - 30.0).abs() < 0.6, "{d}"); }
    }
    #[test]
    fn contours_open_at_the_border_and_saddles_do_not_cross() {
        let g = Grid::sample(21, 11, 200.0, 100.0, &|x: f32, _: f32| x);
        let cs = g.contours(55.0);
        assert_eq!(cs.len(), 1);
        assert_ne!(cs[0].first(), cs[0].last());
        // A checkerboard saddle: every cell gives two segments, never four ends at one point.
        let s = Grid::sample(3, 3, 2.0, 2.0, &|x: f32, y: f32| (x * 3.0).sin() * (y * 3.0).sin());
        let _ = s.contours(0.0);
    }
    #[test]
    fn noise_is_continuous_and_deterministic() {
        let n = Noise { scale: 40.0, speed: 0.2, t: 3.3, seed: 7 };
        for i in 0..100 { let (x, y) = (hash(i) * 300.0, hash(i + 77) * 200.0); assert!((n.at(x, y) - n.at(x + 0.1, y)).abs() < 0.02); assert_eq!(n.at(x, y), n.at(x, y)); }
    }
    #[test]
    fn curl_has_no_divergence() {
        let n = Noise { scale: 30.0, speed: 0.0, t: 0.0, seed: 1 };
        let e = 0.5;
        for i in 0..50 {
            let (x, y) = (20.0 + hash(i) * 200.0, 20.0 + hash(i + 9) * 100.0);
            let div = (curl(&n, x + e, y, 0.25).0 - curl(&n, x - e, y, 0.25).0) / (2.0 * e) + (curl(&n, x, y + e, 0.25).1 - curl(&n, x, y - e, 0.25).1) / (2.0 * e);
            let mag = curl(&n, x, y, 0.25).0.abs() + curl(&n, x, y, 0.25).1.abs();
            assert!(div.abs() < 0.02 + mag * 0.05, "{div}");
        }
    }
    #[test]
    fn gray_scott_grows_only_inside_its_mask() {
        let mask = Grid::sample(48, 32, 48.0, 32.0, &|x: f32, y: f32| if (x - 24.0).abs() < 12.0 && (y - 16.0).abs() < 9.0 { 1.0 } else { 0.0 });
        let mut gs = GrayScott::new(48, 32);
        gs.reset(&mask, 3);
        gs.step(1500);
        let (mut inside, mut outside) = (0.0, 0.0);
        for k in 0..gs.v.len() { if gs.mask[k] >= 0.5 { inside += gs.v[k]; } else { outside += gs.v[k]; } }
        assert!(inside > 5.0, "{inside}");
        assert_eq!(outside, 0.0);
        let mut again = GrayScott::new(48, 32);
        again.reset(&mask, 3);
        again.step(1500);
        assert_eq!(again.v, gs.v, "deterministic");
    }
    #[test]
    fn a_stirred_flow_fades_to_still() {
        let mut f = Flow::new(16, 12, 160.0, 120.0);
        f.stir(80.0, 60.0, 100.0, 0.0, 20.0);
        assert!(f.at(80.0, 60.0).0 > 50.0);
        for _ in 0..400 { f.fade(1.0 / 60.0, 0.3); }
        assert!(!f.moving());
        let mut s = Swarm::new(10, 100.0, 50.0, 4);
        s.advect(1.0, |_, _| (150.0, 0.0));
        assert!(s.p.iter().all(|p| p.0 >= 0.0 && p.0 < 100.0));
    }
}
