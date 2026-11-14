//! The laid-out tree as data (DESIGN.md §1.17): what a model reads instead of a screenshot. `Ui::export_tree`
//! builds it; `serde_json::to_value` (or `to_string_pretty`) turns it into JSON.
use serde::Serialize;

/// One element as it is laid out and drawn right now.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TreeNode {
    /// The element's key (written by the view, or its path when it has none: `root/0/2`).
    pub key: String,
    /// Whether the view wrote the key (a path key changes when the tree around it changes).
    pub keyed: bool,
    /// `text`, `image`, `control`, `shape` or `box` (as the annotation layer names it).
    pub kind: &'static str,
    /// Its role and name in the accessibility mirror (WAI-ARIA role names), when it is in it.
    pub role: Option<&'static str>,
    pub label: Option<String>,
    /// Border box, logical px: x, y, width, height (after scrolling); and the part of it that is visible.
    pub rect: [f32; 4],
    pub visible: [f32; 4],
    /// Its text, when it is text.
    pub text: Option<String>,
    /// The material drawing it: `global` (the look's) or the name of its override (`El::material`).
    pub material: &'static str,
    /// How it answers fronts (`ignore`, `nudge`, `loosen`), whether it is legible (capped by
    /// `Look::legible_cap`), where its presses send fronts (`own`, `container`, `container:<key>`,
    /// `global`, `off`, or none), and whether a change of its text recomposes.
    pub response: &'static str,
    pub legible: bool,
    pub ripple: Option<String>,
    pub recompose: bool,
    /// Input state.
    pub focusable: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub disabled: bool,
    /// Its scroll offset (logical px) when it scrolls, and how long its contents are.
    pub scroll: Option<[f32; 2]>,
    /// The most loosened of its instances right now (0 at rest; see `Ui::looseness`).
    pub loose: f32,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// A node with nothing in it (the tree of an empty view).
    pub fn empty() -> Self {
        TreeNode { key: String::new(), keyed: false, kind: "box", role: None, label: None, rect: [0.0; 4], visible: [0.0; 4], text: None, material: "global",
                   response: "loosen", legible: false, ripple: None, recompose: false, focusable: false, focused: false, hovered: false, pressed: false,
                   disabled: false, scroll: None, loose: 0.0, children: vec![] }
    }
    /// This node without its children.
    pub fn clone_shallow(&self) -> Self { TreeNode { children: vec![], ..self.clone() } }
    /// The first node (depth first) whose key is `key`.
    pub fn find(&self, key: &str) -> Option<&TreeNode> {
        if self.key == key { return Some(self); }
        self.children.iter().find_map(|c| c.find(key))
    }
    /// Every node, depth first.
    pub fn walk(&self) -> Vec<&TreeNode> {
        let mut out = vec![self];
        for c in &self.children { out.extend(c.walk()); }
        out
    }
}
