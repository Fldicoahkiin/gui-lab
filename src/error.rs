//! Errors on the public path, written for whoever reads them (often a model writing a view): each one
//! says what was wrong, where (the element's key path when there is one), and what to write instead.
#![warn(missing_docs)]
use thiserror::Error;

/// A font file could not be read.
#[derive(Debug, Error)]
pub enum FontError {
    #[error("font file `{path}` could not be read ({source}). The bundled faces live in `assets/` at the repository root: call `text::load_bundled(\"assets\")` from the repository root, or pass an absolute path such as `concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets\")`.")]
    /// The file could not be read.
    Missing {
        /// The file.
        path: String,
        /// Why.
        #[source]
        source: std::io::Error,
    },
}

/// A material, theme or page named by an id that does not exist.
#[derive(Debug, Error, PartialEq)]
pub enum LookError {
    #[error("unknown material `{given}`; the closest is `{nearest}`. Write one of: {known} (or its number 0–{last}).")]
    /// No material has that name or number.
    UnknownMaterial {
        /// What was asked for.
        given: String,
        /// The name it is closest to (for a name materials had before, the name it has now).
        nearest: String,
        /// The names there are.
        known: String,
        /// The highest number.
        last: usize,
    },
    #[error("unknown theme `{given}`. Write one of: {known} (or its number 0–{last}).")]
    /// No theme has that name or number.
    UnknownTheme {
        /// What was asked for.
        given: String,
        /// The names there are.
        known: String,
        /// The highest number.
        last: usize,
    },
    #[error("unknown page `{given}`. Write a recipe ({recipes}) or a page the application adds, such as a site page hash.")]
    /// No recipe or application page has that name.
    UnknownPage {
        /// What was asked for.
        given: String,
        /// The recipes there are.
        recipes: String,
    },
    #[error("unknown language `{given}`. Write one of: {known}.")]
    /// No language has that tag.
    UnknownLocale {
        /// What was asked for.
        given: String,
        /// The tags there are.
        known: String,
    },
}

/// The off-screen renderer could not start.
#[derive(Debug, Error)]
pub enum SurfaceError {
    #[error("the surface is {w} × {h} device px; both sides must be at least 1. Pass a size such as `--size 1280x800` (and a scale of 1 or more).")]
    /// A side of the surface is 0.
    ZeroSize {
        /// Width, device px.
        w: u32,
        /// Height, device px.
        h: u32,
    },
    #[error("no GPU adapter (not even a software one) was found. Install Mesa's lavapipe (`mesa-vulkan-drivers`) for a CPU Vulkan driver, or run on a machine with a GPU.")]
    /// No GPU adapter, not even a software one.
    NoAdapter,
    #[error("the GPU device could not be opened: {0}")]
    /// The adapter would not open a device.
    Device(String),
    #[error("the off-screen target is {have_w} × {have_h} device px but {w} × {h} was asked for. Make the `Offscreen` with the same size as the page (`Offscreen::try_new(w, h)`).")]
    /// The off-screen target has another size.
    SizeMismatch {
        /// The width asked for.
        w: u32,
        /// The height asked for.
        h: u32,
        /// The target's width.
        have_w: u32,
        /// The target's height.
        have_h: u32,
    },
}

