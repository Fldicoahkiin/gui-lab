//! The demo application, written with the framework only: a counter, a picture, proportional mixed
//! text, a scrolling list, and a panel that sets every field parameter exactly.
use crate::el::{El, Paint, col, picture, row, space, text, text_lh};
use crate::scene::Fx;
use crate::widgets::{Edit, Spec, button, number, toggle};
use taffy::AlignItems;

#[derive(Default)]
pub struct Demo {
    pub count: i32,
    pub fx: Fx,
    /// The parameter being typed into and its text so far.
    pub edit: Option<(usize, String)>,
    pub picked: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Inc, Param(usize, Edit), Square, Light, Pick(usize), Reset }

pub struct Param { pub name: &'static str, pub spec: Spec }
const fn p(name: &'static str, min: f32, max: f32, step: f32, decimals: usize, unit: &'static str) -> Param {
    Param { name, spec: Spec { min, max, step, decimals, unit } }
}
/// Every number in `Fx`, in the order of `slot`.
pub const PARAMS: [Param; 11] = [
    p("字的点距", 2.0, 16.0, 0.5, 1, "px"),
    p("图的点距", 3.0, 32.0, 0.5, 1, "px"),
    p("不规则度", 0.0, 1.0, 0.05, 2, ""),
    p("错位", 0.0, 2.0, 0.05, 2, ""),
    p("波速", 100.0, 3000.0, 10.0, 0, "px/s"),
    p("波的寿命", 0.2, 6.0, 0.1, 1, "s"),
    p("波宽", 4.0, 120.0, 1.0, 0, "px"),
    p("尾长", 0.0, 600.0, 10.0, 0, "px"),
    p("推力", 0.0, 30.0, 0.5, 1, "px"),
    p("透镜半径", 0.0, 300.0, 2.0, 0, "px"),
    p("凝结时长", 0.0, 3.0, 0.05, 2, "s"),
];
pub fn slot(fx: &mut Fx, i: usize) -> &mut f32 {
    match i {
        0 => &mut fx.pitch, 1 => &mut fx.pic_pitch, 2 => &mut fx.jitter, 3 => &mut fx.glitch,
        4 => &mut fx.wave_speed, 5 => &mut fx.wave_life, 6 => &mut fx.wave_width, 7 => &mut fx.wave_tail,
        8 => &mut fx.push, 9 => &mut fx.lens, _ => &mut fx.enter,
    }
}
fn value(fx: &Fx, i: usize) -> f32 { *slot(&mut fx.clone(), i) }

pub fn update(s: &mut Demo, m: Msg) {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Param(i, Edit::Draft(d)) => s.edit = Some((i, d)),
        Msg::Param(i, Edit::Set(v)) => { *slot(&mut s.fx, i) = v; s.edit = None; }
        Msg::Param(_, Edit::Cancel) => s.edit = None,
        Msg::Square => s.fx.square = !s.fx.square,
        Msg::Light => s.fx.light = !s.fx.light,
        Msg::Pick(i) => s.picked = if s.picked == Some(i) { None } else { Some(i) },
        Msg::Reset => { s.fx = Fx { square: s.fx.square, light: s.fx.light, ..Fx::default() }; s.edit = None; }
    }
}

const PARAGRAPHS: [(&str, Paint); 3] = [
    ("在空白处按一下，会有一圈波从按下的位置传出去。波经过的字和图会松开成点阵，然后合回原样。新出现的字也会从松开的状态凝结出来。", Paint::TEXT),
    ("指针移到图上，图会在指针附近松开；指针经过文字时，文字不变。Tab 键在控件之间移动焦点，Enter 或空格键触发。", Paint::TEXT),
    ("Layout is computed by taffy and text is set by parley, in proportional type at any size. Every glyph, shape and picture is drawn by its own outline, never by a box around it.", Paint::DIM),
];

pub fn view(s: &Demo) -> El<Msg> {
    col().key("page").scroll().fill_width().fill_height().pad(24.0, 28.0)
        .child(row().key("columns").wrap().gap(28.0).align(AlignItems::START).child(content(s)).child(panel(s)))
}

