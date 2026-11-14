//! Runtime: state, view, layout, hit-testing, focus, scrolling and instance birth times. No GPU here,
//! so all of it can be tested headless.
use crate::el::{Content, Cursor, Drag, El, Font, Key, Modal, Mods, OnDrag, OnKey, OnText, Paint, Phase, Role, TextInput};
use crate::morph::{self, Cmd, Ease, FlowSpec, MorphSpec, Particle};
use crate::scene::{Field, Instance, PICTURE, SHAPE, Scene};
use crate::gaze;
use crate::style::{Gaze, GazeMode, Look, gaze as kinds};
use crate::text::{Face, MONO, Nav, Para, Text};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::hash::{DefaultHasher, Hash, Hasher};
use taffy::prelude::*;

/// Logical px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }
impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub fn right(&self) -> f32 { self.x + self.w }
    pub fn bottom(&self) -> f32 { self.y + self.h }
    pub fn center(&self) -> (f32, f32) { (self.x + self.w * 0.5, self.y + self.h * 0.5) }
    pub fn contains(&self, x: f32, y: f32) -> bool { x >= self.x && x < self.right() && y >= self.y && y < self.bottom() }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = (self.right().min(o.right()), self.bottom().min(o.bottom()));
        Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
    pub fn inset(&self, l: f32, t: f32, r: f32, b: f32) -> Rect { Rect::new(self.x + l, self.y + t, (self.w - l - r).max(0.0), (self.h - t - b).max(0.0)) }
    fn lerp(&self, o: &Rect, k: f32) -> Rect {
        Rect::new(self.x + (o.x - self.x) * k, self.y + (o.y - self.y) * k, self.w + (o.w - self.w) * k, self.h + (o.h - self.h) * k)
    }
}

/// One laid-out element. Kept in paint order (parents before children).
pub struct Node<M> {
    pub key: String,
    pub parent: Option<usize>,
    /// Border box, absolute, after scrolling.
    pub rect: Rect,
    /// Where this node is visible (its scroll ancestors' insides).
    pub clip: Rect,
    /// Content box: rect minus border and padding.
    pub inner: Rect,
    pub scroll: bool,
    /// Scrolls along x rather than y.
    pub horizontal: bool,
    pub focusable: bool,
    /// Takes no input (it or an ancestor was marked disabled) and is drawn faint.
    pub disabled: bool,
    /// For scroll nodes: length of what they hold along their axis, so offsets can be clamped.
    pub extent: f32,
    pub content: Content,
    fill: Option<Paint>,
    border: Option<(f32, Paint)>,
    radius: f32,
    on_press: Option<M>,
    on_key: Option<OnKey<M>>,
    on_drag: Option<OnDrag<M>>,
    on_blur: Option<M>,
    loose: f32,
    wipe: Option<f32>,
    bar: bool,
    reveal: bool,
    commit: bool,
    modal: Option<Modal<M>>,
    /// Laid out but not drawn (it or an ancestor is hidden until morphed); takes no input.
    pub hidden: bool,
    /// The nearest flow id at or above this node, and the node that carries it.
    flow: Option<(Rc<str>, usize)>,
    /// The material override of this subtree as the instance code (0 = none, see `style::override_code`).
    pub mat: f32,
    /// Looseness held for the whole subtree (inherited).
    pub loosen: f32,
    /// An explicit callout for the annotation layer; whether this subtree is what it looks at.
    pub annotate: Option<String>,
    pub scope: bool,
    anchor: Option<String>,
    clips: bool,
    calm: bool,
    on_text: Option<OnText<M>>,
    /// A multi-line text field; text that can be selected (read-only); the pointer's shape here.
    pub multiline: bool,
    pub selectable: bool,
    pub cursor: Option<Cursor>,
    /// A virtual list: its row count, row height and selected row.
    pub virt: Option<(usize, f32, Option<usize>)>,
    /// What it is to assistive technology, as the view said.
    pub access: Option<crate::el::Access>,
    /// The box layout gave it, relative to its parent (scrolling does not change it).
    local: taffy::Layout,
    tid: NodeId,
}
impl<M> Node<M> {
    pub fn text(&self) -> Option<&str> { if let Content::Text { s, .. } = &self.content { Some(s) } else { None } }
    pub fn pressable(&self) -> bool { self.on_press.is_some() }
    /// What the annotation layer calls this element.
    pub fn kind(&self) -> &'static str {
        match self.content {
            Content::Text { .. } => "text",
            Content::Picture { .. } => "image",
            Content::None if self.focusable || self.on_press.is_some() || self.on_drag.is_some() => "control",
            Content::None if self.fill.is_some() || self.border.is_some() => "shape",
            Content::None => "box",
        }
    }
    /// Something the pointer can act on: hover marks frame it.
    fn active(&self) -> bool { self.focusable || self.on_press.is_some() || self.on_drag.is_some() }
    /// A text field.
    pub fn editable(&self) -> bool { self.on_text.is_some() }
    /// How its text is laid out, when it is text: the same paragraph that is drawn.
    fn para(&self) -> Option<Para<'_>> {
        let Content::Text { s, size, line, font, print, around, nowrap, .. } = &self.content else { return None };
        let width = if *nowrap { f32::INFINITY } else { self.inner.w + if around.is_some() { 0.0 } else { 1.0 } };
        Some(Para { s, face: font.resolve([0; 3]), size: *size, line: *line, width, print: *print, around: around.as_deref() })
    }
    fn origin(&self) -> (f32, f32) { (self.inner.x, self.inner.y) }
}

/// A drag in progress: the handler and where the press was, captured at the press.
struct Capture<M> { key: String, handler: OnDrag<M>, start: (f32, f32), rect: Rect }

/// The frame as it was when a commit (theme, material) was pressed, and where: the renderer shows it
/// beyond a front travelling out from `at` (device px) from time `t`.
pub struct Split { pub id: u32, pub scene: Scene, pub field: Field, pub at: [f32; 2], pub t: f32 }

/// The four accent corners that frame the hovered or focused element and glide between targets.
#[derive(Default)]
struct Marks { at: Option<Rect>, clip: Rect, alpha: f32 }

/// Which instance of a flow subtree an instance is: glyphs and other instances are counted apart, and
/// a glyph only matches the same glyph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Tag { flow: Rc<str>, glyph: bool, n: u32, gid: u32 }

/// A shared part on its way from its old rect (device px) to the one layout gives it now.
struct FlowRec { from: [f32; 4], clip: [f32; 4], start: f32, dur: f32, loose: f32 }
/// An instance of the old page that has no place on the new one: it loosens and fades.
struct Ghost { inst: Instance, start: f32, dur: f32 }
/// A page instance as it was drawn, kept across a relayout for `Cmd::Flow`.
struct Old { id: u64, tag: Option<Tag>, inst: Instance }

const NEVER: f32 = f32::INFINITY;

/// Run ids are unique in the process, so a renderer shared between runtimes never takes one morph's
/// pictures for another's.
fn next_run() -> u32 {
    static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A morph in progress (DESIGN.md §1.10). Times are seconds on the runtime's clock; a ramp is
/// looseness `k0` at `t0` to `k1` at `t1`. The renderer reads it: the instances of both elements to
/// draw their pictures once, and the tiles to fly while `flight` is on.
pub struct MorphRun {
    /// Changes when there are new elements to draw (a new morph); `upload` when the tiles change.
    pub id: u32,
    pub upload: u32,
    /// The element that loosens and hands over, and the one the tiles become.
    pub from: String,
    pub to: String,
    pub spec: MorphSpec,
    /// The source is drawn (by the morph) until this time, with this looseness.
    pub src_until: f32,
    pub src_ramp: [f32; 4],
    /// While the tiles fly: start and end.
    pub flight: Option<[f32; 2]>,
    /// The target is drawn from this time on, with this looseness.
    pub dst_from: f32,
    pub dst_ramp: [f32; 4],
    pub end: f32,
    pub particles: Vec<Particle>,
    /// Tile size, device px, and the instances of the source and the target (looseness 1).
    pub tile: f32,
    pub src: Vec<Instance>,
    pub dst: Vec<Instance>,
}
impl MorphRun {
    /// Seconds into the flight, while the tiles are flying.
    pub fn clock(&self, now: f32) -> Option<f32> { self.flight.filter(|f| now >= f[0] && now < f[1]).map(|f| now - f[0]) }
    fn phase(&self, now: f32) -> u8 { (now >= self.src_until) as u8 | (self.clock(now).is_some() as u8) << 1 | ((now >= self.dst_from) as u8) << 2 }
}
pub fn ramp(r: [f32; 4], t: f32) -> f32 {
    if r[3] <= r[1] { return if t >= r[3] { r[2] } else { r[0] }; }
    r[0] + (r[2] - r[0]) * ((t - r[1]) / (r[3] - r[1])).clamp(0.0, 1.0)
}
pub struct Ui<S, M> {
    pub state: S,
    view: fn(&S) -> El<M>,
    update: fn(&mut S, M),
    text: Text,
    /// Logical size and device px per logical px.
    size: (f32, f32),
    scale: f32,
    now: f32,
    nodes: Vec<Node<M>>,
    scene: Scene,
    /// Which node and identity each instance of the page (not the marks) belongs to.
    owners: Vec<(usize, u64)>,
    page_len: usize,
    field: Field,
    scroll: HashMap<String, f32>,
    births: HashMap<u64, f32>,
    hover: Option<String>,
    focus: Option<String>,
    pressed: Option<String>,
    /// The element being dragged, if its press started a drag.
    capture: Option<Capture<M>>,
    /// The open modal layer (its node index and key), and where focus was before it opened.
    modal_at: Option<usize>,
    modal_key: Option<String>,
    return_focus: Option<String>,
    split: Option<Split>,
    splits: u32,
    /// The last input came from the keyboard, so the marks follow focus rather than the pointer.
    keyboard: bool,
    marks: Marks,
    /// Tells the state the window's logical size before each layout after a resize.
    sized: Option<fn(&mut S, f32, f32)>,
    /// Takes the commands `update` left in the state.
    commands: Option<fn(&mut S) -> Vec<Cmd>>,
    /// Instances of hidden nodes (laid out, not drawn), for sampling a morph's target.
    unseen: Vec<(usize, Instance)>,
    /// The page instances as painted, before morph ramps and flows are applied; and their flow tags.
    base: Vec<Instance>,
    tags: Vec<Option<Tag>>,
    /// Per node: 1 inside the morph's source, 2 inside its target.
    roles: Vec<u8>,
    run: Option<MorphRun>,
    painted_phase: u8,
    flows: HashMap<u64, FlowRec>,
    ghosts: Vec<Ghost>,
    /// Page instances plus ghosts: where the marks start.
    body_len: usize,
    animating: bool,
    /// Page instance → where its copy drawn over the page is, while it flows.
    lifted: HashMap<usize, usize>,
    /// The annotation layer: its settings (from the state), when each annotated element first
    /// appeared, and this frame's records.
    gaze_of: Option<fn(&S) -> Gaze>,
    gaze_seen: HashMap<String, f32>,
    annots: Vec<gaze::Annot>,
    /// Tells the state which element the pointer is over (when it changes), and the time on every
    /// tick; either returns whether the view changed.
    hovered: Option<fn(&mut S, Option<&str>) -> bool>,
    ticked: Option<fn(&mut S, f32) -> bool>,
    /// Keys nothing on the page used (the flag is Shift), and keys let go; either returns whether
    /// the state changed.
    unused_key: Option<fn(&mut S, Key, bool) -> bool>,
    released: Option<fn(&mut S, Key) -> bool>,
    /// Told of every press, wheel turn and key from the person (before it is handled); returns whether
    /// the state changed.
    input: Option<fn(&mut S) -> bool>,
    // ── Demand-driven frames (DESIGN.md §1.11) ──
    /// Something changed since the last frame was drawn (input, a new scene, an animation step).
    dirty: bool,
    /// The look, for the timings that decide whether anything is still moving.
    look_of: Option<fn(&S) -> Look>,
    timing: Timing,
    /// When the state next wants a tick (a clock, a running demonstration), asked after every tick.
    wake_of: Option<fn(&S, f32) -> Option<f32>>,
    /// The newest birth time among drawn instances (they condense for `enter` seconds after it).
    latest_birth: f32,
    /// Until when an annotation is still drawing on.
    gaze_until: f32,
    /// The marks moved or faded on the last tick.
    marks_moving: bool,
    /// The caret: its page instance, when it last moved (blinking restarts there), its rect, and
    /// whether it is shown in the current blink phase.
    caret: Option<(usize, f32, [f32; 4])>,
    caret_on: bool,
    /// Some drawn instance holds a static looseness under a time-varying material (which moves it).
    static_loose: bool,
    /// The material clock: time terms of materials (glitch's hold, xray's dashes, dot's drift) read it.
    /// It only runs while something moves, so a page at rest is a still image.
    mclock: f32,
    moving: bool,
    // ── Frame times and the readout (DESIGN.md §1.12) ──
    perf: crate::perf::Perf,
    /// Nesting of timed update / layout spans (only the outermost counts), and painting (so `finish`
    /// inside `paint` is not counted twice).
    span: u32,
    painting: bool,
    /// The readout is shown; when it was last drawn (it refreshes twice a second while shown).
    overlay: bool,
    overlay_at: f32,
    // ── Text (DESIGN.md §1.8.1) ──
    /// The text field the pointer pressed in, while the press lasts (its drag selects).
    text_capture: Option<String>,
    /// Read-only selection over selectable text: anchor and focus (node key, byte), and whether the
    /// pointer is still dragging it.
    sel: Option<((String, usize), (String, usize))>,
    sel_drag: bool,
    /// The last press: when, where, and how many presses in a row it made (a double click is 2).
    clicks: (f32, f32, f32, u8),
    /// The accessibility mirror as last sent to the host, and the focus it was told.
    a11y_sent: HashMap<String, A11yNode>,
    a11y_focus: Option<String>,
    // ── Incremental layout (DESIGN.md §1.13) ──
    /// The taffy tree, kept across layouts: each key's taffy node, and what its measured content was.
    tree: TaffyTree<usize>,
    tids: HashMap<String, NodeId>,
    mkeys: HashMap<NodeId, u64>,
    /// Virtual lists: each one's viewport height as last placed, and the rows it has built.
    virt_views: HashMap<String, f32>,
    virt_made: HashMap<String, (usize, usize)>,
    // ── Touch (DESIGN.md §1.8.2) ──
    pub scrolling: Scrolling,
    touch: Option<Touch>,
    touches: HashSet<u64>,
    /// A finger is down: no hover, no lens.
    touching: bool,
    fling: Option<(String, f32)>,
    /// How far each scroll node is pulled past its end (negative: past the start), logical px.
    over: HashMap<String, f32>,
    /// When the pointer event being handled happened (the host's timestamp), if it says: a finger's
    /// speed is measured by these, not by when the events were handled.
    event_t: Option<f32>,
}

/// One element of the accessibility mirror (DESIGN.md §1.14): what a screen reader is told about it.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNode {
    pub key: String,
    /// The nearest ancestor that is in the mirror.
    pub parent: Option<String>,
    pub role: Role,
    pub name: String,
    pub value: Option<(f32, f32, f32)>,
    pub text: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub level: u8,
    pub modal: bool,
    pub disabled: bool,
    pub focusable: bool,
    /// Where it is (logical px).
    pub rect: Rect,
}
/// A change to the mirror since it was last sent.
#[derive(Clone, Debug, PartialEq)]
pub enum A11yOp { Set(A11yNode), Remove(String), Focus(Option<String>) }

/// The kind of pointer: a mouse and a pen hover; a finger does not, and drags scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind { Mouse, Touch, Pen }

/// How touch scrolling behaves (logical px, seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrolling {
    /// After the finger lifts the content keeps going and slows: velocity × e^(−friction · t).
    pub friction: f32,
    pub max_velocity: f32,
    /// A finger must move this far before a press becomes a scroll.
    pub slop: f32,
    /// Past the end the content does not move; the pull (`rubber` of the finger's movement, less as it
    /// grows, at most `max_over`) shows as looseness at that edge and springs back at `spring` per second.
    pub rubber: f32,
    pub max_over: f32,
    pub spring: f32,
}
impl Default for Scrolling {
    fn default() -> Self { Self { friction: 3.5, max_velocity: 5000.0, slop: 8.0, rubber: 0.55, max_over: 140.0, spring: 9.0 } }
}
/// One step of a fling: how far it goes in `dt` and its velocity after (exact for exponential decay).
pub fn fling_step(v: f32, dt: f32, friction: f32) -> (f32, f32) {
    let k = (-friction.max(1e-3) * dt).exp();
    (v * (1.0 - k) / friction.max(1e-3), v * k)
}
/// The pull past an end after the finger moves `d` more: it resists more the further it is pulled.
pub fn rubber_band(over: f32, d: f32, rubber: f32, max: f32) -> f32 {
    let r = rubber * (1.0 - (over.abs() / max.max(1.0)).min(1.0));
    (over + d * r).clamp(-max, max)
}

