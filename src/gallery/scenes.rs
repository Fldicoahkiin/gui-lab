//! The seven showcase scenes (DESIGN.md §6): full-window pages built only from the framework's own
//! parts (components, field events, materials, marks, morph, flow, annotation). Each starts calm, with
//! one obvious way to begin, and has a strip of its numbers at the bottom. `#scenes` lists them.
use super::pages::ratio;
use super::{Gallery, Msg, Page, Target};
use crate::demo::{BUST, CUTOUT, SKY, SPHERE, TERRAIN, cutout_alpha};
use crate::el::{Drag, El, Font, Key, Paint, Phase, col, picture, row, space, stack, text, text_lh};
use crate::morph::{Cmd, FlowSpec, MorphSpec};
use crate::style::{Gaze, GazeMode, Look, Material, PicFx, colour, gaze, set, styles};
use crate::widgets::{Edit, Spec, TextEdit, Variant, button_as, input, number, segmented, switch, tabs};
use crate::wrap::{Around, Silhouette};
use std::sync::OnceLock;
use taffy::{AlignItems, JustifyContent};

/// Hash name, title, what it is, and the style preset (index into `styles()`) it opens in.
pub const SCENES: [(&str, &str, &str, usize); 7] = [
    ("opening", "开场", "满屏的点聚成标题，再散开拼出一页；然后由一台机器一步一步操作这一页。", 1),
    ("reading", "读一篇文章", "图文混排的长文。拖动图片，文字沿着它的真实轮廓重新排开。", 2),
    ("printing", "一张图的印法", "同一张图分成几块，每块一种记号。移动指针，记号在指针下显现；拖动分界线。", 8),
    ("morph", "变形", "一个词散成粒子，拼成一张图；图变成记号，再合成一个按钮。", 10),
    ("xray", "透视", "按住空格，界面被拆开：边框、编号和中间图浮出来。松开，合回去。", 6),
    ("sound", "声音", "一段按节拍生成的信号进入场：低频推动整页起伏，频谱散成点，文字仍然可读。", 3),
    ("styles", "十二种风格", "同一个界面，在十二个风格预设之间切换，变化从按下的地方传开。", 7),
];
pub const COUNT: usize = 7;

