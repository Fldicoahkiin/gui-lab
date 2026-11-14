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
    /// The page ground, from the top of the window (`ground`) to the bottom (`ground2`; the same
    /// colour for a flat ground).
    pub ground: [f32; 3],
    pub ground2: [f32; 3],
    pub ink: [f32; 3],
    /// Accent colours: `accent` is hue 0 of `Paint`, `second` hue 1, `third` hue 2.
    pub accent: [f32; 3],
    pub second: [f32; 3],
    pub third: [f32; 3],
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

const SIZES: TypeScale = TypeScale { display: 34.0, title: 20.0, body: 16.0, small: 13.0 };
const fn theme(name: &'static str, ground: [f32; 3], ink: [f32; 3], accent: [f32; 3]) -> Theme {
    Theme { name, ground, ground2: ground, ink, accent, second: accent, third: accent, display: SANS, body: SANS, mono: MONO, sizes: SIZES, hairline: 1.0, rule: 1.0 }
}

pub const DARK: Theme = Theme { second: [0.30, 0.62, 1.0], third: [0.98, 0.80, 0.25], ..theme("暗", [0.047, 0.051, 0.055], [0.886, 0.89, 0.878], [1.0, 0.42, 0.17]) };
pub const LIGHT: Theme = Theme { second: [0.10, 0.36, 0.78], third: [0.80, 0.60, 0.02], ..theme("明", [0.914, 0.910, 0.890], [0.09, 0.094, 0.10], [0.86, 0.30, 0.08]) };
/// Everything set in the pixel faces, at whole multiples of their native sizes.
pub const PIXEL_THEME: Theme = Theme {
    display: PIXEL, body: PIXEL, sizes: TypeScale { display: 36.0, title: 24.0, body: 12.0, small: 12.0 }, rule: 2.0,
    ..theme("像素", [0.035, 0.04, 0.05], [0.80, 0.93, 0.80], [1.0, 0.78, 0.25])
};
/// Off-white paper and one fluorescent spot ink (Lo-Fi Print).
pub const PAPER: Theme = Theme {
    second: [0.12, 0.36, 0.78], third: [0.98, 0.84, 0.10], sizes: TypeScale { display: 44.0, title: 22.0, body: 16.0, small: 12.0 }, rule: 2.0,
    ..theme("纸", [0.937, 0.918, 0.871], [0.13, 0.12, 0.14], [0.94, 0.24, 0.42])
};
/// Ink on cream, two colours (Linocut).
pub const CREAM: Theme = Theme {
    sizes: TypeScale { display: 52.0, title: 22.0, body: 16.0, small: 12.0 }, rule: 3.0,
    ..theme("米", [0.925, 0.886, 0.792], [0.085, 0.075, 0.07], [0.70, 0.14, 0.09])
};
/// Cool and translucent (Transparent Electronics).
pub const COOL: Theme = Theme {
    second: [1.0, 0.56, 0.24], third: [0.65, 0.95, 0.55], display: MONO,
    ..theme("冷", [0.035, 0.065, 0.09], [0.76, 0.90, 0.96], [0.32, 0.86, 0.96])
};
/// White paper, black and one signal colour, mono captions (Tech-Manual).
pub const MANUAL: Theme = Theme {
    second: [0.12, 0.34, 0.70], sizes: TypeScale { display: 30.0, title: 18.0, body: 15.0, small: 11.0 },
    ..theme("手册", [0.965, 0.962, 0.945], [0.07, 0.07, 0.08], [0.96, 0.34, 0.04])
};
/// Plain system grey (Normcore).
pub const NORMCORE: Theme = Theme {
    sizes: TypeScale { display: 26.0, title: 17.0, body: 14.0, small: 12.0 },
    ..theme("灰", [0.953, 0.953, 0.957], [0.12, 0.125, 0.135], [0.0, 0.45, 0.95])
};
/// Near black, hairlines, one thin accent (Cyberminimalism).
pub const CYBER: Theme = Theme {
    display: MONO, sizes: TypeScale { display: 22.0, title: 15.0, body: 14.0, small: 11.0 },
    ..theme("近黑", [0.016, 0.017, 0.02], [0.78, 0.80, 0.82], [0.36, 1.0, 0.70])
};
/// Paper, black and the three primaries (Bauhaus).
pub const BAUHAUS: Theme = Theme {
    second: [0.10, 0.27, 0.64], third: [0.97, 0.76, 0.08], sizes: TypeScale { display: 64.0, title: 24.0, body: 16.0, small: 12.0 }, rule: 4.0,
    ..theme("包豪斯", [0.925, 0.906, 0.855], [0.06, 0.06, 0.06], [0.85, 0.13, 0.10])
};
/// Silver-blue gradient ground (Neo-Y2K).
pub const Y2K: Theme = Theme {
    ground2: [0.56, 0.62, 0.78], second: [0.96, 0.32, 0.78], third: [0.55, 0.96, 0.92], sizes: TypeScale { display: 48.0, title: 22.0, body: 15.0, small: 12.0 },
    ..theme("银", [0.86, 0.89, 0.95], [0.06, 0.07, 0.14], [0.18, 0.42, 1.0])
};
/// Black, amber LEDs, pixel faces (Neo-Dotmatrix).
pub const LED: Theme = Theme {
    display: PIXEL, body: PIXEL, second: [0.35, 1.0, 0.45], sizes: TypeScale { display: 48.0, title: 24.0, body: 12.0, small: 12.0 }, rule: 2.0,
    ..theme("灯", [0.02, 0.018, 0.016], [0.86, 0.82, 0.74], [1.0, 0.52, 0.12])
};
/// Dark, figures in the mono face (Algorithmic Gaze).
pub const GAZE: Theme = Theme {
    display: MONO, second: [1.0, 0.30, 0.28], sizes: TypeScale { display: 30.0, title: 18.0, body: 15.0, small: 11.0 },
    ..theme("注视", [0.03, 0.034, 0.038], [0.84, 0.86, 0.85], [0.30, 1.0, 0.50])
};
/// Black, white and two signal colours (Glitch).
pub const GLITCH: Theme = Theme {
    second: [0.10, 0.92, 1.0], sizes: TypeScale { display: 56.0, title: 22.0, body: 15.0, small: 12.0 },
    ..theme("故障", [0.02, 0.02, 0.03], [0.93, 0.93, 0.95], [1.0, 0.16, 0.46])
};
/// Kraft card, ink, two colours and white paper (Mixed media).
pub const KRAFT: Theme = Theme {
    second: [0.14, 0.34, 0.56], third: [0.975, 0.96, 0.925], sizes: TypeScale { display: 46.0, title: 22.0, body: 15.0, small: 12.0 },
    ..theme("牛皮纸", [0.78, 0.69, 0.56], [0.09, 0.08, 0.07], [0.86, 0.30, 0.17])
};
pub const THEMES: [Theme; 15] = [DARK, LIGHT, PIXEL_THEME, PAPER, CREAM, COOL, MANUAL, NORMCORE, CYBER, BAUHAUS, Y2K, LED, GAZE, GLITCH, KRAFT];
pub fn theme_index(name: &str) -> usize { THEMES.iter().position(|t| t.name == name).unwrap_or(0) }

