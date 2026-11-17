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
    pub const DIM: Self = Self::tone(0.66);
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
/// `Whisper` is small tracked type in the bundled serif (`text::SERIF`, the same in every theme;
/// `El::whisper` sets it up).
pub enum Font { Display, Body, Mono, Whisper, Face(u8) }
impl Font {
    pub fn resolve(self, roles: [u8; 3]) -> u8 {
        match self { Font::Display => roles[0], Font::Body => roles[1], Font::Mono => roles[2], Font::Whisper => crate::text::SERIF, Font::Face(f) => f }
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
    /// `select` is a byte range of `s` drawn as selected (by nearness to the glyphs, not as a box) while
    /// the field it belongs to has focus; `mark` a byte range underlined (text being composed by an input
    /// method). `nowrap` keeps it on one line whatever its width (a single-line field that scrolls). `track` is extra
    /// letter spacing in em (`El::tracking`).
    Text { s: String, size: f32, line: f32, paint: Paint, font: Font, print: f32, caret: Option<usize>, weight: f32, around: Option<Rc<Around>>,
           select: Option<(usize, usize)>, mark: Option<(usize, usize)>, nowrap: bool, pose: Option<GlyphPose>, track: f32 },
    /// A picture by id, with its height as a fraction of its width, and the `PicFx` slot it uses.
    Picture { id: u32, ratio: f32, fx: u8 },
    /// Meshes, a camera and a light, drawn into the element's box (DESIGN.md §1.18).
    Scene3d(Rc<crate::mesh::Scene3d>),
    /// Marks in their own space, scaled to the element's width (`el::canvas`, DESIGN.md §1.26).
    Canvas(Rc<crate::draw::Drawing>),
}

/// Where one laid-out glyph is, for a pose: its number in the text and how many there are, its centre
/// in the element's content box (logical px), that box's size, and the box (x0, y0, x1, y1) all the
/// glyphs' centres span (so a pose can map the text block onto a path or a sphere).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphAt { pub i: usize, pub n: usize, pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub span: [f32; 4] }
/// How one glyph is drawn instead of where the layout put it: moved by (dx, dy) logical px, turned by
/// `angle` (radians, clockwise), scaled by `sx`, `sy` about its centre; its tone and alpha multiplied,
/// its accent raised to at least `accent`, drawn in accent `hue` (0, 1, 2; negative keeps the text's).
/// Alpha 0 leaves it out. The glyph keeps its true outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose { pub dx: f32, pub dy: f32, pub angle: f32, pub sx: f32, pub sy: f32, pub tone: f32, pub alpha: f32, pub accent: f32, pub hue: f32 }
impl Default for Pose { fn default() -> Self { Self { dx: 0.0, dy: 0.0, angle: 0.0, sx: 1.0, sy: 1.0, tone: 1.0, alpha: 1.0, accent: 0.0, hue: -1.0 } } }
/// A text element's glyph poses (`El::pose`): asked once per glyph each time the scene is made, never
/// for layout, so moving glyphs costs no layout or shaping.
pub type GlyphPose = Rc<dyn Fn(GlyphAt) -> Pose>;
/// The pointer over or on an element (`El::on_point`): where (local px, or a canvas's own space) and
/// whether it is pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point { pub x: f32, pub y: f32, pub down: bool }
/// An element coming into view (`true`, any part inside its clip) or leaving it.
pub type OnVisible<M> = Rc<dyn Fn(bool) -> Option<M>>;
/// Asks for the true outline of an element's subtree as coverage on a grid (`El::on_outline`).
pub struct Outline<M> { pub cols: usize, pub rows: usize, pub f: Rc<dyn Fn(Rc<crate::field::Grid>) -> Option<M>> }

/// Handles the pointer over a `scene3d` element: which copy it is on, or pressed (DESIGN.md §1.18).
pub type OnPick<M> = Rc<dyn Fn(crate::mesh::Pick) -> Option<M>>;
/// Handles the wheel over an element before any scroll node does (dx, dy, logical px).
pub type OnWheel<M> = Rc<dyn Fn(f32, f32) -> Option<M>>;
/// The pointer over an element (`Some` local logical px) or leaving it (`None`).
pub type OnMove<M> = Rc<dyn Fn(Option<(f32, f32)>) -> Option<M>>;
/// Handles a key while the element has focus; the flag is Shift.
pub type OnKey<M> = Rc<dyn Fn(Key, bool) -> Option<M>>;
/// Handles text input while the element has focus (a text field, DESIGN.md §1.8.1).
pub type OnText<M> = Rc<dyn Fn(TextInput) -> Option<M>>;

/// Modifier keys with a key press: Shift, the command key (Ctrl, or ⌘ on macOS), Alt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods { pub shift: bool, pub cmd: bool, pub alt: bool }

/// What a text field is told. Byte indices are into the text the field showed (its text leaf's string,
/// which includes text being composed).
#[derive(Clone, Debug, PartialEq)]
pub enum TextInput {
    /// A key that is not text: arrows, Home / End, Backspace, Delete, Enter; with Cmd: A, Z, Y.
    Key { key: Key, mods: Mods },
    /// Text typed, committed by an input method, or dropped in: inserted at the caret over the selection.
    Insert(String),
    /// Text being composed by an input method, shown at the caret and underlined; `cursor` is a byte
    /// range in it. Empty text ends the composition without inserting anything (cancelled).
    Compose { text: String, cursor: Option<(usize, usize)> },
    /// The clipboard's text, to insert.
    Paste(String),
    /// The selection was copied to the clipboard by the runtime and is to be removed.
    Cut,
    /// The pointer: pressed (with the click count: 2 selects a word, 3 everything), dragged, let go, at a
    /// byte index found from the laid-out text; Shift extends the selection.
    Pointer { at: usize, phase: Phase, clicks: u8, shift: bool },
    /// Put the caret at `at` (the runtime moved it by the layout: a line up or down, a line's start or
    /// end in a multi-line field), extending the selection if asked.
    Goto { at: usize, extend: bool },
}
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

/// How an element answers the field's fronts, lens and pulls (DESIGN.md §1.15). Inherited down the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Response {
    /// Fronts pass it by: no push, no looseness (the caret, a selection, the focus marks).
    Ignore,
    /// Pushed a little and offset by a burst as a front passes, but its looseness stays 0 (text to read,
    /// controls).
    Nudge,
    /// Pushed and loosened (decoration, pictures, a scene's headline).
    Loosen,
}
impl Response {
    pub const ALL: [Response; 3] = [Response::Ignore, Response::Nudge, Response::Loosen];
    pub fn name(self) -> &'static str { match self { Response::Ignore => "ignore", Response::Nudge => "nudge", Response::Loosen => "loosen" } }
}

