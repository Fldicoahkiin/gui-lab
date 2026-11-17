//! The warm-up (DESIGN.md §1.23): a renderer made a piece at a time, so the first frame comes at once.
//! `Renderer::begin` makes what plain needs to draw (the main shader, the ink, nearness, material,
//! annotation and copy pipelines); `Warmup::step` makes the rest, in a fixed order and within a time
//! budget per call: the mark atlas a set at a time, the finishing pass, then the reduced-resolution
//! upsample, the morph tiles' and the mesh pass's pipelines. Until the atlas and the finishing pass are
//! made, frames are drawn as plain (every element in plain, no finishing pass), which is the page as it
//! is at rest (DESIGN.md §0 rule 3); the frame after that condenses into the look.
use super::{Atlas, MONO, Renderer, marks};
use crate::style::{Look, Material};
use crate::text::Text;

/// One piece of the warm-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job { MarksBegin, MarksSet, MarksUpload, Post, Up, Tiles, Mesh }

/// How far a warm-up has come: what is made of how much, and what it is making now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress { pub done: u32, pub total: u32, pub stage: Stage }
impl Progress {
    /// Done as a fraction, 0..1.
    pub fn fraction(&self) -> f32 { if self.total == 0 { 1.0 } else { self.done as f32 / self.total as f32 } }
}
/// What the warm-up is making: plain can draw from the start (`Plain`); the look's material is drawn
/// from `Pipelines` on; `Ready` is the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage { Plain, Pipelines, Ready }

/// What a renderer still has to make.
pub(super) struct Pending { jobs: Vec<Job>, done: u32, total: u32, atlas: Option<marks::AtlasBuild>, chars: Option<marks::CharTiles>, cost: [f32; 7] }
impl Pending {
    fn new() -> Self {
        let mut jobs = vec![Job::MarksBegin];
        jobs.extend(std::iter::repeat_n(Job::MarksSet, crate::style::set::COUNT - 1));
        jobs.extend([Job::MarksUpload, Job::Post, Job::Up, Job::Tiles, Job::Mesh]);
        jobs.reverse();
        let total = jobs.len() as u32;
        Self { jobs, done: 0, total, atlas: None, chars: None, cost: [f32::INFINITY; 7] }
    }
    /// Frames are still drawn as plain.
    pub(super) fn cold(&self) -> bool { self.jobs.iter().any(|j| matches!(j, Job::MarksBegin | Job::MarksSet | Job::MarksUpload | Job::Post)) }
    pub(super) fn fraction(&self) -> f32 { self.progress().fraction() }
    fn progress(&self) -> Progress {
        let stage = if self.jobs.is_empty() { Stage::Ready } else if self.cold() { Stage::Plain } else { Stage::Pipelines };
        Progress { done: self.done, total: self.total, stage }
    }
}

/// A renderer warming up (`Renderer::begin`). Draw with `renderer()` from the first frame; call `step`
/// once a frame (or whenever there is time) until it says `Stage::Ready`.
pub struct Warmup { r: Renderer }

impl Renderer {
    /// A renderer that can draw plain at once and makes the rest over the next frames (`Warmup::step`).
    pub fn begin(device: wgpu::Device, queue: wgpu::Queue, out_format: wgpu::TextureFormat, w: u32, h: u32) -> Warmup {
        let mut r = Self::core(device, queue, out_format, w, h);
        r.warm = Some(Pending::new());
        Warmup { r }
    }
    /// Whether it still draws as plain while warming up.
    pub fn warming(&self) -> bool { self.warm.as_ref().is_some_and(Pending::cold) }
}

