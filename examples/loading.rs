//! Recipe: a list that loads (docs/recipes/loading.md). The title and the button draw at once; the rows
//! are the real rows all along — hidden for the first `skeleton_after_ms`, then their own skeleton
//! (`El::skeleton`), then, when the data comes, the content row by row `swap_stagger_ms` apart, in the
//! same boxes. The tick runs only at the times something changes (`Loading::next_change`).
use lab::el::{El, Font, Paint, col, row, space, text};
use lab::text::Face;
use lab::ui::Ui;
use lab::widgets::{Loading, Show, Variant, button_as};

pub struct App { pub loading: Loading, pub latency_ms: f32, pub now: f32 }

#[derive(Clone, Debug, PartialEq)]
pub enum Msg { Reload }

pub const ROWS: [(&str, &str); 5] = [
    ("晨间的港口", "三张照片 · 上周"), ("一份长一些的读书笔记", "两千字"), ("会议记录", "昨天"), ("旅行清单", "十二项"), ("草图", "一张图"),
];

/// Row `i` as it is when loaded; the loading state decides how it is drawn.
pub fn row_el(i: usize) -> El<Msg> {
    let (title, meta) = ROWS[i];
    row().key(format!("row{i}")).gap(12.0).height(52.0)
        .child(space(40.0, 40.0).radius(6.0).fill(Paint::hue((i % 3) as u8)).key(format!("row{i}/pic")))
        .child(col().gap(4.0).child(text(title, 15.0, Paint::TEXT).key(format!("row{i}/title"))).child(text(meta, 12.0, Paint::DIM).key(format!("row{i}/meta"))))
}

pub fn view(s: &App) -> El<Msg> {
    let mut list = col().key("rows").gap(8.0);
    for i in 0..ROWS.len() {
        list = list.child(match s.loading.show(i, s.now) {
            Show::Content => row_el(i),
            Show::Skeleton => row_el(i).skeleton(),
            Show::Nothing => row_el(i).hidden_until_morphed(true),
        });
    }
    col().key("page").pad(24.0, 24.0).gap(16.0)
        .child(row().gap(16.0).child(text("最近", 22.0, Paint::INK).font(Font::Display)).child(button_as("reload", "重新载入", Variant::Default, Msg::Reload)))
        .child(list)
}

pub fn update(s: &mut App, m: Msg) {
    match m { Msg::Reload => s.loading.start(s.now) }
}

pub fn ui(faces: Vec<Face>, w: u32, h: u32, scale: f32) -> Ui<App, Msg> {
    Ui::new(App { loading: Loading::default(), latency_ms: 900.0, now: 0.0 }, view, update, faces, w, h, scale)
        .on_tick(|s, t| {
            s.now = t;
            // The data comes `latency_ms` after the request (a stand-in for the network).
            if let Some(st) = s.loading.started() && s.loading.arrived().is_none() && t >= st + s.latency_ms / 1000.0 { s.loading.arrive(t); }
            true
        })
        .on_wake(|s, t| {
            let data = s.loading.started().filter(|_| s.loading.arrived().is_none()).map(|st| st + s.latency_ms / 1000.0);
            match (s.loading.next_change(ROWS.len(), t), data) { (Some(a), Some(b)) => Some(a.min(b)), (a, b) => a.or(b) }
        })
}
