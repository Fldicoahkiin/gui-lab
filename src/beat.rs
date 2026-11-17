//! The beat (DESIGN.md §1.32): a look's tempo, cut into steps, that the things that move on their own
//! fall on: a front's radius, glitchr's held rows, scribblr's held pose, a transition's steps. A bar
//! is four beats; each beat is `steps` steps; `swing` pushes every second step later by that share of
//! a step. `bpm` 0 is off: every clock is the runtime's own, byte for byte as without a beat.
//!
//! The beat clock runs with the runtime's clock, behind it by an offset that only the beat's own
//! commands change: `Cmd::Hold` stops it for some steps, `Cmd::Retrigger` plays the current step again
//! from its start, `Cmd::Rewind` takes it back some beats. Nothing else (the application's state, the
//! field map, condensing) is taken back.
#![warn(missing_docs)]
use serde::{Deserialize, Serialize};

/// A look's tempo.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beat {
    /// Beats a minute, 40–300; 0 is off (the default).
    pub bpm: f32,
    /// Steps a beat, 1–8 (4).
    pub steps: u8,
    /// How much later every second step starts, 0–0.5 of a step (0).
    pub swing: f32,
}
impl Default for Beat { fn default() -> Self { Self { bpm: 0.0, steps: 4, swing: 0.0 } } }

/// Beats in a bar.
pub const BAR: u32 = 4;

/// Where the beat clock is: the bar, the beat in it and the step in that (each from 1), and how far
/// into the step (0..1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BeatNow {
    /// The bar, from 1.
    pub bar: u32,
    /// The beat in the bar, 1–4.
    pub beat: u32,
    /// The step in the beat, from 1.
    pub step: u32,
    /// How far into the step, 0..1.
    pub phase: f32,
}

impl Beat {
    /// The beat with its numbers in range (`bpm` 0 stays off).
    pub fn clamped(self) -> Self {
        Self { bpm: if self.bpm <= 0.0 { 0.0 } else { self.bpm.clamp(40.0, 300.0) }, steps: self.steps.clamp(1, 8), swing: self.swing.clamp(0.0, 0.5) }
    }
    /// Whether it is on.
    pub fn on(&self) -> bool { self.bpm > 0.0 }
    /// A beat, s.
    pub fn beat_s(&self) -> f32 { let b = self.clamped(); if b.on() { 60.0 / b.bpm } else { 0.0 } }
    /// A step, s (0 when off).
    pub fn step_s(&self) -> f32 { let b = self.clamped(); if b.on() { 60.0 / (b.bpm * b.steps as f32) } else { 0.0 } }
    /// When step `k` (from 0) starts, s: steps go in pairs, the second of each pair `swing` of a step late.
    pub fn step_start(&self, k: i64) -> f32 {
        let (st, sw) = (self.step_s(), self.clamped().swing);
        let pair = k.div_euclid(2) as f32 * 2.0 * st;
        if k.rem_euclid(2) == 1 { pair + (1.0 + sw) * st } else { pair }
    }
    /// The step `t` s falls in (from 0).
    pub fn step_at(&self, t: f32) -> i64 {
        let st = self.step_s();
        if st <= 0.0 { return 0; }
        let pair = (t / (2.0 * st)).floor() as i64;
        let k = pair * 2 + 1;
        if t + 1e-6 >= self.step_start(k) { k } else { k - 1 }
    }
    /// `t` taken back to the start of its step: what falls on the grid sees only these times. Off, `t`
    /// itself.
    pub fn grid(&self, t: f32) -> f32 { if self.on() { self.step_start(self.step_at(t)) } else { t } }
    /// Where `t` s on the beat clock is.
    pub fn now(&self, t: f32) -> BeatNow {
        if !self.on() { return BeatNow { bar: 1, beat: 1, step: 1, phase: 0.0 }; }
        let steps = self.clamped().steps as i64;
        let k = self.step_at(t.max(0.0));
        let (a, b) = (self.step_start(k), self.step_start(k + 1));
        let beats = k.div_euclid(steps);
        BeatNow { bar: (beats.div_euclid(BAR as i64) + 1) as u32, beat: (beats.rem_euclid(BAR as i64) + 1) as u32, step: (k.rem_euclid(steps) + 1) as u32, phase: ((t - a) / (b - a).max(1e-6)).clamp(0.0, 1.0) }
    }
    /// When bar `bar`, beat `beat`, step `step` (each from 1) starts, s; `None` when the beat is off or a
    /// number is out of range.
    pub fn at(&self, bar: u32, beat: u32, step: u32) -> Option<f32> {
        let steps = self.clamped().steps as u32;
        if !self.on() || bar == 0 || beat == 0 || beat > BAR || step == 0 || step > steps { return None; }
        Some(self.step_start((((bar - 1) * BAR + beat - 1) * steps + step - 1) as i64))
    }
    /// A place on the grid written as `bar.beat.step` (`"3.2.1"`; `"3.2"` is its first step), in s.
    pub fn parse_at(&self, s: &str) -> Option<f32> {
        let n: Vec<u32> = s.trim().split('.').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
        match n[..] { [a, b] => self.at(a, b, 1), [a, b, c] => self.at(a, b, c), _ => None }
    }
    /// The beat a look recipe from before the beat (`look:v2:`) meant by holding scribblr's pose for
    /// `hold_ms`: one step that long, at the fewest steps a beat that keep the tempo in range.
    pub fn of_hold(hold_ms: f32) -> Self {
        if hold_ms <= 0.0 { return Self::default(); }
        let steps = (1..=8u8).find(|s| 60000.0 / (hold_ms * *s as f32) <= 300.0).unwrap_or(8);
        Self { bpm: ((60000.0 / (hold_ms * steps as f32)) * 100.0).round() / 100.0, steps, swing: 0.0 }
    }
}