/// A number of a scene, for its strip.
pub struct P { pub name: &'static str, pub spec: Spec, pub default: f32 }
const fn p(name: &'static str, min: f32, max: f32, step: f32, decimals: usize, unit: &'static str, default: f32) -> P {
    P { name, spec: Spec { min, max, step, decimals, unit }, default }
}
pub const PARAMS: [&[P]; COUNT] = [
    &[p("节拍", 200.0, 2000.0, 50.0, 0, "ms", 750.0), p("飞行", 300.0, 3000.0, 50.0, 0, "ms", 1100.0), p("弧度", 0.0, 200.0, 2.0, 0, "px", 60.0), p("点距", 2.0, 12.0, 0.5, 1, "px", 4.0)],
    &[p("间隔", 0.0, 60.0, 1.0, 0, "px", 14.0), p("图宽", 120.0, 520.0, 10.0, 0, "px", 300.0), p("字号", 12.0, 24.0, 1.0, 0, "px", 17.0), p("行高", 18.0, 44.0, 1.0, 0, "px", 30.0)],
    &[p("点距", 2.0, 32.0, 0.5, 1, "px", 8.0), p("级数", 2.0, 16.0, 1.0, 0, "", 8.0), p("透镜", 20.0, 300.0, 5.0, 0, "px", 130.0), p("松开", 0.0, 1.0, 0.05, 2, "", 0.0)],
    &[p("飞行", 300.0, 3000.0, 50.0, 0, "ms", 1300.0), p("弧度", 0.0, 200.0, 2.0, 0, "px", 80.0), p("错开", 0.0, 0.9, 0.05, 2, "", 0.5), p("点距", 2.0, 12.0, 0.5, 1, "px", 4.0)],
    &[p("层距", 40.0, 260.0, 5.0, 0, "px", 150.0), p("标注", 2.0, 40.0, 1.0, 0, "个", 14.0), p("等距线", 1.5, 12.0, 0.5, 1, "px", 3.0)],
    &[p("速度", 60.0, 180.0, 1.0, 0, "BPM", 112.0), p("强度", 0.0, 1.0, 0.05, 2, "", 0.7), p("低频", 0.0, 2.0, 0.05, 2, "", 1.0), p("种子", 0.0, 99.0, 1.0, 0, "", 7.0)],
    &[p("波速", 200.0, 3000.0, 20.0, 0, "px/s", 760.0), p("波宽", 4.0, 120.0, 1.0, 0, "px", 26.0), p("尾长", 0.0, 600.0, 10.0, 0, "px", 150.0)],
];

/// What a scene does.
#[derive(Clone, Debug, PartialEq)]
pub enum SMsg {
    /// Show or hide the parameter strip; put scene `i`'s numbers back.
    Strip,
    Reset(usize),
    // Opening
    Start,
    Restart,
    Pause,
    OpType(TextEdit),
    OpSwitch,
    OpTab(usize),
    OpPress,
    // Reading: move obstacle `k` to (x, y); which obstacles.
    Place(usize, f32, f32),
    Obstacle(usize),
    // Printing: move seam `i` to a fraction of the picture; which picture; marks everywhere or not.
    Seam(usize, f32),
    Picture(usize),
    // Morph: become form `i`, the next one.
    Form(usize),
    NextForm,
    // X-ray: take apart (held) or put back; keep apart.
    Apart(bool),
    KeepApart,
    // Sound
    Play,
    // Styles
    Look(usize),
}

pub struct Scenes {
    pub p: [[f32; 6]; COUNT],
    pub strip: bool,
    // Opening: what the stage shows (0 dots, 1 title, 2 the page), when the last step happened, the
    // operator's step, and the page's own state.
    pub shown: u8,
    pub at: f32,
    pub op: usize,
    pub running: bool,
    pub op_text: TextEdit,
    pub op_switch: bool,
    pub op_tab: usize,
    pub op_count: u32,
    pub op_value: f32,
    // Reading
    pub obj: [(f32, f32); 2],
    pub obstacle: usize,
    // Printing
    pub seams: [f32; 4],
    pub pic: usize,
    // Morph
    pub form: usize,
    // X-ray
    pub apart: bool,
    pub keep: bool,
    // Sound: playing, the song's time when it was last started or paused, the clock then, the last beat.
    pub playing: bool,
    pub song: f32,
    pub since: f32,
    pub beat: i64,
    pub levels: [f32; BANDS],
    // Styles
    pub look: usize,
}

pub const BANDS: usize = 32;

impl Default for Scenes {
    fn default() -> Self {
        let mut p = [[0.0; 6]; COUNT];
        for (i, ps) in PARAMS.iter().enumerate() { for (j, q) in ps.iter().enumerate() { p[i][j] = q.default; } }
        Self {
            p, strip: true, shown: 0, at: 0.0, op: 0, running: false, op_text: TextEdit::default(), op_switch: false, op_tab: 0, op_count: 0, op_value: 12.0,
            obj: [(330.0, 140.0), (60.0, 620.0)], obstacle: 0, seams: [0.2, 0.4, 0.6, 0.8], pic: 0, form: 0, apart: false, keep: false,
            playing: false, song: 0.0, since: 0.0, beat: -1, levels: [0.0; BANDS], look: 7,
        }
    }
}

impl Scenes {
    pub fn get(&self, scene: usize, i: usize) -> f32 { self.p[scene][i] }
    fn reset(&mut self, scene: usize) {
        let d = Scenes::default();
        self.p[scene] = d.p[scene];
        match scene {
            0 => { self.shown = 0; self.op = 0; self.running = false; self.op_text = d.op_text; self.op_switch = false; self.op_tab = 0; self.op_count = 0; self.op_value = d.op_value; }
            1 => { self.obj = d.obj; self.obstacle = 0; }
            2 => { self.seams = d.seams; self.pic = 0; }
            3 => self.form = 0,
            4 => { self.apart = false; self.keep = false; }
            5 => { self.playing = false; self.song = 0.0; self.beat = -1; self.levels = [0.0; BANDS]; }
            _ => {}
        }
    }
}

pub fn hash(i: usize) -> String { format!("scene-{}", SCENES[i].0) }
fn title_flow(i: usize) -> String { format!("scene-title-{i}") }

// ── Update and the clock ─────────────────────────────────────────────────────────────────────────

pub fn update(g: &mut Gallery, m: SMsg) {
    let now = g.clock;
    match m {
        SMsg::Strip => g.sc.strip = !g.sc.strip,
        SMsg::Reset(i) => {
            g.sc.reset(i);
            if i == 6 { g.demo.fx = crate::scene::Fx { pitch: g.demo.fx.pitch, ..Default::default() }; }
        }
        SMsg::Start => if g.sc.shown == 0 { begin(g, now) },
        SMsg::Restart => { let p = g.sc.p[0]; g.sc.reset(0); g.sc.p[0] = p; }
        SMsg::Pause => { g.sc.running = !g.sc.running; g.sc.at = now; }
        SMsg::OpType(t) => g.sc.op_text = t,
        SMsg::OpSwitch => g.sc.op_switch = !g.sc.op_switch,
        SMsg::OpTab(i) => g.sc.op_tab = i,
        SMsg::OpPress => g.sc.op_count += 1,
        SMsg::Place(k, x, y) => g.sc.obj[k.min(1)] = (x, y),
        SMsg::Obstacle(i) => g.sc.obstacle = i.min(2),
        SMsg::Seam(i, f) => {
            let i = i.min(3);
            let lo = if i == 0 { 0.0 } else { g.sc.seams[i - 1] + 0.02 };
            let hi = if i == 3 { 1.0 } else { g.sc.seams[i + 1] - 0.02 };
            g.sc.seams[i] = f.clamp(lo, hi);
        }
        SMsg::Picture(i) => g.sc.pic = i.min(2),
        SMsg::Form(i) => form_to(g, i),
        SMsg::NextForm => { let n = (g.sc.form + 1) % FORMS.len(); form_to(g, n); }
        SMsg::Apart(a) => apart(g, a || g.sc.keep),
        SMsg::KeepApart => { g.sc.keep = !g.sc.keep; let k = g.sc.keep; apart(g, k); }
        SMsg::Play => {
            if g.sc.playing { g.sc.song += now - g.sc.since; } else { g.sc.since = now; }
            g.sc.playing = !g.sc.playing;
        }
        SMsg::Look(i) => { g.sc.look = i % 12; g.apply_style(i % 12); }
    }
}

/// A number of scene `i` set to `v`.
pub fn set(g: &mut Gallery, scene: usize, i: usize, v: f32) {
    let v = PARAMS[scene.min(COUNT - 1)].get(i).map_or(v, |q| q.spec.clamp(v));
    g.sc.p[scene.min(COUNT - 1)][i.min(5)] = v;
    if scene == 6 {
        match i { 0 => g.demo.fx.wave_speed = v, 1 => g.demo.fx.wave_width = v, _ => g.demo.fx.wave_tail = v }
    }
}

/// Keys no control used, on a scene page. Returns whether the scene used it.
pub fn key(g: &mut Gallery, k: Key, _shift: bool) -> bool {
    let Page::Scene(i) = g.page else { return false };
    if k == Key::Escape { super::update(g, Msg::Go(Page::Scenes)); return true; }
    match (i, k) {
        (0, Key::Enter | Key::Char(' ')) => { update(g, if g.sc.shown == 0 { SMsg::Start } else { SMsg::Pause }); true }
        (3, Key::Right | Key::Char(' ') | Key::Enter) => { update(g, SMsg::NextForm); true }
        (3, Key::Left) => { let n = (g.sc.form + FORMS.len() - 1) % FORMS.len(); update(g, SMsg::Form(n)); true }
        (4, Key::Char(' ')) => { if !g.sc.apart { update(g, SMsg::Apart(true)); } true }
        (5, Key::Char(' ')) => { update(g, SMsg::Play); true }
        (6, Key::Right) => { let n = (g.sc.look + 1) % 12; update(g, SMsg::Look(n)); true }
        (6, Key::Left) => { let n = (g.sc.look + 11) % 12; update(g, SMsg::Look(n)); true }
        _ => false,
    }
}
/// A key let go on a scene page.
pub fn key_up(g: &mut Gallery, k: Key) -> bool {
    if g.page == Page::Scene(4) && k == Key::Char(' ') && g.sc.apart && !g.sc.keep { update(g, SMsg::Apart(false)); return true; }
    false
}

/// The clock: things on a scene that change by themselves (the operator's steps, the signal).
pub fn tick(g: &mut Gallery, t: f32) -> bool {
    match g.page {
        Page::Scene(0) => tick_opening(g, t),
        Page::Scene(5) => tick_sound(g, t),
        _ => false,
    }
}

/// What a scene changes in the look: its picture marks, the field's push, the annotation layer.
pub fn look(g: &Gallery, l: &mut Look) {
    let Page::Scene(i) = g.page else { return };
    let q = |j| g.sc.get(i as usize, j);
    match i {
        0 => {
            l.fx.pitch = q(3);
            // The operator's target is framed as it is reached.
            l.gaze = Gaze { mode: GazeMode::All, kinds: gaze::FRAME | gaze::KIND | gaze::SIZE, density: 2.0, draw_ms: 360.0, colour: l.theme.accent, only_marked: true };
        }
        2 => {
            let base = PicFx { pitch: q(0), levels: q(1), ground: 0.1, ..PicFx::default() };
            l.pics[0] = PicFx { gamma: 1.1, ground: 0.0, ..PicFx { set: set::CHARS as f32, ..base } };
            l.pics[1] = PicFx { palette: 3.0, colours: 4.0, spread: 0.6, pitch: (q(0) * 0.6).max(2.0), ..PicFx { set: set::PIXEL as f32, ..base } };
            l.pics[2] = PicFx { follow: 1.0, colour: colour::TWO as f32, tint_a: [0.05, 0.05, 0.07], tint_b: [0.95, 0.92, 0.84], ..PicFx { set: set::STROKE as f32, ..base } };
            l.pics[3] = PicFx { glow: 1.2, gamma: 0.8, ground: 0.0, colour: colour::GIVEN as f32, tint_a: l.theme.accent, ..PicFx { set: set::GLOW as f32, ..base } };
            l.pics[4] = PicFx { relief: 1.4, ground: 0.25, pitch: q(0) * 1.3, ..PicFx { set: set::BRICK as f32, ..base } };
            l.fx.lens = q(2);
            l.gaze.mode = GazeMode::Off;
        }
        3 => l.fx.pitch = q(3),
        4 => {
            l.params[Material::Near as usize].0[0] = q(2);
            l.gaze = if g.sc.apart {
                Gaze { mode: GazeMode::All, kinds: gaze::BOX | gaze::INDEX | gaze::SIZE | gaze::KIND, density: q(1), draw_ms: 420.0, colour: l.theme.accent, only_marked: false }
            } else { Gaze { mode: GazeMode::Off, ..l.gaze } };
        }
        5 => {
            // The low end pushes the page: fronts from the beat move every instance a little.
            l.fx.push = 3.0 + 16.0 * q(1) * q(2);
            l.fx.wave_life = 1.1;
            l.fx.wave_width = 40.0;
            l.fx.wave_speed = 900.0;
            l.gaze.mode = GazeMode::Off;
        }
        _ => {}
    }
}

// ── Opening ──────────────────────────────────────────────────────────────────────────────────────

fn morph_spec(g: &Gallery, scene: usize) -> MorphSpec {
    let q = |j| g.sc.get(scene, j);
    match scene {
        0 => MorphSpec { duration_ms: q(1), curl: q(2), lattice: q(3), stagger: 0.45, threshold: 0.35, ..MorphSpec::default() },
        _ => MorphSpec { duration_ms: q(0), curl: q(1), stagger: q(2), lattice: q(3), threshold: 0.35, ..MorphSpec::default() },
    }
}
fn begin(g: &mut Gallery, now: f32) {
    let spec = morph_spec(g, 0);
    g.cmds.push(Cmd::Morph { from: "op/dots".into(), to: "op/title".into(), spec });
    g.sc.shown = 1;
    g.sc.at = now;
    g.sc.running = true;
    g.sc.op = 0;
}
/// The operator's script: where it goes, what it says there, and what it does on arrival.
const OPS: [(&str, &str); 9] = [
    ("op/search", "输入框"), ("op/search", "输入 点"), ("op/search", "输入 与"), ("op/search", "输入 线"),
    ("op/switch", "开关"), ("op/num", "数字 +4"), ("op/tabs", "标签页 2"), ("op/publish", "按钮"), ("op/cards", "卡片"),
];
fn tick_opening(g: &mut Gallery, t: f32) -> bool {
    let sc = &mut g.sc;
    if !sc.running { return false; }
    let beat = sc.p[0][0] / 1000.0;
    let flight = sc.p[0][1] / 1000.0;
    match sc.shown {
        // The title holds a moment, then scatters into the page.
        1 if t >= sc.at + flight + 0.9 => {
            let spec = morph_spec(g, 0);
            g.cmds.push(Cmd::Morph { from: "op/title".into(), to: "op/page".into(), spec });
            g.sc.shown = 2;
            g.sc.at = t;
            true
        }
        2 if t >= sc.at + if sc.op == 0 { flight + 0.8 } else { beat } => {
            if sc.op >= OPS.len() { sc.running = false; return true; }
            match sc.op {
                1 => sc.op_text = TextEdit::new("点"),
                2 => sc.op_text = TextEdit::new("点与"),
                3 => sc.op_text = TextEdit::new("点与线"),
                4 => sc.op_switch = !sc.op_switch,
                5 => sc.op_value += 4.0,
                6 => sc.op_tab = 2,
                7 => sc.op_count += 1,
                _ => {}
            }
            sc.op += 1;
            sc.at = t;
            // The operator's mark flies from the last control to this one.
            g.cmds.push(Cmd::Flow(FlowSpec { flow_ms: 380.0, flow_loose: 0.35, out_ms: 160.0 }));
            true
        }
        _ => false,
    }
}

fn opening(g: &Gallery, w: f32, h: f32) -> El<Msg> {
    let sc = &g.sc;
    // The dots: one paragraph of discs, a grid across the stage.
    let (cols, rows) = (((w - 20.0) / 22.0).floor().max(4.0) as usize, ((h - 40.0) / 22.0).floor().max(4.0) as usize);
    let line: String = (0..cols).map(|_| "●").collect::<Vec<_>>().join(" ");
    let grid = (0..rows).map(|_| line.clone()).collect::<Vec<_>>().join("\n");
    let dots = text_lh(grid, 9.0, 22.0, Paint::FAINT).key("op/dots").hidden_until_morphed(sc.shown != 0);
    let title = col().key("op/title").gap(14.0).align(AlignItems::CENTER).hidden_until_morphed(sc.shown != 1)
        .child(text_lh("同一种机制", (w / 6.0).clamp(44.0, 128.0), (w / 6.0).clamp(44.0, 128.0) * 1.15, Paint::ACCENT).font(Font::Display).weight(2.5))
        .child(text_lh("字、图和控件都按自己的轮廓松开、飞行、合拢", 16.0, 26.0, Paint::DIM));
    let page = op_page(g, w).hidden_until_morphed(sc.shown != 2);
    let mut stage = col().key("op/stage").width(w).height(h).center()
        .child(stack([dots, title, page]).key("op/stack"));
    if sc.shown == 0 {
        stage = stage.child(button_as("op/start", "开始", Variant::Accent, Msg::Scene(SMsg::Start)).height(44.0).pad(28.0, 0.0).absolute(Some(w * 0.5 - 50.0), Some(h * 0.5 - 22.0), None, None));
    }
    stage
}

/// The operator's mark: rides along inside the control it is at, so it is wherever layout puts it.
fn operator<M>(g: &Gallery, key: &str) -> Option<El<M>> {
    let sc = &g.sc;
    if sc.shown != 2 || sc.op == 0 { return None; }
    let at = OPS[(sc.op - 1).min(OPS.len() - 1)].0;
    (at == key).then(|| col().key(format!("op/mark{}", sc.op)).flow("operator").width(22.0).height(22.0).center().absolute(None, Some(-6.0), Some(-6.0), None)
        .child(space(22.0, 22.0).radius(11.0).border(1.5, Paint::ACCENT))
        .child(space(8.0, 8.0).radius(4.0).fill(Paint::ACCENT).absolute(Some(7.0), Some(7.0), None, None)))
}
/// A control the operator may be at: annotated while it is there.
fn op_at(g: &Gallery, key: &str, el: El<Msg>) -> El<Msg> {
    let sc = &g.sc;
    let mut c = col().key(format!("{key}/at")).child(el);
    if let Some(m) = operator(g, key) {
        c = c.child(m);
        if sc.op > 0 { c = c.annotate(OPS[(sc.op - 1).min(OPS.len() - 1)].1); }
    }
    c
}

fn op_page(g: &Gallery, w: f32) -> El<Msg> {
    let sc = &g.sc;
    let narrow = w < 640.0;
    let cw = if narrow { w - 8.0 } else { (w * 0.82).min(900.0) };
    let card = |i: usize, id: u32, t: &str| col().key(format!("op/card{i}")).width(if narrow { cw } else { (cw - 32.0) / 3.0 }).gap(8.0)
        .child(picture(id, ratio(id)).key(format!("op/card{i}/pic")))
        .child(text(t, 14.0, Paint::TEXT));
    let mut cards = row().key("op/cards").gap(16.0).fill_width();
    if narrow { cards = cards.wrap(); }
    for (i, (id, t)) in [(SKY, "傍晚"), (TERRAIN, "地形"), (BUST, "石膏像")].into_iter().enumerate() {
        if narrow && i > 0 { break; }
        cards = cards.child(card(i, id, t));
    }
    let panels = ["点是最小的单位。", "线是点的轨迹。", "面是线的轨迹。"];
    col().key("op/page").width(cw).gap(18.0)
        .child(row().fill_width().justify(JustifyContent::SPACE_BETWEEN).align(AlignItems::CENTER).wrap().gap(12.0)
            .child(text_lh("点、线、面", 34.0, 42.0, Paint::INK).font(Font::Display).weight(1.0))
            .child(op_at(g, "op/search", input("op/search", &sc.op_text, "搜索", 220.0, 20, |t| Msg::Scene(SMsg::OpType(t))))))
        .child(op_at(g, "op/cards", cards))
        .child(row().key("op/controls").gap(22.0).wrap().align(AlignItems::CENTER)
            .child(op_at(g, "op/switch", switch("op/switch", "显示网格", sc.op_switch, None, Msg::Scene(SMsg::OpSwitch)).width(170.0)))
            .child(op_at(g, "op/num", number("op/num", "间距", sc.op_value, Spec { min: 0.0, max: 64.0, step: 1.0, decimals: 0, unit: "px" }, g.draft(Target::OpValue), |e| Msg::Num(Target::OpValue, e))))
            .child(op_at(g, "op/publish", button_as("op/publish", &format!("发布 · {}", sc.op_count), Variant::Accent, Msg::Scene(SMsg::OpPress)))))
        .child(op_at(g, "op/tabs", tabs("op/tabs", &["点", "线", "面"], sc.op_tab, 18.0, 2.0, |i| Msg::Scene(SMsg::OpTab(i)), text_lh(panels[sc.op_tab.min(2)], 15.0, 24.0, Paint::TEXT).key(format!("op/panel{}", sc.op_tab)))))
}

// ── Reading ──────────────────────────────────────────────────────────────────────────────────────

const ARTICLE: &str = "一页文字通常被看成一个方框：框里是字，框外是别的东西。可是纸上从来不是这样排的。报纸的版面里，照片的边缘是不规则的，文字沿着它走；手抄本里，大写字母占了好几行，后面的字绕开它。字和图之间留着一条均匀的缝，缝的形状就是图的形状。

在这一页上，图片可以拖动。每次它移动，正文就重新断行：每一行先量出这一条带子里哪一段没有被挡住，再把能放下的字放进去。如果图片把一行分成左右两段，两段各排一行，读的时候从左到右。缝的宽度是一个数字，在下面可以改。

这件事依赖的是图片真实的轮廓，而不是它的方框。图片带着透明的底，所以框的四角是空的，字可以一直排到石膏像的肩膀旁边。圆形也一样：离圆心越远的行，留给文字的地方越宽。

排版本身没有新的规则。断行仍然交给排版系统，它照常处理中文和 Latin words 的混排、标点和行宽；这里只是每一行给它一个不同的起点和宽度。所以字号、行高、字体都可以换，换了以后仍然绕得开。

拖动的时候，你看到的是同一套布局在每一步重新计算的结果：没有动画曲线，也没有特殊的过渡。松手以后，页面就是一张普通的、安静的页面。";

fn silhouette_bust(x: f32, y: f32, w: f32) -> Silhouette {
    static ALPHA: OnceLock<(u32, u32, Vec<u8>)> = OnceLock::new();
    let (iw, ih, a) = ALPHA.get_or_init(cutout_alpha);
    Silhouette::from_alpha(*iw, *ih, |tx, ty| a[(ty * iw + tx) as usize], x, y, w)
}

fn reading(g: &Gallery, w: f32, _h: f32) -> El<Msg> {
    let sc = &g.sc;
    let q = |j| sc.get(1, j);
    let cw = w.min(760.0);
    let pw = q(1).min(cw * 0.7);
    let ph = pw * ratio(CUTOUT);
    let disc = (cw * 0.22).clamp(60.0, 130.0);
    let (bx, by) = sc.obj[0];
    let (dx, dy) = sc.obj[1];
    let (bx, dx) = (bx.min(cw - 40.0), dx.min(cw - 40.0));
    let mut obstacles = vec![];
    let mut body = col().key("read/body").width(cw);
    let drag = |k: usize, x0: f32, y0: f32| move |d: Drag| Some(Msg::Scene(SMsg::Place(k, (x0 + d.dx).round(), (y0 + d.dy).max(-40.0).round())));
    let nudge = |k: usize, x0: f32, y0: f32| move |key: Key, shift: bool| {
        let s = if shift { 40.0 } else { 10.0 };
        let (x, y) = match key { Key::Left => (x0 - s, y0), Key::Right => (x0 + s, y0), Key::Up => (x0, y0 - s), Key::Down => (x0, y0 + s), _ => return None };
        Some(Msg::Scene(SMsg::Place(k, x, y)))
    };
    if sc.obstacle != 1 {
        obstacles.push(silhouette_bust(bx, by, pw));
        body = body.child(col().key("read/bust").width(pw).height(ph).absolute(Some(bx), Some(by), None, None).on_drag_with(drag(0, bx, by)).on_key(nudge(0, bx, by))
            .child(picture(CUTOUT, ratio(CUTOUT)).key("read/bust-pic")));
    }
    if sc.obstacle != 0 {
        obstacles.push(Silhouette::circle(dx + disc * 0.5, dy + disc * 0.5, disc * 0.5));
        body = body.child(space(disc, disc).radius(disc * 0.5).fill(Paint::ACCENT).key("read/disc").absolute(Some(dx), Some(dy), None, None).on_drag_with(drag(1, dx, dy)).on_key(nudge(1, dx, dy)));
    }
    let mut around = Around::new(obstacles, q(0));
    around.min = (q(2) * 3.0).max(40.0);
    body = body.child(text_lh(ARTICLE, q(2), q(3), Paint::TEXT).key("read/text").fill_width().around(around));
    col().key("read/page").width(cw).gap(14.0)
        .child(text("长文 · 第 3 期", 12.0, Paint::DIM).font(Font::Mono))
        .child(row().fill_width().justify(JustifyContent::SPACE_BETWEEN).align(AlignItems::END).wrap().gap(10.0)
            .child(text("文 / 编辑部　约 900 字", 13.0, Paint::DIM))
            .child(segmented("read/which", &["胸像", "圆", "两个"], sc.obstacle, 28.0, |i| Msg::Scene(SMsg::Obstacle(i)))))
        .child(space(1.0, 1.0).fill_width().fill(Paint::FAINT))
        .child(body)
}

// ── Printing ─────────────────────────────────────────────────────────────────────────────────────

const STRIP_NAMES: [&str; 5] = ["字符", "像素画", "笔触", "光点", "积木"];
fn printing(g: &Gallery, w: f32, h: f32) -> El<Msg> {
    let sc = &g.sc;
    let id = [BUST, SKY, TERRAIN][sc.pic.min(2)];
    let r = ratio(id);
    let pw = w.min((h - 70.0) / r).max(200.0);
    let ph = (pw * r).round();
    let loose = sc.get(2, 3);
    let edges = [0.0, sc.seams[0], sc.seams[1], sc.seams[2], sc.seams[3], 1.0];
    let mut wall = col().key("pr/wall").width(pw).height(ph).gaze_scope();
    for k in 0..5 {
        let (x0, x1) = ((edges[k] * pw).round(), (edges[k + 1] * pw).round());
        // A window onto the same picture, at the same place: each window shows it in its own marks.
        wall = wall.child(col().key(format!("pr/region{k}")).width((x1 - x0).max(0.0)).height(ph).clip().absolute(Some(x0), Some(0.0), None, None)
            .child(picture(id, r).key(format!("pr/pic{k}")).pic_fx(k as u8).loose(loose).width(pw).absolute(Some(-x0), Some(0.0), None, None))
            .child(text(format!("{:02} {}", k + 1, STRIP_NAMES[k]), 12.0, Paint::INK).font(Font::Mono).absolute(Some(10.0), Some(10.0), None, None)));
    }
    for i in 0..4 {
        let x = (sc.seams[i] * pw).round();
        let f0 = sc.seams[i];
        let seam = col().key(format!("pr/seam{i}")).width(24.0).height(ph).absolute(Some(x - 12.0), Some(0.0), None, None).align(AlignItems::CENTER)
            .on_drag_with(move |d| Some(Msg::Scene(SMsg::Seam(i, f0 + d.dx / pw))))
            .on_key(move |k, shift| { let s = if shift { 0.1 } else { 0.02 }; match k { Key::Left => Some(Msg::Scene(SMsg::Seam(i, f0 - s))), Key::Right => Some(Msg::Scene(SMsg::Seam(i, f0 + s))), _ => None } })
            .child(space(1.5, ph).fill(Paint::ACCENT))
            .child(space(14.0, 14.0).radius(7.0).fill(Paint::ACCENT).absolute(Some(5.0), Some(ph - 24.0), None, None));
        wall = wall.child(seam);
    }
    col().key("pr/page").gap(12.0).align(AlignItems::CENTER)
        .child(wall)
        .child(row().gap(12.0).align(AlignItems::CENTER).wrap()
            .child(segmented("pr/which", &["胸像", "天空", "地形"], sc.pic, 28.0, |i| Msg::Scene(SMsg::Picture(i))))
            .child(text("指针下的一圈显出这一块的记号；松开设为 1 时整张都是记号。", 12.0, Paint::DIM)))
}

// ── Morph ────────────────────────────────────────────────────────────────────────────────────────

pub const FORMS: [&str; 4] = ["字", "图", "记号", "按钮"];
fn form_key(i: usize) -> String { format!("mo/form{i}") }
fn form_to(g: &mut Gallery, i: usize) {
    if i == g.sc.form || i >= FORMS.len() { return; }
    let spec = morph_spec(g, 3);
    g.cmds.push(Cmd::Morph { from: form_key(g.sc.form), to: form_key(i), spec });
    g.sc.form = i;
}
fn morph(g: &Gallery, w: f32, h: f32) -> El<Msg> {
    let f = g.sc.form;
    let pw = (w * 0.8).min(560.0).min((h - 120.0) / ratio(SKY));
    let size = (w / 4.2).clamp(72.0, 200.0);
    let forms = [
        text_lh("山与水", size, size * 1.15, Paint::INK).font(Font::Display).weight(3.0),
        col().width(pw).child(picture(SKY, ratio(SKY)).key("mo/pic").pic_fx(super::STYLE_SLOT)),
        col().width(pw).child(picture(SKY, ratio(SKY)).key("mo/marks").pic_fx(super::STYLE_SLOT).loose(1.0)),
        row().key("mo/go-button").height(96.0).pad(56.0, 0.0).radius(48.0).fill(Paint::ACCENT).center().on_press(Msg::Scene(SMsg::Form(0)))
            .child(text("出发", 40.0, Paint::GROUND).weight(1.5)),
    ];
    let stage = stack(forms.into_iter().enumerate().map(|(i, e)| col().key(form_key(i)).hidden_until_morphed(i != f).child(e))).key("mo/stack");
    col().key("mo/page").width(w).height(h).align(AlignItems::CENTER).gap(16.0)
        .child(col().key("mo/stage").width(w).grow(1.0).center().on_press(Msg::Scene(SMsg::NextForm)).focusable(false).child(stage))
        .child(row().gap(12.0).align(AlignItems::CENTER).wrap().justify(JustifyContent::CENTER)
            .child(segmented("mo/forms", &FORMS, f, 30.0, |i| Msg::Scene(SMsg::Form(i))))
            .child(button_as("mo/next", "下一步 →", Variant::Default, Msg::Scene(SMsg::NextForm))))
}

// ── X-ray ────────────────────────────────────────────────────────────────────────────────────────

fn apart(g: &mut Gallery, a: bool) {
    if a == g.sc.apart { return; }
    g.sc.apart = a;
    g.cmds.push(Cmd::Flow(FlowSpec { flow_ms: 520.0, flow_loose: 0.3, out_ms: 240.0 }));
}
/// The small application that is taken apart; `layer` names the copy (its keys and flow id).
fn xr_app(g: &Gallery, layer: &str, w: f32) -> El<Msg> {
    let k = |s: &str| format!("xr/{layer}/{s}");
    col().key(k("app")).width(w).gap(12.0).pad(20.0, 18.0).radius(16.0).fill(Paint::tone(0.04)).border(1.0, Paint::DIM)
        .child(row().fill_width().justify(JustifyContent::SPACE_BETWEEN).align(AlignItems::CENTER)
            .child(text("今晚", 22.0, Paint::INK).font(Font::Display).key(k("title")))
            .child(text("21:40", 13.0, Paint::DIM).font(Font::Mono)))
        .child(col().key(k("pic")).fill_width().child(picture(SPHERE, ratio(SPHERE)).key(k("picture"))))
        .child(col().gap(0.0).fill_width()
            .children(["月亮升起　20:56", "气温　14 °C", "风　东北 2 级"].iter().enumerate().map(|(i, t)| {
                row().key(k(&format!("row{i}"))).fill_width().height(34.0).align(AlignItems::CENTER).child(text(*t, 14.0, Paint::TEXT))
            })))
        .child(row().fill_width().gap(10.0).align(AlignItems::CENTER)
            .child(space(0.0, 0.0).grow(1.0))
            .child(row().key(k("remind")).height(32.0).pad(16.0, 0.0).radius(16.0).fill(Paint::ACCENT).center().child(text("提醒我", 14.0, Paint::GROUND))))
        .child(text_lh(if g.sc.apart { "中间图：离墨的远近一圈圈画出来" } else { "按住空格，或按住下面的按钮" }, 12.0, 18.0, Paint::DIM).key(k("hint")))
}
fn xray(g: &Gallery, w: f32, h: f32) -> El<Msg> {
    let sc = &g.sc;
    let narrow = w < 640.0;
    let aw = if narrow { (w - 24.0).min(340.0) } else { 340.0 };
    let d = sc.get(4, 0);
    let (cx, cy) = ((w - aw) * 0.5, 10.0);
    let mut stage = col().key("xr/stage").width(w).height(h - 60.0).gaze_scope();
    // Apart: the top layer lifts off to the right, the outlines and the nearness stay behind it.
    let offs = if narrow { [(0.0, -d * 0.3), (0.0, 0.0), (0.0, d * 0.3)] } else { [(-d, d * 0.12), (0.0, 0.0), (d, -d * 0.12)] };
    if sc.apart {
        stage = stage
            .child(xr_app(g, "near", aw).material(Material::Near.id()).absolute(Some(cx + offs[0].0), Some(cy + offs[0].1 + d * 0.2), None, None))
            .child(xr_app(g, "outline", aw).material(Material::Xray.id()).absolute(Some(cx + offs[1].0), Some(cy + offs[1].1 + d * 0.1), None, None));
    }
    let (ox, oy) = if sc.apart { offs[2] } else { (0.0, 0.0) };
    stage = stage.child(col().key(format!("xr/top-{}", sc.apart)).flow("xr-top").absolute(Some(cx + ox), Some(cy + oy), None, None).child(xr_app(g, if sc.apart { "top-a" } else { "top" }, aw)));
    let hold = row().key("xr/hold").height(36.0).pad(18.0, 0.0).radius(18.0).border(1.0, Paint::ACCENT).center()
        .fill(if sc.apart { Paint::ACCENT.alpha(0.18) } else { Paint::GROUND })
        .on_drag_with(|d| match d.phase { Phase::Start => Some(Msg::Scene(SMsg::Apart(true))), Phase::End => Some(Msg::Scene(SMsg::Apart(false))), Phase::Move => None })
        .on_key(|k, _| matches!(k, Key::Enter).then_some(Msg::Scene(SMsg::KeepApart)))
        .child(text("按住拆开", 14.0, Paint::ACCENT));
    col().key("xr/page").width(w).gap(10.0)
        .child(stage)
        .child(row().gap(16.0).align(AlignItems::CENTER).justify(JustifyContent::CENTER).fill_width().wrap()
            .child(hold)
            .child(switch("xr/keep", "保持拆开", sc.keep, None, Msg::Scene(SMsg::KeepApart)).width(150.0)))
}

// ── Sound ────────────────────────────────────────────────────────────────────────────────────────

fn hash1(a: f32, b: f32) -> f32 { ((a * 127.1 + b * 311.7).sin() * 43_758.547).fract().abs() }
/// The song's time at the clock `t`.
fn song_time(sc: &Scenes, t: f32) -> f32 { if sc.playing { sc.song + (t - sc.since) } else { sc.song } }
/// The signal: per band, a level 0..1. A kick on every beat in the low bands, chords in the middle,
/// hats on the off-beats in the high ones, all scaled by the intensity, with the seed's variation.
pub fn spectrum(sc: &Scenes, t: f32) -> [f32; BANDS] {
    let (bpm, inten, bass, seed) = (sc.p[5][0], sc.p[5][1], sc.p[5][2], sc.p[5][3]);
    let st = song_time(sc, t);
    let b = st * bpm / 60.0;
    let ph = b.fract();
    let bar = (b / 4.0).floor();
    let kick = (-ph * 7.0).exp();
    let snare = if (b.floor() as i64) % 2 == 1 { (-ph * 9.0).exp() } else { 0.0 };
    let off = (b * 2.0).fract();
    let hat = (-off * 14.0).exp() * if off < 0.5 { 1.0 } else { 0.0 } * 0.8;
    let sixteenth = (b * 4.0).floor();
    let mut out = [0.0; BANDS];
    for (i, o) in out.iter_mut().enumerate() {
        let f = i as f32 / (BANDS - 1) as f32;
        let low = (1.0 - f * 5.0).max(0.0);
        let mid = (-((f - 0.4) / 0.22).powi(2)).exp();
        let high = ((f - 0.62) / 0.38).clamp(0.0, 1.0);
        let chord = 0.38 + 0.22 * (st * 0.8 + i as f32 * 0.55 + bar * 1.7 + seed).sin();
        let grit = hash1(i as f32 + seed * 3.1, sixteenth) * 0.25;
        let v = low * kick * bass * 1.05 + mid * (chord + snare * 0.35 + grit * 0.6) * 0.85 + high * (hat + grit * 0.5) * 0.9;
        *o = (v * inten * if sc.playing || st > 0.0 { 1.0 } else { 0.0 }).clamp(0.0, 1.0);
    }
    out
}
fn tick_sound(g: &mut Gallery, t: f32) -> bool {
    if !g.sc.playing { return false; }
    g.sc.levels = spectrum(&g.sc, t);
    let st = song_time(&g.sc, t);
    let beat = (st * g.sc.p[5][0] / 60.0).floor() as i64;
    if beat != g.sc.beat {
        g.sc.beat = beat;
        // The kick: a front from below the middle of the page, as strong as the low end.
        let s = (g.sc.p[5][1] * g.sc.p[5][2] * 0.9).clamp(0.15, 1.0);
        g.cmds.push(Cmd::Front { x: g.width * 0.5, y: g.height * 0.92, strength: s });
    }
    true
}
const LYRICS: [&str; 4] = ["夜里的路灯一盏一盏往后退", "低音从地板下面传上来", "整页轻轻地起伏，字还在原来的地方", "直到最后一个小节"];
fn sound(g: &Gallery, w: f32, h: f32) -> El<Msg> {
    let sc = &g.sc;
    let st = song_time(sc, g.clock);
    let bpm = sc.p[5][0];
    let bars = (st * bpm / 240.0).floor() as usize;
    let kick = sc.levels[0];
    let bw = ((w - 40.0) / BANDS as f32).floor();
    let bh = (h * 0.34).clamp(100.0, 260.0);
    let mut spec = row().key("snd/spectrum").height(bh).gap(0.0).align(AlignItems::END);
    for (i, v) in sc.levels.iter().enumerate() {
        let hgt = (v * bh).max(3.0).round();
        // A loud band loosens: under the dot material it breaks into dots, then holds again.
        spec = spec.child(space((bw - 4.0).max(2.0), hgt).fill(if i < 5 { Paint::ACCENT } else if i < 20 { Paint::hue(1) } else { Paint::INK }).loose((v - 0.55).max(0.0) * 1.4).key(format!("snd/bar{i}")));
        spec = spec.child(space(4.0, 1.0));
    }
    let mm = (st / 60.0).floor() as u32;
    let ss = (st % 60.0).floor() as u32;
    let size = (w / 9.0).clamp(40.0, 104.0);
    let pulse = 120.0 + kick * 60.0;
    col().key("snd/page").width(w).height(h).align(AlignItems::CENTER).justify(JustifyContent::SPACE_BETWEEN)
        .child(col().key("snd/head").align(AlignItems::CENTER).gap(10.0).fill_width()
            .child(row().gap(18.0).align(AlignItems::CENTER)
                .child(space(pulse * 0.25, pulse * 0.25).radius(pulse * 0.125).fill(Paint::ACCENT).key("snd/pulse"))
                .child(text(format!("{bpm:.0} BPM · {mm:02}:{ss:02}"), 14.0, Paint::DIM).font(Font::Mono).key("snd/time")))
            .child(text_lh("夜行", size, size * 1.2, Paint::INK).font(Font::Display).weight(2.0).key("snd/title"))
            .child(text_lh(LYRICS[bars % LYRICS.len()], 20.0, 30.0, Paint::TEXT).key(format!("snd/lyric{}", bars % LYRICS.len()))))
        .child(col().key("snd/lower").align(AlignItems::CENTER).gap(14.0)
            .child(spec)
            .child(row().gap(12.0).align(AlignItems::CENTER)
                .child(button_as("snd/play", if sc.playing { "暂停" } else { "播放" }, Variant::Accent, Msg::Scene(SMsg::Play)).width(96.0))
                .child(text("空格：播放 / 暂停", 12.0, Paint::DIM))))
}

// ── Styles ───────────────────────────────────────────────────────────────────────────────────────

fn styles_scene(g: &Gallery, w: f32, _h: f32) -> El<Msg> {
    let narrow = w < 700.0;
    let all = styles();
    let mut grid = row().key("st/grid").gap(8.0).wrap().width(if narrow { w } else { 300.0 });
    for (i, st) in all.iter().enumerate() {
        let on = g.style == Some(i);
        let mut b = row().key(format!("st/s{i}")).height(34.0).width(if narrow { (w - 16.0) / 3.0 } else { 146.0 }).pad(10.0, 0.0).radius(4.0).align(AlignItems::CENTER)
            .border(1.0, if on { Paint::ACCENT } else { Paint::FAINT }).on_press(Msg::Scene(SMsg::Look(i))).commit()
            .child(text(st.name, 12.0, if on { Paint::ACCENT } else { Paint::TEXT }));
        if on { b = b.fill(Paint::ACCENT.alpha(0.12)); }
        grid = grid.child(b);
    }
    let cw = if narrow { w } else { (w - 340.0).min(620.0) };
    let card = col().key("st/card").width(cw).gap(16.0)
        .child(text("星期五　·　11 月 14 日", 13.0, Paint::DIM).font(Font::Mono))
        .child(text_lh("今天，晴", 64.0, 72.0, Paint::INK).font(Font::Display).weight(2.0).key("st/headline"))
        .child(row().gap(18.0).fill_width().align(AlignItems::START).wrap()
            .child(col().width(if narrow { cw } else { cw * 0.55 }).child(picture(SKY, ratio(SKY)).key("st/pic").pic_fx(super::STYLE_SLOT)))
            .child(col().grow(1.0).gap(10.0).min_width(180.0)
                .child(text_lh("傍晚六点日落。风从东北来，夜里降到九度。适合散步，记得带一件外套。", 15.0, 25.0, Paint::TEXT))
                .child(row().gap(10.0).wrap()
                    .child(button_as("st/remind", "提醒我", Variant::Accent, Msg::Press))
                    .child(button_as("st/share", "分享", Variant::Default, Msg::Press)))
                .child(switch("st/rain", "下雨时通知", g.toggles[6], None, Msg::Toggle(6)).width(200.0))));
    let mut r = row().key("st/page").gap(28.0).align(AlignItems::START).width(w);
    if narrow { r = col().key("st/page").gap(20.0).width(w); }
    r.child(card).child(col().gap(10.0).child(text("风格（← → 切换）", 12.0, Paint::DIM)).child(grid))
}

// ── The page around each scene, and the list ─────────────────────────────────────────────────────

/// A scene, full-window: its title (which flows from the list), the stage, and the strip of numbers.
pub fn page(g: &Gallery, i: usize) -> El<Msg> {
    let narrow = g.width < 720.0;
    let pad = if narrow { 14.0 } else { 36.0 };
    let w = g.width - pad * 2.0;
    let strip_h = if !g.sc.strip { 44.0 } else if narrow { 150.0 } else { 56.0 };
    let h = (g.height - 52.0 - 64.0 - strip_h - 24.0).max(260.0);
    let (_, title, desc, _) = SCENES[i];
    let stage = match i {
        0 => opening(g, w, h),
        1 => reading(g, w, h),
        2 => printing(g, w, h),
        3 => morph(g, w, h),
        4 => xray(g, w, h),
        5 => sound(g, w, h),
        _ => styles_scene(g, w, h),
    };
    // Reading is long; the others fit the window.
    let mut body = col().key(format!("scene/{i}/body")).fill_width().grow(1.0).pad(pad, 0.0).align(AlignItems::CENTER);
    if i == 1 || narrow { body = body.scroll().bar().shrink(); }
    col().key(format!("scene/{i}")).fill_width().fill_height().gap(0.0)
        .child(row().key(format!("scene/{i}/head")).fill_width().pad(pad, 14.0).gap(14.0).align(AlignItems::BASELINE).wrap()
            .child(text(format!("{:02}", i + 1), 13.0, Paint::DIM).font(Font::Mono))
            .child(text(title, 22.0, Paint::INK).font(Font::Display).flow(title_flow(i)).key(format!("scene/{i}/title")))
            .child(text(desc, 13.0, Paint::DIM).grow(1.0).shrink()))
        .child(body.child(stage))
        .child(strip(g, i, pad))
}

fn strip(g: &Gallery, i: usize, pad: f32) -> El<Msg> {
    let mut r = row().key(format!("scene/{i}/strip")).fill_width().pad(pad, 10.0).gap(14.0).wrap().align(AlignItems::CENTER)
        .child(button_as(format!("scene/{i}/strip-toggle"), if g.sc.strip { "收起参数" } else { "参数" }, Variant::Quiet, Msg::Scene(SMsg::Strip)));
    if g.sc.strip {
        for (j, q) in PARAMS[i].iter().enumerate() {
            let t = Target::Scene(i, j);
            r = r.child(number(&format!("scene/{i}/p{j}"), q.name, g.sc.get(i, j), q.spec, g.draft(t), move |e: Edit| Msg::Num(t, e)));
        }
        r = r.child(space(0.0, 0.0).grow(1.0)).child(button_as(format!("scene/{i}/reset"), "重置", Variant::Default, Msg::Scene(SMsg::Reset(i))));
    }
    r
}

/// `#scenes`: the seven, as cards that open into their scene (the title flies there).
pub fn list(g: &Gallery) -> El<Msg> {
    let narrow = g.width < 720.0;
    let cw = if narrow { g.width - 36.0 } else { ((g.width - 220.0 - 80.0 - 40.0) / 3.0).clamp(240.0, 320.0) };
    let mut grid = row().key("scenes/grid").gap(20.0).wrap().fill_width();
    for (i, (_, title, desc, st)) in SCENES.iter().enumerate() {
        grid = grid.child(col().key(format!("scenes/card{i}")).width(cw).gap(10.0).pad(18.0, 16.0).radius(10.0).border(1.0, Paint::FAINT)
            .on_press(Msg::Go(Page::Scene(i as u8)))
            .child(thumb(i, cw - 36.0))
            .child(row().gap(10.0).align(AlignItems::BASELINE)
                .child(text(format!("{:02}", i + 1), 12.0, Paint::DIM).font(Font::Mono))
                .child(text(*title, 22.0, Paint::INK).font(Font::Display).flow(title_flow(i)).key(format!("scenes/title{i}"))))
            .child(text_lh(*desc, 13.0, 21.0, Paint::DIM))
            .child(text(format!("默认风格：{}", styles()[*st].name), 11.0, Paint::DIM).font(Font::Mono)));
    }
    col().key("scenes/page").fill_width().gap(18.0)
        .child(text_lh("场景", 34.0, 44.0, Paint::INK).font(Font::Display))
        .child(text_lh("七个用这个框架本身做成的场景。每一个都从安静的页面开始，用指针或键盘操作；Esc 回到这里。", 14.0, 22.0, Paint::DIM))
        .child(grid)
}

/// A small still for each card, made of the same parts as its scene.
fn thumb(i: usize, w: f32) -> El<Msg> {
    let h = 110.0;
    let k = |s: &str| format!("scenes/t{i}/{s}");
    let base = col().key(k("box")).width(w).height(h);
    match i {
        0 => base.center().child(text_lh("● ● ● ● ● ● ●\n● ● 点 ● ● ● ●\n● ● ● ● ● ● ●", 12.0, 26.0, Paint::DIM)),
        1 => base.child(text_lh("文字沿着圆的边缘排开，每一行只占用没有被挡住的部分，留出一条均匀的缝。文字沿着圆的边缘排开，每一行只占用没有被挡住的部分。", 11.0, 18.0, Paint::DIM).fill_width()
                .around(Around::new(vec![Silhouette::circle(w * 0.5, 50.0, 34.0)], 6.0)))
            .child(space(68.0, 68.0).radius(34.0).fill(Paint::ACCENT).absolute(Some(w * 0.5 - 34.0), Some(16.0), None, None)),
        2 => base.child(picture(SKY, ratio(SKY)).key(k("pic")).pic_fx(super::STYLE_SLOT).loose(1.0).width(w).height(h)),
        3 => base.child(row().gap(12.0).align(AlignItems::CENTER).height(h)
            .child(text("山", 64.0, Paint::INK).font(Font::Display))
            .child(text("→", 20.0, Paint::DIM))
            .child(row().height(40.0).pad(18.0, 0.0).radius(20.0).fill(Paint::ACCENT).center().child(text("出发", 15.0, Paint::GROUND)))),
        4 => base.child(space(w * 0.5, 80.0).radius(8.0).border(1.0, Paint::DIM).absolute(Some(10.0), Some(24.0), None, None))
            .child(space(w * 0.5, 80.0).radius(8.0).border(1.0, Paint::ACCENT).absolute(Some(w * 0.25), Some(14.0), None, None))
            .child(space(w * 0.5, 80.0).radius(8.0).fill(Paint::tone(0.1)).border(1.0, Paint::INK).absolute(Some(w * 0.45), Some(4.0), None, None)),
        5 => (0..16).fold(row().key(k("bars")).height(h).align(AlignItems::END).gap(4.0), |r, b| {
                let v = 0.2 + 0.75 * (((b as f32) * 1.3).sin() * 0.5 + 0.5) * (1.0 - b as f32 / 20.0);
                r.child(space(((w - 64.0) / 16.0).max(2.0), (v * h).round()).fill(if b < 3 { Paint::ACCENT } else { Paint::DIM }))
            }),
        _ => (0..12).fold(row().key(k("sw")).wrap().gap(6.0).width(w), |r, b| r.child(space(((w - 30.0) / 6.0).max(4.0), 46.0).radius(4.0).fill(Paint::hue((b % 3) as u8).alpha(0.35 + 0.05 * (b / 3) as f32)))),
    }
}
