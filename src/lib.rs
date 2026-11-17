extern crate self as amenui;

pub mod add;
pub mod budget;
pub mod decode;
pub mod dirty;
pub mod draw;
pub mod el;
pub mod error;
pub mod experiment;
pub mod field;
pub mod fieldmap;
pub mod strand;
pub mod gaze;
pub mod gesture;
pub mod i18n;
pub mod markdown;
pub mod marks;
pub mod mesh;
pub mod morph;
pub mod motes;
pub mod perf;
pub mod pictures;
pub mod post;
pub mod preset;
#[cfg(not(target_arch = "wasm32"))]
pub mod offscreen;
#[cfg(not(target_arch = "wasm32"))]
pub mod drive;
pub mod recipes;
pub mod sources;
/// AGENTS.md, so `cargo test` compiles every example in it.
#[cfg(doctest)]
#[doc = include_str!("../AGENTS.md")]
pub mod agents {}
/// README.md, so its quick start is compiled and run.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub mod readme {}
pub mod render;
pub mod scene;
pub mod style;
pub mod text;
pub mod tree;
pub mod ui;
pub mod widgets;
pub mod wrap;
