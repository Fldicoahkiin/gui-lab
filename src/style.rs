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
/// The Lab (DESIGN.md §5.3): near-black ground, grey-white ink, one blue accent (all three hues the
/// same blue), everything in the monospaced pixel face.
pub const LAB: Theme = Theme { display: MONO, body: MONO, ..theme("实验室", [0.031, 0.033, 0.036], [0.84, 0.85, 0.86], [0.26, 0.52, 1.0]) };
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
pub const THEMES: [Theme; 13] = [DARK, LIGHT, PIXEL_THEME, PAPER, CREAM, COOL, MANUAL, NORMCORE, CYBER, BAUHAUS, LED, GAZE, GLITCH];
pub fn theme_index(name: &str) -> usize { THEMES.iter().position(|t| t.name == name).unwrap_or(0) }

/// How stage 2 draws text, shapes and the ground (DESIGN.md §1.6). Pictures always follow their own
/// `PicFx` slot, whichever material is on: the slot is the instance's override. The discriminant is
/// the id the shader dispatches on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
    /// The intermediate images made visible: the signed nearness as contour bands (warm inside the
    /// ink, cool outside), the outline itself, and the looseness as a tint. A view of the machinery,
    /// so it has its own look at rest.
    Near = 7,
    /// One thread writes the ink out and back (DESIGN.md §1.30): at rest the page is plain; a loose
    /// element's fill fades as a single line of constant width leaves its true outline, wanders, knots
    /// where the field map has energy and trails off along its flow. The line is geometry
    /// (`strand.rs`), drawn as shapes in the first pass.
    Scribblr = 8,
}
pub const MATERIAL_COUNT: usize = 9;
pub const MATERIALS: [(Material, &str); MATERIAL_COUNT] = [
    (Material::Plain, "plain"), (Material::Dot, "dot"), (Material::Halftone, "halftone"), (Material::Ascii, "ascii"), (Material::Print, "print"),
    (Material::Lino, "lino"), (Material::Glitch, "glitch"), (Material::Near, "near"),
    (Material::Scribblr, "scribblr"),
];
impl Material {
    /// Whether the material has a time term that moves loose things by itself (glitch's held rows,
    /// while something is loose under it, frames keep coming.
    pub fn time_varying(self) -> bool { matches!(self, Material::Glitch) }
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
    &[mp("间距", 1.5, 12.0, 0.5, 1, "px", 3.0), mp("填充", 0.0, 1.0, 0.05, 2, "", 0.25)],
    &[mp("线宽", 0.5, 3.0, 0.25, 2, "px", 1.0), mp("取样", 1.0, 6.0, 0.5, 1, "px", 2.0), mp("游走", 0.0, 64.0, 1.0, 0, "px", 24.0), mp("打结", 0.0, 1.0, 0.05, 2, "", 0.6),
      mp("结的半径", 4.0, 80.0, 1.0, 0, "px", 28.0), mp("线头", 0.0, 400.0, 5.0, 0, "px", 160.0), mp("跟场走", 0.0, 2.0, 0.05, 2, "", 1.0), mp("折角", 0.0, 1.0, 0.05, 2, "", 0.7)],
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    pub const LED: u8 = 12;
    pub const BRAILLE: u8 = 13;
    pub const CUBES: u8 = 14;
    pub const HEX: u8 = 15;
    pub const STIPPLE: u8 = 16;
    pub const HATCH: u8 = 17;
    pub const TRUCHET: u8 = 18;
    pub const TOPO: u8 = 19;
    pub const COUNT: usize = 20;
    pub const NAMES: [&str; COUNT] = ["字符", "抖动", "像素画", "马赛克", "积木", "圆点", "十字", "斜线", "菱形", "横线", "光点", "笔触", "LED 点阵", "盲文", "体素", "六角", "点刻", "交叉线", "特鲁谢", "等高线"];
    /// The families the gallery groups the sets in (DESIGN.md §1.6.1): characters; pixels and blocks;
    /// print and paper; geometry; signal and light.
    pub const FAMILIES: [(&str, &[u8]); 5] = [
        ("字符", &[CHARS, BRAILLE]),
        ("像素与方块", &[PIXEL, DITHER, MOSAIC, BRICK, CUBES, LED]),
        ("印刷与纸", &[DOTS, STIPPLE, HATCH, LINES, STROKE]),
        ("几何", &[CROSS, DIAGONAL, DIAMOND, HEX, TRUCHET, TOPO]),
        ("光", &[GLOW]),
    ];
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// How scrolling looks (DESIGN.md §1.8.3). At rest and below `calm_speed` what scrolls is a picture
/// moved by whole device pixels: crisp, with the materials' lattices fixed to the content, nothing loose.
/// Above it each material shows a deliberate, directional trace of the motion whose strength eases in
/// (smoothstep from `calm_speed` to `full_speed`) and settles `settle_ms` after the motion stops. Whole
/// lines move as one; text to read is never loosened by it. Speeds are logical px a second, lengths
/// logical px.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Scroll {
    /// Below this speed nothing but the motion itself shows.
    pub calm_speed: f32,
    /// At this speed and above the trace is at full strength.
    pub full_speed: f32,
    /// How long the trace takes to settle once the motion stops.
    pub settle_ms: f32,
    /// How far behind the motion the trailing edge reaches at full strength (plain's soft edge, dot's
    /// phosphor, ascii's character trail, lino's carved echo, liquid's smear).
    pub trail: f32,
    /// How far the second colour falls out of register at full strength (halftone, print).
    pub misreg: f32,
    /// Glitch: how far a band slips, how tall a band is, and at least how long between two slips.
    pub slip: f32,
    pub band: f32,
    pub slip_ms: f32,
}
impl Default for Scroll {
    fn default() -> Self { Self { calm_speed: 600.0, full_speed: 2600.0, settle_ms: 140.0, trail: 28.0, misreg: 3.0, slip: 22.0, band: 36.0, slip_ms: 120.0 } }
}
impl Scroll {
    /// How strong the trace is at `speed` (px/s, either sign).
    pub fn strength(&self, speed: f32) -> f32 {
        let (a, b) = (self.calm_speed.max(0.0), self.full_speed.max(self.calm_speed + 1.0));
        let x = ((speed.abs() - a) / (b - a)).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
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
    /// The performance readout: 0 off, 1 one line, 2 every number (F2 steps through them).
    pub overlay: u8,
    /// The most looseness field effects (fronts, the lens, a pull past a scroll end, a press held) may
    /// give text to read and controls (DESIGN.md §1.15). Morphs, flows and recomposition are exempt.
    pub legible_cap: f32,
    /// Looseness per pixel is clamped to this after every source is summed.
    pub loose_max: f32,
    /// How changed glyphs recompose (DESIGN.md §1.16).
    pub recompose: crate::morph::Recompose,
    /// When morphs run on the GPU, and the forces on its particles (DESIGN.md §1.10).
    pub particles: crate::morph::Particles,
    /// The interaction character a preset gives (DESIGN.md §4.1): how long a press's burst lasts, what
    /// text and controls and what everything else do when nothing says, and how a commit arrives
    /// (0 ms: the classic front from the press).
    pub press_ms: f32,
    pub roles: [crate::el::Response; 2],
    pub transition: crate::preset::Transition,
    /// How scrolling looks (DESIGN.md §1.8.3).
    pub scroll: Scroll,
    /// The finishing pass: post effects, dither and palette (DESIGN.md §1.21); all off by default.
    pub post: crate::post::Post,
    /// Marks on 3D elements by view depth (DESIGN.md §1.18).
    pub marks: Marks,
    /// The field map: trails of moving things, how they spread and settle (DESIGN.md §1.29).
    pub map: MapFx,
    /// How long `scribblr`'s line holds a pose before it takes the next (ms; 0: every frame). Its width,
    /// opacity and the fill's fade are not held (DESIGN.md §1.30).
    pub hold_ms: f32,
}
pub use crate::fieldmap::MapFx;
/// How marks on a 3D element follow its depth: nearer parts get larger marks (`depth_scale`: the
/// farthest part's cells are `1 − depth_scale` of the nearest's) and are brighter (`depth_dim`: the
/// farthest is dimmed by this much, 0–1). Both 0: depth does nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Marks { pub depth_scale: f32, pub depth_dim: f32 }
impl Default for Look {
    fn default() -> Self { Self { fx: Fx::default(), theme: DARK, material: Material::Dot, pics: [PicFx::default(); 8], params: MaterialParams::all(), gaze: Gaze::default(), loose_scale: 1.0, quality: 1.0, overlay: 0, legible_cap: LEGIBLE_CAP, loose_max: 1.0, recompose: crate::morph::Recompose::default(), particles: crate::morph::Particles::default(), press_ms: 350.0, roles: [crate::el::Response::Nudge, crate::el::Response::Loosen], transition: crate::preset::Transition { duration_ms: 0.0, steps: [crate::preset::Step { kind: crate::preset::StepKind::None, from: 0.0, to: 0.0, a: 0.0, b: 0.0, c: 0.0, d: 0.0, e: 0.0 }; 4] }, scroll: Scroll::default(), post: crate::post::Post::default(), marks: Marks::default(), map: MapFx::default(), hold_ms: 0.0 } }
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
pub fn styles() -> [Style; 9] {
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
    ]
}

/// The material with this name or number.
pub fn material_named(id: &str) -> Result<Material, crate::error::LookError> {
    let found = MATERIALS.iter().position(|(_, n)| *n == id).or_else(|| id.parse::<usize>().ok().filter(|&i| i < MATERIALS.len()));
    found.map(|i| MATERIALS[i].0).ok_or_else(|| crate::error::LookError::UnknownMaterial { given: id.to_owned(), known: MATERIALS.iter().map(|m| m.1).collect::<Vec<_>>().join(", "), last: MATERIALS.len() - 1 })
}
/// Short ASCII ids of the themes, in `style::THEMES` order.
pub const THEME_IDS: [&str; 15] = ["dark", "light", "pixel", "paper", "cream", "cool", "manual", "normcore", "cyber", "bauhaus", "y2k", "led", "gaze", "glitch", "kraft"];
/// The theme index with this id, name or number.
pub fn theme_named(id: &str) -> Result<usize, crate::error::LookError> {
    THEME_IDS.iter().position(|t| *t == id).or_else(|| THEMES.iter().position(|t| t.name == id)).or_else(|| id.parse::<usize>().ok().filter(|&i| i < THEMES.len()))
        .ok_or_else(|| crate::error::LookError::UnknownTheme { given: id.to_owned(), known: THEME_IDS.join(", "), last: THEMES.len() - 1 })
}