/// How stage 2 draws text, shapes and the ground (DESIGN.md §1.6). Pictures always follow their own
/// `PicFx` slot, whichever material is on: the slot is the instance's override. The discriminant is
/// the id the shader dispatches on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Material {
    /// Nothing loosens into units: fronts still push instances and new things still condense (as a
    /// fade); the ground is flat.
    Plain = 0,
    /// The global dot lattice: loosening text breaks into units sized by the ink under them, and the
    /// ground is a field of small units.
    #[default]
    Dot = 1,
    Halftone = 2,
    Ascii = 3,
    Print = 4,
    Lino = 5,
    Glitch = 6,
    Xray = 7,
    Chrome = 8,
    Liquid = 9,
    Collage = 10,
    /// The intermediate images made visible: the signed nearness as contour bands (warm inside the
    /// ink, cool outside), the outline itself, and the looseness as a tint. A view of the machinery,
    /// so like chrome it has its own look at rest.
    Near = 11,
}
pub const MATERIAL_COUNT: usize = 12;
pub const MATERIALS: [(Material, &str); MATERIAL_COUNT] = [
    (Material::Plain, "plain"), (Material::Dot, "dot"), (Material::Halftone, "halftone"), (Material::Ascii, "ascii"), (Material::Print, "print"),
    (Material::Lino, "lino"), (Material::Glitch, "glitch"), (Material::Xray, "xray"), (Material::Chrome, "chrome"), (Material::Liquid, "liquid"),
    (Material::Collage, "collage"), (Material::Near, "near"),
];
impl Material {
    /// Whether the material has a time term that moves loose things by itself (glitch's held rows,
    /// xray's travelling dashes): while something is loose under it, frames keep coming.
    pub fn time_varying(self) -> bool { matches!(self, Material::Glitch | Material::Xray) }
    pub fn id(self) -> u8 { self as u8 }
    pub fn from_id(i: u8) -> Material { MATERIALS[(i as usize).min(MATERIAL_COUNT - 1)].0 }
    pub fn name(self) -> &'static str { MATERIALS[self as usize].1 }
}

