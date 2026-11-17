//! The accessibility mirror for native screen readers (AccessKit): the same nodes the web mirror gets
//! (`Ui::a11y`), as an AccessKit tree under one window node, and the actions assistive technology asks
//! for turned into the runtime's own focus, activation and keys.
use crate::el::{Key, Live, Mods, Role};
use crate::ui::{A11yNode, Ui};
use accesskit::{Action, ActionRequest, Invalid, Node, NodeId, Rect, Toggled, TreeId, TreeInfo, TreeUpdate};
use std::collections::HashMap;

/// The window's node.
pub const ROOT: NodeId = NodeId(0);

/// A stable id for an element's key (never 0, the window's).
pub fn id(key: &str) -> NodeId {
    // FNV-1a: stable across runs, so assistive technology keeps its place over rebuilds.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x0100_0000_01b3); }
    NodeId(h.max(1))
}

fn role(r: Role) -> accesskit::Role {
    use accesskit::Role as A;
    match r {
        Role::Button => A::Button, Role::Switch => A::Switch, Role::Slider => A::Slider, Role::SpinButton => A::SpinButton,
        Role::TabList => A::TabList, Role::Tab => A::Tab, Role::TextBox => A::TextInput, Role::Dialog => A::Dialog, Role::List => A::List,
        Role::ListItem => A::ListItem, Role::Img => A::Image, Role::Heading => A::Heading, Role::Group => A::Group, Role::Text => A::Label,
        Role::Checkbox => A::CheckBox, Role::Radio => A::RadioButton, Role::RadioGroup => A::RadioGroup, Role::Combobox => A::ComboBox,
        Role::Listbox => A::ListBox, Role::Option => A::ListBoxOption, Role::Tooltip => A::Tooltip, Role::Status => A::Status,
        Role::ProgressBar => A::ProgressIndicator, Role::Region => A::Region, Role::Table => A::Table, Role::Row => A::Row, Role::Cell => A::Cell,
        Role::ColumnHeader => A::ColumnHeader, Role::Navigation => A::Navigation, Role::Menu => A::Menu, Role::MenuItem => A::MenuItem,
        Role::Separator => A::Splitter, Role::Toolbar => A::Toolbar, Role::Link => A::Link,
    }
}

fn node(n: &A11yNode, scale: f64) -> Node {
    let mut a = Node::new(role(n.role));
    if !n.name.is_empty() { a.set_label(n.name.clone()); }
    let r = n.rect;
    a.set_bounds(Rect { x0: r.x as f64 * scale, y0: r.y as f64 * scale, x1: (r.x + r.w) as f64 * scale, y1: (r.y + r.h) as f64 * scale });
    if let Some((v, lo, hi)) = n.value {
        a.set_numeric_value(v as f64);
        a.set_min_numeric_value(lo as f64);
        a.set_max_numeric_value(hi as f64);
        a.add_action(Action::Increment);
        a.add_action(Action::Decrement);
    }
    if let Some(t) = &n.text { a.set_value(t.clone()); }
    if let Some(c) = n.checked { a.set_toggled(if c { Toggled::True } else { Toggled::False }); }
    if let Some(s) = n.selected { a.set_selected(s); }
    if let Some(e) = n.expanded { a.set_expanded(e); }
    if let Some(d) = &n.description { a.set_description(d.clone()); }
    if let Some(l) = n.live { a.set_live(match l { Live::Polite => accesskit::Live::Polite, Live::Assertive => accesskit::Live::Assertive }); }
    if n.invalid { a.set_invalid(Invalid::True); }
    if n.required { a.set_required(); }
    if n.level > 0 { a.set_level(n.level as usize); }
    if n.modal { a.set_modal(); }
    if n.disabled { a.set_disabled(); }
    if n.focusable { a.add_action(Action::Focus); }
    if n.focusable || matches!(n.role, Role::Button | Role::Link | Role::Tab | Role::Option | Role::MenuItem | Role::Checkbox | Role::Radio | Role::Switch) {
        a.add_action(Action::Click);
    }
    a
}

/// The whole tree: the window, and every mirrored element under its nearest mirrored ancestor in
/// document order. `scale` is device px per logical px (AccessKit's bounds are in window px).
pub fn tree(nodes: &[A11yNode], focus: Option<&str>, scale: f32, title: &str) -> TreeUpdate {
    let scale = scale as f64;
    let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for n in nodes { children.entry(n.parent.as_deref().map_or(ROOT, id)).or_default().push(id(&n.key)); }
    let mut out = Vec::with_capacity(nodes.len() + 1);
    let mut window = Node::new(accesskit::Role::Window);
    window.set_label(title.to_owned());
    window.set_children(children.remove(&ROOT).unwrap_or_default());
    out.push((ROOT, window));
    for n in nodes {
        let i = id(&n.key);
        let mut a = node(n, scale);
        if let Some(c) = children.remove(&i) { a.set_children(c); }
        out.push((i, a));
    }
    let focus = focus.filter(|k| nodes.iter().any(|n| n.key == *k)).map_or(ROOT, id);
    let mut info = TreeInfo::new(ROOT);
    info.toolkit_name = Some("amenui".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    TreeUpdate { nodes: out, tree: Some(info), tree_id: TreeId::ROOT, focus }
}

/// Carry out what assistive technology asked: focus, activate, or step a value (as the arrow keys
/// would). Returns whether anything happened.
pub fn act<S, M: Clone>(ui: &mut Ui<S, M>, req: &ActionRequest) -> bool {
    let Some(key) = ui.a11y().into_iter().find(|n| id(&n.key) == req.target_node).map(|n| n.key) else { return false };
    match req.action {
        Action::Focus => { ui.focus_key(&key); true }
        Action::Click => ui.activate(&key),
        Action::Increment | Action::Decrement => {
            ui.focus_key(&key);
            let k = if req.action == Action::Increment { Key::Right } else { Key::Left };
            ui.key_mods(k, Mods::default())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::{El, Paint, col, text};

    #[derive(Clone)]
    enum Msg { Go }
    fn view(n: &u32) -> El<Msg> {
        col().child(text(format!("Count {n}"), 14.0, Paint::TEXT).key("count").role(Role::Status))
            .child(crate::widgets::button("go", "Go", Msg::Go))
    }
    fn update(n: &mut u32, _: Msg) { *n += 1; }

    #[test]
    fn the_tree_has_every_mirrored_element_and_actions_reach_the_runtime() {
        let faces = crate::text::load_bundled(crate::text::ASSETS).expect("fonts");
        let mut ui = Ui::new(0u32, view, update, faces, 400, 300, 2.0);
        let nodes = ui.a11y();
        let t = tree(&nodes, None, 2.0, "Test");
        assert_eq!(t.nodes.len(), nodes.len() + 1);
        assert_eq!(t.focus, ROOT);
        let status = t.nodes.iter().find(|(i, _)| *i == id("count")).expect("status node");
        assert_eq!(status.1.role(), accesskit::Role::Status);
        assert_eq!(status.1.live(), Some(accesskit::Live::Polite));
        let go = nodes.iter().find(|n| n.key == "go").expect("button");
        let b = t.nodes.iter().find(|(i, _)| *i == id("go")).expect("button node");
        assert_eq!(b.1.bounds().map(|r| r.x0), Some(go.rect.x as f64 * 2.0));
        assert!(act(&mut ui, &ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: id("go"), data: None }));
        assert_eq!(ui.state, 1);
        assert!(!act(&mut ui, &ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: NodeId(12345), data: None }));
    }
}