/// The finger that is down: where it started, its last positions (for the velocity), and what it is
/// doing: not yet decided (a press shown but not sent), scrolling node `key`, handled as a mouse press
/// (a drag, a text field, nothing to scroll), or cancelled (a second finger).
struct Touch { id: u64, start: (f32, f32), last: (f32, f32), samples: std::collections::VecDeque<(f32, f32, f32)>, mode: TouchMode }
#[derive(Clone, PartialEq)]
enum TouchMode { Pending, Scroll(String), Direct, Cancelled }

/// What the runtime needs from the look to know whether anything is still moving: how long fronts live
/// and how fast they travel, how long new instances condense, and whether the material (global, or
/// any override) has a time term that moves loose things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing { pub wave_life: f32, pub wave_speed: f32, pub enter: f32, pub material: u8, pub overlay: bool }
impl Default for Timing {
    fn default() -> Self { let f = crate::scene::Fx::default(); Self { wave_life: f.wave_life, wave_speed: f.wave_speed, enter: f.enter, material: crate::style::Material::default().id(), overlay: false } }
}
impl Timing {
    pub fn of(l: &Look) -> Self { Self { wave_life: l.fx.wave_life, wave_speed: l.fx.wave_speed, enter: l.fx.enter, material: l.material.id(), overlay: l.overlay } }
}

/// Blink half-period of the caret, seconds.
pub const BLINK: f32 = 0.53;
/// How far a selected glyph's halo reaches past its outline, logical px.
pub const SELECT_HALO: f32 = 3.0;
/// How far from a pulled edge (logical px) things loosen.
const PULL_REACH: f32 = 160.0;
/// The smallest box (logical px) a control can be hit in: smaller ones are hit as if they were this big.
pub const MIN_HIT: f32 = 28.0;
/// Presses closer than this in time (s) and space (px) count as a double (triple) click.
const CLICK_GAP: (f32, f32) = (0.45, 5.0);

impl<S, M: Clone> Ui<S, M> {
    /// Let the state know the window's logical size whenever it changes (for views that lay out
    /// differently on narrow windows).
    pub fn on_resize(mut self, f: fn(&mut S, f32, f32)) -> Self {
        f(&mut self.state, self.size.0, self.size.1);
        self.sized = Some(f);
        self.relayout();
        self
    }
    /// Read the annotation layer's settings from the state (DESIGN.md §1.7).
    pub fn on_gaze(mut self, f: fn(&S) -> Gaze) -> Self { self.gaze_of = Some(f); self.finish(); self }
    /// Let the state know which element the pointer is over; `f` returns whether the view changed.
    pub fn on_hover(mut self, f: fn(&mut S, Option<&str>) -> bool) -> Self { self.hovered = Some(f); self }
    /// Let the state know the time on every tick (for things that change by themselves, like a clock);
    /// `f` returns whether the view changed.
    pub fn on_tick(mut self, f: fn(&mut S, f32) -> bool) -> Self { self.ticked = Some(f); self }
    /// Read the look from the state, for the timings `needs_frame` uses (front life and speed, how long
    /// new things condense, whether the material moves by itself).
    pub fn on_look(mut self, f: fn(&S) -> Look) -> Self { self.look_of = Some(f); self.timing = Timing::of(&f(&self.state)); self }
    /// Ask the state, after every tick, when it next wants one (`Some(now)`: every frame; a later time:
    /// a timed wake; `None`: only on input). Without it, `on_tick` subscribers only tick when something
    /// else draws a frame.
    pub fn on_wake(mut self, f: fn(&S, f32) -> Option<f32>) -> Self { self.wake_of = Some(f); self }
    /// Give the state the keys nothing on the page used (Esc on a scene, a key that is held);
    /// `f` returns whether the state changed.
    pub fn on_unused_key(mut self, f: fn(&mut S, Key, bool) -> bool) -> Self { self.unused_key = Some(f); self }
    /// Hear of every press, wheel turn and key press before it is handled (a demonstration that the
    /// person takes over).
    pub fn on_input(mut self, f: fn(&mut S) -> bool) -> Self { self.input = Some(f); self }
    fn heard(&mut self) { if let Some(f) = self.input && f(&mut self.state) { self.relayout(); } }
    /// Tell the state when a key is let go; `f` returns whether the state changed.
    pub fn on_key_up(mut self, f: fn(&mut S, Key) -> bool) -> Self { self.released = Some(f); self }
    /// A key let go.
    pub fn key_up(&mut self, k: Key) {
        self.dirty = true;
        if let Some(f) = self.released && f(&mut self.state, k) { self.relayout(); }
    }
    /// This frame's annotation records.
    pub fn annots(&self) -> &[gaze::Annot] { &self.annots }
    /// Take the commands `update` leaves in the state (morphs, flows) after every update.
    pub fn on_commands(mut self, f: fn(&mut S) -> Vec<Cmd>) -> Self { self.commands = Some(f); self }
    /// `w`, `h` are device px; `scale` is device px per logical px.
    pub fn new(state: S, view: fn(&S) -> El<M>, update: fn(&mut S, M), faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Self {
        let mut ui = Self {
            state, view, update, text: Text::new(faces), size: (0.0, 0.0), scale, now: 0.0, nodes: vec![], scene: Scene::default(),
            owners: vec![], page_len: 0, field: Field::default(), scroll: HashMap::new(), births: HashMap::new(),
            hover: None, focus: None, pressed: None, capture: None, modal_at: None, modal_key: None, return_focus: None, split: None, splits: 0,
            keyboard: false, marks: Marks::default(), sized: None, commands: None, unseen: vec![], base: vec![], tags: vec![], roles: vec![],
            run: None, painted_phase: 0, flows: HashMap::new(), ghosts: vec![], body_len: 0, animating: false, lifted: HashMap::new(),
            gaze_of: None, gaze_seen: HashMap::new(), annots: vec![], hovered: None, ticked: None, unused_key: None, released: None, input: None,
            dirty: true, look_of: None, timing: Timing::default(), wake_of: None, latest_birth: f32::NEG_INFINITY, gaze_until: f32::NEG_INFINITY,
            marks_moving: false, caret: None, caret_on: true, static_loose: false, mclock: 0.0, moving: false,
            perf: crate::perf::Perf { quality: 1.0, ..Default::default() }, span: 0, painting: false, overlay: false, overlay_at: 0.0,
            text_capture: None, sel: None, sel_drag: false, clicks: (f32::NEG_INFINITY, 0.0, 0.0, 0), a11y_sent: HashMap::new(), a11y_focus: None,
            tree: TaffyTree::new(), tids: HashMap::new(), mkeys: HashMap::new(), virt_views: HashMap::new(), virt_made: HashMap::new(),
            scrolling: Scrolling::default(), touch: None, touches: HashSet::new(), touching: false, fling: None, over: HashMap::new(), event_t: None,
        };
        ui.resize(w, h, scale);
        ui
    }

    // ── Queries ──────────────────────────────────────────────────────────────────────────────────
    pub fn nodes(&self) -> &[Node<M>] { &self.nodes }
    pub fn node(&self, key: &str) -> Option<&Node<M>> { self.nodes.iter().find(|n| n.key == key) }
    pub fn scroll_offset(&self, key: &str) -> f32 { self.scroll.get(key).copied().unwrap_or(0.0) }
    pub fn focus(&self) -> Option<&str> { self.focus.as_deref() }
    pub fn hover(&self) -> Option<&str> { self.hover.as_deref() }
    pub fn scene(&self) -> &Scene { &self.scene }
    pub fn field(&self) -> &Field { &self.field }
    pub fn text_mut(&mut self) -> &mut Text { &mut self.text }
    pub fn scale(&self) -> f32 { self.scale }
    pub fn now(&self) -> f32 { self.now }
    pub fn logical_size(&self) -> (f32, f32) { self.size }
    /// Device px.
    pub fn device_size(&self) -> (u32, u32) { ((self.size.0 * self.scale).round() as u32, (self.size.1 * self.scale).round() as u32) }
    /// The key of the open modal layer, if any.
    pub fn modal(&self) -> Option<&str> { self.modal_key.as_deref() }
    /// The last commit's frame, kept for the renderer.
    pub fn split(&self) -> Option<&Split> { self.split.as_ref() }
    pub fn dragging(&self) -> Option<&str> { self.capture.as_ref().map(|c| c.key.as_str()) }
    /// The morph in progress, if any.
    pub fn morph_run(&self) -> Option<&MorphRun> { self.run.as_ref() }
    /// The drawn page instances of a node and its descendants, as they are this frame.
    pub fn instances_of(&self, key: &str) -> Vec<Instance> {
        let Some(root) = self.nodes.iter().position(|n| n.key == key) else { return vec![] };
        (0..self.page_len).filter(|&i| within(&self.nodes, self.owners[i].0, root)).map(|i| self.scene.instances[*self.lifted.get(&i).unwrap_or(&i)]).collect()
    }
    /// How many instances of an old page are still dissolving.
    pub fn dissolving(&self) -> usize { self.ghosts.len() }
    /// Whether any part of a node is in a flow right now.
    pub fn flowing(&self) -> usize { self.flows.len() }

    // ── Frames ───────────────────────────────────────────────────────────────────────────────────
    /// Whether a frame has to be drawn at `now` (after `tick(now)`): input arrived or the scene changed
    /// since the last drawn frame, or something is moving (a front, a burst, condensing, a morph, a
    /// flow, the old page dissolving, the marks gliding, an annotation drawing on, a commit's front
    /// still on screen, a time-varying material with something loose), or a timed wake is due (the
    /// caret's blink, a clock the state keeps). When it is false the host draws nothing and waits for
    /// input or `next_wake`.
    pub fn needs_frame(&self, now: f32) -> bool { self.dirty || self.moving_at(now) || self.wake_at(now).is_some_and(|w| w <= now) }
    /// When the next frame is needed: `now` if one is needed now, a later time for a timed wake, `None`
    /// when only input can change anything.
    pub fn next_wake(&self, now: f32) -> Option<f32> {
        if self.dirty || self.moving_at(now) { return Some(now); }
        self.wake_at(now).map(|w| w.max(now))
    }
    /// Ask for a frame even though nothing changed (the host lost what it showed).
    pub fn invalidate(&mut self) { self.dirty = true; }
    /// The host drew a frame of the current scene.
    pub fn frame_drawn(&mut self) { self.dirty = false; }
    /// Frame times and work counters (the renderer closes each frame with `perf_mut().frame(..)`).
    pub fn perf(&self) -> &crate::perf::Perf { &self.perf }
    pub fn perf_mut(&mut self) -> &mut crate::perf::Perf { &mut self.perf }
    /// The work counters (layout passes, paragraphs shaped, views, scenes).
    pub fn counters(&self) -> crate::perf::Counters { let mut c = self.perf.counters; c.shaped = self.text.shaped(); c }
    fn span_begin(&mut self) -> (f64, f32) { self.span += 1; (crate::perf::now_ms(), self.perf.pending.scene) }
    fn span_end(&mut self, (t0, scene0): (f64, f32)) {
        self.span -= 1;
        if self.span == 0 {
            let spent = (crate::perf::now_ms() - t0) as f32 - (self.perf.pending.scene - scene0);
            self.perf.pending.update_layout += spent.max(0.0);
        }
    }
    /// The clock that materials' time terms read: it runs only while something moves.
    pub fn material_clock(&self) -> f32 { self.mclock }
    /// Is anything animating at `now`? (Not counting input or timed wakes.)
    pub fn moving_at(&self, now: f32) -> bool {
        let tm = &self.timing;
        let fronts = self.field.fronts.iter().any(|f| f[3] > 0.0 && now - f[2] < tm.wave_life);
        let split = self.split.as_ref().is_some_and(|sp| {
            let (w, h) = (self.size.0 * self.scale, self.size.1 * self.scale);
            (now - sp.t) * tm.wave_speed * self.scale < (w * w + h * h).sqrt() + 1.0
        });
        fronts || split || self.field.burst_at(now) > 0.0 || now < self.latest_birth + tm.enter
            || self.run.is_some() || !self.flows.is_empty() || !self.ghosts.is_empty() || self.animating
            || self.marks_moving || now < self.gaze_until || self.fling.is_some() || !self.over.is_empty()
            || self.static_loose
    }
    /// About how much of the window (0..1) can be loose at `now`: all of it while a front lives, things
    /// condense, a morph or flow runs; otherwise the area of instances held loose, a burst, the lens over
    /// pictures and a compare wipe. The renderer uses it to decide whether a reduced material pass pays.
    pub fn loose_estimate(&self, now: f32, lens_radius: f32) -> f32 {
        let tm = &self.timing;
        if self.field.fronts.iter().any(|f| f[3] > 0.0 && now - f[2] < tm.wave_life) || now < self.latest_birth + tm.enter
            || self.run.is_some() || !self.flows.is_empty() || !self.ghosts.is_empty() { return 1.0; }
        let s = self.scale;
        let screen = (self.size.0 * s * self.size.1 * s).max(1.0);
        let clip = |r: [f32; 4]| { let w = (r[0] + r[2]).min(self.size.0 * s) - r[0].max(0.0); let h = (r[1] + r[3]).min(self.size.1 * s) - r[1].max(0.0); w.max(0.0) * h.max(0.0) };
        let mut area: f32 = self.scene.instances[..self.page_len].iter().filter(|i| i.extra[2] > 0.0).map(|i| clip(i.rect)).sum();
        if self.field.burst_at(now) > 0.0 { let b = self.field.burst; area += clip([b[0], b[1], b[2] - b[0], b[3] - b[1]]); }
        let pictures = self.scene.instances.iter().any(|i| i.paint[3] == PICTURE);
        if pictures && self.field.pointer[2] > 0.0 { area += std::f32::consts::PI * (lens_radius * s * 1.5).powi(2); }
        if self.field.wipe[1] > 0.0 { let r = self.field.wipe_rect; area += (r[2] - self.field.wipe[0]).max(0.0) * (r[3] - r[1]).max(0.0); }
        (area / screen).min(1.0)
    }
    /// The next timed wake: the caret's next blink, or when the state asked for a tick.
    fn wake_at(&self, now: f32) -> Option<f32> {
        let blink = self.caret.map(|(_, reset, _)| reset + ((now - reset) / BLINK).floor().max(0.0) * BLINK + BLINK);
        let asked = self.wake_of.and_then(|f| f(&self.state, now));
        // The readout refreshes twice a second while it is shown, so its frame counts fall to 0 at rest.
        let readout = self.overlay.then_some(self.overlay_at + 0.5);
        [blink, asked, readout].into_iter().flatten().reduce(f32::min)
    }

    // ── Input ────────────────────────────────────────────────────────────────────────────────────
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
        self.dirty = true;
        self.scale = scale;
        self.text.set_scale(scale);
        let size = (w.max(1) as f32 / scale, h.max(1) as f32 / scale);
        // A morph's tiles are in device px of the old size.
        if size != self.size { self.run = None; }
        self.size = size;
        if let Some(f) = self.sized { f(&mut self.state, self.size.0, self.size.1); }
        self.relayout();
    }