/// The beat clock's place against the runtime's clock: how far behind it is, and a hold in progress
/// (the beat clock's time it stopped at, and the runtime's time it starts again).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    /// How far the beat clock is behind the runtime's, s.
    pub offset: f32,
    /// A hold: the beat time it stopped at and the runtime time it goes on from.
    pub hold: Option<(f32, f32)>,
}
impl Clock {
    /// The beat clock at runtime time `now` (continuous; `Beat::grid` puts it on a step).
    pub fn at(&self, now: f32) -> f32 { match self.hold { Some((stopped, until)) if now < until => stopped, _ => now - self.offset } }
    /// Bring a finished hold into the offset (call each frame).
    pub fn step(&mut self, now: f32) {
        if let Some((stopped, until)) = self.hold && now >= until { self.offset = until - stopped; self.hold = None; }
    }
    /// Stop the beat clock for `n` steps of `beat` from `now`.
    pub fn hold(&mut self, beat: &Beat, now: f32, n: u8) {
        if !beat.on() || n == 0 { return; }
        let at = self.at(now);
        let until = match self.hold { Some((_, u)) if now < u => u, _ => now } + n as f32 * beat.step_s();
        self.hold = Some((at, until));
    }
    /// Play the current step again from its start.
    pub fn retrigger(&mut self, beat: &Beat, now: f32) {
        if !beat.on() { return; }
        let at = self.at(now);
        let back = at - beat.grid(at);
        self.shift(now, back);
    }
    /// Take the beat clock back `beats` beats (never before 0).
    pub fn rewind(&mut self, beat: &Beat, now: f32, beats: f32) {
        if !beat.on() { return; }
        let back = (beats.max(0.0) * beat.beat_s()).min(self.at(now).max(0.0));
        self.shift(now, back);
    }
    fn shift(&mut self, now: f32, back: f32) {
        match &mut self.hold { Some((stopped, until)) if now < *until => *stopped -= back, _ => self.offset += back }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steps_bars_and_swing() {
        let b = Beat { bpm: 120.0, steps: 4, swing: 0.0 };
        assert!((b.step_s() - 0.125).abs() < 1e-6);
        assert_eq!(b.grid(0.3), 0.25);
        let n = b.now(2.0 + 0.5 + 0.125 + 0.06);
        assert!((n.bar, n.beat, n.step) == (2, 2, 2) && (n.phase - 0.48).abs() < 1e-4, "{n:?}");
        assert!((b.parse_at("3.2.1").unwrap() - (2.0 * 2.0 + 0.5)).abs() < 1e-6);
        assert_eq!(b.parse_at("3.2"), b.parse_at("3.2.1"));
        assert!(b.parse_at("3.5.1").is_none() && b.parse_at("x").is_none());
        let s = Beat { swing: 0.5, ..b };
        assert!((s.step_start(1) - 0.1875).abs() < 1e-6 && (s.step_start(2) - 0.25).abs() < 1e-6);
        assert_eq!(s.step_at(0.18), 0);
        assert_eq!(s.step_at(0.19), 1);
        assert_eq!(Beat::default().grid(0.3), 0.3);
        assert!(Beat::default().parse_at("1.1.1").is_none());
    }
    #[test]
    fn a_hold_stops_it_and_a_rewind_takes_it_back() {
        let b = Beat { bpm: 120.0, steps: 4, swing: 0.0 };
        let mut c = Clock::default();
        c.hold(&b, 1.0, 2);
        assert_eq!(c.at(1.1), 1.0);
        c.step(1.3);
        assert!((c.at(1.3) - 1.05).abs() < 1e-6);
        c.rewind(&b, 1.3, 1.0);
        assert!((c.at(1.3) - 0.55).abs() < 1e-6);
        c.retrigger(&b, 1.3);
        assert!((c.at(1.3) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn a_hold_in_ms_is_one_step_that_long() {
        let b = Beat::of_hold(83.0);
        assert!((b.step_s() * 1000.0 - 83.0).abs() < 0.05 && b.bpm <= 300.0, "{b:?}");
        assert_eq!(Beat::of_hold(250.0), Beat { bpm: 240.0, steps: 1, swing: 0.0 });
    }
}
