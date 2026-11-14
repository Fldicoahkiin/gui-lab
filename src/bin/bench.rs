//! CPU timings without a GPU (DESIGN.md §1.13): update / layout and scene build for
//!  1. the gallery's button page, pressing the button (each press is a message and a new view);
//!  2. a list of 10,000 rows (text and a small switch), scrolled by the wheel;
//!  3. a 5,000-character article flowing around a disc that moves (every step reflows).
//!
//! Each line is the mean over the runs, ms. Numbers are only comparable on one machine.
//!
//! usage: bench [runs]
use lab::el::{El, Paint, col, row, space, text, text_lh};
use lab::gallery::{self, Msg, Page};
use lab::ui::Ui;
use lab::wrap::{Around, Silhouette};
use std::time::Instant;

fn fonts() -> Vec<lab::text::Face> { lab::text::load_bundled("assets").expect("fonts") }

fn time(n: usize, mut f: impl FnMut(usize)) -> f64 {
    let t0 = Instant::now();
    for i in 0..n { f(i); }
    t0.elapsed().as_secs_f64() * 1000.0 / n as f64
}

// ── A plain list of 10,000 rows: every row is in the tree ────────────────────────────────────────
#[derive(Default)]
struct Long { on: Vec<bool> }
fn long_view(s: &Long) -> El<usize> {
    let mut list = col().key("long").fill_width().fill_height().scroll().bar();
    for i in 0..10_000 {
        let on = s.on.get(i).copied().unwrap_or(i % 3 == 0);
        list = list.child(row().key(format!("r{i}")).height(32.0).pad(12.0, 0.0).gap(12.0).align(taffy::AlignItems::CENTER)
            .child(text(format!("第 {i} 行 · row {i}"), 14.0, Paint::TEXT).grow(1.0))
            .child(space(30.0, 16.0).radius(8.0).fill(if on { Paint::ACCENT } else { Paint::FAINT }).on_press(i)));
    }
    col().key("root").child(list)
}

// ── An article around a moving disc ──────────────────────────────────────────────────────────────
struct Article { x: f32 }
fn article_text() -> String {
    let unit = "文字绕开障碍物的真实轮廓排，每一行只占用这一条带子里没有被挡住的部分。Text flows around the true outline, not its box. ";
    let mut s = String::new();
    while s.chars().count() < 5000 { s.push_str(unit); }
    s.chars().take(5000).collect()
}
thread_local! { static ARTICLE: String = article_text(); }
fn article_view(s: &Article) -> El<()> {
    let a = Around::new(vec![Silhouette::circle(s.x, 220.0, 90.0)], 8.0);
    col().key("root").pad(24.0, 24.0).child(ARTICLE.with(|t| text_lh(t.clone(), 15.0, 24.0, Paint::TEXT).key("body").fill_width().around(a)))
}

fn main() {
    let runs: usize = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(40);
    let (w, h) = (1280u32, 800u32);

    // 1. Gallery button page: press the live button.
    let mut ui = gallery::new_ui(fonts(), w, h, 1.0);
    gallery::update(&mut ui.state, Msg::Go(Page::from_hash("button").expect("button page")));
    ui.relayout();
    ui.tick(1.0);
    let k = ui.nodes().iter().find(|n| n.pressable() && n.key.contains("live")).map(|n| n.key.clone()).expect("the live button");
    let (x, y) = ui.node(&k).expect("button").rect.center();
    let ms = time(runs, |i| { let t = 1.0 + i as f32 * 0.02; ui.tick(t); ui.press(x, y); ui.release(x, y); });
    println!("button page, press      {ms:8.3} ms   (update, view, layout and scene per press)");

    // 2. 10,000 rows, the wheel.
    let mut ui = Ui::new(Long::default(), long_view, |s: &mut Long, i| { if s.on.len() <= i { s.on.resize(i + 1, false); } s.on[i] = !s.on[i]; }, fonts(), w, h, 1.0);
    ui.tick(1.0);
    let ms = time(runs, |i| { ui.wheel(400.0, 400.0, if (i / 20) % 2 == 0 { 120.0 } else { -120.0 }); });
    println!("10k plain rows, wheel   {ms:8.3} ms   (per wheel step)");
    let first = time(3, |i| { ui.state.on.clear(); ui.state.on.resize(i + 1, true); ui.relayout(); });
    println!("10k plain rows, relayout{first:8.3} ms   (a new view of all rows)");
    gallery_long(runs);

    // 3. The article: the disc moves, everything reflows.
    let mut ui = Ui::new(Article { x: 200.0 }, article_view, |_, _| {}, fonts(), w, h, 1.0);
    ui.tick(1.0);
    let ms = time(runs.min(30), |i| { ui.state.x = 160.0 + (i % 40) as f32 * 18.0; ui.relayout(); });
    println!("5000-char article reflow{ms:8.3} ms   (the obstacle moves; layout, line breaks, scene)");
}

/// The gallery's own 10,000-row page, if there is one: scrolled by the wheel.
fn gallery_long(runs: usize) {
    let Some(p) = Page::from_hash("list-10k") else { println!("10k virtual rows        (no such page)"); return };
    let mut ui = gallery::new_ui(fonts(), 1280, 800, 1.0);
    gallery::update(&mut ui.state, Msg::Go(p));
    ui.relayout();
    ui.tick(1.0);
    let ms = time(runs, |i| { ui.wheel(700.0, 450.0, if (i / 20) % 2 == 0 { 120.0 } else { -120.0 }); });
    println!("10k virtual rows, wheel {ms:8.3} ms   (gallery page 一万行, per wheel step)");
}