    /// Advance the clock to `t` seconds; the marks glide toward their target.
    pub fn tick(&mut self, t: f32) {
        let dt = (t - self.now).clamp(0.0, 0.1);
        // The material clock runs only while something was moving.
        if self.moving { self.mclock += dt; }
        self.now = t;
        if let Some(f) = self.look_of { self.timing = Timing::of(&f(&self.state)); }
        if let Some(f) = self.ticked && f(&mut self.state, t) { self.relayout(); }
        let target = if self.keyboard { self.focus.clone() } else { self.hover.clone().or_else(|| self.focus.clone()) };
        let target = target.and_then(|k| self.node(&k).map(|n| (n.rect, n.clip)));
        let k = 1.0 - (-dt * 16.0).exp();
        let before = (self.marks.at, self.marks.alpha);
        match target {
            Some((r, clip)) => {
                let next = match self.marks.at { Some(cur) if self.marks.alpha > 0.01 => cur.lerp(&r, k), _ => r };
                // Close enough is there: the glide ends instead of creeping on for ever.
                let near = (next.x - r.x).abs() + (next.y - r.y).abs() + (next.w - r.w).abs() + (next.h - r.h).abs() < 0.1;
                self.marks.at = Some(if near { r } else { next });
                self.marks.clip = clip;
                self.marks.alpha += (1.0 - self.marks.alpha) * k;
                if self.marks.alpha > 0.995 { self.marks.alpha = 1.0; }
            }
            None => { self.marks.alpha -= self.marks.alpha * k; if self.marks.alpha < 0.02 { self.marks.alpha = 0.0; } }
        }
        self.marks_moving = before != (self.marks.at, self.marks.alpha);
        // The caret blinks in whole phases from when it last moved.
        if let Some((_, reset, _)) = self.caret {
            let on = ((t - reset).max(0.0) / BLINK).floor() as i64 % 2 == 0;
            if on != self.caret_on { self.caret_on = on; self.dirty = true; }
        }
        // A morph moving into its next phase changes what is drawn; one that is over is dropped and the
        // view decides again.
        let repaint = match &self.run {
            Some(r) if t >= r.end => { self.run = None; true }
            Some(r) => r.phase(t) != self.painted_phase,
            None => false,
        };
        self.flows.retain(|_, f| t < f.start + f.dur);
        let scrolled = self.step_scrolling(dt);
        if repaint || scrolled { self.paint(); } else { self.finish(); }
        if self.marks_moving { self.dirty = true; }
        self.moving = self.moving_at(t);
    }

    /// A fling going on and pulls springing back (it paints what moved; the result is whether the caller
    /// still has to).
    fn step_scrolling(&mut self, dt: f32) -> bool {
        let mut moved = false;
        if let Some((key, v)) = self.fling.clone() {
            let (d, v2) = fling_step(v, dt, self.scrolling.friction);
            match self.nodes.iter().position(|n| n.key == key) {
                Some(i) if v2.abs() >= 20.0 => {
                    if self.scroll_by(i, d) { self.fling = Some((key, v2)); }
                    else {
                        // It reached the end: what is left of its speed pulls a little past it.
                        let o = self.over.get(&key).copied().unwrap_or(0.0);
                        self.over.insert(key, rubber_band(o, v2 * 0.04, self.scrolling.rubber, self.scrolling.max_over));
                        self.fling = None;
                    }
                    moved = true;
                }
                _ => self.fling = None,
            }
        }
        let held = match &self.touch { Some(Touch { mode: TouchMode::Scroll(k), .. }) => Some(k.clone()), _ => None };
        let k = (-self.scrolling.spring * dt).exp();
        let before = self.over.len();
        self.over.retain(|key, o| { if held.as_deref() != Some(key.as_str()) { *o *= k; } o.abs() >= 0.5 });
        moved |= !self.over.is_empty() || before != self.over.len();
        // Moved: placed again (a virtual list builds the rows a fling brings into view) and painted.
        if moved { self.scrolled(); }
        false
    }

    // ── Pointers of every kind (DESIGN.md §1.8.2) ──────────────────────────────────────────────────
    /// A pointer pressed: a mouse or a pen presses at once; a finger shows the press (a front, the
    /// control held) but only sends it when it lifts without having scrolled — unless what it is on is
    /// dragged (a slider) or is a text field, or there is nothing to scroll, which it presses at once.
    pub fn pointer_down(&mut self, id: u64, kind: PointerKind, x: f32, y: f32, shift: bool) {
        if kind != PointerKind::Touch { self.touching = false; self.press_with(x, y, shift); return; }
        self.dirty = true;
        self.touches.insert(id);
        self.fling = None;
        if self.touches.len() > 1 {
            // A second finger (a pinch): what the first was doing is dropped, and nothing is sent.
            let direct = self.touch.as_mut().map(|t| (std::mem::replace(&mut t.mode, TouchMode::Cancelled) == TouchMode::Direct, t.last));
            if let Some((true, (lx, ly))) = direct { self.release(lx, ly); }
            if let Some(k) = self.pressed.take() { self.rebirth(&k); }
            return;
        }
        self.touching = true;
        self.hover = None;
        let hit = self.hit_target(x, y);
        let direct = hit.and_then(|i| self.up(i, |n| n.on_drag.is_some() || n.on_text.is_some())).is_some() || hit.and_then(|i| self.up(i, |n| n.scroll)).is_none();
        let mode = if direct { self.press_with(x, y, shift); TouchMode::Direct } else {
            self.heard();
            let s = self.scale;
            self.field.front(x * s, y * s, self.now, 1.0);
            self.pressed = hit.and_then(|i| self.up(i, |n| n.on_press.is_some())).map(|i| self.nodes[i].key.clone());
            if let Some(i) = hit.and_then(|i| self.up(i, |n| n.on_press.is_some())) { let r = self.nodes[i].rect; self.burst(r, 1.0, 0.35); }
            self.paint();
            TouchMode::Pending
        };
        let at = self.event_t.take().unwrap_or(self.now);
        self.touch = Some(Touch { id, start: (x, y), last: (x, y), samples: [(at, x, y)].into(), mode });
    }
    /// The time (on the runtime's clock) the next pointer event happened, when the host knows it; a
    /// finger's speed is measured from these.
    pub fn event_time(&mut self, t: f32) { self.event_t = Some(t); }
    /// A pointer moved (a mouse or pen also hovers).
    pub fn pointer_moved(&mut self, id: u64, kind: PointerKind, x: f32, y: f32) {
        if kind != PointerKind::Touch { self.touching = false; self.pointer_move(x, y); return; }
        let now = self.event_t.take().unwrap_or(self.now);
        let Some(t) = &mut self.touch else { return };
        if t.id != id || t.mode == TouchMode::Cancelled { return; }
        t.samples.push_back((now, x, y));
        while t.samples.len() > 2 && t.samples.front().is_some_and(|s| now - s.0 > 0.1) { t.samples.pop_front(); }
        let (last, start) = (t.last, t.start);
        t.last = (x, y);
        match t.mode.clone() {
            TouchMode::Direct => self.pointer_move(x, y),
            TouchMode::Pending => {
                let (dx, dy) = (x - start.0, y - start.1);
                if dx.hypot(dy) < self.scrolling.slop { return; }
                // Now it is a scroll: of the nearest scroll ancestor along the way the finger went.
                let vertical = dy.abs() >= dx.abs();
                let at = self.hit(start.0, start.1);
                let node = at.and_then(|i| self.up(i, |n| n.scroll && n.horizontal != vertical)).or_else(|| at.and_then(|i| self.up(i, |n| n.scroll)));
                if let Some(k) = self.pressed.take() { self.rebirth(&k); }
                let Some(i) = node else { if let Some(t) = &mut self.touch { t.mode = TouchMode::Cancelled; } return };
                let key = self.nodes[i].key.clone();
                if let Some(t) = &mut self.touch { t.mode = TouchMode::Scroll(key.clone()); }
                let d = if self.nodes[i].horizontal { -(x - last.0) } else { -(y - last.1) };
                self.drag_scroll(i, d);
            }
            TouchMode::Scroll(key) => {
                let Some(i) = self.nodes.iter().position(|n| n.key == key) else { return };
                let d = if self.nodes[i].horizontal { -(x - last.0) } else { -(y - last.1) };
                self.drag_scroll(i, d);
            }
            TouchMode::Cancelled => {}
        }
    }
    /// Move scroll node `i` with the finger by `d`; past an end, pull instead.
    fn drag_scroll(&mut self, i: usize, d: f32) {
        self.dirty = true;
        let key = self.nodes[i].key.clone();
        let o = self.over.get(&key).copied().unwrap_or(0.0);
        // A pull is given back first, then the content moves.
        let back = if o != 0.0 && o.signum() != d.signum() { d.abs().min(o.abs()) * d.signum() } else { 0.0 };
        if back != 0.0 { self.over.insert(key.clone(), o + back); }
        let rest = d - back;
        if rest != 0.0 && !self.scroll_by(i, rest) {
            let o = self.over.get(&key).copied().unwrap_or(0.0);
            self.over.insert(key, rubber_band(o, rest, self.scrolling.rubber, self.scrolling.max_over));
        }
        self.scrolled();
    }
    /// A pointer let go. A finger that scrolled flings on at its speed (at most `max_velocity`); one that
    /// did not is a tap: the press is sent now.
    pub fn pointer_up(&mut self, id: u64, kind: PointerKind, x: f32, y: f32) {
        if kind != PointerKind::Touch { self.release(x, y); return; }
        self.dirty = true;
        self.touches.remove(&id);
        let Some(t) = self.touch.take_if(|t| t.id == id) else { return };
        self.touching = false;
        match t.mode {
            TouchMode::Direct => { self.touching = true; self.release(x, y); self.touching = false; }
            TouchMode::Pending => {
                if let Some(k) = self.pressed.take() { self.rebirth(&k); }
                self.touching = true;
                self.press_with(x, y, false);
                self.release(x, y);
                self.touching = false;
            }
            TouchMode::Scroll(key) => {
                let (a, b) = (t.samples.front().copied(), t.samples.back().copied());
                if let (Some(a), Some(b)) = (a, b) && b.0 - a.0 > 1e-3 {
                    let horizontal = self.nodes.iter().find(|n| n.key == key).is_some_and(|n| n.horizontal);
                    let v = -if horizontal { b.1 - a.1 } else { b.2 - a.2 } / (b.0 - a.0);
                    let v = v.clamp(-self.scrolling.max_velocity, self.scrolling.max_velocity);
                    if v.abs() > 50.0 && !self.over.contains_key(&key) { self.fling = Some((key, v)); }
                }
            }
            TouchMode::Cancelled => {}
        }
        self.hover = None;
    }
    /// A pointer the system took away (a gesture of its own): nothing is sent.
    pub fn pointer_cancel(&mut self, id: u64) {
        self.dirty = true;
        self.touches.remove(&id);
        let Some(t) = self.touch.take_if(|t| t.id == id) else { return };
        if let Some(k) = self.pressed.take() { self.rebirth(&k); }
        if t.mode == TouchMode::Direct { self.release(t.last.0, t.last.1); }
        self.touching = false;
    }
    /// How far a scroll node is pulled past its end right now (negative: past its start).
    pub fn overscroll(&self, key: &str) -> f32 { self.over.get(key).copied().unwrap_or(0.0) }
    /// Whether a fling is going on.
    pub fn flinging(&self) -> bool { self.fling.is_some() }

    /// Start again at `t`: everything on the page condenses in from loose.
    pub fn restart(&mut self, t: f32) {
        self.now = t;
        for b in self.births.values_mut() { *b = t; }
        self.paint();
    }

    /// Logical px.
    pub fn pointer_move(&mut self, x: f32, y: f32) {
        self.dirty = true;
        let s = self.scale;
        self.keyboard = false;
        // A finger has no hover: it moves what it holds, and the lens does not follow it.
        let hover = if self.touching { None } else {
            self.field.pointer = [x * s, y * s, 1.0, 0.0];
            self.hit_target(x, y).and_then(|i| self.up(i, Node::active)).map(|i| self.nodes[i].key.clone())
        };
        if hover != self.hover {
            // A control the pointer comes onto gets a short burst over it (the glitch material reads it).
            if let Some(n) = hover.as_deref().and_then(|k| self.node(k)) { self.burst(n.rect, 0.75, 0.22); }
            self.hover = hover;
            if let Some(f) = self.hovered && f(&mut self.state, self.hover.as_deref()) { self.relayout(); }
        }
        if self.capture.is_some() { self.drag(Phase::Move, x, y); }
        if let Some(k) = self.text_capture.clone() && let Some(fi) = self.nodes.iter().position(|n| n.key == k) && self.field_at() == Some(fi)
            && let Some(at) = self.leaf_of(fi).and_then(|l| self.text_hit(l, x, y)) {
            self.send_text(TextInput::Pointer { at, phase: Phase::Move, clicks: self.clicks.3, shift: false });
        }
        if self.sel_drag && let Some(i) = self.sel_target(x, y) && let Some(at) = self.text_hit(i, x, y) {
            let key = self.nodes[i].key.clone();
            if let Some((_, focus)) = &mut self.sel && *focus != (key.clone(), at) { *focus = (key, at); self.paint(); }
        }
    }

    /// Give the captured handler the pointer at (x, y), logical px. The element's current rect is
    /// used when it still exists, so a drag follows an element that moves.
    fn drag(&mut self, phase: Phase, x: f32, y: f32) {
        let Some(c) = &mut self.capture else { return };
        if let Some(n) = self.nodes.iter().find(|n| n.key == c.key) { c.rect = n.rect; }
        let r = c.rect;
        let d = Drag { phase, x: x - r.x, y: y - r.y, w: r.w, h: r.h, dx: x - c.start.0, dy: y - c.start.1 };
        if let Some(m) = (c.handler)(d) { self.send(m); }
    }
    pub fn pointer_leave(&mut self) {
        self.dirty = true;
        self.field.pointer = [-1e5, -1e5, 0.0, 0.0];
        if self.hover.take().is_some() && let Some(f) = self.hovered && f(&mut self.state, None) { self.relayout(); }
    }
    /// A burst over a rect (logical px) from now: `strength` 0..1 for `secs`.
    pub fn burst(&mut self, r: Rect, strength: f32, secs: f32) {
        let s = self.scale;
        self.field.burst(Some([r.x * s, r.y * s, r.right() * s, r.bottom() * s]), self.now, strength, secs);
    }

    pub fn press(&mut self, x: f32, y: f32) { self.press_with(x, y, false); }
    /// A press with Shift held or not (Shift extends a text selection).
    pub fn press_with(&mut self, x: f32, y: f32, shift: bool) {
        // A press without a release before it (a lost pointer-up) ends the old capture.
        self.capture = None;
        self.text_capture = None;
        self.sel_drag = false;
        self.heard();
        self.pointer_move(x, y);
        let (t0, x0, y0, n0) = self.clicks;
        let clicks = if self.now - t0 < CLICK_GAP.0 && (x - x0).abs() < CLICK_GAP.1 && (y - y0).abs() < CLICK_GAP.1 { n0.saturating_add(1) } else { 1 };
        self.clicks = (self.now, x, y, clicks);
        let s = self.scale;
        self.field.front(x * s, y * s, self.now, 1.0);
        let target = self.hit_target(x, y).and_then(|i| self.up(i, |n| n.focusable)).map(|i| self.nodes[i].key.clone());
        self.set_focus(target);
        // Losing focus may have changed the page, so look again.
        let hit = self.hit_target(x, y);
        // In a text field the press places the caret (two select a word, three everything) and the drag
        // that follows selects.
        if let Some(fi) = hit.and_then(|i| self.up(i, |n| n.on_text.is_some())) {
            if self.sel.take().is_some() { self.paint(); }
            self.text_capture = Some(self.nodes[fi].key.clone());
            if let Some(at) = self.leaf_of(fi).and_then(|l| self.text_hit(l, x, y)) {
                self.send_text(TextInput::Pointer { at, phase: Phase::Start, clicks, shift });
            }
            return;
        }
        if let Some(i) = hit.and_then(|i| self.up(i, |n| n.on_drag.is_some())) {
            let n = &self.nodes[i];
            let handler = n.on_drag.clone().expect("drag handler");
            self.capture = Some(Capture { key: n.key.clone(), handler, start: (x, y), rect: n.rect });
            self.drag(Phase::Start, x, y);
            return;
        }
        let pressed = hit.and_then(|i| self.up(i, |n| n.on_press.is_some()));
        // Nothing to press here: selectable text starts a selection there; anywhere else ends one.
        if pressed.is_none() {
            match hit.filter(|&i| self.nodes[i].selectable && self.nodes[i].text().is_some()) {
                Some(i) => {
                    let at = self.text_hit(i, x, y).unwrap_or(0);
                    let s = self.nodes[i].text().unwrap_or_default();
                    let (a, b) = match clicks { 1 => (at, at), 2 => crate::text::word_range(s, at), _ => (0, s.len()) };
                    let key = self.nodes[i].key.clone();
                    self.sel = Some(((key.clone(), a), (key, b)));
                    self.sel_drag = true;
                    self.paint();
                }
                None => if self.sel.take().is_some() { self.paint(); },
            }
        }
        if let Some(i) = pressed {
            self.pressed = Some(self.nodes[i].key.clone());
            let r = self.nodes[i].rect;
            self.burst(r, 1.0, 0.35);
            if self.up(i, |n| n.commit).is_some() { self.snapshot(x, y); }
            let m = self.nodes[i].on_press.clone();
            if let Some(m) = m { self.send(m); } else { self.paint(); }
        }
    }

