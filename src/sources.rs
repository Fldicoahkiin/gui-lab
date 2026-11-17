//! The framework's own texts and sources, compiled in: for documentation sites and tools that show them
//! (the components are distributed as source, so a page can show the file `amenui add` copies).

/// The guide for models writing interfaces with amenui.
pub const AGENTS: &str = include_str!("../AGENTS.md");
/// The short index for models.
pub const LLMS: &str = include_str!("../llms.txt");
/// The component registry `amenui add` reads.
pub const REGISTRY: &str = include_str!("../registry.json");
/// What changed, release by release.
pub const CHANGELOG: &str = include_str!("../CHANGELOG.md");
/// The transforms' source (`morph.rs`).
pub const MORPH: &str = include_str!("morph.rs");

/// Each component's source file (`src/widgets/<name>.rs`).
pub mod widget {
    pub const ACCORDION: &str = include_str!("widgets/accordion.rs");
    pub const AVATAR: &str = include_str!("widgets/avatar.rs");
    pub const BADGE: &str = include_str!("widgets/badge.rs");
    pub const BOTTOM_SHEET: &str = include_str!("widgets/bottom_sheet.rs");
    pub const BREADCRUMB: &str = include_str!("widgets/breadcrumb.rs");
    pub const BUTTON: &str = include_str!("widgets/button.rs");
    pub const CAPTION: &str = include_str!("widgets/caption.rs");
    pub const CARD: &str = include_str!("widgets/card.rs");
    pub const CARD_STACK: &str = include_str!("widgets/card_stack.rs");
    pub const CHECKBOX: &str = include_str!("widgets/checkbox.rs");
    pub const CHOICE: &str = include_str!("widgets/choice.rs");
    pub const COMMAND: &str = include_str!("widgets/command.rs");
    pub const CONTEXT_MENU: &str = include_str!("widgets/context_menu.rs");
    pub const DIALOG: &str = include_str!("widgets/dialog.rs");
    pub const DIVIDER: &str = include_str!("widgets/divider.rs");
    pub const DOT_CHART: &str = include_str!("widgets/dot_chart.rs");
    pub const FLOATING: &str = include_str!("widgets/floating.rs");
    pub const IMAGE: &str = include_str!("widgets/image.rs");
    pub const INPUT: &str = include_str!("widgets/input.rs");
    pub const LIST: &str = include_str!("widgets/list.rs");
    pub const MENU: &str = include_str!("widgets/menu.rs");
    pub const NESTED_MENU: &str = include_str!("widgets/nested_menu.rs");
    pub const NUMBER: &str = include_str!("widgets/number.rs");
    pub const ORBIT: &str = include_str!("widgets/orbit.rs");
    pub const PAGINATION: &str = include_str!("widgets/pagination.rs");
    pub const POPOVER: &str = include_str!("widgets/popover.rs");
    pub const PROGRESS: &str = include_str!("widgets/progress.rs");
    pub const RADIO: &str = include_str!("widgets/radio.rs");
    pub const SCROLL: &str = include_str!("widgets/scroll.rs");
    pub const SELECT: &str = include_str!("widgets/select.rs");
    pub const SHEET: &str = include_str!("widgets/sheet.rs");
    pub const SKELETON: &str = include_str!("widgets/skeleton.rs");
    pub const SKIP_LINK: &str = include_str!("widgets/skip_link.rs");
    pub const SLIDER: &str = include_str!("widgets/slider.rs");
    pub const SWITCH: &str = include_str!("widgets/switch.rs");
    pub const TABLE: &str = include_str!("widgets/table.rs");
    pub const TABS: &str = include_str!("widgets/tabs.rs");
    pub const TOAST: &str = include_str!("widgets/toast.rs");
    pub const TOGGLE: &str = include_str!("widgets/toggle.rs");
    pub const TOOLTIP: &str = include_str!("widgets/tooltip.rs");
    /// (file name, source) of every component.
    pub const ALL: [(&str, &str); 40] = [
        ("accordion.rs", ACCORDION),
        ("avatar.rs", AVATAR),
        ("badge.rs", BADGE),
        ("bottom_sheet.rs", BOTTOM_SHEET),
        ("breadcrumb.rs", BREADCRUMB),
        ("button.rs", BUTTON),
        ("caption.rs", CAPTION),
        ("card.rs", CARD),
        ("card_stack.rs", CARD_STACK),
        ("checkbox.rs", CHECKBOX),
        ("choice.rs", CHOICE),
        ("command.rs", COMMAND),
        ("context_menu.rs", CONTEXT_MENU),
        ("dialog.rs", DIALOG),
        ("divider.rs", DIVIDER),
        ("dot_chart.rs", DOT_CHART),
        ("floating.rs", FLOATING),
        ("image.rs", IMAGE),
        ("input.rs", INPUT),
        ("list.rs", LIST),
        ("menu.rs", MENU),
        ("nested_menu.rs", NESTED_MENU),
        ("number.rs", NUMBER),
        ("orbit.rs", ORBIT),
        ("pagination.rs", PAGINATION),
        ("popover.rs", POPOVER),
        ("progress.rs", PROGRESS),
        ("radio.rs", RADIO),
        ("scroll.rs", SCROLL),
        ("select.rs", SELECT),
        ("sheet.rs", SHEET),
        ("skeleton.rs", SKELETON),
        ("skip_link.rs", SKIP_LINK),
        ("slider.rs", SLIDER),
        ("switch.rs", SWITCH),
        ("table.rs", TABLE),
        ("tabs.rs", TABS),
        ("toast.rs", TOAST),
        ("toggle.rs", TOGGLE),
        ("tooltip.rs", TOOLTIP),
    ];
    /// The source of `src/widgets/<file>` (`button.rs`), if there is such a component.
    pub fn get(file: &str) -> Option<&'static str> { ALL.iter().find(|w| w.0 == file).map(|w| w.1) }
}

/// The source of a file `amenui add` copies, by its path in the repository (`src/widgets/button.rs`,
/// `src/el.rs`): the components and the framework files they need, compiled in so the command works
/// wherever it was installed.
pub fn file(path: &str) -> Option<&'static str> {
    match path {
        "src/el.rs" => Some(include_str!("el.rs")),
        "src/gesture.rs" => Some(include_str!("gesture.rs")),
        "src/mesh.rs" => Some(include_str!("mesh.rs")),
        "src/morph.rs" => Some(MORPH),
        "src/widgets/mod.rs" => Some(include_str!("widgets/mod.rs")),
        _ => path.strip_prefix("src/widgets/").and_then(widget::get),
    }
}
