//! Gesture physics for the pointer layer (DESIGN.md §1.8.4), without a GPU: a damped spring that takes
//! the velocity a drag let go with, a rubber band for dragging past a limit, where a flick would come to
//! rest, and the velocity of the last tenth of a second of a drag.
//!
//! These are state, not animation: a widget keeps an `Animated` in the application's state, calls
//! `Animated::step` on every tick while `moving()` and asks for frames only then (`Ui::on_wake`), so a
//! still widget draws nothing (rule 3).

/// A damped spring (mass on a spring with a dashpot). Stepped exactly (the closed-form solution of the
/// equation of motion), so ten small steps land where one large one does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Force per unit of distance from the target (N/m in px units).
    pub stiffness: f32,
    /// Force per unit of speed against the motion.
    pub damping: f32,
    pub mass: f32,
    /// At rest when it is closer than `rest_distance` (px) to the target and slower than `rest_speed`
    /// (px/s): it is then put on the target and stops asking for frames.
    pub rest_distance: f32,
    pub rest_speed: f32,
}
impl Default for Spring {
    /// Stiffness 380, damping 36, mass 1: a damping ratio of 0.92 (a hair under critical: no visible
    /// bounce from a slow release, a small overshoot after a fast one), settling in about 0.35 s.
    fn default() -> Self { Self { stiffness: 380.0, damping: 36.0, mass: 1.0, rest_distance: 0.5, rest_speed: 8.0 } }
}
impl Spring {
    /// The damping ratio: below 1 it overshoots, 1 is critical, above 1 it creeps.
    pub fn ratio(&self) -> f32 { self.damping / (2.0 * (self.stiffness.max(1e-6) * self.mass.max(1e-6)).sqrt()) }
    /// Position and velocity `dt` seconds later, starting at `x` with velocity `v`, pulled to `target`.
    pub fn step(&self, x: f32, v: f32, target: f32, dt: f32) -> (f32, f32) {
        let (m, k) = (self.mass.max(1e-6), self.stiffness.max(1e-6));
        let w0 = (k / m).sqrt();
        let z = self.ratio();
        let x0 = x - target;
        let (dx, dv) = if (z - 1.0).abs() < 1e-4 {
            let c = v + w0 * x0;
            let e = (-w0 * dt).exp();
            (e * (x0 + c * dt), e * (v - w0 * c * dt))
        } else if z < 1.0 {
            let a = z * w0;
            let wd = w0 * (1.0 - z * z).sqrt();
            let b = (v + a * x0) / wd;
            let (s, c) = (wd * dt).sin_cos();
            let e = (-a * dt).exp();
            (e * (x0 * c + b * s), e * (v * c - s * (a * b + x0 * wd)))
        } else {
            let q = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w0 * (z - q), -w0 * (z + q));
            let c2 = (v - r1 * x0) / (r2 - r1);
            let c1 = x0 - c2;
            let (e1, e2) = ((r1 * dt).exp(), (r2 * dt).exp());
            (c1 * e1 + c2 * e2, r1 * c1 * e1 + r2 * c2 * e2)
        };
        (target + dx, dv)
    }
    /// Whether (x, v) counts as resting on `target`.
    pub fn at_rest(&self, x: f32, v: f32, target: f32) -> bool { (x - target).abs() < self.rest_distance && v.abs() < self.rest_speed }
}

/// One number driven by a spring: where it is, how fast it goes, where it is going. `hold` follows a
/// finger (no spring); `release` hands the finger's velocity to the spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animated {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub spring: Spring,
    /// Moving under the spring (not held, not at rest).
    active: bool,
    last: f32,
}
impl Animated {
    pub fn new(value: f32, spring: Spring) -> Self { Self { value, velocity: 0.0, target: value, spring, active: false, last: 0.0 } }
    /// Spring toward `target` from where it is, with velocity `v` (px/s), from time `now` (s).
    pub fn to(&mut self, target: f32, v: f32, now: f32) { self.target = target; self.velocity = v; self.active = true; self.last = now; }
    /// Put it at `x` and stop (a finger holds it).
    pub fn hold(&mut self, x: f32) { self.value = x; self.velocity = 0.0; self.target = x; self.active = false; }
    pub fn moving(&self) -> bool { self.active }
    /// Advance to `now`; returns whether it moved. It comes to rest exactly on the target.
    pub fn step(&mut self, now: f32) -> bool {
        if !self.active { self.last = now; return false; }
        // Long gaps (a host that slept) are taken in pieces no longer than a frame at 30 Hz, which the
        // closed form does not need but keeps the rest test from skipping past a crossing.
        let mut dt = (now - self.last).max(0.0);
        self.last = now;
        if dt == 0.0 { return false; }
        while dt > 0.0 {
            let h = dt.min(1.0 / 30.0);
            (self.value, self.velocity) = self.spring.step(self.value, self.velocity, self.target, h);
            dt -= h;
        }
        if self.spring.at_rest(self.value, self.velocity, self.target) { self.value = self.target; self.velocity = 0.0; self.active = false; }
        true
    }
}

