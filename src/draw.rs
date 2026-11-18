//! Drawings (DESIGN.md §1.26): an element whose content is marks in its own coordinate space — dots
//! and strokes — each made into a shape instance with a true outline (a stroke is a rounded box turned
//! along its direction, `Instance::xf`), so every material draws them and fronts loosen them like any
//! shape. A field becomes a drawing by `dots` (a dot per node, sized by the value) or `contours`
//! (marching squares). `el::canvas` puts one on the page; its input reports positions in its space.
use crate::el::Paint;
use crate::field::{Grid, Scalar};

/// One mark, in the drawing's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    /// A round dot of radius `r` about (x, y).
    Dot { x: f32, y: f32, r: f32, paint: Paint },
    /// A stroke `w` wide from `a` to `b` with round ends.
    Line { a: (f32, f32), b: (f32, f32), w: f32, paint: Paint },
    /// A rooted stroke that the field map bends (`Blade`).
    Blade(Blade),
}

/// A rooted stroke that the field map bends (DESIGN.md §1.26): a stalk of grass. It stands on `root`
/// and is `len` long, `width` wide at the root and half that at the tip (0.5 px at least). The GPU bends
/// it from the field map's flow at its root every frame: it leans `lean + max_lean × flow.x / drift`
/// (radians, clockwise; the flow's part within ±`max_lean`), measured as the angle of the chord from root
/// to tip. It stays upright for its first 4 px (8 % of it when it is shorter) and keeps its length;
/// `stiff` says how much of it stays straight (0: one even arc; 1: the lower 60 %, as far as the lean
/// allows), and its tip never turns past 85° (`Blade::shape`). Its colour goes from
/// `paint` to `lean_paint` as it leans. A `head` glyph rides the tip, the bottom centre of its ink on
/// it, turned with the tip. While only the map moves, nothing of it is worked out on the CPU.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blade {
    /// Where it stands, in the drawing's space.
    pub root: (f32, f32),
    /// Its length.
    pub len: f32,
    /// Its width at the root.
    pub width: f32,
    /// Its own lean at rest, radians (clockwise from upright).
    pub lean: f32,
    /// How far the map's flow can lean it, radians.
    pub max_lean: f32,
    /// How much of it stays straight, 0–1.
    pub stiff: f32,
    /// Its colour upright.
    pub paint: Paint,
    /// Its colour leaning `max_lean`.
    pub lean_paint: Paint,
    /// A glyph on its tip.
    pub head: Option<Head>,
}
impl Default for Blade {
    fn default() -> Self { Self { root: (0.0, 0.0), len: 40.0, width: 1.5, lean: 0.0, max_lean: 57f32.to_radians(), stiff: 0.5, paint: Paint::INK, lean_paint: Paint::INK, head: None } }
}
/// The steps a blade's shape is worked out in (the shader's `BLADE_N`): its segments end on them.
pub const BLADE_STEPS: usize = 18;
/// The most a blade's tip turns from upright.
const BLADE_TIP: f32 = 85.0 * std::f32::consts::PI / 180.0;
/// How sharply a blade bends at its foot when it leans far.
const BLADE_BEND: f32 = 10.0;
impl Blade {
    /// The angle from upright at a share `u` of the length, on the way `t` (0 straight, 1 bent the most)
    /// for a stiffness `s` and an upright foot of `h` (a share of the length). Up to `t` = ½ one arc above
    /// the stiff part grows to 85° at its tip; then the arc starts lower, down to the foot, and bends more
    /// at its start: every angle only grows, so the chord's angle does too.
    fn angle(u: f32, t: f32, s: f32, h: f32) -> f32 {
        let b0 = h.max(0.6 * s);
        let (tip, b, k) = if t <= 0.5 { (BLADE_TIP * t * 2.0, b0, 1.0) } else { let w = (t - 0.5) * 2.0; (BLADE_TIP, b0 + (h - b0) * w, 1.0 + (BLADE_BEND - 1.0) * w) };
        let sp = ((u - b) / (1.0 - b)).clamp(0.0, 1.0);
        tip * (1.0 - (1.0 - sp).max(1e-6).powf(k))
    }
    fn walk(t: f32, s: f32, h: f32, len: f32) -> [[f32; 2]; BLADE_STEPS + 1] {
        let mut p = [[0.0; 2]; BLADE_STEPS + 1];
        let d = len / BLADE_STEPS as f32;
        for j in 0..BLADE_STEPS {
            let a = Self::angle((j as f32 + 0.5) / BLADE_STEPS as f32, t, s, h);
            p[j + 1] = [p[j][0] + a.sin() * d, p[j][1] - a.cos() * d];
        }
        p
    }
    /// Its shape (as the shader works it out) leaning `lean` radians, `len` device px long: the points
    /// at each of `BLADE_STEPS` steps from the root at (0, 0), up being −y, and the tip's angle from
    /// upright. The length is kept; the chord's angle is `lean` as far as an 85° tip allows (about 73°
    /// for a blade of 50 px or more).
    pub fn shape(lean: f32, stiff: f32, len: f32) -> ([[f32; 2]; BLADE_STEPS + 1], f32) {
        let (th, s) = (lean.abs(), stiff.clamp(0.0, 1.0));
        let h = 4f32.min(0.08 * len) / len.max(1e-3);
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..16 {
            let m = (lo + hi) * 0.5;
            let p = Self::walk(m, s, h, len)[BLADE_STEPS];
            if p[0].atan2(-p[1]) < th { lo = m } else { hi = m }
        }
        let t = if th > 0.0 { (lo + hi) * 0.5 } else { 0.0 };
        let mut p = Self::walk(t, s, h, len);
        let sg = if lean < 0.0 { -1.0 } else { 1.0 };
        for q in &mut p { q[0] *= sg; }
        (p, sg * Self::angle(1.0, t, s, h))
    }
}
/// The glyph on a blade's tip: which, its size (in the drawing's space) and face, and its paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Head {
    /// The character.
    pub ch: char,
    /// Its size.
    pub size: f32,
    /// Its face.
    pub font: crate::el::Font,
    /// Its paint.
    pub paint: Paint,
}