    pub fn release(&mut self, x: f32, y: f32) {
        self.dirty = true;
        if let Some(k) = self.text_capture.take() && self.field_at().is_some_and(|f| self.nodes[f].key == k)
            && let Some(at) = self.field_at().and_then(|f| self.leaf_of(f)).and_then(|l| self.text_hit(l, x, y)) {
            self.send_text(TextInput::Pointer { at, phase: Phase::End, clicks: self.clicks.3, shift: false });
        }
        self.sel_drag = false;
        if self.capture.is_some() {
            self.drag(Phase::End, x, y);
            self.capture = None;
        }
        // A held element condenses back from loose when let go.
        if let Some(k) = self.pressed.take() { self.rebirth(&k); }
    }

    /// Scroll the nearest scroll ancestor under the point by `dy` logical px; when it is already at its
    /// end, the next one out takes it.
    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) { self.wheel_xy(x, y, 0.0, dy); }
    /// A wheel with both axes. A horizontal scroll node takes `dx`, or `dy` when there is no `dx`
    /// (most wheels only turn one way).
    pub fn wheel_xy(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        self.dirty = true;
        self.heard();
        let mut at = self.hit(x, y);
        while let Some(i) = at.and_then(|i| self.up(i, |n| n.scroll)) {
            let d = if self.nodes[i].horizontal { if dx != 0.0 { dx } else { dy } } else { dy };
            if d != 0.0 && self.scroll_by(i, d) { self.scrolled(); return; }
            at = self.nodes[i].parent;
        }
    }

    /// A key press; the flag is Shift. Returns whether anything used it.
    pub fn key(&mut self, k: Key, shift: bool) -> bool { self.key_mods(k, Mods { shift, ..Mods::default() }) }
    /// A key press with its modifiers. A focused text field takes it first: characters as text, Up /
    /// Down / Home / End in a multi-line field as moves by the layout, anything else as a key (Cmd+A,
    /// Cmd+Z…). Returns whether anything used it.
    pub fn key_mods(&mut self, k: Key, mods: Mods) -> bool {
        let shift = mods.shift;
        self.dirty = true;
        self.heard();
        self.keyboard = true;
        if k == Key::Tab { self.cycle_focus(shift); return true; }
        if let Some(fi) = self.field_at() && k != Key::Escape {
            let ev = match k {
                Key::Char(c) if !mods.cmd && !mods.alt => Some(TextInput::Insert(c.to_string())),
                Key::Up | Key::Down | Key::Home | Key::End if self.nodes[fi].multiline && !mods.cmd => self.line_nav(fi, k).map(|at| TextInput::Goto { at, extend: shift }),
                _ => Some(TextInput::Key { key: k, mods }),
            };
            if let Some(ev) = ev && self.send_text(ev) { return true; }
        }
        // Shortcuts with Cmd are only for text fields; the rest belong to the system.
        if mods.cmd { return false; }
        let lo = self.modal_at.unwrap_or(0);
        let fi = self.focus.as_deref().and_then(|f| self.nodes.iter().position(|n| n.key == f)).filter(|&i| i >= lo);
        if let Some(fi) = fi && let Some(m) = self.nodes[fi].on_key.clone().and_then(|f| f(k, shift)) {
            let key = self.nodes[fi].key.clone();
            if self.up(fi, |n| n.commit).is_some() {
                let (cx, cy) = self.nodes[fi].rect.center();
                self.snapshot(cx, cy);
                self.field.front(cx * self.scale, cy * self.scale, self.now, 1.0);
            }
            self.send(m);
            self.reveal_marked(&key);
            return true;
        }
        if k == Key::Escape && let Some(m) = self.modal_at.and_then(|i| self.nodes[i].modal.as_ref()).map(|m| m.dismiss.clone()) {
            self.send(m);
            return true;
        }
        let Some(fi) = fi else { return self.key_scroll(None, k) || self.unused(k, shift) };
        if matches!(k, Key::Enter | Key::Char(' ')) && let Some(m) = self.nodes[fi].on_press.clone() {
            let (cx, cy) = self.nodes[fi].rect.center();
            let s = self.scale;
            if self.up(fi, |n| n.commit).is_some() { self.snapshot(cx, cy); }
            self.field.front(cx * s, cy * s, self.now, 1.0);
            let key = self.nodes[fi].key.clone();
            self.send(m);
            self.rebirth(&key);
            return true;
        }
        self.key_scroll(Some(fi), k) || self.unused(k, shift)
    }

    /// A key nothing on the page used goes to the state, if it asked for them.
    fn unused(&mut self, k: Key, shift: bool) -> bool {
        match self.unused_key {
            Some(f) => { let used = f(&mut self.state, k, shift); if used { self.relayout(); } used }
            None => false,
        }
    }

    // ── Accessibility mirror (DESIGN.md §1.14) ───────────────────────────────────────────────────
    /// The mirror of the current tree: every element with a role (set by the widget or `El::role`), every
    /// other pressable control (a button), and text that is not part of a control's name. While a modal
    /// layer is open, only what is in it.
    pub fn a11y(&self) -> Vec<A11yNode> {
        let leafy = |r: Role| !matches!(r, Role::Dialog | Role::List | Role::TabList | Role::Group);
        let lo = self.modal_at.unwrap_or(0);
        let mut out: Vec<A11yNode> = vec![];
        let mut index: Vec<Option<usize>> = vec![None; self.nodes.len()];
        // The nearest ancestor in the mirror, and whether a control above already speaks for this text.
        let mut above: Vec<(Option<usize>, bool)> = vec![(None, false); self.nodes.len()];
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            let (up, spoken) = match n.parent { Some(p) => (index[p].or(above[p].0), above[p].1 || index[p].is_some_and(|k| leafy(out[k].role))), None => (None, false) };
            above[i] = (up, spoken);
            if n.hidden || i < lo && self.modal_at.is_some() { continue; }
            let a = n.access.clone().unwrap_or_default();
            let role = a.role.or_else(|| match &n.content {
                Content::Text { .. } if !spoken => Some(Role::Text),
                Content::Picture { .. } => Some(Role::Img),
                Content::None if n.on_press.is_some() && n.focusable && !spoken => Some(Role::Button),
                _ => None,
            });
            let Some(role) = role else { continue };
            if spoken && role == Role::Text { continue; }
            let name = a.label.clone().unwrap_or_else(|| self.name_within(i));
            if role == Role::Text && name.trim().is_empty() { continue; }
            index[i] = Some(out.len());
            out.push(A11yNode {
                key: n.key.clone(), parent: up.map(|k| out[k].key.clone()), role, name, value: a.value, text: a.text, checked: a.checked, selected: a.selected,
                level: a.level, modal: n.modal.is_some() || (role == Role::Dialog && self.modal_at.is_some()), disabled: n.disabled, focusable: n.focusable, rect: n.rect,
            });
        }
        out
    }
    /// The text inside node `i`, joined: a control's name when it has no label.
    fn name_within(&self, i: usize) -> String {
        let mut s = String::new();
        for j in i..self.nodes.len() {
            if !within(&self.nodes, j, i) { if j > i { break; } continue; }
            if let Some(t) = self.nodes[j].text() { if !s.is_empty() { s.push(' '); } s.push_str(t.trim()); }
            if s.len() > 160 { break; }
        }
        s
    }
    /// What changed in the mirror since the last call (the same keyed comparison as layout's): elements
    /// added or changed, elements gone, and where focus is now if it moved.
    pub fn a11y_diff(&mut self) -> Vec<A11yOp> {
        let now = self.a11y();
        let mut ops = vec![];
        let keys: HashSet<&str> = now.iter().map(|n| n.key.as_str()).collect();
        for k in self.a11y_sent.keys() { if !keys.contains(k.as_str()) { ops.push(A11yOp::Remove(k.clone())); } }
        let mut sent = HashMap::with_capacity(now.len());
        for n in now {
            if self.a11y_sent.get(&n.key) != Some(&n) { ops.push(A11yOp::Set(n.clone())); }
            sent.insert(n.key.clone(), n);
        }
        self.a11y_sent = sent;
        if self.a11y_focus != self.focus { self.a11y_focus = self.focus.clone(); ops.push(A11yOp::Focus(self.focus.clone())); }
        ops
    }
    /// Assistive technology moved focus to the element `key`.
    pub fn focus_key(&mut self, key: &str) {
        self.dirty = true;
        if self.node(key).is_some_and(|n| n.focusable) { self.keyboard = true; self.set_focus(Some(key.to_owned())); }
    }
    /// Assistive technology activated the element `key` (a click on its mirror): the same message as a
    /// press or Enter, with the same front and commit. Returns whether it did anything.
    pub fn activate(&mut self, key: &str) -> bool {
        self.dirty = true;
        self.heard();
        let Some(i) = self.nodes.iter().position(|n| n.key == key) else { return false };
        if self.nodes[i].focusable { self.set_focus(Some(key.to_owned())); }
        let Some(i) = self.nodes.iter().position(|n| n.key == key) else { return false };
        let Some(m) = self.nodes[i].on_press.clone() else { return false };
        let (cx, cy) = self.nodes[i].rect.center();
        if self.up(i, |n| n.commit).is_some() { self.snapshot(cx, cy); }
        let s = self.scale;
        self.field.front(cx * s, cy * s, self.now, 1.0);
        self.send(m);
        self.rebirth(key);
        true
    }

    // ── Text (DESIGN.md §1.8.1) ──────────────────────────────────────────────────────────────────
    /// The focused text field's node, if focus is on one.
    fn field_at(&self) -> Option<usize> {
        let f = self.focus.as_deref()?;
        let i = self.nodes.iter().position(|n| n.key == f)?;
        self.nodes[i].on_text.is_some().then_some(i)
    }
    /// The text a field shows: its first text descendant.
    fn leaf_of(&self, field: usize) -> Option<usize> {
        (field..self.nodes.len()).find(|&j| within(&self.nodes, j, field) && matches!(self.nodes[j].content, Content::Text { .. }))
    }
    /// Whether focus is on a text field: the host then sends typed text, the input method and the
    /// clipboard here (and keeps its hidden text element focused, on the web).
    pub fn text_focused(&self) -> bool { self.field_at().is_some() }
    /// Give the focused field a text event; returns whether it used it. Afterwards its caret is scrolled
    /// into view.
    fn send_text(&mut self, ev: TextInput) -> bool {
        let Some(fi) = self.field_at() else { return false };
        let Some(m) = self.nodes[fi].on_text.clone().and_then(|f| f(ev)) else { return false };
        self.send(m);
        self.reveal_caret();
        true
    }
    /// Text typed (or committed by an input method, or dropped in), for the focused field.
    pub fn text_input(&mut self, s: &str) -> bool {
        self.dirty = true;
        self.heard();
        self.keyboard = true;
        self.send_text(TextInput::Insert(s.into()))
    }
    /// An input method's composition so far (`cursor`: a byte range in it); empty text cancels it.
    pub fn ime_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) -> bool {
        self.dirty = true;
        self.keyboard = true;
        self.send_text(TextInput::Compose { text: text.into(), cursor })
    }
    /// An input method committed `text` (empty: the composition ended with nothing).
    pub fn ime_commit(&mut self, text: &str) -> bool {
        self.dirty = true;
        if text.is_empty() { self.send_text(TextInput::Compose { text: String::new(), cursor: None }) } else { self.text_input(text) }
    }
    /// The clipboard's text, pasted into the focused field.
    pub fn paste(&mut self, s: &str) -> bool { self.dirty = true; self.heard(); self.send_text(TextInput::Paste(s.into())) }
    /// What a copy takes: the focused field's selection, or else the read-only selection (its pieces in
    /// separate text elements joined by line breaks).
    pub fn copy(&self) -> Option<String> {
        if let Some(fi) = self.field_at() {
            let leaf = self.leaf_of(fi)?;
            let Content::Text { s, select: Some((a, b)), .. } = &self.nodes[leaf].content else { return None };
            return s.get(*a..*b).map(str::to_owned);
        }
        let mut pieces: Vec<(usize, (usize, usize))> = self.sel_ranges().into_iter().collect();
        if pieces.is_empty() { return None; }
        pieces.sort_by_key(|p| p.0);
        Some(pieces.iter().filter_map(|(i, (a, b))| self.nodes[*i].text().and_then(|s| s.get(*a..*b))).collect::<Vec<_>>().join("\n"))
    }
    /// Copy, and remove the selection from the focused field.
    pub fn cut(&mut self) -> Option<String> {
        let c = self.copy()?;
        if self.field_at().is_some() { self.dirty = true; self.send_text(TextInput::Cut); }
        Some(c)
    }
    /// Where the focused field's caret is (logical px): the host puts its hidden text element, and so an
    /// input method's candidate window, there.
    pub fn ime_rect(&mut self) -> Option<Rect> {
        let leaf = self.field_at().and_then(|f| self.leaf_of(f))?;
        let n = &self.nodes[leaf];
        let Content::Text { caret, .. } = &n.content else { return None };
        let r = self.text.caret(n.para()?, n.origin(), caret.unwrap_or(0))?;
        Some(Rect::new(r.x, r.y, 2.0, r.h).intersect(&n.clip.inset(-2.0, -2.0, -2.0, -2.0)))
    }
    /// The pointer's shape where it is: an element's own (`El::cursor`), else text over a text field
    /// or selectable text, a pointing hand over what can be pressed, grab over what can be dragged
    /// (grabbing while it is).
    pub fn cursor(&self) -> Cursor {
        if self.text_capture.is_some() || self.sel_drag { return Cursor::Text; }
        if let Some(c) = &self.capture {
            return self.nodes.iter().find(|n| n.key == c.key).and_then(|n| n.cursor).filter(|c| *c != Cursor::Grab).unwrap_or(Cursor::Grabbing);
        }
        if self.field.pointer[2] <= 0.0 { return Cursor::Default; }
        let (x, y) = (self.field.pointer[0] / self.scale, self.field.pointer[1] / self.scale);
        let mut at = self.hit_target(x, y);
        while let Some(i) = at {
            let n = &self.nodes[i];
            if let Some(c) = n.cursor { return c; }
            if n.on_text.is_some() { return Cursor::Text; }
            if n.on_drag.is_some() { return Cursor::Grab; }
            if n.on_press.is_some() { return Cursor::Pointer; }
            if n.selectable && n.text().is_some() { return Cursor::Text; }
            at = n.parent;
        }
        Cursor::Default
    }
    /// The read-only selection now: the byte range selected in each selectable text node it spans.
    fn sel_ranges(&self) -> HashMap<usize, (usize, usize)> {
        let mut out = HashMap::new();
        let Some(((ka, a), (kb, b))) = &self.sel else { return out };
        let pos = |k: &str| self.nodes.iter().position(|n| n.key == k);
        let (Some(ia), Some(ib)) = (pos(ka), pos(kb)) else { return out };
        let ((i0, b0), (i1, b1)) = if (ia, *a) <= (ib, *b) { ((ia, *a), (ib, *b)) } else { ((ib, *b), (ia, *a)) };
        for i in i0..=i1 {
            if !self.nodes[i].selectable { continue; }
            let Some(s) = self.nodes[i].text() else { continue };
            let (lo, hi) = (if i == i0 { b0 } else { 0 }, if i == i1 { b1.min(s.len()) } else { s.len() });
            if lo < hi { out.insert(i, (lo, hi)); }
        }
        out
    }
    /// The read-only selection's text pieces, for tests.
    pub fn selection(&self) -> Vec<(String, usize, usize)> {
        let mut v: Vec<_> = self.sel_ranges().into_iter().map(|(i, (a, b))| (self.nodes[i].key.clone(), a, b)).collect();
        v.sort();
        v
    }
    /// The selectable text node under (x, y), or the nearest visible one (by line, then along it).
    fn sel_target(&self, x: f32, y: f32) -> Option<usize> {
        let seen = |i: usize| { let n = &self.nodes[i]; let r = n.rect.intersect(&n.clip); r.w > 0.0 && r.h > 0.0 };
        let cands: Vec<usize> = (0..self.nodes.len()).filter(|&i| self.nodes[i].selectable && self.nodes[i].text().is_some() && !self.nodes[i].hidden && seen(i)).collect();
        if let Some(&i) = cands.iter().find(|&&i| self.nodes[i].rect.contains(x, y)) { return Some(i); }
        let dist = |i: usize| { let r = self.nodes[i].rect; let dy = (r.y - y).max(y - r.bottom()).max(0.0); let dx = (r.x - x).max(x - r.right()).max(0.0); (dy, dx) };
        cands.into_iter().min_by(|&a, &b| dist(a).partial_cmp(&dist(b)).unwrap_or(std::cmp::Ordering::Equal))
    }
    /// The byte under (x, y) in node `i`'s text.
    fn text_hit(&mut self, i: usize, x: f32, y: f32) -> Option<usize> {
        let n = &self.nodes[i];
        Some(self.text.hit(n.para()?, n.origin(), x, y))
    }
    /// The caret of field `fi` moved by a line or to a line's end (multi-line fields).
    fn line_nav(&mut self, fi: usize, k: Key) -> Option<usize> {
        let leaf = self.leaf_of(fi)?;
        let n = &self.nodes[leaf];
        let Content::Text { caret, .. } = &n.content else { return None };
        let to = match k { Key::Up => Nav::Up, Key::Down => Nav::Down, Key::Home => Nav::LineStart, _ => Nav::LineEnd };
        Some(self.text.nav(n.para()?, caret.unwrap_or(0), to))
    }
    /// Scroll the focused field (and anything around it) so its caret is visible.
    fn reveal_caret(&mut self) {
        for _ in 0..4 {
            let Some(leaf) = self.field_at().and_then(|f| self.leaf_of(f)) else { return };
            let n = &self.nodes[leaf];
            let Some(r) = (match &n.content { Content::Text { caret: Some(c), .. } => n.para().and_then(|p| self.text.caret(p, n.origin(), *c)), _ => None }) else { return };
            let mut a = self.nodes[leaf].parent;
            let mut moved = false;
            while let Some(p) = a.and_then(|p| self.up(p, |n| n.scroll)) {
                let view = self.nodes[p].clip.intersect(&self.nodes[p].inner);
                let (lo, hi, vlo, vhi) = if self.nodes[p].horizontal { (r.x - 2.0, r.x + 3.0, view.x, view.right()) } else { (r.y, r.bottom(), view.y, view.bottom()) };
                let d = if lo < vlo { lo - vlo } else if hi > vhi { hi - vhi } else { 0.0 };
                if d != 0.0 && self.scroll_by(p, d) { moved = true; break; }
                a = self.nodes[p].parent;
            }
            if !moved { return; }
            self.scrolled();
        }
    }

    // ── Internals ────────────────────────────────────────────────────────────────────────────────
    fn send(&mut self, m: M) {
        let sp = self.span_begin();
        (self.update)(&mut self.state, m);
        self.relayout();
        self.span_end(sp);
    }

    /// What a press at (x, y) acts on: the topmost node there when it (or an ancestor) can be acted on;
    /// otherwise the control whose box, grown to at least `MIN_HIT` each way about its centre, holds the
    /// point (the nearest one), so small controls are easy to hit without looking bigger.
    fn hit_target(&self, x: f32, y: f32) -> Option<usize> {
        let h = self.hit(x, y);
        if h.and_then(|i| self.up(i, Node::active)).is_some() { return h; }
        let grown = |r: Rect| { let (gw, gh) = ((MIN_HIT - r.w).max(0.0) * 0.5, (MIN_HIT - r.h).max(0.0) * 0.5); r.inset(-gw, -gh, -gw, -gh) };
        let d = |r: Rect| { let (cx, cy) = r.center(); (cx - x).powi(2) + (cy - y).powi(2) };
        (self.modal_at.unwrap_or(0)..self.nodes.len()).rev()
            .filter(|&i| { let n = &self.nodes[i]; n.active() && !n.hidden && (n.rect.w < MIN_HIT || n.rect.h < MIN_HIT) && n.clip.contains(x, y) && grown(n.rect).contains(x, y) })
            .min_by(|&a, &b| d(self.nodes[a].rect).total_cmp(&d(self.nodes[b].rect)))
            .or(h)
    }

    /// Topmost node under a point, honouring clips. While a modal layer is open only it can be hit.
    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        (self.modal_at.unwrap_or(0)..self.nodes.len()).rev().find(|&i| !self.nodes[i].hidden && self.nodes[i].rect.contains(x, y) && self.nodes[i].clip.contains(x, y))
    }

    /// Keep the frame as it is now for the renderer, with the commit's origin (logical px).
    fn snapshot(&mut self, x: f32, y: f32) {
        self.splits += 1;
        let s = self.scale;
        self.split = Some(Split { id: self.splits, scene: self.scene.clone(), field: self.field, at: [x * s, y * s], t: self.now });
    }

    /// The node itself or its nearest ancestor that satisfies `f`.
    fn up(&self, mut i: usize, f: impl Fn(&Node<M>) -> bool) -> Option<usize> {
        loop {
            if f(&self.nodes[i]) { return Some(i); }
            i = self.nodes[i].parent?;
        }
    }

    fn within(&self, i: usize, root: usize) -> bool { within(&self.nodes, i, root) }

    fn set_focus(&mut self, to: Option<String>) {
        if to == self.focus { return; }
        let old = std::mem::replace(&mut self.focus, to);
        let blur = old.and_then(|k| self.node(&k).and_then(|n| n.on_blur.clone()));
        if let Some(m) = blur { self.send(m); }
        self.reveal();
        // The caret is drawn only where focus is.
        self.paint();
    }

    fn cycle_focus(&mut self, back: bool) {
        // While a modal layer is open, focus goes round inside it only.
        let order: Vec<usize> = (self.modal_at.unwrap_or(0)..self.nodes.len()).filter(|&i| self.nodes[i].focusable).collect();
        if order.is_empty() { return; }
        let cur = self.focus.as_deref().and_then(|f| order.iter().position(|&i| self.nodes[i].key == f));
        let n = order.len();
        let next = match (cur, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(c), false) => (c + 1) % n,
            (Some(c), true) => (c + n - 1) % n,
        };
        self.set_focus(Some(self.nodes[order[next]].key.clone()));
    }

    /// Scroll so the focused node is visible.
    fn reveal(&mut self) {
        if let Some(key) = self.focus.clone() { self.reveal_key(&key); }
    }

    /// After a key changed the state through `key`'s handler: show the elements inside it that ask to
    /// be seen (a newly selected row).
    fn reveal_marked(&mut self, key: &str) {
        let Some(root) = self.nodes.iter().position(|n| n.key == key) else { return };
        // A virtual list's selected row may not be built: scroll by its index.
        if let Some((_, rh, Some(sel))) = self.nodes[root].virt {
            let n = &self.nodes[root];
            let view = n.rect.h - (n.inner.y - n.rect.y) * 2.0;
            let off = self.scroll.get(&n.key).copied().unwrap_or(0.0);
            let (top, bottom) = (sel as f32 * rh, (sel + 1) as f32 * rh);
            let to = if top < off { top } else if bottom > off + view { bottom - view } else { off };
            if to != off && self.scroll_by(root, to - off) { self.scrolled(); }
            return;
        }
        let marked: Vec<String> = (root..self.nodes.len()).filter(|&i| self.nodes[i].reveal && self.within(i, root)).map(|i| self.nodes[i].key.clone()).collect();
        for k in marked { self.reveal_key(&k); }
    }

    /// Scroll every scroll ancestor of a node so that it is inside, innermost first.
    fn reveal_key(&mut self, key: &str) {
        let mut guard = 0;
        loop {
            guard += 1;
            let Some(i) = self.nodes.iter().position(|n| n.key == key) else { return };
            let mut a = self.nodes[i].parent;
            let mut moved = false;
            while let Some(p) = a.and_then(|p| self.up(p, |n| n.scroll)) {
                let (r, view) = (self.nodes[i].rect, self.nodes[p].clip.intersect(&self.nodes[p].inner));
                let (lo, hi, vlo, vhi) = if self.nodes[p].horizontal { (r.x, r.right(), view.x, view.right()) } else { (r.y, r.bottom(), view.y, view.bottom()) };
                let d = if lo < vlo { lo - vlo - 8.0 } else if hi > vhi { (hi - vhi + 8.0).min(lo - vlo) } else { 0.0 };
                if d != 0.0 && self.scroll_by(p, d) { moved = true; break; }
                a = self.nodes[p].parent;
            }
            if !moved || guard > 4 { break; }
            self.scrolled();
        }
    }

    fn key_scroll(&mut self, from: Option<usize>, k: Key) -> bool {
        let dy = match k { Key::Up => -40.0, Key::Down => 40.0, Key::PageUp => -self.size.1 * 0.8, Key::PageDown => self.size.1 * 0.8, _ => return false };
        let start = from.or_else(|| (!self.nodes.is_empty()).then_some(0));
        let Some(i) = start.and_then(|i| self.up(i, |n| n.scroll && !n.horizontal).or_else(|| self.nodes.iter().position(|n| n.scroll && !n.horizontal))) else { return false };
        if self.scroll_by(i, dy) { self.scrolled(); }
        true
    }

    /// Returns whether the offset changed.
    fn scroll_by(&mut self, i: usize, dy: f32) -> bool {
        let n = &self.nodes[i];
        let max = (n.extent - if n.horizontal { n.rect.w } else { n.rect.h }).max(0.0);
        let cur = self.scroll.get(&n.key).copied().unwrap_or(0.0);
        let next = (cur + dy).clamp(0.0, max);
        if next == cur { return false; }
        self.scroll.insert(n.key.clone(), next);
        true
    }

    /// Give every instance of a node and its descendants a fresh birth time.
    fn rebirth(&mut self, key: &str) {
        let Some(root) = self.nodes.iter().position(|n| n.key == key) else { return };
        for &(owner, id) in &self.owners {
            if self.within(owner, root) { self.births.insert(id, self.now); }
        }
        self.paint();
    }

    /// View → taffy tree → absolute rects → scene.
    pub fn relayout(&mut self) {
        let sp = self.span_begin();
        self.relayout_inner();
        self.span_end(sp);
    }
    fn relayout_inner(&mut self) {
        // Commands are taken first: a flow needs the frame as it is before the new layout.
        let cmds = self.commands.map(|f| f(&mut self.state)).unwrap_or_default();
        let flow = cmds.iter().find_map(|c| if let Cmd::Flow(f) = c { Some(*f) } else { None });
        let old = flow.map(|_| self.record());
        self.layout();
        if let (Some(f), Some(old)) = (flow, old) { self.flow_from(old, f); }
        for c in cmds {
            match c {
                Cmd::Morph { from, to, spec } => self.morph(&from, &to, spec),
                Cmd::Front { x, y, strength } => { let s = self.scale; self.field.front(x * s, y * s, self.now, strength); }
                Cmd::FrontAt { key, strength } => if let Some(n) = self.nodes.iter().find(|n| n.key == key) {
                    let ((x, y), s) = (n.rect.center(), self.scale);
                    self.field.front(x * s, y * s, self.now, strength);
                },
                Cmd::Flow(_) => {}
            }
        }
    }

    /// View → nodes → the taffy tree, kept from the last layout and changed only where the new view
    /// differs (DESIGN.md §1.13) → absolute rects → scene.
    fn layout(&mut self) {
        if let Some(f) = self.look_of { self.timing = Timing::of(&f(&self.state)); }
        self.perf.counters.views += 1;
        let el = (self.view)(&self.state);
        let mut nodes = Vec::new();
        // The root is the window, whatever its contents would like.
        let mut el = el;
        el.style.size = Size { width: length(self.size.0), height: length(self.size.1) };
        // Modal layers are lifted out of wherever they were declared and laid over the whole window, last
        // so that they are drawn on top.
        let mut layers = Vec::new();
        hoist(&mut el, &mut layers);
        for mut l in layers {
            l.style.position = taffy::Position::Absolute;
            l.style.inset = taffy::Rect { left: length(0.0), right: length(0.0), top: length(0.0), bottom: length(0.0) };
            l.style.size = Size { width: length(self.size.0), height: length(self.size.1) };
            el.children.push(l);
        }
        // Virtual lists build the rows their viewport shows, at their scroll offset (the viewport as it
        // was last laid out; the window's height the first time).
        let mut vx = VirtCtx { scroll: &self.scroll, views: &self.virt_views, window: self.size.1, made: HashMap::new() };
        let mut built = Built::default();
        flatten(el, None, "root".into(), [0; 3], (false, false), None, Inherit::default(), &mut vx, &mut built, &mut nodes);
        self.virt_made = vx.made;

        // The taffy tree: a node whose key was there last time keeps its taffy node; its style, children
        // and measured content are compared and only what differs is set (which marks it and its
        // ancestors dirty). Children come after their parents in `nodes`, so going backwards every
        // node's children have their taffy nodes already.
        let mut changed = false;
        let mut used: HashSet<NodeId> = HashSet::with_capacity(nodes.len());
        let mut tids: HashMap<String, NodeId> = HashMap::with_capacity(nodes.len());
        for i in (0..nodes.len()).rev() {
            let leaf = !matches!(nodes[i].content, Content::None);
            let style = std::mem::take(&mut built.styles[i]);
            let mk = measure_key(&nodes[i].content);
            let kids: Vec<NodeId> = built.kids[i].iter().map(|&c| nodes[c].tid).collect();
            let old = self.tids.get(&nodes[i].key).copied().filter(|t| !used.contains(t));
            let tid = match old {
                Some(t) => {
                    if self.tree.style(t).map_or(true, |o| *o != style) { let _ = self.tree.set_style(t, style); changed = true; }
                    if leaf {
                        if self.tree.child_count(t) > 0 { let _ = self.tree.set_children(t, &[]); changed = true; }
                        let _ = self.tree.set_node_context(t, Some(i));
                        if self.mkeys.get(&t) != Some(&mk) { let _ = self.tree.mark_dirty(t); changed = true; }
                    } else {
                        let _ = self.tree.set_node_context(t, None);
                        if self.tree.children(t).map_or(true, |c| c != kids) { let _ = self.tree.set_children(t, &kids); changed = true; }
                    }
                    t
                }
                None => {
                    changed = true;
                    if leaf { self.tree.new_leaf_with_context(style, i) } else { self.tree.new_with_children(style, &kids) }.expect("taffy node")
                }
            };
            used.insert(tid);
            self.mkeys.insert(tid, mk);
            nodes[i].tid = tid;
            tids.insert(nodes[i].key.clone(), tid);
        }
        // What is gone from the view goes from the tree.
        for (_, t) in self.tids.drain() {
            if !used.contains(&t) { let _ = self.tree.remove(t); self.mkeys.remove(&t); changed = true; }
        }
        self.tids = tids;
        let root = nodes[0].tid;
        // Nothing changed that layout depends on (taffy's own dirty flag is not kept for the root): the
        // boxes from last time stand.
        if changed {
            let text = &mut self.text;
            let measure = |input: taffy::LayoutInput, _: NodeId, ctx: Option<&mut usize>, style: &Style| {
                let none = |_: *const (), _: f32| 0.0;
                match ctx.map(|i| &nodes[*i].content) {
                    Some(Content::Text { s, size, line, font, around, nowrap, .. }) => taffy::compute_leaf_layout(input, style, none, |known, avail| {
                        let limit = known.width.or(match avail.width { AvailableSpace::Definite(w) => Some(w), AvailableSpace::MinContent => Some(0.0), AvailableSpace::MaxContent => None });
                        // Text flowing around obstacles fills whatever width it is given; text kept on one
                        // line is as wide as it is.
                        let limit = if around.is_some() { limit.filter(|w| *w > 0.0).or(Some(600.0)) } else if *nowrap { None } else { limit };
                        let (w, h) = text.measure_around(s, font.resolve([0; 3]), *size, *line, limit, around.as_deref());
                        Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
                    }),
                    Some(Content::Picture { ratio, .. }) => taffy::compute_leaf_layout(input, style, none, |known, avail| {
                        let w = known.width.or(avail.width.into_option()).unwrap_or(0.0);
                        Size { width: w, height: known.height.unwrap_or((w * ratio).round()) }
                    }),
                    _ => taffy::compute_leaf_layout(input, style, none, |_, _| Size::ZERO),
                }
            };
            let space = Size { width: AvailableSpace::Definite(self.size.0), height: AvailableSpace::Definite(self.size.1) };
            self.perf.counters.layouts += 1;
            if self.tree.compute_layout_with_measure(root, space, measure).is_err() { return; }
        } else {
            self.perf.counters.layouts_skipped += 1;
        }
        for n in nodes.iter_mut() { n.local = *self.tree.layout(n.tid).expect("node in tree"); }
        self.nodes = nodes;
        self.place();
        self.modal_at = self.nodes.iter().rposition(|n| n.modal.is_some());
        self.layer_changed();
        self.paint();
    }

    /// Absolute rects, clips and scroll extents from the laid-out (relative) boxes and the scroll
    /// offsets: all that scrolling changes. No view, no layout, no shaping.
    fn place(&mut self) {
        let nodes = &mut self.nodes;
        // How long each scroll node's contents are, so stored offsets can be clamped before placing.
        for n in nodes.iter_mut() { n.extent = 0.0; }
        for i in 0..nodes.len() {
            let l = nodes[i].local;
            if let Some(p) = nodes[i].parent && nodes[p].scroll {
                let pl = nodes[p].local;
                let end = if nodes[p].horizontal { l.location.x + l.size.width + pl.padding.right + pl.border.right }
                          else { l.location.y + l.size.height + pl.padding.bottom + pl.border.bottom };
                nodes[p].extent = nodes[p].extent.max(end);
            }
        }
        for n in nodes.iter().filter(|n| n.scroll) {
            let size = if n.horizontal { n.local.size.width } else { n.local.size.height };
            let max = (n.extent - size).max(0.0);
            if let Some(o) = self.scroll.get_mut(&n.key) { *o = o.clamp(0.0, max); }
        }
        let screen = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        let mut inside = vec![screen; nodes.len()];
        for i in 0..nodes.len() {
            let l = nodes[i].local;
            let (origin, clip) = match nodes[i].parent {
                Some(p) => {
                    let off = if nodes[p].scroll { self.scroll.get(&nodes[p].key).copied().unwrap_or(0.0) } else { 0.0 };
                    let (ox, oy) = if nodes[p].horizontal { (off, 0.0) } else { (0.0, off) };
                    ((nodes[p].rect.x - ox, nodes[p].rect.y - oy), inside[p])
                }
                None => ((0.0, 0.0), screen),
            };
            // An anchored element goes just below its anchor (laid out earlier in this pass), kept
            // inside the window.
            let at = nodes[i].anchor.as_ref().and_then(|k| nodes[..i].iter().find(|n| &n.key == k)).map(|a| a.rect);
            let n = &mut nodes[i];
            n.rect = Rect::new(origin.0 + l.location.x, origin.1 + l.location.y, l.size.width, l.size.height);
            if let Some(a) = at {
                n.rect.x = a.x.min(screen.w - l.size.width - 8.0).max(0.0);
                n.rect.y = a.bottom() + 4.0;
            }
            n.clip = clip;
            let (b, p) = (l.border, l.padding);
            let inner_border = n.rect.inset(b.left, b.top, b.right, b.bottom);
            n.inner = inner_border.inset(p.left, p.top, p.right, p.bottom);
            inside[i] = if n.scroll { clip.intersect(&inner_border) } else if n.clips { clip.intersect(&n.rect) } else { clip };
            if n.virt.is_some() { self.virt_views.insert(n.key.clone(), n.rect.h); }
        }
    }

    /// After a scroll offset changed: move everything (no layout) — unless a virtual list scrolled past
    /// the rows it has built, which then builds the rows it now shows.
    fn scrolled(&mut self) {
        let past = self.nodes.iter().any(|n| {
            let (Some(v), Some(&(a, b))) = (&n.virt, self.virt_made.get(&n.key)) else { return false };
            let off = self.scroll.get(&n.key).copied().unwrap_or(0.0);
            let (first, last) = visible_rows(off, n.rect.h, v.0, v.1);
            first < a || last > b
        });
        if past { self.relayout(); } else { self.place(); self.paint(); }
    }

    /// A modal layer opened or closed: focus moves into it (remembering where it was), or back.
    fn layer_changed(&mut self) {
        let key = self.modal_at.map(|m| self.nodes[m].key.clone());
        if key == self.modal_key { return; }
        match self.modal_at {
            Some(m) => {
                if self.modal_key.is_none() { self.return_focus = self.focus.clone(); }
                // The first field that takes keys (a text field), else the first control.
                let first = (m..self.nodes.len()).find(|&i| self.nodes[i].focusable && (self.nodes[i].on_key.is_some() || self.nodes[i].on_text.is_some()))
                    .or_else(|| (m..self.nodes.len()).find(|&i| self.nodes[i].focusable));
                self.focus = first.map(|i| self.nodes[i].key.clone());
            }
            None => {
                let back = self.return_focus.take();
                self.focus = back.filter(|k| self.nodes.iter().any(|n| &n.key == k && n.focusable));
            }
        }
        self.modal_key = key;
    }

    /// Nodes → instances. Identity is key + part (+ glyph id), so unchanged things keep their birth time.
    fn paint(&mut self) {
        let t0 = crate::perf::now_ms();
        self.painting = true;
        self.paint_inner();
        self.painting = false;
        self.perf.counters.scenes += 1;
        self.perf.pending.scene += (crate::perf::now_ms() - t0) as f32;
    }
    fn paint_inner(&mut self) {
        self.scene.clear();
        self.owners.clear();
        self.tags.clear();
        self.unseen.clear();
        let (s, now) = (self.scale, self.now);
        let dev = |r: Rect| {
            let (x0, y0, x1, y1) = ((r.x * s).round(), (r.y * s).round(), (r.right() * s).round(), (r.bottom() * s).round());
            [x0, y0, x1 - x0, y1 - y0]
        };
        let mut births = HashMap::with_capacity(self.births.len());
        self.field.wipe = [0.0; 4];
        self.field.calm = [[0.0; 4]; 4];
        let mut calms = 0;
        let mut held = vec![false; self.nodes.len()];
        // Everything beneath an open modal layer is held partly loose.
        let beneath = self.modal_at.and_then(|m| self.nodes[m].modal.as_ref()).map_or(0.0, |m| m.beneath);
        let caret_at = self.focus.as_deref().and_then(|f| self.nodes.iter().position(|n| n.key == f));
        // A morph decides whether its two elements are drawn, whatever the view says.
        let roots = self.run.as_ref().map(|r| (self.nodes.iter().position(|n| n.key == r.from), self.nodes.iter().position(|n| n.key == r.to)));
        let nodes = &self.nodes;
        self.roles = (0..nodes.len()).map(|i| match roots {
            Some((a, _)) if a.is_some_and(|a| within(nodes, i, a)) => 1,
            Some((_, b)) if b.is_some_and(|b| within(nodes, i, b)) => 2,
            _ => 0,
        }).collect();
        let mut counters: HashMap<usize, [u32; 2]> = HashMap::new();
        let (mut latest, mut any_loose) = (f32::NEG_INFINITY, false);
        let global = self.timing.material;
        let mut caret_found: Option<(usize, [f32; 4])> = None;
        let ro = self.sel_ranges();
        // A scroll node pulled past its end: what is near that edge loosens, the more the further it is
        // pulled (no box moves past the end).
        let mut pull_of: Vec<Option<(usize, f32)>> = vec![None; self.nodes.len()];
        for i in 0..self.nodes.len() {
            if let Some(p) = self.nodes[i].parent {
                pull_of[i] = match self.over.get(&self.nodes[p].key) { Some(&o) if self.nodes[p].scroll => Some((p, o)), _ => pull_of[p] };
            }
        }
        let max_over = self.scrolling.max_over;
        let pulled = |nodes: &[Node<M>], i: usize| pull_of[i].map_or(0.0, |(p, o)| {
            let (r, inner, h) = (nodes[i].rect, nodes[p].inner, nodes[p].horizontal);
            let dist = match (h, o < 0.0) {
                (false, true) => r.y - inner.y, (false, false) => inner.bottom() - r.bottom(),
                (true, true) => r.x - inner.x, (true, false) => inner.right() - r.right(),
            };
            0.85 * (o.abs() / max_over).min(1.0).sqrt() * (1.0 - dist.max(0.0) / PULL_REACH).max(0.0)
        });
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            if Some(i) == self.modal_at { self.scene.begin_layer(); }
            held[i] = self.pressed.as_deref() == Some(n.key.as_str()) || n.parent.is_some_and(|p| held[p]);
            let seen = n.rect.intersect(&n.clip);
            if seen.w <= 0.0 || seen.h <= 0.0 { continue; }
            let role = self.roles[i];
            let hidden = match (role, &self.run) { (1, Some(r)) => now >= r.src_until, (2, Some(r)) => now < r.dst_from, _ => n.hidden };
            let c = n.clip;
            let clip = [c.x * s, c.y * s, c.right() * s, c.bottom() * s];
            if n.calm && !hidden && calms < self.field.calm.len() {
                // Its box as seen, a little larger, so a front's push cannot carry an edge across it.
                let r = seen;
                self.field.calm[calms] = [(r.x - 4.0) * s, (r.y - 4.0) * s, (r.right() + 4.0) * s, (r.bottom() + 4.0) * s];
                calms += 1;
            }
            let under = self.modal_at.is_some_and(|m| i < m);
            let loose = if held[i] { 1.0 } else if under { n.loose.max(n.loosen).max(beneath) } else { n.loose.max(n.loosen) };
            let loose = loose.max(pulled(&self.nodes, i));
            let mat = n.mat;
            let dim = if n.disabled { 0.4 } else { 1.0 };
            let mut sink = Sink { scene: &mut self.scene, owners: &mut self.owners, tags: &mut self.tags, unseen: &mut self.unseen, counters: &mut counters, latest: &mut latest, loose: &mut any_loose };
            let mut put = |sink: &mut Sink, mut inst: Instance, id: u64, glyph: Option<u32>| {
                inst.clip = clip;
                inst.paint[2] *= dim;
                inst.extra[2] = loose;
                // Hidden instances are kept for sampling only: they get no birth time, so a view that
                // shows them later lets them condense as usual.
                if hidden { inst.extra[3] = -1e4; sink.unseen.push((i, inst)); return; }
                // What a morph shows has been there all along: it must not condense on top of its ramp.
                let born = if role != 0 { *births.entry(id).or_insert(-1e4) }
                           else { *births.entry(id).or_insert_with(|| self.births.get(&id).copied().unwrap_or(now)) };
                inst.extra[3] = born;
                *sink.latest = sink.latest.max(born);
                // Loose under a material that moves loose things by itself (its own override or the global one).
                let m = if inst.look[0] >= 127.5 { (inst.look[0].round() as u32 % 16) as u8 } else { global };
                *sink.loose |= loose > 0.0 && crate::style::Material::from_id(m).time_varying();
                let tag = n.flow.as_ref().map(|(f, root)| {
                    let c = &mut sink.counters.entry(*root).or_default()[glyph.is_some() as usize];
                    let t = Tag { flow: f.clone(), glyph: glyph.is_some(), n: *c, gid: glyph.unwrap_or(0) };
                    *c += 1;
                    t
                });
                sink.scene.push(inst);
                sink.owners.push((i, id));
                sink.tags.push(tag);
            };
            let shape = |p: Paint, bw: f32| Instance { rect: dev(n.rect), uv: [n.radius * s, 0.0, 0.0, 0.0], paint: [p.tone, p.accent, p.alpha, SHAPE], clip, extra: [bw * s, 0.0, 0.0, 0.0], look: [mat, p.hue as f32, (n.rect.y * s).round(), (n.rect.h * s).round().max(1.0)] };
            if let Some(p) = n.fill { put(&mut sink, shape(p, 0.0), ident(&n.key, u32::MAX, 0), None); }
            if let Some((w, p)) = n.border { put(&mut sink, shape(p, w), ident(&n.key, u32::MAX - 1, 0), None); }
            match &n.content {
                Content::Text { line, paint, caret, weight, select, mark, .. } => {
                    let proto = Instance { paint: [paint.tone, paint.accent, paint.alpha, 0.0], look: [mat, paint.hue as f32, (n.inner.y * s).round(), (line * s).round().max(1.0)], ..Default::default() };
                    let para = n.para().expect("text");
                    let wpx = weight * s;
                    let focused = caret_at.is_some_and(|f| within(&self.nodes, i, f));
                    // Lines the clip hides are not made into instances at all.
                    let rows = Some((clip[1], clip[3]));
                    let mut glyphs: Vec<(Instance, u32)> = vec![];
                    self.text.emit_in(para, n.origin(), proto, s, rows, |mut inst, glyph| { inst.extra[0] = wpx; glyphs.push((inst, glyph)); });
                    // A selection (the field's, while it has focus, or the read-only one) is drawn by
                    // nearness: each selected glyph again, under itself, with its outline moved out, so
                    // neighbouring glyphs' halos run together and follow the letters, not a box.
                    let range = select.filter(|_| focused).or_else(|| ro.get(&i).copied());
                    if let Some((a, b)) = range {
                        let boxes = self.text.boxes(para, n.origin(), a, b);
                        for (k, (g, gid)) in glyphs.iter().enumerate() {
                            let (cx, cy) = ((g.rect[0] + g.rect[2] * 0.5) / s, (g.rect[1] + g.rect[3] * 0.5) / s);
                            if !boxes.iter().any(|r| r.inset(-1.0, -1.0, -1.0, -1.0).contains(cx, cy)) { continue; }
                            let mut halo = *g;
                            halo.extra[0] = wpx + SELECT_HALO * s;
                            halo.paint = [0.0, 1.0, 0.34, halo.paint[3]];
                            halo.look[1] = 0.0;
                            put(&mut sink, halo, ident(&n.key, (k as u32) | 0x4000_0000, *gid), None);
                        }
                    }
                    for (seq, (inst, glyph)) in glyphs.into_iter().enumerate() { put(&mut sink, inst, ident(&n.key, seq as u32, glyph), Some(glyph)); }
                    // Text being composed is underlined.
                    if let Some((a, b)) = mark {
                        for (k, r) in self.text.boxes(para, n.origin(), *a, *b).into_iter().enumerate() {
                            let bar = Instance { rect: dev(Rect::new(r.x, r.bottom() - 2.5, r.w, 1.5)), paint: [paint.tone, 0.0, 1.0, SHAPE], ..Default::default() };
                            put(&mut sink, bar, ident(&n.key, u32::MAX - 8 - k as u32, 0), None);
                        }
                    }
                    // The caret, only while the field it belongs to has focus. It keeps one identity, so
                    // moving it does not make it condense again.
                    if let Some(at) = caret.filter(|_| focused) && let Some(r) = self.text.caret(para, n.origin(), at) {
                        let bar = Instance { rect: dev(Rect::new(r.x, r.y, 1.5, r.h)), paint: [paint.tone, 1.0, 1.0, SHAPE], ..Default::default() };
                        let before = sink.scene.instances.len();
                        put(&mut sink, bar, ident(&n.key, u32::MAX - 4, 0), None);
                        if sink.scene.instances.len() > before { caret_found = Some((before, bar.rect)); }
                    }
                }
                Content::Picture { id, fx, .. } => {
                    let inst = Instance { rect: dev(n.inner), uv: [0.0, 0.0, 1.0, 1.0], paint: [1.0, *fx as f32, 1.0, PICTURE], clip, extra: [0.0, *id as f32, 0.0, 0.0], look: [mat, 0.0, 0.0, 1.0] };
                    put(&mut sink, inst, ident(&n.key, u32::MAX - 2, *id), None);
                    // The wipe is a half-plane source in the field, acting over the visible picture.
                    if let Some(x) = n.wipe && !hidden {
                        let r = n.inner.intersect(&c);
                        self.field.wipe = [(n.inner.x + x * n.inner.w) * s, 1.0, 0.0, 0.0];
                        self.field.wipe_rect = [r.x * s, r.y * s, r.right() * s, r.bottom() * s];
                    }
                }
                Content::None => {}
            }
            // Where a scroll node is: a thin bar along its far edge, when it holds more than it shows.
            if n.scroll && n.bar {
                let off = self.scroll.get(&n.key).copied().unwrap_or(0.0);
                let len = if n.horizontal { n.rect.w } else { n.rect.h };
                if n.extent > len + 0.5 {
                    let thumb = (len * len / n.extent).max(16.0);
                    let at = off / (n.extent - len) * (len - thumb);
                    let r = if n.horizontal { Rect::new(n.rect.x + at, n.rect.bottom() - 4.0, thumb, 2.0) } else { Rect::new(n.rect.right() - 4.0, n.rect.y + at, 2.0, thumb) };
                    let inst = Instance { rect: dev(r), uv: [s, 0.0, 0.0, 0.0], paint: [0.5, 0.0, 0.8, SHAPE], ..Default::default() };
                    put(&mut sink, inst, ident(&n.key, u32::MAX - 3, 0), None);
                }
            }
        }
        self.births = births;
        self.latest_birth = latest;
        self.static_loose = any_loose;
        // The caret blinks from where it last moved to: a new place starts it again, shown.
        self.caret = caret_found.map(|(i, r)| match self.caret {
            Some((_, reset, old)) if old == r => (i, reset, r),
            _ => (i, now, r),
        });
        if let Some((_, reset, _)) = self.caret { self.caret_on = ((now - reset).max(0.0) / BLINK).floor() as i64 % 2 == 0; }
        self.dirty = true;
        self.page_len = self.scene.instances.len();
        self.base = self.scene.instances.clone();
        self.painted_phase = self.run.as_ref().map_or(0, |r| r.phase(now));
        // Ramps and flows apply from the first frame.
        self.animating = true;
        self.finish();
    }

    /// Apply what moves without a new layout (morph ramps, flows, the old page dissolving), then the marks.
    fn finish(&mut self) {
        let t0 = crate::perf::now_ms();
        self.animate();
        self.paint_marks();
        self.paint_gaze();
        self.paint_overlay();
        if !self.painting { self.perf.pending.scene += (crate::perf::now_ms() - t0) as f32; }
    }

    /// The frame-time readout (DESIGN.md §1.12), drawn by the annotation layer in Departure Mono at the
    /// window's bottom right: frames in the last second, last / mean / max ms of each part, what is
    /// drawn, the quality, the backend and the device pixel ratio.
    fn paint_overlay(&mut self) {
        self.overlay = self.timing.overlay;
        if !self.overlay { return; }
        self.overlay_at = self.now;
        let p = &self.perf;
        let sm = p.summary(crate::perf::now_ms());
        let row = |name: &str, v: [f32; 3]| format!("{name:<14}{:>7.2}{:>7.2}{:>7.2}", v[0], v[1], v[2]);
        let parts = self.run.as_ref().map_or(0, |r| r.particles.len());
        let lines = [
            format!("F2 frames      {:>3}/s  drawn {} skipped {}", sm.fps, p.drawn, p.skipped),
            format!("{:<14}{:>7}{:>7}{:>7}", "ms", "last", "avg", "max"),
            row("update+layout", sm.update_layout),
            row("scene build", sm.scene),
            row("gpu submit", sm.submit),
            row("total", sm.total),
            format!("instances {}  particles {}  glyph atlas {:.0}%", self.scene.instances.len(), parts, self.text.atlas_use() * 100.0),
            format!("layouts {}  shaped {}  views {}", p.counters.layouts, self.text.shaped(), p.counters.views),
            format!("quality {}{}  {}  dpr {}", p.quality, if p.auto { " auto" } else { "" }, if p.backend.is_empty() { "?" } else { &p.backend }, self.scale),
        ];
        let look = self.look_of.map(|f| f(&self.state)).unwrap_or_default();
        let col = [look.theme.ink[0], look.theme.ink[1], look.theme.ink[2], 0.0];
        const SIZE: f32 = 11.0;
        const LINE: f32 = 14.0;
        let s = self.scale;
        let width = lines.iter().map(|l| self.text.measure(l, MONO, SIZE, LINE, None).0).fold(0.0, f32::max);
        let (x, y) = ((self.size.0 - width - 16.0).max(4.0).round(), (self.size.1 - LINE * lines.len() as f32 - 14.0).max(4.0).round());
        let clip = [0.0, 0.0, self.size.0 * s, self.size.1 * s];
        let plate = Rect::new(x - 6.0, y - 4.0, width + 12.0, LINE * lines.len() as f32 + 8.0);
        self.scene.annot.push(Instance { rect: [(plate.x * s).round(), (plate.y * s).round(), (plate.w * s).round(), (plate.h * s).round()], paint: [0.0, 1.0, 0.92, SHAPE], clip, look: col, ..Default::default() });
        for (k, l) in lines.iter().enumerate() {
            let proto = Instance { paint: [0.0, 0.0, 1.0, 0.0], look: col, ..Default::default() };
            let mut out = vec![];
            self.text.emit(Para { s: l, face: MONO, size: SIZE, line: LINE, width: 1e5, print: 0.0, around: None }, (x, y + k as f32 * LINE), proto, s, |mut i, _| { i.clip = clip; out.push(i); });
            self.scene.annot.extend(out);
        }
    }

    fn animate(&mut self) {
        self.scene.truncate(self.page_len);
        let now = self.now;
        let active = self.run.is_some() || !self.flows.is_empty();
        // One more pass after the last animation ends puts every instance back as painted.
        // Parts in flight are drawn over the page, so they do not pass behind what is already there.
        let mut lifted = vec![];
        if active || self.animating {
            for i in 0..self.page_len {
                let mut inst = self.base[i];
                let (node, id) = self.owners[i];
                if let Some(r) = &self.run {
                    match self.roles[node] {
                        1 => inst.extra[2] = inst.extra[2].max(ramp(r.src_ramp, now)),
                        2 => inst.extra[2] = inst.extra[2].max(ramp(r.dst_ramp, now)),
                        _ => {}
                    }
                }
                if let Some(f) = self.flows.get(&id) {
                    let p = ((now - f.start) / f.dur.max(1e-4)).clamp(0.0, 1.0);
                    let e = Ease::InOutCubic.at(p);
                    for k in 0..4 { inst.rect[k] = f.from[k] + (inst.rect[k] - f.from[k]) * e; }
                    if p < 1.0 { inst.clip = [f.clip[0].min(inst.clip[0]), f.clip[1].min(inst.clip[1]), f.clip[2].max(inst.clip[2]), f.clip[3].max(inst.clip[3])]; }
                    inst.extra[2] = inst.extra[2].max(f.loose * (std::f32::consts::PI * e).sin());
                    if p < 1.0 && self.scene.layer.is_none() { lifted.push((i, inst)); inst.rect = [0.0; 4]; }
                }
                self.scene.instances[i] = inst;
            }
        }
        if active || self.animating { self.dirty = true; }
        self.animating = active;
        if let Some((i, _, _)) = self.caret && i < self.page_len { self.scene.instances[i].paint[2] = if self.caret_on { self.base[i].paint[2] } else { 0.0 }; }
        self.ghosts.retain(|g| now < g.start + g.dur);
        for g in &self.ghosts {
            let p = ((now - g.start) / g.dur.max(1e-4)).clamp(0.0, 1.0);
            let mut inst = g.inst;
            inst.extra[2] = inst.extra[2].max(p);
            inst.paint[2] *= 1.0 - p;
            self.scene.push(inst);
        }
        self.lifted.clear();
        // Text over pictures and shapes, as it is on any page.
        lifted.sort_by_key(|(_, inst)| std::cmp::Reverse(inst.paint[3] as u32));
        // In a layer of their own (only when there is no modal layer): where they have ink they replace
        // what is beneath in the near image, so a title flying over a picture is drawn as text.
        if !lifted.is_empty() { self.scene.begin_layer(); }
        for (i, inst) in lifted { self.lifted.insert(i, self.scene.instances.len()); self.scene.push(inst); }
        self.body_len = self.scene.instances.len();
    }

    // ── Morph and flow ───────────────────────────────────────────────────────────────────────────

    /// Morph element `from` into element `to` (keys; see DESIGN.md §1.10). Asking for the reverse of
    /// the morph in progress turns it around: in flight, every tile goes back from where it is; while
    /// the source is still loosening, it tightens again. Any other morph in progress is cut short.
    pub fn morph(&mut self, from: &str, to: &str, spec: MorphSpec) {
        let now = self.now;
        let tighten = spec.tighten_ms / 1000.0;
        let mut k0 = 0.0;
        if let Some(r) = &mut self.run {
            let reverse = r.from == to && r.to == from;
            if reverse && let Some(clock) = r.clock(now) {
                let dur = spec.duration_ms / 1000.0 * (1.0 - spec.stagger.clamp(0.0, 0.9));
                morph::reverse(&mut r.particles, clock, r.spec.ease, dur);
                std::mem::swap(&mut r.from, &mut r.to);
                r.spec = MorphSpec { ease: spec.ease, ..r.spec };
                r.flight = Some([now, now + dur]);
                r.src_until = now;
                r.dst_from = now + dur;
                r.dst_ramp = [1.0, now + dur, 0.0, now + dur + tighten];
                r.end = now + dur + tighten;
                r.upload = next_run();
                self.paint();
                return;
            }
            if reverse && now < r.src_until {
                let k = ramp(r.src_ramp, now);
                r.src_until = NEVER;
                r.src_ramp = [k, now, 0.0, now + tighten * k];
                r.flight = None;
                r.dst_from = NEVER;
                r.end = now + tighten * k;
                self.paint();
                return;
            }
            // Starting from where the morph in progress left the element.
            k0 = if r.to == from { if now >= r.dst_from { ramp(r.dst_ramp, now) } else { 1.0 } }
                 else if r.from == from && now < r.src_until { ramp(r.src_ramp, now) } else { 0.0 };
        }
        self.run = None;
        self.paint();
        let (src, dst) = (self.collect(from), self.collect(to));
        if src.is_empty() || dst.is_empty() { return; }
        let pitch = (spec.lattice * self.scale).max(1.5);
        let tiles = morph::tiles(&src, &dst, pitch, &spec, &self.text.pixels);
        let (loosen, flight) = (spec.loosen_ms / 1000.0 * (1.0 - k0), spec.duration_ms / 1000.0);
        let (t1, t2) = (now + loosen, now + loosen + flight);
        let id = next_run();
        self.run = Some(MorphRun {
            id, upload: id, from: from.into(), to: to.into(), spec,
            src_until: t1, src_ramp: [k0, now, 1.0, t1], flight: Some([t1, t2]),
            dst_from: t2, dst_ramp: [1.0, t2, 0.0, t2 + tighten], end: t2 + tighten,
            particles: tiles.particles, tile: tiles.tile, src, dst,
        });
        self.paint();
    }

    /// Everything a node and its descendants draw, drawn or hidden, at looseness 1 and fully born.
    fn collect(&self, key: &str) -> Vec<Instance> {
        let Some(root) = self.nodes.iter().position(|n| n.key == key) else { return vec![] };
        let page = (0..self.page_len).filter(|&i| within(&self.nodes, self.owners[i].0, root)).map(|i| self.base[i]);
        let unseen = self.unseen.iter().filter(|(n, _)| within(&self.nodes, *n, root)).map(|(_, inst)| *inst);
        page.chain(unseen).map(|mut i| { i.extra[2] = 1.0; i.extra[3] = -1e4; i }).collect()
    }

    /// The page as it is drawn now, for a flow.
    fn record(&self) -> Vec<Old> {
        (0..self.page_len).map(|i| Old { id: self.owners[i].1, tag: self.tags[i].clone(), inst: self.scene.instances[i] }).collect()
    }

    /// After a relayout for a new page: instances of a flow subtree that left one place and appeared in
    /// another fly between them and keep their birth time; what is gone dissolves.
    fn flow_from(&mut self, old: Vec<Old>, spec: FlowSpec) {
        let now = self.now;
        let new_ids: HashSet<u64> = self.owners.iter().map(|o| o.1).collect();
        let old_ids: HashSet<u64> = old.iter().map(|o| o.id).collect();
        let by_tag: HashMap<&Tag, usize> = old.iter().enumerate().filter(|(_, o)| !new_ids.contains(&o.id)).filter_map(|(k, o)| o.tag.as_ref().map(|t| (t, k))).collect();
        let mut used = vec![false; old.len()];
        for i in 0..self.page_len {
            let id = self.owners[i].1;
            if old_ids.contains(&id) { continue; }
            let Some(&k) = self.tags[i].as_ref().and_then(|t| by_tag.get(t)) else { continue };
            if used[k] { continue; }
            used[k] = true;
            let o = &old[k];
            self.flows.insert(id, FlowRec { from: o.inst.rect, clip: o.inst.clip, start: now, dur: spec.flow_ms / 1000.0, loose: spec.flow_loose });
            self.births.insert(id, o.inst.extra[3]);
            self.base[i].extra[3] = o.inst.extra[3];
        }
        for (k, o) in old.iter().enumerate() {
            if !used[k] && !new_ids.contains(&o.id) && spec.out_ms > 0.0 { self.ghosts.push(Ghost { inst: o.inst, start: now, dur: spec.out_ms / 1000.0 }); }
        }
        self.animating = true;
        self.finish();
    }

    fn paint_marks(&mut self) {
        self.scene.truncate(self.body_len);
        let Some(r) = self.marks.at else { return };
        if self.marks.alpha < 0.02 { return; }
        let s = self.scale;
        let r = r.inset(-4.0, -4.0, -4.0, -4.0);
        let (len, th) = (8.0f32.min(r.w * 0.4).min(r.h * 0.4), 1.5);
        let c = self.marks.clip.inset(-6.0, -6.0, -6.0, -6.0);
        let clip = [c.x * s, c.y * s, c.right() * s, c.bottom() * s];
        let bar = |x: f32, y: f32, w: f32, h: f32| Instance {
            rect: [(x * s).round(), (y * s).round(), (w * s).round().max(1.0), (h * s).round().max(1.0)],
            paint: [1.0, 1.0, self.marks.alpha, SHAPE], clip, extra: [0.0, 0.0, 0.0, -1e4], ..Default::default()
        };
        for (cx, cy, sx, sy) in [(r.x, r.y, 1.0, 1.0), (r.right(), r.y, -1.0, 1.0), (r.x, r.bottom(), 1.0, -1.0), (r.right(), r.bottom(), -1.0, -1.0)] {
            let x0 = if sx > 0.0 { cx } else { cx - len };
            let y0 = if sy > 0.0 { cy } else { cy - len };
            self.scene.push(bar(x0, if sy > 0.0 { cy } else { cy - th }, len, th));
            self.scene.push(bar(if sx > 0.0 { cx } else { cx - th }, y0, th, len));
        }
    }

    /// Stage 3: the annotation records of this frame and their instances, exact, in Departure Mono at
    /// whole multiples of its size.
    fn paint_gaze(&mut self) {
        self.scene.annot.clear();
        let g = match self.gaze_of { Some(f) => f(&self.state), None => { self.annots.clear(); return; } };
        if g.mode == GazeMode::Off { self.annots.clear(); self.gaze_seen.clear(); self.gaze_until = f32::NEG_INFINITY; return; }
        let s = self.scale;
        let now = self.now;
        let window = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        let pointer = (self.field.pointer[2] > 0.0).then(|| (self.field.pointer[0] / s, self.field.pointer[1] / s));
        let focus = self.focus.as_deref().and_then(|f| self.nodes.iter().position(|n| n.key == f));
        let annots = gaze::select(&self.nodes, &g, pointer, focus, window);
        self.gaze_seen = annots.iter().map(|a| (a.key.clone(), self.gaze_seen.get(&a.key).copied().unwrap_or(now))).collect();
        self.gaze_until = self.gaze_seen.values().fold(f32::NEG_INFINITY, |m, &t| m.max(t + g.draw_ms / 1000.0));
        let (scope, scopes) = gaze::scope(&self.nodes, window);
        let view = scopes.first().map_or(window, |&i| self.nodes[i].clip);
        let clip = [view.x * s, view.y * s, view.right() * s, view.bottom() * s];
        let col = [g.colour[0], g.colour[1], g.colour[2], 0.0];
        let t = s.round().max(1.0);
        let mut out: Vec<Instance> = Vec::new();
        let dev = |v: f32| (v * s).round();
        // A line or a block in device px: the gaze colour, or the page ground (`plate`) under labels.
        let bar = |out: &mut Vec<Instance>, x: f32, y: f32, w: f32, h: f32, alpha: f32, plate: bool| {
            if w <= 0.0 || h <= 0.0 { return; }
            out.push(Instance { rect: [x, y, w, h], paint: [0.0, if plate { 1.0 } else { 0.0 }, alpha, SHAPE], clip, look: col, ..Default::default() });
        };
        const SIZE: f32 = 11.0;
        const LINE: f32 = 14.0;
        let text = &mut self.text;
        // A label at logical (x, y), on a plate; returns its width.
        let label = |text: &mut Text, out: &mut Vec<Instance>, str: &str, x: f32, y: f32| -> f32 {
            let (w, _) = text.measure(str, MONO, SIZE, LINE, None);
            bar(out, dev(x) - t * 2.0, dev(y), dev(w) + t * 4.0, dev(LINE), 1.0, true);
            let proto = Instance { paint: [0.0, 0.0, 1.0, 0.0], look: col, ..Default::default() };
            text.emit(Para { s: str, face: MONO, size: SIZE, line: LINE, width: 1e5, print: 0.0, around: None }, (x, y), proto, s, |mut i, _| { i.clip = clip; out.push(i); });
            w
        };
        let mut tags: Vec<Rect> = vec![];
        for a in &annots {
            let p = gaze::drawn(now - self.gaze_seen[&a.key], g.draw_ms);
            let (x0, y0, x1, y1) = (dev(a.rect.x), dev(a.rect.y), dev(a.rect.right()), dev(a.rect.bottom()));
            let (w, h) = (x1 - x0, y1 - y0);
            if g.has(kinds::BOX) {
                // Drawn on round the rect: top, right, bottom, left.
                bar(&mut out, x0, y0, (w * p).round(), t, 0.9, false);
                bar(&mut out, x1 - t, y0, t, (h * p).round(), 0.9, false);
                bar(&mut out, x1 - (w * p).round(), y1 - t, (w * p).round(), t, 0.9, false);
                bar(&mut out, x0, y1 - (h * p).round(), t, (h * p).round(), 0.9, false);
            }
            if g.has(kinds::FRAME) {
                let len = (10.0f32 * s).min(w * 0.3).min(h * 0.3).max(t * 2.0) * p;
                let len = len.round();
                for (cx, cy, sx, sy) in [(x0, y0, 1.0, 1.0), (x1, y0, -1.0, 1.0), (x0, y1, 1.0, -1.0), (x1, y1, -1.0, -1.0)] {
                    let bx = if sx > 0.0 { cx } else { cx - len };
                    let by = if sy > 0.0 { cy } else { cy - t };
                    bar(&mut out, bx, by, len, t, 1.0, false);
                    let vx = if sx > 0.0 { cx } else { cx - t };
                    let vy = if sy > 0.0 { cy } else { cy - len };
                    bar(&mut out, vx, vy, t, len, 1.0, false);
                }
            }
            if g.has(kinds::LEADER) { continue; }
            let mut tag = String::new();
            if g.has(kinds::INDEX) { tag = format!("{:02}", gaze::count(a.index as f32, p) as u32); }
            if g.has(kinds::KIND) {
                if !tag.is_empty() { tag.push(' '); }
                // A callout may ask for where the element is: `{xy}` becomes its centre in the scope.
                let (cx, cy) = a.rect.center();
                let xy = format!("x {:.0}  y {:.0}", cx - scope.x, cy - scope.y);
                tag.push_str(&a.label.as_deref().unwrap_or(a.kind).replace("{xy}", &xy));
            }
            if !tag.is_empty() {
                let y = if a.rect.y - LINE - 2.0 >= view.y { a.rect.y - LINE - 2.0 } else { a.rect.y + 2.0 };
                // Two elements starting at the same corner: the later tag moves along.
                let (tw, _) = text.measure(&tag, MONO, SIZE, LINE, None);
                let mut x = a.rect.x + 2.0;
                while tags.iter().any(|r: &Rect| r.intersect(&Rect::new(x - 2.0, y, tw + 4.0, LINE)).w > 0.0 && r.intersect(&Rect::new(x - 2.0, y, tw + 4.0, LINE)).h > 0.0) { x += 8.0; }
                tags.push(Rect::new(x - 2.0, y, tw + 4.0, LINE));
                label(text, &mut out, &tag, x, y);
            }
            if g.has(kinds::SIZE) && a.rect.h >= LINE + 6.0 {
                // Inside the bottom right corner, where nothing else is labelled.
                let str = format!("{}×{}", gaze::count(a.rect.w.round(), p), gaze::count(a.rect.h.round(), p));
                let (tw, _) = text.measure(&str, MONO, SIZE, LINE, None);
                label(text, &mut out, &str, a.rect.right() - tw - 4.0, a.rect.bottom() - LINE - 3.0);
            }
        }
        if g.has(kinds::LEADER) {
            let names: Vec<String> = annots.iter().map(|a| {
                let name = a.label.clone().unwrap_or_else(|| a.kind.to_owned());
                if g.has(kinds::INDEX) { format!("{:02}  {name}", a.index) } else { name }
            }).collect();
            let sizes: Vec<(f32, f32)> = names.iter().map(|n| { let (w, _) = text.measure(n, MONO, SIZE, LINE, None); (w + 4.0, LINE) }).collect();
            let lscope = if g.has(kinds::RULER) { scope.inset(22.0, 22.0, 0.0, 0.0) } else { scope };
            let boxes = gaze::place_labels(&annots, lscope, |a| sizes[annots.iter().position(|b| b.node == a.node).unwrap_or(0)]);
            for (k, a) in annots.iter().enumerate() {
                let p = gaze::drawn(now - self.gaze_seen[&a.key], g.draw_ms);
                let (side, b) = boxes[k];
                let (ax, ay) = match side { gaze::Side::Left => (a.rect.x, a.rect.center().1), gaze::Side::Right => (a.rect.right(), a.rect.center().1) };
                let (lx, ly) = match side { gaze::Side::Left => (b.right() + 4.0, b.center().1), gaze::Side::Right => (b.x - 4.0, b.center().1) };
                let knee = match side { gaze::Side::Left => lx + 14.0, gaze::Side::Right => lx - 14.0 };
                // Elbow: along from the element to the knee, up or down to the label's line, into the label.
                let (ax, ay, lx, ly, knee) = (dev(ax), dev(ay), dev(lx), dev(ly), dev(knee));
                let seg1 = ((ax - knee).abs() * p).round();
                if ax > knee { bar(&mut out, ax - seg1, ay, seg1, t, 1.0, false); } else { bar(&mut out, ax, ay, seg1, t, 1.0, false); }
                if p >= 1.0 {
                    bar(&mut out, knee.min(ax).min(knee), ay.min(ly), t, (ay - ly).abs() + t, 1.0, false);
                    bar(&mut out, lx.min(knee), ly, (knee - lx).abs(), t, 1.0, false);
                }
                bar(&mut out, ax - t, ay - t, t * 3.0, t * 3.0, 1.0, false);
                if p >= 1.0 { label(text, &mut out, &names[k], b.x + 2.0, b.y); }
            }
        }
        let pt = pointer.filter(|&(x, y)| scope.contains(x, y));
        if g.has(kinds::GRID) {
            let (x0, y0, x1, y1) = (dev(scope.x), dev(scope.y), dev(scope.right()), dev(scope.bottom()));
            let mut y = y0;
            while y < y1 { bar(&mut out, x0, y, x1 - x0, t, 0.10, false); y += dev(8.0); }
            let cols = 12.0;
            let gutter = 16.0;
            let cw = (scope.w - gutter * (cols - 1.0)) / cols;
            for c in 0..12 {
                let x = scope.x + c as f32 * (cw + gutter);
                bar(&mut out, dev(x), y0, cw.mul_add(s, 0.0).round(), y1 - y0, 0.05, false);
                bar(&mut out, dev(x), y0, t, y1 - y0, 0.22, false);
                bar(&mut out, dev(x + cw) - t, y0, t, y1 - y0, 0.22, false);
            }
        }
        if g.has(kinds::RULER) {
            let (x0, y0, x1, y1) = (dev(scope.x), dev(scope.y), dev(scope.right()), dev(scope.bottom()));
            bar(&mut out, x0, y0, x1 - x0, dev(16.0), 0.85, true);
            bar(&mut out, x0, y0, dev(16.0), y1 - y0, 0.85, true);
            let mut v = 0.0;
            while scope.x + v < scope.right() {
                let len = if v % 100.0 == 0.0 { 9.0 } else if v % 50.0 == 0.0 { 6.0 } else { 3.0 };
                bar(&mut out, dev(scope.x + v), y0 + dev(16.0) - dev(len), t, dev(len), 0.9, false);
                if v % 100.0 == 0.0 && v > 0.0 { label(text, &mut out, &format!("{v}"), scope.x + v + 3.0, scope.y); }
                v += 10.0;
            }
            let mut v = 0.0;
            while scope.y + v < scope.bottom() {
                let len = if v % 100.0 == 0.0 { 9.0 } else if v % 50.0 == 0.0 { 6.0 } else { 3.0 };
                bar(&mut out, x0 + dev(16.0) - dev(len), dev(scope.y + v), dev(len), t, 0.9, false);
                v += 10.0;
            }
            if let Some((px, py)) = pt {
                bar(&mut out, dev(px), y0, t, dev(16.0), 1.0, false);
                bar(&mut out, x0, dev(py), dev(16.0), t, 1.0, false);
            }
        }
        if g.has(kinds::CROSS) && let Some((px, py)) = pt {
            bar(&mut out, dev(scope.x), dev(py), dev(scope.w), t, 0.45, false);
            bar(&mut out, dev(px), dev(scope.y), t, dev(scope.h), 0.45, false);
            label(text, &mut out, &format!("x {:.0}  y {:.0}", px - scope.x, py - scope.y), px + 10.0, py + 8.0);
        }
        self.scene.annot = out;
        self.annots = annots;
    }
}


