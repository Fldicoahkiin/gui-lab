//! Describe: the interface is a tree of plain values. A view is a function from state to this tree.
use std::rc::Rc;
pub use taffy::Style;
use taffy::prelude::*;

/// How an instance is inked. `tone` mixes ground→ink, `accent` mixes that toward the accent colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint { pub tone: f32, pub accent: f32, pub alpha: f32 }
impl Paint {
    pub const fn tone(tone: f32) -> Self { Self { tone, accent: 0.0, alpha: 1.0 } }
    pub const INK: Self = Self::tone(1.0);
    pub const TEXT: Self = Self::tone(0.86);
    pub const DIM: Self = Self::tone(0.5);
    pub const FAINT: Self = Self::tone(0.2);
    /// The page's own ground colour: invisible, but it keeps the ground units out.
    pub const GROUND: Self = Self::tone(0.0);
    pub const ACCENT: Self = Self { tone: 1.0, accent: 1.0, alpha: 1.0 };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Char(char), Enter, Backspace, Escape, Tab, Up, Down, Left, Right, PageUp, PageDown }

impl Key {
    /// From a DOM / winit style key name ("Enter", "ArrowUp", "a", " ").
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Enter" => Key::Enter, "Backspace" => Key::Backspace, "Escape" => Key::Escape, "Tab" => Key::Tab,
            "ArrowUp" => Key::Up, "ArrowDown" => Key::Down, "ArrowLeft" => Key::Left, "ArrowRight" => Key::Right,
            "PageUp" => Key::PageUp, "PageDown" => Key::PageDown,
            _ => {
                let mut cs = name.chars();
                let c = cs.next()?;
                if cs.next().is_some() { return None; }
                Key::Char(c)
            }
        })
    }
}

pub enum Content {
    None,
    Text { s: String, size: f32, line: f32, paint: Paint },
    /// A picture by id, with its height as a fraction of its width.
    Picture { id: u32, ratio: f32 },
}

/// Handles a key while the element has focus; the flag is Shift.
pub type OnKey<M> = Rc<dyn Fn(Key, bool) -> Option<M>>;

pub struct El<M> {
    pub key: String,
    pub style: Style,
    pub content: Content,
    pub fill: Option<Paint>,
    pub border: Option<(f32, Paint)>,
    /// Corner radius of the fill and border, logical px.
    pub radius: f32,
    pub on_press: Option<M>,
    pub on_key: Option<OnKey<M>>,
    /// Sent when focus leaves this element.
    pub on_blur: Option<M>,
    pub focusable: bool,
    /// Scrolls its children vertically and clips them.
    pub scroll: bool,
    pub children: Vec<El<M>>,
}

impl<M> El<M> {
    fn new(style: Style, content: Content) -> Self {
        // Nothing shrinks unless asked: fixed sizes stay fixed inside columns and scroll areas.
        let style = Style { flex_shrink: 0.0, ..style };
        Self { key: String::new(), style, content, fill: None, border: None, radius: 0.0, on_press: None, on_key: None, on_blur: None, focusable: false, scroll: false, children: vec![] }
    }
    pub fn key(mut self, k: impl Into<String>) -> Self { self.key = k.into(); self }
    pub fn gap(mut self, g: f32) -> Self { self.style.gap = Size { width: length(g), height: length(g) }; self }
    pub fn pad(mut self, x: f32, y: f32) -> Self { self.style.padding = Rect { left: length(x), right: length(x), top: length(y), bottom: length(y) }; self }
    pub fn width(mut self, w: f32) -> Self { self.style.size.width = length(w); self }
    pub fn height(mut self, h: f32) -> Self { self.style.size.height = length(h); self }
    pub fn min_width(mut self, w: f32) -> Self { self.style.min_size.width = length(w); self }
    pub fn max_width(mut self, w: f32) -> Self { self.style.max_size.width = length(w); self }
    pub fn fill_width(mut self) -> Self { self.style.size.width = percent(1.0); self }
    pub fn fill_height(mut self) -> Self { self.style.size.height = percent(1.0); self }
    /// Take a share of the free space along the parent's main axis.
    pub fn grow(mut self, g: f32) -> Self { self.style.flex_grow = g; self.style.flex_basis = length(0.0); self.style.flex_shrink = 1.0; self }
    pub fn basis(mut self, b: f32) -> Self { self.style.flex_basis = length(b); self }
    pub fn shrink(mut self) -> Self { self.style.flex_shrink = 1.0; self }
    pub fn wrap(mut self) -> Self { self.style.flex_wrap = FlexWrap::Wrap; self }
    pub fn align(mut self, a: AlignItems) -> Self { self.style.align_items = a; self }
    pub fn justify(mut self, j: JustifyContent) -> Self { self.style.justify_content = j; self }
    pub fn center(self) -> Self { self.align(AlignItems::CENTER).justify(JustifyContent::CENTER) }
    pub fn fill(mut self, p: Paint) -> Self { self.fill = Some(p); self }
    pub fn border(mut self, w: f32, p: Paint) -> Self { self.border = Some((w, p)); self.style.border = Rect { left: length(w), right: length(w), top: length(w), bottom: length(w) }; self }
    pub fn radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn on_press(mut self, m: M) -> Self { self.on_press = Some(m); self.focusable = true; self }
    pub fn on_key(mut self, f: impl Fn(Key, bool) -> Option<M> + 'static) -> Self { self.on_key = Some(Rc::new(f)); self.focusable = true; self }
    pub fn on_blur(mut self, m: M) -> Self { self.on_blur = Some(m); self }
    pub fn scroll(mut self) -> Self { self.scroll = true; self.style.overflow.y = taffy::Overflow::Scroll; self.style.scrollbar_width = 0.0; self }
    pub fn child(mut self, c: El<M>) -> Self { self.children.push(c); self }
    pub fn children(mut self, c: impl IntoIterator<Item = El<M>>) -> Self { self.children.extend(c); self }
}

pub fn row<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Row, ..Default::default() }, Content::None) }
pub fn col<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Default::default() }, Content::None) }
pub fn space<M>(w: f32, h: f32) -> El<M> { El::new(Style { size: Size { width: length(w), height: length(h) }, ..Default::default() }, Content::None) }
pub fn text<M>(s: impl Into<String>, size: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line: (size * 1.6).round(), paint })
}
/// A text leaf with an explicit line height.
pub fn text_lh<M>(s: impl Into<String>, size: f32, line: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line, paint })
}
pub fn picture<M>(id: u32, ratio: f32) -> El<M> { El::new(Style { size: Size { width: percent(1.0), height: auto() }, ..Default::default() }, Content::Picture { id, ratio }) }
