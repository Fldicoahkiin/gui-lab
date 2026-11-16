//! Experiments (the Lab page, DESIGN.md §5.3): small live pieces, each one thing, written against the
//! public API as any application would (`examples/lab/`). What they share is here and nowhere else:
//! the cell's frame (name, hint, the piece, the formula with its live numbers, a label), numbered
//! parameters with their caption, a pointer that springs after the real one, and a trait the gallery
//! and the standalone runtime drive them through. They step only while seen (`El::on_visible`).
use crate::el::{El, Font, Paint, Point, col, row, space, text};
use crate::gesture::{Animated, Spring};
use crate::widgets::{CaptionParam, Edit, Spec, caption};

/// One experiment: state that draws itself, hears its messages, and steps with the clock.
pub trait Experiment: Default + Clone + 'static {
    type Msg: Clone + std::fmt::Debug + PartialEq + 'static;
    /// The whole cell (`frame`), keyed under `key`.
    fn view(&self, key: &str) -> El<Self::Msg>;
    fn update(&mut self, m: Self::Msg, now: f32);
    /// Advance to `now` (s); whether anything changed. The host calls it while the cell can be seen
    /// and either input came less than `IDLE_S` ago (its idle motion: drift, growth) or it is still
    /// settling (`moving`).
    fn step(&mut self, now: f32) -> bool;
    /// Whether it is still settling by itself (a spring, a coast): frames until it rests.
    fn moving(&self) -> bool;
}
/// How long after the last input (or arriving) a cell's idle motion runs before the page comes to rest.
pub const IDLE_S: f32 = 1.2;

/// The cell, in the lab's look: `[ NAME ]` and the hint on top, the piece, the formula line printed
/// from the numbers that drive it with those numbers as controls, and a small label.
#[allow(clippy::too_many_arguments)]
pub fn frame<M: Clone + 'static>(key: &str, name: &str, hint: &str, piece: El<M>, formula: String, params: &[CaptionParam], msg: impl Fn(usize, Edit) -> M + 'static, label: &str) -> El<M> {
    col().key(key.to_owned()).fill_width().min_width(0.0).pad(14.0, 12.0).gap(10.0).border(1.0, Paint::FAINT)
        .child(row().fill_width().gap(8.0)
            .child(text(format!("[ {} ]", name.to_uppercase()), 11.0, Paint::TEXT).font(Font::Mono).key(format!("{key}/name")))
            .child(space(0.0, 0.0).grow(1.0))
            .child(text(hint, 11.0, Paint::DIM).font(Font::Mono).key(format!("{key}/hint"))))
        .child(piece)
        .child(caption(&format!("{key}/cap"), formula, params, msg))
        .child(text(label, 11.0, Paint::FAINT).font(Font::Mono).key(format!("{key}/label")))
}