/// Where `paint` puts instances: the scene (with owners and flow tags) or, for hidden nodes, aside.
struct Sink<'a> {
    scene: &'a mut Scene,
    owners: &'a mut Vec<(usize, u64)>,
    tags: &'a mut Vec<Option<Tag>>,
    unseen: &'a mut Vec<(usize, Instance)>,
    counters: &'a mut HashMap<usize, [u32; 2]>,
    latest: &'a mut f32,
    loose: &'a mut bool,
}

/// What a subtree passes down besides fonts: its material override (nesting level and id) and the
/// looseness it holds.
#[derive(Clone, Copy, Default)]
struct Inherit { level: u8, mat: Option<u8>, loosen: f32, select: bool }
impl Inherit {
    fn with(self, mat: Option<u8>, loosen: f32) -> Self {
        let (level, mat) = match mat { Some(m) => (self.level + 1, Some(m)), None => (self.level, self.mat) };
        Self { level, mat, loosen: self.loosen.max(loosen), select: self.select }
    }
    fn code(&self) -> f32 { self.mat.map_or(0.0, |m| crate::style::override_code(self.level, m)) }
}

/// Whether node `i` is `root` or inside it.
fn within<M>(nodes: &[Node<M>], mut i: usize, root: usize) -> bool {
    loop {
        if i == root { return true; }
        match nodes[i].parent { Some(p) => i = p, None => return false }
    }
}

