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