/// The trail a moving element leaves in the field map (DESIGN.md §1.29): `energy` times the look's
/// `MapFx::stirr`, over `radius` (logical px; `None`: the look's `stirr_radius` plus half the element's
/// shorter side), only on what `scope` reaches (the default: the nearest `ripple_scope`, else the window).
#[derive(Clone, Debug, PartialEq)]
pub struct Stirr { pub energy: f32, pub radius: Option<f32>, pub scope: Scope }
impl Default for Stirr { fn default() -> Self { Self { energy: 1.0, radius: None, scope: Scope::Container(None) } } }

/// What a front reaches (DESIGN.md §1.15).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Only the pressed element's own subtree (the default for presses on controls).
    Own,
    /// The nearest ancestor marked `El::ripple_scope()` (`None`), or the element with this key.
    Container(Option<String>),
    /// The whole window (theme, material, page and scene changes; an explicit global front).
    Global,
    /// No front at all.
    Off,
}

/// Ripple and recomposition settings of an element (DESIGN.md §1.15, §1.16). `None` means inherited
/// (or the default for what the element is).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ripple {
    pub respond: Option<Response>,
    /// Text to read or a control: field effects loosen it at most `Look::legible_cap`.
    pub legible: Option<bool>,
    /// What a press here sends a front to.
    pub scope: Option<Scope>,
    /// Fronts from presses inside with `Scope::Container(None)` stop at this element's box.
    pub container: bool,
    /// Text whose string changes recomposes glyph by glyph (on by default for keyed text).
    pub recompose: Option<bool>,
    /// The focus marks do not frame this element when the pointer is over it (its box is not its
    /// shape: text runs around its true outline, and corners at its box would sit in the text).
    pub unframed: bool,
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
    /// Moved by (dx, dy) logical px after layout, with everything inside it: for motion that follows
    /// the pointer (parallax), which then needs no layout. Hit testing and drawing see the moved box.
    pub shift: (f32, f32),
    /// Marks this element sheds while it moves (DESIGN.md §1.22).
    pub shed: Option<crate::motes::Shed>,
    /// Marks carried along a noise field over this element's box (DESIGN.md §1.22).
    pub flow_field: Option<crate::motes::FlowField>,
    /// Moving, it stirs the field map behind it (DESIGN.md §1.29).
    pub stirr: Option<Stirr>,
    /// A long path drawn by progress (DESIGN.md §1.30).
    pub trace: Option<crate::strand::Trace>,
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
    /// A text field: takes text input while focused (see `TextInput`); `multiline` lets the runtime move
    /// the caret by lines and Enter insert a line break.
    pub on_text: Option<OnText<M>>,
    pub multiline: bool,
    /// The text in this subtree can be selected with the pointer and copied (read-only).
    pub selectable: bool,
    /// The pointer's shape over this element (else derived: text, pointer, grab).
    pub cursor: Option<Cursor>,
    /// A virtual list (DESIGN.md §1.13): the runtime builds only the rows it can see (and a few around).
    pub virt: Option<Virtual<M>>,
    /// What this element is to assistive technology (DESIGN.md §1.14): role, name, state.
    pub access: Option<Access>,
    /// Ripple scope, response, legibility and recomposition (DESIGN.md §1.15, §1.16).
    pub ripple: Ripple,
    /// A `scene3d` element: what the pointer is over, and presses, by the copy under it.
    pub on_pick: Option<OnPick<M>>,
    /// The wheel over this element (taken before scrolling).
    pub on_wheel: Option<OnWheel<M>>,
    /// The pointer moving over this element, in its own coordinates, and leaving it (`El::on_move`).
    pub on_move: Option<OnMove<M>>,
    /// A closed outline (local logical px) the annotation layer draws over this element (`El::guide`).
    pub guide: Option<Rc<Vec<(f32, f32)>>>,
    /// This subtree is drawn as its own loading skeleton (`El::skeleton`), inherited.
    pub skeleton: bool,
    /// Lines and rows run right to left in this subtree (inherited; `None` follows the parent).
    pub rtl: Option<bool>,
    /// Told when it comes into view and leaves it (`El::on_visible`).
    pub on_visible: Option<OnVisible<M>>,
    /// Told its subtree's true outline as coverage on a grid, whenever it changes (`El::on_outline`).
    pub on_outline: Option<Outline<M>>,
    /// Appears and disappears by decoding (`El::decode`, DESIGN.md §1.28).
    pub decode: Option<crate::decode::Decode>,
    pub children: Vec<El<M>>,
}

