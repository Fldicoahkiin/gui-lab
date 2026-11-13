//! Style as data: a theme (colours, type, lines), a material (how stage 2 draws), and up to eight sets
//! of picture-mark parameters that picture instances choose between. See DESIGN.md §1.6.1 and §4.
use crate::scene::Fx;
use crate::text::{MONO, PIXEL, SANS};

/// Font sizes of a theme, logical px. Pixel faces are snapped to whole multiples of their face size
/// when drawn, so these are requests, not promises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale { pub display: f32, pub title: f32, pub body: f32, pub small: f32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub ground: [f32; 3],
    pub ink: [f32; 3],
    pub accent: [f32; 3],
    /// Face indices (see `text::SANS` …) for headings, running text and figures.
    pub display: u8,
    pub body: u8,
    pub mono: u8,
    pub sizes: TypeScale,
    /// Hairline and rule widths, logical px.
    pub hairline: f32,
    pub rule: f32,
}
impl Theme {
    /// display, body, mono: what a subtree's `Font::Display` / `Body` / `Mono` resolve to.
    pub fn roles(&self) -> [u8; 3] { [self.display, self.body, self.mono] }
}

pub const DARK: Theme = Theme {
    name: "暗", ground: [0.047, 0.051, 0.055], ink: [0.886, 0.89, 0.878], accent: [1.0, 0.42, 0.17],
    display: SANS, body: SANS, mono: MONO,
    sizes: TypeScale { display: 34.0, title: 20.0, body: 16.0, small: 13.0 }, hairline: 1.0, rule: 1.0,
};
pub const LIGHT: Theme = Theme {
    name: "明", ground: [0.914, 0.910, 0.890], ink: [0.09, 0.094, 0.10], accent: [0.86, 0.30, 0.08],
    display: SANS, body: SANS, mono: MONO,
    sizes: TypeScale { display: 34.0, title: 20.0, body: 16.0, small: 13.0 }, hairline: 1.0, rule: 1.0,
};
/// Everything set in the pixel faces, at whole multiples of their native sizes.
pub const PIXEL_THEME: Theme = Theme {
    name: "像素", ground: [0.035, 0.04, 0.05], ink: [0.80, 0.93, 0.80], accent: [1.0, 0.78, 0.25],
    display: PIXEL, body: PIXEL, mono: MONO,
    sizes: TypeScale { display: 36.0, title: 24.0, body: 12.0, small: 12.0 }, hairline: 1.0, rule: 2.0,
};
pub const THEMES: [Theme; 3] = [DARK, LIGHT, PIXEL_THEME];

/// How stage 2 draws text, shapes and the ground. Pictures always follow their own `PicFx` slot,
/// whichever material is on: the slot is the instance's override.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Material {
    /// Nothing loosens into units: fronts still push instances and new things still condense (as a
    /// fade); the ground is flat.
    Plain,
    /// The global dot lattice: loosening text breaks into units sized by the ink under them, and the
    /// ground is a field of small units.
    #[default]
    Dot,
}
pub const MATERIALS: [(Material, &str); 2] = [(Material::Plain, "plain"), (Material::Dot, "dot")];

/// Rows of the mark atlas.
pub mod set {
    pub const CHARS: u8 = 0;
    pub const DITHER: u8 = 1;
    pub const PIXEL: u8 = 2;
    pub const MOSAIC: u8 = 3;
    pub const BRICK: u8 = 4;
    pub const DOTS: u8 = 5;
    pub const CROSS: u8 = 6;
    pub const DIAGONAL: u8 = 7;
    pub const DIAMOND: u8 = 8;
    pub const LINES: u8 = 9;
    pub const GLOW: u8 = 10;
    pub const STROKE: u8 = 11;
    pub const COUNT: usize = 12;
    pub const NAMES: [&str; COUNT] = ["字符", "抖动", "像素画", "马赛克", "积木", "圆点", "十字", "斜线", "菱形", "横线", "光点", "笔触"];
}

/// Ink colour of a picture's marks.
pub mod colour {
    pub const ORIGINAL: u8 = 0;
    pub const THEME_INK: u8 = 1;
    pub const TWO: u8 = 2;
    pub const GIVEN: u8 = 3;
    pub const NAMES: [&str; 4] = ["原色", "墨色", "双色", "指定色"];
}

/// How a picture turns into marks (DESIGN.md §1.6.1). Every field is a plain number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PicFx {
    /// Cell size, logical px, 1–64.
    pub pitch: f32,
    /// Brightness levels, 2–16.
    pub levels: f32,
    /// Mark set (atlas row).
    pub set: f32,
    /// 0 original, 1 theme ink, 2 two colours by brightness, 3 a given colour.
    pub colour: f32,
    /// Brightness of the ground between marks: 0 black, 1 the cell's own colour.
    pub ground: f32,
    /// Brightness curve, 0.3–3.
    pub gamma: f32,
    /// Width of the dithered hand-over between levels (and of palette dithering), 0–1.
    pub spread: f32,
    /// How much marks turn along the image's contours, 0–1.
    pub follow: f32,
    /// Parallax: how far the sampling point is pushed toward the pointer by brightness, logical px.
    pub depth: f32,
    /// 0–2.
    pub glow: f32,
    /// Palette row, and how many of its colours to use (0 = no palette).
    pub palette: f32,
    pub colours: f32,
    /// Shading from the atlas' height channel, 0 = off.
    pub relief: f32,
    /// Region size in cells (0 = off); regions choose by hash (0) or by brightness band (1); how many
    /// of `region_sets` are used.
    pub regions: f32,
    pub region_by: f32,
    pub region_count: f32,
    pub region_sets: [f32; 4],
    /// Colours for modes 2 and 3, 0–1.
    pub tint_a: [f32; 3],
    pub tint_b: [f32; 3],
}
impl Default for PicFx {
    fn default() -> Self {
        Self {
            pitch: 8.0, levels: 8.0, set: set::DOTS as f32, colour: 0.0, ground: 0.2, gamma: 0.6, spread: 0.0, follow: 0.0,
            depth: 0.0, glow: 0.0, palette: 0.0, colours: 0.0, relief: 0.0, regions: 0.0, region_by: 0.0, region_count: 4.0,
            region_sets: [set::CHARS as f32, set::DOTS as f32, set::DITHER as f32, set::STROKE as f32],
            tint_a: [1.0, 0.42, 0.17], tint_b: [0.95, 0.93, 0.85],
        }
    }
}
impl PicFx {
    pub fn with_set(s: u8) -> Self { Self { set: s as f32, ..Self::default() } }
    /// The uniform form: seven vec4s, in the order the shader reads them.
    pub fn pack(&self) -> [[f32; 4]; 7] {
        [
            [self.pitch, self.levels, self.set, self.colour],
            [self.ground, self.gamma, self.spread, self.follow],
            [self.depth, self.glow, self.palette, self.colours],
            [self.relief, self.regions, self.region_by, self.region_count],
            [self.tint_a[0], self.tint_a[1], self.tint_a[2], 0.0],
            [self.tint_b[0], self.tint_b[1], self.tint_b[2], 0.0],
            self.region_sets,
        ]
    }
}

/// Everything stage 2 needs that is not in the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look { pub fx: Fx, pub theme: Theme, pub material: Material, pub pics: [PicFx; 8] }
impl Default for Look {
    fn default() -> Self { Self { fx: Fx::default(), theme: DARK, material: Material::Dot, pics: [PicFx::default(); 8] } }
}
