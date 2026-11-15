//! skeleton
//!
//! Purpose: Hold the place of content that is still loading, in its shape.
//! Props: `key`, `width`, `height`, `round` (a disc rather than a bar).
//! States: none (it stays still: no shimmer; loading is not an animation here).
//! Messages: none.
//! Usage: `skeleton("title", 220.0, 16.0, false)`
//!
//! A faint block; replaced by the content when it arrives (keyed text then condenses in).
use super::RADIUS;
use crate::el::{El, Paint, space};

pub fn skeleton<M>(key: &str, width: f32, height: f32, round: bool) -> El<M> {
    space(width, height).key(key).radius(if round { width.min(height) * 0.5 } else { RADIUS }).fill(Paint::FAINT)
}