/// The instance code of a material override at nesting `level` (1 = outermost): stage 1 writes it into
/// `near.a` as `128 + level × 16 + id` (levels above 7 count as 7), so under max blending the deepest
/// override wins and any override wins over a print pitch (which stays below 128).
pub fn override_code(level: u8, id: u8) -> f32 { (128 + level.clamp(1, 7) as u32 * 16 + id.min(15) as u32) as f32 }

/// One number of a material, for its gallery page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatParam { pub name: &'static str, pub min: f32, pub max: f32, pub step: f32, pub decimals: usize, pub unit: &'static str, pub default: f32 }
const fn mp(name: &'static str, min: f32, max: f32, step: f32, decimals: usize, unit: &'static str, default: f32) -> MatParam {
    MatParam { name, min, max, step, decimals, unit, default }
}
/// The numbers of each material, in the order of `MaterialParams` (DESIGN.md §1.6, materials).
pub const MAT_PARAMS: [&[MatParam]; MATERIAL_COUNT] = [
    &[],
    &[],
    &[mp("点距", 2.0, 32.0, 0.5, 1, "px", 7.0), mp("角度", 0.0, 90.0, 1.0, 0, "°", 45.0), mp("分色错开", 0.0, 8.0, 0.25, 2, "px", 1.25), mp("增益", 0.2, 3.0, 0.05, 2, "", 1.1)],
    &[mp("点距", 4.0, 40.0, 1.0, 0, "px", 10.0), mp("级数", 2.0, 16.0, 1.0, 0, "", 12.0), mp("上色方式", 0.0, 1.0, 1.0, 0, "", 0.0), mp("法线", 0.0, 1.0, 0.05, 2, "", 1.0)],
    &[mp("纸纹", 0.0, 1.0, 0.05, 2, "", 0.6), mp("套印错位", 0.0, 6.0, 0.25, 2, "px", 1.5), mp("斑点", 0.0, 1.0, 0.05, 2, "", 0.6), mp("洇开", 0.0, 10.0, 0.5, 1, "px", 3.0)],
    &[mp("刻痕", 0.0, 1.0, 0.05, 2, "", 0.6), mp("刻带", 2.0, 40.0, 1.0, 0, "px", 12.0), mp("刀长", 2.0, 40.0, 1.0, 0, "px", 12.0), mp("刀距", 2.0, 16.0, 0.5, 1, "px", 7.0), mp("抖动", 0.0, 1.0, 0.05, 2, "", 0.4)],
    &[mp("行高", 1.0, 40.0, 1.0, 0, "px", 6.0), mp("位移", 0.0, 200.0, 2.0, 0, "px", 48.0), mp("色差", 0.0, 30.0, 0.5, 1, "px", 6.0), mp("保持", 16.0, 500.0, 2.0, 0, "ms", 90.0), mp("块", 4.0, 120.0, 2.0, 0, "px", 28.0)],
    &[mp("线宽", 0.5, 4.0, 0.25, 2, "px", 1.5), mp("填充", 0.0, 1.0, 0.02, 2, "", 0.22), mp("等距线", 2.0, 32.0, 1.0, 0, "px", 7.0), mp("虚线", 2.0, 24.0, 1.0, 0, "px", 6.0)],
    &[mp("倒角", 1.0, 32.0, 0.5, 1, "px", 9.0), mp("环境", 0.2, 3.0, 0.05, 2, "", 1.0), mp("高光", 0.0, 2.0, 0.05, 2, "", 0.9), mp("最小尺寸", 8.0, 120.0, 1.0, 0, "px", 44.0), mp("着色", 0.0, 1.0, 0.05, 2, "", 0.15)],
    &[mp("融合", 2.0, 40.0, 1.0, 0, "px", 14.0), mp("折射", 0.0, 30.0, 0.5, 1, "px", 7.0), mp("光泽", 0.0, 2.0, 0.05, 2, "", 0.9), mp("下垂", 0.0, 120.0, 2.0, 0, "px", 40.0)],
    &[mp("撕边", 0.0, 20.0, 0.5, 1, "px", 5.0), mp("纸边", 0.0, 4.0, 0.25, 2, "px", 1.5)],
    &[mp("间距", 1.5, 12.0, 0.5, 1, "px", 3.0), mp("填充", 0.0, 1.0, 0.05, 2, "", 0.25)],
];

