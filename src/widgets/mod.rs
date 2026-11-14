//! Widgets are plain functions that return element trees. Each one is a file so it can be copied out
//! and changed (`registry.json` at the repository root lists the files and what each depends on).
mod button;
mod card;
mod choice;
mod dialog;
mod divider;
mod image;
mod input;
mod list;
mod menu;
mod number;
mod scroll;
mod slider;
mod switch;
mod tabs;

pub use button::{Variant, button, button_as};
pub use card::card;
pub use choice::{choice, segmented};
pub use dialog::dialog;
pub use divider::{divider, vdivider};
pub use image::image;
pub use input::{TextEdit, input, textarea};
pub use list::list;
pub use menu::menu;
pub use number::{Edit, SCRUB, Spec, number};
pub use scroll::scroll_area;
pub use slider::{slider, slider_value};
pub use switch::{switch, switch_sized};
pub use tabs::tabs;

/// Sizes shared by the widgets, logical px.
pub const LABEL: f32 = 14.0;
pub const CONTROL_H: f32 = 30.0;
pub const LINE: f32 = 1.0;
pub const RADIUS: f32 = 4.0;
/// The accent colour at low strength, for what is selected.
pub const SELECTED: crate::el::Paint = crate::el::Paint { tone: 0.0, accent: 1.0, alpha: 0.14, hue: 0 };