impl Renderer {
    /// Make the next pieces of a warm-up for up to `budget_ms` (at least one piece a call); see
    /// `Warmup::step`. After the first piece a call goes on only while the next one, as long as the
    /// longest of its kind so far took, still ends within 1.5 × `budget_ms` (a kind not seen yet waits
    /// for the next call). A renderer that is not warming up says `Stage::Ready` at once.
    pub fn warm_step(&mut self, budget_ms: f32, text: &mut Text) -> Progress {
        let t0 = crate::perf::now_ms();
        let mut did = 0;
        loop {
            let Some(p) = self.warm.as_mut() else { return self.warm_progress() };
            let Some(&job) = p.jobs.last() else { self.warm = None; return self.warm_progress() };
            if did > 0 {
                let spent = (crate::perf::now_ms() - t0) as f32;
                if spent >= budget_ms || spent + p.cost[job as usize] > 1.5 * budget_ms { return self.warm_progress(); }
            }
            p.jobs.pop();
            let tj = crate::perf::now_ms();
            let was_cold = p.cold() || matches!(job, Job::MarksBegin | Job::MarksSet | Job::MarksUpload | Job::Post);
            match job {
                Job::MarksBegin => {
                    let face = if text.faces() > MONO as usize { MONO } else { 0 };
                    p.atlas = Some(Atlas::begin(text, face));
                    p.chars = Some(marks::char_tiles(text, face));
                }
                Job::MarksSet => { if let Some(a) = p.atlas.as_mut() { a.step(); } }
                Job::MarksUpload => {
                    if let (Some(a), Some(c)) = (p.atlas.take(), p.chars.take()) { let atlas = a.finish(); self.upload_marks(&atlas, &c); }
                }
                Job::Post => self.ensure_post(),
                Job::Up => self.ensure_up(),
                Job::Tiles => self.ensure_tiles(),
                Job::Mesh => self.ensure_mesh(),
            }
            let p = self.warm.as_mut().expect("still warming");
            p.done += 1;
            did += 1;
            let took = (crate::perf::now_ms() - tj) as f32;
            let c = &mut p.cost[job as usize];
            *c = if c.is_finite() { c.max(took) } else { took };
            if was_cold && !p.cold() { self.condense = true; }
            if p.jobs.is_empty() {
                let end = Progress { done: p.done, total: p.total, stage: Stage::Ready };
                self.warm = None;
                return end;
            }
        }
    }
    /// How far its warm-up has come (`Stage::Ready` when it is not warming up).
    pub fn warm_progress(&self) -> Progress {
        self.warm.as_ref().map_or(Progress { done: 1, total: 1, stage: Stage::Ready }, Pending::progress)
    }
}

impl Renderer {
    /// A host's warm-up for one frame (DESIGN.md §1.9): after the frame is drawn, make the next pieces
    /// within `budget_ms` ([`WARM_MS`] by default) and tell the runtime how far it is
    /// (`Ui::warm`). Nothing to do, and nothing told, once the renderer is warm and the runtime knows.
    /// Returns what the step made it to (`Stage::Ready`, done = total, on the last one).
    pub fn warm_frame<S, M: Clone>(&mut self, ui: &mut crate::ui::Ui<S, M>, budget_ms: f32) -> Progress {
        if self.warm.is_none() { if ui.warm().is_some() { ui.set_warm(None); } return self.warm_progress(); }
        let p = self.warm_step(budget_ms, ui.text_mut());
        ui.set_warm(Some(p));
        p
    }
}

/// How long a host gives the warm-up of each frame, ms.
pub const WARM_MS: f32 = 6.0;

impl Warmup {
    /// Make the next pieces for up to `budget_ms` (at least one piece a call). The mark atlas needs the
    /// text system the runtime draws with (`Ui::text_mut`).
    pub fn step(&mut self, budget_ms: f32, text: &mut Text) -> Progress { self.r.warm_step(budget_ms, text) }
    /// How far it has come.
    pub fn progress(&self) -> Progress { self.r.warm_progress() }
    /// The renderer to draw with (as plain until the atlas and the finishing pass are made).
    pub fn renderer(&mut self) -> &mut Renderer { &mut self.r }
    /// Finish whatever is left at once and hand the renderer over.
    pub fn finish(mut self, text: &mut Text) -> Renderer {
        while self.progress().stage != Stage::Ready { self.step(f32::INFINITY, text); }
        self.r
    }
    /// Hand the renderer over as it is (it goes on drawing as plain while anything is left; step it
    /// with `Renderer::warm_step`).
    pub fn into_renderer(self) -> Renderer { self.r }
}

/// The look a warming renderer draws: plain, without a finishing pass; everything else as given.
pub fn plain(look: &Look) -> Look { Look { material: Material::Plain, post: crate::post::Post::default(), ..*look } }
