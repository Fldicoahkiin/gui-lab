//! Describe: the interface is a tree of plain values. A view is a function from state to this tree.
use std::rc::Rc;
use crate::wrap::Around;
pub use taffy::Style;
use taffy::prelude::*;

/// How an instance is inked. `tone` mixes ground→ink, `accent` mixes that toward one of the theme's
/// accent colours: `hue` 0 is `accent`, 1 `second`, 2 `third`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint { pub tone: f32, pub accent: f32, pub alpha: f32, pub hue: u8 }
impl Paint {
    pub const fn tone(tone: f32) -> Self { Self { tone, accent: 0.0, alpha: 1.0, hue: 0 } }
    /// Full colour in the theme's accent number `hue` (0, 1, 2).
    pub const fn hue(hue: u8) -> Self { Self { tone: 1.0, accent: 1.0, alpha: 1.0, hue } }
    pub const fn alpha(self, alpha: f32) -> Self { Self { alpha, ..self } }
    pub const INK: Self = Self::tone(1.0);
    pub const TEXT: Self = Self::tone(0.86);
    pub const DIM: Self = Self::tone(0.5);
    pub const FAINT: Self = Self::tone(0.2);
    /// The page's own ground colour: invisible, but it keeps the ground units out.
    pub const GROUND: Self = Self::tone(0.0);
    pub const ACCENT: Self = Self { tone: 1.0, accent: 1.0, alpha: 1.0, hue: 0 };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Char(char), Enter, Backspace, Delete, Escape, Tab, Up, Down, Left, Right, Home, End, PageUp, PageDown, F(u8) }

impl Key {
    /// From a DOM / winit style key name ("Enter", "ArrowUp", "a", " ").
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Enter" => Key::Enter, "Backspace" => Key::Backspace, "Delete" => Key::Delete, "Escape" => Key::Escape, "Tab" => Key::Tab,
            "Home" => Key::Home, "End" => Key::End,
            "ArrowUp" => Key::Up, "ArrowDown" => Key::Down, "ArrowLeft" => Key::Left, "ArrowRight" => Key::Right,
            "PageUp" => Key::PageUp, "PageDown" => Key::PageDown,
            _ if name.len() >= 2 && name.starts_with('F') && name[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => Key::F(name[1..].parse().unwrap_or(1)),
            _ => {
                let mut cs = name.chars();
                let c = cs.next()?;
                if cs.next().is_some() { return None; }
                Key::Char(c)
            }
        })
    }
}

/// Which face a text element is set in: a role that the nearest `fonts` ancestor (normally the
/// theme, at the root) resolves, or a face index directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font { Display, Body, Mono, Face(u8) }
impl Font {
    pub fn resolve(self, roles: [u8; 3]) -> u8 {
        match self { Font::Display => roles[0], Font::Body => roles[1], Font::Mono => roles[2], Font::Face(f) => f }
    }
}

pub enum Content {
    None,
    /// `print` > 0 prints the glyphs through a lattice of that pitch (logical px): each cell is filled
    /// when the glyph covers more than half of it.
    /// `caret` is a byte index into `s`: a caret is drawn there while the nearest focusable ancestor
    /// has focus.
    /// `weight` moves the outline threshold out by that many logical px (bolder) or in (lighter), so a
    /// heading can be heavy without a bold face.
    /// `around` makes the paragraph flow around obstacles (see `wrap`).
    Text { s: String, size: f32, line: f32, paint: Paint, font: Font, print: f32, caret: Option<usize>, weight: f32, around: Option<Rc<Around>> },
    /// A picture by id, with its height as a fraction of its width, and the `PicFx` slot it uses.
    Picture { id: u32, ratio: f32, fx: u8 },
}

/// Handles a key while the element has focus; the flag is Shift.
pub type OnKey<M> = Rc<dyn Fn(Key, bool) -> Option<M>>;
/// Handles a press and the drag that follows. The handler is captured when the press starts and the
/// pointer stays captured until release, wherever it goes.
pub type OnDrag<M> = Rc<dyn Fn(Drag) -> Option<M>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Start, Move, End }

