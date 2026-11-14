//! Scene: what the renderer draws. Everything on screen is an instance of one of three kinds, and every
//! instance carries the same small record, so they all go through the same two stages.

pub const GLYPH: f32 = 0.0;
pub const SHAPE: f32 = 1.0;
pub const PICTURE: f32 = 2.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    /// Device px. Glyphs include the spread padding; shapes and pictures are padded by the renderer.
    pub rect: [f32; 4],
    /// Glyphs and pictures: texture coordinates x0, y0, x1, y1. Shapes: corner radius (device px) in x.
    pub uv: [f32; 4],
    /// tone, accent, alpha, kind
    pub paint: [f32; 4],
    /// Visible region, device px: x0, y0, x1, y1.
    pub clip: [f32; 4],
    /// Shapes: border width (device px, 0 = filled). Pictures: picture id in y. Glyphs: print pitch
    /// (logical px, 0 = off) in y. All: static looseness, time the instance first appeared.
    /// Pictures carry their `PicFx` slot in `paint[1]`.
    pub extra: [f32; 4],
    /// Material override code (0 = the global material; else `128 + level × 16 + id`, see
    /// `style::override_code`), accent hue (0, 1, 2), and the box the material's height treatments
    /// measure against: top and height, device px (a text line, or the shape itself).
    pub look: [f32; 4],
    /// How the field may move it (DESIGN.md §1.15): the most looseness fronts may add (0 for `Ignore`
    /// and `Nudge`, `Look::legible_cap` for legible `Loosen`, else 1), whether fronts push it (0 / 1),
    /// and two spare numbers.
    pub resp: [f32; 4],
}

/// A run of instances drawn with one picture bound. Glyphs and shapes do not care which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch { pub start: u32, pub end: u32, pub picture: u32 }

#[derive(Clone, Default)]
pub struct Scene {
    pub instances: Vec<Instance>,
    pub batches: Vec<Batch>,
    /// Where the modal layer starts, if one is open. Everything from here on is drawn over the page
    /// and hides it in the near image too (see the renderer).
    pub layer: Option<u32>,
    /// Stage 3, the annotation layer (DESIGN.md §1.7): drawn after the material, exactly, never loose.
    pub annot: Vec<Instance>,
}

impl Scene {
    pub fn clear(&mut self) { self.instances.clear(); self.batches.clear(); self.layer = None; self.annot.clear(); }
    /// Everything pushed from now on is in the modal layer.
    pub fn begin_layer(&mut self) { self.layer = Some(self.instances.len() as u32); }
    pub fn push(&mut self, i: Instance) {
        let n = self.instances.len() as u32;
        let picture = if i.paint[3] == PICTURE { Some(i.extra[1] as u32) } else { None };
        // A batch never straddles the start of the layer, so the renderer can draw the two apart.
        match self.batches.last_mut() {
            Some(b) if picture.is_none_or(|p| p == b.picture) && self.layer != Some(n) => b.end = n + 1,
            _ => self.batches.push(Batch { start: n, end: n + 1, picture: picture.unwrap_or(0) }),
        }
        self.instances.push(i);
    }
    /// Drop everything from instance `n` on.
    pub fn truncate(&mut self, n: usize) {
        self.instances.truncate(n);
        if self.layer.is_some_and(|l| l as usize >= n) { self.layer = None; }
        self.batches.retain(|b| (b.start as usize) < n);
        if let Some(b) = self.batches.last_mut() { b.end = b.end.min(n as u32); }
    }
}

