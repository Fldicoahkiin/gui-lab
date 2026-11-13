//! The gallery: the main application, written with the framework only. A left rail lists the
//! components, the mark sets and the type; each component has a page with a live preview, every
//! variant, its own numbers and its source. The top bar switches theme and material; the switch is
//! committed from the control that was pressed. The old skeleton demo is one of the pages.
mod pages;

use crate::demo::{self, Demo};
use crate::el::{El, Font, Paint, col, row, space, text};
use crate::style::{Look, MATERIALS, Material, PicFx, THEMES, Theme, set};
use crate::text::Face;
use crate::widgets::{Edit, SELECTED, Spec, TextEdit, Variant, button_as, divider, segmented, vdivider};
use std::rc::Rc;
use taffy::AlignItems;

pub use pages::{COMPONENTS, Comp, ParamDef};

/// Below this width (logical px) the rail becomes a row of tabs under the top bar.
pub const NARROW: f32 = 720.0;
pub const RAIL: f32 = 220.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page { Comp(usize), Marks(u8), Type, Demo }

pub const MARK_HASH: [&str; set::COUNT] = ["chars", "dither", "pixel", "mosaic", "brick", "dots", "cross", "diagonal", "diamond", "lines", "glow", "stroke"];

impl Page {
    /// Every page, in rail order.
    pub fn all() -> Vec<Page> {
        let mut v: Vec<Page> = (0..COMPONENTS.len()).map(Page::Comp).collect();
        v.extend((0..set::COUNT as u8).map(Page::Marks));
        v.push(Page::Type);
        v.push(Page::Demo);
        v
    }
    /// The page's address on the web: plain ASCII words, so a page can be linked.
    pub fn hash(self) -> String {
        match self {
            Page::Comp(i) => COMPONENTS[i].hash.to_owned(),
            Page::Marks(s) => format!("marks-{}", MARK_HASH[s as usize]),
            Page::Type => "type".into(),
            Page::Demo => "demo".into(),
        }
    }
    /// From `#name` or `name`; an empty hash is the first page.
    pub fn from_hash(h: &str) -> Option<Page> {
        let h = h.trim_start_matches('#');
        if h.is_empty() { return Some(Page::Comp(0)); }
        Page::all().into_iter().find(|p| p.hash() == h)
    }
    pub fn name(self) -> &'static str {
        match self {
            Page::Comp(i) => COMPONENTS[i].title,
            Page::Marks(s) => set::NAMES[s as usize],
            Page::Type => "字号与字体",
            Page::Demo => "骨架演示",
        }
    }
}

/// A number on some page, for routing its edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Parameter `i` of component `c`.
    Param(usize, usize),
    /// `PicFx` field `i` of mark set `s` (see `demo::PIC_PARAMS`).
    Pic(u8, usize),
    Wipe,
    /// The live number on the number page.
    Value,
    /// A variant: shown, but its value does not change.
    Shown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Go(Page),
    Theme(usize),
    Material(usize),
    Reset,
    Num(Target, Edit),
    Nothing,
    Press,
    Text(TextEdit),
    DialogText(TextEdit),
    Switch,
    Level(f32),
    Tab(usize),
    Pick(usize),
    Row(usize),
    Dialog(u8),
    Show(u32),
    Wipe(f32),
    Demo(demo::Msg),
}

pub struct Gallery {
    pub page: Page,
    pub theme: usize,
    pub material: Material,
    /// Each component's own numbers, in the order of its `params`.
    pub params: Vec<Vec<f32>>,
    pub edit: Option<(Target, String)>,
    pub presses: u32,
    pub value: f32,
    pub text: TextEdit,
    pub dialog_text: TextEdit,
    pub on: bool,
    pub level: f32,
    pub tab: usize,
    pub pick: usize,
    pub row: Option<usize>,
    /// Which dialog is open (0 = none).
    pub dialog: u8,
    /// One set of mark parameters per mark page, the picture they show, and the compare line.
    pub marks: [PicFx; set::COUNT],
    pub shown: u32,
    pub wipe: f32,
    pub demo: Demo,
    /// The window's logical width, kept up to date by the runtime (`Ui::on_resize`).
    pub width: f32,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            page: Page::Comp(0), theme: 0, material: Material::Dot,
            params: COMPONENTS.iter().map(|c| c.params.iter().map(|p| p.default).collect()).collect(),
            edit: None, presses: 0, value: 42.0, text: TextEdit::default(), dialog_text: TextEdit::default(), on: true, level: 40.0,
            tab: 0, pick: 1, row: Some(2), dialog: 0, marks: std::array::from_fn(|s| mark_defaults(s as u8)), shown: demo::BUST, wipe: 0.3,
            demo: Demo::default(), width: 1280.0,
        }
    }
}