fn content(s: &Demo) -> El<Msg> {
    let card = row().key("card").gap(16.0).pad(18.0, 14.0).radius(6.0).border(1.0, Paint::FAINT).align(AlignItems::CENTER)
        .child(text(format!("计数 {}", s.count), 22.0, Paint::INK).key("count").grow(1.0))
        .child(button("inc", "+1", Msg::Inc));
    let mut list = col().key("list").scroll().height(232.0).radius(6.0).border(1.0, Paint::FAINT).pad(0.0, 4.0);
    for i in 0..40 {
        let on = s.picked == Some(i);
        let mut item = row().key(format!("item/{i}")).height(36.0).pad(14.0, 0.0).gap(12.0).align(AlignItems::CENTER).on_press(Msg::Pick(i))
            .child(text(format!("第 {} 项", i + 1), 15.0, if on { Paint::ACCENT } else { Paint::TEXT }).grow(1.0))
            .child(text(format!("Item {:02}", i + 1), 13.0, Paint::DIM));
        if on { item = item.fill(Paint { tone: 0.0, accent: 1.0, alpha: 0.12 }); }
        list = list.child(item);
    }
    col().key("content").grow(1.0).basis(460.0).max_width(760.0).gap(18.0)
        .child(text_lh("界面骨架", 34.0, 44.0, Paint::INK))
        .child(text_lh("这一页的每个字、每条线和每张图都由框架自己画出：布局、文字排版、点击、焦点和滚动都在框架里。", 15.0, 24.0, Paint::DIM))
        .child(card)
        .child(picture(0, 0.5).key("picture"))
        .children(PARAGRAPHS.iter().map(|(t, p)| text_lh(*t, 16.0, 28.0, *p)))
        .child(text("列表", 14.0, Paint::DIM))
        .child(list)
        .child(space(1.0, 8.0))
}

fn panel(s: &Demo) -> El<Msg> {
    let mut panel = col().key("panel").grow(1.0).basis(330.0).max_width(380.0).gap(10.0).pad(18.0, 16.0).radius(6.0).border(1.0, Paint::FAINT)
        .child(text("参数", 16.0, Paint::INK));
    for (i, prm) in PARAMS.iter().enumerate() {
        let draft = s.edit.as_ref().filter(|(j, _)| *j == i).map(|(_, d)| d.as_str());
        panel = panel.child(number(&format!("param/{i}"), prm.name, value(&s.fx, i), prm.spec, draft, move |e| Msg::Param(i, e)));
    }
    panel
        .child(space(1.0, 2.0))
        .child(toggle("square", "点的形状", s.fx.square, ("圆点", "方块"), Msg::Square))
        .child(toggle("light", "主题", s.fx.light, ("暗", "明"), Msg::Light))
        .child(row().child(button("reset", "恢复默认", Msg::Reset)))
}

/// A picture computed in code: a lit sphere over a horizon. RGBA8, `w` × `h`.
pub fn picture_pixels(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    let (cx, cy, r) = (w as f32 * 0.6, h as f32 * 0.46, h as f32 * 0.33);
    let light = { let l = [-0.5f32, -0.62, 0.6]; let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt(); [l[0] / n, l[1] / n, l[2] / n] };
    for y in 0..h { for x in 0..w {
        let (fx, fy) = (x as f32, y as f32);
        let v = fy / h as f32;
        let horizon = 0.64;
        let mut c = if v < horizon { let k = v / horizon; [0.05 + 0.13 * k, 0.07 + 0.17 * k, 0.09 + 0.2 * k] }
                    else { let k = (v - horizon) / (1.0 - horizon); let glow = (-((fx - cx) / (w as f32 * 0.3)).powi(2)).exp() * (1.0 - k) * 0.4; [0.08 + glow * 0.8, 0.1 + glow * 0.9, 0.12 + glow * 0.9] };
        let (dx, dy) = ((fx - cx) / r, (fy - cy) / r);
        let d2 = dx * dx + dy * dy;
        if d2 < 1.0 {
            let dz = (1.0 - d2).sqrt();
            let lam = (dx * light[0] + dy * light[1] + dz * light[2]).max(0.0);
            let spec = lam.powf(24.0) * 0.5;
            let k = 0.07 + lam * 0.85;
            c = [(0.62 * k + spec).min(1.0), (0.76 * k + spec).min(1.0), (0.78 * k + spec).min(1.0)];
        }
        if (fy - h as f32 * 0.2).abs() < 1.5 && fx > w as f32 * 0.07 && fx < w as f32 * 0.28 { c = [1.0, 0.42, 0.17]; }
        let o = ((y * w + x) * 4) as usize;
        px[o] = (c[0] * 255.0) as u8; px[o + 1] = (c[1] * 255.0) as u8; px[o + 2] = (c[2] * 255.0) as u8; px[o + 3] = 255;
    } }
    px
}

pub type DemoUi = crate::ui::Ui<Demo, Msg>;
pub fn new_ui(font: Vec<u8>, w: u32, h: u32, scale: f32) -> DemoUi { crate::ui::Ui::new(Demo::default(), view, update, font, w, h, scale) }
