//! Recipe: a menu with submenus and a safe zone (docs/recipes/submenu.md). The menu's open path and its
//! pending switch are state; the pointer's moves over it are messages (`El::on_move`), and the tick only
//! runs while a switch waits for its time (`Menu::due`).
use lab::el::{El, Paint, col, text};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::{Menu, MenuItem, MenuMsg, nested_menu};

pub struct App { pub menu: Menu, pub chosen: String, pub now: f32 }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Menu(MenuMsg) }

pub fn view(s: &App) -> El<Msg> {
    col().key("page").pad(24.0, 24.0).gap(16.0)
        .child(nested_menu("menu", &s.menu, Msg::Menu))
        .child(text(format!("选中：{}", s.chosen), 14.0, Paint::DIM).key("chosen"))
}

pub fn update(s: &mut App, m: Msg) {
    let Msg::Menu(m) = m;
    if let Some(path) = s.menu.update(m, s.now) {
        let mut items: &[MenuItem] = &s.menu.items;
        let mut names = vec![];
        for i in path { if let Some(it) = items.get(i) { names.push(it.label.clone()); items = &it.items; } }
        s.chosen = names.join(" › ");
    }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<App, Msg> {
    let sub = |name: &str, n: usize| MenuItem::sub(name, (1..=n).map(|i| MenuItem::leaf(format!("{name} {i}"))).collect());
    let mut menu = Menu::new(vec![sub("新建", 4), sub("打开最近", 5), sub("排列", 3), MenuItem::leaf("关闭")]);
    menu.show_safe_zone = true;
    Ui::new(App { menu, chosen: "—".into(), now: 0.0 }, view, update, faces, w, h, scale)
        .on_tick(|s, t| { s.now = t; s.menu.step(t) })
        // No frames while nothing waits; a waiting switch wakes the page at its time.
        .on_wake(|s, _| s.menu.due())
}
