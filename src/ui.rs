//! Runtime: state, view, layout, hit-testing, focus, scrolling and instance birth times. No GPU here,
//! so all of it can be tested headless.
use crate::el::{Content, El, Key, OnKey, Paint};
use crate::scene::{Field, Instance, PICTURE, SHAPE, Scene};
use crate::text::{Para, Text};
use std::collections::HashMap;
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
    pub focusable: bool,
    /// For scroll nodes: height of what they hold, so offsets can be clamped.
    pub extent: f32,
    pub content: Content,
    fill: Option<Paint>,
    border: Option<(f32, Paint)>,
    radius: f32,
    on_press: Option<M>,
    on_key: Option<OnKey<M>>,
    on_blur: Option<M>,
    tid: NodeId,
}
impl<M> Node<M> {
    pub fn text(&self) -> Option<&str> { if let Content::Text { s, .. } = &self.content { Some(s) } else { None } }
    pub fn pressable(&self) -> bool { self.on_press.is_some() }
}

/// The four accent corners that frame the hovered or focused element and glide between targets.
#[derive(Default)]
struct Marks { at: Option<Rect>, clip: Rect, alpha: f32 }

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
    /// The last input came from the keyboard, so the marks follow focus rather than the pointer.
    keyboard: bool,
    marks: Marks,
}

impl<S, M: Clone> Ui<S, M> {
    /// `w`, `h` are device px; `scale` is device px per logical px.
    pub fn new(state: S, view: fn(&S) -> El<M>, update: fn(&mut S, M), font: Vec<u8>, w: u32, h: u32, scale: f32) -> Self {
        let mut ui = Self {
            state, view, update, text: Text::new(font), size: (0.0, 0.0), scale, now: 0.0, nodes: vec![], scene: Scene::default(),
            owners: vec![], page_len: 0, field: Field::default(), scroll: HashMap::new(), births: HashMap::new(),
            hover: None, focus: None, pressed: None, keyboard: false, marks: Marks::default(),
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

    // ── Input ────────────────────────────────────────────────────────────────────────────────────
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
        self.scale = scale;
        self.size = (w.max(1) as f32 / scale, h.max(1) as f32 / scale);
        self.relayout();
    }

    /// Advance the clock to `t` seconds; the marks glide toward their target.
    pub fn tick(&mut self, t: f32) {
        let dt = (t - self.now).clamp(0.0, 0.1);
        self.now = t;
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
        self.paint_marks();
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
        self.hover = self.hit(x, y).and_then(|i| self.up(i, |n| n.focusable)).map(|i| self.nodes[i].key.clone());
    }
    pub fn pointer_leave(&mut self) {
        self.field.pointer = [-1e5, -1e5, 0.0, 0.0];
        self.hover = None;
    }

    pub fn press(&mut self, x: f32, y: f32) {
        self.pointer_move(x, y);
        let s = self.scale;
        self.field.front(x * s, y * s, self.now, 1.0);
        let hit = self.hit(x, y);
        let target = hit.and_then(|i| self.up(i, |n| n.focusable)).map(|i| self.nodes[i].key.clone());
        self.set_focus(target);
        let pressed = hit.and_then(|i| self.up(i, |n| n.on_press.is_some()));
        if let Some(i) = pressed {
            self.pressed = Some(self.nodes[i].key.clone());
            let m = self.nodes[i].on_press.clone();
            if let Some(m) = m { self.send(m); } else { self.paint(); }
        }
    }

    pub fn release(&mut self, _x: f32, _y: f32) {
        // A held element condenses back from loose when let go.
        if let Some(k) = self.pressed.take() { self.rebirth(&k); }
    }

    /// Scroll the nearest scroll ancestor under the point by `dy` logical px; when it is already at its
    /// end, the next one out takes it.
    pub fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        let mut at = self.hit(x, y);
        while let Some(i) = at.and_then(|i| self.up(i, |n| n.scroll)) {
            if self.scroll_by(i, dy) { self.relayout(); return; }
            at = self.nodes[i].parent;
        }
    }

    /// A key press; the flag is Shift. Returns whether anything used it.
    pub fn key(&mut self, k: Key, shift: bool) -> bool {
        self.keyboard = true;
        if k == Key::Tab { self.cycle_focus(shift); return true; }
        let Some(fi) = self.focus.as_deref().and_then(|f| self.nodes.iter().position(|n| n.key == f)) else {
            return self.key_scroll(None, k);
        };
        if let Some(m) = self.nodes[fi].on_key.clone().and_then(|f| f(k, shift)) { self.send(m); return true; }
        if matches!(k, Key::Enter | Key::Char(' ')) && let Some(m) = self.nodes[fi].on_press.clone() {
            let (cx, cy) = self.nodes[fi].rect.center();
            let s = self.scale;
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

    /// Topmost node under a point, honouring clips.
    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.nodes.len()).rev().find(|&i| self.nodes[i].rect.contains(x, y) && self.nodes[i].clip.contains(x, y))
    }

