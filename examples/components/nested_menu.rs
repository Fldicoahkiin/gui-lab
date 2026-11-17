//! nested_menu: its examples, as the documentation page shows them (each `ex_*` function is a preview
//! and its code).
use amenui::el::El;
use amenui::widgets::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Nothing, Menu(MenuMsg) }

/// 一个子菜单打开着 · With a submenu open
pub fn ex_0() -> El<Msg> {
    let mut m = Menu::new(vec![
        MenuItem::sub("新建", vec![MenuItem::leaf("文档"), MenuItem::leaf("表格"), MenuItem::leaf("文件夹")]),
        MenuItem::sub("打开最近", vec![MenuItem::leaf("报告"), MenuItem::leaf("草稿")]),
        MenuItem::leaf("关闭"),
    ]);
    m.update(MenuMsg::Press(0, 0), 0.0);
    nested_menu("nm1", &m, Msg::Menu)
}
