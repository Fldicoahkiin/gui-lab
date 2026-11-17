//! Running an application: a native window (`native::run`, feature `window`) or a canvas on a web page
//! (`web::Host` and the `web_app!` macro). Both forward the pointer, wheel, keys, text, the input method,
//! the clipboard and the size to the runtime, draw frames only when the runtime needs one, and keep the
//! accessibility mirror (web) up to date.
//!
//! The application is a `Ui` built by a function the shell calls once it knows the window's size and
//! scale, and a state that implements [`App`] for what the shell needs beyond the view and update.
//!
//! ```no_run
//! use amenui::el::{El, Paint, col, text};
//! use amenui::widgets::button;
//!
//! #[derive(Clone)]
//! enum Msg { Inc }
//! struct Counter { n: u32 }
//! impl amenui::app::App for Counter {}
//!
//! fn view(s: &Counter) -> El<Msg> {
//!     col().pad(24.0, 24.0).gap(12.0)
//!         .child(text(s.n.to_string(), 32.0, Paint::INK).key("count"))
//!         .child(button("inc", "Add one", Msg::Inc))
//! }
//! fn update(s: &mut Counter, m: Msg) { match m { Msg::Inc => s.n += 1 } }
//!
//! # #[cfg(feature = "window")]
//! fn main() {
//!     let window = amenui::app::Window::new("Counter").size(400.0, 300.0).from_args();
//!     let made = amenui::app::native::run(window, |_| {}, |faces, w, h, scale| {
//!         amenui::ui::Ui::new(Counter { n: 0 }, view, update, faces, w, h, scale)
//!     });
//!     if let Err(e) = made { eprintln!("{e}"); }
//! }
//! # #[cfg(not(feature = "window"))]
//! # fn main() {}
//! ```

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub mod native;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub mod access;
#[cfg(target_arch = "wasm32")]
pub mod web;

use crate::i18n::Locale;

/// What the shell asks of the application's state, beyond its view and update. Every method has a
/// default, so `impl App for MyState {}` is enough for an application with one page.
pub trait App {
    /// A route from outside (the web page's `location.hash`, the window's start page). Returns whether
    /// it names something this application shows; the shell lays out again when it does.
    fn route(&mut self, _hash: &str) -> bool { false }
    /// The current route, for the web page to put in `location.hash` (empty: leave it as it is).
    fn hash(&self) -> String { String::new() }
    /// Text the application asked to put on the system clipboard (a copy button), taken once.
    fn take_clipboard(&mut self) -> Option<String> { None }
    /// The language, from the host (`<amenui-view lang="en">`). Returns whether it changed anything.
    fn set_locale(&mut self, _locale: Locale) -> bool { false }
    /// Theme and material by name, from the host (`<amenui-view theme="dark" material="dot">`); an empty
    /// or unknown name leaves that one as it is. Returns whether it changed anything.
    fn set_look(&mut self, _theme: &str, _material: &str) -> bool { false }
    /// The system's reduced-motion preference (the runtime's own parts follow it already).
    fn set_reduced_motion(&mut self, _on: bool) {}
    /// The system's appearance: at start and whenever it changes. An application that follows the
    /// system picks its theme here. Returns whether it changed anything (the shell lays out again).
    fn set_appearance(&mut self, _appearance: Appearance) -> bool { false }
    /// Files were dropped on the window or the canvas. Returns whether anything changed.
    fn files_dropped(&mut self, _files: Vec<Dropped>) -> bool { false }
    /// Files are being dragged over the window (`true`) or left it without a drop (`false`).
    /// Returns whether anything changed.
    fn files_hovering(&mut self, _on: bool) -> bool { false }
}

/// The system's appearance preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    /// The system asks for a dark appearance.
    pub dark: bool,
    /// The system asks for more contrast (the web page's `prefers-contrast: more`, Windows' high
    /// contrast through the browser). Native windows report `false`: winit does not expose it.
    pub high_contrast: bool,
}

/// A file dropped on the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dropped {
    /// The file's name, without its folder.
    pub name: String,
    /// Where it is (native windows; the application reads it when it wants).
    pub path: Option<std::path::PathBuf>,
    /// Its contents (web pages, which cannot see paths).
    pub bytes: Option<Vec<u8>>,
}

impl Dropped {
    /// A file on disk.
    pub fn path(path: std::path::PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self { name, path: Some(path), bytes: None }
    }
    /// A file's name and contents.
    pub fn bytes(name: impl Into<String>, bytes: Vec<u8>) -> Self { Self { name: name.into(), path: None, bytes: Some(bytes) } }
}

