//! The pages of the gallery: one per component, one per mark set, and the type. Every component page
//! has the same parts, top to bottom: title, one sentence, 预览, 变体, 参数, 源码.
use super::{Gallery, IMAGE_SLOT, MARK_SLOT, Msg, Page, Target, WIPE_SPEC};
use crate::demo::{self, BUST, PICTURE_NAMES, PICTURE_SIZES, SKY};
use crate::el::{El, Font, Paint, col, picture, row, space, text, text_lh};
use crate::style::set;
use crate::text::{MONO, PIXEL, SANS};
use crate::widgets::{
    Edit, Spec, TextEdit, Variant, button_as, card, choice, dialog, divider, image, input, list, number, scroll_area, segmented,
    slider, switch, switch_sized, tabs, vdivider,
};
use taffy::{AlignItems, JustifyContent};

pub struct ParamDef { pub name: &'static str, pub spec: Spec, pub default: f32 }
const fn p(name: &'static str, min: f32, max: f32, step: f32, decimals: usize, unit: &'static str, default: f32) -> ParamDef {
    ParamDef { name, spec: Spec { min, max, step, decimals, unit }, default }
}

/// One component: its address, names, what it is for, its file, and its own numbers.
pub struct Comp {
    pub hash: &'static str,
    pub title: &'static str,
    /// The function to call.
    pub func: &'static str,
    pub desc: &'static str,
    pub file: &'static str,
    pub source: &'static str,
    pub params: &'static [ParamDef],
}

pub const BUTTON: usize = 0;
pub const NUMBER: usize = 1;
pub const INPUT: usize = 2;
pub const SWITCH: usize = 3;
pub const SLIDER: usize = 4;
pub const TABS: usize = 5;
pub const CHOICE: usize = 6;
pub const LIST: usize = 7;
pub const CARD: usize = 8;
pub const DIALOG: usize = 9;
pub const IMAGE: usize = 10;
pub const DIVIDER: usize = 11;
pub const SCROLL: usize = 12;

pub const COMPONENTS: [Comp; 13] = [
    Comp { hash: "button", title: "按钮", func: "button / button_as", desc: "执行一个操作。按下时立即触发，也可以用 Enter 或空格键触发。",
        file: "src/widgets/button.rs", source: include_str!("../widgets/button.rs"),
        params: &[p("高度", 20.0, 60.0, 1.0, 0, "px", 30.0), p("左右留白", 0.0, 40.0, 1.0, 0, "px", 14.0), p("圆角", 0.0, 20.0, 1.0, 0, "px", 4.0)] },
    Comp { hash: "number", title: "数字", func: "number", desc: "精确设置一个数值：获得焦点后直接键入，或用方向键和两侧的按钮按步长改；在数值上横向拖动可以连续调。",
        file: "src/widgets/number.rs", source: include_str!("../widgets/number.rs"),
        params: &[p("最小值", -1000.0, 1000.0, 1.0, 0, "", 0.0), p("最大值", -1000.0, 1000.0, 1.0, 0, "", 100.0), p("步长", 0.01, 100.0, 0.5, 2, "", 0.5), p("小数位", 0.0, 4.0, 1.0, 0, "", 1.0)] },
    Comp { hash: "input", title: "输入框", func: "input", desc: "输入一行文字。左右方向键、Home 和 End 移动插入点，退格键和 Delete 删除。",
        file: "src/widgets/input.rs", source: include_str!("../widgets/input.rs"),
        params: &[p("宽度", 120.0, 600.0, 10.0, 0, "px", 300.0), p("最多字数", 1.0, 200.0, 1.0, 0, "", 40.0)] },
    Comp { hash: "switch", title: "开关", func: "switch / switch_sized", desc: "在两种状态之间切换一项设置。",
        file: "src/widgets/switch.rs", source: include_str!("../widgets/switch.rs"),
        params: &[p("轨道高度", 12.0, 40.0, 1.0, 0, "px", 18.0)] },
    Comp { hash: "slider", title: "滑块", func: "slider", desc: "在一个范围内大致地选一个值。需要精确的值时，用数字控件。",
        file: "src/widgets/slider.rs", source: include_str!("../widgets/slider.rs"),
        params: &[p("最小值", -1000.0, 1000.0, 1.0, 0, "", 0.0), p("最大值", -1000.0, 1000.0, 1.0, 0, "", 100.0), p("步长", 0.1, 50.0, 0.1, 1, "", 1.0), p("宽度", 120.0, 640.0, 10.0, 0, "px", 320.0)] },
    Comp { hash: "tabs", title: "标签页", func: "tabs", desc: "在同一位置的几组内容之间切换。",
        file: "src/widgets/tabs.rs", source: include_str!("../widgets/tabs.rs"),
        params: &[p("标签间距", 0.0, 60.0, 1.0, 0, "px", 18.0), p("下划线粗细", 1.0, 6.0, 0.5, 1, "px", 2.0)] },
    Comp { hash: "choice", title: "选项", func: "choice / segmented", desc: "从几个互斥的选项中选一个。",
        file: "src/widgets/choice.rs", source: include_str!("../widgets/choice.rs"),
        params: &[p("高度", 20.0, 48.0, 1.0, 0, "px", 30.0)] },
    Comp { hash: "list", title: "列表", func: "list", desc: "显示一组条目，可以选中其中一项；条目多时可以滚动。",
        file: "src/widgets/list.rs", source: include_str!("../widgets/list.rs"),
        params: &[p("行高", 24.0, 64.0, 1.0, 0, "px", 36.0), p("可见高度", 100.0, 480.0, 4.0, 0, "px", 232.0), p("行数", 1.0, 200.0, 1.0, 0, "", 40.0)] },
    Comp { hash: "card", title: "卡片", func: "card", desc: "把相关的内容放在一个带边框的区域里。",
        file: "src/widgets/card.rs", source: include_str!("../widgets/card.rs"),
        params: &[p("内边距", 0.0, 48.0, 1.0, 0, "px", 18.0), p("圆角", 0.0, 24.0, 1.0, 0, "px", 6.0), p("线宽", 0.0, 4.0, 0.5, 1, "px", 1.0)] },
    Comp { hash: "dialog", title: "对话框", func: "dialog", desc: "在页面之上显示需要处理的内容。关闭之前，页面的其余部分不响应操作；Esc 或关闭按钮关闭它。",
        file: "src/widgets/dialog.rs", source: include_str!("../widgets/dialog.rs"),
        params: &[p("宽度", 280.0, 720.0, 10.0, 0, "px", 420.0), p("下方松开度", 0.0, 1.0, 0.05, 2, "", 0.35)] },
    Comp { hash: "image", title: "图片", func: "image", desc: "按原比例显示一张图片。图片松开时变成记号；指针在图上移动时，图会在指针附近松开。",
        file: "src/widgets/image.rs", source: include_str!("../widgets/image.rs"),
        params: &[p("点距", 1.0, 64.0, 1.0, 0, "px", 8.0), p("亮度级数", 2.0, 16.0, 1.0, 0, "", 8.0)] },
    Comp { hash: "divider", title: "分隔线", func: "divider / vdivider", desc: "分开两组内容。",
        file: "src/widgets/divider.rs", source: include_str!("../widgets/divider.rs"),
        params: &[p("粗细", 0.5, 8.0, 0.5, 1, "px", 1.0), p("上下留白", 0.0, 48.0, 1.0, 0, "px", 12.0)] },
    Comp { hash: "scroll-area", title: "滚动区", func: "scroll_area", desc: "在固定的高度里显示更长的内容，用滚轮滚动；一侧的细条表示当前位置。",
        file: "src/widgets/scroll.rs", source: include_str!("../widgets/scroll.rs"),
        params: &[p("高度", 80.0, 480.0, 4.0, 0, "px", 180.0), p("行数", 1.0, 200.0, 1.0, 0, "", 24.0)] },
];

pub fn page(s: &Gallery) -> El<Msg> {
    match s.page {
        Page::Comp(i) => comp_page(s, i),
        Page::Marks(k) => marks_page(s, k),
        Page::Type | Page::Demo => type_page(s),
    }
}

// ── Parts shared by the pages ────────────────────────────────────────────────────────────────────

fn header(s: &Gallery, h: &str, title: &str, ident: &str, desc: &str) -> El<Msg> {
    let sz = s.theme().sizes;
    col().key(format!("{h}/header")).gap(10.0)
        .child(row().key(format!("{h}/title-row")).wrap().gap(16.0).align(AlignItems::BASELINE)
            .child(text_lh(title, sz.display, (sz.display * 1.3).round(), Paint::INK).font(Font::Display).key(format!("{h}/title")))
            .child(text(ident, 13.0, Paint::DIM).font(Font::Mono)))
        .child(text_lh(desc, sz.body, (sz.body * 1.7).round(), Paint::DIM).key(format!("{h}/desc")))
}

fn section(h: &str, name: &str, body: El<Msg>) -> El<Msg> {
    col().key(format!("{h}/{name}")).gap(12.0).fill_width()
        .child(text(name, 13.0, Paint::DIM))
        .child(body)
}

/// The live component on the page ground, with room around it.
fn preview(h: &str, body: El<Msg>) -> El<Msg> {
    col().key(format!("{h}/preview")).fill_width().pad(40.0, 36.0).gap(16.0).radius(6.0).border(1.0, Paint::FAINT).align(AlignItems::START).child(body)
}

/// Every variant and state, each with a small caption.
fn variants(h: &str, items: Vec<(&str, El<Msg>)>) -> El<Msg> {
    let mut g = row().key(format!("{h}/variants")).fill_width().wrap().gap(14.0).align(AlignItems::STRETCH);
    for (i, (cap, el)) in items.into_iter().enumerate() {
        g = g.child(col().key(format!("{h}/v{i}")).gap(12.0).pad(16.0, 14.0).radius(6.0).border(1.0, Paint::FAINT).justify(JustifyContent::SPACE_BETWEEN)
            .child(el)
            .child(text(cap, 12.0, Paint::DIM)));
    }
    g
}

fn params(s: &Gallery, c: usize) -> El<Msg> {
    let h = COMPONENTS[c].hash;
    let mut g = row().key(format!("{h}/params")).fill_width().wrap().gap(10.0);
    for (i, d) in COMPONENTS[c].params.iter().enumerate() {
        let t = Target::Param(c, i);
        g = g.child(number(&format!("{h}/p{i}"), d.name, s.param(c, i), d.spec, s.draft(t), move |e| Msg::Num(t, e)).width(330.0));
    }
    g
}

/// A source file in the mono face, line numbers in the dim tone, in a scroll area. Long lines wrap
/// under their own number.
fn source(h: &str, file: &str, src: &str) -> El<Msg> {
    let mut lines = col().key(format!("{h}/lines")).fill_width();
    for (i, l) in src.lines().enumerate() {
        lines = lines.child(row().gap(14.0).align(AlignItems::START)
            .child(row().width(30.0).justify(JustifyContent::END).child(text_lh((i + 1).to_string(), 12.0, 18.0, Paint::DIM).font(Font::Mono)))
            .child(text_lh(if l.is_empty() { " " } else { l }, 12.0, 18.0, Paint::TEXT).font(Font::Mono).grow(1.0)));
    }
    col().key(format!("{h}/source")).fill_width().gap(8.0)
        .child(text(file, 12.0, Paint::DIM).font(Font::Mono))
        .child(scroll_area(&format!("{h}/code"), 380.0, lines).fill_width())
}

fn comp_page(s: &Gallery, c: usize) -> El<Msg> {
    let comp = &COMPONENTS[c];
    let h = comp.hash;
    let (live, vars) = component(s, c);
    col().key(format!("{h}/page")).fill_width().max_width(980.0).gap(34.0)
        .child(header(s, h, comp.title, comp.func, comp.desc))
        .child(section(h, "预览", preview(h, live)))
        .child(section(h, "变体", variants(h, vars)))
        .child(section(h, "参数", params(s, c)))
        .child(section(h, "源码", source(h, comp.file, comp.source)))
        .child(space(1.0, 24.0))
}

fn shown(e: Edit) -> Msg { Msg::Num(Target::Shown, e) }

/// The live preview of a component and its variants.
fn component(s: &Gallery, c: usize) -> (El<Msg>, Vec<(&'static str, El<Msg>)>) {
    let h = COMPONENTS[c].hash;
    let k = |n: &str| format!("{h}/{n}");
    let pr = |i: usize| s.param(c, i);
    let sz = s.theme().sizes;
    match c {
        BUTTON => {
            let live = row().gap(16.0).wrap().align(AlignItems::CENTER)
                .child(button_as(k("live"), "按下", Variant::Default, Msg::Press).height(pr(0)).pad(pr(1), 0.0).radius(pr(2)))
                .child(button_as(k("live-accent"), "保存", Variant::Accent, Msg::Press).height(pr(0)).pad(pr(1), 0.0).radius(pr(2)))
                .child(text(format!("按下了 {} 次", s.presses), sz.body, Paint::DIM).key(k("count")));
            (live, vec![
                ("默认", button_as(k("a"), "默认", Variant::Default, Msg::Press)),
                ("安静", button_as(k("b"), "安静", Variant::Quiet, Msg::Press)),
                ("强调", button_as(k("c"), "强调", Variant::Accent, Msg::Press)),
                ("不可用", button_as(k("d"), "不可用", Variant::Default, Msg::Press).disabled(true)),
                ("强调，不可用", button_as(k("e"), "强调", Variant::Accent, Msg::Press).disabled(true)),
            ])
        }
        NUMBER => {
            let spec = number_spec(s);
            let live = number(&k("live"), "数值", s.value, spec, s.draft(Target::Value), |e| Msg::Num(Target::Value, e)).width(360.0);
            let int = Spec { min: 0.0, max: 10.0, step: 1.0, decimals: 0, unit: "" };
            (live, vec![
                ("整数", number(&k("a"), "数量", 3.0, int, None, shown).width(260.0)),
                ("两位小数", number(&k("b"), "比例", 0.25, Spec { min: 0.0, max: 1.0, step: 0.05, decimals: 2, unit: "" }, None, shown).width(260.0)),
                ("带单位", number(&k("c"), "宽度", 12.0, Spec { min: 0.0, max: 64.0, step: 1.0, decimals: 0, unit: "px" }, None, shown).width(260.0)),
                ("正在键入", number(&k("d"), "数量", 3.0, int, Some("12"), shown).width(260.0)),
                ("不可用", number(&k("e"), "数量", 3.0, int, None, shown).width(260.0).disabled(true)),
            ])
        }
        INPUT => {
            let live = col().gap(10.0)
                .child(input(&k("live"), &s.text, "输入一些文字", pr(0), pr(1) as usize, Msg::Text))
                .child(text(format!("{} 个字", s.text.text.chars().count()), sz.small, Paint::DIM).key(k("count")));
            (live, vec![
                ("空，显示占位文字", input(&k("a"), &TextEdit::default(), "名称", 220.0, 40, |_| Msg::Nothing)),
                ("有内容", input(&k("b"), &TextEdit::new("Studio 4"), "名称", 220.0, 40, |_| Msg::Nothing)),
                ("不可用", input(&k("c"), &TextEdit::new("Studio 4"), "名称", 220.0, 40, |_| Msg::Nothing).disabled(true)),
            ])
        }
        SWITCH => {
            let live = switch_sized(k("live"), "显示网格", s.on, Some(("关", "开")), pr(0), Msg::Switch).width(320.0);
            (live, vec![
                ("关", switch(k("a"), "通知", false, None, Msg::Nothing).width(200.0)),
                ("开", switch(k("b"), "通知", true, None, Msg::Nothing).width(200.0)),
                ("带状态名", switch(k("c"), "形状", true, Some(("圆点", "方块")), Msg::Nothing).width(200.0)),
                ("不可用", switch(k("d"), "通知", true, None, Msg::Nothing).width(200.0).disabled(true)),
            ])
        }
        SLIDER => {
            let spec = slider_spec(s);
            let live = row().gap(16.0).align(AlignItems::CENTER)
                .child(slider(&k("live"), s.level, spec, pr(3), Msg::Level))
                .child(text(spec.show(s.level), sz.body, Paint::TEXT).font(Font::Mono).key(k("value")));
            let v = Spec { min: 0.0, max: 100.0, step: 1.0, decimals: 0, unit: "" };
            (live, vec![
                ("最小", slider(&k("a"), 0.0, v, 180.0, |_| Msg::Nothing)),
                ("中间", slider(&k("b"), 50.0, v, 180.0, |_| Msg::Nothing)),
                ("最大", slider(&k("c"), 100.0, v, 180.0, |_| Msg::Nothing)),
                ("不可用", slider(&k("d"), 30.0, v, 180.0, |_| Msg::Nothing).disabled(true)),
            ])
        }
        TABS => {
            const PANELS: [&str; 3] = ["概览：这一组内容的摘要。", "参数：可以调整的数值都在这里。", "记录：最近的操作按时间排列。"];
            let live = tabs(&k("live"), &["概览", "参数", "记录"], s.tab, pr(0), pr(1), Msg::Tab,
                text(PANELS[s.tab.min(2)], sz.body, Paint::TEXT).key(k(&format!("panel{}", s.tab))));
            let small = |key: &str, cur: usize| tabs(&k(key), &["全部", "图片", "文字"], cur, 14.0, 2.0, |_| Msg::Nothing, text("内容", 13.0, Paint::DIM));
            (live.width(420.0), vec![("选中第一个", small("a", 0)), ("选中第二个", small("b", 1))])
        }
        CHOICE => {
            let opts = ["小", "中", "大"];
            // A definite width: a wrapping row in a column that sizes to its content would be measured
            // at its narrowest and wrap every option.
            let live = col().width(440.0).gap(18.0)
                .child(choice(&k("live"), "尺寸", &opts, s.pick, Msg::Pick))
                .child(segmented(&k("live-seg"), &opts, s.pick, pr(0), Msg::Pick));
            (live, vec![
                ("choice", choice(&k("a"), "尺寸", &opts, 0, |_| Msg::Nothing)),
                ("segmented", segmented(&k("b"), &opts, 1, 30.0, |_| Msg::Nothing)),
                ("segmented，不可用", segmented(&k("c"), &opts, 2, 30.0, |_| Msg::Nothing).disabled(true)),
            ])
        }
        LIST => {
            let rows: Vec<(String, String)> = (0..pr(2) as usize).map(|i| (format!("第 {} 项", i + 1), format!("Item {:02}", i + 1))).collect();
            let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            let few = [("照片", "JPG"), ("文稿", "MD"), ("录音", "M4A"), ("视频", "MP4")];
            (list(&k("live"), &refs, s.row, pr(0), pr(1), Msg::Row).width(440.0), vec![
                ("没有选中", list(&k("a"), &few, None, 32.0, 150.0, |_| Msg::Nothing).width(240.0)),
                ("选中一项", list(&k("b"), &few, Some(1), 32.0, 150.0, |_| Msg::Nothing).width(240.0)),
            ])
        }
        CARD => {
            let body = text_lh("卡片把一组相关的内容放在一起。它只是一个带边框的区域，里面可以放任何东西。", sz.body, (sz.body * 1.7).round(), Paint::TEXT);
            let live = card(k("live"), Some("卡片标题"), body).pad(pr(0), pr(0)).radius(pr(1)).border(pr(2), Paint::FAINT).width(440.0);
            (live, vec![
                ("带标题", card(k("a"), Some("标题"), text("内容", 13.0, Paint::TEXT)).width(220.0)),
                ("不带标题", card(k("b"), None, text("内容", 13.0, Paint::TEXT)).width(220.0)),
            ])
        }
        DIALOG => {
            let (w, beneath) = (pr(0), pr(1));
            let form = col().gap(14.0)
                .child(text_lh("给这一组设置换一个名称。", sz.body, (sz.body * 1.7).round(), Paint::TEXT))
                .child(input(&k("name"), &s.dialog_text, "名称", w - 44.0, 40, Msg::DialogText))
                .child(row().gap(10.0).justify(JustifyContent::END)
                    .child(button_as(k("cancel"), "取消", Variant::Default, Msg::Dialog(0)))
                    .child(button_as(k("ok"), "确定", Variant::Accent, Msg::Dialog(0))));
            let note = col().gap(14.0)
                .child(text_lh("这个操作不能撤销。", sz.body, (sz.body * 1.7).round(), Paint::TEXT))
                .child(row().gap(10.0).justify(JustifyContent::END).child(button_as(k("ok2"), "好", Variant::Accent, Msg::Dialog(0))));
            let live = row().gap(16.0).align(AlignItems::CENTER)
                .child(button_as(k("open"), "打开对话框", Variant::Default, Msg::Dialog(1)))
                .child(text(if s.dialog == 0 { "对话框已关闭" } else { "对话框已打开" }, sz.body, Paint::DIM).key(k("state")))
                .child(dialog(&k("dialog"), s.dialog == 1, "重命名", w, beneath, Msg::Dialog(0), form))
                .child(dialog(&k("dialog2"), s.dialog == 2, "删除", w, beneath, Msg::Dialog(0), note));
            (live, vec![
                ("带输入框", button_as(k("a"), "重命名…", Variant::Default, Msg::Dialog(1))),
                ("只有说明", button_as(k("b"), "删除…", Variant::Default, Msg::Dialog(2))),
            ])
        }
        IMAGE => {
            let ratio = |id: u32| { let (w, h) = PICTURE_SIZES[id as usize]; h as f32 / w as f32 };
            let live = col().width(560.0).child(image(&k("live"), BUST, ratio(BUST), IMAGE_SLOT, Some("在图上移动指针，图会在指针附近松开成记号。")));
            (live, vec![
                ("原图", col().width(220.0).child(image(&k("a"), SKY, ratio(SKY), IMAGE_SLOT, None))),
                ("完全松开", col().width(220.0).child(picture(SKY, ratio(SKY)).key(k("b")).pic_fx(IMAGE_SLOT).loose(1.0))),
                ("带说明", col().width(220.0).child(image(&k("c"), SKY, ratio(SKY), IMAGE_SLOT, Some("傍晚的天空")))),
            ])
        }
        DIVIDER => {
            let gap = pr(1);
            let live = col().width(440.0).gap(gap)
                .child(text("上面一组", sz.body, Paint::TEXT))
                .child(divider(pr(0)))
                .child(text("下面一组", sz.body, Paint::TEXT));
            let side = row().height(40.0).gap(14.0).align(AlignItems::STRETCH)
                .child(row().align(AlignItems::CENTER).child(text("左", 13.0, Paint::TEXT))).child(vdivider(1.0))
                .child(row().align(AlignItems::CENTER).child(text("右", 13.0, Paint::TEXT)));
            (live, vec![
                ("横向", col().width(180.0).gap(10.0).child(text("上面", 13.0, Paint::TEXT)).child(divider(1.0)).child(text("下面", 13.0, Paint::TEXT))),
                ("竖向", side),
                ("粗", col().width(180.0).gap(10.0).child(text("上面", 13.0, Paint::TEXT)).child(divider(3.0)).child(text("下面", 13.0, Paint::TEXT))),
            ])
        }
        SCROLL => {
            let lines = |n: usize, key: &str| {
                let mut c = col().key(k(key)).gap(6.0);
                for i in 0..n { c = c.child(text(format!("第 {} 行：滚动区里的一行文字。", i + 1), 14.0, Paint::TEXT)); }
                c
            };
            (scroll_area(&k("live"), pr(0), lines(pr(1) as usize, "live-lines")).width(440.0), vec![
                ("内容不多", scroll_area(&k("a"), 110.0, lines(3, "a-lines")).width(240.0)),
                ("内容较多", scroll_area(&k("b"), 110.0, lines(20, "b-lines")).width(240.0)),
            ])
        }
        _ => (space(0.0, 0.0), vec![]),
    }
}

/// The live number's spec, from the number page's own numbers (kept in order).
fn number_spec(s: &Gallery) -> Spec {
    let pr = |i| s.param(NUMBER, i);
    let (lo, hi) = (pr(0).min(pr(1)), pr(0).max(pr(1)));
    Spec { min: lo, max: hi, step: pr(2).max(0.01), decimals: pr(3) as usize, unit: "" }
}
fn slider_spec(s: &Gallery) -> Spec {
    let pr = |i| s.param(SLIDER, i);
    let (lo, hi) = (pr(0).min(pr(1)), pr(0).max(pr(1)).max(pr(0).min(pr(1)) + pr(2)));
    Spec { min: lo, max: hi, step: pr(2).max(0.1), decimals: if pr(2).fract() == 0.0 { 0 } else { 1 }, unit: "" }
}

// ── Mark pages ───────────────────────────────────────────────────────────────────────────────────

const MARK_DESC: [&str; set::COUNT] = [
    "按实际量出的墨量排序的字符：越亮的地方用墨越多的字符。字符按显示时的点距绘制，点距 8 到 10 时仍然清楚。",
    "8 × 8 Bayer 抖动图案，适合低像素画面和限色。",
    "实心色块，每格一侧有一条很淡的格线。",
    "实心块，四边略暗，像一片片瓷砖。",
    "实心块，中间有一个圆形凸点。",
    "圆点的面积随亮度变化。",
    "十字的粗细随亮度变化。",
    "斜线的粗细随亮度变化，相邻的格子连成排线。",
    "菱形的大小随亮度变化。",
    "横线的粗细随亮度变化。",
    "带一圈辉光的软边光点，暗处没有。",
    "短线，方向跟着图片的明暗走。",
];

fn pic_value(f: &crate::style::PicFx, i: usize) -> f32 { let mut c = *f; let (v, k) = demo::pic_slot(&mut c, i); *v * k }

fn marks_page(s: &Gallery, k: u8) -> El<Msg> {
    let h = Page::Marks(k).hash();
    let h = h.as_str();
    let f = &s.marks[k as usize];
    let (w, ph) = PICTURE_SIZES[s.shown as usize];
    let mut nums = row().key(format!("{h}/params")).fill_width().wrap().gap(10.0);
    for (i, prm) in demo::PIC_PARAMS.iter().enumerate() {
        let t = Target::Pic(k, i);
        nums = nums.child(number(&format!("{h}/p{i}"), prm.name, pic_value(f, i), prm.spec, s.draft(t), move |e| Msg::Num(t, e)).width(330.0));
    }
    let view = col().fill_width().gap(14.0)
        .child(row().child(segmented(&format!("{h}/picture-choice"), &PICTURE_NAMES, s.shown as usize, 30.0, |i| Msg::Show(i as u32))))
        .child(col().max_width(820.0).child(picture(s.shown, ph as f32 / w as f32).key(format!("{h}/picture")).pic_fx(MARK_SLOT).wipe(s.wipe).on_drag(|x, _| Msg::Wipe(x))))
        .child(text_lh("对比线左侧是原图，右侧是完全松开后的记号。拖动图片可以移动这条线。", 13.0, 22.0, Paint::DIM))
        .child(number(&format!("{h}/wipe"), "对比线位置", s.wipe, WIPE_SPEC, s.draft(Target::Wipe), |e| Msg::Num(Target::Wipe, e)).width(330.0));
    col().key(format!("{h}/page")).fill_width().max_width(980.0).gap(34.0)
        .child(header(s, h, set::NAMES[k as usize], &format!("set::{}", super::MARK_HASH[k as usize].to_uppercase()), MARK_DESC[k as usize]))
        .child(section(h, "预览", view))
        .child(section(h, "参数", nums))
        .child(space(1.0, 24.0))
}

// ── Type ─────────────────────────────────────────────────────────────────────────────────────────

fn type_page(s: &Gallery) -> El<Msg> {
    let h = "type";
    let sz = s.theme().sizes;
    // Departure Mono has no CJK, so it gets a Latin sample (anything else would be set in the fallback).
    let line = |key: String, label: String, size: f32, face: u8| row().key(key).gap(20.0).align(AlignItems::BASELINE)
        .child(row().width(150.0).child(text(label, 12.0, Paint::DIM).font(Font::Face(MONO))))
        .child(text_lh(if face == MONO { "Departure Mono 0123 ABC abc" } else { "界面上的字 Type 0123" }, size, (size * 1.4).round(), Paint::INK).font(Font::Face(face)).grow(1.0));
    let mut sans = col().key("type/sans").gap(10.0).fill_width();
    for (name, size) in [("显示", sz.display), ("标题", sz.title), ("正文", sz.body), ("小字", sz.small)] {
        sans = sans.child(line(format!("type/sans/{name}"), format!("{name} {size}"), size, SANS));
    }
    let mut pixel = col().key("type/pixel").gap(10.0).fill_width();
    for m in [1.0f32, 2.0, 3.0] {
        pixel = pixel.child(line(format!("type/fusion/{m}"), format!("Fusion Pixel {}", 12.0 * m), 12.0 * m, PIXEL));
    }
    let mut mono = col().key("type/mono").gap(10.0).fill_width();
    for m in [1.0f32, 2.0, 3.0] {
        mono = mono.child(line(format!("type/departure/{m}"), format!("Departure Mono {}", 11.0 * m), 11.0 * m, MONO));
    }
    col().key("type/page").fill_width().max_width(980.0).gap(34.0)
        .child(header(s, h, "字号与字体", "Theme::sizes", "主题的字号阶，以及两套像素字体。像素字体只按字面的整数倍显示，笔画边缘只有墨色和底色。"))
        .child(section(h, "比例字体 Noto Sans SC", sans))
        .child(section(h, "像素字体 Fusion Pixel 12", pixel))
        .child(section(h, "像素字体 Departure Mono 11", mono))
        .child(space(1.0, 24.0))
}