/// Values that suit each mark set (the same ones the contact sheets use).
pub fn mark_defaults(s: u8) -> PicFx {
    let base = PicFx::with_set(s);
    match s {
        set::CHARS => PicFx { pitch: 9.0, levels: 16.0, ground: 0.0, gamma: 1.1, ..base },
        set::DITHER => PicFx { pitch: 3.0, levels: 16.0, ground: 0.0, ..base },
        set::PIXEL => PicFx { pitch: 6.0, ..base },
        set::MOSAIC | set::BRICK => PicFx { pitch: 10.0, relief: 1.0, ground: 0.2, ..base },
        set::GLOW => PicFx { pitch: 8.0, levels: 12.0, ground: 0.0, glow: 1.0, gamma: 0.8, ..base },
        set::STROKE => PicFx { pitch: 9.0, levels: 8.0, ground: 0.15, follow: 1.0, ..base },
        _ => PicFx { pitch: 8.0, levels: 8.0, ground: 0.15, ..base },
    }
}

/// `PicFx` slots: the demo uses 0 and 1; the image page 2; the mark pages 3.
pub const IMAGE_SLOT: u8 = 2;
pub const MARK_SLOT: u8 = 3;
pub const WIPE_SPEC: Spec = Spec { min: 0.0, max: 1.0, step: 0.01, decimals: 2, unit: "" };

impl Gallery {
    pub fn theme(&self) -> Theme { THEMES[self.theme.min(THEMES.len() - 1)] }
    pub fn param(&self, c: usize, i: usize) -> f32 { self.params[c][i] }
    pub fn look(&self) -> Look {
        let mut pics = self.demo.pics;
        let img = pages::IMAGE;
        pics[IMAGE_SLOT as usize] = PicFx { pitch: self.param(img, 0), levels: self.param(img, 1), ..PicFx::with_set(set::DOTS) };
        if let Page::Marks(s) = self.page { pics[MARK_SLOT as usize] = self.marks[s as usize]; }
        Look { fx: self.demo.fx, theme: self.theme(), material: self.material, pics }
    }
    pub fn draft(&self, t: Target) -> Option<&str> { self.edit.as_ref().filter(|(k, _)| *k == t).map(|(_, d)| d.as_str()) }
    /// Go to the page a hash names; returns whether it names one.
    pub fn route(&mut self, hash: &str) -> bool {
        match Page::from_hash(hash) { Some(p) => { self.page = p; self.edit = None; true } None => false }
    }
}

pub fn update(s: &mut Gallery, m: Msg) {
    match m {
        Msg::Go(p) => { s.page = p; s.edit = None; s.dialog = 0; }
        Msg::Theme(i) => { s.theme = i.min(THEMES.len() - 1); s.demo.theme = s.theme; }
        Msg::Material(i) => { s.material = MATERIALS[i.min(MATERIALS.len() - 1)].0; s.demo.material = s.material; }
        Msg::Reset => {
            let d = Gallery::default();
            s.params = d.params; s.marks = d.marks; s.wipe = d.wipe; s.edit = None;
            s.value = d.value; s.level = d.level;
            s.demo = Demo { theme: s.theme, material: s.material, ..Demo::default() };
        }
        // Variants are shown as they are: typing into one does not change it.
        Msg::Num(Target::Shown, _) => {}
        Msg::Num(t, Edit::Draft(d)) => s.edit = Some((t, d)),
        Msg::Num(t, Edit::Set(v)) => {
            match t {
                Target::Param(c, i) => s.params[c][i] = v,
                Target::Pic(k, i) => { let (f, scale) = demo::pic_slot(&mut s.marks[k as usize], i); *f = v / scale; }
                Target::Wipe => s.wipe = v,
                Target::Value => s.value = v,
                Target::Shown => {}
            }
            s.edit = None;
        }
        Msg::Num(_, Edit::Cancel) => s.edit = None,
        Msg::Nothing => {}
        Msg::Press => s.presses += 1,
        Msg::Text(t) => s.text = t,
        Msg::DialogText(t) => s.dialog_text = t,
        Msg::Switch => s.on = !s.on,
        Msg::Level(v) => s.level = v,
        Msg::Tab(i) => s.tab = i,
        Msg::Pick(i) => s.pick = i,
        Msg::Row(i) => s.row = Some(i),
        Msg::Dialog(k) => s.dialog = k,
        Msg::Show(id) => s.shown = id,
        Msg::Wipe(x) => s.wipe = WIPE_SPEC.clamp(x),
        Msg::Demo(m) => {
            // The demo's own theme and material controls are the gallery's.
            match m {
                demo::Msg::Theme(i) => return update(s, Msg::Theme(i)),
                demo::Msg::Material(i) => return update(s, Msg::Material(i)),
                _ => {}
            }
            demo::update(&mut s.demo, m);
        }
    }
}