/// A material's numbers, in the order of `MAT_PARAMS` (unused places are 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialParams(pub [f32; 8]);
impl MaterialParams {
    pub fn defaults(m: Material) -> Self {
        let mut v = [0.0; 8];
        for (i, p) in MAT_PARAMS[m as usize].iter().enumerate() { v[i] = p.default; }
        Self(v)
    }
    pub fn all() -> [MaterialParams; MATERIAL_COUNT] { std::array::from_fn(|i| Self::defaults(Material::from_id(i as u8))) }
}

/// The annotation layer's settings (DESIGN.md §1.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GazeMode {
    #[default]
    Off,
    /// The element under the pointer and its ancestors.
    Hover,
    All,
    /// The focused element and its ancestors.
    Focus,
}
pub const GAZE_MODES: [(GazeMode, &str); 4] = [(GazeMode::Off, "关"), (GazeMode::Hover, "悬停"), (GazeMode::All, "全部"), (GazeMode::Focus, "焦点路径")];
/// What the annotation layer draws, as bits of `Gaze::kinds`.
pub mod gaze {
    pub const FRAME: u16 = 1;
    pub const BOX: u16 = 2;
    pub const INDEX: u16 = 4;
    pub const SIZE: u16 = 8;
    pub const KIND: u16 = 16;
    pub const LEADER: u16 = 32;
    pub const RULER: u16 = 64;
    pub const CROSS: u16 = 128;
    pub const GRID: u16 = 256;
    pub const COUNT: usize = 9;
    pub const NAMES: [&str; COUNT] = ["角标", "边框", "编号", "尺寸", "类型", "引线", "标尺", "十字线", "网格"];
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaze {
    pub mode: GazeMode,
    pub kinds: u16,
    /// At most this many elements are framed and labelled, largest first.
    pub density: f32,
    /// How long a frame takes to draw on, ms.
    pub draw_ms: f32,
    pub colour: [f32; 3],
    /// Only elements with an explicit callout (`El::annotate`) are candidates.
    pub only_marked: bool,
}
impl Default for Gaze {
    fn default() -> Self { Self { mode: GazeMode::Off, kinds: gaze::FRAME | gaze::INDEX | gaze::SIZE, density: 24.0, draw_ms: 260.0, colour: [0.30, 1.0, 0.50], only_marked: false } }
}
impl Gaze {
    pub fn has(&self, k: u16) -> bool { self.kinds & k != 0 }
}

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

/// Everything stages 2 and 3 need that is not in the scene. `params` holds every material's numbers
/// (a collage page uses several materials at once); `loose_scale` scales all looseness of text and
/// shapes (Normcore keeps it small).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fx: Fx, pub theme: Theme, pub material: Material, pub pics: [PicFx; 8], pub params: [MaterialParams; MATERIAL_COUNT], pub gaze: Gaze, pub loose_scale: f32,
    /// The material pass's resolution as a fraction of the device's (`perf::LEVELS`: 1, 0.75, 0.5), or
    /// 0 for automatic (DESIGN.md §1.12). Pixels at rest are always drawn at full resolution.
    pub quality: f32,
    /// Show the frame-time readout (drawn by the annotation layer).
    pub overlay: bool,
    /// The most looseness field effects (fronts, the lens, a pull past a scroll end, a press held) may
    /// give text to read and controls (DESIGN.md §1.15). Morphs, flows and recomposition are exempt.
    pub legible_cap: f32,
    /// Looseness per pixel is clamped to this after every source is summed.
    pub loose_max: f32,
    /// How changed glyphs recompose (DESIGN.md §1.16).
    pub recompose: crate::morph::Recompose,
}
impl Default for Look {
    fn default() -> Self { Self { fx: Fx::default(), theme: DARK, material: Material::Dot, pics: [PicFx::default(); 8], params: MaterialParams::all(), gaze: Gaze::default(), loose_scale: 1.0, quality: 1.0, overlay: false, legible_cap: LEGIBLE_CAP, loose_max: 1.0, recompose: crate::morph::Recompose::default() } }
}
/// The default `Look::legible_cap`.
pub const LEGIBLE_CAP: f32 = 0.15;