/// A script of input events could not be played (`drive::play`, the `render` tool).
#[derive(Debug, Error)]
pub enum DriveError {
    #[error("the events in `{file}` are not a JSON list of events ({message}). Each event is an object such as {{\"at_ms\": 100, \"kind\": \"press\", \"target\": \"button/live\"}}; kinds are press, release, move (x, y or target), key (key: \"Enter\", shift, cmd, alt), wheel (x, y or target, dx, dy) and text (text).")]
    /// The file is not a JSON list of events.
    BadEvents {
        /// The file.
        file: String,
        /// What the JSON reader said.
        message: String,
    },
    #[error("no element has the key `{key}`. Keys on this page that look close: {near}. Run with `--tree out.json` to see every key.")]
    /// An event targets a key no element has.
    UnknownTarget {
        /// The key.
        key: String,
        /// Keys on the page that look close.
        near: String,
    },
    #[error("a pointer event needs a position: give `x` and `y` (logical px) or `target` (an element's key).")]
    /// A pointer event has neither a position nor a target.
    NoPosition,
    #[error("`{name}` is not a key name. Use DOM names: Enter, Backspace, Delete, Escape, Tab, ArrowUp/Down/Left/Right, Home, End, PageUp, PageDown, F1–F24, or a single character such as \"a\" or \" \".")]
    /// A key event names no key.
    UnknownKey {
        /// The name given.
        name: String,
    },
    #[error("{given} frames a second is not one the fixed 60 Hz clock can step by: use 60, 30, 20, 15 or 12.")]
    /// `frames` was asked for a rate that does not divide the 60 Hz clock.
    BadFps {
        /// The rate given.
        given: u32,
    },
    #[error("the frames {from}..{to} are empty: give the first frame and one past the last, e.g. 0..120.")]
    /// `frames` was given an empty range.
    NoFrames {
        /// The first frame.
        from: u32,
        /// One past the last.
        to: u32,
    },
    #[error("event {index}: {inner}")]
    /// An error in one event of the list.
    AtEvent {
        /// The event's place in the list, from 0.
        index: usize,
        /// What was wrong with it.
        inner: Box<DriveError>,
    },
    #[error(transparent)]
    /// The page, theme or material asked for.
    Look(#[from] LookError),
    #[error(transparent)]
    /// The off-screen renderer.
    Surface(#[from] SurfaceError),
    #[error(transparent)]
    /// The fonts.
    Font(#[from] FontError),
}

/// Something about a view that the runtime had to guess at. `Ui::problems()` lists them after each
/// layout; none of them stops the page from drawing.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ViewProblem {
    #[error("the key `{key}` is used by two elements (at `{first}` and at `{second}`). Keys identify elements across views (birth times, scroll offsets, focus, recomposition); give each its own, e.g. `.key(format!(\"{key}/{{i}}\"))` inside a loop.")]
    /// Two elements have the same key.
    DuplicateKey {
        /// The key.
        key: String,
        /// Where the first one is (its key path).
        first: String,
        /// Where the second one is.
        second: String,
    },
    #[error("the flow id `{flow}` is on two elements of the same page (`{first}` and `{second}`). A flow id names one subtree that moves to its place on the next page; give the second one another id.")]
    /// Two elements of a page have the same flow id.
    FlowTwice {
        /// The flow id.
        flow: String,
        /// The first element's key.
        first: String,
        /// The second element's key.
        second: String,
    },
    #[error("`{key}` is anchored to `{anchor}`, which is not laid out before it in this view. Anchor to an element that comes earlier in the tree (the button that opens a menu), or drop `.anchor(..)`.")]
    /// An element is anchored to one not laid out before it.
    UnknownAnchor {
        /// The anchored element.
        key: String,
        /// The key it is anchored to.
        anchor: String,
    },
    #[error("`{key}` sends fronts to `Scope::Container(Some(\"{container}\"))`, but no element has that key. Write the key of an element on the page, or `Scope::Container(None)` and mark the region with `.ripplr_scope()`.")]
    /// Fronts are scoped to a container key no element has.
    UnknownContainer {
        /// The element sending fronts.
        key: String,
        /// The container's key.
        container: String,
    },
}

/// A glTF file could not be read into a mesh (`mesh::load_gltf`).
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MeshError {
    #[error("the glTF JSON could not be parsed ({0}). Pass the bytes of a `.gltf` file (JSON) or a `.glb` file (binary).")]
    /// The JSON could not be parsed.
    Json(String),
    #[error("the glTF file ends before the data it describes (an accessor, a buffer view or a GLB chunk reaches past the end). Export it again, or embed the buffers.")]
    /// The data described runs past the end of the file.
    Truncated,
    #[error("the glTF file has no {0}. Only triangle meshes with positions are read; export the scene with its meshes.")]
    /// Something the loader needs is not there.
    Missing(&'static str),
    #[error("the glTF file refers to an external buffer `{0}…`. Only buffers inside the file are read: export as `.glb`, or as `.gltf` with embedded (data: URI) buffers.")]
    /// A buffer lives in another file.
    ExternalBuffer(String),
    #[error("the glTF file uses {0}, which this loader does not read (positions, normals, indices and node transforms only; no skins, morph targets or textures).")]
    /// A feature the loader does not read.
    Unsupported(&'static str),
    #[error("a buffer's base64 data has the character `{0}`, which is not base64.")]
    /// Embedded data is not base64.
    Base64(char),
}

/// A preset could not be read or changed (`preset::Preset`).
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PresetError {
    #[error("the preset is not valid JSON for a preset ({0}). Export one from the 风格 page to see every field; numbers stay numbers, `material` is a name such as \"dot\", step kinds are names such as \"reveal\".")]
    /// Not a preset's JSON.
    Json(String),
    #[error("the preset has no number at `{0}`. Paths are the JSON keys joined by dots, array places by their index: `transition.steps.0.a`, `interaction.press_ms`.")]
    /// No number at that path.
    NoSuchNumber(String),
    #[error("not a look recipe ({0}). A recipe is `look:v2:` and then base64url of a small JSON object: `base` (a preset's hash, such as \"normcore\") and `set` (paths as `numbers` names them, with their values). Copy one from the 风格 page, or from a PNG the render tool wrote (its `amenui-recipe` text chunk).")]
    /// Not a look recipe.
    Recipe(String),
}

/// A message catalog (`assets/i18n/<tag>.txt`) could not be used.
#[derive(Debug, Error, PartialEq)]
pub enum I18nError {
    #[error("no text for `{key}` in {chain}: add a line `{key} = …` to {files} (for a plural message, `{key}.one = …` and `{key}.other = …` in English, `{key}.other = …` in Chinese)")]
    /// No catalog in the chain has the key.
    MissingKey {
        /// The key.
        key: String,
        /// The locales looked in, in order.
        chain: String,
        /// The catalog files to add it to.
        files: String,
    },
    #[error("line {line} of the catalog is neither a comment nor `key = text`: `{text}` (a continuation line starts with two spaces)")]
    /// A line is neither a comment nor `key = text`.
    Syntax {
        /// Its number, from 1.
        line: usize,
        /// The line.
        text: String,
    },
    #[error("line {line} of the catalog gives `{key}` a second time; keep one of them")]
    /// A key is given twice.
    Duplicate {
        /// The second one's line, from 1.
        line: usize,
        /// The key.
        key: String,
    },
    #[error("{path}: {error}")]
    /// An error in one catalog file.
    Catalog {
        /// The file.
        path: String,
        /// What was wrong.
        error: String,
    },
    #[error("another set of catalogs is already installed; call `i18n::install` once, at start")]
    /// `i18n::install` was called a second time.
    AlreadyInstalled,
}

/// `amenui add` could not do what it was asked.
#[derive(Debug, Error, PartialEq)]
pub enum AddError {
    #[error("`{name}` is not a component in registry.json. Did you mean `{near}`? The components are: {known}.")]
    /// No component has that name.
    Unknown {
        /// The name given.
        name: String,
        /// The closest name there is.
        near: String,
        /// Every component.
        known: String,
    },
    #[error("registry.json could not be read: {0}")]
    /// The registry is not what it should be.
    Registry(String),
    #[error("{path}: {msg}")]
    /// A file could not be written.
    Io {
        /// The file or folder.
        path: String,
        /// Why.
        msg: String,
    },
}

/// A native window could not be opened (`app::native::run`).
#[derive(Debug, Error)]
pub enum WindowError {
    #[error("no window system here ({0}). On Linux, run under X11 or Wayland (or `xvfb-run` for a virtual display); on a server without a display, draw off-screen with `offscreen::Offscreen` or the `render` tool instead.")]
    /// There is no window system.
    NoEventLoop(String),
    #[error("the window could not be created: {0}")]
    /// The window could not be created.
    Create(String),
    #[error("the window's surface could not be made or configured: {0}")]
    /// Its surface could not be made or configured.
    Surface(String),
    #[error(transparent)]
    /// No adapter or device.
    Gpu(#[from] SurfaceError),
    #[error("the event loop stopped with an error: {0}")]
    /// The event loop failed.
    EventLoop(String),
}

/// A material file's header could not be read (`materials::header`).
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MaterialError {
    #[error("the material file does not begin with its name. Its first line is `// @material <name>`, then one `// @param` line for each of its numbers.")]
    /// The first line is not `// @material <name>`.
    NoName,
    #[error("line {line} of the material file, `// @param {text}`, is not a number's description. Write `// @param <label> <min> <max> <step> <decimals> <unit, or -> <default>`, such as `// @param grain 0 1 0.05 2 - 0.6`.")]
    /// A `@param` line does not have its seven fields.
    Param {
        /// The line, counted from 1.
        line: usize,
        /// What follows `@param`.
        text: String,
    },
    #[error("`{name}` cannot name a material: write lowercase letters, digits and `_`, starting with a letter (it becomes part of the shader's function names).")]
    /// The name is not a lowercase identifier.
    BadName {
        /// The name given.
        name: String,
    },
    #[error("`{name}` is a built-in material. Add your copy under a name of its own, such as `my_{name}`.")]
    /// The name is a built-in material's.
    BuiltIn {
        /// The name given.
        name: String,
    },
    #[error("the material file has no `fn m_{name}(p: vec2f) -> vec3f`: a material's function is named after the name in its first line.")]
    /// The file lacks its function.
    NoFunction {
        /// The name in its header.
        name: String,
    },
    #[error("{max} materials have been added already, the most there can be. Add fewer, or add one again under a name it already has to replace it.")]
    /// No id is free.
    Full {
        /// How many can be added.
        max: usize,
    },
    #[error("the material file describes {count} numbers; a material has at most eight.")]
    /// More than eight numbers.
    TooMany {
        /// How many there are.
        count: usize,
    },
}