    /// The node itself or its nearest ancestor that satisfies `f`.
    fn up(&self, mut i: usize, f: impl Fn(&Node<M>) -> bool) -> Option<usize> {
        loop {
            if f(&self.nodes[i]) { return Some(i); }
            i = self.nodes[i].parent?;
        }
    }

    fn within(&self, mut i: usize, root: usize) -> bool {
        loop {
            if i == root { return true; }
            match self.nodes[i].parent { Some(p) => i = p, None => return false }
        }
    }

    fn set_focus(&mut self, to: Option<String>) {
        if to == self.focus { return; }
        let old = std::mem::replace(&mut self.focus, to);
        let blur = old.and_then(|k| self.node(&k).and_then(|n| n.on_blur.clone()));
        if let Some(m) = blur { self.send(m); }
        self.reveal();
    }

    fn cycle_focus(&mut self, back: bool) {
        let order: Vec<usize> = (0..self.nodes.len()).filter(|&i| self.nodes[i].focusable).collect();
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

    /// Scroll every scroll ancestor of the focused node so that it is inside, innermost first.
    fn reveal(&mut self) {
        let Some(key) = self.focus.clone() else { return };
        let mut guard = 0;
        loop {
            guard += 1;
            let Some(i) = self.nodes.iter().position(|n| n.key == key) else { return };
            let mut a = self.nodes[i].parent;
            let mut moved = false;
            while let Some(p) = a.and_then(|p| self.up(p, |n| n.scroll)) {
                let (r, view) = (self.nodes[i].rect, self.nodes[p].clip.intersect(&self.nodes[p].inner));
                let dy = if r.y < view.y { r.y - view.y - 8.0 } else if r.bottom() > view.bottom() { (r.bottom() - view.bottom() + 8.0).min(r.y - view.y) } else { 0.0 };
                if dy != 0.0 && self.scroll_by(p, dy) { moved = true; break; }
                a = self.nodes[p].parent;
            }
            if !moved || guard > 4 { break; }
            self.relayout();
        }
    }

    fn key_scroll(&mut self, from: Option<usize>, k: Key) -> bool {
        let dy = match k { Key::Up => -40.0, Key::Down => 40.0, Key::PageUp => -self.size.1 * 0.8, Key::PageDown => self.size.1 * 0.8, _ => return false };
        let start = from.or_else(|| (!self.nodes.is_empty()).then_some(0));
        let Some(i) = start.and_then(|i| self.up(i, |n| n.scroll).or_else(|| self.nodes.iter().position(|n| n.scroll))) else { return false };
        if self.scroll_by(i, dy) { self.relayout(); }
        true
    }

    /// Returns whether the offset changed.
    fn scroll_by(&mut self, i: usize, dy: f32) -> bool {
        let n = &self.nodes[i];
        let max = (n.extent - n.rect.h).max(0.0);
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
        let el = (self.view)(&self.state);
        let mut tree: TaffyTree<usize> = TaffyTree::new();
        let mut nodes = Vec::new();
        // The root is the window, whatever its contents would like.
        let mut el = el;
        el.style.size = Size { width: length(self.size.0), height: length(self.size.1) };
        let root = flatten(el, None, "root".into(), &mut tree, &mut nodes);
        let text = &mut self.text;
        let measure = |input: taffy::LayoutInput, _: NodeId, ctx: Option<&mut usize>, style: &Style| {
            let none = |_: *const (), _: f32| 0.0;
            match ctx.map(|i| &nodes[*i].content) {
                Some(Content::Text { s, size, line, .. }) => taffy::compute_leaf_layout(input, style, none, |known, avail| {
                    let limit = known.width.or(match avail.width { AvailableSpace::Definite(w) => Some(w), AvailableSpace::MinContent => Some(0.0), AvailableSpace::MaxContent => None });
                    let (w, h) = text.measure(s, *size, *line, limit);
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
                let bottom = l.location.y + l.size.height + pl.padding.bottom + pl.border.bottom;
                nodes[p].extent = nodes[p].extent.max(bottom);
            }
        }
        for n in nodes.iter().filter(|n| n.scroll) {
            let max = (n.extent - tree.layout(n.tid).map(|l| l.size.height).unwrap_or(0.0)).max(0.0);
            if let Some(o) = self.scroll.get_mut(&n.key) { *o = o.clamp(0.0, max); }
        }
        let screen = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        let mut inside = vec![screen; nodes.len()];
        for i in 0..nodes.len() {
            let l = *tree.layout(nodes[i].tid).expect("node in tree");
            let (origin, clip) = match nodes[i].parent {
                Some(p) => {
                    let off = if nodes[p].scroll { self.scroll.get(&nodes[p].key).copied().unwrap_or(0.0) } else { 0.0 };
                    ((nodes[p].rect.x, nodes[p].rect.y - off), inside[p])
                }
                None => ((0.0, 0.0), screen),
            };
            let n = &mut nodes[i];
            n.rect = Rect::new(origin.0 + l.location.x, origin.1 + l.location.y, l.size.width, l.size.height);
            n.clip = clip;
            let (b, p) = (l.border, l.padding);
            let inner_border = n.rect.inset(b.left, b.top, b.right, b.bottom);
            n.inner = inner_border.inset(p.left, p.top, p.right, p.bottom);
            inside[i] = if n.scroll { clip.intersect(&inner_border) } else { clip };
        }
        self.nodes = nodes;
        self.paint();
    }

    /// Nodes → instances. Identity is key + part (+ glyph id), so unchanged things keep their birth time.
    fn paint(&mut self) {
        self.scene.clear();
        self.owners.clear();
        let s = self.scale;
        let dev = |r: Rect| {
            let (x0, y0, x1, y1) = ((r.x * s).round(), (r.y * s).round(), (r.right() * s).round(), (r.bottom() * s).round());
            [x0, y0, x1 - x0, y1 - y0]
        };
        let mut births = HashMap::with_capacity(self.births.len());
        let mut held = vec![false; self.nodes.len()];
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            held[i] = self.pressed.as_deref() == Some(n.key.as_str()) || n.parent.is_some_and(|p| held[p]);
            let seen = n.rect.intersect(&n.clip);
            if seen.w <= 0.0 || seen.h <= 0.0 { continue; }
            let c = n.clip;
            let clip = [c.x * s, c.y * s, c.right() * s, c.bottom() * s];
            let loose = if held[i] { 1.0 } else { 0.0 };
            let mut put = |scene: &mut Scene, owners: &mut Vec<(usize, u64)>, mut inst: Instance, id: u64| {
                let born = *births.entry(id).or_insert_with(|| self.births.get(&id).copied().unwrap_or(self.now));
                inst.clip = clip;
                inst.extra[2] = loose;
                inst.extra[3] = born;
                scene.push(inst);
                owners.push((i, id));
            };
            let shape = |p: Paint, bw: f32| Instance { rect: dev(n.rect), uv: [n.radius * s, 0.0, 0.0, 0.0], paint: [p.tone, p.accent, p.alpha, SHAPE], clip, extra: [bw * s, 0.0, 0.0, 0.0] };
            if let Some(p) = n.fill { put(&mut self.scene, &mut self.owners, shape(p, 0.0), ident(&n.key, u32::MAX, 0)); }
            if let Some((w, p)) = n.border { put(&mut self.scene, &mut self.owners, shape(p, w), ident(&n.key, u32::MAX - 1, 0)); }
            match &n.content {
                Content::Text { s: str, size, line, paint } => {
                    let proto = Instance { paint: [paint.tone, paint.accent, paint.alpha, 0.0], ..Default::default() };
                    let para = Para { s: str, size: *size, line: *line, width: n.inner.w + 1.0 };
                    let mut seq = 0;
                    let (scene, owners) = (&mut self.scene, &mut self.owners);
                    self.text.emit(para, (n.inner.x, n.inner.y), proto, s, |inst, glyph| {
                        put(scene, owners, inst, ident(&n.key, seq, glyph));
                        seq += 1;
                    });
                }
                Content::Picture { id, .. } => {
                    let inst = Instance { rect: dev(n.inner), uv: [0.0, 0.0, 1.0, 1.0], paint: [1.0, 0.0, 1.0, PICTURE], clip, extra: [0.0, *id as f32, 0.0, 0.0] };
                    put(&mut self.scene, &mut self.owners, inst, ident(&n.key, u32::MAX - 2, *id));
                }
                Content::None => {}
            }
        }
        self.births = births;
        self.page_len = self.scene.instances.len();
        self.paint_marks();
    }

    fn paint_marks(&mut self) {
        self.scene.truncate(self.page_len);
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
}

fn ident(key: &str, part: u32, glyph: u32) -> u64 {
    let mut h = DefaultHasher::new();
    (key, part, glyph).hash(&mut h);
    h.finish()
}

/// Move an element tree into flat nodes (paint order) and a taffy tree. Unkeyed nodes are keyed by path.
fn flatten<M>(el: El<M>, parent: Option<usize>, path: String, tree: &mut TaffyTree<usize>, nodes: &mut Vec<Node<M>>) -> NodeId {
    let El { key, style, content, fill, border, radius, on_press, on_key, on_blur, focusable, scroll, children } = el;
    let key = if key.is_empty() { path } else { key };
    let i = nodes.len();
    let leaf = !matches!(content, Content::None);
    let placeholder = NodeId::from(0u64);
    nodes.push(Node {
        key, parent, rect: Rect::default(), clip: Rect::default(), inner: Rect::default(), scroll, focusable, extent: 0.0,
        content, fill, border, radius, on_press, on_key, on_blur, tid: placeholder,
    });
    let kids: Vec<NodeId> = children.into_iter().enumerate().map(|(n, c)| {
        let p = format!("{}/{n}", nodes[i].key);
        flatten(c, Some(i), p, tree, nodes)
    }).collect();
    let tid = if leaf { tree.new_leaf_with_context(style, i) } else { tree.new_with_children(style, &kids) }.expect("taffy node");
    nodes[i].tid = tid;
    tid
}