fn ident(key: &str, part: u32, glyph: u32) -> u64 {
    let mut h = DefaultHasher::new();
    (key, part, glyph).hash(&mut h);
    h.finish()
}

/// Take every modal element out of the tree (they are laid over the window instead).
fn hoist<M>(el: &mut El<M>, out: &mut Vec<El<M>>) {
    let kids = std::mem::take(&mut el.children);
    for mut c in kids {
        if c.modal.is_some() { out.push(c); continue; }
        hoist(&mut c, out);
        el.children.push(c);
    }
}

/// What `flatten` needs to build a virtual list's rows (scroll offsets, viewport heights from the last
/// layout, the window's height) and the row ranges it built.
struct VirtCtx<'a> { scroll: &'a HashMap<String, f32>, views: &'a HashMap<String, f32>, window: f32, made: HashMap<String, (usize, usize)> }
/// Per node, what goes to the taffy tree: its style and its children.
#[derive(Default)]
struct Built { styles: Vec<Style>, kids: Vec<Vec<usize>> }

/// The rows `first..last` a viewport `h` tall at offset `off` shows.
fn visible_rows(off: f32, h: f32, count: usize, row_h: f32) -> (usize, usize) {
    let rh = row_h.max(1.0);
    let first = ((off / rh).floor().max(0.0) as usize).min(count);
    let last = (((off + h) / rh).ceil().max(0.0) as usize).min(count);
    (first, last.max(first))
}

