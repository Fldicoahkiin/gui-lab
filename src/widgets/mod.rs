//! Widgets are plain functions that return element trees. Each one is a file so it can be copied out
//! and changed.
mod button;
mod choice;
mod number;
mod toggle;

pub use button::button;
pub use choice::choice;
pub use number::{Edit, Spec, number};
pub use toggle::toggle;

/// Sizes shared by the widgets, logical px.
pub const LABEL: f32 = 14.0;
pub const CONTROL_H: f32 = 30.0;
pub const LINE: f32 = 1.0;
pub const RADIUS: f32 = 4.0;
