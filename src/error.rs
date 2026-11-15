//! Errors on the public path, written for whoever reads them (often a model writing a view): each one
//! says what was wrong, where (the element's key path when there is one), and what to write instead.
use thiserror::Error;

/// A font file could not be read.
#[derive(Debug, Error)]
pub enum FontError {
    #[error("font file `{path}` could not be read ({source}). The bundled faces live in `assets/` at the repository root: call `text::load_bundled(\"assets\")` from the repository root, or pass an absolute path such as `concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets\")`.")]
    Missing { path: String, #[source] source: std::io::Error },
}

/// A material, theme or page named by an id that does not exist.
#[derive(Debug, Error, PartialEq)]
pub enum LookError {
    #[error("unknown material `{given}`. Write one of: {known} (or its number 0–{last}).")]
    UnknownMaterial { given: String, known: String, last: usize },
    #[error("unknown theme `{given}`. Write one of: {known} (or its number 0–{last}).")]
    UnknownTheme { given: String, known: String, last: usize },
    #[error("unknown page `{given}`. Write a gallery page hash such as `button`, `number`, `material-dot` or `scene-morph`, or a recipe: {recipes}.")]
    UnknownPage { given: String, recipes: String },
    #[error("unknown language `{given}`. Write one of: {known}.")]
    UnknownLocale { given: String, known: String },
}

/// The off-screen renderer could not start.
#[derive(Debug, Error)]
pub enum SurfaceError {
    #[error("the surface is {w} × {h} device px; both sides must be at least 1. Pass a size such as `--size 1280x800` (and a scale of 1 or more).")]
    ZeroSize { w: u32, h: u32 },
    #[error("no GPU adapter (not even a software one) was found. Install Mesa's lavapipe (`mesa-vulkan-drivers`) for a CPU Vulkan driver, or run on a machine with a GPU.")]
    NoAdapter,
    #[error("the GPU device could not be opened: {0}")]
    Device(String),
    #[error("the off-screen target is {have_w} × {have_h} device px but {w} × {h} was asked for. Make the `Offscreen` with the same size as the page (`Offscreen::try_new(w, h)`).")]
    SizeMismatch { w: u32, h: u32, have_w: u32, have_h: u32 },
}

/// A script of input events could not be played (`drive::play`, the `render` tool).
#[derive(Debug, Error)]
pub enum DriveError {
    #[error("the events in `{file}` are not a JSON list of events ({message}). Each event is an object such as {{\"at_ms\": 100, \"kind\": \"press\", \"target\": \"button/live\"}}; kinds are press, release, move (x, y or target), key (key: \"Enter\", shift, cmd, alt), wheel (x, y or target, dx, dy) and text (text).")]
    BadEvents { file: String, message: String },
    #[error("no element has the key `{key}`. Keys on this page that look close: {near}. Run with `--tree out.json` to see every key.")]
    UnknownTarget { key: String, near: String },
    #[error("a pointer event needs a position: give `x` and `y` (logical px) or `target` (an element's key).")]
    NoPosition,
    #[error("`{name}` is not a key name. Use DOM names: Enter, Backspace, Delete, Escape, Tab, ArrowUp/Down/Left/Right, Home, End, PageUp, PageDown, F1–F24, or a single character such as \"a\" or \" \".")]
    UnknownKey { name: String },
    #[error("event {index}: {inner}")]
    AtEvent { index: usize, inner: Box<DriveError> },
    #[error(transparent)]
    Look(#[from] LookError),
    #[error(transparent)]
    Surface(#[from] SurfaceError),
    #[error(transparent)]
    Font(#[from] FontError),
}

/// Something about a view that the runtime had to guess at. `Ui::problems()` lists them after each
/// layout; none of them stops the page from drawing.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ViewProblem {
    #[error("the key `{key}` is used by two elements (at `{first}` and at `{second}`). Keys identify elements across views (birth times, scroll offsets, focus, recomposition); give each its own, e.g. `.key(format!(\"{key}/{{i}}\"))` inside a loop.")]
    DuplicateKey { key: String, first: String, second: String },
    #[error("the flow id `{flow}` is on two elements of the same page (`{first}` and `{second}`). A flow id names one subtree that moves to its place on the next page; give the second one another id.")]
    FlowTwice { flow: String, first: String, second: String },
    #[error("`{key}` is anchored to `{anchor}`, which is not laid out before it in this view. Anchor to an element that comes earlier in the tree (the button that opens a menu), or drop `.anchor(..)`.")]
    UnknownAnchor { key: String, anchor: String },
    #[error("`{key}` sends fronts to `Scope::Container(Some(\"{container}\"))`, but no element has that key. Write the key of an element on the page, or `Scope::Container(None)` and mark the region with `.ripple_scope()`.")]
    UnknownContainer { key: String, container: String },
}

/// A glTF file could not be read into a mesh (`mesh::load_gltf`).
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MeshError {
    #[error("the glTF JSON could not be parsed ({0}). Pass the bytes of a `.gltf` file (JSON) or a `.glb` file (binary).")]
    Json(String),
    #[error("the glTF file ends before the data it describes (an accessor, a buffer view or a GLB chunk reaches past the end). Export it again, or embed the buffers.")]
    Truncated,
    #[error("the glTF file has no {0}. Only triangle meshes with positions are read; export the scene with its meshes.")]
    Missing(&'static str),
    #[error("the glTF file refers to an external buffer `{0}…`. Only buffers inside the file are read: export as `.glb`, or as `.gltf` with embedded (data: URI) buffers.")]
    ExternalBuffer(String),
    #[error("the glTF file uses {0}, which this loader does not read (positions, normals, indices and node transforms only; no skins, morph targets or textures).")]
    Unsupported(&'static str),
    #[error("a buffer's base64 data has the character `{0}`, which is not base64.")]
    Base64(char),
}

/// A preset could not be read or changed (`preset::Preset`).
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PresetError {
    #[error("the preset is not valid JSON for a preset ({0}). Export one from the 风格 page to see every field; numbers stay numbers, `material` is a name such as \"dot\", step kinds are names such as \"reveal\".")]
    Json(String),
    #[error("the preset has no number at `{0}`. Paths are the JSON keys joined by dots, array places by their index: `transition.steps.0.a`, `interaction.press_ms`.")]
    NoSuchNumber(String),
}

/// A message catalog (`assets/i18n/<tag>.txt`) could not be used.
#[derive(Debug, Error, PartialEq)]
pub enum I18nError {
    #[error("no text for `{key}` in {chain}: add a line `{key} = …` to {files} (for a plural message, `{key}.one = …` and `{key}.other = …` in English, `{key}.other = …` in Chinese)")]
    MissingKey { key: String, chain: String, files: String },
    #[error("line {line} of the catalog is neither a comment nor `key = text`: `{text}` (a continuation line starts with two spaces)")]
    Syntax { line: usize, text: String },
    #[error("line {line} of the catalog gives `{key}` a second time; keep one of them")]
    Duplicate { line: usize, key: String },
}
