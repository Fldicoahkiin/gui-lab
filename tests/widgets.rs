//! The components added in step 10, through the runtime: roles in the accessibility mirror, keyboard
//! behaviour per the WAI-ARIA authoring practices (checkbox, radio group, select, table, toggle group,
//! context menu, command palette, accordion, pagination), and what they draw.
use amenui::el::{El, Key, col, text, Paint};
use amenui::ui::Ui;
use amenui::widgets::*;

fn fonts() -> Vec<amenui::text::Face> { amenui::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("fonts") }

#[derive(Default)]
struct S { on: bool, radio: usize, open: bool, active: usize, city: Option<usize>, sel: Option<usize>, tg: [bool; 3], acc: [bool; 2], page: usize, menu: Option<(f32, f32)>, picked: Option<usize>, q: TextEdit, ran: Option<usize> }
#[derive(Clone, Debug)]
enum M { On, Radio(usize), Toggle, Close, City(usize), Active(usize), Sel(usize), Tg(usize), Acc(usize), Page(usize), Pick(usize), Query(TextEdit), Run(usize), Nothing }
const CITIES: [&str; 3] = ["北京", "上海", "广州"];
const CMDS: [&str; 3] = ["open file", "save all", "close window"];
fn view(s: &S) -> El<M> {
    col().gap(8.0).pad(16.0, 16.0)
        .child(checkbox("cb", "同意", s.on, M::On))
        .child(radio_group("rg", &["小", "中", "大"], s.radio, M::Radio))
        .child(select("sel", &CITIES, s.city, s.open, s.active, M::Toggle, M::Close, M::City, M::Active))
        .child(table("tb", &[("名称", 120.0), ("大小", 60.0)], &[vec!["a".into(), "1".into()], vec!["b".into(), "2".into()]], s.sel, M::Sel))
        .child(toggle_group("tg", &["B", "I", "U"], &s.tg, M::Tg))
        .child(accordion("ac", vec![("一", text("内容一", 13.0, Paint::TEXT)), ("二", text("内容二", 13.0, Paint::TEXT))], &s.acc, M::Acc))
        .child(pagination("pg", s.page, 12, M::Page))
        .child(progress("pr", Some(0.4), 200.0))
        .child(badge("bd", "新", Variant::Accent))
        .child(avatar("av", "Lin Mo", 32.0, 1))
        .child(skeleton("sk", 120.0, 12.0, false))
        .child(breadcrumb("bc", &["文档", "组件", "按钮"], |_| M::Nothing))
        .child(command("cmd", &s.q, &CMDS, s.active, M::Query, M::Run))
        .child(context_menu("cm", s.menu, &["打开", "重命名"], s.active, M::Pick, M::Active, M::Close))
}
fn update(s: &mut S, m: M) {
    match m {
        M::On => s.on = !s.on, M::Radio(i) => s.radio = i, M::Toggle => { s.open = !s.open; s.active = s.city.unwrap_or(0); }, M::Close => { s.open = false; s.menu = None; }
        M::City(i) => { s.city = Some(i); s.open = false; } M::Active(i) => s.active = i, M::Sel(i) => s.sel = Some(i), M::Tg(i) => s.tg[i] = !s.tg[i],
        M::Acc(i) => s.acc[i] = !s.acc[i], M::Page(i) => s.page = i, M::Pick(i) => { s.picked = Some(i); s.menu = None; } M::Query(q) => { s.q = q; s.active = 0; } M::Run(i) => s.ran = Some(i), M::Nothing => {}
    }
}
fn ui() -> Ui<S, M> { Ui::new(S::default(), view, update, fonts(), 900, 1400, 1.0) }
fn press(ui: &mut Ui<S, M>, k: &str) { let (x, y) = ui.node(k).unwrap_or_else(|| panic!("{k}")).rect.center(); ui.press(x, y); ui.release(x, y); }
fn role(ui: &Ui<S, M>, k: &str) -> Option<String> { ui.a11y().into_iter().find(|n| n.key == k).map(|n| n.role.aria().to_owned()) }

#[test]
fn roles_are_in_the_mirror() {
    let ui = ui();
    for (k, r) in [("cb", "checkbox"), ("rg", "radiogroup"), ("rg:1", "radio"), ("sel", "combobox"), ("tb", "table"), ("tg", "toolbar"), ("pg", "navigation"), ("pr", "progressbar"), ("av", "img"), ("bc", "navigation")] {
        assert_eq!(role(&ui, k).as_deref(), Some(r), "{k}");
    }
    assert_eq!(initials("Lin Mo"), "LM");
    assert_eq!(initials("林墨"), "林");
}

#[test]
fn keyboard_per_the_authoring_practices() {
    let mut ui = ui();
    press(&mut ui, "cb");
    assert!(ui.state.on);
    ui.key(Key::Char(' '), false);
    assert!(!ui.state.on, "Space toggles a checkbox");
    // Radio group: arrows select, wrapping.
    ui.state.cmds_focus("rg");
    press(&mut ui, "rg:0");
    ui.focus_key("rg");
    ui.key(Key::Down, false);
    assert_eq!(ui.state.radio, 1);
    ui.key(Key::Up, false); ui.key(Key::Up, false);
    assert_eq!(ui.state.radio, 2, "wraps");
    // Select: Down opens, Down moves, Enter picks, closed again.
    ui.focus_key("sel");
    ui.key(Key::Down, false);
    assert!(ui.state.open);
    assert_eq!(ui.focus(), Some("sel/list"), "the open list has the focus");
    ui.key(Key::Down, false);
    ui.key(Key::Enter, false);
    assert_eq!(ui.state.city, Some(1));
    assert!(!ui.state.open);
    // Table: one focus stop, Down selects rows.
    ui.focus_key("tb");
    ui.key(Key::Down, false); ui.key(Key::Down, false);
    assert_eq!(ui.state.sel, Some(1));
    // Toggle group, accordion, pagination by press.
    press(&mut ui, "tg:1");
    assert_eq!(ui.state.tg, [false, true, false]);
    press(&mut ui, "ac:0");
    assert!(ui.node("ac:0/panel").is_some());
    press(&mut ui, "pg/next");
    assert_eq!(ui.state.page, 1);
    // Command palette: typing filters, Enter on the field does nothing by itself; a row press runs.
    ui.focus_key("cmd/query");
    ui.text_input("sa");
    assert_eq!(command_matches("sa", &CMDS), vec![1]);
    press(&mut ui, "cmd/row0");
    assert_eq!(ui.state.ran, Some(1));
    // Context menu: opens where asked, Down then Enter picks, Esc would close.
    ui.state.menu = Some((100.0, 100.0));
    ui.state.active = 0;
    ui.relayout();
    ui.focus_key("cm/menu");
    ui.key(Key::Down, false);
    ui.key(Key::Enter, false);
    assert_eq!(ui.state.picked, Some(1));
    assert!(ui.node("cm/menu").is_none());
}
trait Noop { fn cmds_focus(&mut self, _: &str); }
impl Noop for S { fn cmds_focus(&mut self, _: &str) {} }
