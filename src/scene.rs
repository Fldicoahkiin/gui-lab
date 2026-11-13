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
}

/// A run of instances drawn with one picture bound. Glyphs and shapes do not care which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch { pub start: u32, pub end: u32, pub picture: u32 }

#[derive(Default)]
pub struct Scene {
    pub instances: Vec<Instance>,
    pub batches: Vec<Batch>,
}

impl Scene {
    pub fn clear(&mut self) { self.instances.clear(); self.batches.clear(); }
    pub fn push(&mut self, i: Instance) {
        let n = self.instances.len() as u32;
        let picture = if i.paint[3] == PICTURE { Some(i.extra[1] as u32) } else { None };
        match self.batches.last_mut() {
            Some(b) if picture.is_none_or(|p| p == b.picture) => b.end = n + 1,
            _ => self.batches.push(Batch { start: n, end: n + 1, picture: picture.unwrap_or(0) }),
        }
        self.instances.push(i);
    }
    /// Drop everything from instance `n` on.
    pub fn truncate(&mut self, n: usize) {
        self.instances.truncate(n);
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
    pub next: usize,
}
impl Default for Field {
    fn default() -> Self { Self { fronts: [[0.0; 4]; 8], pointer: [-1e5, -1e5, 0.0, 0.0], wipe: [0.0; 4], wipe_rect: [0.0; 4], next: 0 } }
}
impl Field {
    pub fn front(&mut self, x: f32, y: f32, t: f32, strength: f32) {
        self.fronts[self.next] = [x, y, t, strength];
        self.next = (self.next + 1) % self.fronts.len();
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
    /// Pointer lens over pictures: radius (logical px).
    pub lens: f32,
    /// How long a new instance takes to condense (seconds).
    pub enter: f32,
    pub square: bool,
}
impl Default for Fx {
    fn default() -> Self {
        Self { pitch: 5.0, jitter: 0.0, glitch: 0.4, wave_speed: 760.0, wave_life: 1.4, wave_width: 26.0, wave_tail: 150.0, push: 5.0, lens: 84.0, enter: 0.45, square: false }
    }
}