/// A native window's options. `Window::from_args` reads the standard command line:
/// `[route] [--shot <path.png>@<ms> …] [--exit-after <ms>]`.
#[derive(Clone, Debug)]
pub struct Window {
    pub title: String,
    /// Logical px.
    pub size: (f32, f32),
    /// The route to open with ([`App::route`]).
    pub route: Option<String>,
    /// Frames to read back and save as PNG, at ms after start.
    pub shots: Vec<(String, f32)>,
    /// Close the window this many ms after start.
    pub exit_after: Option<f32>,
    /// Print the adapter and, every two seconds, how many frames were drawn, on stderr.
    pub report: bool,
}

impl Window {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), size: (1280.0, 860.0), route: None, shots: vec![], exit_after: None, report: false }
    }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.size = (w, h); self }
    pub fn route(mut self, route: impl Into<String>) -> Self { self.route = Some(route.into()); self }
    /// The options from the process's command line (after the program name), over these.
    pub fn from_args(self) -> Self { self.with_args(std::env::args().skip(1)) }
    /// The options from a command line: a route, `--shot <path.png>@<ms>` (any number), `--exit-after <ms>`.
    /// A shot or an exit time turns the report on.
    pub fn with_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        let mut args = args.into_iter();
        while let Some(a) = args.next() {
            match a.as_str() {
                "--shot" => {
                    if let Some((p, t)) = args.next().as_deref().and_then(|v| v.rsplit_once('@')) && let Ok(t) = t.parse::<f32>() { self.shots.push((p.to_owned(), t)); }
                }
                "--exit-after" => self.exit_after = args.next().and_then(|v| v.parse().ok()),
                _ => self.route = Some(a),
            }
        }
        self.shots.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.report |= !self.shots.is_empty() || self.exit_after.is_some();
        self
    }
}

/// The accessibility mirror's changes as JSON, for the web page's hidden DOM tree:
/// `[{"set": {...}}, {"remove": key}, {"focus": key}]`.
pub fn a11y_json(ops: &[crate::ui::A11yOp]) -> String {
    use crate::ui::A11yOp;
    use serde_json::{Value, json};
    let ops: Vec<Value> = ops.iter().map(|op| match op {
        A11yOp::Remove(k) => json!({ "remove": k }),
        A11yOp::Focus(k) => json!({ "focus": k }),
        A11yOp::Set(n) => {
            let mut f = serde_json::Map::new();
            f.insert("key".into(), json!(n.key));
            f.insert("role".into(), json!(n.role.aria()));
            f.insert("name".into(), json!(n.name));
            f.insert("rect".into(), json!([n.rect.x, n.rect.y, n.rect.w, n.rect.h]));
            if let Some(p) = &n.parent { f.insert("parent".into(), json!(p)); }
            if let Some((v, lo, hi)) = n.value { f.insert("value".into(), json!([v, lo, hi])); }
            if let Some(t) = &n.text { f.insert("text".into(), json!(t)); }
            if let Some(c) = n.checked { f.insert("checked".into(), json!(c)); }
            if let Some(c) = n.selected { f.insert("selected".into(), json!(c)); }
            if n.level > 0 { f.insert("level".into(), json!(n.level)); }
            if let Some(e) = n.expanded { f.insert("expanded".into(), json!(e)); }
            if let Some(d) = &n.description { f.insert("description".into(), json!(d)); }
            if let Some(l) = n.live { f.insert("live".into(), json!(l.aria())); }
            if n.invalid { f.insert("invalid".into(), json!(true)); }
            if n.required { f.insert("required".into(), json!(true)); }
            if n.modal { f.insert("modal".into(), json!(true)); }
            if n.disabled { f.insert("disabled".into(), json!(true)); }
            if n.focusable { f.insert("focusable".into(), json!(true)); }
            json!({ "set": Value::Object(f) })
        }
    }).collect();
    Value::Array(ops).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_reads_the_standard_command_line() {
        let w = Window::new("t").with_args(["dialog", "--shot", "b.png@2000", "--exit-after", "3000", "--shot", "a.png@1000"].map(String::from));
        assert_eq!(w.route.as_deref(), Some("dialog"));
        assert_eq!(w.shots, vec![("a.png".to_owned(), 1000.0), ("b.png".to_owned(), 2000.0)]);
        assert_eq!(w.exit_after, Some(3000.0));
        assert!(w.report);
        assert!(!Window::new("t").with_args([]).report);
    }

    #[test]
    fn a_dropped_path_is_named_by_its_file() {
        let d = Dropped::path("/tmp/notes/a b.txt".into());
        assert_eq!(d.name, "a b.txt");
        assert!(d.bytes.is_none());
    }
}
