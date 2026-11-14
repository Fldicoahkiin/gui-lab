//! Runtime: state, view, layout, hit-testing, focus, scrolling and instance birth times. No GPU here,
//! so all of it can be tested headless.
use crate::el::{Content, Drag, El, Font, Key, Modal, OnDrag, OnKey, Paint, Phase};
use crate::morph::{self, Cmd, Ease, FlowSpec, MorphSpec, Particle};
use crate::scene::{Field, Instance, PICTURE, SHAPE, Scene};
use crate::gaze;
use crate::style::{Gaze, GazeMode, gaze as kinds};
use crate::text::{Face, MONO, Para, Text};
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
}

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
            gaze_of: None, gaze_seen: HashMap::new(), annots: vec![], hovered: None, ticked: None,
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

    // ── Input ────────────────────────────────────────────────────────────────────────────────────
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
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
        self.now = t;
        if let Some(f) = self.ticked && f(&mut self.state, t) { self.relayout(); }
        let target = if self.keyboard { self.focus.clone() } else { self.hover.clone().or_else(|| self.focus.clone()) };
        let target = target.and_then(|k| self.node(&k).map(|n| (n.rect, n.clip)));
        let k = 1.0 - (-dt * 16.0).exp();
        match target {
            Some((r, clip)) => {
                self.marks.at = Some(match self.marks.at { Some(cur) if self.marks.alpha > 0.01 => cur.lerp(&r, k), _ => r });
                self.marks.clip = clip;
                self.marks.alpha += (1.0 - self.marks.alpha) * k;
            }
            None => self.marks.alpha -= self.marks.alpha * k,
        }
        // A morph moving into its next phase changes what is drawn; one that is over is dropped and the
        // view decides again.
        let repaint = match &self.run {
            Some(r) if t >= r.end => { self.run = None; true }
            Some(r) => r.phase(t) != self.painted_phase,
            None => false,
        };
        self.flows.retain(|_, f| t < f.start + f.dur);
        if repaint { self.paint(); } else { self.finish(); }
    }

    /// Start again at `t`: everything on the page condenses in from loose.
    pub fn restart(&mut self, t: f32) {
        self.now = t;
        for b in self.births.values_mut() { *b = t; }
        self.paint();
    }

    /// Logical px.
    pub fn pointer_move(&mut self, x: f32, y: f32) {
        let s = self.scale;
        self.field.pointer = [x * s, y * s, 1.0, 0.0];
        self.keyboard = false;
        let hover = self.hit(x, y).and_then(|i| self.up(i, Node::active)).map(|i| self.nodes[i].key.clone());
        if hover != self.hover {
            // A control the pointer comes onto gets a short burst over it (the glitch material reads it).
            if let Some(n) = hover.as_deref().and_then(|k| self.node(k)) { self.burst(n.rect, 0.75, 0.22); }
            self.hover = hover;
            if let Some(f) = self.hovered && f(&mut self.state, self.hover.as_deref()) { self.relayout(); }
        }
        if self.capture.is_some() { self.drag(Phase::Move, x, y); }
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
        self.field.pointer = [-1e5, -1e5, 0.0, 0.0];
        if self.hover.take().is_some() && let Some(f) = self.hovered && f(&mut self.state, None) { self.relayout(); }
    }
    /// A burst over a rect (logical px) from now: `strength` 0..1 for `secs`.
    pub fn burst(&mut self, r: Rect, strength: f32, secs: f32) {
        let s = self.scale;
        self.field.burst(Some([r.x * s, r.y * s, r.right() * s, r.bottom() * s]), self.now, strength, secs);
    }

    pub fn press(&mut self, x: f32, y: f32) {
        // A press without a release before it (a lost pointer-up) ends the old capture.
        self.capture = None;
        self.pointer_move(x, y);
        let s = self.scale;
        self.field.front(x * s, y * s, self.now, 1.0);
        let target = self.hit(x, y).and_then(|i| self.up(i, |n| n.focusable)).map(|i| self.nodes[i].key.clone());
        self.set_focus(target);
        // Losing focus may have changed the page, so look again.
        let hit = self.hit(x, y);
        if let Some(i) = hit.and_then(|i| self.up(i, |n| n.on_drag.is_some())) {
            let n = &self.nodes[i];
            let handler = n.on_drag.clone().expect("drag handler");
            self.capture = Some(Capture { key: n.key.clone(), handler, start: (x, y), rect: n.rect });
            self.drag(Phase::Start, x, y);
            return;
        }
        let pressed = hit.and_then(|i| self.up(i, |n| n.on_press.is_some()));
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
        let mut at = self.hit(x, y);
        while let Some(i) = at.and_then(|i| self.up(i, |n| n.scroll)) {
            let d = if self.nodes[i].horizontal { if dx != 0.0 { dx } else { dy } } else { dy };
            if d != 0.0 && self.scroll_by(i, d) { self.relayout(); return; }
            at = self.nodes[i].parent;
        }
    }

    /// A key press; the flag is Shift. Returns whether anything used it.
    pub fn key(&mut self, k: Key, shift: bool) -> bool {
        self.keyboard = true;
        if k == Key::Tab { self.cycle_focus(shift); return true; }
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
        let Some(fi) = fi else { return self.key_scroll(None, k) };
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
        self.key_scroll(Some(fi), k)
    }

    // ── Internals ────────────────────────────────────────────────────────────────────────────────
    fn send(&mut self, m: M) {
        (self.update)(&mut self.state, m);
        self.relayout();
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
            self.relayout();
        }
    }

    fn key_scroll(&mut self, from: Option<usize>, k: Key) -> bool {
        let dy = match k { Key::Up => -40.0, Key::Down => 40.0, Key::PageUp => -self.size.1 * 0.8, Key::PageDown => self.size.1 * 0.8, _ => return false };
        let start = from.or_else(|| (!self.nodes.is_empty()).then_some(0));
        let Some(i) = start.and_then(|i| self.up(i, |n| n.scroll && !n.horizontal).or_else(|| self.nodes.iter().position(|n| n.scroll && !n.horizontal))) else { return false };
        if self.scroll_by(i, dy) { self.relayout(); }
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
        // Commands are taken first: a flow needs the frame as it is before the new layout.
        let cmds = self.commands.map(|f| f(&mut self.state)).unwrap_or_default();
        let flow = cmds.iter().find_map(|c| if let Cmd::Flow(f) = c { Some(*f) } else { None });
        let old = flow.map(|_| self.record());
        self.layout();
        if let (Some(f), Some(old)) = (flow, old) { self.flow_from(old, f); }
        for c in cmds { if let Cmd::Morph { from, to, spec } = c { self.morph(&from, &to, spec); } }
    }

    fn layout(&mut self) {
        let el = (self.view)(&self.state);
        let mut tree: TaffyTree<usize> = TaffyTree::new();
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
        let root = flatten(el, None, "root".into(), [0; 3], (false, false), None, Inherit::default(), &mut tree, &mut nodes);
        let text = &mut self.text;
        let measure = |input: taffy::LayoutInput, _: NodeId, ctx: Option<&mut usize>, style: &Style| {
            let none = |_: *const (), _: f32| 0.0;
            match ctx.map(|i| &nodes[*i].content) {
                Some(Content::Text { s, size, line, font, .. }) => taffy::compute_leaf_layout(input, style, none, |known, avail| {
                    let limit = known.width.or(match avail.width { AvailableSpace::Definite(w) => Some(w), AvailableSpace::MinContent => Some(0.0), AvailableSpace::MaxContent => None });
                    let (w, h) = text.measure(s, font.resolve([0; 3]), *size, *line, limit);
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
        if tree.compute_layout_with_measure(root, space, measure).is_err() { return; }

        // How tall each scroll node's contents are, so stored offsets can be clamped before placing.
        for i in 0..nodes.len() {
            let l = *tree.layout(nodes[i].tid).expect("node in tree");
            if let Some(p) = nodes[i].parent && nodes[p].scroll {
                let pl = tree.layout(nodes[p].tid).expect("node in tree");
                let end = if nodes[p].horizontal { l.location.x + l.size.width + pl.padding.right + pl.border.right }
                          else { l.location.y + l.size.height + pl.padding.bottom + pl.border.bottom };
                nodes[p].extent = nodes[p].extent.max(end);
            }
        }
        for n in nodes.iter().filter(|n| n.scroll) {
            let size = tree.layout(n.tid).map(|l| if n.horizontal { l.size.width } else { l.size.height }).unwrap_or(0.0);
            let max = (n.extent - size).max(0.0);
            if let Some(o) = self.scroll.get_mut(&n.key) { *o = o.clamp(0.0, max); }
        }
        let screen = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        let mut inside = vec![screen; nodes.len()];
        for i in 0..nodes.len() {
            let l = *tree.layout(nodes[i].tid).expect("node in tree");
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
            inside[i] = if n.scroll { clip.intersect(&inner_border) } else { clip };
        }
        self.nodes = nodes;
        self.modal_at = self.nodes.iter().rposition(|n| n.modal.is_some());
        self.layer_changed();
        self.paint();
    }

    /// A modal layer opened or closed: focus moves into it (remembering where it was), or back.
    fn layer_changed(&mut self) {
        let key = self.modal_at.map(|m| self.nodes[m].key.clone());
        if key == self.modal_key { return; }
        match self.modal_at {
            Some(m) => {
                if self.modal_key.is_none() { self.return_focus = self.focus.clone(); }
                // The first field that takes keys (a text field), else the first control.
                let first = (m..self.nodes.len()).find(|&i| self.nodes[i].focusable && self.nodes[i].on_key.is_some())
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
            let under = self.modal_at.is_some_and(|m| i < m);
            let loose = if held[i] { 1.0 } else if under { n.loose.max(n.loosen).max(beneath) } else { n.loose.max(n.loosen) };
            let mat = n.mat;
            let dim = if n.disabled { 0.4 } else { 1.0 };
            let mut sink = Sink { scene: &mut self.scene, owners: &mut self.owners, tags: &mut self.tags, unseen: &mut self.unseen, counters: &mut counters };
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
                Content::Text { s: str, size, line, paint, font, print, caret } => {
                    let proto = Instance { paint: [paint.tone, paint.accent, paint.alpha, 0.0], look: [mat, paint.hue as f32, (n.inner.y * s).round(), (line * s).round().max(1.0)], ..Default::default() };
                    let para = Para { s: str, face: font.resolve([0; 3]), size: *size, line: *line, width: n.inner.w + 1.0, print: *print };
                    let mut seq = 0;
                    self.text.emit(para, (n.inner.x, n.inner.y), proto, s, |inst, glyph| {
                        put(&mut sink, inst, ident(&n.key, seq, glyph), Some(glyph));
                        seq += 1;
                    });
                    // The caret, only while the field it belongs to has focus. It keeps one identity, so
                    // moving it does not make it condense again.
                    let focused = caret_at.is_some_and(|f| within(&self.nodes, i, f));
                    if let Some(at) = caret.filter(|_| focused) && let Some(r) = self.text.caret(para, (n.inner.x, n.inner.y), at) {
                        let bar = Instance { rect: dev(Rect::new(r.x, r.y, 1.5, r.h)), paint: [paint.tone, 1.0, 1.0, SHAPE], ..Default::default() };
                        put(&mut sink, bar, ident(&n.key, u32::MAX - 4, 0), None);
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
        self.page_len = self.scene.instances.len();
        self.base = self.scene.instances.clone();
        self.painted_phase = self.run.as_ref().map_or(0, |r| r.phase(now));
        // Ramps and flows apply from the first frame.
        self.animating = true;
        self.finish();
    }

    /// Apply what moves without a new layout (morph ramps, flows, the old page dissolving), then the marks.
    fn finish(&mut self) {
        self.animate();
        self.paint_marks();
        self.paint_gaze();
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
        self.animating = active;
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
        if g.mode == GazeMode::Off { self.annots.clear(); self.gaze_seen.clear(); return; }
        let s = self.scale;
        let now = self.now;
        let window = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        let pointer = (self.field.pointer[2] > 0.0).then(|| (self.field.pointer[0] / s, self.field.pointer[1] / s));
        let focus = self.focus.as_deref().and_then(|f| self.nodes.iter().position(|n| n.key == f));
        let annots = gaze::select(&self.nodes, &g, pointer, focus, window);
        self.gaze_seen = annots.iter().map(|a| (a.key.clone(), self.gaze_seen.get(&a.key).copied().unwrap_or(now))).collect();
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
            text.emit(Para { s: str, face: MONO, size: SIZE, line: LINE, width: 1e5, print: 0.0 }, (x, y), proto, s, |mut i, _| { i.clip = clip; out.push(i); });
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
            if g.has(kinds::KIND) { if !tag.is_empty() { tag.push(' '); } tag.push_str(a.label.as_deref().unwrap_or(a.kind)); }
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
}

/// What a subtree passes down besides fonts: its material override (nesting level and id) and the
/// looseness it holds.
#[derive(Clone, Copy, Default)]
struct Inherit { level: u8, mat: Option<u8>, loosen: f32 }
impl Inherit {
    fn with(self, mat: Option<u8>, loosen: f32) -> Self {
        let (level, mat) = match mat { Some(m) => (self.level + 1, Some(m)), None => (self.level, self.mat) };
        Self { level, mat, loosen: self.loosen.max(loosen) }
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

/// Move an element tree into flat nodes (paint order) and a taffy tree. Unkeyed nodes are keyed by path.
/// Font roles are inherited: each text leaf's role is resolved here to the face of its nearest
/// `fonts` ancestor. So is being disabled: a disabled subtree has no handlers and cannot take focus.
/// `off` is (disabled, hidden), both inherited; `flow` is the nearest flow id above and its node.
#[allow(clippy::too_many_arguments)]
fn flatten<M>(el: El<M>, parent: Option<usize>, path: String, roles: [u8; 3], off: (bool, bool), flow: Option<(Rc<str>, usize)>, inh: Inherit, tree: &mut TaffyTree<usize>, nodes: &mut Vec<Node<M>>) -> NodeId {
    let El { key, style, mut content, fill, border, radius, mut on_press, mut on_key, mut on_drag, mut on_blur, mut focusable, scroll, horizontal, bar, reveal, disabled, mut commit, modal, fonts, loose, wipe, flow: own_flow, hidden, annotate, gaze_scope, material, loosen, anchor, children } = el;
    let key = if key.is_empty() { path } else { key };
    let roles = fonts.unwrap_or(roles);
    let (off, hidden) = (off.0 || disabled, off.1 || hidden);
    if off || hidden { on_press = None; on_key = None; on_drag = None; on_blur = None; focusable = false; commit = false; }
    if let Content::Text { font, .. } = &mut content { *font = Font::Face(font.resolve(roles)); }
    let i = nodes.len();
    let flow = own_flow.map(|f| (Rc::from(f.as_str()), i)).or(flow);
    // A material override nests: a deeper one gets a higher level, so it wins where both draw.
    let inh = inh.with(material, loosen);
    let leaf = !matches!(content, Content::None);
    let placeholder = NodeId::from(0u64);
    nodes.push(Node {
        key, parent, rect: Rect::default(), clip: Rect::default(), inner: Rect::default(), scroll, horizontal, focusable, disabled: off, extent: 0.0,
        content, fill, border, radius, on_press, on_key, on_drag, on_blur, loose, wipe, bar, reveal, commit, modal, hidden, flow: flow.clone(),
        mat: inh.code(), loosen: inh.loosen, annotate, scope: gaze_scope, anchor, tid: placeholder,
    });
    let kids: Vec<NodeId> = children.into_iter().enumerate().map(|(n, c)| {
        let p = format!("{}/{n}", nodes[i].key);
        flatten(c, Some(i), p, roles, (off, hidden), flow.clone(), inh, tree, nodes)
    }).collect();
    let tid = if leaf { tree.new_leaf_with_context(style, i) } else { tree.new_with_children(style, &kids) }.expect("taffy node");
    nodes[i].tid = tid;
    tid
}
