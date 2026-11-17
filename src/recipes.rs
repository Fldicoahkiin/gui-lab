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
#[path = "../examples/chapter.rs"] pub mod chapter;
#[path = "../examples/gesture.rs"] pub mod gesture;
#[path = "../examples/submenu.rs"] pub mod submenu;
#[path = "../examples/loading.rs"] pub mod loading;
#[path = "../examples/stack.rs"] pub mod stack;
#[path = "../examples/chart.rs"] pub mod chart;
#[path = "../examples/form3d.rs"] pub mod form3d;

/// Every recipe: its name, its source (the file in `examples/`), and what it shows.
pub const RECIPES: [(&str, &str, &str); 17] = [
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
    ("chapter", include_str!("../examples/chapter.rs"), "A chapter: one scene with actors that flow in."),
    ("gesture", include_str!("../examples/gesture.rs"), "A bottom sheet and a floating box driven by springs."),
    ("submenu", include_str!("../examples/submenu.rs"), "A menu with submenus and a safe zone."),
    ("chart", include_str!("../examples/chart.rs"), "A dot chart with a scrubbing marker and draggable points."),
    ("stack", include_str!("../examples/stack.rs"), "A stack of cards that fans under the pointer."),
    ("loading", include_str!("../examples/loading.rs"), "A list that loads: nothing, its own skeleton, then the rows in turn."),
    ("form3d", include_str!("../examples/form3d.rs"), "A passage wrapped on a sphere: depth, facing, thinning, and the pointer on the surface."),
];
pub const NAMES: [&str; 17] = ["form", "list", "dialog", "flow", "morph", "material", "annotate", "counter", "mesh", "preset", "chapter", "gesture", "submenu", "loading", "stack", "chart", "form3d"];

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
        "chapter" => Box::new(chapter::ui(faces, w, h, scale)),
        "gesture" => Box::new(gesture::ui(faces, w, h, scale)),
        "submenu" => Box::new(submenu::ui(faces, w, h, scale)),
        "loading" => Box::new(loading::ui(faces, w, h, scale)),
        "stack" => Box::new(stack::ui(faces, w, h, scale)),
        "chart" => Box::new(chart::ui(faces, w, h, scale)),
        "form3d" => Box::new(form3d::ui(faces, w, h, scale)),
        _ => return None,
    })
}

