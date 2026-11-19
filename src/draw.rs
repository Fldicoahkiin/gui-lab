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
    /// A stroke `w` wide from `a` to `b` with round ends, in `paint` at `a`; with `far`, a ramp to `far`
    /// at `b`, through `mid` (a paint and where it is, 0–1 of the way) if given, still one instance
    /// (`Drawing::ramp`).
    Line { a: (f32, f32), b: (f32, f32), w: f32, paint: Paint, far: Option<Paint>, mid: Option<(Paint, f32)> },
    /// A rooted stroke that the field map bends (`Blade`).
    Blade(Blade),
}

/// A rooted stroke that the field map bends (DESIGN.md §1.26): a stalk of grass. It stands on `root`
/// and is `len` long, `width` wide at the root and half that at the tip (0.5 px at least). The GPU bends
/// it from the field map's flow at its root every frame: it leans `lean + max_lean × flow.x / drift`
/// (radians, clockwise; its own lean and the flow's added, all of it within ±`max_lean` and ±85°), measured as the
/// angle of its tip's tangent from upright. Its root leans 0.35 of that and it bends toward the tip
/// (`Blade::tangent`); `stiff` says how much of it stays straight (0: an even bend from root to tip; 1:
/// the lower 60 % straight); it keeps its length (`Blade::shape`). Its colour goes from
/// `paint` to `lean_paint` as it leans (`Blade::mix`). A `head` glyph rides the tip, the bottom centre of its ink on
/// it, turned with the tip, its colour going from its own paint to `lean_paint` with the stroke's. While only the map moves, nothing of it is worked out on the CPU.
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
    /// Its colour leaning `max_lean` (or `lean_full`).
    pub lean_paint: Paint,
    /// From how far it leans (radians, either way) its colour starts towards `lean_paint` (`Blade::mix`).
    pub lean_from: f32,
    /// How far it leans when its colour is all `lean_paint`; 0 (the default): `max_lean`, the mix linear.
    pub lean_full: f32,
    /// A glyph on its tip.
    pub head: Option<Head>,
}
impl Default for Blade {
    fn default() -> Self { Self { root: (0.0, 0.0), len: 40.0, width: 1.5, lean: 0.0, max_lean: 57f32.to_radians(), stiff: 0.5, paint: Paint::INK, lean_paint: Paint::INK, lean_from: 0.0, lean_full: 0.0, head: None } }
}
/// The steps of a blade's walk to one point (the shader's `BLADE_M`).
const BLADE_WALK: usize = 16;
/// The most a blade leans, its tip from upright.
pub const BLADE_MOST: f32 = 85.0 * std::f32::consts::PI / 180.0;
impl Blade {
    /// How much of the way from `paint` to `lean_paint` its colour (and its head's) is, leaning `lean`
    /// (the shader's `blade_paint`): with `lean_full` above `lean_from`, 0 up to `lean_from`, 1 from
    /// `lean_full` and a smoothstep between; otherwise (the default, `lean_full` 0) `|lean| / max_lean`.
    pub fn mix(&self, lean: f32) -> f32 {
        let a = lean.abs();
        if self.lean_full > self.lean_from {
            let t = ((a - self.lean_from) / (self.lean_full - self.lean_from)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        } else { (a / self.max_lean.abs().clamp(1e-3, 1.5)).clamp(0.0, 1.0) }
    }
    /// How far its bend has come at a share `s` of its length (0 at the root, 1 at the tip): `s` itself
    /// when it is not stiff; with `stiff`, 0 over the lower `0.6 × stiff` and then a smoothstep to 1.
    pub fn bend(s: f32, stiff: f32) -> f32 {
        if stiff <= 0.0 { return s.clamp(0.0, 1.0); }
        let t = ((s - 0.6 * stiff) / (1.0 - 0.6 * stiff)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
    /// Its tangent's angle from upright at a share `s` of its length, leaning `lean` (the tip's angle):
    /// `lean × (0.35 + 0.65 × bend)`, so the root leans 0.35 × `lean`.
    pub fn tangent(s: f32, lean: f32, stiff: f32) -> f32 { lean * (0.35 + 0.65 * Self::bend(s, stiff)) }
    /// How many segments it is drawn in, `len` logical px long: 10 over 48 px, 6 over 24, else 2.
    pub fn segments(len: f32) -> usize { if len > 48.0 { 10 } else if len > 24.0 { 6 } else { 2 } }
    /// Where the `k`th of `n` segment ends is, as a share of the length: evenly by `(s + 3 bend) / 4`,
    /// so the segments are shorter where it bends (each turns at most `(lean − 0.35 lean) × 4 / 3n`).
    pub fn joint(k: usize, n: usize, stiff: f32) -> f32 {
        let want = k as f32 / n as f32;
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..20 {
            let m = (lo + hi) * 0.5;
            if (m + 3.0 * Self::bend(m, stiff)) * 0.25 < want { lo = m } else { hi = m }
        }
        (lo + hi) * 0.5
    }
    /// The point a share `s` along it, `len` px long, from the root at (0, 0) (up is −y): walked by its
    /// length, so the length is kept.
    pub fn at(s: f32, lean: f32, stiff: f32, len: f32) -> [f32; 2] {
        let d = s * len / BLADE_WALK as f32;
        let mut p = [0.0f32; 2];
        for j in 0..BLADE_WALK {
            let a = Self::tangent((j as f32 + 0.5) / BLADE_WALK as f32 * s, lean, stiff);
            p = [p[0] + a.sin() * d, p[1] - a.cos() * d];
        }
        p
    }
    /// Its joints as the shader draws it, `len` logical px long (`segments`) and `len × scale` device
    /// px: root, the ends of its segments, tip.
    pub fn shape(lean: f32, stiff: f32, len: f32, scale: f32) -> Vec<[f32; 2]> {
        let n = Self::segments(len);
        (0..=n).map(|k| Self::at(Self::joint(k, n, stiff), lean, stiff, len * scale)).collect()
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

/// The most colours of their own the drawings of one frame have between them (`Drawing::palette`).
pub const OWN_COLOURS: usize = 32;

/// Marks in a `w × h` space; the element shows the space scaled to its width (its height follows).
/// `palette`: its own colours (`Drawing::palette`), which its marks' `Paint::hue(3 + k)` name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing { pub w: f32, pub h: f32, pub marks: Vec<Mark>, pub palette: Vec<[f32; 3]> }
impl Drawing {
    pub fn new(w: f32, h: f32) -> Self { Self { w, h, marks: Vec::new(), palette: Vec::new() } }
    /// Colours of the drawing's own (sRGB, 0–1), whatever the theme: a mark's `Paint::hue(3 + k)` is
    /// colour `k` in place of a theme accent, its tone and accent mixing towards it as towards one, so a
    /// drawing looks the same wherever it is drawn (its page, a window onto it, a band). Over a picture,
    /// where ink is written as its own colour, it is exact and a ramp between two of them is mixed in
    /// OKLab; off a picture, where ink is one of the theme's accents a pixel, it is drawn in hue 0 and
    /// reported (`ViewProblem::OwnColourOffPicture`). The drawings of a frame have at most
    /// `OWN_COLOURS` between them (the same colour in two counts once); past that, and for a `k` past the
    /// palette, a mark is in hue 0 (DESIGN.md §1.26).
    pub fn palette(&mut self, colours: impl IntoIterator<Item = [f32; 3]>) { self.palette = colours.into_iter().collect(); }
    pub fn dot(&mut self, x: f32, y: f32, r: f32, paint: Paint) { if r > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Dot { x, y, r, paint }); } }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, paint: Paint) { if w > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Line { a, b, w, paint, far: None, mid: None }); } }
    /// A stroke whose colour and opacity go from `paint` at `a` to `far` at `b`, through `mid` (a paint
    /// and where it is, 0–1 of the way from `a`) if given (DESIGN.md §1.26). Over a picture the colours
    /// are mixed in OKLab, each piece linear, the opacity linear and applied after; elsewhere the
    /// paint's numbers (tone, accent, opacity) are mixed in a line and it keeps `paint`'s hue (a ramp
    /// from one hue to another there is reported by `Ui::problems`). Each round end is its end's colour.
    pub fn ramp(&mut self, a: (f32, f32), b: (f32, f32), w: f32, paint: Paint, mid: Option<(Paint, f32)>, far: Paint) {
        if w > 0.0 && (paint.alpha > 0.0 || far.alpha > 0.0 || mid.is_some_and(|m| m.0.alpha > 0.0)) {
            self.marks.push(Mark::Line { a, b, w, paint, far: Some(far), mid: mid.map(|(p, at)| (p, at.clamp(0.0, 1.0))) });
        }
    }
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
