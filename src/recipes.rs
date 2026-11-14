//! The recipes (docs/recipes/): small complete applications, one file each in `examples/`, compiled into
//! the library so the tests can render them and the `render` tool can open them (`--page recipe-form`).
#[path = "../examples/form.rs"] pub mod form;
#[path = "../examples/list.rs"] pub mod list;
#[path = "../examples/dialog.rs"] pub mod dialog;
#[path = "../examples/flow.rs"] pub mod flow;
#[path = "../examples/morph.rs"] pub mod morph;
#[path = "../examples/material.rs"] pub mod material;
#[path = "../examples/annotate.rs"] pub mod annotate;
#[path = "../examples/counter.rs"] pub mod counter;
#[path = "../examples/mesh.rs"] pub mod mesh;
#[path = "../examples/preset.rs"] pub mod preset;

/// Every recipe: its name, its source (the file in `examples/`), and what it shows.
pub const RECIPES: [(&str, &str, &str); 10] = [
    ("form", include_str!("../examples/form.rs"), "A form: text field, number, switch, submit."),
    ("list", include_str!("../examples/list.rs"), "A list with selection."),
    ("dialog", include_str!("../examples/dialog.rs"), "A modal dialog."),
    ("flow", include_str!("../examples/flow.rs"), "A page change with a shared, flowing part."),
    ("morph", include_str!("../examples/morph.rs"), "Morph one element into another."),
    ("material", include_str!("../examples/material.rs"), "A custom material on one element."),
    ("annotate", include_str!("../examples/annotate.rs"), "The annotation layer on a subtree."),
    ("counter", include_str!("../examples/counter.rs"), "A counter with recomposition and a scoped ripple."),
    ("mesh", include_str!("../examples/mesh.rs"), "A 3D object in a card."),
    ("preset", include_str!("../examples/preset.rs"), "A custom preset with its own transition."),
];
pub const NAMES: [&str; 10] = ["form", "list", "dialog", "flow", "morph", "material", "annotate", "counter", "mesh", "preset"];

/// Open recipe `name` as a page to drive and draw.
#[cfg(not(target_arch = "wasm32"))]
pub fn open(name: &str, faces: Vec<crate::text::Face>, w: u32, h: u32, scale: f32) -> Option<Box<dyn crate::drive::Driven>> {
    Some(match name {
        "form" => Box::new(form::ui(faces, w, h, scale)),
        "list" => Box::new(list::ui(faces, w, h, scale)),
        "dialog" => Box::new(dialog::ui(faces, w, h, scale)),
        "flow" => Box::new(flow::ui(faces, w, h, scale)),
        "morph" => Box::new(morph::ui(faces, w, h, scale)),
        "material" => Box::new(material::ui(faces, w, h, scale)),
        "annotate" => Box::new(annotate::ui(faces, w, h, scale)),
        "counter" => Box::new(counter::ui(faces, w, h, scale)),
        "mesh" => Box::new(mesh::ui(faces, w, h, scale)),
        "preset" => Box::new(preset::ui(faces, w, h, scale)),
        _ => return None,
    })
}