/// Rows of a virtual list: how many, how tall each (logical px), how many beyond the visible ones to
/// build on each side, what row `i` is, and which row is selected (it is scrolled into view after a key).
pub struct Virtual<M> { pub count: usize, pub row_h: f32, pub overscan: usize, pub build: Rc<dyn Fn(usize) -> El<M>>, pub selected: Option<usize> }

/// Roles of the accessibility mirror (the WAI-ARIA roles of the same name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Button, Switch, Slider, SpinButton, TabList, Tab, TextBox, Dialog, List, ListItem, Img, Heading, Group, Text,
    Checkbox, Radio, RadioGroup, Combobox, Listbox, Option, Tooltip, Status, ProgressBar, Region, Table, Row, Cell, ColumnHeader,
    Navigation, Menu, MenuItem, Separator, Toolbar, Link,
}
impl Role {
    pub fn aria(self) -> &'static str {
        match self {
            Role::Button => "button", Role::Switch => "switch", Role::Slider => "slider", Role::SpinButton => "spinbutton",
            Role::TabList => "tablist", Role::Tab => "tab", Role::TextBox => "textbox", Role::Dialog => "dialog", Role::List => "list",
            Role::ListItem => "listitem", Role::Img => "img", Role::Heading => "heading", Role::Group => "group", Role::Text => "text",
            Role::Checkbox => "checkbox", Role::Radio => "radio", Role::RadioGroup => "radiogroup", Role::Combobox => "combobox", Role::Listbox => "listbox",
            Role::Option => "option", Role::Tooltip => "tooltip", Role::Status => "status", Role::ProgressBar => "progressbar", Role::Region => "region",
            Role::Table => "table", Role::Row => "row", Role::Cell => "cell", Role::ColumnHeader => "columnheader", Role::Navigation => "navigation",
            Role::Menu => "menu", Role::MenuItem => "menuitem", Role::Separator => "separator", Role::Toolbar => "toolbar", Role::Link => "link",
        }
    }
}
/// An element's accessible role, name and state. Unset parts are derived (the name from the text inside).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Access {
    pub role: Option<Role>,
    pub label: Option<String>,
    /// A slider's or spin button's value, minimum and maximum.
    pub value: Option<(f32, f32, f32)>,
    /// A text box's text.
    pub text: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    /// A heading's level.
    pub level: u8,
    /// Whether what this control opens (a disclosure's panel, a combobox's list, a menu) is open.
    pub expanded: Option<bool>,
    /// A longer description than the name (help text under a field, a hint).
    pub description: Option<String>,
    /// Announce changes to this element's text without moving focus (a status line, a toast).
    pub live: Option<Live>,
    /// A field whose value is not accepted, and a field that must be filled.
    pub invalid: bool,
    pub required: bool,
}

/// How urgently a live region's changes are announced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live { Polite, Assertive }
impl Live {
    /// The `aria-live` value.
    pub fn aria(self) -> &'static str { match self { Live::Polite => "polite", Live::Assertive => "assertive" } }
}

/// Pointer shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cursor { #[default] Default, Pointer, Text, Grab, Grabbing, ColResize, RowResize }
impl Cursor {
    /// The CSS name.
    pub fn css(self) -> &'static str {
        match self { Cursor::Default => "default", Cursor::Pointer => "pointer", Cursor::Text => "text", Cursor::Grab => "grab", Cursor::Grabbing => "grabbing", Cursor::ColResize => "col-resize", Cursor::RowResize => "row-resize" }
    }
}

