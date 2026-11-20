//! The stage (`El::stage`, DESIGN.md §1.35): when the piece a page is about is slow to draw, it holds the
//! image it last drew on frames that answer input, so the controls around it answer in that frame.
//!
//! The rules (the design's ruling on a slow stage): a stage is slow when its draws average more than
//! `SLOW` of a frame over the last `DRAWS`, and no longer slow after `CALM_S` of draws under `FAST` of a
//! frame. A slow stage draws on the first frame without new input, on the first frame after a release,
//! and during a long stream of input at least every `max(HOLD_MIN_S, HOLD_DRAWS × its draw)`, at most
//! `HOLD_MAX_S`. Its image is replaced whole when it draws, and nothing marks it while it holds.
use crate::scene::Instance;
use crate::ui::Rect;

/// Draws averaged to decide whether a stage is slow.
pub const DRAWS: usize = 8;
/// A stage whose draws average more than this share of a frame is slow.
pub const SLOW: f32 = 0.6;
/// A slow stage whose draws stay under this share of a frame for `CALM_S` is no longer slow.
pub const FAST: f32 = 0.4;
/// How long a slow stage's draws must stay under `FAST` before it is no longer slow, s.
pub const CALM_S: f32 = 1.0;
/// The least time a slow stage holds during a stream of input, s.
pub const HOLD_MIN_S: f32 = 0.1;
/// A slow stage holds at least this many of its draws' time during a stream of input.
pub const HOLD_DRAWS: f32 = 6.0;
/// The longest a slow stage holds during a stream of input, s.
pub const HOLD_MAX_S: f32 = 0.5;

/// One stage's last image and how it has been drawing.
#[derive(Default)]
pub(crate) struct Stage {
    /// Its instances as last drawn, with the node (from the stage's own, counted in paint order) and
    /// identity each belongs to, and their flow tags.
    pub snap: Vec<Instance>,
    pub owners: Vec<(usize, u64)>,
    pub tags: Vec<Option<crate::ui::Tag>>,
    /// The text births of its text when last drawn.
    pub texts: Vec<(u64, f32)>,
    /// Keys of its text that recomposes.
    pub recs: Vec<String>,
    /// Its see-through groups, from its first instance.
    pub groups: Vec<crate::scene::Group>,
    /// Where its instances were in the scene of the paint that drew it.
    pub span: (usize, usize),
    /// Its box and clip, and how many nodes it had, when last drawn: if these change it draws.
    pub rect: Rect,
    pub clip: Rect,
    pub nodes: usize,
    /// Its last draws, ms.
    pub draws: Vec<f32>,
    pub slow: bool,
    /// Since when its draws have been under `FAST` while slow.
    pub calm_since: Option<f32>,
    /// When it last drew (the runtime's clock).
    pub drawn_at: f32,
    /// It held through a paint: what it shows may be behind the view.
    pub behind: bool,
    /// It was drawn at least once.
    pub drawn: bool,
    /// A part of it was drawn again alone since it last drew (`El::steps` inside it): its image is not
    /// what it shows, so it does not hold.
    pub stale: bool,
    /// How many times it has drawn.
    pub count: usize,
}

impl Stage {
    /// Its draws' average, ms.
    pub fn average(&self) -> f32 { if self.draws.is_empty() { 0.0 } else { self.draws.iter().sum::<f32>() / self.draws.len() as f32 } }
    /// The longest it holds during a stream of input, s.
    pub fn hold_s(&self) -> f32 { (HOLD_DRAWS * self.average() / 1000.0).clamp(HOLD_MIN_S, HOLD_MAX_S) }
    /// A draw of `ms` at `now`, with frames of `frame_ms`; `may_hold`: its scale is as low as it goes, so
    /// it may become slow (the scale steps down first).
    pub fn drew(&mut self, ms: f32, now: f32, frame_ms: f32, may_hold: bool) {
        self.draws.push(ms);
        if self.draws.len() > DRAWS { self.draws.remove(0); }
        self.drawn_at = now;
        self.behind = false;
        self.stale = false;
        self.drawn = true;
        self.count += 1;
        let avg = self.average();
        if !self.slow {
            self.slow = may_hold && avg > SLOW * frame_ms;
            self.calm_since = None;
        } else if avg < FAST * frame_ms {
            let since = *self.calm_since.get_or_insert(now);
            if now - since >= CALM_S { self.slow = false; self.calm_since = None; }
        } else {
            self.calm_since = None;
        }
    }
    /// Whether it draws in this paint: `input` came since the last paint, `released` a press ended.
    pub fn draws_now(&self, rect: Rect, clip: Rect, nodes: usize, now: f32, input: bool, released: bool) -> bool {
        !self.drawn || self.stale || !self.slow || !input || released || self.rect != rect || self.clip != clip || self.nodes != nodes || now - self.drawn_at >= self.hold_s() - 1e-4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_after_eight_long_draws_and_fast_after_a_calm_second() {
        let mut s = Stage::default();
        for k in 0..8 { s.drew(40.0, k as f32 * 0.1, 16.7, true); }
        assert!(s.slow);
        assert!((s.hold_s() - 0.24).abs() < 1e-4, "6 × 40 ms: {}", s.hold_s());
        for k in 0..8 { s.drew(2.0, 1.0 + k as f32 * 0.1, 16.7, true); }
        assert!(s.slow, "not yet a calm second");
        for k in 0..12 { s.drew(2.0, 2.0 + k as f32 * 0.1, 16.7, true); }
        assert!(!s.slow);
        let mut q = Stage::default();
        for k in 0..8 { q.drew(200.0, k as f32, 16.7, true); }
        assert_eq!(q.hold_s(), HOLD_MAX_S);
        q.draws = vec![5.0; 8];
        assert_eq!(q.hold_s(), HOLD_MIN_S);
    }
    #[test]
    fn a_slow_stage_a_part_of_which_was_drawn_alone_draws_through_input() {
        let mut s = Stage::default();
        for k in 0..8 { s.drew(40.0, k as f32 * 0.01, 16.7, true); }
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        (s.rect, s.clip) = (r, r);
        assert!(!s.draws_now(r, r, 0, 0.08, true, false), "slow, under input: it holds");
        s.stale = true;
        assert!(s.draws_now(r, r, 0, 0.08, true, false), "its image is not what it shows");
        s.drew(40.0, 0.09, 16.7, true);
        assert!(!s.stale);
    }
}