/// One step of a drag, logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub phase: Phase,
    /// The pointer relative to the element's top-left, and the element's size (as it is now).
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
    /// How far the pointer has moved since the press.
    pub dx: f32, pub dy: f32,
}
impl Drag {
    /// The pointer as a fraction of the element, clamped to 0..1.
    pub fn fx(&self) -> f32 { (self.x / self.w.max(1.0)).clamp(0.0, 1.0) }
    pub fn fy(&self) -> f32 { (self.y / self.h.max(1.0)).clamp(0.0, 1.0) }
}

/// An element that opens a layer over everything else (a dialog). While it is there, only it can be
/// hit or focused, everything beneath it is held at `beneath` looseness, and Esc sends `dismiss`.
pub struct Modal<M> { pub dismiss: M, pub beneath: f32 }

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
    pub on_drag: Option<OnDrag<M>>,
    /// Sent when focus leaves this element.
    pub on_blur: Option<M>,
    pub focusable: bool,
    /// Scrolls its children and clips them: vertically, or horizontally when `horizontal`.
    pub scroll: bool,
    pub horizontal: bool,
    /// A scroll node that shows where it is with a thin bar.
    pub bar: bool,
    /// After a key changes the state, scroll so this element is visible (a selected row).
    pub reveal: bool,
    /// This subtree takes no input and is drawn faint.
    pub disabled: bool,
    /// Pressing this commits a new state (theme, material): the renderer keeps the old frame and shows
    /// it beyond a front that travels out from the press.
    pub commit: bool,
    pub modal: Option<Modal<M>>,
    /// Face for each font role in this subtree (display, body, mono).
    pub fonts: Option<[u8; 3]>,
    /// How loose this element is at rest, 0..1 (a picture at 1 shows its marks).
    pub loose: f32,
    /// A compare wipe over this element: the line's position as a fraction of its width. Right of it
    /// the element is fully loosened, left of it untouched.
    pub wipe: Option<f32>,
    /// Identity across pages: on a page change (`Cmd::Flow`) the instances of a subtree with the same
    /// flow id on both pages move from their old place to the new one instead of being replaced.
    pub flow: Option<String>,
    /// Laid out but not drawn and takes no input, unless a morph is showing it (DESIGN.md §1.10).
    pub hidden: bool,
    /// An explicit callout for the annotation layer (DESIGN.md §1.7).
    pub annotate: Option<String>,
    /// The annotation layer only looks inside subtrees marked as its scope (when any is).
    pub gaze_scope: bool,
    /// Per-element material (collage, §1.6): the material id for this subtree, inherited.
    pub material: Option<u8>,
    /// Looseness for the whole subtree (inherited; the larger of this and each element's own wins).
    pub loosen: f32,
    /// Placed just below the element with this key (as laid out in the same pass) instead of where
    /// the layout puts it: for menus in a modal layer.
    pub anchor: Option<String>,
    /// Clips its children to its border box (without scrolling).
    pub clips: bool,
    /// Fronts pass this subtree by: nothing in its box is pushed or loosened by them (a scene's
    /// controls while the page is shaken).
    pub calm: bool,
    pub children: Vec<El<M>>,
}

