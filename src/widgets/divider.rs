//! divider
//!
//! Purpose: A thin line between groups.
//! Props: `thickness` (logical px).
//! States: none.
//! Messages: none.
//! Usage: `divider(1.0)` or `vdivider(1.0)`
//!
//! A thin line between groups: `divider` across, `vdivider` down. `thickness` is logical px.
use crate::el::{El, Paint, space};

pub fn divider<M>(thickness: f32) -> El<M> { space(0.0, thickness).fill_width().fill(Paint::FAINT) }
pub fn vdivider<M>(thickness: f32) -> El<M> { space(thickness, 0.0).fill_height().fill(Paint::FAINT) }