/// The components' examples (`examples/components/`), compiled, for the documentation pages: each
/// `ex_*` function is a preview and, as its source, the code shown under it.
#[path = "../examples/components/button.rs"] pub mod ex_button;
#[path = "../examples/components/menu.rs"] pub mod ex_menu;
#[path = "../examples/components/orbit.rs"] pub mod ex_orbit;
#[path = "../examples/components/number.rs"] pub mod ex_number;
#[path = "../examples/components/input.rs"] pub mod ex_input;
#[path = "../examples/components/switch.rs"] pub mod ex_switch;
#[path = "../examples/components/slider.rs"] pub mod ex_slider;
#[path = "../examples/components/tabs.rs"] pub mod ex_tabs;
#[path = "../examples/components/choice.rs"] pub mod ex_choice;
#[path = "../examples/components/list.rs"] pub mod ex_list;
#[path = "../examples/components/card.rs"] pub mod ex_card;
#[path = "../examples/components/dialog.rs"] pub mod ex_dialog;
#[path = "../examples/components/image.rs"] pub mod ex_image;
#[path = "../examples/components/divider.rs"] pub mod ex_divider;
#[path = "../examples/components/scroll.rs"] pub mod ex_scroll;
#[path = "../examples/components/checkbox.rs"] pub mod ex_checkbox;
#[path = "../examples/components/radio.rs"] pub mod ex_radio;
#[path = "../examples/components/select.rs"] pub mod ex_select;
#[path = "../examples/components/tooltip.rs"] pub mod ex_tooltip;
#[path = "../examples/components/popover.rs"] pub mod ex_popover;
#[path = "../examples/components/toast.rs"] pub mod ex_toast;
#[path = "../examples/components/progress.rs"] pub mod ex_progress;
#[path = "../examples/components/accordion.rs"] pub mod ex_accordion;
#[path = "../examples/components/table.rs"] pub mod ex_table;
#[path = "../examples/components/badge.rs"] pub mod ex_badge;
#[path = "../examples/components/avatar.rs"] pub mod ex_avatar;
#[path = "../examples/components/skeleton.rs"] pub mod ex_skeleton;
#[path = "../examples/components/breadcrumb.rs"] pub mod ex_breadcrumb;
#[path = "../examples/components/pagination.rs"] pub mod ex_pagination;
#[path = "../examples/components/sheet.rs"] pub mod ex_sheet;
#[path = "../examples/components/context_menu.rs"] pub mod ex_context_menu;
#[path = "../examples/components/command.rs"] pub mod ex_command;
#[path = "../examples/components/toggle.rs"] pub mod ex_toggle;
#[path = "../examples/components/bottom_sheet.rs"] pub mod ex_bottom_sheet;
#[path = "../examples/components/floating.rs"] pub mod ex_floating;
#[path = "../examples/components/caption.rs"] pub mod ex_caption;
#[path = "../examples/components/nested_menu.rs"] pub mod ex_nested_menu;
#[path = "../examples/components/card_stack.rs"] pub mod ex_card_stack;
#[path = "../examples/components/dot_chart.rs"] pub mod ex_dot_chart;
pub mod components {
    pub use super::{ex_menu as menu, ex_orbit as orbit, ex_button as button, ex_number as number, ex_input as input, ex_switch as switch, ex_slider as slider, ex_tabs as tabs, ex_choice as choice, ex_list as list, ex_card as card, ex_dialog as dialog, ex_image as image, ex_divider as divider, ex_scroll as scroll, ex_checkbox as checkbox, ex_radio as radio, ex_select as select, ex_tooltip as tooltip, ex_popover as popover, ex_toast as toast, ex_progress as progress, ex_accordion as accordion, ex_table as table, ex_badge as badge, ex_avatar as avatar, ex_skeleton as skeleton, ex_breadcrumb as breadcrumb, ex_pagination as pagination, ex_sheet as sheet, ex_context_menu as context_menu, ex_command as command, ex_toggle as toggle, ex_bottom_sheet as bottom_sheet, ex_floating as floating, ex_caption as caption, ex_nested_menu as nested_menu, ex_card_stack as card_stack, ex_dot_chart as dot_chart};
    /// (component, source, its examples as elements).
    pub fn all() -> Vec<(&'static str, &'static str)> { vec![
        ("button", include_str!("../examples/components/button.rs")),
        ("menu", include_str!("../examples/components/menu.rs")),
        ("orbit", include_str!("../examples/components/orbit.rs")),
        ("number", include_str!("../examples/components/number.rs")),
        ("input", include_str!("../examples/components/input.rs")),
        ("switch", include_str!("../examples/components/switch.rs")),
        ("slider", include_str!("../examples/components/slider.rs")),
        ("tabs", include_str!("../examples/components/tabs.rs")),
        ("choice", include_str!("../examples/components/choice.rs")),
        ("list", include_str!("../examples/components/list.rs")),
        ("card", include_str!("../examples/components/card.rs")),
        ("dialog", include_str!("../examples/components/dialog.rs")),
        ("image", include_str!("../examples/components/image.rs")),
        ("divider", include_str!("../examples/components/divider.rs")),
        ("scroll", include_str!("../examples/components/scroll.rs")),
        ("checkbox", include_str!("../examples/components/checkbox.rs")),
        ("radio", include_str!("../examples/components/radio.rs")),
        ("select", include_str!("../examples/components/select.rs")),
        ("tooltip", include_str!("../examples/components/tooltip.rs")),
        ("popover", include_str!("../examples/components/popover.rs")),
        ("toast", include_str!("../examples/components/toast.rs")),
        ("progress", include_str!("../examples/components/progress.rs")),
        ("accordion", include_str!("../examples/components/accordion.rs")),
        ("table", include_str!("../examples/components/table.rs")),
        ("badge", include_str!("../examples/components/badge.rs")),
        ("avatar", include_str!("../examples/components/avatar.rs")),
        ("skeleton", include_str!("../examples/components/skeleton.rs")),
        ("breadcrumb", include_str!("../examples/components/breadcrumb.rs")),
        ("pagination", include_str!("../examples/components/pagination.rs")),
        ("sheet", include_str!("../examples/components/sheet.rs")),
        ("context_menu", include_str!("../examples/components/context_menu.rs")),
        ("command", include_str!("../examples/components/command.rs")),
        ("toggle", include_str!("../examples/components/toggle.rs")),
        ("bottom_sheet", include_str!("../examples/components/bottom_sheet.rs")),
        ("floating", include_str!("../examples/components/floating.rs")),
        ("caption", include_str!("../examples/components/caption.rs")),
        ("nested_menu", include_str!("../examples/components/nested_menu.rs")),
        ("card_stack", include_str!("../examples/components/card_stack.rs")),
        ("dot_chart", include_str!("../examples/components/dot_chart.rs")),
    ] }
    /// The examples of `name`, as elements sending `()`.
    pub fn examples(name: &str) -> Vec<crate::el::El<()>> {
        fn f<M: 'static>(v: Vec<crate::el::El<M>>) -> Vec<crate::el::El<()>> { v.into_iter().map(|e| e.map(std::rc::Rc::new(|_| ()))).collect() }
        match name {
            "menu" => f(vec![menu::ex_0()]),
            "orbit" => f(vec![orbit::ex_0()]),
            "button" => f(vec![button::ex_0(), button::ex_1(), button::ex_2(), button::ex_3()]),
            "number" => f(vec![number::ex_0(), number::ex_1()]),
            "input" => f(vec![input::ex_0(), input::ex_1(), input::ex_2()]),
            "switch" => f(vec![switch::ex_0(), switch::ex_1()]),
            "slider" => f(vec![slider::ex_0()]),
            "tabs" => f(vec![tabs::ex_0()]),
            "choice" => f(vec![choice::ex_0(), choice::ex_1()]),
            "list" => f(vec![list::ex_0()]),
            "card" => f(vec![card::ex_0()]),
            "dialog" => f(vec![dialog::ex_0()]),
            "image" => f(vec![image::ex_0()]),
            "divider" => f(vec![divider::ex_0()]),
            "scroll" => f(vec![scroll::ex_0()]),
            "checkbox" => f(vec![checkbox::ex_0(), checkbox::ex_1(), checkbox::ex_2()]),
            "radio" => f(vec![radio::ex_0()]),
            "select" => f(vec![select::ex_0()]),
            "tooltip" => f(vec![tooltip::ex_0()]),
            "popover" => f(vec![popover::ex_0()]),
            "toast" => f(vec![toast::ex_0()]),
            "progress" => f(vec![progress::ex_0(), progress::ex_1()]),
            "accordion" => f(vec![accordion::ex_0()]),
            "table" => f(vec![table::ex_0()]),
            "badge" => f(vec![badge::ex_0()]),
            "avatar" => f(vec![avatar::ex_0()]),
            "skeleton" => f(vec![skeleton::ex_0()]),
            "breadcrumb" => f(vec![breadcrumb::ex_0()]),
            "pagination" => f(vec![pagination::ex_0()]),
            "sheet" => f(vec![sheet::ex_0()]),
            "context_menu" => f(vec![context_menu::ex_0()]),
            "command" => f(vec![command::ex_0()]),
            "toggle" => f(vec![toggle::ex_0(), toggle::ex_1()]),
            "bottom_sheet" => f(vec![bottom_sheet::ex_0(), bottom_sheet::ex_1()]),
            "floating" => f(vec![floating::ex_0()]),
            "caption" => f(vec![caption::ex_0()]),
            "nested_menu" => f(vec![nested_menu::ex_0()]),
            "card_stack" => f(vec![card_stack::ex_0()]),
            "dot_chart" => f(vec![dot_chart::ex_0()]),
            _ => vec![],
        }
    }
}