impl<M> El<M> {
    fn new(style: Style, content: Content) -> Self {
        // Nothing shrinks unless asked: fixed sizes stay fixed inside columns and scroll areas.
        let style = Style { flex_shrink: 0.0, ..style };
        Self { key: String::new(), style, content, fill: None, border: None, radius: 0.0, on_press: None, on_key: None, on_drag: None, on_blur: None, focusable: false, scroll: false, horizontal: false, bar: false, reveal: false, disabled: false, commit: false, modal: None, fonts: None, loose: 0.0, wipe: None, flow: None, hidden: false, shift: (0.0, 0.0), shed: None, flow_field: None, stirr: None, trace: None, annotate: None, gaze_scope: false, material: None, loosen: 0.0, anchor: None, clips: false, calm: false, on_text: None, multiline: false, selectable: false, cursor: None, virt: None, access: None, ripple: Ripple::default(), on_pick: None, on_wheel: None, on_move: None, guide: None, skeleton: false, rtl: None, on_visible: None, on_outline: None, decode: None, children: vec![] }
    }
    pub fn key(mut self, k: impl Into<String>) -> Self { self.key = k.into(); self }
    pub fn gap(mut self, g: f32) -> Self { self.style.gap = Size { width: length(g), height: length(g) }; self }
    pub fn pad(mut self, x: f32, y: f32) -> Self { self.style.padding = Rect { left: length(x), right: length(x), top: length(y), bottom: length(y) }; self }
    pub fn width(mut self, w: f32) -> Self { self.style.size.width = length(w); self }
    pub fn height(mut self, h: f32) -> Self { self.style.size.height = length(h); self }
    pub fn min_width(mut self, w: f32) -> Self { self.style.min_size.width = length(w); self }
    pub fn max_width(mut self, w: f32) -> Self { self.style.max_size.width = length(w); self }
    pub fn min_height(mut self, h: f32) -> Self { self.style.min_size.height = length(h); self }
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
    /// Make this a text field (see `TextInput`); it can take focus.
    pub fn on_text(mut self, f: impl Fn(TextInput) -> Option<M> + 'static) -> Self { self.on_text = Some(Rc::new(f)); self.focusable = true; self }
    /// A text field of several lines: Enter breaks the line, Up / Down and Home / End move by lines.
    pub fn multiline(mut self) -> Self { self.multiline = true; self }
    /// Let the pointer select the text in this subtree (read-only), to copy it.
    pub fn selectable(mut self) -> Self { self.selectable = true; self }
    /// The pointer's shape over this element.
    pub fn cursor(mut self, c: Cursor) -> Self { self.cursor = Some(c); self }
    fn access_mut(&mut self) -> &mut Access { self.access.get_or_insert_with(Access::default) }
    /// The same subtree with every key written under `prefix` (`prefix/key`), anchors too: one example
    /// shown several times on a page keeps its keys unique.
    pub fn rekey(mut self, prefix: &str) -> Self {
        if !self.key.is_empty() { self.key = format!("{prefix}/{}", self.key); }
        if let Some(a) = &self.anchor { self.anchor = Some(format!("{prefix}/{a}")); }
        self.children = std::mem::take(&mut self.children).into_iter().map(|c| c.rekey(prefix)).collect();
        self
    }
    /// This subtree runs right to left (DESIGN.md §1.20): a row's first child is at its right, its
    /// start alignment is the right edge, padding and margins mirror. Inherited.
    pub fn rtl(mut self, on: bool) -> Self { self.rtl = Some(on); self }
    /// Its accessible name (what a screen reader says), when the text inside does not say it.
    pub fn label(mut self, name: impl Into<String>) -> Self { self.access_mut().label = Some(name.into()); self }
    /// Its role in the accessibility mirror.
    pub fn role(mut self, r: Role) -> Self { self.access_mut().role = Some(r); self }
    /// A switch's state; a tab's or list item's selection; a slider's value and range; a text box's text;
    /// a heading's level.
    pub fn checked(mut self, on: bool) -> Self { self.access_mut().checked = Some(on); self }
    pub fn selected(mut self, on: bool) -> Self { self.access_mut().selected = Some(on); self }
    pub fn value_range(mut self, v: f32, min: f32, max: f32) -> Self { self.access_mut().value = Some((v, min, max)); self }
    pub fn text_value(mut self, t: impl Into<String>) -> Self { self.access_mut().text = Some(t.into()); self }
    pub fn heading(mut self, level: u8) -> Self { let a = self.access_mut(); a.role = Some(Role::Heading); a.level = level; self }
    /// Whether what this control opens is open (a disclosure, a combobox, a menu button).
    pub fn expanded(mut self, open: bool) -> Self { self.access_mut().expanded = Some(open); self }
    /// A description read after the name (help text, a hint).
    pub fn description(mut self, d: impl Into<String>) -> Self { self.access_mut().description = Some(d.into()); self }
    /// Announce changes to its text without moving focus. `Role::Status` is polite by default.
    pub fn live(mut self, l: Live) -> Self { self.access_mut().live = Some(l); self }
    /// The field's value is not accepted (say why with `description`).
    pub fn invalid(mut self, on: bool) -> Self { self.access_mut().invalid = on; self }
    /// The field must be filled.
    pub fn required(mut self, on: bool) -> Self { self.access_mut().required = on; self }
    /// Make this (a scroll node) a virtual list of `count` rows `row_h` tall: only the visible rows and
    /// `overscan` more on each side exist in the tree; row `i` is `build(i)` (keyed `key/row{i}` unless it
    /// has a key). Its children are replaced by the rows.
    pub fn virtual_rows(mut self, count: usize, row_h: f32, overscan: usize, selected: Option<usize>, build: impl Fn(usize) -> El<M> + 'static) -> Self {
        self.virt = Some(Virtual { count, row_h, overscan, build: Rc::new(build), selected });
        self
    }
    /// Show bytes `a..b` of a text element as selected (while its field has focus).
    pub fn select(mut self, a: usize, b: usize) -> Self { if let Content::Text { select, .. } = &mut self.content { *select = (a != b).then_some((a.min(b), a.max(b))); } self }
    /// Underline bytes `a..b` of a text element (text being composed).
    pub fn mark(mut self, a: usize, b: usize) -> Self { if let Content::Text { mark, .. } = &mut self.content { *mark = (a < b).then_some((a, b)); } self }
    /// Keep a text element on one line.
    pub fn nowrap(mut self) -> Self { if let Content::Text { nowrap, .. } = &mut self.content { *nowrap = true; } self }
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
    /// The focus marks frame it only when it has keyboard focus, never on hover.
    pub fn unframed(mut self) -> Self { self.ripple.unframed = true; self }
    /// Give this subtree an identity across pages (see `flow`).
    pub fn flow(mut self, id: impl Into<String>) -> Self { self.flow = Some(id.into()); self }
    /// Lay this out but do not draw it until a morph shows it.
    pub fn hidden_until_morphed(mut self, h: bool) -> Self { self.hidden = h; self }
    /// Move this subtree by (dx, dy) logical px after layout (no layout when only this changes).
    pub fn shift(mut self, dx: f32, dy: f32) -> Self { self.shift = (dx, dy); self }
    /// Shed marks while this element moves (`rate` 0: none).
    pub fn shed(mut self, s: crate::motes::Shed) -> Self { self.shed = Some(s); self }
    /// Leave a trail in the field map while this element moves (by a drag, a spring, a flow or
    /// `shift`): along the way its centre went, energy and flow in the direction it went (DESIGN.md §1.29).
    pub fn stirr(mut self, s: Stirr) -> Self { self.stirr = Some(s); self }
    /// Draw a long path in this element's box by progress, a line of constant width that the field map's
    /// flow pushes and whose head stirs the map (DESIGN.md §1.30). Any material.
    pub fn trace(mut self, t: crate::strand::Trace) -> Self { self.trace = Some(t); self }
    /// Carry marks along a noise field over this element's box (`speed` 0: they stand still).
    pub fn flow_field(mut self, f: crate::motes::FlowField) -> Self { self.flow_field = Some(f); self }
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
    /// Extra letter spacing, in em (−0.05–1.5, DESIGN.md §4.2): laid out by parley, so the measured width,
    /// selections, recomposition and scribblr's outlines all follow it.
    pub fn tracking(mut self, em: f32) -> Self { if let Content::Text { track, .. } = &mut self.content { *track = em.clamp(-0.05, 1.5); } self }
    /// Whisper (DESIGN.md §4.2): this text in lowercase, in the serif, tracked 0.6 em: the small, airy
    /// type beside a picture or along an edge. Keep it small (11–13 px).
    pub fn whisper(mut self) -> Self {
        if let Content::Text { s, font, track, .. } = &mut self.content { *s = s.to_lowercase(); *font = Font::Whisper; *track = 0.6; }
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
    /// How this subtree answers fronts, the lens and pulls (inherited).
    pub fn respond(mut self, r: Response) -> Self { self.ripple.respond = Some(r); self }
    /// Mark this subtree as text to read / a control (`true`), so field effects loosen it at most
    /// `Look::legible_cap`, or as decoration (`false`). Inherited; text and controls are legible by default.
    pub fn legible(mut self, on: bool) -> Self { self.ripple.legible = Some(on); self }
    /// What a press in this subtree sends a front to (inherited). Controls default to `Scope::Own`;
    /// pressing plain ground sends nothing unless a scope is set here.
    pub fn ripple(mut self, s: Scope) -> Self { self.ripple.scope = Some(s); self }
    /// Fronts sent with `Scope::Container(None)` from inside stop at this element's box.
    pub fn ripple_scope(mut self) -> Self { self.ripple.container = true; self }
    /// Whether a change of this text's string recomposes the changed glyphs (inherited; on by default
    /// for text with an explicit key, off inside text fields).
    pub fn recompose(mut self, on: bool) -> Self { self.ripple.recompose = Some(on); self }
    /// Choose the `PicFx` slot of a picture.
    pub fn pic_fx(mut self, slot: u8) -> Self { if let Content::Picture { fx, .. } = &mut self.content { *fx = slot; } self }
    /// Hear which copy of a `scene3d` element the pointer is over and which one it presses.
    pub fn on_pick(mut self, f: impl Fn(crate::mesh::Pick) -> Option<M> + 'static) -> Self { self.on_pick = Some(Rc::new(f)); self }
    /// Take the wheel over this element (before any scroll node): dx, dy in logical px.
    pub fn on_wheel(mut self, f: impl Fn(f32, f32) -> Option<M> + 'static) -> Self { self.on_wheel = Some(Rc::new(f)); self }
    /// Hear a mouse or pen moving over this element (`Some((x, y))`, logical px from its top left) and
    /// leaving it (`None`): for menus that steer by where the pointer goes. Fingers have no hover.
    /// A closed outline through `points` (logical px from this element's top left), drawn dotted by the
    /// annotation layer in its colour whatever its mode: a debug overlay (a menu's safe zone).
    /// Draw this subtree as its own skeleton (DESIGN.md §1.23): laid out exactly as it is, but every
    /// line of text is a bar as wide as that line measures, every picture or 3D element a block, every
    /// fill and border faint; it takes no input. Swapping `.skeleton()` off shows the content in the same
    /// boxes, so nothing moves.
    pub fn skeleton(mut self) -> Self { self.skeleton = true; self }
    pub fn guide(mut self, points: Vec<(f32, f32)>) -> Self { self.guide = (points.len() >= 2).then(|| Rc::new(points)); self }
    pub fn on_move(mut self, f: impl Fn(Option<(f32, f32)>) -> Option<M> + 'static) -> Self { self.on_move = Some(Rc::new(f)); self }
    /// The pointer over this element and dragging on it, as one handler: `Some(Point)` while it moves
    /// over it (`down` false: a mouse or pen hovering) or presses and drags (`down` true until let go;
    /// a finger only does this), `None` when it leaves. On a canvas the point is in the canvas's space.
    /// The focus marks do not frame it (it is a surface, not a control).
    pub fn on_point(mut self, f: impl Fn(Option<Point>) -> Option<M> + 'static) -> Self {
        let f = Rc::new(f);
        let g = f.clone();
        self.on_move = Some(Rc::new(move |p: Option<(f32, f32)>| g(p.map(|(x, y)| Point { x, y, down: false }))));
        self.on_drag = Some(Rc::new(move |d: Drag| f(Some(Point { x: d.x, y: d.y, down: d.phase != Phase::End }))));
        self.ripple.unframed = true;
        self
    }
    /// Hear when this element comes into view or leaves it (inside its scroll area's and clips' view):
    /// a live demo steps only while it can be seen.
    pub fn on_visible(mut self, f: impl Fn(bool) -> Option<M> + 'static) -> Self { self.on_visible = Some(Rc::new(f)); self }
    /// Be told this subtree's true outline — the coverage of its glyphs, shapes and pictures, hidden
    /// ones included — sampled on a `cols × rows` grid over its box (values 0..1), each time it changes.
    /// What it gives is a mask for a field (DESIGN.md §1.26).
    pub fn on_outline(mut self, cols: usize, rows: usize, f: impl Fn(Rc<crate::field::Grid>) -> Option<M> + 'static) -> Self { self.on_outline = Some(Outline { cols, rows, f: Rc::new(f) }); self }
    /// Appear by decoding and disappear by the same played backwards (DESIGN.md §1.28): blocks over the
    /// true outline resolve coarse to fine, then the content shows. `hold` in it scrubs to a progress.
    pub fn decode(mut self, d: crate::decode::Decode) -> Self { self.decode = Some(d); self }
    /// Draw each glyph of this text by its pose (`Pose`): on a path, on a sphere, lit by distance.
    pub fn pose(mut self, f: impl Fn(GlyphAt) -> Pose + 'static) -> Self { if let Content::Text { pose, .. } = &mut self.content { *pose = Some(Rc::new(f)); } self }
    pub fn scroll(mut self) -> Self { self.scroll = true; self.style.overflow.y = taffy::Overflow::Scroll; self.style.scrollbar_width = 0.0; self }
    /// Scroll horizontally instead (the wheel's vertical movement scrolls it too).
    pub fn scroll_x(mut self) -> Self { self.scroll = true; self.horizontal = true; self.style.overflow.x = taffy::Overflow::Scroll; self.style.scrollbar_width = 0.0; self }
    /// Show a thin bar for where a scroll node is.
    pub fn bar(mut self) -> Self { self.bar = true; self }
    pub fn child(mut self, c: El<M>) -> Self { self.children.push(c); self }
    pub fn children(mut self, c: impl IntoIterator<Item = El<M>>) -> Self { self.children.extend(c); self }

    /// The same tree sending other messages: lets one application embed another's view.
    pub fn map<N: 'static>(self, f: Rc<dyn Fn(M) -> N>) -> El<N> where M: 'static {
        let El { key, style, content, fill, border, radius, on_press, on_key, on_drag, on_blur, focusable, scroll, horizontal, bar, reveal, disabled, commit, modal, fonts, loose, wipe, flow, hidden, shift, shed, flow_field, stirr, trace, annotate, gaze_scope, material, loosen, anchor, clips, calm, on_text, multiline, selectable, cursor, virt, access, ripple, on_pick, on_wheel, on_move, guide, skeleton, rtl, on_visible, on_outline, decode, children } = self;
        let virt = virt.map(|v| { let f = f.clone(); let b = v.build; Virtual { count: v.count, row_h: v.row_h, overscan: v.overscan, selected: v.selected, build: Rc::new(move |i: usize| b(i).map(f.clone())) as Rc<dyn Fn(usize) -> El<N>> } });
        let on_key = on_key.map(|k| { let f = f.clone(); Rc::new(move |key: Key, shift: bool| k(key, shift).map(|m| f(m))) as OnKey<N> });
        let on_text = on_text.map(|t| { let f = f.clone(); Rc::new(move |i: TextInput| t(i).map(|m| f(m))) as OnText<N> });
        let on_drag = on_drag.map(|d| { let f = f.clone(); Rc::new(move |g: Drag| d(g).map(|m| f(m))) as OnDrag<N> });
        let on_pick = on_pick.map(|d| { let f = f.clone(); Rc::new(move |g: crate::mesh::Pick| d(g).map(|m| f(m))) as OnPick<N> });
        let on_wheel = on_wheel.map(|d| { let f = f.clone(); Rc::new(move |x: f32, y: f32| d(x, y).map(|m| f(m))) as OnWheel<N> });
        let on_move = on_move.map(|d| { let f = f.clone(); Rc::new(move |p: Option<(f32, f32)>| d(p).map(|m| f(m))) as OnMove<N> });
        let on_visible = on_visible.map(|d| { let f = f.clone(); Rc::new(move |v: bool| d(v).map(|m| f(m))) as OnVisible<N> });
        let on_outline = on_outline.map(|o| { let f = f.clone(); let g = o.f; Outline { cols: o.cols, rows: o.rows, f: Rc::new(move |x: Rc<crate::field::Grid>| g(x).map(|m| f(m))) } });
        El {
            key, style, content, fill, border, radius, on_press: on_press.map(|m| f(m)), on_key, on_drag, on_blur: on_blur.map(|m| f(m)),
            focusable, scroll, horizontal, bar, reveal, disabled, commit, modal: modal.map(|m| Modal { dismiss: f(m.dismiss), beneath: m.beneath }),
            fonts, loose, wipe, flow, hidden, shift, shed, flow_field, stirr, trace, annotate, gaze_scope, material, loosen, anchor, clips, calm, on_text, multiline, selectable, cursor, virt, access, ripple, on_pick, on_wheel, on_move, guide, skeleton, rtl, on_visible, on_outline, decode, children: children.into_iter().map(|c| c.map(f.clone())).collect(),
        }
    }
}

pub fn row<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Row, ..Default::default() }, Content::None) }
pub fn col<M>() -> El<M> { El::new(Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Default::default() }, Content::None) }
pub fn space<M>(w: f32, h: f32) -> El<M> { El::new(Style { size: Size { width: length(w), height: length(h) }, ..Default::default() }, Content::None) }
pub fn text<M>(s: impl Into<String>, size: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line: (size * 1.6).round(), paint, font: Font::Body, print: 0.0, caret: None, weight: 0.0, around: None, select: None, mark: None, nowrap: false, pose: None, track: 0.0 })
}
/// Scale one taffy length by `k` (lengths only; percentages, auto and the rest stay).
fn zoom_lp(v: LengthPercentage, k: f32) -> LengthPercentage { match v.expand() { taffy::style::ExpandedLengthPercentage::Length(x) => LengthPercentage::length(x * k), _ => v } }
fn zoom_lpa(v: LengthPercentageAuto, k: f32) -> LengthPercentageAuto { match v.expand() { taffy::style::ExpandedLengthPercentageAuto::Length(x) => LengthPercentageAuto::length(x * k), _ => v } }
fn zoom_dim(v: Dimension, k: f32) -> Dimension { match v.expand() { taffy::style::ExpandedDimension::Length(x) => Dimension::length(x * k), _ => v } }
fn zoom_style(s: &mut Style, k: f32) {
    s.size = Size { width: zoom_dim(s.size.width, k), height: zoom_dim(s.size.height, k) };
    s.min_size = Size { width: zoom_lpa(s.min_size.width, k), height: zoom_lpa(s.min_size.height, k) };
    s.max_size = Size { width: zoom_lpa(s.max_size.width, k), height: zoom_lpa(s.max_size.height, k) };
    s.flex_basis = zoom_dim(s.flex_basis, k);
    s.gap = Size { width: zoom_lp(s.gap.width, k), height: zoom_lp(s.gap.height, k) };
    for r in [&mut s.padding, &mut s.border] { *r = Rect { left: zoom_lp(r.left, k), right: zoom_lp(r.right, k), top: zoom_lp(r.top, k), bottom: zoom_lp(r.bottom, k) }; }
    for r in [&mut s.margin, &mut s.inset] { *r = Rect { left: zoom_lpa(r.left, k), right: zoom_lpa(r.right, k), top: zoom_lpa(r.top, k), bottom: zoom_lpa(r.bottom, k) }; }
}
impl<M> El<M> {
    /// Every length in this subtree times `k`: sizes, gaps, padding, margins, insets, corner radii,
    /// borders (never thinner than a hairline) and type. A widget made for one size shown at another
    /// (the film scales its controls with the stage, so a small window does not overflow).
    pub fn zoom(mut self, k: f32) -> Self {
        if (k - 1.0).abs() < 1e-4 || k <= 0.0 { return self; }
        zoom_style(&mut self.style, k);
        self.radius *= k;
        if let Some((w, p)) = self.border { self.border = Some(((w * k).max(w.min(1.0)), p)); }
        if let Content::Text { size, line, weight, .. } = &mut self.content { *size *= k; *line *= k; *weight *= k; }
        self.children = std::mem::take(&mut self.children).into_iter().map(|c| c.zoom(k)).collect();
        self
    }
}

/// A text leaf with an explicit line height.
pub fn text_lh<M>(s: impl Into<String>, size: f32, line: f32, paint: Paint) -> El<M> {
    El::new(Style::default(), Content::Text { s: s.into(), size, line, paint, font: Font::Body, print: 0.0, caret: None, weight: 0.0, around: None, select: None, mark: None, nowrap: false, pose: None, track: 0.0 })
}
pub fn picture<M>(id: u32, ratio: f32) -> El<M> { El::new(Style { size: Size { width: percent(1.0), height: auto() }, ..Default::default() }, Content::Picture { id, ratio, fx: 0 }) }
/// Meshes with a camera and a light (DESIGN.md §1.18), as wide as its parent and 240 px tall unless
/// sized otherwise. It is decoration by default (`Response::Loosen`), like a picture.
pub fn scene3d<M>(s: crate::mesh::Scene3d) -> El<M> {
    El::new(Style { size: Size { width: percent(1.0), height: length(240.0) }, ..Default::default() }, Content::Scene3d(Rc::new(s)))
}
impl<M: Clone + 'static> El<M> {
    /// This demo with its formula caption under it (`widgets::caption`, DESIGN.md §5.2): the line is
    /// formatted from the fields that drive the demo and the controls set those fields.
    pub fn caption_params(self, key: &str, formula: impl Into<String>, params: &[crate::widgets::CaptionParam], msg: impl Fn(usize, crate::widgets::Edit) -> M + 'static) -> El<M> {
        col().key(format!("{key}/with")).gap(10.0).min_width(0.0).child(self).child(crate::widgets::caption(key, formula, params, msg))
    }
}
/// A drawing (DESIGN.md §1.26): as wide as its parent (unless sized), as tall as the drawing's
/// proportions make it; its marks are shapes with true outlines. Pointer handlers on it get points in
/// the drawing's space.
pub fn canvas<M>(d: crate::draw::Drawing) -> El<M> { El::new(Style { size: Size { width: percent(1.0), height: auto() }, ..Default::default() }, Content::Canvas(Rc::new(d))) }
/// One mesh at the origin, with the default camera and light (`mesh::Scene3d::single`).
pub fn mesh<M>(id: crate::mesh::MeshId) -> El<M> { scene3d(crate::mesh::Scene3d::single(id)) }
/// Every child over the one before, each starting at the top left of the same box: the box is the
/// parent's (fill it with `fill_width` / `fill_height`, or size it) and does not grow with the children,
/// so a panel taller than it hangs out of it (clip the box). For sheets and floating boxes over a page;
/// a child sized `fill_width().fill_height()` covers the box.
pub fn layers<M>(children: impl IntoIterator<Item = El<M>>) -> El<M> {
    let track = || vec![minmax(length(0.0), fr(1.0))];
    let mut s = El::new(Style {
        display: Display::Grid, grid_template_columns: track(), grid_template_rows: track(),
        justify_items: AlignItems::START, align_items: AlignItems::START, ..Default::default()
    }, Content::None);
    for mut c in children {
        c.style.grid_row = Line { start: line(1), end: span(1) };
        c.style.grid_column = Line { start: line(1), end: span(1) };
        s.children.push(c);
    }
    s
}
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