/// What a leaf's measured size depends on: when it is unchanged, taffy's cached size stands.
fn measure_key(c: &Content) -> u64 {
    let mut h = DefaultHasher::new();
    match c {
        Content::Text { s, size, line, font, around, nowrap, .. } => {
            (s, size.to_bits(), line.to_bits(), font.resolve([0; 3]), around.as_ref().map_or(0, |a| a.id()), nowrap).hash(&mut h);
        }
        Content::Picture { ratio, .. } => ratio.to_bits().hash(&mut h),
        Content::None => 0u8.hash(&mut h),
    }
    h.finish()
}

/// Move an element tree into flat nodes (paint order). Unkeyed nodes are keyed by path.
/// Font roles are inherited: each text leaf's role is resolved here to the face of its nearest
/// `fonts` ancestor. So is being disabled: a disabled subtree has no handlers and cannot take focus.
/// `off` is (disabled, hidden), both inherited; `flow` is the nearest flow id above and its node.
/// A virtual list's rows are built here, for what its viewport shows.
#[allow(clippy::too_many_arguments)]
fn flatten<M>(el: El<M>, parent: Option<usize>, path: String, roles: [u8; 3], off: (bool, bool), flow: Option<(Rc<str>, usize)>, inh: Inherit, vx: &mut VirtCtx, built: &mut Built, nodes: &mut Vec<Node<M>>) -> usize {
    let El { key, style, mut content, fill, border, radius, mut on_press, mut on_key, mut on_drag, mut on_blur, mut focusable, scroll, horizontal, bar, reveal, disabled, mut commit, modal, fonts, loose, wipe, flow: own_flow, hidden, annotate, gaze_scope, material, loosen, anchor, clips, calm, mut on_text, multiline, selectable, cursor, virt, access, mut children } = el;
    let key = if key.is_empty() { path } else { key };
    let roles = fonts.unwrap_or(roles);
    let (off, hidden) = (off.0 || disabled, off.1 || hidden);
    if off || hidden { on_press = None; on_key = None; on_drag = None; on_blur = None; on_text = None; focusable = false; commit = false; }
    if let Content::Text { font, .. } = &mut content { *font = Font::Face(font.resolve(roles)); }
    let i = nodes.len();
    let flow = own_flow.map(|f| (Rc::from(f.as_str()), i)).or(flow);
    // A material override nests: a deeper one gets a higher level, so it wins where both draw.
    let mut inh = inh.with(material, loosen);
    inh.select |= selectable;
    let virt_info = virt.as_ref().map(|v| (v.count, v.row_h, v.selected));
    if let Some(v) = virt {
        // Only the rows the viewport shows, and `overscan` more on each side, are built; a column as tall
        // as all the rows holds them, so the scroll extent is the whole list's.
        let offset = vx.scroll.get(&key).copied().unwrap_or(0.0);
        let h = vx.views.get(&key).copied().unwrap_or(vx.window);
        let (first, last) = visible_rows(offset, h, v.count, v.row_h);
        let (a, b) = (first.saturating_sub(v.overscan), (last + v.overscan).min(v.count));
        vx.made.insert(key.clone(), (a, b));
        // The built rows flow from a padding as tall as the rows above them.
        let mut rows = crate::el::col().key(format!("{key}/rows")).fill_width().height(v.count as f32 * v.row_h);
        rows.style.padding.top = length(a as f32 * v.row_h);
        for r in a..b {
            let mut row = (v.build)(r);
            if row.key.is_empty() { row.key = format!("{key}/row{r}"); }
            row.style.size.height = length(v.row_h);
            row.style.flex_shrink = 0.0;
            rows.children.push(row);
        }
        children = vec![rows];
    }
    let placeholder = NodeId::from(0u64);
    nodes.push(Node {
        key, parent, rect: Rect::default(), clip: Rect::default(), inner: Rect::default(), scroll, horizontal, focusable, disabled: off, extent: 0.0,
        content, fill, border, radius, on_press, on_key, on_drag, on_blur, loose, wipe, bar, reveal, commit, modal, hidden, flow: flow.clone(),
        mat: inh.code(), loosen: inh.loosen, annotate, scope: gaze_scope, anchor, clips, calm, on_text, multiline, selectable: inh.select, cursor,
        virt: virt_info, access, local: taffy::Layout::new(), tid: placeholder,
    });
    built.styles.push(style);
    built.kids.push(vec![]);
    for (n, c) in children.into_iter().enumerate() {
        let p = format!("{}/{n}", nodes[i].key);
        let k = flatten(c, Some(i), p, roles, (off, hidden), flow.clone(), inh, vx, built, nodes);
        built.kids[i].push(k);
    }
    i
}