/// What disturbs the page right now: fronts started by presses, and the pointer. Device px.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// x, y, start time, strength
    pub fronts: [[f32; 4]; 8],
    /// x, y, strength, _
    pub pointer: [f32; 4],
    /// A half-plane source (the compare wipe): x of the line, strength, _, _. Right of the line the
    /// disturbance is `strength`, left of it nothing. It acts only inside `wipe_rect` (x0, y0, x1, y1).
    pub wipe: [f32; 4],
    pub wipe_rect: [f32; 4],
    /// A short burst over a rect (x0, y0, x1, y1), and when it started, how strong, how long (s).
    /// Hover and press put one over the control; the glitch material reads it.
    pub burst: [f32; 4],
    pub burst_t: [f32; 4],
    /// Up to four boxes (x0, y0, x1, y1, device px) that fronts pass by (`El::calm`).
    pub calm: [[f32; 4]; 4],
    /// Each front's scope (DESIGN.md §1.15): the box (x0, y0, x1, y1, device px) whose instances it
    /// reaches, by their centre; and the element that sent it (0: none), so a new press from the same
    /// element restarts its front instead of adding one.
    pub scopes: [[f32; 4]; 8],
    pub sources: [u64; 8],
    pub next: usize,
}
/// The scope box of a global front: everything.
pub const EVERYWHERE: [f32; 4] = [-1e6, -1e6, 1e6, 1e6];
impl Default for Field {
    fn default() -> Self { Self { fronts: [[0.0; 4]; 8], pointer: [-1e5, -1e5, 0.0, 0.0], wipe: [0.0; 4], wipe_rect: [0.0; 4], burst: [0.0; 4], burst_t: [0.0, 0.0, 0.0, 0.0], calm: [[0.0; 4]; 4], scopes: [EVERYWHERE; 8], sources: [0; 8], next: 0 } }
}
impl Field {
    /// A front over the whole window, from no element in particular.
    pub fn front(&mut self, x: f32, y: f32, t: f32, strength: f32) { self.front_in(x, y, t, strength, EVERYWHERE, 0, f32::INFINITY); }
    /// A front that reaches only instances whose centre is inside `scope` (device px box), sent by
    /// `source` (0: none). While a front from the same source is still alive (younger than `life`
    /// seconds) it is restarted from here, not added to: fronts from one element never stack.
    pub fn front_in(&mut self, x: f32, y: f32, t: f32, strength: f32, scope: [f32; 4], source: u64, life: f32) {
        let alive = (0..self.fronts.len()).find(|&j| source != 0 && self.sources[j] == source && self.fronts[j][3] > 0.0 && t - self.fronts[j][2] < life);
        let j = match alive { Some(j) => j, None => { let j = self.next; self.next = (self.next + 1) % self.fronts.len(); j } };
        self.fronts[j] = [x, y, t, strength];
        self.scopes[j] = scope;
        self.sources[j] = source;
    }
    /// How strongly the fronts disturb device point `p` at time `t`, for an instance centred at `c`
    /// (only fronts whose scope holds `c` count): the shader's `wave`, on the CPU, for tests and the tree
    /// export. `tail_gain` as in the shader (0.6 for text and shapes).
    pub fn wave_at(&self, p: [f32; 2], c: [f32; 2], t: f32, fx: &Fx, scale: f32, tail_gain: f32) -> f32 {
        if self.calm.iter().any(|r| r[2] > r[0] && p[0] >= r[0] && p[1] >= r[1] && p[0] < r[2] && p[1] < r[3]) { return 0.0; }
        let (w, tail) = (fx.wave_width.max(1e-3) * scale, fx.wave_tail.max(1e-3) * scale);
        let mut k: f32 = 0.0;
        for (j, f) in self.fronts.iter().enumerate() {
            let age = t - f[2];
            if f[3] <= 0.0 || age < 0.0 { continue; }
            let sc = self.scopes[j];
            if c[0] < sc[0] || c[1] < sc[1] || c[0] >= sc[2] || c[1] >= sc[3] { continue; }
            let d = ((p[0] - f[0]).powi(2) + (p[1] - f[1]).powi(2)).sqrt() - age * fx.wave_speed * scale;
            let life = fx.wave_life.max(1e-3);
            let x = (age / life).clamp(0.0, 1.0);
            let strength = (1.0 - x * x * (3.0 - 2.0 * x)) * f[3];
            let front = (-(d * d) / (w * w)).exp();
            let behind = if d < 0.0 { (d / tail).exp() * tail_gain } else { 0.0 };
            k = k.max(front.max(behind) * strength);
        }
        k
    }
    /// A burst over `rect` (device px, x0 y0 x1 y1, or the whole window when `None`) from time `t`,
    /// `strength` 0..1, lasting `secs`.
    pub fn burst(&mut self, rect: Option<[f32; 4]>, t: f32, strength: f32, secs: f32) {
        self.burst = rect.unwrap_or([-1e6, -1e6, 1e6, 1e6]);
        self.burst_t = [t, strength, secs.max(1e-3), 0.0];
    }
    /// How strong the burst is at time `t` (it fades out linearly).
    pub fn burst_at(&self, t: f32) -> f32 {
        let age = t - self.burst_t[0];
        if age < 0.0 || age > self.burst_t[2] { 0.0 } else { self.burst_t[1] * (1.0 - age / self.burst_t[2]) }
    }
}

/// Field and material parameters. Every one is a plain number so it can be set exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fx {
    /// Lattice pitch for text and shapes (logical px). Pictures have their own, in `PicFx`.
    pub pitch: f32,
    /// How far each unit may sit off the lattice, 0..1 of the pitch.
    pub jitter: f32,
    /// Sideways slip of thin bands while a front passes, 0 = none.
    pub glitch: f32,
    /// Front speed (logical px per second), how long a front lasts (seconds), its width and the length
    /// of the loosened trail behind it (logical px).
    pub wave_speed: f32,
    pub wave_life: f32,
    pub wave_width: f32,
    pub wave_tail: f32,
    /// How far instances are pushed as a front passes under them (logical px).
    pub push: f32,
    /// Pointer lens over pictures: radius (logical px), and how hard its edge is (0: a soft bell, as
    /// loose as it is near the centre; 1: fully loose inside the radius, not at all outside).
    pub lens: f32,
    pub lens_hard: f32,
    /// How long a new instance takes to condense (seconds).
    pub enter: f32,
    pub square: bool,
}
impl Default for Fx {
    fn default() -> Self {
        Self { pitch: 5.0, jitter: 0.0, glitch: 0.4, wave_speed: 760.0, wave_life: 1.4, wave_width: 26.0, wave_tail: 150.0, push: 5.0, lens: 84.0, lens_hard: 0.0, enter: 0.45, square: false }
    }
}