/// Numbered parameters: their names and ranges, their values, and what is being typed into one.
#[derive(Clone, Debug, PartialEq)]
pub struct Params { pub defs: &'static [(&'static str, Spec)], pub v: Vec<f32>, pub draft: Option<(usize, String)> }
impl Params {
    pub fn new(defs: &'static [(&'static str, Spec)], v: &[f32]) -> Self { Self { defs, v: v.to_vec(), draft: None } }
    pub fn get(&self, i: usize) -> f32 { self.v[i] }
    pub fn edit(&mut self, i: usize, e: Edit) {
        match e {
            Edit::Draft(s) => self.draft = Some((i, s)),
            Edit::Set(x) => { self.v[i] = self.defs[i].1.clamp(x); self.draft = None; }
            Edit::Cancel => self.draft = None,
        }
    }
    /// The caption's controls.
    pub fn caption(&self) -> Vec<CaptionParam> {
        self.defs.iter().enumerate().map(|(i, (n, s))| CaptionParam::new(*n, self.v[i], *s).draft(self.draft.as_ref().filter(|d| d.0 == i).map(|d| d.1.as_str()))).collect()
    }
}
/// A parameter's range.
pub const fn spec(min: f32, max: f32, step: f32, decimals: usize, unit: &'static str) -> Spec { Spec { min, max, step, decimals, unit } }

/// The pointer as a piece sees it: a point that springs after the real one (so what follows it moves
/// smoothly), how fast the real one moves, whether it is over the piece or pressing, and `near`, which
/// rises to 1 while it is there and falls to 0 after it leaves (what the accent follows).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer { pub x: Animated, pub y: Animated, pub over: bool, pub down: bool, pub vx: f32, pub vy: f32, pub near: f32, last: (f32, f32, f32), t: f32 }
impl Pointer {
    /// Resting at (x, y); `stiffness` and `damping` of the follow.
    pub fn new(x: f32, y: f32, stiffness: f32, damping: f32) -> Self {
        let s = Spring { stiffness, damping, rest_distance: 0.05, rest_speed: 0.5, ..Spring::default() };
        Self { x: Animated::new(x, s), y: Animated::new(y, s), over: false, down: false, vx: 0.0, vy: 0.0, near: 0.0, last: (x, y, 0.0), t: 0.0 }
    }
    /// The pointer moved, pressed, dragged (`Some`) or left (`None`) at `now`.
    pub fn hear(&mut self, p: Option<Point>, now: f32) {
        match p {
            Some(p) => {
                let dt = (now - self.last.2).max(1.0 / 240.0);
                if self.over { (self.vx, self.vy) = ((p.x - self.last.0) / dt, (p.y - self.last.1) / dt); }
                self.last = (p.x, p.y, now);
                self.over = true;
                self.down = p.down;
                let (vx, vy) = (self.x.velocity, self.y.velocity);
                self.x.to(p.x, vx, now);
                self.y.to(p.y, vy, now);
            }
            None => { self.over = false; self.down = false; }
        }
    }
    /// Advance the follow, the speed's decay and `near`; whether anything changed.
    pub fn step(&mut self, now: f32) -> bool {
        let dt = (now - self.t).clamp(0.0, 0.1);
        self.t = now;
        let mut moved = self.x.step(now) | self.y.step(now);
        let k = (-dt / 0.12).exp();
        if self.vx != 0.0 || self.vy != 0.0 { self.vx *= k; self.vy *= k; if self.vx.abs() + self.vy.abs() < 1.0 { self.vx = 0.0; self.vy = 0.0; } moved = true; }
        let goal = if self.over { 1.0 } else { 0.0 };
        if self.near != goal { self.near += (goal - self.near) * (1.0 - (-dt / 0.25).exp()); if (self.near - goal).abs() < 0.01 { self.near = goal; } moved = true; }
        moved
    }
    pub fn moving(&self) -> bool { self.x.moving() || self.y.moving() || self.vx != 0.0 || self.vy != 0.0 || (self.near > 0.0 && !self.over) || (self.near < 1.0 && self.over) }
    /// Where the following point is.
    pub fn at(&self) -> (f32, f32) { (self.x.value, self.y.value) }
}

/// Grey-white ink at `tone`, or the one accent where `a` (0..1) says the pointer reaches.
pub fn ink(tone: f32, a: f32) -> Paint { if a > 0.5 { Paint::ACCENT.alpha(tone.max(0.6)) } else { Paint::tone(tone) } }

/// A standalone runtime for one experiment (the `render` tool, tests): the cell on the lab's ground.
pub fn ui<E: Experiment>(faces: Vec<crate::text::Face>, w: u32, h: u32, scale: f32) -> crate::ui::Ui<Solo<E>, E::Msg> {
    fn view<E: Experiment>(s: &Solo<E>) -> El<E::Msg> { col().fill_width().fill_height().pad(24.0, 24.0).fonts(crate::style::LAB.roles()).child(s.e.view("cell")) }
    fn update<E: Experiment>(s: &mut Solo<E>, m: E::Msg) { let now = s.now; s.until = now + IDLE_S; s.e.update(m, now); }
    crate::ui::Ui::new(Solo::<E> { e: E::default(), now: 0.0, until: IDLE_S }, view::<E>, update::<E>, faces, w, h, scale)
        .on_look(|_| crate::style::Look { theme: crate::style::LAB, ..crate::style::Look::default() })
        .on_tick(|s, t| { s.now = t; (t < s.until || s.e.moving()) && s.e.step(t) })
        .on_wake(|s, t| (t < s.until || s.e.moving()).then_some(t))
}
/// One experiment and the clock, for `ui`.
#[derive(Clone)]
pub struct Solo<E> { pub e: E, pub now: f32, pub until: f32 }