impl<M> El<M> {
    fn new(style: Style, content: Content) -> Self {
        // Nothing shrinks unless asked: fixed sizes stay fixed inside columns and scroll areas.
        let style = Style { flex_shrink: 0.0, ..style };
        Self { key: String::new(), style, content, fill: None, border: None, radius: 0.0, on_press: None, on_key: None, on_drag: None, on_blur: None, focusable: false, scroll: false, horizontal: false, bar: false, reveal: false, disabled: false, commit: false, modal: None, fonts: None, loose: 0.0, wipe: None, flow: None, hidden: false, annotate: None, gaze_scope: false, material: None, loosen: 0.0, anchor: None, clips: false, calm: false, children: vec![] }
    }
    pub fn key(mut self, k: impl Into<String>) -> Self { self.key = k.into(); self }
    pub fn gap(mut self, g: f32) -> Self { self.style.gap = Size { width: length(g), height: length(g) }; self }
    pub fn pad(mut self, x: f32, y: f32) -> Self { self.style.padding = Rect { left: length(x), right: length(x), top: length(y), bottom: length(y) }; self }
    pub fn width(mut self, w: f32) -> Self { self.style.size.width = length(w); self }
    pub fn height(mut self, h: f32) -> Self { self.style.size.height = length(h); self }
    pub fn min_width(mut self, w: f32) -> Self { self.style.min_size.width = length(w); self }
    pub fn max_width(mut self, w: f32) -> Self { self.style.max_size.width = length(w); self }
    pub fn fill_width(mut self) -> Self { self.style.size.width = percent(1.0); self }
    /// Width as a fraction of the parent's.
    pub fn width_frac(mut self, f: f32) -> Self { self.style.size.width = percent(f); self }
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
    /// A drag reported as the pointer's fraction of the element (0..1), from the press on.
    pub fn on_drag(self, f: impl Fn(f32, f32) -> M + 'static) -> Self { self.on_drag_with(move |d| Some(f(d.fx(), d.fy()))) }
    pub fn on_drag_with(mut self, f: impl Fn(Drag) -> Option<M> + 'static) -> Self { self.on_drag = Some(Rc::new(f)); self }
    /// Whether Tab and presses can give this element focus (pressable elements are by default).
    pub fn focusable(mut self, f: bool) -> Self { self.focusable = f; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn commit(mut self) -> Self { self.commit = true; self }
    pub fn reveal(mut self, r: bool) -> Self { self.reveal = r; self }
    /// Open this element as a modal layer; see `Modal`.
    pub fn modal(mut self, dismiss: M, beneath: f32) -> Self { self.modal = Some(Modal { dismiss, beneath }); self }
    /// Put a caret at byte `at` of a text element (drawn only while it has focus).
    pub fn caret(mut self, at: usize) -> Self { if let Content::Text { caret, .. } = &mut self.content { *caret = Some(at); } self }
    pub fn fonts(mut self, roles: [u8; 3]) -> Self { self.fonts = Some(roles); self }
    pub fn loose(mut self, k: f32) -> Self { self.loose = k; self }
    pub fn wipe(mut self, x: f32) -> Self { self.wipe = Some(x); self }
    /// Fronts do not push or loosen anything in this element's box.
    pub fn calm(mut self) -> Self { self.calm = true; self }
    /// Give this subtree an identity across pages (see `flow`).
    pub fn flow(mut self, id: impl Into<String>) -> Self { self.flow = Some(id.into()); self }
    /// Lay this out but do not draw it until a morph shows it.
    pub fn hidden_until_morphed(mut self, h: bool) -> Self { self.hidden = h; self }
    /// A callout label for the annotation layer ("①  电源开关").
    pub fn annotate(mut self, label: impl Into<String>) -> Self { self.annotate = Some(label.into()); self }
    /// Make this subtree what the annotation layer looks at.
    pub fn gaze_scope(mut self) -> Self { self.gaze_scope = true; self }
    /// Draw this subtree with another material (`style::Material` id), under the collage material.
    pub fn material(mut self, id: u8) -> Self { self.material = Some(id); self }
    /// Hold the whole subtree at looseness `k`.
    pub fn loosen(mut self, k: f32) -> Self { self.loosen = k; self }
    /// Place this element just below the element `key` (see `anchor`).
    pub fn anchor(mut self, key: impl Into<String>) -> Self { self.anchor = Some(key.into()); self }
    /// Clip the children to this element's box: what lies outside is not drawn and cannot be hit.
    pub fn clip(mut self) -> Self { self.clips = true; self.style.overflow = taffy::Point { x: taffy::Overflow::Clip, y: taffy::Overflow::Clip }; self }
    /// Take this element out of the flow and pin it to its parent's edges (logical px; `None` is auto).
    pub fn absolute(mut self, left: Option<f32>, top: Option<f32>, right: Option<f32>, bottom: Option<f32>) -> Self {
        let v = |o: Option<f32>| o.map_or(auto(), length);
        self.style.position = taffy::Position::Absolute;
        self.style.inset = Rect { left: v(left), top: v(top), right: v(right), bottom: v(bottom) };
        self
    }
    /// Set a text element in another face or role.
    pub fn font(mut self, f: Font) -> Self { if let Content::Text { font, .. } = &mut self.content { *font = f; } self }
    /// Print a text element through a lattice of this pitch (logical px).
    pub fn print(mut self, pitch: f32) -> Self { if let Content::Text { print, .. } = &mut self.content { *print = pitch; } self }
    /// Make a text element bolder (px > 0) or lighter (px < 0) by moving its outline (logical px).
    pub fn weight(mut self, px: f32) -> Self { if let Content::Text { weight, .. } = &mut self.content { *weight = px; } self }
    /// Let a text element flow around the true outlines in `a` (coordinates relative to its own
    /// content box), keeping `a.gap` px from their ink.
    pub fn around(mut self, a: Around) -> Self { if let Content::Text { around, .. } = &mut self.content { *around = Some(Rc::new(a)); } self }
    /// Choose the `PicFx` slot of a picture.
    pub fn pic_fx(mut self, slot: u8) -> Self { if let Content::Picture { fx, .. } = &mut self.content { *fx = slot; } self }
    pub fn scroll(mut self) -> Self { self.scroll = true; self.style.overflow.y = taffy::Overflow::Scroll; self.style.scrollbar_width = 0.0; self }
    /// Scroll horizontally instead (the wheel's vertical movement scrolls it too).
    pub fn scroll_x(mut self) -> Self { self.scroll = true; self.horizontal = true; self.style.overflow.x = taffy::Overflow::Scroll; self.style.scrollbar_width = 0.0; self }
    /// Show a thin bar for where a scroll node is.
    pub fn bar(mut self) -> Self { self.bar = true; self }
    pub fn child(mut self, c: El<M>) -> Self { self.children.push(c); self }
    pub fn children(mut self, c: impl IntoIterator<Item = El<M>>) -> Self { self.children.extend(c); self }

    /// The same tree sending other messages: lets one application embed another's view.
    pub fn map<N: 'static>(self, f: Rc<dyn Fn(M) -> N>) -> El<N> where M: 'static {
        let El { key, style, content, fill, border, radius, on_press, on_key, on_drag, on_blur, focusable, scroll, horizontal, bar, reveal, disabled, commit, modal, fonts, loose, wipe, flow, hidden, annotate, gaze_scope, material, loosen, anchor, clips, calm, children } = self;
        let on_key = on_key.map(|k| { let f = f.clone(); Rc::new(move |key: Key, shift: bool| k(key, shift).map(|m| f(m))) as OnKey<N> });
        let on_drag = on_drag.map(|d| { let f = f.clone(); Rc::new(move |g: Drag| d(g).map(|m| f(m))) as OnDrag<N> });
        El {
            key, style, content, fill, border, radius, on_press: on_press.map(|m| f(m)), on_key, on_drag, on_blur: on_blur.map(|m| f(m)),
            focusable, scroll, horizontal, bar, reveal, disabled, commit, modal: modal.map(|m| Modal { dismiss: f(m.dismiss), beneath: m.beneath }),
            fonts, loose, wipe, flow, hidden, annotate, gaze_scope, material, loosen, anchor, clips, calm, children: children.into_iter().map(|c| c.map(f.clone())).collect(),
        }
    }
}

pub fn row<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Row, ..Default::default() }, Content::None) }
pub fn col<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Default::default() }, Content::None) }
pub fn space<M>(w: f32, h: f32) -> El<M> { El::new(Style { size: Size { width: length(w), height: length(h) }, ..Default::default() }, Content::None) }
pub fn text<M>(s: impl Into<String>, size: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line: (size * 1.6).round(), paint, font: Font::Body, print: 0.0, caret: None, weight: 0.0, around: None })
}
/// A text leaf with an explicit line height.
pub fn text_lh<M>(s: impl Into<String>, size: f32, line: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line, paint, font: Font::Body, print: 0.0, caret: None, weight: 0.0, around: None })
}
pub fn picture<M>(id: u32, ratio: f32) -> El<M> { El::new(Style { size: Size { width: percent(1.0), height: auto() }, ..Default::default() }, Content::Picture { id, ratio, fx: 0 }) }
/// Every child in the same place, centred in the stack (a one-cell grid): for things that replace one
/// another where they stand, like the candidates of a morph.
pub fn stack<M>(children: impl IntoIterator<Item = El<M>>) -> El<M> {
    let mut s = El::new(Style {
        display: Display::Grid, grid_template_columns: vec![fr(1.0)], grid_template_rows: vec![fr(1.0)],
        justify_items: AlignItems::CENTER, align_items: AlignItems::CENTER, ..Default::default()
    }, Content::None);
    for mut c in children {
        c.style.grid_row = Line { start: line(1), end: span(1) };
        c.style.grid_column = Line { start: line(1), end: span(1) };
        s.children.push(c);
    }
    s
}
