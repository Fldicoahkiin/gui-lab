//! Preparation in the frames' spare time (DESIGN.md §1.23): an application's own one-off work (sampling
//! a shape, building ten thousand marks) as small steps the runtime calls between frames, so the first
//! frame is a readable plain page and the rest arrives over the next ones. `Ui::prepare` adds a job,
//! `Ui::ready` reads its progress; the renderer's own warm-up (`render::Warmup`) reports here too, and
//! the annotation layer shows how ready the page is as a readout (`READY 0.62`), never as a spinner.

/// A job: each call does one small piece of the work on the state and says how much is done, 0..1
/// (1 or more: finished, never called again).
pub type Job<S> = Box<dyn FnMut(&mut S) -> f32>;

/// The jobs in progress and what each has said (by key), and how ready the renderer is.
pub struct Prepare<S> {
    jobs: Vec<(String, Job<S>)>,
    done: std::collections::HashMap<String, f32>,
    /// How much of the time between frames the jobs may take (ms); each job is called at least once
    /// per frame while it runs.
    pub budget_ms: f32,
    pub(crate) render: f32,
}
impl<S> Default for Prepare<S> {
    fn default() -> Self { Self { jobs: vec![], done: Default::default(), budget_ms: 6.0, render: 1.0 } }
}
impl<S> Prepare<S> {
    /// Add (or replace) the job under `key`.
    pub fn add(&mut self, key: String, job: Job<S>) {
        self.jobs.retain(|(k, _)| *k != key);
        self.done.insert(key.clone(), 0.0);
        self.jobs.push((key, job));
    }
    /// How far the job under `key` is (1 for one that finished or was never asked for).
    pub fn ready(&self, key: &str) -> f32 { self.done.get(key).copied().unwrap_or(1.0).clamp(0.0, 1.0) }
    /// The least ready of the jobs and the renderer, 0..1.
    pub fn readiness(&self) -> f32 { self.jobs.iter().map(|(k, _)| self.ready(k)).fold(self.render, f32::min) }
    /// Whether anything is still to do.
    pub fn pending(&self) -> bool { !self.jobs.is_empty() }
    /// Call the jobs in turn, each at least once, then again while there is time left. True when any
    /// job said something new (the view may show it).
    pub fn run(&mut self, state: &mut S) -> bool {
        if self.jobs.is_empty() { return false; }
        let t0 = crate::perf::now_ms();
        let mut first = true;
        let mut changed = false;
        while !self.jobs.is_empty() && (first || ((crate::perf::now_ms() - t0) as f32) < self.budget_ms) {
            for (k, job) in self.jobs.iter_mut() {
                let p = job(state);
                changed |= self.done.get(k.as_str()) != Some(&p);
                self.done.insert(k.clone(), p);
            }
            self.jobs.retain(|(k, _)| self.done.get(k.as_str()).copied().unwrap_or(1.0) < 1.0);
            first = false;
        }
        changed
    }
}
