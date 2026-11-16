//! The frame budget for small moving marks (DESIGN.md §1.27): particles of a morph, shed motes, a flow
//! field's marks, an experiment's swarm and a drawing's marks all draw from one count per frame instead
//! of each owning its own. The budget shrinks when frames run over the target time and grows back when
//! there is room, so a page asking for absurd numbers stays bounded and a slow device sheds marks, not
//! frames. One per thread (the runtime and the state it drives run on one), like `i18n::current`.
use std::cell::Cell;

/// The most marks granted in one frame, all effects together, at scale 1.
pub const MAX_MARKS: usize = 24_000;
/// The frame time (ms) the scale steers toward.
pub const TARGET_MS: f32 = 14.0;
/// The smallest scale: something of every effect is always left.
pub const MIN_SCALE: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    /// Granted and asked for in the last whole frame; the most a frame may grant; the scale now.
    pub used: usize, pub asked: usize, pub max: usize, pub scale: f32,
}
thread_local! {
    static STATE: Cell<(Usage, usize, usize, u32, u32)> = const { Cell::new((Usage { used: 0, asked: 0, max: MAX_MARKS, scale: 1.0 }, 0, 0, 0, 0)) };
}
fn with<R>(f: impl FnOnce(&mut (Usage, usize, usize, u32, u32)) -> R) -> R { STATE.with(|s| { let mut v = s.get(); let r = f(&mut v); s.set(v); r }) }

/// Ask for `want` marks this frame: what is granted is `want × scale`, and never more than what is left
/// of this frame's `max`.
pub fn grant(want: usize) -> usize {
    with(|(u, used, asked, _, _)| {
        let room = ((u.max as f32 * u.scale) as usize).saturating_sub(*used);
        let n = ((want as f32 * u.scale).round() as usize).min(room);
        *used += n;
        *asked += want;
        n
    })
}
/// The most marks one drawing may show (a hard limit, whatever it asks): `max`.
pub fn cap() -> usize { with(|(u, ..)| u.max) }
/// A new frame starts: what the last one used becomes the usage.
pub fn begin_frame() { with(|(u, used, asked, _, _)| { u.used = std::mem::take(used); u.asked = std::mem::take(asked); }) }
/// A frame was drawn and cost `ms`: over the target for 8 frames running, the scale drops by a fifth;
/// under half of it for 60 frames running, it grows by a tenth (to 1).
pub fn feed(ms: f32) {
    with(|(u, _, _, bad, good)| {
        if ms > TARGET_MS { *bad += 1; *good = 0; } else if ms < TARGET_MS * 0.5 { *good += 1; *bad = 0; } else { *bad = 0; *good = 0; }
        if *bad >= 8 { u.scale = (u.scale * 0.8).max(MIN_SCALE); *bad = 0; }
        if *good >= 60 { u.scale = (u.scale * 1.1).min(1.0); *good = 0; }
    })
}
pub fn usage() -> Usage { with(|(u, ..)| *u) }
/// Set the most marks per frame and the scale (tests, a host that knows its device).
pub fn set(max: usize, scale: f32) { with(|(u, ..)| { u.max = max; u.scale = scale.clamp(MIN_SCALE, 1.0); }) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_are_bounded_and_scale_follows_the_frame_time() {
        set(MAX_MARKS, 1.0);
        begin_frame();
        assert_eq!(grant(100), 100);
        assert!(grant(10_000_000) <= MAX_MARKS - 100);
        assert_eq!(grant(5), 0, "the frame's budget is spent");
        begin_frame();
        assert_eq!(usage().asked, 10_000_105);
        for _ in 0..8 { feed(40.0); }
        assert!((usage().scale - 0.8).abs() < 1e-6);
        for _ in 0..600 { feed(2.0); }
        assert_eq!(usage().scale, 1.0);
    }
}