/// Marks in a `w × h` space; the element shows the space scaled to its width (its height follows).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing { pub w: f32, pub h: f32, pub marks: Vec<Mark> }
impl Drawing {
    pub fn new(w: f32, h: f32) -> Self { Self { w, h, marks: Vec::new() } }
    pub fn dot(&mut self, x: f32, y: f32, r: f32, paint: Paint) { if r > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Dot { x, y, r, paint }); } }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, paint: Paint) { if w > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Line { a, b, w, paint }); } }
    /// A blade (`Blade`).
    pub fn blade(&mut self, b: Blade) { if b.len > 0.0 && b.width > 0.0 && b.paint.alpha > 0.0 { self.marks.push(Mark::Blade(b)); } }
    /// A polyline as strokes; `paint` is asked for each piece by its midpoint.
    pub fn polyline(&mut self, pts: &[(f32, f32)], w: f32, paint: impl Fn(f32, f32) -> Paint) {
        for p in pts.windows(2) { self.line(p[0], p[1], w, paint((p[0].0 + p[1].0) * 0.5, (p[0].1 + p[1].1) * 0.5)); }
    }
    /// A circle of radius `r` about (x, y) as `n` strokes.
    pub fn circle(&mut self, x: f32, y: f32, r: f32, n: usize, w: f32, paint: Paint) {
        let pts: Vec<(f32, f32)> = (0..=n).map(|i| { let a = i as f32 / n as f32 * std::f32::consts::TAU; (x + r * a.cos(), y + r * a.sin()) }).collect();
        self.polyline(&pts, w, |_, _| paint);
    }
    /// The iso-contours of `g` at each level as strokes.
    pub fn contours(&mut self, g: &Grid, levels: &[f32], w: f32, paint: impl Fn(f32, f32, f32) -> Paint) {
        for &l in levels { for c in g.contours(l) { self.polyline(&c, w, |x, y| paint(x, y, l)); } }
    }
    /// A dot at every node of a `pitch` lattice, its radius `size(value)` (none at 0).
    pub fn dots(&mut self, f: &impl Scalar, pitch: f32, size: impl Fn(f32) -> f32, paint: impl Fn(f32, f32, f32) -> Paint) {
        let (nx, ny) = ((self.w / pitch).floor() as usize, (self.h / pitch).floor() as usize);
        let (ox, oy) = ((self.w - (nx as f32 - 1.0) * pitch) * 0.5, (self.h - (ny as f32 - 1.0) * pitch) * 0.5);
        for j in 0..ny { for i in 0..nx {
            let (x, y) = (ox + i as f32 * pitch, oy + j as f32 * pitch);
            let v = f.at(x, y);
            let r = size(v);
            if r > 0.0 { let p = paint(x, y, v); self.dot(x, y, r, p); }
        } }
    }
}