/// A style preset (DESIGN.md §4): theme × material × its numbers × annotation × the type the theme
/// chooses, and how pictures turn into marks. Plain data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub name: &'static str,
    pub hash: &'static str,
    pub theme: usize,
    pub material: Material,
    pub params: MaterialParams,
    pub gaze: Gaze,
    pub loose_scale: f32,
    /// The lattice pitch of text and shapes (`Fx::pitch`).
    pub pitch: f32,
    /// How the style's pictures turn into marks.
    pub pic: PicFx,
}

fn with(m: Material, vals: &[(usize, f32)]) -> MaterialParams {
    let mut p = MaterialParams::defaults(m);
    for &(i, v) in vals { p.0[i] = v; }
    p
}

/// The twelve presets, in the order of DESIGN.md §4.
pub fn styles() -> [Style; 12] {
    use gaze::*;
    let off = Gaze::default();
    let base = |name, hash, th: Theme, material: Material| Style {
        name, hash, theme: theme_index(th.name), material, params: MaterialParams::defaults(material), gaze: off, loose_scale: 1.0, pitch: 5.0,
        pic: PicFx { pitch: 7.0, levels: 8.0, ground: 0.15, ..PicFx::with_set(set::DOTS) },
    };
    [
        Style { gaze: Gaze { mode: GazeMode::All, kinds: FRAME | INDEX | SIZE | CROSS | KIND, density: 14.0, draw_ms: 320.0, colour: GAZE.accent, only_marked: false },
            pic: PicFx { pitch: 4.0, levels: 6.0, colour: colour::GIVEN as f32, tint_a: GAZE.accent, ground: 0.05, ..PicFx::with_set(set::DITHER) },
            ..base("Algorithmic Gaze", "gaze", GAZE, Material::Plain) },
        Style { pitch: 7.0, pic: PicFx { pitch: 7.0, levels: 6.0, colour: colour::GIVEN as f32, tint_a: LED.accent, ground: 0.0, glow: 1.0, gamma: 0.8, ..PicFx::with_set(set::GLOW) },
            ..base("Neo-Dotmatrix", "dotmatrix", LED, Material::Dot) },
        Style { params: with(Material::Print, &[(1, 1.75)]),
            pic: PicFx { pitch: 4.0, levels: 8.0, colour: colour::TWO as f32, tint_a: [0.13, 0.12, 0.14], tint_b: PAPER.accent, ground: 0.25, ..PicFx::with_set(set::DOTS) },
            ..base("Lo-Fi Print", "lofi-print", PAPER, Material::Print) },
        Style { pic: PicFx { pitch: 3.0, levels: 4.0, palette: 3.0, colours: 4.0, spread: 0.6, ..PicFx::with_set(set::PIXEL) },
            ..base("Glitch", "glitch", GLITCH, Material::Glitch) },
        Style { pic: PicFx { pitch: 7.0, levels: 6.0, colour: colour::TWO as f32, tint_a: [0.085, 0.075, 0.07], tint_b: [0.925, 0.886, 0.792], ground: 0.0, follow: 1.0, gamma: 0.55, ..PicFx::with_set(set::STROKE) },
            ..base("Metamorphosis Linocut", "linocut", CREAM, Material::Lino) },
        Style { gaze: Gaze { mode: GazeMode::All, kinds: LEADER | INDEX, density: 10.0, draw_ms: 420.0, colour: COOL.accent, only_marked: true },
            pic: PicFx { pitch: 5.0, levels: 6.0, colour: colour::GIVEN as f32, tint_a: COOL.accent, ground: 0.1, ..PicFx::with_set(set::LINES) },
            ..base("Transparent Electronics", "transparent", COOL, Material::Xray) },
        Style { gaze: Gaze { mode: GazeMode::All, kinds: LEADER | INDEX | RULER, density: 12.0, draw_ms: 380.0, colour: MANUAL.ink, only_marked: true },
            pic: PicFx { pitch: 4.0, levels: 6.0, colour: colour::THEME_INK as f32, ground: 1.0, ..PicFx::with_set(set::DIAGONAL) },
            ..base("Tech-Manual", "tech-manual", MANUAL, Material::Plain) },
        Style { loose_scale: 0.3, pic: PicFx { pitch: 3.0, ..PicFx::with_set(set::PIXEL) }, ..base("Normcore", "normcore", NORMCORE, Material::Plain) },
        Style { gaze: Gaze { mode: GazeMode::Hover, kinds: BOX | SIZE | KIND, density: 4.0, draw_ms: 200.0, colour: CYBER.accent, only_marked: false },
            pic: PicFx { pitch: 6.0, levels: 10.0, colour: colour::GIVEN as f32, tint_a: CYBER.accent, ground: 0.0, gamma: 1.1, ..PicFx::with_set(set::CHARS) },
            ..base("Cyberminimalism", "cyberminimal", CYBER, Material::Plain) },
        Style { params: with(Material::Halftone, &[(0, 9.0), (2, 0.0), (1, 15.0)]),
            pic: PicFx { pitch: 8.0, levels: 8.0, palette: 6.0, colours: 5.0, spread: 0.4, ..PicFx::with_set(set::DOTS) },
            ..base("Bauhaus", "bauhaus", BAUHAUS, Material::Halftone) },
        Style { pic: PicFx { pitch: 9.0, levels: 8.0, relief: 1.6, ground: 0.3, ..PicFx::with_set(set::BRICK) }, ..base("Neo-Y2K", "y2k", Y2K, Material::Chrome) },
        Style { pic: PicFx { pitch: 6.0, levels: 8.0, colour: colour::TWO as f32, tint_a: [0.09, 0.08, 0.07], tint_b: KRAFT.third, ground: 0.2, ..PicFx::with_set(set::DOTS) },
            ..base("Mixed media", "mixed-media", KRAFT, Material::Collage) },
    ]
}
