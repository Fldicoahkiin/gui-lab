//! Driving a page from outside, deterministically (DESIGN.md §1.17): the `render` tool and the tests open
//! a gallery page or a recipe, play a list of input events on a fixed 60 Hz clock, and read back one
//! frame (PNG) and the laid-out tree (JSON). Same arguments, same bytes.
use crate::error::{DriveError, LookError, SurfaceError, ViewProblem};
use crate::el::{Key, Mods};
use crate::offscreen::{Offscreen, png_bytes_with};
use crate::style::{Look, Material, THEMES};
use crate::tree::TreeNode;
use crate::ui::{PointerKind, Ui};
use serde::Deserialize;

/// One input event of a script, at `at_ms` on the page's clock (which starts at 0).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Event {
    pub at_ms: f32,
    #[serde(flatten)]
    pub input: Input,
}

/// What happens. A pointer event names a point (`x`, `y`, logical px) or an element (`target`, its key:
/// the centre of its box).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Input {
    Press { x: Option<f32>, y: Option<f32>, target: Option<String>, #[serde(default)] shift: bool },
    Release { x: Option<f32>, y: Option<f32>, target: Option<String> },
    Move { x: Option<f32>, y: Option<f32>, target: Option<String> },
    /// A key by its DOM name ("Enter", "ArrowUp", "a", " ", "Tab", "Escape", "F2").
    Key { key: String, #[serde(default)] shift: bool, #[serde(default)] cmd: bool, #[serde(default)] alt: bool },
    Wheel { x: Option<f32>, y: Option<f32>, target: Option<String>, #[serde(default)] dx: f32, #[serde(default)] dy: f32 },
    /// Text typed into the focused field.
    Text { text: String },
}

/// Read a script: a JSON list of events.
pub fn parse_events(json: &str, source: &str) -> Result<Vec<Event>, DriveError> {
    serde_json::from_str(json).map_err(|e| DriveError::BadEvents { file: source.to_owned(), message: e.to_string() })
}

/// A page that can be driven and drawn, whatever its state and message types.
pub trait Driven {
    fn now(&self) -> f32;
    fn tick(&mut self, t: f32);
    fn apply(&mut self, input: &Input) -> Result<(), DriveError>;
    fn look(&self) -> Look;
    fn tree(&self) -> TreeNode;
    fn problems(&self) -> Vec<ViewProblem>;
    fn needs_frame(&self, t: f32) -> bool;
    fn draw(&mut self, o: &mut Offscreen, look: &Look) -> Vec<u8>;
    /// The look as a recipe string (`Preset::recipe`), when the page has one.
    fn recipe(&self) -> Option<String> { None }
}
/// A gallery page: driven as any page, and its preset is its recipe.
struct Gallery(crate::gallery::GalleryUi);
impl Driven for Gallery {
    fn now(&self) -> f32 { self.0.now() }
    fn tick(&mut self, t: f32) { self.0.tick(t) }
    fn apply(&mut self, input: &Input) -> Result<(), DriveError> { self.0.apply(input) }
    fn look(&self) -> Look { Driven::look(&self.0) }
    fn tree(&self) -> TreeNode { self.0.export_tree() }
    fn problems(&self) -> Vec<ViewProblem> { self.0.problems() }
    fn needs_frame(&self, t: f32) -> bool { self.0.needs_frame(t) }
    fn draw(&mut self, o: &mut Offscreen, look: &Look) -> Vec<u8> { o.render(&mut self.0, look) }
    fn recipe(&self) -> Option<String> { Some(self.0.state.recipe()) }
}

impl<S, M: Clone> Driven for Ui<S, M> {
    fn now(&self) -> f32 { Ui::now(self) }
    fn tick(&mut self, t: f32) { Ui::tick(self, t) }
    fn look(&self) -> Look { Ui::look(self) }
    fn tree(&self) -> TreeNode { self.export_tree() }
    fn problems(&self) -> Vec<ViewProblem> { Ui::problems(self) }
    fn needs_frame(&self, t: f32) -> bool { Ui::needs_frame(self, t) }
    fn draw(&mut self, o: &mut Offscreen, look: &Look) -> Vec<u8> { o.render(self, look) }
    fn apply(&mut self, input: &Input) -> Result<(), DriveError> {
        let at = |ui: &Self, x: &Option<f32>, y: &Option<f32>, target: &Option<String>| -> Result<(f32, f32), DriveError> {
            if let Some(k) = target {
                return ui.node(k).map(|n| n.rect.center()).ok_or_else(|| DriveError::UnknownTarget { key: k.clone(), near: near_keys(ui, k) });
            }
            match (x, y) { (Some(x), Some(y)) => Ok((*x, *y)), _ => Err(DriveError::NoPosition) }
        };
        match input {
            Input::Press { x, y, target, shift } => { let (x, y) = at(self, x, y, target)?; self.pointer_down(1, PointerKind::Mouse, x, y, *shift); }
            Input::Release { x, y, target } => { let (x, y) = at(self, x, y, target)?; self.pointer_up(1, PointerKind::Mouse, x, y); }
            Input::Move { x, y, target } => { let (x, y) = at(self, x, y, target)?; self.pointer_moved(1, PointerKind::Mouse, x, y); }
            Input::Wheel { x, y, target, dx, dy } => { let (x, y) = at(self, x, y, target)?; self.wheel_xy(x, y, *dx, *dy); }
            Input::Key { key, shift, cmd, alt } => {
                let k = Key::from_name(key).ok_or_else(|| DriveError::UnknownKey { name: key.clone() })?;
                self.key_mods(k, Mods { shift: *shift, cmd: *cmd, alt: *alt });
            }
            Input::Text { text } => { self.text_input(text); }
        }
        Ok(())
    }
}

/// Keys of the page that contain `k` (or its last part), to suggest when a target is not there.
fn near_keys<S, M: Clone>(ui: &Ui<S, M>, k: &str) -> String {
    let tail = k.rsplit('/').next().unwrap_or(k);
    let mut out: Vec<&str> = ui.nodes().iter().filter(|n| n.keyed && (n.key.contains(k) || n.key.ends_with(tail))).map(|n| n.key.as_str()).take(8).collect();
    if out.is_empty() { out = ui.nodes().iter().filter(|n| n.keyed).map(|n| n.key.as_str()).take(8).collect(); }
    out.join(", ")
}

pub use crate::style::{THEME_IDS, material_named as material, theme_named as theme};

/// A page with the look's material and theme replaced (for recipes, whose look comes from their state).
struct Styled { inner: Box<dyn Driven>, material: Option<Material>, theme: Option<usize> }
impl Driven for Styled {
    fn now(&self) -> f32 { self.inner.now() }
    fn tick(&mut self, t: f32) { self.inner.tick(t) }
    fn apply(&mut self, input: &Input) -> Result<(), DriveError> { self.inner.apply(input) }
    fn look(&self) -> Look {
        let mut l = self.inner.look();
        if let Some(m) = self.material { l.material = m; }
        if let Some(t) = self.theme { l.theme = THEMES[t]; }
        l
    }
    fn tree(&self) -> TreeNode { self.inner.tree() }
    fn problems(&self) -> Vec<ViewProblem> { self.inner.problems() }
    fn needs_frame(&self, t: f32) -> bool { self.inner.needs_frame(t) }
    fn draw(&mut self, o: &mut Offscreen, look: &Look) -> Vec<u8> { self.inner.draw(o, look) }
}

/// Open a gallery page (its hash: `button`, `material-dot`, `scene-morph`…) or a recipe (`recipe-form`, or
/// just `form`), at `w` × `h` device px and `scale`, with the material and theme given (else the page's).
pub fn open(page: &str, faces: Vec<crate::text::Face>, w: u32, h: u32, scale: f32, material: Option<Material>, theme: Option<usize>) -> Result<Box<dyn Driven>, LookError> {
    open_in(page, faces, (w, h, scale), (material, theme, None))
}
/// `open` with the look and the language given together: (material, theme, locale). Recipes keep
/// their own language.
pub fn open_in(page: &str, faces: Vec<crate::text::Face>, (w, h, scale): (u32, u32, f32), (material, theme, locale): (Option<Material>, Option<usize>, Option<crate::i18n::Locale>)) -> Result<Box<dyn Driven>, LookError> {
    use crate::gallery::{self, Msg, Page};
    if let Some(p) = Page::from_hash(page) {
        let mut ui = gallery::new_ui(faces, w, h, scale);
        ui.pointer_leave();
        if let Some(l) = locale { gallery::update(&mut ui.state, Msg::Locale(l)); }
        gallery::update(&mut ui.state, Msg::Go(p));
        if let Some(m) = material { gallery::update(&mut ui.state, Msg::Material(m.id() as usize)); }
        if let Some(t) = theme { gallery::update(&mut ui.state, Msg::Theme(t)); }
        ui.relayout();
        return Ok(Box::new(Gallery(ui)));
    }
    let name = page.strip_prefix("recipe-").unwrap_or(page);
    match crate::recipes::open(name, faces, w, h, scale) {
        Some(inner) => Ok(Box::new(Styled { inner, material, theme })),
        None => Err(LookError::UnknownPage { given: page.to_owned(), recipes: crate::recipes::NAMES.iter().map(|n| format!("recipe-{n}")).collect::<Vec<_>>().join(", ") }),
    }
}

/// Play `events` on a fixed 60 Hz clock from 0 to `at_ms` (each event is applied on the first frame at
/// or after its time, in the order given), then tick exactly at `at_ms`.
pub fn play(d: &mut dyn Driven, events: &[Event], at_ms: f32) -> Result<(), DriveError> {
    let end = (at_ms.max(0.0)) / 1000.0;
    let mut order: Vec<&Event> = events.iter().collect();
    order.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    let mut next = 0;
    let mut k: u32 = 0;
    loop {
        let t = (k as f32 / 60.0).min(end);
        while next < order.len() && order[next].at_ms / 1000.0 <= t + 1e-6 {
            d.tick(t);
            d.apply(&order[next].input).map_err(|e| DriveError::AtEvent { index: next, inner: Box::new(e) })?;
            next += 1;
        }
        d.tick(t);
        if t >= end { break; }
        k += 1;
    }
    // Events after the end are not played.
    Ok(())
}

/// What one `render` call asks for.
#[derive(Clone, Debug)]
pub struct RenderSpec {
    pub page: String,
    pub w: u32,
    pub h: u32,
    pub scale: f32,
    pub material: Option<String>,
    pub theme: Option<String>,
    pub at_ms: f32,
    pub events: Vec<Event>,
    /// The language (`en`, `zh-Hans`), else the page's own (DESIGN.md §1.20).
    pub locale: Option<String>,
}
/// The result of a render: the PNG bytes, the tree, and what the view got wrong.
pub struct Rendered { pub png: Vec<u8>, pub tree: TreeNode, pub problems: Vec<ViewProblem> }

/// Open, play, draw: the whole `render` tool as a function.
pub fn render(spec: &RenderSpec, faces: Vec<crate::text::Face>, o: Option<&mut Offscreen>) -> Result<Rendered, DriveError> {
    let material = spec.material.as_deref().map(material).transpose()?;
    let theme = spec.theme.as_deref().map(theme).transpose()?;
    if spec.w == 0 || spec.h == 0 { return Err(SurfaceError::ZeroSize { w: spec.w, h: spec.h }.into()); }
    let locale = match spec.locale.as_deref() { Some(t) => Some(crate::i18n::Locale::from_tag(t).ok_or_else(|| LookError::UnknownLocale { given: t.to_owned(), known: crate::i18n::Locale::ALL.iter().map(|l| l.tag()).collect::<Vec<_>>().join(", ") })?), None => None };
    let mut d = open_in(&spec.page, faces, (spec.w, spec.h, spec.scale.max(0.25)), (material, theme, locale))?;
    play(d.as_mut(), &spec.events, spec.at_ms)?;
    let look = d.look();
    let mut own;
    let o = match o { Some(o) => o, None => { own = Offscreen::try_new(spec.w, spec.h)?; &mut own } };
    if (o.w, o.h) != (spec.w, spec.h) { return Err(SurfaceError::SizeMismatch { w: spec.w, h: spec.h, have_w: o.w, have_h: o.h }.into()); }
    let px = d.draw(o, &look);
    let recipe = d.recipe();
    let texts: Vec<(&str, &str)> = recipe.as_deref().map(|r| ("amenui-recipe", r)).into_iter().collect();
    Ok(Rendered { png: png_bytes_with(spec.w, spec.h, &px, &texts), tree: d.tree(), problems: d.problems() })
}
