//! Frame times, counters and the automatic quality level (DESIGN.md §1.12). No GPU here.
use std::collections::VecDeque;

/// Milliseconds on a monotonic clock: `performance.now()` on the web, `Instant` elsewhere.
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    { web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now()) }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0
    }
}

/// What one drawn frame cost on the CPU, ms: updating the state and laying out, building the scene
/// (instances), and recording and submitting the GPU work.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameCost { pub update_layout: f32, pub scene: f32, pub submit: f32 }
impl FrameCost { pub fn total(&self) -> f32 { self.update_layout + self.scene + self.submit } }

/// Work counters, for tests and the readout: they only go up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Full layout passes (taffy runs over the tree), and passes that found nothing to lay out.
    pub layouts: u64,
    pub layouts_skipped: u64,
    /// Paragraphs shaped and broken into lines by parley (a cache miss).
    pub shaped: u64,
    /// Views built (the application's `view` called).
    pub views: u64,
    /// Scenes built from the laid-out nodes.
    pub scenes: u64,
}

/// The readout's numbers. The runtime adds update / layout and scene time as it spends it; the
/// renderer adds the submit time and closes the frame.
#[derive(Clone, Debug, Default)]
pub struct Perf {
    pub pending: FrameCost,
    /// The drawn frames of the last two seconds: when (ms) and what they cost.
    pub frames: VecDeque<(f64, FrameCost)>,
    pub drawn: u64,
    pub skipped: u64,
    pub counters: Counters,
    /// What the host set: the graphics backend's name and the device pixel ratio.
    pub backend: String,
    /// The material resolution the last frame used, and whether it was chosen automatically.
    pub quality: f32,
    pub auto: bool,
    pub particles: u32,
}
impl Perf {
    /// A frame was drawn now: what was spent since the last one is its cost.
    pub fn frame(&mut self, submit_ms: f32) {
        let mut c = std::mem::take(&mut self.pending);
        c.submit = submit_ms;
        let now = now_ms();
        self.frames.push_back((now, c));
        while self.frames.front().is_some_and(|(t, _)| now - t > 2000.0) { self.frames.pop_front(); }
        self.drawn += 1;
    }
    /// Frames drawn in the last second (to `now` ms), and the last, mean and largest cost of each part.
    pub fn summary(&self, now: f64) -> Summary {
        let last: Vec<FrameCost> = self.frames.iter().filter(|(t, _)| now - t <= 1000.0).map(|f| f.1).collect();
        let n = last.len();
        let part = |f: fn(&FrameCost) -> f32| -> [f32; 3] {
            if n == 0 { return [0.0; 3]; }
            let v: Vec<f32> = last.iter().map(f).collect();
            [v[n - 1], v.iter().sum::<f32>() / n as f32, v.iter().copied().fold(0.0, f32::max)]
        };
        Summary { fps: n as u32, update_layout: part(|c| c.update_layout), scene: part(|c| c.scene), submit: part(|c| c.submit), total: part(FrameCost::total) }
    }
}

/// Last, mean, max (ms) over the frames of the last second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary { pub fps: u32, pub update_layout: [f32; 3], pub scene: [f32; 3], pub submit: [f32; 3], pub total: [f32; 3] }

/// The quality levels of the material pass: its resolution as a fraction of the device's.
pub const LEVELS: [f32; 3] = [1.0, 0.75, 0.5];

/// Automatic quality: one level down when the mean frame time has been over `BAD_MS` for `BAD_RUN`
/// frames in a row, one level up after `GOOD_RUN` frames in a row under it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AutoQuality { pub level: usize, bad: u32, good: u32, mean: f32 }
pub const BAD_MS: f32 = 12.0;
pub const BAD_RUN: u32 = 30;
pub const GOOD_RUN: u32 = 120;
impl AutoQuality {
    pub fn quality(&self) -> f32 { LEVELS[self.level.min(LEVELS.len() - 1)] }
    /// One more drawn frame that cost `ms`; returns the quality for the next one.
    pub fn feed(&mut self, ms: f32) -> f32 {
        // A short running mean, so one slow frame (a new glyph, a morph's first picture) does not count.
        self.mean = if self.mean == 0.0 { ms } else { self.mean * 0.8 + ms * 0.2 };
        if self.mean > BAD_MS { self.bad += 1; self.good = 0; } else { self.good += 1; self.bad = 0; }
        if self.bad >= BAD_RUN && self.level + 1 < LEVELS.len() { self.level += 1; self.bad = 0; }
        if self.good >= GOOD_RUN && self.level > 0 { self.level -= 1; self.good = 0; }
        self.quality()
    }
}