pub fn view(s: &Gallery) -> El<Msg> {
    let narrow = s.width < NARROW;
    let th = s.theme();
    let mut root = col().key("gallery").fonts(th.roles()).fill_width().fill_height().child(top_bar(s)).child(divider(th.hairline).key("top-rule"));
    let main = col().key(format!("main/{}", s.page.hash())).scroll().bar().grow(1.0).fill_width();
    let main = match s.page {
        Page::Demo => main.child(demo::view(&s.demo).map(Rc::new(Msg::Demo))),
        _ => main.pad(if narrow { 18.0 } else { 40.0 }, 32.0).child(pages::page(s)),
    };
    if narrow {
        root = root.child(rail_row(s)).child(divider(th.hairline).key("rail-rule")).child(main);
    } else {
        root = root.child(row().key("body").grow(1.0).fill_width().align(AlignItems::STRETCH)
            .child(rail(s)).child(vdivider(th.hairline).key("rail-rule")).child(main.shrink()));
    }
    root
}

fn top_bar(s: &Gallery) -> El<Msg> {
    let theme_names: Vec<&str> = THEMES.iter().map(|t| t.name).collect();
    let material_names: Vec<&str> = MATERIALS.iter().map(|m| m.1).collect();
    let material_now = MATERIALS.iter().position(|m| m.0 == s.material).unwrap_or(0);
    row().key("top").fill_width().wrap().gap(12.0).pad(20.0, 10.0).align(AlignItems::CENTER)
        .child(text("gui-lab", 17.0, Paint::INK).font(Font::Mono).key("top/name"))
        .child(space(0.0, 0.0).grow(1.0))
        .child(row().key("top/controls").gap(10.0).wrap().align(AlignItems::CENTER)
            .child(text("主题", 13.0, Paint::DIM))
            .child(segmented("theme", &theme_names, s.theme, 30.0, Msg::Theme).commit())
            .child(text("材质", 13.0, Paint::DIM))
            .child(segmented("material", &material_names, material_now, 30.0, Msg::Material).commit())
            .child(button_as("reset", "恢复默认", Variant::Default, Msg::Reset)))
}

/// The three groups and their pages.
fn groups() -> [(&'static str, Vec<Page>); 3] {
    [
        ("组件", (0..COMPONENTS.len()).map(Page::Comp).collect()),
        ("记号", (0..set::COUNT as u8).map(Page::Marks).collect()),
        ("排版", vec![Page::Type, Page::Demo]),
    ]
}

fn rail_item(s: &Gallery, p: Page, h: f32) -> El<Msg> {
    let on = s.page == p;
    let mut item = row().key(format!("nav/{}", p.hash())).height(h).pad(12.0, 0.0).radius(4.0).align(AlignItems::CENTER).on_press(Msg::Go(p))
        .child(text(p.name(), 14.0, if on { Paint::ACCENT } else { Paint::TEXT }));
    if on { item = item.fill(SELECTED); }
    item
}

fn rail(s: &Gallery) -> El<Msg> {
    let mut r = col().key("rail").width(RAIL).scroll().bar().pad(12.0, 16.0).gap(2.0);
    for (gi, (name, pages)) in groups().into_iter().enumerate() {
        if gi > 0 { r = r.child(space(1.0, 14.0)); }
        r = r.child(row().key(format!("rail/group/{gi}")).pad(12.0, 4.0).child(text(name, 12.0, Paint::DIM)));
        for p in pages { r = r.child(rail_item(s, p, 30.0)); }
    }
    r
}

/// The rail on narrow windows: one row that scrolls sideways, the group names as dim labels.
fn rail_row(s: &Gallery) -> El<Msg> {
    let mut r = row().key("rail").fill_width().scroll_x().bar().pad(12.0, 6.0).gap(4.0).align(AlignItems::CENTER);
    for (gi, (name, pages)) in groups().into_iter().enumerate() {
        r = r.child(row().key(format!("rail/group/{gi}")).pad(if gi > 0 { 12.0 } else { 4.0 }, 0.0).child(text(name, 12.0, Paint::DIM)));
        for p in pages { r = r.child(rail_item(s, p, 32.0)); }
    }
    r
}

pub type GalleryUi = crate::ui::Ui<Gallery, Msg>;

pub fn new_ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> GalleryUi {
    let state = Gallery { width: w as f32 / scale, ..Gallery::default() };
    crate::ui::Ui::new(state, view, update, faces, w, h, scale).on_resize(|s, w, _| s.width = w)
}
