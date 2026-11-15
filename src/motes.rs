//! Motes (DESIGN.md §1.22): small marks the runtime moves by itself, as data on elements. A moving
//! element can shed them (`Shed`): they leave its box as it moves, fall and drift, and fade out; an
//! element that stands still sheds nothing and costs nothing. A ground can carry a flow field
//! (`FlowField`): marks advected along a fixed, continuous noise field, which move only while its speed
//! is above 0 (the page sets the speed: a chapter declares it active). No GPU here.
use serde::{Deserialize, Serialize};

/// What a moving element sheds: `rate` marks per 100 logical px it moves, each living `life_ms`,
/// falling with `gravity` (px/s²), pushed sideways by up to `drift` (px/s), `size` px across.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shed { pub rate: f32, pub life_ms: f32, pub gravity: f32, pub drift: f32, pub size: f32 }
impl Default for Shed { fn default() -> Self { Self { rate: 0.0, life_ms: 900.0, gravity: 240.0, drift: 30.0, size: 2.5 } } }

/// Marks on a ground carried along a noise field: features `scale` px apart, `speed` px/s (0: still),
/// `density` marks per 10,000 px², each `size` px, drawn in the ink at `alpha`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlowField { pub scale: f32, pub speed: f32, pub density: f32, pub size: f32, pub alpha: f32 }
impl Default for FlowField { fn default() -> Self { Self { scale: 140.0, speed: 0.0, density: 6.0, size: 2.0, alpha: 0.4 } } }

/// One shed mark: where it is (logical px), how it moves, when it was made, what it looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mote { pub x: f32, pub y: f32, pub vx: f32, pub vy: f32, pub born: f32, pub life: f32, pub size: f32, pub gravity: f32, pub paint: crate::el::Paint }

/// A hash in 0..1 of an integer (deterministic: the same frames give the same bytes).
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
/// The field's direction at (x, y) for features `scale` apart: the noise turned into an angle (two
/// turns over its range, so neighbouring streams curl around each other).
pub fn direction(x: f32, y: f32, scale: f32) -> (f32, f32) {
    let a = noise(x / scale.max(1.0), y / scale.max(1.0)) * std::f32::consts::TAU * 2.0;
    (a.cos(), a.sin())
}

impl FlowField {
    /// How many marks a box of `w × h` px carries.
    pub fn count(&self, w: f32, h: f32) -> usize { ((w * h / 10_000.0) * self.density.max(0.0)).round().clamp(0.0, 4000.0) as usize }
    /// Where mark `i` starts in a `w × h` box (deterministic).
    pub fn seed(i: usize, w: f32, h: f32) -> (f32, f32) { (hash(i as u32 * 2 + 1) * w, hash(i as u32 * 2 + 2) * h) }
    /// Move a mark at (x, y) in a `w × h` box by `dt` seconds; one that leaves the box comes in again on
    /// the opposite side.
    pub fn advect(&self, p: (f32, f32), w: f32, h: f32, dt: f32) -> (f32, f32) {
        let (dx, dy) = direction(p.0, p.1, self.scale);
        ((p.0 + dx * self.speed * dt).rem_euclid(w.max(1.0)), (p.1 + dy * self.speed * dt).rem_euclid(h.max(1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_field_is_continuous_and_still_at_zero_speed() {
        // Continuous: a small step changes the direction a little.
        for i in 0..200 {
            let (x, y) = (hash(i) * 900.0, hash(i + 999) * 600.0);
            let (a, b) = (direction(x, y, 140.0), direction(x + 0.5, y, 140.0));
            assert!((a.0 - b.0).abs() + (a.1 - b.1).abs() < 0.2, "{a:?} {b:?}");
        }
        let f = FlowField::default();
        assert_eq!(f.advect((10.0, 20.0), 100.0, 100.0, 0.5), (10.0, 20.0));
        let g = FlowField { speed: 40.0, ..f };
        let p = g.advect((10.0, 20.0), 100.0, 100.0, 0.5);
        assert!(((p.0 - 10.0).powi(2) + (p.1 - 20.0).powi(2)).sqrt() > 19.0);
        assert_eq!(f.count(100.0, 100.0), 6);
    }
}