/// Dragging past a limit resists: `d` px past it shows as `(1 − 1 / (d · coeff / dim + 1)) · dim`, which
/// starts at `coeff` of the finger's movement and never reaches `dim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RubberBand { pub coeff: f32 }
impl Default for RubberBand {
    fn default() -> Self { Self { coeff: 0.55 } }
}
impl RubberBand {
    /// How far past the limit `d` px of finger past it shows (`dim`: the size the pull is measured
    /// against, such as the sheet's height).
    pub fn past(&self, d: f32, dim: f32) -> f32 {
        if d <= 0.0 || dim <= 0.0 { return d.max(0.0); }
        (1.0 - 1.0 / (d * self.coeff / dim + 1.0)) * dim
    }
    /// `x` with everything beyond `[min, max]` banded.
    pub fn clamp(&self, x: f32, min: f32, max: f32, dim: f32) -> f32 {
        if x > max { max + self.past(x - max, dim) } else if x < min { min - self.past(min - x, dim) } else { x }
    }
}

/// Where a flick at `v` px/s would come to rest if it decayed by `decel` every millisecond (0.998 is the
/// usual feel; 0.99 stops quickly): `v / 1000 · decel / (1 − decel)` px further on.
pub fn project(v: f32, decel: f32) -> f32 {
    let d = decel.clamp(0.0, 0.9999);
    v / 1000.0 * d / (1.0 - d)
}

/// The velocity of a drag over its last `window` seconds (default 0.1, as a finger's fling, §1.8.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Velocity { pub window: f32, samples: Vec<(f32, f32, f32)> }
impl Default for Velocity {
    fn default() -> Self { Self { window: 0.1, samples: vec![] } }
}
impl Velocity {
    pub fn clear(&mut self) { self.samples.clear(); }
    /// The pointer at (x, y) at time `t` (s).
    pub fn push(&mut self, t: f32, x: f32, y: f32) {
        self.samples.push((t, x, y));
        let w = self.window;
        while self.samples.len() > 2 && t - self.samples[0].0 > w { self.samples.remove(0); }
    }
    /// px/s in x and y; 0 when the samples span no time.
    pub fn get(&self) -> (f32, f32) {
        let (Some(a), Some(b)) = (self.samples.first(), self.samples.last()) else { return (0.0, 0.0) };
        let dt = b.0 - a.0;
        if dt < 1e-3 { return (0.0, 0.0); }
        ((b.1 - a.1) / dt, (b.2 - a.2) / dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_steps_compose_and_settle() {
        for sp in [Spring::default(), Spring { damping: 2.0 * 380f32.sqrt(), ..Spring::default() }, Spring { damping: 80.0, ..Spring::default() }] {
            let (a, va) = sp.step(100.0, -500.0, 0.0, 0.2);
            let (mut x, mut v) = (100.0, -500.0);
            for _ in 0..20 { (x, v) = sp.step(x, v, 0.0, 0.01); }
            assert!((a - x).abs() < 1e-2 && (va - v).abs() < 1e-1, "{a} {x} / {va} {v} at ratio {}", sp.ratio());
        }
        let mut a = Animated::new(0.0, Spring::default());
        a.to(300.0, 0.0, 0.0);
        let mut t = 0.0;
        while a.moving() && t < 3.0 { t += 1.0 / 60.0; a.step(t); }
        assert!(!a.moving() && a.value == 300.0 && t < 1.0, "settled at {t}");
        assert!(!a.step(t + 1.0), "at rest it does not move");
    }

    #[test]
    fn rubber_band_resists_and_is_bounded() {
        let r = RubberBand::default();
        // At first it follows at `coeff`; it never reaches `dim`.
        assert!((r.past(1.0, 600.0) - 0.55).abs() < 0.01);
        let mut last = 0.0;
        for d in [10.0, 100.0, 1000.0, 1e5, 1e9] {
            let p = r.past(d, 600.0);
            assert!(p > last && p < 600.0, "{d} → {p}");
            last = p;
        }
        assert_eq!(r.clamp(50.0, 0.0, 100.0, 600.0), 50.0);
        assert!(r.clamp(-1e6, 0.0, 100.0, 600.0) > -600.0);
    }

    #[test]
    fn projection_and_velocity() {
        assert!((project(1000.0, 0.998) - 499.0).abs() < 0.5);
        assert!(project(1000.0, 0.99) < 100.0);
        let mut v = Velocity::default();
        for i in 0..20 { v.push(i as f32 / 60.0, 0.0, 10.0 * i as f32); }
        assert!((v.get().1 - 600.0).abs() < 1.0);
    }
}
